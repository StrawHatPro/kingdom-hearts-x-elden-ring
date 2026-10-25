//! (HUD2) THE HP GAUGES THROUGH THE ELDEN RING HOST'S PICTURE PATH, FRAME BY FRAME: the real `Hud` (Sora + Donald +
//! Goofy) is driven with damage sequences; every frame the status HUD is drawn the way the Elden Ring host draws it
//! (a kept `Draw`: only the changed part is redrawn, split rectangles, parallel bands, at the user's 2560 x 1440) AND
//! drawn whole from nothing. Any pixel where the two differ is a picture the player saw that the movie does not hold
//! (a stale part of the gauge): logged per frame, and the frames written where it happens.
//!
//!   cargo run -p kh-host --release --example hud2_hp_layers -- OUT_DIR
use std::io::Write as _;

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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = args.first().map(std::path::PathBuf::from).unwrap_or_else(|| root.join("build/sora_tmp/party/HUD2/layers"));
    std::fs::create_dir_all(&out).unwrap();
    let (mut hud, fonts) = Hud::load(&root.join("build/sora_pack/ui")).expect("the UI pack");
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    let mut slots = [
        PartyHudSlot { chara: PartyChara::Donald, hp: 180, hp_max: 180, mp: 70, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
        PartyHudSlot { chara: PartyChara::Goofy, hp: 220, hp_max: 220, mp: 100, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
    ];
    hud.set_party(&slots);
    hud.set_leader(PartyChara::Sora, true);
    let mut snapshot = sora.ui_snapshot();
    let max = snapshot.gauges.hp_max;
    snapshot.gauges.hp = max;
    let (w, h) = (2560.0f32, 1440.0f32);
    let (stage, view) = stage_root(w, h);
    let perf = Perf { split_dirty: true, bands: 10, band_pixels: 150_000 };
    let mut kept = Draw::default();
    let mut r_kept = kh_ui::render::Renderer::new(fonts.clone());
    let mut r_whole = kh_ui::render::Renderer::new(fonts.clone());
    let mut log = std::fs::File::create(out.join("layers.tsv")).unwrap();
    writeln!(log, "frame\tticks\tsora hp\tdonald hp\tgoofy hp\tdrawn\tdiffering px\tworst").unwrap();
    // (frame, Sora's HP as a share of the max, Donald's, Goofy's)
    let script: Vec<(usize, f32, i32, i32)> = vec![
        (100, 0.8, 150, 190),
        (104, 0.7, 140, 170),
        (106, 0.65, 120, 150),
        (160, 0.9, 170, 210),
        (163, 0.5, 100, 120),
        (250, 0.45, 95, 110),
        (251, 0.40, 90, 100),
        (252, 0.35, 85, 90),
        (330, 0.30, 60, 80),
    ];
    let (mut bad_frames, mut total_bad) = (0, 0usize);
    for frame in 0..460usize {
        for (f, s, d, g) in &script {
            if *f == frame {
                snapshot.gauges.hp = (max as f32 * s) as i32;
                slots[0].hp = *d;
                slots[1].hp = *g;
                hud.set_party(&slots);
            }
        }
        // the Elden Ring host's stage ticks: 1 a frame, now and then 0 or 2 (a frame of 55 fps)
        let ticks = match frame % 11 {
            3 => 2,
            7 => 0,
            _ => 1,
        };
        hud.frame(&mut Frame { snapshot: &snapshot, sora: &sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: ticks as f32 / 60.0, ticks, pressed: PadButtons::NONE, nav: Vec::new() });
        let movie = hud.movie("HudStatus").expect("HudStatus");
        let drawn = draw_movie_in(&mut kept, &mut r_kept, movie, &stage, (w, h), frame == 0, perf, Some(view));
        let mut whole = Draw::default();
        let full = draw_movie_in(&mut whole, &mut r_whole, movie, &stage, (w, h), true, Perf::default(), Some(view));
        let what = match &drawn {
            Drawn::Picture(_, Some(d)) => format!("part {}x{}", d.2, d.3),
            Drawn::Picture(_, None) => "whole".into(),
            Drawn::Same => "same".into(),
            Drawn::Empty => "empty".into(),
        };
        let (mut n, mut worst) = (0usize, 0u8);
        if let (Drawn::Picture(..) | Drawn::Same, Drawn::Picture(..)) = (&drawn, &full) {
            if kept.rgba.len() == whole.rgba.len() {
                for (a, b) in kept.rgba.iter().zip(&whole.rgba) {
                    let d = a.abs_diff(*b);
                    if d > 2 {
                        n += 1;
                        worst = worst.max(d);
                    }
                }
            } else {
                n = usize::MAX;
            }
        }
        if n > 0 {
            bad_frames += 1;
            total_bad += n.min(1 << 30);
        }
        writeln!(log, "{frame}\t{ticks}\t{}\t{}\t{}\t{what}\t{n}\t{worst}", snapshot.gauges.hp, slots[0].hp, slots[1].hp).unwrap();
    }
    println!("{bad_frames} frame(s) where the kept picture differs from the whole one ({total_bad} byte(s)); log {}", out.join("layers.tsv").display());
}
