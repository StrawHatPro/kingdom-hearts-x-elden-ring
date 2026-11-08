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

/// (SPIRITSPRING) The spirit spring's own launch whoosh: Torrent's high jump c8000 a000_006200 / 6205 plays `{1, 800007510}`
/// at 0.233 s (TAE 208000, LADDER2 sound/c8000_tae_sound.txt), the event `Play_c800007510` of the same bank `cs_c8000.bnk`.
pub const TORRENT_SPRING_LAUNCH: i32 = 800_007_510;

/// (SPIRITSPRING) THE SPRING'S LAUNCH SOUND at a place (Elden Ring position): the world answering the jump.
pub fn spring_launch(at: Vec3) -> bool {
    play_at(TYPE_CHARACTER, TORRENT_SPRING_LAUNCH, at)
}

// ------------------------------------------------------------------------------------------ (ER-TWEAKS) the Tarnished's voice
//
// THE USER (2026-11-06): "mute all of the voice sounds made by the tarnished". State note:
// build/sora_tmp/weapons/ER-TWEAKS/STATE.md, item 2.
//
// HOW ELDEN RING VOICES A CHARACTER (READ in eldenring.exe 2.7.1.0 and 2.7.2.0 with iced-x86; not run in the game by an
// agent): every grunt, effort shout, pain cry, fall and death scream of the player is a TAE event 129 of type 15 = 'd' (the
// letter table `acfopsmvxbiyzegdw`), a Wwise DIALOGUE event (`Dialogue_%s`, its builder 0xD9E800) whose arguments (the
// character's voice type, read from its sound module ChrIns.modules + 0xA0) pick the clip. In c0000.anibnd (all 40+ TAE
// files) there are 49,506 such events, ids d000000000 / 100 / 200 (efforts by strength), 400 (damage), 6000 / 6200
// (deaths, falls), every one with slot -1 (the one-shot path). The TAE sound handler (0x42AF80, case 128/129 of the event
// switch 0x42F6A0) sends a sound whose type resolves to 15 to ONE function, `chr_dialogue(out, ChrIns, &{type, id},
// &dummy poly, attach)` (0x4AD460; 2.7.2.0 0x4AD540), and every other type to the chr play (0x4AD680). That function is
// called from nowhere else (one caller in each version), clears `*out` first and returns `out` untouched on each of its
// own refusals (no character, no sound module, not type 15, ...); its caller releases the handle, null or not
// (0x1B67D0 tests it). The other users of the dialogue player (0xDC21A0: the hit sounds' material switches) are no voices.
//
// THE RULE: that one function is detoured; when its character is THE MAIN PLAYER (WorldChrMan.main_player = the hidden
// Tarnished, whoever leads) it refuses the voice as the game's own early outs do (`*out = 0`, nothing made, nothing
// posted to Wwise). Every other character's voice (NPCs, invaders, enemies, the Mimic Tear: their own ChrIns) and every
// other sound of the Tarnished (steps, armour, swings, Torrent) are untouched. Key `tarnished.voice_mute = 0` gives the
// voice back (read every frame).

/// The sound type 'd' (index 15 of `acfopsmvxbiyzegdw`): a character's voice, a Wwise dialogue event.
pub const TYPE_DIALOGUE: i32 = 15;

// (ER-TWEAKS follow-up 2026-11-07, the coordinator: "the user said ALL the Tarnished's voices") TWO GAPS CLOSED:
// - THE SPOKEN LINE 'v' (type 7): c0000's TAE holds ONE such event (animation 60280: `v019690225`, 0.000 - 2.067 s; every
//   c0000 TAE dumped, types counted 0: 989, 1: 26218, 5: 7296, 7: 1, 8: 37772, 9: 206, 15: 49506). It takes the
//   character sound play (2.7.1.0 0x4AD680, 2.7.2.0 0x4AD760: the same `(out, ChrIns, &{type, id}, &dummy poly, attach)`
//   shape, `*out = 0` first, the same "no sound" early out), not the dialogue play. Its 8 callers are the TAE handlers
//   (0x42AF80 twice, 0x42B440, 0x42BFC0), the character's sound slots (0x4ADC50, three) and 0x4A3DF0: no talk script plays
//   through it (an NPC's spoken lines are its own character's). That play is detoured too, for the voice types only.
// - THE STAND-IN BODIES: every Elden Ring body that stands in for the player or our party is muted the same way: the main
//   player (the hidden Tarnished, whoever leads) and every body of the ally pool (allybody.rs `is_body`: Donald's, Goofy's,
//   AI Sora's, Mimic Sora's; whatever their route: a Spirit Ash's row, the `c0000` copy of the player's model, a copy of an
//   enemy). Lock-free (`is_body` reads atomics), so it is safe on the characters' task threads. The real Mimic Tear boss,
//   NPCs and enemies are not stand-ins: their voices stay.

/// The sound type 'v' (index 7): a spoken line.
pub const TYPE_VOICE_LINE: i32 = 7;

/// A voice: the dialogue (grunts, pain, deaths) or a spoken line.
pub fn is_voice(kind: i32) -> bool {
    kind == TYPE_DIALOGUE || kind == TYPE_VOICE_LINE
}

/// The character voice play the TAE sound handler calls for type 15 (capture 0 = its start).
const PAT_CHR_VOICE: &str = "48 89 4c 24 08 55 56 57 41 54 41 55 41 56 41 57 48 8d 6c 24 e1 48 81 ec e0 00 00 00 48 c7 45 cf fe ff ff ff 48 89 9c 24 30 01 00 00 4d 8b f9 49 8b f0 48 8b fa 4c 8b f1 33 c0 89 44 24 40 48 89 01 c7 44 24 40 01 00 00 00 48 85 d2 0f 84 ? ? ? ? 48 8b 82 90 01 00 00 48 83 b8 a0 00 00 00 00 0f 84 ? ? ? ? 49 8b 00 48 8b c8 48 c1 e9 20 39 0d ? ? ? ? 75 ? 39 05 ? ? ? ? 0f 84 ? ? ? ? 83 f8 0f 0f 85";

pub(crate) const VOICE_FN: Located = Located { name: "character voice play (TAE type 15, ER-TWEAKS)", pattern: PAT_CHR_VOICE, capture: 0, rva_2_7_1_0: 0x004a_d460, rva_2_7_2_0: 0x004a_d540 };

