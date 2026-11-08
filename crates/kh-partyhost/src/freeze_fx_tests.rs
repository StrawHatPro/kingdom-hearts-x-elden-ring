//! (FREEZE-FX, the user 2026-11-07: "make sure frozen effects are applied to anything that gets frozen by blizzard") THE MIMIC
//! COPIES FROZEN BY THE PARTY wear KH3's frozen look: a Sora copy (his own core: kh-core `badstat::set_frozen`, the ice row
//! on him, the break at its end) now takes the party's FREEZE rows ([`reaction_of`]), as the friend copies always did
//! ([`copy_reaction_hit`] keeps the status: their pawns' freeze runs kh-party `freeze_fx`).

use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{Call, HitRecord, RecordingHost, TargetId};
use kh_core::test_world::TestWorld;
use kh_core::{Input, Pose, Sora, Vec3, SORA_PARAMS};

use crate::mimicboss::*;

const DT: f32 = 1.0 / 60.0;

fn blizzard_from(at: Vec3, seconds: f32) -> HitRecord {
    let mut h = HitRecord::empty(TargetId(1));
    (h.damage, h.damage_kind, h.position, h.direction, h.attack_name) = (10.0, Kind::Small, Vec3::Z * 90.0, Vec3::new(-1.0, 0.0, 0.0), "MAG_BLIZZARD");
    h.attacker.valid = true;
    h.attacker.position = at;
    (h.info.bad_status, h.info.bad_time) = ("FREEZE", seconds);
    h
}

/// The party's Blizzard on a Sora copy: his own KH3 freeze, KH3's ice on him for exactly its time, the break at its end.
#[test]
fn freeze_fx_a_sora_copy_frozen_by_the_party_wears_kh3s_ice() {
    let p = reaction_of(&blizzard_from(Vec3::new(600.0, 0.0, 0.0), 2.0));
    assert_eq!((p.bad_status, p.bad_time), ("FREEZE", 2.0), "the row's status reaches the copy");
    let world = TestWorld::floor();
    let leader = Sora::new(SORA_PARAMS, Pose { position: Vec3::ZERO, yaw: 0.0 });
    let mut core = copy_sora(&leader, Pose { position: Vec3::ZERO, yaw: 0.0 });
    // (its HP mirrors the boss body's in the fight: up)
    core.status_mut().hp = 100;
    let mut host = RecordingHost::new();
    let input = Input::default();
    for _ in 0..20 {
        let _ = host.step(&world, |h| core.step_hosted(DT, &input, h));
    }
    host.calls.clear();
    core.take_hit(p);
    let (mut frozen, mut ice_at, mut break_at) = (0usize, None, None);
    for k in 0..400 {
        let before = host.calls.len();
        let _ = host.step(&world, |h| core.step_hosted(DT, &input, h));
        if core.combat().bad.frozen {
            frozen += 1;
        }
        for c in &host.calls[before..] {
            if let Call::Spawn(_, s) = c {
                match s.name {
                    "vs_abn_frz0" => ice_at = ice_at.or(Some(k)),
                    "vs_abn_frz3" => break_at = break_at.or(Some(k)),
                    _ => {}
                }
            }
        }
    }
    let (ice, brk) = (ice_at.expect("KH3's ice on him"), break_at.expect("and its break"));
    assert!(((frozen as f32 * DT) - 2.0).abs() <= 3.0 * DT, "frozen for the row's 2 s ({frozen} steps)");
    assert!(brk > ice && ((brk - ice) as f32 * DT - 2.0).abs() <= 3.0 * DT, "the ice lasts the freeze: {ice} .. {brk}");
}
