//! `kh_core::Targets` over Elden Ring's characters (IMPL_ER4_STATE.md, item 1).
//!
//! Once per game frame, before Sora's steps (`refresh`), the characters near him are read from
//! the SDK's character sets into plain rows (`Enemy`); the trait answers from those rows, so that
//! the core's questions within a step see one consistent world. The one thing that reaches back
//! into the game inside a step is `apply_hit` (damage.rs), on a character pointer read in the
//! same frame phase.
//!
//! WHO IS A TARGET: a living character the game itself lets the player target (its own "can
//! target" function, found by erfps2's pattern; fallback: the enemy team numbers of ermap.rs),
//! not the player, not a character of the summon set (Torrent, spirit ashes). Everything else
//! near him is a BYSTANDER (alive, not lockable, not hittable: the core side-steps round it).
//!
//! (IMPL_ER51; the user: "You can't damage any friendly NPCs at the moment", "Hitting Boc when he's a tree doesn't turn
//! him back into Boc") FRIENDLY NPCs ARE HIT, NEVER TARGETED (`hit.npcs`, default 1; 0 = every non-hostile character is
//! a bystander, as before): a living, active character of type NPC that the game does not let the player target (a
//! merchant, a quest NPC, Boc in his bush form) is offered to kh-core as ALIVE | HITTABLE without ENEMY or LOCKABLE, as
//! the breakable props are: Sora's soft lock, lock-on, its cycling, magic homing and attack steering (all of which need
//! LOCKABLE) never pick one, but an attack volume that overlaps one hits it. The hit is the same player-owned bullet as
//! on an enemy; THE GAME decides what follows (its damage and reaction animation, the NPC turning hostile, Boc's reveal
//! by his own event, invulnerability). No KH3 puppet / juggle, no poise rules, no direct HP write when a bullet takes
//! nothing (a character the game keeps unhurt stays unhurt). Not friendly-hittable: the summon set, Torrent, the
//! invisible helper characters (c1000: graces and event helpers; team 0), players and phantoms (any type but NPC), and a
//! character whose hit body the game has switched off (`ChrCtrl` "disable hit") UNLESS it is a disguise kind (IMPL_ER54,
//! `hit.disguised`, `DISGUISED`: Boc in his bush): then the bullet crosses the disguise asset the game's reveal waits on. REWARDS: Sora's EXP / prize / loot /
//! boss finish come from a death only when the character was seen HOSTILE while it lived (an NPC the game turned
//! hostile and Sora then killed counts as any enemy); a friendly NPC killed while still friendly gives none of Sora's
//! own (the game's own runes and drops for the player are the game's).
//!
//! IDS: a character's `FieldInsHandle` (8 bytes) gets the next free number the first time it is
//! seen and keeps it for as long as this Sora runs (a handle names one character of a map block;
//! a respawned enemy has its old handle and so its old id).
//!
//! BODY: one upright capsule from `CSChrPhysicsModule::hit_radius / hit_height` (the SDK exposes
//! no per-part hurt shapes); the weight class is taken from its size (ermap.rs) and sets the push
//! level. Every body is `BodyReaction::Default` (no guard, no armour): Elden Ring's own guard and
//! poise act when the damage goes through its pipeline, which the HP route does not (damage.rs).

use std::collections::HashMap;

use eldenring::cs::{ChrIns, ChrSet, ChrSetEntry, EnemyIns, FieldInsHandle, PlayerIns, WorldChrMan};
use fromsoftware_shared::{FromStatic, Superclass};
use glam::Vec3;
use kh_core::host::{PushBody, TargetBody};
use kh_core::{HitRecord, HitResult, HurtShape, TargetFlags, TargetId, TargetInfo, Targets};

use crate::coords::{self, Frame, CM_PER_M};
use crate::ermap::{Params, Weight};
use crate::erlog;
use crate::game::{self, plausible};
use crate::havok::{locate, Located};
use crate::word;

/// The game's own "may this character target that one" (erfps2 `CHR_CAN_TARGET_RVA`, its pattern).
pub(crate) const CAN_TARGET: Located = Located { name: "chr can target", pattern: "56 48 83 ec 30 48 8b f2 48 85 c9 74 ? 48 85 d2 74 ? 48 3b ca", capture: 0, rva_2_7_1_0: 0x0051_b611, rva_2_7_2_0: 0x0051_b8b1 /* (PORT) */ };

static CAN_TARGET_FN: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();

fn can_target_fn() -> Option<usize> {
    *CAN_TARGET_FN.get_or_init(|| {
        let found = locate(&CAN_TARGET);
        match found {
            Some(_) => erlog!("targets: the game's can-target function located by pattern at the recorded RVA (PORT)"),
            None => erlog!("targets: the game's can-target function was NOT located: hostility falls back to team numbers"),
        }
        found
    })
}

/// One character near Sora, as read this frame.
#[derive(Clone, Debug)]
pub struct Enemy {
    pub id: TargetId,
    pub handle: FieldInsHandle,
    /// The character object: valid in the frame phase it was read in, never kept across frames.
    chr: usize,
    /// Feet, Elden Ring space (m) and the core's (cm).
    pub er_pos: Vec3,
    pub pos: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub radius: f32,
    pub half_height: f32,
    pub hp: i32,
    pub hp_max: i32,
    /// (HUD2) THE GAME'S OWN HP as read from the character this frame (or as written by an HP write of ours), NEVER
    /// the expected HP a bullet in flight leaves in `hp` until the game applies it. KH3's enemy gauge reads the pawn's
    /// real HP (vt+0x17B0); the gauge showed `hp`, so a hit made the green drop to the guess, go back up to the
    /// untouched HP at the next read (bullet not landed yet, or waiting its turn) and drop again to what the game
    /// really took: the user's "the green bar jumps before settling". The gauge reads this instead.
    pub hp_game: i32,
    pub team: u8,
    pub chr_type: i32,
    pub npc_id: i32,
    pub npc_param: i32,
    pub think_param: i32,
    pub alive: bool,
    /// The game lets the player target it (or its team is an enemy team: the fallback).
    pub hostile: bool,
    /// (IMPL_ER51) A friendly NPC: Sora's attacks hit it, nothing of his targets it (`Class::Friendly`).
    pub friendly: bool,
    /// (IMPL_ER54) In disguise (its hit body off, a `hit.disguised` kind; friendly or hostile): the bullet must cross its disguise.
    pub disguised: bool,
    pub can_target_raw: u32,
    pub summon: bool,
    pub lock_disabled: bool,
    pub weight: Weight,
    pub active: bool,
    /// The animation the game plays on it now (TimeAct id), for the log.
    pub anim: i32,
    /// Poise: the larger of the super-armour and toughness maxima, and what is left of it.
    pub poise_max: f32,
    pub poise: f32,
    /// (IMPL_ER45) The game's hyper-armour flag (TAE ChrActionFlag 24 SUPER_ARMOR of CSChrActionFlagModule; `None`: the
    /// module is not readable), it plays an attack animation (x3000..x3999), and the refill time of its poise in seconds
    /// (its NpcParam's superArmorRecoverCorrection when usable, else the sheet's; 0 for bodies that are not hostile).
    pub hyper: Option<bool>,
    pub in_attack: bool,
    /// (BM, the user: "combat animations shouldn't start when only animals are around you") THE GAME'S OWN THREAT LEVEL of
    /// its kind (NpcParam `threatLv`; `None`: the row is not readable) and whether that makes it PASSIVE (`passive`):
    /// a hostile (lockable) character whose row says threat level 0. The user's logs and the game's regulation: the
    /// animal kinds seen near the party (c6001 / c6010 / c6040 / c6050 / c6060 / c6070 / c6090 / c6100, 27 - 378 HP) are
    /// team 6 (ER's plain Enemy team, the same as the soldiers: the team cannot tell them apart) with threatLv 0; the
    /// fighters of the same logs (c3000 / c3020 / c4070 / c4311 / c4371 ...) are 1, bosses up to 9; the
    /// variants NpcParam 60601010 / 60501010 and the kind c6080 are 1 (the game marks those as threats). Known exception: c4190 - c4192 (blood-immune
    /// fighters, 205 rows of c4191) are 0 too: they start the battle at their first swing or hit. A passive one starts no
    /// battle unless it fights: see `engages`.
    pub threat_lv: Option<u32>,
    pub passive: bool,
    pub poise_recover: f32,
    /// The class table (ermap.rs) lets it be launched and juggled.
    pub puppet_ok: bool,
    /// It is our puppet in the air this step (puppet.rs wrote this row).
    pub airborne: bool,
    /// (IMPL_ER14) Elden Ring's own physics has it on the ground (standing on / touching solid ground, not falling).
    pub grounded: bool,
    /// (IMPL_ER23A) IT HOVERS ABOVE ITS POSITION NODE by animation: `pos` / `er_pos` (and so its capsule, the marker, the
    /// camera's point, the bullets) are this far (cm) above the game's physics position; 0 for everything else.
    pub lift: f32,
}

// (ER party S7) a row for the offline harnesses (party/team/tests.rs): no game object behind it (`chr` 0 is never
// dereferenced by the pure rules: `enumerate` / `info` / `hurt_shapes` / the puppets' reaction rules). Tests only.
#[cfg(test)]
impl Enemy {
    pub fn stub(id: u32, pos: Vec3, radius: f32, half_height: f32, hp: i32, weight: Weight) -> Enemy {
        // SAFETY: a plain handle value (two integers), never used to reach the game.
        let handle = unsafe { std::mem::zeroed::<FieldInsHandle>() };
        Enemy { id: TargetId(id), handle, chr: 0, er_pos: Vec3::ZERO, pos, velocity: Vec3::ZERO, yaw: 180.0, radius, half_height, hp, hp_max: hp, hp_game: hp /* (HUD2) */, team: 6, chr_type: 0, npc_id: 4000 + id as i32, npc_param: 40000000 + id as i32, think_param: 0, alive: true, hostile: true, friendly: false, disguised: false, can_target_raw: 1, summon: false, lock_disabled: false, weight, active: true, anim: -1, poise_max: 0.0, poise: 0.0, hyper: None, in_attack: false, threat_lv: Some(1), passive: false /* (BM) */, poise_recover: 0.0, puppet_ok: weight != Weight::Large, airborne: false, grounded: true, lift: 0.0 }
    }
}

/// (ZETTA 2026-10-23) The body's armour broken (super armour, toughness to 0); what it was (None: no armour module read).
fn break_armour(chr: &mut ChrIns) -> Option<(f32, f32)> {
    let sa_ok = game::plausible(crate::word!(chr.modules.super_armor));
    let t_ok = game::plausible(crate::word!(chr.modules.toughness));
    if !sa_ok && !t_ok {
        return None;
    }
    let sa = if sa_ok { chr.modules.super_armor.sa_durability } else { 0.0 };
    let tough = if t_ok { chr.modules.toughness.toughness } else { 0.0 };
    if sa_ok {
        chr.modules.super_armor.as_mut().sa_durability = 0.0;
    }
    if t_ok {
        chr.modules.toughness.as_mut().toughness = 0.0;
    }
    Some((sa, tough))
}

/// (ZETTA 2026-10-23) The body's armour put back as it was (a survivor fights on with it).
fn put_armour(chr: &mut ChrIns, sa: f32, tough: f32) {
    if game::plausible(crate::word!(chr.modules.super_armor)) {
        let m = chr.modules.super_armor.as_mut();
        m.sa_durability = sa.min(m.sa_durability_max.max(sa));
    }
    if game::plausible(crate::word!(chr.modules.toughness)) {
        chr.modules.toughness.as_mut().toughness = tough;
    }
}

/// (ZETTA 2026-10-23) Zettaflare's target in its heavy hurt (zpose.rs): who, its stage, the frames its sounds were muted, the
/// frame it began.
#[derive(Clone, Debug)]
pub struct ZPose {
    pub id: u32,
    pub handle: FieldInsHandle,
    pub npc_id: i32,
    pub stage: crate::zpose::Stage,
    pub muted: u32,
    pub frame: u64,
    /// (ZETTA 2026-10-23) its armour as it was before the first forced ask (super armour, toughness): put back at the let-go
    pub armour: Option<(f32, f32)>,
}

/// (ZETTA 2026-10-23) Does the party's end of the hold give the body back? Not one Zettaflare killed (dead on KH3's side, HP
/// owed): the hand-back does, after its death (the user: "Melania was also attacking during Zettaflare!").
pub fn zetta_gives_back(alive: bool, owed: i32) -> bool {
    alive || owed <= 0
}

/// (ZETTA 2026-10-23) How long (s) a character stays ours to target after the game's lock-on last listed it.
pub const LOCK_GATE_HOLD_S: f32 = 3.0;

/// (ZETTA 2026-10-23) The gate's rule: listed by the game's lock-on now or within `LOCK_GATE_HOLD_S`, or held / owed by
/// Zettaflare (never dropped mid-sequence).
pub fn lock_gate_open(cand_age: Option<f32>, held: bool) -> bool {
    held || cand_age.is_some_and(|a| a <= LOCK_GATE_HOLD_S)
}

/// (ZETTA 2026-10-23) The characters Elden Ring's own lock-on lists as candidates this frame (their handles' bits); None: its
/// manager is not there. Game thread.
fn er_lock_candidates() -> Option<std::collections::HashSet<u64>> {
    use eldenring::cs::LockTgtMan;
    // SAFETY: game thread; every pointer of the list is checked.
    unsafe {
        let man = LockTgtMan::instance_mut().ok()?;
        let mut out = std::collections::HashSet::new();
        let mut seen: Vec<usize> = Vec::new();
        let mut next = word!(man.nodes);
        while plausible(next) && seen.len() < 256 && !seen.contains(&next) {
            seen.push(next);
            let node = &*(next as *const eldenring::cs::LockTgtNode);
            let value = word!(node.value);
            if plausible(value) {
                out.insert(game::handle_bits(&(*(value as *const eldenring::cs::LockTgtNodeValue)).chr_handle));
            }
            next = word!(node.next);
        }
        Some(out)
    }
}

/// (ZETTA 2026-10-23) WHICH OF A SPOT'S ROWS IS THE BODY: among the living, active hostile rows standing on one another
/// (`zfinale::coincident`), the one the game fights with: an armoured one (poise) before one without, more HP left, then the
/// lower id (stable). True = a duplicate (no target). A boss whose shown instance changes (its next phase) moves the target
/// to the new one by the same rule.
pub fn duplicates(list: &[Enemy]) -> Vec<bool> {
    let live = |e: &Enemy| e.hostile && e.alive && e.active;
    let rank = |e: &Enemy| (e.poise_max > 0.0, e.hp_game.max(e.hp), std::cmp::Reverse(e.id.0));
    list.iter()
        .map(|e| {
            live(e) && list.iter().any(|o| o.id != e.id && live(o) && o.npc_id == e.npc_id && crate::zfinale::coincident(o.pos, o.radius, e.pos, e.radius) && rank(o) > rank(e))
        })
        .collect()
}

/// (ZETTA) Zettaflare's finale per character (target id, its handle, the look asked): for the character draw route.
pub static ZETTA_FINALE: std::sync::Mutex<Vec<(u32, FieldInsHandle, kh_core::host::ScriptFinale)>> = std::sync::Mutex::new(Vec::new());

/// ONE HIT OF SORA'S ON AN ELDEN RING CHARACTER, as the host delivered it: THE ONE PLACE the
/// effect renderer, the audio and anything else that reacts to hits read them from
/// (`ErTargets::events`, a per-frame list: cleared at the start of every frame's simulation,
/// filled by `apply_hit` during Sora's steps of that frame; `SoraHost::hit_events`).
#[derive(Clone, Copy, Debug)]
pub struct HitEvent {
    /// Sora's step the hit was delivered in.
    pub step: u64,
    pub target: TargetId,
    pub handle: FieldInsHandle,
    /// Row index and name of the attack (`HitRecord::attack`, `attack_name`).
    pub attack: u32,
    pub attack_name: &'static str,
    /// `ETresDamageKind` value: the reaction kind KH3 asks for.
    pub kind: i32,
    /// The contact point, the unit direction of the blow and the impact normal (victim's surface
    /// toward the attacker's shape), in the core's world (cm) and the point in Elden Ring's (m).
    pub position: Vec3,
    pub position_er: Vec3,
    pub direction: Vec3,
    pub normal: Vec3,
    /// KH3 damage of the hit and the Elden Ring attack value asked for it.
    pub damage: f32,
    pub er_damage: i32,
    /// The Elden Ring `dmgLevel` asked for.
    pub level: u8,
    /// What KH3's reaction rules (kh-react) made of the hit for this body: the reaction KIND that
    /// started (an `ETresDamageKind` value; a STRONG body downgrades or refuses), 0 = none.
    pub reaction: i32,
    /// The target was our puppet in the air when it was hit.
    pub airborne: bool,
    /// The hit took no HP (guarded / no-damage row).
    pub guarded: bool,
    /// "bullet" (the game's own pipeline) or "hp" (the direct write).
    pub route: &'static str,
}

/// What is known about where bullets land on one kind of character (by character id): the
/// fraction of the capsule's height that landed, or how many heights were tried without.
#[derive(Clone, Copy, Debug, Default)]
struct AimMemo {
    good: Option<f32>,
    tries: u8,
}

/// The heights tried after the contact's own (fractions of the capsule's height).
const AIM_TRIES: [f32; 5] = [0.3, 0.55, 0.15, 0.75, 0.42];

impl AimMemo {
    fn fraction(&self) -> Option<f32> {
        match (self.good, self.tries) {
            (Some(f), _) => Some(f),
            (None, 0) => None,
            (None, n) => Some(AIM_TRIES[(n as usize - 1) % AIM_TRIES.len()]),
        }
    }
}

/// (IMPL_ER54) THE CHARACTERS THE GAME HIDES INSIDE A DISGUISE ASSET (`hit.disguised`). READ FROM THE GAME'S OWN FILES
/// (er54 survey, read-only): every bush disguise of the event scripts is an asset of model AEG801_480 placed exactly on a
/// character whose hit body the script switches off (`2004[39]` 0), and the reveal WAITS FOR THE PLAYER (20000) TO DAMAGE
/// THAT ASSET (`3[23]` on the asset's entity): Boc the Seamster = c4110_9000 (entity 1043370750, NpcParam 41109010) in
/// AEG801_480_9000 (entity 1043371700), m60_43_37_00 event 1043373732 -> flag 1043379357, leaf burst sfx 641012, the bush
/// asset disabled, his hit body on; the bush demi-humans = c4100 in AEG801_480 (common_func 90005736 and blocks
/// m60_43_34 / 44_33 / 44_34 / 44_37).
pub const DISGUISED: [i32; 2] = [4100, 4110];

/// (IMPL_ER54) `hit.disguised = id,id` (character ids, e.g. 4110 for c4110; "none" = no disguise is hit).
pub fn disguised_ids(text: Option<&str>) -> Vec<i32> {
    match text.map(str::trim) {
        None | Some("") => DISGUISED.to_vec(),
        Some(t) if t.eq_ignore_ascii_case("none") || t == "0" => Vec::new(),
        Some(t) => t.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|w| w.trim().trim_start_matches(['c', 'C']).parse::<i32>().ok()).filter(|v| *v > 0).collect(),
    }
}

/// (IMPL_ER51) WHAT ONE CHARACTER IS TO SORA'S ATTACKS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// The game lets the player target it: an enemy (locked on to, steered at, hit, juggled).
    Hostile,
    /// A friendly NPC: hit when an attack volume overlaps it, never targeted.
    Friendly,
    /// A body in the way, nothing more.
    Bystander,
}

/// (IMPL_ER51) What `classify` reads off one character.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// The game's can-target answer (or the fallback team rule).
    pub can_target: bool,
    /// Of the summon set, or Torrent.
    pub summon: bool,
    /// Its character type is NPC (not the player, not a phantom or ghost).
    pub npc_type: bool,
    /// An invisible helper character: c1000 (graces, event helpers) or team 0.
    pub helper: bool,
    /// The game has switched its hit body off.
    pub hit_off: bool,
    /// (IMPL_ER54) Its character id is one of `hit.disguised` (characters the game hides inside a disguise asset).
    pub disguise_kind: bool,
}

