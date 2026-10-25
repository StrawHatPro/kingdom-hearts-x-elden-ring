//! (DON-UI) DONALD'S HUD AND MENU, OFFLINE (documentation/DONALD_OVERHAUL.md 6.1, 6.3, 6.5, 7, 9): the real HUD
//! (kh-host `Hud` + the UI pack, skipped without it) fed by the real menu (kh-party `FriendMenu`) and the real snapshot
//! (kh-party `donald::hud::DonHud`) through kh-partyhost's conversion (`custom::don_view_of`), as both hosts feed it.
//! Pictures: build/sora_tmp/party/DON-UI/shots/ (the status, command and target HUDs on one 1920 x 1080 stage).
//!
//! - the Resonance badges (KH3's MP base as a plate: the ultimate mark, the element icon, its three chevron pips right
//!   next to it), the full-power rim, the second badge stacked on top with Dual Attunement, the attune cast's blinking
//!   icon; no focus / shotlock gauge for Donald;
//! - the four Arcane Flow stars by the Y command, the Zettaflare tracker;
//! - the lock-on target's status icons and Elemental Insight's arrows; the Scan gate;
//! - the menu: Attack / Magic / Attunement / Items, an element's list, the Attunement list's marks, the ally target
//!   list, Quick Cast's lit shortcut, the Y list with its stars.

use kh_core::input::PadButton;
use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView, View};
use kh_host::party::{PartyChara, PartyHudSlot};
use kh_party::donald::attune::Slot;
use kh_party::donald::{DonAbility, DonLearned, Element, Size, StatusBook};
use kh_party::pad::menu::{AllyRow, DonFeed, FriendMenu};
use kh_party::{Chara, FriendStatus, MemberId};
use kh_partyhost::custom::{don_view_of, DonHudFeed};

const LOCK: TargetId = TargetId(7);

