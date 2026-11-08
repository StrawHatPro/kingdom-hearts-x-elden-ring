//! (WORLDTIME-2 fx) KH3'S WORLD SLOW ON ELDEN RING'S EFFECTS PER OWNER, AND ON ITS PHYSICS (the coordinator's follow-up:
//! the global play-speed rate of chrtime.rs slowed the spirit ashes' effects too; KH3 slows an effect by the type of the
//! actor that owns it, and PHYSX).
//!
//! KH3 (traced): a particle component ticks at its owner actor's `dt x CustomTimeDilation x ObjTypeTimeDilation[type]`
//! (docs/sora-magic-spec.md A6). A HIT effect (`UTresAtkCollComponent` 0x140628960, sora-vfx-spec 2.6) is spawned by
//! `0x14411B3D0` -> `0x14411B280` -> `CreateParticleSystem(template, world, outer = World + 0x110 = the GAME STATE)`
//! (`0x1440FD450`): its owner is the game state, an info actor of type NONE, which no slow names: hit effects keep x1
//! under WorldSlow, whoever hit whom. Effects of a character follow its type; a projectile's take its owner's.
//!
//! ELDEN RING (2.7.1.0 / 2.7.2.0; the FFX library is unoptimised debug-style code, `rep stosd 0xCC` frames):
//! - Every live effect is a 0x90-byte record (ctor 0x20d99d0: +0x00 FXR id, **+0x04 its own speed, 1.0, written by
//!   nobody else** (the only `movss [r+4]` store of the library), +0x0C scale, +0x18 colour, +0x38 the system, +0x88 the
//!   head of its handles' list). Allocation: `FFX::AllocEffect` [`CREATE_FN`] 0x20da400 / 0x20dbb30, the one caller
//!   0x20df8d0 (the scene's create 0x1ca6a20 <- CSSfxImp's play 0x1ca0cd0 <- CS 0xd96ec0 / 0xd96d10: the CS layer plays
//!   effects synchronously, inside the update that asks).
//! - Every frame each root effect is stepped by `FFX::UpdateEffect(effect, dt, ctx)` [`UPDATE_FN`] 0x20daaf0 / 0x20dc220
//!   (callers: the scene's job 0x1ca3ae0 / 0x1ca51d0 and 0x1ca8530: `dt' = (carry + dt) x group+0x10`): it does
//!   `dt *= effect+0x04` and steps its children with that. THAT IS THE PER-EFFECT TIME SCALE.
//! - The CS layer makes them DEFERRED (MEASURED in game 2026-11-05: the Lantern's records were allocated under
//!   0xd8cfd1 < 0xd8e6df < 0xd8b00c, with no character update on the stack): a CS effect (`CSFD4SfxIns`, base constructor
//!   [`CS_CTOR`] 0xd8b330 / 0xd8ca20) is created inside the update that asks for it, and its FFX records are made later by
//!   its play, vtable slot 11 ([`CS_PLAY`]: single 0xd8cf00, multi 0xd8a220, trace 0xd8dfb0), in the effects' own update.
//!   So the CS effect is tagged at construction and its play runs with that tag as the context.
//! - The owner is not stored. THE SEAM BUILT HERE: the update that makes an effect is known while it runs: the 16
//!   per-character phase wrappers [`PHASES`] (0x3f8700 .. 0x3f9ea0; every phase of a character, its TAE and special
//!   effects included) and the bullet update (chrtime.rs) keep their owner's handle in a thread-local context while they
//!   run; `CREATE_FN`'s detour tags the new effect with it. `UPDATE_FN`'s detour scales `dt` by the owner's KH3 type
//!   (chrtime.rs `Table::fx_rate`): an enemy's at ENEMY's rate, the player's side (the Tarnished, the KH bodies, summons,
//!   Torrent) at theirs (x1 for WorldSlow), an effect made by nobody's update (the map's, map objects') at MAP's, an
//!   unknown owner (a map object's bullet) at GIMMICK's; an FXR id of `HitEffectSfxParam` (Elden Ring's hit sparks) at
//!   x1 (KH3: NONE).
//! - Physics: `hknpWorld::stepCollide(world, stepInput, taskQueue)` [`STEP_FN`] 0x18afea0 / 0x18b1590, called by Elden
//!   Ring's physics task 0xc520d0 with the input built from its delta (`stepInput->m_deltaTime` at +0, copied into the
//!   solver info stepSolve 0x18b3c40 uses; the library's own wrapper 0x18b3ac0 is not on Elden Ring's path): the world step of every body Havok integrates (ragdolls, physics objects) gets `dt x PHYSX`.
//!   Character proxies are not stepped by it (each character's physics module moves its proxy on its own delta,
//!   chrtime.rs); our cape is PhysX 5.1 with its own fixed step (kh-er-render), untouched.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{LazyLock, RwLock};

