//! (ER-HITSFX-AUDIT, THE USER 2026-11-07: "...it should only be KH sound effects they make when fighting ... Make sure audio
//! is never choppy or cut off before it's supposed to.") Against the real audio pack (`build/sora_pack/audio`), engine ->
//! voice commands -> the software mixer the Elden Ring host's device callback runs:
//! - a far sound (beyond 4 m of the listener: its SEAD distance curve asks for a low-pass) is HEARD, at nearly its
//!   unfiltered level (the mixer's low-pass held its state frozen: such a voice was silent from its start);
//! - a dense multi-hit combo of the whole party (Sora, Donald, Goofy) and two Mimic copies, three hits each in one frame:
//!   no sound is stopped before its end unless a KH3 rule says so (a SEAD start macro, a category pool, the asset's UE
//!   concurrency), and every such cut is named;
//! - the mixer renders that fight, and a stress of 256 voices, well inside each 256-frame block's real-time budget.
//!
//! Every test SKIPS, saying so, when the pack is not there (the user's local data).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use glam::Vec3;
use kh_audio::copies::{BOSS_SORA, MIMIC_SORA, PARTY_HANDLE_BASE, PARTY_HANDLE_SPAN};
use kh_audio::mixer::RATE;
use kh_audio::{Engine, Language, Listener, Materials, Mixer, Pack, SeamPlay, SoundKey, VoiceCommand, VoiceId, VoiceParams};

const DT: f32 = 1.0 / 60.0;
/// The block the device callback renders (threaded.rs `Source`: 256 frames a pull) and its real-time budget.
const BLOCK: usize = 256;
const BUDGET: Duration = Duration::from_micros(BLOCK as u64 * 1_000_000 / RATE as u64);

fn pack() -> Option<Arc<Pack>> {
    static PACK: OnceLock<Option<Arc<Pack>>> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = Path::new(env!("KH_AUDIO_PACK_DIR"));
        if !dir.join("manifest.json").is_file() {
            eprintln!("SKIPPED: no audio pack at {}", dir.display());
            return None;
        }
        Some(Arc::new(Pack::load(dir).expect("the audio pack loads")))
    })
    .clone()
}

fn seam(name: &str, position: Vec3) -> SeamPlay<'_> {
    SeamPlay { name, position, volume: 1.0, pitch: 1.0, looping: false, spatial: true, anim: None }
}

fn party_handle(member: u8, n: u32) -> u32 {
    PARTY_HANDLE_BASE + u32::from(member) * PARTY_HANDLE_SPAN + n
}

fn voice_line(cue: &str) -> String {
    format!("/Game/Sound/VOICE/Localization/ja/battle/PC/{cue}.{cue}")
}

/// One voice the engine started: its sound, who asked, the step it started and the step its material runs out.
struct Started {
    key: SoundKey,
    who: String,
    ends: f64,
}

/// The engine, the mixer behind it, and what the run saw.
struct Run {
    engine: Engine,
    mixer: Mixer,
    materials: Materials,
    pack: Arc<Pack>,
    listener: Listener,
    step: u64,
    /// key -> (who asked, the engine's cause of it).
    keys: HashMap<SoundKey, (String, String)>,
    voices: HashMap<VoiceId, Started>,
    /// Sounds cut before their end: (step, who, cause, voice frames left).
    cuts: Vec<(u64, String, String, f64)>,
    /// A track released inside its own sound (SEAD key-on / key-off: the sound's own data, A4).
    releases: usize,
    /// The KH3 rules' log lines: steal / macro-stop / concurrency-stop (cuts) and cancel / drop / refused (never started).
    rules: Vec<kh_audio::LogLine>,
    started: usize,
    frames_due: usize,
    block: Vec<f32>,
    block_times: Vec<Duration>,
    peak: f32,
    /// The most voices the mixer held at once.
    most: usize,
}