/// THE CLASSIFICATION: `npcs` = `hit.npcs` (0: every non-hostile character is a bystander, as before IMPL_ER51).
pub fn classify(f: &Facts, npcs: bool) -> Class {
    if f.summon {
        Class::Bystander
    } else if f.can_target {
        Class::Hostile
    } else if npcs && f.npc_type && !f.helper && (!f.hit_off || f.disguise_kind) {
        Class::Friendly
    } else {
        Class::Bystander
    }
}

/// THE CORE'S FLAGS of a character of `class` (`None`: not offered to the core). Hostile: ENEMY, and while it lives and
/// is active ALIVE | HITTABLE (+ LOCKABLE unless the game disabled lock-on on it). Friendly: ALIVE | HITTABLE while it
/// lives and is active, never ENEMY or LOCKABLE (soft lock, lock-on, homing and steering need LOCKABLE). Bystander:
/// ALIVE while it lives and is active (a body to sway round and be pushed by).
pub fn flags_of(class: Class, alive: bool, active: bool, lock_disabled: bool) -> Option<u32> {
    let live = alive && active;
    match class {
        Class::Hostile => Some(TargetFlags::ENEMY | if live { TargetFlags::ALIVE | TargetFlags::HITTABLE | if lock_disabled { 0 } else { TargetFlags::LOCKABLE } } else { 0 }),
        Class::Friendly => live.then_some(TargetFlags::ALIVE | TargetFlags::HITTABLE),
        Class::Bystander => live.then_some(TargetFlags::ALIVE),
    }
}

/// (SP2) Elden Ring's invisible helper character: one per site of grace (the grace's own c1000, which stands 0.5 m from the
/// grace, i.e. inside our KH3 save point) and the event scripts' helpers.
pub const NPC_HELPER: i32 = 1000;

/// (SP2, the user 2026-10-20: "Sora is tripping past save points like when you run past a friendly NPC, he should not do
/// this") WHY HE TRIPPED: a site of grace is a character of the game (c1000, IMPL_ER9) that stands inside the save point;
/// as a non-hostile character it was offered to kh-core as a BYSTANDER (ALIVE: "a body in the way"), and KH3's Run state
/// side-steps every living pawn its body circle touches at full run speed (TresPlayerState_Swaying, `RunSwayL` / `RunSwayR`:
/// kh-core states/swaying.rs `run_contact`) and its push (combat/push.rs) moves him off it. THE RULE: a helper character
/// is no body for the core at all (not offered: no sway, no push, no target, nothing hits it) for every leader (Sora, AI
/// Sora behind a friend leader, the friends: they read the same target list). Hostile characters are never excluded.
pub fn offered_as_body(class: Class, npc_id: i32) -> bool {
    class == Class::Hostile || npc_id != NPC_HELPER
}

/// (IMPL_ER51) Whether a friendly NPC's waiting bullet landed: the game took HP, or it plays another animation than at
/// the spawn (a reaction, Boc's reveal) although it lost nothing.
fn friendly_landed(hp_before: i32, hp_now: i32, anim_before: i32, anim_now: i32) -> bool {
    hp_now < hp_before || (anim_before != anim_now && anim_now >= 0)
}

/// A bullet of ours whose damage has not been seen on its target yet.
#[derive(Clone, Copy, Debug)]
struct Pending {
    id: u32,
    frame: u64,
    wanted: i32,
    hp_before: i32,
    /// The height it flew at (a fraction of the capsule's height) and the kind of body.
    fraction: f32,
    npc_id: i32,
    /// Where it started and which way it flew (Elden Ring space), for the diagnosis.
    from: Vec3,
    dir: Vec3,
    /// The attack it carried and how often it was sent again after taking nothing.
    attack: Option<crate::bullet::Attack>,
    tries: u8,
    /// (IMPL_ER51) At a friendly NPC: no HP write when it takes nothing, one try more at most; its animation at the spawn.
    friendly: bool,
    anim_before: i32,
    /// (ZETTA 2026-10-23) A forced hit (Zettaflare, the user: "Zettaflare ignores all armor, it takes priority over ANYTHING"):
    /// the Elden Ring HP it must take in FULL; what the game's defences / absorption keep is written to its HP directly
    /// (`reconcile`). 0: an ordinary hit.
    full: i32,
}

/// (ZETTA 2026-10-23) How far past its capsule a body's limbs and weapon reach, sideways and up (the framing and the beam's
/// cover of Zettaflare take them in: a long sword, a raised arm).
pub const ZETTA_REACH_RADIUS: f32 = 2.2;
pub const ZETTA_REACH_HEIGHT: f32 = 1.25;

/// (ZETTA 2026-10-23) The real body of `e` as the game draws it: its capsule, the largest of the drawn rows on its very spot
/// (`zfinale::coincident`: one body, whichever row was locked); undrawn rows count for nothing.
pub(crate) fn drawn_body(e: &Enemy, list: &[Enemy]) -> (f32, f32) {
    list.iter().filter(|o| o.active && crate::zfinale::coincident(o.pos, o.radius, e.pos, e.radius)).fold((e.radius, e.half_height), |(r, h), o| (r.max(o.radius), h.max(o.half_height)))
}

/// (ZETTA 2026-10-23) A forced hit's bullet: its full number (within the attack value's limit); the rest the game's defences
/// keep is written to its HP (`reconcile`), so it lands exactly in full, never more.
pub(crate) fn forced_ask(full: i32, _ratio: f32) -> i32 {
    // (exact: the full number itself; what the game keeps of it is written to its HP afterwards, never more than asked)
    full.clamp(1, 60000)
}

/// (ZETTA 2026-10-23) What a forced hit still owes after the game took `drop` HP for bullets asking `asked` (of which the
/// forced ones asked `forced_asked` and must take `full`): the forced share of the drop counted against its full number.
pub(crate) fn forced_shortfall(full: i32, drop: i32, forced_asked: i32, asked: i32) -> i32 {
    if full <= 0 || asked <= 0 {
        return 0;
    }
    let share = (drop.max(0) as f32 * forced_asked as f32 / asked as f32).round() as i32;
    (full - share).max(0)
}

/// A bullet that took nothing is sent again this many times (from Sora's side, at another
/// height) before the hit is written to the HP directly.
const RETRIES: u8 = 2;

/// Frames after which a bullet that took nothing off its target is called a miss.
const MISS_FRAMES: u64 = 8;

/// What is remembered about a target across frames.
#[derive(Clone, Copy, Debug, Default)]
struct Memory {
    /// (ZETTA 2026-10-23) seconds since the game last listed it among its own lock-on candidates (`LockTgtMan`)
    cand_age: Option<f32>,
    er_pos: Option<Vec3>,
    /// Sora's step in which he last hit it / it last hit him.
    hit_step: u64,
    attacked_step: u64,
    /// HP after our last write (the death is reported once).
    dead_logged: bool,
    /// HP and life as last read; the frame Sora last hit it in.
    hp: i32,
    was_alive: bool,
    hit_frame: u64,
    /// Elden Ring HP actually taken per attack point asked (defences), smoothed; 0 = not measured.
    ratio: f32,
    /// (IMPL_ER23A) it was last seen hovering above its position node (the log line is written at a change).
    hovering: bool,
    /// (IMPL_ER51) Seen hostile while alive (Sora's rewards for a death need it: a target the game made untargetable for
    /// a moment keeps it); its class and team as last seen (the `npc:` lines are written at a change); Sora's hits on it
    /// while it was friendly.
    hostile_alive: bool,
    class: Option<Class>,
    team: u8,
    friendly_hits: u32,
    /// (BM) Sora's step in which it was last seen in an attack animation (`Enemy::in_attack`): a passive one engages then.
    attack_seen_step: u64,
    /// (IMPL_ER57) whether it was drawn when last told (the `target:` line of a hostile that is not drawn).
    drawn_told: Option<bool>,
}

pub struct ErTargets {
    /// (ZETTA 2026-10-23) the game's lock-on candidates have been readable (a list seen with a node in it): the gate holds
    pub lock_gate_live: bool,
    /// (ZETTA 2026-10-23) bodies Zettaflare killed, handed back: their death is watched (id, handle bits, seconds since)
    pub zdeaths: Vec<(u32, u64, f32)>,
    /// (ZETTA 2026-10-24) a two-phase boss killed whole: at the hit, its final form's rows (body id -> rows); after the
    /// hand-back, every row watched to its death (id, handle bits, seconds, HP written, the body)
    zwhole_plan: HashMap<u32, Vec<u32>>,
    zwhole: Vec<(u32, u64, f32, bool, bool)>,
    /// (ZETTA 2026-10-24) a floor the hit did not see: the whole kill started from the watch (body, handle bits, final rows)
    zwhole_late: Vec<(u32, u64, Vec<u32>)>,
    /// (ZETTA 2026-10-24) bosses whose change the game keeps (story-critical, a surrender): never killed whole, not even late
    zkeep: Vec<u32>,
    /// (ZETTA 2026-10-23, the user: "It also played the standard boss kingdom hearts cinematic death instead of just
    /// Zettaflare! Zettaflare overrules that remember!!") the deaths that are Zettaflare's (id, seconds since): no KH3 finish
    pub zetta_kills: Vec<(u32, f32)>,
    /// (ZETTA 2026-10-23, the user: "please make sure the enemy hurt animations are their heavy hurt animations, and it should be
    /// paused around the middle of the animation so they are visibly hurt") Zettaflare's target in its heavy hurt (zpose.rs)
    pub zpose: Option<ZPose>,
    /// (ZETTA 2026-10-23) the dmgLevel of the next hit's bullet, instead of its role's (Zettaflare's beam hit on a posed body: 0)
    zlevel: Option<u8>,
    /// (ZETTA 2026-10-23) Zettaflare's owed HP and its ownership of the presentation (zdefer.rs)
    pub zdefer: crate::zdefer::Book,
    zowner: crate::zdefer::Owner,
    pub zowned: bool,
    pub params: Params,
    /// (IMPL_ER14) the sheet's numbers as read at the start (a test key that goes away gives its value back).
    pub sheet: Params,
    pub list: Vec<Enemy>,
    ids: HashMap<u64, u32>,
    next_id: u32,
    memory: HashMap<u32, Memory>,
    pub frame: Frame,
    /// Sora's step in progress and where he is (the engaged rule).
    pub step: u64,
    pub sora: Vec3,
    /// (IMPL_ER14) Sora stands on a floor as of his latest step (the terrain allowance of the attack choice).
    pub sora_on_floor: bool,
    pub player_handle: Option<FieldInsHandle>,
    /// `tgt.bystanders`: non-hostile characters are enumerated as bystanders.
    pub bystanders: bool,
    /// (IMPL_ER51) `hit.npcs` (default on): friendly NPCs are hit by Sora's attacks (never targeted).
    pub npcs: bool,
    /// (IMPL_ER54) `hit.disguised`: character ids the game hides inside a disguise asset (default `DISGUISED`).
    pub disguised_ids: Vec<i32>,
    /// (IMPL_ER23A) `tgt.hover`: bodies that hover above their position node get their capsule lifted to the body.
    pub hover: bool,
    /// (IMPL_ER57) `target.visible_only` (default on): a character the game does not draw (its render switch off, or its
    /// draw group not shown: `target.drawgroup`) is no target at all (counted as inactive: not lockable, not soft-targeted,
    /// not hit, no push body, not engaged).
    pub visible_only: bool,
    pub drawgroups: bool,
    /// Kills since the last look (for the log / EXP), and hits delivered.
    pub kills: Vec<(TargetId, i32, Vec3)>,
    /// (IMPL_ER22) DEATHS THE GAME CONFIRMED of bodies Sora hit (the same moments as `kills`): (target, frames
    /// since his last hit on it). finish.rs decides whether one is a boss finish.
    pub deaths: Vec<(TargetId, u64)>,
    /// KH3's enemy-side reactions, one per enemy (puppet.rs over crates/kh-react).
    pub puppets: crate::puppet::Puppets,
    /// The filter word of Sora's world casts (the reactions move against the same map).
    pub cast_filter: u32,
    /// Bodies that came to lie in the last puppet step.
    landed_now: Vec<(TargetId, u8)>,
    pub hits: u32,
    /// The damage route (damage.rs).
    pub route: crate::damage::Route,
    /// The bullet route's borrowed rows (bullet.rs) and the bullet's shape.
    pub bullets: crate::bullet::Bullets,
    pub shape: crate::bullet::Shape,
    pending: Vec<Pending>,
    aim: HashMap<i32, AimMemo>,
    /// THE HIT EVENTS OF THIS FRAME (see `HitEvent`).
    pub events: Vec<HitEvent>,
    frame_no: u64,
    /// Counters for the once-a-second log line.
    pub landed: u32,
    pub missed: u32,
    /// `blt.diag`: every bullet's spawn point and fate is logged.
    pub diag: bool,
    /// Bullets that landed / took nothing, by character id (the fallback rate; `dbg.stats`).
    pub stats: HashMap<i32, (u32, u32)>,
    /// Bullets sent again after a miss.
    pub retried: u32,
    /// ONE BULLET A FRAME (MEASURED: of three bullets spawned in one frame at three bodies none
    /// took anything, and neither did their three repeats spawned together; single bullets land):
    /// bullets that have to wait their turn, and the frame the last one went out in.
    waiting: std::collections::VecDeque<Pending>,
    fired_frame: u64,
    /// (IMPL_ER32) Elden Ring's breakable map objects as things Sora's attacks hit (breakables.rs).
    pub props: crate::breakables::Props,
    /// (IMPL_ER45) The poise numbers of each kind of character (NpcParam), read and logged at its first sight (poise.rs).
    pub poise_book: crate::poise::Book,
    /// (BM) `target.passive` (default on; 0 = every hostile engages by range, as before): passive wildlife (`Enemy::passive`)
    /// starts no battle on its own.
    pub passive_rule: bool,
    /// (BM) NpcParam row -> its `threatLv` (`None`: not readable), read once per row (and told once in the log).
    threat_book: HashMap<i32, Option<u32>>,
}

/// 5 s at 60 Hz.
const ENGAGE_AFTER_HIT_STEPS: u64 = 300;

/// (BM, the user: "combat animations shouldn't start when only animals are around you") THE ENGAGED RULE FOR ONE LIVING,
/// ACTIVE HOSTILE (`ErTargets::engaged_count`: KH3's battle mode for Sora, kh-core pawn_tick; the same count round a friend
/// leader drives the party's battle flag, kh-partyhost `Leadership::battle_step`, and so the friends' battle state):
/// it fights the party (`fought`: it hit a member or was hit by one in the last 5 s, wherever it stands), or it stands
/// within `target.engage_range` and is either a threat (not `passive`: its kind's NpcParam threatLv is not 0) or a
/// passive kind seen attacking in the last 5 s (`attacking`: a boar or a rat that charges). A passive kind that only
/// stands, grazes or flees nearby engages nobody.
pub fn engages(within: bool, passive: bool, fought: bool, attacking: bool) -> bool {
    fought || (within && (!passive || attacking))
}

impl ErTargets {
    pub fn new(frame: Frame, params: Params) -> ErTargets {
        ErTargets { lock_gate_live: false, zwhole_plan: HashMap::new(), zwhole: Vec::new(), zwhole_late: Vec::new(), zkeep: Vec::new(), zdeaths: Vec::new(), zetta_kills: Vec::new(), zpose: None, zlevel: None, zdefer: Default::default(), zowner: Default::default(), zowned: false /* (ZETTA 2026-10-23) */, params, sheet: params, list: Vec::new(), ids: HashMap::new(), next_id: 1, memory: HashMap::new(), frame, step: 0, sora: Vec3::ZERO, sora_on_floor: false, player_handle: None, hover: true, visible_only: true, drawgroups: true, bystanders: true, npcs: true, disguised_ids: DISGUISED.to_vec(), kills: Vec::new(), deaths: Vec::new(), puppets: crate::puppet::Puppets::default(), cast_filter: crate::havok::FILTER_SORA, landed_now: Vec::new(), hits: 0, route: crate::damage::Route::default(), bullets: crate::bullet::Bullets::default(), shape: crate::bullet::Shape::default(), pending: Vec::new(), aim: HashMap::new(), events: Vec::new(), frame_no: 0, landed: 0, missed: 0, diag: false, stats: HashMap::new(), retried: 0, waiting: std::collections::VecDeque::new(), fired_frame: 0, props: crate::breakables::Props::default(), poise_book: crate::poise::Book::default(), passive_rule: true, threat_book: HashMap::new() /* (BM) */ }
    }

    /// (IMPL_ER32) A BULLET AT A MAP OBJECT, IN ITS TURN: one bullet a frame (see `waiting`), the enemies' waiting
    /// bullets first. `None`: not this frame.
    pub fn fire_prop(&mut self, from: Vec3, dir: Vec3, attack: crate::bullet::Attack, shape: crate::bullet::Shape) -> Option<Result<u32, String>> {
        let owner = self.player_handle?;
        if self.fired_frame == self.frame_no || !self.waiting.is_empty() || !self.bullets.free_slot() || self.route.mode != crate::damage::Mode::Bullet || self.route.off {
            return None;
        }
        let result = self.bullets.fire(owner, None, from, dir, attack, shape);
        if result.is_ok() {
            self.fired_frame = self.frame_no;
        }
        Some(result)
    }

    pub fn get(&self, id: TargetId) -> Option<&Enemy> {
        self.list.iter().find(|e| e.id == id)
    }

    pub fn id_of(&self, handle: &FieldInsHandle) -> Option<TargetId> {
        self.ids.get(&game::handle_bits(handle)).map(|n| TargetId(*n))
    }

