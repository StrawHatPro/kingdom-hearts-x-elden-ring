//! (BC) THE HUD'S BATTLE LOOK FOLLOWS THE LEADER'S COMBAT STATE (THE USER 2026-11-02: "make the HUD change to this colour
//! during combat, and make sure Sora's portrait changes to his battle portrait when he is party leader"). The real `Hud`
//! (UI pack; skipped without it) is driven as the hosts drive it: Sora stepped with a host that reports engaged enemies
//! (kh-core's battle flag, KH3's 1.0 s linger), a friend leading with his slot's battle flag. KH3's rule (0x141631D90):
//! the command menu takes `cmdYellow` (the orange-gold battle menu) while the battle flag is on, `cmdRed` in a special
//! battle (the forced flag), `cmdBlue` otherwise; the portraits take their `battle` look from their pawn's own flag.
//! Pictures: build/sora_tmp/hud/bc_*.png (the whole stage, `_cmd` and `_status` close-ups).

use kh_core::host::{NullHost, TargetInfo};
use kh_core::test_world::TestWorld;
use kh_core::{HitRecord, HitResult, HurtShape, Input, PadButtons, Pose, Sora, TargetId, Targets, Vec3, SORA_PARAMS};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::party::{PartyChara, PartyHudSlot};

const DT: f32 = 1.0 / 60.0;
const MAIN_TOP: &str = "cmdMenu.cmdMainTop";

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