use crate::chrtime::TABLE;
use crate::erlog;
use crate::havok::{locate, Located};

/// The wrappers' common start (the tests check every pattern begins with it).
#[cfg(test)]
const PRE: &str = "40 57 48 83 ec 40 48 c7 44 24 20 fe ff ff ff 48 89 5c 24 50 f3 0f 10 89 b0 00 00 00 48 8d 3d ? ? ? ? 48 89 7c 24 28 0f 57 c0 f3 0f 11 44 24 30 48 8d 1d ? ? ? ? 48 89 5c 24 28 f3 0f 11 4c 24 30";

macro_rules! phase {
    ($name:literal, $tail:literal, $a:literal) => {
        Located { name: concat!("character phase wrapper ", $name), pattern: concat!("40 57 48 83 ec 40 48 c7 44 24 20 fe ff ff ff 48 89 5c 24 50 f3 0f 10 89 b0 00 00 00 48 8d 3d ? ? ? ? 48 89 7c 24 28 0f 57 c0 f3 0f 11 44 24 30 48 8d 1d ? ? ? ? 48 89 5c 24 28 f3 0f 11 4c 24 30 ", $tail), capture: 0, rva_2_7_1_0: $a, rva_2_7_2_0: $a + 0x90 }
    };
}

/// The 16 per-character phase wrappers (each reads `ChrIns + 0xB0` into an `FD4Time` and calls the character's phase).
pub(crate) const PHASES: [Located; 16] = [
    phase!("vt+0x78", "48 8b 01 48 8d 54 24 28 ff 50 78", 0x003f_8ff0),
    phase!("vt+0x80", "48 8b 01 48 8d 54 24 28 ff 90 80 00 00 00", 0x003f_8700),
    phase!("vt+0x88", "48 8b 01 48 8d 54 24 28 ff 90 88 00 00 00", 0x003f_9390),
    phase!("vt+0x90", "48 8b 01 48 8d 54 24 28 ff 90 90 00 00 00", 0x003f_9400),
    phase!("vt+0x98", "48 8b 01 48 8d 54 24 28 ff 90 98 00 00 00", 0x003f_8f60),
    phase!("vt+0xA0", "48 8b 01 48 8d 54 24 28 ff 90 a0 00 00 00", 0x003f_9550),
    phase!("vt+0xA8", "48 8b 01 48 8d 54 24 28 ff 90 a8 00 00 00", 0x003f_95c0),
    phase!("vt+0xB0", "48 8b 01 48 8d 54 24 28 ff 90 b0 00 00 00", 0x003f_9470),
    phase!("vt+0xB8", "48 8b 01 48 8d 54 24 28 ff 90 b8 00 00 00", 0x003f_94e0),
    phase!("vt+0xC0", "48 8b 01 48 8d 54 24 28 ff 90 c0 00 00 00", 0x003f_92b0),
    phase!("vt+0xC8", "48 8b 01 48 8d 54 24 28 ff 90 c8 00 00 00", 0x003f_9320),
    phase!("vt+0xD0", "48 8b 01 48 8d 54 24 28 ff 90 d0 00 00 00", 0x003f_9060),
    phase!("vt+0xD8", "48 8b 01 48 8d 54 24 28 ff 90 d8 00 00 00", 0x003f_90d0),
    phase!("vt+0xE0", "48 8b 01 48 8d 54 24 28 ff 90 e0 00 00 00", 0x003f_9e30),
    phase!("AI (ChrCtrl)", "48 8b 49 58 e8", 0x003f_9dc0),
    phase!("flags 0x1C4", "0f b6 81 c4 01 00 00 24 fd 0c 04 88 81 c4 01 00 00 e8", 0x003f_9ea0),
];

/// `FFX::AllocEffect(fxr)`: the 0x90-byte record allocated and constructed.
pub(crate) const CREATE_FN: Located = Located {
    name: "FFX effect allocation (the record's constructor)",
    pattern: "89 4c 24 08 57 48 83 ec 50 48 8b fc b9 14 00 00 00 b8 cc cc cc cc f3 ab 8b 4c 24 60 48 c7 44 24 38 fe ff ff ff 45 33 c0 ba 08 00 00 00 b9 90 00 00 00 e8",
    capture: 0,
    rva_2_7_1_0: 0x020d_a400,
    rva_2_7_2_0: 0x020d_bb30,
};
/// `FFX::UpdateEffect(effect, dt, ctx)`: `dt *= effect + 0x04`, then its children.
pub(crate) const UPDATE_FN: Located = Located {
    name: "FFX effect update (dt x the effect's own speed)",
    pattern: "4c 89 44 24 18 f3 0f 11 4c 24 10 48 89 4c 24 08 57 48 81 ec a0 00 00 00 48 8b fc b9 28 00 00 00 b8 cc cc cc cc f3 ab 48 8b 8c 24 b0 00 00 00 48 c7 84 24 88 00 00 00 fe ff ff ff 48 8b 84 24 b0 00 00 00",
    capture: 0,
    rva_2_7_1_0: 0x020d_aaf0,
    rva_2_7_2_0: 0x020d_c220,
};
/// `hknpWorld::stepCollide(world, stepInput, taskQueue)`: Elden Ring's physics task 0xc520d0 / 0xc52590 builds the step
/// input on its stack from its delta and calls it (0xc524ce); `movsd xmm0,[rdx]` reads the input's delta first. (0x18b3ac0,
/// the library's own wrapper of it, is not on Elden Ring's path: MEASURED, no call in game.)
pub(crate) const STEP_FN: Located = Located {
    name: "hknpWorld::stepCollide (the physics world's step input)",
    pattern: "48 8b c4 48 89 58 08 48 89 68 10 48 89 70 18 48 89 78 20 41 56 48 83 ec 40 f2 0f 10 02 48 8b f9 48 8b 89 10 0a 00 00 41 be 20 00 00 00 66 44 89 70 d8",
    capture: 0,
    rva_2_7_1_0: 0x018a_fea0,
    rva_2_7_2_0: 0x018b_1590,
};

