//! (GF-AI) AI Goofy, one test per rule of documentation/GOOFY_OVERHAUL.md 12 and the lane's brief. The brain's choices first, then
//! a party with no tree (the buttons and the one judgement of a hit); the trees' side is kh-partyhost tests/gf_ai.rs.

use super::*;
use crate::command::{CommandStatus, LocomotionDef};
use crate::goofy::{GoofyEvent, RESOLVE_FULL};
use crate::host::PartyHost;
use crate::states::NpcStateId;
use crate::{ActionDef, Party, PartyConfig, SoraView};
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{AttackInfo, HitAttacker, HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetInfo, Targets};
use kh_core::test_world::TestWorld;

const DT: f32 = 1.0 / 60.0;
const T: TargetId = TargetId(1);
const NEAR: Vec3 = Vec3::new(200.0, 0.0, 0.0);
/// In reach of a swing at him, out of reach of his string.
const APART: Vec3 = Vec3::new(400.0, 0.0, 0.0);

fn base<'a>(enemies: &'a [AiEnemy], friends: &'a [AiFriend], telegraphs: &'a [Telegraph]) -> AiInput<'a> {
    AiInput { pos: Vec3::ZERO, level: 99, ranks: RankBank::default(), resolve: Resolve::default(), stance: Stance::Free, can_x: true, can_guard: true, guard: 1.0, hop_cd: 0.0, in_battle: true, breaker: false, combo: 0, end_lag: false, arts: [None, None], art_cost: [0, 0], interpose_ready: true, mp: 100, mp_max: 100, style: AiSettings { battle: 0, ability: 0, cure: 0, cure_targets: 7 }, rebound: None, asked: None, art_asked: None, enemies, friends, telegraphs }
}

fn foe(id: u32, x: f32) -> AiEnemy {
    AiEnemy { id: TargetId(id), pos: Vec3::new(x, 0.0, 0.0), radius: 50.0, stagger: 0.0, broken: 0.0 }
}

fn lead(hp: i32, foes: u32) -> AiFriend {
    AiFriend { member: SORA_MEMBER, pos: Vec3::new(-100.0, 0.0, 0.0), hp, hp_max: 100, ko: false, leader: true, foes }
}

fn no_luck() -> f32 {
    0.5
}

// ------------------------------------------------------------------------------------------------ (GF-POLISH) who is targeted

fn aim(id: u32, on: Option<MemberId>) -> EnemyAim {
    EnemyAim { enemy: TargetId(id), on }
}

fn friend_at(member: u8, x: f32) -> AiFriend {
    AiFriend { member: MemberId(member), pos: Vec3::new(x, 0.0, 0.0), hp: 100, hp_max: 100, ko: false, leader: false, foes: 0 }
}

#[test]
fn a_read_aim_beats_the_estimate_either_way() {
    // both enemies stand by friend 1, but their aims are on friend 2, a long way off
    let enemies = [foe(1, 900.0), foe(2, 950.0)];
    let mut friends = [friend_at(1, 1000.0), friend_at(2, -2000.0)];
    count_foes(&mut friends, Vec3::ZERO, &enemies, &[aim(1, Some(MemberId(2))), aim(2, Some(MemberId(2)))]);
    assert_eq!((friends[0].foes, friends[1].foes), (0, 2));
}

#[test]
fn an_aim_at_him_or_at_nobody_of_the_party_counts_for_no_friend() {
    let enemies = [foe(1, 900.0), foe(2, 950.0)];
    let mut friends = [friend_at(1, 1000.0)];
    count_foes(&mut friends, Vec3::ZERO, &enemies, &[aim(1, Some(MemberId(9))), aim(2, None)]);
    assert_eq!(friends[0].foes, 0);
}

#[test]
fn an_enemy_with_no_read_aim_falls_back_on_the_estimate() {
    let enemies = [foe(1, 900.0), foe(2, 950.0)];
    let mut read = [friend_at(1, 1000.0)];
    count_foes(&mut read, Vec3::ZERO, &enemies, &[aim(1, Some(MemberId(1)))]);
    assert_eq!(read[0].foes, 2, "one read, one estimated");
    // the test level reads none at all: the estimate alone, as before
    let mut none = [friend_at(1, 1000.0)];
    count_foes(&mut none, Vec3::ZERO, &enemies, &[]);
    assert_eq!(none[0].foes, 2);
}

#[test]
fn an_aim_at_a_friend_who_is_down_counts_for_nobody() {
    let enemies = [foe(1, 900.0)];
    let mut friends = [AiFriend { ko: true, ..friend_at(1, 1000.0) }];
    count_foes(&mut friends, Vec3::ZERO, &enemies, &[aim(1, Some(MemberId(1)))]);
    assert_eq!(friends[0].foes, 0);
}

// ------------------------------------------------------------------------------------------------ the technique (4.3)

#[test]
fn he_picks_the_technique_for_the_class_among_those_he_has_learned() {
    let i = base(&[], &[], &[]);
    let want = [(AttackClass::Normal, Answer::Deflect), (AttackClass::Thrust, Answer::Pinning), (AttackClass::Sweep, Answer::JumpOver), (AttackClass::Grab, Answer::Slip), (AttackClass::Heavy, Answer::Brace), (AttackClass::Projectile, Answer::Reflect), (AttackClass::Magic, Answer::Ward)];
    for (class, answer) in want {
        assert_eq!(choose(class, &i), answer, "{class:?} at level 99");
    }
}

#[test]
fn no_learned_move_cheating_a_low_level_goofy_only_has_deflects_and_dodges() {
    let mut i = base(&[], &[], &[]);
    i.level = 1;
    let want = [(AttackClass::Normal, Answer::Deflect), (AttackClass::Thrust, Answer::Deflect), (AttackClass::Sweep, Answer::Deflect), (AttackClass::Grab, Answer::Dodge), (AttackClass::Heavy, Answer::Dodge), (AttackClass::Projectile, Answer::Deflect), (AttackClass::Magic, Answer::Deflect)];
    for (class, answer) in want {
        assert_eq!(choose(class, &i), answer, "{class:?} at level 1");
    }
    // each comes at its own level (11.1)
    for (class, tech, answer) in [(AttackClass::Projectile, Tech::ShieldReflect, Answer::Reflect), (AttackClass::Sweep, Tech::HopCounter, Answer::JumpOver), (AttackClass::Thrust, Tech::PinningGuard, Answer::Pinning), (AttackClass::Grab, Tech::SlipCounter, Answer::Slip), (AttackClass::Heavy, Tech::BullBrace, Answer::Brace), (AttackClass::Magic, Tech::SpellWard, Answer::Ward)] {
        let from = tech.mv().learn_level();
        i.level = from - 1;
        assert_ne!(choose(class, &i), answer, "{tech:?} one level early");
        i.level = from;
        assert_eq!(choose(class, &i), answer, "{tech:?} at its level {from}");
    }
}

#[test]
fn a_banked_boss_rank_does_not_teach_him_a_move_early() {
    let mut i = base(&[], &[], &[]);
    i.level = 10;
    let mut parts: Vec<u64> = i.ranks.parts().iter().map(|v| *v as u64).collect();
    let n = GoofyMove::ALL.iter().position(|m| *m == GoofyMove::PinningGuard).unwrap();
    parts[n] = 2;
    i.ranks = RankBank::from_parts(&parts);
    assert_eq!(choose(AttackClass::Thrust, &i), Answer::Deflect);
}

#[test]
fn he_keeps_resolve_up_with_safe_deflects() {
    let mut i = base(&[], &[], &[]);
    i.resolve.thirds = 3;
    assert_eq!(choose(AttackClass::Sweep, &i), Answer::JumpOver, "one pip: the jump is worth it");
    i.resolve.thirds = 6;
    assert_eq!(choose(AttackClass::Sweep, &i), Answer::Deflect, "two pips: the safe deflect");
    i.resolve.thirds = RESOLVE_FULL;
    for class in [AttackClass::Thrust, AttackClass::Grab, AttackClass::Heavy, AttackClass::Magic] {
        assert_eq!(choose(class, &i), Answer::Deflect, "In the Zone every deflect is the right one");
    }
}

#[test]
fn a_mistimed_answer_is_the_shield_too_soon_and_an_unblockable_lands() {
    let mut i = base(&[], &[], &[]);
    assert_eq!(mistimed(Answer::Pinning, &i), Answer::Block);
    assert_eq!(mistimed(Answer::Slip, &i), Answer::Stand);
    assert_eq!(mistimed(Answer::Brace, &i), Answer::Stand);
    i.guard = 0.1;
    assert_eq!(mistimed(Answer::Deflect, &i), Answer::Dodge, "GUARD too low to block: the hop");
    i.hop_cd = 0.5;
    assert_eq!(mistimed(Answer::Deflect, &i), Answer::Stand, "nothing left: he takes the small hit rather than break");
}

#[test]
fn an_answer_that_no_longer_fits_the_time_left_falls_back() {
    let i = base(&[], &[], &[]);
    assert_eq!(fit(Answer::Ward, 0.1, &i), Answer::Deflect);
    assert_eq!(fit(Answer::Ward, 0.6, &i), Answer::Ward);
    assert_eq!(fit(Answer::JumpOver, 0.0, &i), Answer::Deflect);
    let mut cold = base(&[], &[], &[]);
    cold.hop_cd = 0.9;
    assert_eq!(fit(Answer::Slip, 0.2, &cold), Answer::Stand);
}

// ------------------------------------------------------------------------------------------------ the rates

#[test]
fn the_rate_grows_with_level_and_is_good_not_perfect() {
    assert!((parry_rate(1) - K.ai_parry_rate_low).abs() < 1e-5);
    assert!((parry_rate(99) - K.ai_parry_rate_high).abs() < 1e-5);
    assert!(parry_rate(30) > parry_rate(10) && parry_rate(70) > parry_rate(30));
    assert!(parry_rate(99) < 1.0, "never a machine");
    assert!(read_delay(99) > 0.0 && read_delay(1) > read_delay(99));
}

// ------------------------------------------------------------------------------------------------ the aggro rules (12)

