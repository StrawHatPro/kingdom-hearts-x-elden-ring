//! (GOOFY) GOOFY'S HUD AND MENUS, OFFLINE (documentation/GOOFY_OVERHAUL.md 4.3, 5, 7, 8, 11; build/sora_tmp/party/GOOFY/PLAN.md R19,
//! R20): the real HUD (kh-host `Hud` + the UI pack, skipped without it) fed as both hosts feed it. Pictures:
//! build/sora_tmp/party/GOOFY/shots_ui/ (the HudRight, HudTarget, HudCommand and HudStatus movies on one 1920 x 1080 stage, the camp
//! menu alone).

use kh_core::command::leader::goofy as ids;
use kh_core::input::PadButton;
use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView, View};
use kh_host::party::{GoofyHudView, GuardValues, PartyChara, PartyHudSlot, PerilCue, PerilKind, ResolveValues, StaggerValues};
use kh_party::pad::menu::FriendMenu;
use kh_party::{Chara, FriendStatus};
use kh_partyhost::custom::gof_look;
use kh_partyhost::{PartyRuntime, PartyTest, RuntimeConfig};

const LOCK: TargetId = TargetId(7);
const OTHER: TargetId = TargetId(8);
const BEHIND: TargetId = TargetId(9);
const ROW: u32 = 20;
const CORNER: TargetId = TargetId(30);

struct Dummy;
impl HudTargets for Dummy {
    fn target(&self, id: TargetId) -> Option<TargetView> {
        match id {
            LOCK => Some(TargetView { center: Vec3::new(1100.0, 520.0, 0.0), hp: 900.0, hp_max: 1000.0, large: false }),
            OTHER => Some(TargetView { center: Vec3::new(600.0, 480.0, 0.0), hp: 400.0, hp_max: 500.0, large: false }),
            BEHIND => Some(TargetView { center: Vec3::new(-900.0, 480.0, 0.0), hp: 400.0, hp_max: 500.0, large: false }),
            // (Danger Sense) a row of enemies, one per kind of attack, and one past the stage's bottom-right corner
            TargetId(n) if (ROW..ROW + 7).contains(&n) => Some(TargetView { center: Vec3::new(210.0 + 250.0 * (n - ROW) as f32, 720.0, 0.0), hp: 400.0, hp_max: 500.0, large: false }),
            CORNER => Some(TargetView { center: Vec3::new(2600.0, 1700.0, 0.0), hp: 400.0, hp_max: 500.0, large: false }),
            _ => None,
        }
    }

