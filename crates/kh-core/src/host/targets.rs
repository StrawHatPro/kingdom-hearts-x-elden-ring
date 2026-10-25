//! The target seam: the things Sora can lock on to and hit. The host owns them (enemies in a
//! game, training dummies in the test level); the core enumerates them, asks for their shapes,
//! delivers hit records and reads back what became of each hit.
//!
//! Units and axes are the core's (cm, X forward, Y right, Z up). Answers feed the simulation:
//! they must be deterministic (same order, same values for the same history).
//!
//! CONTAINERS ONLY. What makes a target eligible for lock-on, how damage is computed and what a
//! hit does to Sora's own state are rules of docs/sora-combat-spec.md / sora-magic-spec.md and
//! are implemented in `combat/` and `magic/`, not here. Fields those specs turn out to need are
//! added here by their implementers (hosts fill what they know and leave the rest default).

use crate::enums::{ETresDamageAttribute, ETresDamageKind};
use glam::Vec3;

/// The host's name for one target; stable for as long as the target exists. 0 is "no target".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct TargetId(pub u32);

impl TargetId {
    pub const NONE: TargetId = TargetId(0);
}

/// Plain facts about a target, as bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct TargetFlags(pub u32);

impl TargetFlags {
    /// It is alive (a dead target may still be enumerated while it plays its death).
    pub const ALIVE: u32 = 1;
    /// Hits may be delivered to it.
    pub const HITTABLE: u32 = 1 << 1;
    /// It may be locked on to.
    pub const LOCKABLE: u32 = 1 << 2;
    /// It is off the ground (floating, flying, launched).
    pub const AIRBORNE: u32 = 1 << 3;
    /// Its owner carries the actor tag "Enemy" (combat strand D 7.4: a target without it is
    /// only considered within 450 cm).
    pub const ENEMY: u32 = 1 << 4;
    /// Its owner carries the actor tag "Gimmick".
    pub const GIMMICK: u32 = 1 << 5;
    /// `m_LockonPriority` HIGH (LOW without the bit).
    pub const PRIORITY_HIGH: u32 = 1 << 6;
    /// `m_bRushSuppression` of its lock-on component.
    pub const RUSH_SUPPRESSION: u32 = 1 << 7;

    pub const fn has(self, bits: u32) -> bool {
        self.0 & bits == bits
    }
}

/// One candidate target at the start of a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetInfo {
    pub id: TargetId,
    /// Its actor origin (foot level for a standing thing), cm.
    pub position: Vec3,
    /// Centre of its bounds: where a lock-on reticle or a homing attack aims by default.
    pub center: Vec3,
    pub velocity: Vec3,
    /// Facing, degrees, the core's yaw convention.
    pub yaw: f32,
    /// Bounds as an upright capsule about `center` (the `World` shape convention:
    /// `half_height == radius` is a sphere), cm.
    pub radius: f32,
    pub half_height: f32,
    pub flags: TargetFlags,
    /// Its hit points now and at full health (for the UI's target gauge; no rule reads them here).
    pub hp: f32,
    pub hp_max: f32,
}

/// One volume of a target that an attack's collision is tested against: a capsule from `a` to
/// `b` (world space, cm) of `radius`; `a == b` is a sphere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HurtShape {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
    /// The host's id of this part of the target (0 = the body); echoed in [`HitRecord::part`].
    pub part: u32,
}

/// A hit, as the core delivers it. The numbers are whatever the combat / magic rules computed;
/// this struct does not compute anything.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitRecord {
    pub target: TargetId,
    /// Which [`HurtShape::part`] was touched.
    pub part: u32,
    /// The attack: row index of the generated attack table (`TresAttackDataTable` rows, once the
    /// combat spec's sheet exists) and its name there; `u32::MAX` / "" when there is none.
    pub attack: u32,
    pub attack_name: &'static str,
    /// The contact point and the unit direction the blow travels in (from the attacker), world.
    pub position: Vec3,
    pub direction: Vec3,
    /// Damage to take off the target's hit points, already final.
    pub damage: f32,
    /// `TresAttackDataTable.m_DamageKind` and `m_DamageAttribute` of the attack.
    pub damage_kind: ETresDamageKind,
    pub attribute: ETresDamageAttribute,
    /// ALWAYS ZERO, kept for existing hosts: the game has no such number. What a victim does with
    /// a hit is decided on ITS side from `damage_kind`, `direction`, the row fields in `info` and
    /// its own damage parameters (docs/sora-enemy-reaction-spec.md); the engine-free
    /// implementation is the crate `kh-react` (`ReactionRequest::from_hit`).
    pub impulse: Vec3,
    /// (enemy-reaction implementer, 2026-10-12) The attacker as the victim's damage state reads
    /// it (`UTresAICharState_Damage` Setup 0x140457380 / Beat 0x1404463F0): see [`HitAttacker`].
    pub attacker: HitAttacker,
    /// Free bits for the rules to come (the combat spec names them).
    pub flags: u32,
    /// The rest of the game's hit record (`FTresPointDamageEvent` + `TresDamageInfo`, combat
    /// strand C 8.7): what a victim needs to play its reaction.
    pub info: HitInfo,
}

