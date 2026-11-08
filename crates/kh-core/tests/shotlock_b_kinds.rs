//! (SHOT-B) SHOT-B's kinds that run in SHOT-A's family code (KH3 shares the update function; traces: lock-on -> fire ->
//! finish -> back to normal). Which kind: SelectKind 0x141411B80 by the held Keyblade's weapon type, its MAX kind at Param0
//! locks. Every projectile comes from the held Keyblade's own projectile set (pawn vt+0x1450 -> its ProjSet):
//! - Nano Gear (weapon type 9), base: kind 18 `BX_SHOOT` "Cubic Stream" = Meteor Shower's update 0x1413BA090 (group 0x645),
//!   TresProjBX_Shoot_C, row SF_BXSHOOT (TOSS); MAX: kind 19 `BX_GATE` "Zone Connector" = Warp Trick's update 0x1413B7B40.
//! - Grand Chef (11), base: kind 23 `RE_SHOOT` "Steam Spiral" = Meteor Shower's update (group 0x86C), TresProjRE_Shoot_C,
//!   SF_RESHOOT (SIDESPIN); MAX: kind 24 "Fruit Crusher" = Atomic Deluge's update 0x1413B9910 (Frying Pan, weapon form
//!   0x13), TresProjRE_ShieldShot_C, SF_RESHIELDSHOT and the window's SF_RESHIELDSHOT_BLOW.
//! - Hunny Spout (8), base: kind 20 `PO_SHOOT` "Hunny Burst" = King of Hearts' update 0x1413B8C90 (group 0x72B: the charge
//!   effect vw_so042_shf_chg00), TresProjPO_Shoot_C, SF_POSHOT; MAX with at most 2 locked targets and room above the first
//!   (0x1413ECC80): kind 21 `PO_BARRAGE` "Hunny Drizzle" = Diving Barrage's update 0x1413B6320 (Hunny Blasters, form 0x10),
//!   SF_POBARRAGE; else kind 22 `PO_CANNON` "Sweet Surprise" = Cluster Cannonade's update 0x1413B6960.

mod shotlocks_common;

use kh_core::enums::ETresDamageKind;
use kh_core::host::Call;
use kh_core::{AnimId, StateId, TargetId, TargetInfo};
use shotlocks_common::{dummy, level99_with, Bench};

fn two() -> [TargetInfo; 2] {
    [dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)]
}

/// What a shotlock showed: the classes of the projectiles seen, the clips played, the weapon forms seen, the effect names
/// spawned (with their ids).
type Watched = (Vec<&'static str>, Vec<AnimId>, Vec<u8>, Vec<(&'static str, u32)>);

/// Fires and steps to the end of the state.
fn fire_and_watch(b: &mut Bench) -> Watched {
    let mut seen = b.host.calls.len();
    b.fire();
    let (mut classes, mut clips, mut forms, mut effects) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for _ in 0..900 {
        let o = b.step(&[]);
        if !clips.contains(&o.anim.name) {
            clips.push(o.anim.name);
        }
        let m = b.sora.magic();
        if !forms.contains(&m.shot_weapon_form) {
            forms.push(m.shot_weapon_form);
        }
        for p in m.projectile_views() {
            if !classes.contains(&p.class) {
                classes.push(p.class);
            }
        }
        for c in &b.host.calls[seen..] {
            if let Call::Spawn(_, e) = c {
                if !effects.contains(&(e.name, e.id.0)) {
                    effects.push((e.name, e.id.0));
                }
            }
        }
        seen = b.host.calls.len();
        if o.state != StateId::Shootflow {
            return (classes, clips, forms, effects);
        }
    }
    panic!("the state did not end");
}

/// Every hit: one of `rows`, the ShootFlow command and `kind`; `n` hits of the projectile row `rows[0]`.
fn assert_hits(b: &mut Bench, n: usize, rows: &[&str], kind: u8, damage: ETresDamageKind) {
    b.run(400);
    let hits = b.hits();
    assert!(hits.iter().all(|h| rows.contains(&h.attack_name)), "{:?}", hits.iter().map(|h| h.attack_name).collect::<Vec<_>>());
    let shots: Vec<_> = hits.iter().filter(|h| h.attack_name == rows[0]).collect();
    assert_eq!(shots.len(), n, "one {} hit per lock", rows[0]);
    assert!(shots.iter().all(|h| h.flags == 0xFD | (kind as u32) << 8 && h.damage_kind == damage), "{:?}", shots.iter().map(|h| (h.flags, h.damage_kind)).collect::<Vec<_>>());
    assert!(hits.iter().any(|h| h.target == TargetId(1)), "target 1 hit");
}