impl Run {
    fn new(pack: Arc<Pack>) -> Run {
        let mut mixer = Mixer::new();
        mixer.set_lowpass(true); // as the Elden Ring host opens it (fx.rs: Output::device(pack, true, ..))
        Run { engine: Engine::new(pack.clone(), 3, Language::En), mixer, materials: Materials::default(), pack, listener: Listener::default(), step: 0, keys: HashMap::new(), voices: HashMap::new(), cuts: Vec::new(), releases: 0, rules: Vec::new(), started: 0, frames_due: 0, block: vec![0.0; BLOCK * 2], block_times: Vec::new(), peak: 0.0, most: 0 }
    }

    fn mark(&mut self, who: &str, key: Option<SoundKey>) {
        if let Some(key) = key {
            let cause = self.engine.active().into_iter().find(|a| a.key == key).map(|a| a.cause).unwrap_or_default();
            self.keys.insert(key, (who.to_string(), cause));
        }
    }

    /// One simulation step: the engine's voice commands into the mixer, every stop classified, a step of audio rendered
    /// in device blocks (each timed).
    fn step(&mut self) {
        self.step += 1;
        let commands = self.engine.update(DT, &self.listener, &|_| None);
        let mut stopped = Vec::new();
        for command in &commands {
            match *command {
                VoiceCommand::Start { voice, sound, bank, material, offset, params, .. } => {
                    self.started += 1;
                    let frames = self.pack.bank(bank).materials.get(&material).map_or(0.0, |m| m.samples as f64);
                    let rate = self.pack.bank(bank).materials.get(&material).map_or(f64::from(RATE), |m| f64::from(m.sample_rate));
                    let seconds = (frames - offset).max(0.0) / (rate * f64::from(params.pitch.max(0.01)));
                    let who = self.keys.get(&sound).map_or("?".to_string(), |k| k.0.clone());
                    self.voices.insert(voice, Started { key: sound, who, ends: self.step as f64 + seconds / f64::from(DT) });
                }
                VoiceCommand::Stop { voice } => stopped.push(voice),
                _ => {}
            }
            self.mixer.apply(command, &self.pack, &mut self.materials);
        }
        for voice in stopped {
            let Some(v) = self.voices.get(&voice) else { continue };
            if self.engine.driver.is_playing(v.key) {
                self.releases += 1;
                continue;
            }
            let left = v.ends - self.step as f64;
            if left > 1.0 {
                let cause = self.keys.get(&v.key).map_or(String::new(), |k| k.1.clone());
                self.cuts.push((self.step, v.who.clone(), cause, left));
            }
        }
        for line in self.engine.take_log() {
            if matches!(line.kind, "steal" | "macro-stop" | "concurrency-stop" | "cancel" | "drop") || (line.kind == "silent" && (line.note.contains("SEAD") || line.note.contains("concurrency"))) {
                self.rules.push(line);
            }
        }
        self.frames_due += RATE as usize / 60;
        while self.frames_due >= BLOCK {
            self.frames_due -= BLOCK;
            let t = Instant::now();
            self.mixer.render(&mut self.block);
            self.block_times.push(t.elapsed());
            self.most = self.most.max(self.mixer.voices());
            self.peak = self.peak.max(self.block.iter().fold(0.0f32, |m, s| m.max(s.abs())));
        }
    }

    fn mean_block(&self) -> Duration {
        self.block_times.iter().sum::<Duration>() / self.block_times.len().max(1) as u32
    }
}

/// The energy of a mixer's output over `seconds` of one sound started by `play` (a fresh engine and mixer each time).
fn energy_of(pack: &Arc<Pack>, lowpass: bool, play: impl Fn(&mut Engine)) -> (f64, f32) {
    let mut engine = Engine::new(pack.clone(), 1, Language::En);
    let mut mixer = Mixer::new();
    mixer.set_lowpass(lowpass);
    let mut materials = Materials::default();
    play(&mut engine);
    let mut energy = 0.0f64;
    let mut lowpass_param: f32 = 1.0;
    let mut block = vec![0.0f32; 800 * 2];
    for _ in 0..90 {
        for command in engine.update(DT, &Listener::default(), &|_| None) {
            if let VoiceCommand::Start { params, .. } | VoiceCommand::Update { params, .. } = command {
                lowpass_param = lowpass_param.min(params.lowpass);
            }
            mixer.apply(&command, pack, &mut materials);
        }
        mixer.render(&mut block);
        energy += block.iter().map(|s| f64::from(*s) * f64::from(*s)).sum::<f64>();
    }
    (energy, lowpass_param)
}

