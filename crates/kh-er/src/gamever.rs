//! (PORT) WHICH eldenring.exe IS RUNNING, and the per-version address tables that are not found by pattern alone.
//!
//! Supported: 2.7.1.0 (app 1.17.1) and 2.7.2.0 (the 2026-10-21 patch). Every address the mod uses was carried over to
//! 2.7.2.0 by unique masked byte signatures of the 2.7.1.0 code (rel32 / RIP-relative operands wildcarded), checked
//! against the identity of the whole function: build/sora_tmp/er/port272/INVENTORY.md (tools in port272/tools).
//!
//! RULE: an address with no value for the running version (0 in its table) is NOT guessed: its feature stays off with
//! a log line. Functions are found by their pattern at run time (havok.rs `locate`): one unique match, which must
//! equal the table's value when the table has one; title.rs `at` checks the function's first bytes at the table's
//! address and, when the version has no entry, takes the one unique place those bytes are at.

use std::sync::OnceLock;

/// The executables this DLL knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExeVersion {
    V2710,
    V2720,
}

impl ExeVersion {
    pub const fn name(self) -> &'static str {
        match self {
            ExeVersion::V2710 => "2.7.1.0",
            ExeVersion::V2720 => "2.7.2.0",
        }
    }
}

/// The file versions this DLL runs on (anything else: the payload and the loader do nothing).
pub const SUPPORTED: [((u16, u16, u16, u16), ExeVersion); 2] = [((2, 7, 1, 0), ExeVersion::V2710), ((2, 7, 2, 0), ExeVersion::V2720)];

/// The names of the supported versions, for log lines.
pub const SUPPORTED_TEXT: &str = "2.7.1.0 or 2.7.2.0";

pub fn from_file_version(v: (u16, u16, u16, u16)) -> Option<ExeVersion> {
    SUPPORTED.iter().find(|(file, _)| *file == v).map(|(_, e)| *e)
}

static CURRENT: OnceLock<Option<ExeVersion>> = OnceLock::new();

/// The running executable's version (None: not one of ours, or no game executable in this process).
pub fn current() -> Option<ExeVersion> {
    *CURRENT.get_or_init(|| {
        #[cfg(windows)]
        {
            crate::entry::exe_file_version().and_then(from_file_version)
        }
        #[cfg(not(windows))]
        {
            None
        }
    })
}

/// The name of the running version ("unknown" when it is not one of ours).
pub fn current_name() -> &'static str {
    current().map(ExeVersion::name).unwrap_or("unknown")
}

/// One value per version: (2.7.1.0, 2.7.2.0), 0 = not resolved for that version.
pub fn pick_for(version: Option<ExeVersion>, v2710: usize, v2720: usize) -> Option<usize> {
    let v = match version? {
        ExeVersion::V2710 => v2710,
        ExeVersion::V2720 => v2720,
    };
    (v != 0).then_some(v)
}

/// [`pick_for`] for the running executable.
pub fn pick(v2710: usize, v2720: usize) -> Option<usize> {
    pick_for(current(), v2710, v2720)
}

/// (IMPL_ER28 gate.callers) The return addresses the item sheet names, as (2.7.1.0, 2.7.2.0) RVAs. The sheet stays
/// written in 2.7.1.0 terms; a 2.7.2.0 return address is told in those terms before the sheet is asked.
/// (Each: unique signature from the address on, the function around it found; INVENTORY.md.)
pub const CALLERS: [(u32, u32); 4] = [
    (0x006e_8590, 0x006e_8a40), // gather (Rowa Fruit, Erdleaf Flower)
    (0x0056_db3d, 0x0056_dfed), // an event's award
    (0x00ea_8f99, 0x00ea_a689), // a talk's gift
    (0x0055_c97e, 0x0055_ce2e), // the treasure pickup (corpse or ground)
];

