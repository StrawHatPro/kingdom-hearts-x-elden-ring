//! A software mixer over the driver's voice commands: a backend any host can use. It renders
//! interleaved stereo float frames at `RATE`; a host hands them to its audio device, or writes
//! them to a file (the offline render of a scripted run is this same code).
//!
//! Per voice: gain, pitch by resampling (linear interpolation), pan, the loop with the
//! material's loop points (play `[0, end)` once, then `[start, end)`), pause, stop.
//!
//! WHAT IS THIS HOST'S CHOICE, NOT THE GAME'S (the spec leaves them open, question 3):
//! - the pan law: constant power on the lateral component `sin(pan x pi) x depth` (a mono voice
//!   dead ahead is at -3 dB in both channels); a stereo material keeps its channels and is
//!   balanced. The game's pan tables (0x147CA2D80) were not read. Sounds behind the listener
//!   are not distinguished from sounds ahead (two channels).
//! - the low-pass: one pole, cut-off = factor x 20 kHz. The game's factor -> cut-off mapping
//!   was not read; `set_lowpass(false)` passes the factor through unused.
//! - the reverb send is carried by the commands and NOT rendered (no reverb here).
//! - volume changes are ramped over a block and a stopped voice fades over 5 ms, against clicks; (ER-HITSFX-AUDIT) a
//!   paused voice fades out over the same 5 ms and back in on its resume.
//! - (THE USER 2026-11-02: "sometimes Audio from specifically our mod will become very loud") THE SUM GOES THROUGH A
//!   LIMITER after the master gain: a frame whose louder channel would pass `LIMIT` (-0.5 dBFS) is turned down to it at
//!   once (no overshoot, no clipping), and the gain comes back with a `LIMIT_RELEASE` time constant. Before, the sum was
//!   only clipped at full scale: the user's log has 625 clipped samples at a peak of 1.457 (the game-over loop raised
//!   +12 dB as a menu sound). The clamp at full scale stays behind it as the last resort.

use std::collections::HashMap;
use std::sync::Arc;

use crate::driver::{VoiceCommand, VoiceId, VoiceParams};
use crate::pack::Pack;
use crate::wav::Buffer;

/// Output sample rate (every material of the pack is 48 kHz).
pub const RATE: u32 = 48_000;
/// Frames of the declick fade of a stopped voice.
const DECLICK: f32 = 240.0;
/// (ER-HITSFX-AUDIT) Voices the mixer's list holds without reallocating (more than KH3's pools let live at once in a fight).
pub const VOICES_RESERVED: usize = 512;
/// Longest gain ramp, frames.
const RAMP: usize = 256;
/// (limiter) The output level no frame passes: -0.5 dBFS.
pub const LIMIT: f32 = 0.944;
/// (limiter) Seconds of the gain's way back to 1 after a reduction (time constant).
pub const LIMIT_RELEASE: f32 = 0.25;

/// The pack's materials, read on first use.
#[derive(Clone, Debug, Default)]
pub struct Materials {
    cache: HashMap<(u32, u32), Option<Arc<Buffer>>>,
    pub errors: Vec<String>,
}

impl Materials {
    /// (DON-KIT) Reads these materials now (`Engine::effect_materials`); how many are in the cache.
    pub fn preload(&mut self, pack: &Pack, list: &[(u32, u32)]) -> usize {
        list.iter().filter(|(bank, material)| self.get(pack, *bank, *material).is_some()).count()
    }

    /// (DON-KIT) Whether a material is read already.
    pub fn has(&self, bank: u32, material: u32) -> bool {
        self.cache.get(&(bank, material)).is_some_and(|b| b.is_some())
    }

    /// (ER-HITSFX-AUDIT) What the cache knows of a material without reading it: `None` = not read yet, `Some(None)` = it
    /// could not be read, `Some(Some(buffer))` = read.
    pub fn cached(&self, bank: u32, material: u32) -> Option<Option<Arc<Buffer>>> {
        self.cache.get(&(bank, material)).cloned()
    }

    /// (ER-HITSFX-AUDIT) Puts a material read elsewhere (a reader thread's [`Materials::get`]) into this cache.
    pub fn insert(&mut self, bank: u32, material: u32, buffer: Option<Arc<Buffer>>) {
        self.cache.insert((bank, material), buffer);
    }