/// DONALD'S HIT 10 m away: past the 4 m inner radius, so its SEAD distance curve asks the mixer for a low-pass (every
/// sequence of Donald's and Goofy's weapon banks has one, as do the battle voices, the magic, the action SEs and the
/// footsteps: 3293 of the pack's 4099 sequences). It must be heard at nearly the level it has unfiltered. Before the fix
/// the low-pass branch only ever output its frozen state: zero from the voice's start, so every such sound starting
/// beyond 4 m of the camera was silent, and one that crossed 4 m froze to a held sample (a click, then nothing).
#[test]
fn a_far_hit_is_heard_through_the_low_pass() {
    let Some(pack) = pack() else { return };
    let far = Vec3::new(1000.0, 0.0, 0.0);
    let play = |e: &mut Engine| {
        e.party_seam_play(0, party_handle(0, 1), &seam("HIT1", far), None).expect("Donald's HIT1 plays");
    };
    let (filtered, factor) = energy_of(&pack, true, play);
    let (plain, _) = energy_of(&pack, false, play);
    eprintln!("Donald's HIT1 at 10 m: low-pass factor {factor:.3}; energy {filtered:.4} with the low-pass, {plain:.4} without");
    assert!(factor < 0.999, "the distance curve asks for a low-pass at 10 m ({factor})");
    assert!(plain > 1.0e-3, "the hit is audible at 10 m unfiltered");
    assert!(filtered > 0.5 * plain, "the low-passed hit keeps its level: {filtered} vs {plain}");
    // the same sound walking across 4 m (filtered, then not, then filtered again) never freezes to a held value
    let mut engine = Engine::new(pack.clone(), 1, Language::En);
    let mut mixer = Mixer::new();
    let mut materials = Materials::default();
    let line = voice_line("damage_big1");
    let key = engine.seam_play(1, &seam(&line, Vec3::new(1000.0, 0.0, 0.0))).expect("the line plays");
    let mut block = vec![0.0f32; 800 * 2];
    let mut held = 0usize;
    let mut last = f32::NAN;
    for s in 0..60 {
        let x = if (s / 10) % 2 == 0 { 1000.0 } else { 200.0 };
        engine.driver.set_position(key, Vec3::new(x, 0.0, 0.0));
        for command in engine.update(DT, &Listener::default(), &|_| None) {
            mixer.apply(&command, &pack, &mut materials);
        }
        mixer.render(&mut block);
        for frame in block.chunks(2) {
            if frame[0] != 0.0 && frame[0] == last {
                held += 1;
            }
            last = frame[0];
        }
    }
    // (a frozen filter repeats one value for every frame it is far: half of these 48 000; a real wave repeats a value rarely)
    assert!(held < 48_000 / 20, "no frozen output while it crosses 4 m: {held} repeated non-zero samples");
}

