//! What is heard when a sound index is started: the port of the native driver `SQEX::Sd` as
//! docs/sora-audio-spec.md section 2 describes it, with the 3D side of section 3.3 and the
//! category events of 3.4.
//!
//! The driver owns no audio device. `play` registers a sound (after its start macro and its
//! category pool), `update(dt, listener)` runs every live sound's sequences and tracks and
//! returns the VOICE COMMANDS of that step for a backend: start material X at volume / pitch /
//! pan (with its loop flag), update, pause, stop. `mixer.rs` is a backend.
//!
//! TIME. The game runs this on its own thread with real-time dt. The port is stepped by the
//! host; `kh-app` steps it once per fixed simulation step (1/60 s), which makes a run's sound
//! events repeatable. Nothing here feeds back into the simulation.
//!
//! RANDOMNESS: `CrtRand` seeded by the host (rng.rs).
//!
//! ASSUMPTIONS beyond the spec's own (each is marked where it is used):
//! - A1 a sound's sequences start at the first `update` after `play`, with no time elapsed;
//! - A2 the working set of a start macro is every live sound that is not already being stopped
//!   (spec open question 7);
//! - A3 a track's command clock is the sequence's (pitch-scaled);
//! - A4 `keyOff` and the replacement of a sequence's track end the voice at once (no release
//!   envelope was read);
//! - A5 LFO wave 0 is a sine, any other a triangle (four of Sora's tracks have an LFO);
//! - A6 the 3D pan and a track's own pan add; the pan unit is the 3D one (angle / pi);
//! - A7 category pause freezes the clocks of the category's sounds once the pause fade is done;
//! - A8 the Cycle sound's later picks use the same roll as its first; with interval 0 one pick
//!   is made per update;
//! - A9 the 3D volume curve is curve 0 (linear): byte 0x21 of the bank sound header is 0 on
//!   every one of Sora's banks (spec 3.3) and is not carried in the manifest.

use std::collections::HashMap;
use std::sync::Arc;

use glam::Vec3;

use crate::pack::{Cmp, EventCmd, MacroCmd, MacroField, Pack, Param, Selection, SeqCmd, SoundDef, TrackCmd};
use crate::rng::CrtRand;
use crate::rules::RULES;

/// One playing sound (an instance of a bank sound). Counts up from 1.
pub type SoundKey = u32;
/// One voice (a playing material). Counts up from 1.
pub type VoiceId = u32;

/// The fade curves 0..5 (0x14596A9C0): `t`, `1-(1-t^2)^3`, `1-(1-t)^2`, `t^2`, `2^t-1`, `2-2^(1-t)`.
pub fn curve(number: u8, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match number {
        1 => 1.0 - (1.0 - t * t).powi(3),
        2 => 1.0 - (1.0 - t) * (1.0 - t),
        3 => t * t,
        4 => 2f32.powf(t) - 1.0,
        5 => 2.0 - 2f32.powf(1.0 - t),
        _ => t,
    }
}

/// The 3D shape of spec 3.3: (volume, distance rate) at `distance` cm. Full volume to the inner
/// radius (400), linear to zero at the maximum distance (4000).
pub fn attenuation(distance: f32) -> (f32, f32) {
    let (inner, max) = (RULES.attenuation_inner_radius, RULES.attenuation_max_distance);
    let t = if distance <= inner { 1.0 } else { ((max - distance) / (max - inner)).max(0.0) };
    // A9: curve 0.
    (curve(0, t), 1.0 - t)
}

/// The listener: audio-device listener 0, ASSUMED to be the camera (spec open question 9).
/// Core axes, cm; `forward` and `right` are unit vectors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Listener {
    pub position: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
}

impl Default for Listener {
    fn default() -> Listener {
        Listener { position: Vec3::ZERO, forward: Vec3::X, right: Vec3::Y }
    }
}

/// The pan of a 3D sound at `point` (core cm) for this listener (spec 3.3): the direction in the
/// listener's horizontal plane as angle / pi: 0 ahead, 0.5 right, -0.5 left, +-1 behind (the
/// driver's own formula, public so that a host can check its listener against a known point).
pub fn pan_of(listener: &Listener, point: Vec3) -> f32 {
    let rel = point - listener.position;
    let (x, z) = (rel.dot(listener.right), rel.dot(listener.forward));
    if x.abs() < 1.0e-3 && z.abs() < 1.0e-3 {
        0.0
    } else {
        x.atan2(z) / std::f32::consts::PI
    }
}

/// How a sound is started (the UE plugin's side, spec 3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayParams {
    /// The volume handed to SEAD: trigger multiplier x asset `Volume` (spec 3.2).
    pub volume: f32,
    pub pitch: f32,
    /// `Some`: a 3D sound at this point (cm); `None`: 2D.
    pub position: Option<Vec3>,
    pub fade_in: f32,
}

impl Default for PlayParams {
    fn default() -> PlayParams {
        PlayParams { volume: 1.0, pitch: 1.0, position: None, fade_in: 0.0 }
    }
}

