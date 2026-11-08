//! (AUDIT-C) ROUND-2 F16, "Rage Form was never offered": THE ELDEN RING SIDE OF THE OFFER. A hit on the hidden body becomes
//! Sora's hit the way kh-er sora.rs `damage_in` makes it (the host sheet's `Params::damage_in`: the body's loss as a share
//! of HIS maximum, at least 1 HP) and goes to the core by `Sora::take_hit`; KH3's roll (the HP setter's tail 0x14140CE00)
//! then runs on every HP LOSS that leaves <= 25 % of MaxHP. The ROUND-2 test hits of `dbg.hurt = 0.1` took 0 HP (KH3's
//! ApplyHpDamage floors the amount), so they rolled nothing: KH3's rule (new HP < old HP).

use std::path::PathBuf;

use glam::Vec3;
use kh_core::enums::ETresDamageKind;
use kh_core::states::HitOnPlayer;
use kh_core::{Input, TargetId};
use kh_host::seam::{AudioQueue, EffectQueue};

fn project() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn params() -> kh_er::ermap::Params {
    let text = std::fs::read_to_string(project().join("sheets/port/sora-er-host.json")).expect("the host sheet");
    kh_er::ermap::Params::from_sheet(&text).expect("the host numbers").0
}

fn step(rig: &mut kh_host::rig::Rig, n: usize) {
    for _ in 0..n {
        let (mut audio, mut effects) = (AudioQueue::default(), EffectQueue::default());
        rig.step_with(&Input::default(), &mut audio, &mut effects);
    }
}

#[test]
fn the_weakest_elden_ring_hit_takes_hp_and_rolls_rage_form() {
    let p = params();
    // the hidden body at 10 x a Tarnished's 396 base HP (damage.in.body_hp); the weakest loss the game can take: 1 HP
    let (body_max, sora_max) = (3960, 90);
    assert!(p.damage_in(1, body_max, sora_max) >= 1.0, "every Elden Ring hit is at least 1 HP of Sora's");
    let pack = project().join("build").join("sora_pack");
    if !pack.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no pack under {}", pack.display());
        return;
    }
    let loadout = project().join("sheets").join("port").join("sora-loadout-99.json");
    let mut rig = kh_host::rig::Rig::new(&pack, &loadout, Some(Vec3::new(300.0, 0.0, 0.0))).expect("Sora's rig");
    rig.sora.force_battle(true);
    step(&mut rig, 30);
    let max = rig.sora.status().max_hp();
    rig.sora.status_mut().hp = max / 5;
    step(&mut rig, 5);
    let mut offered = false;
    for _ in 0..40 {
        let hp = rig.sora.status().hp;
        let amount = p.damage_in(1, body_max, max);
        let from = rig.out.position + Vec3::new(150.0, 0.0, 0.0);
        rig.sora.take_hit(HitOnPlayer { direction: Vec3::NEG_X, ..HitOnPlayer::new(p.kind_in(1, body_max).unwrap_or(ETresDamageKind::Small), amount, TargetId::NONE, from) });
        step(&mut rig, 90);
        assert!(rig.sora.status().hp < hp, "the hit took HP ({hp} -> {})", rig.sora.status().hp);
        if rig.sora.combat().situation.has(0x8B) {
            offered = true;
            break;
        }
    }
    let r = rig.sora.combat().form.rage;
    assert!(r.rolls >= 1, "the losses rolled the offer");
    assert!(offered, "Rage Form offered within 40 weak hits at 20 % HP ({} rolls)", r.rolls);
    // a hit that takes nothing (dbg.hurt = 0.1: floored to 0) is no HP loss: no roll, as KH3's setter tail
    let rolls = r.rolls;
    let from = rig.out.position + Vec3::new(150.0, 0.0, 0.0);
    rig.sora.take_hit(HitOnPlayer { direction: Vec3::NEG_X, ..HitOnPlayer::new(ETresDamageKind::Small, 0.1, TargetId::NONE, from) });
    step(&mut rig, 60);
    assert_eq!(rig.sora.combat().form.rage.rolls, rolls, "no loss, no roll");
}