/// (AGGRO) WORLDCHRMAN FIELDS THAT MOVED IN 2.7.2.0. The SDK's `WorldChrMan` is the 2.7.1.0 layout; in 2.7.2.0 the
/// constructor (`WorldChrManImp` vtable 0x2a4e980 -> 0x2a509d0, ctor 0x504c8a -> 0x504d6a) allocates TWO more objects
/// at +0x1e600 / +0x1e608, so every field from 2.7.1.0's +0x1e608 on sits 0x10 further (the ctor's stores, side by
/// side: 2.7.1.0 `mov [rbx+1E648h],r13` / `mov [rbx+1ECE0h],r13` = 2.7.2.0 `mov [rbx+1E658h],r13` /
/// `mov [rbx+1ECF0h],r13`; the creator's 0x1C0-byte allocation, identical code, stores to +0x1e648 / +0x1e658). The
/// fields before (+0x1e5e0 and lower: chr sets, main player, summon buddy manager) are where the SDK has them (equal
/// reference counts in both files). Read through `game::debug_chr_creator` / `game::chr_cam` only. Proven by the
/// offline test `aggro_offline::aggro_worldchrman_shift_in_both_executables`.
pub mod wcm {
    /// `debug_chr_creator` (OwnedPtr<CSDebugChrCreator>): (2.7.1.0 = the SDK's, 2.7.2.0).
    pub const DEBUG_CHR_CREATOR: (usize, usize) = (0x1_E648, 0x1_E658);
    /// `chr_cam` (Option<NonNull<ChrCam>>).
    pub const CHR_CAM: (usize, usize) = (0x1_ECE0, 0x1_ECF0);
    /// The RVA of `CS::CSDebugChrCreator`'s vtable (RTTI): a creator pointer is only used when its first word is this.
    pub const DEBUG_CREATOR_VTABLE: (usize, usize) = (0x02A5_1220, 0x02A5_3340);
}

/// A return address of the running executable told as the 2.7.1.0 RVA the sheet uses (unchanged on 2.7.1.0; on
/// another version only the named ones are carried back, anything else is given a value no sheet row has).
pub fn caller_as_2710(version: Option<ExeVersion>, rva: u32) -> u32 {
    match version {
        Some(ExeVersion::V2710) => rva,
        Some(ExeVersion::V2720) => CALLERS.iter().find(|(_, new)| *new == rva).map(|(old, _)| *old).unwrap_or(0x8000_0000 | rva),
        None => 0x8000_0000 | rva,
    }
}

/// (PORT) THE OFFLINE ADDRESS CHECK: every pattern and every recorded address of the mod run against the two copies of
/// eldenring.exe (read-only copies under build/sora_tmp/er; the env keys KH_ER_EXE_2710 / KH_ER_EXE_2720 point
/// elsewhere). A missing file is reported and skipped (the snapshot builds have no game files).
#[cfg(all(test, windows))]
mod offline {
    use super::ExeVersion;
    use crate::havok::{resolve_in, Located};
    use crate::title::{known_in, Known};
    use pelite::pe64::{Pe, PeFile};