/// THE WHOLE PARTY AND TWO MIMIC COPIES IN ONE DENSE COMBO: Sora, Donald and Goofy (KH3's own world: scope 0) and the boss's
/// Sora copy and the player's Mimic Sora (their own instances), each swinging and hitting THREE victims in the same frame,
/// six times, with battle voice lines between. Every sound that started plays to its end unless a KH3 rule cut it, and the
/// test names those rules from the engine's log. Rendered through the mixer as the device callback does, block by block.
#[test]
fn a_dense_party_and_copies_combo_cuts_nothing_but_kh3_rules() {
    let Some(pack) = pack() else { return };
    let mut run = Run::new(pack.clone());
    let sora = Vec3::new(300.0, 0.0, 0.0);
    let donald = Vec3::new(600.0, 300.0, 0.0);
    let goofy = Vec3::new(600.0, -300.0, 0.0);
    let copy = Vec3::new(900.0, 0.0, 0.0);
    let mimic = Vec3::new(800.0, 400.0, 0.0);
    let victims = [Vec3::new(450.0, 50.0, 0.0), Vec3::new(500.0, -80.0, 0.0), Vec3::new(700.0, 0.0, 0.0)];
    let lines = [voice_line("damage_big1"), voice_line("damage_small1"), voice_line("damage_big2")];
    let mut n = 0u32;
    for attack in 0..6u32 {
        let (swing, hit) = if attack % 2 == 0 { ("SWING1", "HIT1") } else { ("SWING2", "HIT2") };
        // Sora (the player)
        n += 1;
        let k = run.engine.seam_play(n, &seam(swing, sora));
        run.mark("Sora", k);
        for v in victims {
            n += 1;
            let k = run.engine.seam_play(n, &seam(hit, v));
            run.mark("Sora", k);
        }
        // Donald and Goofy (their own weapon sets; hits at the impact points)
        for (member, name, at) in [(0u8, "Donald", donald), (1u8, "Goofy", goofy)] {
            n += 1;
            let k = run.engine.party_seam_play(member, party_handle(member, n), &seam(swing, at), None);
            run.mark(name, k);
            for v in victims {
                n += 1;
                let k = run.engine.party_seam_play(member, party_handle(member, n), &seam(hit, v), None);
                run.mark(name, k);
            }
        }
        // the two copies: swings on them, hits on their victims (their handles, outside their member's call)
        for (member, name, at) in [(BOSS_SORA, "the boss's Sora copy", copy), (MIMIC_SORA, "Mimic Sora", mimic)] {
            n += 1;
            let k = run.engine.party_seam_play(member, party_handle(member, n), &seam(swing, at), None);
            run.mark(name, k);
            for v in victims {
                n += 1;
                let k = run.engine.seam_play(party_handle(member, n), &seam(hit, v));
                run.mark(name, k);
            }
        }
        // battle voice lines
        if attack % 3 == 0 {
            let line = &lines[(attack / 3) as usize % lines.len()];
            n += 1;
            let k = run.engine.seam_play(n, &seam(line, sora));
            run.mark("Sora", k);
            for (member, name, at) in [(BOSS_SORA, "the boss's Sora copy", copy), (MIMIC_SORA, "Mimic Sora", mimic)] {
                n += 1;
                let k = run.engine.party_seam_play(member, party_handle(member, n), &seam(line, at), None);
                run.mark(name, k);
            }
        }
        if attack == 1 || attack == 4 {
            for (member, name, at, group) in [(0u8, "Donald", donald, "QuarterHP:1"), (1u8, "Goofy", goofy, "QuarterHP:2")] {
                n += 1;
                let k = run.engine.party_seam_play(member, party_handle(member, n), &seam(group, at), None);
                run.mark(name, k);
            }
        }
        for _ in 0..12 {
            run.step();
        }
    }
    // until everything has played out
    for _ in 0..(8.0 / DT) as usize {
        run.step();
    }
    let who_started: HashMap<&str, usize> = run.voices.values().fold(HashMap::new(), |mut m, v| {
        *m.entry(v.who.as_str()).or_default() += 1;
        m
    });
    eprintln!("voices started: {} ({who_started:?}); tracks released inside their own sound (SEAD data): {}", run.started, run.releases);
    for line in &run.rules {
        eprintln!("KH3 rule: {} | {} | {} | {}", line.kind, line.cause, line.sound, line.note);
    }
    for who in ["Sora", "Donald", "Goofy", "the boss's Sora copy", "Mimic Sora"] {
        assert!(who_started.get(who).copied().unwrap_or(0) >= 6, "{who}'s sounds played: {who_started:?}");
    }
    // every cut is one of KH3's own rules, logged for that very sound
    let cut_rules: Vec<&kh_audio::LogLine> = run.rules.iter().filter(|l| matches!(l.kind, "steal" | "macro-stop" | "concurrency-stop")).collect();
    for (step, who, cause, left) in &run.cuts {
        let rule = cut_rules.iter().find(|l| &l.cause == cause);
        eprintln!("cut at step {step}: {who} {cause} ({:.0} ms left) by {}", left * f64::from(DT) * 1000.0, rule.map_or("NO KH3 RULE".to_string(), |l| format!("{} ({})", l.kind, l.note)));
        assert!(rule.is_some(), "{who}'s {cause} was cut at step {step} with {:.0} ms left and no KH3 rule names it", left * f64::from(DT) * 1000.0);
    }
    // the requests that never started were refused by KH3's start macros (the same hit within 0.067 s: three victims in
    // one frame) or the asset's PreventNew, each named
    assert!(run.rules.iter().filter(|l| matches!(l.kind, "cancel" | "drop" | "silent")).all(|l| l.note.contains("macro") || l.note.contains("pool") || l.note.contains("concurrency")));
    // nothing went over full scale and the limiter caught the sum
    assert!(run.peak <= 1.0 && run.mixer.clipped == 0, "peak {} clipped {}", run.peak, run.mixer.clipped);
    let max = run.block_times.iter().max().copied().unwrap_or_default();
    eprintln!("mixer: {} blocks of {BLOCK} frames, at most {} voices at once, mean {:?}, max {max:?} (budget {BUDGET:?} each)", run.block_times.len(), run.most, run.mean_block());
    assert!(run.mean_block() < BUDGET / 4, "the fight renders far inside the device's real time: mean {:?} of {BUDGET:?}", run.mean_block());
    assert!(run.cuts.is_empty(), "in this fight no KH3 rule had a sound to cut: {:?}", run.cuts);
    // THE DETECTOR SEES A CUT AND NAMES ITS RULE: Sora's next battle line while his first sounds is KH3's macro 106 (the
    // others of the same bank stop over 0.066 s): one cut, named "macro-stop", nothing else
    let first = run.engine.seam_play(9_000, &seam(&lines[0], sora));
    run.mark("Sora", first);
    for _ in 0..6 {
        run.step();
    }
    // a line of the same priority (macro 106 cancels a lower one instead): the first of these the pack plays
    let candidates = ["bt1010510sr0", "bt1010520sr0", "bt1010530sr0", "damage_big2", "damage_small1"].map(voice_line);
    let second = candidates.iter().enumerate().find_map(|(i, line)| run.engine.seam_play(9_001 + i as u32, &seam(line, sora)));
    run.mark("Sora", second);
    for _ in 0..(3.0 / DT) as usize {
        run.step();
    }
    assert!(first.is_some() && second.is_some(), "both lines start");
    assert_eq!(run.cuts.len(), 1, "the first line was cut once: {:?}", run.cuts);
    let (_, who, cause, _) = &run.cuts[0];
    let rule = run.rules.iter().find(|l| l.kind == "macro-stop" && &l.cause == cause).map(|l| l.note.clone());
    eprintln!("the detector's own check: {who} {cause} cut by {rule:?}");
    assert!(rule.is_some_and(|n| n.contains("106")), "named as KH3's macro 106");
}

