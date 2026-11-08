//! THE WHOLE AUDIO SIDE AS ONE OBJECT, for a host without an ECS (the Elden Ring mod, the
//! offline proof). What kh-app's `audio::update` does per frame, done per simulation step:
//!
//! ```text
//! core step (audio = &mut host_audio.queue)  ->  pose of the step  ->  host_audio.step(..)
//! ```
//!
//! `step` records the bones, applies the step's seam commands (Play / Stop / voice group number)
//! and effect commands, the sound states of the step's events, runs ONE engine step (motion
//! sounds on the surface the host names, the low-HP alarm from his HP, 3D against the host's
//! listener) and hands the voice commands to the output (`threaded::Output`: never blocks).
//! While the host's world stands still it calls `idle` once a frame instead: the SE categories
//! are paused (the pack's STATE_MENU_OPEN event, as the game's pause does) and the engine runs
//! on the frame clock so that fades go on; `idle` with `paused = false` resumes.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::Arc;

use glam::Vec3;
use kh_audio::{Engine, Language, Listener, Pack};
use kh_core::Attach;

use super::anchors::Points; // (AUDIT-A SOUND)
use super::pose::PoseTap;
use super::session::{self, Alarm, Attaches, Tick};
use super::threaded::Output;
use super::FRAMES_PER_STEP;
use crate::seam::{AudioCommand, AudioQueue, EffectCommand};

/// One simulation step, as the audio side needs it.
pub struct Step<'a> {
    pub out: &'a kh_core::Output,
    /// The step's length, s (1 / 60).
    pub dt: f32,
    pub skeleton: &'a kh_anim::Skeleton,
    pub pose: &'a kh_anim::Pose,
    pub component_to_world: &'a kh_anim::Trs,
    /// The effect seam's commands of this step (sounds carried by particle systems).
    pub effects: &'a [EffectCommand],
    /// The `SurfaceType` under him.
    pub surface: u32,
    pub listener: Listener,
    /// His HP and maximum (the low-HP alarm); `None`: no alarm.
    pub hp: Option<(i32, i32)>,
    /// Where an attach that is not a bone or socket of the pose is now (a world point, his
    /// origin, a target).
    pub place: &'a dyn Fn(&Attach) -> Vec3,
}

pub struct HostAudio {
    /// Hand this to the core as its `audio` for the step.
    pub queue: AudioQueue,
    pub engine: Engine,
    pub out: Output,
    pub tap: PoseTap,
    attaches: Attaches,
    hud_owns_alarm: bool,
    paused: bool,
    steps: u64,
    paused_clock: f64,
    log: Option<BufWriter<File>>,
    pub listener: Listener,
    /// Seam sounds asked for / those nothing could be played for / motion sounds detected.
    pub played: u64,
    pub missing: u64,
    pub auto_events: u64,
    /// The surface of the latest step and the latest motion sound (for a host's log).
    pub surface: u32,
    pub last_auto: Option<String>,
    /// Seam names without a route, each once (a host logs the new ones).
    pub unknown: Vec<String>,
    /// (ui gain) The linear gain of the voices of sounds started through [`HostAudio::ui_commands`] (a host whose
    /// menus must be heard over another game's audio; 1 = as authored), the engine keys of those sounds and their voices.
    pub ui_gain: f32,
    /// (THE USER 2026-11-02: "...shotlock's startup sfx is way too loud") The peak (0 .. 1 of full scale, before the
    /// output's master gain) `ui_gain` may raise a HUD sound to, never below its authored level (`session::ui_voice_gain`;
    /// the Elden Ring host's sheet row `sound.ui_peak_ceiling`). 1 by default: never raised into clipping.
    pub ui_peak_ceiling: f32,
    ui_keys: Vec<kh_audio::SoundKey>,
    /// The voices of those sounds: (voice, its sound, the gain it was started with: its updates keep it).
    ui_voices: Vec<(kh_audio::VoiceId, kh_audio::SoundKey, f32)>,
    /// (THE USER 2026-11-02) The largest gain a HUD sound got since the last `take_ui_raised` (1 = none raised): a host's
    /// per-second log line.
    ui_raised: f32,
    /// (ER77) The outfit voice lines of the engine (which clip stood for which KH3 line, or why nothing was played), for a
    /// host's log: taken by the host (at most 64 kept).
    pub voice_notes: Vec<String>,
    /// (GOS) [`HostAudio::set_silence`] is in force: every sound but the menus' started under it is silent.
    silenced: bool,
    /// (AUDIT-A SOUND) THE WEAPONS' / PAWNS' OWN POINTS of the coming step (world, cm): `kb:<model>:<mount>:<bone>` and
    /// `pawn:<row>` anchors (super::anchors) as the host's effect side resolves them (`EffectHost::socket_point`), for the
    /// names [`HostAudio::weapon_anchors`] asks for. Set by the host before `step`; a name without a point sounds at its
    /// mount.
    pub anchors: Vec<(&'static str, Vec3)>,
    /// (AUDIT-A SOUND) Where each seam play of the latest step was put (handle, world point): for the audits.
    pub placed: Vec<(u32, Vec3)>,
    /// (AUDIT-A SOUND) Sora's clip time of the latest step (a `pawn:` anchor's fallback track).
    anchor_time: f32,
}

impl HostAudio {
    /// (DON-KIT) The sounds of these effects read ahead (the item effects: `crate::vfx::item_effect_names`), so the first
    /// use is never silent or late. How many materials were asked for.
    pub fn preload_effect_sounds(&mut self, names: &[&str]) -> usize {
        let list = self.engine.effect_materials(names);
        let n = list.len();
        self.out.preload(list);
        n
    }

