//! (weapons P1) KH3'S WEAPONS AS ITEMS, AND WHAT THE SAVE KEEPS OF THEM (documentation/KH3_WEAPONS_PLAN.md 1.2; the
//! user 2026-11-02: "Add all of the keyblades from KH3 ... There is only 1 of each keyblade in the game (no duplicates)
//! but Donald and Goofy can get duplicates").
//!
//! - THE TABLE: the 45 `weapon` rows of the item table (sheets/port/sora-item-table.json, written by
//!   tools/sora_item_table.js from the WD sheets kh3-keyblades / kh3-party-weapons): 17 Keyblades (`m_MaxNum` 1), Donald's
//!   14 staves and Goofy's 14 shields (`m_MaxNum` 99). They are not rows of [`crate::items::table`] (no `items::Class`
//!   for them: the item screens and the shop list only that table).
//! - THE DOOR: [`give`] (and `crate::items::give`, which sends a weapon here). A KEYBLADE ALREADY OWNED IS REFUSED
//!   ([`Gift::AlreadyOwned`]: nothing is queued, no popup; the caller still marks its source done). A new Keyblade is owned
//!   at once, at its Forge level when obtained (KH3's `m_bInitAchieved` rows: kh3-keyblades `init_level`), and queued
//!   for the HUD (the camp menu's stock, the popup). A friend's weapon is queued like any item: the stock holds up to
//!   [`STACK_MAX`] of each (KH3's `m_MaxNum` 99).
//! - THE STATE ([`WeaponsState`], one per running character): the Keyblades owned and their Forge levels, Sora's three
//!   Keyblade slots and the current one, and the Moogle Workshop's record ([`Workshop`]: goals, photo missions, the
//!   recipes' "new" marks seen, made counts, materials ever obtained, Moogle level ...). The save keeps it as its version-6
//!   block (`crate::save`). THE HOST moves it, as it moves the party record: after `SoraSave::capture` it calls
//!   [`SoraSave::take_running_weapons`](crate::save::SoraSave::take_running_weapons), beside `SoraSave::apply` it calls
//!   [`SoraSave::run_weapons`](crate::save::SoraSave::run_weapons), and where it builds a new game instead it calls
//!   [`begin_new_game`] (else the last character's Keyblades stay).
//!
//! Engine-free; the hosts' screens (P6b, P11) and acquisition (P9b) read and write it through the functions here.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::{json, Value};

/// The Keyblade Sora starts with (`TresChrInitData` m_PlayerSora.m_Weapons): always owned.
pub const KINGDOM_KEY: &str = "WEP_KEYBLADE_SO_00";
/// Sora's Keyblade slots (KH3's Equipment screen: three, one of them in hand).
pub const KEYBLADE_SLOTS: usize = 3;
/// The highest Forge level of every Keyblade (WD: "Max level = the highest row (10 for all)").
pub const MAX_LEVEL: u8 = 10;
/// How many of one friend's weapon the stock holds (`m_MaxNum` 99 of every staff and shield).
pub const STACK_MAX: u32 = 99;
/// KH3's Moogle shop levels (plan 6c: 7 levels, 6 raises; `kh3-workshop-rules`: clamped 1..99, only ever raised).
pub const MOOGLE_LEVEL_MAX: u8 = 7;
/// The record a boss's Keyblade leaves once given (`kb:<boss row>`, plan 1.2; P9b gives it, the catch-up skips it).
pub const KB_RECORD_PREFIX: &str = "kb:";
/// The record a chest's / drop's weapon leaves once given (`wsrc:<flag>`, plan 1.2).
pub const WSRC_RECORD_PREFIX: &str = "wsrc:";

/// Whose weapon it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Chara {
    Sora,
    Donald,
    Goofy,
}

/// One weapon row of the item table.
#[derive(Clone, Debug, PartialEq)]
pub struct Weapon {
    /// `m_KeyName`: WEP_KEYBLADE_SO_00, WEP_DONALD_03, WEP_GOOFY_13.
    pub key: String,
    /// 0x300 + the row's index (I03001 = 0x301 Kingdom Key; I03024 = 0x318 Warhammer+): what a prize box / reward holds.
    pub item_id: u32,
    pub chara: Chara,
    pub name: String,
    pub buy: u32,
    pub sell: u32,
    pub can_sell: bool,
    /// `m_MaxNum`: 1 for a Keyblade, 99 for a friend's weapon.
    pub max: u32,
    /// A Keyblade's Forge level when obtained (0 for the friends' weapons: they have no Forge).
    pub init_level: u8,
    /// TresItemData row (I03001 ...).
    pub row: String,
}

impl Weapon {
    pub fn is_keyblade(&self) -> bool {
        self.chara == Chara::Sora
    }
}

static TABLE: OnceLock<Vec<Weapon>> = OnceLock::new();

