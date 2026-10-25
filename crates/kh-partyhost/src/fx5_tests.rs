//! (FX5) THE FRIENDS FIGHT WHAT THREATENS THE PARTY (aiworld.rs `threatens` / `EnemyFeed::keep_threats`): an enemy is
//! in the trees' enemy list only within the threat range of the leader or a member who is up, while it fights the
//! party (the host's memory), or as the leader's hard lock-on target. Pure rules; the scenario with the real trees is
//! tests/aix.rs `fx5_*`.

use crate::aiworld::{threatens, EnemyFeed, THREAT_RANGE};
use kh_ai::{ActorRef, ActorView};
use kh_core::host::TargetId;
use kh_core::Vec3;

fn enemy(id: u32, x: f32, z: f32) -> ActorView {
    ActorView { actor: ActorRef::Enemy(id), position: Vec3::new(x, 0.0, z), center: Vec3::new(x, 0.0, z + 90.0), radius: 50.0, half_height: 90.0, hostile: true, lockable: true, ..ActorView::default() }
}

const NOBODY: fn(u32) -> bool = |_| false;

#[test]
fn fx5_the_threat_range_is_measured_from_the_leader_and_every_member_to_the_enemys_edge() {
    let leader = [Vec3::ZERO];
    // 12 m to the edge: in; 30 m (the CV3 camp): out
    assert!(threatens(&enemy(1, THREAT_RANGE + 49.0, 0.0), &leader, TargetId::NONE, THREAT_RANGE, &NOBODY));
    assert!(!threatens(&enemy(2, 3000.0, 0.0), &leader, TargetId::NONE, THREAT_RANGE, &NOBODY));
    assert!(!threatens(&enemy(3, THREAT_RANGE + 51.0, 0.0), &leader, TargetId::NONE, THREAT_RANGE, &NOBODY));
    // a member who is up 25 m out (Donald ahead of the leader): an enemy 6 m from HIM is a threat
    let party = [Vec3::ZERO, Vec3::new(2500.0, 0.0, 0.0)];
    assert!(threatens(&enemy(4, 3100.0, 0.0), &party, TargetId::NONE, THREAT_RANGE, &NOBODY));
    // 3-D: an enemy on a cliff 20 m above the leader is no threat of the moment
    assert!(!threatens(&enemy(5, 300.0, 2000.0), &leader, TargetId::NONE, THREAT_RANGE, &NOBODY));
}

#[test]
fn fx5_a_far_enemy_that_fights_the_party_or_is_the_leaders_lock_stays_a_threat() {
    let leader = [Vec3::ZERO];
    let archer = enemy(7, 3500.0, 0.0);
    assert!(!threatens(&archer, &leader, TargetId::NONE, THREAT_RANGE, &NOBODY));
    assert!(threatens(&archer, &leader, TargetId::NONE, THREAT_RANGE, &|id| id == 7), "it hit a member / was hit: the host's memory");
    assert!(threatens(&archer, &leader, TargetId(7), THREAT_RANGE, &NOBODY), "the leader locked on to it");
    assert!(!threatens(&archer, &leader, TargetId(8), THREAT_RANGE, &NOBODY), "another enemy's lock is not its");
    // range 0 = the rule off (every hostile, as before FX5)
    assert!(threatens(&archer, &leader, TargetId::NONE, 0.0, &NOBODY));
    // a non-enemy actor is never filtered
    assert!(threatens(&ActorView { actor: ActorRef::Player, ..archer.clone() }, &leader, TargetId::NONE, THREAT_RANGE, &NOBODY));
}

#[test]
fn fx5_the_feed_turns_what_does_not_threaten_into_bystanders_the_trees_never_see() {
    let mut feed = EnemyFeed { actors: vec![enemy(1, 500.0, 0.0), enemy(2, 3000.0, 0.0), enemy(3, 3200.0, 0.0), ActorView { hostile: false, ..enemy(4, 100.0, 0.0) }], attacked: false, left_out: 0 };
    let n = feed.keep_threats(&[Vec3::ZERO], TargetId::NONE, THREAT_RANGE, &|id| id == 3);
    assert_eq!(n, 1, "only the camp's idle one (2) is left out; 3 fights the party; 4 was a bystander already");
    let mut list = Vec::new();
    feed.enemies(&mut list);
    let ids: Vec<ActorRef> = list.iter().map(|a| a.actor).collect();
    assert_eq!(ids, vec![ActorRef::Enemy(1), ActorRef::Enemy(3)]);
    // an actor look-up still finds the bystander (its place for the camera / a decorator), not hostile
    assert!(feed.actor(2).is_some_and(|a| !a.hostile && a.alive));
}
