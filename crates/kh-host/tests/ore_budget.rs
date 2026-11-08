//! (P13 ORE, documentation/KH3_WEAPONS_PLAN.md 3.1; THE USER 2026-11-03: "make sure there is enough orichalcum spread
//! throughout the game for synthesis (some bosses can drop it, can be found in chests, etc, but orichalcum must be a fixed
//! item drop (never random) and there should be enough to craft everything") THE ORE BUDGET.
//!
//! - THE NEED, recomputed: every synthesis recipe once and every Keyblade Forge level a Keyblade lacks when obtained
//!   (`m_bInitAchieved`), from P6a's sheets/port/kh-workshop.json (`syn.*` rows) when it carries the recipes, else the WD
//!   sheets kh3-synthesis; the Forge from kh3-keyblade-levels. Orichalcum (MAT_ITEM56) and Orichalcum+ (MAT_ITEM57).
//! - THE PLACEMENTS (sheet sheets/port/kh-ore-placements.json, read by kh_host::bossrewards::Placements): the base game's
//!   ACTIVE ones (verified, a flag, an item token) cover the need; every counted one is verified against the boss table's
//!   verified defeat flag; no two share a flag; at least 7 Orichalcum come before Morgott; the sheet's need rows are the
//!   recomputed need.
//! - NEVER RANDOM: no rule of the loot sheet (sora-er-items) gives either ore but the Ancient Dragon stones' own rows from
//!   fixed sources and the Larval Tear (a fixed source, margin only); the material families hold neither; the KH shop's stock
//!   and the small bosses' item line neither. (The behaviour of every rule over every source and roll: kh-er lootmap.rs
//!   `p10_orichalcum_is_never_random`.)
//! - ONCE PER SAVE: a placement whose flag is set is given once (record `ore:<id>`), whatever the boss row's own record; an
//!   unverified one never.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use kh_host::bossrewards::{self, Placements, Table, Token};
use serde_json::Value;

const ORICHALCUM: &str = "MAT_ITEM56";
const ORICHALCUM_PLUS: &str = "MAT_ITEM57";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sheet(name: &str) -> Option<Value> {
    let text = std::fs::read_to_string(root().join("sheets/port").join(format!("{name}.json"))).ok()?;
    serde_json::from_str(&text).ok()
}

fn rows(v: &Value) -> &[Value] {
    v["rows"].as_array().map_or(&[][..], |r| r.as_slice())
}

fn col(v: &Value, name: &str) -> usize {
    v["columns"].as_array().and_then(|c| c.iter().position(|x| x.as_str() == Some(name))).unwrap_or_else(|| panic!("no column {name}"))
}

/// The need of every material: (counts, where the recipes came from).
fn need() -> (HashMap<String, u32>, &'static str) {
    let mut out: HashMap<String, u32> = HashMap::new();
    let mut from = "kh3-synthesis";
    let syn = sheet("kh3-synthesis").expect("sheets/port/kh3-synthesis.json");
    let recipes = rows(&syn).len();
    // (P6a) kh-workshop.json's `syn.IS_<n>` rows, when they carry every recipe's materials as [key, count] pairs
    let workshop: Vec<Vec<(String, u32)>> = sheet("kh-workshop")
        .map(|w| {
            rows(&w)
                .iter()
                .filter(|r| r[0].as_str().is_some_and(|id| id.starts_with("syn.IS_")))
                .filter_map(|r| r[1]["materials"].as_array().map(|m| m.iter().filter_map(|p| Some((p[0].as_str()?.to_string(), p[1].as_u64()? as u32))).collect()))
                .collect()
        })
        .unwrap_or_default();
    if workshop.len() == recipes {
        from = "kh-workshop";
        for list in workshop {
            for (key, n) in list {
                *out.entry(key).or_default() += n;
            }
        }
    } else {
        let (m, c) = (col(&syn, "materials"), col(&syn, "counts"));
        for r in rows(&syn) {
            for (key, n) in r[m].as_array().unwrap().iter().zip(r[c].as_array().unwrap()) {
                *out.entry(key.as_str().unwrap().to_string()).or_default() += n.as_u64().unwrap() as u32;
            }
        }
    }
    let lv = sheet("kh3-keyblade-levels").expect("sheets/port/kh3-keyblade-levels.json");
    let (init, m, c) = (col(&lv, "init_achieved"), col(&lv, "materials"), col(&lv, "counts"));
    for r in rows(&lv).iter().filter(|r| r[init].as_bool() != Some(true)) {
        for (key, n) in r[m].as_array().unwrap().iter().zip(r[c].as_array().unwrap()) {
            *out.entry(key.as_str().unwrap().to_string()).or_default() += n.as_u64().unwrap() as u32;
        }
    }
    (out, from)
}

