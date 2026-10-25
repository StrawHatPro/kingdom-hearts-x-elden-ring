//! Elden Ring's archives, read-only: `GAME/<name>.bhd` (the BHD5 index, RSA-encrypted block by block with a key the exe carries
//! in plain PEM text) and `GAME/<name>.bdt` (the files, some byte ranges AES-128-ECB encrypted with a per-file key the index
//! holds). A file is found by the 64-bit hash of its path (lower case, '/' separators, leading '/'; h = h * 0x85 + c),
//! MEASURED on 2.7.2.0: `/chr/c4311.chrbnd.dcx` is in Data3 (the exe maps `chr:` to `data3:/chr`).
//!
//! Nothing is written anywhere; the exe and the archives are only opened for reading.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use num_bigint::BigUint;

use crate::aes::Aes128;

/// The archives a character model can be in, in the order they are searched.
pub const CHR_ARCHIVES: [&str; 2] = ["Data3", "DLC"];

/// The 64-bit path hash of the BHD5 index.
pub fn path_hash(path: &str) -> u64 {
    let mut h: u64 = 0;
    for c in path.chars() {
        let c = if c == '\\' { '/' } else { c.to_ascii_lowercase() };
        h = h.wrapping_mul(0x85).wrapping_add(c as u64);
    }
    h
}

/// An RSA public key (modulus, exponent).
#[derive(Clone)]
pub struct RsaKey {
    n: BigUint,
    e: BigUint,
    bytes: usize,
}

fn base64(text: &str) -> Vec<u8> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    };
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in text.bytes() {
        let Some(v) = value(c) else { continue };
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

/// A DER element at `at`: (tag, content range).
fn der(b: &[u8], at: usize) -> Option<(u8, usize, usize)> {
    let tag = *b.get(at)?;
    let first = *b.get(at + 1)? as usize;
    let (len, head) = if first < 0x80 {
        (first, 2)
    } else {
        let n = first & 0x7F;
        let mut len = 0usize;
        for k in 0..n {
            len = (len << 8) | *b.get(at + 2 + k)? as usize;
        }
        (len, 2 + n)
    };
    let start = at + head;
    (start + len <= b.len()).then_some((tag, start, start + len))
}

impl RsaKey {
    /// From PEM text (`BEGIN RSA PUBLIC KEY`: PKCS#1, a SEQUENCE of the modulus and the exponent).
    pub fn from_pem(pem: &str) -> Option<RsaKey> {
        let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
        let d = base64(&body);
        let (tag, s, _) = der(&d, 0)?;
        if tag != 0x30 {
            return None;
        }
        let (t1, ns, ne) = der(&d, s)?;
        let (t2, es, ee) = der(&d, ne)?;
        if t1 != 2 || t2 != 2 {
            return None;
        }
        let n = BigUint::from_bytes_be(&d[ns..ne]);
        let bytes = (n.bits() as usize).div_ceil(8);
        Some(RsaKey { n, e: BigUint::from_bytes_be(&d[es..ee]), bytes })
    }

    /// The index decrypted: each block of the key's size gives that size less one byte (raw RSA, no padding).
    pub fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        for block in data.chunks(self.bytes) {
            let m = BigUint::from_bytes_be(block).modpow(&self.e, &self.n).to_bytes_be();
            let want = self.bytes - 1;
            if m.len() < want {
                out.extend(std::iter::repeat_n(0u8, want - m.len()));
                out.extend_from_slice(&m);
            } else {
                out.extend_from_slice(&m[m.len() - want..]);
            }
        }
        out
    }
}

