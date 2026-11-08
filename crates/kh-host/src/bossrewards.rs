//! (boss rewards, IMPL_ER31) WHICH BOSS OF THE HOST GAME GIVES SORA WHAT. The user (2026-10-13): "also add magic
//! unlocks to bosses. Make Margit give you thunder, for example, and Godrick the grafted gives you heal"; "Glide
//! should go to Maliketh"; earlier:
//! "bosses will also give powerful abilities". docs/sora-er-boss-rewards.md is the text; every boss is a row of
//! sheets/port/sora-er-boss-rewards.json (read at run time, the compiled copy when the file is gone).
//!
//! This is the host-independent half: the table, the once-only record, the grant through the reward seam
//! (`Sora::grant`: kh_core::status::reward) and the game's own "BONUS!" panel (`HudRightGetBonus`, one or two lines,
//! static SE 38: what KH3 shows for a victory bonus, magic and abilities included). How a host KNOWS a boss is
//! beaten (Elden Ring: its defeat event flag, the character model of a boss-bar death) is the host's
//! (crates/kh-er/src/bossreward.rs).
//!
//! THE RULES:
//! - ONCE PER CHARACTER: a row whose id is in the records (`Inventory::records`, `boss:<row id>`; kept by the save,
//!   `SoraSave::records`) gives nothing again. The record is made in the same call as the grant.
//! - MAGIC IS "ONE TIER MORE OF THAT ELEMENT", whatever tier the row names (KH3 replaces tiers: Thunder -> Thundara
//!   -> Thundaga): the row's name is the tier a player on the usual route gets there. A -ra row beaten before the
//!   element's first source gives the first tier, and the first source beaten later gives the -ra: no order of play
//!   wastes a reward or skips a tier. Each element has exactly as many rows as tiers a new game lacks (unit-tested).
//! - AN ABILITY is one more owned copy, NOT equipped (as the game's victory bonuses): the player installs it on the
//!   Abilities screen when the AP allow.
//! - `slot:great_rune`: the Nth such row this character is rewarded for gives the Nth slot of `rule.slot_order`.
//! - A reward that changes nothing (three copies) is said in the log and shows no line.
//! - (DON-PROG, the Donald overhaul: documentation/DONALD_OVERHAUL.md 9) MAGIC AT THE TOP: a magic row of an element whose
//!   -ga tier (3) Sora already has gives him MAX MP +3 instead ("If Sora already has the -ga tier of that element, the boss
//!   gives him Max MP +3"). Each element has one boss more than Sora's tiers (the extra bosses shared with Donald).
//! - (DON-PROG) DONALD'S STEPS of a boss: rows `don.<boss row id>` = { name, steps: ["magic:<element>" | "dlc:<element>" |
//!   "line2:<element>" (DONLINE2: the line-2 spell's own bosses; "banish" = "line2:light") | "arcane:<line>"] } ([`DonRow`], `Table::don`): the party host gives them to Donald, joined or not (kh-partyhost
//!   `donald::don_due`, record `don:<row id>`). Sora gets nothing from them: Donald-only rewards (Flare and the line-2
//!   spells, the ultimates, Banish, the Arcane Y lines, Zettaflare) are not Sora's.
//! - (GF-STATS) THE FRIENDS' OWN REWARDS (documentation/GOOFY_OVERHAUL.md 11.4, DONALD_OVERHAUL.md 9): a `don.<row>` row also
//!   carries raw `hp:<n>`, `mp:<n>`, `slot:<kind>` steps ([`DonRow::growth`], record `donstat:<row>`, apart from `don:<row>` so a
//!   save whose Donald already took the row's magic still gets them). Rows `gof.<row>` are Goofy's ([`GofRow`], record
//!   `gof:<row>`): those steps and `tech:<name>` / `art:<name>` rank steps. A row with `group` names copies of one boss whose
//!   rows stay `reserved` for Sora: the first defeat flag seen counts. kh-partyhost growth.rs gives them, joined or not.

use kh_core::enums::ETresAbilityKind;
use kh_core::magic::spells::{command_name, Element};
use kh_core::status::{BonusCounts, BonusLine, Reward};
use kh_core::Sora;
use serde_json::Value;

use crate::weapons::Gift;

/// (DON-PROG) What a magic boss gives Sora once he has the element's -ga tier (DONALD_OVERHAUL.md 9: Max MP +3).
pub const SORA_TOP_MP: i32 = 3;

/// The compiled copy of the sheet.
pub const SHEET: &str = include_str!("../../../sheets/port/sora-er-boss-rewards.json");
/// A record of this module in `Inventory::records`.
pub const RECORD_PREFIX: &str = "boss:";

/// One reward of a row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Token {
    /// `magic:<spell>`: one tier more of the element; `tier` is the tier the row names (1..=3).
    Magic { element: Element, tier: u8 },
    /// `ability:<game name>`: one more copy.
    Ability(ETresAbilityKind),
    /// `hp:<n>` / `mp:<n>`: the game's own steps.
    MaxHp(i32),
    MaxMp(i32),
    /// `slot:item|armor|accessory`: the victory-bonus kind.
    Slot(usize),
    /// `slot:great_rune`: by count (`Table::slot_order`).
    GreatRuneSlot,
    /// (P13 ORE, documentation/KH3_WEAPONS_PLAN.md 3.1) `item:<KEY>[:n]`: `count` (1 when not named, at most 99) of a KH3
    /// item of the item table (sheets/port/sora-item-table.json, by its key), handed to Sora through `items::give` (the HUD
    /// takes it with the item popup), once with the row like every token. THE MINIMAL FORM: the fixed ore placements
    /// ([`Placements`]) and any boss row may name it; P9b extends it (the KH3 OBTAINED popup for weapons, `weapon:` tokens).
    Item { item_id: u32, count: u16 },
    /// (P9b ACQUISITION, documentation/KH3_WEAPONS_PLAN.md 1.7 / 6b) `weapon:<KEY>`: one KH3 weapon of the weapon table (a
    /// Keyblade `WEP_KEYBLADE_SO_nn`, a staff `WEP_DONALD_nn`, a shield `WEP_GOOFY_nn`), handed over through
    /// `crate::weapons::give` (a Keyblade already owned is refused there: "already owned: no duplicate"; the row is recorded
    /// all the same). The HUD shows KH3's OBTAINED popup when it arrives (hud_obtained.rs `ObtainedKind::Weapon`).
    Weapon { item_id: u32 },
}

fn slot_kind(name: &str) -> Option<usize> {
    Some(match name {
        "item" => BonusCounts::ITEM_SLOT_UP1,
        "armor" | "armour" => BonusCounts::DEF_SLOT_UP1,
        "accessory" => BonusCounts::ACC_SLOT_UP1,
        _ => return None,
    })
}

/// "thundara" -> (Thunder, 2).
pub fn spell(name: &str) -> Option<(Element, u8)> {
    Element::ALL.into_iter().flat_map(|e| (1..=3u8).map(move |t| (e, t))).find(|(e, t)| command_name(e.command(*t)).eq_ignore_ascii_case(name.trim()))
}

fn ability_name(kind: ETresAbilityKind) -> &'static str {
    kh_core::status::tables::ABILITIES.iter().find(|row| row.kind == kind).map_or(kind.name(), |row| row.name)
}

impl Token {
    pub fn parse(text: &str) -> Option<Token> {
        let (kind, what) = text.split_once(':')?;
        let what = what.trim();
        Some(match kind.trim() {
            "magic" => spell(what).map(|(element, tier)| Token::Magic { element, tier })?,
            "ability" => Token::Ability(ETresAbilityKind::from_game_name(&what.to_ascii_uppercase()).filter(|k| *k != ETresAbilityKind::None)?),
            "hp" => Token::MaxHp(what.parse().ok().filter(|n| Reward::max_hp(*n).is_some())?),
            "mp" => Token::MaxMp(what.parse().ok().filter(|n| Reward::max_mp(*n).is_some())?),
            "slot" if what == "great_rune" => Token::GreatRuneSlot,
            "slot" => Token::Slot(slot_kind(what)?),
            // (P13 ORE) `item:<KEY>[:n]`
            "item" => {
                let (key, n) = what.split_once(':').unwrap_or((what, "1"));
                let item_id = crate::items::by_key(key.trim())?.item_id;
                Token::Item { item_id, count: n.trim().parse().ok().filter(|n| (1..=99).contains(n))? }
            }
            // (P9b) `weapon:<KEY>`
            "weapon" => Token::Weapon { item_id: crate::weapons::by_key(what)?.item_id },
            _ => return None,
        })
    }

    /// As the table shows it ("Thundara", "MP Safety", "Max HP +10", "Slot (Great Rune rule)").
    pub fn label(&self) -> String {
        match *self {
            Token::Magic { element, tier } => command_name(element.command(tier)).to_string(),
            Token::Ability(kind) => ability_name(kind).to_string(),
            Token::MaxHp(n) => format!("Max HP +{n}"),
            Token::MaxMp(n) => format!("Max MP +{n}"),
            Token::Slot(BonusCounts::ITEM_SLOT_UP1) => "Item slot +1".to_string(),
            Token::Slot(BonusCounts::DEF_SLOT_UP1) => "Armor slot +1".to_string(),
            Token::Slot(_) => "Accessory slot +1".to_string(),
            Token::GreatRuneSlot => "Slot (Great Rune rule)".to_string(),
            Token::Item { item_id, count } => format!("{} x{count}", crate::items::by_id(item_id).map_or_else(|| format!("item {item_id}"), |i| i.name.clone())),
            Token::Weapon { item_id } => crate::weapons::by_id(item_id).map_or_else(|| format!("weapon {item_id:#x}"), |w| w.name.clone()),
        }
    }
}

/// How sure an identifier is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conf {
    /// Seen in the running game.
    Verified,
    /// From memory / public lists, not read in the game.
    Guess,
}

/// One boss.
#[derive(Clone, Debug, PartialEq)]
pub struct BossRow {
    /// The row id (`margit`): what the save records and what a host's manual grant takes.
    pub id: String,
    pub name: String,
    pub region: String,
    pub order: u32,
    /// required | main | optional.
    pub path: String,
    /// The host game's defeat event flag.
    pub flag: Option<(u32, Conf)>,
    /// The host game's character model id (Elden Ring: cNNNN).
    pub chr: Option<(i32, Conf)>,
    /// The map area the chr id counts in (`None`: anywhere).
    pub area: Option<u8>,
    /// The rewards: the first `user` of them are the user's own choice, the rest proposals.
    pub rewards: Vec<Token>,
    pub user: usize,
    /// (BOSSFLAGS) A row whose flag is known but which has no reward yet (Goofy's overhaul, the planned extra bosses): the
    /// flag is not watched, so the boss stays a small boss until a reward is attached.
    pub reserved: bool,
    /// (XSI, 2026-10-16) The XS ability rewards of the row (sheet row `xs.<row id>`; already in `rewards`, at its end):
    /// what the XS catch-up gives a character whose record has `boss:<row id>` but not `xs:<row id>`.
    pub xs: Vec<Token>,
    /// (DON-PROG) Sora's magic of an element's extra boss (sheet row `sm.<row id>`; already in `rewards`, at its end): what
    /// the overhaul catch-up gives a character whose record has `boss:<row id>` but not `sora_magic:<row id>`.
    pub sm: Vec<Token>,
}

impl BossRow {
    pub fn great_rune(&self) -> bool {
        self.rewards.contains(&Token::GreatRuneSlot)
    }

    /// "Thunder (user) + MP Safety".
    pub fn reward_text(&self) -> String {
        self.rewards.iter().enumerate().map(|(k, t)| format!("{}{}", t.label(), if k < self.user { " (user)" } else { "" })).collect::<Vec<_>>().join(" + ")
    }
}

/// (DON-PROG) One boss's steps for Donald (sheet row `don.<boss row id>`).
#[derive(Clone, Debug, PartialEq)]
pub struct DonRow {
    /// The boss row id (`margit`).
    pub boss: String,
    pub name: String,
    /// The step tokens (`magic:thunder`, `line2:fire`, `banish`, `arcane:meteor`; kh_party::donald::DonStep::parse).
    pub steps: Vec<String>,
    /// (GF-STATS) His raw HP / MP / slot steps (`hp:9`, `mp:7`, `slot:item`; kh_party::growth::GrowthStep::parse).
    pub growth: Vec<String>,
}

/// (GF-STATS) One boss's steps for Goofy (sheet row `gof.<row>`).
#[derive(Clone, Debug, PartialEq)]
pub struct GofRow {
    /// The boss row id (`margit`), or the group's own name (`crucible_knights`).
    pub boss: String,
    pub name: String,
    /// The step tokens (`tech:deflect`, `art:taunt`, `hp:14`, `mp:4`, `slot:armour`; kh_party::growth::GrowthStep::parse).
    pub steps: Vec<String>,
    /// Boss row ids of a group of copies: the first of their defeat flags seen counts (their rows stay `reserved`). Empty: an
    /// ordinary row, due when `boss` is recorded.
    pub group: Vec<String>,
}

/// (DON-PROG) A step token of a `don.*` row is well formed.
pub fn don_token_ok(t: &str) -> bool {
    let t = t.trim();
    if t.eq_ignore_ascii_case("banish") {
        return true;
    }
    match t.split_once(':') {
        Some(("magic" | "element" | "dlc" | "magic_dlc" | "line2" | "l2" /* (DONLINE2) */, e)) => ["fire", "blizzard", "thunder", "water", "aero", "light", "cure"].contains(&e.trim().to_ascii_lowercase().as_str()),
        Some(("arcane" | "y", l)) => ["comet", "gravity", "meteor", "ultima", "zettaflare"].contains(&l.trim().to_ascii_lowercase().as_str()),
        _ => false,
    }
}

/// (GF-STATS) A raw step token of a friend's row: `hp:<n>`, `mp:<n>`, `slot:item|armour|armor|accessory`.
pub fn stat_token_ok(t: &str) -> bool {
    match t.trim().split_once(':') {
        Some(("hp" | "mp", n)) => n.trim().parse::<i32>().is_ok_and(|n| n > 0),
        Some(("slot", s)) => ["item", "armour", "armor", "accessory"].contains(&s.trim()),
        _ => false,
    }
}

/// (GF-STATS) A step token of a `gof.*` row: a raw step, or the next rank of a technique or Shield Art.
pub fn gof_token_ok(t: &str) -> bool {
    match t.trim().split_once(':') {
        Some(("tech", l)) => ["deflect", "shield_reflect", "hop_counter", "pinning_guard", "slip_counter", "bull_brace", "spell_ward", "rhythm_deflect"].contains(&l.trim()),
        Some(("art", l)) => ["goofy_charge", "taunt", "goofy_tornado", "goofy_bash", "goofy_turbo", "interpose", "shield_toss"].contains(&l.trim()),
        _ => stat_token_ok(t),
    }
}

