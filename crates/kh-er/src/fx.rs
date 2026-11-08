//! (fx) SORA'S SOUND AND VISUAL EFFECTS in the Elden Ring host. State note:
//! build/sora_tmp/er/IMPL_ER_FX_STATE.md. Owned by the sound / effects work; sora.rs calls in
//! here through a few marked lines (`(fx)`) and nothing here touches the gameplay hookup.
//!
//! SOUND. The core's `Audio` seam is `Fx::audio` (a queue); after every simulation step
//! `after_step` hands the step to `kh_host::audio::HostAudio`, the same object the offline proof
//! runs (`cargo run --release -p kh-host --example sora_sound`): `kh_audio::Engine` (footsteps
//! by surface, swings, hits, voices, spell sounds, loops, 3D against the listener = Elden Ring's
//! camera, the shotlock's slow-motion sound states, the low-HP alarm), whose voice commands go
//! through a queue to OUR OWN output: `kh_audio::Mixer` on the default output device (rodio /
//! cpal, WASAPI shared mode; not the game's Wwise), on threads of ours. Nothing on the game's
//! thread reads a file, waits for a lock held by another thread, or calls the audio device.
//!
//! CONTROL KEYS (er\mods\kh-er.ini, (INI) once kh-er.ctl; read while the game runs):
//! - `snd = 0`: Sora's sound off (everything stops; nothing is evaluated);
//! - `snd.vol = dB`: output level (default: sheet row `sound.volume_db`, -6);
//! - `snd.alarm = 0`: no low-HP alarm;
//! - `snd.surface = N`: every step sounds as KH3 surface N (testing);
//! - `snd.lang = ja`: Japanese voices (read when the sound starts).
//!   One line `sound: ...` a second says what is active; `surface: ...` is logged once for every
//!   new ground material the game reports under the player.
//!
//! EFFECTS. The core's `Effects` seam is `Fx::effects` (a queue); after every simulation step
//! the step's commands go to `kh_host::vfx::EffectHost` (what a spawn means, the sockets of the
//! step's pose, the Keyblade trail, targets = Elden Ring's characters, one evaluator step), and
//! once a frame (`draw_drawn`, PostPhysics) its draw list goes to `kh_er_render::vfx`, which draws it
//! with the game's own effect shaders in OUR command list right after Sora (the renderer's
//! scene arrangement). Hit sparks and swing trails on Elden Ring's enemies come the same way:
//! the core spawns them at the hit points it computes.
//! - `vfx = 0`: no effects (everything alive ends; nothing is evaluated or drawn);
//! - `vfx.distort = 0`: no distortion (refraction) twins: no copy of the scene colour is made;
//! - `vfx.decals = 0`: no projected decals; `vfx.hdr = x`: a factor on the effects' brightness
//!   (default: sheet row `vfx.hdr_scale`); `vfx.solid = 1`: diagnosis, the geometry in magenta.
//! - (IMPL_ER39) `fx.light = 0`: the effects' particle lights light nothing; `r.hero_light = 0 / 1 / 2`: the hero
//!   light in the dark off / automatic (default) / always on; `fx.light_gain`, `hero.intensity`: their strength.
//!   REQUESTS OF THE EFFECTS THAT THIS HOST DOES NOT CARRY OUT YET (logged as `vfx: REQUEST ...`):
//!   camera shakes (`EffectHost::take_shakes`: class, scale, duration) and the scene darkening of
//!   Thunder and the grand spells (`EffectHost::scene_light`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eldenring::cs::PlayerIns;
use glam::Vec3;
use kh_audio::{Language, Listener};
use kh_core::{Attach, Output, Status};
use kh_host::audio::{db_to_gain, HostAudio, Step};
use kh_host::audio::PoseTap;
use kh_host::seam::{AudioQueue, EffectCommand, EffectQueue};
use kh_host::vfx::{AssetGeo, EffectHost, Scene};

use crate::control::Control;
use crate::coords::{self, Frame};
use crate::erlog;
use crate::fxmap::FxParams;
use crate::targets::ErTargets;

// (ER party S5) Donald's and Goofy's sounds, voices and effects (the party half of this file)
#[cfg(feature = "party")]
#[path = "fx_party.rs"]
pub mod party;
// (MIMIC-FX) the Mimic Tears' copies' own effects and sounds: ONE shared route for every copy (its own member id, handles,
// taps, Sora instance on the effect side, audio state)
#[cfg(feature = "party")]
#[path = "copyfx.rs"]
pub mod copyfx;

/// The audio side: made once per payload by a thread of ours (the pack's tables are read
/// there), kept over Sora's restarts (a respawn), dropped at the payload's shutdown.
static AUDIO: Mutex<Option<HostAudio>> = Mutex::new(None);
static AUDIO_ASKED: AtomicBool = AtomicBool::new(false);

fn log(text: &str) {
    erlog!("{text}");
}

/// The effect side: made once per payload by a thread of ours (the effect pack's tables and
/// Sora's own effects are parsed there: seconds), kept over Sora's restarts.
static EFFECTS: Mutex<Option<EffectHost>> = Mutex::new(None);
static EFFECTS_ASKED: AtomicBool = AtomicBool::new(false);

fn start_effects(pack_dir: std::path::PathBuf, xsi: bool) {
    if EFFECTS_ASKED.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new().name("kh-vfx-load".to_string()).spawn(move || {
        let made = std::panic::catch_unwind(move || {
            let started = std::time::Instant::now();
            match EffectHost::load(&pack_dir) {
                Ok(mut host) => {
                    let registered = host.preload();
                    // (LDI) THE SAVE POINT'S FIVE SYSTEMS parsed now, with Sora's: their materials and meshes are named to the
                    // renderer below and made ready before the first save point is drawn (before: at its first spawn)
                    let sp = kh_host::savepoint::SavePointData::load(&pack_dir);
                    let sp_names = [sp.base.as_str(), sp.body.as_str(), sp.limit_base.as_str(), sp.limit_body.as_str(), sp.heal.as_str()];
                    let sp_known = host.preload_names(&sp_names);
                    erlog!("vfx: (LDI) the save point's effects preloaded ({sp_known} of {})", sp_names.len());
                    // (ER party S5) THE PARTY'S SECOND ROOT (PACK/party/vfx, with its own shading table: the renderer keeps
                    // its materials and meshes apart as "party:<name>"; the main pack's tables and names are untouched)
                    #[cfg(feature = "party")]
                    match host.load_party_root(&pack_dir) {
                        Ok(line) => {
                            let n = host.preload_party();
                            erlog!("vfx: (party) {line}; {n} of them parsed");
                        }
                        Err(e) => erlog!("vfx: (party) NO PARTY EFFECTS ({e}): Donald's and Goofy's effects are not drawn"),
                    }
                    // (ER90, XSI request 1) THE XS ABILITIES' ROOT (PACK/xsi/vfx, its own shading table: the renderer keeps its
                    // materials and meshes apart as "xsi:<name>"); `fx.xsi = 0` (read when the game starts): not read, as before
                    if xsi {
                        match host.load_xsi_root(&pack_dir) {
                            Ok(line) => {
                                let n = host.preload_xsi();
                                erlog!("vfx: (XSI) {line}; {n} of them parsed");
                            }
                            Err(e) => erlog!("vfx: (XSI) NO XS EFFECTS ({e}): the XS moves play without their form effects"),
                        }
                    } else {
                        erlog!("vfx: (XSI) fx.xsi = 0: the XS effect root is not read (the XS moves play without their form effects)");
                    }
                    if let Err(e) = host.load_kb_root(&pack_dir).map(|line| erlog!("vfx: (P5) {line}")) { erlog!("vfx: (P5) no keyblade effect root ({e}): the keyblades play the Kingdom Key's trail"); } // (P5) PACK/keyblades/vfx as "kbfx:<name>"
                    let (materials, meshes) = host.assets();
                    let materials: Vec<(String, kh_er_render::vfx::Geo, kh_vfx::Blend)> = materials
                        .into_iter()
                        .map(|(name, geo, blend)| {
                            let geo = match geo {
                                AssetGeo::Quad => kh_er_render::vfx::Geo::Quad,
                                AssetGeo::Ribbon => kh_er_render::vfx::Geo::Ribbon,
                                AssetGeo::Mesh => kh_er_render::vfx::Geo::Mesh,
                                AssetGeo::Decal => kh_er_render::vfx::Geo::Decal,
                            };
                            (name, geo, blend)
                        })
                        .collect();
                    // (the renderer's worker makes them ready in the background: textures, meshes, pipeline states)
                    kh_er_render::vfx::preload(&materials, &meshes);
                    erlog!("vfx: {}; {registered} effects parsed in {:.1} s; {} materials and {} meshes named to the renderer", host.describe(), started.elapsed().as_secs_f32(), materials.len(), meshes.len());
                    host.log.clear();
                    if let Ok(mut guard) = EFFECTS.lock() {
                        *guard = Some(host);
                    }
                }
                Err(e) => erlog!("vfx: NO EFFECTS ({e}): nothing of Sora's effects is drawn"),
            }
        });
        if made.is_err() {
            erlog!("vfx: PANIC while reading the effect pack: no effects");
        }
    });
    if let Err(e) = spawned {
        erlog!("vfx: the loader thread could not be started ({e}): no effects");
    }
}

/// (IMPL_ER22) THE BOSS FINISH'S SOUND SIDE, on the real-time path (never slowed with the world): a static SE by its
/// `ETresStaticSE` id (flat, volume 1, pitch 1: KH3's `SpawnSound2D`) and / or a sound-system event by name
/// (`STATE_MISSION_RESULT_START` = KH3's BOSS_DEAD sound mode). False: the audio side is not there, or the pack has
/// no such sound / event.
pub fn finish_sound(static_se: Option<u32>, event: Option<&str>) -> bool {
    let Ok(mut guard) = AUDIO.lock() else { return false };
    let Some(host) = guard.as_mut() else { return false };
    let mut ok = true;
    if let Some(id) = static_se {
        ok &= host.engine.play_static_se(id).is_some();
    }
    if let Some(name) = event {
        ok &= host.engine.event(name);
    }
    ok
}

/// (MUNNYSE) A WORLD SOUND OF OURS PLAYED NOW through the audio host by its seam route (`name`, e.g. "PrizeMunnyS.get"),
/// 3D at `at` (KH3 cm), on the world's sound categories (it pauses with the world; never the menus' gain), whoever leads
/// and whether or not Sora's own step runs. `handle` names it (a range of the caller's own). False: the audio side is busy
/// this instant or not there, or the name has no route.
pub fn world_sound(handle: u32, name: &str, at: glam::Vec3, volume: f32) -> bool {
    let Ok(mut guard) = AUDIO.try_lock() else { return false };
    let Some(host) = guard.as_mut() else { return false };
    host.engine.seam_play(handle, &kh_audio::SeamPlay { name, position: at, volume: volume.max(0.0), pitch: 1.0, looping: false, spatial: true, anim: None }).is_some()
}

