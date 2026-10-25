//! THE OFFLINE PROOF OF A HOST'S SOUND (the Elden Ring mod's audio path without the game):
//! `kh-core` on a flat floor with a training dummy (`kh_host::rig`), the pose of every step,
//! `kh_host::audio::HostAudio` (the same object kh-er runs) and its output: rendered to a WAV
//! file and checked numerically, or played on the default output device in real time through
//! the threaded output the mod uses.
//!
//! ```text
//! cargo run --release -p kh-host --example sora_sound -- --render out.wav [--log out.csv]
//! cargo run --release -p kh-host --example sora_sound -- --play            (listen; device path)
//!     [--script "stop 0.5; run 1.5; ..."] [--surface N] [--volume-db -6] [--pack DIR] [--loadout FILE]
//! ```
//!
//! THE CHECKS of a render (nobody listens here): nothing clips; it is silent before the first
//! sound; every footstep the detector reports during the plain run has its energy in the render
//! right at that step and the steps are evenly spaced; the jump, the three swings with their
//! hits on the dummy, the cast's voice and the fire ball's sounds each start voices in their
//! step windows. Exit code 1 when a check fails.

use std::path::PathBuf;
use std::sync::Arc;

use glam::Vec3;
use kh_audio::{Language, Listener};
use kh_host::audio::{db_to_gain, HostAudio, Output, Step, FRAMES_PER_STEP};
use kh_host::rig::{component_to_world, Rig, Script, STEP};
use kh_host::seam::EffectQueue;

