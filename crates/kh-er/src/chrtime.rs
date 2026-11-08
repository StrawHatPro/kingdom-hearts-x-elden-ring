//! (WORLDTIME-2) KH3'S WORLD SLOW ON ELDEN RING'S OWN CLOCKS: every enemy's whole time (its animation, its AI, its
//! physics and timers), its bullets in flight, Elden Ring's effects and the map's clock, slowed by KH3's factor while a
//! WorldSlow is on, generically (no per-enemy or per-bullet table). The rule (who is slowed by how much) is KH3's,
//! `kh_host::worldtime` (WorldSlow 0x14140E500 -> `SetTypeRates(1, 0x419C, 0.1, blend 0)`: ENEMY, GIMMICK, AVATAR, MAP,
//! BG and PHYSX at 0.1, at once; PLAYER, FRIEND, VEHICLE, CAMERA, PRIZE, EVENT and UI not; a projectile takes its
//! OWNER's type). This module is where Elden Ring keeps each of those clocks, read in eldenring.exe 2.7.1.0 and 2.7.2.0
//! (the masked byte patterns below are unique in both and checked by gamever.rs's PORT offline test):
//!
//! | clock | Elden Ring's own mechanism (2.7.1.0 / 2.7.2.0 RVA) | applied |
//! |---|---|---|
//! | a character's time | `ChrIns + 0xB0` (`chr_update_delta_time`) is THE delta of every per-character phase: the phase wrappers 0x3f8714 .. 0x3f9eb4 read it into an `FD4Time` and call the character's own phase slots (vt+0x78 .. +0xE0); the AI is one of them (WorldChrMan's job 0x510d70 -> 0x3f9dc0 -> ChrCtrl 0x3c91f0 -> 0x3cc5c0 -> the manipulator's vt+0x50, ComManipulator 0x3cf2b0 -> 0x3d0260 -> the AI's update 0x2c7370 with `FD4Time(dt)`). The Prepare phase (0x3f9630 / 0x3f96c0) makes it: `chr+0xB0 = HitStop(chr.modules + 0x90).update(chr+0xB0)` (0x44fe60 / 0x44ff40, Elden Ring's own per-character slow motion: the hit-stop's ramp in / hold / ramp out, `+0x18` the delta it hands back) | [`HITSTOP_FN`] detoured: the delta it is given is `dt x rate` for a slowed character, so its hit-stops, animation, root motion, TAE windows, AI goals and timers, special effects and physics all run on KH3's slowed time |
//! | a bullet's time | `CSBulletIns::Update(bullet, dt)` (0x39a110 in both; one caller, the manager's loop 0x3a8ed0 with the task's `FD4Time`): life `+0xB14 -= dt`, its state's `on_update(dt)` (fly / wait / explosion: speed, homing, acceleration), the motion `pos += v x dt` (0x39f860) | [`BULLET_FN`] detoured: `dt x rate` by the bullet's owner (`+0xB40`, `CSBulletTargetingSystemOwner::owner_chr_handle`, 0x3ac250 / getter 0x3ac360): a slowed character's bullet at ENEMY's rate, one of the player's side at 1, a bullet with no character owner (a trap) at GIMMICK's |
//! | Elden Ring's effects | (WORLDTIME-2 fx) PER OWNER: fxtime.rs (each effect's own update and speed, tagged with the update that made it). The global route below is only its fallback: `GXFfxSceneCtrl::Update` (vt slot 10, 0x1ca5e70 / 0x1ca7560) steps the whole effect world `dt x scene+0x20C` | fallback: `CSSfx -> scene_ctrl + 0x20C` x ENEMY's rate when fxtime.rs could not detour |
//! | the map's clock | `WorldAreaTimeImp::Update` (0x644a10 / 0x644ec0): the clock advances `(int)(dt x +0x38 x 1000)` ms | `WorldAreaTime::time_passage_multiplier` x MAP's rate |
//!
//! Every write is held as AUDIT-A's animation hold is (worldtime.rs `scale` / `give_back`): re-based on anybody else's
//! write, given back exactly at the end (only where nobody wrote since). The per-character and per-bullet rates are a
//! table published once a frame from the game thread and read by the detours (the Prepare phase runs on the game's
//! worker threads): no table, a busy table or no world slow = the game's own delta, untouched.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::RwLock;

use eldenring::cs::{CSBulletIns, CSBulletManager, CSBulletTargetingSystemOwner, CSFlipper, CSSfxImp, ChrIns, ChrSet, PlayerIns, WorldAreaTime, WorldChrMan};
use fromsoftware_shared::{FromStatic, Superclass};
use glam::Vec3;
use kh_host::worldtime::WorldRates;

use crate::erlog;
use crate::game::plausible;
use crate::havok::{locate, Located};
use crate::targets::{each_chr_of, ErTargets};
use crate::worldtime::{give_back, scale, Held};

/// Elden Ring's per-character hit-stop update (the character's own delta: see the module text).
pub(crate) const HITSTOP_FN: Located = Located {
    name: "character hit-stop update (the character's own delta)",
    pattern: "40 53 56 57 48 83 ec 50 f3 0f 10 41 10 33 f6 0f 29 74 24 40 48 8b d9 0f 29 7c 24 30 0f 57 f6 0f 2f c6 89 74 24 20 8b fe 0f 28 f9 f3 0f 11 79 1c",
    capture: 0,
    rva_2_7_1_0: 0x0044_fe60,
    rva_2_7_2_0: 0x0044_ff40,
};
/// `CSBulletIns::Update(bullet, dt)`.
pub(crate) const BULLET_FN: Located = Located {
    name: "bullet update (CSBulletIns, its own delta)",
    pattern: "48 8b c4 55 57 41 56 48 8d a8 d8 f7 ff ff 48 81 ec 10 09 00 00 48 c7 45 80 fe ff ff ff 48 89 58 18 48 89 70 20 0f 29 70 d8 0f 29 78 c8 48 8b 05 ? ? ? ? 48 33 c4 48 89 85 e0 07 00 00 0f 28 f9 48 8b f9 f3 0f 10 81 14 0b 00 00",
    capture: 0,
    rva_2_7_1_0: 0x0039_a110,
    rva_2_7_2_0: 0x0039_a110,
};
/// In `GXFfxSceneCtrl::Update`: `movaps xmm1,xmm7; mov rcx,[rdi+118h]; call; mulss xmm7,[rdi+20Ch]` (the effects' delta
/// times the scene's play-speed rate): the offset [`SCENE_RATE`] is read from the running executable's own code.
pub(crate) const SFX_STEP: Located = Located {
    name: "effects scene update: the play-speed rate (scene + 0x20C)",
    pattern: "0f 28 cf 48 8b 8f 18 01 00 00 e8 ? ? ? ? f3 0f 59 bf 0c 02 00 00 0f 28 cf 48 8b 0d ? ? ? ?",
    capture: 0,
    rva_2_7_1_0: 0x01ca_5f2d,
    rva_2_7_2_0: 0x01ca_761d,
};
/// In `WorldAreaTimeImp::Update`: `call; cmp [rbx+46h],r14b; je; cmp [rbx],r14; je; movaps xmm0,xmm6; mulss xmm0,[rbx+38h]`
/// (the clock's delta times `time_passage_multiplier`).
pub(crate) const CLOCK_STEP: Located = Located {
    name: "map clock update: the passage multiplier (WorldAreaTime + 0x38)",
    pattern: "e8 ? ? ? ? 44 38 73 46 74 25 4c 39 33 74 20 0f 28 c6 f3 0f 59 43 38",
    capture: 0,
    rva_2_7_1_0: 0x0064_4b4c,
    rva_2_7_2_0: 0x0064_4ffc,
};
/// For the PORT offline check (gamever.rs).
#[cfg(test)]
pub(crate) const LOCATED: [Located; 4] = [HITSTOP_FN, BULLET_FN, SFX_STEP, CLOCK_STEP];