    fn top(&self, id: TargetId) -> Option<Vec3> {
        self.target(id).map(|t| t.center - Vec3::Y * 110.0)
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn out_dir() -> std::path::PathBuf {
    let d = root().join("build/sora_tmp/party/GOOFY/shots_ui");
    std::fs::create_dir_all(&d).ok();
    d
}

fn floor(y: usize) -> [f32; 3] {
    let t = y as f32 / 1080.0;
    [0.30 + 0.08 * t, 0.27 + 0.05 * t, 0.24]
}

struct Rig {
    hud: Hud,
    fonts: kh_ui::Fonts,
    sora: Sora,
    menu: FriendMenu,
    status: FriendStatus,
    held: PadButtons,
    lock: bool,
    view: Option<GoofyHudView>,
    guard: GuardValues,
    words: Vec<String>,
    /// The view of the frames: the stage itself, or a test's camera.
    camera: glam::Mat4,
}

impl Rig {
    fn new() -> Option<Rig> {
        let Ok((mut hud, fonts)) = Hud::load(&root().join("build/sora_pack/ui")) else {
            eprintln!("no UI pack: skipped");
            return None;
        };
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
        let status = FriendStatus::new(Chara::Goofy, 0);
        let slots = [
            PartyHudSlot { chara: PartyChara::Goofy, hp: 300, hp_max: 376, mp: 80, mp_max: 136, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
            PartyHudSlot { chara: PartyChara::Donald, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
        ];
        hud.set_party(&slots);
        hud.set_leader(PartyChara::Goofy, true);
        Some(Rig { hud, fonts, sora, menu: FriendMenu::default(), status, held: PadButtons::NONE, lock: true, view: Some(GoofyHudView { look: gof_look(), ..GoofyHudView::default() }), guard: GuardValues { value: 110.0, max: 150.0, broken: false, regenerating: false, rearm_at: 0.5 }, words: Vec::new(), camera: glam::Mat4::orthographic_rh(0.0, 1920.0, 1080.0, 0.0, -1.0, 1.0) })
    }

    fn gv(&mut self) -> &mut GoofyHudView {
        self.view.as_mut().unwrap()
    }

    /// `n` frames, the first with `buttons` pressed.
    fn frames(&mut self, buttons: &[PadButton], n: usize) {
        for k in 0..n {
            let now = if k == 0 { buttons.iter().fold(PadButtons::NONE, |p, b| p.with(*b)) } else { PadButtons::NONE };
            let pad = kh_core::input::PadState::from_held(now, self.held);
            self.held = now;
            let _ = self.menu.tick(Chara::Goofy, &self.status, &pad, false);
            let mut snapshot = self.sora.ui_snapshot();
            snapshot.command = self.menu.snapshot(Chara::Goofy, &self.status);
            snapshot.lock_on = if self.lock { LOCK } else { TargetId::NONE };
            snapshot.step = self.sora.ui_snapshot().step + k as u64 + 1;
            let p = kh_party::guard::GP;
            let look = kh_host::party::GuardLook { fill: p.hud_color_fill, shine: p.hud_color_shine, loss: p.hud_color_loss, regen: p.hud_color_regen, broken: p.hud_color_broken, blink_hz: p.hud_broken_blink, offset: (p.hud_offset_x, p.hud_offset_y) };
            self.hud.set_guard(self.view.as_ref().map(|_| (self.guard, look)));
            self.hud.set_goofy(self.view.clone());
            let words = std::mem::take(&mut self.words);
            let view = View { clip_from_world: self.camera, size: (1920.0, 1080.0) };
            self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &Dummy, to_view: same, view: Some(view), words: &words, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
            self.hud.apply_actions(&mut self.sora);
        }
    }

    fn menu_keys(&mut self, keys: &[&str]) {
        for key in keys {
            self.words.push(format!("menu {key}"));
            self.frames(&[], 14);
        }
    }

    /// The stage with every HUD movie over a mid-tone floor; written as `name`, with close-ups.
    fn picture(&self, name: &str) -> Vec<u8> {
        let (w, h) = (1920usize, 1080usize);
        let mut canvas = kh_ui::Canvas::new(w, h);
        let mut r = kh_ui::Renderer::new(self.fonts.clone());
        for mv in ["HudTarget", "HudRight", "HudCommand", "HudStatus"] {
            let movie = self.hud.movie(mv).unwrap();
            let (root, _) = kh_host::hud_layers::stage_root(w as f32, h as f32);
            r.draw(&mut canvas, movie, &root);
        }
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, y| floor(y), &mut rgba);
        let _ = kh_ui::def::write_png(&out_dir().join(format!("{name}.png")), w as u32, h as u32, &rgba);
        for (tag, (x0, y0, cw, ch), z) in [("status", (1440usize, 860usize, 480usize, 220usize), 2usize), ("cmd", (0, 780, 500, 300), 2), ("right", (1100, 0, 820, 260), 2)] {
            let mut out = vec![0u8; cw * ch * z * z * 4];
            for y in 0..ch * z {
                for x in 0..cw * z {
                    let i = ((y0 + y / z) * w + x0 + x / z) * 4;
                    let o = (y * cw * z + x) * 4;
                    out[o..o + 4].copy_from_slice(&rgba[i..i + 4]);
                }
            }
            let _ = kh_ui::def::write_png(&out_dir().join(format!("{name}_{tag}.png")), (cw * z) as u32, (ch * z) as u32, &out);
        }
        rgba
    }

    /// The camp menu alone.
    fn camp_picture(&self, name: &str) {
        let (w, h) = (1920usize, 1080usize);
        let (root, _) = kh_host::hud_layers::stage_root(w as f32, h as f32);
        let mut canvas = kh_ui::Canvas::new(w, h);
        kh_ui::Renderer::new(self.fonts.clone()).draw(&mut canvas, self.hud.movie("CampMenu").unwrap(), &root);
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| [0.1, 0.1, 0.12], &mut rgba);
        let _ = kh_ui::def::write_png(&out_dir().join(format!("{name}.png")), w as u32, h as u32, &rgba);
    }
}

/// Pixels of `path` of `movie` over black: (mean colour, count).
fn colour_of(rig: &Rig, movie: &str, path: &str) -> ([f32; 3], f32) {
    let draw = |m: &kh_ui::Movie| {
        let mut canvas = kh_ui::Canvas::new(1920, 1080);
        let (root, _) = kh_host::hud_layers::stage_root(1920.0, 1080.0);
        kh_ui::Renderer::new(rig.fonts.clone()).draw(&mut canvas, m, &root);
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| [0.0, 0.0, 0.0], &mut rgba);
        rgba
    };
    let m = rig.hud.movie(movie).unwrap();
    let full = draw(m);
    let mut hidden = m.clone();
    assert!(hidden.set_visible(path, false), "{path}");
    let without = draw(&hidden);
    let (mut n, mut sum) = (0f32, [0f32; 3]);
    for (p, q) in full.as_chunks::<4>().0.iter().zip(without.as_chunks::<4>().0) {
        if p.iter().zip(q).take(3).any(|(x, y)| x.abs_diff(*y) > 24) {
            n += 1.0;
            for c in 0..3 {
                sum[c] += p[c] as f32;
            }
        }
    }
    let n1 = n.max(1.0);
    ([sum[0] / n1, sum[1] / n1, sum[2] / n1], n)
}

#[test]
fn the_resolve_pips_stand_beside_guard_and_read_in_thirds() {
    let Some(mut rig) = Rig::new() else { return };
    rig.frames(&[], 120);
    rig.picture("10_resolve_0");
    assert!(rig.hud.gof_state().0.shown);
    assert_eq!(rig.hud.gof_state().0.last.0, [0.0, 0.0, 0.0]);
    // a third of a pip for a perfect deflect, then two pips and a third
    rig.gv().resolve = ResolveValues { thirds: 1, in_zone: false };
    rig.frames(&[], 2);
    rig.picture("11_resolve_a_third");
    let s = rig.hud.gof_state().0.last.0;
    assert!((s[0] - 1.0 / 3.0).abs() < 0.01 && s[1] == 0.0, "{s:?}");
    let (lit, n) = colour_of(&rig, "HudStatus", "sttsPc.gofPipLit0");
    assert!(n > 100.0 && lit[0] > 90.0, "the first chevron brightens by a third: {lit:?} {n}");
    rig.gv().resolve = ResolveValues { thirds: 7, in_zone: false };
    rig.frames(&[], 2);
    rig.picture("12_resolve_two_and_a_third");
    assert_eq!(rig.hud.gof_state().0.last.0[..2], [1.0, 1.0]);
    // In the Zone: gold
    rig.gv().resolve = ResolveValues { thirds: 9, in_zone: true };
    rig.frames(&[], 8);
    rig.picture("13_resolve_in_the_zone");
    let (zone, _) = colour_of(&rig, "HudStatus", "sttsPc.gofPipLit2");
    assert!(zone[0] > zone[2] * 1.3, "gold: {zone:?}");
    // clear of the HP ring and of the GUARD bar's top, with a bar of any length
    let (x0, y0, x1, y1) = rig.hud.gof_state().0.plate_box;
    let guard_y = rig.hud.movie("HudStatus").unwrap().node(kh_ui::hud_guard::GUARD).map_or(0.0, |n| n.matrix.ty);
    assert!(x0 >= 0.0 && x1 <= 92.0 && y1 <= guard_y, "over the GUARD label, left of the ring, above the bar: plate {:?}, GUARD at {guard_y}", (x0, y0, x1, y1));
    rig.guard = GuardValues { value: 195.0, max: 195.0, ..rig.guard };
    rig.frames(&[], 30);
    rig.picture("14_resolve_level_99_guard");
    assert_eq!(rig.hud.gof_state().0.plate_box, (x0, y0, x1, y1), "the label end of the bar does not move");
    // Sora leads: gone, and the movie's own parts are untouched
    rig.view = None;
    rig.hud.set_leader(PartyChara::Sora, true);
    rig.frames(&[], 30);
    assert!(!rig.hud.gof_state().0.shown);
    assert!(!rig.hud.movie("HudStatus").unwrap().node("sttsPc.gofPipPlate").is_some_and(|n| n.is_visible()));
}

#[test]
fn the_stagger_gauge_needs_no_scan_and_follows_the_lock_on() {
    const GAUGE: &str = kh_ui::hud_goofy::STAGGER;
    let Some(mut rig) = Rig::new() else { return };
    rig.words.push("libra off".to_string());
    rig.frames(&[], 60);
    let shown = |rig: &Rig| rig.hud.movie("HudRight").unwrap().node(GAUGE).is_some_and(|n| n.is_visible());
    // no Scan, no HP gauge: the Stagger gauge alone
    assert!(!rig.hud.movie("HudRight").unwrap().node("hudRight.enmHpPack.enmHpSetAll").is_some_and(|n| n.is_visible()), "no HP gauge without Scan");
    rig.gv().stagger = vec![(LOCK, StaggerValues { value: 40.0, max: 100.0, broken: false })];
    rig.frames(&[], 40);
    rig.picture("20_stagger_no_scan");
    assert!(shown(&rig));
    assert_eq!(rig.hud.gof_state().1.last.0, 200, "40 % of the 500 frames");
    assert!(colour_of(&rig, "HudRight", "hudRight.gofStagger.armorHp.armorHpGg").1 > 800.0, "the fill draws");
    assert!(colour_of(&rig, "HudRight", "hudRight.gofStagger.armorHp.gofLabel").1 > 100.0, "the word draws");
    // with Scan the HP gauge above it, the gauge under its bars
    rig.words.push("libra on".to_string());
    rig.lock = false;
    rig.frames(&[], 30);
    assert!(!rig.hud.gof_state().1.shown, "no target: no gauge");
    rig.lock = true;
    rig.frames(&[], 60);
    rig.picture("21_stagger_under_the_hp_gauge");
    assert!(shown(&rig));
    // it drains: the red part follows
    rig.gv().stagger = vec![(LOCK, StaggerValues { value: 25.0, max: 100.0, broken: false })];
    rig.frames(&[], 6);
    rig.picture("22_stagger_losing");
    let (fill, red, _) = rig.hud.gof_state().1.last;
    assert!(red > fill, "{red} > {fill}");
    rig.frames(&[], 90);
    assert_eq!(rig.hud.gof_state().1.last.1, 0, "the red part caught up");
    // BROKEN: full, the glow, the blink
    rig.gv().stagger = vec![(LOCK, StaggerValues { value: 100.0, max: 100.0, broken: true })];
    rig.frames(&[], 12);
    rig.picture("23_stagger_broken");
    assert!(rig.hud.gof_state().1.last.2);
    assert_eq!(rig.hud.gof_state().1.last.0, 500);
    // an enemy with no entry yet: an empty gauge; another target's book is not shown
    rig.gv().stagger = vec![(OTHER, StaggerValues { value: 90.0, max: 100.0, broken: false })];
    rig.frames(&[], 12);
    assert_eq!(rig.hud.gof_state().1.last.0, 0);
    // Sora leads: gone, and it stays gone once its out animation has played
    rig.view = None;
    rig.hud.set_leader(PartyChara::Sora, true);
    rig.frames(&[], 30);
    assert!(!rig.hud.gof_state().1.shown);
    for _ in 0..4 {
        rig.frames(&[], 20);
        assert_eq!(colour_of(&rig, "HudRight", GAUGE).1, 0.0, "nothing of the gauge on the stage");
    }
    rig.picture("24_stagger_gone");
}

#[test]
fn an_attack_from_behind_the_camera_still_warns_on_the_edge_toward_it() {
    let Some(mut rig) = Rig::new() else { return };
    rig.frames(&[], 60);
    let look = gof_look().peril;
    // the camera stands past them all, looking away: +x is its right. LOCK and OTHER are behind it on the right, BEHIND on the left
    let eye = Vec3::new(0.0, 3000.0, 200.0);
    rig.camera = glam::Mat4::perspective_rh(1.0, 16.0 / 9.0, 10.0, 100_000.0) * glam::Mat4::look_at_rh(eye, eye + Vec3::Y, Vec3::Z);
    rig.lock = false;
    rig.gv().peril = vec![PerilCue { key: LOCK.0 as u64, eta: 0.3, active: false, kind: None }, PerilCue { key: BEHIND.0 as u64, eta: 0.3, active: false, kind: None }];
    rig.frames(&[], 12);
    rig.picture("35_peril_behind_the_camera");
    let last = rig.hud.gof_state().2.last.clone();
    let of = |t: TargetId| *last.iter().find(|m| m.key == t.0 as u64).unwrap();
    let (right, left) = (of(LOCK), of(BEHIND));
    assert!(!right.on_screen && !left.on_screen, "{last:?}");
    assert!(right.alpha > 0.99 && left.alpha > 0.99, "both warn: {last:?}");
    assert!(right.at.0 > 960.0 && left.at.0 < 960.0, "each on its own side: {last:?}");
    for m in [right, left] {
        assert!((look.edge..=1920.0 - look.edge).contains(&m.at.0) && (look.edge..=1080.0 - look.edge).contains(&m.at.1), "inside the margin: {m:?}");
    }
}

#[test]
fn the_red_mark_shows_ahead_of_the_hit_once_per_enemy_and_never_says_which_kind() {
    let Some(mut rig) = Rig::new() else { return };
    rig.frames(&[], 60);
    let dt = 1.0 / 60.0;
    let look = gof_look().peril;
    // a telegraph at 1.2 s: the host's list walks down (kh-party's `Telegraph`, perilous)
    let (mut eta, mut shown_at, mut gone_at, mut t) = (1.2f32, None, None, 0.0f32);
    let mut pictures = 0;
    while t < 1.8 {
        rig.gv().peril = if eta > 0.0 {
            vec![PerilCue { key: LOCK.0 as u64, eta, active: false, kind: None }]
        } else if eta > -0.2 {
            vec![PerilCue { key: LOCK.0 as u64, eta: 0.0, active: true, kind: None }]
        } else {
            Vec::new()
        };
        rig.frames(&[], 1);
        let on = rig.hud.gof_state().2.showing().contains(&(LOCK.0 as u64));
        if on && shown_at.is_none() {
            shown_at = Some(t);
        }
        if !on && shown_at.is_some() && gone_at.is_none() {
            gone_at = Some(t);
        }
        if let Some(at) = shown_at {
            for (i, when) in [(0, 0.03), (1, 0.12), (2, 0.4), (3, 0.56)] {
                if pictures == i && t - at >= when {
                    rig.picture(&format!("3{i}_peril_t{when}"));
                    pictures += 1;
                }
            }
        }
        eta -= dt;
        t += dt;
    }
    let (at, gone) = (shown_at.expect("it showed"), gone_at.expect("it went"));
    assert!((1.2 - at - look.lead).abs() < 0.04, "shown {:.2} s ahead of the window", 1.2 - at);
    assert!(gone >= 1.2 && gone < 1.2 + look.fade + 0.05, "gone at {gone}, the window opened at 1.2");
    // two enemies, two telegraphs from one of them: two marks, over their own heads, one for an attacker off screen on the edge
    // toward it
    let _ = rig.hud.take_sounds();
    rig.gv().peril = vec![PerilCue { key: LOCK.0 as u64, eta: 0.3, active: false, kind: None }, PerilCue { key: LOCK.0 as u64, eta: 0.2, active: false, kind: None }, PerilCue { key: OTHER.0 as u64, eta: 0.4, active: false, kind: None }, PerilCue { key: BEHIND.0 as u64, eta: 0.4, active: false, kind: None }];
    rig.frames(&[], 12);
    rig.picture("34_peril_two_enemies");
    let last = rig.hud.gof_state().2.last.clone();
    assert_eq!(last.iter().filter(|m| m.on_screen).count(), 2, "{last:?}");
    let lock = last.iter().find(|m| m.key == LOCK.0 as u64).unwrap();
    assert_eq!(lock.at, (1100.0, 520.0 - 110.0 - look.lift), "over his head");
    assert!(lock.alpha > 0.99);
    let behind = last.iter().find(|m| m.key == BEHIND.0 as u64).unwrap();
    assert!(!behind.on_screen, "{behind:?}");
    assert!(behind.alpha > 0.99, "it shows: {behind:?}");
    assert_eq!(behind.at.0, look.edge, "on the left edge, the side it stands on: {behind:?}");
    assert!((look.edge..=1080.0 - look.edge).contains(&behind.at.1), "{behind:?}");
    // the cue: the static SE of the sheet, once for the three that popped together
    let names: Vec<String> = rig.hud.take_sounds().iter().map(|s| s.1.clone()).filter(|s| s.contains("se00000_042")).collect();
    assert_eq!(names.len(), 1, "{names:?}");
    // with no lock-on at all it still shows: it is the attacker's, not the target's
    rig.lock = false;
    rig.gv().peril = vec![PerilCue { key: OTHER.0 as u64, eta: 0.1, active: false, kind: None }];
    rig.frames(&[], 2);
    assert!(rig.hud.gof_state().2.last.iter().any(|m| m.key == OTHER.0 as u64 && m.on_screen));
    // Danger Sense
    rig.gv().peril.clear();
    rig.frames(&[], 30);
    assert!(rig.hud.gof_state().2.showing().is_empty());
    rig.gv().peril_extra = 0.15;
    rig.gv().peril = vec![PerilCue { key: OTHER.0 as u64, eta: look.lead + 0.1, active: false, kind: None }];
    rig.frames(&[], 2);
    assert_eq!(rig.hud.gof_state().2.showing(), vec![OTHER.0 as u64], "Danger Sense: earlier");
    // Sora leads: no marks
    rig.view = None;
    rig.hud.set_leader(PartyChara::Sora, true);
    rig.frames(&[], 30);
    assert!(rig.hud.gof_state().2.showing().is_empty());
}

/// (Danger Sense) The cue of an enemy of the row, naming `kind`.
fn named(n: u32, eta: f32, active: bool, kind: PerilKind) -> PerilCue {
    PerilCue { key: n as u64, eta, active, kind: Some(kind) }
}

/// The mean colour of the bright pixels (the word's face) in a box of a 1920 x 1080 picture, and how many there are.
fn bright_in(rgba: &[u8], (x0, y0, x1, y1): (f32, f32, f32, f32)) -> ([f32; 3], f32) {
    let (mut n, mut sum) = (0f32, [0f32; 3]);
    for y in (y0.max(0.0) as usize)..(y1.min(1080.0) as usize) {
        for x in (x0.max(0.0) as usize)..(x1.min(1920.0) as usize) {
            let p = &rgba[(y * 1920 + x) * 4..][..3];
            if p.iter().any(|c| *c > 150) {
                n += 1.0;
                for c in 0..3 {
                    sum[c] += p[c] as f32;
                }
            }
        }
    }
    let n1 = n.max(1.0);
    ([sum[0] / n1, sum[1] / n1, sum[2] / n1], n)
}

#[test]
fn danger_sense_names_each_kind_under_the_mark_and_the_word_goes_with_it() {
    // (THE USER 2026-11-02: "Danger Sense ability should indicate what kind of attack it will be")
    let Some(mut rig) = Rig::new() else { return };
    rig.frames(&[], 60);
    let look = gof_look().peril;
    rig.lock = false;
    rig.gv().peril_extra = kh_party::goofy::K.danger_lead;
    // one enemy per kind, in a row
    rig.gv().peril = PerilKind::ALL.iter().enumerate().map(|(i, k)| named(ROW + i as u32, 0.4, false, *k)).collect();
    rig.frames(&[], 12);
    let rgba = rig.picture("36_danger_sense_every_kind");
    let last = rig.hud.gof_state().2.last.clone();
    // the word's lower half (below the "!"'s glow): its colour is its kind's, of all seven the nearest
    let cos = |a: [f32; 3], b: [f32; 4]| (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / ((a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() * (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt()).max(1e-6);
    for (i, kind) in PerilKind::ALL.iter().enumerate() {
        let m = last.iter().find(|m| m.key == (ROW + i as u32) as u64).unwrap_or_else(|| panic!("{kind:?}: {last:?}"));
        assert_eq!(m.word, Some(*kind), "{m:?}");
        assert!(m.on_screen && m.alpha > 0.99 && (m.scale - 1.0).abs() < 0.01, "{m:?}");
        assert!(m.word_w > look.word_h && m.word_w <= look.word_max_w + 0.01, "{kind:?} {}", m.word_w);
        let top = look.size / 2.0 - look.size * 22.0 / 144.0 + look.word_gap;
        let (mean, n) = bright_in(&rgba, (m.at.0 - m.word_w / 2.0, m.at.1 + top + look.word_h / 2.0, m.at.0 + m.word_w / 2.0, m.at.1 + top + look.word_h));
        assert!(n > 40.0, "{kind:?}: the word draws ({n} bright pixels)");
        let nearest = PerilKind::ALL.iter().max_by(|a, b| cos(mean, look.word(**a).color).total_cmp(&cos(mean, look.word(**b).color))).unwrap();
        assert_eq!(nearest, kind, "{kind:?} reads as its own colour: {mean:?}");
    }
    // on the stage's edge: the pair keeps inside, the widest word too
    rig.gv().peril = vec![PerilCue { key: CORNER.0 as u64, eta: 0.4, active: false, kind: Some(PerilKind::Projectile) }];
    rig.frames(&[], 30);
    rig.picture("37_danger_sense_on_the_edge");
    let m = *rig.hud.gof_state().2.last.iter().find(|m| m.key == CORNER.0 as u64).expect("it warns");
    assert!(!m.on_screen && m.word == Some(PerilKind::Projectile), "{m:?}");
    assert!(m.at.0 + m.word_w / 2.0 <= 1920.0 && m.at.1 + look.size / 2.0 + look.word_h <= 1080.0, "the word is on the stage: {m:?}");
    // the word stays exactly as long as the "!": through the window, the fade, then both are gone
    rig.gv().peril = vec![named(ROW + 2, 0.3, false, PerilKind::Sweep)];
    rig.frames(&[], 30);
    let (mut seen, mut t) = (0, 0);
    while !rig.hud.gof_state().2.showing().is_empty() && t < 120 {
        rig.gv().peril = if t < 20 { vec![named(ROW + 2, 0.0, true, PerilKind::Sweep)] } else { Vec::new() };
        rig.frames(&[], 1);
        for m in &rig.hud.gof_state().2.last {
            assert_eq!(m.word, Some(PerilKind::Sweep), "the \"!\" always carries its word: {m:?}");
            seen += 1;
        }
        t += 1;
    }
    assert!(seen > 0 && rig.hud.gof_state().2.showing().is_empty() && rig.hud.gof_state().2.last.is_empty(), "both gone together ({seen} frames, {t})");
    // without Danger Sense (no kind on the cue): the plain "!", no word
    rig.gv().peril = vec![PerilCue { key: OTHER.0 as u64, eta: 0.3, active: false, kind: None }];
    rig.frames(&[], 12);
    let m = *rig.hud.gof_state().2.last.iter().find(|m| m.key == OTHER.0 as u64).unwrap();
    assert_eq!((m.word, m.word_w), (None, 0.0), "{m:?}");
}

#[test]
fn every_attack_class_gets_its_own_danger_sense_word() {
    use kh_core::host::AttackClass as C;
    let look = gof_look().peril;
    let classes = [C::Normal, C::Thrust, C::Sweep, C::Grab, C::Heavy, C::Projectile, C::Magic];
    let kinds: Vec<PerilKind> = classes.iter().map(|c| kh_partyhost::custom::peril_kind(*c)).collect();
    assert_eq!(kinds, PerilKind::ALL.to_vec(), "one for one");
    for (c, k) in classes.iter().zip(&kinds) {
        let w = look.word(*k);
        assert!(!w.text.is_empty() && w.color[3] > 0.99, "{c:?}: {w:?}");
        assert_eq!(w.text.to_ascii_lowercase(), match c { C::Normal => "strike".to_string(), _ => format!("{c:?}").to_ascii_lowercase() }, "the word names the class");
    }
    let mut words: Vec<&str> = kinds.iter().map(|k| look.word(*k).text.as_str()).collect();
    words.sort_unstable();
    words.dedup();
    assert_eq!(words.len(), 7, "no two classes share a word");
}

#[test]
fn his_command_menu_is_attack_and_items_and_the_cursor_skips_the_hidden_row() {
    use kh_core::command::MenuState;
    const MAIN: &str = "cmdMenu.cmdMainTop.cmdMain";
    let Some(mut rig) = Rig::new() else { return };
    rig.frames(&[], 60);
    rig.picture("40_menu_attack");
    let row = |n: usize| format!("{MAIN}.cmdMain{n}");
    let cmd = |rig: &Rig, n: usize| rig.hud.movie("HudCommand").unwrap().node(&row(n)).map(|m| (m.is_visible(), m.matrix.ty));
    let text = |rig: &Rig, n: usize| rig.hud.movie("HudCommand").unwrap().text(&format!("{}.cmdText1.txCmdName.txCmdNameCol.coCmdName.textField", row(n))).unwrap_or_default();
    assert_eq!(text(&rig, 1), "Attack");
    assert_eq!(text(&rig, 3), "Items");
    assert!(cmd(&rig, 1).unwrap().0 && cmd(&rig, 3).unwrap().0);
    assert!(!cmd(&rig, 2).unwrap().0, "the middle row is hidden");
    assert_eq!(cmd(&rig, 3).unwrap().1, cmd(&rig, 2).unwrap().1, "Items stands in its place: no hole in the list");
    assert!(matches!(rig.hud.movie("HudCommand").unwrap().current_label(&row(4)), Some("off" | "out")), "no Link row");
    // the pad: down lands on Items, down again wraps to Attack, up wraps to Items
    rig.frames(&[PadButton::DPadDown], 12);
    assert_eq!(rig.menu.main_cursor, 2);
    rig.picture("41_menu_items_selected");
    rig.frames(&[PadButton::DPadDown], 12);
    assert_eq!(rig.menu.main_cursor, 0);
    rig.frames(&[PadButton::DPadUp], 12);
    assert_eq!(rig.menu.main_cursor, 2);
    assert_eq!(rig.hud.command_cursor(), 2, "the movie's cursor with it");
    rig.frames(&[PadButton::DPadUp], 12);
    assert_eq!(rig.menu.main_cursor, 0);
    // Items opens (A), its list is his items, B goes back onto Items
    rig.frames(&[PadButton::DPadDown], 12);
    rig.frames(&[PadButton::FaceBottom], 24);
    rig.picture("42_menu_items_list");
    assert_eq!((rig.menu.state, rig.menu.list), (MenuState::SubSel, 2));
    rig.frames(&[PadButton::FaceRight], 24);
    assert_eq!((rig.menu.state, rig.menu.main_cursor), (MenuState::MainSel, 2));
    assert!(cmd(&rig, 3).unwrap().0 && !cmd(&rig, 2).unwrap().0, "the rows after the list closed");
    assert_eq!(cmd(&rig, 3).unwrap().1, cmd(&rig, 2).unwrap().1);
    rig.picture("43_menu_back_on_items");
}

/// A runtime with a Goofy at `level`, leading or not.
fn runtime(level: u32, lead: bool) -> (PartyRuntime, kh_party::MemberId) {
    let mut rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
    rt.party = kh_party::Party::new(kh_party::PartyConfig { level99: false, total_exp: 0, seed: 1 });
    rt.spawn_one(PartyChara::Donald, Vec3::new(200.0, 0.0, 0.0), 0.0);
    rt.spawn_one(PartyChara::Goofy, Vec3::ZERO, 0.0);
    let id = rt.id(PartyChara::Goofy).unwrap();
    let s = rt.party.status_mut(id).unwrap();
    s.level = level;
    s.gof_sync_level();
    if lead {
        rt.party.set_pad(id, Some(kh_core::Tuning::USER));
        rt.leader = Some(id);
    }
    (rt, id)
}

fn rig_with(rt: &mut PartyRuntime, lead: bool) -> Option<Rig> {
    let mut rig = Rig::new()?;
    rt.feed_leader_magic(&rig.sora);
    let f = kh_host::formation::Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], if lead { PartyChara::Goofy } else { PartyChara::Sora }).unwrap();
    rig.hud.set_formation(&f);
    rig.hud.set_party(&rt.hud_slots());
    rig.hud.set_leader(if lead { PartyChara::Goofy } else { PartyChara::Sora }, true);
    rig.hud.set_party_menu(rt.menu_members());
    rig.view = lead.then(|| GoofyHudView { look: gof_look(), ..GoofyHudView::default() });
    rig.frames(&[], 30);
    Some(rig)
}

/// Equipment, Abilities, Items, Customize, Stats.
const CUSTOMIZE_DOWNS: usize = 3;

#[test]
fn customize_equips_two_arts_on_a_pad_and_shows_taunt_and_items_fixed() {
    let (mut rt, _) = runtime(30, true);
    let Some(mut rig) = rig_with(&mut rt, true) else { return };
    rig.words.push("menu open".to_string());
    rig.frames(&[], 70);
    rig.menu_keys(&["down"; CUSTOMIZE_DOWNS]);
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 30);
    rig.camp_picture("50_customize_top");
    // no Magic entry: the first entry is Shortcut Set A
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 30);
    rig.camp_picture("51_customize_set");
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 30);
    rig.camp_picture("52_customize_arts_list");
    // the third Art of the list (Charge, Tornado, Bash, Turbo at level 30) on LB + A
    rig.menu_keys(&["down", "down", "decide"]);
    rig.frames(&[], 20);
    rig.camp_picture("53_customize_after");
    for e in rig.hud.take_party_edits() {
        if let kh_host::party::PartyMenuEdit::Custom { chara, edit } = e {
            assert!(rt.apply_custom_edit(chara, edit), "{edit:?}");
        }
    }
    let block = rt.customs.block(PartyChara::Goofy).unwrap();
    assert_eq!(block.arts()[0], ids::BASH, "LB + A is Goofy Bash now");
    assert_eq!(block.shortcuts[0][ids::SLOT_TAUNT], ids::TAUNT);
    assert_eq!(block.shortcuts[2][ids::SLOT_A], ids::BASH, "the Arts on every page");
}

