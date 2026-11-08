//! (THE USER 2026-11-02: "Give Sora the ability to 'Summon' from the command menu, but he uses Elden Ring's spirit ashes
//! instead of his KH3 summons ... In the equipment menu for Sora, make a new area for him to equip spirit ashes (4) and these
//! equipped spirit ashes are what appears in the Summon command menu. Summon is just greyed out if nothing is equipped.
//! Summoning is also only available when you get the spirit calling bell.") THE SUMMON COMMAND, THE ELDEN RING SIDE.
//! UNTESTED IN THE GAME. The rules (cost, timer, refund, greying, the chant) are kh-core's `command::summon`; this file is the
//! host: what Sora owns, the spirit itself, its timer, HP and leaving.
//!
//! THE VIEW (every frame while Sora runs, `CommandState::summon`): `allowed` = the player's Sora (no friend leads), `summon.er`
//! on (default) and the game's functions found; `bell` = the Spirit Calling Bell in the inventory (goods 8158 by public data,
//! A GUESS verified at run time by its name "Spirit Calling Bell", else every key item's name is searched); `owned` = every
//! spirit ash row in the inventory (EquipParamGoods goodsType 7 / 8: level `id % 100`, its game name with " +N", its MP
//! converted from its FP / HP cost, renowned = type 8, the Mimic Tear = base 207000 (by id; its name logged), `usable` = no BuddyParam row
//! of it is a monument-only spirit (`appearOnAroundSekihi`), offline, no arena, and for the Mimic Tear Mimic Sora is there,
//! spirit_mimic.rs); `slots` = the equipped base ids resolved to the highest level owned; `out` = the live spirit.
//!
//! THE SPAWN: ELDEN RING'S OWN BUDDY SYSTEM, the route of build/sora_tmp/party/SUMMON/IMPL_SUMMON_RE.md ("RECOMMENDED ROUTE"):
//! every abort check first ([`decide`]: the manager, the player alive, no request / dismiss / generator hold / item cooldown
//! pending, no spirit left over, no arena, offline, EVERY BuddyStoneParam row the game's lookup can give for stone entity 0
//! has no `summonedEventFlagId` and no eliminate target (else STOP: the generator would write a save flag), the ash's BuddyParam
//! rows exist and none is monument-only, its level <= 10 and its use effect of that level); then the game's own
//! FindSpawnPointAroundPlayer (around the hidden Tarnished body, which stands where Sora stands while he leads; else his place
//! is written); then `+0x3c = 0`, `+0xe0 = +0xe4 = goods`, LAST `+0x20 = refId_default`. The game's next update makes the
//! spirit (BuddyStoneParam row 0: flagless). The ONE DETOUR that keeps it: the activation-range step (its stone rule dismisses
//! a spirit away from any stone object) is let run, then while OUR request is the active one `+0xb5 = 1`, `+0xb7 = 0` (the
//! same two bytes are written with the request: the game's dismiss task may run before the next range step). The
//! spirit's ChrIns is watched for in the manager's groups (and the summon chr set) for 3 s; no spirit = given up, the MP back.
//!
//! (THE USER 2026-11-02: "Give spirit ashes a timer, upgrading the spirit ash increases the timer ... They can either die or
//! run out of time.") THE TIMER (`summon::duration` of the ash's tier and level) runs on the world's clock from the spawn.
//! LEAVING ([`leave_of`]): Unsummon (THE USER: "pressing it just immediately ends the summon and gives back a refund of MP":
//! the core gave the refund), its time out, the KH save point's menu (THE USER: "going into the save point menu instantly
//! ends the summon", savepoint.rs), Elden Ring's own rest at a grace: KH3's vanish (Anti Form's warp-out `vs_ant_wrp_out03`
//! with the sound it carries, se00001_213, the pack's effect of Sora's aura set) at the spirit, then the game's own dismiss
//! (`+0x28 = 1`: its fade animation). Its death, or the game ending it by its own rules (a map unload, the owner gone, its
//! own dismiss): no effect of ours. The mode's end: the game's remove-now flag on every group. After ANY leave `out` is None
//! (the row reads "Summon"); the log says why (`LeaveReason`). Only Unsummon refunds (the core's).
//!
//! (THE USER 2026-11-02: "For Spirit Ashes, give them their own KH-styled HUD element instead of using Elden Ring's HUD for
//! them.") ELDEN RING'S SPIRIT BLOCK IS HIDDEN: its HUD step for the spirits (0x771ed0 / 0x771a20) is detoured: after the
//! original, while erhud.rs hides any of Elden Ring's HUD (the same lifetime), the block's enable byte (fe+0x4D58), its count
//! (FrontEndViewValues `summoned_spirit_ash_count`) and its five HP-bar handles are cleared (the function itself writes those
//! values when no spirit is out). The KH HUD element (another stream) reads `out`.
//!
//! (THE USER 2026-11-02: "When summoned and out of combat, the spirit ashes should follow Sora like his party allies.")
//! FOLLOW: ELDEN RING'S OWN. A spirit follows the player body by its own AI out of combat (BuddyParam `pcFollowType`), and the
//! game's buddy warp manager teleports it to the player when it is 33 m away, its path stuck 3 s or its ray blocked 5 s
//! (GameSystemCommonParam `buddyWarp_*`, regulation 2.7.2.0; read at run time from `SummonBuddyWarpManager`). The player
//! body is the KH leader's place every frame (sora.rs `write`), so it follows the KH leader. KH3's rule
//! (`summon::follow_plan`, 30 m warp) is the same within 3 m: this file only judges it each frame and says in the log when the
//! game's warp has not taken a spirit KH3 would have warped. No position is ever written.
//!
//! PARTY / TARGETS: a character of the summon chr set is a bystander to Sora's targeting (targets.rs `classify`: never
//! lockable, never hit), so neither Sora nor the party AI (whose targets are the hostile ones of that list) ever aims at it;
//! the aggro director deals only engaged hostiles; Elden Ring's own teams keep it friendly to the party's bodies (spirit team).
//!
//! KEYS: `summon.er` (1; 0 = no Summon row). Logs: `spirit:` lines.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;