/// `GXFfxSceneCtrl`'s vtable (RTTI `.?AVGXFfxSceneCtrl@GXFFX@@`), (2.7.1.0, 2.7.2.0): the scene's first word must be it.
pub const SCENE_VTABLE: (usize, usize) = (0x0305_d5e8, 0x0305_f888);
/// The scene's play-speed rate (`mulss xmm7,[rdi+20Ch]` in [`SFX_STEP`]).
pub const SCENE_RATE: usize = 0x20C;
/// A character module's owner (CSChrModuleBase ctor 0x43cf70: `mov [rcx+8],rdx`).
pub const MODULE_OWNER: usize = 8;
/// A bullet's owner handle (`CSBulletIns::targeting_owner.owner_chr_handle`; the owner's ctor 0x3ac250 at bullet +0xB30).
pub const BULLET_OWNER: usize = std::mem::offset_of!(CSBulletIns, targeting_owner) + std::mem::offset_of!(CSBulletTargetingSystemOwner, owner_chr_handle);
const _: () = assert!(BULLET_OWNER == 0xB40);
const _: () = assert!(std::mem::offset_of!(WorldAreaTime, time_passage_multiplier) == 0x38);
const _: () = assert!(std::mem::offset_of!(ChrIns, chr_update_delta_time) == 0xB0);

/// Who a character is to KH3's table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The local player: Sora's hidden body (PLAYER).
    Player,
    /// A KH party member's body (FRIEND).
    Party,
    /// The player's spirit summons (FRIEND).
    Summon,
    /// Torrent (VEHICLE).
    Mount,
    /// Another player or phantom (the player's side).
    OtherPlayer,
    /// An enemy (ENEMY): slowed.
    Enemy,
    /// A friendly NPC or a helper: not an enemy.
    Bystander,
}

/// What is known of one character.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub main_player: bool,
    pub party_body: bool,
    pub summon: bool,
    pub mount: bool,
    pub player_ins: bool,
    /// The target list's classification when it is listed (within its range): (hostile, friendly NPC, summon).
    pub listed: Option<(bool, bool, bool)>,
    /// Its team is an enemy team (the rule beyond the list's range; ermap.rs `hostile_team`).
    pub hostile_team: bool,
}

/// KH3's object type of an Elden Ring character.
pub fn side(f: &Facts) -> Side {
    if f.main_player {
        Side::Player
    } else if f.party_body {
        Side::Party
    } else if f.mount {
        Side::Mount
    } else if f.summon {
        Side::Summon
    } else if let Some((hostile, friendly, summon)) = f.listed {
        if summon {
            Side::Summon
        } else if hostile && !friendly {
            Side::Enemy
        } else if f.player_ins {
            Side::OtherPlayer
        } else {
            Side::Bystander
        }
    } else if f.player_ins {
        Side::OtherPlayer
    } else if f.hostile_team {
        Side::Enemy
    } else {
        Side::Bystander
    }
}

/// The rates the detours read, published once a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    /// ENEMY's rate (1 = none).
    pub enemy: f32,
    /// GIMMICK's rate: a bullet with no character owner.
    pub gimmick: f32,
    /// The slowed characters (object addresses), sorted.
    pub slowed_chr: Vec<usize>,
    /// Their handles (the 8 bytes), sorted.
    pub slowed_handle: Vec<u64>,
    /// Every other character's handle (the player's side, bystanders), sorted.
    pub other_handle: Vec<u64>,
    /// (WORLDTIME-2 fx) Every character's handle and KH3 object type, sorted by handle (the effects' and the physics' owners).
    pub sides: Vec<(u64, Side)>,
    /// The other types' rates: MAP (an effect of nobody's), PLAYER, FRIEND, VEHICLE, PHYSX.
    pub map: f32,
    pub player: f32,
    pub friend: f32,
    pub vehicle: f32,
    pub physx: f32,
}

impl Default for Table {
    fn default() -> Table {
        Table::NORMAL
    }
}

impl Table {
    /// The delta factor of the character at `chr`.
    pub fn chr_rate(&self, chr: usize) -> f32 {
        if self.slowed_chr.binary_search(&chr).is_ok() {
            self.enemy
        } else {
            1.0
        }
    }

    /// The delta factor of a bullet whose owner handle is `owner` (KH3: a projectile takes its owner's type).
    pub fn bullet_rate(&self, owner: u64) -> f32 {
        if self.slowed_handle.binary_search(&owner).is_ok() {
            self.enemy
        } else if self.other_handle.binary_search(&owner).is_ok() {
            1.0
        } else {
            self.gimmick
        }
    }

    pub fn on(&self) -> bool {
        [self.enemy, self.gimmick, self.map, self.player, self.friend, self.vehicle, self.physx].iter().any(|r| *r < 1.0)
    }

    /// Nothing slowed, no character known.
    pub const NORMAL: Table = Table { enemy: 1.0, gimmick: 1.0, slowed_chr: Vec::new(), slowed_handle: Vec::new(), other_handle: Vec::new(), sides: Vec::new(), map: 1.0, player: 1.0, friend: 1.0, vehicle: 1.0, physx: 1.0 };

    /// (WORLDTIME-2 fx) The delta factor of an effect whose owner (the character or the bullet owner whose update made it)
    /// has the handle `owner`; 0 = made by nobody's update (the map's, a map object's): MAP. KH3: a particle effect follows
    /// the type of the actor that owns it.
    pub fn fx_rate(&self, owner: u64) -> f32 {
        if owner == 0 {
            return self.map;
        }
        match self.sides.binary_search_by_key(&owner, |s| s.0) {
            Ok(i) => self.side_rate(self.sides[i].1),
            Err(_) => self.gimmick,
        }
    }

    /// The rate of a KH3 object type of a character.
    pub fn side_rate(&self, side: Side) -> f32 {
        match side {
            Side::Enemy => self.enemy,
            Side::Player => self.player,
            Side::Party | Side::Summon | Side::OtherPlayer => self.friend,
            Side::Mount => self.vehicle,
            Side::Bystander => 1.0,
        }
    }
}

