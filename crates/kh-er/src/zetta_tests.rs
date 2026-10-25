//! (ZETTA) Zettaflare is unstoppable on Elden Ring's characters, whatever they are (the user: "Zettaflare ignores all armor,
//! it takes priority over ANYTHING" and "this needs to be a dynamic system that just works"): the puppet's reaction rules
//! (poise meter, revenge armour, chain cap, a giant body's endurance) are passed by a forced hit for a small soldier, a
//! knight, a giant armoured body, a boss-sized one with maximal poise in its hyper armour mid-attack; a plain hit is refused
//! where those rules say so (the force is what gets it through). No per-enemy data: the stubs differ only in what an
//! enemy exposes at run time (size, weight, hit points, poise, its attack state).

use crate::ermap::{Params, Weight};
use crate::puppet::Puppets;
use crate::targets::Enemy;
use glam::Vec3;
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::{HitRecord, TargetId};
use kh_react::HitOutcome;

/// (2026-10-23) the user: "make sure Zettaflare scales with your magic stat": the Elden Ring route keeps the core's number
/// (its power x Donald's Magic) in step: a forced hit asks the HP it carries on one scale (`damage_out`), the measured
/// negation of the target cancelled (targets.rs: the ask / its ratio), so twice the Magic damage is twice the HP asked.
#[test]
fn zetta_the_er_route_keeps_the_magic_scaled_damage_in_step() {
    let p = Params::default();
    let (low, high) = (p.damage_out(800.0), p.damage_out(1600.0));
    assert!(low > 0 && high > low, "more KH damage, more ER HP asked ({low} / {high})");
    assert_eq!(high, low * 2, "on one scale: no cap, no boss scaling here");
    // the forced hit's ask through a measured negation (a boss that took a fifth): the full number, whatever its defence
    let ratio = 0.2f32;
    let ask = |w: i32| ((w as f32 / ratio).round() as i32).min(60000);
    assert_eq!((ask(high) as f32 * ratio).round() as i32, high);
}

/// (2026-10-23) the user: "make level 99 Zettaflare do 20,000 damage" / "at level 99, Malenia should die from one Zettaflare";
/// the user's log: Malenia (c2120, 18473 HP) took 8217 of a bullet asking 9940. A forced hit lands in full: the bullet asks
/// the number over the negation measured on it, and what the game kept is written to its HP (targets.rs `reconcile`).
#[test]
fn zetta_a_forced_hit_lands_in_full_and_level_99_kills_malenia() {
    let p = Params::default();
    let full = p.damage_out(2857.0); // level 99: Magic 71 x power 40.2394
    assert!((full - 20000).abs() <= 7, "{full}");
    // first hit on her: nothing measured yet, the bullet asks the full number; she keeps 17.3 % (the user's run)
    let ask = crate::targets::forced_ask(full, 0.0);
    assert_eq!(ask, full);
    let took = (ask as f32 * 8217.0 / 9940.0).round() as i32;
    let short = crate::targets::forced_shortfall(full, took, ask, ask);
    assert_eq!(took + short, full, "in full");
    assert!(took + short >= 18473, "Malenia dies from one level-99 Zettaflare");
    // exactly in full, never more: the bullet asks the full number whatever her negation, the rest is written
    assert_eq!(crate::targets::forced_ask(full, 8217.0 / 9940.0), full);
    // an ordinary hit owes nothing
    assert_eq!(crate::targets::forced_shortfall(0, 100, 0, 200), 0);
    // mixed with another bullet on the same frame: only the forced share counts against its full number
    assert_eq!(crate::targets::forced_shortfall(1000, 1200, 1000, 2000), 400);
}