    /// (AUDIT-A) Sora's engine: the pack's routes plus the random voice lists of the core's clips the pack lacks
    /// (`core_lists`: the formchanges' anim sets).
    fn sora_engine(pack: Arc<Pack>, seed: u32, language: Language) -> Engine {
        let mut engine = Engine::new(pack, seed, language);
        super::core_lists::add_core_random_voices(&mut engine.routes);
        engine
    }

    pub fn new(pack: Arc<Pack>, out: Output, seed: u32, language: Language) -> HostAudio {
        HostAudio {
            queue: AudioQueue::default(),
            engine: Self::sora_engine(pack, seed, language), // (AUDIT-A)
            out,
            tap: PoseTap::default(),
            attaches: Attaches::default(),
            hud_owns_alarm: false,
            paused: false,
            steps: 0,
            paused_clock: 0.0,
            log: None,
            listener: Listener::default(),
            played: 0,
            missing: 0,
            auto_events: 0,
            ui_gain: 1.0,
            ui_peak_ceiling: 1.0,
            ui_keys: Vec::new(),
            ui_voices: Vec::new(),
            ui_raised: 1.0,
            voice_notes: Vec::new(),
            silenced: false,
            anchors: Vec::new(), // (AUDIT-A SOUND)
            placed: Vec::new(),
            anchor_time: 0.0,
            surface: 0,
            last_auto: None,
            unknown: Vec::new(),
        }
    }

    /// One line per sound event into `path` (the test app's `--audio-log` format).
    pub fn log_to(&mut self, path: &str) -> std::io::Result<()> {
        let mut file = BufWriter::new(File::create(path)?);
        writeln!(file, "{}", session::AUDIO_LOG_HEADER)?;
        self.log = Some(file);
        Ok(())
    }

    /// Sora starts again (a respawn): every voice stops, the engine is new, the output stays.
    pub fn restart(&mut self, seed: u32) {
        let (pack, language) = (self.engine.pack().clone(), self.engine.language);
        self.out.stop_all();
        self.engine = Self::sora_engine(pack, seed, language); // (AUDIT-A)
        self.queue.0.clear();
        self.attaches.clear();
        // (THE USER 2026-11-02: "sometimes Audio from specifically our mod will become very loud") the new engine counts
        // its sound keys and voices from 1 again: the menus' keys and voices of the old one must not stay, or the first
        // of Sora's own sounds after a load / respawn with the same numbers were played at the menus' gain (+12 dB)
        self.ui_keys.clear();
        self.ui_voices.clear();
        self.tap = PoseTap::default();
        self.hud_owns_alarm = false;
        self.paused = false;
        // (GOS) a silence in force holds for the new engine too (a respawn under the game over's black plate)
        if self.silenced {
            self.engine.driver.set_silence(0.0, 0.0);
        }
    }