/// The sheet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub rows: Vec<BossRow>,
    /// (DON-PROG) Donald's steps by boss (`don.<boss row id>`), in the table's order.
    pub don: Vec<DonRow>,
    /// (GF-STATS) Goofy's steps by boss (`gof.<boss row id>`), in the table's order; the groups last.
    pub gof: Vec<GofRow>,
    /// `rule.slot_order`: the victory-bonus kinds of the Great Rune rows, by count.
    pub slot_order: Vec<usize>,
    /// The rows `small.*`: the general rule for every boss that is not a row.
    pub small: SmallRules,
    /// (XSI, 2026-10-16) `rule.skip_if_owned`: an `ability:` reward of one of these gives NOTHING when the character
    /// already owns a copy (from his level path or another boss; XS_SORA_ABILITIES.md 4.2 / question 2, approved):
    /// the XS abilities, one copy each.
    pub skip_if_owned: Vec<ETresAbilityKind>,
    /// (P9b) The boss Keyblades (`kb.<boss row id>`), in the table's order.
    pub kb: Vec<KbRow>,
    /// Rows that were not understood.
    pub problems: Vec<String>,
}

/// (P9b ACQUISITION, plan 6b: "Hero's Origin from Margit ... Shooting Star from Rennala ... spread out the keyblades in the
/// bosses and chests") ONE BOSS KEYBLADE: sheet row `kb.<boss row id>` = { add: ["weapon:WEP_KEYBLADE_SO_nn"] }. Given ONCE
/// PER SAVE (record `kb:<boss row id>`, `crate::weapons::kb_record`) when the boss row's verified defeat flag is set, apart
/// from the boss row's own rewards and record (so a save rewarded for the boss before the Keyblades existed still gets it:
/// the catch-up) and whoever leads (given even before Sora joins: the user wants one of each, 1:1 with KH3). A Keyblade
/// already owned is refused by `crate::weapons::give`; the row is recorded all the same.
#[derive(Clone, Debug, PartialEq)]
pub struct KbRow {
    /// The boss row id (`margit`).
    pub boss: String,
    /// The boss's name (its boss row's).
    pub name: String,
    /// The boss row's verified defeat flag (`None`: no verified flag: never given).
    pub flag: Option<u32>,
    /// Its `weapon:` tokens.
    pub tokens: Vec<Token>,
}

/// A record of a small boss in `Inventory::records` (`small:<host key>`).
pub const SMALL_PREFIX: &str = "small:";

/// THE GENERAL RULE FOR A BOSS THAT IS NOT A ROW (the user, 2026-10-13: "small bosses should give meaningful item
/// rewards, either buff items or equipment or accessories. Make sure you don't get too OP too fast though"): one
/// guaranteed reward, once per boss per character. Rows `small.*` of the sheet (proposals).
#[derive(Clone, Debug, PartialEq)]
pub struct SmallRules {
    /// `small.on`.
    pub on: bool,
    /// `small.tier_cap`: the highest item tier (the item code's tiers 1..6) by region depth 0 Limgrave .. 4 the DLC.
    pub tier_cap: Vec<u8>,
    /// `small.hp_steps`: the boss's maximum HP from which the tier is 2, 3, 4 ... (never above the region's cap).
    pub hp_steps: Vec<i32>,
    /// `small.boost_every`: every Nth small boss of a character gives a stat boost instead of a piece (0 = never).
    pub boost_every: u32,
    /// `small.wait`: seconds after the death in which a row's defeat flag may still claim the boss as a NAMED one.
    pub wait: f32,
    /// (IMPL_ER61; the user, 2026-10-14: "reduce amount of accessories and armor found") `small.item_every`: every Nth
    /// small boss that gets no boost gives a recovery item of `small.item_line` instead of a piece (0 = never).
    pub item_every: u32,
    /// (IMPL_ER61) `small.item_line`: the KH3 item keys of that item by the reward's tier (1, 2, ...; the last for
    /// higher tiers).
    pub item_line: Vec<String>,
}

impl Default for SmallRules {
    fn default() -> SmallRules {
        SmallRules { on: true, tier_cap: vec![2, 3, 4, 5, 6], hp_steps: vec![1500, 4000, 9000, 16000, 30000], boost_every: 3, wait: 12.0, item_every: 2, item_line: ["ITEM_HIGHPOTION", "ITEM_ELIXIR", "ITEM_ELIXIR", "ITEM_ELIXIR", "ITEM_LASTELIXIR", "ITEM_LASTELIXIR"].map(str::to_string).to_vec() }
    }
}

/// The permanent stat a boost raises by one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stat {
    Strength,
    Magic,
    Defense,
    Ap,
}

impl Stat {
    pub const CYCLE: [Stat; 4] = [Stat::Strength, Stat::Magic, Stat::Defense, Stat::Ap];

    /// KH3's item of it (sheets/port/sora-item-table.json: ITEM_POWERUP 514 .. ITEM_APUP 517) and what it does.
    pub fn texts(self) -> (&'static str, &'static str) {
        match self {
            Stat::Strength => ("Strength Boost", "Strength +1"),
            Stat::Magic => ("Magic Boost", "Magic +1"),
            Stat::Defense => ("Defense Boost", "Defense +1"),
            Stat::Ap => ("AP Boost", "Maximum AP +1"),
        }
    }

    /// (BN) The panel's lines: the boost item's name after KH3's camp-item icon (the four boosts are camp items,
    /// sora-item-table "camp"; IconText `CampItem`), then the stat line (no icon, as KH3's stat lines).
    pub fn panel_texts(self) -> (&'static str, &'static str) {
        match self {
            Stat::Strength => ("<icon(CampItem)>Strength Boost", "Strength +1"),
            Stat::Magic => ("<icon(CampItem)>Magic Boost", "Magic +1"),
            Stat::Defense => ("<icon(CampItem)>Defense Boost", "Defense +1"),
            Stat::Ap => ("<icon(CampItem)>AP Boost", "Maximum AP +1"),
        }
    }
}

/// What a small boss gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmallReward {
    /// A stat boost, applied at once (+1).
    Boost(Stat),
    /// An armour piece (`armour`) or an accessory of at most `tier`.
    Piece { armour: bool, tier: u8 },
    /// (IMPL_ER61) The recovery item of `small.item_line` at `tier` (`SmallRules::item_key`).
    Item { tier: u8 },
}