/// `CS::CSFD4SfxIns`'s base constructor (every CS effect instance: single, multi, trace ...).
pub(crate) const CS_CTOR: Located = Located {
    name: "CSFD4SfxIns base constructor (the owner of a CS effect)",
    pattern: "48 89 4c 24 08 48 83 ec 18 48 c7 04 24 fe ff ff ff 48 8d 05 ? ? ? ? 48 89 01 33 d2 89 51 08 48 8d 05 ? ? ? ? 48 89 01 89 51 10 8b 41 14 25 7c fc ff ff 83 c8 7c 89 41 14 48 89 51 18 48 8b 05 ? ? ? ? 0f 10 00 0f 11 41 20",
    capture: 0,
    rva_2_7_1_0: 0x00d8_b330,
    rva_2_7_2_0: 0x00d8_ca20,
};
/// The CS effect instances' play (vtable slot 11: their FFX effects are made here, deferred, in the effects' own update:
/// MEASURED in game, the Lantern's 3 records were made by 0xd8cfd1 < 0xd8e6df ... with no character on the stack).
pub(crate) const CS_PLAY: [Located; 3] = [
    Located { name: "CSFD4SingleSfxIns play (slot 11)", pattern: "48 8b c4 56 57 41 56 48 81 ec c0 04 00 00 48 c7 44 24 58 fe ff ff ff 48 89 58 10 48 89 68 18 48 8b 05 ? ? ? ? 48 33 c4", capture: 0, rva_2_7_1_0: 0x00d8_cf00, rva_2_7_2_0: 0x00d8_e5f0 },
    Located { name: "CSFD4MultiSfxIns play (slot 11)", pattern: "48 8b c4 57 41 54 41 55 41 56 41 57 48 81 ec d0 04 00 00 48 c7 44 24 58 fe ff ff ff 48 89 58 10 48 89 68 18 48 89 70 20", capture: 0, rva_2_7_1_0: 0x00d8_a220, rva_2_7_2_0: 0x00d8_b910 },
    Located { name: "CSFD4TraceSfxIns play (slot 11)", pattern: "48 8b c4 57 48 81 ec a0 04 00 00 48 c7 44 24 78 fe ff ff ff 48 89 58 10 48 89 68 18 48 89 70 20 48 8b 05 ? ? ? ? 48 33 c4", capture: 0, rva_2_7_1_0: 0x00d8_dfb0, rva_2_7_2_0: 0x00d8_f6a0 },
];

/// For the PORT offline check (gamever.rs).
#[cfg(test)]
pub(crate) fn located() -> Vec<&'static Located> {
    let mut v: Vec<&'static Located> = PHASES.iter().collect();
    v.extend([&CREATE_FN, &UPDATE_FN, &STEP_FN, &CS_CTOR]);
    v.extend(CS_PLAY.iter());
    v
}

/// The record's own speed (`UPDATE_FN`: `mulss xmm0,[rax+4]`) and its FXR id.
#[cfg(test)]
pub const EFFECT_SPEED: usize = 0x04;
pub const EFFECT_ID: usize = 0x00;
/// A character's handle (its second word).
pub const CHR_HANDLE: usize = 0x08;

thread_local! {
    /// The owner (handle bits) of the update running on this thread; 0 = none.
    static CTX: Cell<u64> = const { Cell::new(0) };
}

