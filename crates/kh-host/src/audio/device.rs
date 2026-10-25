//! `kh_audio::Mixer` on the default output device through rodio (which carries the frames to
//! cpal: WASAPI shared mode on Windows), and the WAV file of an offline render.
//! (Moved here from kh-app/src/audio.rs, unchanged: the test app's output.)

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::num::NonZero;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kh_audio::Mixer;

/// A WAV file (stereo s16 at the mixer's rate) that is appended to step by step; its header is
/// kept valid by `finish`.
pub struct WavOut {
    file: BufWriter<File>,
    frames: u64,
    pub path: String,
}

impl WavOut {
    pub fn create(path: &str) -> std::io::Result<WavOut> {
        let mut file = BufWriter::new(File::create(path)?);
        file.write_all(&kh_audio::wav::encode(2, kh_audio::mixer::RATE, &[]))?;
        Ok(WavOut { file, frames: 0, path: path.to_string() })
    }

    pub fn append(&mut self, samples: &[f32]) {
        for s in samples {
            self.file.write_all(&((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes()).ok();
        }
        self.frames += samples.len() as u64 / 2;
    }

    /// Writes the sizes into the header.
    pub fn finish(&mut self) {
        let data = (self.frames * 4) as u32;
        let ok = self.file.flush().is_ok()
            && self.file.seek(SeekFrom::Start(4)).is_ok()
            && self.file.write_all(&(36 + data).to_le_bytes()).is_ok()
            && self.file.seek(SeekFrom::Start(40)).is_ok()
            && self.file.write_all(&data.to_le_bytes()).is_ok()
            && self.file.flush().is_ok()
            && self.file.seek(SeekFrom::End(0)).is_ok();
        if !ok {
            eprintln!("audio: could not update the header of {}", self.path);
        }
    }
}

/// The mixer as a rodio source: stereo, 48 kHz, endless.
struct MixerSource {
    mixer: Arc<Mutex<Mixer>>,
    block: Vec<f32>,
    at: usize,
}

impl Iterator for MixerSource {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<rodio::Sample> {
        if self.at >= self.block.len() {
            match self.mixer.lock() {
                Ok(mut mixer) => mixer.render(&mut self.block),
                Err(_) => self.block.fill(0.0),
            }
            self.at = 0;
        }
        let sample = self.block[self.at];
        self.at += 1;
        Some(sample as rodio::Sample)
    }
}

impl rodio::Source for MixerSource {
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

/// Opens the default output device on its own thread (the device handle is not `Send`) and
/// lets it pull from the mixer until the returned sender is dropped.
pub fn start_device_thread(mixer: Arc<Mutex<Mixer>>, thread_name: &str) -> Result<Sender<()>, String> {
    let (sender, receiver) = mpsc::channel::<()>();
    let (opened_tx, opened_rx) = mpsc::channel::<Result<(), String>>();
    std::thread::Builder::new()
        .name(thread_name.to_string())
        .spawn(move || {
            let device = match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(mut device) => {
                    device.log_on_drop(false);
                    device
                }
                Err(error) => {
                    opened_tx.send(Err(error.to_string())).ok();
                    return;
                }
            };
            // 256 frames per pull: about 5 ms.
            device.mixer().add(MixerSource { mixer, block: vec![0.0; 512], at: 512 });
            opened_tx.send(Ok(())).ok();
            // Until the sender is dropped.
            while receiver.recv().is_ok() {}
        })
        .map_err(|e| e.to_string())?;
    opened_rx.recv_timeout(Duration::from_secs(5)).map_err(|_| "the audio thread did not answer".to_string())??;
    Ok(sender)
}
