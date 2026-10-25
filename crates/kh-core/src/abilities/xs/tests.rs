//! (XSI) Unit tests of the XS abilities' tables and pure rules (the state-driven tests: tests/xsi_abilities.rs).

use super::*;
use crate::status::tables;

/// The copy of `EnemyBaseStatus.m_BaseDefensePower` is the game table's.
#[test]
fn xsi_enemy_base_defense_is_the_game_table() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/entities/Load/Common/EnemyBaseStatus.json");
    let text = std::fs::read_to_string(path).unwrap();
    // The sheet's rows carry "_row_name" and "m_BaseDefensePower"; read them without a JSON crate.
    let mut by_level: Vec<(u32, i32)> = Vec::new();
    for chunk in text.split("\"_row_name\"").skip(1) {
        let level: u32 = chunk.split('"').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let defense: i32 = chunk.split("\"m_BaseDefensePower\":").nth(1).and_then(|s| s.split(|c: char| c == ',' || c == '}').next()).and_then(|s| s.trim().parse().ok()).unwrap_or(-1);
        by_level.push((level, defense));
    }
    by_level.sort();
    assert_eq!(by_level.len(), 99);
    for (level, defense) in by_level {
        assert_eq!(ENEMY_BASE_DEFENSE[level as usize - 1], defense, "battle level {level}");
    }
}

/// HIDDEN POTENTIAL (XS 2.15): the floor of KH3's rule at the region levels of the sheet (our design).
#[test]
fn xsi_hidden_potential_floor_by_region() {
    // 0x140580E70: m_BaseDefensePower + ceil(0.15 bl + 1.5).
    assert_eq!((overcome_floor(15), overcome_floor(30), overcome_floor(45), overcome_floor(60), overcome_floor(75)), (15 + 4, 22 + 6, 30 + 9, 37 + 11, 45 + 13));
    assert_eq!([None, Some(0), Some(1), Some(2), Some(3), Some(4), Some(9)].map(region_battle_level), [1, 15, 30, 45, 60, 75, 75]);
    assert_eq!(overcome_floor(1), 8 + 2);
}

/// The catalogue rows of the 14 port abilities and the two KH3 rows they switch on (sheet sora-abilities).
#[test]
fn xsi_catalogue_rows() {
    let ours: Vec<_> = tables::ABILITIES.iter().filter(|r| r.port == "ours").collect();
    assert_eq!(ours.len(), 14);
    assert!(ours.iter().all(|r| r.owner == "sora" && r.kind.value() >= 256 && r.max_equip == Some(1)));
    let ap = |k: A| tables::ability_ap(k);
    assert_eq!([A::DivingStrike, A::SonicDash, A::SpiralRush, A::Zantetsuken, A::EventHorizon, A::GiantStamp, A::Chandelier, A::ShotlockSpectralRays, A::LightWave, A::CriticalCounter, A::RageRaid, A::Riskcharge, A::AncientLight, A::Myriad, A::Overcome, A::BreakBoost].map(ap), [2, 3, 3, 4, 4, 5, 3, 3, 4, 5, 4, 5, 6, 7, 5, 4]);
    // XS: "AP of all 16: 67".
    assert_eq!([A::DivingStrike, A::SonicDash, A::SpiralRush, A::Zantetsuken, A::EventHorizon, A::GiantStamp, A::Chandelier, A::ShotlockSpectralRays, A::LightWave, A::CriticalCounter, A::RageRaid, A::Riskcharge, A::AncientLight, A::Myriad, A::Overcome, A::BreakBoost].map(ap).iter().sum::<i32>(), 67);
    // The level slots X1 emptied (XS 4.1).
    let l = |level, path| tables::level_ability(level, path);
    assert_eq!((l(12, 0), l(18, 2), l(22, 0), l(22, 1), l(22, 2), l(44, 1)), (Some(A::SpiralRush), Some(A::CriticalCounter), Some(A::DivingStrike), Some(A::DivingStrike), Some(A::DivingStrike), Some(A::LightWave)));
    // The set holds values above 255.
    let mut set = crate::status::AbilitySet::EMPTY;
    set.set(A::BreakBoost, 1, 1);
    assert_eq!((set.count(A::BreakBoost), set.count(A::SoraCuraga), set.ap_in_use()), (1, 0, 4));
}