use eldenring::cs::{BuddyParam, BuddyStoneParam, CSFeManImp, CSHavokMan, CSSessionManager, ChrIns, EquipParamGoods, FrontEndViewValues, LobbyState, PlayerGameData, PlayerIns, SoloParamRepository, SummonBuddyGroup, SummonBuddyManager, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::Vec3;
use kh_core::command::summon::{self, AshSlot, FollowPlan, LeaveReason, SpiritOut, SummonPhase, SummonView, Timer, SLOTS};
use kh_core::host::EventKind;

use crate::control::Control;
use crate::coords::Frame;
use crate::erlog;
use crate::game::{self, plausible};
use crate::havok::{locate, Located};
use crate::lootmap::Cat;
use crate::word;

/// The goods types of a spirit ash (`EquipParamGoods.goodsType`; 8 = renowned, upgraded with Ghost Gloveworts).
pub const ASH_TYPES: [u8; 2] = [7, 8];
const RENOWNED: u8 = 8;
/// The Mimic Tear's level-0 row (regulation 2.7.2.0: the one ash that costs HP) and its name (checked at run time).
pub const MIMIC_BASE: u32 = 207_000;
const MIMIC_NAME: &str = "Mimic Tear";
/// The Spirit Calling Bell: goods 8158 by public data (A GUESS, verified by its name at run time; regulation 2.7.2.0 row
/// 8158 is a key item, goodsType 1).
const BELL_GUESS: u32 = 8158;
const BELL_NAME: &str = "Spirit Calling Bell";
const KEY_ITEM: u8 = 1;
/// The goods table's bits of an item id (`0x40000000 | goods id`).
const GOODS_BITS: u32 = 0x4000_0000;
/// `EquipParamGoods.refCategory` of a goods item whose use applies a SpEffect (every spirit ash: the summon request).
const REF_SPEFFECT: u8 = 2;
/// The game's buddy levels end at +10 (`dopingSpEffect_lv10`; the generator gives no doping above).
const MAX_LEVEL: u32 = 10;
/// Seconds a spawn may wait for the game (a cooldown, the generator's hold), then for its spirit.
const SPAWN_WAIT: f32 = 3.0;
/// Seconds after our leave before the game's groups are removed outright (its fade is 10 s: `buddyDisappearDelaySec`).
const LINGER_MAX: f32 = 20.0;
/// KH3's vanish at a leaving spirit (the pack's effect, the sound it carries: kh-audio's "effect:<name>" route).
const DISMISS_EFFECT: &str = "vs_ant_wrp_out03";
const DISMISS_SOUND: &str = "effect:vs_ant_wrp_out03";
/// The spirit's chest above its feet, cm (where the vanish is drawn).
const CHEST_CM: f32 = 100.0;
/// A spawn point the game's finder gives farther than this from Sora (m) is not taken: his place is.
const SPAWN_REACH_M: f32 = 8.0;

/// THE OFFSETS the route writes by hand (the RE report's, both executables), each asserted against the SDK's struct where the
/// SDK names the field (`const _` below; `SummonBuddyManager` is 0x110 bytes in the SDK as in the game's allocation).
pub(crate) mod off {
    /// WorldChrMan: the main player, the summon buddy manager (the same in 2.7.1.0 and 2.7.2.0: gamever.rs `wcm`).
    pub const WCM_MAIN_PLAYER: usize = 0x1_E508;
    pub const WCM_MANAGER: usize = 0x1_E538;
    /// SummonBuddyManager: request SpEffect, dismiss request, active stone entity, groups, generator hold counter (no SDK name:
    /// right after the 0x18-byte groups map), dismiss delay, item cooldown, spawn origin / yaw, in activation range, in warn
    /// range, requested goods, active goods.
    pub const REQUEST: usize = 0x20;
    pub const DISAPPEAR: usize = 0x28;
    pub const ACTIVE_STONE: usize = 0x3c;
    pub const GROUPS: usize = 0x70;
    pub const HOLD: usize = 0x88;
    pub const DELAY: usize = 0x8c;
    pub const COOLDOWN: usize = 0x90;
    pub const ORIGIN: usize = 0xa0;
    pub const YAW: usize = 0xb0;
    pub const IN_RANGE: usize = 0xb5;
    pub const IN_WARN: usize = 0xb7;
    pub const REQ_GOODS: usize = 0xe0;
    pub const ACTIVE_GOODS: usize = 0xe4;
    pub const MANAGER_SIZE: usize = 0x110;
    /// SummonBuddyGroup (the list node's value at +0x10): REMOVE NOW at +0x20 (the SDK calls it `warp_requested`; the game's
    /// RemoveAllNow 0x4b83b0 sets node+0x30 = this), dismiss requested at +0x21.
    pub const GROUP_REMOVE_NOW: usize = 0x20;
    pub const GROUP_DISAPPEAR: usize = 0x21;
    pub const GROUP_SIZE: usize = 0x30;
    /// SoloParamRepository: the sorted (talkChrEntityId, BuddyStoneParam index) vector the generator's lookup reads (the
    /// DLVector: allocator, then begin at +0x3730 and end at +0x3738, the words the game's lower-bound lookup loads,
    /// [`super::STONE_LOOKUP_FN`]).
    pub const STONE_IDS: usize = 0x3728;
    /// CSFeManImp: FrontEndViewValues, the spirit block's enable byte, its count, its five HP-bar entries.
    pub const FE_VALUES: usize = 0x80;
    pub const FE_ENABLE: usize = 0x4D58;
    pub const FE_COUNT: usize = 0x4DD8;
    pub const FE_DISPLAYS: usize = 0x4DE0;
    pub const FE_DISPLAY_SIZE: usize = 0x18;
    pub const FE_DISPLAY_COUNT: usize = 5;
}

const _: () = {
    use std::mem::{offset_of, size_of};
    assert!(size_of::<SummonBuddyManager>() == off::MANAGER_SIZE);
    assert!(size_of::<SummonBuddyGroup>() == off::GROUP_SIZE);
    assert!(offset_of!(WorldChrMan, main_player) == off::WCM_MAIN_PLAYER);
    assert!(offset_of!(WorldChrMan, summon_buddy_manager) == off::WCM_MANAGER);
    assert!(offset_of!(SummonBuddyManager, request_summon_speffect_id) == off::REQUEST);
    assert!(offset_of!(SummonBuddyManager, disappear_requested) == off::DISAPPEAR);
    assert!(offset_of!(SummonBuddyManager, active_summmon_buddy_stone_entity_id) == off::ACTIVE_STONE);
    assert!(offset_of!(SummonBuddyManager, groups) == off::GROUPS);
    assert!(offset_of!(SummonBuddyManager, buddy_disappear_delay_sec) == off::DELAY && off::HOLD + 4 == off::DELAY);
    assert!(off::GROUPS + size_of::<eldenring::DLMap<i32, eldenring::DLList<SummonBuddyGroup>>>() == off::HOLD);
    assert!(offset_of!(SummonBuddyManager, item_use_cooldown_timer) == off::COOLDOWN);
    assert!(offset_of!(SummonBuddyManager, spawn_origin) == off::ORIGIN);
    assert!(offset_of!(SummonBuddyManager, spawn_rotation) == off::YAW);
    assert!(offset_of!(SummonBuddyManager, is_within_activation_range) == off::IN_RANGE);
    assert!(offset_of!(SummonBuddyManager, is_within_warn_range) == off::IN_WARN);
    assert!(offset_of!(SummonBuddyManager, requested_summon_goods_id) == off::REQ_GOODS);
    assert!(offset_of!(SummonBuddyManager, active_summon_goods_id) == off::ACTIVE_GOODS);
    assert!(offset_of!(SummonBuddyGroup, warp_requested) == off::GROUP_REMOVE_NOW);
    assert!(offset_of!(SummonBuddyGroup, disappear_requested) == off::GROUP_DISAPPEAR);
    assert!(offset_of!(SoloParamRepository, buddy_stone_entity_ids) == off::STONE_IDS);
    assert!(offset_of!(CSFeManImp, frontend_values) == off::FE_VALUES);
    assert!(off::FE_VALUES + offset_of!(FrontEndViewValues, summoned_spirit_ash_count) == off::FE_COUNT);
    assert!(off::FE_VALUES + offset_of!(FrontEndViewValues, spirit_ash_display) == off::FE_DISPLAYS);
    assert!(size_of::<eldenring::cs::CSFeSpiritAshDisplay>() == off::FE_DISPLAY_SIZE);
    // the enable byte: 0x80 before the count (the RE report: FEVV +0x4CD8, the count FEVV +0x4D58)
    assert!(off::FE_ENABLE + 0x80 == off::FE_COUNT);
};

// ============================================================================================ the game's functions

/// UpdateActivationRange `void(SummonBuddyManager*, f32)`: resets +0xb5 / +0xb7, then measures the active stone's range.
pub(crate) const RANGE_FN: Located = Located {
    name: "summon buddy activation range",
    pattern: "40 55 56 41 55 48 8d 6c 24 b9 48 81 ec a0 00 00 00 0f 29 7c 24 70 48 8b 05 ? ? ? ? 48 33 c4 48 89 45 07 0f b6 81 b5 00 00 00 48 8b f1 88 81 b6 00 00 00 0f b6 81 b7 00 00 00 88 81 b8 00 00 00",
    capture: 0,
    rva_2_7_1_0: 0x004b_d870,
    rva_2_7_2_0: 0x004b_d950,
};
/// FindSpawnPointAroundPlayer `bool(SummonBuddyManager*)`: writes +0xa0 / +0xb0 from the main player's place.
pub(crate) const SPAWN_POINT_FN: Located = Located {
    name: "summon buddy spawn point around the player",
    pattern: "48 8b c4 55 56 41 56 41 57 48 8d a8 a8 fe ff ff 48 81 ec 38 02 00 00 0f 29 70 c8 0f 29 78 b8 44 0f 29 40 a8 44 0f 29 48 98 44 0f 29 98 78 ff ff ff 48 8b 05 ? ? ? ? 48 33 c4",
    capture: 0,
    rva_2_7_1_0: 0x004b_9e70,
    rva_2_7_2_0: 0x004b_9f50,
};
/// IsArena `bool()` (the colosseum rule of the dismiss task; [G] in the report: a colosseum).
pub(crate) const ARENA_FN: Located = Located {
    name: "summon buddy arena check",
    pattern: "40 57 48 83 ec 30 48 8b 05 ? ? ? ? 40 32 ff c7 44 24 20 00 00 00 00 48 85 c0 0f 84 ? ? ? ? 48 83 3d ? ? ? ? 00 0f 84 ? ? ? ? 48 8b 88 08 e5 01 00 48 85 c9 0f 84 ? ? ? ? 48 89 5c 24 48 48 8d 54 24 40",
    capture: 0,
    rva_2_7_1_0: 0x004b_7710,
    rva_2_7_2_0: 0x004b_77f0,
};
/// The HUD's spirit block step `void(CSFeManImp*, f32)` (RE listing fe1.txt: HasAliveSummon, the player's state 457, then
/// fe+0x4D58 = 1 / 0, the active goods' icon, fe+0x4DD8 = the alive count, the HP bars at fe+0x4DE0, unused ones -1).
pub(crate) const HUD_FN: Located = Located {
    name: "HUD spirit ash block",
    pattern: "48 8b c4 55 41 54 41 55 41 56 41 57 48 8d a8 08 ff ff ff 48 81 ec d0 01 00 00 48 c7 44 24 48 fe ff ff ff 48 89 58 10 48 89 70 18 48 89 78 20 48 8b 05 ? ? ? ? 48 33 c4",
    capture: 0,
    rva_2_7_1_0: 0x0077_1a20,
    rva_2_7_2_0: 0x0077_1ed0,
};

/// THE STONE LOOKUP (never called or hooked: a proof of the layout): the generator's BuddyStoneParam-by-entity lookup
/// (0xd29d20 -> this) loads the sorted vector's begin / end from SoloParamRepository +0x3730 / +0x3738 (`mov r10,[rcx+3738h];
/// mov r8d,edx; mov r9,[rcx+3730h]`), a lower bound: the FIRST row of the key. Found by its pattern = the SDK's
/// `buddy_stone_entity_ids` (at 0x3728, asserted) is where this executable keeps it; not found = no Summon.
pub(crate) const STONE_LOOKUP_FN: Located = Located {
    name: "BuddyStoneParam lookup by stone entity (layout proof)",
    pattern: "4c 8b 91 38 37 00 00 44 8b c2 4c 8b 89 30 37 00 00 49 8b ca 49 2b c9 48 c1 f9 03",
    capture: 0,
    rva_2_7_1_0: 0x00d4_ebb0,
    rva_2_7_2_0: 0x00d5_02a0,
};

/// Every address of this file (gamever.rs's offline check runs them against both executables).
#[cfg(test)]
pub(crate) const LOCATED: [Located; 5] = [RANGE_FN, SPAWN_POINT_FN, ARENA_FN, HUD_FN, STONE_LOOKUP_FN];

type RangeFn = unsafe extern "C" fn(*mut u8, f32);
type HudFn = unsafe extern "C" fn(*mut u8, f32);
type SpawnPointFn = unsafe extern "C" fn(*mut SummonBuddyManager) -> u8;
type ArenaFn = unsafe extern "C" fn() -> u8;

static RANGE_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static HUD_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static SPAWN_POINT: AtomicUsize = AtomicUsize::new(0);
static ARENA: AtomicUsize = AtomicUsize::new(0);
/// Our request is (or was) the active summon: the range detour keeps it in range while `+0x3c == 0` and `+0xe4` is ours.
static OUR_ACTIVE: AtomicBool = AtomicBool::new(false);
static OUR_GOODS: AtomicI32 = AtomicI32::new(-1);
/// The manager / front-end objects the game thread saw (the hooks touch only these).
static MGR_AT: AtomicUsize = AtomicUsize::new(0);
static FE_AT: AtomicUsize = AtomicUsize::new(0);
/// For the log: calls in which the detours acted.
static KEPT: AtomicU32 = AtomicU32::new(0);
static HIDDEN: AtomicU32 = AtomicU32::new(0);

/// (nothing in here can panic: plain loads, byte writes into the manager the game passes, which is the one we saw)
unsafe extern "C" fn range_hook(mgr: *mut u8, dt: f32) {
    let original: RangeFn = std::mem::transmute(RANGE_ORIGINAL.load(Ordering::Relaxed));
    original(mgr, dt);
    if OUR_ACTIVE.load(Ordering::Relaxed) && !mgr.is_null() && mgr as usize == MGR_AT.load(Ordering::Relaxed) {
        let stone = std::ptr::read_unaligned(mgr.add(off::ACTIVE_STONE) as *const u32);
        let goods = std::ptr::read_unaligned(mgr.add(off::ACTIVE_GOODS) as *const i32);
        if stone == 0 && goods == OUR_GOODS.load(Ordering::Relaxed) {
            *mgr.add(off::IN_RANGE) = 1;
            *mgr.add(off::IN_WARN) = 0;
            KEPT.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// (nothing in here can panic: plain byte / word writes of the values the function itself writes when no spirit is out)
unsafe extern "C" fn hud_hook(fe: *mut u8, dt: f32) {
    let original: HudFn = std::mem::transmute(HUD_ORIGINAL.load(Ordering::Relaxed));
    original(fe, dt);
    if !fe.is_null() && fe as usize == FE_AT.load(Ordering::Relaxed) && crate::erhud::hiding() {
        *fe.add(off::FE_ENABLE) = 0;
        std::ptr::write_unaligned(fe.add(off::FE_COUNT) as *mut u32, 0);
        for k in 0..off::FE_DISPLAY_COUNT {
            std::ptr::write_unaligned(fe.add(off::FE_DISPLAYS + k * off::FE_DISPLAY_SIZE) as *mut u64, u64::MAX);
        }
        HIDDEN.fetch_add(1, Ordering::Relaxed);
    }
}

/// Finds the functions and detours the two steps (once per payload). True: summoning can work (the range step detoured, the
/// spawn point and arena functions found); the HUD hook may fail on its own (the game's block then shows).
fn install() -> bool {
    let range = locate(&RANGE_FN);
    let spawn = locate(&SPAWN_POINT_FN);
    let arena = locate(&ARENA_FN);
    let hud = locate(&HUD_FN);
    let layout = locate(&STONE_LOOKUP_FN).is_some();
    if let (Some(s), Some(a)) = (spawn, arena) {
        SPAWN_POINT.store(s, Ordering::SeqCst);
        ARENA.store(a, Ordering::SeqCst);
    }
    let kept = match range {
        Some(f) if RANGE_ORIGINAL.load(Ordering::SeqCst) == 0 => {
            // SAFETY: the function's start, found by its unique pattern at the recorded address; the hook has its signature.
            match unsafe { kh_er_render::inline_hook(f, range_hook as *const () as usize, &RANGE_ORIGINAL, "summon buddy range") } {
                Ok(()) => true,
                Err(e) => {
                    RANGE_ORIGINAL.store(0, Ordering::SeqCst);
                    erlog!("spirit: the game's activation-range step was NOT detoured ({e}): no Summon");
                    false
                }
            }
        }
        Some(_) => true,
        None => false,
    };
    let hidden = match hud {
        Some(f) if HUD_ORIGINAL.load(Ordering::SeqCst) == 0 => {
            // SAFETY: as above.
            match unsafe { kh_er_render::inline_hook(f, hud_hook as *const () as usize, &HUD_ORIGINAL, "HUD spirit block") } {
                Ok(()) => true,
                Err(e) => {
                    HUD_ORIGINAL.store(0, Ordering::SeqCst);
                    erlog!("spirit: the HUD's spirit block step was NOT detoured ({e}): Elden Ring's spirit block shows");
                    false
                }
            }
        }
        Some(_) => true,
        None => false,
    };
    let ready = kept && spawn.is_some() && arena.is_some() && layout;
    erlog!(
        "spirit: activation-range step {}, spawn point {}, arena check {}, the stone lookup's layout {}, HUD spirit block {}: {}",
        if kept { "detoured" } else { "NOT FOUND" },
        if spawn.is_some() { "found" } else { "NOT FOUND" },
        if arena.is_some() { "found" } else { "NOT FOUND" },
        if layout { "proven (+0x3730)" } else { "NOT PROVEN" },
        if hidden { "detoured (hidden under Sora's HUD)" } else { "not detoured (Elden Ring's block shows)" },
        if ready { "Summon is available" } else { "NO SUMMON ROW (the game's functions are not all there)" }
    );
    ready
}

/// The hooks are gone (the renderer's shutdown removed every inline hook).
pub fn uninstalled() {
    RANGE_ORIGINAL.store(0, Ordering::SeqCst);
    HUD_ORIGINAL.store(0, Ordering::SeqCst);
    OUR_ACTIVE.store(false, Ordering::SeqCst);
    if let Ok(mut st) = STATE.lock() {
        *st = None;
    }
}

// ============================================================================================ the pure rules (unit-tested)

/// What the game is given for an ash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    /// +0xe0 / +0xe4.
    pub goods: i32,
    /// +0x20: the row's `refId_default` (its level's SpEffect: the generator takes `level = id % 100`, `base = id - level`).
    pub speffect: i32,
    /// The base SpEffect, BuddyParam's `triggerSpEffectId`.
    pub trigger: i32,
    pub level: u8,
}

/// THE REQUEST ARITHMETIC: from the goods row (its id, `refId_default`, `refCategory`).
pub fn request_of(goods: u32, ref_id: i32, ref_category: u8) -> Result<Request, String> {
    let level = goods % 100;
    if level > MAX_LEVEL {
        return Err(format!("goods {goods}: level {level} is above +{MAX_LEVEL} (the game's doping rows end there)"));
    }
    if ref_category != REF_SPEFFECT {
        return Err(format!("goods {goods}: its use is not a SpEffect (refCategory {ref_category})"));
    }
    if ref_id <= 0 {
        return Err(format!("goods {goods}: no use effect (refId_default {ref_id})"));
    }
    if ref_id % 100 != level as i32 {
        return Err(format!("goods {goods}: its use effect {ref_id} is not of level {level} (the game would take level {})", ref_id % 100));
    }
    let goods_i = i32::try_from(goods).map_err(|_| format!("goods {goods}: not a goods id"))?;
    Ok(Request { goods: goods_i, speffect: ref_id, trigger: ref_id - ref_id % 100, level: level as u8 })
}

/// THE STONE CHECK: every BuddyStoneParam row the game's lookup can give for stone entity 0, as (summonedEventFlagId,
/// eliminateTargetEntityId). `None` = safe; `Some(why)` = STOP (never a save flag).
pub fn stone_problem(rows: &[(u32, u32)]) -> Option<String> {
    if rows.is_empty() {
        return Some("no BuddyStoneParam row has talkChrEntityId 0 (the generator would spawn nothing)".into());
    }
    rows.iter().find(|(flag, target)| *flag != 0 || *target != 0).map(|(flag, target)| format!("a BuddyStoneParam row of stone entity 0 has summonedEventFlagId {flag} / eliminate target {target}: the generator would write it (STOP: no save flag is ever set)"))
}

/// THE TRIGGER CHECK: the ash's BuddyParam rows (id, `appearOnAroundSekihi`). `None` = it can be called anywhere.
pub fn trigger_problem(trigger: i32, rows: &[(u32, u8)]) -> Option<String> {
    if rows.is_empty() {
        return Some(format!("no BuddyParam row has triggerSpEffectId {trigger}"));
    }
    rows.iter().find(|(_, sekihi)| *sekihi != 0).map(|(id, _)| format!("BuddyParam {id} of trigger {trigger} appears only around a Rebirth Monument"))
}

/// The facts of the abort checks, read from the game.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    pub player_alive: bool,
    /// +0x20 < 0: no request of the game's or ours is pending.
    pub request_free: bool,
    /// +0x28.
    pub dismissing: bool,
    /// +0x88 (the generator waits while > 0).
    pub hold: i32,
    /// +0x90.
    pub cooldown: f32,
    /// Entries in the manager's group lists (a spirit still there).
    pub groups: usize,
    /// None: the function is not found.
    pub arena: Option<bool>,
    pub offline: Option<bool>,
    pub stone: Option<String>,
    pub trigger: Option<String>,
    pub request: Option<String>,
}

/// What [`decide`] says.
#[derive(Clone, Debug, PartialEq)]
pub enum Decision {
    Go,
    /// Not now (a frame or two of the game's own): asked again until [`SPAWN_WAIT`] s.
    Wait(&'static str),
    /// Never for this request (the MP is given back).
    Refuse(String),
}

/// THE ABORT CHECKS of the RECOMMENDED ROUTE: the lasting ones refuse, the passing ones wait.
pub fn decide(f: &Facts) -> Decision {
    if let Some(problem) = [&f.request, &f.trigger, &f.stone].into_iter().flatten().next() {
        return Decision::Refuse(problem.clone());
    }
    match f.arena {
        Some(false) => {}
        Some(true) => return Decision::Refuse("an arena (the game dismisses spirits there)".into()),
        None => return Decision::Refuse("the game's arena check is not found".into()),
    }
    match f.offline {
        Some(true) => {}
        Some(false) => return Decision::Refuse("online (spirits are an offline thing here)".into()),
        None => return Decision::Refuse("the session state is not readable".into()),
    }
    if !f.player_alive {
        return Decision::Refuse("the player is not alive".into());
    }
    if !f.request_free {
        Decision::Wait("a summon request is pending in the game")
    } else if f.dismissing {
        Decision::Wait("the game is dismissing a spirit")
    } else if f.hold > 0 {
        Decision::Wait("the game's buddy generator holds")
    } else if f.cooldown > 0.0 {
        Decision::Wait("the game's item cooldown runs")
    } else if f.groups > 0 {
        Decision::Wait("an earlier spirit is still there")
    } else {
        Decision::Go
    }
}

/// The equipped slots resolved: the owned ash of each slot's base, the highest level when several are owned.
pub fn pick_slots(equipped: &[u32; SLOTS], owned: &[AshSlot]) -> [Option<AshSlot>; SLOTS] {
    std::array::from_fn(|i| {
        let base = equipped[i];
        (base != 0).then(|| owned.iter().filter(|a| summon::base_of(a.goods) == base).max_by_key(|a| (a.level, a.goods)).cloned()).flatten()
    })
}

/// The ash's name for the menu: the game's own (without a level it may carry), " +N" when N > 0.
pub fn ash_name(game_name: Option<&str>, goods: u32, level: u8) -> String {
    let base = game_name.map(|n| crate::spirittune::plain_name(n).to_string()).unwrap_or_else(|| format!("Spirit ash {}", summon::base_of(goods)));
    if level > 0 {
        format!("{base} +{level}")
    } else {
        base
    }
}

/// What the live spirit is this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LiveFacts {
    /// Our own reason (Unsummon, the save point, a rest).
    pub asked: Option<LeaveReason>,
    /// Its group is listed with a complete character.
    pub present: bool,
    /// The latest HP read (kept from the last frame it was present).
    pub hp: i32,
    /// The game set its dismiss flag without us.
    pub game_dismissing: bool,
    pub timed_out: bool,
}

/// THE LEAVE RULE: ours first, then its death, then the game's own end, then the timer.
pub fn leave_of(f: &LiveFacts) -> Option<LeaveReason> {
    if f.asked.is_some() {
        f.asked
    } else if f.hp <= 0 {
        Some(LeaveReason::Died)
    } else if !f.present || f.game_dismissing {
        Some(LeaveReason::Gone)
    } else if f.timed_out {
        Some(LeaveReason::Timeout)
    } else {
        None
    }
}

/// (NOCOMBAT) A spirit sent away by our own rules: Elden Ring's rest at a grace, or a place where the game forbids attacks
/// (`stays` false: kh_core::nocombat::may_stay). Both are the save point's leave (KH3's vanish, the game's fade).
pub fn sent_away(resting: bool, stays: bool) -> Option<LeaveReason> {
    (resting || !stays).then_some(LeaveReason::SavePoint)
}

/// A leave KH3's vanish is drawn for (ours: the game's own ends show its own).
pub fn vanishes(reason: LeaveReason) -> bool {
    matches!(reason, LeaveReason::Unsummon | LeaveReason::Timeout | LeaveReason::SavePoint)
}

/// Progress toward its place behind the leader: the seconds without getting nearer (for `follow_plan`'s stuck rule).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Progress {
    /// The nearest it has been since its last progress (None: no reading yet).
    pub best: Option<f32>,
    pub stuck: f32,
}

impl Progress {
    /// Metres it must gain to count as progress.
    pub const GAIN: f32 = 0.25;

    pub fn step(&mut self, dist: f32, dt: f32) -> f32 {
        match self.best {
            Some(best) if dist >= best - Self::GAIN => {
                self.stuck += dt.max(0.0);
                self.best = Some(best.min(dist));
            }
            _ => {
                self.best = Some(dist);
                self.stuck = 0.0;
            }
        }
        self.stuck
    }

    pub fn reset(&mut self) {
        *self = Progress::default();
    }
}

/// THE FOLLOW JUDGEMENT (nothing is written: the game's own follow and warp act): `plan` = KH3's rule this frame; `warping`
/// = the game's warp entry for it is past None; `warp_s` = seconds KH3's rule has said Warp. True: worth a log line (KH3 would
/// have warped it 5 s ago and the game's own warp has not).
pub fn follow_lags(plan: FollowPlan, warping: bool, warp_s: f32) -> bool {
    plan == FollowPlan::Warp && !warping && warp_s >= 5.0
}

/// The Elden Ring yaw (radians about +Y) of an upright character's orientation quaternion, from its y and w parts (x and z
/// are 0 for a rotation about +Y alone): the angle `quat_er_from_yaw` builds.
pub fn yaw_of_quat(y: f32, w: f32) -> f32 {
    2.0 * y.atan2(w)
}

// ============================================================================================ the game's data

fn repo() -> Option<&'static SoloParamRepository> {
    // SAFETY: the singleton lives as long as the game; its holder table is checked by the SDK.
    unsafe { SoloParamRepository::instance() }.ok()
}