/// (FACE) The party members' voice lines (as of the audio host's last step) for their faces' lip-sync: by member index (0
/// Donald, 1 Goofy); empty while the audio side is busy or not there (never blocks the frame).
#[cfg(feature = "party")]
pub fn party_voices() -> Vec<(u8, Option<kh_anim::face_layer::VoiceInput>)> {
    let Ok(guard) = AUDIO.try_lock() else { return Vec::new() };
    guard.as_ref().map(|host| kh_host::audio::party_voice_inputs(&host.engine)).unwrap_or_default()
}

/// (IMPL_ER12) THE SOUNDS OF OUR MENUS (the HUD's commands of this frame): played now through the audio host, whether
/// or not a simulation step runs (a menu holds the world; the title has none). Dropped while the audio side is not there.
pub fn ui_sounds(commands: Vec<kh_host::seam::AudioCommand>) {
    if commands.is_empty() {
        return;
    }
    static STARTED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    // (IMPL_ER14, snd.trace = 1) one line per command of a menu: what was asked
    let names: Vec<String> = if UI_TRACE.load(Ordering::Relaxed) { commands.iter().map(|c| match c { kh_host::seam::AudioCommand::Play(_, play) => format!("play \"{}\" (id {:?}, volume {:.2})", play.name, play.id, play.volume), kh_host::seam::AudioCommand::Stop(..) => "stop".to_string(), kh_host::seam::AudioCommand::VoiceGroupNo(n) => format!("voice group {n}") }).collect() } else { Vec::new() };
    // (IMPL_ER14) NEVER DROPPED: when the audio side is busy for an instant (or still being read) the commands wait
    // for the next frame's call (at most 64 of them)
    let mut commands = commands;
    // (IMPL_ER14) THE MENUS' LEVEL (sheet row sound.ui_gain_db; MEASURED at the mixer: the camp menu's sounds peak at
    // 0.13 of full scale, RMS -35 dBFS, 10 dB under Sora's combat sounds: nothing to hear under Elden Ring's audio)
    // (applied to the menus' VOICES on the way to the mixer: the engine clamps a seam's own volume at 1)
    let gain = db_to_gain(f32::from_bits(UI_GAIN_BITS.load(Ordering::Relaxed)));
    // (a sound that has waited longer than 0.4 s would come out of place: dropped)
    if let (Ok(mut waiting), Ok(mut since)) = (UI_WAITING.lock(), UI_WAITING_SINCE.lock()) {
        if since.is_some_and(|at| at.elapsed().as_millis() > 400) {
            waiting.clear();
        }
        *since = None;
    }
    if let Ok(mut waiting) = UI_WAITING.lock() {
        if !waiting.is_empty() {
            let mut all = std::mem::take(&mut *waiting);
            all.append(&mut commands);
            commands = all;
        }
    }
    let mut taken = false;
    if let Ok(mut guard) = AUDIO.try_lock() {
        if let Some(audio) = guard.as_mut() {
            taken = true;
            sync_silence(audio); // (GOS) before the menus' sounds: one started under the silence is spared by it
            audio.ui_gain = gain;
            // (THE USER 2026-11-02: "...shotlock's startup sfx is way too loud") ... only up to the sounds' own peak ceiling
            audio.ui_peak_ceiling = f32::from_bits(UI_PEAK_BITS.load(Ordering::Relaxed));
            let asked = commands.len();
            let missing_before = audio.missing;
            let n = audio.ui_commands(std::mem::take(&mut commands));
            if !names.is_empty() {
                erlog!("uisound: {} | {asked} command(s): {} of them started in the engine, {} without a route", names.join(", "), n, audio.missing - missing_before);
            }
            if n > 0 && STARTED.fetch_add(n as u32, Ordering::Relaxed) == 0 {
                erlog!("sound: the menus' own sounds are played through the audio host (first: {n} this frame)");
            }
        }
    }
    if !taken {
        if let Ok(mut waiting) = UI_WAITING.lock() {
            if waiting.len() + commands.len() <= 64 {
                waiting.append(&mut commands);
            }
            if let Ok(mut since) = UI_WAITING_SINCE.lock() {
                since.get_or_insert_with(std::time::Instant::now);
            }
        }
        if !names.is_empty() {
            erlog!("uisound: {} | the audio side is not there this frame: kept for the next", names.join(", "));
        }
    }
}

/// (IMPL_ER14) The menus' gain in dB as f32 bits (the sheet's row, `snd.ui_gain` over it).
static UI_GAIN_BITS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x4140_0000); // 12.0
static UI_GAIN_SHEET: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x4140_0000);
/// (THE USER 2026-11-02) The HUD sounds' peak ceiling as f32 bits (sheet row sound.ui_peak_ceiling, `snd.ui_peak` over it);
/// 0.8 until the sheet is read (`crate::fxmap::UI_PEAK_CEILING`).
static UI_PEAK_BITS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3F4C_CCCD);
static UI_PEAK_SHEET: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3F4C_CCCD);

/// (IMPL_ER14) `snd.trace = 1`: the menus' sound commands and the mixer's per-voice lines are logged.
static UI_TRACE: AtomicBool = AtomicBool::new(false);
static UI_WAITING: Mutex<Vec<kh_host::seam::AudioCommand>> = Mutex::new(Vec::new());
static UI_WAITING_SINCE: Mutex<Option<std::time::Instant>> = Mutex::new(None);

/// (IMPL_ER14) THE AUDIO SIDE STARTS WITH THE PAYLOAD (frame.rs `init`): the pack is read and the device opened before
/// any menu can ask for a sound (MEASURED: the title's choice menu opened 7.7 s before the pack was read).
pub fn prestart(pack_dir: &std::path::Path) {
    start_audio(pack_dir.to_path_buf(), Language::En);
}
static CAPTURE: Mutex<Option<String>> = Mutex::new(None);

/// (IMPL_ER14) THE MIXER'S OWN EVIDENCE, once a frame from FRAME_BEGIN (world or title): `snd.trace = 1` = a line per
/// voice that started in the mixer (frames rendered after it, output peak); `snd.capture = name` = the mixer's output
/// is captured while the key holds that name and written to `<logs>\<name>.wav` when the key changes or goes.
pub fn trace_frame(control: &Control, logs: &std::path::Path) {
    let on = control.on("snd.trace");
    UI_TRACE.store(on, Ordering::Relaxed);
    UI_GAIN_BITS.store(control.f32("snd.ui_gain").filter(|v| (-24.0..=24.0).contains(v)).unwrap_or(f32::from_bits(UI_GAIN_SHEET.load(Ordering::Relaxed))).to_bits(), Ordering::Relaxed);
    UI_PEAK_BITS.store(control.f32("snd.ui_peak").filter(|v| (0.05..=1.0).contains(v)).unwrap_or(f32::from_bits(UI_PEAK_SHEET.load(Ordering::Relaxed))).to_bits(), Ordering::Relaxed); // (THE USER 2026-11-02)
    let Ok(mut guard) = AUDIO.try_lock() else { return };
    let Some(audio) = guard.as_mut() else { return };
    sync_silence(audio); // (GOS) every frame, the title and loading screens too
    for line in audio.out.trace(on) {
        erlog!("mixer: {line}");
    }
    // (a menu sound that was kept because the audio side was busy: played now, or dropped when it is 0.4 s old)
    if let (Ok(mut waiting), Ok(mut since)) = (UI_WAITING.lock(), UI_WAITING_SINCE.lock()) {
        if !waiting.is_empty() {
            let kept = std::mem::take(&mut *waiting);
            if since.is_none_or(|at| at.elapsed().as_millis() <= 400) {
                audio.ui_commands(kept);
            }
        }
        *since = None;
    }
    let wanted = control.str("snd.capture").map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
    let Ok(mut now) = CAPTURE.lock() else { return };
    if *now != wanted {
        if let Some(name) = now.take() {
            let path = logs.join(format!("{name}.wav"));
            match audio.out.capture(false, &path.to_string_lossy()) {
                Some((frames, peak)) => erlog!("mixer: capture \"{name}\" ended: {frames} frames ({:.2} s) of the mixer's output written to {}, peak {peak:.4}", frames as f64 / 48_000.0, path.display()),
                None => erlog!("mixer: capture \"{name}\" ended: nothing could be written"),
            }
        }
        if let Some(name) = wanted.as_ref() {
            audio.out.capture(true, "");
            erlog!("mixer: capture \"{name}\" started ({})", audio.out.describe());
        }
        *now = wanted;
    }
}

/// (IMPL_ER12) TITLE MODE: no Sora and no `Fx` exist: the audio side is started if it was not, and its engine is run
/// on the frame's real time (as under a pause) so that the menu's sounds are heard.
pub fn title_pump(pack_dir: &std::path::Path, dt: f32, volume_db: f32) {
    start_audio(pack_dir.to_path_buf(), Language::En);
    if let Ok(mut guard) = AUDIO.try_lock() {
        if let Some(audio) = guard.as_mut() {
            // (THE USER 2026-11-02: "sometimes Audio from specifically our mod will become very loud") times KH3's Master
            // Volume row as `Fx::frame` sets it: before, the title played at the bare output level, louder than the game
            // after it whenever that row was under 10
            apply_levels(audio, db_to_gain(volume_db.clamp(-60.0, 12.0))); // (TITLE-AUDIO) Master on the output, Music / Sound Effects / Voice per category
            sync_silence(audio); // (GOS)
            audio.idle(dt, true, Listener::default(), &|_| Vec3::ZERO);
            ENGINE_RAN.store(true, Ordering::Relaxed); // (TITLE-AUDIO)
        }
    }
}

/// (TITLE-AUDIO) KH3's own "config" category layer (the pack's `category_layers` 2: its options sliders' layer, no event
/// writes it): where Elden Ring's Music / Sound Effects / Voice rows go.
pub const CONFIG_LAYER: usize = 2;

/// (TITLE-AUDIO, the coordinator: "Music must not depend on the Sound Effects setting") THE LEVEL OF ONE KH3 SOUND CATEGORY on
/// KH3's config layer, by its kind: the music categories (BGM, BGM_LSI) follow Music, the voices (VOICE, Vo_*) follow Voice,
/// every other (SE, SE_system ... the menus' sounds too) follows Sound Effects; each relative to the game's default 7. Master
/// is the output's own gain (`apply_levels`).
pub fn category_level(name: &str, rows: crate::eroptions::SoundRows) -> f32 {
    let row = if name.starts_with("BGM") {
        rows.music
    } else if name == "VOICE" || name.starts_with("Vo_") {
        rows.voice
    } else {
        rows.sfx
    };
    crate::eroptions::sfx_factor(u32::from(row))
}