    pub fn get(&mut self, pack: &Pack, bank: u32, material: u32) -> Option<Arc<Buffer>> {
        if let Some(found) = self.cache.get(&(bank, material)) {
            return found.clone();
        }
        let loaded = match pack.material_path(bank, material) {
            Some(path) => match crate::wav::read(&path) {
                Ok(mut buffer) => {
                    // The manifest's loop points are authoritative; the WAV carries the same.
                    if let Some(m) = pack.bank(bank).materials.get(&material) {
                        buffer.loop_points = m.loop_points.or(buffer.loop_points);
                    }
                    Some(Arc::new(buffer))
                }
                Err(error) => {
                    self.errors.push(error);
                    None
                }
            },
            None => None,
        };
        self.cache.insert((bank, material), loaded.clone());
        loaded
    }
}

#[derive(Clone, Debug)]
struct MixVoice {
    id: VoiceId,
    buffer: Arc<Buffer>,
    position: f64,
    looped: Option<(f64, f64)>,
    params: VoiceParams,
    /// Current per-channel gains (ramped toward the target).
    gain: [f32; 2],
    started: bool,
    lowpass_state: [f32; 2],
    paused: bool,
    /// (ER-HITSFX-AUDIT) The pause's own declick: 1 = playing, 0 = held; it moves by `1 / DECLICK` a frame toward the pause
    /// state, so a pause (a menu holding the world) and its resume fade over 5 ms instead of cutting mid-wave (a click).
    pause_gain: f32,
    /// `Some(frames left)` of the declick fade.
    stopping: Option<f32>,
    done: bool,
}

/// (left, right) gains of a voice (public for a host's check of its pan, ER62).
pub fn gains(params: &VoiceParams, stereo: bool) -> [f32; 2] {
    let lateral = ((params.pan * std::f32::consts::PI).sin() * params.pan_depth).clamp(-1.0, 1.0);
    if stereo {
        [params.volume * (1.0 - lateral.max(0.0)), params.volume * (1.0 + lateral.min(0.0))]
    } else {
        let angle = (lateral + 1.0) * std::f32::consts::FRAC_PI_4;
        [params.volume * angle.cos(), params.volume * angle.sin()]
    }
}

/// The mixer.
#[derive(Clone, Debug)]
pub struct Mixer {
    voices: Vec<MixVoice>,
    lowpass: bool,
    /// Linear gain of the sum.
    pub master: f32,
    /// Largest absolute sample of the sum after the master gain and before the limiter, since the start.
    pub peak: f32,
    /// Output samples that had to be clipped (the limiter leaves none: the last resort).
    pub clipped: u64,
    pub frames_rendered: u64,
    /// (limiter) Its gain now (1 = nothing taken off), the frames it turned down since the start, and the last
    /// rendered block's peak before it and smallest gain (a host's per-second line).
    pub limit_gain: f32,
    pub limited: u64,
    pub block_peak: f32,
    pub block_gain: f32,
}

impl Default for Mixer {
    fn default() -> Mixer {
        Mixer::new()
    }
}

impl Mixer {
    pub fn new() -> Mixer {
        // (ER-HITSFX-AUDIT) room for VOICES_RESERVED voices from the start: the device callback starts and renders the
        // voices, and a growing list would reallocate there.
        Mixer { voices: Vec::with_capacity(VOICES_RESERVED), lowpass: true, master: 1.0, peak: 0.0, clipped: 0, frames_rendered: 0, limit_gain: 1.0, limited: 0, block_peak: 0.0, block_gain: 1.0 }
    }

    pub fn set_lowpass(&mut self, on: bool) {
        self.lowpass = on;
    }

    pub fn voices(&self) -> usize {
        self.voices.len()
    }

    /// Starts a voice on a buffer (what `VoiceCommand::Start` does once its material is read;
    /// also the way a host plays a sound of its own).
    pub fn start(&mut self, voice: VoiceId, buffer: Arc<Buffer>, offset: f64, looped: bool, params: VoiceParams) {
        let loop_points = if looped { buffer.loop_points.map(|(a, b)| (a as f64, (b as f64).min(buffer.frames() as f64))).filter(|(a, b)| b > a).or(Some((0.0, buffer.frames() as f64))) } else { None };
        if buffer.frames() == 0 {
            return;
        }
        self.voices.push(MixVoice { id: voice, buffer, position: offset, looped: loop_points, params, gain: [0.0; 2], started: false, lowpass_state: [0.0; 2], paused: false, pause_gain: 1.0, stopping: None, done: false });
    }