/// (THE USER 2026-11-02: "...introduce a cooldown with a visible timer") Customize's help line of each Art says its MP and its
/// cooldown from the sheet rows; Taunt's says its MP alone.
#[test]
fn customize_help_gives_each_arts_mp_and_cooldown_and_taunt_none() {
    use kh_party::pad::goofy_arts::{cooldown, mp, Art};
    let (mut rt, _) = runtime(30, true);
    let Some(mut rig) = rig_with(&mut rt, true) else { return };
    rig.words.push("menu open".to_string());
    rig.frames(&[], 70);
    rig.menu_keys(&["down"; CUSTOMIZE_DOWNS]);
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 30);
    let data = rig.hud.camp_menu().unwrap().customize.data.clone();
    let help = |kind: u8| data.entries.iter().find(|e| e.kind == kind).map(|e| e.help.clone()).unwrap_or_default();
    let charge = format!("A shield-first rush that pushes enemies along and ends in a guard stance. MP Cost: {}  Cooldown: {} s  Rank 1", mp(Art::Charge, 1), cooldown(Art::Charge));
    assert_eq!(help(ids::CHARGE), charge);
    for (art, kind) in [(Art::Tornado, ids::TORNADO), (Art::Bash, ids::BASH), (Art::Turbo, ids::TURBO)] {
        assert!(help(kind).contains(&format!("MP Cost: {}  Cooldown: {} s", mp(art, 1), cooldown(art))), "{art:?}: {}", help(kind));
    }
    let taunt = help(ids::TAUNT);
    assert!(taunt.ends_with(&format!("MP Cost: {}", mp(Art::Taunt, 1))) && !taunt.contains("Cooldown") && !taunt.contains('{'), "{taunt}");
}