/// (2026-10-23) the user: "make sure the target, if they survive, is thrown BACK towards the direction of the beam, not to the
/// side": the reaction the puppet runs (kh-react, from the hit as the core sends it: its direction and its carry point the
/// beam's) moves the body along the beam, whichever way the body faces.
#[test]
fn zetta_the_throw_carries_the_body_along_the_beam_whatever_its_facing() {
    let beam = Vec3::new(0.8, 0.6, 0.0);
    for yaw in [0.0f32, 90.0, 180.0, 270.0, 45.0, 143.0] {
        let at = Vec3::new(200.0, 0.0, 0.0);
        let mut r = kh_react::EnemyReaction::new(kh_react::Body::default());
        let mut h = HitRecord { damage: 300.0, damage_kind: Kind::Blow, direction: beam, flags: kh_core::host::HIT_FLAG_FORCE, ..HitRecord::empty(TargetId(1)) };
        h.position = at + Vec3::Z * 50.0;
        h.info.reaction_power = 999;
        h.info.armor_attack_power = 999;
        h.info.move_length = 600.0;
        h.attacker = kh_core::host::HitAttacker { position: at + beam * 600.0, yaw: 0.0, radius: 30.0, valid: true };
        let o = r.hit(&kh_react::ReactionRequest::from_hit(&h, true, false), kh_react::Pose { position: at, yaw, grounded: true });
        assert!(matches!(o, HitOutcome::Started { .. }), "yaw {yaw}: {o:?}");
        let world = kh_react::FlatWorld::default();
        let mut p = at;
        for _ in 0..40 {
            p = r.step_dt(&world, kh_react::STEP).position;
        }
        let moved = Vec3::new(p.x - at.x, p.y - at.y, 0.0);
        assert!(moved.length() > 20.0, "yaw {yaw}: thrown ({moved})");
        assert!(moved.normalize().dot(beam.normalize()) > (3.0f32).to_radians().cos(), "yaw {yaw}: along the beam ({moved})");
    }
}

/// (2026-10-23) Zettaflare frames and covers what the game draws: Malenia (one real c2120 among coincident extras, a 2.6 m x
/// 57 cm capsule, a long sword) is measured as one body with her reach; an undrawn extra or another body nearby is not hers.
#[test]
fn zetta_the_drawn_size_is_one_real_body_with_its_reach() {
    let real = Enemy::stub(4, Vec3::new(600.0, 0.0, 0.0), 57.0, 130.0, 18473, Weight::Large);
    let mut extra = Enemy::stub(5, Vec3::new(610.0, 5.0, 0.0), 40.0, 90.0, 18473, Weight::Large);
    extra.active = false;
    let small_dup = Enemy::stub(6, Vec3::new(605.0, -3.0, 0.0), 30.0, 60.0, 18473, Weight::Large);
    let other = Enemy::stub(7, Vec3::new(900.0, 0.0, 0.0), 300.0, 400.0, 9000, Weight::Large);
    let list = vec![real.clone(), extra, small_dup.clone(), other];
    // locked on the real one or on a drawn duplicate on her spot: her size; never the other body's
    assert_eq!(crate::targets::drawn_body(&real, &list), (57.0, 130.0));
    assert_eq!(crate::targets::drawn_body(&small_dup, &list), (57.0, 130.0));
    let (r, h) = (57.0 * crate::targets::ZETTA_REACH_RADIUS, 130.0 * crate::targets::ZETTA_REACH_HEIGHT);
    assert!(r > 120.0 && h > 160.0, "her sword and raised arms inside ({r} x {h})");
}

