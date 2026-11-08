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
        all.push(&crate::ersound::VOICE_FN); // (ER-TWEAKS) the character voice play (the Tarnished's voice muted)
        all.push(&crate::ersound::PLAY_FN); // (ER-TWEAKS follow-up) the character sound play (the stand-ins' spoken lines muted)
        all.push(&crate::ersound::CREATE_FN); // (ER-HITSFX round 2) the core sound maker (the diagnostic)
        all.extend(crate::ersound::HIT_LOCATED.iter()); // (ER-HITSFX) the material hit-sound play, the character / object hit applies, the field-ins lookup
        all.push(&crate::dragonmsg::BANK_2007_FN); // (ER-TWEAKS) the EMEVD bank-2007 dispatcher (the draconic-power popup not shown)
        all.extend(crate::erdialog::LOCATED); // (ER-POPUPS) the field box open request, the event question waiter's update
        all.extend(crate::gosound::LOCATED.iter()); // (GOS) the game's master-volume factors (the game over's silence)
        all.extend(crate::titlemusic::LOCATED.iter()); // (TITLE-AUDIO) the game's sound-instance maker (its title-menu sounds -> KH3's)
        all.extend(crate::mimic_music::LOCATED.iter()); // (MIMIC-MUSIC) the SetBossBGM core, the BgmVolume setter
        all.extend(crate::savegate::STARTS.iter());
        all.extend(crate::eroptions::LOCATED.iter()); // (CAMP-OPTIONS) Elden Ring's options: load / apply / defaults, the window, the key config, the two menu entries
        all.extend(crate::spirit::LOCATED.iter()); // (THE USER 2026-11-02: "Summon") the buddy range step, spawn point, arena check, HUD spirit block
        all.extend(crate::deathplane::LOCATED.iter()); // (DEATHPLANE) the death barriers' record / kind functions, the tracker's filter
        all.extend(crate::chrtime::LOCATED.iter()); // (WORLDTIME-2) the characters' / bullets' own delta, the effects' rate, the map clock's passage
        all.extend(crate::reactmark::LOCATED.iter()); // (INTERACT-MARK) the action-button region test / in-region test / step, the element dummy-poly point
        all.extend(crate::fxtime::located()); // (WORLDTIME-2 fx) the 16 phase wrappers, the effect allocation / update, the physics step
        all.extend(crate::boc::LOCATED.iter()); // (OUTFIT-UNLOCK) the talk query's GetEventFlag case (Boc's gold needle), the event-flag manager global it reads
        #[cfg(feature = "party")]
        all.push(&crate::allybody::CAN_TARGET);
        #[cfg(feature = "party")]
        all.extend(crate::mimic_boss_ash::LOCATED.iter()); // (MIMIC-ASH) effective team, adopt team, hostile / friendly relation
        all
    }

    fn known() -> Vec<&'static Known> {
        vec![
            &crate::title::SET_STATE,
            &crate::title::BEGIN_NEW_GAME,
            &crate::title::MAKE_BASE_SELECT,
            &crate::title::MAKE_TITLE_TOP,
            &crate::title::MAKE_PROFILE_LOAD,
            &crate::title::MAKE_PROFILE_DELETE,
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
        // (OUTFIT-UNLOCK) the GetEventFlag case sits at its place inside the talk query function (the rule's run-time check)
        let query = resolve_in(pe, &crate::talkkh::hooks::ENV_FN, v).unwrap_or(0);
        match resolve_in(pe, &crate::boc::LOCATED[0], v) {
            Ok(case) if query != 0 && case == query + crate::boc::FLAG_CASE_IN_FN as u32 => lines.push(format!("  {:<60} {case:#09x} (query function + {:#x})", "talk query GetEventFlag case in its function", crate::boc::FLAG_CASE_IN_FN)),
            other => problems.push(format!("talk query GetEventFlag case: {other:x?}, query function {query:#x}, expected + {:#x}", crate::boc::FLAG_CASE_IN_FN)),
        }
        // (WORLDTIME-2) the effects scene: GXFfxSceneCtrl's vtable slot 10 (its Update) is the function the play-speed
        // multiply sits in, 0xBD bytes in (both versions), and the vtable is the scene's (its RTTI locator's name)
        let vt = super::pick_for(v, crate::chrtime::SCENE_VTABLE.0, crate::chrtime::SCENE_VTABLE.1).unwrap_or(0) as u32;
        let image = pe.optional_header().ImageBase;
        let update = pe.derva_slice::<u64>(vt + 0x50, 1).ok().map(|s| s[0].wrapping_sub(image) as u32);
        let col = pe.derva_slice::<u64>(vt - 8, 1).ok().map(|s| s[0].wrapping_sub(image) as u32);
        let td = col.and_then(|c| pe.derva_slice::<u32>(c + 12, 1).ok()).map(|s| s[0]);
        let name = td.and_then(|t| pe.derva_slice::<u8>(t + 0x10, 27).ok()).map(|b| String::from_utf8_lossy(b).into_owned());
        let multiply = resolve_in(pe, &crate::chrtime::SFX_STEP, v).ok();
        if name.as_deref() != Some(".?AVGXFfxSceneCtrl@GXFFX@@\0") || update.zip(multiply).map(|(u, m)| m.wrapping_sub(u)) != Some(0xBD) {
            problems.push(format!("effects scene: vtable {vt:#x} names {name:?}, slot 10 {update:x?}, multiply {multiply:x?}"));
        } else {
            lines.push(format!("  {:<60} {vt:#09x} (RTTI, slot 10 = {:#x})", "GXFfxSceneCtrl vtable", update.unwrap_or(0)));
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
    fn sections(pe: PeFile<'_>, exec: bool) -> Vec<(u32, &[u8])> {
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

    /// (COMBAT-FIX) THE READS OF invuln.rs (Elden Ring's "cannot be damaged" state of a character) in both executables: the game's
    /// own tests of the two flags, by masked byte pattern `test byte [reg+disp32], bit` (`F6 /0`, any base register but rsp,
    /// a REX prefix allowed before it), at the SDK's offsets, found in 2.7.1.0 and in 2.7.2.0 the same number of times. The
    /// SpEffect rows are read through the game's own pointer in each SpEffect entry (`SpecialEffectEntry::param_data`, the
    /// SDK's layout: the apply / remove SpEffect functions are identical in both, port272/sdk_table.md), at the SDK's
    /// SP_EFFECT_PARAM_ST layout: 912 bytes a row = SpEffectParam's row size in both regulations (byte-identical, sha256
    /// 58cc8be709d0464e..., 11354 rows; cut rates at +0x1C .. +0x34 and +0x1D0: Rennala's barrier rows 14361 / 14368 / 14369
    /// read 0 there, build/sora_tmp/weapons/COMBAT-FIX/INVULN.md). (COMBAT-FIX follow-up) Also read from the same rows:
    /// defPlayerDmgCorrectRate physics .. dark at +0x260 .. +0x270, conditionHp +0x4, conditionHpRate +0x1F0 (the same layout;
    /// -1 = no condition in 11287 of the 11354 rows).
    #[test]
    fn invuln_reads_in_both_executables() {
        use eldenring::cs::{ChrIns, SpecialEffectEntry};
        let flags1c5 = std::mem::offset_of!(ChrIns, chr_flags1c5) as u32;
        let debug = std::mem::offset_of!(ChrIns, debug_flags) as u32;
        assert_eq!(flags1c5, 0x1C5);
        assert_eq!(std::mem::offset_of!(SpecialEffectEntry, param_data), 0);
        assert_eq!(std::mem::size_of::<eldenring::param::SP_EFFECT_PARAM_ST>(), 912, "SpEffectParam's row size in 2.7.1.0 and 2.7.2.0");
        let mut counts: Vec<(usize, usize)> = Vec::new();
        for rel in ["build/sora_tmp/er/boss/eldenring.exe", "build/sora_tmp/er/port272/eldenring_2720.exe"] {
            let Some(b) = bytes(rel) else {
                println!("INVULN offline check: no {rel}: skipped");
                continue;
            };
            let pe = PeFile::from_bytes(&b).expect("a PE file");
            let tests = |disp: u32, bit: u8| {
                let mut pat = vec![Some(0xF6u8), None];
                pat.extend(le(disp));
                pat.push(Some(bit));
                find(pe, &pat).into_iter().filter(|rva| pe.derva_slice::<u8>(rva + 1, 1).is_ok_and(|m| (0x80..=0x87).contains(&m[0]) && m[0] != 0x84)).count()
            };
            let (invincible, no_hit) = (tests(flags1c5, 0x10), tests(debug, 0x08));
            println!("INVULN offline check {rel}: `test byte [reg+{flags1c5:#x}], 10h` (invincible) x{invincible}, `test byte [reg+{debug:#x}], 08h` (no hit) x{no_hit}");
            assert!(invincible > 0 && no_hit > 0, "{rel}: the game's tests of the flags not found");
            counts.push((invincible, no_hit));
        }
        if let [a, b] = counts[..] {
            assert_eq!(a, b, "the same tests in 2.7.1.0 and 2.7.2.0");
        }
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

/// (CC-MUTE, THE USER 2026-11-07: "block mouse pointer as well") THE POINTER'S ROUTES of Elden Ring, read from the import tables
/// of both executables: user32's GetCursorPos / ScreenToClient / GetKeyState / GetKeyboardState (dinput_mask.rs detours the
/// readers at level 3) and DirectInput8Create are imported; none of the raw-input or other pointer readers is (so WM_INPUT and
/// DirectInput are the only other ways, both held already).
#[cfg(all(test, windows))]
mod pointer_imports {
    use pelite::pe64::imports::Import;
    use pelite::pe64::{Pe, PeFile};

    fn names(pe: PeFile<'_>) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let Ok(imports) = pe.imports() else { return out };
        for desc in imports {
            let dll = desc.dll_name().map(|n| n.to_string().to_ascii_lowercase()).unwrap_or_default();
            let Ok(int) = desc.int() else { continue };
            for import in int.flatten() {
                if let Import::ByName { name, .. } = import {
                    out.push((dll.clone(), name.to_string()));
                }
            }
        }
        out
    }

    #[test]
    fn cc_pointer_imports_in_both_executables() {
        let mut checked = 0;
        for rel in ["build/sora_tmp/er/boss/eldenring.exe", "build/sora_tmp/er/port272/eldenring_2720.exe"] {
            let path = [std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel), std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(rel)].into_iter().find(|p| p.is_file());
            let Some(bytes) = path.and_then(|p| std::fs::read(p).ok()) else {
                println!("POINTER offline check: no {rel}: skipped");
                continue;
            };
            let pe = PeFile::from_bytes(&bytes).expect("a PE file");
            let all = names(pe);
            let has = |dll: &str, f: &str| all.iter().any(|(d, n)| d == dll && n == f);
            for f in ["GetCursorPos", "ScreenToClient", "GetKeyState", "GetKeyboardState"] {
                assert!(has("user32.dll", f), "{rel}: user32!{f} imported ({} imports read)", all.len());
            }
            assert!(has("dinput8.dll", "DirectInput8Create"), "{rel}: DirectInput8Create imported");
            for f in ["GetRawInputData", "GetRawInputBuffer", "RegisterRawInputDevices", "GetCursorInfo", "GetPhysicalCursorPos", "GetMessagePos", "GetMouseMovePointsEx", "GetAsyncKeyState"] {
                assert!(!all.iter().any(|(_, n)| n == f), "{rel}: {f} is imported: another pointer / key route to hold");
            }
            println!("POINTER offline check {rel}: {} imports; the pointer / key readers are GetCursorPos, GetKeyState, GetKeyboardState and DirectInput", all.len());
            checked += 1;
        }
        println!("POINTER offline check: {checked} executables checked");
    }
}

/// (SAVE-DELETE) THE TITLE'S CONTINUE in both executables (slots.rs "the title's Continue"): Continue's slot is the i32
/// at `[GameDataMan + 0x60] + 0x1200`; the title menu's Continue lambdas offer it only through the "listed slot" gate
/// (`slot >= 0 && summary "in use" byte`), the Continue job loads it, a game started from the title writes it, and the
/// game's own delete clears the slot's "in use" byte and sets it to -1 when it named the deleted slot. Every piece by
/// masked pattern, its call targets followed: no recorded address is needed (the mod writes the field only).
#[cfg(all(test, windows))]
mod continue_offline {
    use pelite::pe64::{Pe, PeFile};

    fn bytes(rel: &str) -> Option<Vec<u8>> {
        [std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel), std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(rel)].into_iter().find_map(|p| std::fs::read(p).ok())
    }

    fn pattern(text: &str) -> Vec<Option<u8>> {
        text.split_whitespace().map(|b| u8::from_str_radix(b, 16).ok()).collect()
    }

    /// RVAs of `text` (`??` = any byte) in the executable sections.
    fn find(pe: PeFile, text: &str) -> Vec<u32> {
        let pat = pattern(text);
        let mut out = Vec::new();
        for s in pe.section_headers().iter().filter(|s| s.Characteristics & 0x2000_0000 != 0) {
            let Ok(b) = pe.derva_slice::<u8>(s.VirtualAddress, s.SizeOfRawData.min(s.VirtualSize.max(s.SizeOfRawData)) as usize) else { continue };
            for i in 0..b.len().saturating_sub(pat.len()) {
                if pat.iter().enumerate().all(|(k, p)| p.is_none_or(|v| b[i + k] == v)) {
                    out.push(s.VirtualAddress + i as u32);
                }
            }
        }
        out
    }

    /// The target of the `call rel32` at `rva`.
    fn call_target(pe: PeFile, rva: u32) -> Option<u32> {
        let c = pe.derva_slice::<u8>(rva, 5).ok()?;
        (c[0] == 0xE8).then(|| (rva + 5).wrapping_add(i32::from_le_bytes([c[1], c[2], c[3], c[4]]) as u32))
    }

    /// The global a `mov rax,[rip+d]` at `rva` reads.
    fn global_of(pe: PeFile, rva: u32) -> Option<u32> {
        let c = pe.derva_slice::<u8>(rva, 7).ok()?;
        (c[..3] == [0x48, 0x8b, 0x05]).then(|| (rva + 7).wrapping_add(i32::from_le_bytes([c[3], c[4], c[5], c[6]]) as u32))
    }

    fn check(pe: PeFile) -> Result<Vec<String>, String> {
        let mut lines = Vec::new();
        let one = |text: &str, what: &str| match find(pe, text).as_slice() {
            [rva] => Ok(*rva),
            other => Err(format!("{what}: {} matches {other:x?}", other.len())),
        };
        // the two GameDataMan getters: + 0x60 (the menu data) and + 0x78 (the profile summary) of ONE global
        let menu_getter = one("48 8b 05 ?? ?? ?? ?? 48 8b 40 60 c3", "the menu-data getter")?;
        let summary_getter = one("48 8b 05 ?? ?? ?? ?? 48 8b 40 78 c3", "the profile-summary getter")?;
        let global = global_of(pe, menu_getter);
        if global.is_none() || global != global_of(pe, summary_getter) {
            return Err(format!("the getters {menu_getter:#x} / {summary_getter:#x} read different globals"));
        }
        lines.push(format!("menu data = [GameDataMan {:#x} + 0x60] (getter {menu_getter:#x}); summary getter {summary_getter:#x}", global.unwrap_or(0)));
        let via_getter = |at: u32| call_target(pe, at) == Some(menu_getter);
        // the summary's "in use" reader / clearer (byte + 8 + slot) and the Continue gate built on the reader's rule
        let in_use = one("83 fa 09 77 09 48 63 c2 0f b6 44 08 08 c3", "the in-use reader")?;
        let clear = one("83 fa 09 77 0b 48 63 c2 c6 44 08 08 00 b0 01 c3", "the in-use clearer")?;
        let gate = one("40 53 48 83 ec 20 8b d9 85 c9 79 08 32 c0 48 83 c4 20 5b c3 e8", "the Continue gate")?;
        // the gate: slot < 0 -> false; else the summary getter, then the in-use reader
        let gate_calls = (call_target(pe, gate + 20) == Some(summary_getter), pe.derva_slice::<u8>(gate + 25, 16).ok().and_then(|c| c.iter().position(|b| *b == 0xE8)).and_then(|k| call_target(pe, gate + 25 + k as u32)) == Some(in_use));
        if gate_calls != (true, true) {
            return Err(format!("the Continue gate {gate:#x} does not call the summary getter then the in-use reader: {gate_calls:?}"));
        }
        lines.push(format!("in-use reader {in_use:#x}, clearer {clear:#x}, Continue gate {gate:#x}"));
        // the title menu's Continue lambdas: `call getter; mov ecx,[rax+1200h]; call gate` (three places, all to the gate)
        let lambdas = find(pe, "e8 ?? ?? ?? ?? 8b 88 00 12 00 00 e8 ?? ?? ?? ??");
        if lambdas.len() != 3 || !lambdas.iter().all(|at| via_getter(*at) && call_target(pe, at + 11) == Some(gate)) {
            return Err(format!("the Continue lambdas: {lambdas:x?}"));
        }
        // the Continue job: `call getter; mov edx,[rax+1200h]; mov rcx,rbx; call` (the slot's summary record, then its load)
        let job = one("e8 ?? ?? ?? ?? 8b 90 00 12 00 00 48 8b cb e8", "the Continue job")?;
        if !via_getter(job) {
            return Err(format!("the Continue job {job:#x} does not read the menu data"));
        }
        // a game started from the title / a load: `lastSlot = GameMan::save_slot` when it is >= 0
        let starts = find(pe, "e8 ?? ?? ?? ?? 8b d8 85 c0 78 0b e8 ?? ?? ?? ?? 89 98 00 12 00 00");
        if starts.len() != 2 || !starts.iter().all(|at| via_getter(at + 11)) {
            return Err(format!("the title start's write: {starts:x?}"));
        }
        // THE GAME'S OWN DELETE (both delete dialogs): the in-use clearer, then `if lastSlot == slot { lastSlot = -1 }`
        let deletes = find(pe, "e8 ?? ?? ?? ?? 39 98 00 12 00 00 75 0a c7 80 00 12 00 00 ff ff ff ff");
        let clears_first = |at: u32| pe.derva_slice::<u8>(at - 5, 5).ok().is_some_and(|c| c[0] == 0xE8) && call_target(pe, at - 5) == Some(clear);
        if deletes.len() != 2 || !deletes.iter().all(|at| via_getter(*at) && clears_first(*at)) {
            return Err(format!("the game's delete: {deletes:x?}"));
        }
        lines.push(format!("Continue lambdas {lambdas:x?}, job {job:#x}, title start {starts:x?}, the game's delete {deletes:x?}"));
        // NO CONTINUE = NO ITEM (the user: Continue "doesn't appear"): in the title menu's builder the gate's false JUMPS
        // OVER the whole Continue item: its make, its add to the list, and `dec r15d` (r15 = the cursor's default item,
        // -1 from the builder's start = the menu's own default). The next block (Load Game) is made and added by the
        // same calls, so the skipped block is an item add. With Continue, r15 = its index: the cursor starts on it.
        let builder = one("e8 ?? ?? ?? ?? 8b 88 00 12 00 00 e8 ?? ?? ?? ?? 4c 8d 35 ?? ?? ?? ?? 84 c0 0f 84", "the title menu's Continue item")?;
        let je = builder + 25;
        let over = pe.derva_slice::<u8>(je + 2, 4).map(|d| (je + 6).wrapping_add(i32::from_le_bytes([d[0], d[1], d[2], d[3]]) as u32)).map_err(|e| format!("the Continue item's jump: {e}"))?;
        let calls_in = |from: u32, to: u32| -> Vec<u32> { (from..to.saturating_sub(4)).filter_map(|at| call_target(pe, at)).collect() };
        let skipped = calls_in(je + 6, over);
        let next = calls_in(over, over + 0x80);
        let skipped_bytes = pe.derva_slice::<u8>(je + 6, over.saturating_sub(je + 6) as usize).map_err(|e| format!("the skipped block: {e}"))?;
        let default_cursor = skipped_bytes.windows(3).any(|w| w == [0x41, 0xff, 0xcf]);
        let shared: Vec<u32> = skipped.iter().copied().filter(|c| next.contains(c)).collect();
        if !(over > je + 0x40 && over < je + 0x200) || !default_cursor || shared.len() < 2 {
            return Err(format!("the Continue item {builder:#x}: false jumps to {over:#x}; default cursor set in the skipped block {default_cursor}; calls shared with the next item {shared:x?}"));
        }
        lines.push(format!("title menu: the gate's false at {je:#x} skips the Continue item up to {over:#x} (its add and the cursor's default; shared item calls {shared:x?})"));
        Ok(lines)
    }

    #[test]
    fn the_titles_continue_slot_is_the_same_field_in_both_executables() {
        let mut checked = 0;
        for rel in ["build/sora_tmp/er/boss/eldenring.exe", "build/sora_tmp/er/port272/eldenring_2720.exe"] {
            let Some(b) = bytes(rel) else {
                println!("CONTINUE offline check: no copy of {rel}: skipped");
                continue;
            };
            let pe = PeFile::from_bytes(&b).expect("a PE file");
            match check(pe) {
                Ok(lines) => {
                    for l in lines {
                        println!("CONTINUE offline check {rel}: {l}");
                    }
                }
                Err(e) => panic!("{rel}: {e}"),
            }
            checked += 1;
        }
        println!("CONTINUE offline check: {checked} executables checked");
    }
}

/// (EMOTE-PROMPTS) THE GESTURE RECORD THE GESTURE PROMPTS READ (gestureprompt.rs; read only, no game function is called for
/// it) in both executables, by masked pattern: the game's SetGesture (`GestureGameData` entries at +8, the id `entry >> 1`,
/// the acquired bit 0 written), the award site (`PlayerGameData + 0x8D8` is the `GestureGameData` pointer; `r8b = 1` =
/// acquired) and the load (every GestureParam row put in as `id * 2`, not acquired; 64 entries).
#[cfg(all(test, windows))]
mod gesture_offline {
    use pelite::pe64::{Pe, PeFile};

    fn bytes(rel: &str) -> Option<Vec<u8>> {
        [std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel), std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(rel)].into_iter().find_map(|p| std::fs::read(p).ok())
    }

    /// RVAs of `text` (`??` = any byte) in the executable sections.
    fn find(pe: PeFile, text: &str) -> Vec<u32> {
        let pat: Vec<Option<u8>> = text.split_whitespace().map(|b| u8::from_str_radix(b, 16).ok()).collect();
        let mut out = Vec::new();
        for s in pe.section_headers().iter().filter(|s| s.Characteristics & 0x2000_0000 != 0) {
            let Ok(b) = pe.derva_slice::<u8>(s.VirtualAddress, s.SizeOfRawData.min(s.VirtualSize.max(s.SizeOfRawData)) as usize) else { continue };
            for i in 0..b.len().saturating_sub(pat.len()) {
                if pat.iter().enumerate().all(|(k, p)| p.is_none_or(|v| b[i + k] == v)) {
                    out.push(s.VirtualAddress + i as u32);
                }
            }
        }
        out
    }

    /// SetGesture(this, id, acquired): `lea rbx, [rcx+8]` .. `mov eax, [r10]; sar eax, 1; cmp eax, edi` (0x25BD50 in both).
    const SET_GESTURE: &str = "41 56 48 83 ec 40 48 c7 44 24 20 fe ff ff ff 48 89 5c 24 50 48 89 6c 24 58 48 89 74 24 60 48 89 7c 24 68 41 0f b6 e8 8b fa 40 32 f6 45 33 c9 48 8d 59 08 4c 8b d3 41 8b 02 d1 f8 3b c7";
    /// Its found-entry write: `and dword [rcx+8], -2; movzx eax, bpl; and eax, 1; or [rcx+8], eax` (bit 0 = acquired).
    const SET_BIT: &str = "83 61 08 fe 40 0f b6 c5 83 e0 01 09 41 08";
    /// The award: `mov rcx, [rdx+8D8h]; test rcx, rcx; jz ..; mov r8b, 1; mov edx, ebx; call SetGesture`.
    const AWARD: &str = "48 8b 8a d8 08 00 00 48 85 c9 0f 84 ?? ?? ?? ?? 41 b0 01 8b d3 e8";
    /// The load's search (`sar eax, 1` over 0x40 entries) and its insert of a missing row (`lea ecx, [r8+r8]`: not acquired).
    const LOAD_SEARCH: &str = "8b 02 d1 f8 41 3b c0 74 ?? ff c1 48 83 c2 04 83 f9 40 72";
    const LOAD_INSERT: &str = "8b 08 83 e1 fe 83 f9 fe 74 ?? 48 83 c0 04 48 3b c2 75 ?? eb ?? 43 8d 0c 00 89 08";

    fn check(pe: PeFile) -> Result<Vec<String>, String> {
        let one = |text: &str, what: &str| match find(pe, text).as_slice() {
            [rva] => Ok(*rva),
            other => Err(format!("{what}: {} matches {other:x?}", other.len())),
        };
        let set = one(SET_GESTURE, "SetGesture")?;
        let bit = one(SET_BIT, "SetGesture's acquired-bit write")?;
        if !(set..set + 0x80).contains(&bit) {
            return Err(format!("the acquired-bit write {bit:#x} is not inside SetGesture {set:#x}"));
        }
        let award = one(AWARD, "the award site")?;
        let call = award + 21;
        let c = pe.derva_slice::<u8>(call, 5).map_err(|e| e.to_string())?;
        let target = (call + 5).wrapping_add(i32::from_le_bytes([c[1], c[2], c[3], c[4]]) as u32);
        if target != set {
            return Err(format!("the award site calls {target:#x}, not SetGesture {set:#x}"));
        }
        let search = one(LOAD_SEARCH, "the load's search")?;
        let insert = one(LOAD_INSERT, "the load's insert")?;
        if insert <= search || insert - search > 0x60 {
            return Err(format!("the load's search {search:#x} and insert {insert:#x} are not one function"));
        }
        let field = crate::gestureprompt::rules::GESTURE_GAME_DATA as u32;
        let disp = pe.derva_slice::<u8>(award + 3, 4).map(|d| u32::from_le_bytes([d[0], d[1], d[2], d[3]])).map_err(|e| e.to_string())?;
        if disp != field {
            return Err(format!("the award reads PlayerGameData + {disp:#x}, gestureprompt.rs reads + {field:#x}"));
        }
        Ok(vec![format!("SetGesture {set:#x} (bit write {bit:#x}), award {award:#x} reads PlayerGameData + {disp:#x}, load {search:#x} / {insert:#x}")])
    }

    #[test]
    fn gesture_record_is_the_same_in_both_executables() {
        let mut checked = 0;
        for rel in ["build/sora_tmp/er/boss/eldenring.exe", "build/sora_tmp/er/port272/eldenring_2720.exe"] {
            let Some(b) = bytes(rel) else {
                println!("GESTURE offline check: no copy of {rel}: skipped");
                continue;
            };
            let pe = PeFile::from_bytes(&b).expect("a PE file");
            match check(pe) {
                Ok(lines) => lines.iter().for_each(|l| println!("GESTURE offline check {rel}: {l}")),
                Err(e) => panic!("{rel}: {e}"),
            }
            checked += 1;
        }
        println!("GESTURE offline check: {checked} executables checked");
    }

    /// The SDK's PlayerGameData (2.7.1.0 layout, PORT-checked for 2.7.2.0): its public `storage` sits right before the
    /// private `gesture_game_data` the award site reads at 0x8D8.
    #[test]
    fn gesture_field_follows_the_sdks_storage() {
        assert_eq!(std::mem::offset_of!(eldenring::cs::PlayerGameData, storage) + 8, crate::gestureprompt::rules::GESTURE_GAME_DATA);
    }
}