/// (enemy-reaction implementer, 2026-10-12) What a victim's reaction reads of the instigator pawn
/// (the causer; a weapon is replaced by its owner): its root location, its facing and its capsule
/// radius. The facing is the knock direction of a hit whose `direction` has no horizontal part
/// (Setup 0x140457380); the radius spaces a BEAT victim out from under the attacker (0x1404463F0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitAttacker {
    /// Root location, cm (the capsule centre for a UE pawn).
    pub position: Vec3,
    /// Facing, degrees, the core's yaw convention.
    pub yaw: f32,
    /// Capsule radius, cm.
    pub radius: f32,
    /// False: no instigator pawn is known (a debug hit, a trap); the fields above are zero.
    pub valid: bool,
}

impl HitAttacker {
    pub const NONE: HitAttacker = HitAttacker { position: Vec3::ZERO, yaw: 0.0, radius: 0.0, valid: false };
}

/// `ETresReactionResultType` (combat strand C 8.2.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ReactionResult {
    None = 0,
    NoReaction = 1,
    #[default]
    Damage = 2,
    Reflect = 3,
    GuardReflect = 4,
    Guard = 5,
    ArmorBreak = 6,
    StrongBodyDamage1 = 7,
    StrongBodyDamage2 = 8,
    StrongBodyDamage3 = 9,
    StrongBodyDamage4 = 10,
}

/// The fields of `TresDamageInfo` a hit carries besides those of [`HitRecord`] itself (combat
/// strand C 8.7; the row fields are `TresAttackDataTable`'s). `HitRecord::direction` is the
/// knockback direction (`m_KnockbackDir` = `ShotDirection`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitInfo {
    /// `m_Power` of the row (`m_AttackSrcPower`).
    pub power: f32,
    pub power_scale: f32,
    pub parabola_angle: f32,
    pub move_length: f32,
    pub brake: f32,
    pub effect_time: f32,
    /// `m_KnockbackType` as the row names it ("NORMAL", "INSTIGATOR_PAWN").
    pub knockback: &'static str,
    /// Bad status asked for (already rolled): name ("NONE", "STUN"), time, parameter.
    pub bad_status: &'static str,
    pub bad_time: f32,
    pub bad_param: f32,
    pub reaction_power: i32,
    pub armor_attack_power: i32,
    pub form_point: i32,
    pub revenge_point: f32,
    /// `m_PhysForceLv` ("LV_1").
    pub phys_force: &'static str,
    /// `TresDamageInfo` +0x50: bit 0 magic, 1 finish, 2 killer, 3 ignore guard, 4 rapid fire,
    /// 5 critical (never set), 6 armor damage, 7 aerial.
    pub flags50: u8,
    /// +0x51: bit 0 attraction, 1 counter, 2 just guard, 3 weak-guard attack, 4 full-MP burst,
    /// 5 magic draw, 6 no-react body correction.
    pub flags51: u8,
    /// +0x52..+0x55: Attack state +0x128 (combo count), +0x12C, +0x130 (the damage formula's
    /// combo step), +0x134, each clamped 0..99.
    pub counters: [u8; 4],
    pub reflect_level: i8,
    /// `m_ReflectReaction` of the row ("DEFAULT", "STAGGER", "GUARD", "NOREACTION").
    pub reflect_reaction: &'static str,
    /// +0x5C: the armour value left, written by an armoured victim (`Targets::take_armor`).
    pub armor_left: i32,
    /// What the victim is to play (decided by the attacker's classification before delivery).
    pub victim_result: ReactionResult,
    pub attacker_result: ReactionResult,
    /// The sweep's impact normal (from the victim's surface toward the attacker's shape).
    pub impact_normal: Vec3,
    /// Fraction of the sweep at the contact, 0..1.
    pub time: f32,
}