/// What `play` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayOutcome {
    Started(SoundKey),
    /// The start macro cancelled it.
    Cancelled { by_macro: u32 },
    /// Its category pool was full of sounds of higher priority.
    Dropped,
    /// No such sound in the pack.
    Missing,
}

impl PlayOutcome {
    pub fn key(self) -> Option<SoundKey> {
        match self {
            PlayOutcome::Started(key) => Some(key),
            _ => None,
        }
    }
}

/// What a voice sounds like now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceParams {
    /// Linear gain, 0..1 (spec 2.4).
    pub volume: f32,
    /// Frequency ratio, 1/512..2.
    pub pitch: f32,
    /// Direction in the listener's horizontal plane as angle / pi: 0 ahead, 0.5 right, -0.5
    /// left, +-1 behind.
    pub pan: f32,
    /// 0 = centred, 1 = fully panned.
    pub pan_depth: f32,
    /// Low-pass factor: 1.0 open ... 0.3 at the maximum distance. The factor -> cut-off mapping
    /// is not known (spec open question 3).
    pub lowpass: f32,
    /// Linear send to the reverb bus ("Revibe").
    pub reverb_send: f32,
}

/// One instruction for the backend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VoiceCommand {
    /// Start `material` of `bank` (`Pack::material_path`), `offset` source frames in. `looped`:
    /// play `[0, loop end)` once, then `[loop start, loop end)` until stopped (the loop points
    /// are the material's; spec 2.5). A voice that is not looped ends by itself at the end of
    /// its material: no `Stop` follows.
    Start { voice: VoiceId, sound: SoundKey, bank: u32, material: u32, offset: f64, looped: bool, params: VoiceParams },
    Update { voice: VoiceId, params: VoiceParams },
    Pause { voice: VoiceId, paused: bool },
    Stop { voice: VoiceId },
}

/// What happened, for a host's log.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Note {
    Play { key: SoundKey, bank: u32, index: u32 },
    Cancel { bank: u32, index: u32, by_macro: u32 },
    Drop { bank: u32, index: u32 },
    /// The pool was full: `victim` was stopped at once for `by`.
    Steal { victim: SoundKey, bank: u32, index: u32 },
    /// A start macro stopped `key`.
    MacroStop { key: SoundKey, by_macro: u32, fade: f32 },
    /// A voice started.
    Voice { key: SoundKey, voice: VoiceId, bank: u32, index: u32, material: u32, params: VoiceParams, distance: Option<f32> },
    /// The sound is over (played out, stopped, or stolen).
    End { key: SoundKey },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Fade {
    from: f32,
    to: f32,
    t: f32,
    duration: f32,
    curve: u8,
}

impl Fade {
    const fn at(value: f32) -> Fade {
        Fade { from: value, to: value, t: 0.0, duration: 0.0, curve: 0 }
    }

    fn value(&self) -> f32 {
        if self.duration <= 0.0 || self.t >= self.duration {
            self.to
        } else {
            self.from + (self.to - self.from) * curve(self.curve, self.t / self.duration)
        }
    }

    fn go(&mut self, to: f32, duration: f32, curve: u8) {
        *self = Fade { from: self.value(), to, t: 0.0, duration: duration.max(0.0), curve };
    }

    fn advance(&mut self, dt: f32) {
        if self.t < self.duration {
            self.t += dt;
        }
    }

    fn done(&self) -> bool {
        self.duration <= 0.0 || self.t >= self.duration
    }
}

/// Category layers: debug, game, config, menu, event, slow, matinee (+1 spare; the native
/// object has 8 fades).
pub const LAYERS: usize = 8;

#[derive(Clone, Copy, Debug)]
struct CategoryState {
    layers: [Fade; LAYERS],
    pitch: Fade,
    pause: Fade,
    paused: bool,
}

impl Default for CategoryState {
    fn default() -> CategoryState {
        CategoryState { layers: [Fade::at(1.0); LAYERS], pitch: Fade::at(1.0), pause: Fade::at(1.0), paused: false }
    }
}

impl CategoryState {
    fn volume(&self) -> f32 {
        self.layers.iter().map(Fade::value).product()
    }

    /// A7.
    fn frozen(&self) -> bool {
        self.paused && self.pause.done()
    }
}

#[derive(Clone, Debug)]
struct TrackRun {
    voice: VoiceId,
    track: u32,
    material: u32,
    /// Position in source frames.
    position: f64,
    length: f64,
    sample_rate: f64,
    looped: Option<(f64, f64)>,
    pc: usize,
    wait: f32,
    time: f32,
    /// What the first tick advances by instead of the step (the key-on's overshoot).
    fresh: Option<f32>,
    set: [Fade; 4],
    random: [(u8, f32); 4],
    lfo: [Option<(f32, f32, u8)>; 4],
    ended: bool,
    sent: Option<VoiceParams>,
    paused: bool,
}