/// (TITLE-AUDIO) Elden Ring's Sound rows on our output: `base` (the output level, `snd.vol`; 0 = off) x Master on the output,
/// Music / Sound Effects / Voice on each category's config layer (`category_level`). Every frame the engine is run.
fn apply_levels(audio: &mut HostAudio, base: f32) {
    let rows = crate::eroptions::sound_rows();
    audio.out.set_master(base * f32::from(rows.master.min(10)) / 10.0);
    set_category_levels(&mut audio.engine.driver, rows);
}

/// (TITLE-AUDIO) `category_level` of every category of the driver's pack, on KH3's config layer.
pub fn set_category_levels(driver: &mut kh_audio::Driver, rows: crate::eroptions::SoundRows) {
    let levels: Vec<(u32, f32)> = driver.pack().categories.values().map(|c| (c.number, category_level(&c.name, rows))).collect();
    for (category, level) in levels {
        driver.set_category_volume(category, CONFIG_LAYER, level, 0.0);
    }
}

/// (TITLE-AUDIO) The audio engine was run since the title music's last frame (`title_pump`, `Fx::frame`): it is not run twice.
static ENGINE_RAN: AtomicBool = AtomicBool::new(false);
/// (TITLE-AUDIO) The title music's sound in the engine.
static TITLE_BGM: Mutex<Option<kh_audio::SoundKey>> = Mutex::new(None);

/// (TITLE-AUDIO) KH3'S TITLE MUSIC on Elden Ring's title (titlemusic.rs, once a frame while it plays or fades): `bgm` =
/// `Some(gain)`: it plays (started when it is not, its level set to the asset's x `gain` over 0.25 s); `None`: it stops over
/// `fade` s. The engine is run on the frame's real time when nothing else ran it since the last call (the plain title has no
/// HUD frame); its KH3 menu ducking (STATE_MENU_OPEN's BGM layer 3, sent by the title-mode pump) is lifted: KH3's title has
/// none. `Some(live)`: whether the music sounds now; `None`: the audio side is not there or busy this instant.
pub fn title_music(pack_dir: &std::path::Path, dt: f32, volume_db: f32, bgm: Option<f32>, fade: f32) -> Option<bool> {
    const BGM_CATEGORY: u32 = 1;
    const DUCK_LAYER: usize = 3;
    start_audio(pack_dir.to_path_buf(), Language::En);
    let mut guard = AUDIO.try_lock().ok()?;
    let audio = guard.as_mut()?;
    let mut key = TITLE_BGM.lock().ok()?;
    match bgm {
        Some(gain) => {
            let volume = gain.clamp(0.0, 4.0) * audio.engine.pack().asset(crate::titlemusic::title_asset()).and_then(|a| a.volume).unwrap_or(1.0);
            match *key {
                Some(k) if audio.engine.driver.is_playing(k) => audio.engine.driver.set_volume(k, volume, 0.25),
                _ => {
                    *key = audio.engine.play_asset(crate::titlemusic::title_asset(), "title music (TITLE-AUDIO)", kh_audio::AssetPlay { volume: gain.clamp(0.0, 4.0), ..kh_audio::AssetPlay::at(None, None) });
                    if key.is_none() {
                        static TOLD: AtomicBool = AtomicBool::new(false);
                        if !TOLD.swap(true, Ordering::Relaxed) {
                            erlog!("sound: (TITLE-AUDIO) KH3's title music is NOT in the pack (PACK/title/audio, tools/title_audio_pack.js): the title stays silent");
                        }
                    }
                }
            }
            audio.engine.driver.set_category_volume(BGM_CATEGORY, DUCK_LAYER, 1.0, 0.0);
        }
        None => {
            if let Some(k) = key.take() {
                audio.engine.driver.stop(k, Some(fade.max(0.0)));
            }
        }
    }
    if !ENGINE_RAN.swap(false, Ordering::Relaxed) {
        apply_levels(audio, db_to_gain(volume_db.clamp(-60.0, 12.0)));
        sync_silence(audio); // (GOS)
        audio.idle(dt, true, Listener::default(), &|_| Vec3::ZERO);
    }
    Some(audio.engine.driver.active().iter().any(|a| audio.engine.pack().asset(crate::titlemusic::title_asset()).is_some_and(|t| t.bank == a.bank && t.index == a.index)))
}

/// (MIMIC-MUSIC) The audio engine was run by Sora's frame since the boss music's last frame (`Fx::frame`).
static BOSS_RAN: AtomicBool = AtomicBool::new(false);
/// (MIMIC-MUSIC) The boss music's sound in the engine.
static BOSS_BGM: Mutex<Option<kh_audio::SoundKey>> = Mutex::new(None);

/// (MIMIC-MUSIC) A KH3 MUSIC ASSET IN PLACE OF A BOSS FIGHT'S MUSIC (mimic_music.rs, once a frame while it plays or fades):
/// `bgm` = `Some(gain)`: `asset` plays, 2D, looped by its own loop points (started when it is not, its level set to the
/// asset's x `gain` over 0.25 s); `None`: it stops over `fade` s. Its level is the BGM category's: Master on the output x
/// Music on KH3's config layer (`apply_levels`), as the title music's. The engine is run on the frame's real time only when
/// Sora's frame did not run it since the last call (a loading screen). `Some(live)`: whether it sounds now; `None`: the
/// audio side is not there or busy this instant.
/// (MIMIC-BOSS hotfix 3, THE USER: "The music played correctly but it needs to fade in") `fade`: with `bgm` Some, the fade-in of
/// a new start (s); with None, the fade-out of the stop.
pub fn boss_music(asset: &str, dt: f32, volume_db: f32, bgm: Option<f32>, fade: f32) -> Option<bool> {
    let mut guard = AUDIO.try_lock().ok()?;
    let audio = guard.as_mut()?;
    let mut key = BOSS_BGM.lock().ok()?;
    match bgm {
        Some(gain) => {
            let volume = gain.clamp(0.0, 4.0) * audio.engine.pack().asset(asset).and_then(|a| a.volume).unwrap_or(1.0);
            match *key {
                Some(k) if audio.engine.driver.is_playing(k) => audio.engine.driver.set_volume(k, volume, 0.25),
                _ => *key = audio.engine.play_asset(asset, "boss music (MIMIC-MUSIC)", kh_audio::AssetPlay { volume: gain.clamp(0.0, 4.0), fade_in: fade.max(0.0) /* (MIMIC-BOSS hotfix 3) */, ..kh_audio::AssetPlay::at(None, None) }),
            }
        }
        None => {
            if let Some(k) = key.take() {
                audio.engine.driver.stop(k, Some(fade.max(0.0)));
            }
        }
    }
    if !BOSS_RAN.swap(false, Ordering::Relaxed) {
        apply_levels(audio, db_to_gain(volume_db.clamp(-60.0, 12.0)));
        sync_silence(audio); // (GOS)
        audio.idle(dt, true, Listener::default(), &|_| Vec3::ZERO);
    }
    // (MIMIC-BOSS hotfix 2) the playing music's category never left muted by a sound state (logged once each time)
    if bgm.is_some() {
        let cat = audio.engine.pack().asset(asset).and_then(|t| audio.engine.pack().sound(t.bank, t.index)).map(|d| d.category);
        if let Some(cat) = cat {
            if music_guard(&mut audio.engine.driver, cat) {
                erlog!("sound: (MIMIC-BOSS) the Mimic Tear fight's music category {cat} was paused or ducked to nothing by a sound state: given back (KH3's own end events)");
            }
        }
    }
    let wanted = audio.engine.pack().asset(asset).map(|t| (t.bank, t.index));
    Some(audio.engine.driver.active().iter().any(|a| wanted == Some((a.bank, a.index))))
}

/// (MIMIC-BOSS hotfix 2, the user: "I wasn't hearing the music we chose playing") THE MUSIC'S REAL LEVEL NOW, for the log: its
/// sound's running voices, the level the mixer was sent, its category's level (every layer and its pause), the host's silence
/// and the output's master; their product is what reaches the speakers. `None`: not playing or the audio side is busy.
pub fn boss_music_probe(asset: &str) -> Option<String> {
    let guard = AUDIO.try_lock().ok()?;
    let audio = guard.as_ref()?;
    let wanted = audio.engine.pack().asset(asset).map(|t| (t.bank, t.index))?;
    let a = audio.engine.driver.active().into_iter().find(|a| (a.bank, a.index) == wanted)?;
    let (category, _) = audio.engine.driver.category_state(a.category);
    let (silence, master) = (audio.engine.driver.silence(), audio.out.master());
    let level = a.params.volume * silence * master;
    Some(format!("{} running voice(s), the voice sent at {:.3} (its category {} at {:.3}, layers and pause), the host's silence {:.3}, the output's master {:.3}: {:.3} reaches the speakers{}", a.voices, a.params.volume, a.category, category, silence, master, level, if level <= 0.001 { " (SILENT: see the factors)" } else { "" }))
}

/// (MIMIC-BOSS hotfix 2) THE MUSIC'S GUARD (the playing music only): its category paused or ducked to nothing by an event of
/// KH3's sound states left unanswered in this port (an all-pause start with no end, a menu duck that stayed) is given back:
/// KH3's own end events (`STATE_ALLPAUSE_END`, `STATE_MENU_CLOSE`'s BGM layer) while the world runs. True: it had to.
pub fn music_guard(driver: &mut kh_audio::Driver, category: u32) -> bool {
    let (level, _) = driver.category_state(category);
    if level > 0.01 {
        return false;
    }
    driver.event("STATE_ALLPAUSE_END");
    for layer in [3, 5] {
        driver.set_category_volume(category, layer, 1.0, 0.2);
    }
    true
}

/// (MIMIC-MUSIC) `asset`'s material read ahead on the feeder thread (a large music: never read at its first play).
/// `Some(true)`: asked; `Some(false)`: the asset is not in the pack; `None`: the audio side is not there or busy.
pub fn preload_asset(asset: &str) -> Option<bool> {
    let mut guard = AUDIO.try_lock().ok()?;
    let audio = guard.as_mut()?;
    let material = audio.engine.pack().asset(asset).map(|a| (a.bank, 0u32));
    if let Some(material) = material {
        audio.out.preload(vec![material]);
    }
    Some(material.is_some())
}

