//! (save) SORA'S SAVE DATA, engine-free: what of him outlives a session (level, EXP, munny, abilities owned
//! and equipped, magic tiers and command slots, shortcut sets, the item pouch and stock, equipment worn and
//! in stock, HP / MP / focus, play time) as one versioned JSON file per host character.
//!
//! THE MODEL. `kh_core::Status` is built by `Status::from_loadout`; a save is therefore a LOADOUT-SHAPED
//! record: restoring = the base loadout (the sheet the host starts Sora from; it carries what a save does not
//! hold: the opening choice, the difficulty) with the saved lists put in, through the same constructor, and
//! then the run-time values (HP, MP, focus, munny, boosts) and the command system's and the pouch's state.
//! "No file for this character" = A NEW GAME = [`new_game_loadout_for`]: THE TRUE START of the sheet
//! `sora-newgame` (compiled in: level 1, the 16 start abilities + Zero EXP, Fire and Water, an empty pouch of
//! three slots, no shortcuts) with the opening pair the player chose in the new-game choice menu
//! ([`crate::hud::Hud::open_newgame_choice`]); the pair is kept in the save (`desire`, `ability_path`) and
//! drives max HP / MP and the level table's ability column from then on. Everything else he gains by
//! levelling (`Status::give_exp`) and by rewards ([`grant`]).
//!
//! THE FILE: `<dir>/<character key>.khsave`, JSON, field `version`. Written atomically (a temporary file in the
//! same folder, flushed, then renamed over the file), the file before it kept as `<key>.khsave.bak`. Reading:
//! the file; when it cannot be read or understood, the backup; when neither, a new game: each said through
//! the caller's log, never a panic. A file of a NEWER version than this code is not touched (new game in
//! memory, the file is not overwritten: [`Loaded::Refused`]).
//!
//! No Bevy, no game SDK: the host says who the character is ([`character_key`]) and when to load and write.

use std::path::{Path, PathBuf};

use kh_core::enums::ETresAbilityKind;
use kh_core::magic::items::{Pouch, PouchSlot};
use kh_core::status::{BonusCounts, GameLevel, Growth, ItemKind, ItemStack, KnownMagic, Loadout, OpeningChoice, Power, Reward, RewardOutcome, Status}; // (DF) GameLevel
use kh_core::Sora;
use serde_json::{json, Value};

use crate::hud::Inventory;
use crate::rig::loadout::{at_test_level, new_game, parse_test_levels, TestLevel};

/// The version this code writes. History: 1 = the first format (2026-10-12); 2 = the opening pair is kept
/// (`choice`), and a new game is the true start: a version-1 file loses what the old level-1 test loadout
/// gave it for nothing (see [`upgrade`]); 3 (2026-10-13, boss rewards) = `records` is kept (which once-only rewards
/// this character has had); a version-2 file has none; 4 (2026-10-15, party P) = `party` is kept (the save point's
/// Party Formation: `kh_host::formation::Formation::to_save`); a version-3 file has none (the default formation);
/// 5 (2026-10-15, ER party S6) = `party.friends` is kept (the v5 FRIENDS RECORD of the Elden Ring party: each friend's
/// weapon, armour, accessories, item slots, abilities owned / equipped, victory bonuses; kh_partyhost
/// `PartyRuntime::party_record`); a version-4 file has none (the friends start from KH3's init data).
///
/// (ER party S6) ADDITIVE AND BACKWARD COMPATIBLE: this code reads every version up to 5; a file is WRITTEN as version 5
/// only when its party record holds the friends record ([`SoraSave::written_version`]), otherwise as version 4,
/// byte for byte what the code before S6 wrote (a host without the party, or with it switched off, changes nothing;
/// an older build still reads those files). A version-5 file is refused by an older build (`Loaded::Refused`: it is
/// never overwritten there), so the friends' progression cannot be lost to a downgrade.
pub const VERSION: u32 = 5;
/// (ER party S6) The version a file without the friends record is written as.
pub const VERSION_WITHOUT_FRIENDS: u32 = 4;
pub const EXTENSION: &str = "khsave";

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PouchSlotSave {
    pub item: Option<String>,
    pub exists: bool,
    pub fixed: bool,
    pub auto_reload: bool,
    pub used: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct SoraSave {
    pub version: u32,
    /// The host character this belongs to (as the host named it; information only).
    pub character: String,
    /// Seconds played as Sora with this character.
    pub play_seconds: u64,
    /// The host's own play-time counter when this was written (0 = unknown). A host uses it to tell a file
    /// that belongs to an EARLIER character of the same key (its counter is then ahead of the character's).
    pub host_play_ms: u64,
    /// (version 2) THE OPENING PAIR: the first choice ("vitality" / "balance" / "wisdom": which of the three
    /// choice bonuses he has) and the second (the ability path 0 / 1 / 2 = warrior / mystic / guardian: the
    /// column of the level table).
    pub desire: String,
    pub ability_path: u8,
    pub level: u32,
    pub exp: i32,
    pub munny: i32,
    pub hp: i32,
    pub mp: i32,
    pub focus: i32,
    /// The victory-bonus counters (`BonusCounts`, 32 kinds) WITHOUT the opening choice's own row.
    pub bonuses: Vec<u8>,
    /// Strength, Magic, Defense, AP boosts.
    pub boosts: [u8; 4],
    pub weapon: String,
    pub weapon_upgrade: u8,
    pub armour: Vec<String>,
    pub accessories: Vec<String>,
    /// (ability kind value, copies owned, mask of the equipped copies).
    pub abilities: Vec<(i32, u8, u8)>,
    /// (element name, tier 1..3).
    pub magic: Vec<(String, u32)>,
    pub shotlocks: Vec<String>,
    /// The loadout's shortcut names (what the command slots are first filled from).
    pub shortcut_names: [Vec<String>; 3],
    pub item_slots: u32,
    pub items: Vec<(String, u32)>,
    // ---- the command system
    pub command_filled: bool,
    pub magic_slots: [u8; 6],
    pub tier_chosen: [bool; 6],
    pub shortcuts: [[u8; 4]; 3],
    pub shortcut_page: u8,
    /// (XSI, 2026-10-16) The Customize screen's Shotlock choice (`CommandState::shotlock`: 0 the Keyblade's own, 11
    /// Spectral Rays). Additive: a save without it reads 0.
    pub shotlock: u8,
    // ---- the pouch
    pub pouch_filled: bool,
    pub pouch_slots: Vec<PouchSlotSave>,
    pub pouch_stock: Vec<(String, u8)>,
    // ---- the camp menu's stock
    pub stock_equipment: Vec<(String, u32)>,
    pub spare_weapons: [Option<String>; 2],
    /// (version 3) What happened once for this character (`boss:<row id>`: that boss's reward was given).
    pub records: Vec<String>,
    /// (version 4, party P) The party formation record (`kh_host::formation::Formation::to_save` / `from_save`); null =
    /// none kept (the host's default). `capture` leaves it null: the host fills it.
    pub party: Value,
    /// (DF, 2026-10-15) THE DIFFICULTY this character plays at (`GameLevel` name: BEGINNER / STANDARD / PROUD / CRITICAL).
    /// KH3 fixes it at the new game (GW+0x7DF0 in the save). ADDITIVE, no version change: a file without it (written before
    /// DF) reads as "" = the difficulty of the base loadout, which is what such a file was made at (STANDARD); an older build
    /// ignores the field. See [`SoraSave::switch_difficulty`] for an existing character.
    pub difficulty: String,
}

/// Why a save text was not accepted.
#[derive(Debug, PartialEq)]
pub enum SaveError {
    NotJson(String),
    NoVersion,
    /// Written by a newer version of this code.
    Newer(u32),
    Field(&'static str),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::NotJson(e) => write!(f, "not JSON ({e})"),
            SaveError::NoVersion => write!(f, "no version field"),
            SaveError::Newer(v) => write!(f, "written by a newer version ({v}; this code knows {VERSION})"),
            SaveError::Field(name) => write!(f, "field \"{name}\" is missing or of the wrong shape"),
        }
    }
}

/// THE NEW GAME with the opening pair `choice` (the result of the new-game choice menu): the true start of
/// the sheet `sora-newgame` (compiled in). `base` (the host's loadout sheet) gives only the difficulty.
pub fn new_game_loadout_for(base: &Loadout, choice: OpeningChoice) -> Loadout {
    new_game(base, choice)
}

/// (DF, 2026-10-15) `base` at the difficulty `level` (what a NEW GAME of the host's chosen difficulty is built from:
/// [`new_game_loadout_for`] takes the difficulty from its base and gives KH3's Critical start on CRITICAL).
pub fn with_difficulty(base: &Loadout, level: GameLevel) -> Loadout {
    let mut out = base.clone();
    out.game_level = level.name().to_string();
    out
}

/// THE NEW GAME of a host that does not ask (yet): [`new_game_loadout_for`] with the pair of `base`
/// (the level-99 sheet: balance + ability path A). `test_levels_sheet` is no longer read (the start is
/// compiled in); the parameter stays for the callers that pass it.
pub fn new_game_loadout(base: &Loadout, _test_levels_sheet: Option<&str>) -> Loadout {
    new_game(base, OpeningChoice::of_loadout(base).unwrap_or(OpeningChoice::DEFAULT))
}

/// THE SEAM FOR "OTHER MEANS" (a boss's reward, a found item ...): gives `reward` to the running Sora now
/// (`Sora::grant`: an ability by id with a copy count, a magic spell or tier, a max-HP / max-MP bonus, a
/// slot, AP, a shotlock, an item) with the game's popup for it at his next step. What it changes is part
/// of what [`SoraSave::capture`] keeps, so the host's next write persists it; [`grant_and_write`] writes
/// at once.
pub fn grant(sora: &mut Sora, reward: Reward) -> RewardOutcome {
    sora.grant(reward)
}

/// [`grant`], and the save of `key` in `dir` is written at once when something changed.
pub fn grant_and_write(sora: &mut Sora, reward: Reward, inventory: &Inventory, dir: &Path, key: &str, character: &str, play_seconds: u64, host_play_ms: u64) -> std::io::Result<RewardOutcome> {
    let outcome = sora.grant(reward);
    if outcome.changed {
        write(&save_path(dir, key), &SoraSave::capture(sora, inventory, character, play_seconds, host_play_ms))?;
    }
    Ok(outcome)
}

/// `base` at `level` for a host's testing key (`sora.level = N`): level 1 = the new game, otherwise the
/// nearest test level of the sheet at or below it, set to `level`.
pub fn test_loadout(base: &Loadout, level: u32, test_levels_sheet: Option<&str>) -> Loadout {
    if level <= 1 {
        return new_game_loadout(base, None);
    }
    let levels = test_levels_sheet.map(|text| parse_test_levels(text).0).filter(|l| !l.is_empty()).unwrap_or_else(|| TestLevel::DEFAULT.to_vec());
    let row = levels.iter().rev().find(|l| l.level <= level).copied().unwrap_or(TestLevel::DEFAULT[0]);
    at_test_level(base, TestLevel { level, ..row })
}