    /// The character object of a target of THIS frame (the pointer is not kept across frames).
    pub fn chr_mut(&self, id: TargetId) -> Option<&'static mut ChrIns> {
        let e = self.get(id)?;
        // SAFETY: read from the character sets in this frame and found complete; game thread.
        let chr = unsafe { &mut *(e.chr as *mut ChrIns) };
        game::chr_complete(chr).then_some(chr)
    }

    /// (FX5) The engaged rule's memory alone: Sora / a friend hit target `id`, or it hit Sora / a friend's body, within
    /// the last `ENGAGE_AFTER_HIT_STEPS` steps (the party AI's "fighting the party", kh-partyhost `threatens`).
    pub fn fought_lately(&self, id: TargetId) -> bool {
        let recent = |at: u64| at > 0 && self.step.saturating_sub(at) <= ENGAGE_AFTER_HIT_STEPS;
        self.memory.get(&id.0).is_some_and(|m| recent(m.hit_step) || recent(m.attacked_step))
    }

    /// (BM) Target `id` was seen in an attack animation within the last `ENGAGE_AFTER_HIT_STEPS` steps.
    fn attacked_lately(&self, id: TargetId) -> bool {
        self.memory.get(&id.0).is_some_and(|m| m.attack_seen_step > 0 && self.step.saturating_sub(m.attack_seen_step) <= ENGAGE_AFTER_HIT_STEPS)
    }

    /// (BM) PASSIVE WILDLIFE AS IT STANDS NOW: a passive kind (`Enemy::passive`) that is not fighting the party (no hit
    /// either way, no attack animation seen, in the last 5 s). It starts no battle (`engages`) and the friends' trees do
    /// not count it as an enemy (party/aiworld.rs `ai_actors`); Sora can still lock on it and hit it, and the hit makes it
    /// count.
    pub fn passive_now(&self, e: &Enemy) -> bool {
        e.passive && !self.fought_lately(e.id) && !self.attacked_lately(e.id)
    }

    /// (BM) Remembers which characters play an attack animation this frame (called once a frame after the read).
    pub(crate) fn note_attacks_seen(&mut self) {
        let step = self.step.max(1);
        for e in self.list.iter().filter(|e| e.in_attack && e.alive) {
            self.memory.entry(e.id.0).or_default().attack_seen_step = step;
        }
    }

    /// (BM) The game's threat level of an NpcParam row (read once per row, told once in the log).
    fn threat_of(&mut self, npc_id: i32, npc_param: i32, team: u8, hostile: bool) -> Option<u32> {
        if let Some(v) = self.threat_book.get(&npc_param) {
            return *v;
        }
        // SAFETY: game thread; the parameter repository lives as long as the game (blood.rs reads it the same way).
        let v = unsafe { eldenring::cs::SoloParamRepository::instance() }.ok().and_then(|repo| repo.get::<eldenring::cs::NpcParam>(npc_param as u32).map(|r| r.threat_lv()));
        self.threat_book.insert(npc_param, v);
        if hostile {
            erlog!(
                "threat: c{npc_id:04} (NpcParam {npc_param}, team {team}) threatLv {}: {}",
                v.map_or("not readable".to_string(), |t| t.to_string()),
                match v {
                    Some(0) if self.passive_rule => "PASSIVE (the game's threat level 0, e.g. wildlife): it starts no battle mode (Sora's, a friend leader's, the friends') unless it attacks, hits the party or is hit by it (target.passive = 0: as before)",
                    Some(0) => "threat level 0, but target.passive = 0: it engages by range as before",
                    Some(_) => "a threat: it engages within target.engage_range as before",
                    None => "the row is not readable: it engages by range as before",
                }
            );
        }
        v
    }

    /// (BM) For the log: who engages the party now, and the passive ones within the engage range left out.
    pub fn engaged_report(&self) -> String {
        let (mut on, mut off) = (Vec::new(), Vec::new());
        for e in self.list.iter().filter(|e| e.hostile && e.alive && e.active) {
            let d = (e.pos - self.sora).length();
            let within = d <= self.params.engage_range;
            if engages(within, e.passive, self.fought_lately(e.id), self.attacked_lately(e.id)) {
                on.push(format!("c{:04} {:.1} m{}", e.npc_id, d / CM_PER_M, if e.passive { " (passive kind, fighting)" } else { "" }));
            } else if within && e.passive {
                off.push(format!("c{:04} {:.1} m", e.npc_id, d / CM_PER_M));
            }
        }
        format!("engaged: {}; passive left out: {}", if on.is_empty() { "none (the 1 s linger or the forced flag)".to_string() } else { on.join(", ") }, if off.is_empty() { "none".to_string() } else { off.join(", ") })
    }

    /// (BM) The party hit target `id` in the step in progress: the same memory write as `apply_hit` makes on a hostile
    /// (tests and the offline harnesses; the game path writes it inside `apply_hit`).
    pub fn note_hit(&mut self, id: TargetId) {
        self.memory.entry(id.0).or_default().hit_step = self.step.max(1);
    }

    /// An enemy hit Sora in the step in progress (the engaged rule).
    pub fn note_attack(&mut self, id: TargetId) {
        self.memory.entry(id.0).or_default().attacked_step = self.step.max(1);
    }

    /// The physics origin moved: remembered positions are of the old origin.
    pub fn rebased(&mut self) {
        for m in self.memory.values_mut() {
            m.er_pos = None;
        }
        self.props.rebased(); // (IMPL_ER32)
    }

    /// Reads the characters within `params.range` of Sora (`feet_er`). `dt`: the frame's time.
    pub fn refresh(&mut self, player: &PlayerIns, frame: Frame, feet_er: Vec3, dt: f32) {
        self.frame = frame;
        self.frame_no += 1;
        self.list.clear();
        self.player_handle = Some(player.chr_ins.field_ins_handle);
        // SAFETY: game thread (a task phase); the singleton lives as long as the game.
        let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
        let player_ptr = &player.chr_ins as *const ChrIns as usize;
        let can_target = can_target_fn();
        let range_m = self.params.range / CM_PER_M;
        // (IMPL_ER57) the draw-group bit is trusted only while the player's own is set (the player is always in his own groups)
        let drawgroups = self.drawgroups && player.chr_ins.chr_flags1c4.is_render_group_enabled();
        let mut seen: Vec<usize> = Vec::with_capacity(256);
        let mut summons: Vec<usize> = Vec::new();
        // SAFETY: every pointer is checked before it is followed.
        unsafe {
            each_chr_of(&wcm.summon_buddy_chr_set, &mut |p| summons.push(p));
            let mut visit = |p: usize| {
                if p == player_ptr || seen.contains(&p) || seen.len() >= 4096 {
                    return;
                }
                seen.push(p);
            };
            for k in 0..wcm.chr_sets.len() {
                let w = word!(wcm.chr_sets[k]);
                if plausible(w) {
                    each_chr_of(&*(w as *const ChrSet<ChrIns>), &mut visit);
                }
            }
            each_chr_of(&wcm.open_field_chr_set.base, &mut visit);
            each_chr_of(&wcm.summon_buddy_chr_set, &mut visit);
        }
        for p in seen {
            #[cfg(feature = "party")]
            if crate::allybody::is_body(p) { continue; } // (ER party S3) the AI allies' invisible bodies are never Sora's targets
            // SAFETY: a pointer of a character-set entry, checked plausible; completeness is checked next.
            let chr = unsafe { &mut *(p as *mut ChrIns) };
            if !game::chr_complete(chr) {
                continue;
            }
            let physics = &chr.modules.physics;
            let er_pos = game::v3(&physics.position);
            if !er_pos.is_finite() || (er_pos - feet_er).length() > range_m {
                continue;
            }
            let data = &chr.modules.data;
            let key = game::handle_bits(&chr.field_ins_handle);
            let id = *self.ids.entry(key).or_insert_with(|| {
                let n = self.next_id;
                self.next_id += 1;
                n
            });
            let (radius, height) = (physics.hit_radius.max(0.05) * CM_PER_M, physics.hit_height.max(0.1) * CM_PER_M);
            let alive = data.hp > 0 && !chr.chr_flags1c5.death_flag();
            let summon = summons.contains(&p) || chr.npc_id == 8000;
            let raw = match can_target {
                // SAFETY: the game's own function on two live characters (erfps2 calls it the same way).
                Some(f) => unsafe {
                    let f: unsafe extern "C" fn(*const ChrIns, *const ChrIns) -> u32 = std::mem::transmute(f);
                    f(&player.chr_ins, chr)
                },
                None => u32::from(crate::ermap::hostile_team(chr.team_type)),
            };
            let active = chr.chr_flags1c8.is_active() && !chr.debug_flags.character_disabled();
            // (IMPL_ER57, the user: "Sora can target invisible/unspawned enemies") A CHARACTER THE GAME DOES NOT DRAW IS NO
            // TARGET: counted as inactive (guards.rs `drawn`). Graces (c1000, invisible helpers) are read by the save points
            // from the list whatever this says.
            let shown = crate::guards::drawn(chr.chr_flags1c5.enable_render(), chr.chr_flags1c4.is_render_group_enabled(), drawgroups);
            // (a character the game hides inside a disguise asset, IMPL_ER54: Boc in his bush, is hit across it as before)
            let active = active && (shown || !self.visible_only || self.disguised_ids.contains(&chr.npc_id));
            // (IMPL_ER51) hostile / friendly NPC / bystander
            let npc_type = chr.chr_type as i32 == 5; // ChrType::Npc
            let facts = Facts { can_target: raw & 0xFF != 0, summon, npc_type, helper: chr.npc_id == 1000 || chr.team_type == 0, hit_off: chr.chr_ctrl.flags.disable_hit(), disguise_kind: self.disguised_ids.contains(&chr.npc_id) };
            let class = classify(&facts, self.npcs);
            let npcs_on = self.npcs;
            let memory = self.memory.entry(id).or_default();
            if memory.class != Some(class) || (class == Class::Friendly && memory.team != chr.team_type) {
                let was = memory.class;
                let d = (er_pos - feet_er).length();
                if ((class != Class::Hostile && !facts.helper) || was == Some(Class::Friendly)) && d < 40.0 {
                    let why = if class == Class::Friendly && facts.hit_off {
                        "IN DISGUISE (its own hit body is off; the game reveals it when the player damages the disguise asset standing on it, AEG801_480): Sora's attacks that overlap it send the player bullet ACROSS the disguise (stopped by the map, like an arrow); nothing of his targets it".to_string()
                    } else if class == Class::Friendly {
                        "a FRIENDLY NPC: Sora's attacks hit it where they overlap it, nothing of his targets it".to_string()
                    } else if class == Class::Hostile {
                        format!("now HOSTILE by the game's word{}: an enemy (lockable, KH3 reactions, Sora's rewards)", if was == Some(Class::Friendly) { format!(" after {} hit(s) of Sora's while it was friendly", memory.friendly_hits) } else { String::new() })
                    } else {
                        format!("a bystander ({})", if facts.summon { "summon set / Torrent" } else if !npc_type { "not of character type NPC: a player, phantom or ghost" } else if facts.hit_off { "the game has its hit body switched off" } else if !npcs_on { "hit.npcs = 0" } else { "not hittable" })
                    };
                    erlog!(
                        "npc: target {id} c{:04} (NpcParam {}, team {} -> {}, chr type {}, hp {}/{}, r {:.0} h {:.0} cm, {:.1} m, animation {}, alive {alive} active {active}, rendered {}, invincible flag {}, no-damage debug flag {}): {why}",
                        chr.npc_id, chr.npc_param_id, memory.team, chr.team_type, chr.chr_type as i32, data.hp, data.max_hp, radius, height, d, crate::chranim::now(chr).id,
                        chr.chr_flags1c5.enable_render(), chr.chr_flags1c5.is_invincible(), chr.debug_flags.disabled_hit()
                    );
                    // (remembered once told: a character first seen far away is told when it comes within 40 m)
                    memory.class = Some(class);
                    memory.team = chr.team_type;
                }
            }
            // (IMPL_ER57) a living hostile within 60 m the game does not draw: said at every change
            if class == Class::Hostile && alive && (er_pos - feet_er).length() < 60.0 && memory.drawn_told != Some(shown) && (memory.drawn_told.is_some() || !shown) {
                memory.drawn_told = Some(shown);
                erlog!(
                    "target: c{:04} (target {id}, NpcParam {}) {} (render switch {}, draw group bit {} [{}], active flag {}, load state {:#x}, hit body {}, animation {}, {:.1} m){}",
                    chr.npc_id, chr.npc_param_id, if shown { "IS DRAWN AGAIN" } else { "IS NOT DRAWN BY THE GAME" },
                    chr.chr_flags1c5.enable_render(), chr.chr_flags1c4.is_render_group_enabled(), if drawgroups { "used" } else { "not used: the player's own is off or target.drawgroup = 0" },
                    chr.chr_flags1c8.is_active(), unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(chr.load_state) as *const u32) }, if chr.chr_ctrl.flags.disable_hit() { "off" } else { "on" }, crate::chranim::now(chr).id, (er_pos - feet_er).length(),
                    if shown { ": a target again" } else if self.visible_only { ": NO TARGET (not lockable, not soft-targeted, not hit, no push body; target.visible_only = 0: as before)" } else { " (target.visible_only = 0: still a target)" }
                );
            }
            let velocity = match memory.er_pos {
                Some(before) if dt > 1e-4 && (er_pos - before).length() < 4.0 => coords::dir_er_to_kh(er_pos - before) / dt,
                _ => Vec3::ZERO,
            };
            memory.er_pos = Some(er_pos);
            if alive {
                memory.dead_logged = false;
            }
            let think_param = chr.as_subclass::<EnemyIns>().map_or(-1, |e| e.npc_think_param);
            let (mut poise_max, mut poise) = (0.0f32, 0.0f32);
            if plausible(word!(chr.modules.super_armor)) {
                (poise_max, poise) = (chr.modules.super_armor.sa_durability_max, chr.modules.super_armor.sa_durability);
            }
            if plausible(word!(chr.modules.toughness)) && chr.modules.toughness.toughness_max > poise_max {
                (poise_max, poise) = (chr.modules.toughness.toughness_max, chr.modules.toughness.toughness);
            }
            let weight = self.params.weight(radius, height);
            // (IMPL_ER23A) a body that hovers above its position node: the capsule goes up to it (guards.rs `hover_lift`)
            let hostile_now = raw & 0xFF != 0 && !summon && alive && active;
            let lift = if hostile_now && self.hover && (er_pos - feet_er).length() < 60.0 {
                let span = model_span(chr, er_pos);
                let lift = crate::guards::hover_lift((height * 0.5).max(radius), span, self.params.hover_min);
                if (lift > 0.0) != memory.hovering {
                    memory.hovering = lift > 0.0;
                    erlog!("target: c{:04} (target {id}) {}: its model's bones are {} above its position node, its capsule is {:.0} cm tall{}", chr.npc_id, if lift > 0.0 { "HOVERS ABOVE ITS POSITION NODE" } else { "no longer hovers above its node" }, span.map_or("not readable".to_string(), |(a, b)| format!("{a:+.0} .. {b:+.0} cm")), height, if lift > 0.0 { format!(": the capsule (marker, aim point, bullets) is taken {lift:.0} cm up; no KH3 launch for it") } else { String::new() });
                }
                lift
            } else {
                0.0
            };
            let er_pos = er_pos + Vec3::Y * (lift / CM_PER_M);
            // (IMPL_ER45) POISE: the game's hyper-armour flag, an attack animation, the refill time of its kind (the
            // kind's numbers are logged once: `poise:`)
            let hyper = if plausible(word!(chr.modules.action_flag)) { Some(chr.modules.action_flag.action_modifiers_flags.super_armor()) } else { None };
            let now_anim = crate::chranim::now(chr).id;
            // (BM) the game's threat level of its kind: threat level 0 = passive wildlife (read and told once per NpcParam row)
            let threat_lv = self.threat_of(chr.npc_id, chr.npc_param_id, chr.team_type, class == Class::Hostile);
            let passive = self.passive_rule && class == Class::Hostile && threat_lv == Some(0);
            let poise_recover = if hostile_now {
                let rules = self.puppets.poise.unwrap_or(self.params.poise);
                let sa = plausible(word!(chr.modules.super_armor)).then(|| (chr.modules.super_armor.sa_durability, chr.modules.super_armor.sa_durability_max, chr.modules.super_armor.recover_time));
                let tough = plausible(word!(chr.modules.toughness)).then(|| (chr.modules.toughness.toughness, chr.modules.toughness.toughness_max));
                let recover = self.poise_book.see(chr, poise_max, sa, tough, &rules);
                if hyper == Some(true) {
                    self.poise_book.hyper_seen(chr, now_anim, chr.modules.action_flag.sa_durability_multiplier);
                }
                recover
            } else {
                0.0
            };
            self.list.push(Enemy {
                id: TargetId(id),
                handle: chr.field_ins_handle,
                chr: p,
                er_pos,
                pos: frame.pos_er_to_kh(er_pos),
                velocity,
                yaw: coords::yaw_kh_of_er_quat(game::quat(&physics.orientation)),
                radius,
                half_height: (height * 0.5).max(radius),
                hp: data.hp,
                hp_max: data.max_hp,
                hp_game: data.hp, // (HUD2)
                team: chr.team_type,
                chr_type: chr.chr_type as i32,
                npc_id: chr.npc_id,
                npc_param: chr.npc_param_id,
                think_param,
                alive,
                hostile: class == Class::Hostile,
                friendly: class == Class::Friendly,
                disguised: class != Class::Bystander && facts.hit_off && facts.disguise_kind,
                can_target_raw: raw,
                summon,
                lock_disabled: chr.chr_ctrl.modifier.data.action_flags.disable_lock_on(),
                weight,
                poise_max,
                poise,
                hyper,
                in_attack: crate::poise::attack_anim(now_anim),
                threat_lv, // (BM)
                passive,
                poise_recover,
                puppet_ok: lift == 0.0 && self.params.puppetable(weight, poise_max, data.max_hp, chr.npc_id, chr.npc_param_id),
                airborne: false,
                grounded: lift == 0.0 && (physics.standing_on_solid_ground || physics.touching_solid_ground) && !physics.is_falling,
                lift,
                active,
                anim: crate::puppet::damage_anim(chr).map_or_else(|| crate::chranim::now(chr).id, |a| a.id),
            });
        }
        self.list.sort_by_key(|e| e.id);
        self.note_attacks_seen(); // (BM)
        self.reconcile();
        self.zetta_owes(dt); // (ZETTA 2026-10-23) zdefer.rs
        self.duplicates_no_target(); // (ZETTA 2026-10-23) the user: "Make sure it's impossible to target invisible things"
        self.lock_gate(dt); // (ZETTA 2026-10-23) only what the game's own lock-on would take
        self.send_waiting();
        // (ZFIN) Zettaflare's finale: the renderer's targets (and their neighbours) of this frame, the crumbled bodies hidden
        let rows: Vec<crate::zfinale::Row> = self.list.iter().map(|e| crate::zfinale::Row { id: e.id.0, feet: e.pos, radius: e.radius, half_height: e.half_height, drawn: e.active /* (round 7) */ }).collect();
        crate::zdraw::note_frame(self.frame); // (ZDRAW) the exact frame between the spaces for zdraw.rs (optional)
        crate::zfinale::frame(&rows);
    }

    /// What the game made of our bullets since the last frame: HP that went, deaths (the kill is
    /// Sora's when he hit it in the last seconds), bullets that took nothing (a miss: the hit is
    /// then written to the HP directly unless `dmg.fallback = 0`, so that a hit the core counted
    /// always counts).
    fn reconcile(&mut self) {
        let frame_no = self.frame_no;
        let (route, attacker) = (self.route, self.player_handle);
        for k in 0..self.list.len() {
            let e = &self.list[k];
            let (id, hp, alive) = (e.id.0, e.hp, e.alive);
            let memory = self.memory.entry(id).or_default();
            let drop = if memory.hp > 0 { memory.hp - hp.max(0) } else { 0 };
            let recent = memory.hit_frame > 0 && frame_no.saturating_sub(memory.hit_frame) <= 600;
            if drop > 0 || (!alive && memory.was_alive) {
                let asked: i32 = self.pending.iter().filter(|p| p.id == id && p.frame < frame_no).map(|p| p.wanted).sum();
                if asked > 0 {
                    let n = self.pending.iter().filter(|p| p.id == id && p.frame < frame_no).count();
                    if let Some(p) = self.pending.iter().find(|p| p.id == id && p.frame < frame_no) {
                        let memo = self.aim.entry(p.npc_id).or_default();
                        if memo.good.is_none() {
                            memo.good = Some(p.fraction);
                            if memo.tries > 0 {
                                erlog!("hit: bullets land on c{:04} at {:.2} of its capsule's height (found after {} miss(es)): remembered", p.npc_id, p.fraction, memo.tries);
                            }
                        }
                    }
                    if self.diag {
                        for p in self.pending.iter().filter(|p| p.id == id && p.frame < frame_no) {
                            erlog!("blt: LANDED on target {id} (c{:04}) after {} frame(s): from ({:.2}, {:.2}, {:.2}) along ({:.2}, {:.2}), at {:.2} of its height; body feet ({:.2}, {:.2}, {:.2}) r {:.2} h {:.2} m, animation {}{}", e.npc_id, frame_no - p.frame, p.from.x, p.from.y, p.from.z, p.dir.x, p.dir.z, p.fraction, e.er_pos.x, e.er_pos.y, e.er_pos.z, e.radius / CM_PER_M, e.half_height * 2.0 / CM_PER_M, e.anim, if e.airborne { ", our puppet in the air" } else { "" });
                        }
                    }
                    // (ZETTA 2026-10-23) forced hits land in full: what the game's defences kept is written to its HP
                    let (full, forced_asked): (i32, i32) = self.pending.iter().filter(|p| p.id == id && p.frame < frame_no && p.full > 0).fold((0, 0), |a, p| (a.0 + p.full, a.1 + p.wanted));
                    let short = forced_shortfall(full, drop, forced_asked, asked);
                    self.pending.retain(|p| !(p.id == id && p.frame < frame_no));
                    self.landed += n as u32;
                    if short > 0 && alive && route.fallback {
                        // SAFETY: `e.chr` was read from the character sets in this frame phase and found complete.
                        let outcome = unsafe { crate::damage::deal_hp(route, &mut *(e.chr as *mut ChrIns), attacker, short, false) };
                        erlog!("hit: a FORCED hit on target {id} (c{:04}) asked {full} in full; the game took {drop}: the {short} its defences kept written to its HP directly: -> {}", e.npc_id, outcome.hp_after);
                        let e = &mut self.list[k];
                        e.hp = outcome.hp_after;
                        e.hp_game = e.hp;
                        e.alive = e.hp > 0;
                        let memory = self.memory.entry(id).or_default();
                        memory.hp = e.hp;
                        continue;
                    }
                    self.stats.entry(e.npc_id).or_default().0 += n as u32;
                    let ratio = (drop as f32 / asked as f32).clamp(0.02, 5.0);
                    if alive {
                        memory.ratio = if memory.ratio > 0.0 { memory.ratio * 0.5 + ratio * 0.5 } else { ratio };
                    }
                    erlog!("hit: LANDED on target {id} (c{:04}): the game took {drop} HP for {n} bullet(s) asking {asked} (x{ratio:.2} after its defences): hp {} -> {}{}; poise {:.0}/{:.0}, animation {}", e.npc_id, memory.hp, hp, if alive { "" } else { " DEAD" }, e.poise, e.poise_max, e.anim);
                    if e.friendly || !memory.hostile_alive && memory.friendly_hits > 0 {
                        erlog!("npc: the game's reaction on friendly target {id} (c{:04} team {}): took {drop} HP, hp {} -> {}{}, animation {}, {}", e.npc_id, e.team, memory.hp, hp, if alive { "" } else { " DEAD" }, e.anim, if e.hostile { "HOSTILE now" } else { "still not hostile" });
                    }
                }
            }
            if !alive && memory.was_alive && recent && !memory.dead_logged {
                memory.dead_logged = true;
                // (IMPL_ER51) Sora's rewards (EXP, prize, loot, boss finish) only for a character seen hostile while it lived
                if memory.hostile_alive {
                    self.kills.push((e.id, e.hp_max, e.pos + Vec3::Z * e.half_height));
                    self.deaths.push((e.id, frame_no.saturating_sub(memory.hit_frame)));
                } else {
                    erlog!("npc: target {id} (c{:04} team {}) DIED while not hostile after Sora's hits: none of Sora's EXP / prize / loot / boss finish (the game's own runes and drops are the game's)", e.npc_id, e.team);
                }
            }
            if alive && e.hostile {
                memory.hostile_alive = true;
            }
            memory.hp = hp;
            memory.was_alive = alive;
        }
        // (IMPL_ER51) A friendly NPC's bullet that took no HP but made the game play another animation (a reaction, Boc's
        // reveal) landed: no retry, no HP write.
        let mut k = 0;
        while k < self.pending.len() {
            let p = self.pending[k];
            let row = self.list.iter().find(|e| e.id.0 == p.id);
            match row {
                Some(e) if p.friendly && p.frame < frame_no && friendly_landed(p.hp_before, e.hp, p.anim_before, e.anim) => {
                    erlog!("npc: the game's reaction on friendly target {} (c{:04} team {}): animation {} -> {}, hp {} -> {}, {}", p.id, e.npc_id, e.team, p.anim_before, e.anim, p.hp_before, e.hp, if e.hostile { "HOSTILE now" } else { "still not hostile" });
                    self.landed += 1;
                    self.pending.swap_remove(k);
                }
                _ => k += 1,
            }
        }
        // Bullets that took nothing.
        let mut k = 0;
        while k < self.pending.len() {
            let p = self.pending[k];
            if frame_no.saturating_sub(p.frame) < MISS_FRAMES {
                k += 1;
                continue;
            }
            self.pending.swap_remove(k);
            if let Some(e) = self.list.iter().find(|e| e.id.0 == p.id) {
                if self.diag {
                    let flat = Vec3::new(e.er_pos.x - p.from.x, 0.0, e.er_pos.z - p.from.z);
                    erlog!("blt: NOTHING from the bullet at target {} (c{:04}, team {}, {}{}): from ({:.2}, {:.2}, {:.2}) along ({:.2}, {:.2}), at {:.2} of its height; body feet NOW ({:.2}, {:.2}, {:.2}) = {:.2} m from the spawn point ({:.2} m off the line), r {:.2} h {:.2} m, animation {}, poise {:.0}/{:.0}", p.id, e.npc_id, e.team, if e.active { "active" } else { "INACTIVE" }, if e.airborne { ", our puppet in the air" } else { "" }, p.from.x, p.from.y, p.from.z, p.dir.x, p.dir.z, p.fraction, e.er_pos.x, e.er_pos.y, e.er_pos.z, flat.length(), flat.cross(p.dir).length(), e.radius / CM_PER_M, e.half_height * 2.0 / CM_PER_M, e.anim, e.poise, e.poise_max);
                }
            }
            {
                // (another height next time, unless one is known to land on this kind of body)
                let memo = self.aim.entry(p.npc_id).or_default();
                if memo.good.is_none() {
                    memo.tries = memo.tries.saturating_add(1);
                }
            }
            let Some(k_e) = self.list.iter().position(|e| e.id.0 == p.id) else { continue };
            if !self.list[k_e].alive {
                continue;
            }
            // ONCE MORE, from Sora's side at another height (MEASURED: a bullet 0.13 s after one that
            // landed took nothing, the next one 0.27 s later landed).
            // (IMPL_ER51: a friendly NPC gets one try more at most: every bullet that lands is a hit the game counts on it)
            if p.attack.is_some() && p.tries < if p.friendly { 1 } else { RETRIES } && self.bullets.available() && attacker.is_some() {
                self.retried += 1;
                self.waiting.push_back(Pending { tries: p.tries + 1, fraction: AIM_TRIES[(p.tries as usize + 1) % AIM_TRIES.len()], ..p });
                continue;
            }
            self.missed += 1;
            self.stats.entry(self.list[k_e].npc_id).or_default().1 += 1;
            let ratio = self.memory.get(&p.id).map_or(0.0, |m| m.ratio);
            let e = &mut self.list[k_e];
            if p.friendly {
                // (IMPL_ER51) never the HP write on a friendly NPC: what the game does not take, it keeps
                erlog!("npc: a bullet asking {} took nothing off friendly target {} (c{:04} team {}) after {} tries, nor changed its animation (hp {} then, {} now): the game keeps it unhurt; no HP write", p.wanted, p.id, e.npc_id, e.team, p.tries + 1, p.hp_before, e.hp);
            } else if route.fallback {
                // (the HP write takes what the game's defences would have left of the attack value:
                // the ratio measured on this target, else a third)
                let write = if p.full > 0 { p.full } else { ((p.wanted as f32 * if ratio > 0.0 { ratio } else { 0.33 }).round() as i32).max(1) };
                // SAFETY: `e.chr` was read from the character sets in this frame phase and found complete.
                let outcome = unsafe { crate::damage::deal_hp(route, &mut *(e.chr as *mut ChrIns), attacker, write, false) };
                erlog!("hit: a bullet asking {} took NOTHING off target {} (c{:04}) after {} tries (hp {} then, {} now): {write} written to its HP directly: -> {}", p.wanted, p.id, e.npc_id, p.tries + 1, p.hp_before, e.hp, outcome.hp_after);
                e.hp = outcome.hp_after;
                e.hp_game = e.hp; // (HUD2) written to the game's HP: real at once
                if let Some(m) = self.memory.get_mut(&p.id) {
                    m.hp = e.hp;
                }
            } else {
                erlog!("hit: a bullet asking {} took NOTHING off target {} (c{:04}) after {} tries (hp {} then, {} now)", p.wanted, p.id, e.npc_id, p.tries + 1, p.hp_before, e.hp);
            }
        }
    }

    /// The next bullet that waits its turn goes out (one a frame), from Sora's side at the body
    /// where it will be when the game tests it.
    fn send_waiting(&mut self) {
        if self.fired_frame == self.frame_no {
            return;
        }
        while let Some(p) = self.waiting.pop_front() {
            let (Some(attack), Some(owner)) = (p.attack, self.player_handle) else { continue };
            let Some(e) = self.list.iter().find(|e| e.id.0 == p.id && e.alive).cloned() else { continue };
            let sora_er = self.frame.pos_kh_to_er(self.sora);
            let (ahead, shape) = led(&e, self.puppets.lead(e.id, 2), self.shape);
            let (from, dir) = aim(&ahead, Vec3::new(sora_er.x, ahead.er_pos.y, sora_er.z), ahead.er_pos - sora_er, self.shape.back, Some(p.fraction));
            // (IMPL_ER54) a disguise: the bullet crosses the disguise asset from outside it
            let (from, dir, shape) = if e.disguised { disguise_path(&e, Vec3::new(sora_er.x, e.er_pos.y, sora_er.z), e.er_pos - sora_er, self.shape) } else { (from, dir, shape) };
            match self.bullets.fire(owner, Some(e.handle), from, dir, attack, shape) {
                Ok(_) => {
                    self.fired_frame = self.frame_no;
                    if self.diag {
                        erlog!("blt: bullet for target {} (c{:04}) sent in its turn (try {}) at {:.2} of its height; {} more wait", p.id, e.npc_id, p.tries + 1, p.fraction, self.waiting.len());
                    }
                    self.pending.push(Pending { frame: self.frame_no, from, dir, hp_before: e.hp, anim_before: e.anim, ..p });
                    return;
                }
                Err(why) => {
                    // (no row pair free: it waits on)
                    if self.diag {
                        erlog!("blt: the waiting bullet for target {} could not go out yet ({why})", p.id);
                    }
                    self.waiting.push_front(p);
                    return;
                }
            }
        }
    }

    /// One fixed 60 Hz step of the enemies' reactions (before each of Sora's steps): bodies in a
    /// reaction move, their rows of this frame are put at the simulated places, a body that came to
    /// lie is asked for the game's knock-down.
    pub fn step_puppets(&mut self) {
        let mut landed = std::mem::take(&mut self.landed_now);
        landed.clear();
        self.puppets.step(&mut self.list, self.frame, self.cast_filter, &mut landed);
        for (id, level) in landed.iter().copied() {
            self.ask_animation(id, level);
        }
        // (the velocity memory follows the written places)
        for e in self.list.iter().filter(|e| e.airborne) {
            if let Some(m) = self.memory.get_mut(&e.id.0) {
                m.er_pos = Some(e.er_pos);
            }
        }
        self.landed_now = landed;
    }

    /// (testing aid, `dbg.launch`) A reaction of `kind` asked of a target without any hit.
    pub fn ask_reaction(&mut self, id: TargetId, kind: kh_core::enums::ETresDamageKind, direction: Vec3) -> String {
        let params = self.params;
        match self.list.iter().find(|e| e.id == id) {
            Some(e) => format!("{:?}", self.puppets.ask(&params, e, kind, direction)),
            None => "no such target".to_string(),
        }
    }

    /// A body of ours flies without a throw animation: the game is asked for one (a bullet of 1
    /// attack point carrying the `dmgLevel`; puppet.rs holds its airborne frame).
    pub fn ask_animation(&mut self, id: TargetId, level: u8) {
        self.ask_animation_with(id, level, 0.0);
    }

    /// (ZETTA 2026-10-23) As `ask_animation`, its bullet carrying `poise` damage (Zettaflare's heavy hurt: past the armour).
    pub fn ask_animation_with(&mut self, id: TargetId, level: u8, poise: f32) {
        let (Some(owner), true) = (self.player_handle, level > 0 && self.route.mode == crate::damage::Mode::Bullet && self.bullets.available()) else { return };
        let Some(e) = self.get(id) else { return };
        if !e.alive {
            return;
        }
        let sora_er = self.frame.pos_kh_to_er(self.sora);
        let memo = self.aim.get(&e.npc_id).copied().unwrap_or_default();
        let (e, shape) = led(e, self.puppets.lead(id, 2), self.shape);
        let e = &e;
        let (from, dir) = aim(e, Vec3::new(sora_er.x, e.er_pos.y + 0.3, sora_er.z), e.er_pos - sora_er, self.shape.back, memo.fraction().or(Some(0.3)));
        let attack = crate::bullet::Attack { phys: self.route.landing_damage.max(1), poise, stamina: 0, level, ..crate::bullet::Attack::PLAIN };
        match self.bullets.fire(owner, Some(e.handle), from, dir, attack, shape) {
            Ok(_) => erlog!("puppet: target {} flies without a throw animation: one asked of the game (dmgLevel {level})", id.0),
            Err(why) => erlog!("puppet: target {}: the animation bullet failed: {why}", id.0),
        }
    }

    /// (discovery, `dbg.raw = who,bulletId`) A bullet row of the game, unpatched, from Sora at a target.
    pub fn test_raw(&mut self, id: TargetId, bullet_id: u32) -> Result<String, String> {
        let owner = self.player_handle.ok_or("no player")?;
        let e = self.get(id).ok_or("no such target")?.clone();
        let centre = e.er_pos + Vec3::Y * (e.half_height / CM_PER_M);
        let from = self.frame.pos_kh_to_er(self.sora) + Vec3::Y * 1.2;
        let dir = (centre - from).normalize_or(Vec3::Z);
        self.bullets.fire_raw(owner, Some(e.handle), from + dir * 0.8, dir, bullet_id)?;
        Ok(format!("from ({:.2}, {:.2}, {:.2}) along ({:.2}, {:.2}, {:.2}), {:.1} m away; target hp {}", from.x, from.y, from.z, dir.x, dir.y, dir.z, (centre - from).length(), e.hp))
    }

    /// (testing aid, `dbg.atk`) One bullet of ours at a target, without any hit of Sora's.
    pub fn test_attack(&mut self, id: TargetId, attack: crate::bullet::Attack, from_sora: bool, shape: Option<crate::bullet::Shape>) -> Result<String, String> {
        let shape = shape.unwrap_or(self.shape);
        let owner = self.player_handle.ok_or("no player")?;
        let e = self.get(id).ok_or("no such target")?.clone();
        let centre = e.er_pos + Vec3::Y * (e.half_height / CM_PER_M);
        let sora_er = self.frame.pos_kh_to_er(self.sora);
        // (from Sora's side, or with `from_sora` false from the side behind it)
        let side = if from_sora { sora_er } else { centre * 2.0 - sora_er };
        let (from, dir) = aim(&e, Vec3::new(side.x, centre.y, side.z), centre - side, shape.back, None);
        let bullet = self.bullets.fire(owner, Some(e.handle), from, dir, attack, shape_for(&e, shape))?;
        self.pending.push(Pending { id: id.0, frame: self.frame_no, wanted: attack.phys as i32, hp_before: e.hp, fraction: 0.5, npc_id: -1, from, dir, attack: None, tries: 0, friendly: e.friendly, anim_before: e.anim, full: 0 });
        self.memory.entry(id.0).or_default().hit_frame = self.frame_no;
        Ok(format!("bullet row {bullet} from ({:.2}, {:.2}, {:.2}) along ({:.2}, {:.2}, {:.2}); target hp {} poise {:.0}/{:.0} animation {}", from.x, from.y, from.z, dir.x, dir.y, dir.z, e.hp, e.poise, e.poise_max, e.anim))
    }

    /// (IMPL_ER51) ONE HIT OF SORA'S ON A FRIENDLY NPC: the player's bullet as on an enemy (one a frame; it waits its turn
    /// like theirs), with the KH3 damage as its attack value, the sheet's poise damage of the hit kind and the stagger
    /// level the game plays for its own staggers; what follows is the game's (damage, reaction, turning hostile, Boc's
    /// reveal). No KH3 reaction, no direct HP write in any route.
    fn hit_friendly(&mut self, hit: &HitRecord, carrier: crate::bullet::Carrier, sp_attribute: Option<u8>, obj: Option<u16>) -> HitResult {
        let (params, route, attacker, step, frame_no) = (self.params, self.route, self.player_handle, self.step, self.frame_no);
        let Some(k) = self.list.iter().position(|e| e.id == hit.target) else { return HitResult::default() };
        let e = self.list[k].clone();
        let memory = self.memory.entry(e.id.0).or_default();
        memory.hit_step = step.max(1);
        if !hit.takes_hp() {
            return HitResult { accepted: true, damage_dealt: 0.0, hp_after: e.hp as f32, killed: false, guarded: true };
        }
        memory.hit_frame = frame_no;
        memory.friendly_hits += 1;
        let (nth, ratio) = (memory.friendly_hits, memory.ratio);
        let wanted = params.damage_out(hit.damage);
        // (ZETTA) a forced hit (Zettaflare): the game's own defence and negation are cancelled by the ratio this target
        // was measured to take (full damage), at most the attack value's limit
        let wanted = if hit.flags & kh_core::host::HIT_FLAG_FORCE != 0 { let r = if memory.ratio > 0.0 { memory.ratio } else { 1.0 }; ((wanted as f32 / r.max(0.05)).round() as i32).min(60000) } else { wanted };
        let kind = hit.damage_kind.value() as i32;
        let level = self.puppets.level_for(e.npc_id, crate::puppet::Role::Flinch);
        let poise = params.reaction(kind).poise * params.poise_scale;
        let contact_er = self.frame.pos_kh_to_er(hit.position);
        let attack = crate::bullet::Attack { phys: wanted.clamp(1, 60000) as u16, poise, stamina: (wanted / 2).clamp(1, 60000) as u16, level, carrier, sp_attribute, obj };
        let event = HitEvent { step, target: e.id, handle: e.handle, attack: hit.attack, attack_name: hit.attack_name, kind, position: hit.position, position_er: contact_er, direction: hit.direction, normal: hit.info.impact_normal, damage: hit.damage, er_damage: wanted, level, reaction: 0, airborne: false, guarded: false, route: "bullet" };
        let usable = route.mode == crate::damage::Mode::Bullet && !route.off && wanted > 0 && self.bullets.available();
        let Some(owner) = attacker.filter(|_| usable) else {
            erlog!("npc: Sora -> friendly target {} (c{:04} team {}) '{}': NOT delivered ({}): friendly NPCs are hit only through the game's own pipeline", e.id.0, e.npc_id, e.team, hit.attack_name, if route.off { "dmg.off" } else if route.mode != crate::damage::Mode::Bullet { "dmg.mode = hp" } else { "no bullet rows / no player" });
            return HitResult { accepted: true, damage_dealt: 0.0, hp_after: e.hp as f32, killed: false, guarded: false };
        };
        let before = e.hp;
        let memo = self.aim.get(&e.npc_id).copied().unwrap_or_default();
        let at = ((contact_er.y - e.er_pos.y) / (e.half_height * 2.0 / CM_PER_M).max(0.01)).clamp(0.15, 0.85);
        let how = if self.fired_frame == frame_no || !self.bullets.free_slot() {
            self.waiting.push_back(Pending { id: e.id.0, frame: frame_no, wanted, hp_before: before, fraction: memo.fraction().unwrap_or(at), npc_id: e.npc_id, from: Vec3::ZERO, dir: Vec3::Z, attack: Some(attack), tries: 0, friendly: true, anim_before: e.anim, full: 0 });
            "a bullet that waits its turn (one a frame)".to_string()
        } else {
            let (from, dir) = aim(&e, contact_er, coords::dir_kh_to_er(hit.direction), self.shape.back, memo.fraction());
            // (IMPL_ER54) a disguise: the bullet crosses the disguise asset from outside it, stopped by the map like an arrow
            let (from, dir, shape) = if e.disguised { disguise_path(&e, contact_er, coords::dir_kh_to_er(hit.direction), self.shape) } else { (from, dir, shape_for(&e, self.shape)) };
            let fraction = (from.y - e.er_pos.y) / (e.half_height * 2.0 / CM_PER_M).max(0.01);
            match self.bullets.fire(owner, Some(e.handle), from, dir, attack, shape) {
                Ok(row) => {
                    self.fired_frame = frame_no;
                    self.pending.push(Pending { id: e.id.0, frame: frame_no, wanted, hp_before: before, fraction, npc_id: e.npc_id, from, dir, attack: Some(attack), tries: 0, friendly: true, anim_before: e.anim, full: 0 });
                    format!("a BULLET (row {row})")
                }
                Err(why) => {
                    erlog!("npc: Sora -> friendly target {} (c{:04}): the bullet could not be made ({why}): nothing is delivered (no HP write on friendly NPCs)", e.id.0, e.npc_id);
                    return HitResult { accepted: true, damage_dealt: 0.0, hp_after: before as f32, killed: false, guarded: false };
                }
            }
        };
        self.hits += 1;
        self.events.push(event);
        let expected = ((wanted as f32 * if ratio > 0.0 { ratio } else { 1.0 }).round() as i32).max(1);
        let after = (before - expected).max(1);
        self.list[k].hp = after;
        erlog!(
            "npc: Sora -> friendly target {} (c{:04} NpcParam {} team {}, hit no. {nth} while friendly) '{}' kind {:?} {:.1} KH = attack {wanted} by {how}, dmgLevel {level}, poise damage {poise:.0}: hp {before}, animation {}; the game decides the rest",
            e.id.0, e.npc_id, e.npc_param, e.team, hit.attack_name, hit.damage_kind, hit.damage, e.anim
        );
        HitResult { accepted: true, damage_dealt: params.damage_out_inverse(expected.min(before)), hp_after: after as f32, killed: false, guarded: false }
    }

    /// (IMPL_ER14) How far above its feet the point is that the core aims at (`info_of`).
    pub fn aim_height(&self, e: &Enemy) -> f32 {
        crate::guards::aim_height(e.half_height, self.params.point_max_height, e.lift)
    }

    fn info_of(&self, e: &Enemy) -> Option<TargetInfo> {
        // (IMPL_ER51) hostile: an enemy; friendly NPC: hittable, never lockable; else a body in the way, nothing more
        let class = if e.hostile { Class::Hostile } else if e.friendly { Class::Friendly } else { Class::Bystander };
        if class != Class::Hostile && ((class == Class::Bystander && !self.bystanders) || (e.pos - self.sora).length() > 1000.0) {
            return None;
        }
        // (SP2) the invisible helpers (a site of grace's own character, c1000) are no body at all for the core
        if !offered_as_body(class, e.npc_id) {
            return None;
        }
        let flags = flags_of(class, e.alive, e.active, e.lock_disabled)?;
        // (IMPL_ER13, the user: "Sora automatically goes into Aerials more often than not") THE POINT THE CORE AIMS AT: KH3's attack
        // choice compares the target's lock point with Sora's height (`dz > 160` = "the target is high": the rising attack,
        // and air combos from there); a KH3 enemy's lock point is about 90 cm above its feet, an Elden Ring capsule's
        // centre is half its height up (a troll's: metres). The point is the capsule's centre, but at most
        // `target.point.max_height` above the feet (the HUD's cursor and the lock-on camera keep the true centre).
        Some(TargetInfo { id: e.id, position: e.pos, center: e.pos + Vec3::Z * self.aim_height(e), velocity: e.velocity, yaw: e.yaw, radius: e.radius, half_height: e.half_height, flags: TargetFlags(flags), hp: e.hp as f32, hp_max: e.hp_max as f32 })
    }

    /// The living hostile characters nearest to Sora first: (distance cm, row).
    pub fn hostiles_by_distance(&self) -> Vec<(f32, &Enemy)> {
        let mut v: Vec<(f32, &Enemy)> = self.list.iter().filter(|e| e.hostile && e.alive && e.active).map(|e| ((e.pos - self.sora).length(), e)).collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v
    }

    /// The bullet route's score by character id (`dbg.stats`, and when Sora stops).
    pub fn stats_line(&self) -> String {
        let mut rows: Vec<(&i32, &(u32, u32))> = self.stats.iter().collect();
        rows.sort();
        let (landed, missed) = rows.iter().fold((0, 0), |a, (_, s)| (a.0 + s.0, a.1 + s.1));
        format!("{landed} hit(s) landed by bullet, {missed} fell back to the HP write ({:.1} %), {} bullet(s) sent again: {}", if landed + missed > 0 { missed as f32 * 100.0 / (landed + missed) as f32 } else { 0.0 }, self.retried, rows.iter().map(|(id, s)| format!("c{:04} {}/{}", id, s.0, s.0 + s.1)).collect::<Vec<_>>().join(", "))
    }

    /// One line per character read (the `dbg.chrs` dump).
    pub fn dump(&self) {
        erlog!("chrs: {} character(s) within {:.0} m of Sora (id | chr / npc param / think | team type | hp | size | distance | flags)", self.list.len(), self.params.range / CM_PER_M);
        let mut rows: Vec<&Enemy> = self.list.iter().collect();
        rows.sort_by(|a, b| (a.pos - self.sora).length().total_cmp(&(b.pos - self.sora).length()));
        for e in rows.iter().take(60) {
            erlog!(
                "chrs:   {} | c{:04} / {} / {} | team {} type {} | hp {}/{} | r {:.0} h {:.0} cm {:?} | {:.1} m | anim {} | can_target {:#x}{}{}{}{}{}",
                e.id.0, e.npc_id, e.npc_param, e.think_param, e.team, e.chr_type, e.hp, e.hp_max, e.radius, e.half_height * 2.0, e.weight, (e.pos - self.sora).length() / CM_PER_M, e.anim, e.can_target_raw,
                if e.hostile { " HOSTILE" } else { "" }, if e.friendly { " FRIENDLY-NPC (hittable)" } else { "" }, if e.summon { " summon-set" } else { "" }, if !e.alive { " dead" } else { "" }, if !e.active { " inactive" } else { "" },
            );
        }
        // (IMPL_ER14) the census: every kind of character in range, with its nearest living one
        let mut kinds: std::collections::BTreeMap<i32, (u32, f32, i32, f32, bool)> = std::collections::BTreeMap::new();
        for e in rows.iter().filter(|e| e.alive) {
            let d = (e.pos - self.sora).length() / CM_PER_M;
            let k = kinds.entry(e.npc_id).or_insert((0, f32::MAX, e.hp_max, e.half_height * 2.0, e.hostile));
            k.0 += 1;
            k.1 = k.1.min(d);
        }
        erlog!("chrs: census (living): {}", kinds.iter().map(|(id, k)| format!("c{id:04} x{} nearest {:.0} m (hp {}, h {:.0} cm{})", k.0, k.1, k.2, k.3, if k.4 { "" } else { ", not hostile" })).collect::<Vec<_>>().join(" | "));
    }
}