static ORIG_PHASE: [AtomicUsize; 16] = [const { AtomicUsize::new(0) }; 16];
static ORIG_CREATE: AtomicUsize = AtomicUsize::new(0);
static ORIG_UPDATE: AtomicUsize = AtomicUsize::new(0);
static ORIG_STEP: AtomicUsize = AtomicUsize::new(0);
static ORIG_CS_CTOR: AtomicUsize = AtomicUsize::new(0);
static ORIG_CS_PLAY: [AtomicUsize; 3] = [const { AtomicUsize::new(0) }; 3];
/// CS effect instance -> the owner whose update made it (0 = nobody's).
static CS_TAGS: LazyLock<RwLock<HashMap<usize, u64>>> = LazyLock::new(|| RwLock::new(HashMap::new()));
/// The fast path: a slow is published (chrtime.rs).
static ON: AtomicBool = AtomicBool::new(false);
/// Effect record -> its owner's handle (0 = nobody's update).
static TAGS: LazyLock<RwLock<HashMap<usize, u64>>> = LazyLock::new(|| RwLock::new(HashMap::new()));
/// Elden Ring's hit sparks: every FXR id of `HitEffectSfxParam`, sorted.
static HIT_IDS: RwLock<Vec<u32>> = RwLock::new(Vec::new());
/// Counters for the log: effects made with an owner / with none; effect steps slowed / at x1 by their owner / hit sparks.
static MADE_OWNED: AtomicU32 = AtomicU32::new(0);
static MADE_NONE: AtomicU32 = AtomicU32::new(0);
static FX_SLOWED: AtomicU32 = AtomicU32::new(0);
static FX_KEPT: AtomicU32 = AtomicU32::new(0);
static FX_HIT: AtomicU32 = AtomicU32::new(0);
/// The last effect stepped per class (0 slowed character's, 1 kept character's, 2 nobody's, 3 hit spark): its id and
/// the factor it got (bits), for the proof.
static SAMPLE: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
/// Physics steps slowed, and the last delta handed (bits).
static STEPS: AtomicU32 = AtomicU32::new(0);
static STEP_DT: AtomicU64 = AtomicU64::new(0);
/// `world.test_fx`'s FXR ids under watch (0 = none): per class the last factor one of them got, and how many steps.
pub static WATCH: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
static WATCHED: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
static WATCH_STEPS: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];

/// Run `f` with `owner` as this thread's update owner (the bullet update, chrtime.rs).
pub fn with_owner<R>(owner: u64, f: impl FnOnce() -> R) -> R {
    let prev = CTX.with(|c| c.replace(owner));
    let r = f();
    CTX.with(|c| c.set(prev));
    r
}

fn phase(chr: *mut u8, i: usize) {
    let original = ORIG_PHASE[i].load(Ordering::Relaxed);
    if original == 0 {
        return;
    }
    // SAFETY: the trampoline of the wrapper we detoured (rcx = the character).
    let original: unsafe extern "C" fn(*mut u8) = unsafe { std::mem::transmute(original) };
    // SAFETY: the game's character object; its handle is its second word.
    let owner = if chr.is_null() { 0 } else { unsafe { std::ptr::read_unaligned(chr.add(CHR_HANDLE) as *const u64) } };
    // SAFETY: as above.
    with_owner(owner, || unsafe { original(chr) })
}

macro_rules! phase_hooks {
    ($($f:ident = $i:literal),*) => {
        $(unsafe extern "C" fn $f(chr: *mut u8) { phase(chr, $i) })*
        const PHASE_HOOKS: [unsafe extern "C" fn(*mut u8); 16] = [$($f),*];
    };
}
phase_hooks!(p0 = 0, p1 = 1, p2 = 2, p3 = 3, p4 = 4, p5 = 5, p6 = 6, p7 = 7, p8 = 8, p9 = 9, p10 = 10, p11 = 11, p12 = 12, p13 = 13, p14 = 14, p15 = 15);

/// `CSFD4SfxIns`'s base constructor, detoured: the CS effect is tagged with the update that made it.
unsafe extern "C" fn cs_ctor_hook(ins: *mut u8) -> *mut u8 {
    let original = ORIG_CS_CTOR.load(Ordering::Relaxed);
    if original == 0 {
        return ins;
    }
    // SAFETY: the trampoline (rcx = the object, rax = it).
    let original: unsafe extern "C" fn(*mut u8) -> *mut u8 = unsafe { std::mem::transmute(original) };
    // SAFETY: as above.
    let r = unsafe { original(ins) };
    if let Ok(mut t) = CS_TAGS.write() {
        if t.len() > 100_000 {
            t.clear();
        }
        t.insert(ins as usize, CTX.with(Cell::get));
    }
    r
}

