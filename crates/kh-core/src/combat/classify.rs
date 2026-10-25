//! The guard / armour / body classification of a hit: `Classify` 0x14061FE20 (combat strand C
//! 8.2.6, read whole with its jump table 0x1406202F4; the 33 rows of sheet
//! `sora-combat-reaction-rules`, each branch below names its row). It runs on the ATTACKER's side
//! before a hit is built and decides both results: what the victim plays (`info+0x60`) and what
//! the attacker gets (`info+0x61`).
//!
//! Used in both directions:
//! - Sora's hits on targets (`hit.rs`): the victim's answers come from `Targets::body`;
//! - hits on Sora (`guard.rs`): the answers come from his own body state, which the actions
//!   implementer's guard / dodge states keep on the pawn (`combat::guard::SoraBody`).
//!
//! A pure function: the two calls that change the victim (`ai.slot70` armour, `ai.slot32`
//! reaction endurance) are passed in.

use crate::enums::{ETresDamageAttribute, ETresDamageKind};
use crate::host::{BodyReaction, HitInfo, ReactionResult, TargetBody};

/// What the attack shape brings to the classification besides the hit record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Shape {
    /// prim+0x986 bit 7, "no clash": set unless the row's `m_ReflectLevel` > 0, and set again for
    /// every swing after the first of a combo (C 8.2.1). A guarded "no clash" hit does not repel.
    pub no_clash: bool,
    /// prim+0x984 bit 5: the victim may refuse the hit.
    pub victim_may_refuse: bool,
    /// The map-hit-reflect window is open (prim+0x987 bit 1).
    pub reflect_window: bool,
    /// `p7` of `Apply` (false in the sweep path).
    pub p7: bool,
}

/// The outcome: the two results are also written into the hit record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Classified {
    pub victim: ReactionResult,
    pub attacker: ReactionResult,
    /// `noReact`: `TakeDamage` is not called (the victim gets notify slot 28 instead).
    pub no_react: bool,
    /// `reg`: the victim goes into the window's hit list.
    pub recorded: bool,
    /// The row of `sora-combat-reaction-rules` that decided (1..=33), for tests and traces.
    pub rule: u8,
}

/// What a wall or prop answers (`A.m_DefaultAttackHitResponse`), for bodies without a hurt-shape
/// interface while the reflect window is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MapResponse {
    /// No actor at all (level geometry).
    #[default]
    NoActor,
    /// An actor whose default response is GUARD (1) or REFLECT (2).
    GuardOrReflect,
    Other,
}

/// `m_ReflectReaction` as the row names it.
fn reflect_reaction(info: &HitInfo) -> u8 {
    match info.reflect_reaction {
        "STAGGER" => 1,
        "GUARD" => 2,
        "NOREACTION" => 3,
        _ => 0,
    }
}