/// The bullet's shape for this body: it must live long enough to cross the whole capsule (MEASURED:
/// a bullet that starts INSIDE a body takes nothing; one from the capsule's edge that dies at the
/// axis missed a giant crab, whose own hit shapes are not on its axis).
fn shape_for(e: &Enemy, shape: crate::bullet::Shape) -> crate::bullet::Shape {
    let across = 2.0 * (e.radius / CM_PER_M + shape.back);
    crate::bullet::Shape { life: shape.life.max(across / shape.speed.max(1.0)), ..shape }
}

/// (IMPL_ER54) How far from a disguised character's axis the bullet starts (m): outside the disguise asset (a bush) as
/// well as the capsule (MEASURED for bodies: a bullet that starts inside takes nothing).
const DISGUISE_STANDOFF: f32 = 1.2;

/// (IMPL_ER54) THE BULLET AT A DISGUISE: level at mid-height from `DISGUISE_STANDOFF` outside the axis on the contact's
/// side, across the whole disguise, NOT passing through the map (the variant that hits map objects: breakables.rs, the
/// objects that broke were broken by it); the game counts it as the player damaging the disguise asset.
fn disguise_path(e: &Enemy, contact_er: Vec3, fallback: Vec3, shape: crate::bullet::Shape) -> (Vec3, Vec3, crate::bullet::Shape) {
    let wide = Enemy { radius: e.radius.max(DISGUISE_STANDOFF * CM_PER_M), ..e.clone() };
    let (from, dir) = aim(&wide, contact_er, fallback, shape.back, Some(0.5));
    let shape = shape_for(&wide, shape);
    (from, dir, crate::bullet::Shape { penetrate: false, ..shape })
}