fn table() -> Table {
    let t = Table::parse(bossrewards::SHEET);
    assert!(t.problems.is_empty(), "{:?}", t.problems);
    t
}

fn placements() -> Placements {
    Placements::parse(bossrewards::PLACEMENTS_SHEET, &table())
}

fn item_id(key: &str) -> u32 {
    kh_host::items::by_key(key).unwrap_or_else(|| panic!("no item {key}")).item_id
}

/// The base game's placements cover the recomputed need; the sheet's need rows are that need; every counted placement is
/// verified, on a flag of its own, and the boss table's verified defeat flag; 7 Orichalcum before Morgott.
#[test]
fn p13_the_base_games_fixed_placements_cover_everything_crafting_needs() {
    let (need, from) = need();
    let p = placements();
    assert!(p.problems.is_empty(), "{:?}", p.problems);
    let (o, op) = (need.get(ORICHALCUM).copied().unwrap_or(0), need.get(ORICHALCUM_PLUS).copied().unwrap_or(0));
    assert_eq!((o, op), (15, 7), "the planner's count (recipes from {from}): 14 recipes, IS_80 x2; Ultima Weapon x7");
    assert_eq!((p.need("orichalcum"), p.need("orichalcum_plus")), (Some(o), Some(op)), "the sheet's need rows");
    let (base_o, base_op) = (p.placed(ORICHALCUM, false), p.placed(ORICHALCUM_PLUS, false));
    assert!(base_o >= o, "Orichalcum: the base game places {base_o}, crafting everything takes {o}");
    assert!(base_op >= op, "Orichalcum+: the base game places {base_op}, crafting everything takes {op}");
    assert!(p.placed(ORICHALCUM, true) > base_o && p.placed(ORICHALCUM_PLUS, true) > base_op, "the DLC adds a margin");
    // every counted placement verified on the table's verified flag; no flag twice
    let t = table();
    let mut flags = HashSet::new();
    for r in p.rows.iter().filter(|r| r.active()) {
        assert!(r.verified, "{}", r.id);
        assert!(flags.insert(r.flag.unwrap()), "{}: flag {:?} twice", r.id, r.flag);
        if let Some(boss) = &r.boss {
            let row = t.row(boss).unwrap_or_else(|| panic!("{}: no boss row {boss}", r.id));
            assert_eq!(row.flag, Some((r.flag.unwrap(), bossrewards::Conf::Verified)), "{}", r.id);
        }
    }
    // the unverified ones are given never and counted never
    let pending: Vec<&str> = p.rows.iter().filter(|r| !r.active()).map(|r| r.id.as_str()).collect();
    assert!(p.rows.iter().filter(|r| !r.verified).all(|r| !r.active()));
    println!("ore budget (recipes from {from}): Orichalcum need {o}, base game {base_o} (+ DLC {}); Orichalcum+ need {op}, base game {base_op} (+ DLC {}); not active yet: {pending:?}", p.placed(ORICHALCUM, true) - base_o, p.placed(ORICHALCUM_PLUS, true) - base_op);
    // at least 7 Orichalcum before Morgott (by the boss table's progression order)
    let morgott = t.row("morgott").expect("boss.morgott").order;
    let early: u32 = p.rows.iter().filter(|r| r.active() && !r.dlc && r.order < morgott).map(|r| match r.give {
        Some(Token::Item { item_id: i, count }) if i == item_id(ORICHALCUM) => u32::from(count),
        _ => 0,
    })
    .sum();
    assert!(early >= 7, "{early} Orichalcum before Morgott");
}

