//! (GOOFY-GG) GOOFY'S GUARD BAR in the status HUD (kh_ui::hud_guard): the user (2026-10-16 23:40) "a new bar above MP
//! called GUARD that is silver and matches the rest of the HUD design". Drawn through the Elden Ring host's layer path
//! (`draw_movie_in` at the stage root) with Goofy leading: the bar is a copy of KH3's MP part above the MP bar, its
//! fill silver, its lost part red (then following down), broken = KH3's MP-charge look with the refill in grey; no bar
//! (and Sora's focus gauge back) when Sora leads. Needs the UI pack (skipped without it).

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::{draw_movie_in, stage_root, Draw, Drawn, Perf};
use kh_host::party::{GuardLook, GuardValues, PartyChara, PartyHudSlot};

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn picture(fonts: &kh_ui::Fonts, movie: &kh_ui::Movie) -> Vec<u8> {
    let (stage, view) = stage_root(1920.0, 1080.0);
    let mut draw = Draw::default();
    let mut renderer = kh_ui::render::Renderer::new(fonts.clone());
    match draw_movie_in(&mut draw, &mut renderer, movie, &stage, (1920.0, 1080.0), true, Perf::default(), Some(view)) {
        Drawn::Picture(..) => draw.rgba,
        _ => panic!("the status HUD drew nothing"),
    }
}

/// Mean straight colour and pixel count of what the node at `path` draws (the pixels that change when it is hidden).
fn colour_of(fonts: &kh_ui::Fonts, movie: &kh_ui::Movie, path: &str) -> ([f32; 3], f32) {
    let full = picture(fonts, movie);
    let mut hidden = movie.clone();
    assert!(hidden.set_visible(path, false), "{path}");
    let without = picture(fonts, &hidden);
    let (mut n, mut sum) = (0f32, [0f32; 3]);
    for (p, q) in full.chunks_exact(4).zip(without.chunks_exact(4)) {
        if p[3] > 200 && p.iter().zip(q).any(|(x, y)| x.abs_diff(*y) > 24) {
            n += 1.0;
            for c in 0..3 {
                sum[c] += p[c] as f32;
            }
        }
    }
    let n1 = n.max(1.0);
    ([sum[0] / n1, sum[1] / n1, sum[2] / n1], n)
}

fn look() -> GuardLook {
    let p = kh_party::guard::GP;
    GuardLook { fill: p.hud_color_fill, shine: p.hud_color_shine, loss: p.hud_color_loss, regen: p.hud_color_regen, broken: p.hud_color_broken, blink_hz: p.hud_broken_blink, offset: (p.hud_offset_x, p.hud_offset_y) }
}

fn values(g: &kh_party::Guard) -> GuardValues {
    let v = g.view();
    GuardValues { value: v.value, max: v.max, broken: v.broken, regenerating: v.regenerating, rearm_at: v.rearm_at }
}