/// The row of a body as it will be when the game tests our bullet against it (two fixed steps
/// ahead of a body in a KH3 reaction), and the bullet's shape widened by half that lead.
fn led(e: &Enemy, lead_kh: Vec3, shape: crate::bullet::Shape) -> (Enemy, crate::bullet::Shape) {
    let lead = coords::dir_kh_to_er(lead_kh) / CM_PER_M;
    let mut ahead = e.clone();
    ahead.er_pos += lead;
    let shape = shape_for(e, shape);
    (ahead, crate::bullet::Shape { radius: (shape.radius + lead.length() * 0.5).min(0.45), ..shape })
}

/// WHERE A BULLET FOR A HIT STARTS AND WHICH WAY IT FLIES (Elden Ring space). MEASURED: the game
/// flies a bullet of our rows HORIZONTALLY whatever the direction given (the pitch is dropped), so
/// it starts at the height of the contact point (kept inside the body's capsule), just outside
/// the capsule on the side the contact is on, and flies level at the body's axis.
/// `fallback`: the blow's direction, used when the contact point is on the axis.
/// THE HEIGHT IS LEARNT PER KIND OF BODY (`AimMemo`): the game tests the bullet against the body's
/// own hit shapes, not its capsule (MEASURED: level bullets at the contact height took nothing off
/// the four-legged c4440, hit after hit, while they landed on soldiers, boars and a crab).
fn aim(e: &Enemy, contact_er: Vec3, fallback: Vec3, back: f32, at_fraction: Option<f32>) -> (Vec3, Vec3) {
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let height = e.half_height * 2.0 / CM_PER_M;
    let (low, high) = (e.er_pos.y + (0.2f32).min(height * 0.5), e.er_pos.y + (height - 0.2).max(height * 0.5));
    // (a height that was learnt for this kind of body overrules the contact's)
    let y = match at_fraction {
        Some(f) => e.er_pos.y + height * f,
        None => contact_er.y.clamp(low, high),
    };
    let axis = Vec3::new(e.er_pos.x, y, e.er_pos.z);
    let dir = flat(axis - contact_er).normalize_or(flat(fallback).normalize_or(Vec3::Z));
    (axis - dir * (e.radius / CM_PER_M + back), dir)
}

/// (IMPL_ER23A) WHERE THE MODEL'S BONES ARE ABOVE THE POSITION NODE, cm: (the lower tenth, the upper tenth of their heights).
/// The layout is the one figure.rs reads the player's own model with (MEASURED there, IMPL_ER16): model item = [model +
/// 0x10]; [item + 0x68] = the FLVER data, node count at + 0x18; [[item + 0x640] + 0x70] = one 3 x 4 world matrix per node,
/// 0x30 bytes, the translation in the 4th column. Tenths, not the extremes: models carry helper nodes at their origin.
fn model_span(chr: &ChrIns, er_pos: Vec3) -> Option<(f32, f32)> {
    let item = crate::probe::word(word!(chr.chr_model_ins) + 0x10).filter(|w| plausible(*w))?;
    let flver = crate::probe::word(item + 0x68).filter(|w| plausible(*w))?;
    let n = crate::probe::u32_at(flver + 0x18)? as usize;
    if !(8..=2048).contains(&n) {
        return None;
    }
    let pose = crate::probe::word(crate::probe::word(item + 0x640).filter(|w| plausible(*w))? + 0x70).filter(|w| plausible(*w))?;
    let mut bytes = vec![0u8; n * 0x30];
    if !crate::probe::read(pose, &mut bytes) {
        return None;
    }
    let f = |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let mut heights: Vec<f32> = (0..n).map(|k| Vec3::new(f(k * 0x30 + 12), f(k * 0x30 + 28), f(k * 0x30 + 44))).filter(|p| p.is_finite() && (*p - er_pos).length() < 40.0).map(|p| (p.y - er_pos.y) * CM_PER_M).collect();
    if heights.len() < 8 {
        return None;
    }
    heights.sort_by(f32::total_cmp);
    Some((heights[heights.len() / 10], heights[heights.len() * 9 / 10]))
}

/// The live, complete character of a handle, found by going through the game's character sets
/// (as `refresh` does). Game thread.
pub fn find_chr(handle: &FieldInsHandle) -> Option<&'static mut ChrIns> {
    // SAFETY: game thread; every pointer is checked before it is followed.
    unsafe {
        let wcm = WorldChrMan::instance_mut().ok()?;
        let wanted = game::handle_bits(handle);
        let mut found = 0usize;
        let mut look = |p: usize| {
            // (the handle is the second word of a character object)
            if found == 0 && std::ptr::read_unaligned((p + 8) as *const u64) == wanted {
                found = p;
            }
        };
        each_chr_of(&wcm.open_field_chr_set.base, &mut look);
        for k in 0..wcm.chr_sets.len() {
            let w = word!(wcm.chr_sets[k]);
            if plausible(w) {
                each_chr_of(&*(w as *const ChrSet<ChrIns>), &mut look);
            }
        }
        if found == 0 {
            return None;
        }
        let chr = &mut *(found as *mut ChrIns);
        (chr.field_ins_handle == *handle && game::chr_complete(chr)).then_some(chr)
    }
}

/// Calls `f` with the pointer word of every character of a set.
///
/// # Safety
/// `set` must be a character set of the live `WorldChrMan`; game thread.
pub(crate) unsafe fn each_chr_of<T>(set: &ChrSet<T>, f: &mut dyn FnMut(usize))
where
    T: fromsoftware_shared::Subclass<ChrIns> + 'static,
{
    let entries = word!(set.entries);
    let capacity = set.capacity as usize;
    if !plausible(entries) || capacity == 0 || capacity > 8192 {
        return;
    }
    for k in 0..capacity {
        // SAFETY: inside the set's entry array.
        let entry = unsafe { (entries as *const ChrSetEntry<T>).add(k) };
        let p = unsafe { std::ptr::read_unaligned(entry as *const usize) };
        if plausible(p) {
            f(p);
        }
    }
}

impl ErTargets {
    /// (ZETTA 2026-10-23) The game's own route of a hit (the trait's `apply_hit` before Zettaflare's owned hits).
    fn apply_hit_game(&mut self, hit: &HitRecord) -> HitResult {
        // (IMPL_ER61) a shotlock's hit: scaled and capped per target per shotlock (ermap.rs `shotlock_hit`)
        let max_hp = self.list.iter().find(|e| e.id == hit.target).map_or(0, |e| e.hp_max);
        let (adjusted, note) = crate::ermap::shotlock_hit(&self.params, hit, max_hp, self.step);
        // (DON-MAGIC) EXPOSED (documentation/DONALD_OVERHAUL.md 4.5): x1.25 on everyone's hits; kh-party's own carry it (flag)
        let mut adjusted = adjusted;
        adjusted.damage *= crate::donstatus::exposed_mult(adjusted.target, adjusted.flags);
        if let Some(note) = note {
            erlog!("hit: {note}");
        }
        let hit = &adjusted;
        // (IMPL_ER32) a breakable map object: a bullet with object damage is queued for it (breakables.rs)
        if crate::breakables::is_prop(hit.target) {
            let wanted = self.params.damage_out(hit.damage);
            return self.props.hit(hit, wanted);
        }
        // (IMPL_ER32) the element of the hit (fire -> fire ...) and the object attack value go into the attack row
        let (carrier, sp_attribute) = self.props.element_of(hit.attribute as i32 as u8);
        let obj = self.props.object_attack();
        let step = self.step;
        let attacker = self.player_handle;
        let (params, route) = (self.params, self.route);
        // ---- KH3's side of the hit: what the body does (kh-react), decided before the damage goes out
        let mut outcome = None;
        let mut airborne_before = false;
        if let Some(row) = self.list.iter().find(|e| e.id == hit.target) {
            // (IMPL_ER23A) a body that hovers above its node is not taken by the puppet (its place is the game's animation's)
            if row.hostile && row.alive && hit.takes_hp() && row.lift == 0.0 {
                let wanted = params.damage_out(hit.damage);
                let ratio = self.memory.get(&row.id.0).map_or(1.0, |m| if m.ratio > 0.0 { m.ratio } else { 1.0 });
                let lethal = route.mode == crate::damage::Mode::Hp && row.hp - wanted <= 0 || (wanted as f32 * ratio) as i32 >= row.hp + row.hp_max / 4;
                airborne_before = self.puppets.airborne(row.id);
                outcome = Some(self.puppets.hit(&params, row, hit, wanted > 0, lethal));
            }
        }
        // (IMPL_ER51) a friendly NPC: the same bullet, the game's own reaction, no KH3 puppet
        if self.list.iter().any(|e| e.id == hit.target && e.friendly && e.alive) {
            return self.hit_friendly(hit, carrier, sp_attribute, obj);
        }
        let Some(e) = self.list.iter_mut().find(|e| e.id == hit.target) else { return HitResult::default() };
        if !e.hostile || !e.alive {
            return HitResult { hp_after: e.hp.max(0) as f32, ..HitResult::default() };
        }
        let memory = self.memory.entry(e.id.0).or_default();
        memory.hit_step = step.max(1);
        if !hit.takes_hp() {
            return HitResult { accepted: true, damage_dealt: 0.0, hp_after: e.hp as f32, killed: false, guarded: true };
        }
        memory.hit_frame = self.frame_no;
        let wanted = params.damage_out(hit.damage);
        // (ZETTA 2026-10-23) a FORCED hit lands in full (the user: "Zettaflare ignores all armor, it takes priority over
        // ANYTHING"; the user's log: Malenia took 8217 of 9940): the bullet asks the number over the negation measured on this
        // target, and what the game still keeps is written to its HP directly once it has taken its share (`reconcile`)
        let forced = hit.flags & kh_core::host::HIT_FLAG_FORCE != 0;
        let full = if forced { wanted } else { 0 };
        let wanted = if forced { forced_ask(wanted, memory.ratio) } else { wanted };
        let kind = hit.damage_kind.value() as i32;
        // THE ELDEN RING ANIMATION: the damage level of the reaction KH3 started (a STRONG body may
        // have downgraded the kind or refused it: then the level of "no reaction", poise only).
        let started = match outcome {
            Some(kh_react::HitOutcome::Started { kind, .. }) => kind.value() as i32,
            _ => 0,
        };
        // (IMPL_ER6: the level is the one that plays the ROLE on this kind of character: a stagger for
        // a flinch, the throw animation for everything that takes the body off its feet or meets it
        // in the air; a body that refuses plays nothing)
        let role = outcome.as_ref().map_or(if e.lift > 0.0 { crate::puppet::Role::Flinch /* (IMPL_ER23A) a hovering body: the game's own stagger, no puppet */ } else { crate::puppet::Role::None }, |o| self.puppets.role_of(e.id, o, airborne_before));
        let level = self.puppets.level_for(e.npc_id, role);
        // (ZETTA 2026-10-23) Zettaflare's beam hit on its posed target: the level it was given (zpose.rs), not the role's
        let (role, level) = match self.zlevel.take() { Some(l) => (crate::puppet::Role::None, l), None => (role, level) };
        // POISE DAMAGE goes to ARMOURED bodies only (KH3's STRONG hurt shape: Elden Ring's stance
        // break is its broken window). On bodies we throw about, Elden Ring's stance break (a long
        // kneel) would play over KH3's reactions.
        // (IMPL_ER21) by the rules of bossguard.rs: none from hits without KH3 reaction power, a share of the body's
        // maximum at most, none for a while after a stance break and during its revenge
        let poise = self.puppets.poise_for(&params, e, params.reaction(kind).poise * params.poise_scale, hit.info.reaction_power);
        // (XSI, 2026-10-16) BREAK BOOST (kh-core abilities/xs, XS 2.16): its hits ask more poise damage
        let poise = if hit.flags & kh_core::abilities::xs::FLAG_BREAK_BOOST != 0 { poise * (1.0 + kh_core::abilities::xs::X.break_boost_poise) } else { poise };
        let frame = self.frame;
        let contact_er = frame.pos_kh_to_er(hit.position);
        let before = e.hp;
        let mut event = HitEvent { step, target: e.id, handle: e.handle, attack: hit.attack, attack_name: hit.attack_name, kind, position: hit.position, position_er: contact_er, direction: hit.direction, normal: hit.info.impact_normal, damage: hit.damage, er_damage: wanted, level, reaction: started, airborne: airborne_before, guarded: false, route: "hp" };

        // ---- THE GAME'S OWN PIPELINE: a bullet of the player's at the contact point
        let mut by_bullet = None;
        let mut fraction = 0.5;
        let mut spawn = (Vec3::ZERO, Vec3::Z);
        let mut sent = None;
        let mut queued = false;
        let usable = route.mode == crate::damage::Mode::Bullet && !route.off && wanted > 0 && self.bullets.available() && attacker.is_some();
        let mut wait = usable && self.fired_frame == self.frame_no;
        if usable && !wait {
            // (dry run of the slot rule: a row pair must be free, else the bullet waits its turn too)
            wait = !self.bullets.free_slot();
        }
        if wait {
            // (one bullet a frame: this one waits its turn)
            let attack = crate::bullet::Attack { phys: wanted.clamp(1, 60000) as u16, poise, stamina: (wanted / 2).clamp(1, 60000) as u16, level, carrier, sp_attribute, obj };
            let at =((contact_er.y - e.er_pos.y) / (e.half_height * 2.0 / CM_PER_M).max(0.01)).clamp(0.15, 0.85);
            self.waiting.push_back(Pending { id: e.id.0, frame: self.frame_no, wanted, hp_before: e.hp, fraction: at, npc_id: e.npc_id, from: Vec3::ZERO, dir: Vec3::Z, attack: Some(attack), tries: 0, friendly: false, anim_before: e.anim, full });
            self.puppets.asked(e.id, role, level);
            queued = true;
        } else if route.mode == crate::damage::Mode::Bullet && !route.off && wanted > 0 && self.bullets.available() {
            if let Some(owner) = attacker {
                let memo = self.aim.get(&e.npc_id).copied().unwrap_or_default();
                let lead = self.puppets.lead(e.id, 2);
                let (ahead, shape) = led(e, lead, self.shape);
                let (from, dir) = aim(&ahead, contact_er + (ahead.er_pos - e.er_pos), coords::dir_kh_to_er(hit.direction), self.shape.back, memo.fraction());
                // (IMPL_ER54) an enemy still in its disguise (a bush demi-human): the bullet crosses the disguise asset
                let (from, dir, shape) = if e.disguised { disguise_path(e, contact_er, coords::dir_kh_to_er(hit.direction), self.shape) } else { (from, dir, shape) };
                fraction = (from.y - ahead.er_pos.y) / (e.half_height * 2.0 / CM_PER_M).max(0.01);
                let attack = crate::bullet::Attack { phys: wanted.clamp(1, 60000) as u16, poise, stamina: (wanted / 2).clamp(1, 60000) as u16, level, carrier, sp_attribute, obj };
                // (ER party S3) stage 2 (U14, `party.bullet_owner`, off by default): a friend's hit fired by his own body
                #[cfg(feature = "party")]
                let fired = crate::allybody::fire_hit(&mut self.bullets, owner, hit, Some(e.handle), from, dir, attack, shape);
                #[cfg(not(feature = "party"))]
                let fired = self.bullets.fire(owner, Some(e.handle), from, dir, attack, shape);
                match fired {
                    Ok(row) => {
                        by_bullet = Some(row);
                        spawn = (from, dir);
                        sent = Some(attack);
                        self.fired_frame = self.frame_no;
                        self.puppets.asked(e.id, role, level);
                    }
                    Err(why) => erlog!("hit: the bullet for target {} could not be made ({why}): this hit is written to its HP directly", e.id.0),
                }
            }
        }
        self.hits += 1;
        if queued {
            let ratio = if memory.ratio > 0.0 { memory.ratio } else { 1.0 };
            let expected = if full > 0 { full } else { ((wanted as f32 * ratio).round() as i32).max(1) };
            e.hp = (before - expected).max(1);
            event.route = "bullet";
            self.events.push(event);
            erlog!("hit: Sora -> target {} (c{:04}) '{}' kind {:?} {:.1} KH = attack {} by a BULLET THAT WAITS ITS TURN (one a frame), dmgLevel {level}: hp {} (expected -> {})", e.id.0, e.npc_id, hit.attack_name, hit.damage_kind, hit.damage, wanted, before, e.hp);
            return HitResult { accepted: true, damage_dealt: params.damage_out_inverse(expected.min(before)), hp_after: e.hp as f32, killed: false, guarded: false };
        }
        if let Some(row) = by_bullet {
            // The game applies it after this phase: what it takes is read next frame (`reconcile`).
            // Until then the row carries the expected HP (the attack value x the ratio measured on this target).
            let ratio = if memory.ratio > 0.0 { memory.ratio } else { 1.0 };
            let expected = if full > 0 { full } else { ((wanted as f32 * ratio).round() as i32).max(1) };
            self.pending.push(Pending { id: e.id.0, frame: self.frame_no, wanted, hp_before: before, fraction, npc_id: e.npc_id, from: spawn.0, dir: spawn.1, attack: sent, tries: 0, friendly: false, anim_before: e.anim, full });
            e.hp = (before - expected).max(1);
            event.route = "bullet";
            self.events.push(event);
            erlog!(
                "hit: Sora -> target {} (c{:04} team {}) '{}' kind {:?} {:.1} KH x {} = attack {} by a BULLET (row {row}), dmgLevel {level} (KH3 reaction kind {}{}), poise damage {:.0}: hp {} (expected -> {})",
                e.id.0, e.npc_id, e.team, hit.attack_name, hit.damage_kind, hit.damage, params.out_scale, wanted, started, if airborne_before { ", airborne" } else { "" }, poise, before, e.hp
            );
            return HitResult { accepted: true, damage_dealt: params.damage_out_inverse(expected.min(before)), hp_after: e.hp as f32, killed: false, guarded: false };
        }
        // ---- THE STOPGAP: the number straight off its HP
        // SAFETY: `e.chr` was read from the character sets in this frame phase and found complete.
        let outcome = unsafe { crate::damage::deal(route, &mut *(e.chr as *mut ChrIns), attacker, if full > 0 { full } else { wanted }, hit, frame) };
        e.hp = outcome.hp_after;
        e.hp_game = e.hp; // (HUD2) written to the game's HP: real at once
        e.alive = e.hp > 0;
        memory.hp = e.hp;
        let killed = before > 0 && e.hp <= 0;
        self.events.push(event);
        erlog!(
            "hit: Sora -> target {} (c{:04} team {}) '{}' kind {:?} {:.1} KH x {} = {} ER by {}: hp {} -> {}{}",
            e.id.0, e.npc_id, e.team, hit.attack_name, hit.damage_kind, hit.damage, params.out_scale, wanted, outcome.route, before, e.hp, if killed { " KILLED" } else { "" }
        );
        if killed && !memory.dead_logged {
            memory.dead_logged = true;
            memory.was_alive = false;
            self.kills.push((e.id, e.hp_max, e.pos + Vec3::Z * e.half_height));
            self.deaths.push((e.id, 0)); // (IMPL_ER22) the direct HP write: dead by our own hand, now
        }
        HitResult { accepted: true, damage_dealt: params.damage_out_inverse(outcome.dealt), hp_after: e.hp.max(0) as f32, killed, guarded: false }
    }