/// FNV-1a of a boss key: the stable choice per boss.
pub fn key_seed(key: &str) -> u32 {
    key.bytes().fold(0x811C_9DC5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

impl SmallRules {
    /// The item tier of a small boss: 1 + the HP steps its maximum HP reaches, at most the region's cap.
    pub fn tier(&self, depth: u8, max_hp: i32) -> u8 {
        let cap = self.tier_cap.get(depth as usize).or(self.tier_cap.last()).copied().unwrap_or(1).max(1);
        (1 + self.hp_steps.iter().filter(|from| max_hp >= **from).count() as u8).min(cap)
    }

    /// THE REWARD of the `nth` small boss of a character (1 = the first). The boosts are placed BY COUNT (every
    /// `boost_every`th boss, Strength -> Magic -> Defense -> AP in turn: never more than one boost per that many
    /// bosses, no stat ahead of another by more than one); a piece's class is the boss's own (stable per boss key).
    pub fn reward(&self, key: &str, depth: u8, max_hp: i32, nth: u32) -> SmallReward {
        if self.boost_every > 0 && nth > 0 && nth.is_multiple_of(self.boost_every) {
            return SmallReward::Boost(Stat::CYCLE[((nth / self.boost_every - 1) % 4) as usize]);
        }
        // (IMPL_ER61) by count as the boosts: every `item_every`th boss of those without a boost gives an item
        if self.item_every > 0 && nth > 0 && nth.is_multiple_of(self.item_every) && !self.item_line.is_empty() {
            return SmallReward::Item { tier: self.tier(depth, max_hp) };
        }
        SmallReward::Piece { armour: (key_seed(key) >> 4) % 2 == 1, tier: self.tier(depth, max_hp) }
    }

    /// (IMPL_ER61) The KH3 item key of a `SmallReward::Item` of `tier`.
    pub fn item_key(&self, tier: u8) -> Option<&str> {
        self.item_line.get((tier.max(1) - 1) as usize).or(self.item_line.last()).map(String::as_str)
    }
}

/// How many small bosses this character has been rewarded for.
pub fn small_count(records: &[String]) -> u32 {
    records.iter().filter(|r| r.starts_with(SMALL_PREFIX)).count() as u32
}

pub fn small_recorded(records: &[String], key: &str) -> bool {
    records.iter().any(|r| r.strip_prefix(SMALL_PREFIX) == Some(key))
}

/// THE SMALL BOSS `key` IS BEATEN: its reward (`None`: this character has had it, or the rule is off) and the
/// record. A boost is applied to `sora` at once, with the bonus panel naming it (KH3's boost ITEMS are camp items
/// nothing in the port can use yet: the item's effect is given, not the item); a piece is the host's to hand over
/// (it knows what he holds).
pub fn small_claim(rules: &SmallRules, records: &mut Vec<String>, sora: &mut Sora, key: &str, depth: u8, max_hp: i32) -> Option<SmallReward> {
    if !rules.on || small_recorded(records, key) {
        return None;
    }
    let reward = rules.reward(key, depth, max_hp, small_count(records) + 1);
    records.push(format!("{SMALL_PREFIX}{key}"));
    if let SmallReward::Boost(stat) = reward {
        small_boost(sora, stat);
    }
    Some(reward)
}

/// A stat boost: +1 for good (the boost bytes of the save), and the panel.
pub fn small_boost(sora: &mut Sora, stat: Stat) {
    if stat == Stat::Ap {
        sora.grant_quiet(Reward::Ap(1));
    } else {
        let status = sora.status_mut();
        match stat {
            Stat::Strength => status.boosts.strength = status.boosts.strength.saturating_add(1),
            Stat::Magic => status.boosts.magic = status.boosts.magic.saturating_add(1),
            _ => status.boosts.defense = status.boosts.defense.saturating_add(1),
        }
        status.rebuild();
    }
    let (name, line) = stat.panel_texts(); // (BN) the item's icon in front of its name
    sora.show_bonus([BonusLine::Text(name), BonusLine::Text(line)]);
}

impl Table {
    /// The sheet as it is on disk (the user edits it), else the compiled copy.
    pub fn load() -> Table {
        let path = kh_gendata::project_file("sheets/port/sora-er-boss-rewards.json");
        let table = std::fs::read_to_string(path).map(|text| Table::parse(&text)).unwrap_or_default();
        if table.rows.is_empty() {
            return Table::parse(SHEET);
        }
        table
    }

    pub fn parse(text: &str) -> Table {
        let mut out = Table::default();
        let mut xs_rows: Vec<(String, Vec<Token>, Option<Token>)> = Vec::new(); // (XSI)
        let mut sm_rows: Vec<(String, Vec<Token>)> = Vec::new(); // (DON-PROG)
        let mut kb_rows: Vec<(String, Vec<Token>)> = Vec::new(); // (P9b)
        let Ok(json) = serde_json::from_str::<Value>(text) else {
            out.problems.push("the sheet is not JSON".to_string());
            return out;
        };
        let conf = |v: &Value| if v.as_str() == Some("verified") { Conf::Verified } else { Conf::Guess };
        for row in json["rows"].as_array().map_or(&[][..], |r| r.as_slice()) {
            let (Some(id), value) = (row[0].as_str(), &row[1]) else { continue };
            if let Some(what) = id.strip_prefix("small.") {
                let numbers = value.as_array().map(|a| a.iter().filter_map(|n| n.as_f64()).collect::<Vec<f64>>());
                match (what, value.as_f64(), numbers) {
                    ("on", Some(v), _) => out.small.on = v != 0.0,
                    ("boost_every", Some(v), _) if (0.0..=1000.0).contains(&v) => out.small.boost_every = v as u32,
                    ("wait", Some(v), _) if (0.0..=120.0).contains(&v) => out.small.wait = v as f32,
                    ("tier_cap", _, Some(list)) if !list.is_empty() => out.small.tier_cap = list.iter().map(|n| n.clamp(1.0, 9.0) as u8).collect(),
                    ("hp_steps", _, Some(list)) => out.small.hp_steps = list.iter().map(|n| *n as i32).collect(),
                    ("item_every", Some(v), _) if (0.0..=1000.0).contains(&v) => out.small.item_every = v as u32, // (IMPL_ER61)
                    ("item_line", _, _) if value.as_array().is_some_and(|a| !a.is_empty() && a.iter().all(|k| k.as_str().is_some_and(|k| crate::items::by_key(k).is_some()))) => {
                        out.small.item_line = value.as_array().into_iter().flatten().filter_map(|k| k.as_str().map(str::to_string)).collect()
                    }
                    _ => out.problems.push(format!("{id}: not understood (the default stands)")),
                }
                continue;
            }
            // (XSI) the abilities that are given once (the "already have it: nothing extra" rule)
            if id == "rule.skip_if_owned" {
                let names = value.as_array().map_or(&[][..], |a| a.as_slice());
                out.skip_if_owned = names.iter().filter_map(|n| n.as_str().and_then(ETresAbilityKind::from_game_name)).collect();
                if out.skip_if_owned.len() != names.len() {
                    out.problems.push("rule.skip_if_owned: a name is not an ability of this build".to_string());
                }
                continue;
            }
            if id == "rule.slot_order" {
                out.slot_order = value.as_array().map_or(Vec::new(), |a| a.iter().filter_map(|s| slot_kind(s.as_str()?)).collect());
                if out.slot_order.len() != value.as_array().map_or(0, |a| a.len()) {
                    out.problems.push("rule.slot_order: a slot kind is not item / armor / accessory".to_string());
                }
                continue;
            }
            // (ER party S6) the friends' victory-bonus rows (`party.<boss row id>`): read by the Elden Ring host (kh-er bossreward.rs)
            if id.starts_with("party.") {
                continue;
            }
            // (DON-PROG) Donald's steps of a boss (`don.<boss row id>`)
            if let Some(key) = id.strip_prefix("don.") {
                let steps: Vec<String> = value["steps"].as_array().map_or(&[][..], |a| a.as_slice()).iter().filter_map(|t| t.as_str().map(str::to_string)).collect();
                let bad: Vec<&String> = steps.iter().filter(|t| !don_token_ok(t) && !stat_token_ok(t)).collect();
                if steps.is_empty() || !bad.is_empty() {
                    out.problems.push(format!("{id}: Donald's step(s) not understood: {bad:?} (magic:<element> | dlc:<element> | line2:<element> | banish | arcane:<line> | hp:<n> | mp:<n> | slot:<kind>)"));
                }
                let growth = steps.iter().filter(|t| stat_token_ok(t)).cloned().collect();
                out.don.push(DonRow { boss: key.to_string(), name: value["name"].as_str().unwrap_or(key).to_string(), steps: steps.into_iter().filter(|t| don_token_ok(t)).collect(), growth });
                continue;
            }
            // (GF-STATS) Goofy's steps of a boss (`gof.<boss row id>`), or of a group of copies of one boss
            if let Some(key) = id.strip_prefix("gof.") {
                let text = |k: &str| -> Vec<String> { value[k].as_array().map_or(&[][..], |a| a.as_slice()).iter().filter_map(|t| t.as_str().map(str::to_string)).collect() };
                let steps = text("steps");
                let bad: Vec<&String> = steps.iter().filter(|t| !gof_token_ok(t)).collect();
                if steps.is_empty() || !bad.is_empty() {
                    out.problems.push(format!("{id}: Goofy's step(s) not understood: {bad:?} (tech:<technique> | art:<Shield Art> | hp:<n> | mp:<n> | slot:<kind>)"));
                }
                out.gof.push(GofRow { boss: key.to_string(), name: value["name"].as_str().unwrap_or(key).to_string(), steps: steps.into_iter().filter(|t| gof_token_ok(t)).collect(), group: text("group") });
                continue;
            }
            // (XSI, 2026-10-16) THE XS REWARDS (XS_SORA_ABILITIES.md 4.2): rows of their own, so that a DLL that predates them
            // reads the boss rows unchanged: `xs.<boss row id>` = { add: [tokens], replace: token | null }.
            // (DON-PROG) Sora's magic of the extra bosses (`sm.<boss row id>` = { add: [tokens] }), the XS rows' way
            if let Some(key) = id.strip_prefix("sm.") {
                let add: Vec<Token> = value["add"].as_array().map_or(&[][..], |a| a.as_slice()).iter().filter_map(|t| t.as_str().and_then(Token::parse)).collect();
                if add.is_empty() {
                    out.problems.push(format!("{id}: no reward understood"));
                } else {
                    sm_rows.push((key.to_string(), add));
                }
                continue;
            }
            // (P9b) the boss Keyblades (`kb.<boss row id>` = { add: ["weapon:<KEY>"] }): rows of their own, the XS rows' way
            if let Some(key) = id.strip_prefix("kb.") {
                let list = value["add"].as_array().map_or(&[][..], |a| a.as_slice());
                let add: Vec<Token> = list.iter().filter_map(|t| t.as_str().and_then(Token::parse)).filter(|t| matches!(t, Token::Weapon { .. })).collect();
                if add.is_empty() || add.len() != list.len() {
                    out.problems.push(format!("{id}: a reward is not a weapon:<KEY> token of the weapon table"));
                }
                if !add.is_empty() {
                    kb_rows.push((key.to_string(), add));
                }
                continue;
            }
            if let Some(key) = id.strip_prefix("xs.") {
                let add: Vec<Token> = value["add"].as_array().map_or(&[][..], |a| a.as_slice()).iter().filter_map(|t| t.as_str().and_then(Token::parse)).collect();
                let replace = value["replace"].as_str().and_then(Token::parse);
                if add.is_empty() {
                    out.problems.push(format!("{id}: no reward understood"));
                } else {
                    xs_rows.push((key.to_string(), add, replace));
                }
                continue;
            }
            let Some(key) = id.strip_prefix("boss.").filter(|k| !k.is_empty() && !k.contains(char::is_whitespace)) else {
                out.problems.push(format!("{id}: not a row of this sheet"));
                continue;
            };
            let mut rewards = Vec::new();
            let mut bad = Vec::new();
            let mut tokens = |list: &Value| {
                let before = rewards.len();
                for text in list.as_array().map_or(&[][..], |a| a.as_slice()) {
                    match text.as_str().and_then(Token::parse) {
                        Some(token) => rewards.push(token),
                        None => bad.push(text.to_string()),
                    }
                }
                rewards.len() - before
            };
            let user = tokens(&value["user"]);
            tokens(&value["proposal"]);
            if !bad.is_empty() {
                out.problems.push(format!("{id}: reward(s) not understood: {} (the row gives the others)", bad.join(", ")));
            }
            if out.rows.iter().any(|r| r.id == key) {
                out.problems.push(format!("{id}: a second row of this id (ignored)"));
                continue;
            }
            let flag = value["flag"].as_u64().filter(|f| *f > 0 && *f <= u32::MAX as u64).map(|f| (f as u32, conf(&value["flag_conf"])));
            if let Some((f, _)) = flag {
                if let Some(other) = out.rows.iter().find(|r| r.flag.is_some_and(|(g, _)| g == f)) {
                    out.problems.push(format!("{id}: its flag {f} is also {}'s: this row is identified by its chr id only", other.id));
                }
            }
            let flag = flag.filter(|(f, _)| !out.rows.iter().any(|r| r.flag.is_some_and(|(g, _)| g == *f)));
            out.rows.push(BossRow {
                id: key.to_string(),
                name: value["name"].as_str().unwrap_or(key).to_string(),
                region: value["region"].as_str().unwrap_or("").to_string(),
                order: value["order"].as_u64().unwrap_or(0) as u32,
                path: value["path"].as_str().unwrap_or("optional").to_string(),
                flag,
                chr: value["chr"].as_i64().filter(|c| *c > 0).map(|c| (c as i32, conf(&value["chr_conf"]))),
                area: value["area"].as_u64().filter(|a| *a <= 255).map(|a| a as u8),
                rewards,
                user,
                reserved: value["reserved"].as_bool().unwrap_or(false), // (BOSSFLAGS)
                xs: Vec::new(),
                sm: Vec::new(), // (DON-PROG)
            });
        }
        // (XSI) the XS rows onto their boss rows: the replaced token goes, the XS abilities come at the end
        for (key, add, replace) in xs_rows {
            let Some(row) = out.rows.iter_mut().find(|r| r.id == key) else {
                out.problems.push(format!("xs.{key}: no boss row of that id"));
                continue;
            };
            if let Some(old) = replace {
                if let Some(at) = row.rewards.iter().position(|t| *t == old) {
                    row.rewards.remove(at);
                    if at < row.user {
                        row.user -= 1;
                    }
                }
            }
            row.rewards.extend(add.iter().copied());
            row.xs = add;
        }
        // (DON-PROG) the extra bosses' Sora magic onto their boss rows, at the end
        for (key, add) in sm_rows {
            let Some(row) = out.rows.iter_mut().find(|r| r.id == key) else {
                out.problems.push(format!("sm.{key}: no boss row of that id"));
                continue;
            };
            row.rewards.extend(add.iter().copied());
            row.sm = add;
        }
        out.rows.sort_by_key(|r| r.order);
        // (P9b) the boss Keyblades onto their boss rows' verified flags, in the table's order
        for (key, tokens) in kb_rows {
            let Some(row) = out.rows.iter().find(|r| r.id == key) else {
                out.problems.push(format!("kb.{key}: no boss row of that id"));
                continue;
            };
            let flag = row.flag.filter(|(_, c)| *c == Conf::Verified).map(|(f, _)| f);
            if flag.is_none() {
                out.problems.push(format!("kb.{key}: its boss row has no verified defeat flag (never given)"));
            }
            if out.kb.iter().any(|k| k.boss == key) {
                out.problems.push(format!("kb.{key}: a second row of this boss (ignored)"));
                continue;
            }
            out.kb.push(KbRow { boss: key, name: row.name.clone(), flag, tokens });
        }
        let kb_order = |b: &str| out.rows.iter().find(|r| r.id == b).map_or(u32::MAX, |r| r.order);
        let mut kb = std::mem::take(&mut out.kb);
        kb.sort_by_key(|k| kb_order(&k.boss));
        out.kb = kb;
        // (DON-PROG) every `don.*` row names a boss row; in the table's order
        for d in &out.don {
            if !out.rows.iter().any(|r| r.id == d.boss) {
                out.problems.push(format!("don.{}: no boss row of that id", d.boss));
            }
        }
        let order = |b: &str| out.rows.iter().find(|r| r.id == b).map_or(u32::MAX, |r| r.order);
        let mut don = std::mem::take(&mut out.don);
        don.sort_by_key(|d| order(&d.boss));
        out.don = don;
        // (GF-STATS) every `gof.*` row names a boss row, or a group of reserved boss rows; in the table's order, the groups last
        let mut gof = std::mem::take(&mut out.gof);
        gof.sort_by_key(|g| order(&g.boss));
        out.gof = gof;
        for g in &out.gof {
            if g.group.is_empty() && out.row(&g.boss).is_none() {
                out.problems.push(format!("gof.{}: no boss row of that id", g.boss));
            }
            for member in &g.group {
                if !out.row(member).is_some_and(|r| r.reserved && r.flag.is_some()) {
                    out.problems.push(format!("gof.{}: group member {member} is not a reserved boss row with a flag", g.boss));
                }
            }
        }
        out
    }

    /// (DON-PROG) Donald's steps of a boss row (empty: none).
    pub fn don_of(&self, id: &str) -> Option<&DonRow> {
        self.don.iter().find(|d| d.boss.eq_ignore_ascii_case(id.trim()))
    }

    pub fn row(&self, id: &str) -> Option<&BossRow> {
        self.rows.iter().find(|r| r.id.eq_ignore_ascii_case(id.trim()))
    }

    /// The row a dead boss-bar character of model `chr` in map area `area` is.
    pub fn by_chr(&self, chr: i32, area: u8) -> Option<&BossRow> {
        self.rows.iter().find(|r| r.chr.is_some_and(|(c, _)| c == chr) && r.area.is_none_or(|a| a == area))
    }

    /// The table in one line, for the log.
    pub fn summary(&self) -> String {
        let count = |f: &dyn Fn(&BossRow) -> bool| self.rows.iter().filter(|r| f(r)).count();
        format!(
            "{} boss rows: {} identified by a defeat flag ({} verified, {} guess), {} also by a chr id ({} verified, {} guess), {} with no identifier; {} with a reward the user fixed, {} proposals only{}",
            self.rows.len(),
            count(&|r| r.flag.is_some()),
            count(&|r| r.flag.is_some_and(|(_, c)| c == Conf::Verified)),
            count(&|r| r.flag.is_some_and(|(_, c)| c == Conf::Guess)),
            count(&|r| r.chr.is_some()),
            count(&|r| r.chr.is_some_and(|(_, c)| c == Conf::Verified)),
            count(&|r| r.chr.is_some_and(|(_, c)| c == Conf::Guess)),
            count(&|r| r.flag.is_none() && r.chr.is_none()),
            count(&|r| r.user > 0),
            count(&|r| r.user == 0),
            if self.problems.is_empty() { String::new() } else { format!(" | {} PROBLEM(S): {}", self.problems.len(), self.problems.join("; ")) }
        )
    }
}

fn record_of(id: &str) -> String {
    format!("{RECORD_PREFIX}{id}")
}

/// This character has had the row's reward.
pub fn recorded(records: &[String], id: &str) -> bool {
    records.iter().any(|r| r.strip_prefix(RECORD_PREFIX).is_some_and(|k| k.eq_ignore_ascii_case(id)))
}

/// One reward as it was given.
#[derive(Clone, Debug, PartialEq)]
pub struct Given {
    pub token: Token,
    /// What he got, in words ("Thunder", "MP Safety", "Maximum HP +10", "Item slot").
    pub text: String,
    /// The bonus panel's line (`BonusLine::None`: nothing changed).
    pub line: BonusLine,
    pub changed: bool,
}

/// A row's reward as it was given.
#[derive(Clone, Debug, PartialEq)]
pub struct Claimed {
    pub id: String,
    pub name: String,
    pub given: Vec<Given>,
}

impl Claimed {
    /// "Thunder, MP Safety".
    pub fn text(&self) -> String {
        self.given.iter().map(|g| g.text.clone()).collect::<Vec<_>>().join(", ")
    }
}

/// What asking for a row's reward came to.
#[derive(Clone, Debug, PartialEq)]
pub enum Claim {
    Given(Claimed),
    /// This character has had it: nothing is given.
    Recorded,
    /// No row of that id.
    Unknown,
}

fn line_text(line: BonusLine) -> String {
    match line {
        BonusLine::None => String::new(),
        BonusLine::MaxHp(n) => format!("Maximum HP +{n}"),
        BonusLine::MaxMp(n) => format!("Maximum MP +{n}"),
        BonusLine::ArmorSlot => "Armor slot".to_string(),
        BonusLine::AccessorySlot => "Accessory slot".to_string(),
        BonusLine::ItemSlot => "Item slot".to_string(),
        BonusLine::Magic(command) => command_name(command).to_string(),
        BonusLine::Ability(kind) => ability_name(kind).to_string(),
        BonusLine::MaxAp(n) => format!("Maximum AP +{n}"),
        BonusLine::Shotlock(name) | BonusLine::Text(name) => name.to_string(),
    }
}

/// The row's rewards go to `sora` (no panel) and the row is recorded.
fn give(table: &Table, records: &mut Vec<String>, sora: &mut Sora, row: &BossRow) -> Claimed {
    let claimed = give_tokens(table, records, sora, row, &row.rewards);
    // (XSI) a row given with its XS rewards needs no XS catch-up
    if !row.xs.is_empty() && !xs_recorded(records, &row.id) {
        records.push(format!("{XS_RECORD_PREFIX}{}", row.id));
    }
    // (DON-PROG) the same for its overhaul Sora magic
    if !row.sm.is_empty() && !sm_recorded(records, &row.id) {
        records.push(format!("{SM_RECORD_PREFIX}{}", row.id));
    }
    claimed
}

/// (XSI) A record of the XS rewards of a row in `Inventory::records` (`xs:<row id>`).
pub const XS_RECORD_PREFIX: &str = "xs:";

/// (XSI) This character has had the row's XS rewards.
pub fn xs_recorded(records: &[String], id: &str) -> bool {
    records.iter().any(|r| r.strip_prefix(XS_RECORD_PREFIX).is_some_and(|k| k.eq_ignore_ascii_case(id)))
}

/// (XSI, 2026-10-16) THE XS CATCH-UP: a character whose record has a boss row's reward (`boss:<id>`, given by a build
/// that predates the XS rows) but not its XS rewards (`xs:<id>`) gets the XS rewards now, once, recorded `xs:<id>`,
/// with the reward panel (one for all, as the catch-up's). A row's replaced token (Mohg, Lord of Blood's Focus Syphon)
/// is not taken back. Call it once when a save is loaded.
pub fn xs_catch_up(table: &Table, records: &mut Vec<String>, sora: &mut Sora) -> Vec<Claimed> {
    let mut out = Vec::new();
    for row in table.rows.iter().filter(|r| !r.xs.is_empty()) {
        if recorded(records, &row.id) && !xs_recorded(records, &row.id) {
            let claimed = give_tokens(table, records, sora, row, &row.xs);
            records.push(format!("{XS_RECORD_PREFIX}{}", row.id));
            out.push(claimed);
        }
    }
    let lines: Vec<BonusLine> = out.iter().flat_map(|c| c.given.iter().filter(|g| g.changed).map(|g| g.line)).collect();
    if lines.len() <= 2 {
        panels(sora, &lines);
    } else {
        let [first, second] = summary_lines(&out);
        sora.show_bonus([BonusLine::Text(Box::leak(first.into_boxed_str())), BonusLine::Text(Box::leak(second.into_boxed_str()))]);
    }
    out
}

/// (DON-PROG) A record of a row's overhaul Sora magic in `Inventory::records` (`sora_magic:<row id>`).
pub const SM_RECORD_PREFIX: &str = "sora_magic:";

/// (DON-PROG) This character has had the row's overhaul Sora magic.
pub fn sm_recorded(records: &[String], id: &str) -> bool {
    records.iter().any(|r| r.strip_prefix(SM_RECORD_PREFIX).is_some_and(|k| k.eq_ignore_ascii_case(id)))
}

/// (DON-PROG) THE OVERHAUL'S SORA CATCH-UP (the user: "anything less than perfect" for an old save with many bosses
/// beaten): a character whose record has an extra boss's reward (`boss:<id>`, given before the Donald overhaul added
/// that boss's Sora magic) but not its overhaul magic (`sora_magic:<id>`) gets it now, once: one tier more of the
/// element, or Max MP +3 when he already has its -ga (the rule of every magic token). Recorded `sora_magic:<id>`; one
/// panel for all, as the XS catch-up. Call it once when a save is loaded.
pub fn sm_catch_up(table: &Table, records: &mut Vec<String>, sora: &mut Sora) -> Vec<Claimed> {
    let mut out = Vec::new();
    for row in table.rows.iter().filter(|r| !r.sm.is_empty()) {
        if recorded(records, &row.id) && !sm_recorded(records, &row.id) {
            let claimed = give_tokens(table, records, sora, row, &row.sm);
            records.push(format!("{SM_RECORD_PREFIX}{}", row.id));
            out.push(claimed);
        }
    }
    let lines: Vec<BonusLine> = out.iter().flat_map(|c| c.given.iter().filter(|g| g.changed).map(|g| g.line)).collect();
    if lines.len() <= 2 {
        panels(sora, &lines);
    } else {
        let [first, second] = summary_lines(&out);
        sora.show_bonus([BonusLine::Text(Box::leak(first.into_boxed_str())), BonusLine::Text(Box::leak(second.into_boxed_str()))]);
    }
    out
}

fn give_tokens(table: &Table, records: &mut Vec<String>, sora: &mut Sora, row: &BossRow, tokens: &[Token]) -> Claimed {
    let runes_before = table.rows.iter().filter(|r| r.great_rune() && recorded(records, &r.id)).count();
    let mut given = Vec::new();
    for &token in tokens {
        // (P13 ORE) an item: into the stock through the item door (the HUD's item popup); no line of the bonus panel
        if let Token::Item { item_id, count } = token {
            let ok = crate::items::give(item_id, u32::from(count));
            given.push(Given { token, text: if ok { token.label() } else { format!("{}: no such item: nothing", token.label()) }, line: BonusLine::None, changed: ok });
            continue;
        }
        // (P9b) a weapon: through the weapons' door (the OBTAINED popup when it arrives); no line of the bonus panel
        if let Token::Weapon { item_id } = token {
            let gift = crate::weapons::give(item_id, 1);
            given.push(Given { token, text: weapon_text(&token.label(), gift), line: BonusLine::None, changed: matches!(gift, Gift::Given(_)) });
            continue;
        }
        let reward = match token {
            // (DON-PROG) the element's -ga tier already his: Max MP +3 instead (DONALD_OVERHAUL.md 9)
            Token::Magic { element, .. } if sora.status().magic_tier(element) >= 3 => Reward::max_mp(SORA_TOP_MP),
            Token::Magic { element, .. } => Some(Reward::Magic { element, tier: 0 }),
            Token::Ability(kind) => Some(Reward::Ability { kind, copies: 1 }),
            Token::MaxHp(n) => Reward::max_hp(n),
            Token::MaxMp(n) => Reward::max_mp(n),
            Token::Slot(kind) => Some(Reward::Bonus(kind)),
            Token::GreatRuneSlot => table.slot_order.get(runes_before).map(|kind| Reward::Bonus(*kind)),
            Token::Item { .. } | Token::Weapon { .. } => None, // (P13 ORE, P9b: given above)
        };
        // (DF, 2026-10-15) KH3 CRITICAL (0x140586280 case 1): a bonus whose ability Critical gives at the start
        // (sheet sora-critical-start) gives only its other parts; the ability is not given again
        let critical_start = matches!(token, Token::Ability(kind) if sora.status().game_level.is_critical() && kh_core::status::tables::is_critical_start(kind));
        // (XSI) "already have it: nothing extra" (rule.skip_if_owned)
        let owned_once = matches!(token, Token::Ability(kind) if table.skip_if_owned.contains(&kind) && sora.status().abilities.owned(kind) > 0);
        let reward = if critical_start || owned_once { None } else { reward };
        let outcome = reward.map(|r| sora.grant_quiet(r)).unwrap_or_default();
        let line = if outcome.changed { outcome.lines[0] } else { BonusLine::None };
        let text = if outcome.changed {
            line_text(line)
        } else if critical_start {
            format!("{}: Critical gives it at the start: nothing", token.label())
        } else if owned_once {
            format!("{}: he has it already (rule.skip_if_owned): nothing", token.label())
        } else if reward.is_none() {
            format!("{}: the slot list is used up (Great Rune no. {}): nothing", token.label(), runes_before + 1)
        } else {
            format!("{}: he has it all already: nothing", token.label())
        };
        given.push(Given { token, text, line, changed: outcome.changed });
    }
    if !recorded(records, &row.id) {
        records.push(record_of(&row.id));
    }
    Claimed { id: row.id.clone(), name: row.name.clone(), given }
}

fn panels(sora: &mut Sora, lines: &[BonusLine]) {
    // (P13 ORE) an item token has no line (its own popup tells it)
    let lines: Vec<BonusLine> = lines.iter().copied().filter(|l| *l != BonusLine::None).collect();
    for pair in lines.chunks(2) {
        sora.show_bonus([pair[0], pair.get(1).copied().unwrap_or(BonusLine::None)]);
    }
}

/// THE BOSS OF ROW `id` IS BEATEN: its reward once per character. `show`: the game's bonus panel(s), two lines each
/// (raised at Sora's next step, with the panel's sound).
pub fn claim(table: &Table, records: &mut Vec<String>, sora: &mut Sora, id: &str, show: bool) -> Claim {
    let Some(row) = table.row(id) else { return Claim::Unknown };
    if recorded(records, &row.id) {
        return Claim::Recorded;
    }
    let claimed = give(table, records, sora, row);
    if show {
        panels(sora, &claimed.given.iter().filter(|g| g.changed).map(|g| g.line).collect::<Vec<_>>());
    }
    Claim::Given(claimed)
}

/// THE CATCH-UP: every row of `ids` not recorded yet is given (in the table's order), with ONE panel for all of
/// them: the lines themselves when there are at most two, else "<n> boss rewards obtained" and the spells' names.
pub fn claim_all(table: &Table, records: &mut Vec<String>, sora: &mut Sora, ids: &[String]) -> Vec<Claimed> {
    let mut out = Vec::new();
    for row in table.rows.iter().filter(|r| ids.iter().any(|id| id.eq_ignore_ascii_case(&r.id))) {
        if !recorded(records, &row.id) {
            out.push(give(table, records, sora, row));
        }
    }
    let lines: Vec<BonusLine> = out.iter().flat_map(|c| c.given.iter().filter(|g| g.changed).map(|g| g.line)).collect();
    if lines.len() <= 2 {
        panels(sora, &lines);
    } else {
        let [first, second] = summary_lines(&out);
        // (a line of the panel lives as long as the program: a handful of them per session)
        sora.show_bonus([BonusLine::Text(Box::leak(first.into_boxed_str())), BonusLine::Text(Box::leak(second.into_boxed_str()))]);
    }
    out
}

/// The two lines of the catch-up's one panel.
pub fn summary_lines(claimed: &[Claimed]) -> [String; 2] {
    let changed: Vec<&Given> = claimed.iter().flat_map(|c| c.given.iter().filter(|g| g.changed)).collect();
    let spells: Vec<String> = changed.iter().filter(|g| matches!(g.line, BonusLine::Magic(_))).map(|g| g.text.clone()).collect();
    let others = changed.len() - spells.len();
    let second = match (spells.len(), others) {
        (0, n) => format!("{n} abilities and bonuses"),
        (_, 0) => spells.join(", "),
        (s, n) if s <= 3 => format!("{} +{n} more", spells.join(", ")),
        (s, n) => format!("{} spells +{n} more", s),
    };
    [format!("Rewards of {} bosses obtained", claimed.len()), second]
}

// ------------------------------------------------------------------------------------------------ (P13 ORE) the placements

/// (P13 ORE) The compiled copy of the fixed ore placements' sheet.
pub const PLACEMENTS_SHEET: &str = include_str!("../../../sheets/port/kh-ore-placements.json");
/// (P13 ORE) A record of a given placement in `Inventory::records` (`ore:<placement id>`, e.g. `ore:boss.radahn`).
pub const PLACEMENT_RECORD_PREFIX: &str = "ore:";

/// (P13 ORE, documentation/KH3_WEAPONS_PLAN.md 3.1; THE USER 2026-11-03: "orichalcum must be a fixed item drop (never random)
/// and there should be enough to craft everything") ONE FIXED PLACEMENT of sheet sheets/port/kh-ore-placements.json: row
/// `ore.boss.<boss row>` (the boss's verified defeat flag, the flag of its row of this module's table) or `ore.chest.<n>` (a
/// chest's item flag: package P9a names it). ONCE PER SAVE (record `ore:<id>`, apart from the boss row's own record, so a
/// save past the boss gets it at its load: the catch-up) its item token is given when its flag is set. Only an ACTIVE
/// placement (verified, a flag, an `item:` token) is ever given; an unverified one waits (and is counted by nothing).
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    /// `boss.radahn`, `chest.1` (the row id without `ore.`).
    pub id: String,
    /// The boss row it follows (`ore.boss.*`).
    pub boss: Option<String>,
    pub flag: Option<u32>,
    pub give: Option<Token>,
    pub verified: bool,
    /// Shadow of the Erdtree: not counted for the base game's need.
    pub dlc: bool,
    /// Its place in the progression (the boss row's `order`; a chest's own).
    pub order: u32,
    pub place: String,
}

impl Placement {
    pub fn active(&self) -> bool {
        self.verified && self.flag.is_some() && matches!(self.give, Some(Token::Item { .. }))
    }
}

/// (P13 ORE) The sheet: the placements and the need rows (`need.<name>` = a count).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Placements {
    pub rows: Vec<Placement>,
    pub need: Vec<(String, u32)>,
    pub problems: Vec<String>,
}