/// (MIMIC-MUSIC) Whether the pack holds `asset` (None: the audio side is not up yet or busy this instant).
pub fn has_asset(asset: &str) -> Option<bool> {
    let guard = AUDIO.try_lock().ok()?;
    Some(guard.as_ref()?.engine.pack().asset(asset).is_some())
}

/// (GOS, THE USER 2026-11-03: "silence all music and other SFX when on death screen" / "(except for menu SFX, dont
/// silence that)") The game over's silence (gosound.rs) put on the audio host whenever it differs from what is wanted:
/// every sound of ours but the menus' started under it fades out, and comes back as it was.
/// (CC-MUTE) ... and Elden Ring's character creation's (ngquiet.rs: from "Select Character Base" until our first KH new-game
/// pop-up; the pop-up's own sounds are menu sounds, exempt, so its first sound is never cut).
fn sync_silence(audio: &mut HostAudio) {
    let (on, fade) = crate::ngquiet::kh_wanted(crate::gosound::kh_wanted());
    if audio.set_silence(on, fade) {
        erlog!("sound: (GOS / CC-MUTE) {} over {fade:.2} s", if on { "THE SILENCE (the game over, or Elden Ring's character creation): every KH sound fades out but the game-over loop and the menus' sounds" } else { "the KH sounds come back (the silence ended)" });
    }
}

/// Starts the audio side in the background (once per payload). Returns at once.
fn start_audio(pack_dir: std::path::PathBuf, language: Language) {
    if AUDIO_ASKED.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new().name("kh-audio-load".to_string()).spawn(move || {
        let made = std::panic::catch_unwind(move || {
            let started = std::time::Instant::now();
            let dir = pack_dir.join("audio");
            match kh_audio::Pack::load(&dir) {
                Ok(mut pack) => {
                    // (ER77) the outfits' own voices (PACK/audio/outfits/*/voice.json: the KH1 outfit's clips)
                    for line in pack.load_outfit_voices() {
                        erlog!("sound: voice of an outfit: {line}");
                    }
                    // (ER party S5) the party's own audio root (D5: build/sora_pack/party/audio, the cure-target SE of
                    // vm_cur_bds1); the party's other sounds and voices are in the main pack (stream E)
                    #[cfg(feature = "party")]
                    match pack.merge_party_root(&party::audio_root(&pack_dir)) {
                        Ok(true) => erlog!("sound: (party) the party audio root is merged (PACK/party/audio)"),
                        Ok(false) => erlog!("sound: (party) no party audio root (PACK/party/audio/manifest.json): the cure-target SE is silent"),
                        Err(e) => erlog!("sound: (party) the party audio root is not merged ({e})"),
                    }
                    // (XSI) the XS clips' own sounds (PACK/xsi/audio: Crimson Blast's and Rage Raid's se00001_201 .. 204)
                    match pack.merge_party_root(&pack_dir.join("xsi").join("audio")) {
                        Ok(true) => erlog!("sound: (XSI) the XS audio root is merged (PACK/xsi/audio)"),
                        Ok(false) => {}
                        Err(e) => erlog!("sound: (XSI) the XS audio root is not merged ({e})"),
                    }
                    // (P8) the Synthesis Moogle's voices (P8 fix: PACK/party/n_ex028/audio, the floating Moogle: se00000_moogle1..5 into the main pack's se00000 bank)
                    match pack.merge_party_root(&pack_dir.join("party").join("n_ex028").join("audio")) {
                        Ok(true) => erlog!("sound: (P8) the Moogle's audio root is merged (PACK/party/n_ex028/audio)"),
                        Ok(false) => erlog!("sound: (P8) no Moogle audio root: he is silent"),
                        Err(e) => erlog!("sound: (P8) the Moogle's audio root is not merged ({e})"),
                    }
                    // (AUDIT-A) the keyblade forms' effect-carried sounds and shot hit SEs the main pack lacks (PACK/keyblades/audio)
                    match pack.merge_party_root(&pack_dir.join("keyblades").join("audio")) {
                        Ok(true) => erlog!("sound: (AUDIT-A) the keyblade forms' audio root is merged (PACK/keyblades/audio)"),
                        Ok(false) => erlog!("sound: (AUDIT-A) no keyblade forms' audio root (PACK/keyblades/audio): those form sounds are silent"),
                        Err(e) => erlog!("sound: (AUDIT-A) the keyblade forms' audio root is not merged ({e})"),
                    }
                    // (FINAL-FIX) the shotlocks' projectile classes (their blueprints' hit / map-hit / fire SE) and the SEs the main
                    // pack lacks (PACK/shotlocks/audio: tools/sora_shotlock_audio.js)
                    match pack.merge_party_root(&pack_dir.join("shotlocks").join("audio")) {
                        Ok(true) => erlog!("sound: (FINAL-FIX) the shotlocks' audio root is merged (PACK/shotlocks/audio)"),
                        Ok(false) => erlog!("sound: (FINAL-FIX) no shotlocks' audio root (PACK/shotlocks/audio): their projectiles' hits have no route"),
                        Err(e) => erlog!("sound: (FINAL-FIX) the shotlocks' audio root is not merged ({e})"),
                    }
                    // (TITLE-AUDIO) KH3's title music for Elden Ring's title (PACK/title/audio: tools/title_audio_pack.js)
                    match pack.merge_party_root(&pack_dir.join("title").join("audio")) {
                        Ok(true) => erlog!("sound: (TITLE-AUDIO) the title music's audio root is merged (PACK/title/audio)"),
                        Ok(false) => erlog!("sound: (TITLE-AUDIO) no title music audio root (PACK/title/audio): Elden Ring's title stays silent while KH3's would play"),
                        Err(e) => erlog!("sound: (TITLE-AUDIO) the title music's audio root is not merged ({e})"),
                    }
                    // (MIMIC-MUSIC) KH3's replica Xehanorts battle theme for the two Mimic Tear fights (PACK/mimic/audio: tools/mimic_music_pack.js)
                    match pack.merge_party_root(&pack_dir.join("mimic").join("audio")) {
                        Ok(true) => erlog!("sound: (MIMIC-MUSIC) the Mimic Tear fights' music root is merged (PACK/mimic/audio)"),
                        Ok(false) => erlog!("sound: (MIMIC-MUSIC) no Mimic Tear music root (PACK/mimic/audio): Elden Ring's own boss music plays in those fights"),
                        Err(e) => erlog!("sound: (MIMIC-MUSIC) the Mimic Tear fights' music root is not merged ({e})"),
                    }
                    // (TITLE-AUDIO) the user's own title track (PACK/title/audio/custom/title.wav) plays instead of KH3's when it is there
                    let title_dir = pack_dir.join("title").join("audio");
                    let custom = match crate::titlemusic::custom_root(&pack_dir) {
                        Ok(Some((manifest, words))) => match pack.merge_root_text(&title_dir, &manifest) {
                            Ok(_) if pack.asset(crate::titlemusic::CUSTOM_ASSET).is_some() => {
                                erlog!("sound: (TITLE-AUDIO) THE USER'S OWN TITLE TRACK plays instead of KH3's: {words}");
                                true
                            }
                            Ok(_) => false,
                            Err(e) => {
                                erlog!("sound: (TITLE-AUDIO) the user's own title track is not merged ({e}): KH3's title music plays");
                                false
                            }
                        },
                        Ok(None) => false,
                        Err(e) => {
                            erlog!("sound: (TITLE-AUDIO) the user's own title track cannot be used ({e}): KH3's title music plays");
                            false
                        }
                    };
                    let asset_now = if custom { crate::titlemusic::CUSTOM_ASSET } else { crate::titlemusic::TITLE_BGM_ASSET };
                    let track_loop = pack.asset(asset_now).and_then(|a| pack.bank(a.bank).materials.get(&0)).map(|m| {
                        let rate = f64::from(m.sample_rate.max(1));
                        let (start, end) = m.loop_points.unwrap_or((0, m.samples));
                        (f64::from(end) / rate, f64::from(end.saturating_sub(start)) / rate)
                    });
                    let (first_end, period) = track_loop.unwrap_or((crate::titlemusic::TITLE_BGM_SECONDS, crate::titlemusic::TITLE_BGM_SECONDS));
                    crate::titlemusic::set_title_track(custom, first_end, period);
                    let title_bgm = pack.asset(crate::titlemusic::title_asset()).map(|a| (a.bank, 0u32));
                    let pack = Arc::new(pack);
                    erlog!("sound: audio pack {} read in {:.2} s: {} banks, {} sound assets, {} triggers, voices {}", dir.display(), started.elapsed().as_secs_f32(), pack.banks.len(), pack.assets.len(), pack.triggers.len(), language.folder());
                    let out = kh_host::audio::Output::device(pack.clone(), true, log);
                    let mut host = HostAudio::new(pack, out, 1, language);
                    // (DON-KIT) the item effects' sounds read ahead with the effects (the first item use was late / silent)
                    let n = host.preload_effect_sounds(&kh_host::vfx::item_effect_names());
                    erlog!("sound: (DON-KIT) the item effects' sounds read ahead ({n} materials)");
                    // (TITLE-AUDIO) the title music's material read ahead on the feeder thread (29 MB: never at its first play)
                    if let Some(material) = title_bgm {
                        host.out.preload(vec![material]);
                    }
                    if let Ok(mut guard) = AUDIO.lock() {
                        *guard = Some(host);
                    }
                }
                Err(e) => erlog!("sound: NO AUDIO PACK at {} ({e}): Sora is silent", dir.display()),
            }
        });
        if made.is_err() {
            erlog!("sound: PANIC while starting the audio side: Sora is silent");
        }
    });
    if let Err(e) = spawned {
        erlog!("sound: the loader thread could not be started ({e}): Sora is silent");
    }
}

/// The payload goes away: the device is closed and our audio threads end (waited for, at most
/// 3 s). Called from entry.rs.
pub fn shutdown() {
    kh_er_render::vfx::clear();
    let taken = AUDIO.lock().ok().and_then(|mut guard| guard.take());
    if taken.is_some() {
        drop(taken);
        erlog!("sound: stopped (device closed)");
    }
}

