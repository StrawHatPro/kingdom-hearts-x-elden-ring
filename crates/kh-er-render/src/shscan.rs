//! DISCOVERY TOOL (`r.shscan = N`, runs once per change of N on our worker thread): the game keeps
//! its compiled shaders in memory as DXBC containers, and those carry their reflection data (the
//! constant buffers' names, members and offsets). This scans the process's committed private
//! memory for containers and writes the ones that mention a keyword to `<dump dir>/shaders/`,
//! with an index of everything found. Read-only: the memory is copied with `ReadProcessMemory`
//! on our own process, so a page that goes away mid-scan is an error code, not a fault.
//! The files are read offline (`dxc -dumpbin`, `fxc /dumpbin`); nothing in the mod depends on them.

use std::collections::HashSet;
use std::ffi::c_void;
use std::io::Write;
use std::path::Path;

use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Memory::{VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_IMAGE, PAGE_READONLY, PAGE_READWRITE};
use windows::Win32::System::Threading::GetCurrentProcess;

use crate::rlog;

const KEYWORDS: [&str; 22] = [
    "PreExposure", "DirLight", "Fog", "Jitter", "Cascade", "Velocity", "TempAA", "HemAmb", "Ambient", "LightScatter", "ShadowMap", "PrevViewProj", "PrevWorldViewProj", "LightDir", "SunDir", "SkyColor", "Exposure", "GIDiffuse", "IrradianceVolume", "EnvDif", "g_Camera", "MotionBlur",
];

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// The container starting at `bytes[0]`, when it is a whole, plausible one: its length and chunk tags.
pub fn container(bytes: &[u8]) -> Option<(usize, Vec<String>)> {
    if bytes.get(..4)? != b"DXBC" || u32_at(bytes, 20)? != 1 {
        return None;
    }
    let size = u32_at(bytes, 24)? as usize;
    let chunks = u32_at(bytes, 28)? as usize;
    if !(64..=8 << 20).contains(&size) || !(1..=32).contains(&chunks) || size > bytes.len() {
        return None;
    }
    let mut tags = Vec::new();
    for k in 0..chunks {
        let offset = u32_at(bytes, 32 + k * 4)? as usize;
        if offset + 8 > size {
            return None;
        }
        let tag = &bytes[offset..offset + 4];
        if !tag.iter().all(|c| c.is_ascii_graphic()) {
            return None;
        }
        tags.push(String::from_utf8_lossy(tag).to_string());
    }
    Some((size, tags))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Scans and writes; returns a summary line.
pub fn run(dir: &Path) -> String {
    let out = dir.join("shaders");
    if let Err(e) = std::fs::create_dir_all(&out) {
        return format!("shader scan: {}: {e}", out.display());
    }
    let Ok(mut index) = std::fs::File::create(out.join("_index.tsv")).map(std::io::BufWriter::new) else { return "shader scan: no index file".to_string() };
    let started = std::time::Instant::now();
    let mut seen: HashSet<[u8; 16]> = HashSet::new();
    let (mut found, mut written, mut scanned) = (0u32, 0u32, 0u64);
    let mut address = 0x10000usize;
    let mut buffer: Vec<u8> = Vec::new();
    // SAFETY: VirtualQuery / ReadProcessMemory on our own process; every read is bounds-checked by the system.
    unsafe {
        let process = GetCurrentProcess();
        loop {
            let mut info = MEMORY_BASIC_INFORMATION::default();
            if VirtualQuery(Some(address as *const c_void), &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) == 0 {
                break;
            }
            let (base, size) = (info.BaseAddress as usize, info.RegionSize);
            address = base + size;
            if address >= 0x7FFF_0000_0000 {
                break;
            }
            if info.State != MEM_COMMIT || info.Type == MEM_IMAGE || (info.Protect != PAGE_READWRITE && info.Protect != PAGE_READONLY) || size > 2 << 30 {
                continue;
            }
            buffer.resize(size, 0);
            let mut got = 0usize;
            if ReadProcessMemory(process, base as *const c_void, buffer.as_mut_ptr() as *mut c_void, size, Some(&mut got)).is_err() || got < 64 {
                continue;
            }
            scanned += got as u64;
            let data = &buffer[..got];
            let mut at = 0usize;
            while at + 64 <= data.len() {
                // (containers are heap blocks: 4-aligned at least)
                if &data[at..at + 4] != b"DXBC" {
                    at += 4;
                    continue;
                }
                let Some((length, tags)) = container(&data[at..]) else {
                    at += 4;
                    continue;
                };
                let blob = &data[at..at + length];
                at += length & !3;
                let digest: [u8; 16] = blob[4..20].try_into().unwrap();
                if !seen.insert(digest) {
                    continue;
                }
                found += 1;
                let hits: Vec<&str> = KEYWORDS.iter().copied().filter(|k| contains(blob, k.as_bytes())).collect();
                let name = format!("{}_{length}.dxbc", digest.iter().take(6).map(|b| format!("{b:02x}")).collect::<String>());
                let _ = writeln!(index, "{name}\t{:#x}\t{}\t{}", base + at - (length & !3), tags.join(","), hits.join(","));
                if !hits.is_empty() && written < 4000 && std::fs::write(out.join(&name), blob).is_ok() {
                    written += 1;
                }
            }
        }
    }
    let _ = index.flush();
    let text = format!("shader scan: {found} distinct containers in {} MB of private memory, {written} with a keyword written to {} ({:.1} s)", scanned >> 20, out.display(), started.elapsed().as_secs_f32());
    rlog!("{text}");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_container_is_recognised_by_its_header() {
        let mut blob = vec![0u8; 96];
        blob[..4].copy_from_slice(b"DXBC");
        blob[20..24].copy_from_slice(&1u32.to_le_bytes());
        blob[24..28].copy_from_slice(&96u32.to_le_bytes());
        blob[28..32].copy_from_slice(&2u32.to_le_bytes());
        blob[32..36].copy_from_slice(&40u32.to_le_bytes());
        blob[36..40].copy_from_slice(&60u32.to_le_bytes());
        blob[40..44].copy_from_slice(b"RDEF");
        blob[60..64].copy_from_slice(b"SHEX");
        assert_eq!(container(&blob), Some((96, vec!["RDEF".to_string(), "SHEX".to_string()])));
        // Cut short, a wrong version, a chunk outside: not one.
        assert_eq!(container(&blob[..80]), None);
        blob[36..40].copy_from_slice(&200u32.to_le_bytes());
        assert_eq!(container(&blob), None);
    }
}