impl SoraSave {
    /// What is worth keeping of `sora` (and of the camp menu's stock) now.
    pub fn capture(sora: &Sora, inventory: &Inventory, character: &str, play_seconds: u64, host_play_ms: u64) -> SoraSave {
        let status = sora.status();
        // The opening choice's own bonus row is added by the constructor: it is taken off here.
        let mut bonuses = status.bonuses;
        for &kind in status.growth.map_or(&[][..], kh_core::status::Growth::bonus_kinds) {
            bonuses.0[kind] = bonuses.0[kind].saturating_sub(1);
        }
        let command = sora.command();
        let pouch = &sora.magic().pouch;
        SoraSave {
            version: VERSION,
            character: character.to_string(),
            play_seconds,
            host_play_ms,
            desire: status.growth.unwrap_or(Growth::Balance).name().to_string(),
            ability_path: status.ability_path.min(2),
            level: status.level,
            exp: status.exp,
            munny: status.munny,
            hp: status.hp,
            mp: status.mp,
            focus: status.focus,
            bonuses: bonuses.0.to_vec(),
            boosts: [status.boosts.strength, status.boosts.magic, status.boosts.defense, status.boosts.ap],
            weapon: status.weapon.clone(),
            weapon_upgrade: status.weapon_upgrade,
            armour: status.armour.clone(),
            accessories: status.accessories.clone(),
            abilities: status.abilities.iter_owned().map(|(kind, owned)| (kind.value(), owned, status.abilities.equipped_mask(kind))).collect(),
            magic: status.magic.iter().map(|m| (m.name.clone(), m.tier)).collect(),
            shotlocks: status.shotlocks.clone(),
            shortcut_names: status.shortcuts.clone(),
            item_slots: status.item_slots,
            items: status.items.iter().map(|i| (i.item.clone(), i.count)).collect(),
            command_filled: command.filled,
            magic_slots: command.magic_slots,
            tier_chosen: command.tier_chosen,
            shortcuts: command.shortcuts,
            shortcut_page: command.shortcut_page,
            shotlock: command.shotlock, // (XSI)
            pouch_filled: pouch.filled,
            pouch_slots: pouch.slots.iter().map(|s| PouchSlotSave { item: s.item.map(|i| i.name().to_string()), exists: s.exists, fixed: s.fixed, auto_reload: s.auto_reload, used: s.used }).collect(),
            pouch_stock: pouch.stock.iter().map(|(item, n)| (item.name().to_string(), *n)).collect(),
            stock_equipment: inventory.equipment.clone(),
            spare_weapons: inventory.spare_weapons.clone(),
            records: inventory.records.clone(),
            party: Value::Null, // (party P) the host fills it
            difficulty: status.game_level.name().to_string(), // (DF)
        }
    }

    /// The status this save describes, built over `base` (the host's loadout sheet).
    pub fn status(&self, base: &Loadout) -> Status {
        let mut loadout = base.clone();
        // The opening pair is the save's, not the base sheet's.
        loadout.set_choice(self.choice());
        loadout.level = self.level.clamp(1, 99);
        loadout.listed_level = 0;
        loadout.exp = self.exp.max(0) as u64;
        let mut counts = BonusCounts::NONE;
        for (to, from) in counts.0.iter_mut().zip(&self.bonuses) {
            *to = *from;
        }
        loadout.bonuses = Some(counts);
        loadout.weapon = self.weapon.clone();
        loadout.weapon_upgrade = self.weapon_upgrade;
        loadout.armour = self.armour.clone();
        loadout.accessories = self.accessories.clone();
        loadout.abilities = self.abilities.iter().filter_map(|(kind, owned, mask)| Some((ETresAbilityKind::from_value(*kind)?, *owned, mask.count_ones() as u8))).collect();
        loadout.critical_extra.clear();
        loadout.magic = self.magic.iter().map(|(name, tier)| KnownMagic { name: name.clone(), tier: *tier }).collect();
        loadout.shotlocks = self.shotlocks.clone();
        loadout.shortcuts = self.shortcut_names.clone();
        loadout.item_slots = self.item_slots;
        loadout.items = self.items.iter().map(|(item, count)| ItemStack { item: item.clone(), count: *count }).collect();
        // (DF) the save's own difficulty; none recorded = the base loadout's (what such a file was made at)
        if kh_core::status::GameLevel::from_name(&self.difficulty).is_some() {
            loadout.game_level = self.difficulty.clone();
        }
        loadout.source = format!("save data of \"{}\" over {}", self.character, base.source);
        let mut status = Status::from_loadout(&loadout);
        // Exactly the copies that were equipped (the constructor equips the lowest ones).
        for (kind, owned, mask) in &self.abilities {
            let Some(kind) = ETresAbilityKind::from_value(*kind) else { continue };
            for copy in 0..*owned {
                status.abilities.set_copy(kind, copy, mask & (1 << copy) != 0);
            }
        }
        (status.boosts.strength, status.boosts.magic, status.boosts.defense, status.boosts.ap) = (self.boosts[0], self.boosts[1], self.boosts[2], self.boosts[3]);
        status.munny = self.munny.max(0);
        status.rebuild();
        // The gauges as they were (a save written at 0 HP starts him with 1).
        status.hp = self.hp.clamp(1, status.max_hp().max(1));
        status.mp = self.mp.clamp(0, status.max_mp().max(0));
        status.focus = self.focus.clamp(0, status.max_focus().max(0));
        status
    }

    /// The opening pair of this save.
    pub fn choice(&self) -> OpeningChoice {
        OpeningChoice { desire: Growth::from_name(&self.desire).unwrap_or(Growth::Balance), power: Power::from_path(self.ability_path) }
    }

    /// Makes `sora` (and the camp menu's stock) what this save describes.
    pub fn apply(&self, base: &Loadout, sora: &mut Sora, inventory: &mut Inventory) {
        *sora.status_mut() = self.status(base);
        // The command system and the pouch: filled afresh from the status, then as saved.
        sora.command_mut().filled = false;
        sora.magic_mut().pouch = Pouch::default();
        kh_core::command::customize::ensure(sora);
        if self.command_filled {
            let command = sora.command_mut();
            (command.magic_slots, command.tier_chosen, command.shortcuts, command.shortcut_page) = (self.magic_slots, self.tier_chosen, self.shortcuts, self.shortcut_page.min(2));
        }
        sora.command_mut().shotlock = self.shotlock; // (XSI)
        if self.pouch_filled {
            let pouch = &mut sora.magic_mut().pouch;
            for (to, from) in pouch.slots.iter_mut().zip(&self.pouch_slots) {
                *to = PouchSlot { item: from.item.as_deref().and_then(ItemKind::from_name), exists: from.exists, fixed: from.fixed, auto_reload: from.auto_reload, used: from.used };
            }
            pouch.stock = self.pouch_stock.iter().filter_map(|(name, n)| Some((ItemKind::from_name(name)?, *n))).collect();
            pouch.filled = true;
        }
        inventory.equipment = self.stock_equipment.clone();
        inventory.spare_weapons = self.spare_weapons.clone();
        inventory.records = self.records.clone();
    }

    /// (ER party S6) THE VERSION THIS SAVE IS WRITTEN AS: 5 when its party record holds the friends record (an array
    /// `party.friends`), else 4 (what the code before the friends record wrote; see [`VERSION`]).
    pub fn written_version(&self) -> u32 {
        if self.party.get("friends").is_some_and(Value::is_array) {
            VERSION
        } else {
            VERSION_WITHOUT_FRIENDS
        }
    }

    /// (ER party S6) The friends record of the party record (version 5; empty for an older save).
    pub fn party_friends(&self) -> &[Value] {
        self.party.get("friends").and_then(Value::as_array).map_or(&[], Vec::as_slice)
    }

    /// (DF) The difficulty this save plays at: its own field, else `fallback` (the base loadout's: what a file written
    /// before the field existed was made at).
    pub fn game_level(&self, fallback: GameLevel) -> GameLevel {
        GameLevel::from_name(&self.difficulty).unwrap_or(fallback)
    }

    /// (DF, 2026-10-15) AN EXISTING CHARACTER'S ONE-TIME SWITCH OF DIFFICULTY (a port rule: KH3 fixes the difficulty at
    /// the new game). `from` = what the save plays at now ([`SoraSave::game_level`]). REVERSIBLE: switching back gives
    /// the record it had, except the start abilities of a Critical switch (kept; also boss rewards in the port).
    /// - EVERYTHING DERIVED follows by itself at the next load: base HP / MP and the bonus amounts (halved on Critical),
    ///   the damage factors, the EXP rate, the MP charge, the gauge rates.
    /// - THE RECORD: the AP boost byte gets / loses the difficulty's `ap_add` (Critical's `m_CriticalUseAPUpNum` 30; KH3
    ///   writes it at record init, 0x140E92280); leaving Critical drops the Critical-only abilities (`m_CriticalEquipAbility`
    ///   / `m_CriticalHaveAbility`: Critical Counter, Critical Recharge, Critical Converter); entering it gives one owned
    ///   copy (not equipped: the player installs them) of each `sora-critical-start` row's ability (at most three).
    ///   The `m_Critical*` set itself is given by `Status::from_loadout` at every load on Critical (equipped).
    /// Returns what changed, one line per change (empty = nothing to do).
    pub fn switch_difficulty(&mut self, from: GameLevel, to: GameLevel) -> Vec<String> {
        let mut out = Vec::new();
        if from == to {
            return out;
        }
        let (add_from, add_to) = (from.row().ap_add.clamp(0, 100), to.row().ap_add.clamp(0, 100));
        let ap = (self.boosts[3] as i32 - add_from).max(0) + add_to;
        if ap != self.boosts[3] as i32 {
            out.push(format!("AP boost {} -> {}", self.boosts[3], ap.clamp(0, 255)));
            self.boosts[3] = ap.clamp(0, 255) as u8;
        }
        if from.is_critical() {
            // (DF, dec032) an excluded one (Critical Counter) is not given by Critical: it stays
            let only: Vec<i32> = from.row().start_abilities.iter().filter_map(|n| ETresAbilityKind::from_game_name(n)).filter(|k| !kh_core::status::tables::critical_start_excluded(*k)).map(|k| k.value()).collect();
            let before = self.abilities.len();
            self.abilities.retain(|(kind, _, _)| !only.contains(kind));
            if self.abilities.len() != before {
                out.push(format!("{} Critical-only abilities removed", before - self.abilities.len()));
            }
        }
        if to.is_critical() {
            let mut added = 0;
            for (kind, _) in kh_core::status::tables::critical_start() {
                let value = kind.value();
                match self.abilities.iter_mut().find(|a| a.0 == value) {
                    Some(a) if a.1 >= 3 => {}
                    Some(a) => {
                        a.1 += 1;
                        added += 1;
                    }
                    None => {
                        self.abilities.push((value, 1, 0));
                        added += 1;
                    }
                }
            }
            if added > 0 {
                out.push(format!("{added} Critical start ability copies added (owned, not equipped)"));
            }
        }
        out.push(format!("difficulty {} -> {}", from.name(), to.name()));
        self.difficulty = to.name().to_string();
        out
    }

