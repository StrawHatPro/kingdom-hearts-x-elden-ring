//! (DF, 2026-10-15; the user: "the overall difficulty of the mod should be like Kingdom Hearts critical mode")
//! The difficulty through the shared host code: the new game's Critical start, the save's difficulty field (an old file
//! without it plays at its base sheet's Standard), the one-time switch of an existing character both ways, and the boss
//! rewards that Critical gives at the start (build/sora_tmp/er/IMPL_DF_STATE.md).

use kh_core::enums::ETresAbilityKind as A;
use kh_core::status::{GameLevel, Loadout, OpeningChoice};
use kh_core::{Pose, Sora, Status, Vec3, SORA_PARAMS};
use kh_host::bossrewards::{claim, Claim, Table};
use kh_host::hud::Inventory;
use kh_host::save::{self, SoraSave};

fn base() -> Loadout {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-loadout-99.json");
    kh_host::rig::loadout::parse(path, &std::fs::read_to_string(path).expect("the loadout sheet")).expect("parse").0
}

fn sora_of(status: Status) -> Sora {
    Sora::new(SORA_PARAMS, Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(status)
}

fn new_game(level: GameLevel) -> Status {
    Status::from_loadout(&save::new_game_loadout_for(&save::with_difficulty(&base(), level), OpeningChoice::DEFAULT))
}

#[test]
fn the_base_sheet_is_standard_and_a_critical_new_game_has_the_critical_start() {
    assert_eq!(GameLevel::from_name(&base().game_level), Some(GameLevel::Standard));
    let std = new_game(GameLevel::Standard);
    let crit = new_game(GameLevel::Critical);
    assert_eq!((std.max_hp(), std.max_mp(), std.max_ap(), std.ap_in_use()), (105, 110, 28, 28));
    assert_eq!((crit.max_hp(), crit.max_mp(), crit.max_ap(), crit.ap_in_use()), (52, 55, 58, 54));
    assert_eq!(crit.game_level, GameLevel::Critical);
    // (dec032) the user: "don't start with critical counter"
    assert_eq!((crit.abilities.owned(A::CriticalCounter), crit.abilities.equipped(A::CriticalCounter)), (0, 0));
    for k in [A::CriticalCharge, A::AirRecovery, A::GuardCounter, A::SlashUpper, A::ComboMaster, A::AirRollBeat, A::Riskdodge] {
        assert_eq!(crit.abilities.equipped(k), 1, "{k:?}");
    }
    for k in [A::CriticalConverter, A::Revengeimpact, A::Revengedive, A::RevengeEx] {
        assert_eq!((crit.abilities.owned(k), crit.abilities.equipped(k)), (1, 0), "{k:?}");
    }
    assert_eq!((crit.abilities.owned(A::Airslide), crit.abilities.owned(A::Superslide)), (3, 3));
    // Proud: KH3's own Standard start (only enemy damage x1.5 differs).
    let proud = new_game(GameLevel::Proud);
    assert_eq!((proud.max_hp(), proud.max_ap(), proud.abilities.owned(A::GuardCounter)), (105, 28, 0));
}

#[test]
fn the_save_keeps_the_difficulty_and_an_old_file_reads_as_its_base() {
    let base = base();
    let sora = sora_of(new_game(GameLevel::Critical));
    let saved = SoraSave::capture(&sora, &Inventory::default(), "Tarnished", 10, 20);
    assert_eq!(saved.difficulty, "CRITICAL");
    let text = saved.to_json();
    assert!(text.contains("\"difficulty\": \"CRITICAL\""));
    let back = SoraSave::from_json(&text).unwrap();
    assert_eq!(back, saved);
    // The status: over the STANDARD base sheet, the file's own Critical decides.
    let status = back.status(&base);
    assert_eq!((status.game_level, status.max_hp(), status.max_ap(), status.ap_in_use()), (GameLevel::Critical, 52, 58, 54));
    assert_eq!(status.abilities.equipped(A::GuardCounter), 1);
    // An old file (no field): the base sheet's Standard, as it was made.
    let old_text = text.replace(",\n  \"difficulty\": \"CRITICAL\"", "").replace("\"difficulty\": \"CRITICAL\",\n  ", "");
    assert!(!old_text.contains("difficulty"), "{old_text}");
    let old = SoraSave::from_json(&old_text).unwrap();
    assert_eq!((old.difficulty.as_str(), old.game_level(GameLevel::Standard)), ("", GameLevel::Standard));
    assert_eq!(old.status(&base).game_level, GameLevel::Standard);
    // A Standard character captured again records STANDARD.
    let std = SoraSave::capture(&sora_of(new_game(GameLevel::Standard)), &Inventory::default(), "T", 1, 1);
    assert_eq!(std.difficulty, "STANDARD");
}

#[test]
fn the_one_time_switch_of_an_existing_character_and_back() {
    let base = base();
    // A Standard character at level 11 with some progress.
    let mut status = new_game(GameLevel::Standard);
    status.exp = 7954;
    status.rebuild();
    assert_eq!(status.level, 11);
    let sora = sora_of(status);
    let before = SoraSave::capture(&sora, &Inventory::default(), "Tarnished", 100, 200);
    let s0 = before.status(&base);
    let mut save = before.clone();
    let lines = save.switch_difficulty(GameLevel::Standard, GameLevel::Critical);
    assert!(lines.iter().any(|l| l.contains("STANDARD -> CRITICAL")), "{lines:?}");
    assert_eq!((save.difficulty.as_str(), save.boosts[3]), ("CRITICAL", 30));
    let s1 = save.status(&base);
    assert_eq!((s1.game_level, s1.level, s1.exp), (GameLevel::Critical, s0.level, s0.exp), "EXP and level are kept");
    assert_eq!(s1.max_hp(), 52, "halved base 45 + halved choice bonus 7");
    assert_eq!(s1.max_ap(), s0.max_ap() + 30);
    assert_eq!((s1.abilities.owned(A::CriticalCharge), s1.abilities.equipped(A::CriticalCharge), s1.abilities.owned(A::GuardCounter), s1.abilities.equipped(A::GuardCounter)), (1, 1, 1, 0), "start set owned; the new copies wait for the menu");
    assert_eq!(s1.abilities.owned(A::CriticalCounter), 0, "dec032");
    // Played and saved at Critical (the capture records the Critical set), through the file, then back to Standard:
    // the record returns (the start abilities stay owned).
    let played = SoraSave::capture(&sora_of(s1.clone()), &Inventory::default(), "Tarnished", 100, 200);
    assert_eq!((played.difficulty.as_str(), played.boosts[3]), ("CRITICAL", 30));
    let mut again = SoraSave::from_json(&played.to_json()).unwrap();
    // (dec032) Critical Counter won later (a level slot, a boss) is his own: switching back keeps it
    again.abilities.push((A::CriticalCounter.value(), 1, 1));
    let lines = again.switch_difficulty(GameLevel::Critical, GameLevel::Standard);
    assert!(lines.iter().any(|l| l.contains("Critical-only")), "{lines:?}");
    let s2 = again.status(&base);
    assert_eq!((s2.game_level, s2.max_hp(), s2.max_ap(), again.boosts[3]), (GameLevel::Standard, s0.max_hp(), s0.max_ap(), 0));
    assert_eq!((s2.abilities.owned(A::CriticalCharge), s2.abilities.owned(A::CriticalConverter), s2.abilities.owned(A::CriticalCounter)), (0, 0, 1));
    // No switch to the same level.
    assert!(again.switch_difficulty(GameLevel::Standard, GameLevel::Standard).is_empty());
}

#[test]
fn critical_boss_rewards_skip_the_abilities_critical_gives_at_the_start() {
    let table = Table::parse(include_str!("../../../sheets/port/sora-er-boss-rewards.json"));
    // Godrick: Cure, Counter Slash (a Critical start ability), a Great Rune slot.
    let mut crit = sora_of(new_game(GameLevel::Critical));
    let mut records = Vec::new();
    let Claim::Given(c) = claim(&table, &mut records, &mut crit, "godrick", false) else { panic!() };
    let counter = c.given.iter().find(|g| g.text.contains("Counter Slash")).expect("the ability's line");
    assert!(!counter.changed && counter.text.contains("Critical gives it at the start"), "{}", c.text());
    assert_eq!(crit.status().abilities.owned(A::GuardCounter), 1, "the start copy only");
    assert!(c.given.iter().filter(|g| !g.text.contains("Counter Slash")).all(|g| g.changed), "{}", c.text());
    // Standard: the ability is given.
    let mut std = sora_of(new_game(GameLevel::Standard));
    let mut records = Vec::new();
    let Claim::Given(c) = claim(&table, &mut records, &mut std, "godrick", false) else { panic!() };
    assert!(c.given.iter().all(|g| g.changed), "{}", c.text());
    assert_eq!(std.status().abilities.owned(A::GuardCounter), 1);
    // (dec032 + XSI) Godskin Noble on Critical, the sheet as it is: Risk Dodge is a Critical start ability (skipped by the
    // Critical rule), Max HP +10 comes as Critical's +5, and Critical Counter (XSI's row xs.godskin_noble; NOT part of the
    // port's Critical start, dec032) IS given. XSI's rule.skip_if_owned then stops a second copy of it.
    let mut crit = sora_of(new_game(GameLevel::Critical));
    let mut records = Vec::new();
    assert_eq!(crit.status().abilities.owned(A::CriticalCounter), 0);
    let Claim::Given(c) = claim(&table, &mut records, &mut crit, "godskin_noble", false) else { panic!() };
    let line = |name: &str| c.given.iter().find(|g| g.text.contains(name)).unwrap_or_else(|| panic!("{name}: {}", c.text()));
    assert!(!line("Risk Dodge").changed && line("Risk Dodge").text.contains("Critical gives it at the start"), "{}", c.text());
    assert!(line("Critical Counter").changed, "{}", c.text());
    assert_eq!(crit.status().abilities.owned(A::CriticalCounter), 1);
    assert!(c.given.iter().any(|g| g.changed && g.text.contains("HP +5")), "{}", c.text());
    // a second Critical Counter (the same token twice) is XSI's skip_if_owned, not the Critical rule
    let text = include_str!("../../../sheets/port/sora-er-boss-rewards.json").replace("\"proposal\":[\"ability:RISKDODGE\",\"hp:10\"]", "\"proposal\":[\"ability:CRITICAL_COUNTER\",\"hp:10\"]");
    let mut again = sora_of(new_game(GameLevel::Critical));
    let mut records = Vec::new();
    let Claim::Given(c) = claim(&Table::parse(&text), &mut records, &mut again, "godskin_noble", false) else { panic!() };
    let counters: Vec<_> = c.given.iter().filter(|g| g.text.contains("Critical Counter")).collect();
    assert_eq!((counters.len(), counters[0].changed, counters[1].changed), (2, true, false), "{}", c.text());
    assert!(counters[1].text.contains("skip_if_owned"), "{}", c.text());
    assert_eq!(again.status().abilities.owned(A::CriticalCounter), 1);
}