/// (GF-UI) LB held: the panel as the fight sees it: his two Arts, Taunt, the set's item, in the HUD's own rows.
#[test]
fn the_lb_panel_shows_his_two_arts_taunt_and_the_item() {
    use kh_core::command::leader::LeaderKind;
    let (mut rt, id) = runtime(30, true);
    let Some(mut rig) = rig_with(&mut rt, true) else { return };
    rig.status = rt.party.status(id).unwrap().clone();
    rig.status.mp = rig.status.max_mp();
    rig.menu.leader.kind = Some(LeaderKind::Goofy);
    rig.menu.leader.custom = *rt.customs.block(PartyChara::Goofy).unwrap();
    let held = PadButtons::NONE.with(PadButton::LeftShoulder);
    for _ in 0..40 {
        rig.held = held;
        rig.frames(&[PadButton::LeftShoulder], 1);
    }
    rig.picture("44_lb_panel");
    let snap = rig.menu.snapshot(Chara::Goofy, &rig.status);
    let names: Vec<&str> = snap.shortcut.iter().map(|r| r.name).collect();
    assert_eq!(names, ["Goofy Tornado", "Potion", "Taunt", "Goofy Charge"], "B, Y, X, A");
}

/// (THE USER 2026-11-02: "Goofy's abilities (except taunt) should cost more MP, and introduce a cooldown with a visible timer, they
/// are way too spammable") LB held while Goofy Charge waits: its entry carries KH3's situation timer gauge and its seconds, as the
/// game's HUD draws it from his panel row; Taunt's entry never does.
#[test]
fn the_lb_panel_shows_a_waiting_art_with_kh3s_timer_gauge_and_its_seconds() {
    use kh_core::command::leader::LeaderKind;
    use kh_party::pad::goofy_arts::{cooldown, Art};
    let (mut rt, id) = runtime(30, true);
    let Some(mut rig) = rig_with(&mut rt, true) else { return };
    rig.status = rt.party.status(id).unwrap().clone();
    rig.status.mp = rig.status.max_mp();
    let charge = Art::ALL.iter().position(|a| *a == Art::Charge).unwrap();
    rig.status.goofy.as_mut().unwrap().run.arts.cooldown[charge] = cooldown(Art::Charge) * 0.5;
    rig.menu.leader.kind = Some(LeaderKind::Goofy);
    rig.menu.leader.custom = *rt.customs.block(PartyChara::Goofy).unwrap();
    let held = PadButtons::NONE.with(PadButton::LeftShoulder);
    for _ in 0..40 {
        rig.held = held;
        rig.frames(&[PadButton::LeftShoulder], 1);
    }
    rig.picture("46_lb_panel_art_waiting");
    let m = rig.hud.movie("HudCommand").unwrap();
    let base = "cmdMenu.cmdScutTop.cmdShort.cmdEx";
    let gauge = |row: usize| m.node(&format!("{base}.cmdExBaseSet.gofRecharge{row}")).filter(|n| n.is_visible()).and_then(|_| m.current_frame(&format!("{base}.cmdExBaseSet.gofRecharge{row}")));
    let half = kh_ui::frames::situation_gauge_frame(0.5, 1.0) as u32;
    assert!(gauge(4).is_some_and(|f| f.abs_diff(half) <= 1), "Goofy Charge on A: half drained, {:?} vs {half}", gauge(4));
    assert_eq!((gauge(1), gauge(2), gauge(3)), (None, None, None), "Goofy Tornado, the Potion and Taunt are ready");
    let number = m.text(&format!("{base}.cmdExTextSet4.cmdExText1.cmdNumber.cmdNumCol.txCmdNumber.cmdNumberTxt"));
    assert_eq!(number.as_deref(), Some(format!("{}", (cooldown(Art::Charge) * 0.5).ceil() as u32).as_str()), "its seconds in the entry's number");
    assert_eq!(m.current_label(&format!("{base}.cmdExTextSet4.cmdExText1")), Some("on"), "lit over its gauge");
}