    /// (ZETTA 2026-10-23, zdefer.rs) ONCE A FRAME after the reconcile: while Zettaflare owns the presentation, KH3's side of
    /// what it owes (the HP, a kill: no longer alive for KH3, its body held in place as the test level's target is, never its
    /// death); at the hand-back the owed HP is taken through the game's own pipeline (its floors and phase changes honoured:
    /// a boss that does not die is shown again for its next phase, zfinale.rs), the bodies given back.
    fn zetta_owes(&mut self, dt: f32) {
        #[cfg(feature = "party")]
        let (running, step) = kh_party::donald::zettaflare::presentation();
        #[cfg(not(feature = "party"))]
        let (running, step) = (false, 0u32);
        self.zowned = self.zowner.frame(running, step);
        self.zpose_frame(dt); // (ZETTA 2026-10-23) the heavy hurt, frozen at its middle
        crate::zboss::frame(crate::game::remo_state()); // (ZETTA 2026-10-23) a Zettaflare boss kill's cutscene skipped
        // (ZETTA 2026-10-23) the hand-back's deaths watched; Zettaflare's deaths remembered a while (the finish overruled)
        self.zetta_kills.retain_mut(|k| { k.1 += dt; k.1 < 30.0 });
        let mut watch = std::mem::take(&mut self.zdeaths);
        watch.retain_mut(|(id, key, since)| {
            *since += dt;
            let alive = self.list.iter().find(|e| e.id.0 == *id && game::handle_bits(&e.handle) == *key).is_some_and(|e| e.alive && e.hp > 0);
            match crate::zdefer::death_watch(alive, *since) {
                crate::zdefer::DeathWatch::Wait => true,
                crate::zdefer::DeathWatch::Died => {
                    erlog!("zetta: target {id}: DIED HIDDEN by the hand-back's damage: never drawn again (no death animation, no corpse); Zettaflare's death, no KH3 finish");
                    false
                }
                crate::zdefer::DeathWatch::Survived if self.zkeep.contains(id) => {
                    // (ZETTA 2026-10-24) a change the game keeps (story-critical, a surrender): shown again, the game's own
                    crate::zfinale::keep_hidden(*id, false);
                    crate::zboss::revoke(*key);
                    self.zetta_kills.retain(|k| k.0 != *id);
                    erlog!("zetta: target {id}: ALIVE after the hand-back: its change is the game's own (story-critical or a surrender): drawn again, the game plays it");
                    false
                }
                crate::zdefer::DeathWatch::Survived => {
                    // (ZETTA 2026-10-24, the user, 2026-10-24: "killing a boss with a second phase during it's first phase with Zettaflare should just kill the boss entirely skipping the second phase") the game floored it (a no-death state the hit did not see): the WHOLE KILL now
                    // (its no-death states off, its HP written 0 if the floor holds; the old route only if nothing will do)
                    let e = self.list.iter().find(|e| e.id.0 == *id && game::handle_bits(&e.handle) == *key).cloned();
                    if let Some(e) = e {
                        if let Some(chr) = crate::targets::find_chr(&e.handle) {
                            crate::zdefer::clear_no_death(chr);
                        }
                        let rows: Vec<(u32, i32, bool)> = self.list.iter().map(|o| (o.id.0, o.npc_id, o.alive && o.hp > 0)).collect();
                        let fin = crate::zdefer::final_rows(*id, e.npc_id, &rows);
                        erlog!("zetta: target {id}: ALIVE {:.1} s after the hand-back's damage (the game floored it: a phase change the hit did not see): THE WHOLE BOSS IS KILLED now (shape {:?}), its HP written past the floor", *since, crate::zdefer::shape_of(fin.len()));
                        self.zwhole_late.push((*id, *key, fin));
                    }
                    false
                }
            }
        });
        self.zdeaths = watch;
        self.zwhole_frame(dt); // (ZETTA 2026-10-24) a two-phase boss killed whole, watched to its death
        crate::zphase::frame(dt); // (ZETTA 2026-10-24) its final form and its defeat, the game's way
        if self.zowned {
            let owed: Vec<(usize, i32)> = self.list.iter().enumerate().filter_map(|(k, e)| { let o = self.zdefer.owed(e.id.0); (o > 0).then_some((k, o)) }).collect();
            for (k, o) in owed {
                let game_hp = self.list[k].hp;
                let _ = o;
                let after = self.zdefer.kh_hp_of(self.list[k].id.0, game_hp);
                let e = &mut self.list[k];
                e.hp = after;
                e.hp_game = after;
                // (2026-10-23, the user: "Melania was also attacking during Zettaflare!") every body Zettaflare owes HP to, its
                // phase rows on its spot with it, acts not at all until the hand-back: its AI off, no actions
                let (id, npc, pos, radius, handle) = (e.id, e.npc_id, e.pos, e.radius, e.handle);
                self.puppets.zetta_ai_lock(&handle);
                let twins: Vec<FieldInsHandle> = self.list.iter().filter(|o| o.id != id && o.npc_id == npc && o.alive && crate::zfinale::coincident(o.pos, o.radius, pos, radius)).map(|o| o.handle).collect();
                for t in &twins {
                    self.puppets.zetta_ai_lock(t);
                }
                let e = &mut self.list[k];
                if after == 0 && game_hp > 0 {
                    // killed on KH3's side, alive in the game: KEPT where it stands, no movement, no actions, AI off (frozen
                    // outright once the crumble hides it), in its hurt while it is seen, as the test level's target; it is
                    // hidden for good from the crumble on (it dies hidden at the hand-back)
                    let mut held = e.clone();
                    held.alive = true;
                    e.alive = false;
                    // SAFETY: `e.chr` was read from the character sets in this frame phase and found complete.
                    let hidden = crate::zfinale::hidden_now(id.0) || (held.chr != 0 && !unsafe { &*(held.chr as *const ChrIns) }.chr_flags1c5.enable_render());
                    let _ = self.puppets.zetta_keep(&held, hidden);
                    crate::zfinale::keep_hidden(id.0, true);
                    // its hurt kept on while it is seen (the hit reaction asked again, as the held target's), never its death
                    if !hidden && self.zpose.as_ref().is_none_or(|z| z.id != id.0) && self.puppets.zetta_hurt_due(&held.handle, true, dt) {
                        let _ = self.ask_reaction(held.id, kh_core::enums::ETresDamageKind::Small, Vec3::X);
                    }
                }
            }
            self.puppets.zetta_ai_frame();
            return;
        }
        self.puppets.zetta_ai_frame();
        if !self.zdefer.any() {
            return;
        }
        let phase_ids: Vec<u32> = self.list.iter().map(|e| e.id.0).filter(|id| self.zdefer.is_phase(*id)).collect(); // (ZETTA 2026-10-23) before `due` forgets them
        for (id, key, hp) in self.zdefer.due() {
            let Some(e) = self.list.iter().find(|e| e.id.0 == id && game::handle_bits(&e.handle) == key).cloned() else {
                erlog!("zetta: target {id}: {hp} ER HP owed, but the game has it no more: nothing taken");
                continue;
            };
            self.puppets.zetta_release(&e.handle);
            if !e.alive || e.hp <= 0 {
                continue;
            }
            // (2026-10-23) a body Zettaflare killed dies HIDDEN (never drawn again first): its death watched; the finish is
            // Zettaflare's
            let killed = crate::zdefer::Book::kh_hp(e.hp, hp) == 0 && !phase_ids.contains(&id);
            // (ZETTA 2026-10-23, the user: "When you kill a boss with Zettaflare, they should not have any dialogue that plays you
            // just get the rewards. This also means skipping any cutscenes associated with them.") zboss.rs
            // (ZETTA 2026-10-24, the user: "killing a boss with a second phase during it's first phase with Zettaflare should just
            // kill the boss entirely skipping the second phase. The cutscene should not play") THE BOSS'S OWN EVENTS' PLAN (zphase.rs)
            let ev_plan = if killed { crate::targets::find_chr(&e.handle).map(|chr| (crate::zphase::map_of(chr), chr.event_entity_id)).map(|(map, ent)| (map.clone(), ent, crate::zphase::plan(&map, ent))) } else { None };
            let mut story = false;
            let mut planned = false;
            if let Some((map, ent, now)) = ev_plan {
                match now {
                    crate::zphase::PlanNow::Plan(p) if p.story_critical => {
                        story = true;
                        erlog!("zetta: target {id} (c{:04}, entity {ent} in {map}): a two-phase boss whose change is STORY-CRITICAL ({}): THE GAME'S OWN CHANGE PLAYS (never blocked: the user's \"Never block the ending sequence or a story transition\")", e.npc_id, p.why);
                    }
                    crate::zphase::PlanNow::Plan(p) => {
                        planned = crate::zphase::begin(p);
                        if !planned {
                            erlog!("zetta: target {id}: the whole kill's event block is not available: the old route");
                        }
                    }
                    crate::zphase::PlanNow::NoPlan => {}
                    crate::zphase::PlanNow::Loading => erlog!("zetta: target {id} (entity {ent} in {map}): its events are still being read: the route without them"),
                    crate::zphase::PlanNow::Failed(why) => erlog!("zetta: target {id} (entity {ent} in {map}): its events could not be read ({why}): the route without them"),
                }
            }
            if story {
                self.zwhole_plan.remove(&id);
                if !self.zkeep.contains(&id) {
                    self.zkeep.push(id);
                }
            }
            if killed {
                let boss = crate::erhud::boss_bar_handles().iter().any(|h| *h == e.handle);
                // (a story-critical change's cutscene is the game's: no skip for it)
                // (2026-10-24) a defeat that plays its own cutscene (an ending, a story beat) keeps it: no skip for it
                let own_cut = crate::targets::find_chr(&e.handle).and_then(|chr| crate::zphase::defeat_cutscenes(&crate::zphase::map_of(chr), chr.event_entity_id)).unwrap_or_default();
                if !own_cut.is_empty() {
                    erlog!("zetta: target {id}: its defeat plays its own cutscene(s) {own_cut:?} (an ending / a story beat): THEY PLAY (no silent-boss skip)");
                }
                crate::zboss::killed(key, e.npc_id, boss && !story && own_cut.is_empty());
            }
            // (ZETTA 2026-10-24, the user, 2026-10-24: "killing a boss with a second phase during it's first phase with Zettaflare should just kill the boss entirely skipping the second phase") THE WHOLE KILL: its no-death states off, its final form's rows killed with it (hidden)
            let whole = self.zwhole_plan.remove(&id).map(|rows| if planned { Vec::new() /* (the events' plan: no other rows) */ } else { rows }).or_else(|| planned.then(Vec::new));
            if let Some(rows) = whole.as_ref().filter(|_| killed) {
                if let Some(chr) = crate::targets::find_chr(&e.handle) {
                    crate::zdefer::clear_no_death(chr);
                }
                for row in rows {
                    let Some(r) = self.list.iter().find(|o| o.id.0 == *row && o.alive && o.hp > 0).cloned() else { continue };
                    let rkey = game::handle_bits(&r.handle);
                    if let Some(chr) = crate::targets::find_chr(&r.handle) {
                        crate::zdefer::clear_no_death(chr);
                    }
                    crate::zfinale::hide_for_good(r.id.0, r.handle);
                    crate::zboss::killed(rkey, r.npc_id, true);
                    self.zetta_kills.retain(|k| k.0 != r.id.0);
                    self.zetta_kills.push((r.id.0, 0.0));
                    let ratio = self.memory.get(&r.id.0).map_or(1.0, |m| if m.ratio > 0.0 { m.ratio } else { 1.0 });
                    let lethal = ((r.hp.max(1) as f32 * 2.0) / ratio.max(0.05)) / self.params.out_scale.max(1.0);
                    let rhit = HitRecord { damage: lethal, damage_kind: kh_core::enums::ETresDamageKind::Small, direction: Vec3::X, position: r.pos + Vec3::Z * r.half_height, flags: kh_core::host::HIT_FLAG_FORCE, ..HitRecord::empty(r.id) };
                    let _ = self.apply_hit_game(&rhit);
                    self.zwhole.push((r.id.0, rkey, 0.0, false, false));
                    erlog!("zetta: WHOLE KILL: target {} (c{:04}), a row of its final form ({} HP, hidden), takes a lethal hit with its no-death state off", r.id.0, r.npc_id, r.hp);
                }
                self.zwhole.push((id, key, 0.0, false, true));
            }
            if killed {
                crate::zfinale::keep_hidden(id, true);
                if whole.is_none() {
                    self.zdeaths.retain(|d| d.0 != id);
                    self.zdeaths.push((id, key, 0.0));
                }
                self.zetta_kills.retain(|k| k.0 != id);
                self.zetta_kills.push((id, 0.0));
            }
            // through the game's pipeline (a bullet over the negation measured on it, its floors honoured)
            let ratio = self.memory.get(&id).map_or(1.0, |m| if m.ratio > 0.0 { m.ratio } else { 1.0 });
            // (ZETTA 2026-10-24) a whole kill: at least twice what it has left (it must die, not stop at a floor)
            let hp = if whole.is_some() { hp.max(e.hp.saturating_mul(2)) } else { hp };
            let kh = (hp as f32 / ratio.max(0.05)) / self.params.out_scale.max(1.0);
            let hit = HitRecord { damage: kh, damage_kind: kh_core::enums::ETresDamageKind::Small, direction: Vec3::X, position: e.pos + Vec3::Z * e.half_height, ..HitRecord::empty(e.id) };
            let r = self.apply_hit_game(&hit);
            erlog!("zetta: THE HAND-BACK: target {id} (c{:04}) takes the {hp} ER HP Zettaflare owed (hp {} -> {} expected){}", e.npc_id, e.hp, r.hp_after, if killed { ": KILLED by Zettaflare: it dies HIDDEN (never drawn again), Zettaflare's sequence is its only finish" } else { ": a survivor: its own phase change and cinematics are the game's from now" });
        }
    }