    pub fn to_json(&self) -> String {
        let value = json!({
            "version": self.written_version(), // (ER party S6) 5 only with the friends record
            "character": self.character,
            "play_seconds": self.play_seconds,
            "host_play_ms": self.host_play_ms,
            "choice": { "desire": self.desire, "ability_path": self.ability_path, "power": Power::from_path(self.ability_path).name() },
            "level": self.level,
            "exp": self.exp,
            "munny": self.munny,
            "hp": self.hp,
            "mp": self.mp,
            "focus": self.focus,
            "bonuses": self.bonuses,
            "boosts": self.boosts,
            "weapon": self.weapon,
            "weapon_upgrade": self.weapon_upgrade,
            "armour": self.armour,
            "accessories": self.accessories,
            "abilities": self.abilities.iter().map(|(kind, owned, mask)| json!({ "kind": kind, "name": ETresAbilityKind::from_value(*kind).map_or("?", |k| k.name()), "owned": owned, "equipped": mask })).collect::<Vec<_>>(),
            "magic": self.magic.iter().map(|(name, tier)| json!({ "name": name, "tier": tier })).collect::<Vec<_>>(),
            "shotlocks": self.shotlocks,
            "shortcut_names": self.shortcut_names,
            "item_slots": self.item_slots,
            "items": self.items.iter().map(|(item, count)| json!({ "item": item, "count": count })).collect::<Vec<_>>(),
            "command": { "filled": self.command_filled, "magic_slots": self.magic_slots, "tier_chosen": self.tier_chosen, "shortcuts": self.shortcuts, "shortcut_page": self.shortcut_page, "shotlock": self.shotlock },
            "pouch": {
                "filled": self.pouch_filled,
                "slots": self.pouch_slots.iter().map(|s| json!({ "item": s.item, "exists": s.exists, "fixed": s.fixed, "auto_reload": s.auto_reload, "used": s.used })).collect::<Vec<_>>(),
                "stock": self.pouch_stock.iter().map(|(item, n)| json!({ "item": item, "count": n })).collect::<Vec<_>>(),
            },
            "stock_equipment": self.stock_equipment.iter().map(|(id, n)| json!({ "id": id, "count": n })).collect::<Vec<_>>(),
            "spare_weapons": self.spare_weapons,
            "records": self.records,
            "party": self.party,
            "difficulty": self.difficulty, // (DF)
        });
        serde_json::to_string_pretty(&value).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Result<SoraSave, SaveError> {
        let mut value: Value = serde_json::from_str(text).map_err(|e| SaveError::NotJson(e.to_string()))?;
        let version = value["version"].as_u64().ok_or(SaveError::NoVersion)? as u32;
        if version > VERSION {
            return Err(SaveError::Newer(version));
        }
        upgrade(&mut value, version)?;
        let v = &value;
        let int = |name: &'static str| v[name].as_i64().ok_or(SaveError::Field(name));
        let text_of = |x: &Value, name: &'static str| x.as_str().map(str::to_string).ok_or(SaveError::Field(name));
        let strings = |x: &Value, name: &'static str| x.as_array().ok_or(SaveError::Field(name))?.iter().map(|s| text_of(s, name)).collect::<Result<Vec<String>, _>>();
        let list = |name: &'static str| v[name].as_array().ok_or(SaveError::Field(name));
        let bytes = |x: &Value, name: &'static str| x.as_array().ok_or(SaveError::Field(name))?.iter().map(|b| b.as_u64().map(|b| b.min(255) as u8).ok_or(SaveError::Field(name))).collect::<Result<Vec<u8>, _>>();
        let fixed = |x: &Value, name: &'static str, n: usize| bytes(x, name).and_then(|b| if b.len() == n { Ok(b) } else { Err(SaveError::Field(name)) });
        let (command, pouch) = (&v["command"], &v["pouch"]);
        let shortcut_names = list("shortcut_names")?;
        let shortcut_rows = command["shortcuts"].as_array().filter(|a| a.len() == 3).ok_or(SaveError::Field("command.shortcuts"))?;
        let mut shortcuts = [[0u8; 4]; 3];
        for (to, from) in shortcuts.iter_mut().zip(shortcut_rows) {
            to.copy_from_slice(&fixed(from, "command.shortcuts", 4)?);
        }
        let tiers = command["tier_chosen"].as_array().filter(|a| a.len() == 6).ok_or(SaveError::Field("command.tier_chosen"))?;
        let spare = list("spare_weapons")?;
        Ok(SoraSave {
            version: VERSION,
            character: v["character"].as_str().unwrap_or("").to_string(),
            play_seconds: v["play_seconds"].as_u64().unwrap_or(0),
            host_play_ms: v["host_play_ms"].as_u64().unwrap_or(0),
            desire: v["choice"]["desire"].as_str().filter(|d| Growth::from_name(d).is_some()).ok_or(SaveError::Field("choice.desire"))?.to_ascii_lowercase(),
            ability_path: v["choice"]["ability_path"].as_u64().filter(|p| *p <= 2).ok_or(SaveError::Field("choice.ability_path"))? as u8,
            level: int("level")?.clamp(1, 99) as u32,
            exp: int("exp")?.clamp(0, i32::MAX as i64) as i32,
            munny: int("munny")?.clamp(0, i32::MAX as i64) as i32,
            hp: int("hp")? as i32,
            mp: int("mp")? as i32,
            focus: int("focus")? as i32,
            bonuses: bytes(&v["bonuses"], "bonuses")?,
            boosts: fixed(&v["boosts"], "boosts", 4)?.try_into().unwrap_or([0; 4]),
            weapon: text_of(&v["weapon"], "weapon")?,
            weapon_upgrade: v["weapon_upgrade"].as_u64().unwrap_or(0).min(255) as u8,
            armour: strings(&v["armour"], "armour")?,
            accessories: strings(&v["accessories"], "accessories")?,
            abilities: list("abilities")?.iter().map(|a| Some((a["kind"].as_i64()? as i32, a["owned"].as_u64()?.min(3) as u8, a["equipped"].as_u64()?.min(7) as u8))).collect::<Option<Vec<_>>>().ok_or(SaveError::Field("abilities"))?,
            magic: list("magic")?.iter().map(|m| Some((m["name"].as_str()?.to_string(), m["tier"].as_u64()?.min(3) as u32))).collect::<Option<Vec<_>>>().ok_or(SaveError::Field("magic"))?,
            shotlocks: strings(&v["shotlocks"], "shotlocks")?,
            shortcut_names: [0, 1, 2].map(|k| shortcut_names.get(k).and_then(|page| strings(page, "shortcut_names").ok()).unwrap_or_default()),
            item_slots: int("item_slots")?.clamp(0, 8) as u32,
            items: list("items")?.iter().map(|i| Some((i["item"].as_str()?.to_string(), i["count"].as_u64()? as u32))).collect::<Option<Vec<_>>>().ok_or(SaveError::Field("items"))?,
            command_filled: command["filled"].as_bool().ok_or(SaveError::Field("command.filled"))?,
            magic_slots: fixed(&command["magic_slots"], "command.magic_slots", 6)?.try_into().unwrap_or([0; 6]),
            tier_chosen: [0, 1, 2, 3, 4, 5].map(|k| tiers[k].as_bool().unwrap_or(false)),
            shortcuts,
            shortcut_page: command["shortcut_page"].as_u64().unwrap_or(0).min(2) as u8,
            shotlock: command["shotlock"].as_u64().filter(|k| *k <= 255).unwrap_or(0) as u8, // (XSI) additive
            pouch_filled: pouch["filled"].as_bool().ok_or(SaveError::Field("pouch.filled"))?,
            pouch_slots: pouch["slots"].as_array().ok_or(SaveError::Field("pouch.slots"))?.iter().map(|s| PouchSlotSave { item: s["item"].as_str().map(str::to_string), exists: s["exists"].as_bool().unwrap_or(false), fixed: s["fixed"].as_bool().unwrap_or(false), auto_reload: s["auto_reload"].as_bool().unwrap_or(false), used: s["used"].as_bool().unwrap_or(false) }).collect(),
            pouch_stock: pouch["stock"].as_array().ok_or(SaveError::Field("pouch.stock"))?.iter().map(|i| Some((i["item"].as_str()?.to_string(), i["count"].as_u64()?.min(255) as u8))).collect::<Option<Vec<_>>>().ok_or(SaveError::Field("pouch.stock"))?,
            stock_equipment: list("stock_equipment")?.iter().map(|i| Some((i["id"].as_str()?.to_string(), i["count"].as_u64()? as u32))).collect::<Option<Vec<_>>>().ok_or(SaveError::Field("stock_equipment"))?,
            spare_weapons: [0, 1].map(|k| spare.get(k).and_then(|s| s.as_str()).map(str::to_string)),
            // (a file without the list, of whatever version, has no records)
            records: v["records"].as_array().map(|a| a.iter().filter_map(|r| r.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            // (version 4, party P) the formation record, null when the file has none
            party: v["party"].clone(),
            // (DF) none in a file written before the difficulty was kept
            difficulty: v["difficulty"].as_str().filter(|d| kh_core::status::GameLevel::from_name(d).is_some()).map(str::to_ascii_uppercase).unwrap_or_default(),
        })
    }
}

/// VERSION UPGRADES: `value` as version `from` is rewritten in place as the current version, one step per
/// version. Version 0 never existed.
///
/// 1 -> 2 (2026-10-12, "Sora should only start with his actual level 1 loadout"): a version-1 file was made
/// when a new game was the level-99 test loadout set to level 1, so it owns every victory-bonus ability of
/// the whole game (Glide, Doubleflight, the reprisals ...), all six elements, eight item slots and two
/// shortcut sets. None of that could have been EARNED in version 1 (it had no rewards: only the level table
/// gave abilities), so the step takes away what a true new game at the saved level would not have:
/// - the pair becomes balance + path A (what every version-1 file was built with);
/// - abilities: at most the start copies plus the level table's grants up to the saved level remain owned
///   (the others go, with their equipped bits); equipped copies beyond the level's AP come off, highest
///   ability id first;
/// - magic: Fire and Water stay (at the saved tier), the other elements go, also from the magic slots and
///   the shortcuts; the counters `MELEM_FIRE` / `MELEM_WATER` are set;
/// - the pouch: three slots (the slots above them and what they held are dropped; the stock is kept);
/// - level, EXP, munny, HP / MP / focus (clamped on load), equipment, the stock, play time: untouched.
fn upgrade(value: &mut Value, from: u32) -> Result<(), SaveError> {
    if from == 0 {
        return Err(SaveError::NoVersion);
    }
    for step in from..VERSION {
        if step == 1 {
            upgrade_1_to_2(value)?;
        }
        // 2 -> 3 (boss rewards): the list of once-only records begins empty (nothing was ever recorded before).
        if step == 2 && !value["records"].is_array() {
            value["records"] = json!([]);
        }
        // 3 -> 4 (party P): no formation was kept before: null (the host's default formation).
        if step == 3 && value.get("party").is_none() {
            value["party"] = Value::Null;
        }
        // 4 -> 5 (ER party S6): no friends record was kept before: none (the friends take KH3's init data). The party
        // record itself is kept as it is.
        if step == 4 && value.get("party").is_none() {
            value["party"] = Value::Null;
        }
    }
    value["version"] = json!(VERSION);
    Ok(())
}

fn upgrade_1_to_2(value: &mut Value) -> Result<(), SaveError> {
    use kh_core::magic::spells::{self, Element};
    use kh_core::status::tables;
    let start = new_game(&Loadout::new("migration"), OpeningChoice::DEFAULT);
    let level = value["level"].as_u64().ok_or(SaveError::Field("level"))?.clamp(1, 99) as u32;
    value["choice"] = json!({ "desire": Growth::Balance.name(), "ability_path": 0, "power": Power::Warrior.name() });
    // Abilities.
    let mut kept: Vec<(ETresAbilityKind, u8, u8)> = Vec::new();
    for entry in value["abilities"].as_array().ok_or(SaveError::Field("abilities"))? {
        let Some(kind) = entry["kind"].as_i64().and_then(|k| ETresAbilityKind::from_value(k as i32)) else { continue };
        let allowed = start.abilities.iter().find(|a| a.0 == kind).map_or(0, |a| a.1) + tables::level_grants(kind, 0, level, 0);
        let owned = (entry["owned"].as_u64().unwrap_or(0).min(3) as u8).min(allowed);
        if owned > 0 {
            kept.push((kind, owned, entry["equipped"].as_u64().unwrap_or(0) as u8 & ((1u8 << owned) - 1)));
        }
    }
    let budget = tables::level_row(level).ap + value["boosts"][3].as_u64().unwrap_or(0) as i32;
    let in_use = |kept: &[(ETresAbilityKind, u8, u8)]| kept.iter().map(|a| tables::ability_ap(a.0) * a.2.count_ones() as i32).sum::<i32>();
    for index in (0..kept.len()).rev() {
        while in_use(&kept) > budget && kept[index].2 != 0 && tables::ability_ap(kept[index].0) > 0 {
            let top = 7 - kept[index].2.leading_zeros() as u8;
            kept[index].2 &= !(1 << top);
        }
    }
    value["abilities"] = kept.iter().map(|(kind, owned, mask)| json!({ "kind": kind.value(), "name": kind.name(), "owned": owned, "equipped": mask })).collect();
    // Magic.
    let known = |element: Element| start.magic.iter().any(|m| Element::from_name(&m.name) == Some(element));
    if let Some(magic) = value["magic"].as_array_mut() {
        magic.retain(|m| m["name"].as_str().and_then(Element::from_name).is_some_and(known));
    }
    let mut counts = value["bonuses"].as_array().cloned().unwrap_or_default();
    counts.resize(32, json!(0));
    for element in Element::ALL {
        let tier = value["magic"].as_array().into_iter().flatten().find(|m| m["name"].as_str().and_then(Element::from_name) == Some(element)).and_then(|m| m["tier"].as_u64()).unwrap_or(0);
        counts[kh_core::status::reward::melem_kind(element)] = json!(tier);
    }
    value["bonuses"] = Value::Array(counts);
    let gone = |kind: u64| spells::unit(kind as u8).is_some_and(|unit| !known(unit.element));
    let mut slots: Vec<u64> = value["command"]["magic_slots"].as_array().into_iter().flatten().filter_map(Value::as_u64).filter(|k| *k != 0 && !gone(*k)).collect();
    slots.resize(6, 0);
    value["command"]["magic_slots"] = json!(slots);
    value["command"]["tier_chosen"] = json!(vec![false; 6]);
    for page in value["command"]["shortcuts"].as_array_mut().into_iter().flatten() {
        for kind in page.as_array_mut().into_iter().flatten() {
            if kind.as_u64().is_some_and(gone) {
                *kind = json!(0);
            }
        }
    }
    for page in value["shortcut_names"].as_array_mut().into_iter().flatten() {
        for name in page.as_array_mut().into_iter().flatten() {
            if name.as_str().and_then(spells::command_by_name).is_some_and(|k| gone(k as u64)) {
                *name = json!("");
            }
        }
    }
    // The pouch: three slots.
    let slots_now = start.item_slots.max(1) as usize;
    value["item_slots"] = json!(slots_now);
    for (index, slot) in value["pouch"]["slots"].as_array_mut().into_iter().flatten().enumerate() {
        if index >= slots_now {
            *slot = json!({ "item": null, "exists": false, "fixed": false, "auto_reload": false, "used": false });
        }
    }
    let mut held: Vec<(String, u64)> = Vec::new();
    for slot in value["pouch"]["slots"].as_array().into_iter().flatten() {
        if let (Some(item), false) = (slot["item"].as_str(), slot["used"].as_bool().unwrap_or(false)) {
            match held.iter_mut().find(|h| h.0 == item) {
                Some(entry) => entry.1 += 1,
                None => held.push((item.to_string(), 1)),
            }
        }
    }
    if value["pouch"]["filled"].as_bool() == Some(true) {
        value["items"] = held.iter().map(|(item, count)| json!({ "item": item, "count": count })).collect();
    }
    // (a pouch that was never filled is filled on load from `items`, which then takes the first three)
    Ok(())
}

// ---------------------------------------------------------------------------------------------- the files

/// A file name for a host character: `slot<N>_<name>_<hash>`; the name's letters and digits only (anything
/// else becomes `_`), the hash (FNV-1a of the exact name, 8 hex digits) keeps names apart that read the same
/// after that. `slot` = the host's save slot (negative: unknown, left out).
pub fn character_key(name: &str, slot: i32) -> String {
    let mut hash: u32 = 0x811C_9DC5;
    for byte in name.as_bytes() {
        hash = (hash ^ *byte as u32).wrapping_mul(0x0100_0193);
    }
    let readable: String = name.chars().take(24).map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    let readable = if readable.is_empty() { "unnamed".to_string() } else { readable };
    if slot >= 0 {
        format!("slot{slot}_{readable}_{hash:08x}")
    } else {
        format!("{readable}_{hash:08x}")
    }
}

pub fn save_path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.{EXTENSION}"))
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".bak");
    PathBuf::from(name)
}

/// Writes `save` to `path` atomically, keeping the file that was there as `<path>.bak`.
pub fn write(path: &Path, save: &SoraSave) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(save.to_json().as_bytes())?;
        file.sync_all()?;
    }
    // The file before becomes the one backup, but only when it is a save that can be read: a damaged file
    // must not replace a good backup.
    if std::fs::read_to_string(path).ok().is_some_and(|text| SoraSave::from_json(&text).is_ok()) {
        std::fs::rename(path, backup_path(path))?;
    }
    std::fs::rename(&temporary, path)
}