/// (GF-UI, R13) A BROKEN enemy in reach: Y says Goofy Breaker.
#[test]
fn y_offers_goofy_breaker() {
    use kh_core::command::leader::LeaderKind;
    let Some(mut rig) = Rig::new() else { return };
    rig.menu.leader.kind = Some(LeaderKind::Goofy);
    rig.frames(&[], 30);
    rig.menu.leader.team = vec![kh_partyhost::custom::gof_breaker_row()];
    rig.menu.leader.sora_busy = true;
    rig.frames(&[], 40);
    rig.picture("45_y_goofy_breaker");
    let snap = rig.menu.snapshot(Chara::Goofy, &rig.status);
    assert_eq!(snap.situations[0].name, "Goofy Breaker");
    assert_eq!(snap.situation_selected, Some(0));
}

#[test]
fn a_goofy_who_does_not_lead_keeps_his_arts_on_the_leaders_screen() {
    let (mut rt, _) = runtime(30, false);
    let Some(mut rig) = rig_with(&mut rt, false) else { return };
    rig.words.push("menu open".to_string());
    rig.frames(&[], 70);
    rig.menu_keys(&["down"; CUSTOMIZE_DOWNS]);
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 30);
    rig.camp_picture("54_customize_sora_leads");
    // Magic, Set A, B, C, the friends, then his Arts: up from the first entry wraps there
    rig.menu_keys(&["up"]);
    rig.frames(&[], 20);
    rig.camp_picture("55_customize_goofys_arts_entry");
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 30);
    rig.camp_picture("56_customize_goofys_arts_set");
    rig.menu_keys(&["decide", "down", "decide"]);
    rig.frames(&[], 20);
    let edits = rig.hud.take_party_edits();
    assert_eq!(edits.len(), 1, "{edits:?}");
    let kh_host::party::PartyMenuEdit::Custom { chara, edit } = edits[0].clone() else { panic!("{edits:?}") };
    assert_eq!(chara, PartyChara::Goofy);
    assert!(rt.apply_custom_edit(chara, edit));
    assert_eq!(rt.customs.block(PartyChara::Goofy).unwrap().arts()[0], ids::TORNADO);
}