#[test]
fn he_taunts_when_a_friend_has_two_enemies_on_him() {
    let enemies = [foe(1, -300.0), foe(2, -350.0)];
    let friends = [lead(100, 2)];
    let i = base(&enemies, &friends, &[]);
    let mut mem = AiMemory::new(MemberId(1));
    let out = think(&i, &mut mem, DT, &mut no_luck);
    assert!(out.pressed.has(PadButton::LeftShoulder) && out.pressed.has(PadButton::FaceLeft), "LB + X");
    assert_eq!(mem.last, Some(AiWord::Taunt));
    let again = think(&i, &mut mem, DT, &mut no_luck);
    assert!(!again.pressed.has(PadButton::LeftShoulder), "the Taunt has its cooldown");
    let friends = [lead(100, 1)];
    let i = base(&enemies, &friends, &[]);
    let mut mem = AiMemory::new(MemberId(1));
    assert!(!think(&i, &mut mem, DT, &mut no_luck).pressed.has(PadButton::LeftShoulder), "one enemy is not enough");
}

#[test]
fn enemies_count_on_the_friend_they_stand_nearest_to() {
    let party = Party::new(PartyConfig::test_level99());
    let sora = SoraView { position: Vec3::new(1000.0, 0.0, 0.0), hp: 100, hp_max: 100, ..SoraView::default() };
    let enemies = [foe(1, 1200.0), foe(2, 900.0), foe(3, 100.0)];
    let f = friends_of(&party, MemberId(5), Vec3::ZERO, &sora, &enemies);
    assert_eq!(f[0].foes, 2, "the third stands by him: it is not on the leader");
}

#[test]
fn he_interposes_when_the_leader_is_low_once_he_has_learned_it() {
    let enemies = [foe(1, -300.0)];
    let friends = [lead(20, 1)];
    let mut i = base(&enemies, &friends, &[]);
    i.arts = [None, Some(GoofyMove::Interpose)];
    let mut mem = AiMemory::new(MemberId(1));
    let out = think(&i, &mut mem, DT, &mut no_luck);
    assert_eq!(out.art, Some(GoofyMove::Interpose));
    assert_eq!(mem.last, Some(AiWord::Interpose));
    // not learned yet: a level 1 Goofy has nothing
    i.level = 1;
    let mut mem = AiMemory::new(MemberId(1));
    assert_ne!({ think(&i, &mut mem, DT, &mut no_luck); mem.last }, Some(AiWord::Interpose));
    // the leader is fine
    let friends = [lead(90, 1)];
    let mut i = base(&enemies, &friends, &[]);
    i.arts = [Some(GoofyMove::Interpose), None];
    let mut mem = AiMemory::new(MemberId(1));
    think(&i, &mut mem, DT, &mut no_luck);
    assert_ne!(mem.last, Some(AiWord::Interpose));
}

#[test]
fn he_takes_a_breaker_when_it_is_offered_after_a_beat() {
    let mut e = foe(1, 200.0);
    e.broken = 2.0;
    let enemies = [e];
    let friends = [lead(100, 0)];
    let mut i = base(&enemies, &friends, &[]);
    i.breaker = true;
    let mut mem = AiMemory::new(MemberId(1));
    assert!(!think(&i, &mut mem, DT, &mut no_luck).pressed.has(PadButton::FaceTop), "a beat first");
    let mut pressed = None;
    for _ in 0..30 {
        let out = think(&i, &mut mem, DT, &mut no_luck);
        if out.pressed.has(PadButton::FaceTop) {
            pressed = Some(out.lock);
        }
    }
    assert_eq!(pressed, Some(TargetId(1)), "Y, locked on the broken enemy");
    assert_eq!(mem.last, Some(AiWord::Breaker));
}

/// (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique") no counter is waited for or pressed: after a
/// parry his A is only ever his plain string, on its own clock.
#[test]
fn the_brain_presses_no_a_for_a_parry() {
    let enemies = [foe(1, 200.0)];
    let friends = [lead(100, 0)];
    let mut i = base(&enemies, &friends, &[]);
    i.stance = Stance::Guarding; // a parry's recoil
    let mut mem = AiMemory::new(MemberId(1));
    for _ in 0..60 {
        assert!(!think(&i, &mut mem, DT, &mut no_luck).pressed.has(PadButton::FaceBottom), "nothing to cash in out of a parry");
    }
}

#[test]
fn a_rush_art_never_comes_from_the_ai_on_its_own() {
    let mut e = foe(1, 200.0);
    e.stagger = 0.9;
    let enemies = [e];
    let friends = [lead(100, 0)];
    let mut i = base(&enemies, &friends, &[]);
    i.arts = [Some(GoofyMove::GoofyCharge), Some(GoofyMove::GoofyTurbo)];
    let mut mem = AiMemory::new(MemberId(1));
    assert_eq!(think(&i, &mut mem, DT, &mut no_luck).art, None, "they would carry him off the leader");
    i.arts = [Some(GoofyMove::GoofyCharge), Some(GoofyMove::ShieldToss)];
    let mut mem = AiMemory::new(MemberId(1));
    assert_eq!(think(&i, &mut mem, DT, &mut no_luck).art, None, "the toss leaves him bare-handed: never on his own");
}

/// (REVENGE, 2026-11-07) A Mimic Tear boss copy's revenge counter (kh-er mimicrevenge.rs): its Arts go first once he is free, the
/// first he wears and may use; busy, the list waits; the rush the AI never picks on its own is taken when asked.
#[test]
fn a_boss_copys_revenge_art_goes_first_once_he_is_free() {
    let enemies = [foe(1, 600.0)];
    let friends = [lead(100, 0)];
    let mut i = base(&enemies, &friends, &[]);
    i.arts = [Some(GoofyMove::GoofyCharge), Some(GoofyMove::GoofyBash)];
    let mut mem = AiMemory::new(MemberId(1));
    mem.revenge_arts = vec![GoofyMove::GoofyCharge, GoofyMove::GoofyBash];
    i.stance = Stance::Busy;
    assert_eq!(think(&i, &mut mem, DT, &mut no_luck).art, None, "in a reaction: it waits");
    assert_eq!(mem.revenge_arts.len(), 2);
    i.stance = Stance::Free;
    assert_eq!(think(&i, &mut mem, DT, &mut no_luck).art, Some(GoofyMove::GoofyCharge), "free: the counter first");
    assert!(mem.revenge_arts.is_empty(), "asked once");
    // one he does not wear: the next of the list
    i.arts = [Some(GoofyMove::GoofyBash), None];
    mem.revenge_arts = vec![GoofyMove::GoofyCharge, GoofyMove::GoofyBash];
    assert_eq!(think(&i, &mut mem, DT, &mut no_luck).art, Some(GoofyMove::GoofyBash));
    // none of them: nothing of the counter, his own rules go on
    i.arts = [None, None];
    mem.revenge_arts = vec![GoofyMove::GoofyCharge];
    assert_eq!(think(&i, &mut mem, DT, &mut no_luck).art, None);
    assert!(mem.revenge_arts.is_empty());
}

// ------------------------------------------------------------------------------------------------ (THE USER 2026-11-02) his new loop

/// "Make sure Goofy as an AI party member uses his new tools properly": the right technique for each class, by the sheet's learn
/// levels (0 / 5 / 10 / 15 / 20 / 26 / 33 / 40), never before.
#[test]
fn the_right_technique_comes_at_each_learn_level() {
    let mut i = base(&[], &[], &[]);
    let rows = [(Tech::ShieldReflect, AttackClass::Projectile, Answer::Reflect, 5), (Tech::HopCounter, AttackClass::Sweep, Answer::JumpOver, 10), (Tech::PinningGuard, AttackClass::Thrust, Answer::Pinning, 15), (Tech::SlipCounter, AttackClass::Grab, Answer::Slip, 20), (Tech::BullBrace, AttackClass::Heavy, Answer::Brace, 26), (Tech::SpellWard, AttackClass::Magic, Answer::Ward, 33)];
    for (tech, class, answer, level) in rows {
        assert_eq!(tech.mv().learn_level(), level, "{tech:?}: the sheet's level");
        i.level = level - 1;
        let before = choose(class, &i);
        assert!(matches!(before, Answer::Deflect | Answer::Dodge), "{tech:?} at level {}: plain or dodge, {before:?}", level - 1);
        i.level = level;
        assert_eq!(choose(class, &i), answer, "{tech:?} at level {level}");
    }
    assert_eq!(Tech::RhythmDeflect.mv().learn_level(), 40);
    i.level = 1;
    assert_eq!(choose(AttackClass::Normal, &i), Answer::Deflect, "Deflect from the start");
}

/// A BROKEN enemy out of his Breaker's reach: he walks in (not with Hands Off, not when its window would end first).
#[test]
fn a_broken_enemy_out_of_reach_draws_him_in_for_the_breaker() {
    let friends = [lead(100, 0)];
    let mut e = foe(1, 520.0);
    e.broken = 2.5;
    let enemies = [e];
    let i = base(&enemies, &friends, &[]);
    let mut mem = AiMemory::new(MemberId(1));
    let out = think(&i, &mut mem, DT, &mut no_luck);
    assert_eq!((out.chase, out.lock, mem.last), (Some(TargetId(1)), TargetId(1), Some(AiWord::Chase)));
    // Hands Off: he stays
    let mut hands_off = base(&enemies, &friends, &[]);
    hands_off.style.battle = STYLE_HANDS_OFF;
    assert_eq!(think(&hands_off, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).chase, None);
    // too little window left for the walk
    let mut late = e;
    late.broken = 0.3;
    let enemies = [late];
    assert_eq!(think(&base(&enemies, &friends, &[]), &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).chase, None);
    // too far for the walk, but Finish the Job reaches further
    let mut far = e;
    far.pos = Vec3::new(K.ai_breaker_chase + 200.0, 0.0, 0.0);
    far.broken = 2.5;
    let friends = [AiFriend { pos: far.pos - Vec3::new(300.0, 0.0, 0.0), ..lead(100, 0) }];
    let enemies = [far];
    assert_eq!(think(&base(&enemies, &friends, &[]), &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).chase, None);
    let mut focus = base(&enemies, &friends, &[]);
    focus.style.battle = STYLE_FOCUS;
    assert_eq!(think(&focus, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).chase, Some(TargetId(1)));
}