static ORIGINAL_HITSTOP: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_BULLET: AtomicUsize = AtomicUsize::new(0);
/// The published table holds a world slow (the detours' fast path).
static ON: AtomicBool = AtomicBool::new(false);
pub(crate) static TABLE: RwLock<Table> = RwLock::new(Table::NORMAL);
/// Counters for the log: characters and bullets stepped on a slowed delta since the last read.
static CHR_SLOWED: AtomicU32 = AtomicU32::new(0);
static BULLET_SLOWED: AtomicU32 = AtomicU32::new(0);
/// Every character step through the detour, and the sampled enemy's: the delta the detour handed the game for it.
static CHR_CALLS: AtomicU32 = AtomicU32::new(0);
static SAMPLE_CHR: AtomicUsize = AtomicUsize::new(0);
static SAMPLE_DT: AtomicU64 = AtomicU64::new(0);
static SAMPLE_HITS: AtomicU32 = AtomicU32::new(0);
/// The last bullet stepped on a slowed delta, and its delta factor (bits), for the log's sample.
static BULLET_SEEN: AtomicUsize = AtomicUsize::new(0);
static BULLET_RATE: AtomicU64 = AtomicU64::new(0);
static SFX_OK: AtomicBool = AtomicBool::new(false);
static CLOCK_OK: AtomicBool = AtomicBool::new(false);

/// The per-character hit-stop update, detoured: the delta handed to it is the character's whole time.
unsafe extern "C" fn hitstop_hook(module: *mut u8, dt: f32) {
    let original = ORIGINAL_HITSTOP.load(Ordering::Relaxed);
    if original == 0 {
        return;
    }
    // SAFETY: the trampoline of the function we detoured, with its own signature (rcx = the module, xmm1 = the delta).
    let original: unsafe extern "C" fn(*mut u8, f32) = unsafe { std::mem::transmute(original) };
    let mut dt = dt;
    CHR_CALLS.fetch_add(1, Ordering::Relaxed);
    if ON.load(Ordering::Relaxed) && !module.is_null() {
        // SAFETY: the game's module object, alive while the game updates it; its owner word is at +8.
        let chr = unsafe { std::ptr::read_unaligned(module.add(MODULE_OWNER) as *const usize) };
        if let Ok(t) = TABLE.try_read() {
            let r = t.chr_rate(chr);
            if r < 1.0 {
                dt *= r;
                CHR_SLOWED.fetch_add(1, Ordering::Relaxed);
            }
        }
        if chr == SAMPLE_CHR.load(Ordering::Relaxed) {
            SAMPLE_DT.store(u64::from(dt.to_bits()), Ordering::Relaxed);
            SAMPLE_HITS.fetch_add(1, Ordering::Relaxed);
        }
    }
    // SAFETY: as above.
    unsafe { original(module, dt) }
}

/// `CSBulletIns::Update`, detoured: a bullet's delta by its owner's type.
unsafe extern "C" fn bullet_hook(bullet: *mut u8, dt: f32) -> u8 {
    let original = ORIGINAL_BULLET.load(Ordering::Relaxed);
    if original == 0 {
        return 0;
    }
    // SAFETY: the trampoline of the function we detoured (rcx = the bullet, xmm1 = the delta, al = alive).
    let original: unsafe extern "C" fn(*mut u8, f32) -> u8 = unsafe { std::mem::transmute(original) };
    let mut dt = dt;
    // SAFETY: the game's bullet object in its manager's loop; the owner handle's 8 bytes at +0xB40.
    let owner = if bullet.is_null() { 0 } else { unsafe { std::ptr::read_unaligned(bullet.add(BULLET_OWNER) as *const u64) } };
    if ON.load(Ordering::Relaxed) && !bullet.is_null() {
        if let Ok(t) = TABLE.try_read() {
            let r = t.bullet_rate(owner);
            if r < 1.0 {
                dt *= r;
                BULLET_SLOWED.fetch_add(1, Ordering::Relaxed);
                BULLET_SEEN.store(bullet as usize, Ordering::Relaxed);
                BULLET_RATE.store(u64::from(r.to_bits()), Ordering::Relaxed);
            }
        }
    }
    // (WORLDTIME-2 fx) the effects this update makes (its trail, its explosion) are its owner's
    // SAFETY: as above.
    crate::fxtime::with_owner(owner, || unsafe { original(bullet, dt) })
}

/// Detours the two updates and checks the two fields' code (once per payload, entry.rs; undone with the renderer's
/// other inline hooks). What is not found stays Elden Ring's own (logged); worldtime.rs then keeps AUDIT-A's
/// animation-speed route for the enemies.
pub fn install() {
    for (item, hook, original, what) in [
        (&HITSTOP_FN, hitstop_hook as *const () as usize, &ORIGINAL_HITSTOP, "character hit-stop update"),
        (&BULLET_FN, bullet_hook as *const () as usize, &ORIGINAL_BULLET, "bullet update"),
    ] {
        let Some(at) = locate(item) else {
            erlog!("worldtime2: the {what} was not found for this executable: that clock stays Elden Ring's own under KH3's world slow");
            continue;
        };
        // SAFETY: the function's start, found by its unique pattern at the version's recorded address; our hook has its signature.
        match unsafe { kh_er_render::inline_hook(at, hook, original, what) } {
            Ok(()) => erlog!("worldtime2: the {what} is detoured (KH3's world slow reaches it; untouched otherwise)"),
            Err(e) => {
                original.store(0, Ordering::SeqCst);
                erlog!("worldtime2: the {what} was NOT detoured ({e}): that clock stays Elden Ring's own");
            }
        }
    }
    crate::fxtime::install(); // (WORLDTIME-2 fx) the effects per owner, the physics step
    SFX_OK.store(locate(&SFX_STEP).is_some(), Ordering::SeqCst);
    CLOCK_OK.store(locate(&CLOCK_STEP).is_some(), Ordering::SeqCst);
    erlog!(
        "worldtime2: effects' play-speed rate (scene + {SCENE_RATE:#x}) {}, map clock passage (WorldAreaTime + 0x38) {}",
        if SFX_OK.load(Ordering::Relaxed) { "read in the code" } else { "NOT found: left alone" },
        if CLOCK_OK.load(Ordering::Relaxed) { "read in the code" } else { "NOT found: left alone" }
    );
}

/// The hooks are gone (the renderer's shutdown removed every inline hook).
pub fn uninstalled() {
    crate::fxtime::uninstalled(); // (WORLDTIME-2 fx)
    ON.store(false, Ordering::SeqCst);
    ORIGINAL_HITSTOP.store(0, Ordering::SeqCst);
    ORIGINAL_BULLET.store(0, Ordering::SeqCst);
}

/// The characters' own delta is ours to scale (else worldtime.rs scales their animation speed).
pub fn chr_hooked() -> bool {
    ORIGINAL_HITSTOP.load(Ordering::Relaxed) != 0
}