#[derive(Clone, Debug)]
struct SeqRun {
    seq: usize,
    /// Index of its group in the selection's `choose` list (Cycle bookkeeping).
    group: Option<usize>,
    pc: usize,
    wait: f32,
    loops: Vec<(usize, i32)>,
    track: Option<TrackRun>,
    reached_end: bool,
}

impl SeqRun {
    fn finished(&self) -> bool {
        self.reached_end && self.track.as_ref().map_or(true, |t| t.ended)
    }
}

#[derive(Clone, Debug)]
struct Instance {
    key: SoundKey,
    bank: u32,
    index: u32,
    bank_number: i32,
    number: u32,
    category: u32,
    pool: u32,
    priority: i32,
    macro_start: u32,
    age: f32,
    started: bool,
    user_volume: Fade,
    user_pitch: f32,
    stop: Fade,
    stopping: bool,
    macro_fade: Fade,
    position: Option<Vec3>,
    distance: f32,
    rate: f32,
    volume_3d: f32,
    pan_3d: f32,
    seqs: Vec<SeqRun>,
    /// Cycle: seconds to the next pick, and the last group picked.
    cycle: Option<(f32, Option<usize>)>,
    dead: bool,
}

/// A live sound, for a debug page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActiveSound {
    pub key: SoundKey,
    pub bank: u32,
    pub index: u32,
    pub category: u32,
    pub priority: i32,
    pub age: f32,
    pub voices: usize,
    /// Of its first voice (zero volume when it has none).
    pub params: VoiceParams,
    /// The material of its first voice.
    pub material: Option<u32>,
    pub distance: Option<f32>,
    /// (ER62) Where a 3D sound is (core cm).
    pub position: Option<Vec3>,
    pub stopping: bool,
}

/// The driver.
#[derive(Clone, Debug)]
pub struct Driver {
    pack: Arc<Pack>,
    rng: CrtRand,
    instances: Vec<Instance>,
    categories: HashMap<u32, CategoryState>,
    /// Order sounds: the cursor byte of the loaded bank's sound (shared by every instance).
    order_cursor: HashMap<(u32, u32), u32>,
    /// Random sounds: the previous pick.
    last_random: HashMap<(u32, u32), usize>,
    next_key: SoundKey,
    next_voice: VoiceId,
    out: Vec<VoiceCommand>,
    notes: Vec<Note>,
}

const SILENT: VoiceParams = VoiceParams { volume: 0.0, pitch: 1.0, pan: 0.0, pan_depth: 1.0, lowpass: 1.0, reverb_send: 0.0 };

fn differs(a: &VoiceParams, b: &VoiceParams) -> bool {
    const E: f32 = 1.0e-4;
    (a.volume - b.volume).abs() > E || (a.pitch - b.pitch).abs() > E || (a.pan - b.pan).abs() > E || (a.pan_depth - b.pan_depth).abs() > E || (a.lowpass - b.lowpass).abs() > E || (a.reverb_send - b.reverb_send).abs() > E
}

impl Driver {
    /// `seed`: the host's seed of the selection generator (the game's is the clock).
    pub fn new(pack: Arc<Pack>, seed: u32) -> Driver {
        Driver { pack, rng: CrtRand::new(seed), instances: Vec::new(), categories: HashMap::new(), order_cursor: HashMap::new(), last_random: HashMap::new(), next_key: 1, next_voice: 1, out: Vec::new(), notes: Vec::new() }
    }

    pub fn pack(&self) -> &Arc<Pack> {
        &self.pack
    }