/// (ER-TWEAKS follow-up) The character sound play (every TAE sound type but 15; capture 0 = its start): prologue, `*out = 0`,
/// the sound id against the "none" constant (its address masked), the attach flag (the fifth argument) tested.
const PAT_CHR_PLAY: &str = "48 8b c4 48 89 48 08 56 57 41 56 48 81 ec 90 00 00 00 48 c7 40 80 fe ff ff ff 48 89 58 10 48 89 68 20 4d 8b f1 49 8b f0 48 8b fa 48 8b d9 33 ed 89 6c 24 20 48 89 29 c7 44 24 20 01 00 00 00 49 8b 00 48 8b d0 48 c1 ea 20 39 15 ? ? ? ? 75 0c 39 05 ? ? ? ? 0f 84 ? ? ? ? 80 bc 24 d0 00 00 00 00 0f 84";

pub(crate) const PLAY_FN: Located = Located { name: "character sound play (TAE types but 15, ER-TWEAKS)", pattern: PAT_CHR_PLAY, capture: 0, rva_2_7_1_0: 0x004a_d680, rva_2_7_2_0: 0x004a_d760 };

// (ER-HITSFX round 2, THE USER 2026-11-07: "I'm still hearing the characters make Elden Ring noises, I even heard the
// tarnished death moan when I teleported") EVERY ELDEN RING SOUND OF A STAND-IN, not only its voice. The cause: the
// character sound play refused only the spoken line 'v', so the stand-ins' steps ('x'), armour ('b'), swings and
// whooshes, landings, rolls ('c') and every other TAE sound played (the ally bodies are `buddy c0000` copies of the
// player's model: the Tarnished's sounds); and a body that LEFT the pool (given up / parked / dead on a teleport or a
// load) was no longer known while its death animation still cried ('d'). Now:
// - both character plays refuse EVERY sound type of a stand-in (standin.rs `classify`: the main player, the ally-pool
//   bodies, the Mimic boss bodies, and any of them that left within the last 20 s, by object + handle);
// - the one exception the user kept: inside a hit apply whose attacker is NOT a stand-in (an enemy hitting a KH
//   character's body) nothing is refused (Elden Ring's hit sounds of the enemy's blow);
// - keys: `tarnished.voice_mute = 0` gives the voices back ('d', 'v'), `standin.er_sound_mute = 0` every other type;
// - the diagnostic (`snd.er_diag`, on): every distinct sound refused from a stand-in is logged once (its id, its kind of
//   stand-in, the function), and every distinct Elden Ring sound made OUTSIDE a character play (the core sound maker,
//   0xDAD650 / 0xDAED40, its callers' return addresses) once, up to a cap: the next session shows any path left.

/// THE PURE RULE OF A CHARACTER PLAY (both detours): refused when its character is a stand-in, outside an enemy's hit on
/// one of ours; a voice ([`is_voice`]) by the voice key, every other type by the stand-in sound key.
pub fn mute_chr_sound(voice_on: bool, sound_on: bool, stand_in: bool, enemy_hit: bool, kind: i32) -> bool {
    stand_in && !enemy_hit && if is_voice(kind) { voice_on } else { sound_on }
}

type ChrVoice = unsafe extern "C" fn(*mut usize, usize, *const [i32; 2], *const i32, usize) -> *mut usize;

static ORIG_VOICE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static VOICE_MUTE: AtomicBool = AtomicBool::new(true);
/// (ER-HITSFX round 2) Every other sound type of a stand-in (`standin.er_sound_mute`).
static SOUND_MUTE: AtomicBool = AtomicBool::new(true);
/// WorldChrMan (the game's lasting singleton), taken on the game thread: the detour runs on the characters' task threads.
static WCM: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static MUTED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static LAST_MUTED: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);
static LAST_KIND: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(TYPE_DIALOGUE); // (ER-TWEAKS follow-up)
static ORIG_PLAY: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0); // (ER-TWEAKS follow-up)
static MUTED_TOLD: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// The main player's ChrIns now (0: none).
fn main_player() -> usize {
    let wcm = WCM.load(Ordering::Relaxed);
    if !crate::game::plausible(wcm) {
        return 0;
    }
    // SAFETY: WorldChrMan is the game's singleton (taken plausible on the game thread, never freed while the game runs);
    // `main_player` is one pointer word, read as a plain value (no reference is made).
    unsafe { std::ptr::read_volatile((wcm + std::mem::offset_of!(eldenring::cs::WorldChrMan, main_player)) as *const usize) }
}

/// (ER-TWEAKS) Both detours' test: a stand-in's voice is refused as the game's own early outs do (`*out = 0`: no
/// instance, the caller releases a null handle). True: refused.
///
/// # Safety
/// `out` and `sid` are the game's arguments (null-checked here).
unsafe fn refused(out: *mut usize, chr: usize, sid: *const [i32; 2], via: &'static str) -> bool {
    let [kind, id] = if sid.is_null() { [-1, -1] } else { std::ptr::read_unaligned(sid) };
    if out.is_null() {
        return false;
    }
    let who = crate::standin::classify(chr, main_player());
    let enemy_hit = in_enemy_hit();
    let mute = mute_chr_sound(VOICE_MUTE.load(Ordering::Relaxed), SOUND_MUTE.load(Ordering::Relaxed), who.is_some(), enemy_hit, kind);
    if let Some(w) = who {
        diag_stand_in(w, kind, id, via, mute, enemy_hit);
    }
    if !mute {
        return false;
    }
    std::ptr::write(out, 0);
    MUTED.fetch_add(1, Ordering::Relaxed);
    LAST_MUTED.store(id, Ordering::Relaxed);
    LAST_KIND.store(kind, Ordering::Relaxed);
    true
}

unsafe extern "C" fn voice_hook(out: *mut usize, chr: usize, sid: *const [i32; 2], dummy: *const i32, attach: usize) -> *mut usize {
    if refused(out, chr, sid, "the character voice play") {
        return out;
    }
    let _ctx = ChrCtx::enter(); // (ER-HITSFX round 2) the core maker's diagnostic: a character's own sound
    let original: ChrVoice = std::mem::transmute(ORIG_VOICE.load(Ordering::Relaxed));
    original(out, chr, sid, dummy, attach)
}

/// (ER-TWEAKS follow-up; ER-HITSFX round 2) The character sound play: every sound of a stand-in is refused (steps, armour,
/// swings, landings, its spoken line ...); every other character's goes to the game.
unsafe extern "C" fn play_hook(out: *mut usize, chr: usize, sid: *const [i32; 2], dummy: *const i32, attach: usize) -> *mut usize {
    if refused(out, chr, sid, "the character sound play") {
        return out;
    }
    let _ctx = ChrCtx::enter();
    let original: ChrVoice = std::mem::transmute(ORIG_PLAY.load(Ordering::Relaxed));
    original(out, chr, sid, dummy, attach)
}