/// The summon buddy manager (game thread; the pointer word checked).
fn manager() -> Option<&'static mut SummonBuddyManager> {
    // SAFETY: game thread (a task phase); the singleton lives as long as the game.
    let wcm = unsafe { WorldChrMan::instance_mut() }.ok()?;
    let w = word!(wcm.summon_buddy_manager);
    if !plausible(w) {
        return None;
    }
    // SAFETY: a plausible pointer of WorldChrMan's own field (the SDK's type, 0x110 bytes asserted).
    Some(unsafe { &mut *(w as *mut SummonBuddyManager) })
}

/// The generator's hold counter (+0x88, no SDK field name).
fn hold_of(mgr: &SummonBuddyManager) -> i32 {
    // SAFETY: inside the manager (0x110 bytes); a plain i32 read.
    unsafe { std::ptr::read_unaligned((mgr as *const SummonBuddyManager as *const u8).add(off::HOLD) as *const i32) }
}

/// One group entry of the manager.
#[derive(Clone, Copy, Debug)]
struct GroupFacts {
    chr: usize,
    dismissing: bool,
}

/// Every group entry (all owners: offline only the player's).
fn groups_of(mgr: &SummonBuddyManager) -> Vec<GroupFacts> {
    let mut out = Vec::new();
    for entry in mgr.groups.iter() {
        for g in entry.second.iter() {
            out.push(GroupFacts { chr: g.chr_ins.as_ptr() as usize, dismissing: g.disappear_requested });
            if out.len() >= 16 {
                return out;
            }
        }
    }
    out
}