impl HitInfo {
    pub const FLAG50_MAGIC: u8 = 1;
    pub const FLAG50_FINISH: u8 = 2;
    pub const FLAG50_KILLER: u8 = 4;
    pub const FLAG50_IGNORE_GUARD: u8 = 8;
    pub const FLAG50_RAPID_FIRE: u8 = 0x10;
    pub const FLAG50_ARMOR_DAMAGE: u8 = 0x40;
    pub const FLAG51_NO_REACT_CORRECTION: u8 = 0x40;
    pub const FLAG50_AERIAL: u8 = 0x80;
    pub const FLAG51_ATTRACTION: u8 = 1;
    pub const FLAG51_COUNTER: u8 = 2;
    pub const FLAG51_JUST_GUARD: u8 = 4;
    pub const FLAG51_WEAK_GUARD: u8 = 8;

    /// "Clear bad status" (0x14064CFD0): a guard, a failed roll, a ghost.
    pub fn clear_bad_status(&mut self) {
        (self.bad_status, self.bad_time, self.bad_param) = ("NONE", 0.0, 0.0);
    }

    /// The static default row (constructor 0x140570870): power 1, scale 1, everything else 0.
    pub const DEFAULT: HitInfo = HitInfo { power: 1.0, power_scale: 1.0, parabola_angle: 0.0, move_length: 0.0, brake: 0.0, effect_time: 0.0, knockback: "NORMAL", bad_status: "NONE", bad_time: 0.0, bad_param: 0.0, reaction_power: 0, armor_attack_power: 0, form_point: 0, revenge_point: 0.0, phys_force: "LV_1", flags50: 0, flags51: 0, counters: [0; 4], reflect_level: 0, reflect_reaction: "DEFAULT", armor_left: 0, victim_result: ReactionResult::Damage, attacker_result: ReactionResult::None, impact_normal: Vec3::ZERO, time: 0.0 };
}

impl Default for HitInfo {
    fn default() -> HitInfo {
        HitInfo::DEFAULT
    }
}

impl HitRecord {
    /// A record with neutral contents for `target`: no attack row, no damage, physical.
    pub const fn empty(target: TargetId) -> HitRecord {
        HitRecord { target, part: 0, attack: u32::MAX, attack_name: "", position: Vec3::ZERO, direction: Vec3::X, damage: 0.0, damage_kind: ETresDamageKind::None, attribute: ETresDamageAttribute::Physical, impulse: Vec3::ZERO, attacker: HitAttacker::NONE, flags: 0, info: HitInfo::DEFAULT }
    }
}

/// What the target made of a hit.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct HitResult {
    /// False: the hit did not land (no such target, not hittable, invulnerable ...).
    pub accepted: bool,
    /// Hit points actually removed.
    pub damage_dealt: f32,
    pub hp_after: f32,
    /// This hit brought it to 0.
    pub killed: bool,
    /// The target blocked it.
    pub guarded: bool,
}

/// `ETresBodyCollReactionType`: what a hurt shape is to a hit (combat strand C 8.2.6; the
/// classification itself is `combat::classify`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BodyReaction {
    #[default]
    Default = 0,
    NoReaction = 1,
    Strong = 2,
    Guard = 3,
    WeakGuard = 4,
    GuardNoReact = 5,
    Invincible = 6,
    InvincibleNoReact = 7,
    Ghost = 8,
    GhostNoReact = 9,
    Reflect1 = 10,
    GhostInvincible = 11,
}

impl BodyReaction {
    pub const fn from_value(v: u8) -> BodyReaction {
        match v {
            1 => BodyReaction::NoReaction,
            2 => BodyReaction::Strong,
            3 => BodyReaction::Guard,
            4 => BodyReaction::WeakGuard,
            5 => BodyReaction::GuardNoReact,
            6 => BodyReaction::Invincible,
            7 => BodyReaction::InvincibleNoReact,
            8 => BodyReaction::Ghost,
            9 => BodyReaction::GhostNoReact,
            10 => BodyReaction::Reflect1,
            11 => BodyReaction::GhostInvincible,
            _ => BodyReaction::Default,
        }
    }
}

