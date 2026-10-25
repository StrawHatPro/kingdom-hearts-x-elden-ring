//! (BM, the user: "combat animations shouldn't start when only animals are around you") PASSIVE WILDLIFE STARTS NO
//! BATTLE MODE, whoever leads. The rule (targets.rs `engages`, `ErTargets::passive_now`): a hostile whose kind's NpcParam
//! threatLv is 0 (the game's own threat level; the user's logs + the game's regulation: the animal kinds c6001 / c6010 /
//! c6040 / c6050 / c6060 / c6070 / c6090 / c6100 are team 6 with threatLv 0, the fighters 1, bosses up to 9) engages
//! the party only while it fights it: it attacks (an attack animation seen), it hit a member or a member hit it, in the
//! last 5 s. The three places battle mode comes from are proven here:
//! - Sora's battle flag (kh-core pawn_tick) = `ErTargets::engaged_count` round Sora;
//! - a friend leader's battle flag (kh-partyhost `Leadership::battle_step`) = the same count round the leader
//!   (`ErPartyTargets::engaged`) + the party aggro over the trees' hostile actors (kh-partyhost runtime `party_aggro`,
//!   which counts `ai_actors` with `hostile`);
//! - the friends' battle state (kh-party `set_in_battle`) = that same flag.

use glam::Vec3;
use kh_ai::{ActorRef, ActorView};
use kh_core::{TargetId, Targets};
use kh_partyhost::PartyTargets;

use crate::coords::Frame;
use crate::ermap::{Params, Weight};
use crate::party::aiworld::ErPartyTargets;
use crate::targets::{engages, Enemy, ErTargets};

/// A passive animal (threatLv 0, the deer's 55 HP) or a fighter (threatLv 1) `cm` from the origin on the X axis.
fn chr(id: u32, x: f32, passive: bool) -> Enemy {
    let mut e = Enemy::stub(id, Vec3::new(x, 0.0, 0.0), 40.0, 90.0, 55, Weight::Small);
    e.threat_lv = Some(u32::from(!passive));
    e.passive = passive;
    e.npc_id = if passive { 6060 } else { 3000 };
    e
}

fn world(list: Vec<Enemy>) -> ErTargets {
    let mut t = ErTargets::new(Frame::at(Vec3::ZERO), Params::default());
    t.list = list;
    t.step = 1000;
    t.sora = Vec3::ZERO;
    t
}

/// The three leaders' points: Sora at the origin; Donald and Goofy leading 30 m away with the same animals round them
/// (AI Sora / where Sora was left stays at the origin). Returns (who, the engaged count, the trees' hostile actors).
fn every_leader(make: &dyn Fn(Vec3) -> ErTargets) -> Vec<(&'static str, u32, usize)> {
    let mut out = Vec::new();
    for (who, at) in [("Sora", Vec3::ZERO), ("Donald", Vec3::new(0.0, 3000.0, 0.0)), ("Goofy", Vec3::new(0.0, -3000.0, 0.0))] {
        let mut t = make(at);
        if who == "Sora" {
            let n = t.engaged_count();
            let mut latch = 0;
            let p = ErPartyTargets::new(&mut t, at, &mut latch);
            let mut actors: Vec<ActorView> = Vec::new();
            p.ai_actors(&mut actors);
            out.push((who, n, actors.iter().filter(|a| a.hostile).count()));
        } else {
            let mut latch = 0;
            let p = ErPartyTargets::new(&mut t, at, &mut latch);
            let n = Targets::engaged_count(&p);
            let mut actors: Vec<ActorView> = Vec::new();
            p.ai_actors(&mut actors);
            out.push((who, n, actors.iter().filter(|a| a.hostile).count()));
        }
    }
    out
}

/// Animals round `at`: three passive bodies at 2, 4 and 6 m (inside the 8 m engage range).
fn animals_at(at: Vec3) -> Vec<Enemy> {
    vec![chr(1, 200.0, true), chr(2, 400.0, true), chr(3, 600.0, true)].into_iter().map(|mut e| { e.pos += at; e }).collect()
}

#[test]
fn bm_the_rule_table() {
    // (within range, passive, fought, attacking)
    assert!(engages(true, false, false, false), "a threat within range engages (as before)");
    assert!(!engages(false, false, false, false), "a threat out of range does not");
    assert!(!engages(true, true, false, false), "a passive animal standing near: no battle");
    assert!(engages(true, true, false, true), "a passive animal that attacks within range engages");
    assert!(engages(true, true, true, false) && engages(false, true, true, false), "a passive animal that fought the party engages wherever it is");
    assert!(!engages(false, true, false, true), "an animal attacking something else 20 m off: no battle");
}