fn parse(text: &str) -> Vec<Weapon> {
    let Ok(json) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    let column = |name: &str| json["columns"].as_array().and_then(|c| c.iter().position(|v| v.as_str() == Some(name)));
    let (Some(id), Some(item_id), Some(kind), Some(name), Some(buy), Some(sell), Some(max), Some(can_sell), Some(row), Some(chara), Some(init)) =
        (column("id"), column("item_id"), column("kind"), column("name"), column("buy"), column("sell"), column("max"), column("can_sell"), column("row"), column("chara"), column("init_level"))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for r in json["rows"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        if r[kind].as_str() != Some("weapon") {
            continue;
        }
        let chara = match r[chara].as_str() {
            Some("sora") => Chara::Sora,
            Some("donald") => Chara::Donald,
            Some("goofy") => Chara::Goofy,
            _ => continue,
        };
        let (Some(key), Some(item)) = (r[id].as_str(), r[item_id].as_u64()) else { continue };
        let number = |k: usize| u32::try_from(r[k].as_u64().unwrap_or(0)).unwrap_or(u32::MAX);
        out.push(Weapon {
            key: key.to_string(),
            item_id: u32::try_from(item).unwrap_or(0),
            chara,
            name: r[name].as_str().unwrap_or(key).to_string(),
            buy: number(buy),
            sell: number(sell),
            can_sell: number(can_sell) != 0,
            max: number(max).max(1),
            init_level: u8::try_from(number(init)).unwrap_or(MAX_LEVEL).min(MAX_LEVEL),
            row: r[row].as_str().unwrap_or("-").to_string(),
        });
    }
    out
}

/// Every weapon, in TresItemData's order (the 17 Keyblades, then Donald's staves, then Goofy's shields).
pub fn table() -> &'static [Weapon] {
    TABLE.get_or_init(|| parse(crate::items::ITEM_TABLE_SHEET))
}

pub fn by_id(item_id: u32) -> Option<&'static Weapon> {
    table().iter().find(|w| w.item_id == item_id)
}

pub fn by_key(key: &str) -> Option<&'static Weapon> {
    table().iter().find(|w| w.key.eq_ignore_ascii_case(key))
}

/// By key, by English name ("Hero's Origin", "Hero's_Origin"), or by item id ("0x302", "770"): the weapons' twin of
/// `crate::items::find` (a host's test command).
pub fn find(what: &str) -> Option<&'static Weapon> {
    let what = what.trim();
    let wanted = what.replace('_', " ");
    let id = what.strip_prefix("0x").or_else(|| what.strip_prefix("0X")).map_or_else(|| what.parse::<u32>().ok(), |hex| u32::from_str_radix(hex, 16).ok());
    by_key(what).or_else(|| table().iter().find(|w| w.name.eq_ignore_ascii_case(&wanted))).or_else(|| id.and_then(by_id))
}

pub fn is_keyblade(key: &str) -> bool {
    by_key(key).is_some_and(Weapon::is_keyblade)
}

/// (plan 1.2: "only KH3's 60 materials count for goals (not gloveworts)") MAT_ITEM01 .. MAT_ITEM60: the materials the
/// Collector's Goals count. The COMPOSED gloveworts (MAT_GLOVE_*) and anything else are not.
pub fn is_goal_material(key: &str) -> bool {
    key.strip_prefix("MAT_ITEM").filter(|n| n.len() == 2).and_then(|n| n.parse::<u8>().ok()).is_some_and(|n| (1..=60).contains(&n))
}

/// The record of a boss's Keyblade given (`kb:<boss row id>`).
pub fn kb_record(boss: &str) -> String {
    format!("{KB_RECORD_PREFIX}{boss}")
}

/// The record of a chest's / drop's weapon given (`wsrc:<flag>`).
pub fn wsrc_record(flag: impl std::fmt::Display) -> String {
    format!("{WSRC_RECORD_PREFIX}{flag}")
}

// ------------------------------------------------------------------------------------------------ the state

/// A Keyblade Sora owns and its Forge level (0 = +0 .. [`MAX_LEVEL`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedKeyblade {
    pub key: String,
    pub level: u8,
}

/// (plan 1.2) THE MOOGLE WORKSHOP'S RECORD as KH3 keeps it (WD section 3; `kh3-workshop-rules`). P6a's rules
/// (`kh-host/src/workshop.rs`) read and write it; the save keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct Workshop {
    /// Collector's Goals achieved (IC_n), in the order they were achieved.
    pub goals: Vec<String>,
    /// Photo Missions done (IM_n; in the port: their Elden Ring milestones, plan 6a).
    pub photos: Vec<String>,
    /// Recipes (IS_n) whose "new" mark was seen (a newly listed recipe is "new" until the list is shown).
    pub recipes_seen: Vec<String>,
    /// Times each recipe (IS_n) was made (KH3's per-recipe count; a recipe with a count was "made once": the SYNTH LOG).
    pub created: BTreeMap<String, u32>,
    /// Items Created: every item synthesized, summed (the SYNTH LOG's count).
    pub items_created: u32,
    /// MATERIALS EVER OBTAINED by kind (MAT_ITEM01 .. 60 only): KH3 counts what was obtained, not what is held.
    pub obtained: BTreeMap<String, u32>,
    /// The total ever obtained (TOTAL_NUM: stock + used + carried).
    pub total_obtained: u32,
    /// Material ranks (1 Shard .. 4 Crystal / ore, the item table's grade) whose GET_RANK reward the player has seen.
    pub ranks_seen: Vec<u8>,
    /// Materials made buyable by a Collector's Goal (ENABLE_BUY: the key of the material).
    pub buy_unlocked: Vec<String>,
    /// The Moogle shop level, 1 .. [`MOOGLE_LEVEL_MAX`] (only ever raised).
    pub moogle_level: u8,
    /// The Workshop's own material stock (KH3: each visit moves the carried materials into it, cap 999); empty until
    /// P6a uses it.
    pub stock: BTreeMap<String, u32>,
    /// A package's own small state until it gets a field of its own (kept as it is; null = none).
    pub ext: Value,
}

