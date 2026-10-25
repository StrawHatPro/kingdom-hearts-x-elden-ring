//! What a hit does when it meets a guard, on Sora's side: the classification of a hit on him
//! against his body state (combat strand C 8.2.6 through `classify.rs`), the hand-off of the
//! guarded results (strand E 8.4: results 3 / 4 / 5 -> request interface +0x448 / +0x458 /
//! +0x470), the forced counter attack kinds 0x21 / 0x23 / 0x24 / 0x25 and the risk-dodge counters
//! 0x28 - 0x2B (strand A 7.2), and the blow counter (strand E 9.5).
//!
//! # THE HOOK BETWEEN THE COMBAT AND THE ACTIONS IMPLEMENTER
//!
//! The Guard state (15), its guard window, the counter window (controller+0x8A0, 1.0 s), the
//! just-guard timer (controller+0x89C, 0.1333 s), Dodge Roll, Risk Dodge (17) and Air Slide (20)
//! are the ACTIONS implementer's (`actions.rs`, `states/guard.rs` ...). The two sides meet in
//! plain data on the pawn:
//!
//! | who writes | field | combat reads it for |
//! |---|---|---|
//! | actions | `actions.reaction`, `actions.guard_parts` | the body Sora presents to a hit (`classify_on_sora`: the full 33-row table, with the row facts `HitOnPlayer` now carries: ignore guard, weak-guard attack, reflect level / reaction, rapid fire) |
//! | actions | `actions.counter_window`, `counter_command`, `counter_attacker`, `critical` | `CanAttack` / `TryAttack` out of GUARD: the forced kind `guard_counter_kind` (0x24 / 0x23 / 0x21), the just-guard bit; the window is closed with `actions::close_counter_window`; block W (Idle, Run ...) upgrades the attack and closes it. The HUD prompt of the window is mirrored into the situation slots (`mirror_prompts`) |
//! | actions | `actions.reaction_command` (Aerial Recovery: `(2, seconds)`) | the reaction command in its single slot for as long as it is `Some`, and the blow counter (forced kind 0x27) for an attack out of AIR_SLIDE |
//! | actions (Risk Dodge, when it exists) | `combat.guard.offer` | a forced counter kind (0x28 - 0x2B) offered by the running state; consumed by `TryAttack`, which sets `combat.guard.counter_used` |
//! | combat | `combat.guard.guarded` | the latest guarded hit with BOTH results (the attacker's recoil result is not in `PlayerGuarded`) |
//!
//! A guarded hit (victim result GUARD) is handed to `states::guard::on_guarded_hit` (theirs),
//! after the one branch that is combat's: a row with `m_ReflectLevel` >= 3 breaks the guard into
//! the weapon recoil (`states/reflect.rs`, written here from actions II.1 because both recoil
//! paths of combat need it now; its Risk Dodge parts are marked for the actions implementer).

use glam::Vec3;

use super::classify::{classify, Classified, MapResponse, Shape};
use crate::enums::{ETresAbilityKind, ETresCommandKind};
use crate::host::{BodyReaction, HitInfo, ReactionResult, TargetBody, TargetId};
use crate::pawn::{Ctx, Pawn};
use crate::states::{HitOnPlayer, StateId};

/// The counter-allowed gate (request interface slot 138 0x141429CF0, verdict differs; the state
/// list is the function's jump table, read in the decompiled C; combat spec 5.6.3): may the
/// guard reaction (slot 140 0x141444C30: a successful guard-hit entry with the 1.0 s counter
/// window) replace what he is doing?
///
/// ```text
/// current state in {12..16 ATTACK, ATTACK_AQ, ATTACK_RI, GUARD, DODGEROLL; 19 DAMAGE; 20 AIR_SLIDE;
///                   75..85, 87..90 the COMBO_* states; 156, 157; 159..161}: go on
/// 137 FR_BAYMAXRIDE: false
/// any other state: only with bAuto and the automatic-guard test ((iface+0x10) vt+0x2D0)   // not ported: false
/// then: weapon = pawn.vt+0x12C8(-1) (equipment vt+0x428); weapon in {1, 2, 4, 8..19}: true
///       TresCmdData[controller.form_cmd (+0xB38)].Param1 == 0 (no form): true;  else false
/// ```
///
/// The equipment's vt+0x428 was not read: in a form (Param1 != 0: Second Form) the answer of the
/// weapon test is not known and is TAKEN AS TRUE for the Kingdom Key (probe request).
pub(crate) fn reaction_allowed(state: StateId, _pawn: &Pawn) -> bool {
    matches!(state.value(), 12..=16 | 19 | 20 | 75..=85 | 87..=90 | 156 | 157 | 159..=161)
}