/// (entry.rs) Detours the character voice play (once per payload; undone by the renderer's shutdown with every other
/// inline hook).
pub fn install() {
    install_hit_se(); // (ER-HITSFX) the KH attacks' Elden Ring hit sounds muted
    install_diag(); // (ER-HITSFX round 2) the core sound maker's diagnostic
    let Some(at) = locate(&VOICE_FN) else {
        erlog!("sound: (ER-TWEAKS) the character voice play is not located: the Tarnished's voice is NOT muted");
        return;
    };
    // SAFETY: the function's start at its located address (pattern + recorded RVA); our hook has its signature (four
    // register arguments, the fifth on the stack, passed through unchanged).
    match unsafe { kh_er_render::inline_hook(at, voice_hook as *const () as usize, &ORIG_VOICE, "character voice play") } {
        Ok(()) => erlog!("sound: (ER-TWEAKS) the character voice play is detoured: the hidden Tarnished's voice (type 'd') is muted (tarnished.voice_mute = 0: Elden Ring's)"),
        Err(e) => {
            ORIG_VOICE.store(0, Ordering::SeqCst);
            erlog!("sound: (ER-TWEAKS) the character voice play was NOT detoured ({e}): the Tarnished's voice stays");
        }
    }
    // (ER-TWEAKS follow-up) the character sound play, for the spoken lines ('v') of the stand-in bodies
    let Some(at) = locate(&PLAY_FN) else {
        erlog!("sound: (ER-TWEAKS) the character sound play is not located: the Tarnished's spoken line ('v') is NOT muted");
        return;
    };
    // SAFETY: as above: the located start of a function of the same signature.
    match unsafe { kh_er_render::inline_hook(at, play_hook as *const () as usize, &ORIG_PLAY, "character sound play") } {
        Ok(()) => erlog!("sound: (ER-TWEAKS, ER-HITSFX round 2) the character sound play is detoured: EVERY sound of the stand-in bodies (the hidden Tarnished, the ally-pool bodies, the Mimic boss bodies, and those that left within {} s) is muted (standin.er_sound_mute = 0: Elden Ring's; voices: tarnished.voice_mute)", crate::standin::GRACE_MS / 1000),
        Err(e) => {
            ORIG_PLAY.store(0, Ordering::SeqCst);
            erlog!("sound: (ER-TWEAKS) the character sound play was NOT detoured ({e}): the Tarnished's spoken line stays");
        }
    }
}

pub fn uninstalled() {
    ORIG_VOICE.store(0, Ordering::SeqCst);
    ORIG_PLAY.store(0, Ordering::SeqCst); // (ER-TWEAKS follow-up)
    ORIG_HIT_SE.store(0, Ordering::SeqCst); // (ER-HITSFX)
    ORIG_HIT_CHR.store(0, Ordering::SeqCst);
    ORIG_HIT_OBJ.store(0, Ordering::SeqCst);
    ORIG_CREATE.store(0, Ordering::SeqCst); // (ER-HITSFX round 2)
    WCM.store(0, Ordering::SeqCst);
}

/// (frame.rs, FRAME_BEGIN: every frame) The key, WorldChrMan for the detour, the log.
pub fn frame(control: &crate::control::Control) {
    hit_se_frame(control); // (ER-HITSFX)
    let on = control.flag("tarnished.voice_mute") != Some(false);
    if VOICE_MUTE.swap(on, Ordering::Relaxed) != on {
        erlog!("sound: (ER-TWEAKS) the Tarnished's voice: {}", if on { "MUTED" } else { "Elden Ring's own (tarnished.voice_mute = 0)" });
    }
    if WCM.load(Ordering::Relaxed) == 0 {
        use fromsoftware_shared::FromStatic;
        if let Ok(p) = eldenring::cs::WorldChrMan::instance_ptr() {
            WCM.store(p as usize, Ordering::Relaxed);
        }
    }
    // (ER-HITSFX round 2) the stand-ins (the main player remembered over a load; the Mimic boss bodies), the keys
    crate::standin::frame(main_player());
    let on = control.flag("standin.er_sound_mute") != Some(false);
    if SOUND_MUTE.swap(on, Ordering::Relaxed) != on {
        erlog!("sound: (ER-HITSFX round 2) every Elden Ring sound of the stand-in bodies: {}", if on { "MUTED" } else { "Elden Ring's own (standin.er_sound_mute = 0)" });
    }
    DIAG.store(control.flag("snd.er_diag") != Some(false), Ordering::Relaxed);
    diag_flush();
    let muted = MUTED.load(Ordering::Relaxed);
    let told = MUTED_TOLD.load(Ordering::Relaxed);
    if muted != told && (told == 0 || muted >= told.saturating_mul(10)) {
        MUTED_TOLD.store(muted, Ordering::Relaxed);
        erlog!("sound: (ER-TWEAKS) {muted} voice sound(s) of the Tarnished muted so far (the last: {}{:09}; the player's and the party's stand-in bodies)", crate::titlemusic::letter(LAST_KIND.load(Ordering::Relaxed)), LAST_MUTED.load(Ordering::Relaxed));
    }
}