impl Default for Workshop {
    fn default() -> Workshop {
        Workshop { goals: Vec::new(), photos: Vec::new(), recipes_seen: Vec::new(), created: BTreeMap::new(), items_created: 0, obtained: BTreeMap::new(), total_obtained: 0, ranks_seen: Vec::new(), buy_unlocked: Vec::new(), moogle_level: 1, stock: BTreeMap::new(), ext: Value::Null }
    }
}

impl Workshop {
    /// Ever obtained of one material.
    pub fn obtained_of(&self, key: &str) -> u32 {
        self.obtained.get(key).copied().unwrap_or(0)
    }

    /// Kinds of material ever obtained (KIND_NUM).
    pub fn kinds_obtained(&self) -> usize {
        self.obtained.values().filter(|n| **n > 0).count()
    }

    /// Raises the Moogle level to `level` (never lowers it; clamped to 1 .. [`MOOGLE_LEVEL_MAX`]).
    pub fn raise_moogle_level(&mut self, level: u8) {
        self.moogle_level = self.moogle_level.max(level.clamp(1, MOOGLE_LEVEL_MAX));
    }

    fn to_value(&self) -> Value {
        let counts = |m: &BTreeMap<String, u32>| m.iter().map(|(id, n)| json!({ "id": id, "count": n })).collect::<Vec<_>>();
        json!({
            "goals": self.goals, "photos": self.photos, "recipes_seen": self.recipes_seen, "created": counts(&self.created),
            "items_created": self.items_created, "obtained": counts(&self.obtained), "total_obtained": self.total_obtained,
            "ranks_seen": self.ranks_seen, "buy_unlocked": self.buy_unlocked, "moogle_level": self.moogle_level,
            "stock": counts(&self.stock), "ext": self.ext,
        })
    }

    fn from_value(v: &Value) -> Option<Workshop> {
        let strings = |name: &str| v[name].as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect::<Vec<_>>());
        let counts = |name: &str| v[name].as_array().map(|a| a.iter().filter_map(|e| Some((e["id"].as_str()?.to_string(), u32::try_from(e["count"].as_u64()?).ok()?))).collect::<BTreeMap<_, _>>());
        let number = |name: &str| v[name].as_u64().and_then(|n| u32::try_from(n).ok());
        Some(Workshop {
            goals: strings("goals")?,
            photos: strings("photos")?,
            recipes_seen: strings("recipes_seen")?,
            created: counts("created")?,
            items_created: number("items_created")?,
            obtained: counts("obtained")?,
            total_obtained: number("total_obtained")?,
            ranks_seen: v["ranks_seen"].as_array()?.iter().filter_map(|n| n.as_u64().and_then(|n| u8::try_from(n).ok())).collect(),
            buy_unlocked: strings("buy_unlocked")?,
            moogle_level: v["moogle_level"].as_u64().and_then(|n| u8::try_from(n).ok())?.clamp(1, MOOGLE_LEVEL_MAX),
            stock: counts("stock")?,
            ext: v["ext"].clone(),
        })
    }
}

/// (plan 1.2) EVERYTHING OF THE WEAPONS A CHARACTER'S SAVE KEEPS (the version-6 `weapons` block). Two records are equal
/// when they hold the same progress (`migrated_from` is information only).
#[derive(Clone, Debug)]
pub struct WeaponsState {
    /// The Keyblades owned, in the order obtained, each with its Forge level. The Kingdom Key is always among them.
    pub keyblades: Vec<OwnedKeyblade>,
    /// Sora's three Keyblade slots as set through [`set_slots`] (P2's switching, P11's Equipment screen); `None` = the
    /// slots are the save's own legacy fields (slot 1 = the weapon in hand, slots 2 / 3 = the two spare weapons), what
    /// the camp menu edits today.
    pub slots: Option<[Option<String>; KEYBLADE_SLOTS]>,
    /// The slot in hand (0 .. 2).
    pub current: u8,
    pub workshop: Workshop,
    /// The save version this record was made from by the migration (5 = a version-5 file; 0 = made by this code).
    /// Information only: it is not compared.
    pub migrated_from: u32,
}

impl PartialEq for WeaponsState {
    fn eq(&self, other: &WeaponsState) -> bool {
        (&self.keyblades, &self.slots, self.current, &self.workshop) == (&other.keyblades, &other.slots, other.current, &other.workshop)
    }
}

impl Default for WeaponsState {
    fn default() -> WeaponsState {
        WeaponsState::start()
    }
}

impl WeaponsState {
    /// A NEW GAME: the Kingdom Key at its level when obtained, in slot 1 (in hand), an empty Workshop at Moogle level 1.
    pub fn start() -> WeaponsState {
        WeaponsState { keyblades: vec![OwnedKeyblade { key: KINGDOM_KEY.to_string(), level: init_level(KINGDOM_KEY) }], slots: None, current: 0, workshop: Workshop::default(), migrated_from: 0 }
    }