fn publish(table: Table) {
    let on = table.on();
    if let Ok(mut t) = TABLE.write() {
        *t = table;
    }
    ON.store(on && (chr_hooked() || ORIGINAL_BULLET.load(Ordering::Relaxed) != 0), Ordering::SeqCst);
    crate::fxtime::frame(on); // (WORLDTIME-2 fx)
}

/// One held float of the game's (the hold rule of worldtime.rs).
#[derive(Default)]
struct Hold {
    held: Option<Held>,
}

impl Hold {
    /// The value to write for the rate (None: nothing to write).
    fn apply(&mut self, current: f32, rate: f32) -> Option<f32> {
        match scale(self.held, current, rate) {
            Some(h) => {
                self.held = Some(h);
                (h.wrote != current).then_some(h.wrote)
            }
            None => self.release(current),
        }
    }

    /// The end: the value to put back (None: nothing, or somebody else's write stays).
    fn release(&mut self, current: f32) -> Option<f32> {
        self.held.take().and_then(|h| give_back(h, current))
    }
}

/// One sampled enemy's numbers between two log lines.
#[derive(Clone, Copy, Debug, Default)]
struct Sample {
    id: u32,
    anim: i32,
    anim_time: f32,
    real: f32,
}

/// One sampled bullet's place.
#[derive(Clone, Copy, Debug, Default)]
struct BulletSample {
    ptr: usize,
    owner: u64,
    pos: Vec3,
    real: f32,
}

/// The game-thread side: publishes the table, holds the effects' and the clock's fields, logs the proof.
#[derive(Default)]
pub struct ErClocks {
    sfx: Hold,
    clock: Hold,
    was_on: bool,
    /// Seconds since the last log line, and seconds of logging left after the slow ended.
    tick: f32,
    tail: f32,
    sample: Option<Sample>,
    bullet: Option<BulletSample>,
    counts: (usize, usize),
    last: Option<std::time::Instant>,
    /// `world.proof = 1` (a testing aid, off by default): the proof lines also while nothing is slowed.
    pub always: bool,
    /// `world.test_shot = N` (a testing aid; a change of the value fires once): the nearest living enemy fires one bullet
    /// of the game's own at Sora through the game's spawn (`CSBulletManager::spawn_bullet`, owner = the enemy). `N` = a
    /// Bullet row id, or `auto`: the first row that flies (initial velocity 8 .. 25 m/s, life 1.5 .. 6 s, an attack row,
    /// an effect, no hit bullet).
    pub test_shot: Option<String>,
    shot_seen: Option<String>,
    /// (WORLDTIME-2 fx) `world.test_fx = SpEffect[:fxr,fxr]` (a testing aid; a change fires once): the special effect put on
    /// the player's body AND the nearest living enemy (the same effect on two owners), its FXR ids watched in the proof
    /// lines; `0` takes it off both again.
    pub test_fx: Option<String>,
    fx_seen: Option<String>,
    fx_on: Option<(i32, eldenring::cs::FieldInsHandle)>,
    /// The nearest dead enemy's place (a ragdoll: PHYSX), for the proof.
    fallen: Option<(u32, Vec3, std::time::Instant)>,
}

impl ErClocks {
    /// ONCE A FRAME (worldtime.rs `post`), game thread: `rates` = the world's rates (the table of Sora's last step).
    pub fn frame(&mut self, rates: &WorldRates, targets: &ErTargets) {
        let now = std::time::Instant::now();
        let real_dt = self.last.map_or(0.0, |t| now.duration_since(t).as_secs_f32()).min(0.5);
        self.last = Some(now);
        let on = [rates.enemy_type, rates.gimmick_type, rates.map_type, rates.physx_type, rates.player_type, rates.friend_type, rates.vehicle_type].iter().any(|r| *r < 1.0);
        if on || self.always {
            let table = classify(targets, rates);
            self.counts = (table.slowed_chr.len(), table.other_handle.len());
            publish(table);
        } else if self.was_on {
            publish(Table { enemy: 1.0, gimmick: 1.0, ..Table::default() });
        }
        // (WORLDTIME-2 fx) the global play-speed route only where the effects could not be scaled per owner (fxtime.rs)
        self.effects(if on && !crate::fxtime::effects_hooked() { rates.enemy_type } else { 1.0 });
        self.map_clock(if on { rates.map_type } else { 1.0 });
        if on != self.was_on {
            erlog!(
                "worldtime2: KH3's world slow {} on Elden Ring's own clocks: {} character(s) at x{:.3} on their own delta ({}), their bullets and the ownerless ones at x{:.3} / x{:.3} ({}), effects x{:.3}, map clock x{:.3}; {} of the player's side / bystanders at x1 (the player, the party, summons, Torrent)",
                if on { "ON" } else { "OFF" },
                self.counts.0,
                rates.enemy_type,
                if chr_hooked() { "the hit-stop update" } else { "NOT detoured: animation speed only" },
                rates.enemy_type,
                rates.gimmick_type,
                if ORIGINAL_BULLET.load(Ordering::Relaxed) != 0 { "the bullet update" } else { "NOT detoured" },
                rates.enemy_type,
                rates.map_type,
                self.counts.1
            );
            self.tick = 1.0;
            self.tail = if on { 0.0 } else { 2.0 };
        }
        self.was_on = on;
        let shot = self.test_shot.clone().unwrap_or_default();
        match &self.shot_seen {
            Some(seen) if *seen == shot => {}
            Some(_) => {
                if !shot.is_empty() {
                    test_shot(&shot, targets);
                }
                self.shot_seen = Some(shot);
            }
            None => self.shot_seen = Some(shot),
        }
        let fx = self.test_fx.clone().unwrap_or_default();
        match &self.fx_seen {
            Some(seen) if *seen == fx => {}
            Some(_) => {
                self.fx_test(&fx, targets);
                self.fx_seen = Some(fx);
            }
            None => self.fx_seen = Some(fx),
        }
        self.prove(on, rates, targets, real_dt);
    }

