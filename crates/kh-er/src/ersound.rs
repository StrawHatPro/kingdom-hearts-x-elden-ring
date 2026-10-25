//! (LADDER2 2026-10-25) ELDEN RING'S OWN SOUNDS, PLAYED BY THE GAME AT RUN TIME. The user: "Use Torrent's wind jump sound
//! effects when jumping up the ladder." Nothing of the game's audio is read, copied or shipped: the game's own sound
//! system is asked to play one of its own events at a place, as its map events do.
//!
//! WHAT THE GAME HAS (READ in eldenring.exe 2.7.1.0 and 2.7.2.0, the research note
//! build/sora_tmp/party/LADDER2/sound/; not run in the game by an agent):
//! - EMEVD 2010[02] `PlaySE(entity, sound type, sound id)` (bank 2010's dispatcher, index 2 -> 0x14060c310) takes the
//!   entity's place and calls `play_at(CSSound, &out SoundIns, &position, &{type, id})` (0x140da7350): a one-shot sound
//!   instance at a Havok position, marked to play; the manager keeps its own reference, so the caller's handle is released
//!   at once (`release`, 0x1401b67d0), as the game does.
//! - The sound: TORRENT'S DOUBLE JUMP WIND (Torrent = c8000; its TAE 208000 plays `{1, 800007500}` (type 1 = 'c') at the
//!   second jumps a000_006111 / 6121 / 6131 / 6141 / 6201, the ones that also spawn the wind ring FFX 680025), the Wwise
//!   event `Play_c800007500` of the bank `cs_c8000.bnk` (one 1.31 s sound inside the bank). The spirit spring's launch
//!   `c800007510` plays the same sound.
//! - That bank is Torrent's character bank, loaded with c8000's resources: `bank_load(CSSound, &8000)` (0x140da6040, the
//!   game's "cs_c%04d" loader through its bank repository, reference counted) holds it loaded for us; it is asked once in
//!   the world and never let go (a missing bank only makes the sound silent: Wwise ignores an event it does not know).
//!
//! Every address is found by PATTERN and must be the version's recorded one (havok.rs `Located`; the offline check
//! gamever.rs runs them against both executables). Anything missing: no sound, logged once. Game thread only.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use glam::Vec3;

use crate::entry::exe_base;
use crate::erlog;
use crate::havok::{locate, Located};

/// The sound types (the exe's letter table `acfopsmvxbiyzegdw`: the index is the type).
pub const TYPE_CHARACTER: i32 = 1;
/// Torrent's double-jump wind (`Play_c800007500`).
pub const TORRENT_WIND_JUMP: i32 = 800_007_500;
/// The character bank that holds it (`cs_c8000.bnk`: Torrent).
pub const TORRENT_BANK: i32 = 8000;

/// Inside PlaySE's core: CSSound's static (capture 1), `play_at` (2), the handle's `release` (3).
const PAT_PLAY_SE: &str = "48 8b 0d $ { ' } 4c 8d 4c 24 20 4c 8d 45 d0 48 8d 54 24 30 e8 $ { ' } 90 48 8d 4c 24 30 e8 ? ? ? ? 85 c0 0f 95 c3 48 8d 4c 24 30 e8 $ { ' }";
/// The character bank loader (`cs_c%04d`): its start (capture 0) and the bank repository's static (capture 1).
const PAT_BANK_LOAD: &str = "40 57 48 83 ec 60 48 c7 44 24 20 fe ff ff ff 48 89 5c 24 70 48 8b 05 ? ? ? ? 48 33 c4 48 89 44 24 58 4c 8b c2 8b 02 83 f8 ff 0f 84 a8 00 00 00 85 c0 0f 84 a0 00 00 00 48 8d 54 24 28 49 8b c8 e8 ? ? ? ? 90 48 8d 5c 24 30 48 83 7c 24 48 08 48 0f 43 5c 24 30 48 8d 7c 24 30 48 0f 43 7c 24 30 48 8b 0d $ { ' }";