/// What the hit classification asks of a victim before a hit is built (combat strand C 8.2.6:
/// the body-collision interface of the touched shape and the victim's actor interface). The
/// default is a DEFAULT body that is not guarding: every hit is a plain DAMAGE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetBody {
    /// `bc.slot2()`: the touched thing has a hurt-shape interface. False = a wall or a prop: the
    /// hit is recorded, the attacker result is NOREACTION and no reaction type is read.
    pub has_body: bool,
    /// `bc.slot7()`: the reaction type of the touched shape.
    pub reaction: BodyReaction,
    /// `ai.slot89()`: the victim is guarding now (turns DEFAULT / WEAK_GUARD / an unweakened
    /// STRONG body into GUARD).
    pub guarding: bool,
    /// `ai.slot63()`: it wears armour (`Targets::take_armor` is then asked).
    pub armoured: bool,
    /// `ai.slot12(attribute)` / `ai.slot13(attribute)`: bit n = `ETresDamageAttribute` n is the
    /// victim's weak point / is nullified by it.
    pub weak: u8,
    pub nullified: u8,
    /// `ai.slot15()`: it shrugs off rapid-fire rows (`m_bRapidFire`).
    pub rapid_fire_proof: bool,
    /// `ai.slot16(0x10)`: it may refuse hits of shapes that allow refusal (prim+0x984 bit 5).
    pub refuses: bool,
}

impl TargetBody {
    pub const DEFAULT: TargetBody = TargetBody { has_body: true, reaction: BodyReaction::Default, guarding: false, armoured: false, weak: 0, nullified: 0, rapid_fire_proof: false, refuses: false };
}

impl Default for TargetBody {
    fn default() -> TargetBody {
        TargetBody::DEFAULT
    }
}

/// A push body of a target (combat strand B.4): an upright capsule about `center` and the
/// character's `m_BodyPushPower` level (`ETresBodyPushPowerLevel` 0..9; Sora is `PPL_50` = 4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PushBody {
    pub center: Vec3,
    pub radius: f32,
    pub half_height: f32,
    pub level: u8,
}

/// An active attack shape of a target that Sora's attacks can clash with (combat strand C
/// 8.2.4): a capsule from `a` to `b`, its row's `m_ReflectLevel` (> 0 = it clashes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackShape {
    pub owner: TargetId,
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
    pub reflect_level: i8,
}

impl HitRecord {
    /// The victim side of `TakeDamage` (strand E 8.4): results REFLECT, GUARD_REFLECT, GUARD and
    /// ARMOR_BREAK (3 - 6) take no HP. A host's `apply_hit` asks this before it subtracts.
    pub fn takes_hp(&self) -> bool {
        !matches!(self.info.victim_result, ReactionResult::Reflect | ReactionResult::GuardReflect | ReactionResult::Guard | ReactionResult::ArmorBreak | ReactionResult::None)
    }
}