/// The game's remove-now flag on every group (RemoveAllNow's own write).
fn remove_all_now(mgr: &mut SummonBuddyManager) -> usize {
    let mut n = 0;
    for entry in mgr.groups.iter_mut() {
        for g in entry.second.iter_mut() {
            g.warp_requested = true; // (= REMOVE NOW, see `off::GROUP_REMOVE_NOW`)
            n += 1;
        }
    }
    n
}

/// The characters of the summon chr set.
fn summon_set() -> Vec<usize> {
    let mut out = Vec::new();
    // SAFETY: game thread; every pointer of the set is checked before it is followed.
    unsafe {
        if let Ok(wcm) = WorldChrMan::instance() {
            crate::targets::each_chr_of(&wcm.summon_buddy_chr_set, &mut |p| out.push(p));
        }
    }
    out
}

/// A spirit's character this frame: (hp, max hp, feet in Elden Ring space, handle bits).
fn chr_facts(p: usize) -> Option<(i32, i32, Vec3, u64)> {
    if !plausible(p) {
        return None;
    }
    // SAFETY: a pointer of the manager's group / the summon set, checked plausible; completeness is checked next.
    let chr = unsafe { &*(p as *const ChrIns) };
    if !game::chr_complete(chr) {
        return None;
    }
    let data = &chr.modules.data;
    let hp = if chr.chr_flags1c5.death_flag() { 0 } else { data.hp };
    Some((hp, data.max_hp, game::v3(&chr.modules.physics.position), game::handle_bits(&chr.field_ins_handle)))
}

/// The game's warp entry for this character is past None (its own teleport runs).
fn game_warping(mgr: &SummonBuddyManager, handle: u64) -> bool {
    if !plausible(word!(mgr.warp_manager)) {
        return false;
    }
    mgr.warp_manager.entries.iter().any(|e| game::handle_bits(&e.first) == handle && e.second.warp_stage != eldenring::cs::SummonBuddyWarpStage::None)
}

/// The BuddyStoneParam rows of stone entity 0 the generator's lookup can give.
fn stone_rows() -> Vec<(u32, u32)> {
    let Some(repo) = repo() else { return Vec::new() };
    repo.buddy_stone_entity_ids.iter().filter(|e| e.talk_chr_entity_id == 0).filter_map(|e| repo.get_row_by_index::<BuddyStoneParam>(e.buddy_stone_param_index as usize)).map(|r| (r.summoned_event_flag_id(), r.eliminate_target_entity_id())).collect()
}

/// BuddyParam rows of each trigger SpEffect: (id, appearOnAroundSekihi).
type BuddyRows = HashMap<i32, Vec<(u32, u8)>>;

/// BuddyParam rows by trigger, read once.
fn buddy_rows(trigger: i32) -> Vec<(u32, u8)> {
    static ROWS: Mutex<Option<BuddyRows>> = Mutex::new(None);
    let mut rows = ROWS.lock().unwrap_or_else(|e| e.into_inner());
    if rows.is_none() {
        let Some(repo) = repo() else { return Vec::new() };
        let mut map = BuddyRows::new();
        for (id, b) in repo.rows::<BuddyParam>() {
            map.entry(b.trigger_sp_effect_id()).or_default().push((id, b.appear_on_around_sekihi()));
        }
        if map.is_empty() {
            return Vec::new();
        }
        erlog!("spirit: {} BuddyParam row(s) of {} spirit(s) read; {} spirit(s) appear only around a Rebirth Monument (greyed)", map.values().map(Vec::len).sum::<usize>(), map.len(), map.values().filter(|v| v.iter().any(|r| r.1 != 0)).count());
        *rows = Some(map);
    }
    rows.as_ref().and_then(|m| m.get(&trigger).cloned()).unwrap_or_default()
}