/// Every RSA public key in plain PEM text in the exe (MEASURED on 2.7.2.0: five; Data0..3 and one more).
pub fn exe_keys(exe: &Path) -> Result<Vec<RsaKey>, String> {
    let bytes = std::fs::read(exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    let (begin, end) = (b"-----BEGIN RSA PUBLIC KEY-----", b"-----END RSA PUBLIC KEY-----");
    let mut keys = Vec::new();
    let mut at = 0;
    while let Some(k) = find(&bytes[at..], begin) {
        let s = at + k;
        let Some(e) = find(&bytes[s..], end) else { break };
        let text = String::from_utf8_lossy(&bytes[s..s + e + end.len()]).to_string();
        if let Some(key) = RsaKey::from_pem(&text) {
            keys.push(key);
        }
        at = s + e + end.len();
    }
    Ok(keys)
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    padded: u64,
    offset: u64,
    aes: u64,
}

/// One archive's index.
pub struct Archive {
    pub name: String,
    bdt: PathBuf,
    index: Vec<u8>,
    files: HashMap<u64, Entry>,
}

fn i32_at(b: &[u8], at: usize) -> Option<i32> {
    b.get(at..at + 4).map(|x| i32::from_le_bytes([x[0], x[1], x[2], x[3]]))
}

fn i64_at(b: &[u8], at: usize) -> Option<i64> {
    b.get(at..at + 8).map(|x| i64::from_le_bytes(x.try_into().unwrap()))
}

impl Archive {
    /// Opens `GAME/<name>.bhd`: plain BHD5, or decrypted with whichever of `keys` gives one.
    pub fn open(game: &Path, name: &str, keys: &[RsaKey]) -> Result<Archive, String> {
        let bhd = game.join(format!("{name}.bhd"));
        let raw = std::fs::read(&bhd).map_err(|e| format!("{}: {e}", bhd.display()))?;
        let index = if raw.starts_with(b"BHD5") {
            raw
        } else {
            // (the right key's first block starts with the magic: tried on one block, then the whole index)
            let key = keys.iter().find(|k| raw.len() >= k.bytes && k.decrypt(&raw[..k.bytes]).starts_with(b"BHD5")).ok_or(format!("{name}.bhd: no key of the exe opens it"))?;
            key.decrypt(&raw)
        };
        let buckets = i32_at(&index, 16).ok_or("short index")? as usize;
        let at = i32_at(&index, 20).ok_or("short index")? as usize;
        let mut files = HashMap::new();
        for b in 0..buckets {
            let (Some(count), Some(off)) = (i32_at(&index, at + b * 8), i32_at(&index, at + b * 8 + 4)) else { break };
            for k in 0..count.max(0) as usize {
                let e = off as usize + k * 40;
                let Some(hash) = index.get(e..e + 8).map(|x| u64::from_le_bytes(x.try_into().unwrap())) else { break };
                let padded = i32_at(&index, e + 8).unwrap_or(0).max(0) as u64;
                let offset = i64_at(&index, e + 16).unwrap_or(0).max(0) as u64;
                let aes = i64_at(&index, e + 32).unwrap_or(0).max(0) as u64;
                files.insert(hash, Entry { padded, offset, aes });
            }
        }
        Ok(Archive { name: name.to_string(), bdt: game.join(format!("{name}.bdt")), index, files })
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn contains(&self, path: &str) -> bool {
        self.files.contains_key(&path_hash(path))
    }

    /// The file's bytes, decrypted (its padded size: a DCX says its own length).
    pub fn read(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
        let Some(e) = self.files.get(&path_hash(path)).copied() else { return Ok(None) };
        let mut f = File::open(&self.bdt).map_err(|x| format!("{}: {x}", self.bdt.display()))?;
        f.seek(SeekFrom::Start(e.offset)).map_err(|x| x.to_string())?;
        let mut data = vec![0u8; e.padded as usize];
        f.read_exact(&mut data).map_err(|x| format!("{}: {x}", self.bdt.display()))?;
        if e.aes > 0 {
            let a = e.aes as usize;
            let key: [u8; 16] = self.index.get(a..a + 16).ok_or("bad AES key offset")?.try_into().unwrap();
            let aes = Aes128::new(&key);
            let ranges = i32_at(&self.index, a + 16).unwrap_or(0).max(0) as usize;
            for r in 0..ranges {
                let (s, t) = (i64_at(&self.index, a + 20 + r * 16).unwrap_or(-1), i64_at(&self.index, a + 28 + r * 16).unwrap_or(-1));
                if s < 0 || t <= s || t as usize > data.len() {
                    continue;
                }
                aes.decrypt_ecb(&mut data[s as usize..t as usize]);
            }
        }
        Ok(Some(data))
    }
}

/// The game's folder: `KH_ER_GAME`, else the Steam default.
pub fn default_game_dir() -> PathBuf {
    std::env::var_os("KH_ER_GAME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_hash_is_the_index_s() {
        // MEASURED in Data3.bhd (2.7.2.0): /chr/c4311.chrbnd.dcx = 938f84d93548cd7b
        assert_eq!(path_hash("/chr/c4311.chrbnd.dcx"), 0x938f84d93548cd7b);
        assert_eq!(path_hash("\\CHR\\C4311.chrbnd.dcx"), 0x938f84d93548cd7b);
    }
}
