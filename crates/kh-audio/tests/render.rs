//! The offline render, without any app: the recorded bone trajectories
//! (tests/data/sora_walk_run_jump.csv) go through the engine (detector -> surface table -> driver)
//! and the mixer, 800 frames per 60 Hz step, and the result is inspected numerically. NOBODY
//! LISTENS HERE: the checks are that sound is where the events are, that it is silent where
//! nothing plays, that nothing clips, and that the step sounds are spaced at the footfall
//! interval. The render is also written next to the test binary's target folder for a person
//! to listen to (`target/kh-audio-render-walk-run-jump.wav`).
//! Skips, saying so, when the audio pack is not there.

use std::path::Path;
use std::sync::Arc;

use glam::Vec3;
use kh_audio::autose::BoneFrame;
use kh_audio::{Engine, Language, Listener, Materials, Mixer, Motion, Pack};

const DT: f32 = 1.0 / 60.0;
const FRAMES: usize = 800;

fn rows() -> Vec<(u32, String, f32, [Vec3; 14], bool)> {
    include_str!("data/sora_walk_run_jump.csv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            let mut p = [Vec3::ZERO; 14];
            for (k, slot) in p.iter_mut().enumerate() {
                let f = |i: usize| c[4 + 3 * k + i].parse::<f32>().unwrap();
                *slot = Vec3::new(f(0), f(1), f(2));
            }
            (c[0].parse().unwrap(), c[2].to_string(), c[3].parse().unwrap(), p, c[1] == "1")
        })
        .collect()
}

#[test]
fn the_recorded_walk_run_and_jump_render_to_step_sounds_at_the_footfall_interval() {
    let dir = Path::new(env!("KH_AUDIO_PACK_DIR"));
    if !dir.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no audio pack at {}", dir.display());
        return;
    }
    let pack = Arc::new(Pack::load(dir).expect("the audio pack loads"));
    let mut engine = Engine::new(pack.clone(), 1, Language::En);
    let mut mixer = Mixer::new();
    let mut materials = Materials::default();
    let mut out: Vec<f32> = Vec::new();
    let mut block = vec![0.0f32; 2 * FRAMES];
    // (step index, motion) of every detected event, and of every voice started.
    let mut events = Vec::new();
    let mut voice_steps = Vec::new();
    for (k, (_, anim, time, positions, grounded)) in rows().into_iter().enumerate() {
        let frame = BoneFrame::from_positions(&positions, grounded);
        // The listener follows 4 m behind and 1.5 m above him, looking along +X: the test level's camera, roughly.
        let listener = Listener { position: positions[0] + Vec3::new(-400.0, 0.0, 150.0), forward: Vec3::X, right: Vec3::Y };
        // Surface 10 (rock); the enable windows of the clips apply.
        for event in engine.auto_se(DT, &frame, 10, Some((anim.as_str(), time)), &listener, &|bone| kh_audio::autose::BONES.iter().position(|b| *b == bone).map(|i| positions[i])) {
            events.push((k, event.motion));
        }
        // The landing clip's explicit notify, as the core sends it through the seam.
        if anim == "Land" && time < 1.5 * DT {
            engine.seam_play(1, &kh_audio::SeamPlay { name: "LandNormal", position: positions[4], volume: 1.0, pitch: 1.0, looping: false, spatial: true, anim: Some((anim.as_str(), time)) });
        }
        for command in engine.update(DT, &listener, &|_| None) {
            if matches!(command, kh_audio::VoiceCommand::Start { .. }) {
                voice_steps.push(k);
            }
            mixer.apply(&command, &pack, &mut materials);
        }
        mixer.render(&mut block);
        out.extend_from_slice(&block);
    }
    assert!(materials.errors.is_empty(), "{:?}", materials.errors);
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("target");
    if target.is_dir() {
        kh_audio::wav::write(&target.join("kh-audio-render-walk-run-jump.wav"), 2, kh_audio::mixer::RATE, &out).ok();
    }

    // 12 s, not clipped, not silent.
    assert_eq!(out.len(), 720 * FRAMES * 2);
    assert_eq!(mixer.clipped, 0);
    assert!(mixer.peak > 0.05 && mixer.peak < 0.9, "peak {}", mixer.peak);
    let rms: Vec<f32> = out.chunks(2 * FRAMES).map(|b| (b.iter().map(|s| s * s).sum::<f32>() / b.len() as f32).sqrt()).collect();

    // Digital silence until the first voice, and whenever no voice is live.
    let first = *voice_steps.first().expect("voices");
    assert!(rms[..first].iter().all(|r| *r == 0.0), "silent before the first sound (step {first})");
    assert!(rms.iter().filter(|r| **r == 0.0).count() > 60, "there are silent stretches (standing still)");

    // Every step sound is in the render: the 4 blocks from its onset hold far more energy than
    // the 4 before it (a footstep is 0.2 .. 0.3 s long and the next one comes 0.27 s later).
    let onsets = |motion: Motion| -> Vec<usize> { events.iter().filter(|e| e.1 == motion).map(|e| e.0).collect() };
    let energy = |from: usize, to: usize| -> f32 { rms[from.min(rms.len())..to.min(rms.len())].iter().map(|r| r * r).sum() };
    for (motion, gap, at_least) in [(Motion::Walk, 32, 6), (Motion::Run, 16, 8)] {
        let at = onsets(motion);
        assert!(at.len() >= at_least, "{motion:?}: {at:?}");
        let regular = at.windows(2).filter(|w| w[1] - w[0] == gap).count();
        assert!(regular >= at_least - 1, "{motion:?} sounds {gap} steps apart: {at:?}");
        for k in &at {
            assert!(voice_steps.contains(k), "{motion:?} at step {k} started a voice");
            assert!(energy(*k, k + 4) > 2.0 * energy(k.saturating_sub(4), *k), "{motion:?} at step {k}: {:.2e} after, {:.2e} before", energy(*k, k + 4), energy(k.saturating_sub(4), *k));
        }
    }
    // The measured spacing of the run's energy peaks: the block of largest RMS in each 16-step
    // window after an onset sits at the same offset from the onset every time (within 2 blocks).
    let run = onsets(Motion::Run);
    let offsets: Vec<usize> = run.iter().map(|k| (0..8).max_by(|a, b| rms[k + a].total_cmp(&rms[k + b])).unwrap()).collect();
    let (lo, hi) = (offsets.iter().min().unwrap(), offsets.iter().max().unwrap());
    assert!(hi - lo <= 2, "peak offsets after the run onsets: {offsets:?}");
    // One jump, one landing sound (the notify's: the detector is switched off by the clips' windows).
    assert_eq!(onsets(Motion::Jump).len(), 1);
    assert_eq!(onsets(Motion::LandNormal).len() + onsets(Motion::LandHard).len(), 0);
    let land = rows().iter().position(|r| r.1 == "Land").unwrap();
    assert!(energy(land, land + 6) > 4.0 * energy(land - 6, land), "the landing sounds");
    eprintln!("render: peak {:.3}, clipped {}, {} voices, walk onsets {:?}, run onsets {:?}", mixer.peak, mixer.clipped, voice_steps.len(), onsets(Motion::Walk), run);
}