/// The base kind `kind` of `key` at 8 locks, Meteor Shower's code: M_1475, the stream of `class`.
fn stream(key: &str, kind: u8, max_locks: i32, class: &str, row: &str, damage: ETresDamageKind) {
    let mut b = Bench::new(level99_with(key), &two());
    assert_eq!(b.aim_until(8, 300), kind, "the base kind");
    assert_eq!(b.sora.aim().max_locks, max_locks, "TresShootFlowData Param0");
    let n = b.sora.aim().total as usize;
    let (classes, clips, _, _) = fire_and_watch(&mut b);
    assert!(clips.contains(&AnimId::M1475), "Meteor Shower's clip: {clips:?}");
    assert_eq!(classes, vec![class], "the Keyblade's projectile");
    assert_hits(&mut b, n, &[row], kind, damage);
    b.assert_back_to_normal();
}

#[test]
fn nano_gear_cubic_stream_kind_18() {
    stream("WEP_KEYBLADE_SO_08", 18, 24, "TresProjBX_Shoot_C", "SF_BXSHOOT", ETresDamageKind::Toss);
}

#[test]
fn grand_chef_steam_spiral_kind_23() {
    stream("WEP_KEYBLADE_SO_10", 23, 32, "TresProjRE_Shoot_C", "SF_RESHOOT", ETresDamageKind::Sidespin);
}

#[test]
fn hunny_spout_hunny_burst_kind_20() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_07"), &two());
    assert_eq!(b.aim_until(8, 300), 20, "Hunny Spout's base kind (weapon type 8)");
    assert_eq!(b.sora.aim().max_locks, 28, "TresShootFlowData row 20 Param0");
    let n = b.sora.aim().total as usize;
    let (classes, clips, _, effects) = fire_and_watch(&mut b);
    assert!(clips.contains(&AnimId::M1473) && clips.contains(&AnimId::M1474) && clips.contains(&AnimId::ShootFlowEnd), "King of Hearts' clips: {clips:?}");
    assert_eq!(classes, vec!["TresProjPO_Shoot_C"]);
    // The charge effect of King of Hearts' clip, the attach-effect data `ead_shf_tame0` played with group 0x72B (1835):
    // its row of that GroupID is vw_so042_shf_chg00 (spec/poshot.md).
    // (The ead's row choice by the play's group is the attach component's: notify.rs `effect_group_passes`.)
    assert!(effects.iter().any(|&(name, _)| name.contains("ead_shf_tame0")), "the charge effect: {effects:?}");
    assert_hits(&mut b, n, &["SF_POSHOT"], 20, ETresDamageKind::Sidespin);
    b.assert_back_to_normal();
}

#[test]
fn hunny_spout_hunny_drizzle_kind_21_at_max_on_one_target() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_07"), &[dummy(1, 800.0, 0.0)]);
    assert_eq!(b.aim_until(0, 1200), 21, "MAX with <= 2 locked targets and room above the first (0x1413ECC80)");
    let n = b.sora.aim().total as usize;
    assert_eq!(n, 28);
    let (classes, _, forms, _) = fire_and_watch(&mut b);
    assert!(forms.contains(&0x10), "Hunny Blasters on (vt+0x13C8(type, 0x10)): {forms:?}");
    assert_eq!(classes, vec!["TresProjPO_Barrage_C"]);
    // 28 bolts on the first 14 of M_2482's 16 TresShots, two each (spec/rocket.md).
    let spawned = b.host.calls.iter().filter(|c| matches!(c, Call::Spawn(_, e) if e.persistent && e.name == "TresProjPO_Barrage_C")).count();
    assert_eq!(spawned, n, "N bolts");
    b.run(400);
    let hits = b.hits();
    assert!(hits.iter().all(|h| h.attack_name == "SF_POBARRAGE" && h.flags == 0xFD | 21 << 8 && h.damage_kind == ETresDamageKind::Small), "{:?}", hits.iter().map(|h| h.attack_name).collect::<Vec<_>>());
    // (Finding for SHOT-A's barrage code: 24 of the 28 hit this dummy straight ahead, kind 6's 18 all do; the projectile rows
    // of PO_Barrage equal SS_Barrage's, so the misses come from the later TresShots' geometry: not traced here.)
    assert!(hits.len() >= n / 2 && hits.len() <= n, "{} of {n} bolts hit", hits.len());
    b.assert_back_to_normal();
    assert_eq!(b.sora.magic().shot_weapon_form, 0, "the Keyblade back");
}