#[test]
fn goofys_guard_bar_is_a_silver_copy_of_the_mp_bar_above_it() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok((mut hud, fonts)) = Hud::load(&root.join("build/sora_pack/ui")) else {
        eprintln!("no UI pack: skipped");
        return;
    };
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    let snapshot = sora.ui_snapshot();
    let step = |hud: &mut Hud, n: usize| {
        for _ in 0..n {
            hud.frame(&mut Frame { snapshot: &snapshot, sora: &sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
        }
    };
    let slots = [
        PartyHudSlot { chara: PartyChara::Donald, hp: 140, hp_max: 200, mp: 70, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
        PartyHudSlot { chara: PartyChara::Goofy, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
    ];
    hud.set_party(&slots);
    hud.set_leader(PartyChara::Goofy, true);
    let mut g = kh_party::Guard::for_level(1);
    hud.set_guard(Some((values(&g), look())));
    step(&mut hud, 150);
    let ((track, fill, under, broken), shown) = hud.guard_bar_state();
    assert!(shown);
    assert_eq!((track, fill, under, broken), (180, 180, 0, false), "GUARD 100 is as long as MP 100 (1.8 frames a point), full");
    let movie = hud.movie("HudStatus").unwrap();
    assert!(movie.node("sttsPc.pcGuard").is_some_and(|n| n.is_visible()));
    // above the MP bar, by the sheet's offset
    let (mp, gd) = (movie.node("sttsPc.pcMp").unwrap().matrix, movie.node("sttsPc.pcGuard").unwrap().matrix);
    assert_eq!((gd.tx - mp.tx, gd.ty - mp.ty), (kh_party::guard::GP.hud_offset_x, kh_party::guard::GP.hud_offset_y));
    assert!(gd.ty < mp.ty, "above MP");
    // silver: the three channels together, bright
    let (c, n) = colour_of(&fonts, movie, "sttsPc.pcGuard.pcMpPack.pcMpGg");
    assert!(n > 500.0, "the fill draws ({n} px)");
    let (lo, hi) = (c[0].min(c[1]).min(c[2]), c[0].max(c[1]).max(c[2]));
    assert!(lo > 170.0 && hi - lo < 25.0, "silver fill {c:?}");
    // the GUARD label: neutral (no MP blue, no CHARGE red) and drawn
    let (c, n) = colour_of(&fonts, movie, "sttsPc.pcGuard.pcMpPack.coPcMpLogo");
    assert!(n > 150.0, "the label draws ({n} px)");
    assert!((c[0] - c[2]).abs() < 30.0, "a neutral label {c:?}");
    // Goofy's focus gauge is gone (GUARD replaces it)
    assert!(colour_of(&fonts, movie, "sttsPc.pcEx").1 < 50.0, "no focus gauge while Goofy leads");
    // a blocked hit: the lost part in red under the fill, then it follows down (KH3's FollowGauge)
    g.absorb(45.0);
    hud.set_guard(Some((values(&g), look())));
    step(&mut hud, 6);
    let ((_, fill, under, _), _) = hud.guard_bar_state();
    assert_eq!(fill, 99);
    assert!(under > fill, "the lost part shows ({under} > {fill})");
    let (c, _) = colour_of(&fonts, hud.movie("HudStatus").unwrap(), "sttsPc.pcGuard.pcMpPack.pcMpGgC");
    assert!(c[0] > 180.0 && c[1] < 120.0, "red lost part {c:?}");
    step(&mut hud, 120);
    assert_eq!(hud.guard_bar_state().0 .2, 0, "followed down");
    // broken: the fill hides, the refill shows in grey (KH3's charge labels on the copy), the label stays
    g.break_now();
    for _ in 0..((kh_party::guard::GP.guard_broken_delay + 0.5) * 60.0) as i32 {
        g.tick(1.0 / 60.0);
        hud.set_guard(Some((values(&g), look())));
        step(&mut hud, 1);
    }
    let ((_, fill, under, broken), _) = hud.guard_bar_state();
    assert!(broken && fill == 0 && under > 0, "broken refill {fill} {under}");
    let movie = hud.movie("HudStatus").unwrap();
    let (c, n) = colour_of(&fonts, movie, "sttsPc.pcGuard.pcMpPack.pcMpGgC");
    assert!(n > 50.0 && (c[0] - c[2]).abs() < 30.0 && c[0] < 140.0, "grey broken refill {c:?} ({n} px)");
    assert!(colour_of(&fonts, movie, "sttsPc.pcGuard.pcMpPack.coPcMpLogo").1 > 150.0, "GUARD label kept while broken");
    // level 99: GUARD 195, the bar longer
    let big = kh_party::Guard::for_level(99);
    hud.set_guard(Some((values(&big), look())));
    step(&mut hud, 30);
    assert_eq!(hud.guard_bar_state().0 .0, 351);
    // Sora leads: no GUARD bar, his focus gauge back
    hud.set_guard(None);
    hud.set_leader(PartyChara::Sora, true);
    step(&mut hud, 150);
    assert!(!hud.guard_bar_state().1);
    let movie = hud.movie("HudStatus").unwrap();
    assert!(!movie.node("sttsPc.pcGuard").is_some_and(|n| n.is_visible()));
    assert!(colour_of(&fonts, movie, "sttsPc.pcEx").1 > 200.0, "Sora's focus gauge");
}

#[test]
fn the_stats_pages_third_row_is_guard_for_goofy_and_focus_for_sora() {
    use kh_ui::camp::{third_gauge_label, StatsIn};
    assert_eq!((third_gauge_label("Goofy"), third_gauge_label("Sora"), third_gauge_label("Donald")), ("GUARD", "Focus", "Focus"));
    let goofy = StatsIn { name: "Goofy".into(), focus: 80, focus_max: 105, focus_unlocked: true, ..StatsIn::default() };
    assert!(goofy.fields().contains(&("GUARD", "80 / 105".to_string())));
    let sora = StatsIn { name: "Sora".into(), focus: 100, focus_max: 100, focus_unlocked: true, ..StatsIn::default() };
    assert!(sora.fields().contains(&("Focus", "100 / 100".to_string())));
}

/// (GOOFY-GG, AUD's Elden Ring run 2026-10-17: "while Goofy leads the whole bottom-right status HUD is not drawn")
/// THE ELDEN RING LAYER PATH (`hud_layers::Layers`, kh-er hud.rs) SHOWS A STATIC STATUS HUD AGAIN AFTER IT WAS HIDDEN.
/// Cause: a hidden layer (a cutscene: `Hud::movie` None -> `LayerEvent::Hide`) kept its last picture's pieces, so when
/// the movie came back unchanged `draw_movie_in` answered `Drawn::Same` and no `Show` was ever sent: the overlay kept
/// the layer hidden until something in the movie moved. Sora's status always moves (his focus gauge's glow), and so did
/// Goofy's with X3G's shield gauge in the focus slot; with GUARD in its place Goofy's status can be still. Checked: a
/// cutscene on / off, a leader switch, a pause menu open / close: the status layer is shown after each.
#[test]
fn a_still_status_hud_is_shown_again_after_a_cutscene_and_every_switch() {
    use kh_host::hud_layers::{LayerEvent, Layers, LAYERS};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok((mut hud, fonts)) = Hud::load(&root.join("build/sora_pack/ui")) else {
        eprintln!("no UI pack: skipped");
        return;
    };
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    let snapshot = sora.ui_snapshot();
    let status = LAYERS.iter().position(|l| *l == "HudStatus").unwrap();
    let (stage_m, stage) = stage_root(2560.0, 1440.0);
    let mut layers = Layers::new(fonts.clone(), true);
    layers.clip = Some(stage);
    layers.clip_only = vec![1, 2, 3, 4];
    // the overlay: whether the status layer is shown
    let mut shown = false;
    let run = |hud: &mut Hud, layers: &mut Layers, shown: &mut bool, n: usize, words: &[&str]| {
        for k in 0..n {
            let w: Vec<String> = if k == 0 { words.iter().map(|s| s.to_string()).collect() } else { Vec::new() };
            hud.frame(&mut Frame { snapshot: &snapshot, sora: &sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &w, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
            layers.update(hud, &stage_m, (2560.0, 1440.0), false, &mut |i, e| {
                if i == status {
                    *shown = matches!(e, LayerEvent::Show { .. });
                }
            });
        }
    };
    // a Goofy start: formation [Goofy], Sora out, his GUARD full
    let f = kh_host::formation::Formation::new(&[PartyChara::Goofy], PartyChara::Goofy).unwrap();
    hud.set_formation(&f);
    hud.set_party(&[PartyHudSlot { chara: PartyChara::Goofy, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: false, focus: None }]);
    hud.set_leader(PartyChara::Goofy, false);
    hud.set_guard(Some((values(&kh_party::Guard::for_level(1)), look())));
    run(&mut hud, &mut layers, &mut shown, 240, &[]);
    assert!(shown, "the status HUD is up");
    // a cutscene: hidden; it ends: shown again although nothing in the status moved
    hud.set_cutscene(true);
    run(&mut hud, &mut layers, &mut shown, 30, &[]);
    assert!(!shown, "hidden in the cutscene");
    hud.set_cutscene(false);
    run(&mut hud, &mut layers, &mut shown, 30, &[]);
    assert!(shown, "THE STATUS HUD IS BACK AFTER THE CUTSCENE");
    // the pause / camp menu open and closed
    run(&mut hud, &mut layers, &mut shown, 80, &["menu open"]);
    run(&mut hud, &mut layers, &mut shown, 80, &["menu open"]);
    run(&mut hud, &mut layers, &mut shown, 60, &[]);
    assert!(shown, "back after the menu");
    // a leader switch to Sora (he joins) and back to Goofy
    hud.set_leader(PartyChara::Sora, true);
    hud.set_guard(None);
    run(&mut hud, &mut layers, &mut shown, 150, &[]);
    assert!(shown, "Sora leads");
    hud.set_leader(PartyChara::Goofy, true);
    hud.set_guard(Some((values(&kh_party::Guard::for_level(1)), look())));
    run(&mut hud, &mut layers, &mut shown, 200, &[]);
    assert!(shown, "Goofy leads again");
    hud.set_cutscene(true);
    run(&mut hud, &mut layers, &mut shown, 10, &[]);
    hud.set_cutscene(false);
    run(&mut hud, &mut layers, &mut shown, 10, &[]);
    assert!(shown, "back after a second cutscene");
}