/// (2026-10-23) the user: "THIS FRAMING IS PERFECT! EVERY ENEMY NEEDS TO BE FRAMED LIKE THIS WITH THE BEAM MATCHING THIS FRAME
/// EXACTLY NO MATTER HOW BIG THE ENEMY IS": the Elden Ring route end to end (no game): the size kh-er hands Zettaflare for a
/// body (`drawn_body` with its reach, as `ErTargets::drawn_extent`), and the enemy shot it gets is the test level's Sora
/// case scaled about it (same angles, same lens; a small body's camera kept clear of the floor), its beam never under the
/// Sora case's: a soldier, Malenia (57 x 130 cm capsule, her sword and duplicates), a giant, a 6 m boss.
#[test]
fn zetta_every_er_body_gets_the_reference_frame_scaled_about_it() {
    use kh_party::donald::zettaflare::{beam_girth, cam_pose, cam_shots, seq_time, staff_local, target_size, Basis, CamSubject, Frame, DON_ZETTA_PARAMS, EVENT_DONALD_YAW_OFF};
    let z = DON_ZETTA_PARAMS; // (the sheet's numbers: other tests here install their own)
    let shots = cam_shots();
    let yaw_of = |d: Vec3| d.y.atan2(d.x).to_degrees();
    let pose = |half: f32, radius: f32, t: f32| {
        // (one aim point for every size, as kh-party's golden test: the beam's line the same, so only the scale differs)
        let goal = Vec3::new(1200.0, 0.0, 90.0);
        let _ = half;
        let yaw = yaw_of(Vec3::new(goal.x, goal.y, 0.0));
        let donald = Frame { origin: Vec3::ZERO, yaw: yaw + EVENT_DONALD_YAW_OFF };
        let target = Frame { origin: goal, yaw };
        let beam = Basis::beam(donald.point(staff_local(&z)), goal);
        (cam_pose(&shots, t, 5.0, &z, donald, target, beam, CamSubject { at: goal, size: target_size(half, radius, &z), animated: true, radius, half: 0.0 }, None).unwrap(), goal)
    };
    // (the Sora case: the test level's Sora in his hurt pose, the body that fills the reference's region: fit 1)
    let sora = (z.zetta_frame_body_height * 0.5, z.zetta_frame_body_width * 0.5);
    let malenia = Enemy::stub(4, Vec3::new(600.0, 0.0, 0.0), 57.0, 130.0, 18473, Weight::Large);
    let mut dup = Enemy::stub(5, Vec3::new(608.0, 4.0, 0.0), 40.0, 90.0, 18473, Weight::Large);
    dup.active = true;
    let bodies = [
        ("soldier", vec![Enemy::stub(1, Vec3::new(200.0, 0.0, 0.0), 35.0, 90.0, 400, Weight::Small)]),
        ("malenia + a duplicate", vec![malenia, dup]),
        ("giant", vec![Enemy::stub(3, Vec3::new(450.0, 0.0, 0.0), 150.0, 400.0, 30000, Weight::Large)]),
        ("6 m boss", vec![Enemy::stub(4, Vec3::new(600.0, 0.0, 0.0), 300.0, 600.0, 90000, Weight::Large)]),
    ];
    for (name, list) in bodies {
        let (r0, h0) = crate::targets::drawn_body(&list[0], &list);
        let (radius, half) = (r0 * crate::targets::ZETTA_REACH_RADIUS, h0 * crate::targets::ZETTA_REACH_HEIGHT);
        let k = target_size(half, radius, &z) / target_size(sora.0, sora.1, &z);
        assert!(beam_girth(half, radius, &z) >= beam_girth(sora.0, sora.1, &z) - 1e-4, "{name}: the beam never under the Sora case's");
        let mut t = seq_time(z.zetta_hit_at, &z, 5.0) + 0.05;
        let mut seen = 0;
        while t < seq_time(z.zetta_white_in_at, &z, 5.0) {
            let (p, goal) = pose(half, radius, t);
            let (r, rg) = pose(sora.0, sora.1, t);
            if p.shot == "shot920" && r.shot == "shot920" {
                seen += 1;
                let scaled = (r.position - rg) * k;
                let s = target_size(half, radius, &z).clamp(0.2, 12.0);
                let want = Vec3::new(scaled.x, scaled.y, scaled.z.max(-z.zetta_target_center_height * s + z.zetta_cam_floor_clear));
                                assert!((p.position - goal - want).length() < 2.0 + 0.01 * want.length(), "{name} at {t:.2}: the Sora case scaled x{k} ({} vs {want})", p.position - goal);
                assert!((p.fov - r.fov).abs() < 1e-3, "{name}: the same lens");
            }
            t += 0.2;
        }
        assert!(seen > 3, "{name}: the enemy shot");
    }
}

fn enemies() -> Vec<(&'static str, Enemy)> {
    let mut boss = Enemy::stub(4, Vec3::new(600.0, 0.0, 0.0), 300.0, 600.0, 90000, Weight::Large);
    boss.poise_max = 999.0;
    boss.poise = 999.0;
    boss.hyper = Some(true);
    boss.in_attack = true;
    boss.puppet_ok = false;
    let mut knight = Enemy::stub(2, Vec3::new(300.0, 0.0, 0.0), 45.0, 95.0, 2500, Weight::Medium);
    knight.poise_max = 80.0;
    knight.poise = 80.0;
    let mut giant = Enemy::stub(3, Vec3::new(450.0, 0.0, 0.0), 150.0, 400.0, 30000, Weight::Large);
    giant.puppet_ok = false;
    vec![("soldier", Enemy::stub(1, Vec3::new(200.0, 0.0, 0.0), 35.0, 90.0, 400, Weight::Small)), ("knight", knight), ("giant", giant), ("boss", boss)]
}