/// What [`load`] found.
#[derive(Debug, PartialEq)]
pub enum Loaded {
    Save(SoraSave),
    /// The file was unusable; this is the backup.
    Backup(SoraSave),
    /// No file (or nothing usable): a new game. The host may write.
    NewGame,
    /// The file is of a newer version: a new game IN MEMORY ONLY; the host must not write over it.
    Refused,
}

/// Reads the save of `key` in `dir`; every decision is said through `log`. Never panics.
pub fn load(dir: &Path, key: &str, log: &mut dyn FnMut(&str)) -> Loaded {
    let path = save_path(dir, key);
    let read = |path: &Path| -> Result<SoraSave, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        SoraSave::from_json(&text).map_err(|e| match e {
            SaveError::Newer(v) => format!("NEWER:{v}"),
            other => other.to_string(),
        })
    };
    if !path.exists() && !backup_path(&path).exists() {
        log(&format!("save: no file {} : A NEW GAME for this character", path.display()));
        return Loaded::NewGame;
    }
    match read(&path) {
        Ok(save) => {
            log(&format!("save: {} read: level {}, EXP {}, munny {}, {} s played", path.display(), save.level, save.exp, save.munny, save.play_seconds));
            Loaded::Save(save)
        }
        Err(why) if why.starts_with("NEWER:") => {
            log(&format!("save: {} was written by a newer version ({}): it is left alone; a new game in memory, NOTHING IS WRITTEN this session", path.display(), &why[6..]));
            Loaded::Refused
        }
        Err(why) => {
            log(&format!("save: {} CANNOT BE USED ({why}): trying the backup", path.display()));
            match read(&backup_path(&path)) {
                Ok(save) => {
                    log(&format!("save: the backup {} is used: level {}, EXP {}, {} s played", backup_path(&path).display(), save.level, save.exp, save.play_seconds));
                    Loaded::Backup(save)
                }
                Err(why) => {
                    // The damaged file is kept aside for a look, once.
                    let mut aside = path.as_os_str().to_owned();
                    aside.push(".damaged");
                    let kept = path.exists() && std::fs::rename(&path, PathBuf::from(&aside)).is_ok();
                    log(&format!("save: the backup cannot be used either ({why}): A NEW GAME{}", if kept { " (the damaged file is kept as .damaged)" } else { "" }));
                    Loaded::NewGame
                }
            }
        }
    }
}

// ------------------------------------------------------- (saveload) the slots as the Save / Load screens show them

/// (saveload) The file of a host slot in `dir`: the `slot<N>_*.khsave` there (the newest when several
/// characters were ever stored under that slot number). `None`: Sora has no file for that slot.
pub fn slot_file(dir: &Path, slot: usize) -> Option<PathBuf> {
    let prefix = format!("slot{slot}_");
    let suffix = format!(".{EXTENSION}");
    let mut found: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&prefix) || !name.ends_with(&suffix) {
            continue;
        }
        let modified = entry.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
        if found.as_ref().is_none_or(|(at, _)| modified > *at) {
            found = Some((modified, entry.path()));
        }
    }
    found.map(|(_, path)| path)
}

/// (saveload) What a save says about its slot: Sora's level, munny and play time and the character's name;
/// the slot is `Used`. The HOST adds what only it knows: `location` (and `world`, `mode`, `clear`,
/// `is_current`, and a play time of its own if it keeps one).
pub fn slot_summary_of(index: usize, save: &SoraSave) -> kh_ui::saveload::SlotSummary {
    kh_ui::saveload::SlotSummary {
        index,
        state: kh_ui::saveload::SlotState::Used,
        name: save.character.clone(),
        level: save.level,
        munny: save.munny.max(0) as u32,
        play_time_seconds: save.play_seconds,
        // (party P) the picture of the party leader who made the save (the user's request); a save without a party
        // record (every save before version 4) reads as Sora
        // (SV, ERT4) the leader the record names once it is true to its unlocks (a Goofy start's title-time file named Sora);
        // any roster character has a picture (his roster key: a new character needs no new case)
        portrait: portrait_of(crate::formation::record_leader(&save.party)),
        // (DF) KH3's row shows the difficulty (kh3_menu.save.difficulty_N) for a save that is not Standard; a Standard save
        // (every save before DF) keeps the row as it was (dec032: "keep everything as it was on 'normal' mode")
        mode: match save.game_level(GameLevel::Standard) {
            GameLevel::Standard => kh_ui::saveload::SlotMode::None,
            level => kh_ui::saveload::SlotMode::Difficulty(level as u8),
        },
        ..kh_ui::saveload::SlotSummary::default()
    }
}

/// (SV) The slot card's face of a party leader (`None`: no party record = Sora).
pub fn portrait_of(leader: Option<crate::party::PartyChara>) -> kh_ui::saveload::Portrait {
    use crate::party::PartyChara as C;
    match leader {
        None | Some(C::Sora) => kh_ui::saveload::Portrait::Sora,
        Some(C::Donald) => kh_ui::saveload::Portrait::Donald,
        Some(C::Goofy) => kh_ui::saveload::Portrait::Goofy,
        #[allow(unreachable_patterns)]
        Some(other) => kh_ui::saveload::Portrait::Chara(crate::formation::key_of(other)),
    }
}

/// (saveload) The summary of the `.khsave` at `path` for slot `index`: [`slot_summary_of`] of a readable file
/// (its backup when the file itself is damaged), else an `Empty` slot. Reads only; never writes or renames.
pub fn slot_summary(index: usize, path: &Path) -> kh_ui::saveload::SlotSummary {
    let read = |path: &Path| std::fs::read_to_string(path).ok().and_then(|text| SoraSave::from_json(&text).ok());
    match read(path).or_else(|| read(&backup_path(path))) {
        Some(save) => slot_summary_of(index, &save),
        None => kh_ui::saveload::SlotSummary::empty(index),
    }
}

/// (saveload) `count` summaries (slots 0 .. count) from the `.khsave` files of `dir` ([`slot_file`] +
/// [`slot_summary`]): what a host hands to `Hud::open_save` / `Hud::open_load` after adding its own fields.
/// NOTE for a host whose own save knows more slots than Sora has files for (a character that never had a
/// Sora): such a slot comes back `Empty` here; the host decides what it shows for it.
pub fn slot_summaries(dir: &Path, count: usize) -> Vec<kh_ui::saveload::SlotSummary> {
    (0..count)
        .map(|index| match slot_file(dir, index) {
            Some(path) => slot_summary(index, &path),
            None => kh_ui::saveload::SlotSummary::empty(index),
        })
        .collect()
}

// ------------------------------------------------------- (SV) deleting a slot's data (the user 2026-10-17)

/// (SV) The folder a deleted slot's files are moved into: `<dir>\deleted\<stamp>_slot<N>` (LOCAL: KH3 deletes for good;
/// this port keeps the files aside, out of every list, so a delete made by mistake can still be undone by hand).
pub fn trash_dir(dir: &Path, slot: usize, stamp: &str) -> PathBuf {
    dir.join("deleted").join(format!("{stamp}_slot{slot}"))
}

/// (SV) Every file of host slot `slot` in `dir`: the names that begin with `slot<N>_` (the `.khsave`, its `.bak`, the
/// `.points` marker, `.old*` copies, any `.tmp`). Sorted. Nothing of another slot (`slot1_` never matches `slot10_`).
pub fn slot_files(dir: &Path, slot: usize) -> Vec<PathBuf> {
    let prefix = format!("slot{slot}_");
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_file()) && e.file_name().to_string_lossy().starts_with(&prefix)).map(|e| e.path()).collect();
    out.sort();
    out
}

/// (SV) DELETES SLOT `slot`'s DATA: every file of [`slot_files`] is moved into [`trash_dir`] (renamed: the bytes are not
/// touched, nothing is written, no other file is opened). Returns where each went. A slot without files is not an
/// error (nothing to move). On a failure part-way the files moved so far stay in the trash and the error is returned.
pub fn delete_slot_files(dir: &Path, slot: usize, stamp: &str) -> std::io::Result<Vec<(PathBuf, PathBuf)>> {
    let files = slot_files(dir, slot);
    let mut moved = Vec::new();
    if files.is_empty() {
        return Ok(moved);
    }
    let trash = trash_dir(dir, slot, stamp);
    std::fs::create_dir_all(&trash)?;
    for from in files {
        let Some(name) = from.file_name() else { continue };
        let to = trash.join(name);
        std::fs::rename(&from, &to)?;
        moved.push((from, to));
    }
    Ok(moved)
}