    /// `world.test_fx`.
    fn fx_test(&mut self, text: &str, targets: &ErTargets) {
        use eldenring::cs::ChrInsExt;
        // SAFETY: game thread; the singleton lives as long as the game.
        let Some(player) = (unsafe { WorldChrMan::instance_mut() }).ok().and_then(|w| w.main_player.as_deref_mut()) else { return };
        if let Some((sp, handle)) = self.fx_on.take() {
            player.chr_ins.remove_speffect(sp);
            if let Some(chr) = crate::targets::find_chr(&handle) {
                chr.remove_speffect(sp);
            }
            erlog!("worldtime2: world.test_fx: SpEffect {sp} taken off the player and the enemy");
        }
        let (sp, ids) = text.split_once(':').unwrap_or((text, ""));
        // (`-N`: SpEffect N taken off the player and the nearest enemy, whoever put it there)
        if let Some(off) = sp.trim().strip_prefix('-').and_then(|n| n.parse::<i32>().ok()) {
            player.chr_ins.remove_speffect(off);
            for e in targets.list.iter().filter(|e| e.alive && e.hostile) {
                if let Some(chr) = targets.chr_mut(e.id) {
                    chr.remove_speffect(off);
                }
            }
            erlog!("worldtime2: world.test_fx: SpEffect {off} taken off the player and the enemies near");
            return;
        }
        let Some(sp) = sp.trim().parse::<i32>().ok().filter(|s| *s > 0) else {
            crate::fxtime::WATCH[0].store(0, Ordering::Relaxed);
            return;
        };
        let ids: Vec<u32> = ids.split(',').filter_map(|i| i.trim().parse().ok()).collect();
        for (k, w) in crate::fxtime::WATCH.iter().enumerate() {
            w.store(ids.get(k).copied().unwrap_or(0), Ordering::Relaxed);
        }
        let Some(e) = targets.list.iter().filter(|e| e.alive && e.active && e.hostile && !e.friendly && !e.summon).min_by(|a, b| (a.pos - targets.sora).length().total_cmp(&(b.pos - targets.sora).length())) else {
            erlog!("worldtime2: world.test_fx = {text}: no living enemy near Sora");
            return;
        };
        // (as the game applies one inside the character's own update: its effects are that character's)
        let me = crate::game::handle_bits(&player.chr_ins.field_ins_handle);
        crate::fxtime::with_owner(me, || player.chr_ins.apply_speffect(sp, false));
        if let Some(chr) = targets.chr_mut(e.id) {
            crate::fxtime::with_owner(crate::game::handle_bits(&e.handle), || chr.apply_speffect(sp, false));
        }
        self.fx_on = Some((sp, e.handle));
        erlog!("worldtime2: world.test_fx = {text}: SpEffect {sp} on the player's body and on target {} (c{:04}); FXR {ids:?} watched", e.id.0, e.npc_id);
    }

    fn effects(&mut self, rate: f32) {
        if !SFX_OK.load(Ordering::Relaxed) {
            return;
        }
        let Some(rate_at) = scene_rate() else { return };
        // SAFETY: the scene's play-speed float (checked: the scene object's vtable is GXFfxSceneCtrl's).
        let current = unsafe { std::ptr::read_unaligned(rate_at) };
        if let Some(v) = self.sfx.apply(current, rate) {
            // SAFETY: as above; a 4-byte float the effect update reads once a frame.
            unsafe { std::ptr::write_unaligned(rate_at, v) };
        }
    }

    fn map_clock(&mut self, rate: f32) {
        if !CLOCK_OK.load(Ordering::Relaxed) {
            return;
        }
        // SAFETY: game thread (a task phase); the singleton lives as long as the game.
        let Ok(time) = (unsafe { WorldAreaTime::instance_mut() }) else { return };
        let current = time.time_passage_multiplier;
        if let Some(v) = self.clock.apply(current, rate) {
            time.time_passage_multiplier = v;
        }
    }

    /// Everything back now (Sora stops, a load, `world.kh = 0`, the payload's shutdown).
    pub fn stop(&mut self) {
        publish(Table { enemy: 1.0, gimmick: 1.0, ..Table::default() });
        if let Some(rate_at) = scene_rate().filter(|_| SFX_OK.load(Ordering::Relaxed)) {
            // SAFETY: as in `effects`.
            let current = unsafe { std::ptr::read_unaligned(rate_at) };
            if let Some(v) = self.sfx.release(current) {
                // SAFETY: as in `effects`.
                unsafe { std::ptr::write_unaligned(rate_at, v) };
            }
        }
        self.sfx = Hold::default();
        // SAFETY: as in `map_clock`.
        if let Ok(time) = unsafe { WorldAreaTime::instance_mut() } {
            if let Some(v) = self.clock.release(time.time_passage_multiplier) {
                time.time_passage_multiplier = v;
            }
        }
        self.clock = Hold::default();
        self.was_on = false;
    }

    /// The proof in the log: every 0.5 s while the world is slow and for 2 s after, the nearest slowed enemy's own
    /// delta (what its AI and its animation were handed this frame) against the frame's, its animation's progress
    /// against real time, and one bullet of the slowed side's metres per real second.
    fn prove(&mut self, on: bool, rates: &WorldRates, targets: &ErTargets, real_dt: f32) {
        if !on && self.tail <= 0.0 && !self.always {
            self.sample = None;
            self.bullet = None;
            return;
        }
        self.tail -= if on { 0.0 } else { real_dt };
        self.tick += real_dt;
        if let Some(s) = self.sample.as_mut() {
            s.real += real_dt;
        }
        if let Some(b) = self.bullet.as_mut() {
            b.real += real_dt;
        }
        if self.tick < 0.5 {
            return;
        }
        self.tick = 0.0;
        let chrs = CHR_SLOWED.swap(0, Ordering::Relaxed);
        let calls = CHR_CALLS.swap(0, Ordering::Relaxed);
        let bullets = BULLET_SLOWED.swap(0, Ordering::Relaxed);
        let hits = SAMPLE_HITS.swap(0, Ordering::Relaxed);
        let handed = f32::from_bits(SAMPLE_DT.load(Ordering::Relaxed) as u32);
        // (a living one: a dead body is not stepped through the Prepare phase, its delta is the raw one)
        let nearest = targets.list.iter().filter(|e| e.alive && e.active && e.hostile && !e.friendly && !e.summon).min_by(|a, b| (a.pos - targets.sora).length().total_cmp(&(b.pos - targets.sora).length()));
        let mut text = format!("worldtime2: proof ({}, enemies x{:.3}): {chrs} slowed character step(s) of {calls}, {bullets} slowed bullet step(s) since the last line", if on { "SLOW" } else { "after" }, rates.enemy_type);
        // the delta the game handed this frame's tasks (CSFlipper::task_delta: the hit-stops' game speed included)
        // SAFETY: game thread; the singleton lives as long as the game.
        let frame_dt = unsafe { CSFlipper::instance() }.map_or(real_dt, |f| f.task_delta);
        if let Some(e) = nearest {
            if let Some(chr) = targets.chr_mut(e.id) {
                let own = chr.chr_update_delta_time;
                let anim = crate::chranim::now(chr);
                let pace = match self.sample {
                    Some(s) if s.id == e.id.0 && s.anim == anim.id && s.real > 0.0 => format!(", animation {} at {:.3} s: {:.3} s of it per real second", anim.id, anim.time, (anim.time - s.anim_time) / s.real),
                    _ => format!(", animation {} at {:.3} s", anim.id, anim.time),
                };
                let ptr = chr as *const ChrIns as usize;
                let listed = TABLE.read().map(|t| t.chr_rate(ptr)).unwrap_or(1.0);
                text += &format!(
                    "; target {} (c{:04}, table x{listed:.3}, {hits} step(s) through the detour, last handed {handed:.5} s): its own delta {own:.5} s = x{:.3} of the frame's {frame_dt:.5} s (the delta its AI think and its behaviour are given){pace}",
                    e.id.0, e.npc_id, if frame_dt > 0.0 { own / frame_dt } else { 0.0 }
                );
                SAMPLE_CHR.store(ptr, Ordering::Relaxed);
                self.sample = Some(Sample { id: e.id.0, anim: anim.id, anim_time: anim.time, real: 0.0 });
            }
        }
        text += &self.bullet_line();
        text += &crate::fxtime::proof_text(); // (WORLDTIME-2 fx)
        text += &self.fallen_line(targets);
        let traces = crate::fxtime::take_traces();
        if !traces.is_empty() {
            text += &format!("; effects made by nobody's update, by call site: {traces}");
        }
        // the effects' play-speed rate and the map clock's passage as the game reads them now
        // SAFETY: as in `effects`.
        let sfx = scene_rate().map(|p| unsafe { std::ptr::read_unaligned(p) });
        // SAFETY: game thread; the singleton lives as long as the game.
        let clock = unsafe { WorldAreaTime::instance() }.ok().map(|t| format!("x{} at {:02}:{:02}:{:02}", t.time_passage_multiplier, t.clock.hours(), t.clock.minutes(), t.clock.seconds()));
        text += &format!("; effects play-speed {}, map clock passage {}", sfx.map_or("-".to_string(), |v| format!("x{v}")), clock.unwrap_or_else(|| "-".to_string()));
        erlog!("{text}");
    }