fn hit(e: &Enemy, kind: Kind, force: bool) -> HitRecord {
    let mut h = HitRecord { damage: 300.0, damage_kind: kind, direction: Vec3::X, flags: if force { kh_core::host::HIT_FLAG_FORCE } else { 0 }, ..HitRecord::empty(e.id) };
    h.position = e.pos + Vec3::Z * 50.0;
    if force {
        h.info.reaction_power = 999;
        h.info.armor_attack_power = 999;
    }
    h
}

#[test]
fn a_forced_blow_gets_through_on_any_enemy() {
    let params = Params::default();
    for (name, e) in enemies() {
        let mut p = Puppets::default();
        // the forced throw at the beam's end: its reaction starts, whatever it is
        let o = p.hit(&params, &e, &hit(&e, Kind::Blow, true), true, false);
        assert!(matches!(o, HitOutcome::Started { .. }), "{name}: {o:?}");
        assert_ne!(p.reaction_state(e.id), 0, "{name}: a reaction runs (an animation the camera may show)");
        // and again at once: no chain cap, no revenge refuses the next forced one
        let o = p.hit(&params, &e, &hit(&e, Kind::Small, true), true, false);
        assert!(matches!(o, HitOutcome::Started { .. }), "{name} again: {o:?}");
    }
}

#[test]
fn where_the_rules_refuse_plain_hits_the_forced_one_still_starts() {
    // plain hits on each enemy until its own rules refuse one (poise, revenge armour, chain cap); the forced one then starts
    let params = Params::default();
    let mut refused_somewhere = false;
    for (name, e) in enemies() {
        let mut p = Puppets::default();
        let mut refused = false;
        for _ in 0..60 {
            let o = p.hit(&params, &e, &hit(&e, Kind::Blow, false), true, false);
            if !matches!(o, HitOutcome::Started { .. }) {
                refused = true;
                break;
            }
        }
        refused_somewhere |= refused;
        let o = p.hit(&params, &e, &hit(&e, Kind::Blow, true), true, false);
        assert!(matches!(o, HitOutcome::Started { .. }), "{name} (plain refused: {refused}): {o:?}");
    }
    assert!(refused_somewhere, "some enemy's rules refuse plain hits (the force is what gets through)");
    let _ = TargetId(0);
}

/// (2026-10-23) the user: "Also in Melania's bossroom if I was targetting something invisible. Make sure it's impossible to
/// target invisible things": the game's rows standing on one body (Malenia's five c2120 rows on her very spot) are ONE target,
/// the one the game fights with; the others are no target. A different character on the same spot (a rider on his horse)
/// stays a target. When the shown instance changes (her next phase), the target is the new one.
#[test]
fn zetta_coincident_duplicates_are_never_targets_and_a_phase_swap_moves_the_target() {
    let mut real = Enemy::stub(4, Vec3::new(600.0, 0.0, 0.0), 57.0, 130.0, 18473, Weight::Medium);
    real.npc_id = 2120;
    real.poise_max = 80.0;
    let mut list = vec![real.clone()];
    for k in 0..4u32 {
        let mut d = Enemy::stub(10 + k, Vec3::new(605.0 + k as f32, 3.0, 0.0), 57.0, 130.0, 18473, Weight::Medium);
        d.npc_id = 2120;
        d.poise_max = 0.0;
        list.push(d);
    }
    // a rider and his horse: two characters on one spot, both real
    let mut rider = Enemy::stub(20, Vec3::new(2000.0, 0.0, 0.0), 40.0, 90.0, 900, Weight::Medium);
    rider.npc_id = 4410;
    let mut horse = Enemy::stub(21, Vec3::new(2002.0, 0.0, 0.0), 80.0, 120.0, 900, Weight::Large);
    horse.npc_id = 4411;
    list.push(rider);
    list.push(horse);
    let dup = crate::targets::duplicates(&list);
    let targets: Vec<u32> = list.iter().zip(&dup).filter(|(_, d)| !**d).map(|(e, _)| e.id.0).collect();
    assert_eq!(targets, vec![4, 20, 21], "one Malenia; the rider and his horse both");
    // her phase-1 body gone (dead / switched off): the instance the game now fights with is the target
    list[0].active = false;
    list[2].poise_max = 80.0;
    let dup = crate::targets::duplicates(&list);
    let targets: Vec<u32> = list.iter().zip(&dup).filter(|(e, d)| !**d && e.active).map(|(e, _)| e.id.0).collect();
    assert_eq!(targets, vec![11, 20, 21], "the target moves to the shown instance");
}