    /// Starts sound `index` of `bank` (spec 2.1): the start macro, then the category pool. The
    /// sound's sequences begin at the next `update` (A1).
    pub fn play(&mut self, bank: u32, index: u32, params: &PlayParams) -> PlayOutcome {
        let pack = self.pack.clone();
        let Some(def) = pack.sound(bank, index) else { return PlayOutcome::Missing };
        let bank_number = pack.bank(bank).number;
        let category = pack.categories.get(&def.category);
        let (pool, pool_size) = category.map_or((def.category, 0), |c| (c.pool, c.pool_size));
        let mut macro_fade = Fade::at(1.0);

        // 1. The start macro (interpreter 0x145976BA0).
        if def.macro_start != 0 {
            if let Some(commands) = pack.macros.get(&def.macro_start) {
                // A2.
                let all = |instances: &[Instance]| -> Vec<usize> { (0..instances.len()).filter(|&i| !instances[i].dead && !instances[i].stopping).collect() };
                let mut set = all(&self.instances);
                let id = |bank_number: i32, number: u32| bank_number as i64 * 65_536 + number as i64;
                for command in commands {
                    match *command {
                        MacroCmd::End => break,
                        MacroCmd::ResetSet => set = all(&self.instances),
                        MacroCmd::Filter { field, cmp, relative, value } => set.retain(|&i| {
                            let other = &self.instances[i];
                            let (mine, theirs) = match field {
                                MacroField::Category => (def.category as i64, other.category as i64),
                                // The same sound = bank number and sound number (spec 2.1).
                                MacroField::SoundId => (id(bank_number, def.number), id(other.bank_number, other.number)),
                                MacroField::Priority => (def.priority as i64, other.priority as i64),
                                MacroField::Macro => (def.macro_start as i64, other.macro_start as i64),
                                MacroField::BankNumber => (bank_number as i64, other.bank_number as i64),
                            };
                            if relative {
                                cmp.test((mine - theirs) as f64, value as f64)
                            } else {
                                cmp.test(theirs as f64, value as f64)
                            }
                        }),
                        // Seconds since the other sound started [inf, spec 2.1].
                        MacroCmd::FilterElapsed { cmp, value } => set.retain(|&i| cmp.test(self.instances[i].age as f64, value as f64)),
                        MacroCmd::FilterUnread | MacroCmd::ActionUnread => {}
                        MacroCmd::StopOthers { count, fade } => {
                            if count.0.test(set.len() as f64, count.1 as f64) {
                                for &i in &set {
                                    let key = self.instances[i].key;
                                    self.notes.push(Note::MacroStop { key, by_macro: def.macro_start, fade });
                                    self.stop_at(i, fade);
                                }
                                set.retain(|&i| !self.instances[i].dead && !self.instances[i].stopping);
                            }
                        }
                        MacroCmd::CancelSelf { count } => {
                            if count.0.test(set.len() as f64, count.1 as f64) {
                                self.notes.push(Note::Cancel { bank, index, by_macro: def.macro_start });
                                self.sweep();
                                return PlayOutcome::Cancelled { by_macro: def.macro_start };
                            }
                        }
                        MacroCmd::SelfVolume { count, volume, fade } => {
                            if count.0.test(set.len() as f64, count.1 as f64) {
                                macro_fade.go(volume, fade, 0);
                            }
                        }
                    }
                }
                self.sweep();
            }
        }

        // 2. The category pool (0x14596E140): full -> the lowest priority, oldest on ties, is
        // stopped at once unless the new sound's priority is lower, which is then dropped.
        if pool_size > 0 {
            let members: Vec<usize> = (0..self.instances.len()).filter(|&i| !self.instances[i].dead && self.instances[i].pool == pool).collect();
            if members.len() >= pool_size as usize {
                let victim = members.into_iter().min_by_key(|&i| (self.instances[i].priority, self.instances[i].key)).expect("a full pool has a member");
                if def.priority < self.instances[victim].priority {
                    self.notes.push(Note::Drop { bank, index });
                    return PlayOutcome::Dropped;
                }
                let v = &self.instances[victim];
                self.notes.push(Note::Steal { victim: v.key, bank: v.bank, index: v.index });
                self.stop_at(victim, 0.0);
                self.sweep();
            }
        }

        let key = self.next_key;
        self.next_key += 1;
        let mut user_volume = Fade::at(params.volume);
        if params.fade_in > 0.0 {
            user_volume = Fade { from: 0.0, to: params.volume, t: 0.0, duration: params.fade_in, curve: 0 };
        }
        self.instances.push(Instance {
            key,
            bank,
            index,
            bank_number,
            number: def.number,
            category: def.category,
            pool,
            priority: def.priority,
            macro_start: def.macro_start,
            age: 0.0,
            started: false,
            user_volume,
            user_pitch: params.pitch,
            stop: Fade::at(1.0),
            stopping: false,
            macro_fade,
            position: params.position,
            distance: 0.0,
            rate: 0.0,
            volume_3d: 1.0,
            pan_3d: 0.0,
            seqs: Vec::new(),
            cycle: None,
            dead: false,
        });
        self.notes.push(Note::Play { key, bank, index });
        PlayOutcome::Started(key)
    }

    /// Stops a sound: at once (`fade` 0) or by fading its stop slot to zero (spec 2.5; a sound
    /// with flags2 bit 0 and no fade given uses its own release time).
    pub fn stop(&mut self, key: SoundKey, fade: Option<f32>) {
        let Some(i) = self.instances.iter().position(|s| s.key == key && !s.dead) else { return };
        let def = self.pack.sound(self.instances[i].bank, self.instances[i].index);
        let fade = fade.unwrap_or_else(|| def.filter(|d| d.flags2 & 1 != 0).map_or(0.0, |d| d.release_seconds));
        self.stop_at(i, fade);
        self.sweep();
    }

    fn stop_at(&mut self, i: usize, fade: f32) {
        let instance = &mut self.instances[i];
        if fade > 0.0 {
            if !instance.stopping {
                instance.stopping = true;
                instance.stop.go(0.0, fade, 0);
            }
            return;
        }
        for seq in &mut instance.seqs {
            if let Some(track) = seq.track.take() {
                if !track.ended && track.sent.is_some() {
                    self.out.push(VoiceCommand::Stop { voice: track.voice });
                }
            }
        }
        instance.dead = true;
        self.notes.push(Note::End { key: instance.key });
    }