/// A host whose only answer is the engaged-enemy count.
struct Engaged(u32);
impl Targets for Engaged {
    fn enumerate(&self, _out: &mut Vec<TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
    fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
        HitResult::default()
    }
    fn engaged_count(&self) -> u32 {
        self.0
    }
    // (AUDIT-B) no bones here: a rider attached to a hit follows the target's origin and yaw
    fn hit_bone(&self, _id: kh_core::TargetId, _at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: kh_core::TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct Rig {
    hud: Hud,
    fonts: kh_ui::Fonts,
    sora: Sora,
    world: TestWorld,
    /// The status movie's calls of the frames run so far (`setFaceLook(slot, look)`, `loadFace(..)`).
    calls: Vec<String>,
}

impl Rig {
    fn new() -> Option<Rig> {
        let Ok((hud, fonts)) = Hud::load(&root().join("build/sora_pack/ui")) else {
            eprintln!("no UI pack: skipped");
            return None;
        };
        let origin = Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
        Some(Rig { hud, fonts, sora, world: TestWorld::floor(), calls: Vec::new() })
    }

    /// `n` frames: Sora stepped with `engaged` enemies (his battle flag, kh-core `pawn_tick`), then the HUD's frame.
    fn frames(&mut self, engaged: u32, n: usize, words: &[&str]) {
        for k in 0..n {
            let mut null = NullHost::default();
            let mut targets = Engaged(engaged);
            let mut host = null.host(&self.world);
            host.targets = &mut targets;
            let out = self.sora.step_hosted(DT, &Input::default(), &mut host);
            let state = out.state;
            let snapshot = self.sora.ui_snapshot();
            let words: Vec<String> = if k == 0 { words.iter().map(|w| w.to_string()).collect() } else { Vec::new() };
            self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state, step_dt: DT, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &words, dt: DT, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
            self.calls.extend(self.hud.take_status_calls());
        }
    }

    fn colour(&self) -> String {
        self.hud.movie("HudCommand").unwrap().current_label(MAIN_TOP).unwrap_or_default().to_string()
    }

    /// The last look sent to a status slot (0 normal, 1 battle, 2 dying, 3 death).
    fn look(&self, slot: usize) -> Option<i32> {
        let head = format!("setFaceLook({slot}, ");
        self.calls.iter().rev().find_map(|c| c.strip_prefix(&head).and_then(|r| r.trim_end_matches(')').parse().ok()))
    }

    /// The face movie last loaded into a status slot.
    fn face(&self, slot: usize) -> Option<String> {
        let head = format!("loadFace({slot}, ");
        self.calls.iter().rev().find_map(|c| c.strip_prefix(&head).map(|r| r.split("Some(\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string()))
    }

    /// The command and status movies on one stage, written as `name` with close-ups; returns the status close-up.
    fn picture(&self, name: &str) -> Vec<u8> {
        let (w, h) = (1920usize, 1080usize);
        let dir = root().join("build/sora_tmp/hud");
        std::fs::create_dir_all(&dir).ok();
        let mut canvas = kh_ui::Canvas::new(w, h);
        let mut r = kh_ui::Renderer::new(self.fonts.clone());
        let (stage, _) = kh_host::hud_layers::stage_root(w as f32, h as f32);
        for mv in ["HudCommand", "HudStatus"] {
            r.draw(&mut canvas, self.hud.movie(mv).unwrap(), &stage);
        }
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, y| [0.30 + 0.08 * (y as f32 / 1080.0), 0.27, 0.24], &mut rgba);
        kh_ui::def::write_png(&dir.join(format!("{name}.png")), w as u32, h as u32, &rgba).unwrap();
        let mut status = Vec::new();
        for (tag, (x0, y0, cw, ch), z) in [("cmd", (0usize, 780usize, 500usize, 300usize), 2usize), ("status", (1440, 780, 480, 300), 2)] {
            let mut out = vec![0u8; cw * ch * z * z * 4];
            for y in 0..ch * z {
                for x in 0..cw * z {
                    let i = ((y0 + y / z) * w + x0 + x / z) * 4;
                    let o = (y * cw * z + x) * 4;
                    out[o..o + 4].copy_from_slice(&rgba[i..i + 4]);
                }
            }
            kh_ui::def::write_png(&dir.join(format!("{name}_{tag}.png")), (cw * z) as u32, (ch * z) as u32, &out).unwrap();
            if tag == "status" {
                status = out;
            }
        }
        status
    }
}

/// Pixels of the portrait (inside the gauge ring of the `_status` close-up, 960 wide) that differ between two pictures.
fn differing(a: &[u8], b: &[u8]) -> usize {
    let (w, x0, x1, y0, y1) = (960usize, 660usize, 820usize, 280usize, 480usize);
    (y0..y1).flat_map(|y| (x0..x1).map(move |x| (y * w + x) * 4)).filter(|i| (0..3).any(|c| a[i + c].abs_diff(b[i + c]) > 24)).count()
}

fn slot(chara: PartyChara, in_battle: bool) -> PartyHudSlot {
    PartyHudSlot { chara, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle, focus: None }
}

#[test]
fn soras_combat_turns_the_menu_to_kh3s_battle_colour_and_his_portrait_to_his_battle_face_with_the_linger() {
    let Some(mut rig) = Rig::new() else { return };
    rig.frames(0, 90, &[]);
    assert_eq!(rig.colour(), "cmdBlue", "out of combat: KH3's field menu");
    assert_eq!(rig.face(0).as_deref(), Some("FacePcExSora01"), "Sora leads: his own portrait in the main slot");
    assert_eq!(rig.look(0), Some(0), "his normal face");
    let field = rig.picture("bc_sora_field");
    // an enemy engages: the flag goes on at once, the menu turns orange-gold, his face to the battle face
    rig.frames(1, 90, &[]);
    assert!(rig.sora.actions().battle);
    assert_eq!(rig.colour(), "cmdYellow", "in combat: KH3's battle menu (mode 1)");
    assert_eq!(rig.look(0), Some(1), "his battle face");
    let battle = rig.picture("bc_sora_battle");
    assert!(differing(&field, &battle) > 400, "the portrait (and the gauges' corner) look different in battle");
    // the last enemy gone: KH3's 1.0 s linger keeps both
    rig.frames(0, 45, &[]);
    assert!(rig.sora.actions().battle);
    assert_eq!((rig.colour().as_str(), rig.look(0)), ("cmdYellow", Some(1)), "0.75 s after: still in battle");
    rig.frames(0, 30, &[]);
    assert!(!rig.sora.actions().battle);
    assert_eq!((rig.colour().as_str(), rig.look(0)), ("cmdBlue", Some(0)), "1.25 s after: the field look again");
    rig.frames(0, 60, &[]);
    let back = rig.picture("bc_sora_field_again");
    assert!(differing(&field, &back) < 40, "back to the field portrait: {}", differing(&field, &back));
}

#[test]
fn the_special_battle_flag_gives_kh3s_red_menu_and_the_test_word_still_fixes_the_colour() {
    let Some(mut rig) = Rig::new() else { return };
    rig.frames(0, 60, &[]);
    rig.sora.force_battle(true);
    rig.frames(0, 60, &[]);
    assert_eq!((rig.colour().as_str(), rig.look(0)), ("cmdRed", Some(1)), "a special battle: mode 2");
    rig.picture("bc_sora_special");
    rig.sora.force_battle(false);
    rig.frames(0, 90, &[]);
    assert_eq!(rig.colour(), "cmdBlue");
    // `hud-cmd battle 1` fixes the battle colour whatever the flag says, `battle live` gives it back
    rig.frames(0, 30, &["battle 1"]);
    assert_eq!((rig.colour().as_str(), rig.look(0)), ("cmdYellow", Some(1)));
    rig.frames(0, 30, &["battle live"]);
    assert_eq!((rig.colour().as_str(), rig.look(0)), ("cmdBlue", Some(0)));
}

#[test]
fn a_friend_leading_drives_the_colour_by_his_own_flag_and_soras_face_stays_out_of_the_main_slot() {
    let Some(mut rig) = Rig::new() else { return };
    rig.hud.set_party(&[slot(PartyChara::Donald, false), slot(PartyChara::Goofy, false)]);
    rig.hud.set_leader(PartyChara::Donald, true);
    rig.frames(0, 120, &[]);
    assert_eq!(rig.face(0).as_deref(), Some("FaceNpcExDonald"), "Donald leads: his face in the main slot");
    assert_eq!(rig.colour(), "cmdBlue");
    // Sora's own fight alone (AI Sora engaged, the party flag off) does not colour Donald's menu
    rig.frames(1, 30, &[]);
    assert_eq!(rig.colour(), "cmdBlue", "the LEADER's flag drives it");
    rig.frames(0, 90, &[]);
    // the party's flag on (kh-partyhost `battle_step` feeds the leader's slot): Donald's menu and face
    rig.hud.set_party(&[slot(PartyChara::Donald, true), slot(PartyChara::Goofy, true)]);
    rig.frames(0, 60, &[]);
    assert_eq!((rig.colour().as_str(), rig.look(0)), ("cmdYellow", Some(1)), "Donald in combat: the battle menu and his battle face");
    rig.picture("bc_donald_battle");
    assert_ne!(rig.face(0).as_deref(), Some("FacePcExSora01"), "Sora's leader portrait only while he leads");
    rig.hud.set_party(&[slot(PartyChara::Donald, false), slot(PartyChara::Goofy, false)]);
    rig.frames(0, 30, &[]);
    assert_eq!((rig.colour().as_str(), rig.look(0)), ("cmdBlue", Some(0)));
    // Sora leads again: his portrait back in the main slot, and his flag drives the colour and his battle face
    rig.hud.set_leader(PartyChara::Sora, true);
    rig.frames(1, 120, &[]);
    assert_eq!(rig.face(0).as_deref(), Some("FacePcExSora01"));
    assert_eq!((rig.colour().as_str(), rig.look(0)), ("cmdYellow", Some(1)));
    rig.picture("bc_sora_leads_again_battle");
}
