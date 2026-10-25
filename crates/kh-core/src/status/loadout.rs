//! A loadout: the values a `Status` starts with. The user's test case is level 99 with
//! everything: `sheets/port/sora-loadout-99.json` (docs/sora-stats-spec.md). The core reads no
//! files: a host parses the sheet and feeds its rows to [`Loadout::set`] one by one, then calls
//! [`Loadout::resolve`] (`kh-app` does that in `loadout.rs`; `--loadout FILE` names another file).
//!
//! THE KEYS ARE THE SHEET'S ROW IDS (as of 2026-10-11).
//!
//! SINCE THE STATS RULES EXIST (docs/sora-stats-spec.md) a loadout is the SAVE-DATA SHAPE of the
//! spec's section 8 and the numbers are COMPUTED by `Status::from_loadout`: the level from the
//! EXP, the stats from the level row and the equipment, max HP / MP from the base row and the
//! victory bonuses, the AP in use from the equipped abilities. The sheet's own number rows
//! (`hp.*`, `mp.*`, `strength`, `magic`, `defense`, `ap`, `abilities.equipped_ap`, `focus`) are
//! kept as what the sheet STATES and compared with what the rules give by [`Loadout::verify`]: a
//! difference is a finding, never silently resolved.
//!
//! Inputs the rules read: `level` / `exp`, `game_level`, `choice.default` (first word = vitality /
//! balance / wisdom: which of the three opening victory bonuses is set; "ability path A / B / C" =
//! the level table's ability column), the victory-bonus counts (below), the weapon, armour,
//! accessories and the two ability lists.
//!
//! VICTORY BONUSES: `bonus.counts` (a map `ETresVictoryBonusKind` name -> how many rows of that
//! kind have their flag set, WITHOUT the opening-choice row, which follows `choice.default`). The
//! sheet of 2026-10-11 has no such row: [`Loadout::resolve`] then takes every story and Re Mind
//! row of stats spec 2.4 ([`BonusCounts::STORY_AND_REMIND`]), which is what the sheet's `hp.*` /
//! `mp.*` rows are written for. A loadout built in code without `resolve` has no bonuses.
//!
//! | row id | value | goes to |
//! |---|---|---|
//! | `level`, `exp` | number | `level`, `exp` |
//! | `game_level` | text | `game_level` |
//! | `choice.default` | text | `choice` |
//! | `hp.<growth>.<standard or critical>`, `mp.<...>` | number | STATED: the row that `resolve` selects becomes `hp_max` / `mp_max` |
//! | `focus` | number | STATED: `focus_max` |
//! | `strength`, `magic`, `defense` | number | STATED: `stats` |
//! | `ap` | number | STATED: `stats.ap_max` |
//! | `abilities.equipped_ap` | number | STATED: `stats.ap` (AP in use) |
//! | `bonus.counts` | map kind name -> count | `bonuses` (optional row, see above) |
//! | `weapon.upgrade` | number 0..10 | `weapon_upgrade` (optional row: keyblade upgrade level) |
//! | `battle_level` | number 1..99 | `battle_level` (optional row: GW+0x7E09 of the area) |
//! | `abilities.owned`, `abilities.equipped` | list of `ETresAbilityKind` names; `NAME xN` or a name listed n times is n copies | `abilities` |
//! | `abilities.critical_extra` | list of names | `critical_extra` (STATED; the Critical start set is applied from the difficulty sheet, stats spec 2.6) |
//! | `abilities.ap_all` | number | accepted, not kept (a sum the sheet gives for information) |
//! | `magic.known` | map name -> tier | `magic` |
//! | `shotlocks`, `armour`, `accessories` | list of text | the same |
//! | `weapon` | text | `weapon` |
//! | `item_slots` | number | `item_slots` |
//! | `items` | list of item names; a name listed n times is n of it | `items` |
//!
//! A row that is not in this table, or whose value has the wrong shape, is an `Err` naming it:
//! the host reports it and goes on; nothing is guessed.

