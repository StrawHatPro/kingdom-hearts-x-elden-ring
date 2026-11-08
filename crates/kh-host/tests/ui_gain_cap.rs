//! (THE USER 2026-11-02: "sometimes Audio from specifically our mod will become very loud ... shotlock's startup sfx is
//! way too loud") THE MENUS' GAIN (`HostAudio::ui_gain`, +12 dB in the Elden Ring host) against the real audio pack
//! (build/sora_pack/audio), rendered offline through the same mixer the device runs:
//! - a HUD sound is raised only until its authored peak reaches `ui_peak_ceiling` (sheet row sound.ui_peak_ceiling 0.8),
//!   never below KH3's own level: the shotlock aim's se00000_039 stays at x1, the game-over loop gets x1.15, the cursor
//!   still about +10 dB;
//! - after a restart of the engine (a load, a respawn) none of Sora's own sounds takes the menus' gain.
//!   Skipped without the pack.

use std::path::PathBuf;
use std::sync::Arc;

use glam::Vec3;
use kh_audio::{Engine, Language, Listener, Pack};
use kh_core::{Attach, SoundId, SoundPlay};
use kh_host::audio::{db_to_gain, ui_voice_gain, HostAudio, Output};
use kh_core::SoundHandle;
use kh_host::seam::AudioCommand;

fn pack() -> Option<Arc<Pack>> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/audio");
    if !dir.join("manifest.json").is_file() {
        println!("no build/sora_pack/audio: skipped");
        return None;
    }
    Pack::load(&dir).ok().map(Arc::new)
}

/// The authored peak of static SE `id` as KH3 plays it (SEAD volume x material peak).
fn authored(pack: &Arc<Pack>, id: u32) -> f32 {
    let mut engine = Engine::new(pack.clone(), 1, Language::En);
    let key = engine.play_static_se(id).expect("it plays");
    engine.driver.peak_level(key).expect("a measured peak")
}

#[test]
fn ui_gain_is_capped_by_the_sounds_own_peak() {
    let Some(pack) = pack() else { return };
    let gain = db_to_gain(12.0);
    let scope = authored(&pack, 18);
    let over = authored(&pack, 21);
    let cursor = authored(&pack, 0);
    println!("authored peaks: shotlock aim SE 18 {scope:.3}, game-over loop SE 21 {over:.3}, cursor SE 0 {cursor:.3}");
    // before: everything x3.98 (+12 dB)
    assert!(scope * gain > 3.3 && over * gain > 2.7, "the old gain put them far over full scale");
    // now
    assert_eq!(ui_voice_gain(gain, 0.8, Some(scope)), 1.0, "the shotlock aim's sound at KH3's own level");
    assert!((ui_voice_gain(gain, 0.8, Some(over)) - 0.8 / over).abs() < 1.0e-4 && 0.8 / over < 1.2, "the game-over loop barely raised");
    assert!(ui_voice_gain(gain, 0.8, Some(cursor)) > 3.0, "the quiet cursor still raised (~+10 dB)");
    // never below KH3's level, a gain of 1 or less as it is, an unknown peak not raised
    assert_eq!(ui_voice_gain(gain, 0.8, Some(0.99)), 1.0);
    assert_eq!(ui_voice_gain(0.5, 0.8, Some(0.1)), 0.5);
    assert_eq!(ui_voice_gain(gain, 0.8, None), 1.0);
}

fn play(handle: u32, name: &'static str) -> AudioCommand {
    AudioCommand::Play(SoundHandle(handle), SoundPlay { id: SoundId(0x5E00_0000), name, at: Attach::SORA, volume: 1.0, pitch: 1.0, looping: true, spatial: false })
}

fn idle(host: &mut HostAudio, seconds: f32) {
    for _ in 0..(seconds * 60.0) as usize {
        host.idle(1.0 / 60.0, true, Listener::default(), &|_| Vec3::ZERO);
    }
}

/// The game-over loop (static SE 21, the clipping in the user's log at both game overs) through the host as the Elden
/// Ring HUD plays it: before (no ceiling) its mix peaked at 2.76 of full scale (1.38 after the -6 dB output level: the
/// log's 1.457); now at the ceiling 0.8, and the limiter never has to act.
#[test]
fn the_game_over_loop_no_longer_overdrives_the_mix() {
    let Some(pack) = pack() else { return };
    let dir = std::env::temp_dir();
    let mut peaks = Vec::new();
    for ceiling in [f32::INFINITY, 0.8] {
        let path = dir.join(format!("kh_ui_gain_cap_{}.wav", if ceiling.is_finite() { "after" } else { "before" }));
        let mut host = HostAudio::new(pack.clone(), Output::render(pack.clone(), true, &path.to_string_lossy()).expect("a temp file"), 1, Language::En);
        (host.ui_gain, host.ui_peak_ceiling) = (db_to_gain(12.0), ceiling);
        assert_eq!(host.ui_commands(vec![play(0x5F00_0001, "se00000_046")]), 1);
        idle(&mut host, 1.0);
        let window = host.out.take_window();
        peaks.push((window.peak, window.gain, host.out.stats.limited.load(std::sync::atomic::Ordering::Relaxed)));
        host.flush();
        let _ = std::fs::remove_file(&path);
    }
    println!("game-over loop through the host: before {:?}, after {:?} (peak before the limiter, limiter gain, frames limited)", peaks[0], peaks[1]);
    assert!(peaks[0].0 > 2.5 && peaks[0].1 < 0.4, "before: far over full scale, the limiter (new) catches it");
    assert!(peaks[1].0 <= 0.81 && peaks[1].0 > 0.7 && peaks[1].2 == 0, "after: at the ceiling, no limiting");
}

/// After `restart` (a load, a respawn: the new engine counts its keys and voices from 1 again) a sound of Sora's that
/// gets the same key as a menu sound before it is played at its own level, not at the menus' gain.
#[test]
fn a_restart_forgets_the_menus_sounds() {
    let Some(pack) = pack() else { return };
    let path = std::env::temp_dir().join("kh_ui_gain_restart.wav");
    let mut host = HostAudio::new(pack.clone(), Output::render(pack.clone(), true, &path.to_string_lossy()).expect("a temp file"), 1, Language::En);
    host.ui_gain = db_to_gain(12.0);
    // a menu sound (the cursor, authored peak 0.26: raised x3.8 to the default ceiling 1) holds key 1 of the first engine
    host.ui_commands(vec![play(0x5F00_0001, "se00000_001")]);
    idle(&mut host, 0.5);
    let raised = host.out.take_window().peak;
    host.restart(1);
    idle(&mut host, 0.1);
    let _ = host.out.take_window();
    // the new engine's key 1: the same sound, but a world sound of the host's (not a menu sound)
    host.engine.seam_play(7, &kh_audio::SeamPlay { name: "se00000_001", position: Vec3::ZERO, volume: 1.0, pitch: 1.0, looping: true, spatial: false, anim: None }).expect("it plays");
    idle(&mut host, 0.5);
    let level = host.out.take_window().peak;
    let own = authored(&pack, 0);
    println!("cursor as a menu sound: peak {raised:.3}; after the restart as a world sound with the same key: peak {level:.3} (its authored level {own:.3})");
    assert!(raised > 0.6, "the menu sound was raised: {raised}");
    assert!(level <= own + 0.02, "its own level, not the menus' gain: {level}");
    host.flush();
    let _ = std::fs::remove_file(&path);
}