fn cs_play(ins: *mut u8, i: usize) -> usize {
    let original = ORIG_CS_PLAY[i].load(Ordering::Relaxed);
    if original == 0 {
        return 0;
    }
    // SAFETY: the trampoline of the play we detoured (rcx = the CS effect).
    let original: unsafe extern "C" fn(*mut u8) -> usize = unsafe { std::mem::transmute(original) };
    let owner = CS_TAGS.read().ok().and_then(|t| t.get(&(ins as usize)).copied()).unwrap_or(0);
    let current = CTX.with(Cell::get);
    // SAFETY: as above.
    with_owner(if owner != 0 { owner } else { current }, || unsafe { original(ins) })
}
unsafe extern "C" fn cs_play0(ins: *mut u8) -> usize {
    cs_play(ins, 0)
}
unsafe extern "C" fn cs_play1(ins: *mut u8) -> usize {
    cs_play(ins, 1)
}
unsafe extern "C" fn cs_play2(ins: *mut u8) -> usize {
    cs_play(ins, 2)
}
const CS_PLAY_HOOKS: [unsafe extern "C" fn(*mut u8) -> usize; 3] = [cs_play0, cs_play1, cs_play2];

/// `FFX::AllocEffect`, detoured: the new record is tagged with the update that asked for it.
unsafe extern "C" fn create_hook(fxr: u32) -> *mut u8 {
    let original = ORIG_CREATE.load(Ordering::Relaxed);
    if original == 0 {
        return std::ptr::null_mut();
    }
    // SAFETY: the trampoline (ecx = the FXR id, rax = the record or null).
    let original: unsafe extern "C" fn(u32) -> *mut u8 = unsafe { std::mem::transmute(original) };
    // SAFETY: as above.
    let effect = unsafe { original(fxr) };
    if !effect.is_null() {
        let owner = CTX.with(Cell::get);
        if owner == 0 {
            MADE_NONE.fetch_add(1, Ordering::Relaxed);
            if TRACE.load(Ordering::Relaxed) {
                trace_unowned();
            }
        } else {
            MADE_OWNED.fetch_add(1, Ordering::Relaxed);
        }
        if let Ok(mut t) = TAGS.write() {
            if t.len() > 200_000 {
                t.clear();
            }
            // (a reused record gets its new owner: every record comes through here)
            t.insert(effect as usize, owner);
        }
    }
    effect
}

/// `world.proof = 2` (a testing aid): the call sites of effects made by nobody's update are counted (where the game
/// makes them), the most frequent logged with the proof lines.
pub static TRACE: AtomicBool = AtomicBool::new(false);
static TRACES: std::sync::Mutex<Vec<([u32; 6], u32)>> = std::sync::Mutex::new(Vec::new());

fn trace_unowned() {
    let mut frames = [std::ptr::null_mut::<core::ffi::c_void>(); 12];
    // SAFETY: the system's stack walk into a buffer of ours.
    let n = unsafe { windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace(2, frames.len() as u32, frames.as_mut_ptr(), std::ptr::null_mut()) } as usize;
    let base = crate::entry::exe_base();
    let mut key = [0u32; 6];
    for (k, f) in frames[..n].iter().map(|f| *f as usize).filter(|f| *f > base && *f - base < 0x1000_0000).take(6).enumerate() {
        key[k] = (f - base) as u32;
    }
    if let Ok(mut t) = TRACES.try_lock() {
        if let Some(e) = t.iter_mut().find(|e| e.0 == key) {
            e.1 += 1;
        } else if t.len() < 64 {
            t.push((key, 1));
        }
    }
}

/// The traced call sites so far (most frequent first), and cleared.
pub fn take_traces() -> String {
    let Ok(mut t) = TRACES.lock() else { return String::new() };
    t.sort_by_key(|e| std::cmp::Reverse(e.1));
    let s: Vec<String> = t.iter().take(8).map(|(k, n)| format!("{n} x [{}]", k.iter().filter(|a| **a != 0).map(|a| format!("{a:#x}")).collect::<Vec<_>>().join(" < "))).collect();
    t.clear();
    s.join("; ")
}

/// The factor of one effect step: its FXR id and its owner by the published table.
pub fn effect_rate(table: &crate::chrtime::Table, hit_ids: &[u32], id: u32, owner: u64) -> (f32, usize) {
    if hit_ids.binary_search(&id).is_ok() {
        return (1.0, 3);
    }
    let r = table.fx_rate(owner);
    (r, if owner == 0 { 2 } else if r < 1.0 { 0 } else { 1 })
}

