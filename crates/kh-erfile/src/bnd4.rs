//! BND4, the game's file binder (little-endian as the PC files are): a 0x40 header (file count at 0x0C, the entry size at
//! 0x20, unicode names at 0x30, the format byte at 0x31), then the entries. The format's flags (ids, names, long offsets,
//! compression = an uncompressed size) are stored bit-reversed unless byte 0x0A says otherwise; which reading is meant is
//! checked against the entry size the header gives.

#[derive(Clone, Debug)]
pub struct BndFile {
    pub id: i32,
    pub name: String,
    pub data: Vec<u8>,
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    b.get(at..at + 8).map(|x| u64::from_le_bytes(x.try_into().unwrap()))
}

fn name_at(b: &[u8], at: usize, unicode: bool) -> String {
    if unicode {
        let mut units = Vec::new();
        let mut k = at;
        while let Some(x) = b.get(k..k + 2) {
            let u = u16::from_le_bytes([x[0], x[1]]);
            if u == 0 {
                break;
            }
            units.push(u);
            k += 2;
        }
        String::from_utf16_lossy(&units)
    } else {
        let end = b[at.min(b.len())..].iter().position(|c| *c == 0).map_or(b.len(), |p| at + p);
        String::from_utf8_lossy(&b[at.min(b.len())..end]).to_string()
    }
}

/// The entry layout one format byte gives: (size, has uncompressed size, long offsets, ids, names).
fn layout(format: u8) -> (usize, bool, bool, bool, bool) {
    let (ids, names, long, compressed) = (format & 0x02 != 0, format & 0x0C != 0, format & 0x10 != 0, format & 0x20 != 0);
    let size = 8 + 8 + if compressed { 8 } else { 0 } + if long { 8 } else { 4 } + if ids { 4 } else { 0 } + if names { 4 } else { 0 };
    (size, compressed, long, ids, names)
}

/// Every file of a BND4.
pub fn read(b: &[u8]) -> Result<Vec<BndFile>, String> {
    if !b.starts_with(b"BND4") {
        return Err("not a BND4".to_string());
    }
    let count = u32_at(b, 0x0C).ok_or("short BND4")? as usize;
    let entry_size = u64_at(b, 0x20).ok_or("short BND4")? as usize;
    let unicode = b.get(0x30).copied().unwrap_or(0) != 0;
    let raw = b.get(0x31).copied().unwrap_or(0);
    let reversed = raw.reverse_bits();
    // the reading whose entry size is the header's (names may carry 4 more bytes of zero in some formats)
    let format = [reversed, raw].into_iter().find(|f| layout(*f).0 == entry_size || layout(*f).0 + 4 == entry_size).ok_or(format!("BND4 format {raw:#x} with entries of {entry_size:#x} bytes is not understood"))?;
    let (_, compressed, long, ids, names) = layout(format);
    let mut out = Vec::with_capacity(count);
    for k in 0..count {
        let mut e = 0x40 + k * entry_size;
        e += 8; // flags, 3 zero bytes, -1
        let size = u64_at(b, e).ok_or("short entry")? as usize;
        e += 8;
        if compressed {
            e += 8;
        }
        let offset = if long {
            let v = u64_at(b, e).ok_or("short entry")? as usize;
            e += 8;
            v
        } else {
            let v = u32_at(b, e).ok_or("short entry")? as usize;
            e += 4;
            v
        };
        let id = if ids {
            let v = u32_at(b, e).ok_or("short entry")? as i32;
            e += 4;
            v
        } else {
            -1
        };
        let name = if names { u32_at(b, e).map(|n| name_at(b, n as usize, unicode)).unwrap_or_default() } else { String::new() };
        let data = b.get(offset..offset + size).ok_or(format!("entry {k} ({name}) lies past the binder's end"))?.to_vec();
        out.push(BndFile { id, name, data });
    }
    Ok(out)
}