#[test]
fn bm_animals_only_start_no_battle_for_any_leader() {
    let rows = every_leader(&|at| world(animals_at(at)));
    for (who, n, hostile) in &rows {
        assert_eq!(*n, 0, "{who}: animals only within 6 m: no engaged count, no battle flag");
        assert_eq!(*hostile, 0, "{who}: animals only: no enemy for the friends' trees and no party aggro");
    }
    // the rule switched off (`target.passive = 0`): the old behaviour, every one of them engages
    let mut t = world(animals_at(Vec3::ZERO));
    for e in &mut t.list {
        e.passive = false; // what refresh gives with passive_rule off
    }
    assert_eq!(t.engaged_count(), 3);
}

#[test]
fn bm_an_animal_that_attacks_starts_the_battle_and_counts() {
    let rows = every_leader(&|at| {
        let mut t = world(animals_at(at));
        t.list[1].in_attack = true; // the one at 4 m charges (an attack animation x3000 - x3999)
        t.note_attacks_seen();
        t.list[1].in_attack = false; // its swing is over: the 5 s memory keeps it engaged
        t
    });
    for (who, n, hostile) in &rows {
        assert_eq!((*n, *hostile), (1, 1), "{who}: the attacking animal (and only it) is engaged and an enemy of the trees");
    }
    // 5 s later without another swing it is passive again
    let mut t = world(animals_at(Vec3::ZERO));
    t.list[1].in_attack = true;
    t.note_attacks_seen();
    t.list[1].in_attack = false;
    t.step += 301;
    assert_eq!(t.engaged_count(), 0);
}

#[test]
fn bm_an_animal_that_hits_or_is_hit_by_the_party_counts() {
    // it hit the leader (ErTargets::note_attack: sora.rs / allybody_live.rs on the game's own hit)
    let rows = every_leader(&|at| {
        let mut t = world(animals_at(at));
        t.note_attack(TargetId(3));
        t
    });
    for (who, n, hostile) in &rows {
        assert_eq!((*n, *hostile), (1, 1), "{who}: the animal that hit a member counts");
    }
    // the leader (or a friend) hit it: apply_hit's memory write; it counts even when it flees out of range
    let rows = every_leader(&|at| {
        let mut t = world(animals_at(at));
        t.note_hit(TargetId(1));
        t.list[0].pos = at + Vec3::new(2500.0, 0.0, 0.0); // fled 25 m
        t
    });
    for (who, n, hostile) in &rows {
        assert_eq!((*n, *hostile), (1, 1), "{who}: the animal the party hit counts (wherever it ran)");
    }
    // the hit animal is a target of the friends' trees (FX5 `threatens`: fighting the party)
    let mut t = world(animals_at(Vec3::ZERO));
    t.note_hit(TargetId(2));
    let mut latch = 0;
    let p = ErPartyTargets::new(&mut t, Vec3::ZERO, &mut latch);
    let mut actors = Vec::new();
    p.ai_actors(&mut actors);
    let hostile: Vec<ActorRef> = actors.iter().filter(|a| a.hostile).map(|a| a.actor).collect();
    assert_eq!(hostile, vec![ActorRef::Enemy(2)]);
    assert!(p.fighting_party(2) && !p.fighting_party(1));
    // every animal stays lockable: the leader can still choose to fight one
    assert!(actors.iter().all(|a| a.lockable));
}

#[test]
fn bm_a_hostile_starts_the_battle_with_animals_round_it() {
    let rows = every_leader(&|at| {
        let mut l = animals_at(at);
        l.push({ let mut e = chr(9, 0.0, false); e.pos = at + Vec3::new(500.0, 0.0, 0.0); e }); // a fighter at 5 m
        world(l)
    });
    for (who, n, hostile) in &rows {
        assert_eq!((*n, *hostile), (1, 1), "{who}: the fighter alone engages; the animals stay out");
    }
    // a fighter 20 m off (out of the 8 m engage range) does not; as before
    let rows = every_leader(&|at| {
        let mut l = animals_at(at);
        l.push({ let mut e = chr(9, 0.0, false); e.pos = at + Vec3::new(2000.0, 0.0, 0.0); e });
        world(l)
    });
    for (who, n, _) in &rows {
        assert_eq!(*n, 0, "{who}: a fighter out of range starts no battle (unchanged)");
    }
}
