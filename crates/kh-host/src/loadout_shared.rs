//! Reads a loadout file into `kh_core::Loadout` (the core reads no files).
//!
//! Default: `sheets/port/sora-loadout-99.json` (the stats spec's level-99 loadout) when it
//! exists, relative to the working directory; `--loadout FILE` names another file; without
//! either the status is the explicitly empty placeholder. Two shapes are accepted, both with the
//! row ids listed in `kh_core::status::loadout`:
//! - a sheet `{ "columns": ["id", "value", ...], "rows": [["level", 99, ...], ...] }`;
//! - a JSON object `{ "level": 99, "game_level": "STANDARD", "abilities.equipped": [...] }`.
//!
//! Rows that are not understood are listed in the log and counted in the debug text; they are
//! never guessed. A null value is "unchecked" in the sheets' sense and is reported the same way.

use kh_core::status::LoadoutValue;
use kh_core::Loadout;

/// The stats spec's level-99 loadout.
const DEFAULT_SHEET: &str = "sheets/port/sora-loadout-99.json";

pub fn load(path: Option<&str>) -> Loadout {
    let path = match path {
        Some(path) => path,
        None if std::path::Path::new(DEFAULT_SHEET).is_file() => DEFAULT_SHEET,
        None => return Loadout::empty_placeholder(),
    };
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("loadout: cannot read {path}: {e}");
        std::process::exit(2);
    });
    let (loadout, problems) = parse(path, &text).unwrap_or_else(|e| {
        eprintln!("loadout: {path}: {e}");
        std::process::exit(2);
    });
    for problem in problems.iter().take(12) {
        println!("loadout: {path}: not understood: {problem}");
    }
    if problems.len() > 12 {
        println!("loadout: {path}: ... and {} more", problems.len() - 12);
    }
    loadout
}

/// TEST-LEVEL CHOICE (integration pass, 2026-10-12; sheet `sora-test-stock`, authored): what the
/// test level puts into the stock so that the camp menu's Items and Equipment screens have
/// content. Not game data, not part of the loadout: applied by the app only with the default
/// loadout (`--loadout FILE` gets none), items into the core's pouch stock, equipment into the HUD
/// host's inventory.
const TEST_STOCK_SHEET: &str = "sheets/port/sora-test-stock.json";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TestStock {
    pub items: Vec<(kh_core::status::ItemKind, u8)>,
    /// (row id of the equipment table, count).
    pub equipment: Vec<(String, u32)>,
}

/// The test stock of the sheet's text; names that are not understood are listed.
pub fn parse_test_stock(text: &str) -> (TestStock, Vec<String>) {
    let (mut stock, mut problems) = (TestStock::default(), Vec::new());
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else { return (stock, vec!["not JSON".to_string()]) };
    for row in json["rows"].as_array().into_iter().flatten() {
        let (id, cells) = (row[0].as_str().unwrap_or(""), row[1].as_object());
        for (name, count) in cells.into_iter().flatten() {
            let n = count.as_u64().unwrap_or(0).min(99) as u32;
            match id {
                "stock.items" => match kh_core::status::ItemKind::from_name(name) {
                    Some(item) => stock.items.push((item, n as u8)),
                    None => problems.push(format!("{id}: {name} is not a battle item")),
                },
                "stock.equipment" => match kh_core::status::tables::equipment_row(name) {
                    Some(row) => stock.equipment.push((row.id.to_string(), n)),
                    None => problems.push(format!("{id}: {name} is not an equipment row")),
                },
                _ => problems.push(format!("{id}: not a row of the test stock")),
            }
        }
    }
    (stock, problems)
}

/// The test stock for this run: empty unless the default loadout is in use and the sheet exists.
pub fn test_stock(loadout_option: Option<&str>) -> TestStock {
    if loadout_option.is_some() {
        return TestStock::default();
    }
    let Ok(text) = std::fs::read_to_string(TEST_STOCK_SHEET) else { return TestStock::default() };
    let (stock, problems) = parse_test_stock(&text);
    for problem in &problems {
        println!("loadout: {TEST_STOCK_SHEET}: not understood: {problem}");
    }
    stock
}