/// His own Arts when they pay off, by the Customize Abilities setting: Bash on a nearly-full Stagger gauge, Tornado on a pack;
/// Conserve keeps to Taunt and Interpose; Watch MP keeps its reserve.
#[test]
fn he_uses_bash_on_a_nearly_broken_enemy_and_tornado_on_a_pack_by_his_ability_setting() {
    let friends = [lead(100, 0)];
    let mut ripe = foe(1, 200.0);
    ripe.stagger = K.ai_bash_stagger + 0.05;
    let enemies = [ripe];
    let mut i = base(&enemies, &friends, &[]);
    i.arts = [Some(GoofyMove::GoofyBash), Some(GoofyMove::GoofyTornado)];
    i.art_cost = [22, 20];
    let out = think(&i, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck);
    assert_eq!((out.art, out.lock), (Some(GoofyMove::GoofyBash), TargetId(1)), "the slam's Stagger breaks it");
    // a fresh gauge: no Bash
    let enemies = [foe(1, 200.0)];
    let mut fresh = base(&enemies, &friends, &[]);
    fresh.arts = i.arts;
    assert_eq!(think(&fresh, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).art, None);
    // a pack of three: the Tornado. (COMBAT-FIX) Within KH3's Attack_Tornado prism (circumradius 130 to their bodies), not
    // the old 280 sphere: a pack 1.5 - 2 m off (1 - 1.5 m to their bodies) is out of the spin
    let enemies = [foe(1, 120.0), foe(2, 150.0), foe(3, 175.0)];
    let mut pack = base(&enemies, &friends, &[]);
    pack.arts = [None, Some(GoofyMove::GoofyTornado)];
    pack.art_cost = [0, 20];
    assert_eq!(think(&pack, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).art, Some(GoofyMove::GoofyTornado));
    let enemies = [foe(1, 150.0), foe(2, 180.0), foe(3, 200.0)];
    let mut off = base(&enemies, &friends, &[]);
    off.arts = pack.arts;
    off.art_cost = pack.art_cost;
    assert_eq!(think(&off, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).art, None, "the spin would not reach them");
    // Conserve: neither
    let ripe_only = [ripe];
    let mut conserve = base(&ripe_only, &friends, &[]);
    conserve.arts = i.arts;
    conserve.style.ability = ABILITY_CONSERVE;
    assert_eq!(think(&conserve, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).art, None);
    // Watch MP: only while 30 % of his bar is left after it
    let mut watch = base(&ripe_only, &friends, &[]);
    watch.arts = i.arts;
    watch.art_cost = i.art_cost;
    watch.style.ability = ABILITY_WATCH_MP;
    watch.mp = 40;
    assert_eq!(think(&watch, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).art, None, "40 - 22 is under the reserve");
    watch.mp = 60;
    assert_eq!(think(&watch, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).art, Some(GoofyMove::GoofyBash));
}

/// Never an Art that waits on its cooldown: the brain is handed only the ready ones, and Interpose's own clock is read too.
#[test]
fn he_never_asks_for_an_art_that_waits() {
    let mut ripe = foe(1, 200.0);
    ripe.stagger = 0.95;
    let enemies = [ripe];
    let friends = [lead(100, 0)];
    let mut i = base(&enemies, &friends, &[]);
    i.arts = [None, None]; // Bash equipped but cooling: `ai_art_moves` left it out
    i.art_asked = Some(GoofyMove::GoofyBash);
    assert_eq!(think(&i, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).art, None, "not even when his tree asks");
    let enemies = [foe(1, -300.0)];
    let friends = [lead(20, 1)];
    let mut low = base(&enemies, &friends, &[]);
    low.arts = [None, Some(GoofyMove::Interpose)];
    low.interpose_ready = false;
    assert_ne!(think(&low, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).art, Some(GoofyMove::Interpose), "Interpose cooling");
}

/// Low GUARD: no string, and back to the leader rather than stand and guard into a Guard Break; a failed roll is the hop.
#[test]
fn at_low_guard_he_backs_off_and_starts_no_string() {
    let enemies = [foe(1, 200.0)];
    let friends = [AiFriend { pos: Vec3::new(-700.0, 0.0, 0.0), ..lead(100, 0) }];
    let mut i = base(&enemies, &friends, &[]);
    i.guard = K.ai_guard_low * 0.5;
    let mut mem = AiMemory::new(MemberId(1));
    let out = think(&i, &mut mem, DT, &mut no_luck);
    assert!(!out.pressed.has(PadButton::FaceBottom) && out.back, "{out:?}");
    assert_eq!(mem.last, Some(AiWord::Recover));
    // at his post: he simply holds off
    let friends = [lead(100, 0)];
    let mut post = base(&enemies, &friends, &[]);
    post.guard = K.ai_guard_low * 0.5;
    let out = think(&post, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck);
    assert!(!out.pressed.has(PadButton::FaceBottom) && !out.back, "{out:?}");
    assert_eq!(mistimed(Answer::Deflect, &post), Answer::Dodge, "a failed roll hops rather than block");
    // full GUARD: the string
    let full = base(&enemies, &friends, &[]);
    assert!(think(&full, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).pressed.has(PadButton::FaceBottom));
}

/// The rebound's opening: a string at once on the enemy his perfect deflect bounced, whatever his string clock says; never in
/// the bash's end lag, and Hands Off strikes nothing.
#[test]
fn he_strikes_into_a_rebounds_opening_and_obeys_the_end_lag() {
    let enemies = [foe(1, 200.0)];
    let friends = [lead(100, 0)];
    let mut mem = AiMemory::new(MemberId(1));
    mem.poke_in = 0.8; // the clock says wait
    let mut i = base(&enemies, &friends, &[]);
    i.rebound = Some(TargetId(1));
    let out = think(&i, &mut mem, DT, &mut no_luck);
    assert!(out.pressed.has(PadButton::FaceBottom) && out.lock == TargetId(1), "{out:?}");
    assert_eq!(mem.last, Some(AiWord::FollowUp));
    // the end lag: no press, the opening or not
    let mut lag = base(&enemies, &friends, &[]);
    lag.rebound = Some(TargetId(1));
    lag.end_lag = true;
    assert!(!think(&lag, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).pressed.has(PadButton::FaceBottom));
    lag.stance = Stance::Striking;
    assert!(!think(&lag, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).pressed.has(PadButton::FaceBottom), "inside his own string too");
    let mut hands_off = base(&enemies, &friends, &[]);
    hands_off.style.battle = STYLE_HANDS_OFF;
    assert!(!think(&hands_off, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).pressed.has(PadButton::FaceBottom));
}

/// Finish the Job strikes the most staggered enemy in reach.
#[test]
fn finish_the_job_strikes_the_most_staggered() {
    let mut a = foe(1, 150.0);
    let mut b = foe(2, 200.0);
    (a.stagger, b.stagger) = (0.1, 0.5);
    let enemies = [a, b];
    let friends = [lead(100, 0)];
    let mut i = base(&enemies, &friends, &[]);
    assert_eq!(think(&i, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).lock, TargetId(1), "the nearest");
    i.style.battle = STYLE_FOCUS;
    assert_eq!(think(&i, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).lock, TargetId(2), "the most staggered");
}

#[test]
fn a_window_that_never_closes_does_not_hold_him_behind_his_shield() {
    let enemies = [foe(1, 200.0)];
    let friends = [lead(100, 0)];
    let t = [Telegraph { attacker: T, class: AttackClass::Normal, perilous: false, eta: 0.0, active: true }];
    let i = base(&enemies, &friends, &t);
    let mut mem = AiMemory::new(MemberId(1));
    let mut held = 0;
    for _ in 0..300 {
        held += think(&i, &mut mem, DT, &mut no_luck).held.has(PadButton::FaceLeft) as u32;
    }
    assert!(held > 0 && held as f32 * DT <= K.ai_answer_max + 0.02, "{held}");
}

#[test]
fn an_attack_on_a_friend_across_the_field_is_not_his_to_answer() {
    let enemies = [foe(1, 900.0)];
    let mut friend = lead(100, 0);
    friend.pos = Vec3::new(800.0, 0.0, 0.0);
    let friends = [friend];
    let t = [Telegraph { attacker: T, class: AttackClass::Normal, perilous: false, eta: 0.5, active: false }];
    let i = base(&enemies, &friends, &t);
    assert!(!meant_for_him(&t[0], &i), "far from him, beside the leader");
    let enemies = [foe(1, 300.0)];
    let mut friend = lead(100, 0);
    friend.pos = Vec3::new(280.0, 0.0, 0.0);
    let friends = [friend];
    let i = base(&enemies, &friends, &t);
    assert!(meant_for_him(&t[0], &i), "a swing in reach is his, whoever stands nearer: he draws them");
    let shot = [Telegraph { attacker: T, class: AttackClass::Projectile, perilous: false, eta: 0.5, active: false }];
    let enemies = [foe(1, 900.0)];
    let mut friend = lead(100, 0);
    friend.pos = Vec3::new(800.0, 0.0, 0.0);
    let friends = [friend];
    let i = base(&enemies, &friends, &shot);
    assert!(!meant_for_him(&shot[0], &i), "a shot clearly nearer to a friend is that friend's");
    let friends = [lead(100, 0)];
    let i = base(&enemies, &friends, &shot);
    assert!(meant_for_him(&shot[0], &i));
}

// ------------------------------------------------------------------------------------------------ a party, no tree

struct Dummies(Vec<(TargetId, Vec3)>, Vec<HitRecord>);