    fn exe_bytes(env: &str, rel: &str) -> Option<Vec<u8>> {
        let candidates = [
            std::env::var(env).ok().map(std::path::PathBuf::from),
            Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)),
            Some(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(rel)),
        ];
        candidates.into_iter().flatten().find_map(|p| std::fs::read(&p).ok())
    }

    fn located() -> Vec<&'static Located> {
        let mut all: Vec<&'static Located> = vec![
            &crate::cam::STEP_FN,
            &crate::cam::LISTENER_FN,
            &crate::chranim::GET,
            &crate::chranim::FIRE,
            &crate::chranim::NAMES,
            &crate::erhud::APPLY_FN,
            &crate::erkeyui::STATUS_FN,
            &crate::erkeyui::ITEM_GET_FN, // (ERUI)
            &crate::loot::GIVE_FN,
            &crate::targets::CAN_TARGET,
            &crate::pause::STEP_FN,
            &crate::status::APPLY_FN,
            &crate::talkkh::hooks::EVENT_FN,
            &crate::talkkh::hooks::ENV_FN,
            &crate::talkkh::hooks::UPDATE_FN,
            &crate::talkkh::hooks::SUBTITLE_FN,
        ];
        all.extend(crate::havok::LOCATED.iter());
        all.extend(crate::ersound::LOCATED.iter()); // (LADDER2) the game's own sound play, Torrent's bank
        all.extend(crate::savegate::STARTS.iter());
        #[cfg(feature = "party")]
        all.push(&crate::allybody::CAN_TARGET);
        all
    }

    fn known() -> Vec<&'static Known> {
        vec![
            &crate::title::SET_STATE,
            &crate::title::BEGIN_NEW_GAME,
            &crate::title::MAKE_BASE_SELECT,
            &crate::title::MAKE_TITLE_TOP,
            &crate::title::MAKE_PROFILE_LOAD,
            &crate::remo::ANIM_ACT_ACCESSOR,
            &crate::tutorial::MAKE_JOB,
        ]
    }

    /// The RIP-relative target of the instruction of `len` bytes at `rva` whose disp32 is at `+disp_at`.
    fn rip_target(pe: PeFile, rva: u32, disp_at: u32, len: u32) -> Option<u32> {
        let d = pe.derva_slice::<u8>(rva + disp_at, 4).ok()?;
        Some((rva + len).wrapping_add(i32::from_le_bytes([d[0], d[1], d[2], d[3]]) as u32))
    }

    fn check(bytes: &[u8], version: ExeVersion) -> Vec<String> {
        let pe = PeFile::from_bytes(bytes).expect("a PE file");
        let mut problems = Vec::new();
        let mut lines = Vec::new();
        for item in located() {
            match resolve_in(pe, item, Some(version)) {
                Ok(rva) => lines.push(format!("  {:<60} {rva:#09x}", item.name)),
                Err(e) => problems.push(e),
            }
            // the pattern alone (no table): must be unique and give the same place (what a future version would get)
            match resolve_in(pe, item, None) {
                Ok(rva) if Some(rva) == item.recorded(Some(version)) => {}
                other => problems.push(format!("{}: by pattern alone {other:x?}, recorded {:x?}", item.name, item.recorded(Some(version)))),
            }
        }
        for k in known() {
            match known_in(pe, k, Some(version)) {
                Ok(rva) => lines.push(format!("  {:<60} {rva:#09x} (bytes checked)", k.name)),
                Err(e) => problems.push(e),
            }
        }
        // the two globals read from inside located functions
        let v = Some(version);
        let step = resolve_in(pe, &crate::pause::STEP_FN, v).unwrap_or(0) + crate::pause::POPUP_CHECK_IN_FN as u32;
        let popup_ok = pe.derva_slice::<u8>(step, 7).map(|c| c[0] == 0x80 && c[1] == 0x3d && c[6] == 0).unwrap_or(false);
        let popup = rip_target(pe, step, 2, 7); // `cmp byte [rip+d], 0`: 7 bytes, the immediate after the disp
        let want_popup = super::pick_for(v, crate::pause::POPUP_BYTE_RVA, crate::pause::POPUP_BYTE_RVA_2720).map(|r| r as u32);
        if !popup_ok || popup != want_popup {
            problems.push(format!("popup byte: compare found {popup_ok}, target {popup:x?}, recorded {want_popup:x?}"));
        } else {
            lines.push(format!("  {:<60} {:#09x} (read from step + 0xab7)", "tutorial popup byte", popup.unwrap()));
        }
        let load = resolve_in(pe, &crate::talkkh::hooks::UPDATE_FN, v).unwrap_or(0) + crate::talkkh::hooks::SKIP_LOAD_IN_FN as u32;
        let load_ok = pe.derva_slice::<u8>(load, 14).map(|c| c[..3] == [0x48, 0x8b, 0x05] && c[7..11] == [0x80, 0xb8, 0xc0, 0x92]).unwrap_or(false);
        let skip = rip_target(pe, load, 3, 7);
        let want_skip = super::pick_for(v, crate::talkkh::hooks::SKIP_GLOBAL_RVA, crate::talkkh::hooks::SKIP_GLOBAL_RVA_2720).map(|r| r as u32);
        if !load_ok || skip != want_skip {
            problems.push(format!("line-skip global: load found {load_ok}, target {skip:x?}, recorded {want_skip:x?}"));
        } else {
            lines.push(format!("  {:<60} {:#09x} (read from update + 0xbd)", "talk line-skip global", skip.unwrap()));
        }
        // the gate's named return addresses: each follows a `call rel32` in this file
        for (old, new) in super::CALLERS {
            let at = if version == ExeVersion::V2710 { old } else { new };
            if pe.derva_slice::<u8>(at - 5, 1).map(|c| c[0] != 0xE8).unwrap_or(true) {
                problems.push(format!("gate caller {at:#x}: not after a call"));
            }
        }
        println!("PORT offline check, {}: {} addresses resolved, {} problems", version.name(), lines.len(), problems.len());
        for l in &lines {
            println!("{l}");
        }
        problems
    }

    #[test]
    fn port_every_address_resolves_in_both_executables() {
        let mut checked = 0;
        for (env, rel, version) in [
            ("KH_ER_EXE_2710", "build/sora_tmp/er/boss/eldenring.exe", ExeVersion::V2710),
            ("KH_ER_EXE_2720", "build/sora_tmp/er/port272/eldenring_2720.exe", ExeVersion::V2720),
        ] {
            let Some(bytes) = exe_bytes(env, rel) else {
                println!("PORT offline check: no copy of eldenring.exe {} ({rel}): skipped", version.name());
                continue;
            };
            let problems = check(&bytes, version);
            assert!(problems.is_empty(), "{}: {problems:#?}", version.name());
            checked += 1;
        }
        println!("PORT offline check: {checked} executables checked");
    }
}