    /// (plan 1.2) THE MIGRATION FROM A VERSION-5 (or older) SAVE, from its legacy fields: `weapon` / `weapon_upgrade` (the
    /// Keyblade in hand and its upgrade), the two spare weapons and the camp menu's stock.
    /// - Keyblades: the Kingdom Key (always), then every Keyblade in hand, in a spare slot or in the stock; each at its
    ///   level when obtained; the Kingdom Key in hand at `weapon_upgrade` if higher ("Kingdom Key level from
    ///   weapon_upgrade": before version 6 only the Kingdom Key had upgrade rows, and the upgrade is the weapon in hand's).
    /// - Slots: the legacy fields (`slots` None), slot 1 in hand.
    /// - Workshop: "ever obtained" BACK-FILLED FROM THE STOCK (each of KH3's 60 materials held counts as obtained that
    ///   often; the total is their sum); nothing achieved, Moogle level 1. The goals are evaluated on load by P6a's rules
    ///   (the load hook, [`set_load_hook`]); the boss / chest catch-up (P9b) finds no `kb:` / `wsrc:` record yet, so every
    ///   weapon due is given then. Friends' weapons already in the stock (from Elden Ring staves / shields) stay there.
    pub fn from_legacy(weapon: &str, weapon_upgrade: u8, spare: &[Option<String>], stock: &[(String, u32)], version: u32) -> WeaponsState {
        let mut state = WeaponsState { migrated_from: version, ..WeaponsState::start() }.with_held(weapon, weapon_upgrade, spare, stock);
        for (key, n) in stock.iter().filter(|e| e.1 > 0 && is_goal_material(&e.0)) {
            *state.workshop.obtained.entry(key.clone()).or_insert(0) += n;
            state.workshop.total_obtained = state.workshop.total_obtained.saturating_add(*n);
        }
        state
    }

    /// The same record, `migrated_from` aside (what decides whether a save must be written as version 6): `==`.
    pub fn same_progress(&self, other: &WeaponsState) -> bool {
        self == other
    }

    pub fn owns(&self, key: &str) -> bool {
        self.keyblades.iter().any(|k| k.key.eq_ignore_ascii_case(key))
    }

    pub fn level(&self, key: &str) -> Option<u8> {
        self.keyblades.iter().find(|k| k.key.eq_ignore_ascii_case(key)).map(|k| k.level)
    }

    /// Owns a Keyblade (at its level when obtained). False: already owned, or not a Keyblade.
    fn own(&mut self, key: &str) -> bool {
        let Some(w) = by_key(key).filter(|w| w.is_keyblade()) else { return false };
        if self.owns(&w.key) {
            return false;
        }
        self.keyblades.push(OwnedKeyblade { key: w.key.clone(), level: w.init_level });
        true
    }

    /// The slots in force: the ones set through [`set_slots`], else the legacy fields given (in hand, spare 1, spare 2).
    pub fn slots_or(&self, weapon: &str, spare: &[Option<String>]) -> [Option<String>; KEYBLADE_SLOTS] {
        self.slots.clone().unwrap_or_else(|| [Some(weapon.to_string()).filter(|w| !w.is_empty()), spare.first().cloned().flatten(), spare.get(1).cloned().flatten()])
    }

    /// The version-6 `weapons` block. `slots`: the slots in force ([`WeaponsState::slots_or`]); whether they were set
    /// through [`set_slots`] is kept beside them.
    pub fn to_value(&self, slots: &[Option<String>; KEYBLADE_SLOTS]) -> Value {
        json!({
            "keyblades": self.keyblades.iter().map(|k| json!({ "key": k.key, "level": k.level })).collect::<Vec<_>>(),
            "slots": slots,
            "slots_set": self.slots.is_some(),
            "current": self.current,
            "workshop": self.workshop.to_value(),
            "migrated_from": self.migrated_from,
        })
    }

    /// The block read back (`None`: missing or of the wrong shape). A Keyblade key the table does not know is dropped; the
    /// Kingdom Key is put back if missing; levels are clamped to [`MAX_LEVEL`] and never below the level when obtained.
    pub fn from_value(v: &Value) -> Option<WeaponsState> {
        let mut keyblades: Vec<OwnedKeyblade> = Vec::new();
        for k in v["keyblades"].as_array()? {
            let (Some(key), Some(level)) = (k["key"].as_str(), k["level"].as_u64()) else { return None };
            let Some(w) = by_key(key).filter(|w| w.is_keyblade()) else { continue };
            if keyblades.iter().all(|o| o.key != w.key) {
                keyblades.push(OwnedKeyblade { key: w.key.clone(), level: u8::try_from(level).unwrap_or(MAX_LEVEL).clamp(w.init_level, MAX_LEVEL) });
            }
        }
        if keyblades.iter().all(|k| k.key != KINGDOM_KEY) {
            keyblades.insert(0, OwnedKeyblade { key: KINGDOM_KEY.to_string(), level: init_level(KINGDOM_KEY) });
        }
        let raw = v["slots"].as_array().filter(|s| s.len() == KEYBLADE_SLOTS)?;
        let slots = [0, 1, 2].map(|k| raw[k].as_str().map(str::to_string));
        Some(WeaponsState {
            keyblades,
            slots: v["slots_set"].as_bool()?.then_some(slots),
            current: v["current"].as_u64().and_then(|n| u8::try_from(n).ok())?.min(KEYBLADE_SLOTS as u8 - 1),
            workshop: Workshop::from_value(&v["workshop"])?,
            migrated_from: v["migrated_from"].as_u64().and_then(|n| u32::try_from(n).ok()).unwrap_or(0),
        })
    }

