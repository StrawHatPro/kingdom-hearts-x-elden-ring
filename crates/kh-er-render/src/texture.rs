//! The pack's textures as RGBA8 with a box-filtered mip chain (the same chain the test app
//! builds: kh-app/src/char_material.rs `load_texture`). Pure; no GPU types.

use std::path::Path;

/// RGBA8 texels: mip 0 first, every further mip after it.
#[derive(Clone, Debug, PartialEq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    /// (offset into `data`, width, height) of each mip.
    pub mips: Vec<(usize, u32, u32)>,
    pub data: Vec<u8>,
}

impl Pixels {
    /// From RGBA8 texels; `mips`: build the chain down to 1 x 1.
    pub fn from_rgba(width: u32, height: u32, mut data: Vec<u8>, mips: bool) -> Pixels {
        let mut levels = vec![(0usize, width, height)];
        if mips {
            let (mut w, mut h, mut from) = (width as usize, height as usize, 0usize);
            while w > 1 || h > 1 {
                let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
                let start = data.len();
                data.resize(start + nw * nh * 4, 0);
                for y in 0..nh {
                    for x in 0..nw {
                        let (x0, y0) = ((x * 2).min(w - 1), (y * 2).min(h - 1));
                        let (x1, y1) = ((x * 2 + 1).min(w - 1), (y * 2 + 1).min(h - 1));
                        for c in 0..4 {
                            let s = data[from + (y0 * w + x0) * 4 + c] as u32 + data[from + (y0 * w + x1) * 4 + c] as u32 + data[from + (y1 * w + x0) * 4 + c] as u32 + data[from + (y1 * w + x1) * 4 + c] as u32;
                            data[start + (y * nw + x) * 4 + c] = ((s + 2) / 4) as u8;
                        }
                    }
                }
                levels.push((start, nw as u32, nh as u32));
                (from, w, h) = (start, nw, nh);
            }
        }
        Pixels { width, height, mips: levels, data }
    }

    /// A PNG file of the pack (8 bits a channel; grey and RGB files are widened to RGBA).
    pub fn load_png(path: &Path, mips: bool) -> Result<Pixels, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().map_err(|e| format!("{}: {e}", path.display()))?;
        let size = reader.output_buffer_size().ok_or_else(|| format!("{}: too large", path.display()))?;
        let mut buffer = vec![0u8; size];
        let info = reader.next_frame(&mut buffer).map_err(|e| format!("{}: {e}", path.display()))?;
        buffer.truncate(info.buffer_size());
        let texels = (info.width * info.height) as usize;
        let rgba: Vec<u8> = match info.color_type {
            png::ColorType::Rgba => buffer,
            png::ColorType::Rgb => buffer.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
            png::ColorType::Grayscale => buffer.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
            png::ColorType::GrayscaleAlpha => buffer.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
            other => return Err(format!("{}: colour type {other:?} after expansion", path.display())),
        };
        if rgba.len() != texels * 4 {
            return Err(format!("{}: {} bytes for {} x {}", path.display(), rgba.len(), info.width, info.height));
        }
        Ok(Pixels::from_rgba(info.width, info.height, rgba, mips))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mip_chain_is_a_box_filter_down_to_one_texel() {
        // 4 x 2: left half black, right half white.
        let mut data = Vec::new();
        for _y in 0..2 {
            for x in 0..4 {
                let v = if x < 2 { 0 } else { 255 };
                data.extend([v, v, v, 255]);
            }
        }
        let p = Pixels::from_rgba(4, 2, data.clone(), true);
        assert_eq!(p.mips, vec![(0, 4, 2), (32, 2, 1), (40, 1, 1)]);
        assert_eq!(&p.data[32..40], &[0, 0, 0, 255, 255, 255, 255, 255]);
        assert_eq!(&p.data[40..44], &[128, 128, 128, 255]);
        // A colour table keeps its single level.
        assert_eq!(Pixels::from_rgba(4, 2, data, false).mips.len(), 1);
    }
}

/// A block-compressed texture as its cooked DDS file holds it (the game's own bytes and its own
/// mip chain; a quarter of the memory of RGBA8).
#[derive(Clone, Debug, PartialEq)]
pub struct Compressed {
    pub width: u32,
    pub height: u32,
    /// The DXGI format number (`srgb` already applied).
    pub format: i32,
    /// Bytes of one 4 x 4 block.
    pub block_bytes: usize,
    /// (offset into `data`, width, height) of each mip.
    pub mips: Vec<(usize, u32, u32)>,
    pub data: Vec<u8>,
}