impl Placements {
    /// The sheet as it is on disk (the user edits it), else the compiled copy; checked against the boss table.
    pub fn load(table: &Table) -> Placements {
        let path = kh_gendata::project_file("sheets/port/kh-ore-placements.json");
        let read = std::fs::read_to_string(path).map(|text| Placements::parse(&text, table)).unwrap_or_default();
        if read.rows.is_empty() {
            return Placements::parse(PLACEMENTS_SHEET, table);
        }
        read
    }

    /// The rows; a problem for a row not understood, a boss row that is not the table's or whose flag is not the table's
    /// verified one (the row is then not verified), two placements on one flag (the second is not verified).
    pub fn parse(text: &str, table: &Table) -> Placements {
        let mut out = Placements::default();
        let Ok(json) = serde_json::from_str::<Value>(text) else {
            out.problems.push("the sheet is not JSON".to_string());
            return out;
        };
        for row in json["rows"].as_array().map_or(&[][..], |r| r.as_slice()) {
            let (Some(id), value) = (row[0].as_str(), &row[1]) else { continue };
            if let Some(name) = id.strip_prefix("need.") {
                match value.as_u64() {
                    Some(n) => out.need.push((name.to_string(), n as u32)),
                    None => out.problems.push(format!("{id}: not a count")),
                }
                continue;
            }
            let Some(key) = id.strip_prefix("ore.") else { continue };
            let give = value["give"].as_str().and_then(Token::parse);
            if !matches!(give, Some(Token::Item { .. })) {
                out.problems.push(format!("{id}: give {} is not an item:<KEY>[:n] token", value["give"]));
            }
            let mut p = Placement {
                id: key.to_string(),
                boss: value["boss"].as_str().map(str::to_string),
                flag: value["flag"].as_u64().filter(|f| *f > 0 && *f <= u32::MAX as u64).map(|f| f as u32),
                give,
                verified: value["verified"].as_bool() == Some(true),
                dlc: value["dlc"].as_bool() == Some(true),
                order: value["order"].as_u64().unwrap_or(u32::MAX as u64).min(u32::MAX as u64) as u32,
                place: value["place"].as_str().unwrap_or(key).to_string(),
            };
            if key.starts_with("boss.") {
                match p.boss.as_deref().and_then(|b| table.row(b)) {
                    None => {
                        out.problems.push(format!("{id}: no boss row {:?} in sheet sora-er-boss-rewards", p.boss));
                        p.verified = false;
                    }
                    Some(b) => {
                        if b.flag != p.flag.map(|f| (f, Conf::Verified)) {
                            out.problems.push(format!("{id}: flag {:?} is not the verified defeat flag {:?} of boss.{}", p.flag, b.flag, b.id));
                            p.verified = false;
                        }
                        p.order = b.order;
                    }
                }
            }
            if let Some(f) = p.flag {
                if let Some(other) = out.rows.iter().find(|o| o.flag == Some(f)) {
                    out.problems.push(format!("{id}: its flag {f} is also ore.{}'s (not given twice)", other.id));
                    p.verified = false;
                }
            }
            if out.rows.iter().any(|o| o.id == p.id) {
                out.problems.push(format!("{id}: a second row of this id (ignored)"));
                continue;
            }
            out.rows.push(p);
        }
        out
    }

    /// The count of item `key` the ACTIVE placements give (`dlc`: the DLC's too).
    pub fn placed(&self, key: &str, dlc: bool) -> u32 {
        let Some(item_id) = crate::items::by_key(key).map(|i| i.item_id) else { return 0 };
        self.rows.iter().filter(|p| p.active() && (dlc || !p.dlc)).map(|p| match p.give {
            Some(Token::Item { item_id: i, count }) if i == item_id => u32::from(count),
            _ => 0,
        })
        .sum()
    }

    /// A need row (`need.orichalcum`).
    pub fn need(&self, name: &str) -> Option<u32> {
        self.need.iter().find(|(n, _)| n == name).map(|(_, c)| *c)
    }
}

/// (P13 ORE) The placements of the sheet, read once (against the boss table as it is on disk).
pub fn placements() -> &'static Placements {
    static PLACEMENTS: std::sync::OnceLock<Placements> = std::sync::OnceLock::new();
    PLACEMENTS.get_or_init(|| Placements::load(&Table::load()))
}

/// (P13 ORE) This save has had the placement.
pub fn placement_recorded(records: &[String], id: &str) -> bool {
    records.iter().any(|r| r.strip_prefix(PLACEMENT_RECORD_PREFIX).is_some_and(|k| k.eq_ignore_ascii_case(id)))
}

/// (P13 ORE) THE PLACEMENTS DUE: active, not recorded, their flag set (`flag`: the host's event flag reader; `None` = not
/// readable: not due). The host calls it when the flags are readable (a catch-up and a defeat in play alike).
pub fn placements_due<'a>(placements: &'a Placements, records: &[String], flag: &dyn Fn(u32) -> Option<bool>) -> Vec<&'a Placement> {
    placements.rows.iter().filter(|p| p.active() && !placement_recorded(records, &p.id) && p.flag.and_then(flag) == Some(true)).collect()
}

/// (P13 ORE) A PLACEMENT GIVEN: its item token (the item door: the HUD's item popup) and the record `ore:<id>`. `None`: not
/// active, or this save has had it.
pub fn claim_placement(records: &mut Vec<String>, placement: &Placement) -> Option<Given> {
    if !placement.active() || placement_recorded(records, &placement.id) {
        return None;
    }
    let token = placement.give?;
    let Token::Item { item_id, count } = token else { return None };
    let ok = crate::items::give(item_id, u32::from(count));
    records.push(format!("{PLACEMENT_RECORD_PREFIX}{}", placement.id));
    Some(Given { token, text: if ok { token.label() } else { format!("{}: no such item: nothing", token.label()) }, line: BonusLine::None, changed: ok })
}