/// A forced counter attack the running state offers (strand A 7.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CounterOffer {
    /// The state that offers it; the offer only counts while that state runs.
    pub source: StateId,
    /// The forced attack kind (Attack state +0x1E8): 0x21 / 0x23 / 0x24 / 0x25 guard counters,
    /// 0x27 blow counter, 0x28 - 0x2B risk-dodge counters. 0 = no forced kind (block W of strand
    /// A 3: the window only upgrades the attack to a just-guard attack).
    pub kind: u8,
    /// The guarded / dodged attacker's lock-on target.
    pub target: TargetId,
    /// Attack +0x23C bit 0: a just-guard attack (damage rule of spec 8.3).
    pub just_guard: bool,
    /// True: `PushState` behind the offering state (Risk Dodge, Air Slide); false: `SetState`
    /// (out of Guard).
    pub push: bool,
}

/// A guarded hit on Sora: the arguments of `OnGuardedHit(info, _, causer, hitDir)` (request
/// interface slot 142 0x14143CAF0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuardedHit {
    pub hit: HitOnPlayer,
    /// The hit direction, flattened and normalised (zero when shorter than 1e-4).
    pub direction: Vec3,
    /// The attacker's result (`info+0x61`): REFLECT / GUARD_REFLECT = the attacker recoils.
    pub attacker: ReactionResult,
    /// Which part took it: 3 `Guard`, 4 `GuardBody`, 0 the body itself.
    pub part: BodyReaction,
    pub step: u64,
}

/// See the module text.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct GuardSeam {
    pub offer: Option<CounterOffer>,
    pub guarded: Option<GuardedHit>,
    pub counter_used: bool,
    /// The reaction commands this module put into the situation slots (`mirror_prompts`): the
    /// counter window's command and the aerial recovery's.
    shown: [i32; 2],
}

/// The guard counter's forced kind (`TryAttack` 0x141437E28 ...; strand A 7.2, sheet row
/// `counter.forced_kind.default`): 0x24 with REVENGEDIVE (6), else 0x23 with REVENGEIMPACT (5),
/// else 0x21. (0x25, the barrier cracker, belongs to the formchange kinds 0x86, 0x87, 0x8E, 0x8F,
/// 0x92, 0x93, 0xDD - 0xE0: never with the Kingdom Key.)
pub fn guard_counter_kind(s: &crate::status::AbilitySet) -> u8 {
    if s.has(ETresAbilityKind::Revengedive) {
        0x24
    } else if s.has(ETresAbilityKind::Revengeimpact) {
        0x23
    } else {
        super::tables::P.counter_forced_kind_default as u8
    }
}

/// The counter command id of the same order (controller+0x8A4: 5 / 4 / 3).
pub fn guard_counter_command(s: &crate::status::AbilitySet) -> i32 {
    match guard_counter_kind(s) {
        0x24 => ETresCommandKind::Revengedive.value(),
        0x23 => ETresCommandKind::Revengeimpact.value(),
        _ => ETresCommandKind::Revengeslash.value(),
    }
}

/// The reaction command id of a risk-dodge counter kind (RiskDodge +0xDC / +0xE0: (7, 0x28),
/// (8, 0x29), (9, 0x2A), (10, 0x2B)).
pub fn risk_dodge_command(kind: u8) -> i32 {
    match kind {
        0x29 => 8,
        0x2A => 9,
        0x2B => 10,
        _ => 7,
    }
}

/// Which hurt shape of Sora a hit from `from` meets, as its reaction type (actions II.1 "Guard
/// direction"): with the guard parts on, `Guard` (GUARD) is the outer surface within 120 degrees
/// of his facing on either side and `GuardBody` (WEAK_GUARD) in the rear wedge. Which primitive
/// the game's sweep reports when both overlap was not read: this is the geometric reading.
pub fn part_hit(reaction: BodyReaction, guard_parts: bool, own: Vec3, yaw: f32, from: Vec3) -> BodyReaction {
    if !guard_parts {
        return reaction;
    }
    let d = (from - own).truncate();
    if d.length_squared() < 1.0e-8 {
        return BodyReaction::Guard;
    }
    let a = yaw.to_radians();
    let cos = d.normalize().dot(glam::Vec2::new(a.cos(), a.sin()));
    // 1 + 2 cos(theta) > 0.
    if 1.0 + 2.0 * cos > 0.0 { BodyReaction::Guard } else { BodyReaction::WeakGuard }
}