/// TEST-LEVEL CHOICE (fixes pass 1, 2026-10-12; sheet `sora-test-levels`, authored, by the
/// user's request): the loadouts the run-time level toggle cycles through. Each is the default
/// loadout at another level (`Loadout::set_level`, what `--level N` does: level, EXP, stats and
/// the level table's ability grants of that level; equipped copies beyond the level's AP are
/// unequipped) with every known element of magic at `magic_tier` and the shortcut sets naming
/// the same elements at that tier.
const TEST_LEVELS_SHEET: &str = "sheets/port/sora-test-levels.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TestLevel {
    pub level: u32,
    /// 1 base spells, 2 the -ra tier, 3 the -ga tier.
    pub magic_tier: u32,
    /// The story and Re Mind victory bonuses (max HP / MP, slots) of the default loadout are
    /// kept; false = none of them: the state of a new game.
    pub story_bonuses: bool,
    /// (new game) THE TRUE START OF A NEW GAME (sheet `sora-newgame`): nothing of the default loadout
    /// is kept but its difficulty; `magic_tier` and `story_bonuses` are not used.
    pub new_game: bool,
}

impl TestLevel {
    /// What the sheet says when it cannot be read (the same three rows).
    pub const DEFAULT: [TestLevel; 3] = [TestLevel { level: 1, magic_tier: 1, story_bonuses: false, new_game: true }, TestLevel { level: 20, magic_tier: 2, story_bonuses: true, new_game: false }, TestLevel { level: 99, magic_tier: 3, story_bonuses: true, new_game: false }];

    pub fn tier_name(&self) -> &'static str {
        if self.new_game {
            return "a new game";
        }
        match self.magic_tier {
            0 => "no magic",
            1 => "base spells",
            2 => "-ra spells",
            _ => "-ga spells",
        }
    }
}

/// The test levels of the sheet's text, ascending by level; rows that are not understood are listed.
pub fn parse_test_levels(text: &str) -> (Vec<TestLevel>, Vec<String>) {
    let (mut levels, mut problems) = (Vec::new(), Vec::new());
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else { return (levels, vec!["not JSON".to_string()]) };
    for row in json["rows"].as_array().into_iter().flatten() {
        let id = row[0].as_str().unwrap_or("");
        match (id.starts_with("level."), row[1]["level"].as_u64(), row[1]["magic_tier"].as_u64()) {
            (true, Some(level @ 1..=99), Some(tier @ 0..=3)) => levels.push(TestLevel { level: level as u32, magic_tier: tier as u32, story_bonuses: row[1]["story_bonuses"].as_u64() != Some(0), new_game: row[1]["new_game"].as_u64() == Some(1) }),
            _ => problems.push(format!("{id}: not a test-level row (level.<N> with level 1..99 and magic_tier 0..3)")),
        }
    }
    levels.sort_by_key(|l| l.level);
    (levels, problems)
}

/// The test levels of this run (the sheet's, else the built-in three).
pub fn test_levels() -> Vec<TestLevel> {
    let Ok(text) = std::fs::read_to_string(TEST_LEVELS_SHEET) else { return TestLevel::DEFAULT.to_vec() };
    let (levels, problems) = parse_test_levels(&text);
    for problem in &problems {
        println!("loadout: {TEST_LEVELS_SHEET}: not understood: {problem}");
    }
    if levels.is_empty() {
        TestLevel::DEFAULT.to_vec()
    } else {
        levels
    }
}

/// (new game) THE SHEET `sora-newgame`, compiled in: what a new game starts Sora with, from the game's
/// start data (docs/sora-stats-spec.md section 14). A host needs no file for it.
pub const NEW_GAME_SHEET: &str = include_str!("../../../sheets/port/sora-newgame.json");