#[test]
fn his_ability_page_and_status_read_right() {
    use kh_party::goofy::GoofyAbility as A;
    let (mut rt, id) = runtime(60, true);
    let s = rt.party.status(id).unwrap();
    assert_eq!(s.magic(), 0, "no magic");
    let rows = kh_partyhost::custom::kit_rows(s);
    assert!(rows.iter().any(|r| r.key == A::DangerSense.name() && r.help == A::DangerSense.description() && r.ap == A::DangerSense.ap()), "{rows:?}");
    assert!(rows.iter().all(|r| r.key.starts_with("GOF_") && !r.help.is_empty() && r.ap > 0), "{rows:?}");
    assert!(s.ap_in_use() <= s.max_ap(), "the level's abilities fit his AP: {} / {}", s.ap_in_use(), s.max_ap());
    // the page's AP is what the check charges: his own rows and KH3's Item Boost / Treasure Magnet
    let kh3: i32 = s.abilities.iter_equipped().map(|(k, n)| kh_core::status::tables::ABILITIES.iter().find(|r| r.kind == k).and_then(|r| r.ap).unwrap_or(0) * n as i32).sum();
    assert_eq!(rows.iter().filter(|r| r.equipped).map(|r| r.ap).sum::<i32>() + kh3, s.ap_in_use());
    let (strength, defense) = (s.strength(), s.defense());
    let Some(mut rig) = rig_with(&mut rt, true) else { return };
    let m = rt.menu_members().into_iter().find(|m| m.chara == PartyChara::Goofy).unwrap();
    assert_eq!((m.strength, m.magic, m.defense), (strength, 0, defense));
    assert!(m.guard.is_some());
    // the Stats page, then the Abilities page
    rig.words.push("menu open".to_string());
    rig.frames(&[], 70);
    rig.menu_keys(&["down"; 4]);
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 20);
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 60);
    rig.camp_picture("60_status_page");
    rig.menu_keys(&["cancel", "cancel"]);
    rig.frames(&[], 30);
    rig.menu_keys(&["up", "up", "up"]);
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 20);
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 60);
    rig.camp_picture("61_ability_page");
}

