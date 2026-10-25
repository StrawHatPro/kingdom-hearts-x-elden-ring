//! (PFY) THE USER'S BUG, OFFLINE: "pressing Y on the party formation menu to change leader does nothing" (the Elden Ring
//! party build). The Elden Ring path of a pad press to the save point's Party Formation screen, frame by frame as the
//! DLL runs it, with a scripted pad:
//! - sora.rs `pre_physics`: the save point's Y rule (`savepoint_rules::y_rule`, Sora standing at a discovered grace,
//!   Elden Ring's "Rest" prompt up) and the per-press Y routing (`YOwner::at_down` / `hides_y`), which takes Y out of
//!   the buttons the HUD reads when the press is not Sora's;
//! - hud.rs `HudHost::input` / `frame`: the menus' pad (`MenuInput`), `partyui::hud_frame` (the party into the HUD),
//!   kh-host `Hud::frame` + `apply_actions`, `partyui::hud_after` (the screen's change into the mailbox);
//! - the party's step while the world is not paused (`route`, `step`, `after_step`, `partyui::sync`), as S1's glue.
//!
//! The screen is the real one (kh-host `Hud` with the UI pack: `build/sora_pack/ui` of the workspace, or `KH_UI_PACK`;
//! skipped without it).

use glam::Vec3;
use kh_core::{Input, PadButton, PadButtons, PadState, Sora, TargetId, UiSnapshot};
use kh_host::formation::Formation;
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::menu::MenuInput;
use kh_host::party::PartyChara;
use kh_partyhost::PartyRuntime;

use crate::control::Control;
use crate::partyui;
use crate::savepoint_rules::{y_rule, YIn};
use crate::sora::YOwner;

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

struct NoEnemies;
impl kh_core::Targets for NoEnemies {
    fn enumerate(&self, _out: &mut Vec<kh_core::host::TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<kh_core::host::TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<kh_core::host::HurtShape>) {}
    fn apply_hit(&mut self, _hit: &kh_core::HitRecord) -> kh_core::host::HitResult {
        kh_core::host::HitResult::default()
    }
}
impl kh_partyhost::PartyTargets for NoEnemies {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

/// Sora stands in the Save-command capsule of a discovered grace, Elden Ring's prompt up, nothing of the game's busy:
/// what the save point's `frame` returns for Y (`y.ours`).
fn at_grace() -> bool {
    y_rule(YIn { sit: false, menu_key: true, at_point: true, prompt_near: false, near: true, discovered: true, allowed: true, prompt: true, game_menu: false, resting: false, busy: false, on_floor: true, alive: true }).ours
}

struct Rig {
    hud: Hud,
    sora: Sora,
    rt: PartyRuntime,
    world: kh_core::test_world::TestWorld,
    control: Control,
    menu: MenuInput,
    held_before: PadButtons,
    y_held: bool,
    y_owner: YOwner,
    t: f64,
    /// Y presses the HUD's menus saw (FaceTop in `pressed`).
    y_seen: u32,
}

impl Rig {
    /// One game frame of the Elden Ring host: `raw` = the buttons physically down.
    fn frame(&mut self, raw: PadButtons) {
        // sora.rs: Y routed per press (the save point is asked first, then the owner at the down-edge)
        let mut held = raw;
        let menu_before = self.hud.paused(); // hud.rs: MENU_OPEN = the HUD's pause of its last frame
        let sp_y = at_grace();
        let y_raw = held.has(PadButton::FaceTop);
        if y_raw && !self.y_held {
            self.y_owner = YOwner::at_down(menu_before, sp_y, true, false);
        }
        self.y_held = y_raw;
        if y_raw && self.y_owner.hides_y() {
            held.set(PadButton::FaceTop, false);
        }
        // hud.rs HudHost::input: the menus' pad
        let pad = PadState::from_held(held, self.held_before);
        self.held_before = held;
        self.menu.step(&Input::default().with_pad(pad), 1.0 / 60.0);
        // hud.rs HudHost::frame
        let (pressed, nav) = self.menu.take();
        self.y_seen += u32::from(pressed.has(PadButton::FaceTop));
        let led = partyui::hud_frame(&mut self.hud, &UiSnapshot::default(), &self.control);
        let snapshot = led.unwrap_or_else(|| self.sora.ui_snapshot());
        self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 100, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed, nav });
        self.hud.apply_actions(&mut self.sora);
        partyui::hud_after(&mut self.hud, &self.control);
        let _ = self.hud.take_sounds();
        // sora.rs: no steps while a menu of ours holds the pause; else the party's step (S1's glue) and S6's sync
        if !self.hud.paused() {
            self.party_step();
        }
    }

    fn party_step(&mut self) {
        let camera = kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        let mut targets = NoEnemies;
        let input = Input::default();
        let t = self.t;
        let _ = self.rt.route(kh_partyhost::RouteIn { sora: &mut self.sora, input: &input, dt: 1.0 / 60.0, targets: &targets, camera: &camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: t });
        let out = self.sora.output();
        let mut audio = kh_core::host::audio::NullAudio;
        let mut effects = kh_core::host::effects::NullEffects;
        self.rt.step(kh_partyhost::StepIn { dt: 1.0 / 60.0, sora: &mut self.sora, out: &out, lock_on: TargetId::NONE, camera: &camera, world: &self.world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: t });
        let _ = self.rt.after_step(&NoEnemies, 1.0 / 60.0, t);
        let _ = partyui::sync(&mut self.rt, &mut self.sora, None, t);
        self.t += 1.0 / 60.0;
    }

    /// A button pressed for `down` frames, then `up` frames with nothing.
    fn press(&mut self, b: PadButton, down: u32, up: u32) {
        let mut buttons = PadButtons::NONE;
        buttons.set(b, true);
        for _ in 0..down {
            self.frame(buttons);
        }
        self.idle(up);
    }

    fn idle(&mut self, n: u32) {
        for _ in 0..n {
            self.frame(PadButtons::NONE);
        }
    }

    /// hud.rs `open_party_formation` (the save point's GraceAction::Party), then the screen's opening.
    fn open(&mut self) {
        let f = self.hud.formation().clone();
        let _ = self.hud.take_formation_change();
        assert!(self.hud.open_party_formation(&f), "the screen opens");
        self.idle(80);
        assert!(self.hud.paused(), "the Party Formation screen holds the pause");
    }
}