/// `Classify(comp, prim, A, C, p7, &info, &noReact)`.
///
/// `take_armor(power, weak) -> left` is `ai.slot70`, `take_reaction(power) -> left` is `ai.slot32`.
pub fn classify(body: &TargetBody, map: MapResponse, shape: &Shape, kind: ETresDamageKind, attribute: ETresDamageAttribute, info: &mut HitInfo, take_armor: &mut dyn FnMut(i32, bool) -> i32, take_reaction: &mut dyn FnMut(i32) -> i32) -> Classified {
    use ReactionResult as R;
    let mut out = Classified { victim: R::Damage, attacker: R::None, no_react: false, recorded: true, rule: 0 };
    let finish = |mut out: Classified, info: &mut HitInfo, rule: u8| {
        out.rule = rule;
        (info.victim_result, info.attacker_result) = (out.victim, out.attacker);
        out
    };
    // ---- no hurt-shape interface: a wall, a prop (rows 1 - 3)
    if !body.has_body {
        let rule;
        if !shape.p7 && shape.reflect_window {
            match map {
                MapResponse::NoActor | MapResponse::GuardOrReflect => {
                    out.attacker = R::Reflect;
                    rule = 2;
                }
                MapResponse::Other => {
                    out.attacker = R::NoReaction;
                    rule = 3;
                }
            }
        } else {
            out.attacker = R::NoReaction;
            rule = 1;
        }
        return finish(out, info, rule);
    }
    let bit = 1u8 << (attribute.value().clamp(0, 7) as u8);
    let (w, x) = (body.weak & bit != 0, body.nullified & bit != 0);
    let rapid = info.flags50 & HitInfo::FLAG50_RAPID_FIRE != 0;
    let g = rapid && body.rapid_fire_proof;
    let ignore_guard = info.flags50 & HitInfo::FLAG50_IGNORE_GUARD != 0;
    let weak_guard_row = info.flags51 & HitInfo::FLAG51_WEAK_GUARD != 0;
    // ---- row 4: the victim refuses
    if shape.victim_may_refuse && body.refuses {
        info.clear_bad_status();
        (out.victim, out.attacker) = (R::None, R::None);
        return finish(out, info, 4);
    }
    // ---- rows 5 - 6: armour
    if body.armoured {
        let ap = info.armor_attack_power;
        if ap > 0 || w {
            info.flags50 |= HitInfo::FLAG50_ARMOR_DAMAGE;
            let left = take_armor(ap, w);
            info.armor_left = left;
            if left <= 0 {
                (out.victim, out.attacker) = (R::ArmorBreak, R::NoReaction);
                return finish(out, info, 5);
            }
            // (Armour that holds falls through to the body type below.)
        } else {
            (out.victim, out.attacker, out.no_react) = (R::None, R::NoReaction, true);
            return finish(out, info, 6);
        }
    }
    // ---- the automatic guard: DEFAULT, WEAK_GUARD and an unweakened STRONG body turn into GUARD
    let mut t = body.reaction;
    if (t == BodyReaction::Default || t == BodyReaction::WeakGuard || (t == BodyReaction::Strong && !w && !x)) && kind != ETresDamageKind::Without && body.guarding {
        t = BodyReaction::Guard;
    }
    let grab = matches!(kind, ETresDamageKind::Catch | ETresDamageKind::Bind);
    // The two shared tails.
    let block = |mut out: Classified| {
        (out.victim, out.attacker, out.no_react) = (R::None, R::NoReaction, true);
        out
    };
    let guarded = |mut out: Classified, info: &mut HitInfo| {
        out.victim = R::Guard;
        if shape.no_clash || shape.p7 {
            out.attacker = R::NoReaction;
        } else {
            out.attacker = match reflect_reaction(info) {
                2 => R::GuardReflect,
                3 => R::NoReaction,
                _ => R::Reflect,
            };
        }
        info.clear_bad_status();
        out
    };
    let default_victim = |g_and_not_w: bool| if g_and_not_w { R::NoReaction } else { R::Damage };
    let rule = match t {
        BodyReaction::NoReaction => {
            out.attacker = R::NoReaction;
            if x {
                out = block(out);
                7
            } else if w {
                out.victim = R::Damage;
                8
            } else {
                info.flags51 |= HitInfo::FLAG51_NO_REACT_CORRECTION;
                out.victim = R::NoReaction;
                9
            }
        }
        BodyReaction::Strong => {
            out.attacker = R::NoReaction;
            if w {
                out.victim = R::Damage;
                10
            } else if x {
                (out.victim, out.no_react) = (R::None, true);
                11
            } else if g {
                out.victim = R::NoReaction;
                12
            } else {
                let rp = info.reaction_power;
                let left = take_reaction(rp);
                out.victim = if left > 0 {
                    if rp > 1 { R::StrongBodyDamage2 } else { R::StrongBodyDamage1 }
                } else if rp > 0 {
                    R::StrongBodyDamage4
                } else {
                    R::StrongBodyDamage3
                };
                13
            }
        }
        BodyReaction::Guard | BodyReaction::GuardNoReact => {
            if w || grab || ignore_guard {
                out.attacker = R::NoReaction;
                if t == BodyReaction::GuardNoReact {
                    out.victim = R::NoReaction;
                    15
                } else {
                    out.victim = R::Damage;
                    14
                }
            } else if x {
                out = block(out);
                16
            } else {
                out = guarded(out, info);
                17
            }
        }
        BodyReaction::WeakGuard => {
            if w {
                // "as default".
                out.attacker = R::NoReaction;
                out.victim = default_victim(false);
                20
            } else if x {
                out = block(out);
                18
            } else if grab || ignore_guard || !weak_guard_row {
                out.attacker = R::NoReaction;
                out.victim = default_victim(g);
                20
            } else {
                out = guarded(out, info);
                19
            }
        }
        BodyReaction::Invincible | BodyReaction::InvincibleNoReact => {
            if w {
                out.attacker = R::NoReaction;
                if t == BodyReaction::InvincibleNoReact {
                    out.victim = R::NoReaction;
                    22
                } else {
                    out.victim = R::Damage;
                    21
                }
            } else {
                info.clear_bad_status();
                out = block(out);
                23
            }
        }
        BodyReaction::Ghost | BodyReaction::GhostNoReact => {
            if w {
                out.attacker = R::NoReaction;
                if t == BodyReaction::GhostNoReact {
                    out.victim = R::NoReaction;
                    25
                } else {
                    out.victim = R::Damage;
                    24
                }
            } else {
                // Not even recorded.
                info.clear_bad_status();
                (out.victim, out.attacker, out.recorded) = (R::None, R::None, false);
                26
            }
        }
        BodyReaction::Reflect1 => {
            out.attacker = R::NoReaction;
            if w {
                out.victim = R::Damage;
                27
            } else if x {
                (out.victim, out.no_react) = (R::None, true);
                28
            } else if shape.no_clash {
                out.victim = if g { R::NoReaction } else { R::Damage };
                29
            } else {
                // The prim may clash.
                info.clear_bad_status();
                (out.victim, out.attacker) = (R::GuardReflect, R::GuardReflect);
                let d = 2 - info.reflect_level as i32;
                if d > 0 {
                    out.attacker = R::Reflect;
                } else if d < 0 {
                    out.victim = R::Reflect;
                }
                match reflect_reaction(info) {
                    1 => out.attacker = R::Reflect,
                    2 => out.attacker = R::GuardReflect,
                    3 => out.attacker = R::None,
                    _ => {}
                }
                30
            }
        }
        BodyReaction::GhostInvincible => {
            info.clear_bad_status();
            (out.victim, out.attacker) = (R::None, R::None);
            31
        }
        BodyReaction::Default => {
            if x {
                out = block(out);
                32
            } else {
                out.attacker = R::NoReaction;
                out.victim = default_victim(g && !w);
                33
            }
        }
    };
    finish(out, info, rule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::tables::REACTION_RULES;
    use ETresDamageAttribute::Physical;
    use ETresDamageKind::{Bind, Small, Without};
    use ReactionResult as R;

    fn body(reaction: BodyReaction) -> TargetBody {
        TargetBody { reaction, ..TargetBody::DEFAULT }
    }

    /// First swing of a combo with a reflect-level-1 row: the prim may clash.
    const FIRST: Shape = Shape { no_clash: false, victim_may_refuse: false, reflect_window: false, p7: false };
    const LATER: Shape = Shape { no_clash: true, victim_may_refuse: false, reflect_window: false, p7: false };

    fn run(body: &TargetBody, shape: &Shape, kind: ETresDamageKind, info: &mut HitInfo, armor_left: i32, reaction_left: i32) -> Classified {
        classify(body, MapResponse::NoActor, shape, kind, Physical, info, &mut |_, _| armor_left, &mut |_| reaction_left)
    }

    fn plain(body: &TargetBody, shape: &Shape) -> Classified {
        run(body, shape, Small, &mut HitInfo { reflect_level: 1, ..HitInfo::DEFAULT }, 0, 0)
    }

    // Strand C 8.2.6: every row of the sheet `sora-combat-reaction-rules`, in its order.
    #[test]
    fn the_33_rows_of_the_reaction_rule_table() {
        assert_eq!(REACTION_RULES.len(), 33, "the sheet this code was written from");
        let weak = |b: TargetBody| TargetBody { weak: 1, ..b };
        let null = |b: TargetBody| TargetBody { nullified: 1, ..b };
        let check = |c: Classified, rule: u8, victim: R, attacker: R, no_react: bool, recorded: bool| {
            assert_eq!((c.rule, c.victim, c.attacker, c.no_react, c.recorded), (rule, victim, attacker, no_react, recorded), "row {rule}: {}", REACTION_RULES[rule as usize - 1].condition);
        };
        // 1 - 3: no hurt-shape interface.
        let wall = TargetBody { has_body: false, ..TargetBody::DEFAULT };
        let open = Shape { reflect_window: true, ..FIRST };
        check(plain(&wall, &FIRST), 1, R::Damage, R::NoReaction, false, true);
        check(plain(&wall, &open), 2, R::Damage, R::Reflect, false, true);
        check(classify(&wall, MapResponse::Other, &open, Small, Physical, &mut HitInfo::DEFAULT, &mut |_, _| 0, &mut |_| 0), 3, R::Damage, R::NoReaction, false, true);
        // 4: the victim refuses.
        check(plain(&TargetBody { refuses: true, ..TargetBody::DEFAULT }, &Shape { victim_may_refuse: true, ..FIRST }), 4, R::None, R::None, false, true);
        // 5 - 6: armour.
        let armoured = TargetBody { armoured: true, ..TargetBody::DEFAULT };
        let mut info = HitInfo { armor_attack_power: 3, ..HitInfo::DEFAULT };
        check(run(&armoured, &FIRST, Small, &mut info, 0, 0), 5, R::ArmorBreak, R::NoReaction, false, true);
        assert!(info.flags50 & HitInfo::FLAG50_ARMOR_DAMAGE != 0);
        check(plain(&armoured, &FIRST), 6, R::None, R::NoReaction, true, true);
        // Armour that holds: the body type decides (here DEFAULT -> row 33).
        check(run(&armoured, &FIRST, Small, &mut HitInfo { armor_attack_power: 3, ..HitInfo::DEFAULT }, 7, 0), 33, R::Damage, R::NoReaction, false, true);
        // 7 - 9: NO_REACTION.
        let b = body(BodyReaction::NoReaction);
        check(plain(&null(b), &FIRST), 7, R::None, R::NoReaction, true, true);
        check(plain(&weak(b), &FIRST), 8, R::Damage, R::NoReaction, false, true);
        let mut info = HitInfo::DEFAULT;
        check(run(&b, &FIRST, Small, &mut info, 0, 0), 9, R::NoReaction, R::NoReaction, false, true);
        assert!(info.flags51 & HitInfo::FLAG51_NO_REACT_CORRECTION != 0);
        // 10 - 13: STRONG.
        let b = body(BodyReaction::Strong);
        check(plain(&weak(b), &FIRST), 10, R::Damage, R::NoReaction, false, true);
        check(plain(&null(b), &FIRST), 11, R::None, R::NoReaction, true, true);
        let rapid = HitInfo { flags50: HitInfo::FLAG50_RAPID_FIRE, ..HitInfo::DEFAULT };
        check(run(&TargetBody { rapid_fire_proof: true, ..b }, &FIRST, Small, &mut { rapid }, 0, 0), 12, R::NoReaction, R::NoReaction, false, true);
        let power = |rp: i32| HitInfo { reaction_power: rp, ..HitInfo::DEFAULT };
        assert_eq!(run(&b, &FIRST, Small, &mut power(1), 5, 5).victim, R::StrongBodyDamage1);
        assert_eq!(run(&b, &FIRST, Small, &mut power(2), 5, 5).victim, R::StrongBodyDamage2);
        assert_eq!(run(&b, &FIRST, Small, &mut power(0), 0, 0).victim, R::StrongBodyDamage3);
        check(run(&b, &FIRST, Small, &mut power(1), 0, 0), 13, R::StrongBodyDamage4, R::NoReaction, false, true);
        // 14 - 17: GUARD / GUARD_NOREACT.
        let (guard, guard_nr) = (body(BodyReaction::Guard), body(BodyReaction::GuardNoReact));
        check(plain(&weak(guard), &FIRST), 14, R::Damage, R::NoReaction, false, true);
        check(run(&guard, &FIRST, Bind, &mut HitInfo::DEFAULT, 0, 0), 14, R::Damage, R::NoReaction, false, true);
        check(run(&guard_nr, &FIRST, Small, &mut HitInfo { flags50: HitInfo::FLAG50_IGNORE_GUARD, ..HitInfo::DEFAULT }, 0, 0), 15, R::NoReaction, R::NoReaction, false, true);
        check(plain(&null(guard), &FIRST), 16, R::None, R::NoReaction, true, true);
        let mut info = HitInfo { bad_status: "STUN", bad_time: 8.0, ..HitInfo::DEFAULT };
        check(run(&guard, &FIRST, Small, &mut info, 0, 0), 17, R::Guard, R::Reflect, false, true);
        assert_eq!((info.bad_status, info.bad_time, info.victim_result, info.attacker_result), ("NONE", 0.0, R::Guard, R::Reflect), "a guard clears the bad status");
        // GUARDED: "no clash" (every swing after the first) -> no recoil; the row's reflect reaction.
        assert_eq!(plain(&guard, &LATER).attacker, R::NoReaction);
        assert_eq!(run(&guard, &FIRST, Small, &mut HitInfo { reflect_reaction: "GUARD", ..HitInfo::DEFAULT }, 0, 0).attacker, R::GuardReflect);
        assert_eq!(run(&guard, &FIRST, Small, &mut HitInfo { reflect_reaction: "NOREACTION", ..HitInfo::DEFAULT }, 0, 0).attacker, R::NoReaction);
        // 18 - 20: WEAK_GUARD.
        let b = body(BodyReaction::WeakGuard);
        let weak_row = HitInfo { flags51: HitInfo::FLAG51_WEAK_GUARD, ..HitInfo::DEFAULT };
        check(plain(&null(b), &FIRST), 18, R::None, R::NoReaction, true, true);
        check(run(&b, &FIRST, Small, &mut { weak_row }, 0, 0), 19, R::Guard, R::Reflect, false, true);
        check(plain(&b, &FIRST), 20, R::Damage, R::NoReaction, false, true);
        check(run(&TargetBody { rapid_fire_proof: true, ..b }, &FIRST, Small, &mut { rapid }, 0, 0), 20, R::NoReaction, R::NoReaction, false, true);
        // 21 - 23: INVINCIBLE.
        check(plain(&weak(body(BodyReaction::Invincible)), &FIRST), 21, R::Damage, R::NoReaction, false, true);
        check(plain(&weak(body(BodyReaction::InvincibleNoReact)), &FIRST), 22, R::NoReaction, R::NoReaction, false, true);
        check(plain(&body(BodyReaction::Invincible), &FIRST), 23, R::None, R::NoReaction, true, true);
        // 24 - 26: GHOST.
        check(plain(&weak(body(BodyReaction::Ghost)), &FIRST), 24, R::Damage, R::NoReaction, false, true);
        check(plain(&weak(body(BodyReaction::GhostNoReact)), &FIRST), 25, R::NoReaction, R::NoReaction, false, true);
        check(plain(&body(BodyReaction::Ghost), &FIRST), 26, R::None, R::None, false, false);
        // 27 - 30: REFLECT1.
        let b = body(BodyReaction::Reflect1);
        check(plain(&weak(b), &FIRST), 27, R::Damage, R::NoReaction, false, true);
        check(plain(&null(b), &FIRST), 28, R::None, R::NoReaction, true, true);
        check(plain(&b, &LATER), 29, R::Damage, R::NoReaction, false, true);
        // Clash on: level 1 < 2 -> the attacker is repelled; level 2 -> both GUARD_REFLECT; 4 -> the victim.
        check(plain(&b, &FIRST), 30, R::GuardReflect, R::Reflect, false, true);
        let level = |l: i8| HitInfo { reflect_level: l, ..HitInfo::DEFAULT };
        let c = run(&b, &FIRST, Small, &mut level(2), 0, 0);
        assert_eq!((c.victim, c.attacker), (R::GuardReflect, R::GuardReflect));
        let c = run(&b, &FIRST, Small, &mut level(4), 0, 0);
        assert_eq!((c.victim, c.attacker), (R::Reflect, R::GuardReflect));
        // 31: GHOST_INVINCIBLE.
        check(plain(&body(BodyReaction::GhostInvincible), &FIRST), 31, R::None, R::None, false, true);
        // 32 - 33: DEFAULT.
        check(plain(&null(TargetBody::DEFAULT), &FIRST), 32, R::None, R::NoReaction, true, true);
        check(plain(&TargetBody::DEFAULT, &FIRST), 33, R::Damage, R::NoReaction, false, true);
    }

    // Strand C 8.2.6, the line before the jump table: a guarding victim turns DEFAULT, WEAK_GUARD
    // and an unweakened STRONG body into GUARD, except against a WITHOUT hit.
    #[test]
    fn a_guarding_victim_turns_its_body_into_a_guard() {
        let guarding = |reaction: BodyReaction| TargetBody { guarding: true, ..body(reaction) };
        for reaction in [BodyReaction::Default, BodyReaction::WeakGuard, BodyReaction::Strong] {
            let c = plain(&guarding(reaction), &FIRST);
            assert_eq!((c.rule, c.victim, c.attacker), (17, R::Guard, R::Reflect), "{reaction:?}");
        }
        assert_eq!(plain(&TargetBody { weak: 1, ..guarding(BodyReaction::Strong) }, &FIRST).rule, 10, "a weakened STRONG body does not guard");
        assert_eq!(run(&guarding(BodyReaction::Default), &FIRST, Without, &mut HitInfo::DEFAULT, 0, 0).rule, 33);
        assert_eq!(plain(&guarding(BodyReaction::NoReaction), &FIRST).rule, 9);
        // The Kingdom Key (C 8.2.6, last paragraph): only the first swing is repelled.
        assert_eq!(plain(&guarding(BodyReaction::Default), &LATER).attacker, R::NoReaction);
    }
}