/// The doc's section 2 at level 99 with every reward: the Stats page's numbers.
#[test]
fn his_status_page_reads_the_docs_numbers_at_level_99() {
    let (mut rt, id) = runtime(1, true);
    *rt.party.status_mut(id).unwrap() = FriendStatus::test_level99(Chara::Goofy);
    let m = rt.menu_members().into_iter().find(|m| m.chara == PartyChara::Goofy).unwrap();
    assert_eq!((m.level, m.strength, m.magic, m.defense), (99, 44, 0, 86), "Strength, Magic, Defense");
    assert_eq!((m.hp_max, m.mp_max), (376, 136), "section 2's totals with every reward");
    assert_eq!(m.guard.map(|g| g.1), Some(195), "GUARD at level 99");
    let Some(mut rig) = rig_with(&mut rt, true) else { return };
    rig.words.push("menu open".to_string());
    rig.frames(&[], 70);
    rig.menu_keys(&["down"; 4]);
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 20);
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 60);
    rig.camp_picture("65_status_level_99");
}

/// The doc's top end (section 2): five armour slots, three accessories, five item slots. The Equipment list scrolls to all of them.
#[test]
fn the_fifth_slots_show_on_the_equipment_screen() {
    let (mut rt, id) = runtime(60, true);
    let s = rt.party.status_mut(id).unwrap();
    (s.armour_slots, s.accessory_slots, s.item_slots) = (5, 3, 5);
    let m = rt.menu_members().into_iter().find(|m| m.chara == PartyChara::Goofy).unwrap();
    assert_eq!((m.armour.len(), m.accessories.len(), m.items.len()), (5, 3, 5));
    let Some(mut rig) = rig_with(&mut rt, true) else { return };
    rig.words.push("menu open".to_string());
    rig.frames(&[], 70);
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 20);
    rig.menu_keys(&["decide"]);
    rig.frames(&[], 60);
    rig.camp_picture("62_equipment_top");
    rig.menu_keys(&["down"; 5]);
    rig.frames(&[], 30);
    rig.camp_picture("63_equipment_fifth_armour");
    rig.menu_keys(&["down"; 8]);
    rig.frames(&[], 30);
    rig.camp_picture("64_equipment_fifth_item");
}