    fn sweep(&mut self) {
        self.instances.retain(|s| !s.dead);
    }

    pub fn is_playing(&self, key: SoundKey) -> bool {
        self.instances.iter().any(|s| s.key == key && !s.dead)
    }

    /// Moves a 3D sound (one attached to a bone follows it: the host calls this every step).
    pub fn set_position(&mut self, key: SoundKey, position: Vec3) {
        if let Some(s) = self.instances.iter_mut().find(|s| s.key == key) {
            if s.position.is_some() {
                s.position = Some(position);
            }
        }
    }

    pub fn set_volume(&mut self, key: SoundKey, volume: f32, fade: f32) {
        if let Some(s) = self.instances.iter_mut().find(|s| s.key == key) {
            s.user_volume.go(volume, fade, 0);
        }
    }

    pub fn set_pitch(&mut self, key: SoundKey, pitch: f32) {
        if let Some(s) = self.instances.iter_mut().find(|s| s.key == key) {
            s.user_pitch = pitch;
        }
    }

    fn category(&mut self, number: u32) -> &mut CategoryState {
        self.categories.entry(number).or_default()
    }

    /// A category's volume on one layer (the options sliders use the config layer 2 with a 0.5 s
    /// fade, spec 3.4).
    pub fn set_category_volume(&mut self, category: u32, layer: usize, volume: f32, fade: f32) {
        self.category(category).layers[layer.min(LAYERS - 1)].go(volume, fade, 0);
    }

    /// (volume, pitch) of a category now.
    pub fn category_state(&self, category: u32) -> (f32, f32) {
        self.categories.get(&category).map_or((1.0, 1.0), |c| (c.volume() * c.pause.value(), c.pitch.value()))
    }

    /// Sends a category event by name (`STATE_SLOW_START`, `STATE_MENU_OPEN`,
    /// `CMD_STOP_RT_SOUNDS` ...: spec 3.4; the commands are the SEAD configuration's). False:
    /// no such event.
    pub fn event(&mut self, name: &str) -> bool {
        let pack = self.pack.clone();
        let Some(commands) = pack.events.get(name) else { return false };
        for command in commands {
            match *command {
                EventCmd::SetVolume { category, layer, volume, fade } => self.set_category_volume(category, layer, volume, fade),
                EventCmd::SetPitch { category, pitch, fade } => self.category(category).pitch.go(pitch, fade, 0),
                EventCmd::Pause { category, fade } => {
                    let c = self.category(category);
                    c.paused = true;
                    c.pause.go(0.0, fade, 0);
                }
                EventCmd::Resume { category, fade } => {
                    let c = self.category(category);
                    c.paused = false;
                    c.pause.go(1.0, fade, 0);
                }
                EventCmd::Stop { category, fade } => {
                    for i in 0..self.instances.len() {
                        if self.instances[i].category == category && !self.instances[i].dead {
                            self.stop_at(i, fade);
                        }
                    }
                }
            }
        }
        self.sweep();
        true
    }

    /// The live sounds, oldest first.
    pub fn active(&self) -> Vec<ActiveSound> {
        self.instances
            .iter()
            .filter(|s| !s.dead)
            .map(|s| {
                let voices: Vec<&TrackRun> = s.seqs.iter().filter_map(|q| q.track.as_ref()).filter(|t| !t.ended).collect();
                ActiveSound {
                    key: s.key,
                    bank: s.bank,
                    index: s.index,
                    category: s.category,
                    priority: s.priority,
                    age: s.age,
                    voices: voices.len(),
                    params: voices.first().and_then(|t| t.sent).unwrap_or(SILENT),
                    material: voices.first().map(|t| t.material),
                    distance: s.position.map(|_| s.distance),
                    position: s.position,
                    stopping: s.stopping,
                }
            })
            .collect()
    }

    pub fn live_count(&self) -> usize {
        self.instances.len()
    }

    /// What happened since the last call.
    pub fn take_notes(&mut self) -> Vec<Note> {
        std::mem::take(&mut self.notes)
    }

    fn start_group(seqs: &mut Vec<SeqRun>, members: &[usize], group: Option<usize>) {
        for &seq in members {
            seqs.push(SeqRun { seq, group, pc: 0, wait: 0.0, loops: Vec::new(), track: None, reached_end: false });
        }
    }

    /// `r = rand() % 101`; the first weighted group with `r <=` the running weight sum.
    fn roll(rng: &mut CrtRand, weights: impl Iterator<Item = (usize, u32)>) -> Option<usize> {
        let r = rng.rand() % RULES.select_random_roll_modulus as u32;
        let mut sum = 0;
        for (i, weight) in weights {
            sum += weight;
            if r <= sum {
                return Some(i);
            }
        }
        None
    }

