//! THE OUTPUT FOR A HOST THAT MUST NEVER BLOCK (a game's frame thread).
//!
//! ```text
//! simulation step (the host's thread)        "kh-audio" thread (ours)             the device's callback thread (cpal)
//!   Engine::update -> voice commands  --q1-->  reads the material of a Start  --q2-->  Mixer::apply + Mixer::render
//!   Output::send: a queue push, nothing else   (a WAV file, on first use), owns        256 frames a pull
//!                                              the device, reopens it
//! ```
//!
//! - `send` is one push on a lock-free queue (std's mpsc: a linked list of blocks): no file is
//!   read, no lock is waited for and no device call is made on the host's thread;
//! - the mixer itself runs on the device's own callback; nobody else touches it while a device
//!   is open (its mutex is only there to hand it from a dead device's callback to the next one);
//! - THE DEVICE IS THE DEFAULT OUTPUT DEVICE, shared mode, opened through rodio as the test app
//!   does. It is reopened when its stream reports an error (headphones unplugged), when it stops
//!   pulling, or when the system's default output device becomes another one. While there is no
//!   device the "kh-audio" thread advances the mixer itself in real time, so loops end and one-
//!   shots run out as they would have, and nothing piles up;
//! - offline (`Output::render`): the same commands go straight into a mixer of the caller's
//!   thread and `advance` appends what it renders to a WAV file.

use std::num::NonZero;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kh_audio::wav::Buffer;
use kh_audio::{Materials, Mixer, Pack, VoiceCommand, VoiceId, VoiceParams};

use super::device::WavOut;

/// What the host's thread says.
enum Feed {
    Voice(VoiceCommand),
    Master(f32),
    /// Every voice stops (Sora starts again, the payload goes away).
    StopAll,
    Quit,
    /// (DON-KIT) Read these materials ahead (the item effects' sounds).
    Preload(Vec<(u32, u32)>),
}

/// What reaches the mixer (a Start with its material already read).
enum Mix {
    Start { voice: VoiceId, buffer: Arc<Buffer>, offset: f64, looped: bool, params: VoiceParams, bank: u32, material: u32 },
    Command(VoiceCommand),
    Master(f32),
    StopAll,
}

/// Numbers of the output, readable from any thread.
#[derive(Default)]
pub struct Stats {
    /// Voices in the mixer now.
    pub voices: AtomicU32,
    /// Voices started since the start.
    pub started: AtomicU64,
    /// Largest absolute output sample (f32 bits) and how many samples had to be clipped.
    peak: AtomicU32,
    pub clipped: AtomicU64,
    /// Frames the device pulled / frames rendered without a device.
    pub frames: AtomicU64,
    pub frames_blind: AtomicU64,
    /// Times a device was opened / times opening failed.
    pub opens: AtomicU32,
    pub open_failures: AtomicU32,
    /// A device is open and pulling.
    pub device: AtomicBool,
    /// Materials that could not be read.
    pub missing: AtomicU32,
    /// (mixer trace) While set, every voice that starts IN THE MIXER leaves a line in `lines` a third of a second
    /// later: the frames the mixer rendered after its start and the peak of the output in them (and in the block
    /// before it): the proof, at the last place before the device, that a sound was played.
    pub trace: AtomicBool,
    pub lines: Mutex<Vec<String>>,
    /// (mixer trace) While `Some`, the mixer's output (stereo f32, 48 kHz) is appended here (at most 90 s).
    pub capture: Mutex<Option<Vec<f32>>>,
}

/// (mixer trace) A voice whose first third of a second is being measured.
struct Watched {
    what: String,
    at_frame: u64,
    before: f32,
    frames: u64,
    peak: f32,
}

/// (mixer trace) Frames a started voice is measured for.
const WATCH_FRAMES: u64 = 14_400;

impl Stats {
    pub fn peak(&self) -> f32 {
        f32::from_bits(self.peak.load(Ordering::Relaxed))
    }
}