// ------------------------------------------------------------------------------- (ER-HITSFX) the KH attacks' Elden Ring hit sounds
//
// THE USER (2026-11-07): "mute all of the Elden Ring combat sounds made by the KH characters (Elden Ring's weapon hit SFX)
// it should only be KH sound effects they make when fighting. Enemies hitting them should still use the Elden Ring SFX
// though." State note: build/sora_tmp/er_hitsfx/STATE.md.
//
// HOW OUR HITS MADE ELDEN RING SOUNDS: every KH hit reaches the game as an invisible bullet (bullet.rs: owner = the main
// player for the leader's hits, an ally-pool body for a friend's / Mimic Sora's / a staged friend's), whose own bullet
// sounds are already cleared (`se_id_bullet1/2 = -1`). But when it HITS, the game's hit routine plays the MATERIAL HIT
// SOUND of the attack against what it hit (the clang / slash / thud by material), whatever the bullet row says.
// READ in eldenring.exe 2.7.1.0 and 2.7.2.0 (iced-x86; not run in the game by an agent):
// - the material hit-sound play `hit_se(out, &position, kind, ?, a5 .. a14)` (2.7.1.0 0xDC21A0, 2.7.2.0 0xDC3890): `*out`
//   cleared first; it refuses (`*out = 0`, returns `out`) when there is no sound manager, the kind is 6, the SE material
//   (a7) is <= 0 or 139, the kind's name is "None", or the place is out of earshot. Its only callers: the CHARACTER hit
//   apply (0x6BCF00 / 0x6BD3B0, the call at +0x6EF), the OBJECT hit apply (0x709890 / 0x709D40, at +0x911), and two map-hit
//   paths of a character's own melee (0x440440, 0x44A470; no KH hit is a character's melee: not hooked).
// - the two hit applies `(target, &hit, flag)` are called by the three attacker dispatchers (bullets 0x38B060 / 0x38B620,
//   characters' attacks 0x44AC90 / 0x44B4D0, 0x6DEEF0 / 0x6DF140) with the HIT record: +0x1A0 the position, +0x220 / +0x222
//   the SE materials (a guard puts the guard's material there at the apply's start), +0x224 / +0x226 the effect materials,
//   +0x1D8 THE ATTACKER: an object whose virtual +0x10 gives its kind, 1 = a character (its handle at +8), 3 = a bullet
//   (its handle at +8; the field-ins-by-handle lookup 0x62EEB0 / 0x62F360, then the bullet's owner handle at +0x4C0). That
//   is the game's own derivation of the attacker inside the character hit apply (0x6BD09F .. 0x6BD0EA).
//
// THE RULE: both hit applies are detoured; when the hit's attacker is ONE OF OUR STAND-INS (the main player = the hidden
// Tarnished whoever leads, or a body of the ally pool: ER-TWEAKS's stand-ins, see `mute_voice`), the apply runs as always
// but the material hit-sound play refuses inside it, on that thread only, as its own early outs do. Everything else of
// the hit is the game's (damage, poise, reaction, hit effects / sparks, the enemy's hurt cry from its own animation).
// Generic: every attack, every material, every enemy and object, no table. An ENEMY's hit on a KH character's body has
// the enemy as its attacker: Elden Ring's sound stays. Key `hit.er_se_mute = 0` gives the game's hit sounds back.

/// The material hit-sound play (capture 0 = its start).
const PAT_HIT_SE: &str = "48 8b c4 48 89 48 08 55 41 54 41 55 41 56 41 57 48 8d 68 d1 48 81 ec b0 00 00 00 48 c7 45 e7 fe ff ff ff 48 89 58 10 48 89 70 18 48 89 78 20 0f 29 70 c8 45 8b e1 45 8b f0 4c 8b fa 48 8b f9 45 33 ed 44 89 6d a7 4c 89 6d 9f 4c 39 2d ? ? ? ? 75 ? 48 8b 4d 9f 48 89 0f 48 85 c9 74 ? 48 83 c1 08 e8 ? ? ? ? 90 e9 ? ? ? ? 8b 5d 67 8b 75 6f 80 bd 8f 00 00 00 00 74 ? 85 db 78 ? b8 1f 85 eb 51 f7 eb c1 fa 05";
/// The character hit apply (capture 0 = its start): its guard-material override of the hit's +0x220 .. +0x226 included.
const PAT_HIT_CHR: &str = "48 8b c4 55 56 57 41 54 41 55 41 56 41 57 48 8d a8 08 fb ff ff 48 81 ec c0 05 00 00 48 c7 45 d0 fe ff ff ff 48 89 58 18 0f 29 70 b8 48 8b 05 ? ? ? ? 48 33 c4 48 89 85 a0 04 00 00 45 0f b6 e8 4c 8b fa 4c 8b f1 33 c0 8b f8 89 45 90 0f b6 92 65 02 00 00 48 83 c1 18 e8 ? ? ? ? 49 83 cc ff 85 c0 78 ? 66 41 89 87 20 02 00 00 66 45 89 a7 22 02 00 00";
/// The object hit apply (capture 0 = its start).
const PAT_HIT_OBJ: &str = "48 8b c4 55 56 57 41 54 41 55 41 56 41 57 48 8d a8 28 fa ff ff 48 81 ec a0 06 00 00 48 c7 45 c0 fe ff ff ff 48 89 58 18 0f 29 70 b8 0f 29 78 a8 44 0f 29 40 98 44 0f 29 48 88 44 0f 29 90 78 ff ff ff 44 0f 29 98 68 ff ff ff 44 0f 29 a0 58 ff ff ff 48 8b 05 ? ? ? ? 48 33 c4 48 89 85 20 05 00 00 4c 8b fa 4c 8b f1 33 ff 89 7d 80 48 83 cb ff 83 79 10 01 75 ? 48 8b 49 18";
/// The field-ins-by-handle lookup (the handle's category in bits 28..31; capture 0 = its start).
const PAT_INS_BY_HANDLE: &str = "48 89 4c 24 08 48 83 ec 28 48 8b d1 48 c1 ea 1c 83 e2 0f 0f 84 ? ? ? ? 83 ea 01 0f 84 ? ? ? ? 83 ea 02 0f 84 ? ? ? ? 83 ea 03 74 ?";

pub(crate) const HIT_LOCATED: [Located; 4] = [
    Located { name: "material hit-sound play (ER-HITSFX)", pattern: PAT_HIT_SE, capture: 0, rva_2_7_1_0: 0x00dc_21a0, rva_2_7_2_0: 0x00dc_3890 },
    Located { name: "character hit apply (ER-HITSFX)", pattern: PAT_HIT_CHR, capture: 0, rva_2_7_1_0: 0x006b_cf00, rva_2_7_2_0: 0x006b_d3b0 },
    Located { name: "object hit apply (ER-HITSFX)", pattern: PAT_HIT_OBJ, capture: 0, rva_2_7_1_0: 0x0070_9890, rva_2_7_2_0: 0x0070_9d40 },
    Located { name: "field ins by handle (ER-HITSFX)", pattern: PAT_INS_BY_HANDLE, capture: 0, rva_2_7_1_0: 0x0062_eeb0, rva_2_7_2_0: 0x0062_f360 },
];

/// The hit record's attacker object.
const HIT_ATTACKER: usize = 0x1d8;
/// A bullet's owner handle.
const BULLET_OWNER: usize = 0x4c0;
/// The attacker object's kinds (its virtual +0x10).
pub const ATTACKER_CHR: i32 = 1;
pub const ATTACKER_BULLET: i32 = 3;

/// THE PURE RULE OF THE ATTACKER: (its kind, the handle at its +8, a bullet's owner handle) -> the attacking character's
/// handle, as the game derives it (a handle whose low word is -1 is none).
pub fn attacker_of(kind: i32, own: u64, bullet_owner: Option<u64>) -> Option<u64> {
    let h = match kind {
        ATTACKER_CHR => own,
        ATTACKER_BULLET => bullet_owner?,
        _ => return None,
    };
    (h as u32 != u32::MAX && h != 0).then_some(h)
}