/// (new game) THE TRUE START OF A NEW GAME with the opening pair `choice`: level 1, EXP 0, the start
/// abilities of `TresChrInitData` (16 equipped + Zero EXP, 28 AP of 28), Fire and Water, an empty pouch
/// with three slots, no shortcuts, the Kingdom Key; max HP / MP by the pair's first half, the level
/// table's ability column by its second. `base` (the host's loadout sheet) gives only what a new
/// game does not decide here: the difficulty and the battle level.
pub fn new_game(base: &Loadout, choice: kh_core::status::OpeningChoice) -> Loadout {
    let mut loadout = match parse("sheets/port/sora-newgame.json (compiled in)", NEW_GAME_SHEET) {
        Ok((loadout, _)) => loadout,
        // (cannot happen with the sheet the unit test reads: an empty level-1 loadout then)
        Err(_) => {
            let mut empty = Loadout::new("NEW GAME (the compiled-in sheet could not be read)");
            (empty.level, empty.listed_level, empty.bonuses) = (1, 1, Some(kh_core::status::BonusCounts::NONE));
            empty
        }
    };
    if !base.game_level.is_empty() {
        loadout.game_level = base.game_level.clone();
    }
    loadout.battle_level = base.battle_level;
    loadout.set_choice(choice);
    // (DF, 2026-10-15) KH3 Critical's start: the flagged victory-bonus rows' abilities (nothing below Critical)
    loadout.add_critical_start();
    loadout.source = format!("A NEW GAME ({}) of sheets/port/sora-newgame.json", choice.loadout_text());
    loadout
}

/// `base` (a loadout as its file gives it) as the test loadout `test`.
pub fn at_test_level(base: &Loadout, test: TestLevel) -> Loadout {
    use kh_core::magic::spells;
    if test.new_game {
        // The test levels keep a fixed pair (the default loadout's: balance + path A).
        let mut loadout = new_game(base, kh_core::status::OpeningChoice::of_loadout(base).unwrap_or(kh_core::status::OpeningChoice::DEFAULT));
        if test.level > 1 {
            loadout.set_level(test.level);
        }
        return loadout;
    }
    let mut loadout = base.clone();
    loadout.set_level(test.level);
    for known in &mut loadout.magic {
        known.tier = test.magic_tier;
    }
    loadout.magic.retain(|m| m.tier > 0);
    if !test.story_bonuses {
        // (The opening choice's own bonus row is added by the status rules.)
        loadout.bonuses = Some(kh_core::status::BonusCounts::NONE);
    }
    // The shortcut sets: the same element at the test tier (an item or an empty slot stays).
    for page in &mut loadout.shortcuts {
        for name in page.iter_mut() {
            if let Some(unit) = spells::command_by_name(name).and_then(spells::unit) {
                *name = if test.magic_tier == 0 { String::new() } else { spells::command_name(unit.element.command(test.magic_tier.min(3) as u8)).to_string() };
            }
        }
    }
    loadout.source = format!("{} AS TEST LEVEL {} ({})", base.source, test.level, test.tier_name());
    loadout
}