/// The host's targets.
pub trait Targets {
    /// Appends every candidate target to `out`, in an order that depends only on the host's own
    /// history (e.g. ascending id).
    fn enumerate(&self, out: &mut Vec<TargetInfo>);
    /// One target's state now; `None` when it no longer exists.
    fn info(&self, id: TargetId) -> Option<TargetInfo>;
    /// Appends the target's hurt shapes at their current world positions. A host without
    /// per-part shapes appends one capsule equal to its bounds.
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>);
    /// Delivers a hit. The host applies it to its target and reports what happened.
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult;
    /// The target's numbers for the damage function (`status::compute_damage`; added by the
    /// stats implementer). `None`, the default, is a target WITHOUT a KH3 status object: its
    /// Defense is 0 and no floor, cap or resist applies (docs/sora-combat-spec.md 8.3, last
    /// paragraph). A host whose targets have such numbers overrides this.
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        None
    }
    /// The other pawns of SORA'S team (party members), for Cure's partner heal (magic spec I 5).
    /// The default is no party. Added by the magic implementer.
    fn allies(&self, _out: &mut Vec<Ally>) {}

    /// A heal of `amount` HP on an ally (`Recover`; `effect` = the HUD effect kind).
    fn heal_ally(&mut self, _id: TargetId, _amount: i32, _effect: u8) {}

    /// `m_AutoLockOnRange` of the target's lock-on component (combat strand D 7.4, 7.9): 0, the
    /// default, = Sora's own 900 cm; a larger value lets a big target be auto-targeted from
    /// farther away (it shortens the measured distance). Added by the combat implementer.
    fn lock_on_range(&self, _id: TargetId) -> f32 {
        0.0
    }
    /// How many enemies are ENGAGED with Sora now: the game object's count behind the battle flag
    /// (0x140EA0B20: enemy pawns whose own battle flag is set; actions spec II.3 "The inputs").
    /// More than 0 puts Sora in battle (Keyblade drawn, battle stance, Aerial Dodge); the core
    /// keeps the flag for 1.0 s after the count returns to 0. The default, 0, is a host without
    /// that notion: never in battle. Added by the actions implementer.
    fn engaged_count(&self) -> u32 {
        0
    }
    /// The victim's answers to the hit classification for hurt shape `part` (combat strand C
    /// 8.2.6). The default is a DEFAULT body that does not guard. Added by the combat implementer.
    fn body(&self, _id: TargetId, _part: u32) -> TargetBody {
        TargetBody::DEFAULT
    }
    /// `ai.slot70(power, weak)`: an armoured victim takes `power` off its armour and answers what
    /// is left (<= 0: the armour breaks). Only asked when `TargetBody::armoured`.
    fn take_armor(&mut self, _id: TargetId, _power: i32, _weak: bool) -> i32 {
        0
    }
    /// `ai.slot32(power)`: a STRONG body takes `power` off its reaction endurance and answers
    /// what is left (> 0: it holds).
    fn take_reaction_power(&mut self, _id: TargetId, _power: i32) -> i32 {
        0
    }
    /// The target's push body (combat strand B.4), `None` = it has none: Sora passes through it,
    /// exactly as the capsules do. Added by the combat implementer.
    /// (combat implementer, fourth pass) `TresLockonTargetComponent.m_bRushSuppression` (component
    /// vt+0xB10): while Sora's push body touches this target's during an attack on it, the attack
    /// stops homing and its root motion shrinks to a tenth (combat spec 7.10). Default: no.
    fn rush_suppression(&self, _id: TargetId) -> bool {
        false
    }

    /// (er-host, IMPL_ER14) THE GROUND BETWEEN SORA AND A TARGET THAT STANDS ON IT: how many cm of the height
    /// difference the attack choice measures (`TargetMetrics`: lock point Z - own Z, "the target is high" above
    /// 160) are the terrain's own rise or fall and not the target being up in the air. Subtracted from that
    /// measure only. The default, 0, is the game's own measure (its arenas are level); a host whose fights
    /// happen on hillsides answers the slope's share.
    fn terrain_dz(&self, _id: TargetId) -> f32 {
        0.0
    }

    fn push_body(&self, _id: TargetId) -> Option<PushBody> {
        None
    }
    /// The damage reaction the target is in right now, as the damage kind that caused it
    /// (`ETresDamageKind`: 2 BLOW, 3 SIDESPIN, 4 VERTICALROLL ...; 0 = none): the target's
    /// request-interface slot 89 (for a player pawn 0x1414339F0: the running Damage state's
    /// kind). Final Blow's chase is armed while it answers 2, 3 or 4 (combat spec 5.6.4). The
    /// default: never in a reaction. Added by the combat implementer (2026-10-12).
    fn reaction_state(&self, _id: TargetId) -> u8 {
        0
    }
    /// The active attack shapes of every target, for the clash pass (strand C 8.2.4). The
    /// default: none. Added by the combat implementer.
    fn attack_shapes(&self, _out: &mut Vec<AttackShape>) {}
    /// A hit that was resolved without damage reaches the victim all the same (victim notify slot
    /// 28, strand C 8.2.5 step 20: blocked, armoured, zero amount): the default ignores it.
    fn touched(&mut self, _hit: &HitRecord) {}
    /// (boss finish, docs/sora-bossfinish-spec.md) What a hit that just KILLED the target is to the host:
    /// asked right after `apply_hit` answered `killed`. KH3 decides this per map (the boss group's
    /// wipe-out event in the level blueprint), so it is the host's word: `Boss` = the boss-finish
    /// presentation, `Battle` = the port's short slow-down on the last enemy of a fight (NOT a KH3
    /// effect). The default: never. The core answers with `EventKind::BossFinish`.
    fn finish_kind(&self, _id: TargetId) -> FinishKind {
        FinishKind::None
    }

    // ---- (XSI, 2026-10-16) the XS abilities (crates/kh-core/src/abilities/xs/). Defaulted: a host without the
    // notion answers "not a powerful foe" and "one HP per damage point".
    /// HIDDEN POTENTIAL (OVERCOME) and the boss-only offers: `Some(battle level)` when the target is a POWERFUL
    /// FOE (a boss or a Giant body: our rule, XS 2.15) with the KH3 battle level its Strength / Magic floor is
    /// read at (`EnemyBaseStatus`; in Elden Ring the region band of sheets/port/sora-xs-battle-levels); `None` =
    /// an ordinary enemy (the default).
    fn foe_level(&self, _id: TargetId) -> Option<u8> {
        None
    }
    /// How many of the host's hit points one KH3 damage point becomes (Elden Ring: `damage.out.scale`). The
    /// spectacle caps (Ancient Light, Myriad: a share of `TargetInfo::hp_max` per use) divide by it. Default 1.
    fn damage_to_hp(&self) -> f32 {
        1.0
    }
    /// RAGE RAID (XS 2.11): the target broke out of a combo and its revenge runs (an armoured body: unflinchable for
    /// `react.revenge.armour`; kh-react: the revenge cool-down after `clear_revenge`). Default: never.
    fn in_revenge(&self, _id: TargetId) -> bool {
        false
    }
    /// RAGE RAID's END hit landed: the target's revenge value is cleared and its revenge armour ends. Default: nothing.
    fn end_revenge(&mut self, _id: TargetId) {}

    // ---- party stream D (team attacks; docs/party-contracts.md section D). Defaulted: a host
    // without a party answers "no friends" and ignores the requests.
    /// The party members that can take part in team attacks (Donald uid 11, Goofy uid 12), in
    /// party-slot order, with their team-attack side (`kh_party::team` fills these rows).
    fn friends(&self, _out: &mut Vec<Friend>) {}
    /// A request from Sora's side to the friends' side (team-attack start / actions / end, the
    /// gauge state, the unison partner). The host relays it to `kh_party::team::Team::request`.
    fn friend_link(&mut self, _req: FriendLinkRequest) {}
    /// (DON-MAGIC) How big the target is for Donald's statuses (documentation/DONALD_OVERHAUL.md 6.1: large enemies and
    /// bosses never freeze; 4.2: launches): 1 small, 2 medium, 3 large, 4 boss; 0, the default, = judged by its bounds
    /// (kh-party donald::magic_fx::size_of).
    fn size_class(&self, _id: TargetId) -> u8 {
        0
    }
    /// (DON-PROG) The target's elements for ELEMENTAL INSIGHT and AI Donald's choice of attunement (documentation/
    /// DONALD_OVERHAUL.md 9, 11): resist % by `ETresDamageAttribute` (negative = weak), with Donald's Light (holy) in slot
    /// 7. Read by no damage rule. The default is the KH3 status object's own resists (`defense`); a host whose targets
    /// have no KH3 status says it here (Elden Ring: its NpcParam rows, kh-er insight.rs).
    fn element_affinity(&self, id: TargetId) -> Option<[i32; 8]> {
        self.defense(id).map(|d| d.resist)
    }
    /// (ZETTA) A SCRIPTED HOLD (Donald's Zettaflare; the user: "the enemy needs to freeze, look at Donald"): asked every
    /// step while it lasts. `Some`: the target stays where it is (no movement, no turning but toward `face` at most
    /// `turn_rate` deg/s, no attacks of its own); `None`: the hold is over, the target is the host's again (asked once,
    /// also for an id never held). A hit during the hold still lands (its reaction is the host's). The default: nothing (a
    /// host without it leaves the target alone).
    fn script_hold(&mut self, _id: TargetId, _hold: Option<ScriptHold>) {}
    /// (ZETTA) Does the target held by `script_hold` play an animation the camera may show (its own idle / guard loop,
    /// never a frozen pose: the user, "make sure the enemy is never shown during the cutscene without an appropriate
    /// animation")? False: the cutscene keeps it out of its shots until its reaction. The default: yes.
    fn held_animated(&self, _id: TargetId) -> bool {
        true
    }
    /// (ZETTA) THE FINALE'S LOOK of a target (the user: "the enemy to get immersed in the white light, with a black
    /// silhouette that crumbles leaving only the white light ... universally on any enemy"; the silhouette only if
    /// Zettaflare killed it): asked every step while it lasts, `None` once at its end (the target drawn as itself again,
    /// or gone). The host draws its own body: engulfed in white by `immerse` (0..1); `silhouette`: pure black, unlit; `crumble`
    /// (0..1): broken away into fragments that scatter and dissolve (1 = nothing of it left). The death and its rewards
    /// stay the game's. The default: nothing (the beam's own white light is all a host without it shows).
    /// (ZFIN) The crumble's dissolve noise needs no field here: a host seeds it from the target's own id (Elden Ring:
    /// kh-er-render finale.rs, fixed to the body, the same every run for one target).
    fn script_finale(&mut self, _id: TargetId, _finale: Option<ScriptFinale>) {}
    /// (ZETTA 2026-10-23, the user: "make sure the camera dynamically zooms out to frame the whole target and the beam and all
    /// other effects dynamically scale to cover the target completely") HOW BIG THE TARGET IS DRAWN: (radius, half height),
    /// cm, of what the host draws of it (its model, limbs and weapon in its pose; one real body, never an invisible duplicate),
    /// when the host knows more than its capsule. Zettaflare frames and covers the larger of the two. Default: unknown.
    fn drawn_extent(&self, _id: TargetId) -> Option<(f32, f32)> {
        None
    }
}

