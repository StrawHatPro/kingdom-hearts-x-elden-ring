//! (DF, 2026-10-15; the user: "the overall difficulty of the mod should be like Kingdom Hearts critical mode")
//! KH3's Critical mode, every multiplier in one place, and Critical's new-game start (sheets sora-difficulty,
//! sora-critical-start; build/sora_tmp/er/IMPL_DF_STATE.md).

use kh_core::enums::ETresAbilityKind as A;
use kh_core::status::damage::{difficulty_factor, Side};
use kh_core::status::tables::{self, GameLevel};
use kh_core::status::{BonusCounts, LoadoutValue};
use kh_core::{Loadout, Status};

fn sheet(name: &str) -> serde_json::Value {
    let path = format!("{}/../../sheets/port/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap()
}

fn cell(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

/// THE NEW GAME of the sheet `sora-newgame` (what kh-app's `loadout::new_game` parses) at `game_level`, Balance + path A.
fn new_game_loadout(game_level: &str) -> Loadout {
    let json = sheet("sora-newgame");
    let mut loadout = Loadout::new("sora-newgame");
    for row in json["rows"].as_array().unwrap() {
        let (Some(key), Some(value)) = (row[0].as_str(), cell(&row[1])) else { continue };
        let _ = loadout.set(key, &value);
    }
    loadout.set("game_level", &LoadoutValue::Text(game_level.to_string())).unwrap();
    loadout.set("choice.default", &LoadoutValue::Text("balance + ability path A".to_string())).unwrap();
    let _ = loadout.resolve();
    loadout
}

#[test]
fn every_critical_multiplier_of_the_difficulty_sheet() {
    // 0x140642820: the other side's damage x0.5 / 1 / 1.5 / 2; the player side's x1.25 on Beginner only.
    let other: Vec<f32> = GameLevel::ALL.iter().map(|g| difficulty_factor(Side::Other, *g)).collect();
    let player: Vec<f32> = GameLevel::ALL.iter().map(|g| difficulty_factor(Side::Player, *g)).collect();
    assert_eq!(other, vec![0.5, 1.0, 1.5, 2.0]);
    assert_eq!(player, vec![1.25, 1.0, 1.0, 1.0]);
    let c = GameLevel::Critical.row();
    // 0x140575D73: base HP / MP halved (floor); 0x140E92280: AP +30; 0x1405AB250: MP charge +20 s; 0x140576130: enemy attack +5.
    assert_eq!((c.base_hp, c.base_mp, c.ap_add, c.mp_charge_time, c.enemy_attack_add), (45, 50, 30, 70.0, 5));
    let s = GameLevel::Standard.row();
    assert_eq!((s.base_hp, s.base_mp, s.ap_add, s.mp_charge_time, s.enemy_attack_add), (90, 100, 0, 50.0, 0));
    // 0x1405864C0: HP_UP 3/5/10/15/30 -> 1/2/5/7/15, MP_UP 3/5/10/20 -> 1/2/5/10.
    let hp: Vec<i32> = (1..=5).map(|k| BonusCounts::amount(k, GameLevel::Critical).0).collect();
    let mp: Vec<i32> = (8..=11).map(|k| BonusCounts::amount(k, GameLevel::Critical).1).collect();
    assert_eq!((hp, mp), (vec![1, 2, 5, 7, 15], vec![1, 2, 5, 10]));
    // m_CriticalEquipAbility + m_CriticalHaveAbility.
    assert_eq!(c.start_abilities, &["CRITICAL_COUNTER", "CRITICAL_CHARGE", "CRITICAL_CONVERTER"]);
}

#[test]
fn critical_exp_mp_charge_and_ap_on_a_status() {
    let std = Status::from_loadout(&new_game_loadout("STANDARD"));
    let crit = Status::from_loadout(&new_game_loadout("CRITICAL"));
    // EXP 0x140632CB0: ceil(0.75 e) on Critical.
    for (e, want) in [(1, 1), (2, 2), (4, 3), (10, 8), (50, 38), (999, 750)] {
        assert_eq!(crit.clone().give_exp(e, 1.0).added, want, "EXP {e}");
        assert_eq!(std.clone().give_exp(e, 1.0).added, e);
    }
    assert_eq!((std.mp_charge_time(), crit.mp_charge_time()), (50.0, 70.0));
    assert_eq!((std.max_hp(), std.max_mp(), crit.max_hp(), crit.max_mp()), (105, 110, 52, 55));
    assert_eq!((std.max_ap(), crit.max_ap()), (28, 58));
}

#[test]
fn the_critical_start_sheet() {
    let rows = tables::critical_start();
    assert_eq!(rows.len(), 13);
    assert_eq!(rows.iter().filter(|r| r.1).count(), 11, "EQUIP rows");
    assert_eq!(rows.iter().filter(|r| !r.1).map(|r| r.0).collect::<Vec<_>>(), vec![A::Revengeimpact, A::Revengedive], "HAVE rows");
    assert_eq!(rows.iter().filter(|r| r.0 == A::Airslide).count(), 2);
    assert_eq!(rows.iter().filter(|r| r.0 == A::Superslide).count(), 2);
    assert!(tables::is_critical_start(A::GuardCounter) && tables::is_critical_start(A::RevengeEx) && !tables::is_critical_start(A::Glide));
    // (dec032) Critical Counter: KH3's start set, excluded from the port's; a reward of it is never skipped
    assert!(tables::critical_start_excluded(A::CriticalCounter) && !tables::is_critical_start(A::CriticalCounter));
    assert!(!tables::critical_start_excluded(A::CriticalCharge) && !tables::critical_start_excluded(A::CriticalConverter));
}

#[test]
fn a_critical_new_game_starts_with_the_flagged_abilities_within_its_ap() {
    // Standard: nothing is added.
    let mut std = new_game_loadout("STANDARD");
    assert_eq!(std.add_critical_start(), (0, 0));
    let std = Status::from_loadout(&std);
    assert_eq!((std.ap_in_use(), std.max_ap(), std.abilities.owned(A::GuardCounter)), (28, 28, 0));
    // Critical: 13 copies owned; equipped in row order while the 58 AP allow (28 start + 3 Critical Recharge; Critical
    // Counter is excluded: decision dec032).
    let mut crit = new_game_loadout("CRITICAL");
    assert_eq!(crit.add_critical_start(), (13, 10));
    let s = Status::from_loadout(&crit);
    assert_eq!((s.ap_in_use(), s.max_ap()), (54, 58), "4 AP free: Final Blow (5) does not fit");
    let owned_eq = |k: A| (s.abilities.owned(k), s.abilities.equipped(k));
    assert_eq!(owned_eq(A::AirRecovery), (1, 1));
    assert_eq!(owned_eq(A::GuardCounter), (1, 1));
    assert_eq!(owned_eq(A::SlashUpper), (1, 1));
    assert_eq!(owned_eq(A::ComboMaster), (1, 1));
    assert_eq!(owned_eq(A::AirRollBeat), (1, 1));
    assert_eq!(owned_eq(A::Riskdodge), (1, 1));
    assert_eq!(owned_eq(A::Airslide), (3, 3), "start copy + two rows");
    assert_eq!(owned_eq(A::Superslide), (3, 3));
    assert_eq!(owned_eq(A::Revengeimpact), (1, 0), "HAVE");
    assert_eq!(owned_eq(A::Revengedive), (1, 0), "HAVE");
    assert_eq!(owned_eq(A::RevengeEx), (1, 0), "Final Blow (5 AP) does not fit");
    assert_eq!(owned_eq(A::CriticalCounter), (0, 0), "dec032: not at the start");
    assert_eq!(owned_eq(A::CriticalCharge), (1, 1));
    assert_eq!(owned_eq(A::CriticalConverter), (1, 0));
    assert_eq!((s.level, s.exp, s.max_hp(), s.max_mp()), (1, 0, 52, 55));
}