/// A JSON cell as a loadout value; `None` for null and for shapes a loadout row cannot have.
fn value(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

/// The loadout of a file's text and the rows that were not understood.
pub fn parse(path: &str, text: &str) -> Result<(Loadout, Vec<String>), String> {
    let json: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
    let mut rows: Vec<(String, serde_json::Value)> = Vec::new();
    if let (Some(columns), Some(sheet_rows)) = (json.get("columns").and_then(|c| c.as_array()), json.get("rows").and_then(|r| r.as_array())) {
        let column = |name: &str| columns.iter().position(|c| c.as_str() == Some(name));
        let (id, cell) = (column("id").ok_or("a sheet without an id column")?, column("value").ok_or("a sheet without a value column")?);
        for row in sheet_rows.iter().filter_map(|r| r.as_array()) {
            rows.push((row.get(id).and_then(|v| v.as_str()).unwrap_or("(row without id)").to_string(), row.get(cell).cloned().unwrap_or(serde_json::Value::Null)));
        }
    } else if let Some(object) = json.as_object() {
        rows.extend(object.iter().map(|(k, v)| (k.clone(), v.clone())));
    } else {
        return Err("neither a sheet with columns and rows nor a JSON object of loadout rows".to_string());
    }
    let mut loadout = Loadout::new(path);
    let (mut set, mut problems) = (0, Vec::new());
    for (key, cell) in rows {
        match value(&cell).ok_or_else(|| format!("{key}: the value {} is unchecked or not a loadout value", cell.to_string().chars().take(60).collect::<String>())).and_then(|v| loadout.set(&key, &v)) {
            Ok(()) => set += 1,
            Err(problem) => problems.push(problem),
        }
    }
    // HP and MP: the rows the sheet's own default choice and game level select.
    if let Err(problem) = loadout.resolve() {
        problems.push(problem);
    }
    loadout.source = format!("{path}: {set} rows set, {} not understood", problems.len());
    Ok((loadout, problems))
}

/// (party, stream F) TEST-LEVEL CHOICE (sheet `party-test`, authored; plan open note N2): the friends' loadout on
/// top of the level-99 status kh-party builds, the spare staffs / shields of the camp menu's stock, where they
/// stand at start. The friends' weapon rows themselves are sheet `party-weapons` (stream B, TresEquipItemData).
pub const PARTY_TEST_SHEET: &str = "sheets/port/party-test.json";
pub const PARTY_WEAPONS_SHEET: &str = "sheets/port/party-weapons.json";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartyLoadout {
    pub weapon: String,
    pub items: Vec<String>,
    /// Record bytes +0x158 .. +0x15B.
    pub ai: [u8; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartyTest {
    pub donald: PartyLoadout,
    pub goofy: PartyLoadout,
    pub stock_weapons: Vec<(String, u32)>,
    /// (length cm, angle deg from Sora's facing) for Donald, Goofy.
    pub offsets: [(f32, f32); 2],
    /// Sora's team abilities to equip, and the copies taken off to make the AP (row sora.team_abilities).
    pub sora_team: (Vec<String>, Vec<String>),
}

/// One weapon row of sheet `party-weapons`: (item key, owner npc, Strength, Magic, Defense, abilities, model).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartyWeapon {
    pub key: String,
    pub npc: String,
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
    pub abilities: Vec<String>,
    pub model: String,
}

fn sheet_rows(path: &str) -> Vec<serde_json::Value> {
    let full = kh_gendata::project_file(path).to_string(); // (release) under the repository
    let text = std::fs::read_to_string(path).or_else(|_| std::fs::read_to_string(&full)).unwrap_or_default();
    serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|j| j["rows"].as_array().cloned()).unwrap_or_default()
}

pub fn party_test() -> PartyTest {
    let mut out = PartyTest { offsets: [(180.0, 70.0), (180.0, -70.0)], ..PartyTest::default() };
    for row in sheet_rows(PARTY_TEST_SHEET) {
        let (id, value) = (row[0].as_str().unwrap_or(""), &row[1]);
        let loadout = |v: &serde_json::Value| PartyLoadout {
            weapon: v["weapon"].as_str().unwrap_or("").to_string(),
            items: v["items"].as_array().into_iter().flatten().filter_map(|i| i.as_str().map(str::to_string)).collect(),
            ai: {
                let a: Vec<u8> = v["ai"].as_array().into_iter().flatten().filter_map(|x| x.as_u64().map(|x| x as u8)).collect();
                [a.first().copied().unwrap_or(0), a.get(1).copied().unwrap_or(0), a.get(2).copied().unwrap_or(1), a.get(3).copied().unwrap_or(7)]
            },
        };
        match id {
            "donald.loadout" => out.donald = loadout(value),
            "goofy.loadout" => out.goofy = loadout(value),
            "stock.weapons" => out.stock_weapons = value.as_object().into_iter().flatten().map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(1) as u32)).collect(),
            "sora.team_abilities" => {
                let list = |k: &str| value[k].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>();
                out.sora_team = (list("equip"), list("make_room"));
            }
            "spawn.offsets" => {
                for (i, who) in ["donald", "goofy"].into_iter().enumerate() {
                    if let (Some(l), Some(a)) = (value[who][0].as_f64(), value[who][1].as_f64()) {
                        out.offsets[i] = (l as f32, a as f32);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

pub fn party_weapons() -> Vec<PartyWeapon> {
    sheet_rows(PARTY_WEAPONS_SHEET)
        .iter()
        .map(|r| PartyWeapon {
            key: r[1].as_str().unwrap_or("").to_string(),
            npc: r[2].as_str().unwrap_or("").to_string(),
            strength: r[4].as_i64().unwrap_or(0) as i32,
            magic: r[5].as_i64().unwrap_or(0) as i32,
            defense: r[6].as_i64().unwrap_or(0) as i32,
            abilities: r[7].as_str().unwrap_or("").split([',', ' ']).filter(|s| !s.is_empty()).map(str::to_string).collect(),
            model: r[8].as_str().unwrap_or("").to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_party_test_sheet_loads() {
        let t = super::party_test();
        assert_eq!(t.donald.weapon, "WEP_DONALD_00");
        assert_eq!(t.goofy.items, vec!["ITEM_POTION", "ITEM_POTION"]);
        assert_eq!(t.donald.ai, [0, 0, 1, 7]);
        assert_eq!(t.stock_weapons.len(), 26);
        assert_eq!(t.offsets, [(180.0, 70.0), (180.0, -70.0)]);
        let w = super::party_weapons();
        assert_eq!(w.len(), 28);
        assert_eq!((w[0].key.as_str(), w[0].strength, w[0].magic), ("WEP_DONALD_00", 3, 4));
    }
    use super::*;
    use kh_core::enums::ETresAbilityKind;

    #[test]
    fn a_sheet_and_an_object_give_the_same_loadout() {
        let sheet = r#"{ "sheet": "x", "columns": ["id", "value", "unit"], "rows": [["level", 99, "-"], ["game_level", "STANDARD", "-"], ["choice.default", "balance + ability path A", "-"],
            ["hp.balance.standard", 265, "HP"], ["mp.balance.standard", 169, "MP"], ["abilities.owned", ["AIR_RECOVERY", "BLOW_COUNTER"], "-"], ["abilities.equipped", ["AIR_RECOVERY"], "-"],
            ["magic.known", {"fire": 3}, "tier"], ["bogus", 3, "-"], ["exp", null, "-"]] }"#;
        let object = r#"{ "level": 99, "game_level": "STANDARD", "choice.default": "balance + ability path A", "hp.balance.standard": 265, "mp.balance.standard": 169,
            "abilities.owned": ["AIR_RECOVERY", "BLOW_COUNTER"], "abilities.equipped": ["AIR_RECOVERY"], "magic.known": {"fire": 3}, "bogus": 3, "exp": null }"#;
        let (a, problems_a) = parse("a", sheet).unwrap();
        let (b, problems_b) = parse("b", object).unwrap();
        assert_eq!((a.level, a.hp_max, a.mp_max), (99, 265.0, 169.0));
        assert_eq!(a.abilities, vec![(ETresAbilityKind::AirRecovery, 1, 1), (ETresAbilityKind::BlowCounter, 1, 0)]);
        assert_eq!((a.level, a.hp_max, a.mp_max, &a.abilities, &a.magic), (b.level, b.hp_max, b.mp_max, &b.abilities, &b.magic));
        assert_eq!((problems_a.len(), problems_b.len()), (2, 2), "{problems_a:?}");
        assert!(!a.placeholder && a.source.contains("8 rows set, 2 not understood"));
        assert!(parse("c", "[1, 2]").is_err() && parse("d", "not json").is_err());
    }

    #[test]
    fn the_test_levels_are_level_1_20_and_99_with_their_magic_tiers() {
        // Fixes pass 1: the sheet's rows, and what each makes of the level-99 loadout.
        let levels = match std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-test-levels.json")) {
            Ok(text) => {
                let (levels, problems) = parse_test_levels(&text);
                assert!(problems.is_empty(), "{problems:?}");
                levels
            }
            Err(_) => TestLevel::DEFAULT.to_vec(),
        };
        assert_eq!(levels, TestLevel::DEFAULT.to_vec());
        let (bad, problems) = parse_test_levels(r#"{ "rows": [["level.5", {"level": 5, "magic_tier": 2}], ["level.x", {"level": 0, "magic_tier": 1}], ["other", {}]] }"#);
        assert_eq!((bad, problems.len()), (vec![TestLevel { level: 5, magic_tier: 2, story_bonuses: true, new_game: false }], 2));
        let Ok(text) = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-loadout-99.json")) else { return };
        let (base, _) = parse("sora-loadout-99", &text).unwrap();
        // (new game) Level 1 is the true start of a new game: Fire and Water, no shortcut, nothing of the level-99 lists.
        let start = kh_core::Status::from_loadout(&at_test_level(&base, levels[0]));
        assert!(levels[0].new_game && start.level == 1 && start.ap_in_use() == 28 && start.max_ap() == 28 && start.abilities.iter_owned().count() == 17);
        assert_eq!(start.magic.iter().map(|m| (m.name.as_str(), m.tier)).collect::<Vec<_>>(), vec![("fire", 1), ("water", 1)]);
        let mut command = kh_core::command::CommandState::default();
        command.refill(&start);
        assert_eq!((command.magic_slots[0], command.magic_slots[2], command.shortcuts), (kh_core::magic::spells::Element::Fire.command(1), 0, [[0; 4]; 3]));
        let mut ap_before = start.max_ap();
        for (test, slot0, shortcut0, cure) in [(levels[1], "Fira", "Fira", "Cura"), (levels[2], "Firaga", "Firaga", "Curaga")] {
            let loadout = at_test_level(&base, test);
            let status = kh_core::Status::from_loadout(&loadout);
            assert_eq!(status.level, test.level);
            assert!(status.magic.len() == 6 && status.magic.iter().all(|m| m.tier == test.magic_tier), "{:?}", status.magic);
            assert!(status.ap_within_budget() && status.hp == status.max_hp() && status.mp == status.max_mp());
            assert!(status.max_ap() >= ap_before, "AP grows with the level");
            ap_before = status.max_ap();
            // The command menu's Magic list and the shortcut sets follow the tier.
            let mut command = kh_core::command::CommandState::default();
            command.refill(&status);
            let name = |kind: u8| kh_core::magic::spells::command_name(kind);
            assert_eq!((name(command.magic_slots[0]), name(command.shortcuts[0][0]), name(command.shortcuts[0][3])), (slot0, shortcut0, cure), "level {}", test.level);
            assert_eq!(name(command.shortcuts[1][2]), "Potion", "an item shortcut stays");
        }
        // Level 1 has less of everything; level 99 has the sheet's own numbers again.
        let one = kh_core::Status::from_loadout(&at_test_level(&base, levels[0]));
        let top = kh_core::Status::from_loadout(&at_test_level(&base, levels[2]));
        assert!(one.max_ap() < top.max_ap() && one.strength() < top.strength());
        // Level 1 is a new game: no story bonus (balance: HP 90 + 15, MP 100 + 10).
        assert_eq!((one.max_hp(), one.max_mp()), (105, 110));
        assert_eq!((top.level, top.max_hp(), top.max_mp(), top.max_ap()), (99, 265, 169, 180));
    }

    #[test]
    fn the_test_stock_sheet_names_items_and_equipment() {
        let (stock, problems) = parse_test_stock(r#"{ "rows": [["stock.items", {"ITEM_POTION": 9, "ITEM_NOPE": 1}], ["stock.equipment", {"ACC_ITEM03": 2, "X": 1}], ["other", {"a": 1}]] }"#);
        assert_eq!(stock.items, vec![(kh_core::status::ItemKind::Potion, 9)]);
        assert_eq!(stock.equipment, vec![("ACC_ITEM03".to_string(), 2)]);
        assert_eq!(problems.len(), 3, "{problems:?}");
        // The real sheet has no row that is not understood.
        if let Ok(text) = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-test-stock.json")) {
            let (stock, problems) = parse_test_stock(&text);
            assert!(problems.is_empty() && stock.items.len() == 11 && stock.equipment.len() == 8, "{problems:?}");
        }
    }

    #[test]
    fn the_level_99_sheet_loads_completely_when_it_exists() {
        // The real sheet, from the project root (cargo runs the tests in the crate's folder).
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-loadout-99.json");
        let Ok(text) = std::fs::read_to_string(path) else {
            println!("SKIPPED: {path} does not exist");
            return;
        };
        let (loadout, problems) = parse(path, &text).unwrap();
        assert!(problems.is_empty(), "rows of the loadout sheet that kh_core::status::loadout does not know: {problems:?}");
        let status = kh_core::Status::from_loadout(&loadout);
        assert_eq!(status.level, 99);
        assert!(status.hp_max > 0 && status.mp_max > 0 && status.focus_max > 0);
        // The sheet's stated numbers are what the status rules compute from its save data.
        assert_eq!(loadout.verify(&status), Vec::<String>::new());
        assert_eq!((status.hp_max, status.mp_max, status.stats.strength, status.stats.magic, status.stats.defense, status.stats.ap, status.stats.ap_max), (265, 169, 66, 67, 59, 180, 180));
        assert!(status.abilities.iter_equipped().count() > 0 && !status.magic.is_empty());
    }
}
