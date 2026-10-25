//! (DF, 2026-10-15; the user: "the overall difficulty of the mod should be like Kingdom Hearts critical mode")
//! The difficulty on the Elden Ring route, offline: the factor on the share rule, the status-tick exception, the host
//! sheet's rows, and THE TABLES of IMPL_DF_STATE.md (run with `--nocapture` to print them):
//! - Elden Ring hits on Sora at level 1 / 30 / 60 / 99, before (the live build: Standard) and after (Critical);
//! - KH3's own par-level hits, Standard against Critical (what the factor stands for);
//! - EXP and max-HP growth, Standard against Critical.

use super::{difficulty_in, Params};
use kh_core::enums::ETresAbilityKind as A;
use kh_core::status::{GameLevel, OpeningChoice};
use kh_core::{Pose, Sora, Status, Vec3, SORA_PARAMS};
use kh_host::bossrewards::{claim, Table};
use kh_host::save;

const HOST_SHEET: &str = include_str!("../../../sheets/port/sora-er-host.json");
const BOSS_SHEET: &str = include_str!("../../../sheets/port/sora-er-boss-rewards.json");

fn base() -> kh_core::status::Loadout {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-loadout-99.json");
    kh_host::rig::loadout::parse(path, &std::fs::read_to_string(path).expect("the loadout sheet")).expect("parse").0
}

/// A new game at `level` (Balance + the warrior's path), then the bosses of the reward sheet up to `order` beaten (their
/// HP / MP steps by the difficulty's own table), then the EXP of `exp_level`.
fn sora_at(level: GameLevel, order: u32, exp_level: u32) -> Sora {
    let status = Status::from_loadout(&save::new_game_loadout_for(&save::with_difficulty(&base(), level), OpeningChoice::DEFAULT));
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(status);
    let table = Table::parse(BOSS_SHEET);
    let mut records = Vec::new();
    let mut rows: Vec<_> = table.rows.iter().filter(|r| r.order <= order).collect();
    rows.sort_by_key(|r| r.order);
    for row in rows {
        claim(&table, &mut records, &mut sora, &row.id, false);
    }
    let s = sora.status_mut();
    s.exp = kh_core::status::tables::level_row(exp_level).exp_total;
    s.rebuild();
    s.full_recover();
    sora
}

#[test]
fn the_factor_on_elden_rings_hits() {
    let f: Vec<f32> = GameLevel::ALL.iter().map(|g| difficulty_in(*g)).collect();
    assert_eq!(f, vec![0.5, 1.0, 1.5, 4.0]);
    let std = Params::default();
    let crit = Params { diff_in: difficulty_in(GameLevel::Critical), ..Params::default() };
    // the user's log: "Elden Ring took 82 of 522 HP off its player -> Sora is hit for 23.6" at max HP 150
    assert!((std.damage_in(82, 522, 150) - 23.56).abs() < 0.01);
    // Critical: the same hit on a Critical Sora (75 max) takes 4 x the share: 47.1 of 75
    assert!((crit.damage_in(82, 522, 75) - 47.13).abs() < 0.01);
    // a status tick is a share of the maximum: no difficulty (0x140632240 step 2)
    assert_eq!(crit.damage_in_tick(10, 522, 75), std.damage_in_tick(10, 522, 75));
    // dmg.in = 0 still means no damage at all
    assert_eq!(Params { in_scale: 0.0, ..crit }.damage_in(82, 522, 75), 0.0);
}