    /// (WORLDTIME-2 fx) The nearest dead enemy (its body in the game's physics, a ragdoll): metres per real second.
    fn fallen_line(&mut self, targets: &ErTargets) -> String {
        let Some(e) = targets.list.iter().filter(|e| !e.alive && e.hostile).min_by(|a, b| (a.pos - targets.sora).length().total_cmp(&(b.pos - targets.sora).length())) else {
            self.fallen = None;
            return String::new();
        };
        let now = std::time::Instant::now();
        let line = match self.fallen {
            Some((id, pos, at)) if id == e.id.0 && now > at => format!("; fallen target {} (c{:04}): its body moved {:.2} m per real second", id, e.npc_id, (e.er_pos - pos).length() / now.duration_since(at).as_secs_f32()),
            _ => format!("; fallen target {} (c{:04}): first seen", e.id.0, e.npc_id),
        };
        self.fallen = Some((e.id.0, e.er_pos, now));
        line
    }

    /// One bullet of a slowed owner (or the last one the detour slowed): metres per real second since the last line.
    fn bullet_line(&mut self) -> String {
        // SAFETY: game thread; the singleton lives as long as the game; the list is the manager's own.
        let Ok(man) = (unsafe { CSBulletManager::instance_mut() }) else { return String::new() };
        let table = TABLE.read().map(|t| t.clone()).unwrap_or_default();
        // (rank: the one followed since the last line; a flying one of a slowed character; a flying one of nobody's
        // (GIMMICK); any flying one: an emitter that stands still proves nothing)
        let mut pick: Option<(u8, usize, u64, Vec3, f32, f32)> = None;
        let (mut alive, mut flying_n) = (0, 0);
        for b in man.bullets() {
            alive += 1;
            let ptr = b as *const CSBulletIns as usize;
            let owner = crate::game::handle_bits(&b.targeting_owner.owner_chr_handle);
            let pos = Vec3::new(b.physics.position.0, b.physics.position.1, b.physics.position.2);
            let speed = Vec3::new(b.physics.velocity.0, b.physics.velocity.1, b.physics.velocity.2).length();
            let flying = speed > 0.5;
            flying_n += usize::from(flying);
            let rank = if self.bullet.is_some_and(|s| s.ptr == ptr && s.owner == owner) {
                0
            } else if flying && table.slowed_handle.binary_search(&owner).is_ok() {
                1
            } else if flying && table.other_handle.binary_search(&owner).is_err() {
                2
            } else if flying {
                3
            } else {
                continue;
            };
            if pick.is_none_or(|p| rank < p.0) {
                pick = Some((rank, ptr, owner, pos, table.bullet_rate(owner), speed));
            }
        }
        let Some((rank, ptr, owner, pos, rate, speed)) = pick else {
            self.bullet = None;
            let seen = BULLET_SEEN.load(Ordering::Relaxed);
            return format!("; {alive} bullet(s) alive, none flying{}", if seen != 0 { format!(" (the last one the detour slowed: x{:.3})", f32::from_bits(BULLET_RATE.load(Ordering::Relaxed) as u32)) } else { String::new() });
        };
        let whose = match rank {
            1 => "a slowed character's",
            2 => "no character's (GIMMICK)",
            _ if table.other_handle.binary_search(&owner).is_ok() => "the player's side's",
            _ => "followed",
        };
        let line = match self.bullet {
            Some(s) if s.ptr == ptr && s.owner == owner && s.real > 0.0 => {
                let real = (pos - s.pos).length() / s.real;
                format!("; {alive} bullet(s) alive ({flying_n} flying); bullet {ptr:#x} ({whose}, owner {owner:#018x}) at x{rate:.3}: its velocity {speed:.2} m/s of game time, moved {real:.2} m per real second = x{:.3}", if speed > 0.0 { real / speed } else { 0.0 })
            }
            _ => format!("; {alive} bullet(s) alive ({flying_n} flying); bullet {ptr:#x} ({whose}, owner {owner:#018x}) at x{rate:.3}: velocity {speed:.2} m/s, first seen"),
        };
        self.bullet = Some(BulletSample { ptr, owner, pos, real: 0.0 });
        line
    }
}

/// `world.test_shot`: the nearest living enemy's bullet at Sora (the game's own spawn; its owner is the enemy).
fn test_shot(text: &str, targets: &ErTargets) {
    use eldenring::cs::{Bullet, SoloParamRepository};
    let Some(e) = targets.list.iter().filter(|e| e.alive && e.active && e.hostile && !e.friendly && !e.summon).min_by(|a, b| (a.pos - targets.sora).length().total_cmp(&(b.pos - targets.sora).length())) else {
        erlog!("worldtime2: world.test_shot = {text}: no living enemy near Sora");
        return;
    };
    // SAFETY: game thread; the singleton lives as long as the game.
    let Some(player) = (unsafe { WorldChrMan::instance() }).ok().and_then(|w| w.main_player.as_deref()) else { return };
    let to = crate::game::v3(&player.chr_ins.modules.physics.position) + Vec3::new(0.0, 1.0, 0.0);
    // SAFETY: as above.
    let Ok(repo) = (unsafe { SoloParamRepository::instance() }) else { return };
    let row = match text.trim() {
        "auto" => repo.rows::<Bullet>().find(|(_, b)| (8.0..=25.0).contains(&b.init_vellocity()) && (1.5..=6.0).contains(&b.life()) && b.atk_id_bullet() > 0 && b.sfx_id_bullet() > 0 && b.hit_bullet_id() < 0).map(|(id, _)| id),
        t => t.parse::<u32>().ok(),
    };
    let Some(id) = row else {
        erlog!("worldtime2: world.test_shot = {text}: no such Bullet row");
        return;
    };
    let (speed, life) = repo.get::<Bullet>(id).map_or((0.0, 0.0), |b| (b.init_vellocity(), b.life()));
    let from = e.er_pos + Vec3::new(0.0, 1.5, 0.0);
    let dir = (to - from).normalize_or_zero();
    let shape = crate::bullet::Shape { flags: crate::bullet::ONE_SHOT, ..crate::bullet::Shape::default() };
    let result = crate::bullet::spawn(e.handle, Some(player.chr_ins.field_ins_handle), from, dir, id, shape);
    erlog!("worldtime2: world.test_shot = {text}: target {} (c{:04}) fires Bullet {id} (initial velocity {speed} m/s, life {life} s) at Sora from {:.1} m: {result:?}", e.id.0, e.npc_id, (to - from).length());
}