    /// Which sequences start (spec 2.2).
    fn select(&mut self, def: &SoundDef, bank: u32, index: u32) -> (Vec<SeqRun>, Option<(f32, Option<usize>)>) {
        let mut seqs = Vec::new();
        let mut cycle = None;
        match &def.selection {
            Selection::All | Selection::Switch => Self::start_group(&mut seqs, &(0..def.sequences.len()).collect::<Vec<_>>(), None),
            Selection::Random { always, choose, no_repeat } => {
                for g in always {
                    Self::start_group(&mut seqs, &g.members, None);
                }
                if let Some(mut pick) = Self::roll(&mut self.rng, choose.iter().enumerate().map(|(i, g)| (i, g.weight))) {
                    // The previous pick again, flag 0x20 clear: the heaviest other group
                    // (I 0x145983E10).
                    if *no_repeat && self.last_random.get(&(bank, index)) == Some(&pick) {
                        let mut best: Option<usize> = None;
                        for (i, g) in choose.iter().enumerate() {
                            if i != pick && best.map_or(true, |b| g.weight > choose[b].weight) {
                                best = Some(i);
                            }
                        }
                        pick = best.unwrap_or(pick);
                    }
                    self.last_random.insert((bank, index), pick);
                    Self::start_group(&mut seqs, &choose[pick].members, Some(pick));
                }
            }
            Selection::Order { always, rotate } => {
                for g in always {
                    Self::start_group(&mut seqs, &g.members, None);
                }
                if !rotate.is_empty() {
                    let cursor = self.order_cursor.entry((bank, index)).or_insert(RULES.select_order_cursor_start as u32);
                    let pick = (*cursor as usize) % rotate.len();
                    *cursor = ((pick + 1) % rotate.len()) as u32;
                    Self::start_group(&mut seqs, &rotate[pick].members, Some(pick));
                }
            }
            Selection::Cycle { always, choose, interval_ms, .. } => {
                for g in always {
                    Self::start_group(&mut seqs, &g.members, None);
                }
                let pick = Self::roll(&mut self.rng, choose.iter().enumerate().map(|(i, g)| (i, g.weight)));
                if let Some(pick) = pick {
                    Self::start_group(&mut seqs, &choose[pick].members, Some(pick));
                }
                cycle = Some((self.interval(*interval_ms), pick));
            }
        }
        (seqs, cycle)
    }

    fn interval(&mut self, (min, max): (u32, u32)) -> f32 {
        (min + if max > min { self.rng.rand() % (max - min + 1) } else { 0 }) as f32 * 0.001
    }