#[test]
fn the_host_sheet_rows() {
    let (p, problems) = Params::from_sheet(HOST_SHEET).expect("the host sheet");
    assert!(!problems.iter().any(|id| id.starts_with("difficulty.") || id.contains("critical")), "{problems:?}");
    // (dec032) new games are STANDARD; the 1.5 MP multiplier is the base cost for everyone (user, 2026-10-22: no difficulty select)
    assert_eq!((p.new_game_level, p.mp_cost_scale, p.mp_cost_scale_critical, p.diff_in), (1, 1.5, 1.0, 1.0));
    assert_eq!(Params::default().new_game_level, 1);
    assert_eq!(difficulty_in(GameLevel::Standard), 1.0, "Standard: Elden Ring's hits exactly as before DF");
    let (p, _) = Params::from_sheet(&HOST_SHEET.replace("\"difficulty.new_game\",\"STANDARD\"", "\"difficulty.new_game\",\"proud\"")).unwrap();
    assert_eq!(p.new_game_level, 2);
}

/// Hits of Elden Ring enemies as a share of the Tarnished's maximum: the user's log (522 HP Tarnished; c4160 / c4161
/// soldiers 66 / 82 / 89, c4060 a 172 blow) and two reference sizes.
const HITS: [(&str, i32, i32); 6] = [("light (5 %)", 26, 522), ("c4161 jab (66/522)", 66, 522), ("c4160 slash (82/522)", 82, 522), ("c4161 (89/522)", 89, 522), ("c4060 blow (172/522)", 172, 522), ("boss heavy (50 %)", 261, 522)];
/// Sora's level -> the bosses of the reward sheet beaten by then (a progression ASSUMPTION for the table only).
const LEVELS: [(u32, u32); 4] = [(1, 0), (30, 9), (60, 24), (99, 48)];

#[test]
fn table_elden_ring_hits_on_sora_before_and_after() {
    let before = Params::default();
    let after = Params { diff_in: difficulty_in(GameLevel::Critical), ..Params::default() };
    println!("\nDF TABLE 1: Elden Ring hits on Sora (share rule). BEFORE = live build (Standard, diff 1). AFTER = Critical (diff 4).");
    println!("{:<6} {:>10} {:>10} | {:<22} {:>14} {:>9} | {:>14} {:>9}", "level", "maxHP std", "maxHP crit", "hit", "before HP", "hits KO", "after HP", "hits KO");
    for (level, order) in LEVELS {
        let (s, c) = (sora_at(GameLevel::Standard, order, level), sora_at(GameLevel::Critical, order, level));
        let (ms, mc) = (s.status().max_hp(), c.status().max_hp());
        for (name, lost, of) in HITS {
            let (b, a) = (before.damage_in(lost, of, ms), after.damage_in(lost, of, mc));
            let ko = |dmg: f32, max: i32| (max as f32 / dmg.floor().max(1.0)).ceil() as i32;
            println!("{:<6} {:>10} {:>10} | {:<22} {:>8.1} ({:>3.0}%) {:>9} | {:>8.1} ({:>3.0}%) {:>9}", level, ms, mc, name, b, 100.0 * b / ms as f32, ko(b, ms), a, 100.0 * a / mc as f32, ko(a, mc));
            // after = 4 x the share of the bar, whatever the level
            assert!(((a / mc as f32) - 4.0 * (b / ms as f32)).abs() < 0.03 || a >= mc as f32 * 0.99, "{level} {name}");
        }
    }
}