    /// (the save's capture) This record with every Keyblade the legacy fields show owned too (one put into the stock by a
    /// host's test command, say: at its level when obtained), and the Kingdom Key in hand at least at `weapon_upgrade` (the
    /// status's upgrade of the weapon in hand: the Kingdom Key's level before the Forge).
    pub fn with_held(&self, weapon: &str, weapon_upgrade: u8, spare: &[Option<String>], stock: &[(String, u32)]) -> WeaponsState {
        let mut out = self.clone();
        let held = std::iter::once(weapon).chain(spare.iter().flatten().map(String::as_str)).chain(stock.iter().filter(|e| e.1 > 0).map(|e| e.0.as_str()));
        for key in held {
            out.own(key);
        }
        if weapon == KINGDOM_KEY {
            if let Some(k) = out.keyblades.iter_mut().find(|k| k.key == KINGDOM_KEY) {
                k.level = k.level.max(weapon_upgrade.min(MAX_LEVEL));
            }
        }
        out
    }
}

/// A Keyblade's level when obtained (0 for anything else).
pub fn init_level(key: &str) -> u8 {
    by_key(key).map_or(0, |w| w.init_level)
}

// ------------------------------------------------------------------------------------------------ the running record
// One record per process: the character that runs. Each test thread has its own (the tests of this crate run in
// parallel and must not see each other's Keyblades).

/// What the running record keeps beside the state: the load hook and the material gains not yet taken.
struct Store {
    state: Option<WeaponsState>,
    load_hook: Option<fn(&mut WeaponsState)>,
    gains: Vec<(String, u32)>,
}

const GAINS_MAX: usize = 256;

#[cfg(not(test))]
static STORE: std::sync::Mutex<Store> = std::sync::Mutex::new(Store { state: None, load_hook: None, gains: Vec::new() });

#[cfg(test)]
thread_local! {
    static STORE: std::cell::RefCell<Store> = const { std::cell::RefCell::new(Store { state: None, load_hook: None, gains: Vec::new() }) };
}

fn with_store<R>(f: impl FnOnce(&mut Store) -> R) -> R {
    #[cfg(not(test))]
    {
        f(&mut STORE.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
    }
    #[cfg(test)]
    {
        STORE.with(|s| f(&mut s.borrow_mut()))
    }
}

/// Reads or changes the running record (a new game's start until a save is applied).
pub fn with<R>(f: impl FnOnce(&mut WeaponsState) -> R) -> R {
    with_store(|s| f(s.state.get_or_insert_with(WeaponsState::start)))
}

/// A copy of the running record.
pub fn snapshot() -> WeaponsState {
    with(|s| s.clone())
}

/// (the save's apply) The running record becomes `state`; then the load hook runs (P6a: goals / recipes evaluated on load).
pub fn restore(state: WeaponsState) {
    let hook = with_store(|s| {
        s.state = Some(state);
        s.gains.clear();
        s.load_hook
    });
    if let Some(hook) = hook {
        with(hook);
    }
}

/// A NEW GAME (no save for the character): the running record starts again ([`WeaponsState::start`]). A host calls it
/// wherever it builds a new-game Sora instead of applying a save (else the last character's Keyblades stay).
pub fn begin_new_game() {
    with_store(|s| {
        s.state = Some(WeaponsState::start());
        s.gains.clear();
    });
}

/// (plan 1.2: "goals evaluated on load (by P6a later)") The function run on the record each time a save is applied
/// ([`restore`]); `migrated_from` tells a migrated record (5) from a native one (0). One hook; a second call replaces it.
pub fn set_load_hook(hook: fn(&mut WeaponsState)) {
    with_store(|s| s.load_hook = Some(hook));
}

/// What [`give`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gift {
    /// Queued for the HUD (the stock, the popup): how many the stock has room for (a friend's weapon past
    /// [`STACK_MAX`] still comes, with KH3's stock-full popup).
    Given(u32),
    /// A Keyblade already owned: refused, nothing queued, no popup. The caller marks its source done all the same.
    AlreadyOwned,
    /// Not a weapon of the table (or a count of 0).
    NotAWeapon,
}

/// HANDS SORA `count` OF A WEAPON (by item id): a Keyblade once (owned at once, at its level when obtained); a friend's
/// weapon `count` times (the stock keeps at most [`STACK_MAX`]).
pub fn give(item_id: u32, count: u32) -> Gift {
    let Some(w) = by_id(item_id).filter(|_| count > 0) else { return Gift::NotAWeapon };
    if w.is_keyblade() {
        if !with(|s| s.own(&w.key)) {
            return Gift::AlreadyOwned;
        }
        crate::items::queue(item_id, 1);
        return Gift::Given(1);
    }
    let room = STACK_MAX.saturating_sub(crate::items::held(&w.key));
    crate::items::queue(item_id, count);
    Gift::Given(count.min(room))
}

/// [`give`] by key.
pub fn give_key(key: &str, count: u32) -> Gift {
    by_key(key).map_or(Gift::NotAWeapon, |w| give(w.item_id, count))
}

/// Does Sora own it: a Keyblade owned, or at least one of a friend's weapon in the stock (or on its way there).
pub fn owns(key: &str) -> bool {
    count(key) > 0
}