    /// (ZETTA 2026-10-23, the user: "Also in Melania's bossroom if I was targetting something invisible. Make sure it's impossible
    /// to target invisible things") ONE BODY PER SPOT: the game's rows standing on one drawn body (`zfinale::coincident`:
    /// Malenia's five c2120 rows on her very place, her phase-2 instances) are one target, the one the game shows of them
    /// ([`representative`]); the others are no target (not lockable, not soft-targeted, not hit by anything of ours, no
    /// gauge, no push body: inactive). A character the game does not draw is no target already (IMPL_ER57 `drawn`).
    /// (ZETTA 2026-10-23, the user: "STILL targetting something invisible here, I TOLD YOU to make anything invisible
    /// untargettable! If you can't lock onto it in Elden Ring why would you be able to lock onto it with the kingdom hearts
    /// characters?!") THE GAME'S OWN LOCK-ON IS THE GATE: a hostile character is a target of ours (lock-on, soft target, aim,
    /// the party's AI, Zettaflare, the gauge, the magic) only while Elden Ring's own lock-on search lists it among its
    /// candidates (`LockTgtMan`'s nodes: the game's filter, whatever it tests: lock-on point, visibility, flags, range),
    /// or did within `LOCK_GATE_HOLD_S` (a moment out of its view); everything else is inactive for us (the user's log after
    /// Malenia: her leftover c2120 rows, alive and hostile by every flag we read, invisible, never in the game's list).
    /// Our own exclusions stand beside it (the bodies we hide, the allies' proxies, a spot's duplicates). A body Zettaflare
    /// holds or owes HP to is never dropped mid-sequence. Until the game's list has been seen with a node in it (unreadable),
    /// the gate does not hold (told once).
    fn lock_gate(&mut self, dt: f32) {
        let cands = er_lock_candidates();
        if !self.lock_gate_live && cands.as_ref().is_some_and(|c| !c.is_empty()) {
            self.lock_gate_live = true;
            erlog!("targets: ELDEN RING'S OWN LOCK-ON CANDIDATES ARE THE GATE (the user: \"If you can't lock onto it in Elden Ring why would you be able to lock onto it\"): a character is ours to target only while the game's lock-on lists it (or did within {LOCK_GATE_HOLD_S} s)");
        }
        let Some(cands) = cands else { return };
        for e in self.list.iter_mut() {
            let m = self.memory.entry(e.id.0).or_default();
            let listed = cands.contains(&game::handle_bits(&e.handle));
            m.cand_age = if listed { Some(0.0) } else { m.cand_age.map(|a| a + dt) };
            if !self.lock_gate_live || !e.hostile {
                continue;
            }
            let held = self.puppets.held_at(&e.handle).is_some() || self.zdefer.owed(e.id.0) > 0;
            if !lock_gate_open(m.cand_age, held) {
                e.active = false;
            }
        }
    }

    /// (ZETTA 2026-10-23) The heavy hurt of Zettaflare's target begins (zpose.rs): its heaviest level asked of the game.
    fn zpose_begin(&mut self, e: &Enemy, when: &str) {
        let before = crate::targets::find_chr(&e.handle).and_then(|c| crate::puppet::damage_anim(c));
        self.zpose = Some(ZPose { id: e.id.0, handle: e.handle, npc_id: e.npc_id, stage: crate::zpose::Stage::Asked { level: 0, waited: 0.0, before }, muted: 0, frame: self.frame_no, armour: None });
        self.zpose_ask(0);
        erlog!("zetta: target {} (c{:04}): ITS HEAVY HURT asked at {when}, frame {} (dmgLevel {}: the heaviest standing reaction; the next levels down {:?} if it plays nothing); its sounds muted from now", e.id.0, e.npc_id, self.frame_no, crate::zpose::first_level(), &crate::zpose::HEAVY_LEVELS[1..]);
    }

    /// (ZETTA 2026-10-23, the user, 2026-10-23: "Margit also didn't look like he was playing any kind of hurt animation") THE `k`th ASK OF THE HEAVY HURT (zpose.rs `ask_of`): the body's armour broken first (its
    /// super armour and toughness to 0, as they were kept for the let-go), then a 1-point bullet carrying the level and a poise
    /// damage past any body's.
    fn zpose_ask(&mut self, k: usize) {
        let Some(ask) = crate::zpose::ask_of(k) else { return };
        let Some(z) = self.zpose.as_mut() else { return };
        let (id, handle) = (z.id, z.handle);
        if ask.break_armour {
            if let Some(chr) = crate::targets::find_chr(&handle) {
                let was = break_armour(chr);
                if z.armour.is_none() {
                    z.armour = was;
                    if let Some((sa, tough)) = was {
                        erlog!("zetta: target {id} (c{:04}): its armour broken for the heavy hurt (super armour {sa:.0} -> 0, toughness {tough:.0} -> 0; put back when it is let go)", z.npc_id);
                    }
                }
            }
        }
        self.ask_animation_with(TargetId(id), ask.level, if ask.break_armour { crate::zpose::FORCE_POISE } else { 0.0 });
    }

    /// (ZETTA 2026-10-23) At Zettaflare's beam hit on `id`: the dmgLevel its token carries: 0 when it is posed already (no
    /// new animation over its frozen pose), else its heaviest level, the pose beginning with this hit.
    fn zpose_at_hit(&mut self, id: TargetId) -> u8 {
        if self.zpose.as_ref().is_some_and(|z| z.id == id.0) {
            return 0;
        }
        let Some(e) = self.get(id).cloned() else { return crate::zpose::first_level() };
        let before = crate::targets::find_chr(&e.handle).and_then(|c| crate::puppet::damage_anim(c));
        self.zpose = Some(ZPose { id: id.0, handle: e.handle, npc_id: e.npc_id, stage: crate::zpose::Stage::Asked { level: 0, waited: 0.0, before }, muted: 0, frame: self.frame_no, armour: None });
        erlog!("zetta: target {} (c{:04}): ITS HEAVY HURT asked AT THE HIT, frame {} (no unseen window: the cinematic camera off): dmgLevel {} on the beam's hit", id.0, e.npc_id, self.frame_no, crate::zpose::first_level());
        crate::zpose::first_level()
    }

    /// (ZETTA 2026-10-23) Once a frame: the posed body's next step (zpose.rs `step`): the next level asked, its clip run to its
    /// middle, frozen there; its sounds muted throughout; ended when Zettaflare holds it no more.
    fn zpose_frame(&mut self, dt: f32) {
        let Some(mut z) = self.zpose.take() else { return };
        let held = self.puppets.held_at(&z.handle).is_some() || self.puppets.zetta_ai_held(&z.handle);
        let Some(chr) = crate::targets::find_chr(&z.handle) else {
            erlog!("zetta: target {}: its heavy hurt ends (the game has it no more)", z.id);
            return;
        };
        if !held && !matches!(z.stage, crate::zpose::Stage::Asked { .. }) || z.frame + 60 * 60 < self.frame_no {
            self.puppets.zetta_freeze(&z.handle, false);
            if let Some((sa, tough)) = z.armour {
                put_armour(chr, sa, tough);
            }
            erlog!("zetta: target {}: its heavy hurt is let go (Zettaflare holds it no more: thrown or handed back); its sounds were muted {} frame(s)", z.id, z.muted);
            return;
        }
        // silent: its sounds off every frame (the game's distance rule switches them on again once we stop)
        chr.chr_flags1ca.set_sounds_active(false);
        z.muted += 1;
        let now = crate::puppet::damage_anim(chr);
        match crate::zpose::step(&mut z.stage, now, dt) {
            crate::zpose::Do::Nothing => {}
            crate::zpose::Do::Ask(level) => {
                erlog!("zetta: target {} (c{:04}): dmgLevel {} started no damage animation: dmgLevel {level} asked (its armour broken first)", z.id, z.npc_id, crate::zpose::HEAVY_LEVELS.iter().position(|l| *l == level).map_or(0, |k| crate::zpose::HEAVY_LEVELS[k.saturating_sub(1)]));
                let k = crate::zpose::HEAVY_LEVELS.iter().position(|l| *l == level).unwrap_or(0);
                self.zpose = Some(z);
                self.zpose_ask(k);
                return;
            }
            crate::zpose::Do::Speed(v) => crate::chranim::set_speed_fast(chr, v),
            crate::zpose::Do::Freeze { first } => {
                crate::chranim::set_speed(chr, 0.0);
                if first {
                    self.puppets.zetta_freeze(&z.handle, true);
                    if let crate::zpose::Stage::Frozen { level, anim, at } = z.stage {
                        erlog!("zetta: target {} (c{:04}): HEAVY HURT FROZEN at its middle: dmgLevel {} played animation {} ({:.2} s), held at {:.2} s ({:.0} %), frame {} ({} frame(s) after it was asked); method FAST-FORWARD (the SDK has no seek of the Havok clip: TimeAct play_time is the record), sounds muted {} frame(s) (the skipped events are not counted one by one: the SDK lists none)", z.id, z.npc_id, crate::zpose::HEAVY_LEVELS[level], anim.id, anim.length, at, 100.0 * at / anim.length.max(1e-3), self.frame_no, self.frame_no - z.frame, z.muted);
                    }
                }
            }
            crate::zpose::Do::FreezeAsIs => {
                // (the user, 2026-10-23: "Margit also didn't look like he was playing any kind of hurt animation") nothing could be forced: its own animation frozen where it is (no idling, no attack on)
                crate::chranim::set_speed(chr, 0.0);
                self.puppets.zetta_freeze(&z.handle, true);
                let a = crate::chranim::now(chr);
                erlog!("zetta: target {} (c{:04}): NO dmgLevel of {:?} started a damage animation even with its armour broken: ITS OWN ANIMATION IS FROZEN where it is (animation {} at {:.2} s of {:.2}), frame {}", z.id, z.npc_id, crate::zpose::HEAVY_LEVELS, a.id, a.time, a.length, self.frame_no);
            }
        }
        self.zpose = Some(z);
    }

    /// (ZETTA 2026-10-24, the user, 2026-10-24: "killing a boss with a second phase during it's first phase with Zettaflare should just kill the boss entirely skipping the second phase") EVERY ROW OF A WHOLE KILL WATCHED TO ITS DEATH (zdefer `whole_watch`): its no-death states
    /// kept off and its render switch off every frame (no reveal of a final form); a floor that holds after the lethal
    /// bullet: its HP written 0 (the game's own death follows: its death event, rewards, flags); still alive after that: the
    /// old route for the body (drawn again, the game's own phase change, its last words and cutscene its own), logged.
    fn zwhole_frame(&mut self, dt: f32) {
        // a late whole kill (the watch saw the floor): the rows killed now
        for (id, key, rows) in std::mem::take(&mut self.zwhole_late) {
            for row in rows {
                let Some(r) = self.list.iter().find(|o| o.id.0 == row && o.alive && o.hp > 0).cloned() else { continue };
                let rkey = game::handle_bits(&r.handle);
                crate::zfinale::hide_for_good(r.id.0, r.handle);
                crate::zboss::killed(rkey, r.npc_id, true);
                self.zetta_kills.retain(|k| k.0 != r.id.0);
                self.zetta_kills.push((r.id.0, 0.0));
                self.zwhole.push((r.id.0, rkey, crate::zdefer::WHOLE_BULLET_S, false, false));
            }
            self.zwhole.push((id, key, crate::zdefer::WHOLE_BULLET_S, false, true));
        }
        let mut watch = std::mem::take(&mut self.zwhole);
        watch.retain_mut(|(id, key, since, written, body)| {
            *since += dt;
            let row = self.list.iter().find(|e| e.id.0 == *id && game::handle_bits(&e.handle) == *key).cloned();
            let alive = row.as_ref().is_some_and(|e| e.alive && e.hp > 0);
            if let Some(chr) = row.as_ref().and_then(|e| crate::targets::find_chr(&e.handle)) {
                if alive {
                    crate::zdefer::clear_no_death(chr);
                }
                crate::zfinale::hold_hidden(&row.as_ref().unwrap().handle);
            }
            match crate::zdefer::whole_watch(alive, *since, *written) {
                crate::zdefer::Finish::Wait => true,
                crate::zdefer::Finish::Done => {
                    erlog!("zetta: WHOLE KILL: target {id}{} is DEAD by the game's own word {:.1} s after the hand-back (hidden for good): its death event, rewards and flags are the game's", if *body { " (the boss)" } else { " (its final form's row)" }, *since);
                    false
                }
                crate::zdefer::Finish::WriteHp => {
                    if let Some(chr) = row.as_ref().and_then(|e| crate::targets::find_chr(&e.handle)) {
                        crate::zdefer::clear_no_death(chr);
                        chr.modules.data.as_mut().hp = 0;
                        erlog!("zetta: WHOLE KILL: target {id} still alive {:.1} s after its lethal hit (a floor held): its HP WRITTEN 0, its no-death states off", *since);
                    }
                    *written = true;
                    true
                }
                crate::zdefer::Finish::FallBack => {
                    if *body {
                        crate::zfinale::keep_hidden(*id, false);
                        crate::zboss::revoke(*key);
                        self.zetta_kills.retain(|k| k.0 != *id);
                        erlog!("!!! zetta: WHOLE KILL FAILED: target {id} is still alive {:.1} s after its HP was written 0: the game cannot be made to finish it: THE OLD ROUTE (drawn again, the game's own phase change)", *since);
                    } else {
                        erlog!("!!! zetta: WHOLE KILL: the final form's row {id} would not die: left hidden");
                    }
                    false
                }
            }
        });
        self.zwhole = watch;
    }

    /// (ZETTA 2026-10-23, the user: "Zettaflare overrules that remember!!") Was this death Zettaflare's (a body it killed,
    /// handed back, or one it owes HP to)? Then KH3's boss finish does not play.
    pub fn zetta_death(&self, id: TargetId) -> bool {
        self.zetta_kills.iter().any(|k| k.0 == id.0) || self.zdefer.owed(id.0) > 0
    }

    fn duplicates_no_target(&mut self) {
        let dup = duplicates(&self.list);
        for (e, d) in self.list.iter_mut().zip(dup) {
            if d {
                e.active = false;
            }
        }
    }
}

impl Targets for ErTargets {
    /// (ZETTA) Zettaflare's held target: the puppet holds it (puppet.rs `zetta_hold`), turning it to Donald; `None` gives
    /// it back. UNTESTED IN GAME.
    fn script_hold(&mut self, id: TargetId, hold: Option<kh_core::host::ScriptHold>) {
        let Some(e) = self.get(id).cloned() else { return };
        match hold {
            Some(h) => {
                let d = h.face - e.pos;
                let face_yaw = if d.x.abs() + d.y.abs() > 1.0 { d.y.atan2(d.x).to_degrees() } else { e.yaw };
                let _ = self.puppets.zetta_hold(&e, face_yaw, h.turn_rate * h.dt.max(0.0), h.hurt); // (ZETTA round 5c) hurt
                // (ZETTA 2026-10-23, the user: "could the fast forward happen while Donald is charging so it is never actually
                // visible?") the pre-pose of its heavy hurt in the window kh-party opens (Donald's shot, the target out of frame)
                #[cfg(feature = "party")]
                if self.zpose.is_none() && e.alive && kh_party::donald::zettaflare::prepose_now() {
                    self.zpose_begin(&e, "the PRE-POSE (Donald's shot, the target out of frame, or the last frames before the hit's cut)");
                }
                let posed = self.zpose.as_ref().is_some_and(|z| z.id == id.0);
                // (ZETTA round 5c) THE HURT DURING THE HOLD (UNTESTED IN GAME): from the beam's hit its generic hit reaction
                // (Small, along the beam: from Donald through it) is asked again every puppet::ZETTA_HURT_EVERY s while it is
                // held; a reaction without a hit: no damage, no numbers (the forced damage of the hit is its own route)
                if self.puppets.zetta_hurt_due(&e.handle, h.hurt, h.dt) && !posed {
                    let dir = ((e.pos - h.face) * Vec3::new(1.0, 1.0, 0.0)).normalize_or(Vec3::X);
                    let asked = self.ask_reaction(id, kh_core::enums::ETresDamageKind::Small, dir);
                    static TOLD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                    if !TOLD.swap(true, std::sync::atomic::Ordering::Relaxed) {
                        erlog!("zetta: held target {} hurt: its hit reaction is asked every {} s ({asked})", id.0, crate::puppet::ZETTA_HURT_EVERY);
                    }
                }
            }
            // (ZETTA 2026-10-23, the user: "Melania was also attacking during Zettaflare!") a body Zettaflare killed is kept
            // until the hand-back (`zetta_owes`), not given back as the party's hold ends
            None if !zetta_gives_back(e.alive, self.zdefer.owed(id.0)) => {}
            None => self.puppets.zetta_release(&e.handle),
        }
    }

    /// (ZETTA) THE FINALE ON AN ELDEN RING CHARACTER (UNTESTED IN GAME): its state is published (`ZETTA_FINALE`) for the
    /// character draw route; until that route draws the black silhouette over a ChrIns, the fallback is the beam's own
    /// white light engulfing its bounds (kh-party spawns it sized to the target) and the game's own death (rewards intact).
    /// (ZETTA 2026-10-23, the user: "make sure the camera dynamically zooms out to frame the whole target and the beam and all
    /// other effects dynamically scale to cover the target completely") ITS DRAWN SIZE: the real body (the drawn rows on its
    /// very spot count as one: `zfinale::coincident`, RSF's rule, so Malenia's five extras neither shrink nor stand in for
    /// her), its capsule widened by the reach of limbs and weapons beyond it (`ZETTA_REACH_RADIUS` / `ZETTA_REACH_HEIGHT`:
    /// Malenia's capsule is 2.6 m x 57 cm, her sword reaches far past it). UNTESTED IN GAME.
    fn drawn_extent(&self, id: TargetId) -> Option<(f32, f32)> {
        let e = self.get(id)?;
        let (r, h) = drawn_body(e, &self.list);
        Some((r * ZETTA_REACH_RADIUS, h * ZETTA_REACH_HEIGHT))
    }

