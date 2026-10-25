//! (HB) SORA'S STATUS GAUGES ARE AS VIVID AS THE FRIENDS': the user (live party build): "Sora's health, MP and drive
//! bars are too dark, they need to be colourful like the party member bars". Cause: Sora's HP ring (`pcHpGg`) and focus
//! gauge (`pcExGg`) are masked by SPRITES whose shape is filled at alpha 127; kh-ui drew a sprite mask with its paint, so
//! the gauges showed at half strength over the red lag gauge / the dark track. Masks are coverage only now (kh-ui
//! render.rs `set_solid_masks`). The status HUD with Donald and Goofy is drawn through the Elden Ring host's layer path
//! (`draw_movie_in` at the stage root, clipped), and each gauge's pixels (those that change when it is hidden) are
//! compared with the atlas colours. Needs the UI pack (skipped without it).

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::{draw_movie_in, stage_root, Draw, Drawn, Perf};
use kh_host::party::{PartyChara, PartyHudSlot};

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

/// Mean straight colour (over a black scene: what the overlay shows) of the pixels the gauge at `path` draws.
fn gauge_colour(fonts: &kh_ui::Fonts, movie: &kh_ui::Movie, path: &str, solid: bool) -> [f32; 3] {
    kh_ui::render::set_solid_masks(true);
    let full = picture(fonts, movie);
    let mut hidden = movie.clone();
    assert!(hidden.set_visible(path, false), "{path}");
    let without = picture(fonts, &hidden);
    kh_ui::render::set_solid_masks(solid);
    let shown = picture(fonts, movie);
    kh_ui::render::set_solid_masks(true);
    let (mut n, mut sum) = (0f32, [0f32; 3]);
    for ((p, q), s) in full.chunks_exact(4).zip(without.chunks_exact(4)).zip(shown.chunks_exact(4)) {
        if p[3] > 200 && p.iter().zip(q).any(|(x, y)| x.abs_diff(*y) > 24) {
            n += 1.0;
            for c in 0..3 {
                sum[c] += s[c] as f32 * s[3] as f32 / 255.0;
            }
        }
    }
    assert!(n > 200.0, "{path}: only {n} pixels");
    [sum[0] / n, sum[1] / n, sum[2] / n]
}

#[test]
fn sora_gauges_are_as_vivid_as_the_friends() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok((mut hud, fonts)) = Hud::load(&root.join("build/sora_pack/ui")) else {
        eprintln!("no UI pack: skipped");
        return;
    };
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    let slots = [
        PartyHudSlot { chara: PartyChara::Donald, hp: 140, hp_max: 200, mp: 70, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
        PartyHudSlot { chara: PartyChara::Goofy, hp: 160, hp_max: 220, mp: 100, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
    ];
    hud.set_party(&slots);
    hud.set_leader(PartyChara::Sora, true);
    let mut snapshot = sora.ui_snapshot();
    snapshot.gauges.hp = snapshot.gauges.hp_max * 7 / 10;
    snapshot.gauges.mp = snapshot.gauges.mp_max * 6 / 10;
    snapshot.gauges.focus = snapshot.gauges.focus_max / 2;
    for _ in 0..150 {
        hud.frame(&mut Frame { snapshot: &snapshot, sora: &sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
    let movie = hud.movie("HudStatus").expect("HudStatus").clone();
    let hp = gauge_colour(&fonts, &movie, "sttsPc.pcHp.pcHpPack.pcHpGgSet.pcHpGg", true);
    let focus = gauge_colour(&fonts, &movie, "sttsPc.pcEx.pcFocusGg.pcExGg", true);
    let mp = gauge_colour(&fonts, &movie, "sttsPc.pcMp.pcMpPack.pcMpGg", true);
    let friend = gauge_colour(&fonts, &movie, "sttsNpc1.sttsNpc.npcHp.npcHpGg", true);
    let hp_old = gauge_colour(&fonts, &movie, "sttsPc.pcHp.pcHpPack.pcHpGgSet.pcHpGg", false);
    let focus_old = gauge_colour(&fonts, &movie, "sttsPc.pcEx.pcFocusGg.pcExGg", false);
    eprintln!("Sora HP {hp:?} (before {hp_old:?}), focus {focus:?} (before {focus_old:?}), MP {mp:?}; Donald's HP {friend:?}");
    // the atlas: HP ring green (13..154, 191..235, 8..17), focus yellow -> orange -> red, MP blue (96, 85, 255)
    assert!(hp[1] >= 160.0 && hp[1] >= friend[1] * 0.85 && hp[0] < 80.0, "Sora's HP ring is vivid green like the friends': {hp:?} vs {friend:?}");
    assert!(focus[0] >= 220.0 && focus[1] >= 110.0, "the focus gauge is vivid orange / yellow: {focus:?}");
    assert!(mp[2] >= 230.0, "the MP bar is vivid blue: {mp:?}");
    assert!(hp_old[1] < 130.0 && focus_old[0] < 180.0, "the old rule showed them at half strength: {hp_old:?} {focus_old:?}");
}