/// How many: a Keyblade 0 or 1; a friend's weapon its count in the camp menu's stock plus the ones queued
/// (`crate::items::held`; the weapon a friend holds is in the friends' record, not counted).
pub fn count(key: &str) -> u32 {
    match by_key(key) {
        Some(w) if w.is_keyblade() => u32::from(with(|s| s.owns(&w.key))),
        Some(w) => crate::items::held(&w.key),
        None => 0,
    }
}

/// The Keyblades owned, in the order obtained, with their levels.
pub fn keyblades() -> Vec<OwnedKeyblade> {
    with(|s| s.keyblades.clone())
}

/// A Keyblade's Forge level (`None`: not owned).
pub fn level(key: &str) -> Option<u8> {
    with(|s| s.level(key))
}

/// Sets an owned Keyblade's Forge level (the Forge, P6a), clamped to its level when obtained .. [`MAX_LEVEL`]. False:
/// not owned.
pub fn set_level(key: &str, level: u8) -> bool {
    let floor = init_level(key);
    with(|s| match s.keyblades.iter_mut().find(|k| k.key.eq_ignore_ascii_case(key)) {
        Some(k) => {
            k.level = level.clamp(floor, MAX_LEVEL);
            true
        }
        None => false,
    })
}

/// The slots set through [`set_slots`] (`None`: they follow the save's legacy fields: in hand, spare 1, spare 2).
pub fn slots() -> Option<[Option<String>; KEYBLADE_SLOTS]> {
    with(|s| s.slots.clone())
}

/// The slot in hand (0 .. 2).
pub fn current_slot() -> u8 {
    with(|s| s.current)
}

/// Sets Sora's three Keyblade slots and the one in hand (P2's switching, P11's Equipment screen). A slot may only hold an
/// owned Keyblade (others are emptied); `current` is clamped to 0 .. 2.
pub fn set_slots(slots: [Option<String>; KEYBLADE_SLOTS], current: u8) {
    with(|s| {
        let slots = slots.map(|k| k.filter(|k| s.owns(k)));
        s.slots = Some(slots);
        s.current = current.min(KEYBLADE_SLOTS as u8 - 1);
    });
}

/// Only the slot in hand changes.
pub fn set_current_slot(current: u8) {
    with(|s| s.current = current.min(KEYBLADE_SLOTS as u8 - 1));
}

/// A copy of the Workshop's record.
pub fn workshop() -> Workshop {
    with(|s| s.workshop.clone())
}

/// Reads or changes the Workshop's record (P6a's rules).
pub fn with_workshop<R>(f: impl FnOnce(&mut Workshop) -> R) -> R {
    with(|s| f(&mut s.workshop))
}

/// (plan 1.2) A MATERIAL WAS OBTAINED (`crate::items::on_material_gain` is the one hook every host path calls): one of
/// KH3's 60 materials counts as ever obtained `n` times more (and in the total); the gain is also kept for
/// [`take_material_gains`]. Anything else (the gloveworts, battle items) is not counted.
pub fn note_material_gain(key: &str, n: u32) {
    if n == 0 || !is_goal_material(key) {
        return;
    }
    with_store(|s| {
        let state = s.state.get_or_insert_with(WeaponsState::start);
        let entry = state.workshop.obtained.entry(key.to_string()).or_insert(0);
        *entry = entry.saturating_add(n);
        state.workshop.total_obtained = state.workshop.total_obtained.saturating_add(n);
        if s.gains.len() >= GAINS_MAX {
            s.gains.remove(0);
        }
        s.gains.push((key.to_string(), n));
    });
}