/// The classification of a hit on Sora (strand C 8.2.6 with Sora as the victim). A host that
/// resolved the hit itself says so with `HitOnPlayer::reaction_type` != 2: that value is then
/// the victim result and the attacker result is NOREACTION.
pub(crate) fn classify_on_sora(s: &Pawn, hit: &HitOnPlayer) -> (Classified, BodyReaction, HitInfo) {
    let mut info = HitInfo { reflect_level: hit.reflect_level, reflect_reaction: hit.reflect_reaction, ..HitInfo::DEFAULT };
    if hit.rapid_fire {
        info.flags50 |= HitInfo::FLAG50_RAPID_FIRE;
    }
    if hit.ignore_guard {
        info.flags50 |= HitInfo::FLAG50_IGNORE_GUARD;
    }
    if hit.weak_guard {
        info.flags51 |= HitInfo::FLAG51_WEAK_GUARD;
    }
    if hit.reaction_type != 2 {
        let victim = match hit.reaction_type {
            0 => ReactionResult::None,
            1 => ReactionResult::NoReaction,
            3 => ReactionResult::Reflect,
            4 => ReactionResult::GuardReflect,
            5 => ReactionResult::Guard,
            6 => ReactionResult::ArmorBreak,
            _ => ReactionResult::Damage,
        };
        let c = Classified { victim, attacker: ReactionResult::NoReaction, no_react: victim == ReactionResult::None, recorded: true, rule: 0 };
        return (c, BodyReaction::from_value(s.actions.reaction), info);
    }
    let part = part_hit(BodyReaction::from_value(s.actions.reaction), s.actions.guard_parts, s.cmc.location, s.yaw, hit.causer_position);
    // (Sora wears no armour; no attribute is his weak point or nullified; the automatic-guard
    // option `ai.slot89()` is off.)
    let body = TargetBody { reaction: part, ..TargetBody::DEFAULT };
    let shape = Shape { no_clash: hit.reflect_level <= 0, victim_may_refuse: false, reflect_window: false, p7: false };
    let c = classify(&body, MapResponse::Other, &shape, hit.kind, hit.attribute, &mut info, &mut |_, _| 0, &mut |_| 0);
    (c, part, info)
}

/// A hit on Sora came out GUARD (`TakeDamage`: R 5 -> interface +0x470 `OnGuardedHit(info, _,
/// causer, hitDir)`, slot 142 0x14143CAF0). The branch combat owns comes first: a row with
/// `m_ReflectLevel` >= 3 (0x14143CDEE) breaks the guard into the weapon recoil (Reflect, flags 9,
/// speed 400, the direction from the hit). Everything else is the Guard state's hit entry.
pub(crate) fn guarded(state: &mut crate::states::StateData, cx: &mut Ctx, hit: &HitOnPlayer, attacker: ReactionResult, part: BodyReaction) {
    let flat = Vec3::new(hit.direction.x, hit.direction.y, 0.0);
    let direction = if flat.length_squared() < 1.0e-8 { Vec3::ZERO } else { flat.normalize() };
    cx.pawn.combat.guard.guarded = Some(GuardedHit { hit: *hit, direction, attacker, part, step: cx.pawn.step });
    if hit.reflect_level >= 3 {
        cx.pawn.actions.hits_guarded += 1;
        cx.pawn.event(crate::host::EventKind::PlayerGuarded { hit: *hit, success: false, critical: false });
        crate::states::reflect::request(cx, hit.causer, hit.causer_position, direction, true);
        return;
    }
    if !crate::states::guard::on_guarded_hit(state, cx, hit) {
        // Its gate (slot 141) refused: no HP loss, no reaction (R 5 applies nothing).
        cx.pawn.actions.hits_guarded += 1;
        cx.pawn.event(crate::host::EventKind::PlayerGuarded { hit: *hit, success: false, critical: false });
    }
}