#[test]
fn the_boss_panels_say_a_saved_rank_a_rank_and_max_hp_past_rank_3() {
    use kh_party::growth::{GrowthGrant, RankLine, SlotKind};
    use kh_partyhost::growth::growth_line;
    let Some(mut rig) = Rig::new() else { return };
    // through GF-STATS' label, a rank after the ability sparkle
    let saved = GrowthGrant::Rank(RankLine::PinningGuard, 2);
    assert_eq!(growth_line(&saved, 10), "<icon(Ability)>Pinning Guard: Rank 2 ready");
    assert_eq!(growth_line(&saved, 20), "<icon(Ability)>Pinning Guard: Rank 2");
    assert_eq!(growth_line(&GrowthGrant::MaxHp(10), 40), "Max HP +10");
    assert_eq!(growth_line(&GrowthGrant::MaxMp(4), 40), "Max MP +4");
    assert_eq!(growth_line(&GrowthGrant::Slot(SlotKind::Armour), 40), "Armor slot unlocked!");
    rig.frames(&[], 30);
    rig.hud.party_bonus(PartyChara::Goofy, &[growth_line(&saved, 10), growth_line(&GrowthGrant::Rank(RankLine::Deflect, 3), 10)]);
    rig.frames(&[], 80);
    rig.picture("70_panel_saved_rank");
    // the popups stand under the Stagger gauge, not over it
    let bottom = 76.0 + 28.0;
    let y = rig.hud.movie("HudRight").unwrap().node("hudRight.getBonus").map_or(0.0, |n| n.matrix.ty);
    assert!(y >= bottom, "{y}");
    rig.frames(&[], 320);
    rig.hud.party_bonus(PartyChara::Goofy, &[growth_line(&GrowthGrant::MaxHp(10), 40)]);
    rig.frames(&[], 80);
    rig.picture("71_panel_max_hp");
    rig.frames(&[], 320);
    // a level that teaches more than the window's three lines: the rest on a panel
    rig.hud.party_level_up(PartyChara::Goofy, 20, &["Strength increased!".to_string(), "Defense increased!".to_string(), "GUARD increased!".to_string(), "<icon(Ability)>Pinning Guard: Rank 2".to_string()]);
    rig.frames(&[], 80);
    rig.picture("72_level_up_with_a_saved_rank");
    let right = rig.hud.movie("HudRight").unwrap();
    assert!(right.text("hudRight.getBonus.getBonusSet.txGetBonusCapacity1.textField").is_some_and(|t| t.contains("Pinning Guard")), "the fourth line is on a panel");
}

#[test]
fn a_level_says_what_it_taught_with_his_saved_ranks() {
    use kh_party::growth::{GrowthStep, RankLine};
    // (THE USER 2026-11-02: every technique by level 40) Pinning Guard is learned at level 15
    let lv = RankLine::PinningGuard.learn_level();
    let (mut rt, id) = runtime(lv - 1, false);
    assert!(rt.gof_level_lines(PartyChara::Goofy, lv - 2, lv - 1).is_empty());
    // (level 15 also teaches the Shield Art Goofy Bash: both are named)
    assert!(rt.gof_level_lines(PartyChara::Goofy, lv - 1, lv).contains(&"Pinning Guard".to_string()), "rank 1: the name");
    let mut g = rt.party.status(id).unwrap().growth;
    g.apply(Chara::Goofy, GrowthStep::Rank(RankLine::PinningGuard));
    rt.party.status_mut(id).unwrap().set_growth(g);
    let lines = rt.gof_level_lines(PartyChara::Goofy, lv - 1, lv);
    assert!(lines.contains(&"Pinning Guard: Rank 2".to_string()) && !lines.contains(&"Pinning Guard".to_string()), "a saved rank applies at once: {lines:?}");
    assert!(rt.gof_level_lines(PartyChara::Donald, lv - 1, lv).is_empty());
    // every technique is named on the level that teaches it, all of them by 40
    for line in RankLine::ALL.into_iter().filter(|l| l.token().starts_with("tech:") && l.learn_level() > 0) {
        assert!(line.learn_level() <= 40, "{} at {}", line.label(), line.learn_level());
        assert!(rt.gof_level_lines(PartyChara::Goofy, line.learn_level() - 1, line.learn_level()).iter().any(|s| s.starts_with(line.label())), "{}", line.label());
    }
}

#[test]
fn the_view_of_the_runtime_reads_his_state_and_only_when_he_leads() {
    use kh_core::host::AttackClass;
    use kh_party::goofy::GoofyAbility as A;
    let (mut rt, id) = runtime(5, true);
    let list = [
        kh_party::Telegraph { attacker: TargetId(3), class: AttackClass::Thrust, perilous: true, eta: 0.4, active: false },
        kh_party::Telegraph { attacker: TargetId(4), class: AttackClass::Normal, perilous: false, eta: 0.2, active: false },
        kh_party::Telegraph { attacker: TargetId(5), class: AttackClass::Grab, perilous: true, eta: 0.0, active: true },
    ];
    rt.party.set_telegraphs(&list);
    let g = rt.party.status_mut(id).unwrap().goofy.as_mut().unwrap();
    g.core.resolve.thirds = 5;
    let v = rt.gof_hud_view().expect("he leads");
    assert_eq!(v.resolve, ResolveValues { thirds: 5, in_zone: false });
    assert_eq!(v.peril, vec![PerilCue { key: 3, eta: 0.4, active: false, kind: None }, PerilCue { key: 5, eta: 0.0, active: true, kind: None }], "only the perilous, no kind");
    assert_eq!(v.peril_extra, 0.0);
    let g = rt.party.status_mut(id).unwrap().goofy.as_mut().unwrap();
    g.abilities.give(A::DangerSense);
    g.abilities.set_equipped(A::DangerSense, true);
    assert_eq!(rt.gof_hud_view().unwrap().peril_extra, kh_party::goofy::K.danger_lead);
    // (THE USER 2026-11-02: "Danger Sense ability should indicate what kind of attack it will be") with it, each cue names its kind
    assert_eq!(rt.gof_hud_view().unwrap().peril, vec![PerilCue { key: 3, eta: 0.4, active: false, kind: Some(PerilKind::Thrust) }, PerilCue { key: 5, eta: 0.0, active: true, kind: Some(PerilKind::Grab) }]);
    let g = rt.party.status_mut(id).unwrap().goofy.as_mut().unwrap();
    g.abilities.set_equipped(A::DangerSense, false);
    assert!(rt.gof_hud_view().unwrap().peril.iter().all(|c| c.kind.is_none()), "taken off: the plain \"!\" again");
    rt.party.set_pad(id, None);
    assert!(rt.gof_hud_view().is_none(), "an AI Goofy has no HUD pieces");
}