const DEFAULT_SCRIPT: &str = "stop 0.5; run 1.6; jump; run 0.9; stop 0.6; tap a; stop 0.5; tap a; stop 0.5; tap a; stop 1.2; cast firaga; stop 2.5";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|k| args.get(k + 1)).cloned();
    let project = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let pack_dir = value("--pack").map(PathBuf::from).unwrap_or_else(|| project.join("build").join("sora_pack"));
    let loadout = value("--loadout").map(PathBuf::from).unwrap_or_else(|| project.join("sheets").join("port").join("sora-loadout-99.json"));
    let render = value("--render");
    let play = args.iter().any(|a| a == "--play");
    let surface: u32 = value("--surface").and_then(|v| v.parse().ok()).unwrap_or(0);
    let volume_db: f32 = value("--volume-db").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let script_text = value("--script").unwrap_or_else(|| DEFAULT_SCRIPT.to_string());
    let custom_script = value("--script").is_some();
    if render.is_none() && !play {
        eprintln!("sora_sound: give --render FILE.wav or --play");
        std::process::exit(2);
    }

    let script = Script::parse(&script_text).unwrap_or_else(|e| {
        eprintln!("sora_sound: script: {e}");
        std::process::exit(2);
    });
    // The dummy stands where the run and the jump end, so that the combo reaches it.
    let mut rig = Rig::new(&pack_dir, &loadout, None).unwrap_or_else(|e| {
        eprintln!("sora_sound: {e}");
        std::process::exit(2);
    });
    let mut pack = kh_audio::Pack::load(&pack_dir.join("audio")).unwrap_or_else(|e| {
        eprintln!("sora_sound: audio pack: {e}");
        std::process::exit(2);
    });
    // (ER77) `--outfit kh1`: that outfit's own voice (PACK/audio/outfits/kh1/voice.json) in place of every KH3 voice line
    let outfit = value("--outfit");
    if outfit.is_some() {
        for line in pack.load_outfit_voices() {
            println!("sora_sound: {line}");
        }
    }
    let pack = Arc::new(pack);
    println!("sora_sound: audio pack: {} banks, {} sound assets, {} triggers", pack.banks.len(), pack.assets.len(), pack.triggers.len());
    let out = match &render {
        Some(path) => Output::render(pack.clone(), true, path).unwrap_or_else(|e| {
            eprintln!("sora_sound: cannot create {path}: {e}");
            std::process::exit(2);
        }),
        None => Output::device(pack.clone(), true, |text| println!("{text}")),
    };
    let mut audio = HostAudio::new(pack, out, 1, Language::En);
    audio.out.set_master(db_to_gain(volume_db));
    if let Some(path) = value("--log") {
        audio.log_to(&path).expect("the log file");
    }

    let mut effects = EffectQueue::default();
    // Per step: (motion sounds detected, voices started so far, state name, on floor, x)
    let mut rows: Vec<(u64, u64, String, bool, f32)> = Vec::new();
    let mut autos: Vec<(usize, String)> = Vec::new();
    let started = std::time::Instant::now();
    let mut work = std::time::Duration::ZERO;
    let total = script.len() + 30;
    let mut cast_at = None;
    for k in 0..total {
        // The dummy appears 1.8 m in front of him when the run is over (the first attack tap).
        if rig.dummy.is_none() && !custom_script && k >= 216 {
            let facing = Vec3::new(rig.out.yaw.to_radians().cos(), rig.out.yaw.to_radians().sin(), 0.0);
            rig.dummy = Some(kh_host::rig::Dummy::at(Vec3::new(rig.out.position.x, rig.out.position.y, 0.0) + facing * 180.0));
        }
        let before = std::time::Instant::now();
        rig.step(&script, k, 0.0, &mut audio.queue, &mut effects);
        if cast_at.is_none() && !rig.casts.is_empty() {
            cast_at = Some(k);
        }
        let position = rig.out.position;
        // The listener: 4 m behind and 1.5 m above him, looking along +X (the test level's camera, roughly).
        let listener = Listener { position: position + Vec3::new(-400.0, 0.0, 150.0), forward: Vec3::X, right: Vec3::Y };
        let detected_before = audio.auto_events;
        let status = rig.sora.status();
        let hp = (status.hp, status.max_hp());
        let to_world = component_to_world(&rig.out);
        let commands = std::mem::take(&mut effects.0);
        audio.engine.set_voice_outfit(outfit.as_deref());
        audio.step(&Step { out: &rig.out, dt: STEP, skeleton: rig.animator.skeleton(), pose: &rig.pose, component_to_world: &to_world, effects: &commands, surface, listener, hp: Some(hp), place: &|at| rig.place(at) });
        work += before.elapsed();
        if audio.auto_events != detected_before {
            autos.push((k, audio.last_auto.clone().unwrap_or_default()));
        }
        rows.push((audio.auto_events, audio.out.stats.started.load(std::sync::atomic::Ordering::Relaxed), format!("{:?}", rig.out.state), rig.out.on_floor, position.x));
        if play {
            // Real time: the device pulls on its own thread.
            let due = std::time::Duration::from_secs_f32((k + 1) as f32 * STEP);
            if let Some(wait) = due.checked_sub(started.elapsed()) {
                std::thread::sleep(wait);
            }
        }
    }
    audio.flush();
    for name in &audio.unknown {
        println!("sora_sound: no route for \"{name}\"");
    }
    println!("sora_sound: {total} steps, {:.3} ms of host work per step (core + pose + audio engine); seam sounds asked {} (nothing played for {}), motion sounds {}; {}", work.as_secs_f64() * 1000.0 / total as f64, audio.played, audio.missing, audio.auto_events, audio.out.describe());
    println!("sora_sound: casts {:?}; dummy hits {}", rig.casts, rig.dummy.map_or(0, |d| d.hits));
    for (k, text) in &autos {
        println!("  step {k:4}  {text}");
    }
    if play {
        std::thread::sleep(std::time::Duration::from_millis(400));
        println!("sora_sound: played: {}", audio.out.describe());
        return;
    }

    // ---- the numeric checks of the render
    let path = render.unwrap();
    drop(audio);
    let wav = kh_audio::wav::read(std::path::Path::new(&path)).expect("the render reads back");
    let samples: Vec<f32> = wav.data.iter().map(|s| *s as f32 / 32768.0).collect();
    let rms: Vec<f32> = samples.chunks(2 * FRAMES_PER_STEP).map(|b| (b.iter().map(|s| s * s).sum::<f32>() / b.len() as f32).sqrt()).collect();
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let energy = |from: usize, to: usize| -> f32 { rms[from.min(rms.len())..to.min(rms.len())].iter().map(|r| r * r).sum() };
    let voices_in = |from: usize, to: usize| -> u64 { rows[to.min(rows.len() - 1)].1 - rows[from.min(rows.len() - 1)].1 };
    let mut failed = 0;
    let mut check = |ok: bool, text: String| {
        println!("  [{}] {text}", if ok { "ok" } else { "FAILED" });
        if !ok {
            failed += 1;
        }
    };
    println!("sora_sound: checks of {path} ({} steps of audio, peak {peak:.3}):", rms.len());
    check(rms.len() == total, format!("the render holds one block of {FRAMES_PER_STEP} frames per step ({} of {total})", rms.len()));
    check(peak < 0.999, format!("nothing clips (peak {peak:.3})"));
    let first = rows.iter().position(|r| r.1 > 0).unwrap_or(0);
    check(first > 0 && rms[..first].iter().all(|r| *r == 0.0), format!("digital silence until the first voice (step {first})"));
    if !custom_script {
        // The plain run: steps 30..126 (before the jump).
        let run: Vec<usize> = autos.iter().filter(|(k, text)| text.starts_with("Run ") && *k < 126).map(|(k, _)| *k).collect();
        let gaps: Vec<usize> = run.windows(2).map(|w| w[1] - w[0]).collect();
        check(run.len() >= 4, format!("the run has footsteps: at steps {run:?}"));
        check(!gaps.is_empty() && gaps.iter().max().unwrap() - gaps.iter().min().unwrap() <= 2, format!("evenly spaced: gaps {gaps:?} steps"));
        let loud = run.iter().filter(|k| energy(**k, **k + 4) > 2.0 * energy(k.saturating_sub(4), **k)).count();
        check(loud + 1 >= run.len(), format!("each footstep's energy starts at its step ({loud} of {})", run.len()));
        let jump = autos.iter().find(|(_, text)| text.starts_with("Jump "));
        check(jump.is_some(), format!("the jump's sound is detected: {jump:?}"));
        check(autos.iter().any(|(k, text)| text.starts_with("Land") && *k > 126) || voices_in(140, 216) > 0, "a landing sound after the jump".to_string());
        // The combo: three taps at steps 216, 246, 276; each swing and its hit start voices.
        for (n, at) in [216usize, 246, 276].into_iter().enumerate() {
            check(voices_in(at, at + 30) >= 2, format!("attack {} (tap at step {at}): {} voices started in the next 0.5 s (swing, hit, voice)", n + 1, voices_in(at, at + 30)));
        }
        let cast = cast_at.unwrap_or(348);
        check(voices_in(cast, cast + 90) >= 2, format!("Firaga (cast at step {cast}): {} voices started in the next 1.5 s", voices_in(cast, cast + 90)));
        check(energy(cast, cast + 90) > 10.0 * energy(cast - 20, cast), format!("Firaga is in the render: energy {:.2e} after the cast, {:.2e} in the 20 steps before", energy(cast, cast + 90), energy(cast - 20, cast)));
    }
    if failed > 0 {
        println!("sora_sound: {failed} CHECK(S) FAILED");
        std::process::exit(1);
    }
    println!("sora_sound: all checks passed");
}