fn rig() -> Option<Rig> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::var_os("KH_UI_PACK").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("build/sora_pack/ui"));
    let Ok((hud, _)) = Hud::load(&dir) else {
        eprintln!("no UI pack at {}: skipped", dir.display());
        return None;
    };
    let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
    *sora.status_mut() = kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None));
    let mut rt = PartyRuntime::new(kh_partyhost::RuntimeConfig { wanted: true, test: kh_partyhost::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
    partyui::configure(&mut rt);
    let control = Control::new(std::env::temp_dir().join("kh-er-pfy-test-no-such.ctl"));
    Some(Rig { hud, sora, rt, world: kh_core::test_world::TestWorld::floor(), control, menu: MenuInput::new(), held_before: PadButtons::NONE, y_held: false, y_owner: YOwner::Sora, t: 0.0, y_seen: 0 })
}

#[test]
fn y_at_a_grace_belongs_to_the_menu_that_is_open() {
    assert!(at_grace(), "standing at the grace, the save point claims Y");
    // the press that OPENS the save menu stays the save point's (no menu of ours before it): the menu does not see it
    assert_eq!(YOwner::at_down(false, true, true, false), YOwner::SavePoint);
    assert!(YOwner::at_down(false, true, true, false).hides_y());
    // a press while a menu of ours is up (Party Formation, the camp menu, the save menu) is the menu's
    assert_eq!(YOwner::at_down(true, true, true, false), YOwner::Menu);
    assert!(!YOwner::at_down(true, true, true, false).hides_y());
    assert_eq!(YOwner::at_down(true, false, true, false), YOwner::Menu, "Elden Ring's prompt does not take it from our menu either");
    // nothing else changed
    assert_eq!(YOwner::at_down(false, false, true, false), YOwner::EldenRing);
    assert_eq!(YOwner::at_down(false, false, false, true), YOwner::EldenRing);
    assert_eq!(YOwner::at_down(false, false, false, false), YOwner::Sora);
    assert!(YOwner::EldenRing.hides_y() && !YOwner::Sora.hides_y());
}

/// THE BUG AND ITS FIX END TO END: Party Formation opened at a grace, the pad's D-pad / Y / A / B through the Elden Ring
/// path; the new leader is in force after the screen closes and stays (no revert), the HUD's formation follows; a second
/// visit makes Sora the leader again.
#[test]
fn y_makes_the_leader_on_the_party_formation_screen_in_elden_ring() {
    let _serial = partyui::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    partyui::reset();
    let Some(mut r) = rig() else { return };
    r.idle(3);
    assert!(partyui::live(&r.control), "the party is live for the screens");
    assert_eq!(r.hud.formation().leader(), PartyChara::Sora);
    assert_eq!(r.hud.formation().active(), &[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy]);

    // the save point's menu -> Party Formation
    r.open();
    // D-pad right (Donald), Y: Donald leads
    r.press(PadButton::DPadRight, 1, 6);
    r.press(PadButton::FaceTop, 4, 30);
    assert_eq!(r.y_seen, 1, "the menus saw the Y press (sora.rs routed it to the save point and hid it before the fix)");
    assert_eq!(r.hud.formation().leader(), PartyChara::Donald, "Y made Donald the leader on the screen");
    // D-pad right (Goofy), A: Goofy leaves
    r.press(PadButton::DPadRight, 1, 6);
    r.press(PadButton::FaceBottom, 3, 30);
    assert_eq!(r.hud.formation().active(), &[PartyChara::Sora, PartyChara::Donald], "A took Goofy out");
    // D-pad left (Donald, the leader), A: refused (the leader cannot leave): nothing changes
    r.press(PadButton::DPadLeft, 1, 6);
    r.press(PadButton::FaceBottom, 3, 30);
    assert_eq!((r.hud.formation().active(), r.hud.formation().leader()), (&[PartyChara::Sora, PartyChara::Donald][..], PartyChara::Donald), "the refused edit changed nothing");
    // nothing reached the runtime yet: the world is paused under the screen
    assert_eq!(r.rt.leader_chara(), PartyChara::Sora);
    // B: back (the camp menu closes, the world runs again)
    r.press(PadButton::FaceRight, 3, 80);
    assert!(!r.hud.paused(), "B closed the screen");

    // AFTER CLOSING: the runtime's leader, the HUD's feed (main slot, the leader's command menu), the members
    assert_eq!(r.rt.leader_chara(), PartyChara::Donald, "Donald leads the party");
    let (leader, sora_in, led, present, formation) = partyui::feed(|f| (f.leader, f.sora_in, f.led.is_some(), f.present.clone(), f.formation.clone())).unwrap();
    assert_eq!((leader, sora_in, led), (PartyChara::Donald, true, true), "the HUD's main slot and command menu are Donald's");
    assert_eq!(present, vec![PartyChara::Donald], "Goofy left the party");
    assert_eq!(formation.active(), &[PartyChara::Sora, PartyChara::Donald]);
    // ... and it stays (no restore / sync puts the old formation back)
    r.idle(120);
    assert_eq!(r.rt.leader_chara(), PartyChara::Donald, "still Donald 2 s later");
    assert_eq!((r.hud.formation().leader(), r.hud.formation().active()), (PartyChara::Donald, &[PartyChara::Sora, PartyChara::Donald][..]), "the HUD's formation follows the runtime");
    let record = partyui::party_record(&r.control).unwrap();
    assert_eq!(record["leader"].as_str(), Some("donald"), "the save's party record names Donald");

    // a second visit: the screen opens on Donald's party; Y on Sora (the cursor's first place) makes him lead again
    r.open();
    r.press(PadButton::FaceTop, 4, 30);
    assert_eq!(r.y_seen, 2);
    assert_eq!(r.hud.formation().leader(), PartyChara::Sora);
    r.press(PadButton::FaceRight, 3, 80);
    assert!(!r.hud.paused());
    r.idle(10);
    assert_eq!(r.rt.leader_chara(), PartyChara::Sora, "Sora leads again");
    assert_eq!(partyui::feed(|f| (f.leader, f.led.is_some())), Some((PartyChara::Sora, false)));
    let check = Formation::new(&[PartyChara::Sora, PartyChara::Donald], PartyChara::Sora).unwrap();
    assert_eq!(r.hud.formation(), &check);
    partyui::reset();
}