/// (ZETTA) What [`Targets::script_finale`] asks of a target's look this step.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ScriptFinale {
    pub immerse: f32,
    pub silhouette: bool,
    pub crumble: f32,
    /// (ZETTA round 5c, the user: "when I say crumbling I mean it needs to look like the silhouette is actually breaking
    /// apart") the beam's direction at the target (unit, game space; zero: none known): the crumble's fragments scatter
    /// outward and upward on its push.
    pub push: crate::Vec3,
    /// (ZETTA round 5c, the user: "make the transition from the first to the second much smoother, no full black suddenly
    /// popping in") how far the silhouette has turned from the white the target was washed into to its full look (0..1,
    /// eased): a host blends its fill, rim and ragged outline in by it, never in one step.
    pub dark: f32,
}

/// (ZETTA) A FORCED HIT (`HitRecord::flags`; Donald's Zettaflare, the user: "Zettaflare ignores all armor, it takes
/// priority over ANYTHING"): a host takes its full damage past defence, negation, guard and boss reduction, and plays its
/// reaction past super armour, poise and any chain limit (its fall guard still applies). A host that cannot force one logs it.
pub const HIT_FLAG_FORCE: u32 = 1 << 27;

/// (ZETTA) What [`Targets::script_hold`] asks of a target this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScriptHold {
    /// The point it turns to face (world, cm: Donald's feet).
    pub face: Vec3,
    /// Its turn rate toward it (deg/s; 0 = keep the facing it has).
    pub turn_rate: f32,
    /// The step's seconds.
    pub dt: f32,
    /// (ZETTA round 5c, the user: "the enemy that is hit should play a hurt animation as a universal rule") the target has
    /// been hit: the host plays ITS OWN hurt / hit reaction (looping or held) through its damage-reaction route while it is
    /// held, never a frozen pose, until the silhouette or the throw takes over. False: its own idle (before the hit).
    pub hurt: bool,
}

