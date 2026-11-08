//! (weapons P2, 2026-11-03) Sora's Keyblades (status/keyblade.rs; documentation/KH3_WEAPONS_PLAN.md 1.3): the stats of
//! every Forge level of all 17 against KH3's table, the init levels, the abilities that enter and leave on a switch,
//! the slot search, the switch command on the pad, the save interface, and THE KINGDOM KEY ALONE EXACTLY AS BEFORE
//! (its rows against `sora-equipment`'s, no switch with one Keyblade). The KH3 numbers are read from the WD sheets
//! (`sheets/port/kh3-keyblades`, `kh3-keyblade-levels`), as a host would read them.

use kh_core::enums::ETresAbilityKind as A;
use kh_core::host::EventKind;
use kh_core::status::keyblade::{KeybladeId, Keyblades, SLOTS};
use kh_core::status::{tables, LoadoutValue};
use kh_core::test_world::TestWorld;
use kh_core::{AnimId, Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

fn sheet(name: &str) -> serde_json::Value {
    let path = format!("{}/../../sheets/port/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap()
}

/// The rows of a sheet as maps column -> cell.
fn rows(name: &str) -> Vec<serde_json::Map<String, serde_json::Value>> {
    let json = sheet(name);
    let columns: Vec<String> = json["columns"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect();
    json["rows"].as_array().unwrap().iter().map(|r| columns.iter().cloned().zip(r.as_array().unwrap().iter().cloned()).collect()).collect()
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

/// The level-99 loadout sheet (the Kingdom Key alone), with extra rows laid over it.
fn level99(extra: &[(&str, LoadoutValue)]) -> Status {
    let json = sheet("sora-loadout-99");
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        loadout.set(row[0].as_str().unwrap(), &cell(&row[1]).unwrap()).unwrap_or_else(|e| panic!("{e}"));
    }
    for (key, value) in extra {
        loadout.set(key, value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

fn kb(key: &str) -> KeybladeId {
    KeybladeId::from_key(key).unwrap_or_else(|| panic!("{key}: no Keyblade"))
}

fn list(items: &[&str]) -> LoadoutValue {
    LoadoutValue::List(items.iter().map(|s| s.to_string()).collect())
}

/// Sora with these Keyblades in the slots (the level-99 sheet otherwise).
fn with_slots(slots: &[&str]) -> Status {
    level99(&[("keyblades", list(slots))])
}

// ------------------------------------------------------------------------------------ the data

#[test]
fn all_17_keyblades_stats_per_level_match_kh3s_forge_table() {
    // WD 3 (Keyblade Forge): stats = base + every done row; the sheet's cumulative columns are KH3's own sums.
    let keyblades = rows("kh3-keyblades");
    assert_eq!((keyblades.len(), KeybladeId::all().count()), (17, 17));
    let levels = rows("kh3-keyblade-levels");
    for k in &keyblades {
        let key = k["id"].as_str().unwrap();
        let id = kb(key);
        assert_eq!((id.key(), id.name(), id.row().item_id as u64), (key, k["name"].as_str().unwrap(), k["item_id"].as_u64().unwrap()));
        let base = (k["strength"].as_i64().unwrap() as i32, k["magic"].as_i64().unwrap() as i32);
        assert_eq!((id.stats(0).strength, id.stats(0).magic), base, "{key} +0");
        assert_eq!(id.max_level() as u64, k["max_level"].as_u64().unwrap(), "{key} max level");
        let mut names: Vec<String> = k["abilities"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().to_string()).collect();
        assert_eq!(id.ability_names(0), names, "{key} +0 abilities");
        for l in levels.iter().filter(|l| l["keyblade"] == key) {
            let level = l["level"].as_u64().unwrap() as u8;
            let stats = id.stats(level);
            assert_eq!((stats.strength, stats.magic), (l["strength"].as_i64().unwrap() as i32, l["magic"].as_i64().unwrap() as i32), "{key} +{level}");
            let ability = l["ability"].as_str().unwrap();
            if ability != "-" {
                names.push(ability.to_string());
            }
            assert_eq!(id.ability_names(level), names, "{key} +{level} abilities");
            assert_eq!(id.abilities(level).len(), names.len(), "{key} +{level}: every ability name is an ETresAbilityKind");
        }
        let forms: Vec<&str> = k["formchanges"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
        assert_eq!(id.forms(), forms.as_slice(), "{key} formchanges");
    }
    // The pairing by item key, not by the WEP_KEYBLADEnn number (WD 2): Shooting Star is SO_02 (= WEP_KEYBLADE01).
    assert_eq!((kb("WEP_KEYBLADE_SO_02").name(), kb("WEP_KEYBLADE_SO_02").forms()), ("Shooting Star", &["CHANGE_DW", "CHANGE_BZ"][..]));
    assert_eq!(KeybladeId::from_item_id(0x301), Some(KeybladeId::KINGDOM_KEY));
}

#[test]
fn init_levels_are_held_from_the_start_and_never_undercut() {
    // WD 3: m_bInitAchieved levels count as done live: Ultima Weapon +10, Favorite Deputy / Ever After +1, Happy Gear /
    // Crystal Snow / Grand Chef +2, Hunny Spout / Nano Gear / Wheel of Fate +3; the plan's 6b: "KH3 init levels from the
    // data apply on obtaining".
    let expected = [("WEP_KEYBLADE_SO_14", 10), ("WEP_KEYBLADE_SO_03", 1), ("WEP_KEYBLADE_SO_04", 1), ("WEP_KEYBLADE_SO_05", 2), ("WEP_KEYBLADE_SO_06", 2), ("WEP_KEYBLADE_SO_10", 2), ("WEP_KEYBLADE_SO_07", 3), ("WEP_KEYBLADE_SO_08", 3), ("WEP_KEYBLADE_SO_09", 3), ("WEP_KEYBLADE_SO_00", 0)];
    let fresh = Keyblades::default();
    for (key, level) in expected {
        assert_eq!((kb(key).init_level(), fresh.level(kb(key))), (level, level), "{key}");
    }
    for k in rows("kh3-keyblades") {
        let id = kb(k["id"].as_str().unwrap());
        assert_eq!(fresh.level(id) as u64, k["init_level"].as_u64().unwrap());
        let mut state = fresh;
        assert_eq!(state.set_level(id, 0), id.init_level(), "below the init level is never held");
        assert_eq!(state.set_level(id, 200), id.max_level(), "above the last Forge row is never held");
    }
    // Equipping Hunny Spout gives +3 at once: base 4 / 4 + its three init rows.
    let mut s = with_slots(&["WEP_KEYBLADE_SO_00", "WEP_KEYBLADE_SO_07"]);
    let (str0, mag0) = (s.strength(), s.magic());
    s.switch_keyblade(true).unwrap();
    let hunny = kb("WEP_KEYBLADE_SO_07");
    let kk = KeybladeId::KINGDOM_KEY.stats(0);
    assert_eq!((s.weapon_upgrade, s.strength() - str0, s.magic() - mag0), (3, hunny.stats(3).strength - kk.strength, hunny.stats(3).magic - kk.magic));
}

// ------------------------------------------------------------------------------------ the Kingdom Key alone

#[test]
fn the_kingdom_key_rows_are_the_old_equipment_rows() {
    // The rows the status read before P2 (sora-equipment WEP_KEYBLADE_SO_00 and .upgrade.1..10) give the same
    // Strength / Magic / abilities as the new sheet at every level.
    let kk = KeybladeId::KINGDOM_KEY;
    assert_eq!(kk.key(), "WEP_KEYBLADE_SO_00");
    for level in 0..=10u8 {
        let old = tables::weapon_row("WEP_KEYBLADE_SO_00", level).unwrap();
        let new = kk.stats(level);
        assert_eq!((new.strength, new.magic), (old.attack, old.magic), "+{level}");
        assert_eq!(kk.ability_names(level), old.abilities.to_vec(), "+{level}");
        assert_eq!((old.ap, old.defense, old.resist.iter().sum::<i32>()), (0, 0, 0), "the sheet carries no AP / defense / resist for it");
    }
    // Every Keyblade's base row equals its sora-equipment row (which carries no AP, defense or resist either).
    for id in KeybladeId::all() {
        let old = tables::equipment_row(id.key()).unwrap();
        assert_eq!((id.stats(0).strength, id.stats(0).magic, id.ability_names(0)), (old.attack, old.magic, old.abilities.to_vec()), "{}", id.key());
        assert_eq!((old.ap, old.defense, old.resist.iter().sum::<i32>()), (0, 0, 0), "{}", id.key());
    }
}

#[test]
fn the_kingdom_key_alone_behaves_as_before() {
    // tests/status.rs `equipment_adds_stats_and_abilities_but_no_hp_or_mp` numbers: level 99, Strength 62 + 4 / Magic 64 + 3.
    let mut s = level99(&[]);
    assert_eq!((s.keyblades().slots(), s.keyblades().current_slot(), s.current_keyblade()), ([Some(KeybladeId::KINGDOM_KEY), None, None], 0, Some(KeybladeId::KINGDOM_KEY)));
    assert_eq!((s.strength(), s.magic(), s.abilities.worn(A::PrizeDraw)), (66, 67, 1));
    s.weapon_upgrade = 10;
    s.rebuild();
    assert_eq!((s.strength(), s.magic(), s.count(A::FormTime), s.keyblade_level(KeybladeId::KINGDOM_KEY)), (71, 72, 1, 10), "a weapon_upgrade write is the Kingdom Key's level");
    // No switch with one Keyblade (sora-ui-spec 10.5: "With the Kingdom Key alone there is no switch").
    let before = s.clone();
    assert_eq!((s.switch_keyblade(true), s.switch_keyblade(false)), (None, None));
    assert_eq!(s, before, "nothing changed");
    // Unarmed (the magic tests clear the weapon) and back.
    s.weapon.clear();
    s.rebuild();
    assert_eq!((s.current_keyblade(), s.strength()), (None, 62));
    s.weapon = "WEP_KEYBLADE_SO_00".to_string();
    s.rebuild();
    assert_eq!((s.current_keyblade(), s.strength(), s.weapon_upgrade), (Some(KeybladeId::KINGDOM_KEY), 71, 10), "the Kingdom Key keeps its level");
}

// ------------------------------------------------------------------------------------ slots and switching

#[test]
fn the_slot_search_skips_empty_slots_and_wraps() {
    // 0x141782070 / 0x1417820E0: the following non-empty slot in the order 0 -> 1 -> 2 -> 0, previous the other way.
    let (mut k, notes) = Keyblades::from_keys(&[Some("WEP_KEYBLADE_SO_00"), None, Some("WEP_KEYBLADE_SO_01")], 0, &[]);
    assert!(notes.is_empty());
    assert_eq!((k.next_slot(true), k.next_slot(false)), (Some(2), Some(2)));
    k.set_current(2).unwrap();
    assert_eq!((k.next_slot(true), k.next_slot(false)), (Some(0), Some(0)));
    let (k3, _) = Keyblades::from_keys(&[Some("WEP_KEYBLADE_SO_00"), Some("WEP_KEYBLADE_SO_01"), Some("WEP_KEYBLADE_SO_02")], 1, &[]);
    assert_eq!((k3.next_slot(true), k3.next_slot(false)), (Some(2), Some(0)));
    assert_eq!(Keyblades::kingdom_key().next_slot(true), None);
    let (mut outside, mut empty) = (k3, Keyblades::kingdom_key());
    assert!(outside.set_current(SLOTS).is_err() && empty.set_current(1).is_err(), "no current slot outside 0..3 or on an empty slot");
}

#[test]
fn a_switch_moves_the_abilities_and_the_stats_to_the_new_keyblade() {
    // Kingdom Key (Treasure Magnet) <-> Happy Gear (Focus Converter, init +2) <-> Nano Gear (Stun Protection, init +3).
    let mut s = with_slots(&["WEP_KEYBLADE_SO_00", "WEP_KEYBLADE_SO_05", "WEP_KEYBLADE_SO_08"]);
    let base_strength = s.strength() - KeybladeId::KINGDOM_KEY.stats(0).strength;
    assert_eq!((s.abilities.worn(A::PrizeDraw), s.has(A::FpConverter), s.has(A::StunGuard)), (1, false, false));
    let sw = s.switch_keyblade(true).unwrap();
    assert_eq!((sw.from, sw.to, sw.from_slot, sw.to_slot), (Some(KeybladeId::KINGDOM_KEY), kb("WEP_KEYBLADE_SO_05"), 0, 1));
    assert_eq!((s.weapon.as_str(), s.weapon_upgrade, s.keyblade_switches, s.keyblade_switch), ("WEP_KEYBLADE_SO_05", 2, 1, Some(sw)));
    assert_eq!((s.abilities.worn(A::PrizeDraw), s.has(A::FpConverter), s.has(A::StunGuard)), (0, true, false), "the Kingdom Key's ability left, Happy Gear's came");
    assert_eq!(s.strength(), base_strength + kb("WEP_KEYBLADE_SO_05").stats(2).strength);
    s.switch_keyblade(true).unwrap();
    assert_eq!((s.has(A::FpConverter), s.has(A::StunGuard), s.weapon_upgrade), (false, true, 3));
    s.switch_keyblade(true).unwrap();
    assert_eq!((s.current_keyblade(), s.abilities.worn(A::PrizeDraw), s.has(A::StunGuard)), (Some(KeybladeId::KINGDOM_KEY), 1, false), "wrapped to slot 0");
    s.switch_keyblade(false).unwrap();
    assert_eq!(s.current_keyblade(), Some(kb("WEP_KEYBLADE_SO_08")), "previous wraps to slot 2");
    // A level is kept per Keyblade across switches.
    s.set_keyblade_level(kb("WEP_KEYBLADE_SO_08"), 7);
    s.switch_keyblade(true).unwrap();
    s.switch_keyblade(false).unwrap();
    assert_eq!((s.weapon_upgrade, s.keyblade_level(KeybladeId::KINGDOM_KEY)), (7, 0));
    // Equipped abilities and their AP do not move: Keyblade abilities are worn copies (no AP).
    assert_eq!(s.ap_in_use(), level99(&[]).ap_in_use());
}

#[test]
fn the_keyblade_only_abilities_are_reachable() {
    // The plan 1.3: Focus Converter, Stun Protection, Situation Boost, Magic Roulette, Harvester, Wizard's Ruse come only
    // with a Keyblade (base abilities); Chef Extraordinaire (CHARISMA_CHEF) is Grand Chef's +3 level ability: shown and
    // counted, with no effect (no cooking in the port).
    for (key, level, kind) in [
        ("WEP_KEYBLADE_SO_05", 0, A::FpConverter),
        ("WEP_KEYBLADE_SO_08", 0, A::StunGuard),
        ("WEP_KEYBLADE_SO_14", 0, A::FormUp),
        ("WEP_KEYBLADE_SO_18", 0, A::MagicRoulette),
        ("WEP_KEYBLADE_SO_07", 0, A::Harvest),
        ("WEP_KEYBLADE_SO_10", 0, A::WizzardStar),
        ("WEP_KEYBLADE_SO_10", 3, A::CharismaChef),
    ] {
        let mut s = with_slots(&[key]);
        assert!(!tables::is_excluded(kind), "{kind:?} is a port ability");
        s.set_keyblade_level(kb(key), level);
        assert!(s.has(kind), "{key} +{level} gives {kind:?}");
        assert_eq!(s.ap_in_use(), level99(&[]).ap_in_use(), "worn: no AP");
    }
    // Harvester and Chef Extraordinaire have nothing to act on in the port: their help gets the COMPOSED note.
    assert_eq!((kh_core::status::keyblade::help_note(A::Harvest), kh_core::status::keyblade::help_note(A::CharismaChef), kh_core::status::keyblade::help_note(A::FpConverter)), (Some("(no effect in this world)"), Some("(no effect in this world)"), None));
    // Grand Chef below +3 does not have it (its init level is 2).
    let s = with_slots(&["WEP_KEYBLADE_SO_10"]);
    assert_eq!((s.weapon_upgrade, s.has(A::CharismaChef)), (2, false));
}

#[test]
fn slots_refuse_removing_the_current_keyblade_and_hold_each_keyblade_once() {
    let mut s = with_slots(&["WEP_KEYBLADE_SO_00", "WEP_KEYBLADE_SO_01"]);
    assert!(s.set_keyblade_slot(0, None).is_err(), "KH3 dialog_0001: the current Keyblade cannot be removed");
    s.set_keyblade_slot(2, Some(kb("WEP_KEYBLADE_SO_02"))).unwrap();
    s.set_keyblade_slot(2, Some(KeybladeId::KINGDOM_KEY)).unwrap();
    assert_eq!(s.keyblades().slots(), [Some(kb("WEP_KEYBLADE_SO_02")), Some(kb("WEP_KEYBLADE_SO_01")), Some(KeybladeId::KINGDOM_KEY)], "one of each: the two slots changed places");
    assert_eq!((s.weapon.as_str(), s.current_keyblade()), ("WEP_KEYBLADE_SO_02", Some(kb("WEP_KEYBLADE_SO_02"))), "the active slot now holds Shooting Star");
    // An old-style write of `weapon` puts it into the active slot.
    s.weapon = "WEP_KEYBLADE_SO_01".to_string();
    s.rebuild();
    assert_eq!(s.keyblades().slots(), [Some(kb("WEP_KEYBLADE_SO_01")), Some(kb("WEP_KEYBLADE_SO_02")), Some(KeybladeId::KINGDOM_KEY)]);
}

#[test]
fn the_save_interface_round_trips_and_the_status_copies() {
    let mut s = with_slots(&["WEP_KEYBLADE_SO_12", "", "WEP_KEYBLADE_SO_00"]);
    s.set_keyblade_level(kb("WEP_KEYBLADE_SO_12"), 6);
    s.set_keyblade_level(KeybladeId::KINGDOM_KEY, 4);
    s.switch_keyblade(true).unwrap();
    let k = *s.keyblades();
    let slots = k.slot_keys();
    let levels = k.levels_by_key();
    assert_eq!((slots, k.current_slot(), levels.len()), ([Some("WEP_KEYBLADE_SO_12"), None, Some("WEP_KEYBLADE_SO_00")], 2, 17));
    let (back, notes) = Keyblades::from_keys(&slots, k.current_slot(), &levels);
    assert_eq!((back, notes.len()), (k, 0));
    let mut t = level99(&[]);
    t.set_keyblades(back);
    assert_eq!((t.weapon.as_str(), t.weapon_upgrade, t.strength(), t.magic()), (s.weapon.as_str(), 4, s.strength(), s.magic()));
    // Unknown keys are said, not guessed.
    let (_, notes) = Keyblades::from_keys(&[Some("NO_SUCH_KEYBLADE")], 0, &[("WEP_DONALD_00", 3)]);
    assert_eq!(notes.len(), 2);
    // Mimic Sora copies the status: the copy has the same Keyblades and does not follow later changes.
    let copy = s.clone();
    s.switch_keyblade(true).unwrap();
    assert_ne!(copy.keyblades(), s.keyblades());
    assert_eq!(copy.weapon.as_str(), "WEP_KEYBLADE_SO_00");
}

#[test]
fn the_loadout_rows_set_slots_current_and_levels() {
    let mut levels = LoadoutValue::Map(vec![("WEP_KEYBLADE_SO_01".to_string(), 5.0), ("WEP_KEYBLADE_SO_14".to_string(), 0.0)]);
    let s = level99(&[("keyblades", list(&["WEP_KEYBLADE_SO_00", "WEP_KEYBLADE_SO_01", "WEP_KEYBLADE_SO_14"])), ("keyblade.current", LoadoutValue::Number(1.0)), ("keyblade.levels", levels.clone())]);
    assert_eq!((s.weapon.as_str(), s.weapon_upgrade, s.keyblade_level(kb("WEP_KEYBLADE_SO_14"))), ("WEP_KEYBLADE_SO_01", 5, 10), "Ultima Weapon is never below +10");
    assert!(s.notes.is_empty(), "{:?}", s.notes);
    levels = LoadoutValue::Map(vec![("NOPE".to_string(), 1.0)]);
    let s = level99(&[("keyblade.levels", levels)]);
    assert_eq!((s.current_keyblade(), s.notes.len()), (Some(KeybladeId::KINGDOM_KEY), 1));
    assert!(Loadout::new("t").set("keyblades", &list(&["a", "b", "c", "d"])).is_err(), "at most three slots");
}

// ------------------------------------------------------------------------------------ the command on the pad

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    held: PadButtons,
}

impl Bench {
    fn new(status: Status, dummies: &[TargetInfo]) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut host = RecordingHost::new();
        host.targets.extend_from_slice(dummies);
        let mut bench = Bench { sora, host, world: TestWorld::floor(), held: PadButtons::NONE };
        for _ in 0..5 {
            bench.step(&[]);
        }
        bench
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let pad = PadState::from_held(now, self.held);
        self.held = now;
        let input = Input::default().with_pad(pad);
        let (sora, world) = (&mut self.sora, &self.world);
        self.host.step(world, |h| sora.step_hosted(DT, &input, h))
    }

    /// Press and release; the outputs of both steps.
    fn tap(&mut self, button: PadButton) -> Vec<Output> {
        vec![self.step(&[button]), self.step(&[])]
    }

    /// Empty steps until the state is `id` again (at most `max`); every output on the way.
    fn until(&mut self, id: StateId, max: usize) -> Vec<Output> {
        let mut outs = Vec::new();
        for _ in 0..max {
            let out = self.step(&[]);
            let done = out.state == id;
            outs.push(out);
            if done {
                return outs;
            }
        }
        panic!("the state never became {id:?}");
    }

    /// A tap of D-pad right / left and the steps until Idle again: the outputs.
    fn switch(&mut self, button: PadButton) -> Vec<Output> {
        let mut outs = self.tap(button);
        outs.extend(self.until(StateId::Idle, 30));
        outs
    }
}

fn spawned(outs: &[Output]) -> Vec<&'static str> {
    outs.iter().flat_map(|o| o.events.iter()).filter_map(|e| match &e.kind { EventKind::EffectSpawn { spawn, .. } => Some(spawn.name), _ => None }).collect()
}

fn states(outs: &[Output]) -> Vec<StateId> {
    let mut seen: Vec<StateId> = Vec::new();
    for o in outs {
        if seen.last() != Some(&o.state) {
            seen.push(o.state);
        }
    }
    seen
}

fn dummy(id: u32, x: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

#[test]
fn dpad_on_attack_runs_kh3s_weapon_change_state() {
    // KH3's game help detail_01400_00: "move the cursor to Attack on the Command Menu and then use <left / right>";
    // the request (0x1414426E0) out of IDLE: SetState(WEAPONCHANGE 32); its Tick makes the switch and draws the weapon.
    let mut b = Bench::new(with_slots(&["WEP_KEYBLADE_SO_00", "WEP_KEYBLADE_SO_13"]), &[]);
    assert_eq!((b.sora.command().main_cursor, b.sora.actions().weapon_out), (0, false));
    let outs = b.switch(PadButton::DPadRight);
    assert_eq!(states(&outs), vec![StateId::Idle, StateId::Weaponchange, StateId::Idle], "Idle -> WEAPONCHANGE (two ticks) -> Idle");
    assert_eq!((b.sora.status().current_keyblade(), b.sora.status().has(A::HpConverter)), (Some(kb("WEP_KEYBLADE_SO_13")), true), "Oblivion's HP Converter counts");
    // The weapon was away: no disappear / appear pair, the Draw of the tick shows Oblivion with ITS appear effect.
    assert_eq!(spawned(&outs), vec!["vw_so210_app0"]);
    assert!(b.sora.actions().weapon_out);
    // Idle Begin after a switch (pawn+0x1460 bit 0): BattleStart, no carry timer out of Idle.
    assert_eq!((b.sora.output().anim.name, b.sora.magic().combo_keep), (AnimId::BattleStart, 0.0));
    // Back with D-pad left, the weapon out now: the old Keyblade's m_DisappearEffect, then the new one's m_AppearEffect.
    let outs = b.switch(PadButton::DPadLeft);
    assert_eq!((b.sora.status().current_keyblade(), spawned(&outs)), (Some(KeybladeId::KINGDOM_KEY), vec!["vw_so210_app0", "VW_SO010_APP0"]));
    assert_eq!(b.sora.status().keyblade_switches, 2);
    // Not on Magic: D-pad right opens the Magic list there; the Keyblade stays.
    b.tap(PadButton::DPadDown);
    let outs = b.tap(PadButton::DPadRight);
    assert_eq!((b.sora.status().current_keyblade(), b.sora.status().keyblade_switches, states(&outs)), (Some(KeybladeId::KINGDOM_KEY), 2, vec![StateId::Idle]));
}

#[test]
fn with_the_kingdom_key_alone_the_dpad_on_attack_does_nothing() {
    let mut b = Bench::new(level99(&[]), &[]);
    let before = b.sora.status().clone();
    let mut outs = b.tap(PadButton::DPadRight);
    outs.extend(b.tap(PadButton::DPadLeft));
    assert!(spawned(&outs).is_empty() && states(&outs) == vec![StateId::Idle]);
    assert_eq!((b.sora.status().keyblade_switches, b.sora.status().weapon.as_str(), b.sora.status().strength()), (0, "WEP_KEYBLADE_SO_00", before.strength()));
}

/// Attacks the dummy until the attack has landed a hit; the step count.
fn hit_once(b: &mut Bench) {
    b.step(&[PadButton::ATTACK]);
    for _ in 0..60 {
        b.step(&[]);
        if b.sora.combat().hit_flag && b.sora.attack().is_some_and(|a| a.enable_input) {
            return;
        }
    }
    panic!("no hit landed");
}

#[test]
fn a_switch_out_of_a_hit_combo_carries_the_combo_for_half_a_second() {
    // The request out of an attack: PushState(32) with +0x8C = the hit flag; the attack ends at TresEnableChange (a state
    // is queued), its End hands the counters over without ending the chain; the state's tick sets the carry timer
    // pawn+0x10E4 = 0.5 s; TryAttack out of IDLE with the timer running: combo = pawn+0xE18 + 1 (combat spec 3.3).
    let mut b = Bench::new(with_slots(&["WEP_KEYBLADE_SO_00", "WEP_KEYBLADE_SO_01"]), &[dummy(1, 120.0)]);
    hit_once(&mut b);
    let combo = b.sora.attack().unwrap().combo;
    let mut outs = b.tap(PadButton::DPadRight);
    assert_eq!(b.sora.status().current_keyblade(), Some(KeybladeId::KINGDOM_KEY), "queued behind the attack");
    outs.extend(b.until(StateId::Idle, 120));
    assert!(states(&outs).contains(&StateId::Weaponchange));
    assert_eq!(b.sora.status().current_keyblade(), Some(kb("WEP_KEYBLADE_SO_01")));
    let keep = b.sora.magic().combo_keep;
    assert!(keep > 0.4 && keep <= 0.5, "carry timer {keep}");
    assert_eq!(b.sora.combat().combo, combo, "the chain did not end");
    b.step(&[PadButton::ATTACK]);
    b.step(&[]);
    assert_eq!(b.sora.attack().map(|a| a.combo), Some(combo + 1), "the combo goes on with the new Keyblade");
    b.step(&[]);
    // The timer is cleared once a state other than 1..3 / 32 runs (pawn tick 0x14144A530).
    assert_eq!(b.sora.magic().combo_keep, 0.0);
}

#[test]
fn a_switch_without_a_hit_ends_the_combo_and_the_carry_runs_out() {
    // No dummy and no Combo Master: no hit; the request sets pawn+0xE18 = 0 and +0x8C = 0: no carry timer.
    let mut status = with_slots(&["WEP_KEYBLADE_SO_00", "WEP_KEYBLADE_SO_01"]);
    while status.unequip(A::ComboMaster) {}
    let mut b = Bench::new(status, &[]);
    b.step(&[PadButton::ATTACK]);
    for _ in 0..8 {
        b.step(&[]);
    }
    assert!(b.sora.attack().is_some() && !b.sora.combat().hit_flag);
    b.tap(PadButton::DPadRight);
    b.until(StateId::Idle, 120);
    assert_eq!((b.sora.status().current_keyblade(), b.sora.magic().combo_keep, b.sora.combat().combo), (Some(kb("WEP_KEYBLADE_SO_01")), 0.0, 0));
    b.step(&[PadButton::ATTACK]);
    b.step(&[]);
    assert_eq!(b.sora.attack().map(|a| a.combo), Some(0), "a new combo");
}

#[test]
fn the_switch_is_refused_in_rage_form() {
    // CanWeaponChange 0x1414300E0: controller+0xB38 == CHANGE_AN (0x8B) refuses (controller+0xB78 bit 3, the form's
    // time up, too: the form framework raises and drops that bit within its own tick).
    let mut b = Bench::new(with_slots(&["WEP_KEYBLADE_SO_00", "WEP_KEYBLADE_SO_01"]), &[]);
    b.sora.combat_mut().form.cmd = 0x8B;
    let outs = b.tap(PadButton::DPadRight);
    assert_eq!((states(&outs), b.sora.status().current_keyblade()), (vec![StateId::Idle], Some(KeybladeId::KINGDOM_KEY)));
    b.sora.combat_mut().form.cmd = 0;
    b.switch(PadButton::DPadRight);
    assert_eq!(b.sora.status().current_keyblade(), Some(kb("WEP_KEYBLADE_SO_01")));
}

#[test]
fn a_formchange_stays_with_its_keyblade_slot_across_switches() {
    // ChangeWeapon 0x14144A9F0 with 0x14141D9D0 / 0x14141E2B0: Second Form is stored in the Kingdom Key's slot (time left,
    // finish level), taken off while Hero's Origin is in hand (no form, wear form NORMAL), and back with its stored time
    // and its finish command when the Kingdom Key is current again (SetStyleFinishCommand(0, bWeaponChange): >= 1.0 s).
    let mut b = Bench::new(with_slots(&["WEP_KEYBLADE_SO_00", "WEP_KEYBLADE_SO_01"]), &[]);
    b.sora.debug_add_style(60.0);
    for _ in 0..2 {
        b.step(&[]);
    }
    b.sora.debug_add_style(40.0);
    for _ in 0..15 {
        b.step(&[]);
    }
    b.step(&[PadButton::SITUATION]);
    b.until(StateId::Stylechange, 30);
    b.until(StateId::Idle, 400);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form), (0x94, 5), "in Second Form");
    let gauge = b.sora.combat().style;
    b.switch(PadButton::DPadRight);
    let held = b.sora.status().keyblades().slot_form(0).expect("the Kingdom Key's slot holds the form");
    let f = b.sora.combat().form;
    assert_eq!((b.sora.status().current_keyblade(), f.cmd, f.wear_form, held.cmd), (Some(kb("WEP_KEYBLADE_SO_01")), 0, 0, 0x94));
    assert!(held.time_left > 0.0 && held.time_left <= held.time_full);
    assert_eq!(b.sora.combat().style.points, gauge.points, "the formchange gauge is the controller's: it stays");
    // The stored time does not run while the other Keyblade is in hand.
    for _ in 0..60 {
        b.step(&[]);
    }
    assert_eq!(b.sora.status().keyblades().slot_form(0).map(|r| r.time_left), Some(held.time_left));
    b.switch(PadButton::DPadLeft);
    let f = b.sora.combat().form;
    assert_eq!((b.sora.status().current_keyblade(), f.cmd, f.level, f.wear_form, f.finish_pending), (Some(KeybladeId::KINGDOM_KEY), 0x94, held.level, 5, true));
    assert!(f.time_left >= held.time_left.max(1.0) - 0.2, "time back: {} (stored {})", f.time_left, held.time_left);
    assert!(b.sora.status().keyblades().slot_form(0).is_none(), "taken back");
}

// ------------------------------------------------------------------------------------ (P2 hitfx) each Keyblade's own hits

/// What the core spawned / played around each hit of a run: (attack row, attribute index, effects, sounds) per Hit event,
/// the effect spawns and sound plays of the same step before it.
fn hits_of(outs: &[Output]) -> Vec<(&'static str, usize, Vec<&'static str>, Vec<&'static str>)> {
    let mut out = Vec::new();
    for o in outs {
        let (mut fx, mut se) = (Vec::new(), Vec::new());
        for e in &o.events {
            match &e.kind {
                EventKind::EffectSpawn { spawn, .. } => fx.push(spawn.name),
                EventKind::SoundPlay { play, .. } => se.push(play.name),
                EventKind::Hit { record, .. } => {
                    out.push((record.attack_name, record.attribute.value() as usize, std::mem::take(&mut fx), std::mem::take(&mut se)));
                }
                _ => {}
            }
        }
    }
    out
}

/// A full ground combo on a dummy with the Keyblade `key` alone in his hand: every step's output.
fn combo_with(key: &str) -> (Bench, Vec<Output>) {
    let mut b = Bench::new(with_slots(&[key]), &[dummy(1, 120.0)]);
    let mut outs = Vec::new();
    for i in 0..240 {
        outs.push(b.step(if i % 2 == 0 { &[PadButton::ATTACK] } else { &[] }));
    }
    (b, outs)
}

/// The chooser's expectation from the sheet row `hit.<blueprint>.None`, written out (not through the core's code).
fn expected(key: &str, attribute: usize) -> (&'static str, Option<&'static str>) {
    let bp = kb(key).row().blueprint.rsplit('/').next().unwrap();
    let row = kh_core::status::keyblade::SORA_KEYBLADES.iter().find(|r| r.kind == "hit" && r.blueprint.eq_ignore_ascii_case(bp) && r.group == "None").unwrap_or_else(|| panic!("{key}: no hit.{bp}.None row"));
    let name = |a: &'static str| (a != "-").then(|| a.rsplit('/').next().unwrap());
    (row.effects.get(attribute).copied().and_then(name).or_else(|| name(row.asset)).expect("an effect"), name(row.direct))
}

#[test]
fn every_keyblade_hits_with_its_own_kh3_hit_effects_and_sound_set() {
    // KH3's chooser 0x140628960 on the hit window's component (docs/sora-combat-spec.md 8.6): the held Keyblade's own
    // TresAtkColl0 m_HitEffects entry "None": m_PawnHitEffectPerAttr[attribute] or m_PawnHitEffect, plus m_DirectHitEffect on
    // a phys shape (the blade). The window's HITn alias plays from THAT weapon's hit set (Status::attack_model).
    // A normal hit (PHYSICAL) per Keyblade, from the blueprints (w_soNNN_Wep), written out:
    let normal = [
        ("WEP_KEYBLADE_SO_00", "vw_so010_hit0", "vw_so010_str0", "w_so010"),
        ("WEP_KEYBLADE_SO_01", "vw_so030_hit", "vw_so030_str", "w_so030"),
        ("WEP_KEYBLADE_SO_02", "VW_SO020_HIT0", "vw_so020_str0", "w_so020"),
        ("WEP_KEYBLADE_SO_03", "vw_so050_hit0", "vw_so050_str0", "w_so050"),
        ("WEP_KEYBLADE_SO_04", "vw_so070_hit2", "vw_so070_str0", "w_so070"),
        ("WEP_KEYBLADE_SO_05", "vw_so060_hit0", "vw_so060_str0", "w_so060"),
        ("WEP_KEYBLADE_SO_06", "vw_so080_hit0", "vw_so080_str0", "w_so080"),
        ("WEP_KEYBLADE_SO_07", "vw_so040_hit0", "vw_so040_str", "w_so040"),
        ("WEP_KEYBLADE_SO_08", "vw_so170_hit00", "vw_so170_str00", "w_so170"),
        ("WEP_KEYBLADE_SO_09", "vw_so090_hit0", "vw_so090_str", "w_so090"),
        ("WEP_KEYBLADE_SO_10", "vw_so140_hit0", "vw_so140_str0", "w_so140"),
        ("WEP_KEYBLADE_SO_11", "vw_so110_hit00", "vw_so110_str00", "w_so110"),
        ("WEP_KEYBLADE_SO_12", "vw_ex090_hit0", "vw_ex090_str0", "w_so200"),
        ("WEP_KEYBLADE_SO_13", "vw_ex100_hit0", "vw_ex100_str0", "w_so210"),
        ("WEP_KEYBLADE_SO_14", "vw_so190_hit0", "vw_so190_str0", "w_so190"),
        ("WEP_KEYBLADE_SO_17", "vw_so150_hit0", "vw_so150_str0", "w_so150"),
        ("WEP_KEYBLADE_SO_18", "vw_so120_hit00", "vw_so120_str00", "w_so121"),
    ];
    assert_eq!(normal.len(), 17);
    for (key, hit, direct, model) in normal {
        assert_eq!(expected(key, 0), (hit, Some(direct)), "{key}: the sheet row");
        let (b, outs) = combo_with(key);
        let hits = hits_of(&outs);
        assert!(hits.len() >= 3, "{key}: a combo lands ({} hits)", hits.len());
        // The first (normal) hit: its own spark and its blade's direct effect, never the Kingdom Key's (unless it is the KK).
        let (row, attribute, fx, se) = &hits[0];
        assert_eq!(*attribute, 0, "{key} {row}: a PHYSICAL swing");
        assert!(fx.contains(&hit) && fx.contains(&direct), "{key} {row}: {fx:?}");
        if key != "WEP_KEYBLADE_SO_00" {
            assert!(!fx.iter().any(|f| f.eq_ignore_ascii_case("vw_so010_hit0") || f.eq_ignore_ascii_case("vw_so010_str0")), "{key}: no Kingdom Key spark: {fx:?}");
        }
        assert!(se.iter().any(|s| s.starts_with("HIT") || s.starts_with("se")), "{key} {row}: the window's hit sound {se:?}");
        assert_eq!(b.sora.status().attack_model, model, "{key}: the hit / swing sets of its own model");
        // The finisher (the combo's last hit): the same entry, by its row's attribute.
        let (row, attribute, fx, _) = hits.last().unwrap();
        let (want, want_direct) = expected(key, *attribute);
        assert!(fx.contains(&want), "{key} finisher {row} (attribute {attribute}): {fx:?}, want {want}");
        if let Some(d) = want_direct {
            assert!(fx.contains(&d), "{key} finisher {row}: {fx:?}, want the direct {d}");
        }
        println!("{key}: normal {} -> {:?}; finisher {row} (attr {attribute}) -> {fx:?}; sound set {model}", hits[0].0, hits[0].2);
    }
}

#[test]
fn every_hit_effect_the_keyblades_name_is_in_an_effect_root() {
    // The effects the hit rows of the 17 Keyblades name (their blueprints and form parts) are packed in one of the roots the
    // effect side reads (vfx, party/vfx, xsi/vfx, keyblades/vfx). Skips, saying so, without the pack.
    let pack = format!("{}/../../build/sora_pack", env!("CARGO_MANIFEST_DIR"));
    let mut have = std::collections::HashSet::new();
    for root in ["vfx", "party/vfx", "xsi/vfx", "keyblades/vfx"] {
        if let Ok(dir) = std::fs::read_dir(format!("{pack}/{root}/effects")) {
            for f in dir.flatten() {
                have.insert(f.file_name().to_string_lossy().trim_end_matches(".json").to_ascii_lowercase());
            }
        }
    }
    if have.is_empty() {
        eprintln!("SKIPPED: no effect pack at {pack}");
        return;
    }
    let mut missing = Vec::new();
    for id in KeybladeId::all() {
        for name in kh_core::status::keyblade::effect_names(id) {
            if name.starts_with("ead_") {
                continue; // attach data assets (the cast / charge / bad-status groups), not particle systems
            }
            if !have.contains(&name.to_ascii_lowercase()) {
                missing.push(format!("{}: {name}", id.key()));
            }
        }
    }
    assert!(missing.is_empty(), "{missing:?}");
}

#[test]
fn form_weapons_and_override_groups_take_their_own_entries() {
    use kh_core::status::keyblade::{attack_weapons, hit_source, HitSource};
    // Hyper Hammer's weapon part (w_so052_Wep): "None" -> Favorite Deputy's vw_so050_hit0; its STAMP group (the notify's
    // m_HitEffectGrpName) -> vw_so052_jwr_hit0. Not the Kingdom Key's, though the Kingdom Key is in another slot.
    let units = ["RWeapon@RIGHT_HAND:R_wep_skt=/Game/Blueprints/Player/p_ex001/w_so052_Wep"];
    let fd = Some(kb("WEP_KEYBLADE_SO_03"));
    let weapons = attack_weapons(&units, fd);
    assert_eq!(weapons, vec!["w_so052_Wep"]);
    assert_eq!(hit_source(&weapons, false, None).effect(0, false), Some("vw_so050_hit0"));
    assert_eq!(hit_source(&weapons, false, Some("STAMP")).effect(0, false), Some("vw_so052_jwr_hit0"));
    // Without a form: the held Keyblade's own blueprint; its map hit (m_bChangeMapHit): Oblivion's vw_so100_wht0.
    let oblivion = attack_weapons(&[], Some(kb("WEP_KEYBLADE_SO_13")));
    assert_eq!(hit_source(&oblivion, false, None).effect(0, true), Some("vw_so100_wht0"));
    // Second Form's Magnet Burst groups and Starlight's Union hit come from the weapon's own groups.
    assert_eq!(hit_source(&attack_weapons(&[], Some(KeybladeId::KINGDOM_KEY)), false, Some("MagneSplash1")).effect(0, false), Some("vw_lmt_mgs_hit00"));
    assert_eq!(hit_source(&attack_weapons(&[], Some(kb("WEP_KEYBLADE_SO_17"))), false, Some("UnionHit1")).effect(0, false), Some("vw_so150_udis_hit0"));
    // The pawn's own groups (Rage Form's AN_LHAND) with an override: the pawn's component.
    assert_eq!(hit_source(&oblivion, true, Some("AN_LHAND")).effect(0, false), Some("vp_ant_lhnd_hit00"));
    // A group no component has: no effect (KH3: no entry, nothing spawns).
    assert_eq!(hit_source(&oblivion, false, Some("NO_SUCH_GROUP")), HitSource::NoEntry);
    // Every Keyblade's own blueprint has its "None" entry.
    for id in KeybladeId::all() {
        assert!(matches!(hit_source(&attack_weapons(&[], Some(id)), false, None), HitSource::Entry(_)), "{}", id.key());
    }
}