struct Dummy;
impl HudTargets for Dummy {
    fn target(&self, id: TargetId) -> Option<TargetView> {
        (id == LOCK).then_some(TargetView { center: Vec3::new(1200.0, 420.0, 0.0), hp: 700.0, hp_max: 1000.0, large: false })
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn out_dir() -> std::path::PathBuf {
    let d = root().join("build/sora_tmp/party/DON-UI/shots");
    std::fs::create_dir_all(&d).ok();
    d
}

struct Rig {
    hud: Hud,
    fonts: kh_ui::Fonts,
    sora: Sora,
    menu: FriendMenu,
    status: FriendStatus,
    book: StatusBook,
    lock: bool,
    held: PadButtons,
    worn: Vec<(PartyChara, bool, u8)>,
    affinity: Option<(kh_party::donald::ElemSet, kh_party::donald::ElemSet)>,
}

fn wear(s: &mut FriendStatus, k: DonAbility) {
    let a = &mut s.donald.as_mut().unwrap().abilities;
    a.give(k);
    a.set_equipped(k, true);
}

impl Rig {
    fn new() -> Option<Rig> {
        let Ok((mut hud, fonts)) = Hud::load(&root().join("build/sora_pack/ui")) else {
            eprintln!("no UI pack: skipped");
            return None;
        };
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
        let mut status = FriendStatus::new(Chara::Donald, 0);
        status.donald.as_mut().unwrap().learned = DonLearned::all();
        status.mp = status.max_mp() * 8 / 10;
        let mut menu = FriendMenu::default();
        menu.don.feed = DonFeed {
            on: true,
            members: vec![
                AllyRow { id: MemberId(0), name: "Donald", hp: 140, hp_max: 200 },
                AllyRow { id: MemberId(1), name: "Goofy", hp: 160, hp_max: 220 },
                AllyRow { id: MemberId(7), name: "Sora", hp: 120, hp_max: 160 },
            ],
            ..DonFeed::default()
        };
        let slots = [
            PartyHudSlot { chara: PartyChara::Donald, hp: 140, hp_max: 200, mp: 80, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
            PartyHudSlot { chara: PartyChara::Goofy, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
        ];
        hud.set_party(&slots);
        hud.set_leader(PartyChara::Donald, true);
        Some(Rig { hud, fonts, sora, menu, status, book: StatusBook::default(), lock: false, held: PadButtons::NONE, worn: Vec::new(), affinity: None })
    }

    fn st(&mut self) -> &mut kh_party::donald::DonaldState {
        self.status.donald.as_mut().unwrap()
    }

    fn attune(&mut self, slots: [Option<(Element, f32)>; 2]) {
        let st = self.st();
        st.core.attune.slots = slots.map(|s| s.map(|(element, res)| Slot { element, res, period: 1, ult_used: false, full_at: None }));
    }

    /// `n` frames, the first with `buttons` pressed.
    fn frames(&mut self, buttons: &[PadButton], n: usize) {
        for k in 0..n {
            let now = if k == 0 { buttons.iter().fold(PadButtons::NONE, |p, b| p.with(*b)) } else { PadButtons::NONE };
            let pad = kh_core::input::PadState::from_held(now, self.held);
            self.held = now;
            self.menu.don.feed.lock = self.lock.then(|| (LOCK.0, self.book.get(LOCK.0).copied().unwrap_or_default()));
            self.menu.don.feed.affinity = self.affinity;
            let _ = self.menu.tick(Chara::Donald, &self.status, &pad, false);
            let mut snapshot = self.sora.ui_snapshot();
            snapshot.command = self.menu.snapshot(Chara::Donald, &self.status);
            snapshot.lock_on = if self.lock { LOCK } else { TargetId::NONE };
            snapshot.step = self.sora.ui_snapshot().step + k as u64 + 1;
            let st = self.status.donald.as_ref().unwrap();
            let hud = kh_party::donald::hud::DonHud::with(st, &self.book, self.lock.then_some(LOCK.0), self.affinity, &kh_party::donald::data());
            let feed = DonHudFeed { hud, menu: self.menu.don_view(Chara::Donald, &self.status), worn: self.worn.clone() };
            self.hud.set_don(Some(don_view_of(&feed, &[])));
            let view = View { clip_from_world: glam::Mat4::orthographic_rh(0.0, 1920.0, 1080.0, 0.0, -1.0, 1.0), size: (1920.0, 1080.0) };
            self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &Dummy, to_view: same, view: Some(view), words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
            let dt = 1.0 / 60.0;
            let st = self.st();
            st.core.clock += dt;
        }
    }

    /// The stage (1920 x 1080) with the status, command and target HUDs over a mid-tone floor; written as `name`.
    fn picture(&self, name: &str) -> Vec<u8> {
        self.picture_on(name, |y| { let t = y as f32 / 1080.0; [0.30 + 0.08 * t, 0.27 + 0.05 * t, 0.24] }, false)
    }

    /// [`Rig::picture`] over the floor `bg` (by row); `ult`: also a x4 close-up of the badges' corner (`name_ult`).
    fn picture_on(&self, name: &str, bg: impl Fn(usize) -> [f32; 3], ult: bool) -> Vec<u8> {
        let (w, h) = (1920usize, 1080usize);
        let mut canvas = kh_ui::Canvas::new(w, h);
        let mut r = kh_ui::Renderer::new(self.fonts.clone());
        for mv in ["HudTarget", "HudCommand", "HudStatus"] {
            let movie = self.hud.movie(mv).unwrap();
            let (root, _) = kh_host::hud_layers::stage_root(w as f32, h as f32);
            r.draw(&mut canvas, movie, &root);
        }
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, y| bg(y), &mut rgba);
        let _ = kh_ui::def::write_png(&out_dir().join(format!("{name}.png")), w as u32, h as u32, &rgba);
        // close-ups (x2): the status HUD, the command HUD
        let crops = [("status", (1480usize, 820usize, 440usize, 260usize), 2usize), ("cmd", (0, 600, 760, 480), 2), ("ult", (1470, 870, 250, 140), 4)];
        for (tag, (x0, y0, cw, ch), z) in crops.into_iter().filter(|c| ult || c.0 != "ult") {
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
}

/// Mean colour and count of the pixels that change when `path` of `movie` is hidden.
fn colour_of(rig: &Rig, movie: &str, path: &str) -> ([f32; 3], f32) {
    colour_on(rig, movie, &[path], [0.0, 0.0, 0.0])
}

/// [`colour_of`] over a flat floor `bg`.
fn colour_on(rig: &Rig, movie: &str, paths: &[&str], bg: [f32; 3]) -> ([f32; 3], f32) {
    let draw = |m: &kh_ui::Movie| {
        let mut canvas = kh_ui::Canvas::new(1920, 1080);
        let (root, _) = kh_host::hud_layers::stage_root(1920.0, 1080.0);
        kh_ui::Renderer::new(rig.fonts.clone()).draw(&mut canvas, m, &root);
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| bg, &mut rgba);
        rgba
    };
    let m = rig.hud.movie(movie).unwrap();
    let full = draw(m);
    let mut hidden = m.clone();
    for path in paths {
        assert!(hidden.set_visible(path, false), "{path}");
    }
    let without = draw(&hidden);
    let (mut n, mut sum) = (0f32, [0f32; 3]);
    for (p, q) in full.chunks_exact(4).zip(without.chunks_exact(4)) {
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

/// THE CLUSTER'S LAYOUT (the user: "Lay it out in a way that is easy to understand and read"): every plate above the MP
/// bar (its top at y 128 in `sttsPc` units), left of the portrait (x 90), none overlapping another.
fn tidy(rig: &Rig) {
    let (plates, stars, _) = rig.hud.don_layout();
    // THE ORDER (the user: "place the three pip meters below the other ones"), bottom to top: the badges, the Arcane
    // Flow row, the Zettaflare tracker; no empty slot: the lowest row always sits right above the MP bar
    let look = kh_ui::hud_resonance::ResonanceLayout::default();
    let nb = rig.hud.don_badges().len();
    let (badges, zetta) = plates.split_at(nb.min(plates.len()));
    if let Some(st) = stars {
        for b in badges {
            assert!(st.3 <= b.1, "the stars row above the badges: {st:?} {b:?}");
        }
        for z in zetta {
            assert!(z.3 <= st.1, "the tracker above the stars row: {z:?} {st:?}");
        }
        let lowest = badges.iter().map(|b| b.3).chain(std::iter::once(st.3)).fold(f32::MIN, f32::max);
        assert!((lowest - (look.badge_y + look.plate_h / 2.0)).abs() < 0.01, "the lowest row right above the MP bar: {lowest}");
    }
    if nb == 2 {
        assert!(badges[1].3 <= badges[0].1, "the second badge on top of the first");
    }
    let all: Vec<(f32, f32, f32, f32)> = plates.iter().copied().chain(stars).collect();
    for (i, a) in all.iter().enumerate() {
        assert!(a.3 <= 128.0 && a.2 <= 90.0, "plate {a:?} clear of the MP bar and the portrait");
        for b in &all[i + 1..] {
            let apart = a.3 <= b.1 || b.3 <= a.1 || a.2 <= b.0 || b.2 <= a.0;
            assert!(apart, "plates overlap: {a:?} {b:?}");
        }
    }
}

#[test]
fn donalds_hud_badges_pips_marks_stars_tracker_and_statuses() {
    use kh_ui::hud_resonance::UltLook;
    let Some(mut rig) = Rig::new() else { return };
    let ex_hidden = |rig: &Rig| !rig.hud.movie("HudStatus").unwrap().node("sttsPc.pcEx").is_some_and(|n| n.is_visible());
    // the pips always right next to their element icon: the first pip starts the icon's half width + the gap after its
    // centre, in every badge
    let next_to_icon = |rig: &Rig| {
        let look = kh_ui::hud_resonance::ResonanceLayout::default();
        for (icon_x, pips_left, _) in rig.hud.don_badges() {
            let gap = pips_left - (icon_x + look.icon_size / 2.0);
            assert!((gap - look.icon_gap).abs() < 0.01, "pips {gap} px from their icon");
        }
    };
    // NONE: not attuned: no badge, no focus / shotlock gauge (he has none); the stars stand empty by the menu
    rig.frames(&[], 120);
    rig.picture("01_none");
    let (rings, _, stars, _, shown) = rig.hud.don_state();
    assert!(rings.is_empty() && shown[1], "no badge before he attunes; the Arcane Flow row is up");
    tidy(&rig);
    assert!(ex_hidden(&rig), "no focus / shotlock gauge for Donald");
    assert_eq!((stars.0, stars.3), (0, 6), "no charges; the tracker's six icons");
    // SINGLE: Fire at 2.4: two pips lit, the third at its share; the ultimate waits (dim)
    rig.attune([Some((Element::Fire, 2.4)), None]);
    rig.frames(&[], 30);
    rig.picture("02_single");
    tidy(&rig);
    let (rings, ults, _, _, _) = rig.hud.don_state();
    assert_eq!(rings, vec![(0, 2, false)]);
    assert_eq!(ults, vec![UltLook::Waiting]);
    next_to_icon(&rig);
    assert!(ex_hidden(&rig));
    let (c, n) = colour_of(&rig, "HudStatus", "sttsPc.donBadge0Pip0");
    assert!(n > 60.0, "a pip draws ({n} px)");
    let fire = kh_partyhost::custom::rgba(Element::Fire.color(&kh_party::donald::data().tune));
    assert!(c[0] > c[2] && (c[1] / c[0] - fire[1] / fire[0]).abs() < 0.15, "a Fire pip (its hue) {c:?} vs {fire:?}");
    assert!(rig.hud.movie("HudStatus").unwrap().node("sttsPc.donBadge0Plate").is_some_and(|n| n.is_visible()), "the plate is up");
    assert!(colour_of(&rig, "HudStatus", "sttsPc.donBadge0Icon").1 > 80.0, "the icon draws");
    // SINGLE FULL POWER: three pips, the rim, the ultimate lit
    rig.attune([Some((Element::Fire, 3.0)), None]);
    rig.frames(&[], 30);
    rig.picture("03_single_full_power");
    tidy(&rig);
    let (rings, ults, _, _, _) = rig.hud.don_state();
    assert_eq!((rings, ults), (vec![(0, 3, true)], vec![UltLook::Ready]));
    assert!(colour_of(&rig, "HudStatus", "sttsPc.donBadge0Rim").1 > 40.0, "the full-power rim draws");
    next_to_icon(&rig);
    // DUAL: Water (the newest, primary) at 1 at the bottom, Fire at 2 stacked on top of it, both fully visible
    wear(&mut rig.status, DonAbility::DualAttunement);
    rig.attune([Some((Element::Water, 1.0)), Some((Element::Fire, 2.0))]);
    rig.frames(&[], 30);
    rig.picture("04_dual");
    tidy(&rig);
    let (rings, _, _, _, _) = rig.hud.don_state();
    assert_eq!(rings, vec![(0, 1, false), (1, 2, false)]);
    let b = rig.hud.don_badges();
    let look = kh_ui::hud_resonance::ResonanceLayout::default();
    assert!((b[0].2 - b[1].2 - look.row_step).abs() < 0.01 && look.row_step >= look.plate_h, "the second badge on top of the first, not overlapping");
    next_to_icon(&rig);
    assert!(colour_of(&rig, "HudStatus", "sttsPc.donBadge1Pip1").1 > 60.0 && colour_of(&rig, "HudStatus", "sttsPc.donBadge0Pip0").1 > 60.0, "both badges visible");
    // DUAL, ONE AT FULL POWER: Fire at 3 (its rim, its ultimate lit), Water at 1
    rig.attune([Some((Element::Water, 1.0)), Some((Element::Fire, 3.0))]);
    rig.frames(&[], 30);
    rig.picture("05_dual_one_full_power");
    tidy(&rig);
    let (rings, ults, _, _, _) = rig.hud.don_state();
    assert_eq!(rings, vec![(0, 1, false), (1, 3, true)]);
    assert_eq!(ults, vec![UltLook::Waiting, UltLook::Ready]);
    next_to_icon(&rig);
    // THE ULTIMATE SPENT: dark
    rig.st().core.attune.slots[1].as_mut().unwrap().ult_used = true;
    rig.frames(&[], 10);
    rig.picture("06_ultimate_spent");
    tidy(&rig);
    assert_eq!(rig.hud.don_state().1[1], UltLook::Spent);
    next_to_icon(&rig);
    // THE ARCANE FLOW STARS, 1 .. 4 (0 above), and the lock-on statuses with Scan / Elemental Insight
    rig.st().core.fight_full.insert(Element::Fire);
    rig.lock = true;
    let s = rig.book.entry(LOCK.0, Size::Medium);
    s.wet = 5.0;
    s.chill = 2;
    s.chill_left = 4.0;
    s.reveal = 4.0;
    wear(&mut rig.status, DonAbility::Scan);
    wear(&mut rig.status, DonAbility::ElementalInsight);
    rig.affinity = Some((kh_party::donald::ElemSet::one(Element::Thunder), kh_party::donald::ElemSet::one(Element::Fire)));
    for n in 1..=4u8 {
        let now = rig.st().core.clock;
        rig.st().core.arcane.gain(now, 20.0, 4);
        rig.frames(&[], 20);
        rig.picture(&format!("07_stars_{n}"));
        assert_eq!(rig.hud.don_state().2 .0, n);
    }
    let (_, _, stars, icons, shown) = rig.hud.don_state();
    assert_eq!(stars.2, 1, "Fire lit on the tracker");
    assert_eq!((icons.0, icons.1), (3, 2), "Chill, Wet, Reveal over the target; Thunder weak, Fire resisted");
    assert!(shown == [true, true, true]);
    tidy(&rig);
    assert!(!rig.hud.movie("HudCommand").unwrap().node("cmdMenu.donStar0").is_some_and(|n| n.is_visible()), "no stars by the command menu any more");
    // the oldest charge about to go: its star blinks
    rig.st().core.arcane.expiry[0] = rig.st().core.clock + 2.0;
    rig.frames(&[], 10);
    assert!(rig.hud.don_state().2 .1, "the oldest charge's star blinks in its last seconds");
    // THE WORN SPELLS (the user: "next to the ally's picture in the empty grey space next to them. For the leader ...
    // inbetween the gap of the MP bar and the start of the health bar"; one icon per character)
    let look = kh_ui::hud_resonance::ResonanceLayout::default();
    // the leader with a barrier
    rig.worn = vec![(PartyChara::Donald, true, 0)];
    rig.frames(&[], 6);
    rig.picture("08_worn_leader_barrier");
    let w = rig.hud.don_worn();
    assert_eq!(w.len(), 1);
    // (the user: "between the magic and health, in the empty space between the top of the MP bar and the health bar right
    // next to the face"): above the MP bar's right end (its top at y 128, its right end at x 103) and left of the HP
    // ring (its left edge near x 92 at that height), clear of every plate
    let (x, y, half) = (w[0].1, w[0].2, look.worn_size_pc / 2.0);
    assert!(y + half <= 128.0, "above the MP bar's top: {w:?}");
    assert!(x + half <= 92.0 && x - half >= 60.0, "left of the HP ring, by the MP bar's right end: {w:?}");
    let (plates, stars, _) = rig.hud.don_layout();
    for p in plates.iter().chain(stars.iter()) {
        let apart = p.2 <= x - half || y + half <= p.1 || p.3 <= y - half;
        assert!(apart, "the icon clear of the plate {p:?}: {w:?}");
    }
    // a friend with orbs (Goofy, the lower friend slot): in the grey plate right of his portrait
    rig.worn = vec![(PartyChara::Goofy, false, 3)];
    rig.frames(&[], 6);
    rig.picture("09_worn_friend_orbs");
    assert_eq!(rig.hud.don_worn().len(), 1);
    assert!(colour_of(&rig, "HudStatus", "sttsPc.donWorn2").1 > 60.0, "the orbs icon draws by Goofy");
    // all three wearing one; a member with both shows one icon (the rule itself is allyfx's)
    rig.worn = vec![(PartyChara::Donald, false, 2), (PartyChara::Goofy, true, 0), (PartyChara::Sora, true, 4)];
    rig.st().core.attune.cast = Some(kh_party::donald::attune::AttuneCast { element: Element::Aero, left: 0.3, total: 0.6 });
    rig.frames(&[], 6);
    rig.picture("08_worn_all_three_and_attuning");
    assert_eq!(rig.hud.don_worn().len(), 3);
    for slot in 0..3 {
        assert_eq!(rig.hud.movie("HudStatus").unwrap().sprite(&format!("sttsPc.donWorn{slot}")).map(|s| s.gfx.len()), Some(1), "one icon on slot {slot}");
    }
    // Sora leads again: the focus gauge is the HUD driver's again
    rig.hud.set_don(None);
    rig.hud.set_leader(PartyChara::Sora, true);
    rig.frames(&[], 1);
    assert!(rig.hud.movie("HudStatus").unwrap().node("sttsPc.pcEx").is_some_and(|n| n.script_visible.is_none()), "the focus gauge handed back");
}

/// THE READY ULTIMATE, WAY BRIGHTER (the user, 2026-10-22: "the icon that tells you ultimate magic is ready needs to be
/// way brighter"): white-hot, a wide halo in the element's colour, KH3's glint, a burst on becoming Ready; it pops on a
/// bright scene and a dark one, and stays clear of the other rows and the leader's worn icon.
#[test]
fn donalds_ready_ultimate_pops_on_light_and_dark_scenes() {
    use kh_ui::hud_resonance::UltLook;
    let Some(mut rig) = Rig::new() else { return };
    let look = kh_ui::hud_resonance::ResonanceLayout::default();
    let (light, dark) = ([0.93, 0.91, 0.86], [0.04, 0.04, 0.06]);
    wear(&mut rig.status, DonAbility::DualAttunement);
    rig.worn = vec![(PartyChara::Donald, true, 0)];
    rig.attune([Some((Element::Water, 2.0)), Some((Element::Fire, 2.0))]);
    rig.frames(&[], 10);
    // both reach full power: the burst
    rig.attune([Some((Element::Water, 3.0)), Some((Element::Fire, 3.0))]);
    rig.frames(&[], 4);
    rig.picture_on("20_ult_ready_burst_dark", |_| dark, true);
    rig.picture_on("20_ult_ready_burst_light", |_| light, true);
    let burst = rig.hud.don_ult_glow().0;
    rig.frames(&[], 60);
    assert_eq!(rig.hud.don_state().1, vec![UltLook::Ready, UltLook::Ready]);
    rig.picture_on("21_ult_ready_light", |_| light, true);
    rig.picture_on("22_ult_ready_dark", |_| dark, true);
    tidy(&rig);
    // white-hot: the mark (and its over-bright copy) near white on a dark scene
    let (c, n) = colour_on(&rig, "HudStatus", &["sttsPc.donBadge1Core"], dark);
    let (c2, n2) = colour_on(&rig, "HudStatus", &["sttsPc.donBadge1Ult"], dark);
    assert!(n + n2 > 60.0, "the mark draws ({n} + {n2} px)");
    let hot = if n >= n2 { c } else { c2 };
    assert!(hot.iter().all(|v| *v > 200.0), "white-hot {c:?} {c2:?}");
    // the halo spreads well past the mark, in Fire's colour, on a dark scene and (the tint) on a bright one
    let (g, gn) = colour_on(&rig, "HudStatus", &["sttsPc.donBadge1Glow", "sttsPc.donBadge1Tint"], dark);
    let (_, mark_px) = colour_of(&rig, "HudStatus", "sttsPc.donBadge1Ult");
    assert!(gn > 6.0 * mark_px.max(1.0) && g[0] > g[2], "a wide warm halo on dark: {gn} px vs the mark's {mark_px}, {g:?}");
    let (t, tn) = colour_on(&rig, "HudStatus", &["sttsPc.donBadge1Glow", "sttsPc.donBadge1Tint"], light);
    assert!(tn > 6.0 * mark_px.max(1.0) && t[0] > t[2] + 20.0, "the halo shows on a bright scene: {tn} px, {t:?}");
    let (w, wn) = colour_on(&rig, "HudStatus", &["sttsPc.donBadge0Glow", "sttsPc.donBadge0Tint"], light);
    assert!(wn > 6.0 * mark_px.max(1.0) && w[2] > w[0] + 20.0, "Water's halo blue on a bright scene: {wn} px, {w:?}");
    assert!(colour_on(&rig, "HudStatus", &["sttsPc.donBadge1Flare"], dark).1 > 40.0, "the glint draws");
    // clear of the other rows (no reach over a neighbour row's icons and marks) and of the leader's worn icon
    let (reach, boxes) = rig.hud.don_ult_glow();
    assert_eq!(boxes.len(), 2);
    let rows: Vec<f32> = rig.hud.don_badges().iter().map(|b| b.2).collect();
    let (_, stars, _) = rig.hud.don_layout();
    let mut centres = rows.clone();
    centres.extend(stars.map(|s| (s.1 + s.3) / 2.0));
    let w = rig.hud.don_worn();
    assert_eq!(w.len(), 1);
    let half = look.worn_size_pc / 2.0;
    for r in reach.iter().chain(burst.iter()) {
        let cy = (r.1 + r.3) / 2.0;
        for other in centres.iter().filter(|c| (*c - cy).abs() > 1.0) {
            // that row's tallest piece: its icon, or its own grown mark
            let half_row = look.icon_size.max(look.ult_size * look.ult_ready_scale) / 2.0;
            assert!(r.3 <= other - half_row || r.1 >= other + half_row, "the glow {r:?} reaches the row at {other}");
        }
        let apart = r.2 <= w[0].1 - half || r.0 >= w[0].1 + half || r.3 <= w[0].2 - half || r.1 >= w[0].2 + half;
        assert!(apart, "the glow {r:?} clear of the worn icon {w:?}");
    }
    // the mark grown, still inside its row
    for (b, cy) in boxes.iter().zip(&rows) {
        assert!(b.1 >= cy - look.row_step / 2.0 && b.3 <= cy + look.row_step / 2.0, "the mark {b:?} inside its row {cy}");
    }
}

#[test]
fn donalds_menu_on_the_hud_lists_target_list_quick_cast_and_y() {
    let Some(mut rig) = Rig::new() else { return };
    rig.attune([Some((Element::Fire, 3.0)), None]);
    rig.frames(&[], 60);
    // the main list, then Magic -> Fire
    rig.frames(&[PadButton::DPadDown], 12);
    rig.frames(&[PadButton::FaceBottom], 20);
    rig.picture("10_magic_elements");
    rig.frames(&[PadButton::FaceBottom], 20);
    rig.picture("11_magic_fire_full_power");
    let cmd = rig.hud.movie("HudCommand").unwrap();
    let texts: Vec<String> = (1..=9).filter_map(|i| cmd.text(&format!("cmdMenu.cmdSubTop.cmdSub1Set.cmdSub1_{i}.cmdText1.txCmdName.txCmdNameCol.coCmdName.textField"))).filter(|t| !t.is_empty()).collect();
    assert!(texts.iter().any(|t| t == "Firaza"), "{texts:?}");
    // (DON-UI) the user's menu rules (2026-10-22): B one level at a time; a choice returns to Attack; Magic always opens
    // the element select
    rig.frames(&[PadButton::FaceRight], 30);
    rig.picture("16_b_back_to_the_elements");
    assert_eq!((rig.menu.state, rig.menu.don.list), (kh_core::command::MenuState::SubSel, kh_party::pad::menu::DonList::Elements));
    rig.frames(&[PadButton::FaceRight], 30);
    rig.picture("17_b_back_to_the_main_list_on_magic");
    assert_eq!((rig.menu.state, rig.menu.main_cursor), (kh_core::command::MenuState::MainSel, 1));
    // Attunement: the marks; B back to the main list (on Attunement)
    rig.frames(&[PadButton::DPadDown], 12);
    rig.frames(&[PadButton::FaceBottom], 20);
    rig.picture("12_attunement_release_marks");
    rig.frames(&[PadButton::FaceRight], 20);
    assert_eq!(rig.menu.main_cursor, 2);
    // Magic -> the element select -> Aero -> Aero Barrier -> the party target list
    rig.frames(&[PadButton::DPadUp], 12);
    rig.frames(&[PadButton::FaceBottom], 20);
    for _ in 0..4 {
        rig.frames(&[PadButton::DPadDown], 8);
    }
    rig.frames(&[PadButton::FaceBottom], 20);
    rig.frames(&[PadButton::DPadDown], 8);
    rig.frames(&[PadButton::FaceBottom], 30);
    rig.picture("13_ally_target_list");
    assert!(rig.menu.don.target.is_some());
    let cmd = rig.hud.movie("HudCommand").unwrap();
    let names: Vec<String> = (1..=6).filter_map(|i| cmd.text(&format!("cmdMenu.cmdSubTop.cmdSub2Set.cmdSub2_{i}.cmdText1.txCmdName.txCmdNameCol.coCmdName.textField"))).filter(|t| !t.is_empty()).collect();
    assert_eq!(names.len(), 3, "{names:?}");
    // B: back to Aero's spells (one level)
    rig.frames(&[PadButton::FaceRight], 30);
    rig.picture("18_b_target_list_back_to_the_spells");
    assert!(rig.menu.don.target.is_none() && rig.menu.don.list == kh_party::pad::menu::DonList::Spells(kh_party::pad::menu::Group::Elem(Element::Aero)));
    // cast: the menu collapses onto Attack
    rig.frames(&[PadButton::DPadUp], 8);
    rig.frames(&[PadButton::FaceBottom], 30);
    rig.picture("19_cast_returns_to_attack");
    assert_eq!((rig.menu.state, rig.menu.main_cursor), (kh_core::command::MenuState::MainSel, 0));
    // Magic again: the element select (its cursor on Aero), never Aero's spells
    rig.frames(&[PadButton::DPadDown], 12);
    rig.frames(&[PadButton::FaceBottom], 30);
    rig.picture("1a_magic_reopens_the_element_select");
    assert_eq!((rig.menu.don.list, rig.menu.sub_cursor), (kh_party::pad::menu::DonList::Elements, 4));
    rig.frames(&[PadButton::FaceRight], 20);
    rig.frames(&[PadButton::DPadUp], 12);
    // Quick Cast on a Wet target: the Thunder shortcut lit
    wear(&mut rig.status, DonAbility::QuickCast);
    rig.menu.don.feed.custom.filled = true;
    rig.menu.don.feed.custom.shortcuts[0] = [kh_core::command::leader::donald::line1(0, 3), kh_core::command::leader::donald::line1(2, 3), kh_core::command::leader::donald::attune(3), kh_core::command::leader::donald::line2(4)];
    rig.lock = true;
    rig.book.entry(LOCK.0, Size::Medium).wet = 6.0;
    let held = PadButtons::NONE.with(PadButton::LeftShoulder);
    for _ in 0..30 {
        let pad = kh_core::input::PadState::from_held(held, rig.held);
        rig.held = held;
        let _ = rig.menu.tick(Chara::Donald, &rig.status, &pad, false);
        rig.held = PadButtons::NONE; // keep LB held through `frames`
        rig.frames(&[PadButton::LeftShoulder], 1);
    }
    rig.picture("14_quick_cast");
    // the Y list: two charges, Comet II and Graviga stacked as KH3's situation commands, the stars by them
    rig.held = PadButtons::NONE;
    for _ in 0..2 {
        let now = rig.st().core.clock;
        rig.st().core.arcane.gain(now, 20.0, 4);
    }
    rig.frames(&[], 60);
    rig.picture("15_y_list_and_stars");
    assert_eq!(rig.menu.y_selected(), Some(kh_party::donald::SpellRef::Arcane(kh_party::donald::ArcaneLine::Comet, 2)));
    // THE STAR COST (the user: "the Y magic commands need to indicate how many stars they cost"): 1 .. 4 charges, every
    // Y row with its cost in gold stars (Comet 1, Gravity 2, Meteor 3, Ultima / Zettaflare 4)
    rig.attune([Some((Element::Fire, 3.0)), None]);
    for e in Element::ALL {
        rig.st().core.fight_full.insert(e);
    }
    for n in 1..=4u8 {
        rig.st().core.arcane = Default::default();
        for _ in 0..n {
            let now = rig.st().core.clock;
            rig.st().core.arcane.gain(now, 20.0, 4);
        }
        rig.frames(&[], 60);
        rig.picture(&format!("15_y_cost_{n}_charges"));
        let snap = rig.menu.snapshot(Chara::Donald, &rig.status);
        let view = rig.menu.don_view(Chara::Donald, &rig.status).unwrap();
        let rows: Vec<(String, usize)> = view.y_costs.iter().map(|c| (snap.situations.iter().find(|r| r.serial == c.0).map_or("", |r| r.name).split("  ").next().unwrap_or("").to_string(), c.1 as usize)).collect();
        assert!(!rows.is_empty(), "{n} charges: Y rows");
        // drawn on the HUD: one star row per Y row, as many stars as it costs
        let drawn = rig.hud.don_y_costs();
        let mut want: Vec<usize> = rows.iter().map(|r| r.1).collect();
        let mut got: Vec<usize> = drawn.iter().map(|d| d.1 as usize).collect();
        want.sort();
        got.sort();
        assert_eq!(got, want, "the stars drawn per row: {drawn:?} {rows:?}");
        for (name, stars) in &rows {
            let cost = match name.as_str() {
                // (user 2026-10-23: "make gravity cost 1 star and comet cost 2 stars")
                s if s.starts_with("Comet") => 2,
                s if s.starts_with("Grav") => 1,
                s if s.starts_with("Meteor") => 3,
                s if s.starts_with("Ultima") || s.starts_with("Zettaflare") => 4,
                s => panic!("an unknown Y row {s}"),
            };
            assert!(*stars == cost && cost <= n as usize, "{name}: {stars} stars, costs {cost}, {n} charges");
        }
        // names only (the user: "the Y commands should not say things like 'on the ground only' or their MP cost"),
        // every row on show castable
        assert!(snap.situations.iter().all(|r| r.enabled && !r.name.contains("  ") && !r.name.contains("MP")), "{:?}", snap.situations.iter().map(|r| r.name).collect::<Vec<_>>());
        if n == 4 {
            assert!(rows.iter().any(|r| r.0.starts_with("Ultima")), "{rows:?}");
        }
    }
    // Zettaflare (ZETTA, the user 2026-10-23: a FULL MP bar, "Requires a full MP bar"):
    // offered at full MP only once its requirements are met (they are "either available or they are not"), and only
    // while he has a target (the user: "Zettaflare isn't castable without a target")
    let has = |rig: &Rig, s: &str| rig.menu.snapshot(Chara::Donald, &rig.status).situations.iter().any(|r| r.name.starts_with(s));
    rig.status.mp = rig.status.max_mp();
    rig.menu.don.feed.has_target = false;
    rig.frames(&[], 30);
    rig.picture("15_y_cost_4_charges_full_mp");
    assert!(!has(&rig, "Zettaflare"), "no target: no Zettaflare row");
    rig.menu.don.feed.has_target = true;
    rig.frames(&[], 30);
    rig.picture("15_y_cost_zettaflare_with_target");
    assert!(has(&rig, "Zettaflare"), "a target: Zettaflare offered");
    assert!(rig.hud.don_y_costs().iter().filter(|c| c.1 == 4).count() == 2, "Ultima and Zettaflare: four stars each");
    // (ZETTA 2026-10-23, the user: "Zettaflare needs to use up all of your magic (it needs all of your current MP to be useable) and then put you into the red MP bar mode with a 30% slower recharge rate"; asked, the user chose "Requires a full MP bar") short of a full bar: no Zettaflare row
    rig.status.mp = rig.status.max_mp() - 1;
    rig.frames(&[], 30);
    assert!(!has(&rig, "Zettaflare"), "max MP - 1: no Zettaflare row (it needs a full bar)");
    rig.status.mp = 1;
    rig.frames(&[], 30);
    assert!(!has(&rig, "Zettaflare"), "1 MP: no Zettaflare row");
    rig.status.mp = 0;
    rig.frames(&[], 30);
    assert!(!has(&rig, "Zettaflare"), "no MP: no Zettaflare row");
    rig.menu.don.feed.has_target = false;
    // short of MP: the rows it can't pay leave the list
    rig.status.mp = 1;
    rig.frames(&[], 30);
    rig.picture("15_y_cost_4_charges_no_mp");
    assert!(rig.menu.snapshot(Chara::Donald, &rig.status).situations.is_empty(), "no greyed Y rows");
    assert!(rig.hud.don_y_costs().is_empty(), "no stars drawn");
    // in the air: Ultima and Zettaflare leave the list; the rest stay
    rig.status.mp = rig.status.max_mp();
    rig.menu.don.feed.airborne = true;
    rig.frames(&[], 30);
    rig.picture("15_y_cost_4_charges_airborne");
    assert!(!has(&rig, "Ultima") && !has(&rig, "Zettaflare") && has(&rig, "Meteor"), "in the air: no ground-only Y rows");
    rig.menu.don.feed.airborne = false;
    rig.frames(&[], 30);
    assert!(has(&rig, "Ultima"), "back on landing");
}