// ------------------------------------------------------------------------------------- (P9b ACQUISITION) the weapons' sources
//
// documentation/KH3_WEAPONS_PLAN.md 1.7, 2, 6b; the sources are P9a's verified rows of sheets/port/kh-weapon-sources.json
// (chests by their lot's getItemFlagId, rare drops by NpcParam row) and the `kb.*` rows of this module's sheet (bosses by
// their verified defeat flag). THE RULES:
// - A BOSS KEYBLADE (`kb.<boss>`) and A CHEST'S WEAPON (`chest.<KEY>`) are given ONCE PER SAVE (records `kb:<boss>`,
//   `wsrc:<flag>`) when their flag is set: in play when it rises, and at a save's load for every flag already set (the
//   catch-up: P1's migration leaves no record on an older save, so everything due is given). A Keyblade already owned is
//   refused by `crate::weapons::give` ("already owned: no duplicate") and its source is recorded all the same.
// - A RARE DROP (`drop.<KEY>.<n>`, chance 0.02) is rolled for each REAL death of an enemy of its NpcParam rows (the host's
//   kill handling: no respawn, no reload counts); a friend's weapon may come again (duplicates allowed), a Keyblade only
//   while it is not owned. No record.
// - The gift reaches the HUD through the weapons' door; the HUD shows KH3's OBTAINED popup when it arrives
//   (hud_obtained.rs `ObtainedKind::Weapon`); a purchase never goes through the door (no popup).

static HOLD_POPUPS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (P9b, plan Q4: "synthesis shows it after KH3's own result") THE SYNTHESIS SCREEN'S CALL (P6b): `true` before a synthesis
/// whose reward is a weapon, `false` once KH3's own result screen has closed. Meanwhile a weapon reaching the stock keeps its
/// OBTAINED popup back (hud_obtained.rs); on `false` the popup is queued at the HUD's next frame.
pub fn hold_weapon_popups(on: bool) {
    HOLD_POPUPS.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// (P9b) The weapon popups are held now (a synthesis' result screen is up).
pub fn weapon_popups_held() -> bool {
    HOLD_POPUPS.load(std::sync::atomic::Ordering::Relaxed)
}

/// (P9b) A weapon source's gift in words ("Hero's Origin", "Hero's Origin: already owned: no duplicate ...").
pub fn weapon_text(name: &str, gift: Gift) -> String {
    match gift {
        Gift::Given(_) => name.to_string(),
        Gift::AlreadyOwned => format!("{name}: already owned: no duplicate (the source is marked done)"),
        Gift::NotAWeapon => format!("{name}: not a weapon of the table: nothing"),
    }
}

/// (P9b) One weapon a source handed over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponGiven {
    pub key: String,
    pub name: String,
    pub gift: Gift,
}

impl WeaponGiven {
    /// It is on its way to the stock (not refused).
    pub fn given(&self) -> bool {
        matches!(self.gift, Gift::Given(_))
    }

    pub fn text(&self) -> String {
        weapon_text(&self.name, self.gift)
    }
}

fn give_weapon(item_id: u32, give: &mut dyn FnMut(u32) -> Gift) -> WeaponGiven {
    let (key, name) = crate::weapons::by_id(item_id).map_or_else(|| (format!("{item_id:#x}"), format!("weapon {item_id:#x}")), |w| (w.key.clone(), w.name.clone()));
    WeaponGiven { key, name, gift: give(item_id) }
}

/// The weapons' door for one weapon (`crate::weapons::give`, one copy).
pub fn give_one(item_id: u32) -> Gift {
    crate::weapons::give(item_id, 1)
}

/// (P9b) This save has had the boss's Keyblade (`kb:<boss>`).
pub fn kb_recorded(records: &[String], boss: &str) -> bool {
    records.iter().any(|r| r.strip_prefix(crate::weapons::KB_RECORD_PREFIX).is_some_and(|k| k.eq_ignore_ascii_case(boss)))
}

/// (P9b) THE BOSS KEYBLADES DUE: a verified flag that is set (`flag`: the host's reader; `None` = not readable: not due),
/// not recorded. In the table's order.
pub fn kb_due<'a>(table: &'a Table, records: &[String], flag: &dyn Fn(u32) -> Option<bool>) -> Vec<&'a KbRow> {
    table.kb.iter().filter(|k| !kb_recorded(records, &k.boss) && k.flag.and_then(flag) == Some(true)).collect()
}

/// (P9b) A BOSS KEYBLADE GIVEN through `give` (the weapons' door: [`give_one`]) and recorded `kb:<boss>`, owned or not.
/// `None`: recorded already.
pub fn claim_kb_with(records: &mut Vec<String>, row: &KbRow, give: &mut dyn FnMut(u32) -> Gift) -> Option<Vec<WeaponGiven>> {
    if kb_recorded(records, &row.boss) {
        return None;
    }
    let out = row.tokens.iter().filter_map(|t| if let Token::Weapon { item_id } = t { Some(give_weapon(*item_id, give)) } else { None }).collect();
    records.push(crate::weapons::kb_record(&row.boss));
    Some(out)
}

/// The compiled copy of P9a's sources sheet.
pub const WEAPON_SOURCES_SHEET: &str = include_str!("../../../sheets/port/kh-weapon-sources.json");

/// (P9b) A chest (or a fixed pickup where the area has none) whose weapon is given once when the game sets its lot's
/// getItemFlagId (`chest.<KEY>`). The chest's own Elden Ring item comes as before (the loot conversion).
#[derive(Clone, Debug, PartialEq)]
pub struct ChestSource {
    /// `chest.WEP_KEYBLADE_SO_17`.
    pub id: String,
    pub key: String,
    pub item_id: u32,
    pub flag: u32,
    pub place: String,
    pub verified: bool,
}

/// (P9b) A rare drop (`drop.<KEY>.<n>`): an enemy kind's NpcParam rows and the chance per real death.
#[derive(Clone, Debug, PartialEq)]
pub struct DropSource {
    pub id: String,
    pub key: String,
    pub item_id: u32,
    /// The kind's character model (`c3702` -> 3702).
    pub chr: i32,
    /// The NpcParam rows the maps place (Spirit Ash summons of the same model use other rows: they do not count).
    pub npc_params: Vec<i32>,
    pub chance: f32,
    pub kind: String,
    pub verified: bool,
}

/// (P9b) P9a's sheet as the acquisition reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponSources {
    pub chests: Vec<ChestSource>,
    pub drops: Vec<DropSource>,
    pub problems: Vec<String>,
}

impl WeaponSources {
    /// The sheet on disk (the user edits it), else the compiled copy.
    pub fn load() -> WeaponSources {
        let path = kh_gendata::project_file("sheets/port/kh-weapon-sources.json");
        let read = std::fs::read_to_string(path).map(|t| WeaponSources::parse(&t)).unwrap_or_default();
        if read.chests.is_empty() && read.drops.is_empty() {
            return WeaponSources::parse(WEAPON_SOURCES_SHEET);
        }
        read
    }

    /// The `chest.*` and `drop.*` rows; a problem (and the row not verified) for a key that is no weapon, a flag shared
    /// by two chest rows, a chance outside 0 .. 1.
    pub fn parse(text: &str) -> WeaponSources {
        let mut out = WeaponSources::default();
        let Ok(json) = serde_json::from_str::<Value>(text) else {
            out.problems.push("the sheet is not JSON".to_string());
            return out;
        };
        for row in json["rows"].as_array().map_or(&[][..], |r| r.as_slice()) {
            let (Some(id), v) = (row[0].as_str(), &row[1]) else { continue };
            let weapon = v["item"].as_str().and_then(crate::weapons::by_key);
            if id.starts_with("chest.") {
                let flag = v["flag"].as_u64().filter(|f| *f > 0 && *f <= u64::from(u32::MAX)).map_or(0, |f| f as u32);
                let mut verified = v["verified"].as_bool() == Some(true) && flag > 0 && weapon.is_some();
                if weapon.is_none() {
                    out.problems.push(format!("{id}: item {} is not a weapon of the table", v["item"]));
                }
                if let Some(other) = out.chests.iter().find(|c| c.flag == flag && flag > 0) {
                    out.problems.push(format!("{id}: its flag {flag} is also {}'s (not given twice)", other.id));
                    verified = false;
                }
                out.chests.push(ChestSource { id: id.to_string(), key: weapon.map_or_else(String::new, |w| w.key.clone()), item_id: weapon.map_or(0, |w| w.item_id), flag, place: v["place"].as_str().unwrap_or(id).to_string(), verified });
            } else if id.starts_with("drop.") {
                let chance = v["chance"].as_f64().unwrap_or(-1.0);
                let ok_chance = (0.0..=1.0).contains(&chance);
                if weapon.is_none() || !ok_chance {
                    out.problems.push(format!("{id}: item {} / chance {} not understood", v["item"], v["chance"]));
                }
                let chr = v["chr"].as_str().and_then(|c| c.trim_start_matches('c').parse::<i32>().ok()).unwrap_or(0);
                let npc_params: Vec<i32> = v["npc_params"].as_array().map_or(&[][..], |a| a.as_slice()).iter().filter_map(|n| n.as_i64().and_then(|n| i32::try_from(n).ok())).collect();
                out.drops.push(DropSource {
                    id: id.to_string(),
                    key: weapon.map_or_else(String::new, |w| w.key.clone()),
                    item_id: weapon.map_or(0, |w| w.item_id),
                    chr,
                    verified: v["verified"].as_bool() == Some(true) && weapon.is_some() && ok_chance && (chr > 0 || !npc_params.is_empty()),
                    npc_params,
                    chance: chance.clamp(0.0, 1.0) as f32,
                    kind: v["kind"].as_str().unwrap_or("").to_string(),
                });
            }
        }
        out
    }

    /// The verified drop rows of an enemy of this NpcParam row (its own rows, else its model when a row names none).
    pub fn drops_of(&self, npc_param: i32) -> impl Iterator<Item = &DropSource> {
        self.drops.iter().filter(move |d| d.verified && npc_param > 0 && if d.npc_params.is_empty() { d.chr == npc_param / 10_000 } else { d.npc_params.contains(&npc_param) })
    }
}

/// (P9b) The sources of the sheet, read once.
pub fn weapon_sources() -> &'static WeaponSources {
    static SOURCES: std::sync::OnceLock<WeaponSources> = std::sync::OnceLock::new();
    SOURCES.get_or_init(WeaponSources::load)
}

/// (P9b) This save has had the weapon of the source with this flag (`wsrc:<flag>`).
pub fn wsrc_recorded(records: &[String], flag: u32) -> bool {
    let want = crate::weapons::wsrc_record(flag);
    records.contains(&want)
}

/// (P9b) THE CHESTS DUE: verified, their flag set (`None` = not readable: not due), not recorded.
pub fn chests_due<'a>(sources: &'a WeaponSources, records: &[String], flag: &dyn Fn(u32) -> Option<bool>) -> Vec<&'a ChestSource> {
    sources.chests.iter().filter(|c| c.verified && !wsrc_recorded(records, c.flag) && flag(c.flag) == Some(true)).collect()
}

/// (P9b) A CHEST'S WEAPON GIVEN through `give` and recorded `wsrc:<flag>`, owned or not. `None`: not verified, or recorded.
pub fn claim_chest_with(records: &mut Vec<String>, chest: &ChestSource, give: &mut dyn FnMut(u32) -> Gift) -> Option<WeaponGiven> {
    if !chest.verified || wsrc_recorded(records, chest.flag) {
        return None;
    }
    let out = give_weapon(chest.item_id, give);
    records.push(crate::weapons::wsrc_record(chest.flag));
    Some(out)
}