#[test]
fn table_kh3_par_level_hits_standard_against_critical() {
    // EnemyBaseStatus at battle level = Sora's level, attack power 1.0, Sora's Defense of the level row, the 80 cap;
    // Critical: attack +5 (0x140575ECF), x2 (0x140642820), max HP of the same progression (Balance, every story bonus
    // by level 99 interpolated by the reward sheet's order as in table 1).
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/entities/Load/Common/EnemyBaseStatus.json");
    let rows: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).expect("EnemyBaseStatus")).unwrap();
    let rows = rows["rows"].as_array().unwrap();
    println!("\nDF TABLE 2: KH3's own hits at par (battle level = Sora's level, power 1), the factor's meaning.");
    println!("{:<6} {:>6} {:>4} | {:>9} {:>6} {:>8} | {:>9} {:>6} {:>8} | {:>6}", "level", "attack", "def", "std dmg", "maxHP", "hits KO", "crit dmg", "maxHP", "hits KO", "ratio");
    for (level, order) in LEVELS {
        let attack = rows[(level - 1) as usize]["m_BaseAttackPower"].as_i64().unwrap() as i32;
        let def = kh_core::status::tables::level_row(level).defense;
        let std_dmg = (attack - def).clamp(1, 80);
        let crit_dmg = 2 * (attack + 5 - def).clamp(1, 80);
        let (ms, mc) = (sora_at(GameLevel::Standard, order, level).status().max_hp(), sora_at(GameLevel::Critical, order, level).status().max_hp());
        let (ks, kc) = ((ms as f32 / std_dmg as f32).ceil(), (mc as f32 / crit_dmg as f32).ceil());
        println!("{:<6} {:>6} {:>4} | {:>9} {:>6} {:>8} | {:>9} {:>6} {:>8} | {:>6.2}", level, attack, def, std_dmg, ms, ks, crit_dmg, mc, kc, (std_dmg as f32 / ms as f32) / (crit_dmg as f32 / mc as f32).max(1e-6));
    }
}

#[test]
fn curves_exp_and_hp_growth() {
    let p = Params::default();
    println!("\nDF TABLE 3: EXP. An Elden Ring enemy of N max HP gives ceil(N x {}) base EXP; Critical ceil(0.75 x) (0x140632CB0).", p.exp_per_max_hp);
    println!("{:<6} {:>10} | {:>16} {:>16} | {:>16} {:>16}", "level", "total EXP", "kills 400HP std", "kills 400HP crit", "kills 3000HP std", "kills 3000HP crit");
    let mut s = Status::from_loadout(&save::new_game_loadout_for(&base(), OpeningChoice::DEFAULT));
    let mut c = Status::from_loadout(&save::new_game_loadout_for(&save::with_difficulty(&base(), GameLevel::Critical), OpeningChoice::DEFAULT));
    s.set_ability(A::Expzero, 1, 0);
    c.set_ability(A::Expzero, 1, 0);
    let per = |st: &Status, hp: i32| st.clone().give_exp(p.exp_of(hp), 1.0).added.max(1);
    for level in [2u32, 10, 30, 60, 99] {
        let total = kh_core::status::tables::level_row(level).exp_total;
        let kills = |st: &Status, hp: i32| (total as f32 / per(st, hp) as f32).ceil() as i64;
        println!("{:<6} {:>10} | {:>16} {:>16} | {:>16} {:>16}", level, total, kills(&s, 400), kills(&c, 400), kills(&s, 3000), kills(&c, 3000));
    }
    for hp in [400, 3000] {
        assert_eq!(per(&c, hp), (0.75 * per(&s, hp) as f32).ceil() as i32, "Critical EXP = ceil(0.75 e)");
    }
    println!("\nDF TABLE 4: max HP / MP / AP by bosses beaten (reward sheet order; Balance start), Standard against Critical.");
    println!("{:<14} {:>8} {:>8} | {:>8} {:>8} | {:>8} {:>8}", "bosses beaten", "HP std", "HP crit", "MP std", "MP crit", "AP std", "AP crit");
    for order in [0u32, 5, 9, 13, 24, 36, 48] {
        let (a, b) = (sora_at(GameLevel::Standard, order, 1), sora_at(GameLevel::Critical, order, 1));
        let (a, b) = (a.status(), b.status());
        println!("{:<14} {:>8} {:>8} | {:>8} {:>8} | {:>8} {:>8}", order, a.max_hp(), b.max_hp(), a.max_mp(), b.max_mp(), a.max_ap(), b.max_ap());
        assert!(b.max_hp() * 2 <= a.max_hp() + 2 && b.max_hp() * 2 >= a.max_hp() - 40, "order {order}: {} / {}", a.max_hp(), b.max_hp());
    }
}