/// THE PURE RULE OF THE HIT SOUND: refused when the mute is on and the attacker is a stand-in: the main player (`player`;
/// 0 = none now) or a body of the ally pool (`party_body`).
pub fn mute_hit_se(on: bool, attacker: Option<u64>, player: u64, party_body: bool) -> bool {
    on && attacker.is_some_and(|a| (player != 0 && a == player) || party_body)
}

type HitApply = unsafe extern "C" fn(usize, usize, usize, usize) -> usize;
type HitSe = unsafe extern "C" fn(*mut usize, usize, usize, usize, usize, usize, usize, usize, usize, usize, usize, usize, usize, usize) -> *mut usize;
type InsByHandle = unsafe extern "C" fn(u64) -> usize;
type Kind = unsafe extern "C" fn(usize) -> i32;

static ORIG_HIT_SE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ORIG_HIT_CHR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ORIG_HIT_OBJ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static INS_BY_HANDLE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static HIT_MUTE: AtomicBool = AtomicBool::new(true);
static HIT_MUTED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static HIT_MUTED_TOLD: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

std::thread_local! {
    /// Inside a hit apply whose attacker is a stand-in (a depth: the applies never nest in practice).
    static OUR_HIT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Leaves the "our hit" state of this thread however the apply returns.
struct OurHit;

impl OurHit {
    fn enter() -> OurHit {
        OUR_HIT.with(|c| c.set(c.get() + 1));
        OurHit
    }
}

impl Drop for OurHit {
    fn drop(&mut self) {
        OUR_HIT.with(|c| c.set(c.get().saturating_sub(1)));
    }
}

/// Inside a stand-in's hit apply on this thread.
fn in_our_hit() -> bool {
    OUR_HIT.with(|c| c.get()) > 0
}


/// The attacker of the game's hit record, as the character hit apply derives it.
///
/// # Safety
/// `hit` is the hit record the game passes to its hit apply (read only; checked plausible), on that game thread.
unsafe fn hit_attacker(hit: usize) -> Option<u64> {
    if !crate::game::plausible(hit) {
        return None;
    }
    let obj = std::ptr::read_unaligned((hit + HIT_ATTACKER) as *const usize);
    if !crate::game::plausible(obj) {
        return None;
    }
    let vtable = std::ptr::read_unaligned(obj as *const usize);
    let kind_at = if crate::game::plausible(vtable) { std::ptr::read_unaligned((vtable + 0x10) as *const usize) } else { 0 };
    if kind_at == 0 {
        return None;
    }
    let kind = std::mem::transmute::<usize, Kind>(kind_at)(obj);
    let own = std::ptr::read_unaligned((obj + 8) as *const u64);
    let owner = if kind == ATTACKER_BULLET {
        let lookup = INS_BY_HANDLE.load(Ordering::Relaxed);
        let bullet = if lookup == 0 { 0 } else { std::mem::transmute::<usize, InsByHandle>(lookup)(own) };
        crate::game::plausible(bullet).then(|| std::ptr::read_unaligned((bullet + BULLET_OWNER) as *const u64))
    } else {
        None
    };
    attacker_of(kind, own, owner)
}

/// Both hit applies' test: their attacker is a stand-in (the hit sound inside is refused).
///
/// # Safety
/// As [`hit_attacker`].
unsafe fn ours(hit: usize) -> bool {
    if !HIT_MUTE.load(Ordering::Relaxed) {
        return false;
    }
    let attacker = hit_attacker(hit);
    // (ER-HITSFX round 2) every stand-in by handle (standin.rs: the main player, the pool bodies, the Mimic boss bodies,
    // those that left)
    mute_hit_se(true, attacker, crate::standin::player_handle(), attacker.is_some_and(|a| crate::standin::classify_handle(a).is_some()))
}

unsafe extern "C" fn hit_chr_hook(target: usize, hit: usize, flag: usize, r9: usize) -> usize {
    let ours = ours(hit);
    let _ours = ours.then(OurHit::enter);
    // (ER-HITSFX round 2) an ENEMY's hit on a stand-in (the target, a live character): every sound inside is kept
    let _enemy = (!ours && crate::standin::classify(target, main_player()).is_some()).then(EnemyHit::enter);
    let original: HitApply = std::mem::transmute(ORIG_HIT_CHR.load(Ordering::Relaxed));
    original(target, hit, flag, r9)
}

unsafe extern "C" fn hit_obj_hook(target: usize, hit: usize, r8: usize, r9: usize) -> usize {
    let _ours = ours(hit).then(OurHit::enter);
    let original: HitApply = std::mem::transmute(ORIG_HIT_OBJ.load(Ordering::Relaxed));
    original(target, hit, r8, r9)
}

/// The material hit-sound play: refused inside a stand-in's hit apply, as its own early outs do (`*out = 0`).
unsafe extern "C" fn hit_se_hook(out: *mut usize, pos: usize, kind: usize, a4: usize, a5: usize, a6: usize, a7: usize, a8: usize, a9: usize, a10: usize, a11: usize, a12: usize, a13: usize, a14: usize) -> *mut usize {
    if !out.is_null() && in_our_hit() {
        std::ptr::write(out, 0);
        HIT_MUTED.fetch_add(1, Ordering::Relaxed);
        return out;
    }
    let original: HitSe = std::mem::transmute(ORIG_HIT_SE.load(Ordering::Relaxed));
    original(out, pos, kind, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14)
}

/// (install) The hit-sound filter: the sound play first, then the two applies (an apply hooked without the play would
/// only mark threads).
fn install_hit_se() {
    let at: Vec<Option<usize>> = HIT_LOCATED.iter().map(locate).collect();
    let [Some(se), Some(chr), Some(obj), Some(lookup)] = at[..] else {
        erlog!("sound: (ER-HITSFX) the hit functions are not all located: the KH attacks' Elden Ring hit sounds are NOT muted");
        return;
    };
    INS_BY_HANDLE.store(lookup, Ordering::SeqCst);
    // SAFETY: the located starts (pattern + recorded RVA) of functions of these signatures (the sound play's ten stack
    // arguments passed through unchanged; the applies' four register arguments).
    let hooked = unsafe {
        kh_er_render::inline_hook(se, hit_se_hook as *const () as usize, &ORIG_HIT_SE, "material hit-sound play")
            .and_then(|()| kh_er_render::inline_hook(chr, hit_chr_hook as *const () as usize, &ORIG_HIT_CHR, "character hit apply"))
            .and_then(|()| kh_er_render::inline_hook(obj, hit_obj_hook as *const () as usize, &ORIG_HIT_OBJ, "object hit apply"))
    };
    match hooked {
        Ok(()) => erlog!("sound: (ER-HITSFX) the hit applies and the material hit-sound play are detoured: the KH characters' hits make no Elden Ring hit sound (enemies' hits keep theirs; hit.er_se_mute = 0: Elden Ring's)"),
        Err(e) => erlog!("sound: (ER-HITSFX) the hit-sound filter was NOT fully detoured ({e}): some KH hits may still make Elden Ring's hit sound"),
    }
}

/// (frame) The key, the log.
fn hit_se_frame(control: &crate::control::Control) {
    let on = control.flag("hit.er_se_mute") != Some(false);
    if HIT_MUTE.swap(on, Ordering::Relaxed) != on {
        erlog!("sound: (ER-HITSFX) the KH attacks' Elden Ring hit sounds: {}", if on { "MUTED" } else { "Elden Ring's own (hit.er_se_mute = 0)" });
    }
    let muted = HIT_MUTED.load(Ordering::Relaxed);
    let told = HIT_MUTED_TOLD.load(Ordering::Relaxed);
    if muted != told && (told == 0 || muted >= told.saturating_mul(10)) {
        HIT_MUTED_TOLD.store(muted, Ordering::Relaxed);
        erlog!("sound: (ER-HITSFX) {muted} Elden Ring hit sound(s) of the KH characters' hits muted so far");
    }
}

// ------------------------------------------------------- (ER-HITSFX round 2) the hooks' contexts and the sound diagnostic
//
// THE CORE SOUND MAKER (READ in eldenring.exe 2.7.1.0 / 2.7.2.0): every Elden Ring sound instance is made by ONE function,
// `make(CSSound, &{type, id}, attach node or null) -> instance or null` (2.7.1.0 0xDAD650, 2.7.2.0 0xDAED40), reached only
// through three wrappers: the plain / attached play 0xDA73F0 (28 calls in 20 functions: the character plays, the
// character's sound slots, bullets, map objects, menus ...), the play at a place 0xDA7350 (PlaySE of the events, the
// ladder wind) and the switch-event play 0xDA7AA0 (the material hit sounds, the character voices). Every wrapper takes a
// null instance as "no sound". It is detoured for the DIAGNOSTIC only (nothing is refused there): every distinct sound id
// made OUTSIDE the two character plays is logged once with its callers' return addresses (RVAs), so a stand-in's sound
// reaching the game by another path shows in the user's next log.

std::thread_local! {
    /// Inside a hit apply whose attacker is NOT a stand-in and whose target IS one (an enemy hitting a KH character).
    static ENEMY_HIT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// Inside a character play that was let through (a character's own sound: not the diagnostic's business).
    static CHR_CTX: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Leaves the "an enemy's hit on one of ours" state of this thread however the apply returns.
struct EnemyHit;

impl EnemyHit {
    fn enter() -> EnemyHit {
        ENEMY_HIT.with(|c| c.set(c.get() + 1));
        EnemyHit
    }
}

impl Drop for EnemyHit {
    fn drop(&mut self) {
        ENEMY_HIT.with(|c| c.set(c.get().saturating_sub(1)));
    }
}

fn in_enemy_hit() -> bool {
    ENEMY_HIT.with(|c| c.get()) > 0
}

/// Leaves the "inside a character play" state of this thread however the play returns.
struct ChrCtx;

impl ChrCtx {
    fn enter() -> ChrCtx {
        CHR_CTX.with(|c| c.set(c.get() + 1));
        ChrCtx
    }
}

impl Drop for ChrCtx {
    fn drop(&mut self) {
        CHR_CTX.with(|c| c.set(c.get().saturating_sub(1)));
    }
}

fn in_chr_ctx() -> bool {
    CHR_CTX.with(|c| c.get()) > 0
}

/// The core sound maker (capture 0 = its start).
const PAT_CREATE: &str = "40 57 48 83 ec 30 48 c7 44 24 20 fe ff ff ff 48 89 5c 24 40 48 89 74 24 48 49 8b d8 48 8b fa 48 8b f1 4c 8b 05 ? ? ? ? 4c 89 44 24 58 ba 08 00 00 00 b9 28 01 00 00 e8 ? ? ? ? 48 89 44 24 28 48 85 c0 74 ? 4c 8b c3 48 8b d7 48 8b c8 e8 ? ? ? ?";

pub(crate) const CREATE_FN: Located = Located { name: "core sound maker (ER-HITSFX round 2, diagnostic)", pattern: PAT_CREATE, capture: 0, rva_2_7_1_0: 0x00da_d650, rva_2_7_2_0: 0x00da_ed40 };

type Create = unsafe extern "C" fn(usize, *const [i32; 2], usize) -> usize;

static ORIG_CREATE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// `snd.er_diag` (on).
static DIAG: AtomicBool = AtomicBool::new(true);
/// Diagnostic lines in one session at most.
pub const DIAG_CAP: u32 = 400;
static DIAG_LINES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// The keys already told, and the lines waiting for the game thread's log (both taken with `try_lock` only).
static DIAG_SEEN: std::sync::Mutex<Option<std::collections::HashSet<u64>>> = std::sync::Mutex::new(None);
static DIAG_QUEUE: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// THE PURE KEY of a diagnostic line: (what kind of line, the stand-in's kind or 0, the sound type, the sound id).
pub fn diag_key(line: u8, who: u8, kind: i32, id: i32) -> u64 {
    (u64::from(line) << 60) | (u64::from(who) << 52) | ((kind as u8 as u64) << 40) | u64::from(id as u32)
}

/// True once per key (until the cap), never waiting: the hooks run on the game's task threads.
fn diag_once(key: u64) -> bool {
    if !DIAG.load(Ordering::Relaxed) || DIAG_LINES.load(Ordering::Relaxed) >= DIAG_CAP {
        return false;
    }
    let Ok(mut g) = DIAG_SEEN.try_lock() else { return false };
    let fresh = g.get_or_insert_with(std::collections::HashSet::new).insert(key);
    if fresh {
        DIAG_LINES.fetch_add(1, Ordering::Relaxed);
    }
    fresh
}

fn diag_push(line: String) {
    if let Ok(mut q) = DIAG_QUEUE.try_lock() {
        q.push(line);
    }
}

/// What became of a stand-in's sound, for the diagnostic.
pub fn diag_verdict(muted: bool, enemy_hit: bool) -> &'static str {
    match (muted, enemy_hit) {
        (true, _) => "MUTED",
        (false, true) => "heard (inside an enemy's hit on it: kept, the user's rule)",
        (false, false) => "heard (its mute key is off)",
    }
}

/// A stand-in's sound through a character play: logged once per (stand-in kind, type, id).
fn diag_stand_in(who: crate::standin::Who, kind: i32, id: i32, via: &'static str, muted: bool, enemy_hit: bool) {
    let w = match who {
        crate::standin::Who::Player => 1,
        crate::standin::Who::Body => 2,
        crate::standin::Who::MimicBoss => 3,
    };
    if diag_once(diag_key(1, w, kind, id)) {
        diag_push(format!("sound diag: (ER-HITSFX round 2) {}{:09} from {} through {}: {}", crate::titlemusic::letter(kind), id, who.name(), via, diag_verdict(muted, enemy_hit)));
    }
}

unsafe extern "C" fn create_hook(sound: usize, sid: *const [i32; 2], node: usize) -> usize {
    if DIAG.load(Ordering::Relaxed) && !sid.is_null() && !in_chr_ctx() && DIAG_LINES.load(Ordering::Relaxed) < DIAG_CAP {
        let [kind, id] = std::ptr::read_unaligned(sid);
        if diag_once(diag_key(2, 0, kind, id)) {
            let mut frames = [std::ptr::null_mut::<std::ffi::c_void>(); 6];
            // SAFETY: the system's own stack walk of this thread into our buffer.
            let n = windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace(1, frames.len() as u32, frames.as_mut_ptr(), std::ptr::null_mut()) as usize;
            let base = exe_base();
            let callers: Vec<String> = frames[..n.min(frames.len())].iter().map(|f| {
                let a = *f as usize;
                if a > base && a - base < 0x0800_0000 { format!("{:#x}", a - base) } else { "ours/other".to_string() }
            }).collect();
            let ctx = if in_our_hit() { " (inside a KH hit's apply)" } else if in_enemy_hit() { " (inside an enemy's hit on a KH character)" } else { "" };
            diag_push(format!("sound diag: (ER-HITSFX round 2) {}{:09} made outside a character play{ctx}, {}: callers {}", crate::titlemusic::letter(kind), id, if node == 0 { "not attached" } else { "attached" }, callers.join(" < ")));
        }
    }
    let original: Create = std::mem::transmute(ORIG_CREATE.load(Ordering::Relaxed));
    original(sound, sid, node)
}

/// (install) The core sound maker's diagnostic.
fn install_diag() {
    let Some(at) = locate(&CREATE_FN) else {
        erlog!("sound: (ER-HITSFX round 2) the core sound maker is not located: no sound diagnostic");
        return;
    };
    // SAFETY: the located start of a function of this signature (three register arguments, passed through unchanged).
    match unsafe { kh_er_render::inline_hook(at, create_hook as *const () as usize, &ORIG_CREATE, "core sound maker") } {
        Ok(()) => erlog!("sound: (ER-HITSFX round 2) the core sound maker is watched: every distinct Elden Ring sound made outside a character play, and every stand-in's sound refused or heard, is logged once (`sound diag:`, at most {DIAG_CAP} lines; snd.er_diag = 0: off)"),
        Err(e) => {
            ORIG_CREATE.store(0, Ordering::SeqCst);
            erlog!("sound: (ER-HITSFX round 2) the core sound maker was NOT detoured ({e}): no sound diagnostic");
        }
    }
}

/// (frame) The diagnostic's waiting lines into the log (the game thread).
fn diag_flush() {
    let lines = DIAG_QUEUE.try_lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
    for l in lines {
        erlog!("{l}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (ER-TWEAKS; ER-HITSFX round 2) EVERY sound of a stand-in is refused (its voices by the voice key, the rest by the
    /// stand-in key); nothing of any other character; nothing inside an enemy's hit on a stand-in (the user's kept rule).
    #[test]
    fn every_sound_of_a_stand_in_is_muted_and_nothing_else() {
        // the hidden Tarnished's / a pool body's / a Mimic boss body's grunts, pain, death ('d'), spoken line ('v')
        for kind in [TYPE_DIALOGUE, TYPE_VOICE_LINE] {
            assert!(mute_chr_sound(true, true, true, false, kind));
            assert!(mute_chr_sound(true, false, true, false, kind), "a voice follows the voice key only");
            assert!(!mute_chr_sound(false, true, true, false, kind), "tarnished.voice_mute = 0: Elden Ring's voices");
        }
        // their steps 'x', armour 'b', swings / whooshes / landings 'c', SFX 's', and every other type
        for kind in [TYPE_CHARACTER, 0, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 16] {
            assert!(mute_chr_sound(true, true, true, false, kind), "type {kind}");
            assert!(mute_chr_sound(false, true, true, false, kind), "type {kind}: the stand-in key only");
            assert!(!mute_chr_sound(true, false, true, false, kind), "type {kind}: standin.er_sound_mute = 0");
        }
        // any other character (an enemy, an NPC, the real Mimic Tear of an ash, another player): never
        for kind in 0..17 {
            assert!(!mute_chr_sound(true, true, false, false, kind), "type {kind}");
        }
        // an ENEMY's hit on a stand-in: every sound inside its hit apply is kept
        for kind in 0..17 {
            assert!(!mute_chr_sound(true, true, true, true, kind), "type {kind}");
        }
    }

    /// (ER-HITSFX round 2) The diagnostic's keys: one per (line, stand-in kind, type, id), never two for different sounds.
    #[test]
    fn the_diagnostic_keys_are_distinct_per_sound() {
        let a = diag_key(1, 2, TYPE_DIALOGUE, 6000);
        assert_ne!(a, diag_key(1, 2, TYPE_DIALOGUE, 6200), "another id");
        assert_ne!(a, diag_key(1, 1, TYPE_DIALOGUE, 6000), "another stand-in kind");
        assert_ne!(a, diag_key(1, 2, TYPE_CHARACTER, 6000), "another type");
        assert_ne!(a, diag_key(2, 2, TYPE_DIALOGUE, 6000), "the other line");
        assert_eq!(a, diag_key(1, 2, TYPE_DIALOGUE, 6000));
        assert_ne!(diag_key(2, 0, 8, 999_999_999), diag_key(2, 0, 8, 999_999_998));
        assert_eq!(diag_verdict(true, false), "MUTED");
        assert!(diag_verdict(false, true).contains("enemy"));
        assert!(diag_verdict(false, false).contains("key"));
    }

    /// (ER-HITSFX round 2) An enemy's hit on a KH character keeps its sounds only inside that apply, on that thread.
    #[test]
    fn an_enemys_hit_is_kept_only_inside_its_apply() {
        assert!(!in_enemy_hit());
        {
            let _g = EnemyHit::enter();
            assert!(in_enemy_hit());
            std::thread::spawn(|| assert!(!in_enemy_hit())).join().unwrap();
        }
        assert!(!in_enemy_hit(), "the body's later sounds (its pain cry, its steps) are muted again");
        assert!(!in_chr_ctx());
        {
            let _c = ChrCtx::enter();
            assert!(in_chr_ctx());
        }
        assert!(!in_chr_ctx());
    }

    /// The voice types are exactly 'v' and 'd'.
    #[test]
    fn the_voices_are_v_and_d() {
        let voices: Vec<char> = (0..17).filter(|k| is_voice(*k)).map(crate::titlemusic::letter).collect();
        assert_eq!(voices, vec!['v', 'd']);
    }

    /// (ER-HITSFX) The attacker as the game derives it: a character's own handle, a bullet's owner, nothing else.
    #[test]
    fn hitsfx_the_attacker_is_the_character_or_the_bullets_owner() {
        let (player, body) = (0x0000_0001_1000_0005_u64, 0x0000_0002_1000_0007_u64);
        assert_eq!(attacker_of(ATTACKER_CHR, body, None), Some(body));
        assert_eq!(attacker_of(ATTACKER_BULLET, 0x0000_0009_3000_0001, Some(player)), Some(player), "our bullet: its owner, not the bullet");
        assert_eq!(attacker_of(ATTACKER_BULLET, 0x0000_0009_3000_0001, None), None, "the bullet is gone: unknown");
        assert_eq!(attacker_of(ATTACKER_BULLET, 0x0000_0009_3000_0001, Some(0xffff_ffff_ffff_ffff)), None, "an ownerless bullet");
        assert_eq!(attacker_of(ATTACKER_BULLET, 0x0000_0009_3000_0001, Some(0x0000_0003_ffff_ffff)), None, "the low word -1 is no handle");
        assert_eq!(attacker_of(2, body, Some(player)), None, "another kind: no attacker (the game's own default)");
        assert_eq!(attacker_of(ATTACKER_CHR, 0, None), None);
    }

    /// (ER-HITSFX) Only the stand-ins' hits lose Elden Ring's hit sound; an enemy's hit keeps it, wherever it lands.
    #[test]
    fn hitsfx_only_the_stand_ins_hits_are_muted() {
        let (player, body, enemy) = (0x0000_0001_1000_0005_u64, 0x0000_0002_1000_0007_u64, 0x0000_0003_1000_0011_u64);
        // the leader's hits (player-owned bullets), a friend's / Mimic Sora's / a staged friend's (body-owned bullets)
        assert!(mute_hit_se(true, Some(player), player, false));
        assert!(mute_hit_se(true, Some(body), player, true));
        assert!(mute_hit_se(true, Some(body), 0, true), "a body's hit during a load");
        // an enemy's hit (on Sora's Tarnished, on a friend's body, on anyone): Elden Ring's sound
        assert!(!mute_hit_se(true, Some(enemy), player, false));
        // no attacker known: the game's sound
        assert!(!mute_hit_se(true, None, player, false));
        assert!(!mute_hit_se(true, None, 0, false));
        // no player now: an unknown character is not taken for him
        assert!(!mute_hit_se(true, Some(enemy), 0, false));
        // the key off: Elden Ring's hit sounds back
        assert!(!mute_hit_se(false, Some(player), player, false));
        assert!(!mute_hit_se(false, Some(body), player, true));
    }

    /// (ER-HITSFX) The "our hit" mark lives only inside the apply, on its own thread: a dense frame of hits (the leader,
    /// two friends, two copies' bodies, enemies between them, on several hit threads) mutes exactly the stand-ins'.
    #[test]
    fn hitsfx_the_mark_is_per_apply_and_per_thread() {
        let (player, enemy) = (0x0000_0001_1000_0005_u64, 0x0000_0003_1000_0011_u64);
        let bodies = [0x0000_0002_1000_0007_u64, 0x0000_0002_1000_0009, 0x0000_0002_1000_000b, 0x0000_0002_1000_000d];
        assert!(!in_our_hit());
        {
            let _g = mute_hit_se(true, Some(player), player, false).then(OurHit::enter);
            assert!(in_our_hit(), "inside the leader's hit apply");
            std::thread::spawn(|| assert!(!in_our_hit(), "another hit thread is not marked")).join().unwrap();
        }
        assert!(!in_our_hit(), "after the apply: enemies' later hits keep their sound");
        let workers: Vec<_> = (0..4)
            .map(|k| {
                std::thread::spawn(move || {
                    let mut muted = 0;
                    for i in 0..200 {
                        let attacker = if i % 3 == 0 { enemy } else if i % 3 == 1 { player } else { bodies[(i + k) % 4] };
                        let _g = mute_hit_se(true, Some(attacker), player, bodies.contains(&attacker)).then(OurHit::enter);
                        if in_our_hit() {
                            muted += 1;
                        } else {
                            assert_eq!(attacker, enemy);
                        }
                    }
                    assert!(!in_our_hit());
                    muted
                })
            })
            .collect();
        for w in workers {
            assert_eq!(w.join().unwrap(), 133, "every stand-in hit muted (2 of 3), every enemy hit heard");
        }
    }

    /// (ER-HITSFX) The four functions' records are distinct and recorded for both executables.
    #[test]
    fn hitsfx_every_function_is_recorded_for_both_versions() {
        for l in &HIT_LOCATED {
            assert!(l.rva_2_7_1_0 != 0 && l.rva_2_7_2_0 != 0, "{}", l.name);
            assert_eq!(l.capture, 0, "{}", l.name);
        }
        let mut rvas: Vec<u32> = HIT_LOCATED.iter().map(|l| l.rva_2_7_2_0).collect();
        rvas.sort_unstable();
        rvas.dedup();
        assert_eq!(rvas.len(), 4);
    }

    #[test]
    fn the_letter_of_type_15_is_d() {
        assert_eq!(crate::titlemusic::letter(TYPE_DIALOGUE), 'd');
        assert_eq!(crate::titlemusic::letter(TYPE_CHARACTER), 'c');
    }
}