use super::{tables, BonusCounts, Growth, ItemStack, KnownMagic, Stats, Status};
use crate::enums::ETresAbilityKind;

/// One cell of a loadout row.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadoutValue {
    Number(f64),
    Text(String),
    List(Vec<String>),
    /// Name -> number.
    Map(Vec<(String, f64)>),
}

/// Initial values for a [`super::Status`].
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Loadout {
    /// Where it came from (a file name), for the debug text.
    pub source: String,
    /// True for [`Loadout::empty_placeholder`] only.
    pub placeholder: bool,
    /// The level asked for. The game stores only the EXP: `Status::from_loadout` takes the
    /// level's own total EXP when `exp` does not belong to this level.
    pub level: u32,
    pub exp: u64,
    /// The level the ability lists were written for (the `level` ROW; [`Loadout::set_level`]
    /// does not change it): the level table's grants above `level` are taken off the owned
    /// copies. 0 = the same as `level`.
    pub listed_level: u32,
    /// Victory-bonus counters (`record+0x960 + kind`) without the opening-choice row; `None` =
    /// not given.
    pub bonuses: Option<BonusCounts>,
    /// Keyblade upgrade level 0..10.
    pub weapon_upgrade: u8,
    /// Battle level of the area (GW+0x7E09); 0 = not given (the status then uses 1).
    pub battle_level: u8,
    /// `game_level` row: STANDARD, CRITICAL ...
    pub game_level: String,
    /// `choice.default` row.
    pub choice: String,
    /// Every `hp.*` / `mp.*` row: (row id, value). `resolve` picks from them.
    pub gauge_rows: Vec<(String, f32)>,
    pub hp_max: f32,
    pub mp_max: f32,
    pub focus_max: f32,
    pub stats: Stats,
    /// (ability, copies owned, copies equipped)
    pub abilities: Vec<(ETresAbilityKind, u8, u8)>,
    pub critical_extra: Vec<ETresAbilityKind>,
    pub magic: Vec<KnownMagic>,
    pub shotlocks: Vec<String>,
    /// OPTIONAL rows `shortcuts.a` / `.b` / `.c` (magic implementer): the three shortcut pages,
    /// four command names each ("" = empty). A TEST-CONFIGURATION CHOICE: a new game has all 12
    /// slots empty and only the Customize menu writes them (magic spec J 3.1).
    pub shortcuts: [Vec<String>; 3],
    pub weapon: String,
    pub armour: Vec<String>,
    pub accessories: Vec<String>,
    pub item_slots: u32,
    pub items: Vec<ItemStack>,
}

fn kinds(key: &str, names: &[String]) -> Result<Vec<(ETresAbilityKind, u8)>, String> {
    let mut out: Vec<(ETresAbilityKind, u8)> = Vec::new();
    for entry in names {
        // The sheet writes several copies as "NAME xN".
        let (name, copies) = match entry.rsplit_once(" x") {
            Some((name, count)) if count.parse::<u8>().is_ok() => (name.trim(), count.parse::<u8>().unwrap_or(1)),
            _ => (entry.trim(), 1),
        };
        let kind = ETresAbilityKind::from_game_name(name).ok_or_else(|| format!("{key}: {name} is not an ETresAbilityKind value"))?;
        match out.iter_mut().find(|k| k.0 == kind) {
            Some(known) => known.1 = known.1.saturating_add(copies),
            None => out.push((kind, copies)),
        }
    }
    Ok(out)
}

impl Loadout {
    /// An empty loadout to be filled with `set`.
    pub fn new(source: &str) -> Loadout {
        Loadout { source: source.to_string(), ..Loadout::default() }
    }

    /// THE EXPLICITLY EMPTY LOADOUT, used when no loadout is given: level 0, every gauge 0, no
    /// abilities, no magic, no items, and `placeholder` set so that nobody mistakes it for data.
    /// It is NOT a guess at Sora's starting values.
    pub fn empty_placeholder() -> Loadout {
        Loadout { source: "EMPTY PLACEHOLDER (no loadout given)".to_string(), placeholder: true, ..Loadout::default() }
    }