/// (2026-10-23) the user: "Zettaflare should completely overrule any sort of animation for an enemy dying"; "ensure that if a
/// boss has a second phase that this does not happen" (the user's log: a phase-1 Malenia at 958 HP hit for 9940 dropped to
/// the game's floor of 1 and its phase-change cutscene took over at once, 307.49 s): while Zettaflare owns the presentation its
/// hits are OWED (the game keeps the body alive: no death animation, no ragdoll, no phase change, no "felled" sequence), KH3
/// sees the kill; at the hand-back the whole number is taken through the game's pipeline (zdefer.rs).
#[test]
fn zetta_a_kill_is_owed_until_the_hand_back_and_the_body_never_dies_mid_sequence() {
    let mut book = crate::zdefer::Book::default();
    let mut owner = crate::zdefer::Owner::default();
    assert!(owner.frame(true, 1), "Zettaflare owns the presentation");
    book.add(4, 99, 9940);
    let game_hp = 958;
    assert!(book.killed(4, game_hp), "KH3's side: killed (the finale plays); the game's: alive (held, no death)");
    // the party hidden by a cutscene of the game's (or the run over): the hand-back
    let mut owned = true;
    for _ in 0..crate::zdefer::STALE_FRAMES + 1 {
        owned = owner.frame(true, 1);
    }
    assert!(!owned);
    assert_eq!(book.due(), vec![(4, 99, 9940)], "taken in full at the hand-back");
}

/// (2026-10-23) the user: "If you can't lock onto it in Elden Ring why would you be able to lock onto it with the kingdom hearts
/// characters?!" (the user's log after Malenia: her leftover c2120 rows 7, 8, 9, alive, hostile, never in the game's lock-on
/// list; the reticle on row 7). The gate: the game's own lock-on candidates (now, or within a moment), or a body Zettaflare
/// holds; beside it our own: a dead body, a hidden one, an undrawn row, a duplicate (targets.rs), an ally's proxy (never read).
#[test]
fn zetta_only_what_the_games_lock_on_lists_is_ours_to_target() {
    use crate::targets::{lock_gate_open, LOCK_GATE_HOLD_S};
    assert!(lock_gate_open(Some(0.0), false), "listed by the game now");
    assert!(lock_gate_open(Some(LOCK_GATE_HOLD_S - 0.1), false), "a moment out of its view");
    assert!(!lock_gate_open(Some(LOCK_GATE_HOLD_S + 0.1), false), "not listed for a while: dropped");
    assert!(!lock_gate_open(None, false), "never listed (Malenia's leftover rows): never ours");
    assert!(lock_gate_open(None, true), "a body Zettaflare holds is never dropped mid-sequence");
    // the flags our own rules read: a dead, an undrawn (or hidden by us: its render switch off), a lock-disabled row
    use crate::targets::{flags_of, Class};
    use kh_core::host::TargetFlags as F;
    let dead = flags_of(Class::Hostile, false, true, false).unwrap();
    assert!(dead & F::LOCKABLE == 0 && dead & F::HITTABLE == 0, "dead: no target");
    let undrawn = flags_of(Class::Hostile, true, false, false).unwrap();
    assert!(undrawn & F::LOCKABLE == 0 && undrawn & F::ALIVE == 0, "undrawn / hidden / gated: no target");
    let lock_off = flags_of(Class::Hostile, true, true, true).unwrap();
    assert!(lock_off & F::LOCKABLE == 0, "the game's lock-on disabled: not lockable");
}

/// (2026-10-23, the user: "Melania was also attacking during Zettaflare!") the party's hold ending (the log: "Zettaflare gives
/// c2120 back" 11 s before the hand-back) gives back a body it threw (a survivor), never one it killed: that one is kept
/// (no movement, no actions, AI off; puppet.rs zetta_keep) until the hand-back.
#[test]
fn zetta_a_killed_body_is_kept_until_the_hand_back() {
    use crate::targets::zetta_gives_back;
    assert!(!zetta_gives_back(false, 23667), "killed, HP owed: kept");
    assert!(zetta_gives_back(true, 23667), "a survivor: thrown back (the reaction has it)");
    assert!(zetta_gives_back(true, 0) && zetta_gives_back(false, 0), "nothing owed: as before");
}