/// THE MIXER UNDER LOAD: many looped voices of the pack's materials at once, half of them low-passed, panned and pitched.
/// The shipped mod is the RELEASE build (opt-level 3): there 256 voices (far beyond what KH3's pools let live in one fight)
/// render each 256-frame block in under a quarter of its real-time budget. A test build of this crate is unoptimised (the
/// workspace's dev profile, about ten times slower): there 64 voices (more than the dense fight above holds at once) must
/// still render in under half the budget. Timed best-of-5 so another job on the machine does not decide it. The cost per
/// voice stays flat (it grows with the voices, never faster).
#[test]
fn the_mixer_renders_many_voices_inside_the_block_budget() {
    let Some(pack) = pack() else { return };
    let mut materials = Materials::default();
    let mut buffers = Vec::new();
    'banks: for (b, bank) in pack.banks.iter().enumerate() {
        if !bank.name.contains("/weapon/") {
            continue;
        }
        let mut ids: Vec<&u32> = bank.materials.keys().collect();
        ids.sort();
        for m in ids {
            if let Some(buffer) = materials.get(&pack, b as u32, *m) {
                buffers.push(buffer);
                if buffers.len() >= 32 {
                    break 'banks;
                }
            }
        }
    }
    assert!(!buffers.is_empty(), "weapon materials read");
    let time_for = |voices: usize| -> Duration {
        let mut mixer = Mixer::new();
        for i in 0..voices {
            let params = VoiceParams { volume: 0.05, pitch: 0.8 + (i % 5) as f32 * 0.1, pan: (i % 7) as f32 * 0.1 - 0.3, pan_depth: 1.0, lowpass: if i % 2 == 0 { 0.6 } else { 1.0 }, reverb_send: 0.0 };
            mixer.start(i as VoiceId + 1, buffers[i % buffers.len()].clone(), 0.0, true, params);
        }
        let mut block = vec![0.0f32; BLOCK * 2];
        mixer.render(&mut block);
        let blocks = 40;
        let best = (0..5)
            .map(|_| {
                let t = Instant::now();
                for _ in 0..blocks {
                    mixer.render(&mut block);
                }
                t.elapsed() / blocks
            })
            .min()
            .unwrap_or_default();
        assert_eq!(mixer.voices(), voices, "looped voices all still sound");
        best
    };
    let optimised = !cfg!(debug_assertions);
    let (many, share) = if optimised { (256, 4) } else { (64, 2) };
    let few = time_for(many / 8);
    let lots = time_for(many);
    eprintln!("mixer block of {BLOCK} frames ({} build): {} voices {few:?}, {many} voices {lots:?} (budget {BUDGET:?})", if optimised { "optimised" } else { "unoptimised test" }, many / 8);
    assert!(lots < BUDGET / share, "{many} voices render in under 1/{share} of a block's real time: {lots:?} of {BUDGET:?}");
    assert!(lots.as_secs_f64() < few.as_secs_f64() * 8.0 * 2.0, "the cost grows with the voices, not faster: {few:?} -> {lots:?}");
}