impl Dummies {
    fn of(&self, (id, p): (TargetId, Vec3)) -> TargetInfo {
        TargetInfo { id, position: p, center: p + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
    }
}

impl Targets for Dummies {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        for &t in &self.0 {
            out.push(self.of(t));
        }
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.0.iter().find(|t| t.0 == id).map(|&t| self.of(t))
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(i) = self.info(id) {
            out.push(HurtShape { a: i.position + Vec3::Z * 30.0, b: i.position + Vec3::Z * 150.0, radius: 60.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.1.push(*hit);
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: 1.0e6, killed: false, guarded: false }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: 10, max_hp: 1_000_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
    }
    // (AUDIT-B) no bones here: a rider attached to a hit follows the target's origin and yaw
    fn hit_bone(&self, _id: kh_core::TargetId, _at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: kh_core::TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

struct Rig {
    world: TestWorld,
    dummies: Dummies,
    audio: kh_core::host::audio::NullAudio,
    effects: kh_core::host::effects::NullEffects,
    party: Party,
    sora: SoraView,
    id: MemberId,
    events: Vec<GoofyEvent>,
}

impl Rig {
    /// AI Goofy at the origin, the leader behind him, the dummies where they are told.
    fn new(dummies: &[Vec3], level99: bool, seed: u64) -> Rig {
        let mut party = Party::new(PartyConfig { level99, total_exp: if level99 { 1_527_640 } else { 0 }, seed });
        let id = party.spawn(Chara::Goofy, Vec3::ZERO, 0.0);
        let d = Dummies(dummies.iter().enumerate().map(|(i, p)| (TargetId(i as u32 + 1), *p)).collect(), Vec::new());
        let sora = SoraView { position: Vec3::new(-150.0, 0.0, 0.0), hp: 200, hp_max: 200, ..SoraView::default() };
        let mut rig = Rig { world: TestWorld::floor(), dummies: d, audio: Default::default(), effects: Default::default(), party, sora, id, events: Vec::new() };
        rig.party.set_in_battle(true);
        rig.mm().status.goofy.as_mut().unwrap().core.hop_cd = 0.0;
        rig.step(10);
        rig
    }

    fn m(&self) -> &crate::pawn::NpcPawn {
        self.party.member(self.id).unwrap()
    }

    fn mm(&mut self) -> &mut crate::pawn::NpcPawn {
        self.party.member_mut(self.id).unwrap()
    }

    fn step(&mut self, n: usize) {
        for _ in 0..n {
            self.one();
        }
    }

    fn one(&mut self) {
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host);
        self.events.extend(self.party.gof.events.iter().map(|e| e.1));
    }

    fn mem(&self) -> &AiMemory {
        self.party.gof.ai.iter().find(|a| a.member == self.id).expect("his memory")
    }

    fn log(&self) -> Vec<&'static str> {
        self.m().status.goofy.as_ref().unwrap().run.log.clone()
    }

    /// The hit as the Elden Ring ally route builds it (kh-er allybody.rs `AllyHit::record`).
    fn er_hit(&mut self, class: AttackClass, damage: f32) -> HitResult {
        let me = self.m().position();
        let mut h = HitRecord::empty(T);
        h.attack_name = "er.body";
        h.damage = damage;
        h.damage_kind = Kind::Small;
        h.direction = (me - NEAR).normalize_or_zero();
        h.position = me + Vec3::Z * 60.0;
        h.attacker = HitAttacker { position: NEAR, yaw: 180.0, radius: 50.0, valid: true };
        h.attack_info = AttackInfo { class, perilous: is_perilous(class), attacker: Some(T) };
        self.party.receive_hit(self.id, &h)
    }

    /// One telegraphed attack: on the list `eta` s ahead, its hit window open `active` s, the hit `after` s into it (Elden Ring's
    /// come about 60 ms in: SENSOR_NOTES). Returns what the hit did, None when it passed under him.
    fn attack(&mut self, class: AttackClass, eta: f32, active: f32, after: f32) -> Option<HitResult> {
        let mut left = eta;
        while left > 0.0 {
            self.party.set_telegraphs(&[Telegraph { attacker: T, class, perilous: is_perilous(class), eta: left, active: false }]);
            self.one();
            left -= DT;
        }
        let mut result = None;
        let mut t = 0.0;
        let mut fired = false;
        while t < active {
            self.party.set_telegraphs(&[Telegraph { attacker: T, class, perilous: is_perilous(class), eta: 0.0, active: true }]);
            if !fired && t >= after {
                fired = true;
                if !(class == AttackClass::Sweep && !self.m().on_ground()) {
                    result = Some(self.er_hit(class, 20.0));
                }
            }
            self.one();
            t += DT;
        }
        self.party.set_telegraphs(&[]);
        self.one();
        result
    }

    /// Back to rest between attacks.
    fn settle(&mut self) {
        self.step(150);
        let m = self.mm();
        // (COMBAT-FIX) back on his spot (an unanswered blow knocks him away; his KH3 tree walks him back in, this rig has none)
        m.cmc.location = Vec3::new(0.0, 0.0, m.cmc.location.z);
        m.yaw = 0.0;
        m.status.hp = m.status.max_hp();
        if let Some(g) = m.status.guard.as_mut() {
            g.value = g.max;
            g.broken = false;
        }
        m.status.goofy.as_mut().unwrap().core.resolve = Resolve::default();
    }

    fn parried(&self, tech: Tech) -> usize {
        self.events.iter().filter(|e| matches!(e, GoofyEvent::Parried { tech: t, right: true, .. } if *t == tech)).count()
    }
}

fn is_perilous(class: AttackClass) -> bool {
    matches!(class, AttackClass::Thrust | AttackClass::Sweep | AttackClass::Grab | AttackClass::Heavy)
}

/// Attacks of one class until the technique has worked once.
fn answers(class: AttackClass, tech: Tech, eta: f32, active: f32, after: f32) {
    let mut rig = Rig::new(&[NEAR], true, 21);
    for n in 0..16 {
        let hp = rig.m().status.hp;
        let hit = rig.attack(class, eta, active, after);
        if rig.parried(tech) > 0 {
            assert!(hit.is_none_or(|r| r.damage_dealt == 0.0), "{tech:?}: no HP goes: {hit:?}");
            assert_eq!(rig.m().status.hp, hp, "{tech:?}");
            return;
        }
        assert!(n < 15, "{tech:?} never worked: {:?} {:?}", rig.mem().last, rig.log());
        rig.settle();
    }
}

#[test]
fn every_technique_runs_through_the_players_moves_and_judgement() {
    answers(AttackClass::Normal, Tech::Deflect, 0.8, 0.12, 0.06);
    answers(AttackClass::Thrust, Tech::PinningGuard, 0.8, 0.12, 0.06);
    answers(AttackClass::Projectile, Tech::ShieldReflect, 0.8, 0.12, 0.06);
    answers(AttackClass::Magic, Tech::SpellWard, 1.0, 0.12, 0.06);
    answers(AttackClass::Heavy, Tech::BullBrace, 1.0, 0.12, 0.06);
    answers(AttackClass::Grab, Tech::SlipCounter, 0.8, 0.12, 0.06);
    answers(AttackClass::Sweep, Tech::HopCounter, 0.8, 0.2, 0.06);
}

#[test]
fn a_long_hit_window_gets_a_second_tap_and_no_mash() {
    let mut rig = Rig::new(&[NEAR], true, 21);
    let mut worked = false;
    for _ in 0..12 {
        rig.events.clear();
        rig.attack(AttackClass::Normal, 0.8, 0.5, 0.45);
        worked |= rig.parried(Tech::Deflect) > 0;
        if worked {
            break;
        }
        rig.settle();
    }
    assert!(worked, "a hit late in a long window can be deflected: {:?}", rig.log());
    assert_eq!(rig.m().status.goofy.as_ref().unwrap().core.mash, 0, "no tap inside the mash gap");
}

#[test]
fn parry_rates_over_seeded_runs_at_low_and_high_level() {
    let rate = |level99: bool| {
        let (mut parried, mut attacks, mut good) = (0, 0, 0);
        for seed in 1..=16 {
            let mut rig = Rig::new(&[NEAR], level99, seed);
            for _ in 0..30 {
                let before = rig.parried(Tech::Deflect);
                let hit = rig.attack(AttackClass::Normal, 0.8, 0.12, 0.06);
                // (a blow that found him inside his own Breaker is no test of his timing: he is invincible there)
                let judged = hit.is_some_and(|h| h.accepted);
                attacks += judged as u32;
                parried += (judged && rig.parried(Tech::Deflect) > before) as u32;
                rig.settle();
            }
            good += rig.mem().rolled_good;
        }
        println!("AI Goofy, {attacks} seeded attacks: {parried} parried, {good} rolled well");
        parried as f32 / attacks as f32
    };
    let (low, high) = (rate(false), rate(true));
    println!("AI Goofy parry rate: level 1 {low:.3}, level 99 {high:.3}");
    assert!((low - K.ai_parry_rate_low).abs() < 0.1, "level 1: {low}");
    assert!((high - K.ai_parry_rate_high).abs() < 0.1, "level 99: {high}");
    assert!(high > low + 0.25 && high < 0.97, "{low} {high}");
}

/// (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique") an AI parry costs no HP, fills Stagger and gives
/// MP back; no counter follows it.
#[test]
fn a_parried_hit_by_the_er_ally_route_costs_no_hp_fills_stagger_and_no_counter_follows() {
    let mut rig = Rig::new(&[NEAR], true, 21);
    for _ in 0..20 {
        let hp = rig.m().status.hp;
        rig.mm().status.mp = 10;
        let r = rig.attack(AttackClass::Normal, 0.8, 0.12, 0.06).expect("the hit came");
        if r.guarded && rig.parried(Tech::Deflect) > 0 {
            assert_eq!(r.damage_dealt, 0.0);
            assert_eq!(rig.m().status.hp, hp, "the parry cost no HP");
            assert!(rig.events.iter().any(|e| matches!(e, GoofyEvent::Stagger { target: T, .. })), "Stagger on the attacker");
            assert_eq!(rig.m().status.mp, 10 + K.parry_mp as i32, "a perfect parry's MP");
            let before = rig.dummies.1.len();
            for _ in 0..90 {
                rig.one();
                assert_ne!(rig.m().state, NpcStateId::Pad(43), "the old counter state");
            }
            assert!(rig.dummies.1[before..].iter().all(|h| h.attack_name != "GOOFY_COUNTER"), "no counter");
            assert!(!rig.log().iter().any(|w| matches!(*w, "Counter Bash" | "Pin Down" | "Crescendo")), "{:?}", rig.log());
            return;
        }
        rig.settle();
    }
    panic!("no parry in 20 attacks");
}

/// "Make sure Goofy as an AI party member uses his new tools properly": THE LOOP, in a party with no tree. His parries and
/// Stagger break the enemy; a BROKEN enemy a few metres off draws him in, and Goofy Breaker lands on it: the big damage.
#[test]
fn the_ai_walks_to_a_broken_enemy_and_lands_goofy_breaker() {
    let far = Vec3::new(520.0, 0.0, 0.0);
    let mut rig = Rig::new(&[far], true, 7);
    rig.mm().status.goofy.as_mut().unwrap().abilities = crate::goofy::GoofyAbilities::default(); // the plain Breaker
    rig.party.gof.word.push((T, 1000.0)); // the gauge fills: BROKEN
    rig.step(2);
    assert!(rig.party.gof.book.view(T).is_some_and(|v| v.broken.is_some()), "broken");
    let mut landed = None;
    for n in 0..240 {
        rig.one();
        if let Some(h) = rig.dummies.1.iter().find(|h| h.attack_name == "GOOFY_BREAKER") {
            landed = Some((n, h.info.power));
            break;
        }
        // Stagger's BROKEN window is short: hold it while he walks (the walk is what is tested, not its length)
        if let Some(v) = rig.party.gof.book.view(T) {
            if v.broken.is_none() {
                rig.party.gof.word.push((T, 1000.0));
            }
        }
    }
    let (n, power) = landed.unwrap_or_else(|| panic!("no Breaker: {:?} {:?} at {:?}", rig.mem().last, rig.log(), rig.m().position()));
    assert!(rig.log().contains(&"breaker"), "{:?}", rig.log());
    assert!((power - K.breaker_power).abs() < 1e-4, "Goofy Breaker at its sheet power, {power}");
    assert!(n < 200, "promptly: {n} steps");
    assert!(rig.m().position().x > 100.0, "he walked in: {:?}", rig.m().position());
}

/// (THE USER 2026-11-03 follow-up, in game: a soldier was broken twice with Sora leading and AI Goofy stayed Idle / Move) out of
/// battle, walking after the leader with his tree's following re-asked every step, a BROKEN enemy 4.7 m off still draws him in
/// and Goofy Breaker lands on it; his tree's moves are refused while he walks.
#[test]
fn out_of_battle_and_following_his_tree_he_still_walks_to_a_broken_enemy_and_breaks_it() {
    let far = Vec3::new(520.0, 0.0, 0.0);
    let mut rig = Rig::new(&[far], true, 7);
    rig.party.set_in_battle(false);
    rig.mm().status.goofy.as_mut().unwrap().abilities = crate::goofy::GoofyAbilities::default();
    rig.step(5);
    // the brain alone: out of battle, the chase is still there
    let enemies = [AiEnemy { broken: 2.5, ..foe(1, 520.0) }];
    let friends = [lead(100, 0)];
    let mut i = base(&enemies, &friends, &[]);
    i.in_battle = false;
    assert_eq!(think(&i, &mut AiMemory::new(MemberId(1)), DT, &mut no_luck).chase, Some(TargetId(1)), "in battle or not");
    // the party: the gauge fills while his tree keeps asking him to follow the leader
    rig.party.gof.word.push((T, 1000.0));
    let mut landed = false;
    let mut refused = 0;
    for _ in 0..240 {
        let t = rig.party.command(rig.id, MemberCommand::MoveTo { goal: MoveGoal::Sora, locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 100.0, follow_path: true });
        refused += (rig.party.command_status(rig.id, t) == CommandStatus::Failed) as u32;
        rig.one();
        if rig.dummies.1.iter().any(|h| h.attack_name == "GOOFY_BREAKER") {
            landed = true;
            break;
        }
        if rig.party.gof.book.view(T).is_some_and(|v| v.broken.is_none()) {
            rig.party.gof.word.push((T, 1000.0)); // (the window is short: hold it while he walks)
        }
    }
    assert!(landed, "no Breaker: {:?} {:?} at {:?}", rig.mem().last, rig.log(), rig.m().position());
    assert!(refused > 0, "his tree's follow was refused while he walked");
    assert!(rig.m().status.goofy.as_ref().unwrap().run.ai_chase.is_none() || rig.log().contains(&"breaker"));
}

/// Never an Art that waits: with Bash equipped and an enemy whose Stagger is nearly full, he slams once, then lets its cooldown
/// run (no refused press).
#[test]
fn the_ai_slams_a_nearly_broken_enemy_once_and_never_presses_the_waiting_art() {
    let mut rig = Rig::new(&[NEAR], true, 9);
    let bash = crate::pad::goofy_arts::Art::Bash;
    let s = rig.mm().status.goofy.as_mut().unwrap();
    s.run.arts.equipped = Some([bash.id(), crate::pad::goofy_arts::Art::Charge.id()]);
    rig.mm().status.mp = rig.m().status.max_mp();
    for _ in 0..300 {
        // keep the gauge at 70 %, never full
        let have = rig.party.gof.book.view(T).map_or(0.0, |v| v.value);
        let want = K.stagger_gauge * 0.7 - have;
        if want > 0.0 {
            rig.party.gof.word.push((T, want));
        }
        rig.one();
        if rig.party.gof.book.view(T).is_some_and(|v| v.broken.is_some()) {
            break; // the slam broke it: done
        }
    }
    let log = rig.log();
    assert!(log.contains(&"art_bash"), "the slam: {log:?}");
    assert!(!log.contains(&"art_cooling"), "no press of a waiting Art: {log:?}");
}

/// (COMBAT-FIX) THE USER 2026-11-07: "goofy as an ai ally at level 1 should have a 5% chance ... Goofy should still use his
/// attacks and abilties as well as deflecting". A roll the ally fails is not answered (`ai.miss_blocks` 0: KH3's own Goofy
/// tree has no guard): no shield goes up for it, the blow lands, and GUARD is not touched.
#[test]
fn a_failed_roll_is_not_answered_the_blow_lands_and_no_guard_goes_up() {
    assert_eq!(K.ai_miss_blocks, 0.0);
    let mut rig = Rig::new(&[NEAR], false, 3);
    let mut landed = false;
    for _ in 0..12 {
        let hp = rig.m().status.hp;
        let g = rig.m().status.guard.unwrap().value;
        let r = rig.attack(AttackClass::Normal, 0.8, 0.12, 0.06).expect("the hit came");
        if rig.parried(Tech::Deflect) == 0 {
            assert!(!r.guarded, "no block of a failed roll: {r:?}");
            assert!(rig.m().status.hp < hp, "the blow landed");
            assert_eq!(rig.m().status.guard.unwrap().value, g, "GUARD untouched");
            assert!(!rig.log().contains(&"block"), "{:?}", rig.log());
            landed = true;
            break;
        }
        rig.events.clear();
        rig.settle();
    }
    assert!(landed, "{:?}", rig.log());
}

#[test]
fn a_low_goofy_cannot_pin_a_thrust_he_only_deflects_it() {
    let mut rig = Rig::new(&[NEAR], false, 5);
    // (COMBAT-FIX) level 1 times one attack in 20 (`ai.parry_rate_low` 0.05): enough thrusts for his plain deflects to show
    for _ in 0..240 {
        rig.attack(AttackClass::Thrust, 0.8, 0.12, 0.06);
        rig.settle();
        if rig.events.iter().any(|e| matches!(e, GoofyEvent::Parried { tech: Tech::Deflect, right: false, .. })) {
            break;
        }
    }
    assert_eq!(rig.parried(Tech::PinningGuard), 0);
    assert!(rig.events.iter().any(|e| matches!(e, GoofyEvent::Parried { tech: Tech::Deflect, right: false, .. })), "plain deflects: half Stagger");
}

#[test]
fn he_reads_an_attack_before_he_answers_it() {
    let mut rig = Rig::new(&[APART], true, 9);
    rig.party.set_telegraphs(&[Telegraph { attacker: T, class: AttackClass::Normal, perilous: false, eta: 0.0, active: true }]);
    rig.one();
    assert!(!matches!(rig.m().state, NpcStateId::Pad(_)), "a window that opens with no warning: no instant shield");
}

#[test]
fn with_nothing_to_parry_he_pokes_what_stands_near_and_leaves_time_between_strings() {
    let mut rig = Rig::new(&[NEAR], true, 2);
    rig.step(300);
    let hits = rig.dummies.1.len();
    assert!(hits >= 2, "not idle: {:?} {:?}", rig.log(), rig.m().state);
    assert!(hits <= 12, "low priority: {hits} hits in 5 s");
    assert!(rig.log().contains(&"shove2"), "{:?}", rig.log());
    assert!(!rig.log().contains(&"shove3"), "his strings are short: {:?}", rig.log());
}

#[test]
fn he_does_not_chase_and_a_far_enemy_gets_no_string() {
    let mut rig = Rig::new(&[Vec3::new(1500.0, 0.0, 0.0)], true, 2);
    rig.step(240);
    assert!(rig.dummies.1.is_empty());
    assert!(rig.m().position().length() < 10.0, "he stood: {:?}", rig.m().position());
}

#[test]
fn a_tree_move_past_the_leash_is_cut_to_the_guard_post() {
    let mut rig = Rig::new(&[Vec3::new(2000.0, 0.0, 0.0)], true, 2);
    let t = rig.party.command(rig.id, MemberCommand::MoveTo { goal: MoveGoal::Target(T), locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 100.0, follow_path: false });
    rig.step(2);
    match rig.m().command_running() {
        Some(MemberCommand::MoveTo { goal: MoveGoal::Point(p), .. }) => assert!((p - rig.sora.position).length() <= K.ai_post + 1.0, "{p}"),
        c => assert!(rig.party.command_status(rig.id, t) == CommandStatus::Succeeded, "{c:?}"),
    }
    rig.step(240);
    assert!((rig.m().position() - rig.sora.position).length() <= K.ai_leash, "{:?}", rig.m().position());
    // his following is his tree's: a move to the leader stands
    rig.party.command(rig.id, MemberCommand::MoveTo { goal: MoveGoal::Sora, locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 100.0, follow_path: false });
    rig.step(2);
    assert!(matches!(rig.m().command_running(), Some(MemberCommand::MoveTo { goal: MoveGoal::Sora, .. })));
}

#[test]
fn thrown_off_the_leader_he_goes_back_and_only_then_strikes() {
    let mut rig = Rig::new(&[Vec3::new(1400.0, 0.0, 0.0)], true, 2);
    rig.mm().cmc.location.x = 1250.0;
    rig.step(2);
    assert!(matches!(rig.m().command_running(), Some(MemberCommand::MoveTo { goal: MoveGoal::Sora, .. })), "{:?}", rig.m().command_running());
    rig.step(240);
    assert!((rig.m().position() - rig.sora.position).length() <= K.ai_leash, "{:?}", rig.m().position());
    assert!(rig.dummies.1.is_empty(), "the enemy beside him is not near the party");
}

#[test]
fn out_of_battle_nothing_of_this_moves_him() {
    let mut rig = Rig::new(&[NEAR], true, 2);
    rig.party.set_in_battle(false);
    rig.step(120);
    let hits = rig.dummies.1.len();
    rig.step(120);
    assert_eq!(rig.dummies.1.len(), hits, "no string out of battle");
    let t = rig.party.command(rig.id, MemberCommand::MoveTo { goal: MoveGoal::Point(Vec3::new(3000.0, 0.0, 0.0)), locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 100.0, follow_path: false });
    rig.step(2);
    assert_eq!(rig.party.command_status(rig.id, t), CommandStatus::Running);
    assert!(matches!(rig.m().command_running(), Some(MemberCommand::MoveTo { goal: MoveGoal::Point(p), .. }) if p.x == 3000.0));
}

#[test]
fn the_trees_attacks_are_replaced_and_the_kh3_switch_keeps_them() {
    let mut rig = Rig::new(&[APART], true, 2);
    let t = rig.party.command(rig.id, MemberCommand::Action { def: ActionDef::GoofyPush, target: crate::TargetRef::Enemy(T) });
    rig.step(1);
    assert_eq!(rig.party.command_status(rig.id, t), CommandStatus::Succeeded);
    assert_ne!(rig.m().state, NpcStateId::Action);
    let mut rig = Rig::new(&[APART], true, 2);
    rig.party.gof.kh3_ai = true;
    rig.step(60);
    let t = rig.party.command(rig.id, MemberCommand::Action { def: ActionDef::GoofyPush, target: crate::TargetRef::Enemy(T) });
    rig.step(1);
    assert_eq!(rig.party.command_status(rig.id, t), CommandStatus::Running);
    assert_eq!(rig.m().state, NpcStateId::Action);
}

#[test]
fn a_tree_art_is_never_played_as_kh3s() {
    let mut rig = Rig::new(&[APART], true, 2);
    let t = rig.party.command(rig.id, MemberCommand::Action { def: ActionDef::GoofyTornado, target: crate::TargetRef::Enemy(T) });
    rig.step(1);
    assert_ne!(rig.m().state, NpcStateId::Action);
    assert!(matches!(rig.party.command_status(rig.id, t), CommandStatus::Failed | CommandStatus::Succeeded));
}

#[test]
fn his_shield_is_not_run_over_by_the_tree_and_the_tree_gets_him_back() {
    let mut rig = Rig::new(&[APART], true, 21);
    let far = Vec3::new(500.0, 400.0, 0.0);
    let t = rig.party.command(rig.id, MemberCommand::MoveTo { goal: MoveGoal::Point(far), locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 10.0, follow_path: false });
    rig.step(2);
    assert_eq!(rig.m().state, NpcStateId::Move);
    let mut left = 0.6;
    while left > 0.0 && !matches!(rig.m().state, NpcStateId::Pad(_)) {
        rig.party.set_telegraphs(&[Telegraph { attacker: T, class: AttackClass::Normal, perilous: false, eta: left, active: false }]);
        rig.one();
        left -= DT;
    }
    assert!(matches!(rig.m().state, NpcStateId::Pad(_)), "the shield came up out of his walk");
    assert_eq!(rig.party.command_status(rig.id, t), CommandStatus::Aborted, "his tree is told, it would wait for ever");
    let refused = rig.party.command(rig.id, MemberCommand::MoveTo { goal: MoveGoal::Sora, locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 10.0, follow_path: false });
    assert_eq!(rig.party.command_status(rig.id, refused), CommandStatus::Failed);
    rig.party.set_telegraphs(&[]);
    rig.step(60);
    assert!(matches!(rig.m().state, NpcStateId::Idle | NpcStateId::Move | NpcStateId::Pad(_)));
    let mut back = false;
    for _ in 0..300 {
        rig.one();
        let t = rig.party.command(rig.id, MemberCommand::MoveTo { goal: MoveGoal::Sora, locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 10.0, follow_path: false });
        back |= rig.party.command_status(rig.id, t) == CommandStatus::Running;
    }
    assert!(back, "free again: {:?}", rig.m().state);
}

#[test]
fn a_hit_taken_mid_guard_does_not_leave_him_unable_to_walk() {
    let mut rig = Rig::new(&[APART], true, 21);
    rig.mm().cmc.speed_limit_override = Some(0.0);
    let last = crate::states::NpcStateId::Pad(crate::pad::goofy::GUARD);
    rig.mm().status.goofy.as_mut().unwrap().run.last_state = last;
    rig.step(1);
    assert_eq!(rig.m().cmc.speed_limit_override, None);
}

#[test]
fn when_he_leads_the_pad_has_him_and_the_ai_lets_go() {
    let mut rig = Rig::new(&[NEAR], true, 2);
    rig.step(30);
    rig.party.set_pad(rig.id, Some(kh_core::Tuning::USER));
    rig.step(2);
    let hits = rig.dummies.1.len();
    rig.step(240);
    assert!(rig.dummies.1.len() <= hits + 1, "the pad presses nothing: the last shove at most");
}

/// (PARTY-AI 2026-11-03, ROUND-2 E3: the broken soldier was not his target, his KH3 tree stayed on another) TWO ENEMIES: he strings
/// A, his own target, when B beside him is BROKEN. The rule (GOOFY_OVERHAUL.md 12) is any BROKEN enemy within `ai.breaker_chase`, his
/// target or not: in reach he takes it from the string at once; farther he ends the string, walks in and slams B, never A.
#[test]
fn a_broken_enemy_that_is_not_his_target_gets_his_breaker() {
    for b in [Vec3::new(0.0, 250.0, 0.0), Vec3::new(0.0, 450.0, 0.0), Vec3::new(-100.0, 550.0, 0.0)] {
        let mut rig = Rig::new(&[NEAR, b], true, 7);
        rig.mm().status.goofy.as_mut().unwrap().abilities = crate::goofy::GoofyAbilities::default(); // the plain Breaker
        let mut n = 0;
        while crate::pad::goofy::ai_stance(rig.m()) != crate::goofy::ai::Stance::Striking && n < 300 {
            rig.one();
            n += 1;
        }
        assert_eq!(crate::pad::goofy::ai_stance(rig.m()), crate::goofy::ai::Stance::Striking, "he strings A first");
        assert!(rig.dummies.1.iter().all(|h| h.target == T), "A is the one he fights: {:?}", rig.dummies.1.iter().map(|h| h.target).collect::<Vec<_>>());
        let before = rig.dummies.1.len();
        rig.party.gof.word.push((TargetId(2), 1000.0)); // B: BROKEN
        let mut landed = None;
        for k in 0..420 {
            rig.one();
            if let Some(h) = rig.dummies.1[before..].iter().find(|h| h.attack_name == "GOOFY_BREAKER") {
                landed = Some((k, h.target));
                break;
            }
        }
        let (k, on) = landed.unwrap_or_else(|| panic!("B at {b:?}: no Breaker: {:?} {:?}", rig.mem().last, rig.log()));
        assert_eq!(on, TargetId(2), "B at {b:?}: the slam is B's, not his own target's");
        assert!(k < 300, "B at {b:?}: within its window (held by the Breaker): {k} steps");
    }
}

/// (PARTY-AI, THE USER 2026-11-03: "the party leader can hit friendly NPCs, but not AI allies") AI Goofy's blows and his Goofy
/// Tornado carry the AI bit: they pass through friendly NPCs. Given the pad (he leads), his own bit is the player's.
#[test]
fn ai_goofys_tornado_and_strings_pass_through_friendly_npcs_and_leading_flips_it() {
    let mut rig = Rig::new(&[NEAR, Vec3::new(150.0, -150.0, 0.0), Vec3::new(180.0, 120.0, 0.0)], true, 7);
    rig.mm().status.goofy.as_mut().unwrap().run.arts.equipped = Some([crate::pad::goofy_arts::Art::Tornado.id(), crate::pad::goofy_arts::Art::Bash.id()]);
    rig.mm().status.mp = rig.m().status.max_mp();
    rig.step(240);
    let tornado: Vec<&HitRecord> = rig.dummies.1.iter().filter(|h| h.attack_name == "GOOFY_ART_TORNADO").collect();
    assert!(!tornado.is_empty(), "his Tornado on the pack: {:?}", rig.log());
    assert!(rig.dummies.1.iter().all(|h| !kh_core::host::may_hit_friendly(h.flags)), "every hit of AI Goofy passes through friendly NPCs");
    assert_eq!(rig.m().hit_flag(), kh_core::host::HIT_FLAG_AI);
    rig.party.set_pad(rig.id, Some(kh_core::Tuning::USER));
    assert_eq!(rig.m().hit_flag(), 0, "leading: the player's character hits friendly NPCs");
    rig.party.set_pad(rig.id, None);
    assert_eq!(rig.m().hit_flag(), kh_core::host::HIT_FLAG_AI, "AI again");
}

/// (PARTY-AI, AUDIT-A WORLDTIME) THE ONE TIME RULE for every character: with the host's [FRIEND] at x0.4 (Sora's shotlock aim)
/// an AI member's body and his AI run at 0.4 of the step; the member the player drives keeps his time; 1.0 gives it back.
#[test]
fn the_friend_rate_slows_ai_members_only_and_the_player_keeps_his_time() {
    assert_eq!((crate::party::time_rate(true, 0.4), crate::party::time_rate(false, 0.4)), (1.0, 0.4));
    let clip_time = |rate: f32, pad: bool| {
        let mut rig = Rig::new(&[APART], true, 3);
        if pad {
            rig.party.set_pad(rig.id, Some(kh_core::Tuning::USER));
        }
        rig.party.set_friend_rate(rate);
        assert_eq!(rig.party.member_rate(rig.id), if pad { 1.0 } else { rate });
        let t0 = rig.m().anim.time;
        rig.step(10);
        rig.m().anim.time - t0
    };
    let (full, slowed) = (clip_time(1.0, false), clip_time(0.4, false));
    assert!(full > 0.0, "his clip runs: {full}");
    assert!((slowed - full * 0.4).abs() < 1e-3, "AI Goofy at x0.4: {slowed} of {full}");
    assert!((clip_time(0.4, true) - full).abs() < 1e-3, "leading (a pad): his own time");
}

// ------------------------------------------------------------------------------------------------ (FINAL-FIX) FINAL 1a

/// (FINAL-FIX) The `goofy-ai:` lines of one rig step.
fn notes(rig: &mut Rig) -> Vec<String> {
    std::mem::take(&mut rig.party.gof.ai_notes)
}

/// (FINAL-FIX, FINAL 1a: "Goofy never switches to a broken enemy that isn't his own target") THE TRACE OF THE SWITCH. KH3's own
/// trees never pick a target by its stance (BT_n_Common_BattleStyle_Together's target-update service runs
/// EQ_NpcTarget_LockOnTarget every 20 s: near the player, the camera, the player's lock; none of its tests reads a DOWN state;
/// BT_n_ex002_BattleFlow has no such decorator either), and KH3 has no Goofy Breaker: the rule is the overhaul's (GOOFY_OVERHAUL.md
/// 12: any BROKEN enemy within `ai.breaker_chase` of him, his target or not). He strings A (his lock); B, beside him, is broken by
/// the debug word `x3g stagger other` (the in-game aid: the nearest enemy that is NOT his lock); he leaves A, walks to B, and the
/// Breaker begins on B (`BreakerStarted`: the hold that "Protect the slam" keys on, kh-er stance.rs `slam_held` / targets.rs
/// `protect_slam`) and lands on B; B stays BROKEN through the wind-up.
#[test]
fn final_1a_a_broken_enemy_beside_him_that_is_not_his_lock_gets_the_slam_and_the_trace_says_so() {
    let b = Vec3::new(0.0, 450.0, 0.0);
    let mut rig = Rig::new(&[NEAR, b], true, 7);
    rig.mm().status.goofy.as_mut().unwrap().abilities = crate::goofy::GoofyAbilities::default(); // the plain Breaker
    let mut n = 0;
    while crate::pad::goofy::ai_stance(rig.m()) != Stance::Striking && n < 300 {
        rig.one();
        n += 1;
    }
    assert_eq!(rig.m().status.goofy.as_ref().unwrap().run.ai.lock, T, "A is his lock");
    notes(&mut rig);
    rig.events.clear();
    let before = rig.dummies.1.len();
    rig.party.gof.word.push((crate::goofy::hooks::WORD_OTHER, 1000.0));
    let mut said = Vec::new();
    let mut started = None;
    let mut landed = None;
    for k in 0..420 {
        rig.one();
        said.extend(notes(&mut rig));
        if started.is_none() {
            started = rig.party.gof.events.iter().find_map(|(_, e)| match e {
                GoofyEvent::BreakerStarted { target, .. } => Some((k, *target)),
                _ => None,
            });
        } else if landed.is_none() {
            assert!(rig.party.gof.book.view(TargetId(2)).is_some_and(|v| v.broken.is_some()) || rig.dummies.1[before..].iter().any(|h| h.attack_name == "GOOFY_BREAKER"), "B stays BROKEN through the wind-up (the hold)");
        }
        if let Some(h) = rig.dummies.1[before..].iter().find(|h| h.attack_name == "GOOFY_BREAKER") {
            landed = Some((k, h.target));
            break;
        }
    }
    println!("{}", said.join("\n"));
    assert!(said.iter().any(|l| l.contains("x3g stagger: 1000 on target 2 (AI Goofy's lock 1)")), "the aid broke B, not his lock: {said:?}");
    assert!(said.iter().any(|l| l.starts_with("goofy-ai: target 2 BROKEN") && (l.contains("walks in to it") || l.contains("in his reach"))), "the switch is said: {said:?}");
    let (s, on) = started.unwrap_or_else(|| panic!("no Breaker began: {said:?}"));
    assert_eq!(on, TargetId(2), "the Breaker began on B (the hold Protect the slam keys on)");
    let (k, on) = landed.unwrap_or_else(|| panic!("no Breaker landed: {:?} {:?}", rig.mem().last, rig.log()));
    assert_eq!(on, TargetId(2), "GOOFY_BREAKER on B, not on his lock A");
    assert!(s < k && k < 300, "began at step {s}, landed at step {k}");
}

/// (FINAL-FIX, FINAL 1a: every in-game window broke an enemy 8-33 m from the party, `x3g stagger near` picks the nearest at
/// any distance) A BROKEN enemy beyond his walk range draws no walk, by the rule (GOOFY_OVERHAUL.md 12: within 6 m), and the
/// trace now says so instead of nothing.
#[test]
fn final_1a_a_broken_enemy_out_of_his_walk_range_gets_no_walk_and_the_trace_says_why() {
    let far = Vec3::new(0.0, K.ai_breaker_chase + 400.0, 0.0);
    let mut rig = Rig::new(&[NEAR, far], true, 7);
    rig.mm().status.goofy.as_mut().unwrap().abilities = crate::goofy::GoofyAbilities::default();
    rig.step(30);
    notes(&mut rig);
    let before = rig.dummies.1.len();
    rig.party.gof.word.push((TargetId(2), 1000.0));
    let mut said = Vec::new();
    for _ in 0..120 {
        rig.one();
        said.extend(notes(&mut rig));
    }
    assert!(rig.dummies.1[before..].iter().all(|h| h.target != TargetId(2)), "no slam on a body 10 m off");
    assert!(said.iter().any(|l| l.starts_with("goofy-ai: target 2 BROKEN") && l.contains("out of his walk range")), "{said:?}");
}

/// (FINAL-FIX) A hit window that never closes (a sensor that lost its enemy) let go of his shield after `ai.answer_max` but kept
/// `due` true for ever: no walk to a BROKEN enemy, no Art, no string while it stood. Now it stops counting with the shield.
#[test]
fn a_window_that_never_closes_does_not_keep_him_from_a_broken_enemy() {
    let mut b = foe(2, K.breaker_reach + 130.0);
    b.broken = 2.5;
    let enemies = [foe(1, 200.0), b];
    let friends = [lead(100, 0)];
    let t = [Telegraph { attacker: T, class: AttackClass::Normal, perilous: false, eta: 0.0, active: true }];
    let i = base(&enemies, &friends, &t);
    let mut mem = AiMemory::new(MemberId(1));
    let chased = (0..300).position(|_| think(&i, &mut mem, DT, &mut no_luck).chase == Some(TargetId(2)));
    let at = chased.expect("he walks to the BROKEN enemy once the window stops counting");
    assert!(at as f32 * DT >= K.ai_answer_max - 0.05, "not before the read attack's answer ran out: {at}");
}

/// (FINAL-FIX, the user 2026-11-03 "accepted": an Art cannot be cancelled) BUSY: mid-Art (or a reaction, a team attack) a
/// BROKEN enemy in his reach gets nothing until he is free; then the Breaker. The trace says which.
#[test]
fn the_busy_cases_take_no_breaker_until_he_is_free_accept_it() {
    let mut b = foe(2, 200.0);
    b.broken = 2.5;
    let enemies = [foe(1, 150.0), b];
    let friends = [lead(100, 0)];
    let mut i = base(&enemies, &friends, &[]);
    i.stance = Stance::Busy;
    i.breaker = false; // (`breaker_offered` is false while the Art runs: `can_start`)
    let mut mem = AiMemory::new(MemberId(1));
    for _ in 0..60 {
        let out = think(&i, &mut mem, DT, &mut no_luck);
        assert!(!out.pressed.has(PadButton::FaceTop) && out.chase.is_none(), "nothing mid-Art");
        assert!(break_verdict(&i, &b, &out, &mem).starts_with("busy"), "{}", break_verdict(&i, &b, &out, &mem));
    }
    // the Art is over: Y is offered, he takes it after the beat
    i.stance = Stance::Free;
    i.breaker = true;
    let pressed = (0..30).find_map(|_| {
        let out = think(&i, &mut mem, DT, &mut no_luck);
        out.pressed.has(PadButton::FaceTop).then_some(out)
    });
    let out = pressed.expect("the Breaker once free");
    assert_eq!(out.lock, TargetId(2), "on the BROKEN one, not the nearer enemy");
    assert_eq!(break_verdict(&i, &b, &out, &mem), "Goofy Breaker pressed (rule 1)");
    // a string is not busy: the walk waits for it to end (the Breaker in reach cancels it: pad/goofy_breaker.rs can_start)
    i.stance = Stance::Striking;
    i.breaker = false;
    let mut far = b;
    far.pos = Vec3::new(K.breaker_reach + 130.0, 0.0, 0.0);
    let none = AiOut::default();
    assert_eq!(break_verdict(&i, &far, &none, &AiMemory::new(MemberId(1))), "not free yet (a string, his guard, the air): the walk waits");
    i.style.battle = STYLE_HANDS_OFF;
    i.stance = Stance::Free;
    assert_eq!(break_verdict(&i, &far, &none, &AiMemory::new(MemberId(1))), "Hands Off: no walk to it");
}

/// (FINAL-FIX, FINAL 12: "No log line exists") The members' time rates are said when they change: the AI friends at KH3's
/// [FRIEND] x0.4 during Sora's shotlock aim, the member the player drives at x1, back to x1 after; nothing when nothing changed.
#[test]
fn the_time_rate_line_proves_the_friends_x04_and_the_players_x1() {
    let mut rig = Rig::new(&[APART], true, 3);
    let donald = rig.party.spawn(Chara::Donald, Vec3::new(-200.0, 0.0, 0.0), 0.0);
    let mut said = Vec::new();
    let first = rig.party.rate_note(&mut said).expect("the first line");
    assert!(first.contains("[FRIEND] x1.000"), "{first}");
    assert_eq!(rig.party.rate_note(&mut said), None, "nothing changed: no line");
    rig.party.set_pad(donald, Some(kh_core::Tuning::USER)); // Donald leads
    rig.party.rate_note(&mut said);
    rig.party.set_friend_rate(0.4);
    let aim = rig.party.rate_note(&mut said).expect("the aim's line");
    assert!(aim.contains("[FRIEND] x0.400") && aim.contains("Goofy x0.400 (AI: KH3 [FRIEND])") && aim.contains("Donald x1.000 (the player drives him: his own time)"), "{aim}");
    assert_eq!(rig.party.rate_note(&mut said), None);
    rig.party.set_friend_rate(1.0);
    let back = rig.party.rate_note(&mut said).expect("the line after the aim");
    assert!(back.contains("Goofy x1.000"), "{back}");
}

/// (FINAL-FIX, THE USER 2026-11-05: "do option 1") HE RUNS TO A BROKEN ENEMY AT HIS OWN RUN SPEED. As in game (FINAL-FIX launch
/// 1: 1.2 m in 1.4 s, the window ran out): his KH3 tree is in Battle Follow mode (`SetBattleFollowMode`, the Together style), the
/// leader stands, and his tree keeps asking a WALK every step; B breaks 6 m off (edge) beyond a crowd (two enemies and Donald
/// between). He runs at `move.run` in his battle set (no walk, no follow-rule crawl), and Goofy Breaker begins on B inside KH3's
/// own 2.5 s window (nothing holds it longer before the slam) and lands on B.
#[test]
fn final_fix_from_6_m_through_a_crowd_he_runs_and_slams_inside_the_window() {
    let b = Vec3::new(0.0, K.ai_breaker_chase + 50.0 - 10.0, 0.0); // edge 590 cm (radius 50)
    let mut rig = Rig::new(&[NEAR, b, Vec3::new(-100.0, 400.0, 0.0), Vec3::new(100.0, 430.0, 0.0)], true, 7); // the crowd beyond his Tornado's whirl (the busy case is its own test)
    rig.party.spawn(Chara::Donald, Vec3::new(0.0, 260.0, 0.0), 0.0); // a mate in his way (the mates' avoidance)
    rig.mm().status.goofy.as_mut().unwrap().abilities = crate::goofy::GoofyAbilities::default();
    rig.mm().battle_follow = true; // his tree's Battle Follow mode: the follow rule would hold him to the standing leader's pace
    rig.step(20);
    notes(&mut rig);
    let before = rig.dummies.1.len();
    rig.party.gof.word.push((TargetId(2), 1000.0));
    rig.step(1);
    let window = rig.party.gof.book.view(TargetId(2)).and_then(|v| v.broken).map(|w| w.0).expect("B is BROKEN");
    assert!(window <= 2.5 + 1e-3, "KH3's own window: {window}");
    let (mut run, mut walked, mut began): (Option<(Vec3, Vec3, usize)>, bool, Option<f32>) = (None, false, None);
    for k in 0..300 {
        // his tree asks him to walk after the leader every step (refused while he runs to B: ai_chase_refuses)
        let _ = rig.party.command(rig.id, MemberCommand::MoveTo { goal: MoveGoal::Sora, locomotion: LocomotionDef::Move1, walk: true, acceptable_radius: 100.0, follow_path: true });
        rig.one();
        for l in notes(&mut rig) {
            println!("{k}: {l}");
        }
        let m = rig.m();
        if m.status.goofy.as_ref().unwrap().run.ai_chase.is_some() && m.state == NpcStateId::Move {
            let at = m.position();
            run = Some(run.map_or((at, at, 0), |(from, _, n)| (from, at, n + 1)));
            walked |= m.anim.clip.name.starts_with("WalkLoopN");
        }
        if began.is_none() && rig.party.gof.events.iter().any(|(_, e)| matches!(e, GoofyEvent::BreakerStarted { target: TargetId(2), .. })) {
            began = Some(k as f32 * DT);
        }
        if rig.dummies.1[before..].iter().any(|h| h.attack_name == "GOOFY_BREAKER") {
            break;
        }
    }
    let began = began.unwrap_or_else(|| panic!("no Breaker began on B: {:?} {:?} at {:?}", rig.mem().last, rig.log(), rig.m().position()));
    assert!(began < window, "the Breaker began at {began:.2} s, inside the {window:.2} s window");
    let (from, to, n) = run.expect("he ran to B");
    let pace = crate::pawn::horizontal(to - from).length() / (n.max(1) as f32 * DT);
    assert!((pace - K.move_run).abs() < 0.1 * K.move_run, "his own run speed ({} cm/s), not a walk or the follow crawl: {pace:.0} cm/s over {n} steps", K.move_run);
    assert!(!walked, "a run in his battle set, never the walk loop");
    let slam = rig.dummies.1[before..].iter().find(|h| h.attack_name == "GOOFY_BREAKER").expect("the slam landed");
    assert_eq!(slam.target, TargetId(2), "on B");
}

// ------------------------------------------------------------------------------------------------ (COMBAT-FIX) the offline duel
// THE USER 2026-11-07: "goofy as an ai ally at level 1 should have a 5% chance, and a 65% change at level 99. Goofy should still
// use his attacks and abilties as well as deflecting".

/// What one minute of the duel measured.
#[derive(Clone, Copy, Debug, Default)]
struct DuelTally {
    /// His hits that landed on the enemies, per minute.
    hits: f32,
    /// Arts (and Taunts) he started, per minute.
    arts: f32,
    /// Parries (any technique, right or not), per minute.
    parries: f32,
    /// The share of the minute he stood in his guard with no swing at him due within 0.5 s (waiting behind the shield).
    idle_guard: f32,
    /// The share of the minute in his guard at all.
    guard: f32,
    /// The share of the minute in a hit reaction (a blow he did not answer landed).
    hurt: f32,
}

/// ONE MINUTE OF AI GOOFY (no tree: his brain, his pad moves and the one judgement of a hit) beside the leader, against three
/// enemies around him (gaps 70 - 90 cm), which swing at him in turn, one swing every `every` s (each on the list 0.8 s ahead, its hit
/// window 0.12 s, the hit 0.06 s in, 5 HP; his HP and GUARD are never what stops him: HP is topped up), with Bash and Tornado
/// equipped at every rank his level gives.
fn duel(level99: bool, seed: u64, every: f32) -> DuelTally {
    use crate::pad::goofy::{ART, GUARD, GUARD_HIT, TAUNT};
    let foes = [Vec3::new(140.0, 0.0, 0.0), Vec3::new(110.0, 110.0, 0.0), Vec3::new(110.0, -110.0, 0.0)];
    let mut rig = Rig::new(&foes, level99, seed);
    rig.mm().status.goofy.as_mut().unwrap().run.arts.equipped = Some([crate::pad::goofy_arts::Art::Tornado.id(), crate::pad::goofy_arts::Art::Bash.id()]);
    rig.events.clear();
    let (period, eta, active, after) = (3.0 * every, 0.8f32, 0.12f32, 0.06f32);
    let steps = 60 * 60;
    let mut tally = DuelTally::default();
    let (mut arts, mut idle_guard, mut guard, mut hurt) = (0usize, 0usize, 0usize, 0usize);
    let mut was_art = false;
    for n in 0..steps {
        let t = n as f32 * DT;
        let mut tele = Vec::new();
        let mut hits = Vec::new();
        let mut due_soon = false;
        for k in 0..foes.len() {
            let local = t - every * k as f32;
            if local < 0.0 {
                continue;
            }
            let phase = local % period;
            let id = TargetId(k as u32 + 1);
            let class = AttackClass::Normal;
            if phase < eta {
                tele.push(Telegraph { attacker: id, class, perilous: false, eta: eta - phase, active: false });
                due_soon |= eta - phase <= 0.5;
            } else if phase < eta + active {
                tele.push(Telegraph { attacker: id, class, perilous: false, eta: 0.0, active: true });
                due_soon = true;
                if phase - DT < eta + after && phase >= eta + after {
                    hits.push((id, ()));
                }
            }
        }
        rig.party.set_telegraphs(&tele);
        for (id, _) in hits {
            let me = rig.m().position();
            let at = rig.dummies.0.iter().find(|d| d.0 == id).map_or(me, |d| d.1);
            let mut h = HitRecord::empty(id);
            h.attack_name = "er.body";
            h.damage = 5.0;
            h.damage_kind = Kind::Small;
            h.direction = (me - at).normalize_or_zero();
            h.position = me + Vec3::Z * 60.0;
            h.attacker = HitAttacker { position: at, yaw: 180.0, radius: 50.0, valid: true };
            h.attack_info = AttackInfo { class: AttackClass::Normal, perilous: false, attacker: Some(id) };
            let _ = rig.party.receive_hit(rig.id, &h);
        }
        rig.one();
        // the enemies keep around him (his KH3 tree walks him back in after a blow knocked him away: this rig has none)
        let me = rig.m().position();
        for (k, d) in rig.dummies.0.iter_mut().enumerate() {
            d.1 = Vec3::new(me.x, me.y, 0.0) + foes[k];
        }
        let state = rig.m().state;
        let in_art = matches!(state, NpcStateId::Pad(ART | TAUNT));
        arts += (in_art && !was_art) as usize;
        was_art = in_art;
        let guarding = matches!(state, NpcStateId::Pad(GUARD | GUARD_HIT));
        guard += guarding as usize;
        idle_guard += (guarding && !due_soon) as usize;
        hurt += matches!(state, NpcStateId::Damage) as usize;
        let m = rig.mm();
        m.status.hp = m.status.max_hp();
    }
    let minutes = steps as f32 * DT / 60.0;
    tally.hits = rig.dummies.1.len() as f32 / minutes;
    tally.arts = arts as f32 / minutes;
    tally.parries = rig.events.iter().filter(|e| matches!(e, GoofyEvent::Parried { .. })).count() as f32 / minutes;
    tally.idle_guard = idle_guard as f32 / steps as f32;
    tally.guard = guard as f32 / steps as f32;
    tally.hurt = hurt as f32 / steps as f32;
    tally
}

/// The duel at level 1 and 99, averaged over seeds: printed (`-- --nocapture`), and he keeps attacking, using his Arts and
/// deflecting; he never waits behind his shield for a swing that is not coming.
#[test]
fn offline_duel_he_attacks_uses_his_arts_and_deflects_at_level_1_and_99() {
    let avg = |level99: bool, every: f32| {
        let runs: Vec<DuelTally> = (1..=8).map(|s| duel(level99, s, every)).collect();
        let n = runs.len() as f32;
        DuelTally {
            hits: runs.iter().map(|r| r.hits).sum::<f32>() / n,
            arts: runs.iter().map(|r| r.arts).sum::<f32>() / n,
            parries: runs.iter().map(|r| r.parries).sum::<f32>() / n,
            idle_guard: runs.iter().map(|r| r.idle_guard).sum::<f32>() / n,
            guard: runs.iter().map(|r| r.guard).sum::<f32>() / n,
            hurt: runs.iter().map(|r| r.hurt).sum::<f32>() / n,
        }
    };
    let mut seen = Vec::new();
    for (every, name, t) in [(2.0, "level 1", avg(false, 2.0)), (2.0, "level 99", avg(true, 2.0)), (5.0, "level 1", avg(false, 5.0)), (5.0, "level 99", avg(true, 5.0))] {
        println!("AI Goofy duel {name}, a swing at him every {every} s: hits landed {:.1}/min, arts {:.1}/min, parries {:.1}/min, in guard {:.1}% of the time, idle in guard {:.1}%, in a hit reaction {:.1}%", t.hits, t.arts, t.parries, t.guard * 100.0, t.idle_guard * 100.0, t.hurt * 100.0);
        seen.push((name, t));
    }
    for (name, t) in &seen {
        assert!(t.idle_guard < 0.02, "{name}: he does not wait behind his shield: {:.3}", t.idle_guard);
        assert!(t.hits > 30.0, "{name}: he keeps attacking: {:.1} hits a minute", t.hits);
    }
    // level 99 knows his Arts and uses them; his deflects grow with his level (5 % to 65 % of what he reads)
    assert!(seen.iter().filter(|(n, _)| *n == "level 99").all(|(_, t)| t.arts > 2.0), "{seen:?}");
    assert!(seen[1].1.parries > seen[0].1.parries * 3.0 && seen[3].1.parries > seen[2].1.parries * 3.0, "{seen:?}");
}