    fn script_finale(&mut self, id: TargetId, finale: Option<kh_core::host::ScriptFinale>) {
        let handle = self.get(id).map(|e| e.handle);
        crate::zfinale::asked(id.0, handle, finale); // (ZFIN) the character draw route (zfinale.rs -> kh-er-render finale.rs)
        let mut book = ZETTA_FINALE.lock().unwrap_or_else(|e| e.into_inner());
        book.retain(|x| x.0 != id.0);
        if let (Some(f), Some(h)) = (finale, handle) {
            if f.silhouette && f.crumble <= 0.0 {
                static TOLD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                if !TOLD.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    erlog!("zetta: target {} killed by Zettaflare: its black silhouette waits for the character draw route; the white light engulfs it meanwhile", id.0);
                }
            }
            book.push((id.0, h, f));
        }
    }

    /// (ZETTA) THE GENERIC RULE (UNTESTED IN GAME; no per-enemy data): a held enemy may be shown while its own animation
    /// visibly moves on (puppet.rs `zetta_tick`: its TimeAct animation advancing) and it is not in an attack; otherwise the
    /// cutscene favours Donald and the beam. A body the puppet is reacting (its hit reaction) is animating by definition.
    fn held_animated(&self, id: TargetId) -> bool {
        let Some(e) = self.get(id) else { return false };
        if self.puppets.reaction_state(id) != 0 {
            return true;
        }
        !e.in_attack && self.puppets.zetta_animating(&e.handle)
    }

    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.extend(self.list.iter().filter_map(|e| self.info_of(e)));
        self.props.enumerate(out); // (IMPL_ER32) breakable map objects: hit, never locked on to
    }

    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        if crate::breakables::is_prop(id) {
            return self.props.info(id);
        }
        self.get(id).and_then(|e| self.info_of(e))
    }

    /// (DON-MAGIC) Donald's size rule (6.1: large enemies and bosses never freeze): a boss-bar character 4, a Large body 3,
    /// Medium 2, Small 1 (ermap.rs weight classes).
    /// (DON-PROG) Elemental Insight / AI Donald: the enemy's NpcParam elements (insight.rs); never a damage rule.
    #[cfg(feature = "party")]
    fn element_affinity(&self, id: TargetId) -> Option<[i32; 8]> {
        let e = self.get(id)?;
        crate::insight::of_row(e.npc_param).map(|x| crate::insight::affinity(&x, &kh_party::donald::ability_info::prog()))
    }

    fn size_class(&self, id: TargetId) -> u8 {
        match self.get(id) {
            Some(e) if crate::erhud::boss_bar_handles().contains(&e.handle) => 4,
            Some(e) => match e.weight {
                crate::ermap::Weight::Large => 3,
                crate::ermap::Weight::Medium => 2,
                crate::ermap::Weight::Small => 1,
            },
            None => 0,
        }
    }

    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if crate::breakables::is_prop(id) {
            return self.props.hurt_shapes(id, out);
        }
        if let Some(e) = self.get(id) {
            let centre = e.pos + Vec3::Z * e.half_height;
            let axis = Vec3::Z * (e.half_height - e.radius).max(0.0);
            out.push(HurtShape { a: centre - axis, b: centre + axis, radius: e.radius, part: 0 });
        }
    }

    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        // (ZETTA 2026-10-23, zdefer.rs) ZETTAFLARE'S HITS WHILE IT OWNS THE PRESENTATION: their HP is owed (KH3's side sees
        // it taken: the kill, the finale), the game's own side gets one point (its hit animation, the throw's), never its death
        if self.zowned && crate::zdefer::owned_hit(hit.flags) && hit.takes_hp() && !crate::breakables::is_prop(hit.target) {
            if let Some((kh_before, game_hp, key)) = self.list.iter().find(|e| e.id == hit.target && e.hostile && (e.alive || self.zdefer.owed(e.id.0) > 0)).map(|e| (e.hp, e.hp_game.max(e.hp), game::handle_bits(&e.handle))) {
                let full = self.params.damage_out(hit.damage * crate::donstatus::exposed_mult(hit.target, hit.flags));
                self.zdefer.add(hit.target.0, key, full);
                // (ZETTA 2026-10-24) a boss's own map events read ahead (zphase.rs: the whole kill at the hand-back)
                if let Some(e) = self.list.iter().find(|e| e.id == hit.target && e.chr != 0) {
                    // SAFETY: `e.chr` was read from the character sets in this frame phase and found complete.
                    let chr = unsafe { &*(e.chr as *const ChrIns) };
                    let _ = crate::zphase::plan(&crate::zphase::map_of(chr), chr.event_entity_id);
                }
                // (2026-10-23, the user: "mod detects the second phase and just throws them back") its death a phase change?
                // the game's no-death state on it, or its phase rows on its spot: then a survivor on KH3's side
                if let Some(e) = self.list.iter().find(|e| e.id == hit.target) {
                    // SAFETY: `e.chr` was read from the character sets in this frame phase and found complete.
                    let no_dead = if e.chr != 0 { crate::zdefer::no_dead_of(unsafe { &*(e.chr as *const ChrIns) }) } else { None };
                    let immortal = no_dead.is_some();
                    if e.chr != 0 {
                        // (2026-10-23) what the game's no-death states read at the hit (the log's evidence)
                        let chr = unsafe { &*(e.chr as *const ChrIns) };
                        erlog!("zetta: target {} (c{:04}) at the hit: data flags 0x{:02x}, TimeAct immortality {}, no-death state {:?}", e.id.0, e.npc_id, crate::zdefer::data_flags(chr), chr.modules.action_flag.action_modifiers_flags.set_immortality(), no_dead);
                    }
                    let twins = self.list.iter().any(|o| o.id != e.id && o.npc_id == e.npc_id && crate::zfinale::coincident(o.pos, o.radius, e.pos, e.radius));
                    if crate::zdefer::phase_change(immortal, twins) && !self.zwhole_plan.contains_key(&hit.target.0) {
                        // (ZETTA 2026-10-24, the user, 2026-10-24: "killing a boss with a second phase during it's first phase with Zettaflare should just kill the boss entirely skipping the second phase") THE WHOLE BOSS IS KILLED: a kill on KH3's side (the frozen hurt, the
                        // silhouette, the crumble, hidden for good); at the hand-back its no-death states are taken off and it
                        // dies, its final form's rows with it (hidden); the old survivor's route only if the game will not
                        let rows: Vec<(u32, i32, bool)> = self.list.iter().map(|o| (o.id.0, o.npc_id, o.alive && o.hp > 0)).collect();
                        let fin = crate::zdefer::final_rows(e.id.0, e.npc_id, &rows);
                        let shape = crate::zdefer::shape_of(fin.len());
                        erlog!("zetta: target {} (c{:04}): its death would be a PHASE CHANGE ({}): THE WHOLE BOSS IS KILLED (shape {:?}: {}); its no-death state is taken off at the hand-back, the game's own death and rewards follow", e.id.0, e.npc_id, if immortal { no_dead.unwrap_or("the game's no-death state on it") } else { "its phase rows on its spot" }, shape, match shape { crate::zdefer::Shape::Duplicates(_) => format!("its final form's rows {fin:?} are killed with it, hidden"), crate::zdefer::Shape::SameCharacter => "its own body (no final-form row in the world)".to_string() });
                        self.zwhole_plan.insert(e.id.0, fin);
                    }
                }
                let owed = self.zdefer.owed(hit.target.0);
                if kh_before > 0 && game_hp > 2 {
                    let mut token = hit.clone();
                    token.damage = 1.0 / self.params.out_scale.max(1.0);
                    // (2026-10-23) its heavy hurt: already posed (the pre-pose, unseen): the token starts nothing new; else it is
                    // asked by this very hit (no unseen window: the cinematic camera off)
                    self.zlevel = Some(self.zpose_at_hit(hit.target));
                    let _ = self.apply_hit_game(&token);
                    self.zlevel = None;
                }
                let game_now = self.list.iter().find(|e| e.id == hit.target).map_or(game_hp, |e| e.hp.max(1));
                let _ = owed;
                let after = self.zdefer.kh_hp_of(hit.target.0, game_hp.max(game_now));
                let killed = kh_before > 0 && after == 0;
                if let Some(e) = self.list.iter_mut().find(|e| e.id == hit.target) {
                    e.hp = after;
                    e.hp_game = after;
                    if after == 0 {
                        e.alive = false;
                    }
                }
                erlog!("zetta: target {} '{}' {:.1} KH = {} ER OWED until the hand-back (Zettaflare owns the presentation: the game keeps it alive, its death and cinematics wait): KH3's hp {} -> {}{}", hit.target.0, hit.attack_name, hit.damage, full, kh_before, after, if killed { " KILLED (held until hidden)" } else { "" });
                return HitResult { accepted: true, damage_dealt: self.params.damage_out_inverse(full.min(kh_before.max(0))), hp_after: after as f32, killed, guarded: false };
            }
        }
        self.apply_hit_game(hit)
    }

    /// (IMPL_ER14) THE HILLSIDE'S SHARE of the height difference the attack choice measures (sheet row
    /// `target.point.terrain_slope`): while Sora stands on a floor and the target stands on the ground (Elden Ring's
    /// own physics says so, and it is not our puppet in the air), the difference between their feet up to
    /// tan(slope) x their horizontal distance, and the vertical part of the target's lead (a body walking uphill).
    fn terrain_dz(&self, id: TargetId) -> f32 {
        let slope = self.params.terrain_slope;
        let Some(e) = self.get(id) else { return 0.0 };
        if slope <= 0.0 || !self.sora_on_floor || !e.grounded || e.airborne {
            return 0.0;
        }
        let flat = ((e.pos - self.sora) * Vec3::new(1.0, 1.0, 0.0)).length();
        let allowance = flat * slope.to_radians().tan();
        (e.pos.z - self.sora.z).clamp(-allowance, allowance) + e.velocity.z * (8.0 / 30.0)
    }

    fn engaged_count(&self) -> u32 {
        // (BM) a passive kind within range engages only while it fights (`engages`)
        self.list
            .iter()
            .filter(|e| e.hostile && e.alive && e.active)
            .filter(|e| engages((e.pos - self.sora).length() <= self.params.engage_range, e.passive, self.fought_lately(e.id), self.attacked_lately(e.id)))
            .count() as u32
    }

    /// The hurt shape's reaction type: KH3's DEFAULT body, or for the armoured classes its STRONG
    /// body with a reaction endurance (kh-react keeps it).
    fn body(&self, id: TargetId, _part: u32) -> TargetBody {
        match self.get(id) {
            Some(e) if e.hostile => self.puppets.body(&self.params, e),
            _ => TargetBody::DEFAULT,
        }
    }

    fn take_reaction_power(&mut self, id: TargetId, power: i32) -> i32 {
        let params = self.params;
        match self.list.iter().find(|e| e.id == id) {
            Some(e) if e.hostile => self.puppets.take_reaction_power(&params, e, power),
            _ => 0,
        }
    }

    /// (XSI, 2026-10-16) THE XS ABILITIES' POWERFUL FOE (kh-core abilities/xs: Hidden Potential's floor; XS 2.15, our
    /// rule): a boss-bar character or a Large body, at the battle level of its region band
    /// (sheets/port/sora-xs-battle-levels; the region depth the boss rewards track: bossreward.rs).
    fn foe_level(&self, id: TargetId) -> Option<u8> {
        let e = self.get(id)?;
        if !e.hostile || !e.alive {
            return None;
        }
        let boss = e.weight == crate::ermap::Weight::Large || crate::erhud::boss_bar_handles().contains(&e.handle);
        boss.then(|| kh_core::abilities::xs::region_battle_level(crate::bossreward::region_depth()))
    }

    /// (XSI) Elden Ring HP per KH3 damage point (`damage.out.scale`): the spectacle caps of Ancient Light / Myriad.
    fn damage_to_hp(&self) -> f32 {
        self.params.out_scale
    }

    /// (XSI) RAGE RAID (XS 2.11): an armoured body in its revenge (bossguard.rs rule 2).
    fn in_revenge(&self, id: TargetId) -> bool {
        self.puppets.armoured(id)
    }

    /// (XSI) Rage Raid's END hit: the body's revenge and its armour end.
    fn end_revenge(&mut self, id: TargetId) {
        self.puppets.end_revenge(id);
    }

    fn reaction_state(&self, id: TargetId) -> u8 {
        self.puppets.reaction_state(id)
    }

    fn push_body(&self, id: TargetId) -> Option<PushBody> {
        let e = self.get(id)?;
        if !e.alive || !e.active {
            return None;
        }
        Some(PushBody { center: e.pos + Vec3::Z * e.half_height, radius: e.radius, half_height: e.half_height, level: self.params.class(e.weight).push_level })
    }
}

// ---------------------------------------------------------------- Elden Ring's own lock-on

/// Makes Elden Ring's own lock-on hold `handle` (so that its camera looks at Sora's locked
/// target and its reticle and HP bar show on it): erfps2's `lock_on_to`, data writes only. The
/// target must be among the candidates the game's `LockTgtMan` found itself this frame; returns
/// false when it is not (nothing is changed then).
pub fn er_lock_on(handle: &FieldInsHandle) -> bool {
    use eldenring::cs::LockTgtMan;
    // SAFETY: game thread; every pointer of the list is checked.
    unsafe {
        let Ok(man) = LockTgtMan::instance_mut() else { return false };
        let mut found: Option<*mut eldenring::cs::LockTgtNode> = None;
        let mut next = word!(man.nodes);
        let mut nodes: Vec<usize> = Vec::new();
        while plausible(next) && nodes.len() < 256 && !nodes.contains(&next) {
            nodes.push(next);
            let node = &*(next as *const eldenring::cs::LockTgtNode);
            let value = word!(node.value);
            if plausible(value) && (*(value as *const eldenring::cs::LockTgtNodeValue)).chr_handle == *handle {
                found = Some(next as *mut eldenring::cs::LockTgtNode);
            }
            next = word!(node.next);
        }
        let Some(target) = found else { return false };
        for n in nodes {
            (*(n as *mut eldenring::cs::LockTgtNode)).flags &= !32;
        }
        (*target).flags |= 32;
        man.is_locked_on = true;
        man.is_lock_on_requested = true;
        true
    }
}

/// Ends Elden Ring's own lock-on.
pub fn er_unlock() {
    use eldenring::cs::LockTgtMan;
    // SAFETY: game thread.
    if let Ok(man) = unsafe { LockTgtMan::instance_mut() } {
        if man.is_lock_on_requested || man.is_locked_on {
            man.is_lock_on_requested = false;
            man.is_locked_on = false;
        }
    }
}

/// (testing aid, `dbg.spawn`) Asks the game's debug character creator for a copy of `like` at
/// `at` (Elden Ring space): the same character id, NpcParam and NpcThinkParam, so its files are loaded.
pub fn debug_spawn(like: &Enemy, at: Vec3) -> bool {
    // SAFETY: game thread.
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return false };
    if !plausible(word!(wcm.debug_chr_creator)) {
        return false;
    }
    wcm.spawn_debug_character(&eldenring::cs::ChrDebugSpawnRequest { chr_id: like.npc_id, chara_init_param_id: -1, npc_param_id: like.npc_param, npc_think_param_id: like.think_param, event_entity_id: 0, talk_id: 0, pos_x: at.x, pos_y: at.y, pos_z: at.z });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const NPC: Facts = Facts { can_target: false, summon: false, npc_type: true, helper: false, hit_off: false, disguise_kind: false };
    const ENEMY: Facts = Facts { can_target: true, ..NPC };

    #[test]
    fn hostile_is_lockable_and_hittable() {
        assert_eq!(classify(&ENEMY, true), Class::Hostile);
        assert_eq!(classify(&ENEMY, false), Class::Hostile);
        let f = TargetFlags(flags_of(Class::Hostile, true, true, false).unwrap());
        assert!(f.has(TargetFlags::ENEMY | TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE));
        let f = TargetFlags(flags_of(Class::Hostile, true, true, true).unwrap());
        assert!(f.has(TargetFlags::HITTABLE) && !f.has(TargetFlags::LOCKABLE));
        // (dead: enumerated as an enemy that is neither alive nor hittable, as before)
        assert_eq!(flags_of(Class::Hostile, false, true, false), Some(TargetFlags::ENEMY));
    }

    #[test]
    fn friendly_npc_is_hittable_never_lockable() {
        assert_eq!(classify(&NPC, true), Class::Friendly);
        let f = TargetFlags(flags_of(Class::Friendly, true, true, false).unwrap());
        assert!(f.has(TargetFlags::ALIVE | TargetFlags::HITTABLE));
        assert!(!f.has(TargetFlags::LOCKABLE) && !f.has(TargetFlags::ENEMY) && !f.has(TargetFlags::GIMMICK));
        assert_eq!(flags_of(Class::Friendly, false, true, false), None);
        assert_eq!(flags_of(Class::Friendly, true, false, false), None);
    }

    #[test]
    fn hit_npcs_off_is_the_old_bystander() {
        assert_eq!(classify(&NPC, false), Class::Bystander);
        assert_eq!(flags_of(Class::Bystander, true, true, false), Some(TargetFlags::ALIVE));
    }

    #[test]
    fn summons_helpers_phantoms_and_hitless_bodies_are_bystanders() {
        assert_eq!(classify(&Facts { summon: true, ..NPC }, true), Class::Bystander);
        assert_eq!(classify(&Facts { summon: true, ..ENEMY }, true), Class::Bystander);
        assert_eq!(classify(&Facts { helper: true, ..NPC }, true), Class::Bystander);
        assert_eq!(classify(&Facts { npc_type: false, ..NPC }, true), Class::Bystander);
        assert_eq!(classify(&Facts { hit_off: true, ..NPC }, true), Class::Bystander);
    }

    #[test]
    fn a_disguise_with_its_hit_body_off_is_hit_never_locked() {
        // (Boc in his bush: hit body off, c4110 of the disguise kinds)
        let boc = Facts { hit_off: true, disguise_kind: true, ..NPC };
        assert_eq!(classify(&boc, true), Class::Friendly);
        assert!(!TargetFlags(flags_of(classify(&boc, true), true, true, false).unwrap()).has(TargetFlags::LOCKABLE));
        assert_eq!(classify(&boc, false), Class::Bystander);
        assert_eq!(classify(&Facts { helper: true, ..boc }, true), Class::Bystander);
        assert_eq!(classify(&Facts { summon: true, ..boc }, true), Class::Bystander);
    }

    /// (SP2) "Sora is tripping past save points like when you run past a friendly NPC": a site of grace's own character
    /// (c1000) stands inside the save point; it is NOT offered to the core (no sway, no push), while a real bystander, a
    /// friendly NPC and every hostile (even one of id 1000) are.
    #[test]
    fn a_grace_character_is_no_body_to_sway_round_or_be_pushed_by() {
        assert!(!offered_as_body(Class::Bystander, NPC_HELPER));
        assert!(!offered_as_body(Class::Friendly, NPC_HELPER));
        assert!(offered_as_body(Class::Hostile, NPC_HELPER));
        assert!(offered_as_body(Class::Bystander, 3000) && offered_as_body(Class::Friendly, 4110));
        let mut t = ErTargets::new(Frame::at(Vec3::ZERO), Params::default());
        let mut grace = Enemy::stub(1, Vec3::new(50.0, 0.0, 0.0), 30.0, 90.0, 1, Weight::Small);
        (grace.npc_id, grace.hostile, grace.team) = (NPC_HELPER, false, 0);
        let mut bystander = Enemy::stub(2, Vec3::new(-50.0, 0.0, 0.0), 30.0, 90.0, 100, Weight::Small);
        bystander.hostile = false;
        let enemy = Enemy::stub(3, Vec3::new(0.0, 300.0, 0.0), 30.0, 90.0, 100, Weight::Small);
        t.list = vec![grace, bystander, enemy];
        let mut out = Vec::new();
        t.enumerate(&mut out);
        let ids: Vec<u32> = out.iter().map(|i| i.id.0).collect();
        assert_eq!(ids, vec![2, 3], "the grace's character is not in the core's list");
        assert!(t.info(TargetId(1)).is_none());
        // the bystander stays a body to sway round (ALIVE only), the enemy an enemy
        assert_eq!(out[0].flags, TargetFlags(TargetFlags::ALIVE));
        assert!(out[1].flags.has(TargetFlags::ENEMY | TargetFlags::LOCKABLE));
        // the save points still read the grace from the list itself (savepoint.rs: `targets.list`, npc_id 1000)
        assert!(t.list.iter().any(|e| e.npc_id == NPC_HELPER));
    }

    #[test]
    fn the_disguise_key() {
        assert_eq!(disguised_ids(None), vec![4100, 4110]);
        assert_eq!(disguised_ids(Some("c4110, 4100 3000")), vec![4110, 4100, 3000]);
        assert!(disguised_ids(Some("none")).is_empty());
    }

    #[test]
    fn the_disguise_bullet_starts_outside_the_bush_and_hits_the_map() {
        let mut e = Enemy { id: TargetId(1), handle: unsafe { std::mem::zeroed::<FieldInsHandle>() }, chr: 0, er_pos: Vec3::ZERO, pos: Vec3::ZERO, velocity: Vec3::ZERO, yaw: 0.0, radius: 40.0, half_height: 45.0, hp: 683, hp_max: 683, hp_game: 683 /* (HUD2) */, team: 26, chr_type: 5, npc_id: 4110, npc_param: 41109010, think_param: 0, alive: true, hostile: false, friendly: true, disguised: true, can_target_raw: 0, summon: false, lock_disabled: false, weight: Weight::Small, active: true, anim: -1, poise_max: 0.0, poise: 0.0, hyper: None, in_attack: false, threat_lv: Some(0), passive: false /* (BM) */, poise_recover: 0.0, puppet_ok: false, airborne: false, grounded: true, lift: 0.0 };
        e.er_pos = Vec3::new(10.0, 2.0, 5.0);
        let (from, dir, shape) = disguise_path(&e, Vec3::new(12.0, 2.3, 5.0), Vec3::NEG_X, crate::bullet::Shape::default());
        assert!(!shape.penetrate);
        assert!((Vec3::new(from.x, 0.0, from.z) - Vec3::new(10.0, 0.0, 5.0)).length() >= DISGUISE_STANDOFF);
        assert!((from.y - 2.45).abs() < 1e-4 && dir.y == 0.0 && dir.x < -0.99);
        // (it lives long enough to cross the whole standoff on the far side)
        assert!(shape.life * shape.speed >= 2.0 * DISGUISE_STANDOFF);
    }

    #[test]
    fn a_friendly_bullet_lands_on_hp_or_a_new_animation() {
        assert!(friendly_landed(500, 480, 0, 0));
        assert!(friendly_landed(500, 500, 0, 8000));
        assert!(!friendly_landed(500, 500, 0, 0));
        assert!(!friendly_landed(500, 500, 0, -1));
    }
}