/// A PAUSE (a menu holding the world: the SE categories pause) and its resume fade over the mixer's 5 ms declick instead of
/// cutting the wave mid-cycle (a click on every popup); the paused voice holds its place and goes on from it.
#[test]
fn a_pause_and_its_resume_are_declicked() {
    let mut mixer = Mixer::new();
    let tone: Vec<i16> = (0..48_000).map(|i| ((i as f32 * 0.05).sin() * 20_000.0) as i16).collect();
    let buffer = Arc::new(kh_audio::wav::Buffer { channels: 1, sample_rate: RATE, data: tone, loop_points: None });
    let flat = VoiceParams { volume: 1.0, pitch: 1.0, pan: 0.0, pan_depth: 1.0, lowpass: 1.0, reverb_send: 0.0 };
    mixer.start(1, buffer, 0.0, false, flat);
    let mut out = vec![0.0f32; 2 * 1000];
    mixer.render(&mut out);
    let before = out[2 * 999];
    mixer.pause(1, true);
    mixer.render(&mut out);
    let jump = (out[0] - before).abs();
    assert!(jump < 0.05, "no step at the pause: {jump}");
    assert!(out[2 * 500..].iter().all(|s| *s == 0.0), "held silent after its 5 ms fade");
    let steps = out.chunks(2).take(260).zip(out.chunks(2).skip(1)).map(|(a, b)| (b[0] - a[0]).abs()).fold(0.0f32, f32::max);
    assert!(steps < 0.05, "the fade has no step: {steps}");
    mixer.pause(1, false);
    mixer.render(&mut out);
    assert!(out[0].abs() < 0.01, "the resume starts from silence: {}", out[0]);
    assert!(out[2 * 600..].iter().any(|s| s.abs() > 0.2), "and the voice goes on");
    assert_eq!(mixer.voices(), 1);
}