    /// Sets one row by its id (table in this file's header). `Err` names what was wrong; the
    /// loadout is then unchanged.
    pub fn set(&mut self, key: &str, value: &LoadoutValue) -> Result<(), String> {
        let number = || match value {
            LoadoutValue::Number(n) if n.is_finite() => Ok(*n),
            other => Err(format!("{key}: expected a number, got {other:?}")),
        };
        let whole = || number().and_then(|n| if n >= 0.0 && n.fract() == 0.0 { Ok(n) } else { Err(format!("{key}: expected a whole number, 0 or more, got {n}")) });
        let text = || match value {
            LoadoutValue::Text(t) => Ok(t.clone()),
            other => Err(format!("{key}: expected text, got {other:?}")),
        };
        let list = || match value {
            LoadoutValue::List(l) => Ok(l.clone()),
            other => Err(format!("{key}: expected a list, got {other:?}")),
        };
        match key {
            "level" => {
                self.level = whole()? as u32;
                self.listed_level = self.level;
            }
            "weapon.upgrade" => self.weapon_upgrade = whole()?.min(10.0) as u8,
            "battle_level" => self.battle_level = whole()?.min(99.0) as u8,
            "bonus.counts" => match value {
                LoadoutValue::Map(map) => {
                    let mut counts = BonusCounts::NONE;
                    for (name, count) in map {
                        let kind = BonusCounts::kind_by_name(name).ok_or_else(|| format!("{key}: {name} is not an HP_UP / MP_UP / slot kind of ETresVictoryBonusKind"))?;
                        counts.0[kind] = count.clamp(0.0, 255.0) as u8;
                    }
                    self.bonuses = Some(counts);
                }
                other => return Err(format!("{key}: expected a map of kind -> count, got {other:?}")),
            },
            "exp" => self.exp = whole()? as u64,
            "game_level" => self.game_level = text()?,
            "choice.default" => self.choice = text()?,
            "focus" => self.focus_max = number()? as f32,
            "strength" => self.stats.strength = number()? as i32,
            "magic" => self.stats.magic = number()? as i32,
            "defense" => self.stats.defense = number()? as i32,
            "ap" => self.stats.ap_max = number()? as i32,
            "abilities.equipped_ap" => self.stats.ap = number()? as i32,
            "abilities.ap_all" => {
                number()?;
            }
            "abilities.owned" | "abilities.equipped" => {
                let listed = kinds(key, &list()?)?;
                let equipped = key == "abilities.equipped";
                // Start from what the other list said, then lay this list over it.
                for entry in &mut self.abilities {
                    if equipped {
                        entry.2 = 0;
                    } else {
                        entry.1 = 0;
                    }
                }
                for (kind, copies) in listed {
                    match self.abilities.iter_mut().find(|a| a.0 == kind) {
                        Some(entry) if equipped => entry.2 = copies,
                        Some(entry) => entry.1 = copies,
                        None => self.abilities.push(if equipped { (kind, 0, copies) } else { (kind, copies, 0) }),
                    }
                }
                self.abilities.retain(|a| a.1 > 0 || a.2 > 0);
            }
            "abilities.critical_extra" => self.critical_extra = kinds(key, &list()?)?.into_iter().map(|k| k.0).collect(),
            "magic.known" => match value {
                LoadoutValue::Map(map) => self.magic = map.iter().map(|(name, tier)| KnownMagic { name: name.clone(), tier: *tier as u32 }).collect(),
                other => return Err(format!("{key}: expected a map of name -> tier, got {other:?}")),
            },
            "shotlocks" => self.shotlocks = list()?,
            "shortcuts.a" => self.shortcuts[0] = list()?,
            "shortcuts.b" => self.shortcuts[1] = list()?,
            "shortcuts.c" => self.shortcuts[2] = list()?,
            "weapon" => self.weapon = text()?,
            "armour" => self.armour = list()?,
            "accessories" => self.accessories = list()?,
            "item_slots" => self.item_slots = whole()? as u32,
            "items" => {
                let mut items: Vec<ItemStack> = Vec::new();
                for name in list()? {
                    match items.iter_mut().find(|i| i.item == name) {
                        Some(stack) => stack.count += 1,
                        None => items.push(ItemStack { item: name, count: 1 }),
                    }
                }
                self.items = items;
            }
            _ if key.starts_with("hp.") || key.starts_with("mp.") => {
                let n = number()? as f32;
                self.gauge_rows.retain(|r| r.0 != key);
                self.gauge_rows.push((key.to_string(), n));
            }
            _ => return Err(format!("{key}: not a loadout row this port knows (crates/kh-core/src/status/loadout.rs lists them)")),
        }
        Ok(())
    }