/// (AGGRO) The WorldChrMan shift of 2.7.2.0 read from the two executables (the read-only copies the PORT check uses).
#[cfg(all(test, windows))]
mod aggro_offline {
    use super::wcm;
    use pelite::pe64::{Pe, PeFile};

    fn bytes(rel: &str) -> Option<Vec<u8>> {
        [std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel), std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(rel)].into_iter().find_map(|p| std::fs::read(p).ok())
    }

    /// (rva, bytes) of every section of the image with `exec` / without.
    fn sections(pe: PeFile, exec: bool) -> Vec<(u32, &[u8])> {
        pe.section_headers().iter().filter(|s| (s.Characteristics & 0x2000_0000 != 0) == exec).filter_map(|s| pe.derva_slice::<u8>(s.VirtualAddress, s.SizeOfRawData.min(s.VirtualSize.max(s.SizeOfRawData)) as usize).ok().map(|b| (s.VirtualAddress, b))).collect()
    }

    /// RVAs of `pat` (None = any byte) in the code.
    fn find(pe: PeFile, pat: &[Option<u8>]) -> Vec<u32> {
        let mut out = Vec::new();
        for (va, b) in sections(pe, true) {
            for i in 0..b.len().saturating_sub(pat.len()) {
                if pat.iter().enumerate().all(|(k, p)| p.is_none_or(|v| b[i + k] == v)) {
                    out.push(va + i as u32);
                }
            }
        }
        out
    }

    fn le(v: u32) -> [Option<u8>; 4] {
        v.to_le_bytes().map(Some)
    }

    /// The vtable RVA of an RTTI class (type descriptor name -> complete object locator -> the vtable after it).
    fn vtable(pe: PeFile, name: &str) -> Option<u32> {
        let image = pe.optional_header().ImageBase;
        let data = sections(pe, false);
        let td = data.iter().find_map(|(va, b)| b.windows(name.len()).position(|w| w == name.as_bytes()).map(|p| va + p as u32 - 0x10))?;
        for (va, b) in &data {
            for o in (0..b.len().saturating_sub(24)).step_by(4) {
                let u = |k: usize| u32::from_le_bytes([b[o + k], b[o + k + 1], b[o + k + 2], b[o + k + 3]]);
                if u(0) == 1 && u(4) == 0 && u(12) == td && u(20) == va + o as u32 {
                    let col = (image + (va + o as u32) as u64).to_le_bytes();
                    for (va2, b2) in &data {
                        if let Some(q) = (0..b2.len().saturating_sub(8)).step_by(8).find(|q| b2[*q..*q + 8] == col) {
                            return Some(va2 + q as u32 + 8);
                        }
                    }
                }
            }
        }
        None
    }

    #[test]
    fn aggro_worldchrman_shift_in_both_executables() {
        let mut checked = 0;
        for (rel, idx) in [("build/sora_tmp/er/boss/eldenring.exe", 0usize), ("build/sora_tmp/er/port272/eldenring_2720.exe", 1)] {
            let Some(b) = bytes(rel) else {
                println!("AGGRO offline check: no {rel}: skipped");
                continue;
            };
            let pe = PeFile::from_bytes(&b).expect("a PE file");
            let pick = |t: (usize, usize)| if idx == 0 { t.0 } else { t.1 } as u32;
            let other = |t: (usize, usize)| if idx == 0 { t.1 } else { t.0 } as u32;
            // the creator's allocation (0x1C0 bytes, CSDebugChrCreator's size) and the store of the new object:
            // mov ecx,1C0h; call; mov rdi,rax; mov [rsp+20h],rax; test rax,rax; je; ... mov [rbx+creator],rax
            let alloc = find(pe, &[Some(0xB9), Some(0xC0), Some(0x01), Some(0), Some(0), Some(0xE8), None, None, None, None, Some(0x48), Some(0x8B), Some(0xF8), Some(0x48), Some(0x89), Some(0x44), Some(0x24), Some(0x20), Some(0x48), Some(0x85), Some(0xC0), Some(0x74)]);
            let mut store = [Some(0x48), Some(0x89), Some(0x83), None, None, None, None].to_vec();
            store.splice(3..7, le(pick(wcm::DEBUG_CHR_CREATOR)));
            let stores = find(pe, &store);
            let paired: Vec<_> = alloc.iter().filter(|a| stores.iter().any(|s| *s > **a && *s - **a < 0x60)).collect();
            assert_eq!(paired.len(), 1, "{rel}: one 0x1C0 allocation stored to WorldChrMan+{:#x} (allocs {alloc:x?}, stores {stores:x?})", pick(wcm::DEBUG_CHR_CREATOR));
            // the constructor's clears: mov [rbx+X],r13 (4C 89 AB X) for the creator and for chr_cam, in this file's place
            // only (the other version's offset holds another field here)
            let clear = |x: u32| {
                let mut p = vec![Some(0x4C), Some(0x89), Some(0xAB)];
                p.extend(le(x));
                find(pe, &p)
            };
            let cam = clear(pick(wcm::CHR_CAM));
            assert_eq!(cam.len(), 1, "{rel}: chr_cam cleared once at +{:#x}: {cam:x?}", pick(wcm::CHR_CAM));
            // chr_cam's clear is followed by lea rdi,[rbx+X+8] (the task block after it) in this file only
            let mut lea = vec![Some(0x48), Some(0x8D), Some(0xBB)];
            lea.extend(le(pick(wcm::CHR_CAM) + 8));
            assert!(find(pe, &lea).iter().any(|l| *l == cam[0] + 7), "{rel}: the task block follows chr_cam");
            let mut lea_other = vec![Some(0x48), Some(0x8D), Some(0xBB)];
            lea_other.extend(le(other(wcm::CHR_CAM) + 8));
            assert!(clear(other(wcm::CHR_CAM)).iter().all(|c| !find(pe, &lea_other).contains(&(c + 7))), "{rel}: the other version's chr_cam place is not this file's");
            assert_eq!(clear(pick(wcm::DEBUG_CHR_CREATOR)).len(), 1, "{rel}: the creator cleared once by the constructor");
            // the creator's vtable by RTTI
            assert_eq!(vtable(pe, ".?AVCSDebugChrCreator@CS@@"), Some(pick(wcm::DEBUG_CREATOR_VTABLE)), "{rel}: CSDebugChrCreator's vtable");
            println!("AGGRO offline check {rel}: creator at WorldChrMan+{:#x} (alloc {:#x}), chr_cam +{:#x}, creator vtable {:#x}", pick(wcm::DEBUG_CHR_CREATOR), paired[0], pick(wcm::CHR_CAM), pick(wcm::DEBUG_CREATOR_VTABLE));
            checked += 1;
        }
        println!("AGGRO offline check: {checked} executables checked");
    }

    /// The 2.7.1.0 values are the SDK's own offsets.
    #[test]
    fn aggro_the_2710_values_are_the_sdks() {
        assert_eq!(std::mem::offset_of!(eldenring::cs::WorldChrMan, debug_chr_creator), wcm::DEBUG_CHR_CREATOR.0);
        assert_eq!(std::mem::offset_of!(eldenring::cs::WorldChrMan, chr_cam), wcm::CHR_CAM.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_versions_and_tables() {
        assert_eq!(from_file_version((2, 7, 1, 0)), Some(ExeVersion::V2710));
        assert_eq!(from_file_version((2, 7, 2, 0)), Some(ExeVersion::V2720));
        assert_eq!(from_file_version((2, 7, 1, 1)), None);
        assert_eq!(from_file_version((2, 7, 3, 0)), None);
        assert_eq!(pick_for(Some(ExeVersion::V2710), 0x10, 0x20), Some(0x10));
        assert_eq!(pick_for(Some(ExeVersion::V2720), 0x10, 0x20), Some(0x20));
        assert_eq!(pick_for(Some(ExeVersion::V2720), 0x10, 0), None, "an unresolved address is never guessed");
        assert_eq!(pick_for(None, 0x10, 0x20), None);
        assert_eq!(caller_as_2710(Some(ExeVersion::V2710), 0x6e8590), 0x6e8590);
        assert_eq!(caller_as_2710(Some(ExeVersion::V2720), 0x6e8a40), 0x6e8590);
        assert_eq!(caller_as_2710(Some(ExeVersion::V2720), 0x55ce2e), 0x55c97e);
        // a 2.7.2.0 address that happens to equal a 2.7.1.0 sheet address is not mistaken for it
        assert_ne!(caller_as_2710(Some(ExeVersion::V2720), 0x6e8590), 0x6e8590);
    }
}