/// Neither ore can come at random: only the Ancient Dragon stones' own rows from fixed sources and the Larval Tear name
/// them in the loot sheet; no family, pool, base drop, shop stock or small-boss item line holds them.
#[test]
fn p13_no_random_source_gives_orichalcum() {
    let items = sheet("sora-er-items").expect("sheets/port/sora-er-items.json");
    let fixed: HashSet<&str> = ["world", "chest", "npc", "boss"].into_iter().collect();
    let names = |v: &Value| v.to_string().contains(ORICHALCUM) || v.to_string().contains(ORICHALCUM_PLUS);
    let mut allowed = Vec::new();
    for r in rows(&items) {
        let (id, v) = (r[0].as_str().unwrap(), &r[1]);
        if !names(v) {
            continue;
        }
        match id {
            // the Ancient Dragon stones (10140 / Somber 10168) from Elden Ring's own fixed sources: margin only
            "r2.304.smithing_ancient" | "r2.305.smithing_somber_ancient" => {
                let sources: HashSet<&str> = v["source"].as_array().expect("a source list").iter().filter_map(|s| s.as_str()).collect();
                assert!(!sources.is_empty() && sources.is_subset(&fixed), "{id}: sources {sources:?}");
                let ids = v["ids"].to_string();
                assert!(ids == "[[10140,10140]]" || ids == "[[10168,10168]]", "{id}: ids {ids}");
                allowed.push(id);
            }
            // the Larval Tear: a fixed source (flagged lots); (ER-TWEAKS, the user 2026-11-06: "Remove larval tears from the
            // game and replace them with rare synthesis materials") both ids (8185, the DLC's 2008033), a rare material by
            // its value tier with no chance in it
            "r2.024.larval_tear" => {
                assert_eq!(v["ids"].to_string(), "[[8185,8185],[2008033,2008033]]");
                assert!(!v["give"].as_str().unwrap_or("chance:").contains("chance:"), "{v}");
                allowed.push(id);
            }
            _ => panic!("{id} gives Orichalcum / Orichalcum+: {v}"),
        }
    }
    assert_eq!(allowed.len(), 3, "{allowed:?}");
    // the same stones from any other source: Electrum
    let random = rows(&items).iter().find(|r| r[0] == "r2.306.smithing_ancient_random").expect("r2.306");
    assert_eq!((random[1]["give"].as_str(), random[1]["source"].is_null()), (Some("item:MAT_ITEM58"), true));
    // the families of the `material` give hold no ore
    let families = rows(&items).iter().find(|r| r[0] == "material.families").unwrap()[1].to_string();
    assert!(!families.contains("Orichalcum"), "{families}");
    // the KH shop sells neither; the small bosses' item line holds neither
    let shop = std::fs::read_to_string(root().join("sheets/port/sora-er-shop.json")).unwrap();
    assert!(!shop.contains(ORICHALCUM) && !shop.contains(ORICHALCUM_PLUS));
    assert!(!table().small.item_line.iter().any(|k| k == ORICHALCUM || k == ORICHALCUM_PLUS));
}

