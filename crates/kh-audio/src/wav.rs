//! WAV files as the audio pack stores them: PCM s16 little-endian, the loop in a `smpl` chunk
//! (start, end inclusive). Reading for the mixer, writing for the offline render.

use std::path::Path;

/// Decoded audio: interleaved s16.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Buffer {
    pub channels: u16,
    pub sample_rate: u32,
    pub data: Vec<i16>,
    /// `[start, end)` in frames, from the `smpl` chunk.
    pub loop_points: Option<(u32, u32)>,
}

impl Buffer {
    pub fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.data.len() / self.channels as usize
        }
    }

    /// A mono buffer from float samples (a host test tone).
    pub fn mono(sample_rate: u32, samples: &[f32]) -> Buffer {
        Buffer { channels: 1, sample_rate, data: samples.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect(), loop_points: None }
    }
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

pub fn parse(bytes: &[u8]) -> Result<Buffer, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a RIFF WAVE file".to_string());
    }
    let mut buffer = Buffer::default();
    let mut bits = 0u16;
    let mut format = 0u16;
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32_at(bytes, at + 4) as usize;
        let body = at + 8;
        let end = (body + size).min(bytes.len());
        match id {
            b"fmt " if size >= 16 && end >= body + 16 => {
                format = u16::from_le_bytes([bytes[body], bytes[body + 1]]);
                buffer.channels = u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]);
                buffer.sample_rate = u32_at(bytes, body + 4);
                bits = u16::from_le_bytes([bytes[body + 14], bytes[body + 15]]);
            }
            b"data" => buffer.data = bytes[body..end].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect(),
            // 36 bytes of header, then loops of 24 bytes: id, type, start, end (inclusive) ...
            b"smpl" if size >= 36 + 24 && end >= body + 60 && u32_at(bytes, body + 28) >= 1 => buffer.loop_points = Some((u32_at(bytes, body + 36 + 8), u32_at(bytes, body + 36 + 12) + 1)),
            _ => {}
        }
        at = body + size + (size & 1);
    }
    if format != 1 || bits != 16 || buffer.channels == 0 {
        return Err(format!("unsupported WAV (format {format}, {bits} bits, {} channels): the pack holds PCM s16", buffer.channels));
    }
    Ok(buffer)
}

pub fn read(path: &Path) -> Result<Buffer, String> {
    parse(&std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?).map_err(|e| format!("{}: {e}", path.display()))
}

/// Interleaved float samples as a PCM s16 WAV file.
pub fn encode(channels: u16, sample_rate: u32, samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() * 2;
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * channels as u32 * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    }
    out
}

pub fn write(path: &Path, channels: u16, sample_rate: u32, samples: &[f32]) -> Result<(), String> {
    std::fs::write(path, encode(channels, sample_rate, samples)).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_file_reads_back() {
        let samples: Vec<f32> = (0..200).map(|i| ((i as f32) * 0.1).sin() * 0.5).collect();
        let b = parse(&encode(2, 48_000, &samples)).unwrap();
        assert_eq!((b.channels, b.sample_rate, b.frames(), b.loop_points), (2, 48_000, 100, None));
        assert!((b.data[7] as f32 / 32767.0 - samples[7]).abs() < 1.0e-4);
        assert!(parse(b"RIFFxxxxWAVE").is_err() && parse(b"nope").is_err());
    }
}