    /// After every row is set: picks `hp_max` / `mp_max` from the `hp.<growth>.<level>` rows by
    /// the sheet's own `choice.default` (first word) and `game_level`. `Err` says what is
    /// missing; the gauges are then left as they are.
    pub fn resolve(&mut self) -> Result<(), String> {
        let growth = self.choice.split_whitespace().next().unwrap_or("").to_lowercase();
        let level = self.game_level.to_lowercase();
        if growth.is_empty() || level.is_empty() {
            return Err("hp / mp: the rows choice.default and game_level are needed to pick the HP / MP rows".to_string());
        }
        let row = |gauge: &str| {
            let id = format!("{gauge}.{growth}.{level}");
            // Beginner and Proud have the Standard numbers (stats spec section 9: one column for
            // "Standard / Beginner / Proud"): the sheet has rows for standard and critical only.
            let fallback = format!("{gauge}.{growth}.standard");
            let find = |id: &str| self.gauge_rows.iter().find(|r| r.0 == id).map(|r| r.1);
            find(&id).or_else(|| if level == "beginner" || level == "proud" { find(&fallback) } else { None }).ok_or(format!("{id}: no such row (choice.default = \"{}\", game_level = \"{}\")", self.choice, self.game_level))
        };
        let (hp, mp) = (row("hp")?, row("mp")?);
        (self.hp_max, self.mp_max) = (hp, mp);
        // No `bonus.counts` row: the sheet's HP / MP rows are written for every story and
        // Re Mind bonus (stats spec 2.4, section 9).
        if self.bonuses.is_none() {
            self.bonuses = Some(BonusCounts::STORY_AND_REMIND);
        }
        Ok(())
    }

    /// The first choice of the opening (`choice.default`'s first word); `None` when not given.
    pub fn growth(&self) -> Option<Growth> {
        Growth::from_name(self.choice.split_whitespace().next().unwrap_or(""))
    }

    /// The ability path 0 / 1 / 2 (`choice.default`: "... ability path A / B / C"; A when not said).
    pub fn ability_path(&self) -> u8 {
        let lower = self.choice.to_lowercase();
        match lower.split("path").nth(1).and_then(|rest| rest.trim().chars().next()) {
            Some('b') | Some('1') => 1,
            Some('c') | Some('2') => 2,
            _ => 0,
        }
    }

    /// The same loadout at another level (the app's `--level N`): the level and the level's own
    /// total EXP from the level table. Everything else is recomputed by `Status::from_loadout`
    /// (the stats of the level row, the level table's ability grants up to that level); the
    /// victory bonuses, the equipment and the rest of the ability lists stay as listed.
    pub fn set_level(&mut self, level: u32) {
        let level = level.clamp(1, tables::LEVEL_MAX);
        if self.listed_level == 0 {
            self.listed_level = self.level;
        }
        self.level = level;
        self.exp = tables::level_row(level).exp_total as u64;
    }