    /// (THE USER 2026-11-02) The NEWEST voice of that id: after a host's restart (a new engine counts its voices from 1
    /// again) an old voice of the same id may still be in its declick fade; it must not take the new one's updates.
    pub fn update(&mut self, voice: VoiceId, params: VoiceParams) {
        if let Some(v) = self.voices.iter_mut().rev().find(|v| v.id == voice) {
            v.params = params;
        }
    }

    pub fn pause(&mut self, voice: VoiceId, paused: bool) {
        if let Some(v) = self.voices.iter_mut().rev().find(|v| v.id == voice) {
            v.paused = paused;
        }
    }

    pub fn stop(&mut self, voice: VoiceId) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == voice && v.stopping.is_none()) {
            v.stopping = Some(DECLICK);
        }
    }

    /// Every voice stops, with the declick fade (a host starting over).
    pub fn stop_all(&mut self) {
        for v in self.voices.iter_mut().filter(|v| v.stopping.is_none()) {
            v.stopping = Some(DECLICK);
        }
    }

    /// Applies one driver command; materials come from `materials`.
    pub fn apply(&mut self, command: &VoiceCommand, pack: &Pack, materials: &mut Materials) {
        match *command {
            VoiceCommand::Start { voice, bank, material, offset, looped, params, .. } => {
                if let Some(buffer) = materials.get(pack, bank, material) {
                    self.start(voice, buffer, offset, looped, params);
                }
            }
            VoiceCommand::Update { voice, params } => self.update(voice, params),
            VoiceCommand::Pause { voice, paused } => self.pause(voice, paused),
            VoiceCommand::Stop { voice } => self.stop(voice),
        }
    }

    /// Mixes the next `out.len() / 2` frames into `out` (interleaved stereo; overwritten).
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        let frames = out.len() / 2;
        for v in &mut self.voices {
            // (ER-HITSFX-AUDIT) a paused voice is skipped once its pause fade is down (a stopping one still fades out)
            if v.paused && v.stopping.is_none() && v.pause_gain <= 0.0 {
                continue;
            }
            let channels = v.buffer.channels as usize;
            let stereo = channels >= 2;
            let target = gains(&v.params, stereo);
            if !v.started {
                v.started = true;
                v.gain = target;
            }
            let ramp = frames.clamp(1, RAMP) as f32;
            let delta = [(target[0] - v.gain[0]) / ramp, (target[1] - v.gain[1]) / ramp];
            let step = v.params.pitch as f64 * v.buffer.sample_rate as f64 / RATE as f64;
            let total = v.buffer.frames() as f64;
            let data = &v.buffer.data;
            let alpha = if self.lowpass && v.params.lowpass < 0.999 { Some(1.0 - (-std::f32::consts::TAU * (20_000.0 * v.params.lowpass.max(0.01)) / RATE as f32).exp()) } else { None };
            for f in 0..frames {
                if let Some((start, end)) = v.looped {
                    while v.position >= end {
                        v.position -= end - start;
                    }
                } else if v.position >= total {
                    v.done = true;
                    break;
                }
                let i = v.position as usize;
                let t = (v.position - i as f64) as f32;
                // The frame after the last one: the loop start when looping, else silence.
                let j = match v.looped {
                    Some((start, end)) if (i + 1) as f64 >= end => start as usize,
                    _ => i + 1,
                };
                let sample = |frame: usize, channel: usize| -> f32 { data.get(frame * channels + channel.min(channels - 1)).map_or(0.0, |s| *s as f32 / 32768.0) };
                let mut x = [sample(i, 0) + (sample(j, 0) - sample(i, 0)) * t, 0.0];
                x[1] = if stereo { sample(i, 1) + (sample(j, 1) - sample(i, 1)) * t } else { x[0] };
                // (ER-HITSFX-AUDIT) THE ONE-POLE LOW-PASS: the state follows the input by `alpha` each frame and is the output.
                // Before, the filtered branch only copied the state out and never moved it: every voice whose distance curve
                // asked for a low-pass (3293 of the pack's 4099 sequences, beyond 4 m of the listener) played the frozen last
                // sample (silence from its start, a held DC step mid-sound): cut off, and choppy when it crossed 4 m.
                match alpha {
                    Some(alpha) => {
                        for (state, input) in v.lowpass_state.iter_mut().zip(x.iter_mut()) {
                            *state += alpha * (*input - *state);
                            *input = *state;
                        }
                    }
                    None => v.lowpass_state = x,
                }
                if (f as f32) < ramp {
                    v.gain[0] += delta[0];
                    v.gain[1] += delta[1];
                }
                let fade = match &mut v.stopping {
                    Some(left) => {
                        *left -= 1.0;
                        if *left <= 0.0 {
                            v.done = true;
                        }
                        (*left / DECLICK).max(0.0)
                    }
                    None => 1.0,
                };
                // (ER-HITSFX-AUDIT) the pause's declick: toward 0 while paused, back to 1 when resumed
                v.pause_gain = if v.paused { (v.pause_gain - 1.0 / DECLICK).max(0.0) } else { (v.pause_gain + 1.0 / DECLICK).min(1.0) };
                let fade = fade * v.pause_gain;
                out[f * 2] += x[0] * v.gain[0] * fade;
                out[f * 2 + 1] += x[1] * v.gain[1] * fade;
                if v.done {
                    break;
                }
                if v.paused && v.stopping.is_none() && v.pause_gain <= 0.0 {
                    // held where its fade ended: the resume goes on from here
                    break;
                }
                v.position += step;
            }
            v.gain = if v.done { v.gain } else { target };
        }
        self.voices.retain(|v| !v.done);
        // (THE USER 2026-11-02) the master gain, then the limiter (instant attack, LIMIT_RELEASE back), then the clamp
        let release = 1.0 - (-1.0 / (LIMIT_RELEASE * RATE as f32)).exp();
        let (mut block_peak, mut block_gain) = (0.0f32, 1.0f32);
        for frame in out.as_chunks_mut::<2>().0 {
            frame[0] *= self.master;
            frame[1] *= self.master;
            let a = frame[0].abs().max(frame[1].abs());
            block_peak = block_peak.max(a);
            let need = if a > LIMIT { LIMIT / a } else { 1.0 };
            self.limit_gain = (self.limit_gain + (1.0 - self.limit_gain) * release).min(need);
            if self.limit_gain < 0.999 {
                self.limited += 1;
                block_gain = block_gain.min(self.limit_gain);
                frame[0] *= self.limit_gain;
                frame[1] *= self.limit_gain;
            }
            for s in frame.iter_mut() {
                if s.abs() > 1.0 {
                    self.clipped += 1;
                    *s = s.clamp(-1.0, 1.0);
                }
            }
        }
        self.peak = self.peak.max(block_peak);
        (self.block_peak, self.block_gain) = (block_peak, block_gain);
        self.frames_rendered += frames as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLAT: VoiceParams = VoiceParams { volume: 1.0, pitch: 1.0, pan: 0.0, pan_depth: 1.0, lowpass: 1.0, reverb_send: 0.0 };

    fn ramp_buffer(frames: usize) -> Arc<Buffer> {
        Arc::new(Buffer { channels: 1, sample_rate: RATE, data: (0..frames).map(|i| (i * 10) as i16).collect(), loop_points: None })
    }

    #[test]
    fn a_one_shot_plays_its_samples_once_and_ends() {
        let mut m = Mixer::new();
        m.start(1, ramp_buffer(100), 0.0, false, FLAT);
        let mut out = vec![0.0; 2 * 160];
        m.render(&mut out);
        let centre = std::f32::consts::FRAC_PI_4.cos();
        assert!((out[2 * 50] - 500.0 / 32768.0 * centre).abs() < 1.0e-6 && (out[2 * 50 + 1] - out[2 * 50]).abs() < 1.0e-7);
        assert_eq!(out[2 * 120], 0.0);
        assert_eq!(m.voices(), 0);
    }

    #[test]
    fn pitch_resamples_and_pan_moves_the_gains() {
        let mut m = Mixer::new();
        m.start(1, ramp_buffer(1000), 0.0, false, VoiceParams { pitch: 2.0, pan: 0.5, ..FLAT });
        let mut out = vec![0.0; 2 * 100];
        m.render(&mut out);
        // Twice the rate: output frame 40 is source frame 80; fully right.
        assert!((out[2 * 40 + 1] - 800.0 / 32768.0).abs() < 1.0e-6 && out[2 * 40].abs() < 1.0e-6);
        let mut m2 = Mixer::new();
        m2.start(2, ramp_buffer(1000), 0.0, false, VoiceParams { pitch: 0.5, pan: -0.5, ..FLAT });
        m2.render(&mut out);
        // Half the rate: output frame 41 is source frame 20.5, interpolated; fully left.
        assert!((out[2 * 41] - 205.0 / 32768.0).abs() < 1.0e-6 && out[2 * 41 + 1].abs() < 1.0e-6);
    }

    #[test]
    fn a_loop_plays_the_head_once_and_then_the_loop_region() {
        let mut m = Mixer::new();
        let buffer = Arc::new(Buffer { channels: 1, sample_rate: RATE, data: (0..10).map(|i| i * 100).collect(), loop_points: Some((6, 10)) });
        m.start(1, buffer, 0.0, true, VoiceParams { pan: 0.5, ..FLAT });
        let mut out = vec![0.0; 2 * 18];
        m.render(&mut out);
        let got: Vec<i32> = (0..18).map(|f| (out[2 * f + 1] * 32768.0).round() as i32).collect();
        assert_eq!(got, [0, 100, 200, 300, 400, 500, 600, 700, 800, 900, 600, 700, 800, 900, 600, 700, 800, 900]);
        assert_eq!(m.voices(), 1);
        m.stop(1);
        let mut out = vec![0.0; 2 * 400];
        m.render(&mut out);
        assert_eq!(m.voices(), 0);
    }

    /// (THE USER 2026-11-02: "sometimes Audio from specifically our mod will become very loud") A sum over full scale is
    /// turned down to LIMIT by the limiter, never clipped; the peak before it is still counted.
    #[test]
    fn the_sum_is_limited_not_clipped() {
        let mut m = Mixer::new();
        let loud = Arc::new(Buffer { channels: 2, sample_rate: RATE, data: vec![30_000; 200], loop_points: None });
        m.start(1, loud.clone(), 0.0, false, FLAT);
        m.start(2, loud, 0.0, false, FLAT);
        let mut out = vec![0.0; 2 * 50];
        m.render(&mut out);
        assert!(m.peak > 1.8 && m.clipped == 0, "peak {} clipped {}", m.peak, m.clipped);
        assert!(out.iter().all(|s| s.abs() <= LIMIT + 1.0e-6), "nothing passes the limit");
        assert!((out[20] - LIMIT).abs() < 1.0e-3, "held at the limit: {}", out[20]);
        assert_eq!(m.limited, 50);
        assert!(m.block_peak > 1.8 && m.block_gain < 0.52, "{} {}", m.block_peak, m.block_gain);
    }

    /// (limiter) After a loud burst the gain comes back with LIMIT_RELEASE: a quiet sound right after it is turned down at
    /// first and back at its own level within a second; a mix under the limit is never touched.
    #[test]
    fn the_limiter_lets_go_and_leaves_a_quiet_mix_alone() {
        let mut m = Mixer::new();
        let loud = Arc::new(Buffer { channels: 2, sample_rate: RATE, data: vec![30_000; 2 * 480], loop_points: None });
        let quiet = Arc::new(Buffer { channels: 2, sample_rate: RATE, data: vec![8_000; 2 * 96_000], loop_points: None });
        m.start(1, loud.clone(), 0.0, false, FLAT);
        m.start(2, loud, 0.0, false, FLAT);
        let mut out = vec![0.0; 2 * 480];
        m.render(&mut out);
        m.start(3, quiet, 0.0, false, FLAT);
        let level = 8_000.0 / 32_768.0;
        m.render(&mut out);
        assert!(out[0] < level * 0.7, "just after the burst: turned down ({})", out[0]);
        let mut long = vec![0.0; 2 * 48_000];
        m.render(&mut long);
        assert!((long[2 * 47_999] / level - 1.0).abs() < 0.02, "a second later (4 time constants): its own level within 2 % ({})", long[2 * 47_999]);
        let mut calm = Mixer::new();
        calm.start(1, Arc::new(Buffer { channels: 2, sample_rate: RATE, data: vec![20_000; 400], loop_points: None }), 0.0, false, FLAT);
        let mut out = vec![0.0; 2 * 100];
        calm.render(&mut out);
        assert!(calm.limited == 0 && (out[10] - 20_000.0 / 32_768.0).abs() < 1.0e-6);
    }

    /// (THE USER 2026-11-02) A restarted host's new voice 1 takes the updates, not the old voice 1 in its declick fade.
    #[test]
    fn updates_go_to_the_newest_voice_of_an_id() {
        let mut m = Mixer::new();
        let buffer = Arc::new(Buffer { channels: 2, sample_rate: RATE, data: vec![10_000; 20_000], loop_points: None });
        m.start(1, buffer.clone(), 0.0, false, FLAT);
        m.stop_all();
        m.start(1, buffer, 0.0, false, FLAT);
        m.update(1, VoiceParams { volume: 0.25, ..FLAT });
        assert_eq!((m.voices[0].params.volume, m.voices[1].params.volume), (1.0, 0.25));
    }
}
