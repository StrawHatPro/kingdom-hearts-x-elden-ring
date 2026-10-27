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
//! One line `sound: ...` a second says what is active; `surface: ...` is logged once for every
//! new ground material the game reports under the player.
//!
//! EFFECTS. The core's `Effects` seam is `Fx::effects` (a queue); after every simulation step
//! the step's commands go to `kh_host::vfx::EffectHost` (what a spawn means, the sockets of the
//! step's pose, the Keyblade trail, targets = Elden Ring's characters, one evaluator step), and
//! once a frame (`draw`, PostPhysics) its draw list goes to `kh_er_render::vfx`, which draws it
//! with the game's own effect shaders in OUR command list right after Sora (the renderer's
//! scene arrangement). Hit sparks and swing trails on Elden Ring's enemies come the same way:
//! the core spawns them at the hit points it computes.
//! - `vfx = 0`: no effects (everything alive ends; nothing is evaluated or drawn);
//! - `vfx.distort = 0`: no distortion (refraction) twins: no copy of the scene colour is made;
//! - `vfx.decals = 0`: no projected decals; `vfx.hdr = x`: a factor on the effects' brightness
//!   (default: sheet row `vfx.hdr_scale`); `vfx.solid = 1`: diagnosis, the geometry in magenta.
//! - (IMPL_ER39) `fx.light = 0`: the effects' particle lights light nothing; `r.hero_light = 0 / 1 / 2`: the hero
//!   light in the dark off / automatic (default) / always on; `fx.light_gain`, `hero.intensity`: their strength.
//! REQUESTS OF THE EFFECTS THAT THIS HOST DOES NOT CARRY OUT YET (logged as `vfx: REQUEST ...`):
//! camera shakes (`EffectHost::take_shakes`: class, scale, duration) and the scene darkening of
//! Thunder and the grand spells (`EffectHost::scene_light`).

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
            audio.ui_gain = gain;
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
    let Ok(mut guard) = AUDIO.try_lock() else { return };
    let Some(audio) = guard.as_mut() else { return };
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
            audio.out.set_master(db_to_gain(volume_db.clamp(-60.0, 12.0)));
            audio.idle(dt, true, Listener::default(), &|_| Vec3::ZERO);
        }
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
                    let pack = Arc::new(pack);
                    erlog!("sound: audio pack {} read in {:.2} s: {} banks, {} sound assets, {} triggers, voices {}", dir.display(), started.elapsed().as_secs_f32(), pack.banks.len(), pack.assets.len(), pack.triggers.len(), language.folder());
                    let out = kh_host::audio::Output::device(pack.clone(), true, log);
                    let mut host = HostAudio::new(pack, out, 1, language);
                    // (DON-KIT) the item effects' sounds read ahead with the effects (the first item use was late / silent)
                    let n = host.preload_effect_sounds(&kh_host::vfx::item_effect_names());
                    erlog!("sound: (DON-KIT) the item effects' sounds read ahead ({n} materials)");
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
    sora_gone: bool,
}

impl Fx {
    /// When Sora starts (payload start, respawn). `sheet`: sheets/port/sora-er-fx.json.
    pub fn start(pack_dir: &std::path::Path, sheet: &std::path::Path, control: &Control) -> Fx {
        let params = match std::fs::read_to_string(sheet).map_err(|e| e.to_string()).and_then(|text| FxParams::from_sheet(&text)) {
            Ok((params, problems)) => {
                erlog!("fx: numbers {} ({} rows not understood): {} surface rows, default surface {}, volume {} dB", sheet.display(), problems.len(), params.surfaces.len(), params.surface_default, { UI_GAIN_SHEET.store(params.ui_gain_db.to_bits(), Ordering::Relaxed); params.volume_db });
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
        Fx { cut_pending: 0.0, audio: AudioQueue::default(), effects: EffectQueue::default(), surface: params.surface_default, params, sound_on: true, alarm: true, material: -1, materials_seen: Vec::new(), listener: Listener::default(), since_log: 0.0, unknown_told: 0, told_start: false, steps: 0, vfx_on: true, vfx_told: None, tap: PoseTap::default(), ground: None, seconds: 0.0, shakes: Vec::new(), scene_light: 1.0, darkest: 1.0, effect_dt: 1.0 / 60.0, last_position: None, kh1_voice: None, #[cfg(feature = "party")] party: party::PartyFx::default(), int_cut_alpha: None, sora_gone: false }
    }

    /// Once per game frame, before the simulation steps of the frame.
    pub fn frame(&mut self, player: &PlayerIns, control: &Control, dt: f32, paused: bool, frame: &Frame) {
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
        audio.out.set_master(if self.sound_on { db_to_gain(volume_db) * crate::erconfig::master_gain() } else { 0.0 });
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
            self.party.track(&commands);
            for (m, _, a) in &self.party.members {
                if audio.engine.party(*m).is_some() {
                    audio.engine.set_party_anim(*m, &a.anim.0, a.anim.1);
                    if !a.weapon.is_empty() {
                        audio.engine.set_party_weapon(*m, &a.weapon);
                    }
                }
            }
        }
        audio.queue.0.extend(commands);
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
                    events += engine.party_auto_se(*m, *n as f32 / 60.0, &frame, member_surface, Some((a.anim.0.as_str(), a.anim.1)), &listener, &|b| fx.bones.get(b).map(|x| x.0)).len() as u64;
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
        self.party.feed(taps, projectiles, visible || !staged.is_empty());
    }

    /// (FX5) NOTHING OF AN ABSENT SORA IS DRAWN (the CV3 run: after the starter choice on a Donald start, KH3's Keyblade
    /// lay on the chapel floor until a reload). Sora out of the party is not stepped (party/mod.rs `step_sora`), so the
    /// core never carries out a stop asked of it: the hero light's steady Keyblade glow (`VW_SO010_APP0#glow`, its "Key"
    /// emitter is the Keyblade's own mesh, persistent at R_wep_skt, sora.rs IMPL_ER53) and every other effect of his ran
    /// on where his last step left them. Called once per step (sora.rs, before `after_step`): the step he becomes absent
    /// (out of the party and not staged at Margit) every effect of his ends at once, here, in the effect host
    /// (`EffectHost::stop_sora`); the party's and the free ones (save points, prizes) stay. Back in the party: nothing.
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
    pub fn draw(&mut self, frame: &Frame) {
        self.draw_at(frame, 1.0);
    }

    /// (INT) As `draw`, `alpha` of the way from the step before to the latest one: AS SORA IS DRAWN (sora.rs's alpha;
    /// the cutscene's own between its steps); the material clock likewise.
    pub fn draw_at(&mut self, frame: &Frame, alpha: f32) {
        self.draw_drawn(frame, alpha, None);
    }

    /// (INT) As `draw_at`, with `drawn` = a scene of Sora AS DRAWN this frame (sora.rs `int_drawn_scene`): the effects on
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
        kh_er_render::vfx::set_char_drives(effects.char_drives().into_iter().map(|d| kh_er_render::vfx::CharDrive { owner: d.owner, name: d.name, value: d.value, vector: d.vector }).collect());
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