    /// (GOS, THE USER 2026-11-03: "silence all music and other SFX when on death screen" / "(except for menu SFX, dont
    /// silence that)") THE HOST'S SILENCE: `on` = every sound of the engine (Sora's and the party's voices, battle and
    /// world sounds, the alarm) fades to nothing over `fade` s, and so does every one started later, EXCEPT the menus'
    /// sounds started through [`HostAudio::ui_commands`] while it holds (marked by their sound, whatever category they
    /// share with the world); `on = false` gives everything back over `fade` s. Idempotent. True: the state changed.
    pub fn set_silence(&mut self, on: bool, fade: f32) -> bool {
        if self.silenced == on {
            return false;
        }
        self.silenced = on;
        self.engine.driver.set_silence(if on { 0.0 } else { 1.0 }, fade.max(0.0));
        true
    }

    /// (GOS) The host's silence is in force.
    pub fn silenced(&self) -> bool {
        self.silenced
    }

    /// Where an attach is now: a bone or a socket of Sora from the latest pose when there is
    /// one (the offset in the bone's own frame), else the host's answer.
    /// (AUDIT-A SOUND) + a weapon's / pawn's own point (`points`: the host's answers for `kb:` / `pawn:` anchors).
    fn place(tap: &PoseTap, pack: &Pack, points: &Points, at: &Attach, host: &dyn Fn(&Attach) -> Vec3) -> Vec3 {
        if let Some(position) = points.place(at, tap, &pack.sockets, host) {
            return position;
        }
        if let Attach::Sora { bone: Some(bone), offset } = at {
            if let Some(position) = tap.point(bone, *offset, &pack.sockets) {
                return position;
            }
        }
        host(at)
    }

    /// (AUDIT-A SOUND) The weapon / pawn anchors (`kb:` / `pawn:`) whose points the coming step needs: those of the plays
    /// queued (`self.queue` and `commands`, the host's own queue not yet handed over), of the step's effect spawns
    /// (`effects`: their carried sounds) and of the live seam sounds and effects (followed). The host answers them in
    /// [`HostAudio::anchors`] before `step`.
    pub fn weapon_anchors(&self, commands: &[AudioCommand], effects: &[EffectCommand]) -> Vec<&'static str> {
        let mut names = super::anchors::wanted(&self.queue.0, effects, self.attaches.seam.values().chain(self.attaches.effect.values()));
        for name in super::anchors::wanted(commands, &[], std::iter::empty()) {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names
    }

    /// After one simulation step and its pose.
    pub fn step(&mut self, s: &Step) {
        self.step_with(s, &mut |_| {});
    }