pub struct Fx {
    /// The core's seams for a step (sora.rs hands these in its `Host`).
    pub audio: AudioQueue,
    pub effects: EffectQueue,
    params: FxParams,
    sound_on: bool,
    alarm: bool,
    /// The game's ground material under the player and the KH3 surface it sounds as.
    material: i32,
    surface: u32,
    materials_seen: Vec<i32>,
    listener: Listener,
    since_log: f32,
    unknown_told: usize,
    told_start: bool,
    steps: u32,
    // ---- effects
    vfx_on: bool,
    vfx_told: Option<bool>,
    /// The bones of the latest step (the effects' sockets).
    tap: PoseTap,
    /// The height of the ground under him when he last stood on it (the effects' ground rays), cm.
    ground: Option<f32>,
    /// (IMPL_ER33) Real time not yet stepped by `cutscene_step`.
    cut_pending: f32,
    /// The effect clock, s.
    seconds: f32,
    /// Requests of the effects this host does not carry out: said once a second.
    shakes: Vec<kh_host::vfx::Shake>,
    /// The scene light the effects ask for now (1 = untouched): handed to the renderer every frame.
    scene_light: f32,
    darkest: f32,
    /// (IMPL_ER75) the effects' clock for the next step: the WORLD's step (sora.rs sets it to `wdt`: slowed in KH3's finish
    /// sequences, as his own step), so that a trail's history and fade advance as his pose does; and the position of the
    /// step before (a jump of more than 3 m in a step cuts the trails).
    pub effect_dt: f32,
    last_position: Option<Vec3>,
    /// (ER77) `boc.kh1_voice` (default on): the KH1 outfit speaks with the user's KH1 clips.
    kh1_voice: Option<bool>,
    /// (ER party S5) the party's step feed, members, loops (fx_party.rs)
    #[cfg(feature = "party")]
    pub party: party::PartyFx,
    /// (INT) the effects were last stepped by `cutscene_step`: drawn at ITS place between its steps (else sora.rs's alpha)
    int_cut_alpha: Option<f32>,
    /// (FX5) Sora is out of the party and his effects were ended (`sora_absent`).
    #[cfg(feature = "party")]
    sora_gone: bool,
}