/// A party member as Sora's team-attack code sees it (stream D seam).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Friend {
    /// `ETresChrUniqueID`: 11 Donald, 12 Goofy.
    pub uid: u8,
    /// Actor origin (feet), cm; yaw degrees.
    pub position: Vec3,
    pub yaw: f32,
    /// KO'd (`TresNpcState_Die`): no team command, no unison.
    pub ko: bool,
    /// Equipped copies of COMBI_TIME (197) and COMBI_UP (196) of this friend.
    pub combi_time: u8,
    pub combi_up: u8,
    /// A team command this friend offers now (`NotifySetActionCommand(kind, time, info = friend)`):
    /// (command kind, reception time). Sora's side takes it in the step it reads it.
    pub offer: Option<(i32, f32)>,
    /// The friend's side of a running team attack (link pawn and FR NPC state).
    pub link: FriendLinkView,
}

/// What the friends' side tells Sora's FR state about the running team attack.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct FriendLinkView {
    /// The command kind of the running team attack (0 = none).
    pub kind: i32,
    /// The link pawn's own step counter (for traces), and "the link pawn has finished" (the Sora
    /// state's sub 1 waits for it; `m_pObj` gone / its end flag).
    pub phase: u8,
    pub finished: bool,
    /// Where the link pawn holds Sora (attached / carried): actor origin and yaw; None = Sora moves
    /// himself.
    pub sora: Option<(Vec3, f32)>,
    /// The anim-set row the link pawn makes Sora play now (None = the state's own choice).
    pub sora_motion: Option<&'static str>,
    /// The friend-manager camera target (`UTresFriendComponent::m_pCameraTargetActor`) location and
    /// yaw, for the state-event cameras (`GetCameraTargetActor`).
    pub camera_target: Option<(Vec3, f32)>,
    /// The link pawn's actor location and yaw (`GetSpecificActionAccompanyPawn(0)`), for the cameras.
    pub accompany: Option<(Vec3, f32)>,
    /// A `SpecificAction(action, 0)` the link pawn sends on Sora's pawn this step (pawn vt+0x1038:
    /// 78 / 79 the meteor cameras, 87 Flare Force's, 68 / 69 Trinity Guard's finish): Sora's FR
    /// state emits it as `EventKind::PlayerSpecificAction`.
    pub sora_action: Option<u8>,
    /// Hits of the link pawn and its projectiles this step, delivered as SORA'S shots (the team
    /// rows are rows of `p_ex001_AttackData`: FR_CALLMETEOR_FINISH, FR_FLAREFORCE_FINISH01 ...).
    pub shots: [Option<TeamShot>; 4],
}