/// `FFX::UpdateEffect`, detoured: `dt` by the effect owner's KH3 type.
unsafe extern "C" fn update_hook(effect: *mut u8, dt: f32, ctx: *mut u8) -> u8 {
    let original = ORIG_UPDATE.load(Ordering::Relaxed);
    if original == 0 {
        return 1;
    }
    // SAFETY: the trampoline (rcx = the record, xmm1 = the delta, r8 = the job context, al = done).
    let original: unsafe extern "C" fn(*mut u8, f32, *mut u8) -> u8 = unsafe { std::mem::transmute(original) };
    let mut dt = dt;
    if ON.load(Ordering::Relaxed) && !effect.is_null() {
        // SAFETY: the game's live record (its update is running); its id is its first word.
        let id = unsafe { std::ptr::read_unaligned(effect.add(EFFECT_ID) as *const u32) };
        let owner = TAGS.read().ok().and_then(|t| t.get(&(effect as usize)).copied()).unwrap_or(0);
        if let (Ok(table), Ok(hits)) = (TABLE.try_read(), HIT_IDS.try_read()) {
            let (r, class) = effect_rate(&table, &hits, id, owner);
            match class {
                3 => FX_HIT.fetch_add(1, Ordering::Relaxed),
                _ if r < 1.0 => FX_SLOWED.fetch_add(1, Ordering::Relaxed),
                _ => FX_KEPT.fetch_add(1, Ordering::Relaxed),
            };
            SAMPLE[class].store((u64::from(id) << 32) | u64::from(r.to_bits()), Ordering::Relaxed);
            if WATCH.iter().any(|w| w.load(Ordering::Relaxed) == id) {
                WATCHED[class].store((u64::from(id) << 32) | u64::from(r.to_bits()), Ordering::Relaxed);
                WATCH_STEPS[class].fetch_add(1, Ordering::Relaxed);
            }
            dt *= r;
        }
    }
    // SAFETY: as above.
    unsafe { original(effect, dt, ctx) }
}

/// `hknpWorld::stepCollide`, detoured: the step input's delta x PHYSX for the call (put back after).
unsafe extern "C" fn step_hook(world: *mut u8, input: *mut f32, queue: *mut u8) -> usize {
    let original = ORIG_STEP.load(Ordering::Relaxed);
    if original == 0 {
        return 0;
    }
    // SAFETY: the trampoline (rcx = the world, rdx = the step input, r8 = the task queue).
    let original: unsafe extern "C" fn(*mut u8, *mut f32, *mut u8) -> usize = unsafe { std::mem::transmute(original) };
    let rate = if ON.load(Ordering::Relaxed) { TABLE.try_read().map(|t| t.physx).unwrap_or(1.0) } else { 1.0 };
    if rate < 1.0 && !input.is_null() {
        // SAFETY: the caller's step input (hknpStepInput, the delta first); written for this call and put back.
        let dt = unsafe { std::ptr::read_unaligned(input) };
        unsafe { std::ptr::write_unaligned(input, dt * rate) };
        STEPS.fetch_add(1, Ordering::Relaxed);
        STEP_DT.store(u64::from((dt * rate).to_bits()), Ordering::Relaxed);
        // SAFETY: as above.
        let r = unsafe { original(world, input, queue) };
        unsafe { std::ptr::write_unaligned(input, dt) };
        return r;
    }
    // SAFETY: as above.
    unsafe { original(world, input, queue) }
}

/// Detours the phase wrappers, the effect allocation and update and the physics step (entry.rs, with chrtime.rs).
pub fn install() {
    let mut phases = 0;
    for (i, item) in PHASES.iter().enumerate() {
        if let Some(at) = locate(item) {
            // SAFETY: the wrapper's start, found by its unique pattern at the recorded address; same signature.
            if unsafe { kh_er_render::inline_hook(at, PHASE_HOOKS[i] as *const () as usize, &ORIG_PHASE[i], item.name) }.is_ok() {
                phases += 1;
            } else {
                ORIG_PHASE[i].store(0, Ordering::SeqCst);
            }
        }
    }
    let mut done = Vec::new();
    for (i, item) in CS_PLAY.iter().enumerate() {
        if let Some(at) = locate(item) {
            // SAFETY: as above.
            if unsafe { kh_er_render::inline_hook(at, CS_PLAY_HOOKS[i] as *const () as usize, &ORIG_CS_PLAY[i], item.name) }.is_ok() {
                done.push(item.name);
            } else {
                ORIG_CS_PLAY[i].store(0, Ordering::SeqCst);
            }
        }
    }
    for (item, hook, original) in [
        (&CS_CTOR, cs_ctor_hook as *const () as usize, &ORIG_CS_CTOR),
        (&CREATE_FN, create_hook as *const () as usize, &ORIG_CREATE),
        (&UPDATE_FN, update_hook as *const () as usize, &ORIG_UPDATE),
        (&STEP_FN, step_hook as *const () as usize, &ORIG_STEP),
    ] {
        let Some(at) = locate(item) else { continue };
        // SAFETY: as above.
        match unsafe { kh_er_render::inline_hook(at, hook, original, item.name) } {
            Ok(()) => done.push(item.name),
            Err(e) => {
                original.store(0, Ordering::SeqCst);
                erlog!("worldtime2 fx: {} NOT detoured ({e})", item.name);
            }
        }
    }
    erlog!("worldtime2 fx: {phases} of 16 character phase wrappers detoured (the owner of the effects they make), and {done:?}");
}

pub fn uninstalled() {
    ON.store(false, Ordering::SeqCst);
    for o in &ORIG_PHASE {
        o.store(0, Ordering::SeqCst);
    }
    for o in ORIG_CS_PLAY.iter() {
        o.store(0, Ordering::SeqCst);
    }
    for o in [&ORIG_CREATE, &ORIG_UPDATE, &ORIG_STEP, &ORIG_CS_CTOR] {
        o.store(0, Ordering::SeqCst);
    }
}