impl Fx {
    /// When Sora starts (payload start, respawn). `sheet`: sheets/port/sora-er-fx.json.
    pub fn start(pack_dir: &std::path::Path, sheet: &std::path::Path, control: &Control) -> Fx {
        let params = match std::fs::read_to_string(sheet).map_err(|e| e.to_string()).and_then(|text| FxParams::from_sheet(&text)) {
            Ok((params, problems)) => {
                erlog!("fx: numbers {} ({} rows not understood): {} surface rows, default surface {}, volume {} dB", sheet.display(), problems.len(), params.surfaces.len(), params.surface_default, { UI_GAIN_SHEET.store(params.ui_gain_db.to_bits(), Ordering::Relaxed); UI_PEAK_SHEET.store(params.ui_peak_ceiling.to_bits(), Ordering::Relaxed); params.volume_db });
                params
            }
            Err(e) => {
                erlog!("fx: numbers {} not read ({e}): the built-in defaults are used (every ground sounds as surface 0)", sheet.display());
                FxParams::default()
            }
        };
        let language = control.str("snd.lang").and_then(Language::parse).unwrap_or(Language::En);
        #[cfg(feature = "party")]
        kh_party::donald::zettaflare::set_voice_ja(matches!(language, Language::Ja)); // (ZETTA) Donald's lip track follows the voice
        start_audio(pack_dir.to_path_buf(), language);
        // (XW-c) `fx.main_fix = 0` (read when the game starts): the main table without its overlay (the Blizzard ice mask as before)
        kh_er_render::vfx::MAIN_OVERLAY.store(control.flag("fx.main_fix") != Some(false), std::sync::atomic::Ordering::Relaxed);
        // (party VF-b) `fx.trig_fix = 0` / `fx.shards_own = 0` (read when the game starts): the tables without the VF-b overlays
        kh_er_render::vfx::TRIG_OVERLAY.store(control.flag("fx.trig_fix") != Some(false), std::sync::atomic::Ordering::Relaxed);
        kh_er_render::xsiweapon::SHARDS_OWN.store(control.flag("fx.shards_own") != Some(false), std::sync::atomic::Ordering::Relaxed);
        // (XW-d) `fx.xs_own = 0` / `fx.xs_fix = 0` (read when the game starts): the XS effects from effects/ only (not their own files of
        // effects_xwd/: the main pack's same-named materials, no fragment pieces) / the tables without the XW-d overlays (tmt_cry_00a)
        kh_host::vfx::XS_OWN_FILES.store(control.flag("fx.xs_own") != Some(false), std::sync::atomic::Ordering::Relaxed);
        kh_er_render::vfx::XWD_OVERLAY.store(control.flag("fx.xs_fix") != Some(false), std::sync::atomic::Ordering::Relaxed);
        // (FP) `fx.cull_one_sided = 0` (read when the game starts): one-sided mesh particles draw their back faces too (before FP)
        kh_er_render::vfx::CULL_ONE_SIDED.store(control.flag("fx.cull_one_sided") != Some(false), std::sync::atomic::Ordering::Relaxed);
        start_effects(pack_dir.to_path_buf(), control.flag("fx.xsi") != Some(false));
        // (a respawn: the effects of the Sora before have no owner)
        if let Ok(mut guard) = EFFECTS.try_lock() {
            if let Some(effects) = guard.as_mut() {
                effects.clear();
            }
        }
        kh_er_render::vfx::clear();
        // A respawn: what was playing stops, the engine starts over.
        if let Ok(mut guard) = AUDIO.try_lock() {
            if let Some(audio) = guard.as_mut() {
                audio.restart(1);
            }
        }
        Fx { cut_pending: 0.0, audio: AudioQueue::default(), effects: EffectQueue::default(), surface: params.surface_default, params, sound_on: true, alarm: true, material: -1, materials_seen: Vec::new(), listener: Listener::default(), since_log: 0.0, unknown_told: 0, told_start: false, steps: 0, vfx_on: true, vfx_told: None, tap: PoseTap::default(), ground: None, seconds: 0.0, shakes: Vec::new(), scene_light: 1.0, darkest: 1.0, effect_dt: 1.0 / 60.0, last_position: None, kh1_voice: None, #[cfg(feature = "party")] party: party::PartyFx::default(), int_cut_alpha: None, #[cfg(feature = "party")] sora_gone: false }
    }

    /// Once per game frame, before the simulation steps of the frame.
    pub fn frame(&mut self, player: &PlayerIns, control: &Control, dt: f32, paused: bool, frame: &Frame) {
        BOSS_RAN.store(true, Ordering::Relaxed); // (MIMIC-MUSIC) Sora's frame runs the engine: the boss music's frame does not
        ENGINE_RAN.store(true, Ordering::Relaxed); // (TITLE-AUDIO) Sora's frame runs the engine: the title music's fade does not
        self.sound_on = control.flag("snd") != Some(false);
        self.alarm = control.flag("snd.alarm") != Some(false);
        self.kh1_voice = control.flag("boc.kh1_voice");
        // ---- the surface under him: the game's own ground reading under its (hidden) player
        let read = player.chr_ins.modules.physics.material_info.hit_material;
        let forced = player.chr_ins.hit_material_override;
        let material = if forced >= 0 { forced } else { read };
        let mapped = self.params.surface(material);
        self.surface = control.i64("snd.surface").filter(|v| *v >= 0).map_or(mapped.unwrap_or(self.params.surface_default), |v| v as u32);
        if material != self.material {
            // `snd.surflog = 1` (surveying aid): EVERY change of the ground material, with the place.
            if control.on("snd.surflog") {
                let at = crate::game::v3(&player.chr_ins.modules.physics.position);
                erlog!("surface: now material {material} (hit_material {read}, override {forced}) -> KH3 surface {} at Elden Ring ({:.1}, {:.1}, {:.1})", mapped.map_or("none".to_string(), |s| s.to_string()), at.x, at.y, at.z);
            }
            self.material = material;
            if !self.materials_seen.contains(&material) && self.materials_seen.len() < 256 {
                self.materials_seen.push(material);
                erlog!(
                    "surface: Elden Ring ground material {material} (hit_material {read}, override {forced}) -> KH3 surface {}{}",
                    self.surface,
                    if mapped.is_none() { " (NO ROW in sheets/port/sora-er-fx.json: the default; add surface.er.<material>)" } else { "" }
                );
            }
        }
        // ---- the listener: (ER62) the camera on screen, in KH3's space (audio_listener.rs; `audio.listener_fix = 0`: pers_cam_1 as before)
        if let Some(listener) = crate::audio_listener::pick(frame, control.flag("audio.listener_fix") != Some(false)) {
            self.listener = listener;
        }
        // ---- the effects' switches
        self.vfx_on = control.flag("vfx") != Some(false);
        // (AUDIT-B W-HANDOVER) KH3's SQEX_Raycast casts (the hammers' / Nano Arms' ground cracks and water pillars) through Elden
        // Ring's Havok world on this frame's KH3 <-> ER frame (fxrays.rs); `vfx.rays = 0`: the flat ground under him as before
        if let Ok(mut guard) = EFFECTS.try_lock() {
            if let Some(effects) = guard.as_mut() {
                let rays: Option<std::sync::Arc<dyn kh_vfx::RayWorld>> = (control.flag("vfx.rays") != Some(false)).then(|| std::sync::Arc::new(crate::fxrays::ErRays { frame: *frame }) as std::sync::Arc<dyn kh_vfx::RayWorld>);
                effects.world.set_rays(rays);
            }
        }
        kh_er_render::vfx::set_options(kh_er_render::vfx::VfxOptions {
            on: self.vfx_on,
            distort: control.flag("vfx.distort") != Some(false),
            hdr: control.f32("vfx.hdr").filter(|v| *v > 0.0).unwrap_or(self.params.vfx_hdr_scale),
            solid: control.on("vfx.solid"),
            decals: control.flag("vfx.decals") != Some(false),
            // `vfx.late = 0`: everything right after Sora again (before the game's forward stage)
            late: control.flag("vfx.late") != Some(false),
        });
        // (VFXR) KH3's bloom on the effects (`vfx.bloom = x`: a factor, 0 = off; default 1 = KH3's UE4 settings) and the
        // screen-wide passes the effects ask for (`vfx.screen_fx = 0`: off): kh-er-render vfxpost.rs
        // (VFXR) `vfx.eye_adapt = 0`: the eye-adaptation materials by the lit-surface rule as before (3.27 x darker than KH3 shows them)
        kh_er_render::vfx::set_eye_adapt(control.flag("vfx.eye_adapt") == Some(true)); // (VFXR eye evidence) default off
        // (VFXR) `vfx.char_fx = 0`: the effects' character-material drives (in-light, aura ...) off
        kh_er_render::vfx::CHAR_FX.store(control.flag("vfx.char_fx") != Some(false), std::sync::atomic::Ordering::Relaxed);
        kh_er_render::vfx::set_post(control.f32("vfx.bloom").filter(|v| *v >= 0.0).unwrap_or(1.0), control.flag("vfx.screen_fx") != Some(false));
        // (VFXR occlusion) `vfx.decal_chars = 0`: our decals paint the characters too (as before); default: they skip them
        kh_er_render::vfx::set_decal_skip_chars(control.flag("vfx.decal_chars") != Some(false));
        // (VFXR cinematic) a sequence's look (kh_core::host::cinematic), advanced here once a frame in real time (paused: held);
        // `vfx.cinematic = 0` ignores it. It lives only on our characters and effects: nothing of the game's to restore.
        kh_er_render::vfx::CINEMATIC.store(control.flag("vfx.cinematic") != Some(false), std::sync::atomic::Ordering::Relaxed);
        let look = kh_core::host::cinematic::step(if paused { 0.0 } else { dt });
        kh_er_render::vfx::set_cine_look(look.exposure_ev, look.key_ev, look.bloom_threshold);
        // (VFXR decal eye) `vfx.eye_decals = 1`: the decals take the eye-adaptation boost too (default off)
        kh_er_render::vfx::EYE_DECALS.store(control.flag("vfx.eye_decals") == Some(true), std::sync::atomic::Ordering::Relaxed);
        // (VFXR decal bloom) `vfx.decal_bloom = 1`: the decals feed our bloom too (default off)
        // (VFXR sort) `vfx.sort = 0`: the effect batches by material across emitters, far to near by their centre (as before)
        kh_er_render::vfx::SORT_GROUPS.store(control.flag("vfx.sort") != Some(false), std::sync::atomic::Ordering::Relaxed);
        // (PARTY-NATIVE) `vfx.fxlit = 0`: the lit translucent effects in their fog variant (a test A / B; default on)
        kh_er_render::vfx::LIT_VARIANTS.store(control.flag("vfx.fxlit") != Some(false), std::sync::atomic::Ordering::Relaxed);
        kh_er_render::vfx::DECAL_BLOOM.store(control.flag("vfx.decal_bloom") == Some(true), std::sync::atomic::Ordering::Relaxed);
        // (IMPL_ER20) `vfx.depth = 0`: SQEX_DepthOffset read as before (negative = nearer); `vfx.lens = 0`: the HP orb's
        // ball as the literal data (a 10 x 36 cm sliver): kh-vfx's two placement switches, for comparing
        kh_vfx::set_depth_offset_toward_camera(control.flag("vfx.depth") != Some(false));
        kh_vfx::set_lens_rule(control.flag("vfx.lens") != Some(false));
        // THE SCENE DARKENING of Thunder and the grand spells (`vfx.dim = 0`: off; `vfx.dim_test = x`: a fixed scene light for looking at it)
        kh_er_render::vfx::set_scene_light(if control.flag("vfx.dim") == Some(false) { 1.0 } else { control.f32("vfx.dim_test").unwrap_or(self.scene_light) });
        // (IMPL_ER39) THE LIGHT PASS (kh_er_render::pointlight): the effects' particle lights light the world (`fx.light = 0`:
        // off) and a slight hero light comes up around Sora in the dark (`r.hero_light = 0` off, 2 = always on for looking
        // at it); numbers: sheet rows fx.light*, hero.* (`fx.light_gain`, `hero.intensity` override while the game runs).
        {
            let mut light = self.params.light;
            if let Some(on) = control.flag("fx.light") {
                light.fx = on;
            }
            if let Some(mode) = control.i64("r.hero_light") {
                light.hero = mode.clamp(0, 2) as u8;
            }
            if let Some(v) = control.f32("fx.light_gain").filter(|v| *v >= 0.0) {
                light.fx_gain = v;
            }
            if let Some(v) = control.f32("hero.intensity").filter(|v| *v >= 0.0) {
                light.hero_intensity = v;
            }
            // (IMPL_ER44) the hero light / the effects' lights on Sora's own materials (0 = none)
            if let Some(v) = control.f32("hero.sora_light").filter(|v| *v >= 0.0) {
                light.hero_sora = v;
            }
            if let Some(v) = control.f32("fx.sora_light").filter(|v| *v >= 0.0) {
                light.fx_sora = v;
            }
            // (IMPL_ER44 part 2) `hero.keyblade = 0`: the plain light above him, no Keyblade drawn for it
            if let Some(on) = control.flag("hero.keyblade") {
                light.hero_keyblade = on;
            }
            // (IMPL_ER64) `r.world_lights = 0`: no light of the world's on Sora; `hero.sora_placed = 0`: the Keyblade light
            // on him from the blade (as before)
            if let Some(on) = control.flag("r.world_lights") {
                light.world = on;
            }
            if let Some(on) = control.flag("hero.sora_placed") {
                light.hero_sora_placed = on;
            }
            kh_er_render::pointlight::set_settings(light);
        }
        if self.vfx_told != Some(self.vfx_on) {
            self.vfx_told = Some(self.vfx_on);
            erlog!("vfx: Sora's effects are {} (vfx = 0 switches them off; vfx.distort = 0 the refraction, vfx.decals = 0 the decals, vfx.hdr = x their brightness)", if self.vfx_on { "ACTIVE" } else { "OFF" });
            if !self.vfx_on {
                if let Ok(mut guard) = EFFECTS.try_lock() {
                    if let Some(effects) = guard.as_mut() {
                        effects.clear();
                    }
                }
                kh_er_render::vfx::clear();
            }
        }
        self.since_log += dt;
        let tick = self.since_log >= 1.0;
        if tick {
            self.vfx_tick();
        }
        // (never waited for: the only other taker is the loader thread, for an instant, once)
        let Ok(mut guard) = AUDIO.try_lock() else { return };
        let Some(audio) = guard.as_mut() else {
            if tick {
                self.since_log = 0.0;
                erlog!("sound: not ready (the audio pack is being read, or there is none)");
            }
            return;
        };
        let volume_db = control.f32("snd.vol").unwrap_or(self.params.volume_db).clamp(-60.0, 12.0);
        let was = audio.out.master();
        // (IMPL_ER21) ... times KH3's Master Volume row (the Config screen; the game's own master volume follows the same row)
        // (TITLE-AUDIO) the output = Master; Music / Sound Effects / Voice per category (`apply_levels`)
        apply_levels(audio, if self.sound_on { db_to_gain(volume_db) } else { 0.0 });
        if !self.sound_on && was > 0.0 {
            audio.out.stop_all();
            audio.restart(1);
        }
        if !self.told_start || (was > 0.0) != self.sound_on {
            self.told_start = true;
            erlog!("sound: Sora's sound is {} (snd = 0 switches it off; snd.vol = dB, now {volume_db}); {}", if self.sound_on { "ACTIVE" } else { "OFF" }, audio.out.describe());
        }
        // The world stands still (a popup): the SE categories pause, fades go on.
        if self.sound_on {
            let position = self.listener.position;
            audio.idle(dt, paused, self.listener, &|_| position);
        }
        if tick {
            self.since_log = 0.0;
            for name in audio.unknown.iter().skip(self.unknown_told) {
                erlog!("sound: no route for \"{name}\" (nothing is heard for it)");
            }
            self.unknown_told = audio.unknown.len();
            // (ER77) the outfit voice lines of the last second (which KH1 clip stood for which KH3 line, or silence)
            for note in audio.voice_notes.drain(..) {
                erlog!("sound: voice {note}");
            }
            erlog!(
                "sound: {} | {} dB | {} step(s) | seam sounds {} (nothing played for {}), motion sounds {} (last: {}) | ground material {} -> surface {} | {}",
                if self.sound_on { "on" } else { "OFF" },
                volume_db,
                self.steps,
                audio.played,
                audio.missing,
                audio.auto_events,
                audio.last_auto.as_deref().unwrap_or("none"),
                self.material,
                self.surface,
                audio.out.describe()
            );
            // (THE USER 2026-11-02: "sometimes Audio from specifically our mod will become very loud") THE MIX OF THE LAST
            // SECOND: its peak after the output level and before the limiter, the limiter's deepest cut, the largest gain a
            // HUD sound got; when the limiter had to act, the loudest live sounds by name (what was loud)
            {
                let window = audio.out.take_window();
                let (raised, hud_live) = audio.take_ui_raised();
                erlog!("sound: mix this second: peak {:.3} before the limiter (limit {:.3}), limiter cut {:.1} dB, master {:.3}, HUD sounds raised up to x{raised:.2} ({hud_live} live)", window.peak, kh_audio::mixer::LIMIT, window.cut_db(), audio.out.master());
                if window.peak > kh_audio::mixer::LIMIT {
                    let mut live = audio.engine.active();
                    live.sort_by(|a, b| b.params.volume.total_cmp(&a.params.volume));
                    let names: Vec<String> = live.iter().take(6).map(|l| format!("{} {} vol {:.2}", l.cause, l.bank, l.params.volume)).collect();
                    erlog!("!!! sound: THE MIX RAN OVER FULL SCALE this second (peak {:.3}): the limiter held it at {:.3}; loudest live sounds: {}", window.peak, kh_audio::mixer::LIMIT, names.join(" | "));
                }
            }
            crate::audio_listener::log(&self.listener, &audio.engine.active(), control.flag("audio.listener_fix") != Some(false)); // (ER62)
            self.steps = 0;
        }
    }

    /// After one simulation step and its pose. Returns the effect seam's commands of the step
    /// (for the effect side).
    pub fn after_step(&mut self, out: &Output, animator: Option<&kh_anim::Animator>, pose: &kh_anim::Pose, targets: &ErTargets, sora: &kh_core::Sora) -> Vec<EffectCommand> {
        self.steps += 1;
        let status: &Status = sora.status();
        let effects = std::mem::take(&mut self.effects.0);
        let commands = std::mem::take(&mut self.audio.0);
        self.effect_step(out, animator, pose, targets, sora, &effects);
        // (AUDIT-A SOUND) the weapons' / pawns' own points (`kb:` / `pawn:`) the sounds on them need, from the effect side
        // on this step's pose (the tower's se02007_026 on its bigD_01, Pegasus's / the Chariot's sockets ...)
        let wanted = if self.sound_on { AUDIO.try_lock().ok().and_then(|g| g.as_ref().map(|a| a.weapon_anchors(&commands, &effects))).unwrap_or_default() } else { Vec::new() };
        let weapon_points = self.weapon_points(out, targets, sora, &wanted);
        let (Some(animator), true) = (animator, self.sound_on) else { return effects };
        if pose.component.is_empty() {
            return effects;
        }
        let Ok(mut guard) = AUDIO.try_lock() else { return effects };
        let Some(audio) = guard.as_mut() else { return effects };
        // (ER party S5) the party's looping sounds of the step (counted until stopped), its members' rows and weapons
        #[cfg(feature = "party")]
        let runs: Vec<(u8, u64)> = {
            let steps: Vec<(u8, u64)> = self.party.members.iter().map(|(m, _, a)| (*m, a.step)).collect();
            steps.into_iter().filter_map(|(m, s)| self.party.new_steps(m, s).map(|n| (m, n))).collect()
        };
        #[cfg(feature = "party")]
        {
            // the party went off or hidden: its looping sounds stop (no loop outlives it)
            if std::mem::take(&mut self.party.ended) {
                let n = self.party.loop_sounds.len();
                for h in std::mem::take(&mut self.party.loop_sounds).into_keys() {
                    audio.engine.seam_stop(h, 0.2);
                }
                erlog!("sound: (party) the party is off or hidden: its {n} looping sound(s) stop");
            }
            // (MIMIC-FX) a copy gone (it left, died away, the fight ended): its looping sounds stop
            for gone in copyfx::take_gone() {
                let mine: Vec<u32> = self.party.loop_sounds.keys().copied().filter(|h| kh_core::host::party::handle_member(*h).is_some_and(|m| m.0 == gone)).collect();
                for h in &mine {
                    audio.engine.seam_stop(*h, 0.2);
                    self.party.loop_sounds.remove(h);
                }
                if !mine.is_empty() {
                    erlog!("sound: (MIMIC-FX) {} is gone: its {} looping sound(s) stop", kh_audio::copies::name(gone), mine.len());
                }
            }
            self.party.track(&commands);
            for (m, _, a) in &self.party.members {
                if audio.engine.party(*m).is_some() {
                    audio.engine.set_party_anim(*m, &a.anim.0, a.anim.1);
                    // (MIMIC-FX) a Sora copy's swing / hit sets: his attack model's and his left hand's ("model|left")
                    if kh_audio::copies::is_sora_copy(*m) {
                        let (model, left) = a.weapon.split_once('|').unwrap_or((a.weapon.as_str(), ""));
                        if !model.is_empty() {
                            audio.engine.set_party_sora_weapons(*m, model, left);
                        }
                    } else if !a.weapon.is_empty() {
                        audio.engine.set_party_weapon(*m, &a.weapon);
                    }
                }
            }
        }
        crate::sora::kbdraw::select_sounds(&mut audio.engine); // (P5) his swing / hit sets: the keyblade in his hand's
        audio.queue.0.extend(commands);
        audio.anchors = weapon_points; // (AUDIT-A SOUND)
        #[cfg(feature = "party")]
        let party_fx = &self.party;
        let place = |at: &Attach| match at {
            Attach::World(point) => *point,
            Attach::Sora { bone, offset } => kh_host::seam::on_actor(out.position, out.yaw, bone.is_some(), *offset),
            Attach::Target { target, offset } => targets.get(*target).map_or(out.position + *offset, |e| e.pos + Vec3::Z * e.half_height + *offset),
            // (party stream E: compile fix only, the ER host has no party members)
            // (ER party S5) it has: on the member's bones of the step (his feet when he is not there)
            #[cfg(feature = "party")]
            Attach::Party { offset, .. } => party_fx.place(at).unwrap_or(out.position + *offset),
            #[cfg(not(feature = "party"))]
            Attach::Party { offset, .. } => out.position + *offset,
        };
        let to_world = kh_host::rig::component_to_world(out);
        // (ER77) the KH1 outfit worn: every voice line of this step is one of the user's KH1 clips, or silent
        audio.engine.set_voice_outfit(crate::boc::voice_outfit(&crate::boc::worn(), self.kh1_voice));
        let step = Step {
            out,
            dt: 1.0 / 60.0,
            skeleton: animator.skeleton(),
            pose,
            component_to_world: &to_world,
            effects: &effects,
            surface: self.surface,
            listener: self.listener,
            hp: self.alarm.then(|| (status.hp, status.max_hp())),
            place: &place,
        };
        #[cfg(not(feature = "party"))]
        audio.step(&step);
        // (ER party S5) THE MEMBERS' MOTION SOUNDS (their own detector over their bones, FsTable_Donald / _Goofy) on the
        // ground under them (the leader's when theirs is not known), after the step's commands, before the engine step
        #[cfg(feature = "party")]
        {
            let (surface, listener, params) = (self.surface, self.listener, &self.params);
            let mut events = 0u64;
            audio.step_with(&step, &mut |engine| {
                for (m, n) in &runs {
                    let Some((_, fx, a)) = party_fx.members.iter().find(|e| e.0 == *m) else { continue };
                    let positions: [Vec3; 14] = std::array::from_fn(|i| fx.bones.get(kh_audio::autose::BONES[i]).map_or(if i == 0 { fx.position } else { fx.position + Vec3::Z * 50.0 }, |b| b.0));
                    let frame = kh_audio::BoneFrame::from_positions(&positions, a.grounded);
                    // (ER party I, S5's request) his own ground (his ally body's reading) when it is known, else the leader's
                    let own = party::chara_of_member(*m).and_then(crate::allybody::ground_material);
                    let member_surface = params.member_surface(own, surface);
                    events += engine.party_auto_se(*m, kh_audio::AutoSeTick { dt: *n as f32 / 60.0, frame: &frame, surface: member_surface, anim: Some((a.anim.0.as_str(), a.anim.1)), listener: &listener, bone: &|b| fx.bones.get(b).map(|x| x.0) }).len() as u64;
                }
            });
            audio.auto_events += events;
            let engine = &audio.engine;
            let live: Vec<u32> = self.party.loop_sounds.keys().copied().filter(|h| engine.seam_live(*h)).collect();
            self.party.prune(|h| live.contains(&h));
        }
        effects
    }
}

impl Fx {
    /// (AUDIT-A SOUND) The world points of the weapon / pawn anchors `wanted` (kh-host audio/anchors.rs) as the effect side
    /// resolves them on this step's pose (`EffectHost::socket_point`: the form weapons' rigs under his clip, the tower's
    /// actor, the shield pawns' sockets). Empty without the effect side: the audio side then uses their mounts.
    fn weapon_points(&self, out: &Output, targets: &ErTargets, sora: &kh_core::Sora, wanted: &[&'static str]) -> Vec<(&'static str, Vec3)> {
        if wanted.is_empty() || !self.vfx_on {
            return Vec::new();
        }
        let Ok(guard) = EFFECTS.try_lock() else { return Vec::new() };
        let Some(effects) = guard.as_ref() else { return Vec::new() };
        let target = |id: kh_core::TargetId| targets.get(id).map(|e| e.pos + Vec3::Z * e.half_height);
        let scene = Scene { out, sora, tap: &self.tap, target: &target, ground: self.ground.or(Some(out.position.z)), eye: self.listener.position };
        wanted.iter().map(|name| (*name, effects.socket_point(&scene, name))).collect()
    }

    /// One simulation step of the effects: the bones of the step, the seam's commands, one evaluator step.
    fn effect_step(&mut self, out: &Output, animator: Option<&kh_anim::Animator>, pose: &kh_anim::Pose, targets: &ErTargets, sora: &kh_core::Sora, commands: &[EffectCommand]) {
        if !self.vfx_on {
            return;
        }
        if let (Some(animator), false) = (animator, pose.component.is_empty()) {
            // (ER72) with an outfit of other proportions on (KH1 Sora): the effects hang on ITS bones (boc.rs)
            let outfit_pose = crate::boc::effect_pose(animator.skeleton(), pose);
            self.tap.record(out.step, out, animator.skeleton(), outfit_pose.as_ref().unwrap_or(pose), &kh_host::rig::component_to_world(out));
            self.tap.samples.clear();
        }
        if out.on_floor {
            self.ground = Some(out.position.z);
        }
        let effect_dt = if self.effect_dt > 0.0 { self.effect_dt } else { 1.0 / 60.0 };
        self.seconds += effect_dt;
        let jumped = self.last_position.is_some_and(|p| p.distance(out.position) > 300.0);
        self.last_position = Some(out.position);
        // (never waited for: the only other taker is the loader thread, once)
        let Ok(mut guard) = EFFECTS.try_lock() else { return };
        let Some(effects) = guard.as_mut() else { return };
        let target = |id: kh_core::TargetId| targets.get(id).map(|e| e.pos + Vec3::Z * e.half_height);
        // (IMPL_ER53) the steady Keyblade glow's copies of KH3's appear effect (registered on first use)
        for (name, base, keep, hold) in kh_er_render::pointlight::glow_variants() {
            let keep: Vec<&str> = keep.iter().map(String::as_str).collect();
            effects.define_variant(name, base, &keep, hold);
        }
        if jumped {
            effects.cut_trails();
            effects.snap(); // (INT) and nothing is blended across the jump
        }
        self.int_cut_alpha = None; // (INT)
        effects.slash_follow = self.params.slash_follow;
        // (ER party S5) the members and spell actors of the step; the party switched off or hidden: what it started ends
        #[cfg(feature = "party")]
        if std::mem::take(&mut self.party.fed) {
            effects.set_party(self.party.members.iter().map(|(m, fx, _)| (*m, fx.clone())).collect(), self.party.projectiles.clone());
            if self.party.ended {
                let n = effects.stop_party(false);
                erlog!("vfx: (party) the party is off or hidden: its {n} effect(s) end");
            }
        }
        effects.step(effect_dt, commands, &Scene { out, sora, tap: &self.tap, target: &target, ground: self.ground.or(Some(out.position.z)), eye: self.listener.position });
        // What this host does not carry out (yet): kept for the once-a-second line.
        for shake in effects.take_shakes() {
            if self.shakes.len() < 8 {
                self.shakes.push(shake);
            }
        }
        self.scene_light = effects.scene_light();
        self.darkest = self.darkest.min(self.scene_light);
    }

    /// (IMPL_ER33) IN A REAL-TIME CUTSCENE Sora's steps do not run, and the effects stood still with them (drawn frozen:
    /// the user's log, 1763 instances a frame through a whole scene). The free effects (the save points: `cut.savepoint`)
    /// are to be seen alive in a scene: the effect host is stepped once per 1/60 s of real time, without commands and
    /// without a new pose (`out` = his last output). sora.rs calls it once a frame while a scene plays.
    pub fn cutscene_step(&mut self, out: &Output, sora: &kh_core::Sora, dt: f32) {
        if !self.vfx_on {
            return;
        }
        self.cut_pending = (self.cut_pending + dt.clamp(0.0, 0.1)).min(0.1);
        let Ok(mut guard) = EFFECTS.try_lock() else { return };
        let Some(effects) = guard.as_mut() else { return };
        while self.cut_pending >= 1.0 / 60.0 {
            self.cut_pending -= 1.0 / 60.0;
            self.seconds += 1.0 / 60.0;
            let target = |_: kh_core::TargetId| None;
            effects.step(1.0 / 60.0, &[], &Scene { out, sora, tap: &self.tap, target: &target, ground: self.ground.or(Some(out.position.z)), eye: self.listener.position });
            // (a shake an effect asks for during a scene is not played: the camera is the scene's)
            let _ = effects.take_shakes();
        }
        self.int_cut_alpha = Some((self.cut_pending * 60.0).clamp(0.0, 1.0)); // (INT)
    }

    /// (IMPL_ER75) The world moved under him (a rebase that moves him in the world, a teleport): the trails drop their history.
    pub fn cut_trails(&mut self) {
        if let Ok(mut guard) = EFFECTS.try_lock() {
            if let Some(effects) = guard.as_mut() {
                effects.cut_trails();
            }
        }
    }

    /// (ER party S5) THE PARTY OF THE COMING STEP (sora.rs, right before `after_step`): S1's feed (the members' taps, the
    /// spell actors / team props) and whether the party is shown (`PartyCore::visible`: on, not parked, members there).
    /// None = no party core (`party.er = 0`). A party that goes off or hidden has its sounds and effects stopped.
    #[cfg(feature = "party")]
    pub fn set_party(&mut self, party: Option<(&crate::party::glue::StepFeed, bool)>) {
        let tap = |t: &kh_partyhost::MemberTap| (t.member, t.step, t.position, t.yaw, t.bones.clone(), t.grounded, t.anim.clone(), t.weapon.clone());
        let (mut taps, projectiles, visible): (Vec<_>, Vec<_>, bool) = match party {
            Some((feed, visible)) => (if visible { feed.taps.iter().map(tap).collect() } else { Vec::new() }, if visible { feed.projectiles.iter().map(|p| (p.effect, p.position, p.forward)).collect() } else { Vec::new() }, visible),
            None => (Vec::new(), Vec::new(), false),
        };
        // (S8's request) THE STAGED FRIENDS (Goofy fighting Leonine, Donald at the Finger Reader, their own stage runtimes:
        // member ids Donald 0 / Goofy 1, never in the main party at the same time): their sounds and effects sit on them too
        let staged = crate::unlock::stage_taps();
        for (_, t) in &staged {
            if !taps.iter().any(|m: &(u8, _, _, _, _, _, _, _)| m.0 == t.member) {
                taps.push(tap(t));
            }
        }
        // (MIMIC-FX) THE COPIES (the Mimic Tears' copies of a character, each its own member id: kh_audio::copies): their sounds
        // and effects sit on THEIR bones, their spell actors' bodies follow them, their footsteps come from their feet; they are
        // fed while they are stepped, whether or not the party is shown (a Sora-led solo fight has no member of the party out)
        let copies = copyfx::taps();
        let mut projectiles = projectiles;
        projectiles.extend(copyfx::projectiles());
        let any_copy = !copies.is_empty();
        taps.extend(copies);
        self.party.feed(taps, projectiles, visible || !staged.is_empty() || any_copy);
    }

    /// (MIMIC-FX) The effects are on (`vfx`), for the copies' own effect steps (copyfx.rs).
    #[cfg(feature = "party")]
    pub fn vfx_active(&self) -> bool {
        self.vfx_on
    }

    /// (FX5) NOTHING OF AN ABSENT SORA IS DRAWN (the CV3 run: after the starter choice on a Donald start, KH3's Keyblade
    /// lay on the chapel floor until a reload). Sora out of the party is not stepped (party/mod.rs `step_sora`), so the
    /// core never carries out a stop asked of it: the hero light's steady Keyblade glow (`VW_SO010_APP0#glow`, its "Key"
    /// emitter is the Keyblade's own mesh, persistent at R_wep_skt, sora.rs IMPL_ER53) and every other effect of his ran
    /// on where his last step left them. Called once per step (sora.rs, before `after_step`): the step he becomes absent
    /// (out of the party and not staged at Margit) every effect of his ends at once, here, in the effect host
    /// (`EffectHost::stop_sora`); the party's and the free ones (save points, prizes) stay. Back in the party: nothing.
    #[cfg(feature = "party")]
    pub fn sora_absent(&mut self, absent: bool) {
        if !absent || self.sora_gone {
            self.sora_gone = absent;
            return;
        }
        // (never waited for, as `effect_step`: the loader thread holds it once; then the next step does it)
        let Ok(mut guard) = EFFECTS.try_lock() else { return };
        self.sora_gone = true;
        let n = guard.as_mut().map_or(0, |effects| effects.stop_sora(true));
        erlog!("vfx: (FX5) Sora is out of the party: his {n} effect(s) end at once (nothing of an absent Sora is drawn: the Keyblade's glow, his trails, his sparkles)");
    }

    /// The camera shakes the effects asked for since the last call (cam.rs plays them on the camera in use).
    pub fn take_shakes(&mut self) -> Vec<kh_host::vfx::Shake> {
        std::mem::take(&mut self.shakes)
    }

    /// (INT) A discontinuity (sora.rs `int_snap_check`): drawn at the latest step until the next one.
    pub fn snap(&mut self) {
        if let Ok(mut guard) = EFFECTS.try_lock() {
            if let Some(effects) = guard.as_mut() {
                effects.snap();
            }
        }
    }

    /// Once per game frame, after the frame's steps (PostPhysics): the effects' draw list of the
    /// frame goes to the renderer, for the camera as it is now.
    /// (INT) `alpha` of the way from the step before to the latest one: AS SORA IS DRAWN (sora.rs's alpha;
    /// the cutscene's own between its steps); the material clock likewise.
    /// (INT) `drawn` = a scene of Sora AS DRAWN this frame (sora.rs `int_drawn_scene`): the effects on
    /// his sockets and the Keyblade trail's head are drawn on his drawn sockets (`EffectHost::set_drawn`).
    pub fn draw_drawn(&mut self, frame: &Frame, alpha: f32, drawn: Option<&kh_host::vfx::Scene>) {
        let alpha = self.int_cut_alpha.unwrap_or(alpha).clamp(0.0, 1.0);
        let clock_dt = if self.int_cut_alpha.is_some() { 1.0 / 60.0 } else if self.effect_dt > 0.0 { self.effect_dt } else { 1.0 / 60.0 };
        if !self.vfx_on {
            return;
        }
        let Some(camera) = crate::game::camera() else { return };
        let Ok(mut guard) = EFFECTS.try_lock() else { return };
        let Some(effects) = guard.as_mut() else { return };
        let view = kh_vfx::Camera { position: frame.pos_er_to_kh(camera.position), forward: coords::dir_er_to_kh(camera.forward).normalize_or(Vec3::X), right: coords::dir_er_to_kh(camera.right).normalize_or(Vec3::Y), up: coords::dir_er_to_kh(camera.up).normalize_or(Vec3::Z) };
        if let Some(s) = drawn.filter(|_| alpha < 1.0 && self.int_cut_alpha.is_none()) {
            effects.set_drawn(s);
        }
        let (list, materials, meshes) = effects.draw(alpha, &view);
        kh_er_render::vfx::submit(list, materials, meshes, &view, self.seconds - (1.0 - alpha) * clock_dt);
        // (VFXR) the effects' character-material drives (SQEX_Material_InLight ...), on the character each effect hangs on
        kh_er_render::vfx::set_char_drives(effects.char_drives().into_iter().map(|d| kh_er_render::vfx::CharDrive { owner: d.owner, name: d.name, value: d.value, vector: d.vector, mesh: d.mesh /* (AUDIT-B W-VFX) */, equipment: d.equipment /* (FREEZE-FX) */ }).collect());
        effects.clear_drawn();
    }

    /// The effects' once-a-second lines.
    fn vfx_tick(&mut self) {
        let Ok(mut guard) = EFFECTS.try_lock() else { return };
        let Some(effects) = guard.as_mut() else {
            erlog!("vfx: not ready (the effect pack is being read, or there is none)");
            return;
        };
        for line in effects.log.drain(..) {
            erlog!("{line}");
        }
        // (ER party S5) THE PARTY'S LOOPS (looping sounds, persistent effects, endless one-shots still running): a line a
        // second while any runs, and one when they are back to 0 (after a move every loop it started has ended)
        #[cfg(feature = "party")]
        {
            let text = party::loops_text(&self.party.loops(&effects.party_loops()));
            if text != "0" || (!self.party.told.is_empty() && self.party.told != "0") {
                erlog!("party loops: {text}");
            }
            self.party.told = text;
        }
        let (systems, emitters, particles, open, ending) = effects.stats();
        if systems > 0 || !self.shakes.is_empty() || self.darkest < 1.0 {
            erlog!("vfx: {systems} systems live ({open} persistent open, {ending} ending), {emitters} emitters, {particles} particles; {} spawned so far", effects.spawned);
            // (IMPL_ER75) the Keyblade trails of the last second (the newest sample against the blade's tip after each step)
            let t = effects.take_trail_stats();
            if t.steps > 0 || t.cuts > 0 {
                erlog!("trail: {} step(s) with a trail; newest sample to the blade tip at most {:.1} cm; history up to {} sample(s); effect clock step {:.4} s (the world's); history cut {} time(s) (teleport / world shifted under him)", t.steps, t.max_gap, t.max_samples, t.dt, t.cuts);
            }
        }
        if self.darkest < 1.0 {
            erlog!("vfx: scene light {:.2} asked by an effect in this second (the scene is multiplied by its square in the late pass)", self.darkest);
            self.darkest = 1.0;
        }
    }
}

impl Drop for Fx {
    /// Sora is gone (a loading screen, the payload's release): what was playing stops.
    fn drop(&mut self) {
        if let Ok(mut guard) = AUDIO.try_lock() {
            if let Some(audio) = guard.as_mut() {
                audio.out.stop_all();
            }
        }
        if let Ok(mut guard) = EFFECTS.try_lock() {
            if let Some(effects) = guard.as_mut() {
                effects.clear();
            }
        }
        kh_er_render::vfx::clear();
    }
}

/// (prize) (savepoint) The effect host, for the parts of this host that draw FREE effects (the
/// prize orbs): `f` runs when the effect side is loaded and not busy; false = it did not run.
pub fn with_effects(f: impl FnOnce(&mut EffectHost)) -> bool {
    let Ok(mut guard) = EFFECTS.try_lock() else { return false };
    match guard.as_mut() {
        Some(effects) => {
            f(effects);
            true
        }
        None => false,
    }
}