    /// (DF, 2026-10-15; the user: "the overall difficulty of the mod should be like Kingdom Hearts critical mode")
    /// KH3 CRITICAL'S START, the part a NEW GAME's ability lists carry: one more owned copy of the ability of every
    /// victory-bonus row flagged `m_InitAbilityEquipOnCriticalMode` (sheet `sora-critical-start`, [`tables::critical_start`];
    /// at most three copies). An EQUIP row's copy is also equipped while the AP allow, in row order. The AP budget is the
    /// level row's AP + the difficulty's `ap_add` (30), the AP in use those of the equipped lists + the equipped
    /// `m_CriticalEquipAbility` set that `Status::from_loadout` adds (Critical Recharge 3; Critical Counter is excluded by
    /// decision dec032, the user: "don't start with critical counter").
    /// Nothing happens unless `game_level` is CRITICAL. Call it once, on a new game's loadout (it adds, it is not
    /// idempotent). Returns the copies added (owned, equipped).
    pub fn add_critical_start(&mut self) -> (u32, u32) {
        let Some(level) = super::GameLevel::from_name(&self.game_level).filter(|g| g.is_critical()) else { return (0, 0) };
        let row = level.row();
        let budget = tables::level_row(self.level.max(1)).ap + row.ap_add.clamp(0, 100);
        let mut used: i32 = self.abilities.iter().map(|&(kind, _, equipped)| tables::ability_ap(kind) * equipped as i32).sum();
        for name in row.start_abilities {
            if let Some(kind) = ETresAbilityKind::from_game_name(name).filter(|k| !tables::critical_start_excluded(*k)) {
                let listed = self.abilities.iter().find(|a| a.0 == kind).map_or(0, |a| a.2);
                if kind != ETresAbilityKind::CriticalConverter && listed == 0 {
                    used += tables::ability_ap(kind);
                }
            }
        }
        let (mut owned_added, mut equipped_added) = (0, 0);
        for (kind, equip) in tables::critical_start() {
            let index = match self.abilities.iter().position(|a| a.0 == kind) {
                Some(i) => i,
                None => {
                    self.abilities.push((kind, 0, 0));
                    self.abilities.len() - 1
                }
            };
            let entry = &mut self.abilities[index];
            if entry.1 >= 3 {
                continue;
            }
            entry.1 += 1;
            owned_added += 1;
            let ap = tables::ability_ap(kind);
            if equip && entry.2 < entry.1 && used + ap <= budget {
                entry.2 += 1;
                used += ap;
                equipped_added += 1;
            }
        }
        (owned_added, equipped_added)
    }

    /// Where the sheet's STATED numbers differ from what the rules give for this loadout
    /// (`status` = `Status::from_loadout(self)`). Empty = the sheet and the rules agree. Only
    /// meaningful at the level the sheet was written for: at another level (`set_level`) the
    /// stated numbers do not apply and nothing is compared.
    pub fn verify(&self, status: &Status) -> Vec<String> {
        let mut out = Vec::new();
        if self.placeholder || (self.listed_level != 0 && self.listed_level != self.level) {
            return out;
        }
        let mut check = |what: &str, stated: f64, computed: f64| {
            if stated != 0.0 && stated != computed {
                out.push(format!("{what}: the sheet states {stated}, the rules give {computed}"));
            }
        };
        check("max HP", self.hp_max as f64, status.hp_max as f64);
        check("max MP", self.mp_max as f64, status.mp_max as f64);
        check("max focus", self.focus_max as f64, status.focus_max as f64);
        check("strength", self.stats.strength as f64, status.strength() as f64);
        check("magic", self.stats.magic as f64, status.magic() as f64);
        check("defense", self.stats.defense as f64, status.defense() as f64);
        // The sheet's `ap` and `abilities.equipped_ap` are the Standard numbers; Critical adds
        // its own (30 AP and two equipped start abilities: stats spec 2.6).
        if !status.game_level.is_critical() {
            check("max AP", self.stats.ap_max as f64, status.max_ap() as f64);
            check("AP in use", self.stats.ap as f64, status.abilities.ap_in_use() as f64);
        }
        check("total EXP", self.exp as f64, status.exp as f64);
        out
    }
}