/// The effects are scaled per owner (else chrtime.rs keeps the global play-speed route).
pub fn effects_hooked() -> bool {
    ORIG_CREATE.load(Ordering::Relaxed) != 0 && ORIG_UPDATE.load(Ordering::Relaxed) != 0
}

/// Once a frame (chrtime.rs, after the table was published): the fast path, and the hit-spark ids read once.
pub fn frame(on: bool) {
    ON.store(on, Ordering::SeqCst);
    if HIT_IDS.read().is_ok_and(|v| !v.is_empty()) {
        return;
    }
    use eldenring::cs::{HitEffectSfxParam, SoloParamRepository};
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the singleton lives as long as the game.
    let Ok(repo) = (unsafe { SoloParamRepository::instance() }) else { return };
    let mut ids: Vec<u32> = Vec::new();
    for (_, row) in repo.rows::<HitEffectSfxParam>() {
        // SAFETY: HIT_EFFECT_SFX_PARAM_ST is 20 plain i32 sfx ids (slash / blow / thrust / neutral x 5).
        let words: [i32; 20] = unsafe { std::mem::transmute_copy(row) };
        ids.extend(words.iter().filter(|w| **w > 0).map(|w| *w as u32));
    }
    ids.sort_unstable();
    ids.dedup();
    if !ids.is_empty() {
        erlog!("worldtime2 fx: {} hit-spark FXR id(s) from HitEffectSfxParam (KH3's hit effects are the game state's: x1)", ids.len());
        if let Ok(mut h) = HIT_IDS.write() {
            *h = ids;
        }
    }
}

