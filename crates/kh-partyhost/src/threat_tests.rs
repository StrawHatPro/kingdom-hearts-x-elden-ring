//! (GF-THREAT) THE TEST LEVEL'S ENEMY CHOICE WITH GOOFY'S TAUNT AND THE THREAT BOOK: `victim` of a fixture dummy, so threat works without
//! Elden Ring. A dummy's choice follows the book by a roll, a Taunt in reach takes it, and with neither the nearest rule stands.

use kh_core::{TargetId, Vec3};
use kh_host::party::PartyChara;
use kh_party::goofy::threat;

use crate::k1_tests::Harness;
use crate::runtime::Victim;

fn ready() -> Option<Harness> {
    let mut h = Harness::new()?;
    for _ in 0..30 {
        h.step(None);
    }
    Some(h)
}

fn goofy_at(h: &Harness) -> Vec3 {
    h.rt.current.iter().find(|v| v.chara == kh_party::Chara::Goofy).expect("Goofy is in the party").position
}

/// Of `n` dummies standing at `from`, how many choose Goofy.
fn chosen(h: &Harness, from: Vec3, n: u32) -> usize {
    (1..=n).filter(|k| h.rt.victim(*k, from, h.out.position) == Victim::Friend(PartyChara::Goofy)).count()
}

#[test]
fn gf_threat_shifts_a_dummys_choice_taunt_takes_it_and_both_end() {
    let Some(mut h) = ready() else {
        println!("no party AI pack: skipped");
        return;
    };
    let g = h.rt.id(PartyChara::Goofy).expect("Goofy");
    // a spot at Sora's side, nearer him than Goofy: the nearest rule never gives Goofy
    let from = h.out.position + (h.out.position - goofy_at(&h)).normalize_or(Vec3::X) * 200.0;
    assert_eq!(chosen(&h, from, 60), 0, "no threat, no taunt: the nearest rule stands");
    // threat on half the dummies: those lean to Goofy by a roll, the others keep the nearest rule
    for k in 1..=60 {
        h.rt.party.gof.threat.add(TargetId(k), g, 30.0);
    }
    let drawn = chosen(&h, from, 60);
    let others = (61..=120).filter(|k| h.rt.victim(*k, from, h.out.position) == Victim::Friend(PartyChara::Goofy)).count();
    println!("threat: {drawn} of 60 dummies choose Goofy");
    assert!(drawn >= 25, "{drawn}");
    assert!(drawn < 60, "a bias, not a lock");
    assert_eq!(others, 0, "no row on those");
    // a Taunt in reach takes every dummy; out of reach it does nothing
    h.rt.party.gof.threat = Default::default();
    let centre = goofy_at(&h);
    h.rt.party.status_mut(g).unwrap().goofy.as_mut().unwrap().run.taunt = Some(kh_party::pad::goofy::Taunt { centre, radius: 1000.0, left: 5.0 });
    assert_eq!(chosen(&h, from, 60), 60, "in range");
    assert_eq!(h.rt.gof_victim(7, centre + Vec3::new(1500.0, 0.0, 0.0)), None, "out of range: the next rule stands");
    assert_eq!(threat::taunter_of(&h.rt.party, centre + Vec3::new(999.0, 0.0, 0.0)), Some(g));
    // it ends on its own clock
    h.rt.party.status_mut(g).unwrap().goofy.as_mut().unwrap().run.taunt = Some(kh_party::pad::goofy::Taunt { centre, radius: 1000.0, left: 0.05 });
    for _ in 0..6 {
        h.step(None);
    }
    assert!(h.rt.party.goofy_taunt(g).is_none());
    assert_eq!(chosen(&h, from, 60), 0, "it ended: the nearest rule again");
}

/// A dummy's explicit `dummy-target` stands; Taunt Pass stays Donald's route: with an ally's guard up, the book does not choose.
#[test]
fn gf_threat_leaves_explicit_targets_and_taunt_pass_alone() {
    let Some(mut h) = ready() else {
        println!("no party AI pack: skipped");
        return;
    };
    let g = h.rt.id(PartyChara::Goofy).expect("Goofy");
    h.rt.party.gof.threat.add(TargetId(5), g, 30.0);
    h.rt.set_victim(5, Victim::Sora);
    assert_eq!(h.rt.victim(5, Vec3::ZERO, h.out.position), Victim::Sora);
    kh_party::donald::ai::note_guard(&mut h.rt.party, g);
    assert_eq!(h.rt.gof_victim(6, Vec3::ZERO), None);
}
