//! The status side (docs/sora-stats-spec.md; MP / focus / items of docs/sora-magic-spec.md parts
//! C, D, E; gauges and popups of docs/sora-ui-spec.md sections 4, 8, 11b). Every test cites the
//! spec section or the sheet row its expectation comes from. The level-99 loadout is read from
//! the real sheet, as a host would (`sheets/port/sora-loadout-99.json`).

use kh_core::abilities::passive::{self, CommandTimer};
use kh_core::enums::{ETresAbilityKind as A, ETresDamageAttribute as Attr};
use kh_core::host::{EventKind, RecordingHost, TargetDefense};
use kh_core::status::damage::{CalcType, DefenderKind};
use kh_core::status::tables::{self, GameLevel};
use kh_core::status::{compute_damage, AbilitySet, Attacker, DamageAttack, DamageContext, Defender, HitOnSora, ItemKind, LoadoutValue, Meal, MpCost, PrizeKind, StatusRequest, Survived};
use kh_core::test_world::TestWorld;
use kh_core::ui_model::{gauge_frame, PopupDetail, UiPopup};
use kh_core::{Input, Loadout, Pose, Sora, Status, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

// ------------------------------------------------------------------------------------ helpers

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

/// The level-99 sheet with some rows replaced before it is resolved.
fn loadout_with(overrides: &[(&str, LoadoutValue)]) -> Loadout {
    let json = sheet("sora-loadout-99");
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = overrides.iter().find(|o| o.0 == key).map(|o| o.1.clone()).or_else(|| cell(&row[1])).unwrap();
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    for (key, value) in overrides {
        loadout.set(key, value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    loadout
}

fn text(s: &str) -> LoadoutValue {
    LoadoutValue::Text(s.to_string())
}

/// Level 99 with everything, by first choice and difficulty.
fn level99(choice: &str, game_level: &str) -> Status {
    Status::from_loadout(&loadout_with(&[("choice.default", text(&format!("{choice} + ability path A"))), ("game_level", text(game_level))]))
}

/// The sheet's default: Balance, path A, Standard.
fn default99() -> Status {
    Status::from_loadout(&loadout_with(&[]))
}

/// A new game: level 1, no victory bonus but the opening choice's, the start weapon.
fn new_game(choice: &str, game_level: &str) -> Status {
    let mut loadout = Loadout::new("new game");
    for (key, value) in [("level", LoadoutValue::Number(1.0)), ("game_level", text(game_level)), ("choice.default", text(&format!("{choice} + ability path A"))), ("weapon", text("WEP_KEYBLADE_SO_00"))] {
        loadout.set(key, &value).unwrap();
    }
    Status::from_loadout(&loadout)
}

fn spawn(status: Status) -> Sora {
    Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 }).with_status(status)
}

fn popups(events: &[kh_core::Event]) -> Vec<UiPopup> {
    events.iter().filter_map(|e| if let EventKind::Popup(p) = e.kind { Some(p) } else { None }).collect()
}

// ------------------------------------------------------------------------------------ 1. stats

#[test]
fn level_99_numbers_for_the_three_first_choices_on_standard_and_critical() {
    // Stats spec section 9 table; sheet sora-loadout-99 rows hp.* / mp.*.
    for (choice, standard, critical) in [("vitality", (280, 159), (139, 72)), ("balance", (265, 169), (131, 77)), ("wisdom", (250, 179), (124, 82))] {
        for (game_level, expected) in [("STANDARD", standard), ("BEGINNER", standard), ("PROUD", standard), ("CRITICAL", critical)] {
            let s = level99(choice, game_level);
            assert_eq!((s.hp_max, s.mp_max), expected, "{choice} {game_level}");
            assert_eq!((s.hp, s.mp, s.focus, s.focus_max), (expected.0, expected.1, 100, 100), "full gauges, focus 100 in every mode");
            // Rows level.99.*: Strength 62, Magic 64, Defense 59 before equipment; the Kingdom Key adds 4 / 3.
            assert_eq!((s.level, s.exp, s.base.strength, s.base.magic, s.base.defense, s.base.ap), (99, 1_527_640, 62, 64, 59, 180));
            assert_eq!((s.strength(), s.magic(), s.defense()), (66, 67, 59), "Critical does not change Strength, Magic, Defense (spec 3)");
            // AP: 180, Critical 210 (m_CriticalUseAPUpNum 30, sheet sora-difficulty column ap_add).
            assert_eq!(s.max_ap(), if game_level == "CRITICAL" { 210 } else { 180 });
        }
    }
}

#[test]
fn the_loadout_sheets_stated_numbers_are_what_the_rules_compute() {
    let loadout = loadout_with(&[]);
    let status = Status::from_loadout(&loadout);
    assert_eq!(loadout.verify(&status), Vec::<String>::new());
    assert!(status.notes.is_empty(), "{:?}", status.notes);
    // And a wrong statement is found.
    let mut wrong = loadout.clone();
    wrong.stats.strength = 70;
    assert_eq!(wrong.verify(&status).len(), 1);
}

#[test]
fn a_new_game_has_base_hp_and_mp_plus_the_opening_choice() {
    // Spec 2.3: 120 / 100 (Vitality), 105 / 110 (Balance), 90 / 120 (Wisdom); Critical 60 / 50, 52 / 55, 45 / 60.
    for (choice, standard, critical) in [("vitality", (120, 100), (60, 50)), ("balance", (105, 110), (52, 55)), ("wisdom", (90, 120), (45, 60))] {
        let s = new_game(choice, "STANDARD");
        assert_eq!((s.hp_max, s.mp_max), standard, "{choice}");
        let c = new_game(choice, "CRITICAL");
        assert_eq!((c.hp_max, c.mp_max), critical, "{choice} critical");
        // Sheet sora-levels row 1: 11 / 12 / 8, AP 28; neither choice changes them (spec 2.3).
        assert_eq!((s.level, s.exp, s.base.strength, s.base.magic, s.base.defense, s.max_ap()), (1, 0, 11, 12, 8, 28));
        assert_eq!((s.strength(), s.magic()), (15, 15), "Kingdom Key +4 / +3 (sheet sora-equipment row WEP_KEYBLADE_SO_00)");
        // Critical start (spec 2.6, sheet sora-difficulty CRITICAL): +30 AP, Critical Recharge equipped, Critical Converter
        // owned. (DF, decision dec032, the user: "don't start with critical counter": KH3's Critical Counter is left out)
        assert_eq!(c.max_ap(), 58);
        assert!(!c.has(A::CriticalCounter) && c.abilities.owned(A::CriticalCounter) == 0 && c.has(A::CriticalCharge) && !c.has(A::CriticalConverter) && c.abilities.owned(A::CriticalConverter) == 1);
        assert_eq!(c.ap_in_use(), 3);
    }
}

#[test]
fn equipment_adds_stats_and_abilities_but_no_hp_or_mp() {
    // Spec 2.5; sheet sora-equipment rows WEP_KEYBLADE_SO_00 and its upgrade rows.
    let mut s = default99();
    assert_eq!((s.abilities.worn(A::PrizeDraw), s.abilities.equipped(A::PrizeDraw), s.count(A::PrizeDraw)), (1, 1, 2), "Treasure Magnet: one copy equipped, one from the keyblade");
    let (hp, mp, ap_used) = (s.hp_max, s.mp_max, s.ap_in_use());
    s.weapon_upgrade = 10;
    s.rebuild();
    assert_eq!((s.strength(), s.magic()), (71, 72), "fully upgraded: Strength +9, Magic +8 (spec section 9)");
    assert_eq!((s.hp_max, s.mp_max, s.ap_in_use()), (hp, mp, ap_used), "no max HP / MP from equipment; worn abilities cost no AP");
    assert_eq!(s.count(A::FormTime), 1, "FORM_TIME from upgrade level 4 on");
    s.weapon_upgrade = 3;
    s.rebuild();
    assert_eq!((s.strength(), s.magic(), s.count(A::FormTime)), (62 + 5, 64 + 5, 0));
    // Another keyblade: only the CURRENT weapon's row counts.
    s.weapon = "WEP_KEYBLADE_SO_01".to_string();
    s.weapon_upgrade = 0;
    s.rebuild();
    assert_eq!((s.strength(), s.magic(), s.count(A::PrizeDraw), s.count(A::Defender)), (62 + 5, 64 + 2, 1, 1));
    // An unknown name gives nothing and is said.
    s.weapon = "NO_SUCH_WEAPON".to_string();
    s.rebuild();
    assert_eq!((s.strength(), s.notes.len()), (62, 1));
}

#[test]
fn meals_boosts_and_defender_enter_the_getters_in_the_specs_order() {
    // Spec 2.7 (stat getters), sheet sora-stats-rules rows stat.max_hp, stat.attack, stat.defense.
    let mut s = default99();
    s.boosts.strength = 2;
    s.boosts.defense = 1;
    s.meal = Some(Meal { hp: 10, mp: 7, strength: 2, magic: 1, defense: 3, ability: None });
    s.rebuild();
    assert_eq!((s.max_hp(), s.max_mp(), s.strength(), s.magic(), s.defense()), (275, 176, 62 + 2 + 4 + 2, 64 + 3 + 1, 59 + 1 + 3));
    // Hearty Meal (sheet sora-abilities row POWER_LUNCH): every meal term becomes ceil(term * 1.25).
    s.meal = Some(Meal { hp: 10, mp: 7, strength: 2, magic: 1, defense: 3, ability: Some(A::PowerLunch) });
    s.rebuild();
    assert_eq!((s.max_hp(), s.max_mp(), s.strength(), s.magic(), s.defense()), (265 + 13, 169 + 9, 62 + 2 + 4 + 3, 64 + 3 + 2, 59 + 1 + 4));
    // Berserk Charge (row CHARGE_BERSERK): Strength +1 while the MP charge runs.
    s.meal = Some(Meal { ability: Some(A::ChargeBerserk), ..Meal::default() });
    s.rebuild();
    let before = s.strength();
    s.force_mp_zero();
    assert_eq!(s.strength(), before + 1);
    // Defender (row DEFENDER; thresholds 25 / 50 / 75 / always).
    let mut d = default99();
    let base = d.defense();
    for (copies, hp_percent_hits) in [(1u8, [false, false, false, true]), (2, [false, false, true, true]), (3, [false, true, true, true])] {
        d.set_ability(A::Defender, copies, copies);
        for (i, hp) in [265, 198, 132, 66].into_iter().enumerate() {
            d.hp = hp; // 100 %, 74.7 %, 49.8 %, 24.9 %
            assert_eq!(d.defense() - base, if hp_percent_hits[i] { 2 } else { 0 }, "{copies} copies at HP {hp}");
        }
    }
}

// ------------------------------------------------------------------------------------ 2. EXP

#[test]
fn the_level_table_round_trips() {
    // Spec 2.2; sheet sora-levels: level 2 at 80, level 99 at 1,527,640.
    assert_eq!((tables::level_from_exp(0), tables::level_from_exp(79), tables::level_from_exp(80), tables::level_from_exp(279), tables::level_from_exp(280)), (1, 1, 2, 2, 3));
    assert_eq!((tables::level_from_exp(1_527_639), tables::level_from_exp(1_527_640), tables::level_from_exp(9_999_999)), (98, 99, 99));
    assert_eq!(tables::LEVELS.len(), 99);
    for level in 1..=99u32 {
        let row = tables::level_row(level);
        assert_eq!(row.level_name, level.to_string());
        assert_eq!(tables::level_from_exp(row.exp_total), level);
        if level > 1 {
            assert_eq!(tables::level_from_exp(row.exp_total - 1), level - 1);
            let previous = tables::level_row(level - 1);
            assert!(row.exp_total > previous.exp_total && row.attack >= previous.attack && row.ap >= previous.ap);
        }
    }
    // Spec 2.2 samples.
    let row = |l: u32| { let r = tables::level_row(l); (r.exp_total, r.attack, r.magic, r.defense, r.ap) };
    assert_eq!((row(10), row(30), row(50)), ((5_410, 15, 17, 12, 40), (87_850, 25, 28, 23, 76), (376_030, 36, 38, 34, 118)));
    // All three paths hold the same 28 abilities, in different orders (spec 2.2). (X1) The port excludes two of them
    // (ATTRACTION_TIME, ATTRACTION_UP; decision dec029): 26 are granted, the KH3 rows still name 28.
    // (XSI, 2026-10-16) The six slots X1 emptied hold the XS abilities (XS_SORA_ABILITIES.md 4.1, approved): level A 12
    // Spiral Rush, C 18 Critical Counter, 22 Diving Strike on all three paths, B 44 Light Wave: 28 granted per path,
    // the 26 KH3 abilities common to the three, the slot abilities by path.
    let path = |p: u8| { let mut v: Vec<i32> = (1..=99).filter_map(|l| tables::level_ability(l, p)).map(|k| k.value()).collect(); v.sort(); v };
    assert_eq!((path(0).len(), path(1).len(), path(2).len()), (28, 28, 28));
    let named = (1..=99).filter(|&l| tables::level_row(l).ability_a != "-").count();
    assert_eq!(named, 28);
    assert_eq!((tables::level_ability(12, 0), tables::level_ability(22, 1), tables::level_ability(18, 2), tables::level_row(22).ability_a), (Some(A::SpiralRush), Some(A::DivingStrike), Some(A::CriticalCounter), "DIVING_STRIKE"));
    let xs = [A::SpiralRush, A::CriticalCounter, A::DivingStrike, A::LightWave].map(|k| k.value());
    let common = |p: u8| path(p).into_iter().filter(|k| !xs.contains(k)).collect::<Vec<_>>();
    assert!(common(0).len() == 26 && common(0) == common(1) && common(1) == common(2));
    assert_eq!((tables::level_ability(2, 0), tables::level_ability(2, 1), tables::level_ability(2, 2)), (Some(A::ComboPlus), Some(A::MagicComboSave), Some(A::CriticalHalf)));
}

#[test]
fn exp_awards_are_ceiled_scaled_by_difficulty_and_capped() {
    // Spec 4.1 / 4.2; sheet sora-stats-rules rows exp.enemy_reward, exp.award, exp.total_cap;
    // sheet sora-difficulty column exp_rule.
    assert_eq!((Status::enemy_reward(3, 1.5), Status::enemy_reward(10, 1.0), Status::enemy_reward(7, 0.1)), (5, 10, 1));
    let award = |game_level: &str, base: i32| new_game("balance", game_level).give_exp(base, 1.0);
    assert_eq!((award("STANDARD", 50).added, award("PROUD", 50).added), (50, 50));
    assert_eq!(award("BEGINNER", 50).added, 50 + 13, "e + ceil(0.25 e)");
    assert_eq!(award("CRITICAL", 50).added, 38, "ceil(0.75 e)");
    assert_eq!(award("CRITICAL", 5).added, 4);
    // At most 999 per award.
    let mut s = new_game("balance", "STANDARD");
    let a = s.give_exp(5_000, 1.0);
    assert_eq!((a.reward, a.adjusted, a.added, a.total), (5_000, 999, 999, 999));
    assert_eq!(new_game("balance", "BEGINNER").give_exp(900, 1.0).added, 999);
    // The total stops at 9,999,999 and reports what was really added.
    let mut s = default99();
    s.exp = 9_999_500;
    let a = s.give_exp(999, 1.0);
    assert_eq!((a.added, a.total, s.exp, s.level), (499, 9_999_999, 9_999_999, 99));
    assert_eq!(s.give_exp(999, 1.0).added, 0);
    // Zero EXP (sheet sora-abilities row EXPZERO): nothing accrues at all while equipped.
    let mut s = new_game("balance", "STANDARD");
    s.set_ability(A::Expzero, 1, 1);
    assert_eq!((s.give_exp(500, 1.0).added, s.exp, s.level), (0, 0, 1));
    s.set_ability(A::Expzero, 1, 0);
    assert_eq!(s.give_exp(500, 1.0).added, 500, "owned but not equipped does nothing");
    // A dead Sora gets nothing (AdjustExp: HP <= 0).
    let mut s = new_game("balance", "STANDARD");
    s.hp = 0;
    assert_eq!(s.give_exp(500, 1.0).added, 0);
    // EXP Incentive (row EXP_BARGAIN): exp += ceil((0.5 + 0.1 * max(battle level - level, 0)) * exp).
    let mut s = new_game("balance", "STANDARD");
    s.meal = Some(Meal { ability: Some(A::ExpBargain), ..Meal::default() });
    s.battle_level = 11;
    s.rebuild();
    assert_eq!(s.give_exp(40, 1.0).added, 40 + 60, "d = 10: factor 1.5");
    let mut high = default99();
    high.meal = Some(Meal { ability: Some(A::ExpBargain), ..Meal::default() });
    high.rebuild();
    assert_eq!(high.give_exp(40, 1.0).added, 60, "level above the battle level: factor 0.5");
}

#[test]
fn one_award_can_raise_several_levels_and_refills_nothing() {
    // Spec 4.3: the level is recomputed from the EXP; one panel; no HP / MP refill.
    let mut s = new_game("wisdom", "STANDARD");
    s.hp = 40;
    s.mp = 10;
    let a = s.give_exp(999, 1.0);
    assert_eq!((a.level_before, a.level_after, s.level), (1, 4, 4), "999 EXP is level 4 (sheet sora-levels: 640 <= 999 < 1120)");
    let up = a.level_up.unwrap();
    // LevelUpDelta(newLevel) = row(4) - row(3): Strength 0, Magic +1, Defense 0, AP +1, ability of row 4 on path A.
    assert_eq!((up.level, up.strength, up.magic, up.defense, up.ap, up.ability), (4, 0, 1, 0, 1, Some(A::AircomboPlus)));
    assert_eq!((s.hp, s.mp, s.hp_max, s.mp_max), (40, 10, 90, 120), "no refill, no change of max HP / MP");
    assert_eq!(a.next_lv, Some(1_120 - 999));
    assert_eq!((s.exp_level, s.exp_next, s.exp_to_next()), (640, 1_120, 121));
    assert!((s.exp_fraction() - 359.0 / 480.0).abs() < 1e-6, "spec 4.4 gauge fraction");
    // The abilities of every level passed are owned (regranted on rebuild), none is equipped by the level-up.
    assert_eq!((s.abilities.owned(A::ComboPlus), s.abilities.owned(A::PrizeDraw), s.abilities.owned(A::AircomboPlus)), (1, 1, 1));
    assert_eq!((s.abilities.equipped(A::ComboPlus), s.has(A::ComboPlus), s.max_combo(false)), (0, false, 4));
    assert_eq!((s.base.strength, s.base.magic, s.base.defense, s.max_ap()), (12, 14, 9, 31));
    // An award inside a level: no panel.
    let b = s.give_exp(10, 1.0);
    assert_eq!((b.level_up, b.next_lv, b.show_exp_point), (None, Some(111), true));
}

#[test]
fn levelling_from_1_to_99_arrives_at_the_level_99_loadout() {
    // `--level N`: the same loadout at another level, from the level table.
    let full = loadout_with(&[]);
    let at99 = Status::from_loadout(&full);
    let mut low = full.clone();
    low.set_level(1);
    let mut s = Status::from_loadout(&low);
    assert_eq!((s.level, s.exp, s.strength(), s.magic(), s.defense(), s.max_ap()), (1, 0, 15, 15, 8, 28));
    assert_eq!((s.hp_max, s.mp_max), (265, 169), "HP / MP have no level term (spec 0)");
    // The level table's grants above level 1 are not owned yet.
    assert_eq!((s.abilities.owned(A::ComboPlus), s.abilities.owned(A::CriticalHalf), s.abilities.owned(A::PrizeDraw), s.abilities.owned(A::Superslide)), (0, 1, 1, 3));
    assert!(s.ap_within_budget(), "the equipped set is trimmed to the level's AP: {} of {}", s.ap_in_use(), s.max_ap());
    assert!(low.verify(&s).is_empty(), "the sheet's level-99 statements are not compared at another level");
    let mut level_ups = 0;
    let mut awards = 0u32;
    while s.level < 99 {
        let a = s.give_exp(999, 1.0);
        assert_eq!(a.added, 999);
        level_ups += a.level_up.is_some() as u32;
        awards += 1;
    }
    assert_eq!(awards, 1_527_640_u32.div_ceil(999));
    assert!(level_ups > 60 && level_ups <= 98);
    assert_eq!((s.strength(), s.magic(), s.defense(), s.max_ap()), (66, 67, 59, 180));
    for &kind in A::ALL {
        assert_eq!(s.abilities.owned(kind), at99.abilities.owned(kind), "{}", kind.name());
    }
    // A middle level straight from the table: sheet sora-levels row 50.
    let mut mid = full.clone();
    mid.set_level(50);
    let m = Status::from_loadout(&mid);
    assert_eq!((m.level, m.exp, m.base.strength, m.base.magic, m.base.defense, m.max_ap()), (50, 376_030, 36, 38, 34, 118));
    assert!(m.ap_within_budget());
}

#[test]
fn exp_popups_come_in_the_ui_specs_order() {
    // UI spec 11b test 6: award of 120 at level 1 (total 0): EXP point "120" at the enemy,
    // level-up popup LV 2 with its delta lines, Next LV 280 - 120 = 160. Order (reconciliation 2 item 13):
    // level-up, EXP point, Next LV.
    let world = TestWorld::floor();
    let mut sora = spawn(new_game("balance", "STANDARD"));
    let mut host = RecordingHost::new();
    let enemy = Vec3::new(300.0, 0.0, 90.0);
    sora.give_exp(120, 1.0, Some(enemy));
    let out = host.step(&world, |h| sora.step_hosted(DT, &Input::default(), h));
    let raised = popups(&out.events);
    assert_eq!(raised.iter().map(|p| p.kind).collect::<Vec<_>>(), [UiPopup::KIND_LEVEL_UP, UiPopup::KIND_EXP_POINT, UiPopup::KIND_NEXT_LV]);
    assert_eq!((raised[1].value, raised[1].position, raised[0].value, raised[2].value), (120.0, Some(enemy), 2.0, 160.0));
    let PopupDetail::LevelUp(up) = raised[0].detail else { panic!("no level-up lines") };
    // Sheet sora-levels row 2: d_attack 0, d_magic 1, d_defense 1, d_ap 0, ability_a COMBO_PLUS.
    assert_eq!((up.level, up.strength, up.magic, up.defense, up.ap, up.ability), (2, 0, 1, 1, 0, Some(A::ComboPlus)));
    let ui = host.ui.clone().unwrap();
    assert_eq!((ui.popups.len(), ui.gauges.level, ui.gauges.exp, ui.gauges.exp_to_next), (3, 2, 120, 160));
    assert_eq!((ui.board.exp_points.len(), ui.board.level_up.is_some(), ui.board.next_lv.is_some()), (1, true, true));
    // Lifetimes (sheet sora-ui-params rows popup.*): Next LV 146 frames, level up 272 frames, EXP point 3.0 s.
    let mut seen = (0, 0, 0);
    for step in 1..=300 {
        host.step(&world, |h| sora.step_hosted(DT, &Input::default(), h));
        let board = &host.ui.as_ref().unwrap().board;
        if board.next_lv.is_some() { seen.0 = step; }
        if board.level_up.is_some() { seen.1 = step; }
        if !board.exp_points.is_empty() { seen.2 = step; }
    }
    assert_eq!(seen, (145, 271, 179));
    // An award that crosses several levels: ONE panel with the final level.
    sora.give_exp(999, 1.0, Some(enemy));
    let raised = popups(&host.step(&world, |h| sora.step_hosted(DT, &Input::default(), h)).events);
    assert_eq!(raised.iter().map(|p| (p.kind, p.value)).collect::<Vec<_>>(), [(UiPopup::KIND_LEVEL_UP, 4.0), (UiPopup::KIND_EXP_POINT, 999.0), (UiPopup::KIND_NEXT_LV, 1.0)]);
    // Without a position: no EXP point.
    sora.give_exp(1, 1.0, None);
    let raised = popups(&host.step(&world, |h| sora.step_hosted(DT, &Input::default(), h)).events);
    assert_eq!(raised.iter().map(|p| (p.kind, p.value)).collect::<Vec<_>>(), [(UiPopup::KIND_LEVEL_UP, 5.0), (UiPopup::KIND_NEXT_LV, 560.0)]);
    // At level 99: no EXP point, no Next LV, but the EXP accrues (spec 4.2).
    let mut top = spawn(default99());
    top.give_exp(500, 1.0, Some(enemy));
    let out = top.step(DT, &Input::default(), &world);
    assert!(popups(&out.events).is_empty());
    assert_eq!((top.status().exp, top.status().level, top.ui_snapshot().gauges.exp_to_next), (1_528_140, 99, 0));
}

// ------------------------------------------------------------------------------------ 3. damage

fn standard() -> DamageContext {
    DamageContext::new(GameLevel::Standard, 1)
}

fn enemy(defense: i32, min: i32, max: i32) -> Defender {
    Defender { has_status: true, defense, max_hp: 1000, hp_percent: 100.0, kind: DefenderKind::Enemy, damage_min: min, damage_max: max, ..Defender::NONE }
}

#[test]
fn damage_sora_hits_a_target() {
    // Spec section 7 steps 4, 6, 9; worked by hand from the formula. Level 99, Kingdom Key:
    // Strength 66, Magic 67; the sheet's equipped set has one copy each of COMBO_UP, AIRCOMBO_UP,
    // MAGIC_COMBO_UP, COUNTER_UP and the five element boosts.
    let s = default99();
    let me = s.as_attacker();
    let hit = |attack: DamageAttack, defender: &Defender| compute_damage(&me, defender, &attack, &standard()).amount;
    // A target without a status object: Defense 0, no floor, no cap. ceil(66 * 1.0) = 66.
    assert_eq!(hit(DamageAttack::physical(1.0), &Defender::NONE), 66.0);
    assert_eq!(hit(DamageAttack::physical(2.5), &Defender::NONE), 165.0);
    assert_eq!(hit(DamageAttack::physical(0.35), &Defender::NONE), 24.0, "ceil(23.1)");
    // Combo Boost (row COMBO_UP): += ceil(0.05 * count * c * dmg) per combo step. c = 3: 66 + ceil(9.9) = 76.
    assert_eq!(hit(DamageAttack { combo_step: 3, ..DamageAttack::physical(1.0) }, &Defender::NONE), 76.0);
    // Air Combo Boost (row AIRCOMBO_UP) for hits flagged aerial. c = 2: 66 + ceil(6.6) = 73.
    assert_eq!(hit(DamageAttack { combo_step: 2, aerial: true, ..DamageAttack::physical(1.0) }, &Defender::NONE), 73.0);
    // Reprisal Boost (row COUNTER_UP): += ceil(0.5 * count * dmg): 66 + 33 = 99; with the combo step first: 76 + 38 = 114.
    assert_eq!(hit(DamageAttack { counter: true, ..DamageAttack::physical(1.0) }, &Defender::NONE), 99.0);
    assert_eq!(hit(DamageAttack { counter: true, combo_step: 3, ..DamageAttack::physical(1.0) }, &Defender::NONE), 114.0);
    // Magic uses the Magic stat. Fire, power 2.0: ceil(67 * 2) = 134; Fire Boost (row FIRE_UP) += ceil(0.2 * 134) = 27 -> 161.
    assert_eq!(hit(DamageAttack::magic(2.0, Attr::Fire), &Defender::NONE), 161.0);
    // Magic Galvanizer (row MAGIC_COMBO_UP) first, c = 2: 134 + ceil(26.8) = 161; then Fire Boost: 161 + ceil(32.2) = 194.
    assert_eq!(hit(DamageAttack { combo_step: 2, ..DamageAttack::magic(2.0, Attr::Fire) }, &Defender::NONE), 194.0);
    // An attribute without a boost ability (dark, none): 134.
    assert_eq!(hit(DamageAttack::magic(2.0, Attr::Dark), &Defender::NONE), 134.0);
    // The magic stat is not boosted by the physical combo ability and vice versa.
    assert_eq!(hit(DamageAttack { combo_step: 3, ..DamageAttack::magic(1.0, Attr::Notype) }, &Defender::NONE), 67.0 + 21.0);
    // An enemy with a status object (spec step 5). Defense 20: base 46.
    assert_eq!(hit(DamageAttack::physical(1.0), &enemy(20, 0, 0)), 46.0);
    // Stronger defence than attack: base 1, raised to the enemy floor 1 + ceil(0.1 * battle level).
    let at = |level: i32, attack: DamageAttack, defender: &Defender| compute_damage(&me, defender, &attack, &DamageContext::new(GameLevel::Standard, level));
    assert_eq!(at(30, DamageAttack::physical(1.0), &enemy(100, 0, 0)).base, 4.0);
    assert_eq!(at(30, DamageAttack::physical(3.0), &enemy(100, 0, 0)).amount, 12.0);
    assert_eq!(at(1, DamageAttack::physical(1.0), &enemy(100, 0, 0)).amount, 2.0);
    // The row's own m_DamageMin wins over the level floor.
    assert_eq!(at(30, DamageAttack::physical(1.0), &enemy(100, 9, 0)).base, 9.0);
    // m_DamageMax: base 46 > 30 -> min(46, max(floor((30 + 66 - 20) / 2), min)) = 38.
    assert_eq!(at(30, DamageAttack::physical(1.0), &enemy(20, 0, 30)).base, 38.0);
    // Above battle level 70 an enemy's base is also held at max * 2: floor((10 + 46) / 2) = 28 -> 20.
    assert_eq!(at(80, DamageAttack::physical(1.0), &enemy(20, 0, 10)).base, 20.0);
    assert_eq!(at(70, DamageAttack::physical(1.0), &enemy(20, 0, 10)).base, 28.0);
    // Attribute resist: ceil(clamp(100 - r, 0, 200) / 100 * dmg). Fire 2.0 on Defense 20: ceil(47 * 2) = 94.
    let resist = |r: i32| { let mut d = enemy(20, 0, 0); d.resist[Attr::Fire.value() as usize] = r; d };
    let fire_no_boost = |d: &Defender| compute_damage(&Attacker { abilities: None, ..me }, d, &DamageAttack::magic(2.0, Attr::Fire), &standard()).amount;
    assert_eq!((fire_no_boost(&resist(0)), fire_no_boost(&resist(50)), fire_no_boost(&resist(-50)), fire_no_boost(&resist(-300))), (94.0, 47.0, 141.0, 188.0));
    // Immune: an exact 0 is returned as 0.1 (floored to 0 HP by the HP rule).
    assert_eq!(fire_no_boost(&resist(100)), 0.1);
    assert_eq!(fire_no_boost(&resist(250)), 0.1);
    // Percent attacks and the degenerate cases (steps 1, 2).
    assert_eq!(hit(DamageAttack { calc: CalcType::MaxHpRatio, ..DamageAttack::physical(0.1) }, &enemy(20, 0, 0)), 100.0);
    assert_eq!(hit(DamageAttack { calc: CalcType::ValueRatio(265), ..DamageAttack::physical(0.1) }, &Defender::NONE), 27.0);
    assert_eq!(hit(DamageAttack::physical(0.0), &Defender::NONE), 0.1);
    assert_eq!(hit(DamageAttack { recover: true, ..DamageAttack::magic(1.0, Attr::Notype) }, &Defender::NONE), 67.0, "a RECOVER hit returns the stat (listing 0x140632407)");
    // The host's numbers for a target (Targets::defense) and the default "no status object".
    let host_numbers = TargetDefense { defense: 20, max_hp: 500, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 1 };
    assert_eq!(hit(DamageAttack::physical(1.0), &Defender::of_target(Some(&host_numbers))), 46.0);
    assert_eq!(Defender::of_target(None), Defender::NONE);
}

#[test]
fn damage_ability_multipliers_compound_in_the_listings_order() {
    // Spec section 7 step 9 with counts that the level-99 loadout does not have.
    let mut set = AbilitySet::EMPTY;
    let attacker = |set: &AbilitySet| compute_damage(&Attacker { has_status: true, strength: 50, magic: 40, abilities: Some(set), side: kh_core::status::Side::Player, state_boost: false }, &enemy(10, 0, 0), &DamageAttack { combo_step: 2, counter: true, just_guard: true, full_mp: true, ..DamageAttack::physical(1.0) }, &standard()).amount;
    assert_eq!(attacker(&set), 40.0, "no ability: base 40");
    // Combo Boost per step per copy: 3 copies, c = 2: 40 + ceil(0.05 * 3 * 2 * 40) = 52.
    set.set(A::ComboUp, 3, 3);
    assert_eq!(attacker(&set), 52.0);
    // Reprisal Boost 2 copies: 52 + ceil(0.5 * 2 * 52) = 104.
    set.set(A::CounterUp, 2, 2);
    assert_eq!(attacker(&set), 104.0);
    // Critical Counter (row CRITICAL_COUNTER): counter + just guard deals x2: 208.
    set.set(A::CriticalCounter, 1, 1);
    assert_eq!(attacker(&set), 208.0);
    // Heartless Buster 2 copies on a Heartless: ceil((1 + 0.3 * 2) * 208) = 333.
    set.set(A::HeartlessBuster, 2, 2);
    set.set(A::NobodyBuster, 3, 3);
    let on = |set: &AbilitySet, bio: u8, attack: DamageAttack| compute_damage(&Attacker { has_status: true, strength: 50, magic: 40, abilities: Some(set), side: kh_core::status::Side::Player, state_boost: false }, &Defender { bio_type: bio, ..enemy(10, 0, 0) }, &attack, &standard()).amount;
    let full = DamageAttack { combo_step: 2, counter: true, just_guard: true, full_mp: true, ..DamageAttack::physical(1.0) };
    assert_eq!((on(&set, 0, full), on(&set, 1, full), on(&set, 2, full)), (208.0, 333.0, 396.0));
    // Full MP Blast (row FULLMP_BURST) on hits flagged full MP: += ceil(0.5 * n * dmg): 208 + 104.
    set.set(A::FullmpBurst, 1, 1);
    assert_eq!(on(&set, 0, full), 312.0);
    assert_eq!(on(&set, 0, DamageAttack { full_mp: false, ..full }), 208.0);
    // The "no bonus" call skips the combo boost and Full MP Blast, not the counter boosts.
    let no_bonus = compute_damage(&Attacker { has_status: true, strength: 50, magic: 40, abilities: Some(&set), side: kh_core::status::Side::Player, state_boost: false }, &enemy(10, 0, 0), &full, &DamageContext { no_bonus: true, ..standard() }).amount;
    assert_eq!(no_bonus, (40.0 + 40.0) * 2.0);
    // Double Team (row JOIN_FORCE): +5 on the stat before the defence when the target is shared.
    set = AbilitySet::EMPTY;
    set.set(A::JoinForce, 1, 1);
    let join = |shared: bool| compute_damage(&Attacker { has_status: true, strength: 50, magic: 40, abilities: Some(&set), side: kh_core::status::Side::Player, state_boost: false }, &enemy(10, 0, 0), &DamageAttack::physical(1.0), &DamageContext { join_force_target: shared, ..standard() }).amount;
    assert_eq!((join(false), join(true)), (40.0, 45.0));
    // The attacker's state boost (vt+0x98): ceil(1.5 * dmg).
    let boosted = compute_damage(&Attacker { has_status: true, strength: 50, magic: 40, abilities: Some(&AbilitySet::EMPTY), side: kh_core::status::Side::Player, state_boost: true }, &enemy(10, 0, 0), &DamageAttack::physical(1.0), &standard()).amount;
    assert_eq!(boosted, 60.0);
}

#[test]
fn damage_on_sora_is_capped_at_80_before_the_power_and_scaled_by_difficulty() {
    // Spec section 7 step 5 (kind 0: base at most 80; min 1, 13 above battle level 70), step 8
    // (sheet sora-difficulty column enemy_damage), step 10 (row CRITICAL_HALF).
    let hit_on = |status: &Status, attack: i32, power: f32| compute_damage(&Attacker::enemy(attack), &status.as_defender(), &DamageAttack::physical(power), &DamageContext::of(status));
    let s = default99();
    // Attack 200 against Defense 59: 141 -> 80; x1.5 = 120.
    let r = hit_on(&s, 200, 1.5);
    assert_eq!((r.base, r.after_power, r.amount), (80.0, 120.0, 120.0));
    assert_eq!(hit_on(&s, 100, 1.5).amount, 62.0, "below the cap: ceil(41 * 1.5)");
    assert_eq!(hit_on(&s, 30, 1.5).amount, 2.0, "attack below defence: base 1");
    let mut deep = default99();
    deep.battle_level = 80;
    assert_eq!(hit_on(&deep, 30, 1.5).amount, 20.0, "above battle level 70 the floor is 13: ceil(19.5)");
    for (game_level, expected) in [("BEGINNER", 60.0), ("STANDARD", 120.0), ("PROUD", 180.0), ("CRITICAL", 240.0)] {
        assert_eq!(hit_on(&level99("balance", game_level), 200, 1.5).amount, expected, "{game_level}");
    }
    // Damage dealt by the player side: x1.25 on Beginner only.
    let beginner = level99("balance", "BEGINNER");
    assert_eq!(compute_damage(&beginner.as_attacker(), &Defender::NONE, &DamageAttack::physical(1.0), &DamageContext::of(&beginner)).amount, 83.0, "ceil(66 * 1.25)");
    // Damage Control: at HP% <= 25, x0.5 with one copy, x1/3 with two or more (the sheet's loadout has two).
    let mut low = default99();
    low.hp = 66; // 24.9 %
    assert_eq!(hit_on(&low, 200, 1.5).amount, 40.0);
    low.hp = 67; // 25.3 %
    assert_eq!(hit_on(&low, 200, 1.5).amount, 120.0);
    low.hp = 66;
    low.set_ability(A::CriticalHalf, 0, 0); // the level table's copy stays owned
    low.abilities.set(A::CriticalHalf, 1, 1);
    assert_eq!(hit_on(&low, 200, 1.5).amount, 60.0);
    low.abilities.set(A::CriticalHalf, 1, 0);
    assert_eq!(hit_on(&low, 200, 1.5).amount, 120.0);
    // Critical at low HP with both copies: ceil(240 / 3) = 80.
    let mut critical = level99("balance", "CRITICAL");
    critical.hp = 30;
    assert_eq!(hit_on(&critical, 200, 1.5).amount, 80.0);
}

// ------------------------------------------------------------------------------------ 4. HP

#[test]
fn second_chance_needs_hp_2_and_withstand_combo_runs_on_the_06_s_timer() {
    // Spec 6.1; sheet sora-abilities rows LAST_LEAVE, COMBO_LEAVE; sora-stats-rules row hp.combo_leave_timer.
    let lethal = HitOnSora::default();
    let mut s = default99();
    assert!(s.has(A::LastLeave) && s.has(A::ComboLeave));
    // The first lethal hit is survived by Second Chance.
    let a = s.apply_damage_to_sora(9_999.0, &lethal);
    assert_eq!((a.hp_before, a.hp_after, a.lost, a.survived, a.dead), (265, 1, 264, Some(Survived::SecondChance), false));
    assert_eq!(s.combo_leave_timer, 0.6);
    // Further lethal hits within 0.6 s of the previous hit: Withstand Combo, also from HP 1.
    for _ in 0..30 {
        s.tick(DT, 0.0);
    }
    assert!((s.combo_leave_timer - 0.1).abs() < 1e-4);
    let b = s.apply_damage_to_sora(50.0, &lethal);
    assert_eq!((b.hp_after, b.survived), (1, Some(Survived::WithstandCombo)));
    assert_eq!(s.combo_leave_timer, 0.6, "every hit resets the timer");
    // After the timer ran out, outside the damage state: nothing saves him at HP 1.
    for _ in 0..40 {
        s.tick(DT, 0.0);
    }
    assert_eq!(s.combo_leave_timer, 0.0);
    let mut in_state = s.clone();
    let c = s.apply_damage_to_sora(1.0, &lethal);
    assert_eq!((c.hp_after, c.survived, c.dead), (0, None, true));
    assert!(!s.apply_damage_to_sora(10.0, &lethal).applied, "HP < 1: nothing more happens");
    // ... but while he is in the damage state (the combat strand's hook) Withstand Combo holds.
    in_state.in_damage_state = true;
    assert_eq!(in_state.apply_damage_to_sora(1.0, &lethal).survived, Some(Survived::WithstandCombo));
    // Second Chance alone: from HP 2 yes, from HP 1 no; and it does not depend on the difficulty.
    for game_level in ["STANDARD", "CRITICAL"] {
        let mut only = level99("balance", game_level);
        only.set_ability(A::ComboLeave, 0, 0);
        only.abilities.set(A::ComboLeave, 1, 0); // the level table's copy, unequipped
        only.hp = 2;
        assert_eq!(only.apply_damage_to_sora(500.0, &lethal).survived, Some(Survived::SecondChance));
        only.tick(DT, 0.0);
        assert_eq!(only.combo_leave_timer, 0.0, "without COMBO_LEAVE the timer is held at 0");
        assert!(only.apply_damage_to_sora(500.0, &lethal).dead);
    }
    // Without either ability a lethal hit kills at once; the damage is floored.
    let mut bare = new_game("balance", "STANDARD");
    assert_eq!(bare.apply_damage_to_sora(10.9, &lethal).hp_after, 95);
    assert_eq!(bare.apply_damage_to_sora(0.1, &lethal).lost, 0, "the damage function's 0.1 takes no HP");
    assert!(!bare.apply_damage_to_sora(0.0, &lethal).applied);
    assert!(bare.apply_damage_to_sora(95.0, &lethal).dead);
    // The hit's kill rule and the invincible pawn leave 1.
    let mut ruled = new_game("balance", "STANDARD");
    assert_eq!(ruled.apply_damage_to_sora(500.0, &HitOnSora { leave_one: true, ..lethal }).survived, Some(Survived::LeaveOne));
    ruled.hp = 50;
    ruled.invincible = true;
    assert_eq!(ruled.apply_damage_to_sora(500.0, &lethal).survived, Some(Survived::Invincible));
}

#[test]
fn low_hp_heals_and_the_syphons() {
    // Spec 6.2: pinch = HP <= maxHP * 25 / 100, integer arithmetic (row hp.pinch).
    let mut s = level99("wisdom", "STANDARD");
    for (hp, pinch) in [(250, false), (63, false), (62, true), (1, true)] {
        s.hp = hp;
        assert_eq!(s.is_pinch(), pinch, "HP {hp} of 250");
    }
    let mut c = level99("vitality", "CRITICAL");
    c.hp = 35;
    assert!(!c.is_pinch());
    c.hp = 34;
    assert!(c.is_pinch(), "max HP 139: pinch at HP <= 34 (UI spec 11b test 3)");
    // Spec 6.3: SetHitPoint clamps; Recover; FullRecover.
    s.set_hp(9_999);
    assert_eq!(s.hp, 250);
    s.set_hp(-5);
    assert_eq!(s.hp, 0);
    s.hp = 100;
    s.mp = 20;
    s.focus = 10;
    s.recover(30, 15, 5, 0);
    assert_eq!((s.hp, s.mp, s.focus), (130, 35, 15));
    s.full_recover();
    assert_eq!((s.hp, s.mp, s.focus, s.mp_charging), (250, 179, 100, false));
    // Spec 6.4, Damage Syphon (row DAMAGE_ASPIR): MP += ceil(HP lost / max HP * 100 * count).
    let hit = HitOnSora::default();
    s.set_ability(A::DamageAspir, 2, 2);
    s.mp = 100;
    let out = s.apply_damage_to_sora(50.0, &hit);
    assert_eq!((out.lost, out.mp_gained, s.mp), (50, 40, 140), "ceil(50 / 250 * 100 * 2)");
    // Attribute syphons (rows FIRE_ASPIR .. DARK_ASPIR): MP += ceil(power * 3 * count), power = the hit's source power.
    s.set_ability(A::DamageAspir, 0, 0);
    s.set_ability(A::FireAspir, 2, 2);
    s.mp = 100;
    assert_eq!(s.apply_damage_to_sora(10.0, &HitOnSora { attribute: Attr::Fire, src_power: 1.5, ..hit }).mp_gained, 9);
    assert_eq!(s.apply_damage_to_sora(10.0, &HitOnSora { attribute: Attr::Blizzard, src_power: 1.5, ..hit }).mp_gained, 0);
    assert_eq!(s.apply_damage_to_sora(10.0, &HitOnSora { attribute: Attr::Fire, src_power: 9.0, percent: true, ..hit }).mp_gained, 6, "percent damage: power 1");
    // Not while the MP charge runs.
    s.force_mp_zero();
    assert_eq!(s.apply_damage_to_sora(10.0, &HitOnSora { attribute: Attr::Fire, src_power: 1.5, ..hit }).mp_gained, 0);
    // HP Walker / MP Walker (rows WALK_HEALING, WALK_REGENE): after 5 s of movement +1 every 1 / (count + 1) s.
    let mut w = level99("wisdom", "STANDARD");
    w.set_ability(A::WalkHealing, 1, 1);
    w.set_ability(A::WalkRegene, 3, 3);
    w.hp = 100;
    w.mp = 50;
    let moving = 680.0 * 680.0;
    for _ in 0..(5 * 60 - 2) {
        w.tick(DT, moving);
    }
    assert_eq!((w.hp, w.mp), (100, 50), "nothing in the first 5 s");
    for _ in 0..(2 * 60 + 4) {
        w.tick(DT, moving);
    }
    assert_eq!((w.hp, w.mp), (100 + 4, 50 + 8), "2 s: every 0.5 s and every 0.25 s");
    // Standing still (past the 0.25 s grace) resets the timer.
    for _ in 0..20 {
        w.tick(DT, 0.0);
    }
    let (hp, mp) = (w.hp, w.mp);
    for _ in 0..200 {
        w.tick(DT, moving);
    }
    assert_eq!((w.hp, w.mp), (hp, mp));
    // Percent damage (0x1405A3970) is of max or of current HP.
    let mut p = new_game("wisdom", "STANDARD");
    assert_eq!(p.apply_percent_damage(0.5, false, &hit).lost, 45);
    assert_eq!(p.apply_percent_damage(0.2, true, &hit).lost, 9);
}

// ------------------------------------------------------------------------------------ 5. MP

#[test]
fn mp_overdraw_starts_the_charge_and_landing_on_zero_does_not() {
    // Magic spec part C section 2 (UseMagicPoint 0x1405BCDC0) and 3 (SetMpCharge).
    let mut s = default99();
    assert_eq!((s.mp, s.mp_max), (169, 169));
    assert_eq!(s.use_mp(69).used, 69);
    // Landing exactly on 0 does not start the charge; the next cast does.
    let exact = s.use_mp(100);
    assert_eq!((exact.used, exact.charge_started, s.mp, s.mp_charging), (100, false, 0, false));
    assert!(s.can_cast(MpCost::Points(10), 0), "without MP Safety a spell can be cast at 0 MP: it triggers the charge");
    let next = s.use_mp(10);
    assert_eq!((next.used, next.charge_started, s.mp, s.mp_charging, s.mp_charge_timer), (0, true, 0, true, 50.0));
    // During the charge: no cast, SetMagicPoint and UseMagicPoint are no-ops, MP stays 0.
    assert!(!s.can_cast(MpCost::Points(1), 0) && !s.can_cast(MpCost::All, 0));
    s.set_mp(50);
    assert_eq!((s.use_mp(5).used, s.mp), (0, 0));
    assert_eq!(s.mp_charge_gauge(), 169.0, "timer / T * maxMP");
    // An overdraw: the rest is spent and the charge starts.
    let mut o = default99();
    o.mp = 5;
    let over = o.use_mp(10);
    assert_eq!((over.used, over.charge_started, o.mp), (5, true, 0));
    // Cure's cost 255 spends everything (and is never blocked by MP Safety).
    let mut c = default99();
    c.mp = 30;
    let all = c.spend_command_mp(MpCost::from_row(255), 0);
    assert_eq!((all.used, all.charge_started), (30, true));
    // Extra Cast (row MP_LEAVE): an overdraw with MP > 0 goes to 0 WITHOUT the charge; the next cast starts it.
    let mut e = default99();
    e.set_ability(A::MpLeave, 1, 1);
    e.mp = 5;
    let extra = e.use_mp(10);
    assert_eq!((extra.used, extra.charge_started, e.mp), (5, false, 0));
    assert!(e.use_mp(10).charge_started);
    // MP Safety (row MP_SAFETY): a spell costing more than the MP is refused, not Cure; Extra Cast lets it pass.
    let mut safe = default99();
    safe.set_ability(A::MpSafety, 1, 1);
    safe.mp = 5;
    assert!(!safe.can_cast(MpCost::Points(10), 0) && safe.can_cast(MpCost::Points(5), 0) && safe.can_cast(MpCost::All, 0));
    safe.set_ability(A::MpLeave, 1, 1);
    assert!(safe.can_cast(MpCost::Points(10), 0));
    safe.mp = 0;
    assert!(!safe.can_cast(MpCost::Points(10), 0));
}

#[test]
fn mp_costs_with_the_thrift_abilities() {
    // Magic spec C 2, EffCost 0x1405A4B90; rows MP_SAVE, MAGIC_COMBO_SAVE.
    let mut s = default99();
    assert!(s.has(A::MagicComboSave), "equipped in the sheet's loadout");
    assert_eq!((s.effective_mp_cost(20, 0), s.effective_mp_cost(20, 3), s.effective_mp_cost(20, 30), s.effective_mp_cost(0, 3)), (20, 17, 1, 0));
    s.set_ability(A::MagicComboSave, 0, 0);
    s.abilities.set(A::MagicComboSave, 1, 0);
    assert_eq!(s.effective_mp_cost(20, 3), 20);
    // MP Thrift: cost = max(cost - floor(0.2 * n * cost + 0.9), 1).
    s.set_ability(A::MpSave, 1, 1);
    assert_eq!((s.effective_mp_cost(20, 0), s.effective_mp_cost(14, 0), s.effective_mp_cost(10, 0), s.effective_mp_cost(1, 0)), (16, 11, 8, 1));
    s.set_ability(A::MpSave, 2, 2);
    assert_eq!((s.effective_mp_cost(20, 0), s.effective_mp_cost(28, 0)), (12, 16));
    // Spent through the command helper: Firaga 20 with two Thrift copies costs 12.
    assert_eq!((s.spend_command_mp(MpCost::from_row(20), 0).used, s.mp), (12, 157));
}

#[test]
fn a_hosts_mp_cost_scale_comes_before_the_thrift() {
    // (IMPL_ER61) the Elden Ring host's factor; 1 (the default) leaves KH3's numbers alone.
    let mut s = default99();
    s.set_ability(A::MagicComboSave, 0, 0);
    s.abilities.set(A::MagicComboSave, 1, 0);
    assert_eq!(s.mp_cost_scale, 1.0);
    assert_eq!((s.effective_mp_cost(10, 0), s.effective_mp_cost(17, 0)), (10, 17));
    s.mp_cost_scale = 1.5;
    // Fire 10 -> 15, Blizzara 17 -> 26 (25.5 rounded), Thundaga 28 -> 42; a cost of 0 stays 0
    assert_eq!((s.effective_mp_cost(10, 0), s.effective_mp_cost(17, 0), s.effective_mp_cost(28, 0), s.effective_mp_cost(0, 0)), (15, 26, 42, 0));
    // MP Thrift then works on the scaled cost: 30 - floor(0.2 x 30 + 0.9) = 24
    s.set_ability(A::MpSave, 1, 1);
    assert_eq!(s.effective_mp_cost(20, 0), 24);
    // spent through the command helper; "all MP" (Cure) is untouched
    s.mp = 100;
    assert_eq!(s.spend_command_mp(MpCost::from_row(20), 0).used, 24);
    assert_eq!(s.mp, 76);
}

#[test]
fn mp_charge_durations_by_haste_level_and_difficulty() {
    // Magic spec C 3: 50 s (70 on Critical; sheet sora-difficulty column mp_charge_time), rate
    // 1 + 0.1 a + 0.2 b + 0.3 c (rows MP_HASTE, MP_HASTERA, MP_HASTEGA).
    let seconds = |status: &mut Status| {
        status.force_mp_zero();
        assert!(status.mp_charging);
        let mut steps = 0;
        while status.mp_charging {
            status.tick(DT, 0.0);
            steps += 1;
            assert!(steps < 10_000);
        }
        assert_eq!(status.mp, status.mp_max, "the charge ends with full MP");
        steps as f32 * DT
    };
    let with = |haste: u8, hastera: u8, hastega: u8, game_level: &str| {
        let mut s = level99("balance", game_level);
        s.set_ability(A::MpHaste, haste, haste);
        s.set_ability(A::MpHastera, hastera, hastera);
        s.set_ability(A::MpHastega, hastega, hastega);
        (s.mp_charge_rate(), seconds(&mut s))
    };
    let close = |(rate, time): (f32, f32), expected_rate: f32, expected_time: f32| {
        assert!((rate - expected_rate).abs() < 1e-5, "rate {rate} != {expected_rate}");
        assert!((time - expected_time).abs() < 0.05, "time {time} != {expected_time}");
    };
    close(with(0, 0, 0, "STANDARD"), 1.0, 50.0);
    close(with(1, 0, 0, "STANDARD"), 1.1, 50.0 / 1.1);
    close(with(0, 1, 0, "STANDARD"), 1.2, 50.0 / 1.2);
    close(with(0, 0, 1, "STANDARD"), 1.3, 50.0 / 1.3);
    close(with(1, 1, 1, "STANDARD"), 1.6, 50.0 / 1.6);
    close(with(3, 0, 0, "STANDARD"), 1.3, 50.0 / 1.3);
    close(with(0, 0, 0, "CRITICAL"), 1.0, 70.0);
    close(with(1, 0, 0, "CRITICAL"), 1.1, 70.0 / 1.1);
    // The sheet's own loadout has one MP Haste equipped.
    assert!((default99().mp_charge_rate() - 1.1).abs() < 1e-6);
    // What shortens it: prizes 2 s / 6 s (spec 6.3), Recover kind 1 (1 MP = 1 s), the halving.
    let mut s = default99();
    s.force_mp_zero();
    s.apply_prize(PrizeKind::MpSmall);
    s.apply_prize(PrizeKind::MpLarge);
    assert_eq!((s.mp_charge_timer, s.mp), (42.0, 0));
    s.recover(0, 12, 0, 1);
    assert_eq!(s.mp_charge_timer, 30.0);
    s.recover(0, 12, 0, 0);
    assert_eq!((s.mp_charge_timer, s.mp), (30.0, 0), "any other kind is lost while charging");
    s.halve_mp_charge();
    assert_eq!(s.mp_charge_timer, 15.0);
    assert!((s.mp_charge_gauge() - 15.0 / 50.0 * 169.0).abs() < 1e-4);
    // A prize that takes the timer below 0 does not clamp: the next tick ends the charge.
    s.mp_charge_timer = 1.0;
    s.apply_prize(PrizeKind::MpLarge);
    assert!(s.mp_charging && s.mp_charge_timer < 0.0);
    s.tick(DT, 0.0);
    assert_eq!((s.mp_charging, s.mp), (false, 169));
}

#[test]
fn wizards_ruse_returns_spent_mp_as_hp_at_n_plus_1_in_8() {
    // Magic spec C 2 / row WIZZARD_STAR: probability (count + 1) / 8. The port's own random stream.
    let mut s = default99();
    s.set_ability(A::WizzardStar, 3, 3);
    let mut heals = 0;
    for _ in 0..4_000 {
        s.hp = 100;
        s.mp = 50;
        let used = s.use_mp(10);
        if used.healed > 0 {
            assert_eq!((used.healed, s.hp), (10, 110));
            heals += 1;
        }
    }
    assert!((1_850..=2_150).contains(&heals), "4 in 8 expected, got {heals} of 4000");
    // Deterministic: the same status gives the same rolls.
    let (mut a, mut b) = (s.clone(), s.clone());
    for _ in 0..50 {
        a.mp = 50;
        b.mp = 50;
        assert_eq!(a.use_mp(10), b.use_mp(10));
    }
}

// ------------------------------------------------------------------------------------ 6. focus, items, prizes

#[test]
fn focus_gauge_aim_time_and_the_empty_flag() {
    // Magic spec part D S1: 0..100; aiming burns 20 points per real second (0.05 s per point).
    let mut s = default99();
    assert_eq!((s.focus, s.focus_max), (100, 100));
    let mut aim = s.focus_aim_begin();
    assert_eq!((aim.focus_at_start, aim.time, aim.focus()), (100, 5.0, 100), "releasing at once costs nothing");
    for _ in 0..60 {
        aim.tick(DT, 1.0);
    }
    assert!((79..=80).contains(&aim.focus()), "1 s of aiming burns 20 points: {}", aim.focus());
    for _ in 0..300 {
        aim.tick(DT, 1.0);
    }
    assert!(aim.exhausted() && aim.focus() == 0);
    s.focus_aim_end(&aim);
    assert_eq!((s.focus, s.focus_empty), (0, true), "reaching 0 from >= 1 sets the focus-empty flag");
    // Refill from prizes (+3 / +5) and AddFocus; the flag clears when the gauge is full.
    s.apply_prize(PrizeKind::FocusSmall);
    s.apply_prize(PrizeKind::FocusLarge);
    s.add_focus(7, false);
    s.add_focus(50, true);
    assert_eq!((s.focus, s.focus_empty), (15, true), "AddFocus is ignored while aiming");
    s.set_focus(500);
    assert_eq!((s.focus, s.focus_empty), (100, false));
    // Time dilation: the aim time is real time.
    let mut slow = s.focus_aim_begin();
    slow.tick(0.5, 0.5);
    assert_eq!(slow.time, 4.0);
}

#[test]
fn item_amounts_with_item_boost() {
    // Magic spec part E I5 table; sheet sora-magic-items rows item.*; row ITEM_UP (x(1 + 0.5 n)).
    let mut s = level99("wisdom", "STANDARD");
    let set = |s: &mut Status, hp: i32, mp: i32, focus: i32| { s.hp = hp; s.mp = mp; s.focus = focus; };
    set(&mut s, 50, 10, 0);
    assert_eq!((s.item_effect(ItemKind::Potion).hp, s.item_effect(ItemKind::HiPotion).hp), (80, 120), "Hi-Potion 160, clamped at max HP 250");
    assert_eq!((s.item_effect(ItemKind::Ether).mp, s.mp), (100, 110));
    assert_eq!((s.item_effect(ItemKind::HiEther).mp, s.mp), (69, 179), "Hi-Ether: + max MP");
    assert_eq!((s.item_effect(ItemKind::Refocuser).focus, s.item_effect(ItemKind::HiRefocuser).focus), (40, 60));
    set(&mut s, 1, 0, 0);
    let elixir = s.item_effect(ItemKind::Elixir);
    assert_eq!((elixir.hp, elixir.mp, elixir.focus), (249, 179, 100));
    // Item Boost, one and two copies.
    s.set_ability(A::ItemUp, 1, 1);
    assert_eq!(s.item_multiplier(), 1.5);
    set(&mut s, 1, 0, 0);
    assert_eq!((s.item_effect(ItemKind::Potion).hp, s.item_effect(ItemKind::Ether).mp, s.item_effect(ItemKind::Refocuser).focus), (120, 150, 60));
    s.set_ability(A::ItemUp, 2, 2);
    set(&mut s, 1, 0, 0);
    assert_eq!((s.item_effect(ItemKind::Potion).hp, s.item_effect(ItemKind::Refocuser).focus), (160, 80));
    // In the MP charge: Ether takes T * 0.5 * mult off the timer, Hi-Ether T * mult (ends it).
    s.set_ability(A::ItemUp, 0, 0);
    s.force_mp_zero();
    let ether = s.item_effect(ItemKind::Ether);
    assert_eq!((ether.charge_seconds, ether.charge_ended, s.mp_charge_timer, s.mp), (25.0, false, 25.0, 0));
    let hi = s.item_effect(ItemKind::HiEther);
    assert_eq!((hi.charge_ended, s.mp_charging, s.mp), (true, false, 179));
    s.force_mp_zero();
    assert_eq!((s.item_effect(ItemKind::Megalixir).charge_ended, s.mp), (true, 179));
    // Panacea: 60 s of bad-status immunity; the guard abilities refuse their own status (rows BURN_GUARD .. STUN_GUARD).
    assert!(!s.resists_bad_status(17));
    s.set_ability(A::BurnGuard, 1, 1);
    s.set_ability(A::StunGuard, 1, 1);
    assert_eq!([17u8, 15, 13, 7, 16, 12, 8].map(|kind| s.resists_bad_status(kind)), [true, false, false, false, false, true, false]);
    for (kind, bad) in [(A::CloudGuard, 15u8), (A::SneezeGuard, 13), (A::FreezeGuard, 7), (A::DischargeGuard, 16)] {
        s.set_ability(kind, 1, 1);
        assert!(s.resists_bad_status(bad));
    }
    s.item_effect(ItemKind::Panacea);
    assert!(s.resists_bad_status(8) && s.status_immunity == 60.0);
    // The pouch: the sheet's loadout holds 2 Potions, 2 Hi-Potions, 2 Ethers.
    let mut p = default99();
    assert_eq!((p.item_count(ItemKind::Potion), p.item_count(ItemKind::HiPotion), p.item_count(ItemKind::Ether), p.item_count(ItemKind::Elixir)), (2, 2, 2, 0));
    p.hp = 10;
    assert_eq!(p.use_item(ItemKind::Potion).unwrap().hp, 80);
    assert!(p.use_item(ItemKind::Potion).is_ok() && p.use_item(ItemKind::Potion).is_err() && p.use_item(ItemKind::Elixir).is_err());
    assert_eq!(ItemKind::from_name("item_highpotion"), Some(ItemKind::HiPotion));
    assert_eq!(ItemKind::from_name("0x104"), Some(ItemKind::Ether));
    // Nothing is applied to a dead Sora.
    p.hp = 0;
    assert_eq!(p.item_effect(ItemKind::HiPotion).hp, 0);
}

#[test]
fn prizes_follow_the_sheets_values_and_the_prize_abilities() {
    // Sheet sora-stats-rules row prize.values (the row is informational in the generator: this
    // test pins the constants of Status::apply_prize against it).
    let rules = sheet("sora-stats-rules");
    let values = rules["rows"].as_array().unwrap().iter().find(|r| r[0] == "prize.values").unwrap()[1].clone();
    let n = |key: &str| values[key].as_i64().unwrap() as i32;
    let mut s = default99();
    s.hp = 100;
    s.mp = 100;
    s.focus = 50;
    s.apply_prize(PrizeKind::HpSmall);
    assert_eq!(s.hp - 100, n("hp_small"));
    s.apply_prize(PrizeKind::HpLarge);
    assert_eq!(s.hp - 100, n("hp_small") + n("hp_large"));
    s.apply_prize(PrizeKind::MpSmall);
    s.apply_prize(PrizeKind::MpLarge);
    assert_eq!(s.mp - 100, n("mp_small") + n("mp_large"));
    s.apply_prize(PrizeKind::FocusSmall);
    s.apply_prize(PrizeKind::FocusLarge);
    assert_eq!(s.focus - 50, n("focus_small") + n("focus_large"));
    let munny: Vec<i32> = values["munny"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i32).collect();
    assert_eq!([s.apply_prize(PrizeKind::Munny1), s.apply_prize(PrizeKind::Munny5), s.apply_prize(PrizeKind::Munny10)].to_vec(), munny);
    assert_eq!(s.munny, 16);
    s.force_mp_zero();
    s.apply_prize(PrizeKind::MpSmall);
    assert_eq!(50.0 - s.mp_charge_timer, n("mp_small_charge_s") as f32);
    s.apply_prize(PrizeKind::MpLarge);
    assert_eq!(48.0 - s.mp_charge_timer, n("mp_large_charge_s") as f32);
    // Treasure Magnet (row PRIZE_DRAW): 300 / 600 / 900 cm for 0 / 1 / 2+ copies; Master (row MASTER_DRAW): unlimited.
    let mut m = default99();
    assert_eq!(passive::prize_draw_radius(&m), Some(900.0), "one equipped + one from the Kingdom Key");
    m.unequip(A::PrizeDraw);
    assert_eq!(passive::prize_draw_radius(&m), Some(600.0));
    m.weapon.clear();
    m.rebuild();
    assert_eq!(passive::prize_draw_radius(&m), Some(300.0));
    m.set_ability(A::MasterDraw, 1, 1);
    assert_eq!(passive::prize_draw_radius(&m), None);
    // Converters (rows HP_CONVERTER, MP_CONVERTER, MUNNY_CONVERTER, FP_CONVERTER) and Rags to Riches (row MILLIONAIRE).
    use PrizeKind::*;
    let mut c = default99();
    assert_eq!(PrizeKind::ALL.map(|k| passive::convert_prize(&c, k)), PrizeKind::ALL);
    for kind in [A::HpConverter, A::MpConverter, A::MunnyConverter, A::FpConverter] {
        c.set_ability(kind, 1, 1);
    }
    assert_eq!(PrizeKind::ALL.map(|k| passive::convert_prize(&c, k)), [HpLarge, HpLarge, MpLarge, MpLarge, Munny5, Munny10, Munny10, FocusLarge, FocusLarge]);
    let mut r = default99();
    r.meal = Some(Meal { ability: Some(A::Millionaire), ..Meal::default() });
    r.rebuild();
    assert_eq!(PrizeKind::ALL.map(|k| passive::convert_prize(&r, k)), [Munny1, Munny5, Munny1, Munny5, Munny5, Munny10, Munny10, Munny1, FocusLarge]);
}

// ------------------------------------------------------------------------------------ 7. abilities

#[test]
fn the_default_loadout_uses_exactly_180_ap() {
    // Stats spec section 9; sheet sora-loadout-99 rows abilities.equipped_ap, abilities.ap_all.
    let s = default99();
    assert_eq!((s.ap_in_use(), s.max_ap(), s.stats.ap, s.stats.ap_max), (180, 180, 180, 180));
    assert!(s.ap_within_budget());
    // 73 abilities owned in 88 copies; equipping all would cost 250 AP (spec 5.3). (X1) Without the two the port
    // excludes (ATTRACTION_TIME x3, ATTRACTION_UP x2, 3 AP each; decision dec029): 71 in 83 copies, 235 AP. (XSI, 2026-10-16)
    // Path A's two XS level abilities are owned by level 99 (granted by the level table: Spiral Rush 3 AP at 12,
    // Diving Strike 2 AP at 22; XS 4.1): 73 in 85 copies, 240 AP.
    assert_eq!((s.abilities.iter_owned().count(), s.abilities.iter_owned().map(|(_, n)| n as u32).sum::<u32>(), s.abilities.ap_all_owned()), (73, 85, 240));
    // Owned-only copies do nothing (spec 5.1).
    assert!(s.abilities.owned(A::Expzero) == 1 && !s.has(A::Expzero) && s.abilities.owned(A::FocusAspir) == 3 && s.count(A::FocusAspir) == 1);
    // No AP left: a further copy cannot be equipped; after unequipping one that costs as much it can.
    let mut e = s.clone();
    assert_eq!(e.equip(A::FocusAspir), Err("not enough AP"));
    assert_eq!(e.equip(A::Libra), Err("no unequipped copy is owned"));
    assert!(e.unequip(A::Dodge) && e.equip(A::FocusAspir).is_ok());
    assert_eq!((e.count(A::FocusAspir), e.ap_in_use(), passive::focus_syphon_rate(&e)), (2, 180, Some(1.9)));
    // Zero-cost abilities fit a full budget (MP Safety costs 0 AP).
    assert!(e.equip(A::MpSafety).is_ok());
    // Critical: 210 AP, 183 in use with the equipped start ability Critical Recharge (DF, decision dec032: KH3's Critical
    // Counter is not part of the port's start).
    let c = level99("balance", "CRITICAL");
    assert_eq!((c.ap_in_use(), c.max_ap()), (183, 210));
    // The player gates (spec 5.2): without the script flag the ability does not count.
    let mut g = s.clone();
    assert_eq!((g.count(A::Airslide), g.count(A::Superslide), g.has(A::Doubleflight), g.has(A::Airdodge)), (3, 3, true, true));
    g.action_flags = 0xFF & !(1 << 1) & !(1 << 7);
    assert_eq!((g.count(A::Airslide), g.count(A::Superslide), g.has(A::Doubleflight), g.has(A::Airdodge), g.has(A::Superjump)), (0, 0, true, false, false));
    g.action_flags = 0xFF & !1 & !(1 << 4);
    assert_eq!((g.has(A::Doubleflight), g.has(A::FocusAspir), g.count(A::Airslide)), (false, false, 3));
    // A meal's bonus ability counts once without being owned (0x140580810).
    let mut meal = s.clone();
    meal.meal = Some(Meal { ability: Some(A::MpHaste), ..Meal::default() });
    meal.rebuild();
    assert_eq!((meal.count(A::MpHaste), meal.ap_in_use()), (2, 180));
    // A victory bonus awarded during play: the totals rise and the current HP / MP with them (row bonus.apply_now).
    let mut b = new_game("balance", "STANDARD");
    b.hp = 50;
    b.award_bonus(kh_core::status::BonusCounts::HP_UP10);
    b.award_bonus(kh_core::status::BonusCounts::MP_UP5);
    assert_eq!((b.hp, b.hp_max, b.mp, b.mp_max), (60, 115, 115, 115));
    b.grant_ability(A::Highjump, true);
    assert!(b.has(A::Highjump) && passive::has_high_jump(&b));
}

#[test]
fn the_ability_catalogue_is_the_specs() {
    // Spec 5.3 / 5.4: 183 enum abilities; Sora's are ids 1..112, 170..186, 190..198 = 138, of
    // which 70 are fully read, 58 partial, 7 hook only, 3 without a reference ((CC) CRITICAL_CONVERTER now full).
    // (XSI) + the 14 port abilities of XS_SORA_ABILITIES.md (values 256 .. 269, column port = ours).
    assert_eq!(tables::ABILITIES.len(), 183 + 14);
    let sora: Vec<_> = tables::ABILITIES.iter().filter(|row| matches!(row.kind.value(), 1..=112 | 170..=186 | 190..=198)).collect();
    let count = |status: &str| sora.iter().filter(|row| row.status == status).count();
    assert_eq!((sora.len(), count("full"), count("partial"), count("hook-only"), count("none")), (138, 70, 58, 7, 3));
    // AP and ids of some rows (TresAbilityData).
    let row = |kind: A| tables::ability_row(kind).unwrap();
    assert_eq!((row(A::ComboUp).ap, row(A::LastLeave).ap, row(A::MpSafety).ap, row(A::FinalHeaven).ap), (Some(5), Some(4), Some(0), Some(7)));
    assert_eq!((row(A::Superslide).id, row(A::Superslide).copies, row(A::Airslide).copies), ("SUPERSLIDE", 3, 3));
    assert_eq!(tables::ABILITIES[tables::ABILITY_COMBO_PLUS].kind, A::ComboPlus);
}

#[test]
fn passive_numbers_handed_to_other_systems() {
    let mut s = default99();
    // Rows COMBO_PLUS / AIRCOMBO_PLUS: max combo = 4 + copies (two each at level 99).
    assert_eq!((s.max_combo(false), s.max_combo(true)), (6, 6));
    // Rows SUPERSLIDE, AIRSLIDE: by equipped count (three each).
    assert_eq!((passive::superslide_time(&s), passive::airslide_speed_scale(&s)), (0.5, 1.425));
    s.abilities.set(A::Superslide, 3, 2);
    s.abilities.set(A::Airslide, 3, 1);
    assert_eq!((passive::superslide_time(&s), passive::airslide_speed_scale(&s)), (0.35, 0.725));
    // Row FOCUS_ASPIR: absorb rate 1.4 / 1.9 / 2.4.
    assert_eq!(passive::focus_syphon_rate(&s), Some(1.4));
    // Rows FORM_TIME, MAGIC_TIME, ATTRACTION_TIME: timer x(1 + 0.2 n), cap 99.9 s. None of the three is equipped in the sheet's loadout.
    assert_eq!(passive::command_timer(&s, CommandTimer::GrandMagic, 20.0), 20.0);
    s.unequip(A::CounterUp);
    s.unequip(A::ComboMaster);
    s.unequip(A::CriticalHalf);
    s.unequip(A::CriticalHalf);
    for _ in 0..3 {
        s.equip(A::MagicTime).unwrap();
    }
    s.equip(A::FormTime).unwrap();
    assert_eq!((passive::command_timer(&s, CommandTimer::GrandMagic, 20.0), passive::command_timer(&s, CommandTimer::Formchange, 20.0), passive::command_timer(&s, CommandTimer::Attraction, 20.0)), (32.0, 24.0, 20.0));
    assert_eq!(passive::command_timer(&s, CommandTimer::GrandMagic, 90.0), 99.9);
    // Row LINK_BOOST: the link timer runs at 0.8x speed.
    assert_eq!(passive::link_timer_speed(&s), 1.0);
    s.set_ability(A::LinkBoost, 1, 1);
    assert_eq!(passive::link_timer_speed(&s), 0.8);
    // Row ATTRACTION_UP: chance 0.2 + 0.05 n; x0.5 on Critical; 0 with Critical Converter. (X1) The port excludes
    // ATTRACTION_UP and ATTRACTION_TIME (decision dec029): they cannot be held, so n stays 0 and the timer factor 1.
    let mut a = default99();
    assert_eq!(passive::attraction_marker_chance(&a, false), 0.2);
    a.set_ability(A::AttractionUp, 2, 2);
    a.set_ability(A::AttractionTime, 3, 3);
    assert_eq!((a.abilities.owned(A::AttractionUp), a.count(A::AttractionUp), a.abilities.owned(A::AttractionTime)), (0, 0, 0));
    assert_eq!((passive::attraction_marker_chance(&a, false), passive::command_timer(&a, CommandTimer::Attraction, 20.0)), (0.2, 20.0));
    let mut c = level99("balance", "CRITICAL");
    assert!((passive::attraction_marker_chance(&c, false) - 0.1).abs() < 1e-6 && (passive::attraction_marker_chance(&c, true) - 0.2).abs() < 1e-6);
    c.abilities.set(A::CriticalConverter, 1, 1);
    assert_eq!(passive::attraction_marker_chance(&c, false), 0.0);
    // Row POWER_CURE: 15 + ceil((15 n + 20) * MP / maxMP) gauge points at a Cure.
    assert_eq!(passive::cure_converter_points(&a), None);
    a.set_ability(A::PowerCure, 2, 2);
    assert_eq!(passive::cure_converter_points(&a), Some(15 + 50));
    a.mp = 85;
    assert_eq!(passive::cure_converter_points(&a), Some(15 + 26), "ceil(50 * 85 / 169)");
    // Row GUARD_REGENE: a successful block gives MP +3 (equipped in the sheet's loadout).
    assert_eq!(passive::block_replenisher_mp(&default99()), 3);
    // Rows FORM_UP, CRITICAL_CHARGE: the situation-gauge terms.
    assert_eq!(passive::situation_gauge_terms(&default99()), (0.0, 1.0));
    c.force_mp_zero();
    assert_eq!(passive::situation_gauge_terms(&c), (0.0, 1.3));
}

// ------------------------------------------------------------------------------------ 8. the HUD model

#[test]
fn gauge_frames_of_the_ui_specs_test_list() {
    // UI spec 11b test 1 (level 99, max HP 250 / max MP 179 = Wisdom on Standard).
    let world = TestWorld::floor();
    let mut sora = spawn(level99("wisdom", "STANDARD"));
    sora.step(DT, &Input::default(), &world);
    let g = sora.ui_snapshot().gauges;
    assert_eq!((g.hp_max, g.mp_max, g.focus_max, g.level, g.exp, g.exp_to_next), (250, 179, 100, 99, 1_527_640, 0));
    assert_eq!((g.hp_base_frame, g.hp_frame, g.hp_red_frame, g.mp_base_frame, g.mp_frame, g.focus_frame, g.focus_is_max, g.pinch, g.mp_charging), (501, 501, 501.0, 323, 323, 101, true, false, false));
    assert_eq!((g.strength, g.magic, g.defense, g.ap, g.ap_max), (66, 67, 59, 180, 180));
    let frames = |sora: &mut Sora, hp: i32, focus: i32| {
        sora.status_mut().hp = hp;
        sora.status_mut().focus = focus;
        sora.step(DT, &Input::default(), &world);
        let g = sora.ui_snapshot().gauges;
        (g.hp_frame, g.focus_frame, g.focus_is_max, g.pinch)
    };
    assert_eq!(frames(&mut sora, 125, 60), (251, 61, false, false));
    assert_eq!(frames(&mut sora, 1, 100), (3, 101, true, true), "never frame 1 while alive");
    assert_eq!(frames(&mut sora, 0, 0), (1, 1, false, true));
    assert_eq!(frames(&mut sora, 249, 99), (499, 100, false, false), "never full unless full");
    // Test 3: pinch at HP <= 62 (frame 125), off at 63.
    assert_eq!(frames(&mut sora, 62, 100), (125, 101, true, true));
    assert_eq!(frames(&mut sora, 63, 100).3, false);
    // Max HP 280 -> base 561; max HP 139 -> base 279; max HP 400 -> the base stays 669.
    let base = |status: Status| { let mut s = spawn(status); s.step(DT, &Input::default(), &world); s.ui_snapshot().gauges };
    assert_eq!(base(level99("vitality", "STANDARD")).hp_base_frame, 561);
    assert_eq!(base(level99("vitality", "CRITICAL")).hp_base_frame, 279);
    let mut big = level99("wisdom", "STANDARD");
    big.meal = Some(Meal { hp: 150, ..Meal::default() });
    big.rebuild();
    big.full_recover();
    let g = base(big);
    assert_eq!((g.hp_max, g.hp_base_frame, g.hp_frame), (400, 669, 669));
    // Starting values (UI spec section 4): max HP 90 -> base frame 181, max MP 100 -> 181.
    let g = base(new_game("wisdom", "STANDARD"));
    assert_eq!((g.hp_base_frame, g.mp_base_frame, g.mp_max), (181, 217, 120));
    assert_eq!((gauge_frame(90, 90, 180) + 1, gauge_frame(100, 100, 180) + 1), (181, 181));
}

#[test]
fn the_red_gauge_waits_08_s_then_follows_in_10_ticks() {
    // UI spec 11b test 2: hit 250 -> 180 at t = 0: the fill jumps to 361 at once, red stays at
    // 501 until 0.8 s, then moves 14 frames per 33.3 ms tick and stops at 361 after 10 ticks
    // (t = 1.13 s). Heal: red jumps with the fill. A second hit during the follow: red
    // continues from where it is.
    let world = TestWorld::floor();
    let mut sora = spawn(level99("wisdom", "STANDARD"));
    sora.step(DT, &Input::default(), &world);
    let hit = |sora: &mut Sora, amount: f32| sora.status_request(StatusRequest::Damage { amount, hit: HitOnSora::default() });
    let step = |sora: &mut Sora| { sora.step(DT, &Input::default(), &world); let g = sora.ui_snapshot().gauges; (g.hp_frame, g.hp_red_frame) };
    hit(&mut sora, 70.0);
    assert_eq!(step(&mut sora), (361, 501.0), "t = 0: the fill at once, red where it was");
    let mut reds = vec![501.0];
    for _ in 0..80 {
        let (fill, red) = step(&mut sora);
        assert_eq!(fill, 361);
        reds.push(red);
    }
    // reds[k] = the red frame k steps (k / 60 s) after the hit.
    assert!(reds[..=49].iter().all(|&r| r == 501.0), "red stays until 0.8 s (48 steps) and one tick more");
    let moves: Vec<(usize, f32)> = (1..reds.len()).filter(|&k| reds[k] != reds[k - 1]).map(|k| (k, reds[k - 1] - reds[k])).collect();
    assert_eq!(moves.len(), 10, "{moves:?}");
    assert!(moves.iter().all(|&(_, by)| (by - 14.0).abs() < 1e-3), "14 frames per tick: {moves:?}");
    assert_eq!(moves.iter().map(|m| m.0).collect::<Vec<_>>(), (0..10).map(|i| 50 + 2 * i).collect::<Vec<_>>(), "one tick every 33.3 ms");
    assert_eq!((reds[68], reds[80]), (361.0, 361.0), "stops at the fill after 10 ticks: 68 steps = 1.13 s");
    // A second hit during the follow.
    hit(&mut sora, 30.0); // 180 -> 150
    assert_eq!(step(&mut sora), (301, 361.0));
    for _ in 0..53 {
        step(&mut sora);
    }
    let (_, moving) = step(&mut sora); // 54 steps after the wait began: 3 ticks of 6 done
    assert!((moving - (361.0 - 3.0 * 6.0)).abs() < 1e-3, "{moving}");
    hit(&mut sora, 50.0); // 150 -> 100: frame 201
    let (fill, red) = step(&mut sora);
    assert_eq!(fill, 201);
    assert!((red - moving).abs() < 1e-3, "the red gauge continues from where it is: {red}");
    let mut last = red;
    for _ in 0..40 {
        let (_, r) = step(&mut sora);
        assert!(r <= last);
        last = r;
    }
    assert_eq!(last, 201.0);
    // A heal: red jumps with the fill.
    sora.status_request(StatusRequest::Recover { hp: 100, mp: 0, focus: 0, kind: 0 });
    assert_eq!(step(&mut sora), (401, 401.0));
}

#[test]
fn the_mp_charge_shows_on_the_hud_model() {
    // UI spec section 4 "MP Charge" / 11b test 4: the pink gauge shows F(trunc(charge value), maxMP, mpMaxFrames) + 1.
    let world = TestWorld::floor();
    let mut sora = spawn(level99("wisdom", "STANDARD"));
    sora.status_request(StatusRequest::UseMp { cost: MpCost::All, combo: 0 });
    sora.step(DT, &Input::default(), &world);
    let g = sora.ui_snapshot().gauges;
    assert!(g.mp_charging && g.mp == 0 && g.mp_frame == 1);
    assert!(g.mp_charge_value < 179.0 && g.mp_charge_value > 178.9, "one step of the charge has run: {}", g.mp_charge_value);
    assert_eq!(g.mp_charge_frame, gauge_frame(178, 179, 322) + 1);
    let mut previous = g.mp_charge_frame;
    for _ in 0..(46 * 60) {
        sora.step(DT, &Input::default(), &world);
        let g = sora.ui_snapshot().gauges;
        assert!(g.mp_charge_frame <= previous);
        previous = g.mp_charge_frame;
    }
    let g = sora.ui_snapshot().gauges;
    assert!(!g.mp_charging && g.mp == 179 && g.mp_frame == 323 && g.mp_charge_value == 0.0, "50 s / 1.1 = 45.5 s later the charge is over");
}

#[test]
fn munny_accumulates_and_items_queue() {
    // UI spec section 8 "Pickups" / 11b test 7: munny 10 then 25 within 2.4 s -> the counter
    // shows 10 then 35; three items at once -> three popups in sequence, 1.9 s each.
    let world = TestWorld::floor();
    let mut sora = spawn(default99());
    sora.status_request(StatusRequest::Prize(PrizeKind::Munny10));
    sora.step(DT, &Input::default(), &world);
    assert_eq!(sora.ui_snapshot().board.munny.unwrap().popup.value, 10.0);
    for _ in 0..60 {
        sora.step(DT, &Input::default(), &world);
    }
    for kind in [PrizeKind::Munny10, PrizeKind::Munny10, PrizeKind::Munny5] {
        sora.status_request(StatusRequest::Prize(kind));
    }
    let out = sora.step(DT, &Input::default(), &world);
    assert_eq!(popups(&out.events).len(), 3);
    assert_eq!((sora.ui_snapshot().board.munny.unwrap().popup.value, sora.status().munny), (35.0, 35));
    // After it closed (146 frames) the counter restarts.
    for _ in 0..150 {
        sora.step(DT, &Input::default(), &world);
    }
    assert!(sora.ui_snapshot().board.munny.is_none());
    sora.status_request(StatusRequest::Prize(PrizeKind::Munny1));
    sora.step(DT, &Input::default(), &world);
    assert_eq!(sora.ui_snapshot().board.munny.unwrap().popup.value, 1.0);
    // Items.
    for item in [ItemKind::Potion, ItemKind::Ether, ItemKind::Elixir] {
        sora.status_request(StatusRequest::ObtainItem(item));
    }
    sora.step(DT, &Input::default(), &world);
    let mut shown = Vec::new();
    for _ in 0..(3 * 114 + 10) {
        if let Some(item) = sora.ui_snapshot().board.item {
            if shown.last() != Some(&item.popup.text) {
                shown.push(item.popup.text);
            }
        }
        sora.step(DT, &Input::default(), &world);
    }
    assert_eq!(shown, ["ITEM_POTION", "ITEM_ETHER", "ITEM_ELIXIR"]);
    assert!(sora.ui_snapshot().board.item.is_none() && sora.ui_snapshot().board.is_empty());
    assert_eq!((sora.status().item_count(ItemKind::Potion), sora.status().item_count(ItemKind::Elixir)), (3, 1));
}

#[test]
fn the_status_is_deterministic_and_does_not_touch_the_movement() {
    // The same inputs give the same outputs with and without a status (the golden test pins the
    // placeholder case); the status itself is part of the save state.
    let world = TestWorld::floor();
    let (mut bare, mut full) = (spawn(Status::default()), spawn(default99()));
    // (Since the action states exist the MOVEMENT abilities do change the movement: High Jump
    // makes every ground jump type 1, Glide takes over a held fall, Doubleflight a re-press:
    // actions spec II.3. They are unequipped here; everything else of the status must not matter.)
    for kind in [kh_core::enums::ETresAbilityKind::Highjump, kh_core::enums::ETresAbilityKind::Glide, kh_core::enums::ETresAbilityKind::Doubleflight] {
        full.status_mut().abilities.set(kind, 1, 0);
    }
    full.status_request(StatusRequest::UseMp { cost: MpCost::All, combo: 0 });
    for i in 0..600 {
        let input = Input { move_stick: kh_core::Vec2::new(0.2, 1.0), jump_pressed: i % 97 == 5, jump_held: i % 97 < 20, ..Input::default() };
        let (a, b) = (bare.step(DT, &input, &world), full.step(DT, &input, &world));
        assert_eq!((a.position, a.velocity, a.state, a.sub_step, a.anim), (b.position, b.velocity, b.state, b.sub_step, b.anim), "step {i}");
    }
    assert!(full.status().mp_charging && bare.status().placeholder);
    let mut copy = full.clone();
    for _ in 0..3_000 {
        full.step(DT, &Input::default(), &world);
        copy.step(DT, &Input::default(), &world);
    }
    assert_eq!(full.status(), copy.status());
    assert_eq!(full.ui_snapshot(), copy.ui_snapshot());
    assert!(!full.status().mp_charging);
}

/// (IMPL_ER48, host bookkeeping) Every landing of an item's effect is counted by kind (for the Elden Ring host's
/// status clearing); nothing is counted on a dead Sora.
#[test]
fn item_effects_are_counted_by_kind() {
    let mut s = default99();
    s.item_effect(ItemKind::Elixir);
    s.item_effect(ItemKind::Elixir);
    s.item_effect(ItemKind::Panacea);
    let at = |k: ItemKind| ItemKind::ALL.iter().position(|i| *i == k).unwrap();
    assert_eq!((s.item_effects[at(ItemKind::Elixir)], s.item_effects[at(ItemKind::Panacea)], s.item_effects[at(ItemKind::Tent)]), (2, 1, 0));
    s.hp = 0;
    s.item_effect(ItemKind::Elixir);
    assert_eq!(s.item_effects[at(ItemKind::Elixir)], 2);
}

/// (party P2) Spec 4.2 step 2: `AdjustExp` asks "player 0". A host whose controlled pawn is a friend passes him
/// (`adjust_exp_for` / `StatusRequest::GiveExpAs`): Sora's own Zero EXP or a KO'd AI Sora no longer gate the award, the
/// leader's do; the total grows by the leader-gated gain; `panel: false` hides only Sora's level-up panel.
#[test]
fn a_friend_leader_is_player_0_for_the_exp() {
    use kh_core::status::ExpPlayer;
    let leader = ExpPlayer { alive: true, exp_zero: 0, exp_bargain: 0, level: 50 };
    let mut s = new_game("balance", "STANDARD");
    s.set_ability(A::Expzero, 1, 1);
    s.hp = 0;
    assert_eq!(s.adjust_exp_for(500, leader), 500, "Sora's Zero EXP and his KO do not count while a friend leads");
    assert_eq!(s.adjust_exp_for(500, ExpPlayer { alive: false, ..leader }), 0, "a fallen leader gets nothing");
    assert_eq!(s.adjust_exp_for(500, ExpPlayer { exp_zero: 1, ..leader }), 0);
    let critical = new_game("balance", "CRITICAL");
    assert_eq!(critical.adjust_exp_for(500, leader), 375, "the difficulty is game-wide");
    let mut s = new_game("balance", "STANDARD");
    let a = s.give_exp_gain(500, s.adjust_exp_for(500, leader));
    assert_eq!((a.added, s.exp), (500, 500));
    // through the request: no panel for a hidden Sora, the other popups as GiveExp
    let mut sora = spawn(new_game("balance", "STANDARD"));
    let mut host = RecordingHost::new();
    let world = TestWorld::floor();
    sora.status_request(StatusRequest::GiveExpAs { base_exp: 999, rate: 1.0, position: None, player: leader, panel: false });
    let raised = popups(&host.step(&world, |h| sora.step_hosted(DT, &Input::default(), h)).events);
    assert!(sora.status().level > 1, "the level rose");
    assert!(!raised.iter().any(|p| p.kind == UiPopup::KIND_LEVEL_UP), "no level-up panel: {raised:?}");
    assert!(raised.iter().any(|p| p.kind == UiPopup::KIND_NEXT_LV));
}