impl Compressed {
    /// Reads a DDS file of one of the block formats the Sora pack's sources use: DXT1 (BC1),
    /// DXT5 (BC3), ATI2 (BC5) and, through the DX10 header, BC7. Anything else is an error (the
    /// caller then takes the PNG).
    pub fn from_dds(bytes: &[u8], srgb: bool) -> Result<Compressed, String> {
        let word = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or("the DDS file is cut short".to_string());
        if bytes.get(..4) != Some(b"DDS ") || word(4)? != 124 {
            return Err("not a DDS file".to_string());
        }
        let (height, width, mips) = (word(12)?, word(16)?, word(28)?.max(1));
        let four = &bytes[84..88];
        // (DXGI numbers: BC1 71 / 72 sRGB, BC3 77 / 78, BC5 83, BC7 98 / 99)
        let (format, block_bytes, header) = match four {
            b"DXT1" => (if srgb { 72 } else { 71 }, 8, 128),
            b"DXT5" => (if srgb { 78 } else { 77 }, 16, 128),
            b"ATI2" | b"BC5U" => (83, 16, 128),
            b"DX10" => match word(128)? {
                98 | 99 => (if srgb { 99 } else { 98 }, 16, 148),
                71 | 72 => (if srgb { 72 } else { 71 }, 8, 148),
                77 | 78 => (if srgb { 78 } else { 77 }, 16, 148),
                83 => (83, 16, 148),
                other => return Err(format!("DXGI format {other} is not one this reader takes")),
            },
            other => return Err(format!("pixel format {:?} is not block compressed", String::from_utf8_lossy(other))),
        };
        if width == 0 || height == 0 || width > 16384 || height > 16384 || mips > 15 {
            return Err(format!("implausible DDS header ({width} x {height}, {mips} mips)"));
        }
        let mut levels = Vec::new();
        let (mut w, mut h, mut at) = (width, height, 0usize);
        for _ in 0..mips {
            let size = (w.div_ceil(4) * h.div_ceil(4)) as usize * block_bytes;
            if header + at + size > bytes.len() {
                break;
            }
            levels.push((at, w, h));
            at += size;
            (w, h) = ((w / 2).max(1), (h / 2).max(1));
        }
        if levels.is_empty() {
            return Err("the DDS file holds no complete mip".to_string());
        }
        Ok(Compressed { width, height, format, block_bytes, mips: levels, data: bytes[header..header + at].to_vec() })
    }

    pub fn load_dds(path: &Path, srgb: bool) -> Result<Compressed, String> {
        Compressed::from_dds(&std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?, srgb)
    }
}

#[cfg(test)]
mod dds_tests {
    use super::*;

    #[test]
    fn a_block_compressed_dds_is_read_with_its_mips() {
        // 8 x 8 DXT5 with 4 mips (8, 4, 2, 1): 4 + 1 + 1 + 1 blocks of 16 bytes.
        let mut file = vec![0u8; 128 + 7 * 16];
        file[..4].copy_from_slice(b"DDS ");
        for (at, value) in [(4usize, 124u32), (12, 8), (16, 8), (28, 4)] {
            file[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        file[84..88].copy_from_slice(b"DXT5");
        file[128 + 64] = 9;
        let t = Compressed::from_dds(&file, true).unwrap();
        assert_eq!((t.width, t.height, t.format, t.block_bytes), (8, 8, 78, 16));
        assert_eq!(t.mips, vec![(0, 8, 8), (64, 4, 4), (80, 2, 2), (96, 1, 1)]);
        assert_eq!((t.data.len(), t.data[64]), (112, 9));
        // BC5 has no sRGB form; a file cut short keeps the mips that are whole; RGBA8 is refused.
        file[84..88].copy_from_slice(b"ATI2");
        assert_eq!(Compressed::from_dds(&file, true).unwrap().format, 83);
        assert_eq!(Compressed::from_dds(&file[..128 + 80], false).unwrap().mips.len(), 2);
        file[84..88].copy_from_slice(&[0; 4]);
        assert!(Compressed::from_dds(&file, false).is_err());
    }
}