/// The material gains counted since the last call (material key, count): for a consumer that reacts to them (the
/// Workshop tracker, a log). The counts themselves are already in the record.
pub fn take_material_gains() -> Vec<(String, u32)> {
    with_store(|s| std::mem::take(&mut s.gains))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table: KH3's 17 Keyblades (one of each, `m_MaxNum` 1, none sellable, the init levels of WD 6d) and the friends'
    /// 28 weapons (`m_MaxNum` 99), with the item ids items.rs already used for the friends' weapons.
    #[test]
    fn the_weapon_table_is_kh3s() {
        let keyblades: Vec<&Weapon> = table().iter().filter(|w| w.is_keyblade()).collect();
        assert_eq!((keyblades.len(), table().iter().filter(|w| w.chara == Chara::Donald).count(), table().iter().filter(|w| w.chara == Chara::Goofy).count()), (17, 14, 14));
        assert!(keyblades.iter().all(|w| w.max == 1 && !w.can_sell && w.key.starts_with("WEP_KEYBLADE_SO_")));
        assert!(table().iter().filter(|w| !w.is_keyblade()).all(|w| w.max == STACK_MAX && w.init_level == 0));
        let init = |key: &str| by_key(key).map(|w| w.init_level);
        // Ultima Weapon +10; Favorite Deputy / Ever After +1; Happy Gear / Crystal Snow / Grand Chef +2; Hunny Spout / Nano Gear /
        // Wheel of Fate +3; the rest +0
        assert_eq!([init("WEP_KEYBLADE_SO_14"), init("WEP_KEYBLADE_SO_03"), init("WEP_KEYBLADE_SO_04"), init("WEP_KEYBLADE_SO_05"), init("WEP_KEYBLADE_SO_10"), init("WEP_KEYBLADE_SO_07"), init("WEP_KEYBLADE_SO_09"), init(KINGDOM_KEY)], [Some(10), Some(1), Some(1), Some(2), Some(2), Some(3), Some(3), Some(0)]);
        assert_eq!(by_key(KINGDOM_KEY).map(|w| (w.item_id, w.name.as_str(), w.row.as_str())), Some((0x301, "Kingdom Key", "I03001")));
        assert_eq!(by_key("WEP_KEYBLADE_SO_18").map(|w| (w.item_id, w.name.as_str())), Some((0x313, "Dead of Night")));
        for w in table().iter().filter(|w| !w.is_keyblade()) {
            assert_eq!(crate::items::party_weapon_key(w.item_id).as_deref(), Some(w.key.as_str()), "the same ids as before");
        }
        assert_eq!(find("Hero's_Origin").map(|w| w.item_id), Some(0x302));
        assert_eq!(find("0x318").map(|w| w.name.as_str()), Some("Warhammer+"));
        assert!(table().iter().all(|w| crate::items::by_id(w.item_id).is_none()), "not rows of items::table");
    }

    #[test]
    fn only_kh3s_60_materials_count_for_goals() {
        assert!(is_goal_material("MAT_ITEM01") && is_goal_material("MAT_ITEM60"));
        for not in ["MAT_ITEM00", "MAT_ITEM61", "MAT_GLOVE_GRAVE_01", "ITEM_POTION", "MAT_ITEM1", "MAT_ITEM001"] {
            assert!(!is_goal_material(not), "{not}");
        }
        begin_new_game();
        take_material_gains();
        note_material_gain("MAT_ITEM05", 3);
        note_material_gain("MAT_GLOVE_GRAVE_01", 4);
        note_material_gain("MAT_ITEM05", 2);
        note_material_gain("MAT_ITEM60", 0);
        let w = workshop();
        assert_eq!((w.obtained_of("MAT_ITEM05"), w.obtained_of("MAT_GLOVE_GRAVE_01"), w.total_obtained, w.kinds_obtained()), (5, 0, 5, 1));
        assert_eq!(take_material_gains(), vec![("MAT_ITEM05".to_string(), 3), ("MAT_ITEM05".to_string(), 2)]);
        assert!(take_material_gains().is_empty());
    }

    /// (the user: "There is only 1 of each keyblade in the game (no duplicates)") A Keyblade is owned once, at its level when
    /// obtained; a second one is refused (nothing queued). The Kingdom Key is owned from the start.
    #[test]
    fn no_keyblade_is_owned_twice() {
        let _door = crate::items::TEST_DOOR.lock().unwrap_or_else(|e| e.into_inner());
        begin_new_game();
        crate::items::take_given();
        assert!(owns(KINGDOM_KEY) && !owns("WEP_KEYBLADE_SO_07"));
        assert_eq!(give_key(KINGDOM_KEY, 1), Gift::AlreadyOwned);
        let hunny = by_key("WEP_KEYBLADE_SO_07").unwrap();
        assert_eq!(give(hunny.item_id, 5), Gift::Given(1), "one, whatever the count");
        assert_eq!((count(&hunny.key), level(&hunny.key)), (1, Some(3)), "Hunny Spout arrives at +3");
        assert_eq!(give(hunny.item_id, 1), Gift::AlreadyOwned);
        assert!(!crate::items::give(hunny.item_id, 1), "the item door refuses it too");
        let queued: Vec<(u32, u32)> = crate::items::take_given().into_iter().filter(|e| e.0 == hunny.item_id).collect();
        assert!(queued.len() <= 1 && queued.iter().all(|e| e.1 == 1), "only the first one went to the HUD: {queued:?}");
        assert_eq!(keyblades().iter().filter(|k| k.key == hunny.key).count(), 1);
        assert_eq!((give(0x999, 1), give(hunny.item_id, 0)), (Gift::NotAWeapon, Gift::NotAWeapon));
        // levels: clamped to the level when obtained .. +10; not owned = refused
        assert!(set_level(&hunny.key, 1) && level(&hunny.key) == Some(3));
        assert!(set_level(&hunny.key, 12) && level(&hunny.key) == Some(MAX_LEVEL));
        assert!(!set_level("WEP_KEYBLADE_SO_01", 2));
        // a new game forgets it
        begin_new_game();
        assert_eq!(keyblades(), vec![OwnedKeyblade { key: KINGDOM_KEY.to_string(), level: 0 }]);
    }

    /// (the user: "Donald and Goofy can get duplicates") A friend's weapon comes as often as given; the stock holds 99.
    #[test]
    fn friends_weapons_stack_to_99() {
        let _door = crate::items::TEST_DOOR.lock().unwrap_or_else(|e| e.into_inner());
        crate::items::take_given();
        crate::items::publish_held(vec![("WEP_DONALD_03".to_string(), 97)]);
        let staff = by_key("WEP_DONALD_03").unwrap();
        assert_eq!(give(staff.item_id, 1), Gift::Given(1));
        assert_eq!(count(&staff.key), 98, "97 in the stock + 1 queued");
        assert_eq!(give(staff.item_id, 5), Gift::Given(1), "room for one more");
        assert_eq!(give(staff.item_id, 1), Gift::Given(0), "stock full: it still comes (KH3's stock-full popup), nothing added");
        assert!(crate::items::give(staff.item_id, 2), "through the item door as before");
        let queued: u32 = crate::items::take_given().into_iter().filter(|e| e.0 == staff.item_id).map(|e| e.1).sum();
        assert!(queued <= 1 + 5 + 1 + 2);
        crate::items::publish_held(Vec::new());
        assert_eq!(count("WEP_GOOFY_00"), 0);
    }

    #[test]
    fn slots_hold_owned_keyblades_only() {
        begin_new_game();
        assert_eq!((slots(), current_slot()), (None, 0));
        let state = snapshot();
        assert_eq!(state.slots_or(KINGDOM_KEY, &[None, Some("WEP_KEYBLADE_SO_02".to_string())]), [Some(KINGDOM_KEY.to_string()), None, Some("WEP_KEYBLADE_SO_02".to_string())]);
        with(|s| s.own("WEP_KEYBLADE_SO_02"));
        set_slots([Some("WEP_KEYBLADE_SO_02".to_string()), Some(KINGDOM_KEY.to_string()), Some("WEP_KEYBLADE_SO_05".to_string())], 7);
        assert_eq!((slots(), current_slot()), (Some([Some("WEP_KEYBLADE_SO_02".to_string()), Some(KINGDOM_KEY.to_string()), None]), 2));
        set_current_slot(1);
        assert_eq!(current_slot(), 1);
    }

    /// The block's round trip; odd blocks are refused or repaired; the Moogle level is only raised.
    #[test]
    fn the_block_round_trips() {
        let mut state = WeaponsState::from_legacy("WEP_KEYBLADE_SO_02", 4, &[Some(KINGDOM_KEY.to_string()), None], &[("MAT_ITEM01".to_string(), 5), ("WEP_KEYBLADE_SO_05".to_string(), 1), ("MAT_GLOVE_GRAVE_01".to_string(), 3), ("MAT_ITEM58".to_string(), 0)], 5);
        assert_eq!(state.keyblades, vec![OwnedKeyblade { key: KINGDOM_KEY.to_string(), level: 0 }, OwnedKeyblade { key: "WEP_KEYBLADE_SO_02".to_string(), level: 0 }, OwnedKeyblade { key: "WEP_KEYBLADE_SO_05".to_string(), level: 2 }], "the upgrade is the Kingdom Key's only");
        assert_eq!(WeaponsState::from_legacy(KINGDOM_KEY, 4, &[], &[], 5).level(KINGDOM_KEY), Some(4), "Kingdom Key level from weapon_upgrade");
        assert_eq!((state.workshop.obtained.len(), state.workshop.total_obtained, state.migrated_from), (1, 5, 5));
        state.workshop.goals.push("IC_1".to_string());
        state.workshop.created.insert("IS_12".to_string(), 2);
        state.workshop.raise_moogle_level(3);
        state.workshop.raise_moogle_level(2);
        state.workshop.raise_moogle_level(99);
        assert_eq!(state.workshop.moogle_level, MOOGLE_LEVEL_MAX);
        state.workshop.ext = json!({ "p6a": [1, 2] });
        state.current = 1;
        let slots = state.slots_or("WEP_KEYBLADE_SO_02", &[Some(KINGDOM_KEY.to_string()), None]);
        let back = WeaponsState::from_value(&state.to_value(&slots)).unwrap();
        assert_eq!(back, state);
        state.slots = Some(slots.clone());
        assert_eq!(WeaponsState::from_value(&state.to_value(&slots)).unwrap(), state);
        // an unknown Keyblade is dropped, the Kingdom Key put back, a level clamped
        let mut odd = state.to_value(&slots);
        odd["keyblades"] = json!([{ "key": "WEP_NOT_A_KEYBLADE", "level": 3 }, { "key": "WEP_KEYBLADE_SO_14", "level": 2 }, { "key": "WEP_KEYBLADE_SO_14", "level": 9 }]);
        let read = WeaponsState::from_value(&odd).unwrap();
        assert_eq!(read.keyblades, vec![OwnedKeyblade { key: KINGDOM_KEY.to_string(), level: 0 }, OwnedKeyblade { key: "WEP_KEYBLADE_SO_14".to_string(), level: 10 }]);
        for broken in ["slots", "workshop", "keyblades", "current"] {
            let mut v = state.to_value(&slots);
            v.as_object_mut().unwrap().remove(broken);
            assert!(WeaponsState::from_value(&v).is_none(), "{broken}");
        }
        assert!(state.same_progress(&WeaponsState { migrated_from: 0, ..state.clone() }));
    }

    /// The load hook runs on every applied record.
    #[test]
    fn the_load_hook_runs_on_restore() {
        fn hook(state: &mut WeaponsState) {
            if state.migrated_from == 5 {
                state.workshop.goals.push("IC_HOOK".to_string());
            }
        }
        set_load_hook(hook);
        restore(WeaponsState::from_legacy(KINGDOM_KEY, 0, &[], &[], 5));
        assert_eq!(workshop().goals, vec!["IC_HOOK".to_string()]);
        restore(WeaponsState::start());
        assert!(workshop().goals.is_empty());
        with_store(|s| s.load_hook = None);
        assert_eq!((kb_record("boss.margit"), wsrc_record(10_000_850)), ("kb:boss.margit".to_string(), "wsrc:10000850".to_string()));
    }
}