/// The play-speed float of the effects scene (None: no scene, or not the object the code was read for).
fn scene_rate() -> Option<*mut f32> {
    // SAFETY: game thread; the singleton lives as long as the game; every pointer is checked before it is followed.
    let sfx = unsafe { CSSfxImp::instance() }.ok()?;
    let scene = crate::word!(sfx.scene_ctrl);
    if !plausible(scene) {
        return None;
    }
    let want = crate::gamever::pick(SCENE_VTABLE.0, SCENE_VTABLE.1)? + crate::entry::exe_base();
    // SAFETY: a plausible heap object's first word.
    let vtable = unsafe { std::ptr::read_unaligned(scene as *const usize) };
    (vtable == want).then_some((scene + SCENE_RATE) as *mut f32)
}

/// Every character of the loaded world, by KH3's object type (the target list's word where it has one).
fn classify(targets: &ErTargets, rates: &WorldRates) -> Table {
    let mut table = Table { enemy: rates.enemy_type, gimmick: rates.gimmick_type, map: rates.map_type, player: rates.player_type, friend: rates.friend_type, vehicle: rates.vehicle_type, physx: rates.physx_type, ..Table::default() };
    // SAFETY: game thread (a task phase); the singleton lives as long as the game; every pointer is checked.
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return table };
    let player = wcm.main_player.as_deref().map_or(0, |p| p as *const PlayerIns as usize);
    let mut all: Vec<usize> = Vec::with_capacity(512);
    let mut summons: Vec<usize> = Vec::new();
    // SAFETY: the game's character sets, read as targets.rs reads them.
    unsafe {
        each_chr_of(&wcm.summon_buddy_chr_set, &mut |p| summons.push(p));
        let mut visit = |p: usize| {
            if all.len() < 4096 && !all.contains(&p) {
                all.push(p);
            }
        };
        for k in 0..wcm.chr_sets.len() {
            let w = crate::word!(wcm.chr_sets[k]);
            if plausible(w) {
                each_chr_of(&*(w as *const ChrSet<ChrIns>), &mut visit);
            }
        }
        each_chr_of(&wcm.open_field_chr_set.base, &mut visit);
        each_chr_of(&wcm.summon_buddy_chr_set, &mut visit);
    }
    if player != 0 && !all.contains(&player) {
        all.push(player);
    }
    for p in all {
        // SAFETY: a pointer of a character-set entry, checked plausible; its handle is its second word.
        let chr = unsafe { &*(p as *const ChrIns) };
        let bits = crate::game::handle_bits(&chr.field_ins_handle);
        let listed = targets.list.iter().find(|e| crate::game::handle_bits(&e.handle) == bits).map(|e| (e.hostile, e.friendly, e.summon));
        #[cfg(feature = "party")]
        let party_body = crate::allybody::is_body(p);
        #[cfg(not(feature = "party"))]
        let party_body = false;
        let facts = Facts {
            main_player: p == player,
            party_body,
            summon: summons.contains(&p),
            mount: chr.npc_id == 8000,
            player_ins: chr.as_subclass::<PlayerIns>().is_some(),
            listed,
            hostile_team: crate::ermap::hostile_team(chr.team_type),
        };
        let kind = side(&facts);
        table.sides.push((bits, kind));
        if kind == Side::Enemy {
            table.slowed_chr.push(p);
            table.slowed_handle.push(bits);
        } else {
            table.other_handle.push(bits);
        }
    }
    table.slowed_chr.sort_unstable();
    table.slowed_handle.sort_unstable();
    table.other_handle.sort_unstable();
    table.sides.sort_unstable_by_key(|s| s.0);
    table.sides.dedup_by_key(|s| s.0);
    table
}

