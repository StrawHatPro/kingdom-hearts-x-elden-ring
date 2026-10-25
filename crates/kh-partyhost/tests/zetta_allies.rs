//! (ZETTA) The allies leave Zettaflare's target alone (the user: "the allies you have target everybody else except for the
//! zettaflare's target"): the friends' trees see it as a bystander while kh-party reserves it (`Party::don_zetta_reserved`).

use kh_ai::{ActorRef, ActorView};
use kh_core::TargetId;
use kh_partyhost::aiworld::EnemyFeed;

fn enemy(id: u32) -> ActorView {
    ActorView { actor: ActorRef::Enemy(id), hostile: true, alive: true, ..ActorView::default() }
}

#[test]
fn the_reserved_target_is_a_bystander_of_the_trees() {
    let mut feed = EnemyFeed { actors: vec![enemy(1), enemy(2)], attacked: false, left_out: 0 };
    assert_eq!(feed.reserve(&[TargetId(1)]), 1);
    let mut out = Vec::new();
    feed.enemies(&mut out);
    assert_eq!(out.iter().map(|a| a.actor).collect::<Vec<_>>(), vec![ActorRef::Enemy(2)], "they fight the other one");
    // the only enemy left: none for the trees (they hold off: they stay by the leader)
    let mut feed = EnemyFeed { actors: vec![enemy(1)], attacked: false, left_out: 0 };
    feed.reserve(&[TargetId(1)]);
    let mut out = Vec::new();
    feed.enemies(&mut out);
    assert!(out.is_empty());
    assert!(feed.actor(1).is_some_and(|a| !a.hostile), "still known (a bystander), never attacked");
    // nothing reserved: nothing changes
    let mut feed = EnemyFeed { actors: vec![enemy(1), enemy(2)], attacked: false, left_out: 0 };
    assert_eq!(feed.reserve(&[]), 0);
}