struct MixState {
    rx: Receiver<Mix>,
    mixer: Mixer,
    /// (mixer trace) frames rendered so far, the peak of the last block, the voices being measured.
    rendered: u64,
    last_peak: f32,
    watched: Vec<Watched>,
}

impl MixState {
    /// Applies what has arrived and renders the next block.
    fn render(&mut self, block: &mut [f32], stats: &Stats) {
        loop {
            match self.rx.try_recv() {
                Ok(Mix::Start { voice, buffer, offset, looped, params, bank, material }) => {
                    if stats.trace.load(Ordering::Relaxed) && self.watched.len() < 64 {
                        self.watched.push(Watched { what: format!("voice {voice:?} bank {bank} material {material}{}", if looped { " (looped)" } else { "" }), at_frame: self.rendered, before: self.last_peak, frames: 0, peak: 0.0 });
                    }
                    self.mixer.start(voice, buffer, offset, looped, params);
                    stats.started.fetch_add(1, Ordering::Relaxed);
                }
                Ok(Mix::Command(VoiceCommand::Update { voice, params })) => self.mixer.update(voice, params),
                Ok(Mix::Command(VoiceCommand::Pause { voice, paused })) => self.mixer.pause(voice, paused),
                Ok(Mix::Command(VoiceCommand::Stop { voice })) => self.mixer.stop(voice),
                Ok(Mix::Command(VoiceCommand::Start { .. })) => {}
                Ok(Mix::Master(gain)) => self.mixer.master = gain,
                Ok(Mix::StopAll) => self.mixer.stop_all(),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        self.mixer.render(block);
        // (mixer trace) what came out
        let frames = block.len() as u64 / 2;
        self.rendered += frames;
        let peak = block.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        self.last_peak = peak;
        if !self.watched.is_empty() {
            for w in &mut self.watched {
                w.frames += frames;
                w.peak = w.peak.max(peak);
            }
            if self.watched.iter().any(|w| w.frames >= WATCH_FRAMES) {
                if let Ok(mut lines) = stats.lines.try_lock() {
                    for w in self.watched.iter().filter(|w| w.frames >= WATCH_FRAMES) {
                        if lines.len() < 256 {
                            lines.push(format!("{} STARTED IN THE MIXER at its frame {} ({:.2} s): {} frames rendered after it, output peak {:.4} (the block before it: {:.4}), master {:.3}", w.what, w.at_frame, w.at_frame as f64 / kh_audio::mixer::RATE as f64, w.frames, w.peak, w.before, self.mixer.master));
                        }
                    }
                    self.watched.retain(|w| w.frames < WATCH_FRAMES);
                }
            }
        }
        if let Ok(mut capture) = stats.capture.try_lock() {
            if let Some(samples) = capture.as_mut() {
                if samples.len() < 90 * 2 * kh_audio::mixer::RATE as usize {
                    samples.extend_from_slice(block);
                }
            }
        }
        stats.voices.store(self.mixer.voices() as u32, Ordering::Relaxed);
        stats.peak.store(self.mixer.peak.to_bits(), Ordering::Relaxed);
        stats.clipped.store(self.mixer.clipped, Ordering::Relaxed);
    }
}

/// The mixer as a rodio source that owns nothing but a handle: stereo, 48 kHz, endless.
struct Source {
    state: Arc<Mutex<MixState>>,
    stats: Arc<Stats>,
    block: Vec<f32>,
    at: usize,
}

impl Iterator for Source {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<rodio::Sample> {
        if self.at >= self.block.len() {
            // (never waits: the only other taker is the "kh-audio" thread while NO device pulls)
            match self.state.try_lock() {
                Ok(mut state) => state.render(&mut self.block, &self.stats),
                Err(_) => self.block.fill(0.0),
            }
            self.stats.frames.fetch_add(self.block.len() as u64 / 2, Ordering::Relaxed);
            self.at = 0;
        }
        let sample = self.block[self.at];
        self.at += 1;
        Some(sample as rodio::Sample)
    }
}

impl rodio::Source for Source {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        NonZero::new(2).expect("2")
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        NonZero::new(kh_audio::mixer::RATE).expect("rate")
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// The name of the system's default output device now (None: there is none).
fn default_device_name() -> Option<String> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};
    let device = rodio::cpal::default_host().default_output_device()?;
    #[allow(deprecated)]
    let name = device.name().ok();
    name
}

/// The "kh-audio" thread: materials, the device and its reopening.
fn feeder(rx: Receiver<Feed>, pack: Arc<Pack>, stats: Arc<Stats>, log: fn(&str), lowpass: bool) {
    let (mix_tx, mix_rx) = mpsc::channel::<Mix>();
    let mut mixer = Mixer::new();
    mixer.set_lowpass(lowpass);
    let state = Arc::new(Mutex::new(MixState { rx: mix_rx, mixer, rendered: 0, last_peak: 0.0, watched: Vec::new() }));
    let failed = Arc::new(AtomicBool::new(false));
    let mut materials = Materials::default();
    let mut device: Option<(rodio::MixerDeviceSink, Option<String>)> = None;
    let mut next_open = Instant::now();
    let mut checked = Instant::now();
    let mut pulled = (0u64, Instant::now());
    let mut blind = Instant::now();
    let mut scratch = vec![0.0f32; 512];
    let mut told_no_device = false;
    loop {
        // ---- the device: open, watch, reopen
        let now = Instant::now();
        if let Some((_, name)) = &device {
            let frames = stats.frames.load(Ordering::Relaxed);
            if frames != pulled.0 {
                pulled = (frames, now);
            }
            let mut why = None;
            if failed.swap(false, Ordering::SeqCst) {
                why = Some("its stream reported an error (unplugged?)".to_string());
            } else if now.duration_since(pulled.1) > Duration::from_millis(1500) {
                why = Some("it stopped pulling".to_string());
            } else if now.duration_since(checked) > Duration::from_secs(2) {
                checked = now;
                let default = default_device_name();
                if default.is_some() && &default != name {
                    why = Some(format!("the default output device is now \"{}\"", default.unwrap_or_default()));
                }
            }
            if let Some(why) = why {
                log(&format!("audio: the output device \"{}\" is closed: {why}; reopening", name.clone().unwrap_or_default()));
                device = None;
                stats.device.store(false, Ordering::Relaxed);
                next_open = now;
                blind = now;
            }
        }
        if device.is_none() && now >= next_open {
            let name = default_device_name();
            let failed_flag = failed.clone();
            let opened = rodio::DeviceSinkBuilder::from_default_device().and_then(|builder| builder.with_error_callback(move |_error| failed_flag.store(true, Ordering::SeqCst)).open_stream());
            match opened {
                Ok(mut sink) => {
                    sink.log_on_drop(false);
                    sink.mixer().add(Source { state: state.clone(), stats: stats.clone(), block: vec![0.0; 512], at: 512 });
                    stats.opens.fetch_add(1, Ordering::Relaxed);
                    stats.device.store(true, Ordering::Relaxed);
                    failed.store(false, Ordering::SeqCst);
                    pulled = (stats.frames.load(Ordering::Relaxed), now);
                    checked = now;
                    told_no_device = false;
                    log(&format!("audio: output device \"{}\" opened ({} Hz, {} channel(s), shared mode through rodio / cpal)", name.clone().unwrap_or_default(), sink.config().sample_rate(), sink.config().channel_count()));
                    device = Some((sink, name));
                }
                Err(error) => {
                    stats.open_failures.fetch_add(1, Ordering::Relaxed);
                    if !told_no_device {
                        log(&format!("audio: NO OUTPUT DEVICE ({error}): silent; trying again every 2 s"));
                        told_no_device = true;
                    }
                    next_open = now + Duration::from_secs(2);
                }
            }
        }
        // ---- without a device the mixer still runs, in real time, into nothing
        if device.is_none() {
            let due = (now.duration_since(blind).as_secs_f64() * kh_audio::mixer::RATE as f64) as usize;
            if due >= 256 {
                if let Ok(mut state) = state.lock() {
                    let mut left = due.min(kh_audio::mixer::RATE as usize / 2);
                    while left >= 256 {
                        state.render(&mut scratch, &stats);
                        left -= 256;
                    }
                }
                stats.frames_blind.fetch_add(due as u64, Ordering::Relaxed);
                blind = now;
            }
        }
        // ---- the host's commands
        match rx.recv_timeout(Duration::from_millis(if device.is_some() { 250 } else { 20 })) {
            Ok(first) => {
                let mut next = Some(first);
                while let Some(feed) = next {
                    let mix = match feed {
                        Feed::Voice(VoiceCommand::Start { voice, bank, material, offset, looped, params, .. }) => match materials.get(&pack, bank, material) {
                            Some(buffer) => Some(Mix::Start { voice, buffer, offset, looped, params, bank, material }),
                            None => {
                                stats.missing.fetch_add(1, Ordering::Relaxed);
                                None
                            }
                        },
                        Feed::Voice(command) => Some(Mix::Command(command)),
                        Feed::Master(gain) => Some(Mix::Master(gain)),
                        Feed::StopAll => Some(Mix::StopAll),
                        Feed::Quit => return,
                        Feed::Preload(list) => {
                            materials.preload(&pack, &list);
                            None
                        }
                    };
                    if let Some(mix) = mix {
                        if mix_tx.send(mix).is_err() {
                            return;
                        }
                    }
                    next = rx.try_recv().ok();
                }
                for error in materials.errors.drain(..) {
                    log(&format!("audio: {error}"));
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

enum Kind {
    /// Nothing is heard and nothing is kept.
    Null,
    Device { tx: Sender<Feed>, thread: Option<std::thread::JoinHandle<()>> },
    Render { mixer: Mixer, materials: Materials, pack: Arc<Pack>, wav: WavOut, block: Vec<f32> },
}

/// Where the voice commands of a host go.
pub struct Output {
    kind: Kind,
    pub stats: Arc<Stats>,
    master: f32,
}

impl Output {
    /// Silence (no pack, or switched off).
    pub fn null() -> Output {
        Output { kind: Kind::Null, stats: Arc::new(Stats::default()), master: 1.0 }
    }

    /// The default output device, on a thread of ours. Returns at once; the device is opened
    /// (and reopened) by that thread, which says what it does through `log`.
    pub fn device(pack: Arc<Pack>, lowpass: bool, log: fn(&str)) -> Output {
        let stats = Arc::new(Stats::default());
        let (tx, rx) = mpsc::channel::<Feed>();
        let thread_stats = stats.clone();
        let thread = std::thread::Builder::new().name("kh-audio".to_string()).spawn(move || {
            // (nothing of ours may unwind into a host: a panic ends this thread and the sound, no more)
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| feeder(rx, pack, thread_stats, log, lowpass))).is_err() {
                log("audio: PANIC in the audio thread: Sora is silent from now on");
            }
        });
        match thread {
            Ok(thread) => Output { kind: Kind::Device { tx, thread: Some(thread) }, stats, master: 1.0 },
            Err(error) => {
                log(&format!("audio: the audio thread could not be started ({error}): silent"));
                Output { kind: Kind::Null, stats, master: 1.0 }
            }
        }
    }

    /// An offline render into a WAV file: `advance` writes what the mixer produces.
    pub fn render(pack: Arc<Pack>, lowpass: bool, path: &str) -> std::io::Result<Output> {
        let mut mixer = Mixer::new();
        mixer.set_lowpass(lowpass);
        Ok(Output { kind: Kind::Render { mixer, materials: Materials::default(), pack, wav: WavOut::create(path)?, block: Vec::new() }, stats: Arc::new(Stats::default()), master: 1.0 })
    }

    pub fn is_device(&self) -> bool {
        matches!(self.kind, Kind::Device { .. })
    }

    /// The voice commands of one engine step. Never blocks.
    pub fn send(&mut self, commands: &[VoiceCommand]) {
        match &mut self.kind {
            Kind::Null => {}
            Kind::Device { tx, .. } => {
                for command in commands {
                    let _ = tx.send(Feed::Voice(*command));
                }
            }
            Kind::Render { mixer, materials, pack, .. } => {
                for command in commands {
                    if let VoiceCommand::Start { .. } = command {
                        self.stats.started.fetch_add(1, Ordering::Relaxed);
                    }
                    mixer.apply(command, pack, materials);
                }
                self.stats.missing.fetch_add(materials.errors.drain(..).count() as u32, Ordering::Relaxed);
            }
        }
    }

    /// The gain of everything this output plays (linear).
    pub fn set_master(&mut self, gain: f32) {
        let gain = gain.clamp(0.0, 4.0);
        if gain == self.master {
            return;
        }
        self.master = gain;
        match &mut self.kind {
            Kind::Null => {}
            Kind::Device { tx, .. } => {
                let _ = tx.send(Feed::Master(gain));
            }
            Kind::Render { mixer, .. } => mixer.master = gain,
        }
    }

    pub fn master(&self) -> f32 {
        self.master
    }

    /// (DON-KIT) Reads these WAV materials ahead of their first play (on the feeder thread for a device).
    pub fn preload(&mut self, list: Vec<(u32, u32)>) {
        match &mut self.kind {
            Kind::Null => {}
            Kind::Device { tx, .. } => {
                let _ = tx.send(Feed::Preload(list));
            }
            Kind::Render { materials, pack, .. } => {
                materials.preload(pack, &list);
            }
        }
    }

    /// Every voice stops (with the mixer's short fade).
    pub fn stop_all(&mut self) {
        match &mut self.kind {
            Kind::Null => {}
            Kind::Device { tx, .. } => {
                let _ = tx.send(Feed::StopAll);
            }
            Kind::Render { mixer, .. } => mixer.stop_all(),
        }
    }

    /// Offline render only: the next `frames` frames go into the file.
    pub fn advance(&mut self, frames: usize) {
        if let Kind::Render { mixer, wav, block, .. } = &mut self.kind {
            block.clear();
            block.resize(frames * 2, 0.0);
            mixer.render(block);
            wav.append(block);
            self.stats.voices.store(mixer.voices() as u32, Ordering::Relaxed);
            self.stats.peak.store(mixer.peak.to_bits(), Ordering::Relaxed);
            self.stats.clipped.store(mixer.clipped, Ordering::Relaxed);
            self.stats.frames.fetch_add(frames as u64, Ordering::Relaxed);
        }
    }

    /// Offline render only: the file's header is made valid.
    pub fn finish(&mut self) {
        if let Kind::Render { wav, .. } = &mut self.kind {
            wav.finish();
        }
    }

    /// (mixer trace) Switches the per-voice lines on / off; takes the lines so far.
    pub fn trace(&self, on: bool) -> Vec<String> {
        self.stats.trace.store(on, Ordering::Relaxed);
        self.stats.lines.lock().map(|mut l| std::mem::take(&mut *l)).unwrap_or_default()
    }

    /// (mixer trace) Starts capturing the mixer's output (true), or ends it and writes what was captured as a WAV
    /// file (stereo s16, 48 kHz) at `path`: answers (frames, peak) of the capture.
    pub fn capture(&self, start: bool, path: &str) -> Option<(u64, f32)> {
        let mut guard = self.stats.capture.lock().ok()?;
        if start {
            *guard = Some(Vec::new());
            return None;
        }
        let samples = guard.take()?;
        drop(guard);
        let mut wav = WavOut::create(path).ok()?;
        wav.append(&samples);
        wav.finish();
        Some((samples.len() as u64 / 2, samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))))
    }

    /// One line for a host's log.
    pub fn describe(&self) -> String {
        let s = &self.stats;
        match &self.kind {
            Kind::Null => "no output".to_string(),
            Kind::Render { wav, .. } => format!("rendering to {} ({} voices started, peak {:.3}, clipped {})", wav.path, s.started.load(Ordering::Relaxed), s.peak(), s.clipped.load(Ordering::Relaxed)),
            Kind::Device { .. } => format!(
                "device {} (opened {} time(s), {} failure(s)), {} voice(s) now, {} started, peak {:.3}, clipped {}, {} material(s) missing",
                if s.device.load(Ordering::Relaxed) { "OPEN" } else { "NONE" },
                s.opens.load(Ordering::Relaxed),
                s.open_failures.load(Ordering::Relaxed),
                s.voices.load(Ordering::Relaxed),
                s.started.load(Ordering::Relaxed),
                s.peak(),
                s.clipped.load(Ordering::Relaxed),
                s.missing.load(Ordering::Relaxed),
            ),
        }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        match &mut self.kind {
            Kind::Null => {}
            Kind::Render { wav, .. } => wav.finish(),
            Kind::Device { tx, thread } => {
                let _ = tx.send(Feed::Quit);
                // The thread closes the device (cpal joins its callback thread there). Waited for,
                // but not for ever: a host's shutdown must come back.
                if let Some(thread) = thread.take() {
                    let started = Instant::now();
                    while !thread.is_finished() && started.elapsed() < Duration::from_secs(3) {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    if thread.is_finished() {
                        let _ = thread.join();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_null_output_takes_everything_and_keeps_nothing() {
        let mut out = Output::null();
        out.send(&[VoiceCommand::Stop { voice: 3 }]);
        out.set_master(0.5);
        out.stop_all();
        out.advance(800);
        assert!(!out.is_device() && out.stats.frames.load(Ordering::Relaxed) == 0 && out.master() == 0.5);
    }

    #[test]
    fn the_mix_state_starts_updates_and_stops_what_the_queue_carries() {
        let (tx, rx) = mpsc::channel::<Mix>();
        let stats = Stats::default();
        let mut state = MixState { rx, mixer: Mixer::new(), rendered: 0, last_peak: 0.0, watched: Vec::new() };
        let buffer = Arc::new(Buffer { channels: 1, sample_rate: kh_audio::mixer::RATE, data: vec![16_000; 48_000], loop_points: None });
        let params = VoiceParams { volume: 1.0, pitch: 1.0, pan: 0.0, pan_depth: 1.0, lowpass: 1.0, reverb_send: 0.0 };
        tx.send(Mix::Start { voice: 7, buffer, offset: 0.0, looped: true, params, bank: 0, material: 0 }).unwrap();
        tx.send(Mix::Master(0.5)).unwrap();
        let mut block = vec![0.0f32; 512];
        state.render(&mut block, &stats);
        // A mono voice dead ahead at -3 dB in both channels, times the master gain.
        let expected = 16_000.0 / 32_768.0 * std::f32::consts::FRAC_PI_4.cos() * 0.5;
        assert!((block[200] - expected).abs() < 1.0e-4, "{} vs {expected}", block[200]);
        assert_eq!((stats.voices.load(Ordering::Relaxed), stats.started.load(Ordering::Relaxed)), (1, 1));
        tx.send(Mix::StopAll).unwrap();
        state.render(&mut block, &stats);
        state.render(&mut block, &stats);
        assert_eq!(stats.voices.load(Ordering::Relaxed), 0);
        assert!(block[400].abs() < 1.0e-6 && stats.clipped.load(Ordering::Relaxed) == 0);
    }
}