/// The token and the claim: `item:<KEY>[:n]` parses (bad keys and counts do not); a placement whose flag is set is given
/// once per save (whatever the boss row's own record), into the item door; an unverified one never; a flag not readable
/// gives nothing.
#[test]
fn p13_a_placement_is_given_once_per_save_by_its_flag() {
    assert_eq!(Token::parse("item:MAT_ITEM56"), Some(Token::Item { item_id: item_id(ORICHALCUM), count: 1 }));
    assert_eq!(Token::parse("item:MAT_ITEM57:7"), Some(Token::Item { item_id: item_id(ORICHALCUM_PLUS), count: 7 }));
    assert_eq!([Token::parse("item:NO_SUCH_ITEM"), Token::parse("item:MAT_ITEM56:0"), Token::parse("item:MAT_ITEM56:100")], [None, None, None]);
    assert_eq!(Token::parse("item:MAT_ITEM56:2").unwrap().label(), "Orichalcum x2");
    let p = placements();
    let radahn = p.rows.iter().find(|r| r.id == "boss.radahn").expect("ore.boss.radahn");
    let flag = radahn.flag.unwrap();
    let set = move |f: u32| Some(f == flag);
    // a save that beat Radahn under an older build (its boss record is there): the ore still comes, once
    let mut records = vec!["boss:radahn".to_string()];
    let due = bossrewards::placements_due(&p, &records, &set);
    assert_eq!(due.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["boss.radahn"]);
    let _ = kh_host::items::take_given();
    let given = bossrewards::claim_placement(&mut records, due[0]).expect("given");
    assert!(given.changed && given.text == "Orichalcum x1", "{given:?}");
    assert!(kh_host::items::take_given().contains(&(item_id(ORICHALCUM), 1)), "through the item door");
    assert!(records.contains(&"ore:boss.radahn".to_string()));
    assert!(bossrewards::placements_due(&p, &records, &set).is_empty(), "once per save");
    assert!(bossrewards::claim_placement(&mut records, radahn).is_none());
    // the flags not readable: nothing; an unverified placement: never, flag or not
    assert!(bossrewards::placements_due(&p, &[], &|_| None).is_empty());
    let mut unverified = p.rows.iter().find(|r| r.id == "chest.1").expect("ore.chest.1 (P9a's Leyndell chest)").clone();
    assert!(unverified.active(), "P9a verified it");
    unverified.verified = false;
    assert!(!unverified.active() && bossrewards::claim_placement(&mut Vec::new(), &unverified).is_none());
    assert!(bossrewards::placements_due(&Placements { rows: vec![unverified], ..Placements::default() }, &[], &|_| Some(true)).is_empty());
    // a boss row may name an item token too (P9b extends it): it reads, and the problems tell a bad one
    let with_item = bossrewards::SHEET.replace("\"user\":[\"magic:thunder\"]", "\"user\":[\"magic:thunder\",\"item:MAT_ITEM56\"]");
    assert_ne!(with_item, bossrewards::SHEET);
    let t = Table::parse(&with_item);
    assert!(t.problems.is_empty(), "{:?}", t.problems);
    assert!(t.row("margit").unwrap().rewards.contains(&Token::Item { item_id: item_id(ORICHALCUM), count: 1 }));
}

/// A placement sheet that breaks the rules is told: a flag that is not the boss row's, two rows on one flag, a give that is
/// not an item token; such rows are not active.
#[test]
fn p13_a_broken_placement_row_is_a_problem_and_not_given() {
    let t = table();
    let wrong_flag = bossrewards::PLACEMENTS_SHEET.replace("\"boss\":\"radahn\",\"flag\":1252380800", "\"boss\":\"radahn\",\"flag\":1252380801");
    assert_ne!(wrong_flag, bossrewards::PLACEMENTS_SHEET);
    let p = Placements::parse(&wrong_flag, &t);
    assert!(p.problems.iter().any(|x| x.contains("ore.boss.radahn")), "{:?}", p.problems);
    assert!(!p.rows.iter().find(|r| r.id == "boss.radahn").unwrap().active());
    let twice = bossrewards::PLACEMENTS_SHEET.replace("\"boss\":\"rykard\",\"flag\":16000800", "\"boss\":\"radahn\",\"flag\":1252380800");
    assert_ne!(twice, bossrewards::PLACEMENTS_SHEET);
    let p = Placements::parse(&twice, &t);
    assert!(p.problems.iter().any(|x| x.contains("also ore.boss.radahn")), "{:?}", p.problems);
    let bad_give = bossrewards::PLACEMENTS_SHEET.replacen("\"give\":\"item:MAT_ITEM56\"", "\"give\":\"magic:fire\"", 1);
    let p = Placements::parse(&bad_give, &t);
    assert!(p.problems.iter().any(|x| x.contains("not an item")), "{:?}", p.problems);
}