/// Every goods id in the inventory (key items and normal items).
fn held_goods(data: &PlayerGameData) -> Vec<u32> {
    let items = &data.equipment.equip_inventory_data.items_data;
    // (OUTFIT-UNLOCK) every key-item slot (crate::loot::all_key_entries), not only those inside the in-use length
    let mut out: Vec<u32> = crate::loot::all_key_entries(items).iter().chain(items.normal_entries().iter()).filter_map(|e| e.as_option()).map(|e| e.item_id.into_inner()).filter(|raw| raw >> 28 == GOODS_BITS >> 28).map(|raw| raw & 0x0FFF_FFFF).collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The Spirit Calling Bell's goods id: 8158 when its name says so (A GUESS verified), else the key item of that name; a game
/// in another language: 8158 when it is a key item (said once). `None` while the names cannot be read yet.
fn bell_goods() -> Option<u32> {
    static BELL: Mutex<Option<u32>> = Mutex::new(None);
    let mut bell = BELL.lock().unwrap_or_else(|e| e.into_inner());
    if bell.is_some() {
        return *bell;
    }
    let repo = repo()?;
    let guess_row = repo.get::<EquipParamGoods>(BELL_GUESS)?;
    let guess_name = crate::loot::item_name(Cat::Goods, BELL_GUESS)?;
    let found = if guess_name == BELL_NAME {
        erlog!("spirit: the Spirit Calling Bell is goods {BELL_GUESS} (verified by its name)");
        BELL_GUESS
    } else if let Some(id) = repo.rows::<EquipParamGoods>().filter(|(_, r)| r.goods_type() == KEY_ITEM).map(|(id, _)| id).find(|id| crate::loot::item_name(Cat::Goods, *id).as_deref() == Some(BELL_NAME)) {
        erlog!("spirit: the Spirit Calling Bell is goods {id} (found by its name; {BELL_GUESS} is \"{guess_name}\")");
        id
    } else {
        erlog!("spirit: no key item is named \"{BELL_NAME}\" (another language?): goods {BELL_GUESS} (\"{guess_name}\", goodsType {}) is taken as the bell", guess_row.goods_type());
        BELL_GUESS
    };
    *bell = Some(found);
    Some(found)
}

/// The arena check (the game's own function).
fn in_arena() -> Option<bool> {
    let f = ARENA.load(Ordering::Relaxed);
    if f == 0 {
        return None;
    }
    // SAFETY: the game's leaf query at its unique pattern, `bool()`, on the game thread.
    let arena: ArenaFn = unsafe { std::mem::transmute(f) };
    Some(unsafe { arena() } != 0)
}

/// Offline: the session manager's lobby is None.
fn offline() -> Option<bool> {
    // SAFETY: game thread; the singleton lives as long as the game.
    let s = unsafe { CSSessionManager::instance() }.ok()?;
    Some(s.lobby_state == LobbyState::None)
}

/// Every spirit ash he owns now, as the menu shows them (the inventory's order).
fn owned_ashes(data: &PlayerGameData, names: &mut HashMap<u32, String>, usable_now: bool) -> Vec<AshSlot> {
    let Some(repo) = repo() else { return Vec::new() };
    let stone_ok = stone_problem(&stone_rows()).is_none();
    let mut rows: Vec<(i32, u32, AshSlot)> = Vec::new();
    for goods in held_goods(data) {
        let Some(row) = repo.get::<EquipParamGoods>(goods) else { continue };
        if !ASH_TYPES.contains(&row.goods_type()) {
            continue;
        }
        let level = summon::level_of(goods);
        let name = names.entry(goods).or_insert_with(|| ash_name(crate::loot::item_name(Cat::Goods, goods).as_deref(), goods, level)).clone();
        let base = summon::base_of(goods);
        // (THE USER 2026-11-02: "Mimic Tear needs to mimic Sora") the id decides (never Elden Ring's copy of the Tarnished, in
        // any language); its name is only checked for the log
        let mimic = base == MIMIC_BASE;
        static MIMIC_TOLD: AtomicBool = AtomicBool::new(false);
        if mimic && !MIMIC_TOLD.swap(true, Ordering::Relaxed) {
            let name = crate::loot::item_name(Cat::Goods, base);
            erlog!("spirit: goods {MIMIC_BASE} is the Mimic Tear (its name: {:?}{}): it calls Mimic Sora ({})", name, if name.as_deref().is_some_and(|n| !n.contains(MIMIC_NAME)) { ", NOT \"Mimic Tear\": another language or another row" } else { "" }, if crate::spirit_mimic::available() { "available".to_string() } else { format!("not available now ({}): greyed", crate::spirit_mimic::why_not()) });
        }
        let request = request_of(goods, row.ref_id_default(), row.ref_category());
        let callable = request.as_ref().is_ok_and(|r| trigger_problem(r.trigger, &buddy_rows(r.trigger)).is_none());
        let usable = usable_now && stone_ok && if mimic { crate::spirit_mimic::available() } else { callable };
        let ash = AshSlot { goods, level, name, mp: summon::mp_cost(row.consume_mp() as i32, row.consume_hp() as i32), renowned: row.goods_type() == RENOWNED, mimic, usable, icon: crate::ericon::goods_icon_id(goods) };
        rows.push((row.sort_id(), goods, ash));
    }
    rows.sort_by_key(|(sort, goods, _)| (*sort, *goods));
    rows.into_iter().map(|(_, _, a)| a).collect()
}

// ============================================================================================ the state

#[derive(Clone, Debug)]
struct Pending {
    slot: u8,
    cost: i32,
    waited: f32,
    told: Option<&'static str>,
}

#[derive(Clone, Debug)]
struct Requested {
    slot: u8,
    ash: AshSlot,
    cost: i32,
    waited: f32,
}

#[derive(Clone, Debug)]
struct Live {
    slot: u8,
    ash: AshSlot,
    cost: i32,
    timer: Timer,
    hp: i32,
    hp_max: i32,
    /// This frame's character (never kept for a later frame's use but to find it again).
    chr: usize,
    handle: u64,
    feet_er: Vec3,
    plan: FollowPlan,
    progress: Progress,
    warp_s: f32,
    lag_told: bool,
}

#[derive(Default)]
struct State {
    ready: Option<bool>,
    events: Vec<(SummonPhase, u8, i32)>,
    pending: Option<Pending>,
    requested: Option<Requested>,
    live: Option<Live>,
    /// Seconds since our leave while the game's groups are still there (OUR_ACTIVE kept).
    linger: Option<f32>,
    asked: Option<LeaveReason>,
    names: HashMap<u32, String>,
    owned: Vec<AshSlot>,
    bell: bool,
    refresh: f32,
    told_warp: bool,
    hooks_told: (u32, u32),
    /// (MIMIC SORA, TEST AID) `test.mimic`
    test_mimic: Option<crate::control::OneShot>,
    /// (THE USER 2026-11-03: "have the spirit ashes HUD element fade away after they either die or time out") the spirit
    /// that just left, its final values, why, and seconds since (the view's `leaving`, kept `summon::LEAVING_HOLD` s).
    leaving: Option<(SpiritOut, LeaveReason, f32)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// The view's picture of a live spirit.
fn out_of(l: &Live) -> SpiritOut {
    SpiritOut { slot: l.slot, name: l.ash.name.clone(), cost: l.cost, left: l.timer.left, full: l.timer.full, hp: l.hp.max(0), hp_max: l.hp_max.max(1), mimic: l.ash.mimic, icon: l.ash.icon }
}

/// A Summon event of the core's step (sora.rs, every event of every step).
pub fn note_event(kind: &EventKind) {
    if let EventKind::Summon { phase, slot, mp } = kind {
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        st.get_or_insert_with(State::default).events.push((*phase, *slot, *mp));
    }
}

/// (THE USER 2026-11-02: "going into the save point menu instantly ends the summon") A leave asked by another part of the
/// mod (savepoint.rs when its menu opens): carried out at the next frame, through the normal leave path.
pub fn leave(reason: LeaveReason) {
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(st) = st.as_mut().filter(|s| s.live.is_some() || s.requested.is_some() || s.pending.is_some()) {
        st.asked.get_or_insert(reason);
    }
}

/// The mode's end (sora.rs `stop`): the spirit goes at once (the game's remove-now), nothing kept.
pub fn stop(why: &str) {
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(s) = st.as_mut() else { return };
    let had = s.live.is_some() || s.requested.is_some() || s.linger.is_some();
    if let Some(live) = s.live.take() {
        if live.ash.mimic {
            crate::spirit_mimic::dismiss(LeaveReason::Gone);
        }
    }
    if had && OUR_ACTIVE.load(Ordering::SeqCst) {
        if let Some(mgr) = manager() {
            let n = remove_all_now(mgr);
            erlog!("spirit: {why}: the spirit is removed at once (the game's remove-now on {n} group(s)); it left: {}", LeaveReason::Gone.name());
        }
    }
    OUR_ACTIVE.store(false, Ordering::SeqCst);
    (s.pending, s.requested, s.linger, s.asked, s.leaving) = (None, None, None, None, None);
}

/// What sora.rs hands over once a frame (before Sora's steps).
pub struct SpiritFrame<'a> {
    pub control: &'a Control,
    pub sora: &'a mut kh_core::Sora,
    pub player: &'a PlayerIns,
    pub frame: Frame,
    /// Sora's feet (KH3 space) and yaw (KH3 degrees): the spawn place.
    pub feet_kh: Vec3,
    pub yaw: f32,
    /// The leader's feet and yaw (KH3): the follow place.
    pub leader_kh: Vec3,
    pub leader_yaw: f32,
    /// The player's Sora (no friend leads).
    pub leads: bool,
    /// The HUD's battle rule (the leader's battle flag with its linger, or the forced one).
    pub in_battle: bool,
    /// Elden Ring's own rest at a grace (savepoint.rs `resting`).
    pub resting: bool,
    pub paused: bool,
    pub dt: f32,
}

/// ONCE A FRAME (sora.rs pre_physics, before the steps): see the file header.
pub fn frame(f: SpiritFrame) {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let st = guard.get_or_insert_with(State::default);
    let ready = *st.ready.get_or_insert_with(install);
    crate::spirit_mimic::frame(f.control); // (MIMIC SORA) `summon.mimic`, his pose side read on its thread
    let on = f.control.flag("summon.er") != Some(false);
    let wdt = if f.paused { 0.0 } else { f.dt.clamp(0.0, 0.25) };
    // SAFETY: game thread; the singletons live as long as the game (their addresses are what the hooks compare with).
    if let Ok(fe) = unsafe { CSFeManImp::instance() } {
        FE_AT.store(fe as *const CSFeManImp as usize, Ordering::Relaxed);
    }
    let mgr_word = manager().map_or(0, |m| m as *mut SummonBuddyManager as usize);
    MGR_AT.store(mgr_word, Ordering::Relaxed);

    // ---- the inventory, twice a second (and when something happens)
    st.refresh -= f.dt.max(0.0);
    let busy = !st.events.is_empty();
    if st.refresh <= 0.0 || busy {
        st.refresh = 0.5;
        if let Some(data) = crate::loot::player_data() {
            let held = held_goods(data);
            st.bell = bell_goods().is_some_and(|b| held.binary_search(&b).is_ok());
            let usable_now = in_arena() == Some(false) && offline() == Some(true);
            st.owned = owned_ashes(data, &mut st.names, usable_now);
        }
    }

    // ---- the core's events
    for (phase, slot, mp) in std::mem::take(&mut st.events) {
        match phase {
            SummonPhase::Chant => erlog!("spirit: Sora's summon chant begins for slot {slot} ({mp} MP paid)"),
            SummonPhase::Spawn => {
                if st.pending.is_some() || st.requested.is_some() || st.live.is_some() {
                    erlog!("spirit: a second Spawn for slot {slot} while one is under way: refused, {mp} MP back");
                    summon::give_refund(f.sora.status_mut(), mp);
                } else {
                    st.pending = Some(Pending { slot, cost: mp, waited: 0.0, told: None });
                }
            }
            SummonPhase::Unsummon => {
                if st.live.is_some() {
                    st.asked = Some(LeaveReason::Unsummon);
                }
                erlog!("spirit: Unsummon chosen ({mp} MP given back by the core)");
            }
        }
    }
    // (NOCOMBAT) where Elden Ring forbids attacks (the Roundtable Hold) the spirit does not stay: it goes as at a save point
    let who = if st.live.as_ref().is_some_and(|l| l.ash.mimic) { kh_core::nocombat::Fighter::MimicSora } else { kh_core::nocombat::Fighter::SpiritAsh };
    if let Some(reason) = sent_away(f.resting, kh_core::nocombat::may_stay(who)).filter(|_| st.live.is_some() || st.requested.is_some()) {
        if st.asked.is_none() {
            erlog!("spirit: {} leaves: {}", who.name(), if f.resting { "Elden Ring's rest at a grace" } else { "Elden Ring forbids attacks here (nocombat), as at a save point" });
        }
        st.asked.get_or_insert(reason);
    }
    if !on && st.live.is_some() {
        st.asked.get_or_insert(LeaveReason::Gone);
    }

    // ---- (MIMIC SORA, TEST AID, the test profile only) `test.mimic = N` (a one-shot: a change of the value fires it): Mimic Sora
    // is called as if the Mimic Tear +N (0 .. 10) were summoned, without the ash, the bell or MP, under every rule of a live
    // spirit (the timer of its level, the KH gauge, Unsummon, the save point), so his AI, body and drawing can be checked on a
    // save that has no Mimic Tear. Written only into ermods_test's ini, never the user's.
    let shot = st.test_mimic.get_or_insert_with(|| crate::control::OneShot::new("test.mimic", f.control));
    if let Some(v) = shot.fired(f.control) {
        let level = v.trim().parse::<u8>().unwrap_or(0).min(10);
        let full = summon::duration(false, level);
        if st.live.is_some() || st.pending.is_some() || st.requested.is_some() {
            erlog!("spirit: test.mimic = {v}: a spirit is already out or coming: not called");
        } else if crate::spirit_mimic::spawn(f.sora, f.feet_kh, f.yaw, level, full) {
            let goods = MIMIC_BASE + level as u32;
            let ash = AshSlot { goods, level, name: format!("Mimic Tear +{level} (test.mimic)"), mp: 0, renowned: false, mimic: true, usable: true, icon: crate::ericon::goods_icon_id(goods) };
            erlog!("spirit: test.mimic = {v}: MIMIC SORA is called by the test aid (+{level}, {full:.0} s, no MP)");
            st.live = Some(Live { slot: 0, timer: Timer::new(full), hp: 1, hp_max: 1, chr: 0, handle: 0, feet_er: Vec3::ZERO, plan: FollowPlan::Stay, progress: Progress::default(), warp_s: 0.0, lag_told: false, cost: 0, ash });
        } else {
            erlog!("spirit: test.mimic = {v}: Mimic Sora could not be called ({})", crate::spirit_mimic::why_not());
        }
    }

    // ---- the spawn asked for
    if let Some(mut p) = st.pending.take() {
        p.waited += wdt;
        let (slot, cost) = (p.slot, p.cost);
        let ash = pick_slots(&f.sora.command().spirit_ashes, &st.owned)[(p.slot as usize).min(SLOTS - 1)].clone();
        let outcome = match ash {
            _ if st.asked.is_some() => Err("it was ended before it came (the save point)".to_string()),
            None => Err(format!("slot {} holds no ash he owns now", p.slot)),
            Some(a) if !a.usable => Err(format!("{} cannot be called now", a.name)),
            Some(a) if a.mimic => {
                let full = summon::duration(a.renowned, a.level);
                if crate::spirit_mimic::spawn(f.sora, f.feet_kh, f.yaw, a.level, full) {
                    erlog!("spirit: MIMIC SORA is called for the Mimic Tear (slot {}, {} MP, {full:.0} s)", p.slot, p.cost);
                    st.live = Some(Live { slot: p.slot, timer: Timer::new(full), hp: 1, hp_max: 1, chr: 0, handle: 0, feet_er: Vec3::ZERO, plan: FollowPlan::Stay, progress: Progress::default(), warp_s: 0.0, lag_told: false, cost: p.cost, ash: a });
                    Ok(None)
                } else {
                    Err("Mimic Sora could not be called".to_string())
                }
            }
            Some(a) => match begin(&a, &f) {
                Decision::Go => {
                    erlog!("spirit: {} (goods {}, +{}) is asked of Elden Ring's buddy system for slot {} ({} MP paid)", a.name, a.goods, a.level, p.slot, p.cost);
                    st.requested = Some(Requested { slot: p.slot, ash: a, cost: p.cost, waited: 0.0 });
                    Ok(None)
                }
                Decision::Wait(why) if p.waited < SPAWN_WAIT => {
                    if p.told != Some(why) {
                        erlog!("spirit: {} waits: {why}", a.name);
                        p.told = Some(why);
                    }
                    if why == "an earlier spirit is still there" && st.linger.is_some() {
                        if let Some(mgr) = manager() {
                            remove_all_now(mgr); // (our own leaving one: gone at once for the new one)
                        }
                    }
                    Ok(Some(p))
                }
                Decision::Wait(why) => Err(format!("still not possible after {SPAWN_WAIT:.0} s: {why}")),
                Decision::Refuse(why) => Err(why),
            },
        };
        match outcome {
            Ok(again) => st.pending = again,
            Err(why) => {
                let back = summon::give_refund(f.sora.status_mut(), cost);
                erlog!("spirit: the summon of slot {slot} is REFUSED: {why}; the {cost} MP are given back ({back:?})");
                st.asked = None;
            }
        }
    }

    // ---- the spirit asked of the game: watched for
    if let Some(mut r) = st.requested.take() {
        r.waited += wdt;
        let set = summon_set();
        let found = manager().map(|m| groups_of(m)).unwrap_or_default().into_iter().find(|g| set.contains(&g.chr) && !g.dismissing).and_then(|g| chr_facts(g.chr).map(|c| (g.chr, c)));
        match found {
            Some((chr, (hp, hp_max, feet_er, handle))) => {
                let full = summon::duration(r.ash.renowned, r.ash.level);
                erlog!("spirit: {} IS OUT after {:.2} s (HP {hp}/{hp_max}, {full:.0} s on its timer)", r.ash.name, r.waited);
                st.live = Some(Live { slot: r.slot, ash: r.ash, cost: r.cost, timer: Timer::new(full), hp, hp_max, chr, handle, feet_er, plan: FollowPlan::Stay, progress: Progress::default(), warp_s: 0.0, lag_told: false });
            }
            None if r.waited < SPAWN_WAIT => st.requested = Some(r),
            None => {
                let back = summon::give_refund(f.sora.status_mut(), r.cost);
                erlog!("spirit: {} did NOT come within {SPAWN_WAIT:.0} s of the request: given up, the {} MP are given back ({back:?})", r.ash.name, r.cost);
                OUR_ACTIVE.store(false, Ordering::SeqCst);
                st.asked = None;
            }
        }
    }

    // ---- the live spirit
    if let Some(mut live) = st.live.take() {
        let timed_out = live.timer.tick(wdt);
        let (present, dismissing) = if live.ash.mimic {
            match crate::spirit_mimic::live() {
                Some((hp, max)) => {
                    (live.hp, live.hp_max) = (hp, max);
                    (true, false)
                }
                None => (false, false),
            }
        } else {
            let mgr = manager();
            let groups = mgr.as_deref().map(groups_of).unwrap_or_default();
            let ours = groups.iter().find(|g| g.chr == live.chr).copied();
            match ours.and_then(|g| chr_facts(g.chr).map(|c| (g, c))) {
                Some((g, (hp, max, feet, handle))) if handle == live.handle => {
                    (live.hp, live.hp_max, live.feet_er) = (hp, max, feet);
                    if !g.dismissing {
                        follow(&mut live, &f, mgr.as_deref(), wdt);
                    }
                    (true, g.dismissing)
                }
                _ => (false, false),
            }
        };
        let facts = LiveFacts { asked: st.asked.take(), present, hp: live.hp, game_dismissing: dismissing, timed_out };
        match leave_of(&facts) {
            None => st.live = Some(live),
            Some(reason) => {
                let left = live.timer.left;
                if live.ash.mimic {
                    // (MIMIC SORA, THE USER 2026-11-03: "KH3 knockdown + silver dissolve") his own leaving: the knockdown when he died,
                    // then he melts back into the Mimic Tear's silver with the game's own sounds (spirit_mimic.rs `phase`)
                    crate::spirit_mimic::dismiss(reason);
                } else {
                    if vanishes(reason) && present {
                        vanish(f.sora, f.frame, live.feet_er);
                    }
                    if present && !dismissing && reason != LeaveReason::Died {
                        if let Some(mgr) = manager() {
                            mgr.disappear_requested = true; // the game's own dismiss (its fade); the remove-now follows if it lingers
                        }
                    }
                    st.linger = Some(0.0);
                }
                erlog!("spirit: {} LEFT: {} (LeaveReason::{reason:?}; {left:.1} of {:.0} s left, HP {}/{}){}", live.ash.name, reason.name(), live.timer.full, live.hp, live.hp_max, if reason.refunds() { "; the core gave the refund" } else { "; no MP back" });
                // (THE USER 2026-11-03) the KH gauge fades out on its final values (summon.rs `final_values`)
                st.leaving = summon::final_values(&out_of(&live), reason).map(|o| (o, reason, 0.0));
            }
        }
    } else if st.requested.is_none() {
        st.asked = None; // (nothing out, nothing coming: an ask is void; one for a spirit still coming waits for it)
    }

    // ---- after a leave: the game's groups until they are gone
    if let Some(t) = st.linger.as_mut() {
        *t += wdt;
        let groups = manager().map(|m| groups_of(m).len()).unwrap_or(0);
        if groups == 0 || *t > 2.0 * LINGER_MAX {
            OUR_ACTIVE.store(false, Ordering::SeqCst);
            st.linger = None;
        } else if *t > LINGER_MAX {
            if let Some(mgr) = manager() {
                let n = remove_all_now(mgr);
                erlog!("spirit: the left spirit was still there after {LINGER_MAX:.0} s: the game's remove-now on {n} group(s)");
            }
        }
    }

    // ---- the log of the detours, now and then
    let told = (KEPT.load(Ordering::Relaxed), HIDDEN.load(Ordering::Relaxed));
    if st.live.is_some() && told.0 > 0 && st.hooks_told.0 == 0 {
        erlog!("spirit: the range detour keeps our spirit in range ({} call(s) so far); Elden Ring's spirit block hidden in {} call(s)", told.0, told.1);
        st.hooks_told = told;
    }
    if !st.told_warp && st.live.is_some() {
        if let Some(m) = manager().filter(|m| plausible(word!(m.warp_manager))) {
            let w = &m.warp_manager;
            erlog!("spirit: Elden Ring's own buddy warp: {:.0} m from the player, path stuck {:.1} s, ray blocked {:.1} s (KH3's rule: {:.0} m)", w.trigger_dist_to_player, w.trigger_threshold_time_path_stacked, w.trigger_time_ray_block, kh_core::magic::tables::Sheet::Mp.num("summon.follow.warp_m"));
            st.told_warp = true;
        }
    }

    // ---- the gauge's fade-out notice: real time (it fades while a save point's menu pauses the world); a new spirit ends it
    if let Some((_, _, age)) = st.leaving.as_mut() {
        *age += f.dt.clamp(0.0, 0.25);
    }
    if st.live.is_some() || st.leaving.as_ref().is_some_and(|l| l.2 > summon::LEAVING_HOLD) {
        st.leaving = None;
    }

    // ---- the view
    let view = SummonView {
        allowed: on && ready && f.leads,
        bell: st.bell,
        slots: pick_slots(&f.sora.command().spirit_ashes, &st.owned),
        out: st.live.as_ref().map(out_of),
        calling: st.pending.is_some() || st.requested.is_some(),
        owned: st.owned.clone(),
        leaving: st.leaving.as_ref().map(|(o, r, _)| (o.clone(), *r)),
    };
    f.sora.command_mut().summon = view;
}

/// THE REQUEST (the RECOMMENDED ROUTE): checks, the spawn point, the fields in the report's order. `Go`: written.
fn begin(ash: &AshSlot, f: &SpiritFrame) -> Decision {
    let Some(mgr) = manager() else { return Decision::Refuse("no summon buddy manager".into()) };
    let Some(repo) = repo() else { return Decision::Refuse("the params are not there".into()) };
    let Some(row) = repo.get::<EquipParamGoods>(ash.goods) else { return Decision::Refuse(format!("goods {} has no row", ash.goods)) };
    let request = request_of(ash.goods, row.ref_id_default(), row.ref_category());
    let facts = Facts {
        player_alive: f.player.chr_ins.modules.data.hp > 0,
        request_free: mgr.request_summon_speffect_id < 0,
        dismissing: mgr.disappear_requested,
        hold: hold_of(mgr),
        cooldown: mgr.item_use_cooldown_timer,
        groups: groups_of(mgr).len(),
        arena: in_arena(),
        offline: offline(),
        stone: stone_problem(&stone_rows()),
        trigger: request.as_ref().ok().and_then(|r| trigger_problem(r.trigger, &buddy_rows(r.trigger))),
        request: request.as_ref().err().cloned(),
    };
    let decision = decide(&facts);
    let (Decision::Go, Ok(req)) = (&decision, request) else { return decision };
    // the place: the game's own finder around the player body (Sora's place while he leads), else Sora's place
    // SAFETY: game thread; CSHavokMan is what the finder's casts use (its null path is a fatal assert: checked first).
    let havok = unsafe { CSHavokMan::instance() }.is_ok();
    let sora_er = f.frame.pos_kh_to_er(f.feet_kh);
    let finder = SPAWN_POINT.load(Ordering::Relaxed);
    let found = havok && finder != 0 && {
        // SAFETY: the game's own finder at its unique pattern, `bool(SummonBuddyManager*)`, on the game thread, with the
        // manager it is always given; the main player exists (checked by the caller's frame).
        let find: SpawnPointFn = unsafe { std::mem::transmute(finder) };
        unsafe { find(mgr) != 0 }
    };
    let origin = game::v3(&mgr.spawn_origin);
    let taken = found && origin.is_finite() && (origin - sora_er).length() <= SPAWN_REACH_M;
    if !taken {
        let q = &f.player.chr_ins.modules.physics.orientation;
        mgr.spawn_origin = game::hp(sora_er);
        mgr.spawn_rotation = yaw_of_quat(q.1, q.3);
    }
    erlog!("spirit: the spawn place is {} ({:.1} m from Sora)", if taken { "the game's own (around the player body)" } else if found { "Sora's own (the game's point was too far)" } else { "Sora's own (the game's finder found none)" }, (game::v3(&mgr.spawn_origin) - sora_er).length());
    // the fields, in the report's order; the request last
    OUR_GOODS.store(req.goods, Ordering::SeqCst);
    OUR_ACTIVE.store(true, Ordering::SeqCst);
    mgr.active_summmon_buddy_stone_entity_id = 0;
    mgr.requested_summon_goods_id = req.goods;
    mgr.active_summon_goods_id = req.goods;
    // (the detour's own values, at once: the game's dismiss task may run between this frame's generator and its next range
    // step, and it dismisses a spirit while +0x20 < 0 and neither range byte is set)
    mgr.is_within_activation_range = true;
    mgr.is_within_warn_range = false;
    mgr.request_summon_speffect_id = req.speffect;
    Decision::Go
}

/// KH3's vanish at a leaving spirit (its effect and the sound it carries), through Sora's own effect / sound side.
fn vanish(sora: &mut kh_core::Sora, frame: Frame, feet_er: Vec3) {
    let at = kh_core::Attach::World(frame.pos_er_to_kh(feet_er) + Vec3::Z * CHEST_CM);
    sora.debug_emit(kh_core::DebugEmit::Effect(kh_core::EffectSpawn { id: kh_core::EffectId(kh_core::notify::notify_asset_id(DISMISS_EFFECT)), name: DISMISS_EFFECT, at, rotation: Vec3::ZERO, scale: 1.0, persistent: false }));
    sora.debug_emit(kh_core::DebugEmit::Sound(kh_core::SoundPlay { id: kh_core::SoundId(kh_core::notify::notify_asset_id(DISMISS_SOUND)), name: DISMISS_SOUND, at, volume: 1.0, pitch: 1.0, looping: false, spatial: true }));
}

/// THE FOLLOW JUDGEMENT of this frame (no writes): KH3's plan, logged when it changes; a lag of the game's warp said once.
fn follow(live: &mut Live, f: &SpiritFrame, mgr: Option<&SummonBuddyManager>, dt: f32) {
    let leader_er = f.frame.pos_kh_to_er(f.leader_kh);
    let place = summon::follow_place(leader_er, f.leader_yaw.to_radians());
    let dist = (live.feet_er - place).length();
    let stuck = live.progress.step(dist, dt);
    let plan = summon::follow_plan(f.in_battle, dist, stuck);
    if plan != live.plan {
        erlog!("spirit: {} {} ({dist:.1} m from its place behind the leader){}", live.ash.name, match plan { FollowPlan::Fight => "fights (its own Elden Ring AI)", FollowPlan::Stay => "is at the leader", FollowPlan::Follow => "follows the leader (Elden Ring's own follow)", FollowPlan::Warp => "is far: Elden Ring's own buddy warp is due" }, if plan == FollowPlan::Warp { format!(", stuck {stuck:.1} s") } else { String::new() });
        if plan != FollowPlan::Follow {
            live.progress.reset();
        }
        live.plan = plan;
        live.lag_told = false;
    }
    live.warp_s = if plan == FollowPlan::Warp { live.warp_s + dt } else { 0.0 };
    let warping = mgr.is_some_and(|m| game_warping(m, live.handle));
    if follow_lags(plan, warping, live.warp_s) && !live.lag_told {
        live.lag_told = true;
        erlog!("spirit: !!! {} has been {dist:.1} m from its place for {:.0} s and Elden Ring's own warp has not taken it (nothing of ours moves it)", live.ash.name, live.warp_s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ash(goods: u32, usable: bool) -> AshSlot {
        AshSlot { goods, level: summon::level_of(goods), name: format!("ash {goods}"), mp: 10, renowned: false, mimic: false, usable, icon: None }
    }

    #[test]
    fn spirit_request_values_from_the_goods_row() {
        // Black Knife Tiche +5 (regulation 2.7.2.0: goods 200005, refId_default 200005, refCategory 2)
        assert_eq!(request_of(200_005, 200_005, 2), Ok(Request { goods: 200_005, speffect: 200_005, trigger: 200_000, level: 5 }));
        assert_eq!(request_of(207_000, 207_000, 2).map(|r| (r.trigger, r.level)), Ok((207_000, 0)), "the Mimic Tear +0");
        assert!(request_of(200_011, 200_011, 2).is_err(), "+11 does not exist");
        assert!(request_of(200_005, 200_005, 0).is_err(), "not a SpEffect use");
        assert!(request_of(200_005, -1, 2).is_err(), "no use effect");
        assert!(request_of(200_005, 200_000, 2).is_err(), "a use effect of another level: the game would summon +0");
    }

    #[test]
    fn spirit_stone_row_0_must_be_flagless_or_the_summon_stops() {
        assert_eq!(stone_problem(&[(0, 0)]), None, "regulation 2.7.2.0 row 0");
        assert!(stone_problem(&[]).is_some(), "no row: nothing would spawn");
        assert!(stone_problem(&[(0, 0), (10_002_100, 0)]).is_some_and(|w| w.contains("10002100")), "a second row with a save flag stops it");
        assert!(stone_problem(&[(0, 31_000_800)]).is_some(), "an eliminate target too");
        assert_eq!(trigger_problem(200_000, &[(20_000_000, 0)]), None);
        assert!(trigger_problem(215_000, &[(21_500_000, 1), (21_500_001, 1)]).is_some_and(|w| w.contains("Rebirth Monument")));
        assert!(trigger_problem(1, &[]).is_some());
    }

    #[test]
    fn spirit_abort_checks_refuse_the_lasting_and_wait_for_the_passing() {
        let ok = Facts { player_alive: true, request_free: true, arena: Some(false), offline: Some(true), ..Facts::default() };
        assert_eq!(decide(&ok), Decision::Go);
        assert!(matches!(decide(&Facts { stone: Some("flag".into()), ..ok.clone() }), Decision::Refuse(_)), "the stone rule first");
        assert!(matches!(decide(&Facts { arena: Some(true), ..ok.clone() }), Decision::Refuse(_)));
        assert!(matches!(decide(&Facts { arena: None, ..ok.clone() }), Decision::Refuse(_)), "no arena function: no summon");
        assert!(matches!(decide(&Facts { offline: Some(false), ..ok.clone() }), Decision::Refuse(_)));
        assert!(matches!(decide(&Facts { player_alive: false, ..ok.clone() }), Decision::Refuse(_)));
        assert!(matches!(decide(&Facts { request_free: false, ..ok.clone() }), Decision::Wait(_)));
        assert!(matches!(decide(&Facts { dismissing: true, ..ok.clone() }), Decision::Wait(_)));
        assert!(matches!(decide(&Facts { hold: 2, ..ok.clone() }), Decision::Wait(_)));
        assert!(matches!(decide(&Facts { cooldown: 0.4, ..ok.clone() }), Decision::Wait(_)));
        assert_eq!(decide(&Facts { groups: 1, ..ok.clone() }), Decision::Wait("an earlier spirit is still there"));
        assert!(matches!(decide(&Facts { groups: 1, request: Some("bad".into()), ..ok }), Decision::Refuse(_)), "a lasting problem beats a passing one");
    }

    #[test]
    fn spirit_slots_take_the_highest_level_owned_of_their_base() {
        let owned = vec![ash(200_003, true), ash(200_005, true), ash(214_000, true)];
        let slots = pick_slots(&[200_000, 0, 214_000, 207_000], &owned);
        assert_eq!(slots[0].as_ref().map(|a| a.goods), Some(200_005), "+5 over +3");
        assert_eq!(slots[1], None, "an empty slot");
        assert_eq!(slots[2].as_ref().map(|a| a.goods), Some(214_000));
        assert_eq!(slots[3], None, "an ash he does not own");
        assert_eq!(ash_name(Some("Black Knife Tiche"), 200_003, 3), "Black Knife Tiche +3");
        assert_eq!(ash_name(Some("Lone Wolf Ashes +2"), 230_002, 2), "Lone Wolf Ashes +2", "the game's own level is not doubled");
        assert_eq!(ash_name(None, 200_000, 0), "Spirit ash 200000");
    }

    #[test]
    fn spirit_leaves_by_the_first_reason_that_holds() {
        let up = LiveFacts { present: true, hp: 500, ..LiveFacts::default() };
        assert_eq!(leave_of(&up), None);
        assert_eq!(leave_of(&LiveFacts { timed_out: true, ..up }), Some(LeaveReason::Timeout));
        assert_eq!(leave_of(&LiveFacts { hp: 0, timed_out: true, ..up }), Some(LeaveReason::Died), "death before the timer");
        assert_eq!(leave_of(&LiveFacts { present: false, ..up }), Some(LeaveReason::Gone), "a map unload, the game's own end");
        assert_eq!(leave_of(&LiveFacts { game_dismissing: true, ..up }), Some(LeaveReason::Gone));
        assert_eq!(leave_of(&LiveFacts { present: false, hp: 0, ..up }), Some(LeaveReason::Died), "gone after its HP reached 0");
        assert_eq!(leave_of(&LiveFacts { asked: Some(LeaveReason::Unsummon), hp: 0, ..up }), Some(LeaveReason::Unsummon), "ours first");
        assert_eq!(leave_of(&LiveFacts { asked: Some(LeaveReason::SavePoint), ..up }), Some(LeaveReason::SavePoint));
        assert!(vanishes(LeaveReason::Unsummon) && vanishes(LeaveReason::Timeout) && vanishes(LeaveReason::SavePoint));
        assert!(!vanishes(LeaveReason::Died) && !vanishes(LeaveReason::Gone), "the game's own ends show its own");
    }

    /// (NOCOMBAT) "you can't summon in round table hold either": a spirit out on arrival (fast travel into the Hold) leaves
    /// the save point's way (KH3's vanish, the game's fade), Mimic Sora as any ash; outside it stays.
    #[test]
    fn nocombat_a_spirit_out_on_arrival_leaves_as_at_a_save_point() {
        use kh_core::nocombat::{self, Fighter};
        for who in [Fighter::SpiritAsh, Fighter::MimicSora] {
            let zone = nocombat::scope(true);
            assert_eq!(sent_away(false, nocombat::may_stay(who)), Some(LeaveReason::SavePoint), "{}", who.name());
            assert!(vanishes(LeaveReason::SavePoint), "with the fade");
            drop(zone);
            let _out = nocombat::scope(false);
            assert_eq!(sent_away(false, nocombat::may_stay(who)), None, "{} stays outside", who.name());
        }
        assert_eq!(sent_away(true, true), Some(LeaveReason::SavePoint), "the rest at a grace, as before");
    }

    #[test]
    fn spirit_follow_progress_and_the_lag_of_the_game_warp() {
        let mut p = Progress::default();
        assert_eq!(p.step(10.0, 0.5), 0.0, "the first reading");
        assert_eq!(p.step(9.0, 0.5), 0.0, "nearer");
        assert_eq!(p.step(9.0, 0.5), 0.5);
        assert_eq!(p.step(8.9, 0.5), 1.0, "less than the gain is no progress");
        assert_eq!(p.step(8.0, 0.5), 0.0, "progress again");
        assert!(follow_lags(FollowPlan::Warp, false, 5.0));
        assert!(!follow_lags(FollowPlan::Warp, true, 9.0), "the game's warp runs");
        assert!(!follow_lags(FollowPlan::Warp, false, 1.0));
        assert!(!follow_lags(FollowPlan::Follow, false, 9.0));
        let q = crate::coords::quat_er_from_yaw(30.0);
        assert!((yaw_of_quat(q.y, q.w) - crate::coords::yaw_kh_to_er(30.0)).abs() < 1e-4);
    }

    #[test]
    fn spirit_offsets_match_the_report() {
        // the RE report's numbers (IMPL_SUMMON_RE.md "Offsets"), asserted against the SDK at compile time above
        assert_eq!((off::REQUEST, off::DISAPPEAR, off::ACTIVE_STONE, off::GROUPS, off::HOLD, off::COOLDOWN), (0x20, 0x28, 0x3c, 0x70, 0x88, 0x90));
        assert_eq!((off::ORIGIN, off::YAW, off::IN_RANGE, off::IN_WARN, off::REQ_GOODS, off::ACTIVE_GOODS), (0xa0, 0xb0, 0xb5, 0xb7, 0xe0, 0xe4));
        assert_eq!((off::FE_ENABLE, off::FE_COUNT, off::FE_DISPLAYS), (0x4D58, 0x4DD8, 0x4DE0));
        assert_eq!(LOCATED.iter().map(|l| l.rva_2_7_2_0).collect::<Vec<_>>(), [0x4b_d950, 0x4b_9f50, 0x4b_77f0, 0x77_1ed0, 0xd5_02a0]);
        assert_eq!(off::STONE_IDS + 8, 0x3730, "the vector's begin pointer: the lookup's `mov r9,[rcx+3730h]`");
    }
}