/// (P9b) THE RARE DROPS OF ONE REAL DEATH of an enemy of NpcParam row `npc_param`: each verified row of it is rolled once
/// (`unit`: 0 .. 1, a hit below the row's chance); a Keyblade's row only while `owns` says it is not owned (no duplicate);
/// a friend's weapon always (duplicates allowed). The host gives what comes out as a KH3 prize box.
pub fn drop_roll<'a>(sources: &'a WeaponSources, npc_param: i32, unit: &mut dyn FnMut() -> f32, owns: &dyn Fn(&str) -> bool) -> Vec<&'a DropSource> {
    sources.drops_of(npc_param).filter(|d| !(crate::weapons::is_keyblade(&d.key) && owns(&d.key))).filter(|d| unit() < d.chance).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::Inventory;
    use crate::save::{new_game_loadout, SoraSave};
    use kh_core::status::{Loadout, Status};
    use ETresAbilityKind as A;

    const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../");

    fn base() -> Loadout {
        let path = format!("{ROOT}sheets/port/sora-loadout-99.json");
        crate::rig::loadout::parse(&path, &std::fs::read_to_string(&path).unwrap()).unwrap().0
    }

    /// The true level-1 start.
    fn new_sora() -> Sora {
        let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: glam::Vec3::ZERO, yaw: 0.0 }).with_status(Status::from_loadout(&new_game_loadout(&base(), None)));
        kh_core::command::customize::ensure(&mut sora);
        sora
    }

    fn tiers(sora: &Sora) -> Vec<(&'static str, u32)> {
        Element::ALL.into_iter().map(|e| (e.name(), sora.status().magic_tier(e))).collect()
    }

    #[test]
    fn the_sheet_parses_and_the_users_two_rows_are_as_the_user_said() {
        let table = Table::parse(SHEET);
        assert!(table.problems.is_empty(), "{:?}", table.problems);
        assert_eq!(table.rows.len(), 64); // (DON-PROG) + the 6 Arcane bosses of the Donald overhaul (R27); (BOSSFLAGS) + 10 reserved
        assert_eq!(table.slot_order.len(), 7);
        assert_eq!(table.rows.iter().filter(|r| r.reserved).count(), 10); // (BOSSFLAGS)
        assert!(table.row("crucible_knight_stormhill").is_some_and(|r| r.reserved && r.rewards.is_empty()));
        // THE USER: "Make Margit give you thunder ... and Godrick the grafted gives you heal"
        let margit = table.row("margit").unwrap();
        assert_eq!((margit.rewards[0], margit.user, margit.flag, margit.chr, margit.area), (Token::Magic { element: Element::Thunder, tier: 1 }, 1, Some((10000850, Conf::Verified)), Some((2130, Conf::Verified)), Some(10)));
        let godrick = table.row("godrick").unwrap();
        assert_eq!((godrick.rewards[0], godrick.user, godrick.great_rune()), (Token::Magic { element: Element::Cure, tier: 1 }, 1, true));
        // every other row is a proposal; every row has an identifier; no two rows share a flag; orders are 1..=48
        assert_eq!(table.rows.iter().filter(|r| r.user > 0).count(), 3);
        // THE USER: "Glide should go to Maliketh" (the last Cure tier beside it is a proposal)
        let maliketh = table.row("maliketh").unwrap();
        assert_eq!((maliketh.rewards.clone(), maliketh.user), (vec![Token::Ability(A::Glide), Token::Magic { element: Element::Cure, tier: 3 }], 1));
        assert!(!table.row("morgott").unwrap().rewards.contains(&Token::Ability(A::Glide)));
        // (DON-PROG) a row without a reward of Sora's is a Donald-only boss (its don.* row)
        assert!(table.rows.iter().all(|r| r.flag.is_some() && (!r.rewards.is_empty() || table.don_of(&r.id).is_some() || r.reserved)));
        assert_eq!(table.rows.iter().map(|r| r.order).collect::<Vec<_>>(), (1..=64).collect::<Vec<_>>());
        // Margit's chr id never matches in the capital (the re-encounter outside Leyndell, Morgott): only in map area 10
        assert_eq!(table.by_chr(2130, 10).map(|r| r.id.as_str()), Some("margit"));
        assert_eq!(table.by_chr(2130, 11), None);
        assert_eq!(table.by_chr(2130, 60), None);
        // seven Great Rune rows for the seven slots
        assert_eq!(table.rows.iter().filter(|r| r.great_rune()).count(), 7);
        assert!(table.summary().starts_with("64 boss rows: 64 identified by a defeat flag (64 verified, 0 guess), 2 also by a chr id (1 verified, 1 guess), 0 with no identifier; 3 with a reward the user fixed, 61 proposals only"), "{}", table.summary());
    }

    /// NOTHING A NEW GAME ALREADY HAS IS GIVEN, AND NOTHING IS LEFT OVER: per element the table has exactly as many
    /// rows as tiers the true start lacks, naming each missing tier once; no ability is given beyond three copies.
    #[test]
    fn the_table_fits_the_true_level_1_start() {
        let table = Table::parse(SHEET);
        let sora = new_sora();
        assert_eq!(tiers(&sora), vec![("fire", 1), ("blizzard", 0), ("thunder", 0), ("water", 1), ("aero", 0), ("cure", 0)]);
        for element in Element::ALL {
            let mut named: Vec<u8> = table.rows.iter().flat_map(|r| r.rewards.iter()).filter_map(|t| match t { Token::Magic { element: e, tier } if *e == element => Some(*tier), _ => None }).collect();
            named.sort();
            let have = sora.status().magic_tier(element) as u8;
            // (DON-PROG) one boss more than his tiers (the element's DLC boss, shared with Donald): its token names the -ga
            assert_eq!(named, ((have + 1)..=3).chain([3]).collect::<Vec<_>>(), "{element:?}");
        }
        // the whole table on a new game: every reward changes something, and he ends with every spell at tier 3
        let mut sora = new_sora();
        let mut records = Vec::new();
        let (hp, mp, slots) = (sora.status().max_hp(), sora.status().max_mp(), sora.status().item_slots);
        for row in &table.rows {
            let Claim::Given(claimed) = claim(&table, &mut records, &mut sora, &row.id, false) else { panic!("{}", row.id) };
            assert!(claimed.given.iter().all(|g| g.changed), "{}: {}", row.id, claimed.text());
        }
        assert!(tiers(&sora).iter().all(|(_, tier)| *tier == 3));
        assert_eq!((sora.status().max_hp() - hp, sora.status().max_mp() - mp), (280, 76 + 6 * SORA_TOP_MP)); // (2026-11-01, the user: "give Sora more stat rewards from the DLC": +120 HP, +12 MP)
        // (DON-PROG) + Max MP +3 from each element's DLC boss // (KH3 hands out +160 / +59: the Dragonkin Soldier's MP step stands where its Thunder was)
        // the Great Rune rule in the table's order: item, accessory, armor, item, accessory, armor, item (+ the DLC's two item slots)
        assert_eq!(sora.status().item_slots - slots, 5);
        // (XSI) Focus Syphon: Regal Ancestor and Malenia (KH3's two victory-bonus copies); Mohg's became Riskcharge (XS 4.2)
        assert_eq!((sora.status().abilities.owned(A::Glide), sora.status().abilities.owned(A::Superslide), sora.status().abilities.owned(A::Airslide), sora.status().abilities.owned(A::FocusAspir)), (1, 3, 3, 2));
        // (XSI) every XS ability is given by its boss, once
        for kind in [A::SpiralRush, A::SonicDash, A::ShotlockSpectralRays, A::BreakBoost, A::Overcome, A::LightWave, A::GiantStamp, A::CriticalCounter, A::AncientLight, A::RageRaid, A::Riskcharge, A::Chandelier, A::Zantetsuken, A::EventHorizon, A::Myriad] {
            assert_eq!(sora.status().abilities.owned(kind), 1, "{kind:?}");
        }
    }

    /// (XSI, 2026-10-16) THE XS BOSS ROWS (XS_SORA_ABILITIES.md 4.2) as rows of their own (`xs.<boss>`), and "ALREADY HAVE IT:
    /// NOTHING EXTRA" (rule.skip_if_owned, question 2): Spiral Rush from the Tree Sentinel when level A 12 already gave it is
    /// nothing, and the panel leaves it out; a path that has not got it gets it.
    #[test]
    fn xsi_a_boss_gives_nothing_for_an_xs_ability_he_already_has() {
        let table = Table::parse(SHEET);
        assert!(table.problems.is_empty(), "{:?}", table.problems);
        assert_eq!(table.skip_if_owned.len(), 16);
        assert_eq!(table.rows.iter().filter(|r| !r.xs.is_empty()).count(), 15);
        let rewards = |id: &str| table.row(id).unwrap().rewards.clone();
        assert_eq!(rewards("tree_sentinel"), vec![Token::MaxHp(10), Token::Ability(A::SpiralRush)]);
        assert_eq!(rewards("mohg_lord"), vec![Token::MaxHp(10), Token::GreatRuneSlot, Token::Ability(A::Riskcharge)]);
        assert_eq!(rewards("godskin_noble").last(), Some(&Token::Ability(A::CriticalCounter)));
        // owned from the level path: nothing, and no panel line for it (the HP step still comes)
        let mut sora = new_sora();
        sora.status_mut().grant_ability(A::SpiralRush, false);
        let mut records = Vec::new();
        let Claim::Given(claimed) = claim(&table, &mut records, &mut sora, "tree_sentinel", true) else { panic!() };
        assert_eq!((claimed.given[1].changed, claimed.given[1].line, sora.status().abilities.owned(A::SpiralRush)), (false, BonusLine::None, 1));
        assert!(claimed.given[1].text.contains("nothing"), "{}", claimed.given[1].text);
        assert!(claimed.given[0].changed, "the rest of the row is given");
        assert_eq!(records, vec!["boss:tree_sentinel".to_string(), "xs:tree_sentinel".to_string()]);
        // not owned yet: one copy
        let mut sora = new_sora();
        let mut records = Vec::new();
        let Claim::Given(claimed) = claim(&table, &mut records, &mut sora, "tree_sentinel", true) else { panic!() };
        assert_eq!((claimed.given[1].changed, sora.status().abilities.owned(A::SpiralRush)), (true, 1));
        // an ordinary KH3 ability keeps its rule (one more copy)
        assert!(!table.skip_if_owned.contains(&A::Riskdodge));
    }

    /// (XSI) THE SHEET AS THE DEPLOYED DLL READS IT: the boss rows are the ones it had (the XS rewards are rows of their own, which
    /// it logs as not understood): Mohg, Lord of Blood still gives Focus Syphon there. And THE XS CATCH-UP: a character rewarded for
    /// bosses before the XSI build gets their XS abilities once at load, with the panel; nothing a second time.
    #[test]
    fn xsi_the_live_sheet_keeps_its_rows_and_the_catch_up_gives_the_xs_rewards_once() {
        let json: serde_json::Value = serde_json::from_str(SHEET).unwrap();
        let mohg = json["rows"].as_array().unwrap().iter().find(|r| r[0] == "boss.mohg_lord").unwrap();
        assert_eq!(mohg[1]["proposal"], serde_json::json!(["ability:FOCUS_ASPIR", "hp:10", "slot:great_rune"]));
        // the old parser's rule (Token::parse of the boss rows): every token of every boss row is understood without the xs rows
        for row in json["rows"].as_array().unwrap().iter().filter(|r| r[0].as_str().is_some_and(|id| id.starts_with("boss."))) {
            for t in row[1]["proposal"].as_array().unwrap().iter().chain(row[1]["user"].as_array().unwrap()) {
                let k = t.as_str().unwrap();
                assert!(!k.contains("SPIRAL") && !k.contains("RISKCHARGE") && !k.contains("MYRIAD"), "{}: {k}", row[0]);
            }
        }
        // a character of the old build: Tree Sentinel, Mohg Lord and Margit recorded (Focus Syphon given)
        let table = Table::parse(SHEET);
        let mut sora = new_sora();
        let mut records = vec!["boss:tree_sentinel".to_string(), "boss:mohg_lord".to_string(), "boss:margit".to_string()];
        sora.status_mut().grant_ability(A::FocusAspir, false);
        let caught = xs_catch_up(&table, &mut records, &mut sora);
        assert_eq!(caught.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["tree_sentinel", "mohg_lord"]);
        assert_eq!((sora.status().abilities.owned(A::SpiralRush), sora.status().abilities.owned(A::Riskcharge), sora.status().abilities.owned(A::FocusAspir)), (1, 1, 1));
        assert!(caught.iter().all(|c| c.given.len() == 1 && c.given[0].changed), "only the XS rewards: {:?}", caught);
        assert!(records.contains(&"xs:tree_sentinel".to_string()) && records.contains(&"xs:mohg_lord".to_string()));
        assert_eq!(sora.status().max_hp(), new_sora().status().max_hp(), "the rows' HP steps are not given again");
        // once
        assert!(xs_catch_up(&table, &mut records, &mut sora).is_empty());
        assert_eq!(sora.status().abilities.owned(A::Riskcharge), 1);
    }

    #[test]
    fn a_reward_is_given_once_per_character() {
        let table = Table::parse(SHEET);
        let mut sora = new_sora();
        let mut records = Vec::new();
        let Claim::Given(claimed) = claim(&table, &mut records, &mut sora, "margit", true) else { panic!() };
        assert_eq!(claimed.text(), "Thunder, MP Safety");
        assert_eq!(records, vec!["boss:margit".to_string()]);
        assert_eq!((sora.status().magic_tier(Element::Thunder), sora.status().abilities.owned(A::MpSafety)), (1, 1));
        // usable at once: the command is in a magic slot (the command menu's Magic list and the camp menu read these)
        assert!(sora.command().magic_slots.contains(&Element::Thunder.command(1)), "{:?}", sora.command().magic_slots);
        // again (the flag is still set, the re-encounter, a second session): nothing
        assert_eq!(claim(&table, &mut records, &mut sora, "MARGIT", true), Claim::Recorded);
        assert_eq!((sora.status().magic_tier(Element::Thunder), sora.status().abilities.owned(A::MpSafety), records.len()), (1, 1, 1));
        assert_eq!(claim(&table, &mut records, &mut sora, "nobody", true), Claim::Unknown);
        // Godrick: Cure, Counter Slash and the first Great Rune slot (an item slot)
        let slots = sora.status().item_slots;
        let Claim::Given(claimed) = claim(&table, &mut records, &mut sora, "godrick", true) else { panic!() };
        assert_eq!(claimed.text(), "Cure, Counter Slash, Item slot");
        assert_eq!((sora.status().magic_tier(Element::Cure), sora.status().item_slots), (1, slots + 1));
        // the second Great Rune boss, whichever it is, gives the second slot of the list (an accessory slot)
        let Claim::Given(claimed) = claim(&table, &mut records, &mut sora, "radahn", false) else { panic!() };
        assert_eq!((claimed.given[0].text.as_str(), claimed.given.last().unwrap().line), ("Aero", BonusLine::AccessorySlot));
    }

    /// KH3 REPLACES TIERS, AND NO ORDER OF PLAY WASTES ONE: the -ra row before the first source gives the first tier.
    #[test]
    fn magic_is_one_tier_more_whatever_the_row_names() {
        let table = Table::parse(SHEET);
        let mut sora = new_sora();
        let mut records = Vec::new();
        // the Draconic Tree Sentinel (the row names Thundara) before Margit: Thunder
        let Claim::Given(first) = claim(&table, &mut records, &mut sora, "draconic_tree_sentinel", false) else { panic!() };
        assert_eq!((first.given[0].token, first.given[0].text.as_str()), (Token::Magic { element: Element::Thunder, tier: 2 }, "Thunder"));
        // then Margit (the row names Thunder): Thundara takes Thunder's place in the same slot
        let slot = sora.command().magic_slots.iter().position(|c| *c == Element::Thunder.command(1)).expect("Thunder has a slot");
        let Claim::Given(second) = claim(&table, &mut records, &mut sora, "margit", false) else { panic!() };
        assert_eq!(second.given[0].text, "Thundara");
        assert_eq!(sora.status().magic_tier(Element::Thunder), 2);
        assert_eq!(sora.status().magic.iter().filter(|m| m.name == "thunder").count(), 1);
        assert_eq!(sora.command().magic_slots[slot], Element::Thunder.command(2), "{:?}", sora.command().magic_slots);
        // Fire: he starts with tier 1, so Rykard's Fira is Fira
        let Claim::Given(fire) = claim(&table, &mut records, &mut sora, "rykard", false) else { panic!() };
        assert_eq!(fire.given[0].text, "Fira");
        // (DON-PROG) the -ga already his: Max MP +3 instead (DONALD_OVERHAUL.md 9; before, a reward that changed nothing)
        sora.grant_quiet(Reward::Magic { element: Element::Thunder, tier: 3 });
        let mp = sora.status().max_mp();
        let Claim::Given(third) = claim(&table, &mut records, &mut sora, "fortissax", false) else { panic!() };
        assert!(third.given[0].changed && third.given[0].line == BonusLine::MaxMp(SORA_TOP_MP) && third.given[0].text == "Maximum MP +3", "{:?}", third.given[0]);
        assert_eq!((sora.status().max_mp() - mp, sora.status().magic_tier(Element::Thunder)), (SORA_TOP_MP, 3));
        assert!(recorded(&records, "fortissax"));
    }

    #[test]
    fn the_catch_up_gives_what_is_due_once_with_one_panel() {
        let table = Table::parse(SHEET);
        let mut sora = new_sora();
        let mut records = vec!["boss:tree_sentinel".to_string()];
        // an existing character: the Tree Sentinel's reward was had; Margit, Godrick, Rennala and an unknown row are due
        let due = vec!["rennala".to_string(), "tree_sentinel".to_string(), "godrick".to_string(), "margit".to_string(), "no_such_row".to_string()];
        let hp = sora.status().max_hp();
        let claimed = claim_all(&table, &mut records, &mut sora, &due);
        assert_eq!(claimed.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), vec!["margit", "godrick", "rennala"]);
        assert_eq!(sora.status().max_hp(), hp, "the Tree Sentinel's HP step is not given again");
        assert_eq!((sora.status().magic_tier(Element::Thunder), sora.status().magic_tier(Element::Cure), sora.status().magic_tier(Element::Blizzard)), (1, 1, 1));
        assert_eq!(summary_lines(&claimed), ["Rewards of 3 bosses obtained".to_string(), "Thunder, Cure, Blizzard +5 more".to_string()]);
        // a second start: nothing is due any more
        assert!(claim_all(&table, &mut records, &mut sora, &due).is_empty());
        assert_eq!(records.len(), 4);
        // one boss with two rewards: the lines themselves
        let one = claim_all(&table, &mut records, &mut sora, &["red_wolf".to_string()]);
        assert_eq!(one[0].text(), "Aerial Recovery, Maximum MP +5");
    }

    /// (GF-STATS) THE FRIENDS' ROWS: raw steps go apart from Donald's magic, a row may have only them, Goofy's rows and the two
    /// groups read, and every token that is not understood is a problem of the sheet.
    #[test]
    fn gf_the_friends_rows_parse_and_bad_tokens_are_problems() {
        let table = Table::parse(SHEET);
        assert!(table.problems.is_empty(), "{:?}", table.problems);
        let makar = table.don_of("makar").unwrap();
        assert_eq!((makar.steps.as_slice(), makar.growth.as_slice()), (["line2:fire".to_string()].as_slice(), ["hp:9".to_string(), "slot:item".to_string()].as_slice()));
        let messmer = table.don_of("messmer").unwrap();
        assert!(messmer.steps.is_empty() && !messmer.growth.is_empty(), "no magic on that boss, only raw steps");
        assert_eq!(table.gof.len(), 54, "the doc's rows less the two with only an ability, plus the two groups");
        let last: Vec<&str> = table.gof.iter().rev().take(2).map(|g| g.boss.as_str()).collect();
        assert_eq!(last, ["erdtree_avatars", "crucible_knights"], "the groups come last");
        assert_eq!(table.gof.iter().filter(|g| !g.group.is_empty()).map(|g| g.group.len()).sum::<usize>(), 7, "the seven flags");
        for (row, want) in [(r#"["don.margit", {"steps": ["hp:0"]}]"#, "not understood"), (r#"["gof.margit", {"steps": ["tech:nope"]}]"#, "not understood"), (r#"["gof.margit", {"steps": []}]"#, "not understood"), (r#"["gof.nobody", {"steps": ["hp:1"]}]"#, "no boss row"), (r#"["gof.g", {"steps": ["hp:1"], "group": ["nobody"]}]"#, "not a reserved boss row")] {
            let t = Table::parse(&format!("{{\"rows\": [{row}]}}"));
            assert!(t.problems.iter().any(|p| p.contains(want)), "{row}: {:?}", t.problems);
        }
        assert!(stat_token_ok("slot:armour") && stat_token_ok("mp:3") && !stat_token_ok("tech:deflect"));
        assert!(gof_token_ok("art:shield_toss") && gof_token_ok("hp:14") && !gof_token_ok("art:deflect"));
    }

    /// (DON-PROG) THE DONALD OVERHAUL'S SHARED MAGIC BOSSES (DONALD_OVERHAUL.md 9): the `don.*` rows name the doc's bosses
    /// (the italic ones added); Sora takes his next tier from ANY of an element's bosses, in any order, and Max MP +3 from
    /// the one after his -ga; a Donald-only boss (the Arcane lines' new rows) gives Sora nothing.
    #[test]
    fn don_prog_the_shared_magic_bosses_in_any_order() {
        let table = Table::parse(SHEET);
        assert!(table.problems.is_empty(), "{:?}", table.problems);
        let of = |step: &str| -> Vec<&str> { table.don.iter().filter(|d| d.steps.iter().any(|s| s == step)).map(|d| d.boss.as_str()).collect() };
        // (the user, 2026-10-22: "Donald's ZA magic should be unlocked in Elden Ring's DLC") Sora's base bosses up to -ga, one DLC boss each
        assert_eq!(of("magic:fire"), ["rykard", "fire_giant"]);
        assert_eq!(of("magic:water"), ["ancestor_spirit", "valiant_gargoyles"]);
        assert_eq!(of("magic:thunder"), ["margit", "fortissax", "draconic_tree_sentinel"]);
        assert_eq!(of("magic:light"), ["godrick", "godfrey_shade", "maliketh"]);
        assert_eq!(of("magic:blizzard"), ["loretta_caria", "rennala", "niall"]);
        assert_eq!(of("magic:aero"), ["radahn", "godskin_duo", "placidusax"]);
        assert_eq!(["fire", "water", "thunder", "light", "blizzard", "aero"].map(|e| of(&format!("dlc:{e}"))), [["midra"], ["putrescent"], ["bayle"], ["scadutree_avatar"], ["rellana"], ["dancing_lion"]]);
        assert_eq!(of("banish"), ["tree_sentinel"]);
        // (DONLINE2, the user 2026-10-27: "Tier 2 magic should be given by different bosses, seperate from tier 1 bosses")
        // each element's line 2 has three base-game bosses of its own (Light: the Tree Sentinel's banish + two), none of
        // them a boss of another don.* row, every one a boss row the mod can tell is beaten (a defeat flag or a chr id)
        let line2: Vec<Vec<&str>> = ["fire", "water", "thunder", "blizzard", "aero", "light"].iter().map(|e| of(&format!("line2:{e}"))).collect();
        assert_eq!(line2, [vec!["agheel", "makar", "mohg_lord"], vec!["leonine", "regal_ancestor", "mohg_omen"], vec!["dragonkin_nokstella", "elemer", "hoarah_loux"], vec!["red_wolf", "mimic_tear", "loretta_haligtree"], vec!["godskin_noble", "malenia", "gideon"], vec!["morgott", "golden_hippo"]]);
        let line2_bosses: Vec<&str> = line2.iter().flatten().copied().chain(of("banish")).collect();
        for id in &line2_bosses {
            let d = table.don_of(id).unwrap();
            assert_eq!(d.steps.len(), 1, "{id}: one line-2 step only");
            assert!(table.don.iter().filter(|o| o.boss == *id).count() == 1, "{id}: one don row");
            let row = table.row(id).unwrap();
            assert!(row.flag.is_some() || row.chr.is_some(), "{id}: detectable");
            assert!(!row.region.contains("Shadow of the Erdtree") || *id == "golden_hippo", "{id}: a base-game boss (2026-11-01: Banish Lv 3 moved to the Golden Hippopotamus, the user)");
        }
        let others: Vec<&str> = table.don.iter().filter(|d| !d.steps.iter().any(|s| s.starts_with("line2:") || s == "banish")).map(|d| d.boss.as_str()).collect();
        assert!(line2_bosses.iter().all(|b| !others.contains(b)), "the line-2 bosses are not the tier / Arcane / DLC bosses");
        assert_eq!(line2_bosses.iter().collect::<std::collections::HashSet<_>>().len(), 18);
        assert!(don_token_ok("line2:fire") && don_token_ok("l2:cure") && !don_token_ok("line2:dark"));
        assert_eq!((of("arcane:comet"), of("arcane:gravity"), of("arcane:meteor"), of("arcane:ultima"), of("arcane:zettaflare")), (vec!["smarag", "fallingstar_beast"], vec!["onyx_lord", "onyx_lord_liurnia"], vec!["fullgrown_fallingstar", "astel_stars", "adula"], vec!["astel"], vec!["elden_beast"]));
        assert!(don_token_ok("magic:cure") && don_token_ok("dlc:fire") && don_token_ok("BANISH") && !don_token_ok("magic:dark") && !don_token_ok("arcane:holy"));
        // every element's bosses, forward and backward: one tier at a time, then Max MP +3
        let element_of = |e: &str| Element::ALL.into_iter().find(|x| x.name() == if e == "light" { "cure" } else { e }).unwrap();
        for e in ["fire", "water", "thunder", "light", "blizzard", "aero"] {
            let el = element_of(e);
            let mut list = of(&format!("magic:{e}"));
            list.extend(of(&format!("dlc:{e}"))); // the DLC boss gives Sora his share too
            for pass in 0..2 {
                if pass == 1 {
                    list.reverse();
                }
                let mut sora = new_sora();
                let mut records = Vec::new();
                let start = sora.status().magic_tier(el);
                let mp = sora.status().max_mp();
                let mut tiers = Vec::new();
                for id in &list {
                    let Claim::Given(c) = claim(&table, &mut records, &mut sora, id, false) else { panic!("{id}") };
                    let magic = c.given.iter().find(|g| matches!(g.token, Token::Magic { .. })).expect("Sora's magic token");
                    assert!(magic.changed, "{e} {id}: {}", magic.text);
                    tiers.push(sora.status().magic_tier(el));
                }
                let want: Vec<u32> = ((start + 1)..=3).chain([3]).collect();
                assert_eq!(tiers, want, "{e} pass {pass}");
                assert_eq!(sora.status().max_mp() - mp, SORA_TOP_MP + list.iter().map(|id| table.row(id).unwrap().rewards.iter().map(|t| if let Token::MaxMp(n) = t { *n } else { 0 }).sum::<i32>()).sum::<i32>(), "{e}: the top boss's Max MP +3");
            }
        }
        // a Donald-only boss: nothing for Sora (recorded all the same, for Donald's steps)
        for id in ["fallingstar_beast", "onyx_lord", "onyx_lord_liurnia", "fullgrown_fallingstar", "astel_stars", "adula"] {
            let mut sora = new_sora();
            let mut records = Vec::new();
            let before = (tiers(&sora), sora.status().max_mp(), sora.status().max_hp());
            let Claim::Given(c) = claim(&table, &mut records, &mut sora, id, true) else { panic!("{id}") };
            assert!(c.given.is_empty(), "{id}: {:?}", c.given);
            assert_eq!((tiers(&sora), sora.status().max_mp(), sora.status().max_hp()), before);
            assert!(recorded(&records, id));
        }
    }

    /// (DON-PROG) AN OLD SAVE'S SORA CATCH-UP: bosses beaten before the overhaul gave them Sora magic (the extra bosses'
    /// `sm.*` rows) give it once: his next tier, or Max MP +3 with the -ga already; recorded sora_magic:<row>; a
    /// boss rewarded now records it with the row (no catch-up twice); a second session nothing.
    #[test]
    fn don_prog_an_old_saves_sora_gets_the_extra_bosses_magic_once() {
        let table = Table::parse(SHEET);
        assert!(table.problems.is_empty(), "{:?}", table.problems);
        assert_eq!(table.rows.iter().filter(|r| !r.sm.is_empty()).map(|r| r.id.as_str()).collect::<Vec<_>>(), ["dancing_lion", "rellana", "putrescent", "scadutree_avatar", "midra", "bayle"]);
        // the old save: Fire at -ga already (Rykard, the Fire Giant), Midra, the Dancing Lion and the Scadutree Avatar beaten before
        let mut sora = new_sora();
        sora.grant_quiet(Reward::Magic { element: Element::Fire, tier: 3 });
        let mut records: Vec<String> = ["boss:rykard", "boss:fire_giant", "boss:midra", "boss:dancing_lion", "boss:scadutree_avatar", "xs:rellana"].map(String::from).to_vec();
        let old = records.clone();
        let (mp, aero, cure) = (sora.status().max_mp(), sora.status().magic_tier(Element::Aero), sora.status().magic_tier(Element::Cure));
        let got = sm_catch_up(&table, &mut records, &mut sora);
        assert_eq!(got.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["dancing_lion", "scadutree_avatar", "midra"]);
        assert!(got.iter().all(|c| c.given.len() == 1 && c.given[0].changed), "{got:?}");
        assert_eq!((sora.status().max_mp() - mp, sora.status().magic_tier(Element::Aero) - aero, sora.status().magic_tier(Element::Cure) - cure), (SORA_TOP_MP, 1, 1), "Midra: Max MP +3 (Firaga already); the Dancing Lion: Aero; the Scadutree Avatar: Cure");
        assert!(records.starts_with(&old), "only added to");
        assert_eq!(records.iter().filter(|r| r.starts_with(SM_RECORD_PREFIX)).count(), 3);
        // once
        assert!(sm_catch_up(&table, &mut records, &mut sora).is_empty());
        // a new character: the row gives it with the rest and records it; the catch-up has nothing
        let mut fresh = new_sora();
        let mut rec = Vec::new();
        let Claim::Given(c) = claim(&table, &mut rec, &mut fresh, "midra", false) else { panic!() };
        assert!(c.given.iter().any(|g| matches!(g.token, Token::Magic { element: Element::Fire, .. }) && g.changed));
        assert!(sm_recorded(&rec, "midra"));
        assert!(sm_catch_up(&table, &mut rec, &mut fresh).is_empty());
        // the deployed DLL's view: the boss rows themselves are as before the overhaul (the sm.* rows are separate)
        let json: Value = serde_json::from_str(SHEET).unwrap();
        let midra = json["rows"].as_array().unwrap().iter().find(|r| r[0] == "boss.midra").unwrap();
        assert_eq!(midra[1]["proposal"], serde_json::json!(["mp:3", "hp:10"])); // (2026-11-01) + the DLC's HP
        let makar = json["rows"].as_array().unwrap().iter().find(|r| r[0] == "boss.makar").unwrap();
        assert_eq!(makar[1]["proposal"], serde_json::json!(["ability:AIR_DOWN", "hp:10"]), "no longer a tier source: as before");
    }

    /// The record is in the save; a file written before the field existed (version 2) loads with no records.
    #[test]
    fn the_record_survives_the_save_and_an_old_file_has_none() {
        let table = Table::parse(SHEET);
        let base = base();
        let mut sora = new_sora();
        let mut inventory = Inventory::default();
        // an OLD FILE: version 2, no `records`
        let mut old: Value = serde_json::from_str(&SoraSave::capture(&sora, &inventory, "Sora", 5, 0).to_json()).unwrap();
        old["version"] = serde_json::json!(2);
        old.as_object_mut().unwrap().remove("records");
        let loaded = SoraSave::from_json(&old.to_string()).expect("a version-2 file is read");
        assert_eq!((loaded.version, loaded.records.len()), (crate::save::VERSION, 0));
        loaded.apply(&base, &mut sora, &mut inventory);
        assert!(inventory.records.is_empty());
        // the reward, the write, the read: the record and the spell are both there, and nothing is given twice
        assert!(matches!(claim(&table, &mut inventory.records, &mut sora, "margit", true), Claim::Given(_)));
        let text = SoraSave::capture(&sora, &inventory, "Sora", 6, 0).to_json();
        assert!(text.contains("\"boss:margit\""));
        let mut again = new_sora();
        let mut inventory_again = Inventory::default();
        SoraSave::from_json(&text).unwrap().apply(&base, &mut again, &mut inventory_again);
        assert_eq!(inventory_again.records, vec!["boss:margit".to_string()]);
        assert_eq!(again.status().magic_tier(Element::Thunder), 1);
        assert!(again.command().magic_slots.contains(&Element::Thunder.command(1)));
        assert_eq!(claim(&table, &mut inventory_again.records, &mut again, "margit", true), Claim::Recorded);
        assert_eq!(again.status().magic_tier(Element::Thunder), 1);
    }

    /// (P9b) THE TOKENS AND THE `kb.*` ROWS: `weapon:<KEY>` names a weapon of the table (a Keyblade, a staff, a shield);
    /// anything else is not a weapon token; the seven boss Keyblades of table 6b are rows of their own with their boss rows'
    /// verified defeat flags; the boss rows themselves carry no weapon (the deployed DLL reads them unchanged).
    #[test]
    fn p9b_weapon_tokens_and_the_seven_boss_keyblades() {
        assert_eq!(Token::parse("weapon:WEP_KEYBLADE_SO_01"), Some(Token::Weapon { item_id: 770 }));
        assert_eq!(Token::parse("weapon:WEP_DONALD_05").map(|t| t.label()), Some("Magician's Wand+".to_string()));
        assert_eq!(Token::parse("weapon:WEP_KEYBLADE_SO_01").unwrap().label(), "Hero's Origin");
        for bad in ["weapon:ITEM_POTION", "weapon:", "weapon:WEP_KEYBLADE_SO_99"] {
            assert_eq!(Token::parse(bad), None, "{bad}");
        }
        let table = Table::parse(SHEET);
        assert!(table.problems.is_empty(), "{:?}", table.problems);
        let kb: Vec<(&str, u32, Option<u32>)> = table.kb.iter().map(|k| (k.boss.as_str(), match k.tokens[..] { [Token::Weapon { item_id }] => item_id, _ => 0 }, k.flag)).collect();
        assert_eq!(kb, vec![("margit", 770, Some(10000850)), ("rennala", 771, Some(14000800)), ("valiant_gargoyles", 774, Some(12020800)), ("rykard", 777, Some(16000800)), ("niall", 775, Some(1051570800)), ("rellana", 781, Some(2048440800)), ("messmer", 782, Some(21010800))]);
        assert!(table.rows.iter().all(|r| !r.rewards.iter().any(|t| matches!(t, Token::Weapon { .. }))), "no boss row carries a weapon itself");
        for (row, want) in [(r#"["kb.nobody", {"add": ["weapon:WEP_KEYBLADE_SO_01"]}]"#, "no boss row"), (r#"["kb.margit", {"add": ["item:ITEM_POTION"]}]"#, "not a weapon")] {
            let t = Table::parse(&format!("{{\"rows\": [[\"boss.margit\", {{\"flag\": 10000850, \"flag_conf\": \"verified\", \"user\": [], \"proposal\": []}}], {row}]}}"));
            assert!(t.problems.iter().any(|p| p.contains(want)), "{row}: {:?}", t.problems);
        }
        // a boss row's own weapon token is given with the row (the token's path through the reward seam)
        crate::weapons::begin_new_game();
        let mut sora = new_sora();
        let t = Table::parse(r#"{"rows": [["boss.x", {"flag": 1, "flag_conf": "verified", "user": ["weapon:WEP_KEYBLADE_SO_02"], "proposal": []}]]}"#);
        let Claim::Given(c) = claim(&t, &mut Vec::new(), &mut sora, "x", true) else { panic!() };
        assert!(c.given[0].changed && c.given[0].line == BonusLine::None && crate::weapons::owns("WEP_KEYBLADE_SO_02"), "{c:?}");
    }

    /// (P9b) THE CATCH-UP WITH MOCKED FLAGS: an older save (rewarded for Margit before the Keyblades existed: `boss:margit`,
    /// no `kb:` record) with Margit's and Rennala's defeat flags set gets Hero's Origin and Shooting Star once; a second
    /// load nothing; a flag not readable gives nothing; a save past Morgott that skipped Margit gets Hero's Origin (Morgott's
    /// event sets Margit's flag: P9a).
    #[test]
    fn p9b_the_catch_up_gives_each_boss_keyblade_once() {
        crate::weapons::begin_new_game();
        let table = Table::parse(SHEET);
        let set = [10000850u32, 14000800, 11000800];
        let flags = |f: u32| Some(set.contains(&f));
        let mut records = vec!["boss:margit".to_string()];
        let due: Vec<&str> = kb_due(&table, &records, &flags).iter().map(|k| k.boss.as_str()).collect();
        assert_eq!(due, ["margit", "rennala"]);
        let mut given = Vec::new();
        for row in kb_due(&table, &records, &flags).into_iter().cloned().collect::<Vec<_>>() {
            given.extend(claim_kb_with(&mut records, &row, &mut give_one).unwrap());
        }
        assert_eq!(given.iter().map(|g| (g.name.as_str(), g.given())).collect::<Vec<_>>(), [("Hero's Origin", true), ("Shooting Star", true)]);
        assert!(crate::weapons::owns("WEP_KEYBLADE_SO_01") && crate::weapons::owns("WEP_KEYBLADE_SO_02"));
        assert_eq!(records, ["boss:margit", "kb:margit", "kb:rennala"]);
        assert!(kb_due(&table, &records, &flags).is_empty(), "the next load: nothing");
        assert!(claim_kb_with(&mut records, &table.kb[0], &mut give_one).is_none(), "recorded: nothing");
        assert!(kb_due(&table, &[], &|_| None).is_empty(), "flags not readable: nothing due");
        // Morgott beaten, Margit skipped: his event sets 10000850 (P9a) -> Hero's Origin
        let morgott = |f: u32| Some(f == 10000850 || f == 11000800);
        assert_eq!(kb_due(&table, &[], &morgott).iter().map(|k| k.boss.as_str()).collect::<Vec<_>>(), ["margit"]);
    }

    /// (P9b) AN OWNED KEYBLADE IS REFUSED, ITS SOURCE MARKED: Hero's Origin already owned (synthesis or another source):
    /// Margit's Keyblade is refused by the weapons' door ("already owned: no duplicate"), the row is recorded all the same
    /// and never tried again; the same for a chest.
    #[test]
    fn p9b_an_owned_keyblade_is_refused_and_its_source_still_marked() {
        crate::weapons::begin_new_game();
        assert_eq!(crate::weapons::give_key("WEP_KEYBLADE_SO_01", 1), Gift::Given(1));
        let table = Table::parse(SHEET);
        let mut records = Vec::new();
        let margit = table.kb.iter().find(|k| k.boss == "margit").unwrap();
        let got = claim_kb_with(&mut records, margit, &mut give_one).unwrap();
        assert_eq!(got[0].gift, Gift::AlreadyOwned);
        assert!(got[0].text().contains("already owned: no duplicate"), "{}", got[0].text());
        assert_eq!(records, ["kb:margit"]);
        assert!(kb_due(&table, &records, &|f| Some(f == 10000850)).is_empty());
        // a chest: Starlight owned already
        let src = WeaponSources::parse(WEAPON_SOURCES_SHEET);
        assert!(src.problems.is_empty(), "{:?}", src.problems);
        let starlight = src.chests.iter().find(|c| c.key == "WEP_KEYBLADE_SO_17").unwrap();
        assert_eq!(crate::weapons::give_key("WEP_KEYBLADE_SO_17", 1), Gift::Given(1));
        let g = claim_chest_with(&mut records, starlight, &mut give_one).unwrap();
        assert_eq!((g.gift, wsrc_recorded(&records, starlight.flag)), (Gift::AlreadyOwned, true));
        assert!(claim_chest_with(&mut records, starlight, &mut give_one).is_none());
    }

    /// (P9b) THE CHESTS: P9a's 14 verified chest / pickup rows (8 Keyblades of table 6b, 6 friends' weapons); a chest whose
    /// lot flag is set gives its weapon once (record wsrc:<flag>), the catch-up the same way; a friend's weapon from a chest
    /// is a copy (duplicates allowed) but the chest itself gives once.
    #[test]
    fn p9b_chests_give_their_weapon_once_by_their_lot_flag() {
        crate::weapons::begin_new_game();
        let src = WeaponSources::parse(WEAPON_SOURCES_SHEET);
        assert_eq!((src.chests.len(), src.chests.iter().filter(|c| c.verified).count()), (14, 14));
        assert_eq!(src.chests.iter().filter(|c| crate::weapons::is_keyblade(&c.key)).count(), 8);
        // (STARLIGHT-SPOT) Starlight's chest is the Demi-Human Forest Ruins cellar chest (lot 1043340100's flag); the
        // Gatefront Ruins carriage chest (flag 1042377070) is no source any more: it gives ER's Lordsworn's Greatsword alone
        let starlight = 1043347100u32;
        let gatefront = 1042377070u32;
        assert!(chests_due(&src, &[], &|f| Some(f == gatefront)).is_empty(), "Gatefront: ER's own item only");
        let mut ids: Vec<&str> = src.chests.iter().map(|c| c.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), src.chests.len(), "no chest id twice");
        let mut records = Vec::new();
        let due: Vec<&str> = chests_due(&src, &records, &|f| Some(f == starlight || f == 10007965)).iter().map(|c| c.key.as_str()).collect();
        assert_eq!(due, ["WEP_KEYBLADE_SO_17", "WEP_DONALD_01"]);
        for c in chests_due(&src, &records.clone(), &|f| Some(f == starlight || f == 10007965)) {
            assert!(claim_chest_with(&mut records, c, &mut give_one).unwrap().given());
        }
        assert_eq!(records, [format!("wsrc:{starlight}"), "wsrc:10007965".to_string()]);
        assert!(chests_due(&src, &records, &|_| Some(true)).iter().all(|c| c.flag != starlight && c.flag != 10007965));
        assert!(crate::weapons::owns("WEP_KEYBLADE_SO_17"));
    }

    /// (P9b) THE RARE DROPS: 9 verified rows; a sorcerer's row hits below 2 %; a Keyblade drop row (none in the sheet: the
    /// rule) is not rolled while it is owned; a friend's weapon always is (duplicates allowed).
    #[test]
    fn p9b_rare_drops_roll_and_a_keyblade_never_twice() {
        let src = WeaponSources::parse(WEAPON_SOURCES_SHEET);
        assert_eq!((src.drops.len(), src.drops.iter().filter(|d| d.verified).count()), (9, 9));
        assert_eq!(drop_roll(&src, 37020020, &mut || 0.019, &|_| true).iter().map(|d| d.key.as_str()).collect::<Vec<_>>(), ["WEP_DONALD_01"], "a friend's weapon: owned or not");
        assert!(drop_roll(&src, 37020020, &mut || 0.02, &|_| false).is_empty(), "2 %: not at 0.02");
        assert!(drop_roll(&src, 37029999, &mut || 0.0, &|_| false).is_empty(), "a row the maps do not place (a Spirit Ash's)");
        let kb = WeaponSources::parse(r#"{"rows": [["drop.WEP_KEYBLADE_SO_05.1", {"item": "WEP_KEYBLADE_SO_05", "chr": "c1000", "npc_params": [], "chance": 0.5, "verified": true}]]}"#);
        assert_eq!(drop_roll(&kb, 10000010, &mut || 0.0, &|_| false).len(), 1, "by the model when the row names no NpcParam rows");
        assert!(drop_roll(&kb, 10000010, &mut || 0.0, &|k| k == "WEP_KEYBLADE_SO_05").is_empty(), "an owned Keyblade: no roll");
    }

    /// THE SMALL-BOSS RULE: the tier is capped by the region and rises with the boss's HP; the boosts are every third
    /// boss in turn; once per boss; the same boss gives the same class of piece.
    #[test]
    fn small_bosses_give_one_capped_reward_once() {
        let table = Table::parse(SHEET);
        let rules = table.small.clone();
        assert_eq!(rules, SmallRules::default(), "the sheet's rows are the defaults");
        // the cap per region: Limgrave 2, Liurnia 3, Caelid / Altus / underground 4, Mountaintops / Farum / Haligtree 5, the DLC 6
        assert_eq!((0..=4).map(|d| rules.tier(d, 1_000_000)).collect::<Vec<_>>(), vec![2, 3, 4, 5, 6]);
        // within the cap by HP: a Limgrave cave boss 1, a Limgrave field boss 2; the same 5000-HP body is 2 in Limgrave, 3 from Liurnia on
        assert_eq!((rules.tier(0, 800), rules.tier(0, 2889), rules.tier(0, 5000), rules.tier(1, 5000), rules.tier(3, 5000), rules.tier(3, 20000), rules.tier(4, 40000)), (1, 2, 2, 3, 3, 5, 6));
        // thirty bosses: boosts at 3, 6, 9 ... in the cycle; never two in three; no stat more than one ahead
        let mut sora = new_sora();
        let mut records = vec!["boss:margit".to_string()];
        let stats = |s: &Sora| (s.status().boosts.strength, s.status().boosts.magic, s.status().boosts.defense, s.status().boosts.ap);
        assert_eq!(stats(&sora), (0, 0, 0, 0));
        let ap = sora.status().max_ap();
        let mut got = Vec::new();
        for n in 1..=30u32 {
            got.push(small_claim(&rules, &mut records, &mut sora, &format!("2130{n:04}@a00000{n:02}"), 0, 900).expect("a new boss"));
        }
        let boosts: Vec<(usize, Stat)> = got.iter().enumerate().filter_map(|(k, r)| match r { SmallReward::Boost(s) => Some((k + 1, *s)), _ => None }).collect();
        assert_eq!(boosts.iter().map(|b| b.0).collect::<Vec<_>>(), vec![3, 6, 9, 12, 15, 18, 21, 24, 27, 30]);
        assert_eq!(boosts.iter().take(5).map(|b| b.1).collect::<Vec<_>>(), vec![Stat::Strength, Stat::Magic, Stat::Defense, Stat::Ap, Stat::Strength]);
        assert_eq!((stats(&sora), sora.status().max_ap() - ap), ((3, 3, 2, 2), 2));
        assert!(got.iter().all(|r| !matches!(r, SmallReward::Piece { tier, .. } if *tier != 1)), "Limgrave cave bosses: tier 1 pieces");
        assert!(got.iter().any(|r| matches!(r, SmallReward::Piece { armour: true, .. })) && got.iter().any(|r| matches!(r, SmallReward::Piece { armour: false, .. })));
        // once per boss: the same key again gives nothing and counts nothing; the named records are not counted
        assert_eq!(small_claim(&rules, &mut records, &mut sora, "21300001@a0000001", 0, 900), None);
        assert_eq!((small_count(&records), records.len()), (30, 31));
        // the same boss gives the same thing for another character at the same count
        assert_eq!(rules.reward("x@1", 2, 7000, 5), SmallReward::Piece { armour: (key_seed("x@1") >> 4) % 2 == 1, tier: 3 });
        assert_eq!(rules.reward("x@1", 2, 7000, 5), rules.reward("x@1", 2, 7000, 7));
        // (IMPL_ER61) every second boss without a boost gives a recovery item: pieces only at 1, 5, 7, 11 ... (10 of 30, was 20)
        let items: Vec<usize> = got.iter().enumerate().filter_map(|(k, r)| matches!(r, SmallReward::Item { .. }).then_some(k + 1)).collect();
        assert_eq!(items, vec![2, 4, 8, 10, 14, 16, 20, 22, 26, 28]);
        assert_eq!(got.iter().filter(|r| matches!(r, SmallReward::Piece { .. })).count(), 10);
        assert_eq!((rules.reward("x@1", 2, 7000, 4), rules.item_key(1), rules.item_key(3), rules.item_key(9)), (SmallReward::Item { tier: 3 }, Some("ITEM_HIGHPOTION"), Some("ITEM_ELIXIR"), Some("ITEM_LASTELIXIR")));
        assert_eq!(SmallRules { item_every: 0, ..rules.clone() }.reward("x@1", 2, 7000, 4), rules.reward("x@1", 2, 7000, 5));
        // switched off: nothing, no record
        let off = SmallRules { on: false, ..rules.clone() };
        assert_eq!(small_claim(&off, &mut records, &mut sora, "new@1", 0, 900), None);
        assert_eq!(records.len(), 31);
        // the boosts survive the save (the boost bytes), and so does the count
        let text = SoraSave::capture(&sora, &Inventory { records: records.clone(), ..Inventory::default() }, "Sora", 1, 0).to_json();
        let mut again = new_sora();
        let mut inventory = Inventory::default();
        SoraSave::from_json(&text).unwrap().apply(&base(), &mut again, &mut inventory);
        assert_eq!((stats(&again), small_count(&inventory.records)), ((3, 3, 2, 2), 30));
    }
}