/// (WORLDTIME-2 fx) The tests that publish the shared table run one at a time.
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    fn enemy_table() -> Table {
        let mut t = Table { enemy: 0.1, gimmick: 0.1, slowed_chr: vec![0x3000, 0x1000], slowed_handle: vec![0x30, 0x10], other_handle: vec![0x99, 0x20], ..Table::NORMAL };
        t.slowed_chr.sort_unstable();
        t.slowed_handle.sort_unstable();
        t.other_handle.sort_unstable();
        t
    }

    #[test]
    fn kh3s_object_types_of_elden_rings_characters() {
        let enemy = Facts { listed: Some((true, false, false)), ..Facts::default() };
        assert_eq!(side(&enemy), Side::Enemy);
        assert_eq!(side(&Facts { main_player: true, ..enemy }), Side::Player, "Sora's hidden body is PLAYER");
        assert_eq!(side(&Facts { party_body: true, ..enemy }), Side::Party, "a party body is FRIEND whatever its team says");
        assert_eq!(side(&Facts { listed: Some((true, false, true)), ..Facts::default() }), Side::Summon);
        assert_eq!(side(&Facts { summon: true, hostile_team: true, ..Facts::default() }), Side::Summon);
        assert_eq!(side(&Facts { mount: true, ..Facts::default() }), Side::Mount, "Torrent is VEHICLE");
        assert_eq!(side(&Facts { listed: Some((false, true, false)), ..Facts::default() }), Side::Bystander, "a friendly NPC");
        assert_eq!(side(&Facts { listed: Some((true, true, false)), ..Facts::default() }), Side::Bystander, "friendly by the list's word");
        // beyond the list's range: the team
        assert_eq!(side(&Facts { hostile_team: true, ..Facts::default() }), Side::Enemy, "a far archer is an enemy too");
        assert_eq!(side(&Facts { hostile_team: false, ..Facts::default() }), Side::Bystander);
        assert_eq!(side(&Facts { player_ins: true, hostile_team: true, ..Facts::default() }), Side::OtherPlayer);
        // a hostile phantom within range is an enemy by the list's word
        assert_eq!(side(&Facts { player_ins: true, listed: Some((true, false, false)), ..Facts::default() }), Side::Enemy);
    }

    #[test]
    fn a_slowed_characters_delta_and_nobody_elses() {
        let t = enemy_table();
        assert_eq!(t.chr_rate(0x1000), 0.1);
        assert_eq!(t.chr_rate(0x3000), 0.1);
        assert_eq!(t.chr_rate(0x2000), 1.0, "the player, the party and anybody not listed keep the game's delta");
        assert!(t.on());
        let off = Table { enemy: 1.0, gimmick: 1.0, ..t.clone() };
        assert!(!off.on());
        assert_eq!(off.chr_rate(0x1000), 1.0, "WorldSlow off: x1");
    }

    #[test]
    fn a_bullet_takes_its_owners_type() {
        let t = enemy_table();
        assert_eq!(t.bullet_rate(0x10), 0.1, "an enemy's arrow in flight");
        assert_eq!(t.bullet_rate(0x20), 1.0, "the player's (Sora's hits) and the party's bullets");
        assert_eq!(t.bullet_rate(0x77), 0.1, "no character owner (a trap): GIMMICK");
        assert_eq!(t.bullet_rate(u64::MAX), 0.1, "the empty handle: GIMMICK");
        let aim = Table { enemy: 0.4, gimmick: 0.4, ..t };
        assert_eq!(aim.bullet_rate(0x30), 0.4);
    }

    #[test]
    fn the_effects_and_the_clock_are_held_and_given_back_exactly() {
        let mut h = Hold::default();
        // the clock's own multiplier (say 1.0) slowed, kept while it plays what we wrote
        assert_eq!(h.apply(1.0, 0.1), Some(0.1));
        assert_eq!(h.apply(0.1, 0.1), None);
        // the end: back to 1.0
        assert_eq!(h.apply(0.1, 1.0), Some(1.0));
        assert_eq!(h.apply(1.0, 1.0), None);
        // somebody (frame.rs `time.rate`) writes 4.0 meanwhile: the new base, and theirs stays at the end if written last
        assert_eq!(h.apply(1.0, 0.1), Some(0.1));
        assert_eq!(h.apply(4.0, 0.1), Some(0.4));
        assert_eq!(h.release(9.0), None, "a write after ours stays");
        assert_eq!(h.apply(1.0, 0.05), Some(0.05));
        assert_eq!(h.release(0.05), Some(1.0));
    }

    /// The two detours on mocked game objects: a hit-stop module (owner word at +8) and a bullet (owner handle at
    /// +0xB40), with stand-ins for the game's functions that record the delta they were handed.
    #[test]
    fn the_detours_hand_the_game_the_slowed_delta_on_mocked_objects() {
        let _one = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        static GOT: AtomicU64 = AtomicU64::new(0);
        unsafe extern "C" fn game_hitstop(_module: *mut u8, dt: f32) {
            GOT.store(u64::from(dt.to_bits()), Ordering::SeqCst);
        }
        unsafe extern "C" fn game_bullet(_bullet: *mut u8, dt: f32) -> u8 {
            GOT.store(u64::from(dt.to_bits()), Ordering::SeqCst);
            1
        }
        let got = || f32::from_bits(GOT.load(Ordering::SeqCst) as u32);
        ORIGINAL_HITSTOP.store(game_hitstop as *const () as usize, Ordering::SeqCst);
        ORIGINAL_BULLET.store(game_bullet as *const () as usize, Ordering::SeqCst);
        let enemy = 0x7000_0000_1000usize;
        let player = 0x7000_0000_2000usize;
        let module_of = |owner: usize| {
            let mut m = [0u8; 0x30];
            m[MODULE_OWNER..MODULE_OWNER + 8].copy_from_slice(&owner.to_le_bytes());
            m
        };
        let bullet_of = |owner: u64| {
            let mut b = vec![0u8; 0xE30];
            b[BULLET_OWNER..BULLET_OWNER + 8].copy_from_slice(&owner.to_le_bytes());
            b
        };
        let dt = 1.0 / 60.0;
        // no world slow: the game's own delta, bit for bit
        publish(Table { enemy: 1.0, gimmick: 1.0, ..Table::default() });
        let mut m = module_of(enemy);
        unsafe { hitstop_hook(m.as_mut_ptr(), dt) };
        assert_eq!(got(), dt);
        // WorldSlow: the enemy's delta x0.1, the player's untouched
        publish(Table { enemy: 0.1, gimmick: 0.1, slowed_chr: vec![enemy], slowed_handle: vec![0x10], other_handle: vec![0x20], ..Table::NORMAL });
        unsafe { hitstop_hook(m.as_mut_ptr(), dt) };
        assert!((got() - dt * 0.1).abs() < 1e-9, "{}", got());
        let mut p = module_of(player);
        unsafe { hitstop_hook(p.as_mut_ptr(), dt) };
        assert_eq!(got(), dt, "Sora's hidden body keeps the game's delta");
        // bullets: the enemy's arrow in flight slowed, the player's (Sora's hit carriers) not, a trap's slowed
        let mut arrow = bullet_of(0x10);
        assert_eq!(unsafe { bullet_hook(arrow.as_mut_ptr(), dt) }, 1, "the game's answer is handed back");
        assert!((got() - dt * 0.1).abs() < 1e-9);
        let mut hit = bullet_of(0x20);
        unsafe { bullet_hook(hit.as_mut_ptr(), dt) };
        assert_eq!(got(), dt);
        let mut trap = bullet_of(u64::MAX);
        unsafe { bullet_hook(trap.as_mut_ptr(), dt) };
        assert!((got() - dt * 0.1).abs() < 1e-9);
        // the end of the slow: exactly the game's delta again
        publish(Table { enemy: 1.0, gimmick: 1.0, ..Table::default() });
        unsafe { hitstop_hook(m.as_mut_ptr(), dt) };
        assert_eq!(got(), dt);
        unsafe { bullet_hook(arrow.as_mut_ptr(), dt) };
        assert_eq!(got(), dt);
        uninstalled();
        assert!(!chr_hooked());
    }

    #[test]
    fn the_detours_read_the_game_layout_they_were_measured_on() {
        assert_eq!(BULLET_OWNER, 0xB40);
        assert_eq!(MODULE_OWNER, 8);
        assert_eq!(SCENE_RATE, 0x20C);
        // the four patterns hold the fields they read (the offline check proves them unique in both executables)
        assert!(HITSTOP_FN.pattern.ends_with("f3 0f 11 79 1c"), "movss [rcx+1Ch],xmm7: the delta it is handed");
        assert!(BULLET_FN.pattern.ends_with("f3 0f 10 81 14 0b 00 00"), "movss xmm0,[rcx+0B14h]: the bullet's life");
        assert!(SFX_STEP.pattern.contains("f3 0f 59 bf 0c 02 00 00"), "mulss xmm7,[rdi+20Ch]");
        assert!(CLOCK_STEP.pattern.ends_with("f3 0f 59 43 38"), "mulss xmm0,[rbx+38h]");
    }
}