#[cfg(test)]
mod delete_tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kh-host-sv-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn bytes_of(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut out: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir).unwrap().flatten().filter(|e| e.file_type().unwrap().is_file()).map(|e| (e.file_name().to_string_lossy().to_string(), std::fs::read(e.path()).unwrap())).collect();
        out.sort();
        out
    }

    /// (SV) Delete one slot: only its files leave (into the trash, byte for byte); every other file stays byte-identical.
    #[test]
    fn deleting_one_slot_leaves_every_other_file_byte_identical() {
        let dir = folder("delete");
        for slot in [0usize, 1, 2, 10] {
            let key = character_key("Sora", slot as i32);
            std::fs::write(save_path(&dir, &key), format!("{{\"slot\":{slot}}}")).unwrap();
            std::fs::write(dir.join(format!("{key}.{EXTENSION}.bak")), format!("backup {slot}")).unwrap();
            std::fs::write(dir.join(format!("{key}.points")), "marker").unwrap();
        }
        std::fs::write(dir.join("unrelated.txt"), "keep").unwrap();
        let before: Vec<(String, Vec<u8>)> = bytes_of(&dir);
        let slot1: Vec<(String, Vec<u8>)> = before.iter().filter(|(n, _)| n.starts_with("slot1_")).cloned().collect();
        assert_eq!(slot1.len(), 3);
        let moved = delete_slot_files(&dir, 1, "20261017-010000").unwrap();
        assert_eq!(moved.len(), 3);
        let after = bytes_of(&dir);
        // every other file, slot 10's included, is byte-identical; slot 1 has nothing left
        assert_eq!(after, before.iter().filter(|(n, _)| !n.starts_with("slot1_")).cloned().collect::<Vec<_>>());
        // the trash holds exactly slot 1's files, byte for byte
        assert_eq!(bytes_of(&trash_dir(&dir, 1, "20261017-010000")), slot1);
        // the lists see an empty slot 1 and the others as before
        assert!(slot_file(&dir, 1).is_none());
        assert!(slot_file(&dir, 0).is_some() && slot_file(&dir, 2).is_some() && slot_file(&dir, 10).is_some());
        // a slot without files: nothing moved, no error, no trash folder
        assert!(delete_slot_files(&dir, 5, "x").unwrap().is_empty());
        assert!(!trash_dir(&dir, 5, "x").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (SV, ERT4) The slot card shows the true leader: a Goofy start's title-time file (leader "sora", unlocked [goofy]) shows
    /// Goofy; a written and read-back file the same; an old file without a record shows Sora.
    #[test]
    fn the_slot_card_shows_the_leader_true_to_the_unlocks() {
        use kh_ui::saveload::Portrait;
        let sheet = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-loadout-99.json");
        let base = crate::rig::loadout::parse(sheet, &std::fs::read_to_string(sheet).expect("the loadout sheet")).expect("parse").0;
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: glam::Vec3::ZERO, yaw: 0.0 }).with_status(Status::from_loadout(&new_game_loadout(&base, None)));
        let mut save = SoraSave::capture(&sora, &Inventory::default(), "Sora", 10, 0);
        save.party = serde_json::json!({ "format": 1, "leader": "sora", "active": ["sora", "donald", "goofy"], "unlocked": ["goofy"], "starter": "goofy" });
        assert_eq!(slot_summary_of(2, &save).portrait, Portrait::Goofy);
        let dir = folder("portrait");
        let path = save_path(&dir, &character_key("Sora", 2));
        write(&path, &save).unwrap();
        assert_eq!(slot_summary(2, &path).portrait, Portrait::Goofy);
        save.party = serde_json::json!({ "format": 2, "leader": "donald", "active": ["donald", "goofy"], "unlocked": ["donald", "goofy"], "starter": "goofy" });
        assert_eq!(slot_summary_of(2, &save).portrait, Portrait::Donald);
        save.party = serde_json::Value::Null;
        assert_eq!(slot_summary_of(2, &save).portrait, Portrait::Sora);
        assert_eq!(portrait_of(Some(crate::party::PartyChara::Goofy)), Portrait::Goofy);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (SV) Save -> load: the state read back is the state written (every field the format knows, other streams' ones
    /// included: the comparison is of the whole written record).
    #[test]
    fn a_save_reads_back_identical() {
        let dir = folder("roundtrip");
        let sheet = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-loadout-99.json");
        let base = crate::rig::loadout::parse(sheet, &std::fs::read_to_string(sheet).expect("the loadout sheet")).expect("parse").0;
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: glam::Vec3::ZERO, yaw: 0.0 }).with_status(Status::from_loadout(&new_game_loadout(&base, None)));
        let mut save = SoraSave::capture(&sora, &Inventory::default(), "Sora", 1234, 98765);
        save.munny = 4321;
        let path = save_path(&dir, &character_key("Sora", 3));
        write(&path, &save).unwrap();
        let Loaded::Save(back) = load(&dir, &character_key("Sora", 3), &mut |_| {}) else { panic!("not read back") };
        assert_eq!(back.to_json(), save.to_json());
        // written again from what was read: byte-identical file
        let first = std::fs::read(&path).unwrap();
        write(&path, &back).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), first);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod saveload_tests {
    use super::*;
    use kh_ui::saveload::SlotState;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kh-host-saveload-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_slot_file_gives_its_summary_and_a_missing_or_damaged_one_an_empty_slot() {
        let dir = folder("summary");
        let sheet = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-loadout-99.json");
        let base = crate::rig::loadout::parse(sheet, &std::fs::read_to_string(sheet).expect("the loadout sheet")).expect("parse").0;
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: glam::Vec3::ZERO, yaw: 0.0 }).with_status(Status::from_loadout(&new_game_loadout(&base, None)));
        let mut save = SoraSave::capture(&sora, &Inventory::default(), "Sora", 3723, 0);
        save.level = 17;
        save.munny = 4321;
        write(&save_path(&dir, &character_key("Sora", 3)), &save).unwrap();
        std::fs::write(save_path(&dir, &character_key("Sora", 5)), "not a save").unwrap();
        let slots = slot_summaries(&dir, 10);
        assert_eq!(slots.len(), 10);
        for (index, slot) in slots.iter().enumerate() {
            assert_eq!(slot.index, index);
            assert_eq!(slot.state, if index == 3 { SlotState::Used } else { SlotState::Empty }, "slot {index}");
        }
        let three = &slots[3];
        assert_eq!((three.name.as_str(), three.level, three.munny, three.play_time_seconds), ("Sora", 17, 4321, 3723));
        assert_eq!(kh_ui::saveload::play_time_text(three.play_time_seconds), "1:02");
        // slot 30 must not be taken for slot 3 and the other way round
        assert!(slot_file(&dir, 30).is_none());
        assert_eq!(slot_file(&dir, 3).unwrap().file_name().unwrap().to_string_lossy(), format!("{}.khsave", character_key("Sora", 3)));
        // the damaged file was not touched
        assert_eq!(std::fs::read_to_string(save_path(&dir, &character_key("Sora", 5))).unwrap(), "not a save");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Loadout {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-loadout-99.json");
        let text = std::fs::read_to_string(path).expect("the loadout sheet");
        crate::rig::loadout::parse(path, &text).expect("parse").0
    }

    fn sora_of(loadout: &Loadout) -> Sora {
        let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: glam::Vec3::ZERO, yaw: 0.0 }).with_status(Status::from_loadout(loadout));
        kh_core::command::customize::ensure(&mut sora);
        sora
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("khsave_test_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_new_game_is_level_1_with_level_1_numbers() {
        let base = base();
        let new = Status::from_loadout(&new_game_loadout(&base, None));
        let full = Status::from_loadout(&base);
        assert_eq!(new.level, 1);
        assert_eq!(new.exp, 0);
        // (the ER4 finding: `set_level(1)` alone left HP at the level-99 loadout's 265)
        assert!(new.max_hp() < full.max_hp(), "new game HP {} against the full loadout's {}", new.max_hp(), full.max_hp());
        assert!(new.max_mp() < full.max_mp());
        assert!(new.magic.iter().all(|m| m.tier == 1), "{:?}", new.magic);
        assert!(new.abilities.ap_in_use() <= new.max_ap());
        assert!(new.abilities.iter_owned().count() < full.abilities.iter_owned().count());
    }

    #[test]
    fn round_trip_through_the_file() {
        let base = base();
        let mut sora = sora_of(&new_game_loadout(&base, None));
        // He played: EXP to a few levels up, munny, an item used, an ability taken off, HP lost.
        sora.status_mut().give_exp(5000, 1.0);
        sora.status_mut().munny = 1234;
        sora.status_mut().hp -= 7;
        sora.magic_mut().pouch.stock_add(ItemKind::ALL[0], 3);
        let first = sora.status().abilities.iter_equipped().next().map(|(kind, _)| kind);
        if let Some(kind) = first {
            sora.status_mut().abilities.set_copy(kind, 0, false);
            sora.status_mut().rebuild();
        }
        sora.command_mut().shortcuts[1][2] = sora.command().magic_slots[0];
        let mut inventory = Inventory::default();
        inventory.add("PRT_ITEM01", 2);
        inventory.spare_weapons[0] = Some("WEP_KEYBLADE_SO_02".to_string());
        assert!(sora.status().level > 1);

        let dir = scratch("round");
        let save = SoraSave::capture(&sora, &inventory, "Tarnished", 321, 55_000);
        let path = save_path(&dir, &character_key("Tarnished", 0));
        write(&path, &save).unwrap();
        let mut lines = Vec::new();
        let loaded = load(&dir, &character_key("Tarnished", 0), &mut |l| lines.push(l.to_string()));
        assert_eq!(loaded, Loaded::Save(save.clone()), "{lines:?}");

        // Applied to a fresh Sora it gives the same Sora again.
        let mut again = sora_of(&base);
        let mut inventory_again = Inventory::default();
        save.apply(&base, &mut again, &mut inventory_again);
        let recaptured = SoraSave::capture(&again, &inventory_again, "Tarnished", 321, 55_000);
        assert_eq!(recaptured, save);
        let (a, b) = (sora.status(), again.status());
        assert_eq!((a.level, a.exp, a.munny, a.hp, a.mp, a.focus), (b.level, b.exp, b.munny, b.hp, b.mp, b.focus));
        assert_eq!((a.max_hp(), a.max_mp(), a.strength(), a.magic(), a.defense(), a.max_ap(), a.ap_in_use()), (b.max_hp(), b.max_mp(), b.strength(), b.magic(), b.defense(), b.max_ap(), b.ap_in_use()));
        assert_eq!(sora.command().shortcuts, again.command().shortcuts);
        assert_eq!(sora.magic().pouch, again.magic().pouch);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_level_99_loadout_round_trips_too() {
        let base = base();
        let sora = sora_of(&base);
        let save = SoraSave::capture(&sora, &Inventory::default(), "x", 0, 0);
        let again = SoraSave::from_json(&save.to_json()).unwrap();
        assert_eq!(again, save);
        let status = again.status(&base);
        let was = sora.status();
        assert_eq!((status.level, status.max_hp(), status.max_mp(), status.max_ap(), status.ap_in_use(), status.strength()), (was.level, was.max_hp(), was.max_mp(), was.max_ap(), was.ap_in_use(), was.strength()));
        assert_eq!(status.bonuses, was.bonuses);
    }

    #[test]
    fn a_second_write_keeps_one_backup_and_a_damaged_file_falls_back_to_it() {
        let base = base();
        let sora = sora_of(&new_game_loadout(&base, None));
        let dir = scratch("backup");
        let key = character_key("Någon / else", 3);
        assert!(key.starts_with("slot3_N") && !key.contains('/') && !key.contains(' '), "{key}");
        let path = save_path(&dir, &key);
        let mut one = SoraSave::capture(&sora, &Inventory::default(), "Någon / else", 10, 0);
        write(&path, &one).unwrap();
        assert!(!backup_path(&path).exists());
        one.munny = 77;
        one.play_seconds = 20;
        write(&path, &one).unwrap();
        assert!(backup_path(&path).exists());
        assert!(!dir.join(format!("{key}.khsave.tmp")).exists());
        // The file is damaged (a crash in the middle of a copy, a disk fault): the backup is used.
        std::fs::write(&path, b"{ \"version\": 1, \"level\": ").unwrap();
        let mut lines = Vec::new();
        match load(&dir, &key, &mut |l| lines.push(l.to_string())) {
            Loaded::Backup(save) => assert_eq!((save.munny, save.play_seconds), (0, 10)),
            other => panic!("{other:?}"),
        }
        assert!(lines.iter().any(|l| l.contains("CANNOT BE USED")) && lines.iter().any(|l| l.contains("backup")), "{lines:?}");
        // A write over the damaged file does not put it in the backup's place.
        write(&path, &one).unwrap();
        assert!(SoraSave::from_json(&std::fs::read_to_string(backup_path(&path)).unwrap()).is_ok());
        // Both damaged: a new game, said in the log, no panic; the damaged file is kept aside.
        std::fs::write(&path, b"\x00\x01garbage").unwrap();
        std::fs::write(backup_path(&path), b"").unwrap();
        lines.clear();
        assert_eq!(load(&dir, &key, &mut |l| lines.push(l.to_string())), Loaded::NewGame);
        assert!(lines.iter().any(|l| l.contains("A NEW GAME")), "{lines:?}");
        assert!(dir.join(format!("{key}.khsave.damaged")).exists());
        // No file at all: a new game.
        assert_eq!(load(&dir, "nobody", &mut |_| {}), Loaded::NewGame);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn versions() {
        let base = base();
        let save = SoraSave::capture(&sora_of(&base), &Inventory::default(), "x", 0, 0);
        // The upgrade path runs for every read (a stub with one version: the text of version 1 reads as itself).
        let mut value: Value = serde_json::from_str(&save.to_json()).unwrap();
        assert_eq!(SoraSave::from_json(&value.to_string()).unwrap(), save);
        // A newer file is refused, not misread, and the loader says not to write over it.
        value["version"] = json!(VERSION + 1);
        assert_eq!(SoraSave::from_json(&value.to_string()), Err(SaveError::Newer(VERSION + 1)));
        let dir = scratch("newer");
        std::fs::write(save_path(&dir, "k"), value.to_string()).unwrap();
        assert_eq!(load(&dir, "k", &mut |_| {}), Loaded::Refused);
        // No version, version 0, a missing field, not JSON: errors, never a panic.
        value["version"] = json!(0);
        assert_eq!(SoraSave::from_json(&value.to_string()), Err(SaveError::NoVersion));
        value["version"] = json!(1);
        value.as_object_mut().unwrap().remove("abilities");
        assert_eq!(SoraSave::from_json(&value.to_string()), Err(SaveError::Field("abilities")));
        assert!(matches!(SoraSave::from_json("not json"), Err(SaveError::NotJson(_))));
        assert_eq!(SoraSave::from_json("{}"), Err(SaveError::NoVersion));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

// ---------------------------------------------------------------------------------------------- (new game)
/// (new game) The true start, levelling by the level table, rewards, the pair in the save, the version-1 upgrade.
#[cfg(test)]
mod newgame_tests {
    use super::*;
    use kh_core::magic::spells::Element;
    use kh_core::status::BonusLine;
    use ETresAbilityKind as A;

    const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../");

    fn json(path: &str) -> Value {
        serde_json::from_str(&std::fs::read_to_string(format!("{ROOT}{path}")).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap()
    }

    fn base() -> Loadout {
        let path = format!("{ROOT}sheets/port/sora-loadout-99.json");
        crate::rig::loadout::parse(&path, &std::fs::read_to_string(&path).unwrap()).unwrap().0
    }

    fn kind(name: &str) -> A {
        A::from_game_name(name.trim_start_matches("ETresAbilityKind::")).unwrap_or(A::None)
    }

    fn new_sora(status: Status) -> Sora {
        let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: glam::Vec3::ZERO, yaw: 0.0 }).with_status(status);
        kh_core::command::customize::ensure(&mut sora);
        sora
    }

    /// (kind, copies) lists in ascending kind order.
    fn owned(status: &Status) -> Vec<(A, u8)> {
        status.abilities.iter_owned().collect()
    }

    fn equipped(status: &Status) -> Vec<(A, u8)> {
        status.abilities.iter_equipped().collect()
    }

    /// THE GAME'S START DATA, read here from the data asset itself (not from the port's sheet):
    /// `TresChrInitData.m_PlayerSora` -> (m_EquipAbility, m_HaveAbility).
    fn init_data() -> (Vec<A>, Vec<A>) {
        let data = json("sheets/entities/Load/Common/TresChrInitData.json");
        let props = data["objects"][0]["m_PlayerSora"]["props"].as_array().expect("m_PlayerSora").clone();
        let list = |name: &str| -> Vec<A> { props.iter().find(|p| p["tag"]["name"] == name).map(|p| p["value"]["items"].as_array().unwrap().iter().map(|i| kind(i.as_str().unwrap())).collect()).unwrap_or_default() };
        (list("m_EquipAbility"), list("m_HaveAbility"))
    }

    /// The level table of the game's data asset: per level (EXP, Strength, Magic, Defense, AP, [ability per path]).
    fn level_table() -> Vec<(i32, i32, i32, i32, i32, [A; 3])> {
        let table = json("sheets/entities/DataTable/Player/LevelUp/p_ex001_LevelUpData.json");
        let rows: Vec<_> = table["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                let n = |f: &str| r[f].as_i64().unwrap() as i32;
                (n("m_Exp"), n("m_AttackPower"), n("m_MagicPower"), n("m_DefensePower"), n("m_AbilityPoint"), [kind(r["m_AbilityKind1"].as_str().unwrap()), kind(r["m_AbilityKind2"].as_str().unwrap()), kind(r["m_AbilityKind3"].as_str().unwrap())])
            })
            .collect();
        assert_eq!(rows.len(), 99);
        rows
    }

    #[test]
    fn a_new_game_is_exactly_the_games_start() {
        let (equip, have) = init_data();
        assert_eq!((equip.len(), have), (16, vec![A::Expzero]));
        let loadout = new_game_loadout(&base(), None);
        let status = Status::from_loadout(&loadout);
        // The sheet's stated numbers are what the rules give.
        assert_eq!(loadout.verify(&status), Vec::<String>::new());
        // Abilities: exactly the start set, one copy each; the 16 equipped, Zero EXP owned only.
        let mut all: Vec<A> = equip.iter().chain(&[A::Expzero]).copied().collect();
        all.sort_by_key(|k| k.value());
        assert_eq!(owned(&status), all.iter().map(|k| (*k, 1)).collect::<Vec<_>>());
        let mut on = equip.clone();
        on.sort_by_key(|k| k.value());
        assert_eq!(equipped(&status), on.iter().map(|k| (*k, 1)).collect::<Vec<_>>());
        // None of what the old level-1 loadout had for nothing.
        for gone in [A::Glide, A::Doubleflight, A::Highjump, A::AirRecovery, A::Superjump, A::GuardCounter, A::ComboPlus, A::FocusAspir, A::MpSafety] {
            assert_eq!(status.abilities.owned(gone), 0, "{gone:?}");
        }
        // The Kingdom Key's own ability works without AP; the start set fills the AP exactly.
        assert_eq!((status.abilities.worn(A::PrizeDraw), status.count(A::PrizeDraw)), (1, 1));
        assert_eq!((status.level, status.exp, status.exp_next, status.ap_in_use(), status.max_ap()), (1, 0, 80, 28, 28));
        assert_eq!((status.strength(), status.magic(), status.defense(), status.max_focus(), status.focus), (15, 15, 8, 100, 100));
        assert_eq!((status.max_hp(), status.hp, status.max_mp(), status.mp), (105, 105, 110, 110));
        // Magic: Fire and Water of the prologue's Darkside bonus, nothing else; the command menu lists them in that order.
        assert_eq!(status.magic.iter().map(|m| (m.name.as_str(), m.tier)).collect::<Vec<_>>(), vec![("fire", 1), ("water", 1)]);
        let mut command = kh_core::command::CommandState::default();
        command.fill(&status);
        assert_eq!(command.magic_slots, [Element::Fire.command(1), Element::Water.command(1), 0, 0, 0, 0]);
        assert_eq!(command.shortcuts, [[0; 4]; 3]);
        // Pouch: three slots, nothing in them, no stock. Kingdom Key, no armour or accessory, no munny.
        let pouch = Pouch::from_status(&status);
        assert_eq!((status.item_slots, pouch.slot_count(), pouch.list().len(), pouch.stock.len(), status.items.len()), (3, 3, 0, 0, 0));
        assert_eq!((status.weapon.as_str(), status.weapon_upgrade, status.armour.len(), status.accessories.len(), status.munny), ("WEP_KEYBLADE_SO_00", 0, 0, 0, 0));
        assert_eq!(status.shotlocks.len(), 2);
        assert_eq!(status.action_flags, 0xFF, "the script switches (GW+0x95B0) start all on");
        // The testing key's level 1 is the same start.
        assert_eq!(Status::from_loadout(&test_loadout(&base(), 1, None)), status);
        // Critical: base halved, the three Critical abilities, 30 AP more.
        let mut critical = base();
        critical.game_level = "CRITICAL".to_string();
        let status = Status::from_loadout(&new_game_loadout(&critical, None));
        assert_eq!((status.max_hp(), status.max_mp(), status.max_ap(), status.abilities.owned(A::CriticalConverter), status.abilities.equipped(A::CriticalCounter)), (52, 55, 58, 1, 0)); // (DF, dec032) no Critical Counter at the start
    }

    #[test]
    fn the_mirrors_of_the_choice_menu_are_the_pairs() {
        use kh_ui::newgame::NewGamePick;
        // Left, centre, right: Wisdom, Vitality, Balance and the guardian, the warrior, the mystic.
        let pair = |mirrors| crate::hud::Hud::choice_of_pick(NewGamePick { mirrors });
        assert_eq!(pair([0, 0]), OpeningChoice { desire: Growth::Wisdom, power: Power::Guardian });
        assert_eq!(pair([1, 1]), OpeningChoice { desire: Growth::Vitality, power: Power::Warrior });
        assert_eq!(pair([2, 2]), OpeningChoice { desire: Growth::Balance, power: Power::Mystic });
        let all: std::collections::HashSet<OpeningChoice> = (0..9).map(|k| pair([k / 3, k % 3])).collect();
        assert_eq!(all.len(), 9);
    }

    #[test]
    fn the_nine_opening_pairs() {
        let (base, table) = (base(), level_table());
        let mut firsts = Vec::new();
        for choice in OpeningChoice::all() {
            let status = Status::from_loadout(&new_game_loadout_for(&base, choice));
            // The first choice: max HP / MP only. The second: the level table's column only.
            let (hp, mp) = match choice.desire {
                Growth::Vitality => (120, 100),
                Growth::Balance => (105, 110),
                Growth::Wisdom => (90, 120),
            };
            assert_eq!((status.max_hp(), status.max_mp(), status.hp, status.mp), (hp, mp, hp, mp), "{choice:?}");
            assert_eq!(choice.start_hp_mp(), (hp, mp));
            assert_eq!((status.growth, status.ability_path), (Some(choice.desire), choice.power.path()));
            assert_eq!((status.strength(), status.magic(), status.defense(), status.max_ap(), status.ap_in_use(), status.abilities.iter_owned().count()), (15, 15, 8, 28, 28, 17), "{choice:?}");
            // Level 2: the first ability of the pair's column, as the data table has it.
            let mut at2 = status.clone();
            let award = at2.give_exp(80, 1.0);
            let first = table[1].5[choice.power.path() as usize];
            assert_eq!(award.level_up.and_then(|l| l.ability), Some(first), "{choice:?}");
            assert_eq!(at2.abilities.owned(first), 1 + status.abilities.owned(first));
            firsts.push((choice.power, first));
        }
        // The three columns start differently: Combo Plus, Magic Combo Thrift, Damage Control.
        for (power, first) in [(Power::Warrior, A::ComboPlus), (Power::Mystic, A::MagicComboSave), (Power::Guardian, A::CriticalHalf)] {
            assert!(firsts.iter().filter(|f| f.0 == power).all(|f| f.1 == first), "{power:?}");
        }
    }

    #[test]
    fn levelling_grants_what_the_level_table_grants() {
        let (base, table) = (base(), level_table());
        // (XSI, 2026-10-16) the port's level table (sheets/port/sora-levels.json): KH3's rows with the six slots X1 emptied
        // filled by the XS abilities (A 12 Spiral Rush, C 18 Critical Counter, 22 Diving Strike, B 44 Light Wave)
        let port_levels: Vec<[A; 3]> = {
            let sheet = json("sheets/port/sora-levels.json");
            let col = |name: &str| sheet["columns"].as_array().unwrap().iter().position(|c| c == name).unwrap();
            let (a, b, c) = (col("ability_a"), col("ability_b"), col("ability_c"));
            let k = |v: &Value| if v.as_str() == Some("-") { A::None } else { kind(v.as_str().unwrap()) };
            sheet["rows"].as_array().unwrap().iter().map(|r| [k(&r[a]), k(&r[b]), k(&r[c])]).collect()
        };
        assert_eq!(port_levels.len(), 99);
        let (equip, have) = init_data();
        let sheet = json("sheets/port/sora-abilities.json");
        let column = |name: &str| sheet["columns"].as_array().unwrap().iter().position(|c| c == name).unwrap();
        let (c_id, c_sources) = (column("id"), column("sources"));
        let top = Status::from_loadout(&base);
        for choice in [OpeningChoice { desire: Growth::Balance, power: Power::Warrior }, OpeningChoice { desire: Growth::Vitality, power: Power::Mystic }, OpeningChoice { desire: Growth::Wisdom, power: Power::Guardian }] {
            let path = choice.power.path() as usize;
            let mut status = Status::from_loadout(&new_game_loadout_for(&base, choice));
            let equipped_at_start = equipped(&status);
            let (hp_max, mp_max) = (status.max_hp(), status.max_mp());
            status.hp -= 30;
            let mut expected = [0u8; kh_core::status::ABILITY_SLOTS]; // (XSI) the port's abilities are 256..
            for k in equip.iter().chain(&have) {
                expected[k.value() as usize] += 1;
            }
            let mut panels = 0;
            for level in 2..=99u32 {
                // Exactly to the next level (an award is at most 999: several awards for the long levels).
                let mut last = None;
                while status.level < level {
                    let award = status.give_exp(status.exp_to_next().min(999), 1.0);
                    assert!(award.added > 0);
                    last = award.level_up.or(last);
                }
                let row = &table[level as usize - 1];
                let before = &table[level as usize - 2];
                // (X1) the levels of an ability the port excludes (decision dec029) grant nothing of it; (XSI) such a slot holds the
                // port's own fill-in (sheet sora-levels), every other slot is KH3's.
                let port = port_levels[level as usize - 1][path];
                let granted = if kh_core::status::tables::is_excluded(row.5[path]) {
                    assert_ne!(port, row.5[path], "level {level} path {path}: the excluded KH3 ability is not granted");
                    port
                } else {
                    assert_eq!(port, row.5[path], "level {level} path {path}: the port keeps KH3's ability");
                    row.5[path]
                };
                if granted != A::None {
                    expected[granted.value() as usize] = (expected[granted.value() as usize] + 1).min(3);
                }
                // The numbers of the row, the panel's lines, the owned list.
                assert_eq!((status.level, status.exp, status.base.strength, status.base.magic, status.base.defense, status.base.ap), (level, row.0, row.1, row.2, row.3, row.4), "level {level}");
                let panel = last.expect("a level-up panel");
                panels += 1;
                assert_eq!((panel.level, panel.strength, panel.magic, panel.defense, panel.ap, panel.ability), (level, row.1 - before.1, row.2 - before.2, row.3 - before.3, row.4 - before.4, (granted != A::None).then_some(granted)), "level {level} path {path}");
                let want: Vec<(A, u8)> = A::ALL.iter().filter(|k| expected[k.value() as usize] > 0).map(|k| (*k, expected[k.value() as usize])).collect();
                assert_eq!(owned(&status), want, "level {level} path {path}");
            }
            assert_eq!(panels, 98);
            // AS IN THE GAME a learnt ability is owned, not equipped: the equipped set is the start's.
            assert_eq!(equipped(&status), equipped_at_start);
            // A level-up refills nothing and does not change max HP / MP.
            assert_eq!((status.max_hp(), status.max_mp(), status.hp), (hp_max, mp_max, hp_max - 30));
            // LEVEL 99 AGAINST sora-loadout-99 minus everything that is not "start" or "level": per ability of
            // the catalogue the copies its sources "start (...)" and "level A:.. B:.. C:.." give.
            let mut from_sheet: Vec<(A, u8)> = Vec::new();
            for row in sheet["rows"].as_array().unwrap().iter().filter(|row| row[column("port")] != "excluded") {
                let id = row[c_id].as_str().unwrap();
                let (mut copies, mut copies_a) = (0u8, 0u8);
                for source in row[c_sources].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    if source.starts_with("start (") && !source.contains("ritical") {
                        copies += 1;
                        copies_a += 1;
                    } else if let Some(levels) = source.strip_prefix("level ") {
                        // (XSI) a path without the ability names no level ("B:")
                        let on = |letter: &str| levels.split_whitespace().find(|p| p.starts_with(letter)).unwrap_or_else(|| panic!("{source}"))[2..].split('+').filter(|l| !l.is_empty()).count() as u8;
                        copies += on(["A:", "B:", "C:"][path]);
                        copies_a += on("A:");
                    }
                }
                if copies > 0 {
                    let kind = kind(id);
                    // (the level-99 loadout is path A's: an XS level ability of path B / C only is not in it)
                    assert!(top.abilities.owned(kind) >= copies_a.min(3), "{kind:?}: the level-99 loadout owns fewer than its start + level sources");
                    from_sheet.push((kind, copies.min(3)));
                }
            }
            from_sheet.sort_by_key(|a| a.0.value());
            assert_eq!(owned(&status), from_sheet, "path {path}");
            // The 17 start copies + the 28 copies of the level table on every path, less the two of the abilities the port
            // excludes (X1: ATTRACTION_TIME and ATTRACTION_UP, one each per path); the stats of the level-99 sheet.
            // (XSI) + two XS level abilities on every path (A: Spiral Rush, Diving Strike; B: Diving Strike, Light Wave; C: Critical
            // Counter, Diving Strike): 17 + 28.
            assert_eq!(owned(&status).iter().map(|a| a.1 as u32).sum::<u32>(), 17 + 28);
            assert_eq!((status.strength(), status.magic(), status.defense(), status.max_ap()), (top.strength(), top.magic(), top.defense(), top.max_ap()));
            assert_eq!((status.strength(), status.magic(), status.defense(), status.max_ap()), (66, 67, 59, 180));
        }
    }

    /// (X1) A SAVE THAT HOLDS AN ABILITY THE PORT EXCLUDES (an Attraction Extender / Enhancer of a build before decision
    /// dec029) loads cleanly: the ability is dropped without a word, its equipped AP is free again, nothing else changes,
    /// and the next save no longer names it. The save format is the same.
    #[test]
    fn a_save_with_an_excluded_ability_loads_without_it() {
        let base = base();
        let choice = OpeningChoice { desire: Growth::Balance, power: Power::Warrior };
        let mut sora = new_sora(Status::from_loadout(&new_game_loadout_for(&base, choice)));
        // Path A, level 30: KH3 grants Attraction Enhancer at 12 and Attraction Extender at 22; the port grants neither.
        while sora.status().level < 30 {
            let next = sora.status().exp_to_next().min(999);
            let award = sora.status_mut().give_exp(next, 1.0);
            if let Some(up) = award.level_up {
                assert!(!matches!(up.ability, Some(A::AttractionUp | A::AttractionTime)), "level {}", up.level);
            }
        }
        assert_eq!((sora.status().abilities.owned(A::AttractionUp), sora.status().abilities.owned(A::AttractionTime)), (0, 0));
        let clean = SoraSave::capture(&sora, &Inventory::default(), "Tarnished", 5, 6);
        let mut value: Value = serde_json::from_str(&clean.to_json()).unwrap();
        let keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        value["abilities"].as_array_mut().unwrap().push(json!({ "kind": A::AttractionTime.value(), "name": "ATTRACTION_TIME", "owned": 3, "equipped": 0b111 }));
        value["abilities"].as_array_mut().unwrap().push(json!({ "kind": A::AttractionUp.value(), "name": "ATTRACTION_UP", "owned": 1, "equipped": 0b1 }));
        let old = SoraSave::from_json(&value.to_string()).expect("the old save reads");
        let mut loaded = new_sora(Status::from_loadout(&base));
        old.apply(&base, &mut loaded, &mut Inventory::default());
        let mut reference = new_sora(Status::from_loadout(&base));
        clean.apply(&base, &mut reference, &mut Inventory::default());
        let (a, b) = (reference.status(), loaded.status());
        for kind in [A::AttractionTime, A::AttractionUp] {
            assert_eq!((b.abilities.owned(kind), b.abilities.equipped(kind), b.count(kind)), (0, 0, 0), "{kind:?}");
        }
        assert_eq!(owned(b), owned(a));
        assert_eq!((b.level, b.max_ap(), b.ap_in_use(), b.max_hp(), b.max_mp()), (a.level, a.max_ap(), a.ap_in_use(), a.max_hp(), a.max_mp()));
        let again: Value = serde_json::from_str(&SoraSave::capture(&loaded, &Inventory::default(), "Tarnished", 5, 6).to_json()).unwrap();
        assert!(again["abilities"].as_array().unwrap().iter().all(|e| ![A::AttractionTime.value() as i64, A::AttractionUp.value() as i64].contains(&e["kind"].as_i64().unwrap())));
        assert_eq!(again.as_object().unwrap().keys().cloned().collect::<Vec<_>>(), keys, "the save format is unchanged");
    }

    #[test]
    fn progress_and_the_pair_survive_the_save() {
        let base = base();
        let choice = OpeningChoice { desire: Growth::Wisdom, power: Power::Guardian };
        let mut sora = new_sora(Status::from_loadout(&new_game_loadout_for(&base, choice)));
        // Levels (path C: Damage Control at 2, Treasure Magnet at 3, Withstand Combo at 4), then rewards.
        while sora.status().level < 6 {
            let next = sora.status().exp_to_next().min(999);
            sora.status_mut().give_exp(next, 1.0);
        }
        assert_eq!((sora.status().abilities.owned(A::CriticalHalf), sora.status().abilities.equipped(A::CriticalHalf), sora.status().abilities.owned(A::ComboLeave)), (2, 1, 1));
        assert_eq!(grant(&mut sora, Reward::Ability { kind: A::Airslide, copies: 2 }).lines[0], BonusLine::Ability(A::Airslide));
        assert_eq!(grant(&mut sora, Reward::Magic { element: Element::Cure, tier: 0 }).learned, Some(Element::Cure.command(1)));
        assert_eq!(grant(&mut sora, Reward::Magic { element: Element::Fire, tier: 0 }).learned, Some(Element::Fire.command(2)));
        grant(&mut sora, Reward::max_hp(10).unwrap());
        grant(&mut sora, Reward::max_mp(5).unwrap());
        grant(&mut sora, Reward::Ap(2));
        grant(&mut sora, Reward::Bonus(BonusCounts::ITEM_SLOT_UP1));
        grant(&mut sora, Reward::Item { item: ItemKind::Potion, count: 3 });
        grant(&mut sora, Reward::Shotlock("TEST SHOTLOCK"));
        // The player installs the second Damage Control copy and one Air Slide copy (the third stays off).
        let ap_before = sora.status().ap_in_use();
        assert!(sora.status_mut().equip(A::CriticalHalf).is_ok());
        sora.status_mut().abilities.set_copy(A::Airslide, 2, true);
        sora.status_mut().refresh();
        {
            let status = sora.status();
            let level_ap = kh_core::status::tables::level_row(6).ap;
            assert_eq!((status.max_hp(), status.max_mp(), status.max_ap(), status.item_slots, status.ap_in_use()), (90 + 10, 120 + 5, level_ap + 2, 4, ap_before + 4 + 1));
            assert_eq!((status.abilities.owned(A::Airslide), status.abilities.equipped_mask(A::Airslide)), (3, 0b101));
            assert_eq!(sora.command().magic_slots, [Element::Fire.command(2), Element::Water.command(1), Element::Cure.command(1), 0, 0, 0]);
            assert_eq!((sora.magic().pouch.slot_count(), sora.magic().pouch.stock_of(ItemKind::Potion)), (4, 3));
        }
        // Through the file and back into a fresh Sora built from the level-99 sheet.
        let dir = std::env::temp_dir().join(format!("khsave_newgame_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let outcome = grant_and_write(&mut sora, Reward::Ability { kind: A::Glide, copies: 1 }, &Inventory::default(), &dir, "k", "Tarnished", 5, 6).unwrap();
        assert!(outcome.changed);
        let Loaded::Save(save) = load(&dir, "k", &mut |_| {}) else { panic!("no save") };
        assert_eq!((save.version, save.desire.as_str(), save.ability_path, save.choice()), (VERSION, "wisdom", 2, choice));
        let mut again = new_sora(Status::from_loadout(&base));
        save.apply(&base, &mut again, &mut Inventory::default());
        let (a, b) = (sora.status(), again.status());
        assert_eq!((b.growth, b.ability_path, b.level, b.exp), (Some(Growth::Wisdom), 2, a.level, a.exp));
        assert_eq!(owned(b), owned(a));
        for (kind, _) in owned(a) {
            assert_eq!(b.abilities.equipped_mask(kind), a.abilities.equipped_mask(kind), "{kind:?}");
        }
        assert_eq!((b.abilities.owned(A::Glide), b.abilities.equipped(A::Glide)), (1, 0));
        assert_eq!(b.magic, a.magic);
        assert_eq!((b.max_hp(), b.max_mp(), b.max_ap(), b.ap_in_use(), b.item_slots, &b.shotlocks, b.bonuses, b.boosts), (a.max_hp(), a.max_mp(), a.max_ap(), a.ap_in_use(), a.item_slots, &a.shotlocks, a.bonuses, a.boosts));
        assert_eq!((again.command().magic_slots, &again.magic().pouch), (sora.command().magic_slots, &sora.magic().pouch));
        // The path keeps driving the level table after the load: level 7 on path C.
        let next = again.status().exp_to_next();
        let award = again.status_mut().give_exp(next, 1.0);
        assert_eq!(award.level_up.map(|l| (l.level, l.ability)), Some((7, kh_core::status::tables::level_ability(7, 2))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_version_1_file_loses_what_it_had_for_nothing() {
        let base = base();
        // A file as version 1 wrote it for a new game that then reached level 5: the level-99 lists at that level.
        let mut old = base.clone();
        old.set_level(1);
        for known in &mut old.magic {
            known.tier = 1;
        }
        old.bonuses = Some(BonusCounts::NONE);
        let mut sora = new_sora(Status::from_loadout(&old));
        while sora.status().level < 5 {
            let next = sora.status().exp_to_next().min(999);
            sora.status_mut().give_exp(next, 1.0);
        }
        sora.status_mut().munny = 77;
        assert!(sora.status().abilities.owned(A::Glide) == 1 && sora.status().magic.len() == 6);
        let mut value: Value = serde_json::from_str(&SoraSave::capture(&sora, &Inventory::default(), "old", 9, 0).to_json()).unwrap();
        value["version"] = json!(1);
        value.as_object_mut().unwrap().remove("choice");
        let save = SoraSave::from_json(&value.to_string()).expect("the version-1 file is read");
        assert_eq!((save.version, save.desire.as_str(), save.ability_path, save.level, save.munny), (VERSION, "balance", 0, 5, 77));
        let status = save.status(&base);
        // What a true new game on path A has at level 5: the start set + Combo Plus, Treasure Magnet, Air Combo Plus, Auto-Finish.
        let mut walked = Status::from_loadout(&new_game_loadout(&base, None));
        while walked.level < 5 {
            let next = walked.exp_to_next().min(999);
            walked.give_exp(next, 1.0);
        }
        assert_eq!(owned(&status), owned(&walked));
        assert_eq!((status.level, status.exp, status.max_hp(), status.max_mp(), status.max_ap(), status.item_slots), (5, walked.exp, 105, 110, walked.max_ap(), 3));
        assert!(status.ap_within_budget(), "AP {} of {}", status.ap_in_use(), status.max_ap());
        assert_eq!(status.magic.iter().map(|m| (m.name.as_str(), m.tier)).collect::<Vec<_>>(), vec![("fire", 1), ("water", 1)]);
        let mut again = new_sora(Status::from_loadout(&base));
        save.apply(&base, &mut again, &mut Inventory::default());
        assert_eq!(again.command().magic_slots, [Element::Fire.command(1), Element::Water.command(1), 0, 0, 0, 0]);
        assert!(again.command().shortcuts.iter().flatten().all(|k| kh_core::magic::spells::unit(*k).is_none_or(|u| matches!(u.element, Element::Fire | Element::Water))));
        assert_eq!(again.magic().pouch.slot_count(), 3);
        // Written again it is version 2 and reads back the same.
        assert_eq!(SoraSave::from_json(&save.to_json()).unwrap(), save);
    }
}

// ---------------------------------------------------------------------------------------------- (ER party S6)
/// (ER party S6) The version-5 friends record: the round trip, the older versions, the backup, odd records.
#[cfg(test)]
mod party_v5_tests {
    use super::*;

    fn base() -> Loadout {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-loadout-99.json");
        crate::rig::loadout::parse(path, &std::fs::read_to_string(path).expect("the loadout sheet")).expect("parse").0
    }

    fn save_of(base: &Loadout) -> SoraSave {
        let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: glam::Vec3::ZERO, yaw: 0.0 }).with_status(Status::from_loadout(&new_game_loadout(base, None)));
        kh_core::command::customize::ensure(&mut sora);
        SoraSave::capture(&sora, &Inventory::default(), "Tarnished", 60, 1000)
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("khsave_v5_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The party record as kh_partyhost `PartyRuntime::party_record` writes it (format 2 + friends + Zero EXP).
    fn party_v5() -> Value {
        json!({
            "format": 2, "leader": "donald", "active": ["sora", "donald", "goofy"],
            "ai": { "sora": [0, 0, 1, 7], "donald": [3, 2, 0, 5], "goofy": [0, 0, 1, 7] },
            "friends_version": 5,
            "friends": [
                { "chara": "Donald", "weapon": "WEP_DONALD_03", "armour": ["PRT_ITEM01"], "accessories": [null, "ACC_ITEM02"], "items": [["ITEM_POTION", true], [null, false]], "slots": [3, 1, 2], "abilities": [["DONALD_FIRE", 1, 1], ["MP_HASTE", 1, 1]], "bonuses": [0, 0, 0, 2] },
                { "chara": "Goofy", "weapon": "WEP_GOOFY_00", "armour": [null, null], "accessories": [null], "items": [], "slots": [3, 2, 1], "abilities": [["EXPZERO", 0, 0]], "bonuses": [] }
            ],
            "zero_exp": true
        })
    }

    #[test]
    fn a_save_with_the_friends_record_is_version_5_and_round_trips() {
        let base = base();
        let mut save = save_of(&base);
        save.party = party_v5();
        assert_eq!(save.written_version(), 5);
        let text = save.to_json();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["version"], 5);
        let back = SoraSave::from_json(&text).unwrap();
        assert_eq!(back, save);
        assert_eq!(back.party_friends().len(), 2);
        assert_eq!(back.party["friends"][0]["weapon"], "WEP_DONALD_03");
        // through the file, with the slot summary's leader (the save-slot picture)
        let dir = scratch("round");
        let path = save_path(&dir, &character_key("Tarnished", 2));
        write(&path, &save).unwrap();
        assert_eq!(load(&dir, &character_key("Tarnished", 2), &mut |_| {}), Loaded::Save(save.clone()));
        assert_eq!(slot_summary(2, &path).portrait, kh_ui::saveload::Portrait::Donald);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_the_friends_record_a_save_is_written_as_version_4_as_before() {
        let base = base();
        let mut save = save_of(&base);
        // no party record (a host without the party) and the v4 formation record: version 4, readable by older builds
        for party in [Value::Null, crate::formation::party_record(&crate::formation::Formation::default(), &[])] {
            save.party = party;
            assert_eq!(save.written_version(), 4);
            let value: Value = serde_json::from_str(&save.to_json()).unwrap();
            assert_eq!(value["version"], 4);
            assert_eq!(SoraSave::from_json(&save.to_json()).unwrap(), save);
        }
        // an odd party record (friends not a list): kept as it is, not taken for the friends record
        save.party = json!({ "format": 2, "leader": "goofy", "active": ["goofy"], "friends": 7 });
        assert_eq!((save.written_version(), save.party_friends().len()), (4, 0));
        let back = SoraSave::from_json(&save.to_json()).unwrap();
        assert_eq!(back.party["friends"], 7);
        assert_eq!(slot_summary_of(0, &back).portrait, kh_ui::saveload::Portrait::Goofy);
    }

    #[test]
    fn version_4_and_3_files_load_with_no_friends_record() {
        let base = base();
        let save = save_of(&base);
        let mut value: Value = serde_json::from_str(&save.to_json()).unwrap();
        // version 4 with a format-2 party record: the record stands, no friends
        value["version"] = json!(4);
        value["party"] = json!({ "format": 2, "leader": "sora", "active": ["sora", "goofy"], "ai": { "goofy": [1, 1, 1, 7] } });
        let v4 = SoraSave::from_json(&value.to_string()).unwrap();
        assert_eq!((v4.version, v4.party["leader"].as_str(), v4.party_friends().len()), (VERSION, Some("sora"), 0));
        assert_eq!(serde_json::from_str::<Value>(&v4.to_json()).unwrap()["version"], 4, "written back as it was");
        // version 3: no party field at all
        value["version"] = json!(3);
        value.as_object_mut().unwrap().remove("party");
        let v3 = SoraSave::from_json(&value.to_string()).unwrap();
        assert_eq!(v3.party, Value::Null);
        assert_eq!(slot_summary_of(0, &v3).portrait, kh_ui::saveload::Portrait::Sora, "an old save reads as Sora's");
        // a version this code does not know yet is refused, not misread
        value["version"] = json!(VERSION + 1);
        assert_eq!(SoraSave::from_json(&value.to_string()), Err(SaveError::Newer(VERSION + 1)));
    }

    #[test]
    fn the_backup_keeps_the_friends_record() {
        let base = base();
        let dir = scratch("bak");
        let key = character_key("Tarnished", 4);
        let path = save_path(&dir, &key);
        let mut one = save_of(&base);
        one.party = party_v5();
        write(&path, &one).unwrap();
        let mut two = one.clone();
        two.party["friends"][0]["weapon"] = json!("WEP_DONALD_05");
        write(&path, &two).unwrap();
        assert!(backup_path(&path).exists());
        std::fs::write(&path, b"{ \"version\": 5, \"level\": ").unwrap();
        match load(&dir, &key, &mut |_| {}) {
            Loaded::Backup(save) => assert_eq!((save.party["friends"][0]["weapon"].as_str(), save.written_version()), (Some("WEP_DONALD_03"), 5)),
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