    /// (ER party S5) `step` with `before_run` called on the engine after the step's seam and effect commands and before
    /// the engine step (the party members' motion sounds: kh-app audio.rs runs them at the same place).
    pub fn step_with(&mut self, s: &Step, before_run: &mut dyn FnMut(&mut Engine)) {
        self.steps += 1;
        self.paused_clock = 0.0;
        self.listener = s.listener;
        self.surface = s.surface;
        if self.paused {
            self.paused = false;
            self.engine.event("STATE_MENU_CLOSE");
        }
        self.tap.record(self.steps, s.out, s.skeleton, s.pose, s.component_to_world);
        let sample = self.tap.samples.pop();
        self.tap.samples.clear();
        let pack = self.engine.pack().clone();
        let anim = (s.out.anim_ticked.name.name(), s.out.anim_ticked.time);
        let reflected = session::reflected_hit_sounds(&s.out.events);
        self.anchor_time = s.out.anim_ticked.body_samples()[0].2; // (AUDIT-A SOUND)
        self.placed.clear();
        let anchors = std::mem::take(&mut self.anchors);
        let points = Points { points: &anchors, time: self.anchor_time };
        for command in std::mem::take(&mut self.queue.0) {
            match command {
                AudioCommand::Play(handle, play) => {
                    self.played += 1;
                    if reflected.contains(&handle.0) {
                        continue;
                    }
                    let position = Self::place(&self.tap, &pack, &points, &play.at, s.place);
                    self.placed.push((handle.0, position)); // (AUDIT-A SOUND)
                    if !session::seam_play(&mut self.engine, &mut self.attaches, &mut self.hud_owns_alarm, handle.0, &play, position, anim) {
                        self.missing += 1;
                    }
                }
                AudioCommand::Stop(handle, fade_out) => self.engine.seam_stop(handle.0, fade_out),
                AudioCommand::VoiceGroupNo(number) => self.engine.set_voice_group_no(number),
            }
        }
        {
            let tap = &self.tap;
            let place = |at: &Attach| Self::place(tap, &pack, &points, at, s.place);
            for command in s.effects {
                session::effect_command(&mut self.engine, &mut self.attaches, command, &place);
            }
        }
        for name in session::sound_states(&mut self.engine, &s.out.events) {
            let text = format!("sound state {name}");
            if !self.unknown.contains(&text) {
                self.unknown.push(text);
            }
        }
        let alarm = match s.hp {
            Some((hp, max)) if !self.hud_owns_alarm => Alarm::Hp { low: kh_audio::alarm::low_hp(hp, max) },
            Some(_) => Alarm::Hud,
            None => Alarm::Hp { low: false },
        };
        before_run(&mut self.engine); // (ER party S5)
        self.anchors = anchors; // (AUDIT-A SOUND) kept for the engine step's following (and `idle`) until the host's next ones
        self.run(s.dt, false, sample.as_ref(), s.surface, alarm, s.place);
        self.out.advance(FRAMES_PER_STEP);
    }

    /// (UI) THE SOUNDS OF A MENU, played now at the listener: the HUD's commands (`Hud::take_sounds`) of a frame in which
    /// no simulation step runs (a menu holds the world, or there is no world at all: a title). The engine is advanced by
    /// [`HostAudio::idle`] meanwhile. Returns how many sounds were started.
    pub fn ui_commands(&mut self, commands: Vec<AudioCommand>) -> usize {
        let mut started = 0;
        let position = self.listener.position;
        for command in commands {
            match command {
                AudioCommand::Play(handle, play) => {
                    self.played += 1;
                    if let Some(key) = session::seam_play_key(&mut self.engine, &mut self.attaches, &mut self.hud_owns_alarm, handle.0, &play, position, ("", 0.0)) {
                        started += 1;
                        // (GOS) a menu's sound started under the silence is not silenced (the game over's loop, its
                        // menu, the Load screen); one started before it is (an alarm the HUD began in the fight)
                        if self.silenced {
                            self.engine.driver.set_exempt(key);
                        }
                        if self.ui_keys.len() >= 64 {
                            self.ui_keys.remove(0);
                        }
                        self.ui_keys.push(key);
                    } else {
                        self.missing += 1;
                    }
                }
                AudioCommand::Stop(handle, fade_out) => self.engine.seam_stop(handle.0, fade_out),
                AudioCommand::VoiceGroupNo(number) => self.engine.set_voice_group_no(number),
            }
        }
        started
    }