/// The reaction prompts of the controller as situation-slot entries (strand F 11.1: reaction
/// commands reach the HUD as ex-kind entries through 0x141575640; their lifetime is the
/// controller's timer, not a list countdown): the counter window's command (`FUN_1414145C0`:
/// `HUD 0x14158C180(hud, 0, id, 0)`) and the aerial recovery's (`FUN_141414350(ctrl, kind, 2.0
/// s, 0)`: BLOWCOUNTER). Called every UI tick.
pub(crate) fn mirror_prompts(cx: &mut Ctx) {
    let a = &cx.pawn.actions;
    let wanted = [if a.counter_window > 0.0 { (a.counter_command, a.counter_window) } else { (0, 0.0) }, a.reaction_command.filter(|c| c.1 > 0.0).unwrap_or((0, 0.0))];
    for (k, (kind, time)) in wanted.into_iter().enumerate() {
        let shown = cx.pawn.combat.guard.shown[k];
        if shown != kind {
            if shown != 0 {
                super::situation::clear_reaction(cx, shown);
            }
            if kind != 0 {
                super::situation::set_reaction(cx, kind, time);
            }
            cx.pawn.combat.guard.shown[k] = kind;
        }
    }
}

/// The forced attack an attack request out of state `id` gets, if any (strand A 7.2; E 9.5).
/// Consumes what it uses.
pub(crate) fn forced_attack(id: StateId, s: &mut Pawn) -> Option<CounterOffer> {
    use crate::actions;
    match id {
        // GUARD with the counter window open: 0x24 / 0x23 / 0x21, the guarded attacker, the
        // just-guard bit from controller+0x878 bit 8; the window closes.
        StateId::Guard if s.actions.counter_window > 0.0 => {
            let offer = CounterOffer { source: id, kind: guard_counter_kind(&s.status.abilities), target: s.actions.counter_attacker, just_guard: s.actions.critical, push: false };
            actions::close_counter_window(s);
            Some(offer)
        }
        // Block W of strand A 3: the open window upgrades an attack out of these states to a
        // just-guard attack and closes; no forced kind.
        StateId::Idle | StateId::Run | StateId::Dodgeroll | StateId::Glide | StateId::Floating if s.actions.counter_window > 0.0 => {
            let offer = CounterOffer { source: id, kind: 0, target: TargetId::NONE, just_guard: s.actions.critical, push: false };
            actions::close_counter_window(s);
            Some(offer)
        }
        // AIR_SLIDE (aerial recovery with BLOW_COUNTER: AirSlide+0xAC = 2, +0xB0 = 0x27).
        StateId::AirSlide if s.actions.reaction_command.is_some_and(|c| c.0 == super::tables::P.recovery_blow_counter_command as i32) => {
            let target = s.actions.last_attacker.map_or(TargetId::NONE, |a| a.0);
            s.actions.reaction_command = None;
            Some(CounterOffer { source: id, kind: super::tables::P.recovery_blow_counter_attack_kind as u8, target, just_guard: false, push: true })
        }
        // A state's own offer (Risk Dodge: kinds 0x28 - 0x2B).
        _ => match s.combat.guard.offer {
            Some(offer) if offer.source == id => {
                s.combat.guard.offer = None;
                s.combat.guard.counter_used = true;
                Some(offer)
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Actions II.1 "Guard direction": the Guard part covers 120 degrees either side of the facing.
    #[test]
    fn the_guard_part_faces_forward_and_the_weak_part_covers_the_rear_wedge() {
        let from = |deg: f32| Vec3::new(deg.to_radians().cos(), deg.to_radians().sin(), 0.0) * 200.0;
        assert_eq!(part_hit(BodyReaction::Default, true, Vec3::ZERO, 0.0, from(0.0)), BodyReaction::Guard);
        assert_eq!(part_hit(BodyReaction::Default, true, Vec3::ZERO, 0.0, from(119.0)), BodyReaction::Guard);
        assert_eq!(part_hit(BodyReaction::Default, true, Vec3::ZERO, 0.0, from(121.0)), BodyReaction::WeakGuard);
        assert_eq!(part_hit(BodyReaction::Default, true, Vec3::ZERO, 0.0, from(180.0)), BodyReaction::WeakGuard);
        assert_eq!(part_hit(BodyReaction::Default, true, Vec3::ZERO, 90.0, from(-20.0)), BodyReaction::Guard);
        // Without the parts the body's own type answers.
        assert_eq!(part_hit(BodyReaction::GhostInvincible, false, Vec3::ZERO, 0.0, from(0.0)), BodyReaction::GhostInvincible);
    }
}