pub(crate) const LOCATED: [Located; 5] = [
    Located { name: "CSSound static (PlaySE)", pattern: PAT_PLAY_SE, capture: 1, rva_2_7_1_0: 0x03d8_7d48, rva_2_7_2_0: 0x03d8_ad68 },
    Located { name: "sound play at a place (PlaySE)", pattern: PAT_PLAY_SE, capture: 2, rva_2_7_1_0: 0x00da_7350, rva_2_7_2_0: 0x00da_8a40 },
    Located { name: "sound instance handle release (PlaySE)", pattern: PAT_PLAY_SE, capture: 3, rva_2_7_1_0: 0x001b_67d0, rva_2_7_2_0: 0x001b_67d0 },
    Located { name: "character sound bank load (cs_c%04d)", pattern: PAT_BANK_LOAD, capture: 0, rva_2_7_1_0: 0x00da_6040, rva_2_7_2_0: 0x00da_7730 },
    Located { name: "sound bank repository static", pattern: PAT_BANK_LOAD, capture: 1, rva_2_7_1_0: 0x03d8_8480, rva_2_7_2_0: 0x03d8_b4a0 },
];

type PlayAt = unsafe extern "C" fn(*mut c_void, *mut *mut c_void, *const Pos, *const [i32; 2]) -> *mut *mut c_void;
type Release = unsafe extern "C" fn(*mut *mut c_void);
type BankLoad = unsafe extern "C" fn(*mut c_void, *const i32);

#[repr(C, align(16))]
struct Pos([f32; 4]);

#[derive(Clone, Copy)]
struct Functions {
    sound_static: usize,
    play_at: usize,
    release: usize,
    bank_load: usize,
    repository_static: usize,
}

static FUNCTIONS: OnceLock<Option<Functions>> = OnceLock::new();
static BANK_HELD: AtomicBool = AtomicBool::new(false);
static TOLD: AtomicBool = AtomicBool::new(false);

fn functions() -> Option<Functions> {
    *FUNCTIONS.get_or_init(|| {
        let f = Functions { sound_static: locate(&LOCATED[0])?, play_at: locate(&LOCATED[1])?, release: locate(&LOCATED[2])?, bank_load: locate(&LOCATED[3])?, repository_static: locate(&LOCATED[4])? };
        erlog!("sound: the game's own sound play (PlaySE's play_at / release) and the character bank loader located at the recorded RVAs (base {:#x})", exe_base());
        Some(f)
    })
}

/// A static of the executable holding a pointer: its value (null: not made yet).
fn read_static(at: usize) -> *mut c_void {
    // SAFETY: `at` is an address inside the running image (located above); the static is a plain pointer word.
    unsafe { std::ptr::read_volatile(at as *const *mut c_void) }
}

/// Torrent's bank is held loaded (once in the world; the game would otherwise load it only with Torrent).
fn hold_bank(f: &Functions, sound: *mut c_void) {
    if BANK_HELD.load(Ordering::Relaxed) || read_static(f.repository_static).is_null() {
        return;
    }
    BANK_HELD.store(true, Ordering::Relaxed);
    let bank = TORRENT_BANK;
    // SAFETY: the game's own loader (bytes checked for this version), game thread, its sound manager and bank repository
    // both made (non-null); the bank number is read through the pointer only during the call.
    unsafe { std::mem::transmute::<usize, BankLoad>(f.bank_load)(sound, &bank) };
    erlog!("sound: Torrent's sound bank (cs_c{TORRENT_BANK:04}) held loaded for the ladder jump's wind (the game's own loader)");
}

/// PLAYS ONE OF THE GAME'S SOUNDS at an Elden Ring (Havok) position, once. False: not available now (logged once).
pub fn play_at(kind: i32, id: i32, at: Vec3) -> bool {
    let Some(f) = functions() else {
        if !TOLD.swap(true, Ordering::Relaxed) {
            erlog!("!!! sound: the game's own sound play is not located for this executable: the ladder jump is silent");
        }
        return false;
    };
    let sound = read_static(f.sound_static);
    if sound.is_null() || !at.is_finite() {
        return false;
    }
    hold_bank(&f, sound);
    let pos = Pos([at.x, at.y, at.z, 1.0]);
    let sid = [kind, id];
    let mut handle: *mut c_void = std::ptr::null_mut();
    // SAFETY: the game's own PlaySE path (bytes checked for this version), game thread, its sound manager made; the
    // position and id are read during the call; the handle we get is released at once as the game's caller does.
    unsafe {
        std::mem::transmute::<usize, PlayAt>(f.play_at)(sound, &mut handle, &pos, &sid);
        if !handle.is_null() {
            std::mem::transmute::<usize, Release>(f.release)(&mut handle);
        }
    }
    true
}

/// THE LADDER JUMP'S SOUND: Torrent's double-jump wind at the jumper's feet (Elden Ring position).
pub fn ladder_jump(at: Vec3) -> bool {
    play_at(TYPE_CHARACTER, TORRENT_WIND_JUMP, at)
}