    /// A frame in which the simulation took no step. `paused`: the host's world stands still
    /// (the SE categories pause, as in the game's pause; the engine runs on the frame clock).
    pub fn idle(&mut self, dt: f32, paused: bool, listener: Listener, place: &dyn Fn(&Attach) -> Vec3) {
        self.listener = listener;
        if paused != self.paused {
            self.paused = paused;
            self.engine.event(if paused { "STATE_MENU_OPEN" } else { "STATE_MENU_CLOSE" });
        }
        if !paused {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.paused_clock += dt as f64;
        self.run(dt, true, None, self.surface, Alarm::Hp { low: false }, place);
        self.out.advance((dt * kh_audio::mixer::RATE as f32).round() as usize);
    }

    fn run(&mut self, dt: f32, standing: bool, sample: Option<&super::pose::BoneSample>, surface: u32, alarm: Alarm, host_place: &dyn Fn(&Attach) -> Vec3) {
        let pack = self.engine.pack().clone();
        let tap = &self.tap;
        let points = Points { points: &self.anchors, time: self.anchor_time }; // (AUDIT-A SOUND)
        let place = |at: &Attach| Self::place(tap, &pack, &points, at, host_place);
        let listener = self.listener;
        let (commands, detected) = session::tick(&mut self.engine, &self.attaches, &Tick { dt, standing, sample, surface: &|_| surface, alarm, listener: &listener, tap, place: &place });
        for (event, surface) in &detected {
            self.auto_events += 1;
            self.last_auto = Some(format!("{} {} volume {:.2} surface {surface}", event.motion.name(), event.part.bone(), event.volume));
        }
        // (ui gain) the menus' voices at the host's gain
        // (THE USER 2026-11-02: "...shotlock's startup sfx is way too loud") ... capped per sound by its authored peak
        // (`session::ui_voice_gain`), fixed for the voice at its start; the keys and voices of sounds that have ended go
        let mut commands = commands;
        if !(self.ui_keys.is_empty() && self.ui_voices.is_empty()) {
            for command in &mut commands {
                match command {
                    kh_audio::VoiceCommand::Start { voice, sound, params, .. } if self.ui_keys.contains(sound) => {
                        let gain = session::ui_voice_gain(self.ui_gain, self.ui_peak_ceiling, self.engine.driver.peak_level(*sound));
                        params.volume *= gain;
                        self.ui_raised = self.ui_raised.max(gain);
                        if self.ui_voices.len() >= 64 {
                            self.ui_voices.remove(0);
                        }
                        self.ui_voices.push((*voice, *sound, gain));
                    }
                    kh_audio::VoiceCommand::Update { voice, params } => {
                        if let Some((_, _, gain)) = self.ui_voices.iter().find(|v| v.0 == *voice) {
                            params.volume *= gain;
                        }
                    }
                    kh_audio::VoiceCommand::Stop { voice } => self.ui_voices.retain(|v| v.0 != *voice),
                    _ => {}
                }
            }
            let driver = &self.engine.driver;
            self.ui_keys.retain(|k| driver.is_playing(*k));
            self.ui_voices.retain(|v| driver.is_playing(v.1));
        }
        self.out.send(&commands);
        let lines = self.engine.take_log();
        for line in lines.iter().filter(|l| l.kind.starts_with("outfit-")) {
            if self.voice_notes.len() < 64 {
                self.voice_notes.push(format!("{}: {} ({})", line.kind, line.note, line.cause));
            }
        }
        if let Some(file) = &mut self.log {
            let time = self.steps as f64 / 60.0 + self.paused_clock;
            for line in &lines {
                writeln!(file, "{}", session::log_line(self.steps, time, line)).ok();
            }
        }
        for name in self.engine.unknown.drain(..) {
            if self.unknown.len() < 64 && !self.unknown.contains(&name) {
                self.unknown.push(name);
            }
        }
        self.attaches.retire(&self.engine);
    }

    pub fn steps(&self) -> u64 {
        self.steps
    }

    /// (THE USER 2026-11-02) The largest gain a HUD sound was started with since the last call (1 = none was raised),
    /// and how many HUD sounds are live now: for a host's per-second log line.
    pub fn take_ui_raised(&mut self) -> (f32, usize) {
        (std::mem::replace(&mut self.ui_raised, 1.0), self.ui_keys.len())
    }

    pub fn flush(&mut self) {
        if let Some(file) = &mut self.log {
            file.flush().ok();
        }
        self.out.finish();
    }
}