/// The counters since the last call, for chrtime.rs's proof line.
pub fn proof_text() -> String {
    let take = |a: &AtomicU32| a.swap(0, Ordering::Relaxed);
    let sample = |i: usize| {
        let v = SAMPLE[i].load(Ordering::Relaxed);
        if v == 0 {
            "-".to_string()
        } else {
            format!("FXR {} x{:.3}", v >> 32, f32::from_bits(v as u32))
        }
    };
    let steps = take(&STEPS);
    let watched = if WATCH[0].load(Ordering::Relaxed) == 0 {
        String::new()
    } else {
        let names = ["an enemy's", "the player's side's", "nobody's", "hit"];
        let parts: Vec<String> = (0..4)
            .filter_map(|i| {
                let v = WATCHED[i].swap(0, Ordering::Relaxed);
                let n = WATCH_STEPS[i].swap(0, Ordering::Relaxed);
                (v != 0).then(|| format!("{} FXR {} x{:.3} ({n} steps)", names[i], v >> 32, f32::from_bits(v as u32)))
            })
            .collect();
        format!("; watched effects: {}", if parts.is_empty() { "none stepped".to_string() } else { parts.join(", ") })
    };
    format!(
        "; effects made: {} by a character's / bullet's update, {} by nobody's; effect steps: {} slowed, {} at x1 by their owner, {} hit sparks at x1 (last: slowed {}, kept {}, nobody's {}, hit {}); physics steps slowed {steps}{}",
        take(&MADE_OWNED), take(&MADE_NONE), take(&FX_SLOWED), take(&FX_KEPT), take(&FX_HIT), sample(0), sample(1), sample(2), sample(3),
        if steps > 0 { format!(" (last delta {:.5} s)", f32::from_bits(STEP_DT.load(Ordering::Relaxed) as u32)) } else { String::new() }
    ) + &watched
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrtime::{Side, Table};

    fn slow() -> Table {
        Table { enemy: 0.1, gimmick: 0.1, map: 0.1, physx: 0.1, sides: vec![(0x10, Side::Enemy), (0x20, Side::Player), (0x30, Side::Summon), (0x40, Side::Mount), (0x50, Side::Party)], ..Table::NORMAL }
    }

    #[test]
    fn an_effect_follows_its_owners_type_and_hit_sparks_keep_time() {
        let t = slow();
        let hits = [8001u32, 8100];
        assert_eq!(effect_rate(&t, &hits, 500, 0x10), (0.1, 0), "an enemy's effect");
        assert_eq!(effect_rate(&t, &hits, 500, 0x20), (1.0, 1), "the Tarnished's");
        assert_eq!(effect_rate(&t, &hits, 500, 0x30), (1.0, 1), "a spirit ash's (FRIEND is not in 0x419C)");
        assert_eq!(effect_rate(&t, &hits, 500, 0x40), (1.0, 1), "Torrent's (VEHICLE)");
        assert_eq!(effect_rate(&t, &hits, 500, 0x50), (1.0, 1), "a KH body's");
        assert_eq!(effect_rate(&t, &hits, 500, 0), (0.1, 2), "the map's (MAP)");
        assert_eq!(effect_rate(&t, &hits, 500, 0x99), (0.1, 0), "a map object's / unknown owner (GIMMICK)");
        assert_eq!(effect_rate(&t, &hits, 8100, 0x10), (1.0, 3), "a hit spark on an enemy: KH3's hit effects are the game state's (NONE)");
        let aim = Table { friend: 0.4, player: 0.4, vehicle: 0.4, ..slow() };
        assert_eq!(effect_rate(&aim, &hits, 500, 0x30), (0.4, 0), "the shotlock aim slows FRIEND");
        assert_eq!(effect_rate(&Table::NORMAL, &hits, 500, 0x10).0, 1.0, "no slow: x1");
    }

    #[test]
    fn the_owner_context_nests_and_is_given_back() {
        assert_eq!(CTX.with(Cell::get), 0);
        with_owner(0x10, || {
            assert_eq!(CTX.with(Cell::get), 0x10);
            with_owner(0x20, || assert_eq!(CTX.with(Cell::get), 0x20));
            assert_eq!(CTX.with(Cell::get), 0x10, "the bullet inside a character's phase gives the character back");
        });
        assert_eq!(CTX.with(Cell::get), 0);
    }

    /// The three detours on mocked objects with stand-ins for the game's functions.
    #[test]
    fn the_detours_tag_scale_and_give_back_on_mocked_objects() {
        let _one = crate::chrtime::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        static GOT: AtomicU64 = AtomicU64::new(0);
        static RECORD: [u8; 0x90] = [0; 0x90];
        unsafe extern "C" fn game_alloc(_fxr: u32) -> *mut u8 {
            RECORD.as_ptr() as *mut u8
        }
        unsafe extern "C" fn game_update(_e: *mut u8, dt: f32, _c: *mut u8) -> u8 {
            GOT.store(u64::from(dt.to_bits()), Ordering::SeqCst);
            1
        }
        unsafe extern "C" fn game_step(_w: *mut u8, input: *mut f32, _q: *mut u8) -> usize {
            GOT.store(u64::from(unsafe { *input }.to_bits()), Ordering::SeqCst);
            7
        }
        let got = || f32::from_bits(GOT.load(Ordering::SeqCst) as u32);
        ORIG_CREATE.store(game_alloc as *const () as usize, Ordering::SeqCst);
        ORIG_UPDATE.store(game_update as *const () as usize, Ordering::SeqCst);
        ORIG_STEP.store(game_step as *const () as usize, Ordering::SeqCst);
        *TABLE.write().unwrap() = slow();
        ON.store(true, Ordering::SeqCst);
        // made inside an enemy's phase: slowed; the same record remade by the Tarnished's: kept
        let e = with_owner(0x10, || unsafe { create_hook(500) });
        assert_eq!(e as usize, RECORD.as_ptr() as usize);
        unsafe { update_hook(e, 0.02, std::ptr::null_mut()) };
        assert!((got() - 0.002).abs() < 1e-9);
        with_owner(0x20, || unsafe { create_hook(500) });
        unsafe { update_hook(e, 0.02, std::ptr::null_mut()) };
        assert_eq!(got(), 0.02);
        // the physics step: x PHYSX for the call, the caller's input given back
        let mut input = [1.0f32 / 60.0, 0.0];
        assert_eq!(unsafe { step_hook(std::ptr::null_mut(), input.as_mut_ptr(), std::ptr::null_mut()) }, 7);
        assert!((got() - 1.0 / 600.0).abs() < 1e-9);
        assert_eq!(input[0], 1.0 / 60.0);
        // no slow: the game's own deltas bit for bit
        ON.store(false, Ordering::SeqCst);
        unsafe { update_hook(e, 0.02, std::ptr::null_mut()) };
        assert_eq!(got(), 0.02);
        unsafe { step_hook(std::ptr::null_mut(), input.as_mut_ptr(), std::ptr::null_mut()) };
        assert_eq!(got(), 1.0 / 60.0);
        *TABLE.write().unwrap() = Table::NORMAL;
        uninstalled();
        assert!(!effects_hooked());
    }

    #[test]
    fn the_patterns_hold_what_the_detours_read() {
        assert!(STEP_FN.pattern.contains("f2 0f 10 02"), "movsd xmm0,[rdx]: the step input's delta");
        assert!(UPDATE_FN.pattern.starts_with("4c 89 44 24 18 f3 0f 11 4c 24 10"), "r8 = ctx, xmm1 = dt, rcx = the effect");
        assert!(CREATE_FN.pattern.contains("b9 90 00 00 00"), "the 0x90-byte record");
        assert!(PHASES.iter().all(|p| p.pattern.starts_with(PRE)), "every wrapper reads ChrIns + 0xB0");
        assert_eq!(EFFECT_SPEED, 4);
    }
}