/// One hit sphere of a team attack (spawned by Sora's FR state through the combat hit code).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeamShot {
    /// The attack-data row (`p_ex001_AttackData`).
    pub row: &'static str,
    pub position: Vec3,
    /// Radius, growth per second, maximum (cm); life (s).
    pub radius: f32,
    pub grow: f32,
    pub max: f32,
    pub life: f32,
    /// The particle to show at it ("" = none).
    pub effect: &'static str,
}

/// Sora's side -> the friends' side (relayed by the host, stream D seam).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FriendLinkRequest {
    /// Every step: ctrl slot 896 "the friend gauge is full", the battle flag, Sora's transform.
    Sora { gauge_full: bool, in_battle: bool, position: Vec3, yaw: f32 },
    /// The command `kind` was executed: the FR state of Sora begins; the friends' FR states and
    /// the link pawn start (`FUN_14127A780(friend manager, kind)`); `target` = Sora's lock-on.
    Start { kind: i32, target: TargetId },
    /// A button of the team attack's own command menu or a timing press (Trinity Guard: 0 Shield
    /// Bash, 1 Jump, 2 cancel; others: 0 = the Triangle / attack press).
    Action { kind: i32, action: u8 },
    /// Every step of a running team attack: the player's input the FR state passes on to the link
    /// pawn (Trinity Guard 0x141377AC0: the stick as a world direction x magnitude -> link pawn
    /// vt+0x2600, the attack press -> vt+0x27D0 Shield Bash, the jump press -> vt+0x2618 Jump).
    /// (D4) `target`: the player controller's current target (vt+0x1690 = ctrl+0x994, soft or
    /// locked) this step: the link pawns re-read it every tick (FUN_1412AC290 0x1412E72B0, the
    /// GoofyShoot throw 0x1412DCE50 case 5).
    Input { kind: i32, stick: Vec3, attack: bool, jump: bool, target: TargetId },
    /// Sora's FR state ended (`End`): the friends leave their FR states.
    End { kind: i32, interrupted: bool },
    /// The unison / fusion finisher `kind` (0x3F..0x43) begins with the friend `uid`.
    Unison { uid: u8, kind: u8, target: TargetId },
}

/// See [`Targets::finish_kind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FinishKind {
    #[default]
    None,
    /// The last enemy of an ordinary fight (a port option; KH3 has no such effect).
    Battle,
    /// A boss: KH3's boss-finish presentation.
    Boss,
}

/// A pawn of Sora's own team.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ally {
    pub id: TargetId,
    /// Its root position.
    pub position: Vec3,
    /// It is in the DIE state (a heal on it is halved, rounded up).
    pub dead: bool,
}

/// What the damage function reads from a defender's status object (docs/sora-stats-spec.md
/// section 7, `UTresChrBaseParam`): filled by a host that has these numbers for its targets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetDefense {
    /// Defense (`m_DefensePower`-derived, 0x140581830).
    pub defense: i32,
    pub max_hp: i32,
    /// HP percent 0..100.
    pub hp_percent: f32,
    /// True for the game's enemy kinds (`bp+0x7C` 1 or 5): the battle-level damage floor applies.
    pub enemy_kind: bool,
    /// `m_DamageMin` / `m_DamageMax` of the base row (0 / 0 = none).
    pub damage_min: i32,
    pub damage_max: i32,
    /// Attribute resist percent by `ETresDamageAttribute` value 0..7 (100 = immune, negative = weak).
    pub resist: [i32; 8],
    /// Biological type: 1 Heartless, 2 Nobody, 0 otherwise.
    pub bio_type: u8,
}

/// No targets at all.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullTargets;

impl Targets for NullTargets {
    fn enumerate(&self, _out: &mut Vec<TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
    fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
        HitResult::default()
    }
}