#[test]
fn grand_chef_fruit_crusher_kind_24_at_max() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_10"), &two());
    assert_eq!(b.aim_until(0, 900), 24, "Grand Chef's MAX kind (weapon type 11)");
    let n = b.sora.aim().total as usize;
    assert_eq!(n, 32);
    let (classes, _, forms, _) = fire_and_watch(&mut b);
    assert!(forms.contains(&0x13), "the Frying Pan on (weapon form 0x13): {forms:?}");
    assert!(classes.contains(&"TresProjRE_ShieldShot_C"), "{classes:?}");
    assert_hits(&mut b, n, &["SF_RESHIELDSHOT", "SF_RESHIELDSHOT_BLOW"], 24, ETresDamageKind::Small);
    b.assert_back_to_normal();
    assert_eq!(b.sora.magic().shot_weapon_form, 0, "the Keyblade back");
}

#[test]
fn nano_gear_zone_connector_kind_19_at_max() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_08"), &[dummy(1, 800.0, 0.0)]);
    assert_eq!(b.aim_until(0, 900), 19, "Nano Gear's MAX kind (weapon type 9)");
    let n = b.sora.aim().total as usize;
    assert_eq!(n, 24, "TresShootFlowData row 18 Param0");
    let (_, clips, forms, _) = fire_and_watch(&mut b);
    // Warp Trick's update with weapon form 8 (Nano Arms' w_so172): its anim set's M_4481..3 are sequences 8481..3.
    assert!(forms.contains(&8), "weapon form 8 on (vt+0x13C8(type, 8)): {forms:?}");
    let names: Vec<&str> = clips.iter().map(|c| c.data().name).collect();
    assert!(["8481", "8482", "8483"].iter().all(|s| names.contains(s)), "{names:?}");
    let gates = b.host.calls.iter().filter(|c| matches!(c, Call::Spawn(_, e) if e.persistent && e.name == "TresProjBX_Gate_C")).count();
    assert_eq!(gates, n, "N gates");
    assert_hits(&mut b, n, &["SF_BXGATE"], 19, ETresDamageKind::Verticalroll);
    b.assert_back_to_normal();
    assert_eq!(b.sora.magic().shot_weapon_form, 0, "the Keyblade back");
}

#[test]
fn hunny_spout_sweet_surprise_kind_22_at_max_on_three_targets() {
    // More than 2 locked targets: 0x1413ECC80 says no -> Sweet Surprise.
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_07"), &[dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0), dummy(3, 800.0, -150.0)]);
    assert_eq!(b.aim_until(0, 1200), 22, "MAX with more than 2 locked targets ({:?})", b.sora.aim());
    let n = b.sora.aim().total as usize;
    assert_eq!(n, 28);
    let (classes, _, forms, _) = fire_and_watch(&mut b);
    assert!(forms.contains(&0x11), "the Hunny Launcher on (vt+0x13C8(type, 0x11)): {forms:?}");
    assert!(classes.contains(&"TresProjPO_Cannon_C"), "the shell: {classes:?}");
    b.run(600);
    let spawned = |class: &str| b.host.calls.iter().filter(|c| matches!(c, Call::Spawn(_, e) if e.persistent && e.name == class)).count();
    assert_eq!(spawned("TresProjPO_Cannon2_C"), n, "N shots out of the shell");
    let hits = b.hits();
    assert!(!hits.is_empty() && hits.iter().all(|h| h.attack_name == "SF_POCANNON" && h.flags == 0xFD | 22 << 8), "{:?}", hits.iter().map(|h| (h.attack_name, h.flags)).collect::<Vec<_>>());
    b.assert_back_to_normal();
    assert_eq!(b.sora.magic().shot_weapon_form, 0, "the Keyblade back");
}