    /// One step: every live sound's fades, sequences and tracks; returns the voice commands.
    pub fn update(&mut self, dt: f32, listener: &Listener) -> Vec<VoiceCommand> {
        let pack = self.pack.clone();
        for c in self.categories.values_mut() {
            for layer in &mut c.layers {
                layer.advance(dt);
            }
            c.pitch.advance(dt);
            c.pause.advance(dt);
        }
        let mut instances = std::mem::take(&mut self.instances);
        for s in &mut instances {
            if s.dead {
                continue;
            }
            let Some(def) = pack.sound(s.bank, s.index) else {
                s.dead = true;
                continue;
            };
            let category = self.categories.get(&s.category).copied().unwrap_or_default();
            let frozen = category.frozen();

            // 3D (spec 3.3).
            if let Some(position) = s.position {
                let rel = position - listener.position;
                s.distance = rel.length();
                let (volume, rate) = attenuation(s.distance);
                s.volume_3d = volume;
                s.rate = rate;
                s.pan_3d = pan_of(listener, position);
            }

            // A1: the first update starts the sequences with no time elapsed.
            let step = if !s.started {
                s.started = true;
                let (seqs, cycle) = self.select(def, s.bank, s.index);
                s.seqs = seqs;
                s.cycle = cycle;
                0.0
            } else if frozen {
                0.0
            } else {
                dt
            };
            if !frozen {
                s.age += step;
                s.user_volume.advance(step);
                s.macro_fade.advance(step);
                s.stop.advance(step);
            }
            if s.stopping && s.stop.done() {
                for seq in &mut s.seqs {
                    if let Some(track) = seq.track.take() {
                        if !track.ended && track.sent.is_some() {
                            self.out.push(VoiceCommand::Stop { voice: track.voice });
                        }
                    }
                }
                s.dead = true;
                self.notes.push(Note::End { key: s.key });
                continue;
            }

            let sound_pitch = s.user_pitch * category.pitch.value();
            let clock = step * sound_pitch;

            // Cycle: a further pick whenever the interval runs out while fewer than `max_active`
            // groups are active (A8).
            if let (Some((timer, last)), Selection::Cycle { choose, interval_ms, max_active, .. }) = (s.cycle, &def.selection) {
                let mut timer = timer - step;
                let mut last = last;
                if timer <= 0.0 && !frozen && step > 0.0 && !s.stopping {
                    let active: Vec<usize> = s.seqs.iter().filter(|q| !q.finished()).filter_map(|q| q.group).collect();
                    let mut distinct = active.clone();
                    distinct.sort_unstable();
                    distinct.dedup();
                    if (distinct.len() as u32) < *max_active {
                        let eligible = choose.iter().enumerate().filter(|(i, _)| !active.contains(i) && Some(*i) != last).map(|(i, g)| (i, g.weight));
                        if let Some(pick) = Self::roll(&mut self.rng, eligible) {
                            Self::start_group(&mut s.seqs, &choose[pick].members, Some(pick));
                            last = Some(pick);
                        }
                    }
                    timer = self.interval(*interval_ms);
                }
                s.cycle = Some((timer, last));
            }

            let base_volume = def.volume * s.user_volume.value() * s.stop.value() * category.pause.value() * s.macro_fade.value() * s.volume_3d * category.volume();
            for seq in &mut s.seqs {
                let Some(seq_def) = def.sequences.get(seq.seq) else {
                    seq.reached_end = true;
                    continue;
                };
                // The sequence's commands, on a clock that advances by pitch x dt (spec 2.3).
                let mut remaining = clock;
                let mut guard = 0;
                while !seq.reached_end && !frozen {
                    guard += 1;
                    if guard > 4096 {
                        break;
                    }
                    if seq.wait > 0.0 {
                        if seq.wait > remaining {
                            seq.wait -= remaining;
                            break;
                        }
                        remaining -= seq.wait;
                        seq.wait = 0.0;
                    }
                    let Some(command) = seq_def.commands.get(seq.pc) else {
                        seq.reached_end = true;
                        break;
                    };
                    seq.pc += 1;
                    match *command {
                        SeqCmd::KeyOn { track, looped } => {
                            // One track at a time per sequence: the previous one is released (A4).
                            if let Some(old) = seq.track.take() {
                                if !old.ended && old.sent.is_some() {
                                    self.out.push(VoiceCommand::Stop { voice: old.voice });
                                }
                            }
                            let Some(track_def) = def.tracks.get(&track) else { continue };
                            let Some(material) = pack.bank(s.bank).materials.get(&track_def.material) else { continue };
                            let voice = self.next_voice;
                            self.next_voice += 1;
                            seq.track = Some(TrackRun {
                                voice,
                                track,
                                material: track_def.material,
                                position: 0.0,
                                length: material.samples as f64,
                                sample_rate: material.sample_rate as f64,
                                looped: if looped { material.loop_points.map(|(a, b)| (a as f64, b as f64)).filter(|(a, b)| b > a) } else { None },
                                pc: 0,
                                wait: 0.0,
                                time: 0.0,
                                // The time overshoot is the start offset into the stream.
                                fresh: Some(if sound_pitch > 0.0 { remaining / sound_pitch } else { 0.0 }),
                                set: [Fade::at(1.0), Fade::at(1.0), Fade::at(0.0), Fade::at(1.0)],
                                random: [(0, 0.0); 4],
                                lfo: [None; 4],
                                ended: false,
                                sent: None,
                                paused: false,
                            });
                        }
                        SeqCmd::Wait { ms } => seq.wait = ms as f32 * 0.001,
                        // `min + rand() % (max - min + 1)` ms [I].
                        SeqCmd::WaitRandom { min_ms, max_ms } => seq.wait = (min_ms + if max_ms > min_ms { self.rng.rand() % (max_ms - min_ms + 1) } else { 0 }) as f32 * 0.001,
                        SeqCmd::KeyOff => {
                            // A4.
                            if let Some(old) = seq.track.take() {
                                if !old.ended && old.sent.is_some() {
                                    self.out.push(VoiceCommand::Stop { voice: old.voice });
                                }
                            }
                        }
                        SeqCmd::LoopStart { min, max } => {
                            let count = if min == 0 && max == 0 { -1 } else { (min + if max > min { self.rng.rand() % (max - min + 1) } else { 0 }) as i32 };
                            seq.loops.push((seq.pc, count));
                        }
                        SeqCmd::LoopEnd => match seq.loops.last_mut() {
                            Some((start, count)) if *count < 0 || *count > 1 => {
                                if *count > 1 {
                                    *count -= 1;
                                }
                                seq.pc = *start;
                                // A loop body without a wait must not spin within one update.
                                if remaining <= 0.0 {
                                    break;
                                }
                                remaining = 0.0;
                            }
                            Some(_) => {
                                seq.loops.pop();
                            }
                            None => {}
                        },
                        SeqCmd::End => {
                            seq.pc -= 1;
                            seq.reached_end = true;
                        }
                    }
                }

                // The track: its own commands, then the voice.
                let Some(track) = &mut seq.track else { continue };
                if track.ended {
                    continue;
                }
                if frozen {
                    if !track.paused && track.sent.is_some() {
                        track.paused = true;
                        self.out.push(VoiceCommand::Pause { voice: track.voice, paused: true });
                    }
                    continue;
                }
                if track.paused {
                    track.paused = false;
                    self.out.push(VoiceCommand::Pause { voice: track.voice, paused: false });
                }
                let real = track.fresh.take().unwrap_or(step);
                // A3.
                let track_clock = real * sound_pitch;
                if let Some(track_def) = def.tracks.get(&track.track) {
                    let mut remaining = track_clock;
                    let mut guard = 0;
                    loop {
                        guard += 1;
                        if guard > 4096 {
                            break;
                        }
                        if track.wait > 0.0 {
                            if track.wait > remaining {
                                track.wait -= remaining;
                                break;
                            }
                            remaining -= track.wait;
                            track.wait = 0.0;
                        }
                        let Some(command) = track_def.commands.get(track.pc) else { break };
                        match *command {
                            TrackCmd::End => break,
                            TrackCmd::Wait { ms } => track.wait = ms as f32 * 0.001,
                            TrackCmd::Set { param, target, ms, curve } => track.set[param as usize].go(target, ms as f32 * 0.001, curve),
                            // Drawn once: `min + (rand() % 1000) * 0.001 * (max - min)`.
                            TrackCmd::Random { param, min, max, mode } => track.random[param as usize] = (mode, min + (self.rng.rand() % 1000) as f32 * 0.001 * (max - min)),
                            TrackCmd::Lfo { param, period, depth, wave } => track.lfo[param as usize] = Some((period, depth, wave)),
                            TrackCmd::Other => {}
                        }
                        track.pc += 1;
                    }
                }
                for fade in &mut track.set {
                    fade.advance(track_clock);
                }
                track.time += track_clock;
                let value = |track: &TrackRun, param: Param| -> f32 {
                    let i = param as usize;
                    let mut v = track.set[i].value();
                    match track.random[i] {
                        (1, r) => v += r,
                        (2, r) => v *= r,
                        _ => {}
                    }
                    if let Some((period, depth, wave)) = track.lfo[i] {
                        let phase = if period > 0.0 { (track.time / period).fract() } else { 0.0 };
                        // A5.
                        let w = if wave == 0 { (phase * std::f32::consts::TAU).sin() } else { 1.0 - 4.0 * (phase - 0.5).abs() };
                        v *= 1.0 + depth * w;
                    }
                    v
                };
                let (lo, hi) = (RULES.voice_volume_clamp[0], RULES.voice_volume_clamp[1]);
                let (pitch_lo, pitch_hi) = (RULES.voice_pitch_clamp[0], RULES.voice_pitch_clamp[1]);
                let params = VoiceParams {
                    volume: (base_volume * value(track, Param::Volume)).clamp(lo, hi),
                    pitch: (sound_pitch * value(track, Param::Pitch)).clamp(pitch_lo, pitch_hi),
                    // A6.
                    pan: s.pan_3d + value(track, Param::Pan),
                    pan_depth: value(track, Param::PanDepth).clamp(0.0, 1.0),
                    lowpass: seq_def.lowpass.as_ref().map_or(1.0, |c| c.eval(s.rate)),
                    reverb_send: seq_def.aux_send_db.as_ref().map_or(def.aux_send_volume, |c| (c.eval(s.rate) * RULES.zeroone_aux_send_db_to_linear).exp() * def.aux_send_volume),
                };
                // Where the stream is at the end of this step.
                track.position += params.pitch as f64 * real as f64 * track.sample_rate;
                match track.looped {
                    Some((start, end)) => {
                        while track.position >= end {
                            track.position -= end - start;
                        }
                    }
                    None => {
                        if track.position >= track.length {
                            track.ended = true;
                        }
                    }
                }
                match track.sent {
                    None => {
                        if !track.ended {
                            self.out.push(VoiceCommand::Start { voice: track.voice, sound: s.key, bank: s.bank, material: track.material, offset: track.position, looped: track.looped.is_some(), params });
                        }
                        self.notes.push(Note::Voice { key: s.key, voice: track.voice, bank: s.bank, index: s.index, material: track.material, params, distance: s.position.map(|_| s.distance) });
                        track.sent = Some(params);
                    }
                    Some(before) => {
                        if !track.ended && differs(&before, &params) {
                            self.out.push(VoiceCommand::Update { voice: track.voice, params });
                            track.sent = Some(params);
                        }
                    }
                }
            }

            // One-shot: voice ends -> track ends -> the sequence ends once it has reached `end`
            // -> the sound ends (spec 2.5). A Cycle sound goes on until it is stopped.
            if s.cycle.is_none() && s.seqs.iter().all(SeqRun::finished) {
                s.dead = true;
                self.notes.push(Note::End { key: s.key });
            } else if s.cycle.is_some() {
                s.seqs.retain(|q| !q.finished());
            }
        }
        instances.retain(|s| !s.dead);
        // Sounds started from inside this update do not exist (nothing here calls `play`).
        self.instances = instances;
        std::mem::take(&mut self.out)
    }
}

#[allow(dead_code)]
fn _cmp_is_used(_: Cmp) {}
