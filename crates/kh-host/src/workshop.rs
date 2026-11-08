//! (P6a, documentation/KH3_WEAPONS_PLAN.md) THE MOOGLE WORKSHOP'S RULES, 1:1 WITH KH3, engine-free and without a screen
//! (the screens are P6b's: kh-ui `shop_moogle.rs` / `workshop/*`, kh-host `hud_workshop.rs`). The user (2026-11-02):
//! "Synthesis and upgrading needs to work and behave exactly like KH3 with every option and function from KH3".
//!
//! EVERY NUMBER IS A ROW of sheets/port/kh-workshop.json (written by tools/kh_workshop_sheet.js from the KH3 tables of
//! documentation/KH3_WEAPON_DATA.md; KH3's rules with their function addresses: sheets/port/kh3-workshop-rules.json):
//! - THE WORKSHOP STOCK (`workshop.stock_cap` 999 a material): each visit ([`visit`]) first moves every material Sora
//!   carries into it (dialog_0010). Synthesis counts materials ONLY there; the Forge too, and spends from it first, then
//!   from the bag.
//! - RECIPES (`syn.IS_n`, 88): listed always (IS_0), once their Collector's Goal is achieved (67) or once their Photo
//!   Mission is done (20); a newly listed one wears "NEW" until the screen marks it seen. A count is capped by the reward's
//!   maximum minus what is held and by the stock; it consumes count x each material and gives count x the reward; no munny;
//!   repeatable; a Keyblade only while none is held (Ultima Weapon); it counts per recipe (the "made once" bit).
//! - COLLECTOR'S GOALS (`goal.IC_n`, 78): tested on each visit in table order against the counts from BEFORE the visit's
//!   rewards (no chaining); granted once. Conditions: KIND_NUM (kinds ever obtained), TOTAL_NUM (all 60 ever obtained),
//!   GET_RANK (every material of a rank obtained), KIND_ALL (every material of an item-id range obtained), SPKIND_NUM (one
//!   material ever obtained n times). KH3 counts the last two as stock + used + carried: materials are never sold or lost,
//!   so that is what P1's record counts as ever obtained (`Workshop::obtained` / `total_obtained`). Rewards: GET_ITEM (one
//!   item; a material over the bag's maximum goes to the stock), ENABLE_SYNTHESIS (recipes listed), ENABLE_BUY (the
//!   Moogle sells the material).
//! - PHOTO MISSIONS AS ELDEN RING MILESTONES (`photo.IM_n`, the user 2026-11-03 "Unlock them another way"), shown as
//!   "JOURNEY TO THE ERDTREE" (the user 2026-11-07): done when the milestone's boss is defeated (IM_6, the finale: every
//!   shardbearer and the Elden Beast), tested on each visit as KH3 tests photos (retroactively: a boss beaten before counts);
//!   ALL 20 LISTED FROM THE START; COMPOSED texts in the sheet. A save checked against an older milestone list is brought
//!   over once ([`migrate_milestones`], `workshop.milestone_rev`).
//! - THE KEYBLADE FORGE (`forge.<KEY>`, 17 x 10): only the owned Keyblades; only the level after the current one; costs
//!   materials only (no munny at any level); stats = base + every done level, each level's ability added.
//! - THE MOOGLE SHOP: its level (`moogle.level.<1..7>`, plan 6c: 1 start, 2 Godrick, 3 / 4 the second / third Great Rune
//!   boss the player defeats, 5 Morgott, 6 Maliketh, 7 Godfrey / Hoarah Loux) is the highest step reached, never lowered.
//!   An item is stocked when not sold out and (1 <= its level <= the shop level, or its ENABLE_BUY goal is achieved, or it
//!   needs the M.O.G. Card and the card is held). A stocked item never seen is a "new arrival".
//! - THE MATERIAL LIST names a material never obtained "? ? ?"; THE SYNTH LOG counts recipes made / 88, missions / 20,
//!   goals / 78.
//!
//! THE STATE is P1's record (`crate::weapons::Workshop`, kept by the save v6); its `ext` object holds what the record has
//! no field for (`photos_seen`, `shop_seen`). WHAT THE RULES TOUCH OUTSIDE IT goes through [`WorkshopHost`] (Sora's bag,
//! the item door, the Keyblade levels), the host game's event flags through a plain `Fn(u32) -> bool`. The rules count
//! every material they hand over themselves (into the bag or the stock); [`with_live`] runs them on the running record
//! with the real bag and door, handing the items over once the record is released (no material counted twice).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::{json, Value};

use crate::weapons::{OwnedKeyblade, Workshop};

/// The compiled copy of the sheet.
pub const SHEET: &str = include_str!("../../../sheets/port/kh-workshop.json");

/// One of KH3's 60 materials (`mat.<MAT_ITEMnn>`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Material {
    pub key: String,
    /// 0x701 .. 0x73C.
    pub item_id: u32,
    /// `m_ItemRank`: 0 = RANK00 (the rarest: crystals, ores) .. 3 = RANK03 (shards); what GET_RANK tests.
    pub rank: u8,
    pub grade: u8,
    pub name: String,
    /// `m_MaxNum`: the bag's maximum (the stock's is `workshop.stock_cap`).
    pub max: u32,
}

/// What lists a recipe (`m_Condition`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Unlock {
    /// TISC_NONE.
    #[default]
    Always,
    /// TISC_COLLECT_LIST: the Collector's Goal IC_n.
    Goal(String),
    /// TISC_MOTIF_LIST: the Photo Mission IM_n (here an Elden Ring milestone).
    Photo(String),
}

/// One recipe (`syn.IS_n`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recipe {
    pub id: String,
    /// `m_FlagIndex`.
    pub flag: u32,
    /// The item made, by key (PRT_ITEM11, WEP_DONALD_03, WEP_KEYBLADE_SO_14 ...).
    pub reward: String,
    pub name: String,
    /// battle | camp | armour | accessory | material | weapon.
    pub kind: String,
    pub item_id: u32,
    /// The reward's `m_MaxNum`.
    pub max: u32,
    pub unlock: Unlock,
    /// (material key, count) for one item.
    pub materials: Vec<(String, u32)>,
}

/// A Collector's Goal's condition (`m_Condition`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GoalCondition {
    /// TICC_MAT_KIND_NUM: n kinds ever obtained.
    KindNum(u32),
    /// TICC_MAT_TOTAL_NUM: stock + used + carried over all 60 >= n.
    TotalNum(u32),
    /// TICC_MAT_GET_RANK: every material of this rank ever obtained.
    GetRank(u8),
    /// TICC_MAT_KIND_ALL: every material with an item id in from..=to ever obtained.
    KindAll { from: u32, to: u32 },
    /// TICC_MAT_SPKIND_NUM: this material's stock + used + carried >= n.
    SpKindNum { material: String, n: u32 },
}

/// A Collector's Goal's reward (`m_RewardKind`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GoalReward {
    /// TICR_GET_ITEM: one item, once.
    GetItem(String),
    /// TICR_ENABLE_SYNTHESIS: these recipes are listed.
    EnableSynthesis(Vec<String>),
    /// TICR_ENABLE_BUY: the Moogle sells these.
    EnableBuy(Vec<String>),
}

/// One Collector's Goal (`goal.IC_n`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Goal {
    pub id: String,
    /// `m_FlagIndex`: also the unlock bit an ENABLE_BUY stock row names.
    pub flag: u32,
    pub condition: GoalCondition,
    pub reward: GoalReward,
    /// The list text's string id (kh3_menu.synthesis.collection_00n0).
    pub text: String,
}

/// What completes a milestone mission.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Milestone {
    /// None (a row without one: never done).
    #[default]
    Never,
    /// This defeat flag.
    Flag(u32),
    /// Every one of these defeat flags (IM_6: the seven Great Rune holders).
    All(Vec<u32>),
}

/// One Photo Mission as an Elden Ring milestone (`photo.IM_n`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Photo {
    pub id: String,
    pub flag: u32,
    /// The recipes it lists (one each).
    pub recipes: Vec<String>,
    /// KH3's `m_UIPriority` (the list's order).
    pub order: u32,
    pub done: Milestone,
    /// The milestone revision that last changed this row's subject (`workshop.milestone_rev`; 1 = the first list).
    pub rev: u32,
    /// COMPOSED texts: the Moogle's line and the detail ("{0} / {1}" = defeated / needed for IM_6).
    pub name: String,
    pub help: String,
    /// (KH3TEXT, THE USER 2026-11-08: "keep radhan's milestone text what it was") the Moogle's line is KH3's own, read from
    /// the player's pack at run time (never compiled in); `name` is empty then.
    pub name_from: Option<PackText>,
}

/// A text the sheet takes from the player's UI pack by its KH3 id (`{ "pack": id, "period_to": "!" }`), with KH3's final
/// "." changed to `period_to` (empty: kept). KH3's text is never compiled into the DLL.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackText {
    pub id: String,
    pub period_to: String,
}

impl PackText {
    fn parse(v: &Value) -> Option<PackText> {
        let id = v["pack"].as_str().filter(|s| !s.is_empty())?;
        Some(PackText { id: id.to_string(), period_to: text(&v["period_to"]) })
    }

    /// The pack's text through `raw` (the Workshop's lookup: the pack's text, or the id itself when the pack lacks it);
    /// nothing when the pack lacks it (never a compiled copy).
    pub fn resolve(&self, raw: &dyn Fn(&str) -> String) -> String {
        let line = raw(&self.id);
        if line.is_empty() || line == self.id {
            return String::new();
        }
        match line.strip_suffix('.') {
            Some(body) if !self.period_to.is_empty() => format!("{body}{}", self.period_to),
            _ => line,
        }
    }
}

/// One Keyblade Forge level (`forge.<KEY>` levels).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ForgeLevel {
    pub level: u8,
    /// `m_AttackPlus` / `m_MagicPlus`.
    pub strength: i32,
    pub magic: i32,
    /// `m_AppendAbility` (game name), if any.
    pub ability: Option<String>,
    pub materials: Vec<(String, u32)>,
    /// `m_bInitAchieved`: done when the Keyblade is obtained.
    pub init: bool,
}

/// One Sora Keyblade of the Forge (`forge.<WEP_KEYBLADE_SO_nn>`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keyblade {
    pub key: String,
    pub item_id: u32,
    pub name: String,
    /// The base weapon's Strength / Magic (+0).
    pub strength: i32,
    pub magic: i32,
    pub abilities: Vec<String>,
    pub max: u8,
    /// The level it has when obtained (the highest `m_bInitAchieved` row).
    pub init: u8,
    /// Levels 1..=max in order.
    pub levels: Vec<ForgeLevel>,
}

/// What raises the Moogle shop to a level (`moogle.level.<n>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShopStep {
    Start,
    /// A defeat flag.
    Flag(u32),
    /// This many of the Great Rune holders (`moogle.great_rune_bosses`) defeated.
    GreatRunes(u32),
}

/// One row of the Moogle's stock (`moogle.stock`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StockRow {
    pub key: String,
    pub item_id: u32,
    pub kind: String,
    /// `m_BuyPrice` (KH3's munny; the screens apply the mod's factor).
    pub buy: u32,
    /// `m_ShopLevel` (0 = never by level).
    pub level: u8,
    /// `m_bNeedMogCard`.
    pub mog_card: bool,
    /// `m_UnlockFlagIndex`: the flag of the ENABLE_BUY goal that unlocks it.
    pub unlock_flag: Option<u32>,
    /// `m_bCanbeSoldOut`.
    pub sold_out: bool,
    pub max: u32,
}

/// One entry of the Workshop's menu (`workshop.menu`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuEntry {
    /// synthesize | forge | photo | goals | materials.
    pub entry: String,
    /// Its string id (kh3_menu.synthesis.menu_00n0).
    pub text: String,
}

/// The sheet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tables {
    pub stock_cap: u32,
    pub count_cap: u32,
    /// The Workshop's materials and the Sora Keyblades, as item-id ranges.
    pub material_ids: (u32, u32),
    pub keyblade_ids: (u32, u32),
    pub unknown_name: String,
    pub menu: Vec<MenuEntry>,
    pub materials: Vec<Material>,
    pub recipes: Vec<Recipe>,
    pub goals: Vec<Goal>,
    pub photos: Vec<Photo>,
    /// The milestone list's revision (`workshop.milestone_rev`; 1 when the sheet has none).
    pub milestone_rev: u32,
    /// (level, step), in level order.
    pub shop_levels: Vec<(u8, ShopStep)>,
    pub great_rune_flags: Vec<u32>,
    pub stock: Vec<StockRow>,
    pub keyblades: Vec<Keyblade>,
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn number(v: &Value) -> u32 {
    v.as_u64().unwrap_or(0) as u32
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn numbers(v: &Value) -> Vec<u32> {
    v.as_array().map(|a| a.iter().filter_map(Value::as_u64).map(|n| n as u32).collect()).unwrap_or_default()
}

/// [["MAT_ITEM33", 3], ...].
fn pairs(v: &Value) -> Vec<(String, u32)> {
    v.as_array().map(|a| a.iter().filter_map(|p| Some((p[0].as_str()?.to_string(), p[1].as_u64()? as u32))).collect()).unwrap_or_default()
}

fn range(v: &Value) -> (u32, u32) {
    (number(&v["first"]), number(&v["last"]))
}

fn goal_condition(v: &Value) -> Option<GoalCondition> {
    Some(match v["kind"].as_str()? {
        "kind_num" => GoalCondition::KindNum(number(&v["n"])),
        "total_num" => GoalCondition::TotalNum(number(&v["n"])),
        "get_rank" => GoalCondition::GetRank(number(&v["rank"]) as u8),
        "kind_all" => GoalCondition::KindAll { from: number(&v["from"]), to: number(&v["to"]) },
        "spkind_num" => GoalCondition::SpKindNum { material: text(&v["material"]), n: number(&v["n"]) },
        _ => return None,
    })
}

fn goal_reward(v: &Value) -> Option<GoalReward> {
    Some(match v["kind"].as_str()? {
        "get_item" => GoalReward::GetItem(text(&v["item"])),
        "enable_synthesis" => GoalReward::EnableSynthesis(strings(&v["recipes"])),
        "enable_buy" => GoalReward::EnableBuy(strings(&v["items"])),
        _ => return None,
    })
}

/// Reads the sheet (a row it cannot read is left out).
pub fn parse(sheet: &str) -> Tables {
    let mut t = Tables::default();
    let Ok(json) = serde_json::from_str::<Value>(sheet) else { return t };
    for row in json["rows"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let (Some(id), v) = (row[0].as_str(), &row[1]) else { continue };
        if let Some(key) = id.strip_prefix("mat.") {
            t.materials.push(Material { key: key.to_string(), item_id: number(&v["item_id"]), rank: number(&v["rank"]) as u8, grade: number(&v["grade"]) as u8, name: text(&v["name"]), max: number(&v["max"]).max(1) });
        } else if let Some(key) = id.strip_prefix("syn.") {
            let unlock = match (v["condition"].as_str(), v["row"].as_str()) {
                (Some("goal"), Some(row)) => Unlock::Goal(row.to_string()),
                (Some("photo"), Some(row)) => Unlock::Photo(row.to_string()),
                _ => Unlock::Always,
            };
            t.recipes.push(Recipe { id: key.to_string(), flag: number(&v["flag"]), reward: text(&v["reward"]), name: text(&v["name"]), kind: text(&v["kind"]), item_id: number(&v["item_id"]), max: number(&v["max"]).max(1), unlock, materials: pairs(&v["materials"]) });
        } else if let Some(key) = id.strip_prefix("goal.") {
            let (Some(condition), Some(reward)) = (goal_condition(&v["condition"]), goal_reward(&v["reward"])) else { continue };
            t.goals.push(Goal { id: key.to_string(), flag: number(&v["flag"]), condition, reward, text: text(&v["text"]) });
        } else if let Some(key) = id.strip_prefix("photo.") {
            let done = if v["done"]["all"].is_array() { Milestone::All(numbers(&v["done"]["all"])) } else { v["done"]["flag"].as_u64().map_or(Milestone::Never, |f| Milestone::Flag(f as u32)) };
            t.photos.push(Photo { id: key.to_string(), flag: number(&v["flag"]), recipes: strings(&v["recipes"]), order: number(&v["order"]), done, rev: number(&v["rev"]).max(1), name: text(&v["name"]), help: text(&v["help"]), name_from: PackText::parse(&v["name_from"]) });
        } else if let Some(n) = id.strip_prefix("moogle.level.") {
            let Ok(level) = n.parse::<u8>() else { continue };
            let step = if let Some(f) = v["flag"].as_u64() { ShopStep::Flag(f as u32) } else if let Some(n) = v["great_runes"].as_u64() { ShopStep::GreatRunes(n as u32) } else { ShopStep::Start };
            t.shop_levels.push((level, step));
        } else if let Some(key) = id.strip_prefix("forge.").filter(|k| k.starts_with("WEP_")) {
            let levels = v["levels"].as_array().map(Vec::as_slice).unwrap_or(&[]).iter().map(|l| ForgeLevel { level: number(&l["level"]) as u8, strength: l["strength"].as_i64().unwrap_or(0) as i32, magic: l["magic"].as_i64().unwrap_or(0) as i32, ability: l["ability"].as_str().map(str::to_string), materials: pairs(&l["materials"]), init: l["init"].as_bool().unwrap_or(false) }).collect();
            t.keyblades.push(Keyblade { key: key.to_string(), item_id: number(&v["item_id"]), name: text(&v["name"]), strength: v["strength"].as_i64().unwrap_or(0) as i32, magic: v["magic"].as_i64().unwrap_or(0) as i32, abilities: strings(&v["abilities"]), max: number(&v["max"]) as u8, init: number(&v["init"]) as u8, levels });
        } else {
            match id {
                "workshop.stock_cap" => t.stock_cap = number(v),
                "workshop.count_cap" => t.count_cap = number(v),
                "workshop.materials" => t.material_ids = range(v),
                "workshop.keyblades" => t.keyblade_ids = range(v),
                "workshop.unknown_name" => t.unknown_name = text(v),
                "workshop.milestone_rev" => t.milestone_rev = number(v),
                "workshop.menu" => t.menu = v.as_array().map(Vec::as_slice).unwrap_or(&[]).iter().map(|e| MenuEntry { entry: text(&e["entry"]), text: text(&e["text"]) }).collect(),
                "moogle.great_rune_bosses" => t.great_rune_flags = numbers(&v["flags"]),
                "moogle.stock" => {
                    t.stock = v.as_array().map(Vec::as_slice).unwrap_or(&[]).iter().map(|r| StockRow { key: text(&r["key"]), item_id: number(&r["item_id"]), kind: text(&r["kind"]), buy: number(&r["buy"]), level: number(&r["level"]) as u8, mog_card: r["mog_card"].as_bool().unwrap_or(false), unlock_flag: r["unlock_flag"].as_u64().map(|f| f as u32), sold_out: r["sold_out"].as_bool().unwrap_or(false), max: number(&r["max"]).max(1) }).collect();
                }
                _ => {}
            }
        }
    }
    t.shop_levels.sort_by_key(|l| l.0);
    t.milestone_rev = t.milestone_rev.max(1);
    t
}

static TABLES: OnceLock<Tables> = OnceLock::new();

/// The compiled sheet.
pub fn tables() -> &'static Tables {
    TABLES.get_or_init(|| parse(SHEET))
}

impl Tables {
    pub fn material(&self, key: &str) -> Option<&Material> {
        self.materials.iter().find(|m| m.key == key)
    }
    pub fn recipe(&self, id: &str) -> Option<&Recipe> {
        self.recipes.iter().find(|r| r.id == id)
    }
    pub fn goal(&self, id: &str) -> Option<&Goal> {
        self.goals.iter().find(|g| g.id == id)
    }
    pub fn photo(&self, id: &str) -> Option<&Photo> {
        self.photos.iter().find(|p| p.id == id)
    }
    pub fn keyblade(&self, key: &str) -> Option<&Keyblade> {
        self.keyblades.iter().find(|k| k.key == key)
    }
    /// A Sora Keyblade (item id 0x301 .. 0x314): one at most, never offered while held.
    pub fn is_keyblade(&self, item_id: u32) -> bool {
        (self.keyblade_ids.0..=self.keyblade_ids.1).contains(&item_id)
    }
}

/// What the rules reach outside the record: Sora's bag (the camp menu's stock), the item door and the Keyblade levels
/// (P1: kh-host items.rs / weapons.rs; [`with_live`] is the running game's). Every key is the item's own (MAT_ITEM01,
/// PRT_ITEM11, WEP_GOOFY_03, WEP_KEYBLADE_SO_14).
pub trait WorkshopHost {
    /// Copies of an item in Sora's bag (a friend's weapon: copies held).
    fn carried(&self, key: &str) -> u32;
    /// Takes copies out of the bag (never more than it holds).
    fn take_carried(&mut self, key: &str, count: u32);
    /// Hands Sora copies of an item through the item door WITHOUT counting a material gain (the rules count what they hand
    /// over); a Keyblade arrives at its level when obtained (P1's weapons.rs).
    fn give(&mut self, key: &str, count: u32);
    /// The Forge level of a Keyblade Sora owns or has equipped; `None` = not owned.
    fn keyblade_level(&self, key: &str) -> Option<u8>;
    fn set_keyblade_level(&mut self, key: &str, level: u8);
}

/// What a Workshop visit did, for the screens' dialogs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Visit {
    /// Materials were moved into the stock (dialog_0010).
    pub deposited: bool,
    /// Goals achieved now, in table order.
    pub goals: Vec<String>,
    /// GET_ITEM rewards handed over (dialog_0030 each).
    pub items: Vec<String>,
    /// An ENABLE_SYNTHESIS goal listed new recipes (dialog_0020, once).
    pub synthesis_unlocked: bool,
    /// ENABLE_BUY items now sold (dialog_0040 each).
    pub buy_unlocked: Vec<String>,
    /// Milestone missions done now (dialog_0050, once).
    pub photos: Vec<String>,
}

/// Why a synthesis is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SynthRefusal {
    NoSuchRecipe,
    /// Not listed yet.
    Locked,
    Zero,
    /// A Keyblade Sora already holds (one of each).
    KeybladeHeld,
    /// More than the reward's maximum minus what is held.
    Full,
    /// dialog_0210 "You lack the needed materials."
    LackMaterials,
}

/// A synthesis carried out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Synthesized {
    pub recipe: String,
    pub reward: String,
    pub count: u32,
    /// The first time this recipe was made (the SYNTH LOG's Items Created).
    pub first: bool,
}

/// Why the Forge refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForgeRefusal {
    NoSuchKeyblade,
    /// The Forge lists only owned Keyblades.
    NotOwned,
    /// materials_0010 "This Keyblade cannot be made any stronger."
    MaxLevel,
    LackMaterials,
}

/// A Keyblade's numbers at a level: base + every done level.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeybladeStats {
    pub strength: i32,
    pub magic: i32,
    /// The base abilities, then each level's in order.
    pub abilities: Vec<String>,
}

/// A Forge level carried out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Forged {
    pub key: String,
    pub level: u8,
    pub ability: Option<String>,
}

/// One row of the Forge's list.
#[derive(Clone, Debug, PartialEq)]
pub struct ForgeEntry {
    pub key: String,
    pub level: u8,
    pub max: u8,
    /// The level the Forge offers (current + 1), if any.
    pub next: Option<ForgeLevel>,
    /// Its materials are in the stock.
    pub ready: bool,
    pub stats: KeybladeStats,
    pub next_stats: Option<KeybladeStats>,
}

/// One listed recipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeView {
    pub id: String,
    pub new: bool,
    /// How many were made (0: no clear mark).
    pub made: u32,
    /// How many can be made now (0: greyed).
    pub max: u32,
}

/// One listed milestone mission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhotoView {
    pub id: String,
    pub done: bool,
    pub new: bool,
    /// (defeated, needed) of an all-of mission (IM_6's "{0} / {1}").
    pub progress: Option<(u32, u32)>,
}

/// One row of the Material List.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterialView {
    pub key: String,
    /// The name, or "? ? ?" for a material never obtained.
    pub name: String,
    pub known: bool,
    pub stock: u32,
}

/// The SYNTH LOG.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SynthLog {
    /// Recipes made at least once (KH3's Items Created: the "made once" bits).
    pub created: u32,
    pub recipes: u32,
    pub photos: u32,
    pub photo_total: u32,
    pub goals: u32,
    pub goal_total: u32,
}

impl SynthLog {
    /// The three COMPLETE marks (Items Created, Photo Missions, Collector's Goals).
    pub fn complete(&self) -> [bool; 3] {
        [self.created >= self.recipes, self.photos >= self.photo_total, self.goals >= self.goal_total]
    }
}

/// THE MOOGLE'S entries (`workshop.menu`): Synthesize Items, Journey to the Erdtree (KH3's Photo Missions), Collector's
/// Goals, Material List, all shown from the start. NO KEYBLADE FORGE (the user 2026-11-07: "remove the ability to upgrade
/// keyblades from moogle, that is what the blacksmiths are for"): the blacksmiths open the Forge straight.
pub fn menu() -> &'static [MenuEntry] {
    &tables().menu
}

/// The Moogle shop level the flags reach: the highest step whose condition holds (at least 1).
pub fn shop_level_of(flags: &dyn Fn(u32) -> bool) -> u8 {
    let t = tables();
    let runes = t.great_rune_flags.iter().filter(|f| flags(**f)).count() as u32;
    t.shop_levels
        .iter()
        .filter(|(_, step)| match step {
            ShopStep::Start => true,
            ShopStep::Flag(f) => flags(*f),
            ShopStep::GreatRunes(n) => runes >= *n,
        })
        .map(|l| l.0)
        .max()
        .unwrap_or(1)
        .max(1)
}

/// A Keyblade's numbers at a Forge level.
pub fn keyblade_stats(key: &str, level: u8) -> Option<KeybladeStats> {
    let k = tables().keyblade(key)?;
    let mut stats = KeybladeStats { strength: k.strength, magic: k.magic, abilities: k.abilities.clone() };
    for l in k.levels.iter().filter(|l| l.level <= level) {
        stats.strength += l.strength;
        stats.magic += l.magic;
        stats.abilities.extend(l.ability.clone());
    }
    Some(stats)
}

/// The defeat-flag count of a milestone (done, needed).
fn milestone_progress(done: &Milestone, flags: &dyn Fn(u32) -> bool) -> (u32, u32) {
    match done {
        Milestone::Never => (0, 1),
        Milestone::Flag(f) => (u32::from(flags(*f)), 1),
        Milestone::All(list) => (list.iter().filter(|f| flags(**f)).count() as u32, list.len() as u32),
    }
}

/// The record's `ext` lists of this module: missions seen (KH3 flags+0x698) and stocked items seen (the new-arrival rule);
/// and the milestone revision the record was last checked against (absent = 1).
const EXT_PHOTOS_SEEN: &str = "photos_seen";
const EXT_MILESTONE_REV: &str = "milestone_rev";
const EXT_SHOP_SEEN: &str = "shop_seen";

fn ext_has(ws: &Workshop, list: &str, id: &str) -> bool {
    ws.ext[list].as_array().is_some_and(|a| a.iter().any(|v| v.as_str() == Some(id)))
}

/// Adds `id` to an `ext` list; true when it was not there.
fn ext_insert(ws: &mut Workshop, list: &str, id: &str) -> bool {
    if ext_has(ws, list, id) {
        return false;
    }
    if !ws.ext.is_object() {
        ws.ext = json!({});
    }
    if !ws.ext[list].is_array() {
        ws.ext[list] = json!([]);
    }
    if let Some(a) = ws.ext[list].as_array_mut() {
        a.push(json!(id));
    }
    true
}

fn push_once(list: &mut Vec<String>, id: &str) {
    if !list.iter().any(|x| x == id) {
        list.push(id.to_string());
    }
}

pub fn stock_of(ws: &Workshop, key: &str) -> u32 {
    ws.stock.get(key).copied().unwrap_or(0)
}

/// Ever obtained (the "? ? ?" rule).
pub fn is_known(ws: &Workshop, key: &str) -> bool {
    ws.obtained_of(key) > 0
}

/// A material the Workshop hands over counts as obtained (the record's counts; P1's hook counts every other path).
fn count_gain(ws: &mut Workshop, key: &str, count: u32) {
    if count > 0 && tables().material(key).is_some() {
        let n = ws.obtained.entry(key.to_string()).or_insert(0);
        *n = n.saturating_add(count);
        ws.total_obtained = ws.total_obtained.saturating_add(count);
    }
}

fn add_stock(ws: &mut Workshop, key: &str, count: u32) {
    let n = (stock_of(ws, key) + count).min(tables().stock_cap);
    ws.stock.insert(key.to_string(), n);
}

/// Spends materials: the stock first, then the bag.
fn spend(ws: &mut Workshop, host: &mut dyn WorkshopHost, key: &str, count: u32) {
    let have = stock_of(ws, key);
    let from_stock = have.min(count);
    ws.stock.insert(key.to_string(), have - from_stock);
    let rest = count - from_stock;
    if rest > 0 {
        let carried = host.carried(key);
        host.take_carried(key, rest.min(carried));
    }
}

/// Hands over an item: a material into the bag up to its maximum and the rest into the stock (counted as obtained);
/// anything else through the item door.
fn hand_over(ws: &mut Workshop, host: &mut dyn WorkshopHost, key: &str, count: u32) {
    let Some(m) = tables().material(key) else {
        host.give(key, count);
        return;
    };
    let to_bag = count.min(m.max.saturating_sub(host.carried(key)));
    if to_bag > 0 {
        host.give(key, to_bag);
    }
    if count > to_bag {
        add_stock(ws, key, count - to_bag);
    }
    count_gain(ws, key, count);
}

/// Moves every material Sora carries into the stock (up to its cap; what does not fit stays in the bag). True: some
/// moved (dialog_0010).
pub fn deposit(ws: &mut Workshop, host: &mut dyn WorkshopHost) -> bool {
    let cap = tables().stock_cap;
    let mut moved = false;
    for m in &tables().materials {
        let n = host.carried(&m.key).min(cap.saturating_sub(stock_of(ws, &m.key)));
        if n > 0 {
            host.take_carried(&m.key, n);
            add_stock(ws, &m.key, n);
            moved = true;
        }
    }
    moved
}

/// (have, need) of a goal, for the list (kinds, units, or the materials of the rank / range).
pub fn goal_progress(ws: &Workshop, goal: &Goal) -> (u32, u32) {
    let mats = &tables().materials;
    let known = |pick: &dyn Fn(&Material) -> bool| {
        let of: Vec<&Material> = mats.iter().filter(|m| pick(m)).collect();
        (of.iter().filter(|m| is_known(ws, &m.key)).count() as u32, of.len() as u32)
    };
    match &goal.condition {
        GoalCondition::KindNum(n) => (mats.iter().filter(|m| is_known(ws, &m.key)).count() as u32, *n),
        GoalCondition::TotalNum(n) => (ws.total_obtained, *n),
        GoalCondition::GetRank(rank) => known(&|m| m.rank == *rank),
        GoalCondition::KindAll { from, to } => known(&|m| (*from..=*to).contains(&m.item_id)),
        GoalCondition::SpKindNum { material, n } => (ws.obtained_of(material), *n),
    }
}

pub fn goal_met(ws: &Workshop, goal: &Goal) -> bool {
    let (have, need) = goal_progress(ws, goal);
    have >= need
}

pub fn goal_done(ws: &Workshop, id: &str) -> bool {
    ws.goals.iter().any(|g| g == id)
}

pub fn photo_done(ws: &Workshop, id: &str) -> bool {
    ws.photos.iter().any(|p| p == id)
}

/// BRINGS A SAVE OVER TO THE SHEET'S MILESTONE LIST, once (the record's `ext` "milestone_rev" < `workshop.milestone_rev`):
/// a stored completion of a row whose subject changed since (its `rev` is newer than the record's) is kept only if its new
/// milestone is met now (its boss defeat flags), so no mission reads done for a boss never beaten; a row whose boss stayed
/// keeps its completion even if the flags read false (New Game+ clears Elden Ring's defeat flags, the record keeps what was
/// done, as KH3 never takes a photo back). Every changed row shows NEW again. True when the record was brought over.
pub fn migrate_milestones(ws: &mut Workshop, flags: &dyn Fn(u32) -> bool) -> bool {
    let t = tables();
    let had = ws.ext.get(EXT_MILESTONE_REV).and_then(Value::as_u64).map_or(1, |r| r as u32);
    if had >= t.milestone_rev {
        return false;
    }
    let changed = |id: &str| t.photo(id).is_some_and(|p| p.rev > had);
    ws.photos.retain(|id| {
        let Some(p) = t.photo(id) else { return true };
        !changed(id) || {
            let (have, need) = milestone_progress(&p.done, flags);
            have >= need
        }
    });
    if let Some(seen) = ws.ext.get_mut(EXT_PHOTOS_SEEN).and_then(Value::as_array_mut) {
        seen.retain(|v| v.as_str().is_some_and(|id| !changed(id)));
    }
    if !ws.ext.is_object() {
        ws.ext = json!({});
    }
    ws.ext[EXT_MILESTONE_REV] = json!(t.milestone_rev);
    true
}

/// THE WORKSHOP VISIT (KH3 0x14172EFF0): the deposit, every open goal in table order (no chaining: the counts are those
/// from before the visit's rewards), then the milestone missions, then the shop level. The Moogle's Workshop calls it on
/// opening, and so does a smith's Keyblade Forge (the Forge counts the stock).
pub fn visit(ws: &mut Workshop, host: &mut dyn WorkshopHost, flags: &dyn Fn(u32) -> bool) -> Visit {
    let t = tables();
    migrate_milestones(ws, flags);
    let mut out = Visit { deposited: deposit(ws, host), ..Visit::default() };
    let passing: Vec<&Goal> = t.goals.iter().filter(|g| !goal_done(ws, &g.id) && goal_met(ws, g)).collect();
    for goal in passing {
        ws.goals.push(goal.id.clone());
        out.goals.push(goal.id.clone());
        match &goal.reward {
            GoalReward::GetItem(item) => {
                hand_over(ws, host, item, 1);
                out.items.push(item.clone());
            }
            GoalReward::EnableSynthesis(_) => out.synthesis_unlocked = true,
            GoalReward::EnableBuy(items) => {
                for item in items {
                    push_once(&mut ws.buy_unlocked, item);
                }
                out.buy_unlocked.extend(items.iter().cloned());
            }
        }
    }
    for photo in &t.photos {
        let (have, need) = milestone_progress(&photo.done, flags);
        if !photo_done(ws, &photo.id) && have >= need {
            ws.photos.push(photo.id.clone());
            out.photos.push(photo.id.clone());
        }
    }
    raise_shop_level(ws, flags);
    out
}

/// A recipe is listed (KH3: also when the row it names is missing).
pub fn recipe_listed(ws: &Workshop, recipe: &Recipe) -> bool {
    let t = tables();
    match &recipe.unlock {
        Unlock::Always => true,
        Unlock::Goal(id) => goal_done(ws, id) || t.goal(id).is_none(),
        Unlock::Photo(id) => photo_done(ws, id) || t.photo(id).is_none(),
    }
}

/// Listed and not yet viewed.
pub fn recipe_is_new(ws: &Workshop, recipe: &Recipe) -> bool {
    recipe_listed(ws, recipe) && !ws.recipes_seen.contains(&recipe.id)
}

pub fn mark_recipe_seen(ws: &mut Workshop, id: &str) {
    push_once(&mut ws.recipes_seen, id);
}

/// How many of a recipe can be made now: min(maximum - held, the stock's share of each material); 0 for a Keyblade Sora
/// holds.
pub fn synth_max(ws: &Workshop, host: &dyn WorkshopHost, recipe: &Recipe) -> u32 {
    let keyblade = tables().is_keyblade(recipe.item_id);
    if keyblade && host.keyblade_level(&recipe.reward).is_some() {
        return 0;
    }
    let held = if keyblade { 0 } else { host.carried(&recipe.reward) };
    let by_stock = recipe.materials.iter().map(|(k, n)| stock_of(ws, k) / (*n).max(1)).min().unwrap_or(0);
    recipe.max.saturating_sub(held).min(by_stock)
}

/// THE SYNTHESIS (KH3 0x14112A100): `count` x the reward for `count` x each material from the stock; no munny.
pub fn synthesize(ws: &mut Workshop, host: &mut dyn WorkshopHost, id: &str, count: u32) -> Result<Synthesized, SynthRefusal> {
    let t = tables();
    let recipe = t.recipe(id).ok_or(SynthRefusal::NoSuchRecipe)?;
    if !recipe_listed(ws, recipe) {
        return Err(SynthRefusal::Locked);
    }
    if count == 0 {
        return Err(SynthRefusal::Zero);
    }
    let keyblade = t.is_keyblade(recipe.item_id);
    if keyblade && host.keyblade_level(&recipe.reward).is_some() {
        return Err(SynthRefusal::KeybladeHeld);
    }
    let held = if keyblade { 0 } else { host.carried(&recipe.reward) };
    if count > recipe.max.saturating_sub(held) {
        return Err(SynthRefusal::Full);
    }
    if recipe.materials.iter().any(|(k, n)| stock_of(ws, k) < n * count) {
        return Err(SynthRefusal::LackMaterials);
    }
    for (k, n) in &recipe.materials {
        spend(ws, host, k, n * count);
    }
    hand_over(ws, host, &recipe.reward, count);
    let made = ws.created.entry(recipe.id.clone()).or_insert(0);
    let first = *made == 0;
    *made = (*made + count).min(t.count_cap.max(1));
    ws.items_created = ws.items_created.saturating_add(count);
    Ok(Synthesized { recipe: recipe.id.clone(), reward: recipe.reward.clone(), count, first })
}

/// The listed recipes in table order (the screen sorts by tab).
pub fn recipes(ws: &Workshop, host: &dyn WorkshopHost) -> Vec<RecipeView> {
    tables()
        .recipes
        .iter()
        .filter(|r| recipe_listed(ws, r))
        .map(|r| RecipeView { id: r.id.clone(), new: recipe_is_new(ws, r), made: ws.created.get(&r.id).copied().unwrap_or(0), max: synth_max(ws, host, r) })
        .collect()
}

/// The milestone missions, every one listed from the start (the user 2026-11-07), in the journey's order.
pub fn photo_list(ws: &Workshop, flags: &dyn Fn(u32) -> bool) -> Vec<PhotoView> {
    let mut list: Vec<&Photo> = tables().photos.iter().collect();
    list.sort_by_key(|p| p.order);
    list.into_iter()
        .map(|p| PhotoView {
            id: p.id.clone(),
            done: photo_done(ws, &p.id),
            new: !ext_has(ws, EXT_PHOTOS_SEEN, &p.id),
            progress: matches!(p.done, Milestone::All(_)).then(|| milestone_progress(&p.done, flags)),
        })
        .collect()
}

pub fn mark_photo_seen(ws: &mut Workshop, id: &str) {
    ext_insert(ws, EXT_PHOTOS_SEEN, id);
}

/// The level the Forge offers a Keyblade, if its materials are in the stock.
pub fn forge_check(ws: &Workshop, host: &dyn WorkshopHost, key: &str) -> Result<&'static ForgeLevel, ForgeRefusal> {
    let k = tables().keyblade(key).ok_or(ForgeRefusal::NoSuchKeyblade)?;
    let level = host.keyblade_level(key).ok_or(ForgeRefusal::NotOwned)?;
    let next = k.levels.iter().find(|l| l.level == level + 1).ok_or(ForgeRefusal::MaxLevel)?;
    if next.materials.iter().any(|(m, n)| stock_of(ws, m) < *n) {
        return Err(ForgeRefusal::LackMaterials);
    }
    Ok(next)
}

/// THE KEYBLADE FORGE (KH3 0x14112A3C0): the next level for its materials (stock first, then the bag); no munny.
pub fn forge(ws: &mut Workshop, host: &mut dyn WorkshopHost, key: &str) -> Result<Forged, ForgeRefusal> {
    let next = forge_check(ws, host, key)?;
    for (m, n) in &next.materials {
        spend(ws, host, m, *n);
    }
    host.set_keyblade_level(key, next.level);
    Ok(Forged { key: key.to_string(), level: next.level, ability: next.ability.clone() })
}

/// The Forge's list: every owned Keyblade, by item id.
pub fn forge_list(ws: &Workshop, host: &dyn WorkshopHost) -> Vec<ForgeEntry> {
    let mut list: Vec<&Keyblade> = tables().keyblades.iter().filter(|k| host.keyblade_level(&k.key).is_some()).collect();
    list.sort_by_key(|k| k.item_id);
    list.into_iter()
        .map(|k| {
            let level = host.keyblade_level(&k.key).unwrap_or(0);
            let next = k.levels.iter().find(|l| l.level == level + 1).cloned();
            ForgeEntry {
                key: k.key.clone(),
                level,
                max: k.max,
                ready: forge_check(ws, host, &k.key).is_ok(),
                stats: keyblade_stats(&k.key, level).unwrap_or_default(),
                next_stats: next.as_ref().and_then(|n| keyblade_stats(&k.key, n.level)),
                next,
            }
        })
        .collect()
}

/// Raises the shop level to what the flags reach (never lowers it). KH3 sets it as each level loads; the host calls it
/// on load and the visit does.
pub fn raise_shop_level(ws: &mut Workshop, flags: &dyn Fn(u32) -> bool) -> u8 {
    ws.raise_moogle_level(shop_level_of(flags));
    ws.moogle_level
}

/// An ENABLE_BUY row's goal is achieved.
pub fn buy_unlocked(ws: &Workshop, row: &StockRow) -> bool {
    row.unlock_flag.is_some_and(|f| tables().goals.iter().any(|g| g.flag == f && goal_done(ws, &g.id)))
}

/// THE MOOGLE'S STOCK now, in the table's order (the same at every Moogle).
pub fn moogle_stock(ws: &Workshop, host: &dyn WorkshopHost, has_mog_card: bool) -> Vec<&'static StockRow> {
    let level = ws.moogle_level.max(1);
    tables()
        .stock
        .iter()
        .filter(|r| !(r.sold_out && host.carried(&r.key) >= r.max))
        .filter(|r| (1..=level).contains(&r.level) || buy_unlocked(ws, r) || (r.mog_card && has_mog_card))
        .collect()
}

/// Marks the stock seen; true when any of it was not seen before (a "new arrival").
pub fn mark_stock_seen(ws: &mut Workshop, rows: &[&StockRow]) -> bool {
    let mut new = false;
    for r in rows {
        new |= ext_insert(ws, EXT_SHOP_SEEN, &r.key);
    }
    new
}

/// The Material List: all 60 in the table's order; a material never obtained is "? ? ?".
pub fn material_list(ws: &Workshop) -> Vec<MaterialView> {
    let t = tables();
    t.materials
        .iter()
        .map(|m| {
            let known = is_known(ws, &m.key);
            MaterialView { key: m.key.clone(), name: if known { m.name.clone() } else { t.unknown_name.clone() }, known, stock: stock_of(ws, &m.key) }
        })
        .collect()
}

pub fn log(ws: &Workshop) -> SynthLog {
    let t = tables();
    SynthLog {
        created: t.recipes.iter().filter(|r| ws.created.get(&r.id).is_some_and(|n| *n > 0)).count() as u32,
        recipes: t.recipes.len() as u32,
        photos: t.photos.iter().filter(|p| photo_done(ws, &p.id)).count() as u32,
        photo_total: t.photos.len() as u32,
        goals: t.goals.iter().filter(|g| goal_done(ws, &g.id)).count() as u32,
        goal_total: t.goals.len() as u32,
    }
}

// ------------------------------------------------------------------------------------------------ the running game

/// The running game's side: the camp menu's stock as the HUD last told it (`crate::items::held`) less what the rules took
/// (asked of the HUD through `crate::items::take`), the Keyblades of P1's running record, and the handovers kept until
/// the record is released.
struct LiveHost<'a> {
    keyblades: &'a mut Vec<OwnedKeyblade>,
    taken: BTreeMap<String, u32>,
    given: Vec<(String, u32)>,
}

impl WorkshopHost for LiveHost<'_> {
    fn carried(&self, key: &str) -> u32 {
        let given: u32 = self.given.iter().filter(|g| g.0 == key).map(|g| g.1).sum();
        (crate::items::held(key) + given).saturating_sub(self.taken.get(key).copied().unwrap_or(0))
    }
    fn take_carried(&mut self, key: &str, count: u32) {
        let mut rest = count.min(self.carried(key));
        // what was handed over in this run and not delivered yet first, then the HUD's stock
        for g in self.given.iter_mut().filter(|g| g.0 == key) {
            let n = g.1.min(rest);
            g.1 -= n;
            rest -= n;
        }
        if rest > 0 {
            if let Some(item) = crate::items::by_key(key) {
                crate::items::take(item.item_id, rest);
            }
            *self.taken.entry(key.to_string()).or_insert(0) += rest;
        }
    }
    fn give(&mut self, key: &str, count: u32) {
        self.given.push((key.to_string(), count));
    }
    fn keyblade_level(&self, key: &str) -> Option<u8> {
        let owned = self.keyblades.iter().find(|k| k.key == key).map(|k| k.level);
        owned.or_else(|| (crate::weapons::is_keyblade(key) && self.given.iter().any(|g| g.0 == key)).then(|| crate::weapons::init_level(key)))
    }
    fn set_keyblade_level(&mut self, key: &str, level: u8) {
        if let Some(k) = self.keyblades.iter_mut().find(|k| k.key == key) {
            k.level = level.clamp(crate::weapons::init_level(key), crate::weapons::MAX_LEVEL);
        }
    }
}

/// RUNS THE RULES ON THE RUNNING GAME (P6b's screens, P7's smiths): P1's running record (`crate::weapons::with`) with
/// the camp menu's stock and the Keyblades as the host; once the record is released, each item handed over goes through
/// the item door (a Keyblade through `crate::weapons::give_key`, a material onto the door's queue without the gain hook:
/// the rules counted it already, anything else through `crate::items::give`).
pub fn with_live<R>(f: impl FnOnce(&mut Workshop, &mut dyn WorkshopHost) -> R) -> R {
    let (out, given) = crate::weapons::with(|s| {
        let mut host = LiveHost { keyblades: &mut s.keyblades, taken: BTreeMap::new(), given: Vec::new() };
        let out = f(&mut s.workshop, &mut host);
        (out, host.given)
    });
    for (key, n) in given.into_iter().filter(|g| g.1 > 0) {
        if crate::weapons::by_key(&key).is_some() {
            crate::weapons::give_key(&key, n);
        } else if let Some(item) = crate::items::by_key(&key) {
            if item.class == crate::items::Class::Material {
                crate::items::queue(item.item_id, n);
            } else {
                crate::items::give(item.item_id, n);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (KH3TEXT, THE USER 2026-11-08: "keep radhan's milestone text what it was") Radahn's line is KH3's mission_0080 read
    /// from the pack with its final "." as "!"; nothing when the pack lacks it (no compiled copy).
    #[test]
    fn radahns_line_is_the_packs_with_its_bang() {
        let t = tables();
        let radahn = t.photo("IM_17").expect("IM_17");
        let r = radahn.name_from.as_ref().expect("IM_17 reads its line from the pack");
        assert_eq!((r.id.as_str(), r.period_to.as_str(), radahn.name.as_str()), ("kh3_menu.shop.mission_0080", "!", ""));
        let pack = |id: &str| if id == "kh3_menu.shop.mission_0080" { "<line>, kupo.".to_string() } else { id.to_string() };
        assert_eq!(r.resolve(&pack), "<line>, kupo!");
        assert_eq!(r.resolve(&|id: &str| id.to_string()), "", "the pack lacks it: nothing");
        assert_eq!(r.resolve(&|_: &str| String::new()), "");
        assert_eq!(PackText { id: "x".into(), period_to: String::new() }.resolve(&|_: &str| "Kept.".to_string()), "Kept.");
        assert_eq!(t.photos.iter().filter(|p| p.name_from.is_some()).count(), 1, "only Radahn's");
    }

    /// Sora's bag and Keyblades, as a host keeps them.
    #[derive(Default)]
    struct Bag {
        items: BTreeMap<String, u32>,
        keyblades: BTreeMap<String, u8>,
    }

    impl WorkshopHost for Bag {
        fn carried(&self, key: &str) -> u32 {
            self.items.get(key).copied().unwrap_or(0)
        }
        fn take_carried(&mut self, key: &str, count: u32) {
            let n = self.carried(key).saturating_sub(count);
            self.items.insert(key.to_string(), n);
        }
        fn give(&mut self, key: &str, count: u32) {
            if let Some(k) = tables().keyblade(key) {
                self.keyblades.insert(key.to_string(), k.init);
            } else {
                *self.items.entry(key.to_string()).or_default() += count;
            }
        }
        fn keyblade_level(&self, key: &str) -> Option<u8> {
            self.keyblades.get(key).copied()
        }
        fn set_keyblade_level(&mut self, key: &str, level: u8) {
            self.keyblades.insert(key.to_string(), level);
        }
    }

    fn no_flags(_: u32) -> bool {
        false
    }

    fn boss(row: &str) -> u32 {
        let json: Value = serde_json::from_str(crate::bossrewards::SHEET).unwrap();
        let r = json["rows"].as_array().unwrap().iter().find(|r| r[0].as_str() == Some(&format!("boss.{row}"))).unwrap_or_else(|| panic!("{row}"));
        assert_eq!(r[1]["flag_conf"].as_str(), Some("verified"), "{row}");
        r[1]["flag"].as_u64().unwrap() as u32
    }

    fn material_keys(pick: impl Fn(&Material) -> bool) -> Vec<String> {
        tables().materials.iter().filter(|m| pick(m)).map(|m| m.key.clone()).collect()
    }

    /// Materials ever obtained, as P1's hook counts them.
    fn obtain(ws: &mut Workshop, keys: &[String], n: u32) {
        for k in keys {
            count_gain(ws, k, n);
        }
    }

    /// The sheet is KH3's: 88 recipes (1 always, 67 by a goal, 20 by a mission), 78 goals (24 items, 24 recipe lists, 30
    /// purchases), 20 missions, 60 materials, 17 Keyblades x 10 levels, 7 shop levels, 7 Great Rune holders; every goal
    /// names its recipes and every recipe its goal; every ENABLE_BUY goal unlocks one stock row by its flag.
    #[test]
    fn the_sheet_is_kh3s() {
        let t = tables();
        let by = |u: fn(&Unlock) -> bool| t.recipes.iter().filter(|r| u(&r.unlock)).count();
        assert_eq!((t.recipes.len(), by(|u| *u == Unlock::Always), by(|u| matches!(u, Unlock::Goal(_))), by(|u| matches!(u, Unlock::Photo(_)))), (88, 1, 67, 20));
        let rewards = |f: fn(&GoalReward) -> bool| t.goals.iter().filter(|g| f(&g.reward)).count();
        assert_eq!((t.goals.len(), rewards(|r| matches!(r, GoalReward::GetItem(_))), rewards(|r| matches!(r, GoalReward::EnableSynthesis(_))), rewards(|r| matches!(r, GoalReward::EnableBuy(_)))), (78, 24, 24, 30));
        assert_eq!((t.photos.len(), t.materials.len(), t.keyblades.len(), t.shop_levels.len(), t.great_rune_flags.len()), (20, 60, 17, 7, 7));
        assert_eq!((t.stock_cap, t.count_cap, t.material_ids, t.keyblade_ids, t.unknown_name.as_str()), (999, 999, (0x701, 0x73C), (0x301, 0x314), "? ? ?"));
        assert_eq!(menu().iter().map(|e| e.entry.as_str()).collect::<Vec<_>>(), ["synthesize", "photo", "goals", "materials"], "the Moogle has no Keyblade Forge (the user 2026-11-07)");
        assert!(t.keyblades.iter().all(|k| k.levels.len() == 10 && k.max == 10));
        assert!(t.materials.iter().all(|m| crate::weapons::is_goal_material(&m.key) && m.max == 99), "the materials P1's record counts");
        for r in &t.recipes {
            assert!(r.materials.iter().all(|(k, n)| t.material(k).is_some() && *n > 0), "{}: every ingredient is a material", r.id);
            match &r.unlock {
                Unlock::Goal(g) => assert!(matches!(&t.goal(g).unwrap().reward, GoalReward::EnableSynthesis(list) if list.contains(&r.id)), "{}", r.id),
                Unlock::Photo(p) => assert_eq!(t.photo(p).unwrap().recipes, vec![r.id.clone()]),
                Unlock::Always => assert_eq!(r.id, "IS_0"),
            }
            assert!(crate::items::by_key(&r.reward).is_some() || crate::weapons::by_key(&r.reward).is_some(), "{}: the reward is an item of the mod", r.id);
        }
        for g in &t.goals {
            if let GoalReward::EnableBuy(items) = &g.reward {
                let rows: Vec<String> = t.stock.iter().filter(|s| s.unlock_flag == Some(g.flag)).map(|s| s.key.clone()).collect();
                assert_eq!(rows, *items, "{}", g.id);
            }
        }
        assert_eq!(t.recipe("IS_58").map(|r| (r.reward.as_str(), r.max, r.unlock.clone())), Some(("WEP_KEYBLADE_SO_14", 1, Unlock::Goal("IC_24".into()))));
        assert_eq!(t.goal("IC_24").map(|g| g.condition.clone()), Some(GoalCondition::KindNum(58)));
        for k in &t.keyblades {
            assert_eq!(crate::weapons::init_level(&k.key), k.init, "{}: P1's level when obtained", k.key);
        }
    }

    /// (THE USER 2026-11-03, plan 6c) Moogle shop levels by defeat flags: 1 start, 2 Godrick, 3 / 4 the second / third
    /// Great Rune holder defeated (in any order), 5 Morgott, 6 Maliketh, 7 Godfrey / Hoarah Loux; never lowered.
    #[test]
    fn the_shop_level_follows_the_flags() {
        let level = |rows: &[&str]| {
            let flags: Vec<u32> = rows.iter().map(|r| boss(r)).collect();
            shop_level_of(&|f| flags.contains(&f))
        };
        assert_eq!(level(&[]), 1);
        assert_eq!(level(&["margit"]), 1);
        assert_eq!(level(&["godrick"]), 2);
        assert_eq!(level(&["rennala"]), 1, "one Great Rune holder that is not Godrick");
        assert_eq!(level(&["godrick", "rennala"]), 3);
        assert_eq!(level(&["rennala", "radahn"]), 3, "the second Great Rune holder, whoever it is");
        assert_eq!(level(&["godrick", "rennala", "radahn"]), 4);
        assert_eq!(level(&["rykard", "mohg_lord", "malenia"]), 4);
        assert_eq!(level(&["godrick", "rennala", "morgott"]), 5);
        assert_eq!(level(&["maliketh"]), 6);
        assert_eq!(level(&["hoarah_loux"]), 7);
        let mut ws = Workshop { moogle_level: 5, ..Workshop::default() };
        assert_eq!(raise_shop_level(&mut ws, &no_flags), 5, "never lowered");
        let flags = [boss("maliketh")];
        assert_eq!(raise_shop_level(&mut ws, &|f| flags.contains(&f)), 6);
        assert_eq!(Workshop::default().moogle_level, 1);
    }

    /// The stock by shop level, the M.O.G. Card rule and the ENABLE_BUY unlock (IC_48: 30 Blazing Shards obtained ->
    /// the Moogle sells them; a new arrival once).
    #[test]
    fn the_moogle_stock_follows_its_rules() {
        let mut bag = Bag::default();
        let keys = |ws: &Workshop, bag: &Bag, card: bool| moogle_stock(ws, bag, card).iter().map(|r| r.key.clone()).collect::<Vec<_>>();
        let mut ws = Workshop::default();
        let start = keys(&ws, &bag, false);
        assert!(start.iter().all(|k| tables().stock.iter().any(|r| r.key == *k && r.level == 1)));
        assert!(start.contains(&"ITEM_POTION".to_string()) && !start.contains(&"WEP_DONALD_02".to_string()));
        ws.moogle_level = 2;
        assert!(keys(&ws, &bag, false).contains(&"WEP_DONALD_02".to_string()) && !keys(&ws, &bag, false).contains(&"WEP_DONALD_04".to_string()));
        ws.moogle_level = 6;
        for w in ["WEP_DONALD_02", "WEP_GOOFY_04", "WEP_DONALD_06", "WEP_GOOFY_08", "MAT_ITEM53"] {
            assert!(keys(&ws, &bag, false).contains(&w.to_string()), "{w}");
        }
        assert!(!keys(&ws, &bag, false).contains(&"ITEM_ELIXIR".to_string()) && keys(&ws, &bag, true).contains(&"ITEM_ELIXIR".to_string()), "the M.O.G. Card");
        assert!(!keys(&ws, &bag, false).contains(&"MAT_ITEM01".to_string()), "a material only after its goal");
        obtain(&mut ws, &["MAT_ITEM01".to_string()], 29);
        visit(&mut ws, &mut bag, &no_flags);
        assert!(!goal_done(&ws, "IC_48"));
        obtain(&mut ws, &["MAT_ITEM01".to_string()], 1);
        let v = visit(&mut ws, &mut bag, &no_flags);
        assert!(v.goals.contains(&"IC_48".to_string()) && v.buy_unlocked == vec!["MAT_ITEM01".to_string()]);
        assert_eq!(ws.buy_unlocked, vec!["MAT_ITEM01".to_string()]);
        let stock = moogle_stock(&ws, &bag, false);
        assert!(stock.iter().any(|r| r.key == "MAT_ITEM01"));
        assert!(mark_stock_seen(&mut ws, &stock), "a new arrival");
        assert!(!mark_stock_seen(&mut ws, &stock), "seen");
    }

    /// Every goal condition kind, at its threshold and one short.
    #[test]
    fn every_goal_condition_kind() {
        let t = tables();
        let goal = |id: &str| t.goal(id).unwrap();
        // KIND_NUM: IC_2 = 4 kinds
        let mut ws = Workshop::default();
        obtain(&mut ws, &material_keys(|m| m.item_id <= 0x703), 1);
        assert!(!goal_met(&ws, goal("IC_2")));
        count_gain(&mut ws, "MAT_ITEM04", 1);
        assert_eq!(goal_progress(&ws, goal("IC_2")), (4, 4));
        count_gain(&mut ws, "ITEM_POTION", 1);
        count_gain(&mut ws, "MAT_GLOVE_GRAVE_01", 1);
        assert_eq!(ws.kinds_obtained(), 4, "only KH3's materials are obtained kinds");
        // TOTAL_NUM: IC_26 = 50 ever obtained (all 60)
        obtain(&mut ws, &["MAT_ITEM40".to_string()], 45);
        assert_eq!(goal_progress(&ws, goal("IC_26")), (49, 50));
        count_gain(&mut ws, "MAT_ITEM41", 1);
        assert!(goal_met(&ws, goal("IC_26")));
        // GET_RANK: IC_31 = every RANK03 (the 13 shards)
        let shards = material_keys(|m| m.rank == 3);
        assert_eq!(shards.len(), 13);
        let mut ws = Workshop::default();
        obtain(&mut ws, &shards[1..], 1);
        assert_eq!(goal_progress(&ws, goal("IC_31")), (12, 13));
        count_gain(&mut ws, &shards[0], 1);
        assert!(goal_met(&ws, goal("IC_31")));
        // KIND_ALL: IC_41 = MAT_ITEM21 ..= MAT_ITEM24
        let mut ws = Workshop::default();
        obtain(&mut ws, &["MAT_ITEM21", "MAT_ITEM22", "MAT_ITEM24"].map(String::from), 1);
        assert!(!goal_met(&ws, goal("IC_41")));
        count_gain(&mut ws, "MAT_ITEM23", 1);
        assert!(goal_met(&ws, goal("IC_41")));
        // SPKIND_NUM: IC_77 = 20 Sinister Gems ever obtained
        let mut ws = Workshop::default();
        count_gain(&mut ws, "MAT_ITEM39", 19);
        assert!(!goal_met(&ws, goal("IC_77")));
        count_gain(&mut ws, "MAT_ITEM39", 1);
        assert!(goal_met(&ws, goal("IC_77")));
    }

    /// The visit: the deposit (cap 999, the rest stays carried), goals in table order granted once with their rewards
    /// (GET_ITEM handed over and counted), no chaining within one visit, missions after.
    #[test]
    fn the_visit_deposits_then_tests_goals_without_chaining() {
        let mut bag = Bag::default();
        let mut ws = Workshop::default();
        // every shard but the Hungry Shard, 5 each (60 units): IC_26 (50 units) gives a Hungry Shard; IC_31 (every shard)
        // waits for the next visit
        let shards = material_keys(|m| m.rank == 3 && m.key != "MAT_ITEM49");
        for key in &shards {
            bag.items.insert(key.clone(), 5);
        }
        obtain(&mut ws, &shards, 5);
        let first = visit(&mut ws, &mut bag, &no_flags);
        assert!(first.deposited && shards.iter().all(|k| bag.carried(k) == 0 && stock_of(&ws, k) == 5));
        assert!(first.goals.contains(&"IC_26".to_string()) && first.items.contains(&"MAT_ITEM49".to_string()));
        assert!(first.goals.contains(&"IC_0".to_string()) && bag.carried("ITEM_ETHER") == 1, "IC_0's Ether");
        assert!(first.synthesis_unlocked, "IC_1 .. IC_6 list recipes");
        assert!(!first.goals.contains(&"IC_31".to_string()), "no chaining: the Hungry Shard came from this visit");
        assert_eq!((bag.carried("MAT_ITEM49"), ws.obtained_of("MAT_ITEM49"), ws.total_obtained), (1, 1, 61), "handed over and counted once");
        let second = visit(&mut ws, &mut bag, &no_flags);
        assert_eq!((second.goals, second.items), (vec!["IC_31".to_string()], vec!["MAT_ITEM50".to_string()]));
        let third = visit(&mut ws, &mut bag, &no_flags);
        assert!(third.goals.is_empty() && third.items.is_empty() && bag.carried("ITEM_ETHER") == 1, "each goal once");
        // the stock's cap
        ws.stock.insert("MAT_ITEM02".into(), 997);
        bag.items.insert("MAT_ITEM02".into(), 5);
        assert!(deposit(&mut ws, &mut bag));
        assert_eq!((stock_of(&ws, "MAT_ITEM02"), bag.carried("MAT_ITEM02")), (999, 3));
        assert!(!deposit(&mut ws, &mut bag), "nothing fits");
    }

    /// Every recipe's unlock path: IS_0 always; a goal recipe once its goal is achieved; a mission recipe once its
    /// milestone is done; "NEW" until seen.
    #[test]
    fn every_recipe_unlock_path() {
        let t = tables();
        for r in &t.recipes {
            let mut ws = Workshop::default();
            assert_eq!(recipe_listed(&ws, r), r.unlock == Unlock::Always, "{}", r.id);
            match &r.unlock {
                Unlock::Goal(g) => ws.goals.push(g.clone()),
                Unlock::Photo(p) => ws.photos.push(p.clone()),
                Unlock::Always => {}
            }
            assert!(recipe_listed(&ws, r) && recipe_is_new(&ws, r), "{}", r.id);
            mark_recipe_seen(&mut ws, &r.id);
            assert!(!recipe_is_new(&ws, r));
        }
        // through the visit: IC_1 (two kinds) lists IS_1, IS_3, IS_4; Rykard's defeat (IM_0) lists IS_66 (Journey to the Erdtree: the 5th milestone, the 5th tier)
        let mut bag = Bag::default();
        let mut ws = Workshop::default();
        assert!(recipes(&ws, &bag).iter().map(|v| v.id.as_str()).eq(["IS_0"]));
        obtain(&mut ws, &["MAT_ITEM01", "MAT_ITEM02"].map(String::from), 1);
        visit(&mut ws, &mut bag, &no_flags);
        let listed: Vec<String> = recipes(&ws, &bag).into_iter().map(|v| v.id).collect();
        assert_eq!(listed, ["IS_0", "IS_1", "IS_3", "IS_4"]);
        assert!(!recipe_listed(&ws, t.recipe("IS_66").unwrap()));
        let rykard = boss("rykard");
        let v = visit(&mut ws, &mut bag, &|f| f == rykard);
        assert_eq!(v.photos, vec!["IM_0".to_string()]);
        assert!(recipe_is_new(&ws, t.recipe("IS_66").unwrap()));
    }

    /// Synthesis consumes exactly count x the recipe's materials from the stock, gives count x the reward, counts it, and
    /// refuses short materials, a full bag, a locked recipe.
    #[test]
    fn synthesis_consumes_exactly_the_recipe() {
        let t = tables();
        let mut bag = Bag::default();
        let mut ws = Workshop::default();
        let recipe = t.recipe("IS_1").unwrap(); // Fire Bangle: Mythril Shard 3, Blazing Stone 2, Blazing Shard 5
        assert_eq!(synthesize(&mut ws, &mut bag, "IS_1", 1), Err(SynthRefusal::Locked));
        ws.goals.push("IC_1".into());
        for (k, n) in &recipe.materials {
            ws.stock.insert(k.clone(), n * 2);
        }
        ws.stock.insert("MAT_ITEM45".into(), 7);
        assert_eq!(synth_max(&ws, &bag, recipe), 2);
        assert_eq!(synthesize(&mut ws, &mut bag, "IS_1", 3), Err(SynthRefusal::LackMaterials));
        assert_eq!(synthesize(&mut ws, &mut bag, "IS_1", 0), Err(SynthRefusal::Zero));
        let done = synthesize(&mut ws, &mut bag, "IS_1", 2).unwrap();
        assert_eq!((done.reward.as_str(), done.count, done.first), ("PRT_ITEM11", 2, true));
        assert_eq!(bag.carried("PRT_ITEM11"), 2);
        assert!(recipe.materials.iter().all(|(k, _)| stock_of(&ws, k) == 0), "exactly the recipe's materials");
        assert_eq!(stock_of(&ws, "MAT_ITEM45"), 7, "nothing else is touched");
        assert_eq!((ws.created.get("IS_1").copied(), ws.items_created, log(&ws).created), (Some(2), 2, 1));
        assert_eq!(ws.total_obtained, 0, "a crafted armour is not a material gain");
        assert_eq!(synthesize(&mut ws, &mut bag, "IS_1", 1), Err(SynthRefusal::LackMaterials));
        // the bag's maximum
        bag.items.insert("PRT_ITEM11".into(), 99);
        for (k, n) in &recipe.materials {
            ws.stock.insert(k.clone(), *n);
        }
        assert_eq!(synth_max(&ws, &bag, recipe), 0);
        assert_eq!(synthesize(&mut ws, &mut bag, "IS_1", 1), Err(SynthRefusal::Full));
        assert_eq!(synthesize(&mut ws, &mut bag, "IS_999", 1), Err(SynthRefusal::NoSuchRecipe));
        // a material reward (IS_9 Mythril Shard from a Betwixt + a Twilight Shard), capped by max - held, counted obtained
        ws.goals.push("IC_3".into());
        ws.stock.insert("MAT_ITEM25".into(), 3);
        ws.stock.insert("MAT_ITEM29".into(), 3);
        bag.items.insert("MAT_ITEM33".into(), 98);
        assert_eq!(synth_max(&ws, &bag, t.recipe("IS_9").unwrap()), 1, "max - held");
        synthesize(&mut ws, &mut bag, "IS_9", 1).unwrap();
        // (the 3 Mythril Shards in the stock are the refused Fire Bangle's)
        assert_eq!((bag.carried("MAT_ITEM33"), stock_of(&ws, "MAT_ITEM33"), ws.obtained_of("MAT_ITEM33")), (99, 3, 1));
        // a material handed over past the bag's maximum (a goal's item) goes to the stock
        hand_over(&mut ws, &mut bag, "MAT_ITEM33", 2);
        assert_eq!((bag.carried("MAT_ITEM33"), stock_of(&ws, "MAT_ITEM33"), ws.obtained_of("MAT_ITEM33")), (99, 5, 3));
    }

    /// (KH3) ULTIMA WEAPON AT IC_24: 57 material kinds do not list it, the 58th does; it costs 7 Orichalcum+, 2 Wellspring,
    /// 2 Lucid and 2 Pulsing Crystals, arrives at +10, and is refused while held (one of each Keyblade).
    #[test]
    fn ultima_weapon_at_ic_24() {
        let t = tables();
        let ultima = t.recipe("IS_58").unwrap();
        let mut bag = Bag::default();
        let mut ws = Workshop::default();
        let keys = material_keys(|_| true);
        obtain(&mut ws, &keys[..57], 1);
        visit(&mut ws, &mut bag, &no_flags);
        assert!(!goal_done(&ws, "IC_24") && !recipe_listed(&ws, ultima));
        assert!(goal_done(&ws, "IC_23"), "55 kinds");
        count_gain(&mut ws, &keys[57], 1);
        let v = visit(&mut ws, &mut bag, &no_flags);
        assert!(v.goals.contains(&"IC_24".to_string()) && recipe_listed(&ws, ultima));
        assert_eq!(ultima.materials, [("MAT_ITEM57", 7), ("MAT_ITEM48", 2), ("MAT_ITEM16", 2), ("MAT_ITEM20", 2)].map(|(k, n)| (k.to_string(), n)));
        let fill = |ws: &mut Workshop| {
            for (k, n) in &ultima.materials {
                ws.stock.insert(k.clone(), *n);
            }
        };
        fill(&mut ws);
        assert_eq!(synthesize(&mut ws, &mut bag, "IS_58", 2), Err(SynthRefusal::Full), "one at most");
        synthesize(&mut ws, &mut bag, "IS_58", 1).unwrap();
        assert_eq!(bag.keyblade_level("WEP_KEYBLADE_SO_14"), Some(10), "Ultima Weapon has every level");
        assert!(ultima.materials.iter().all(|(k, _)| stock_of(&ws, k) == 0));
        fill(&mut ws);
        assert_eq!(synth_max(&ws, &bag, ultima), 0);
        assert_eq!(synthesize(&mut ws, &mut bag, "IS_58", 1), Err(SynthRefusal::KeybladeHeld));
        assert_eq!(forge_check(&ws, &bag, "WEP_KEYBLADE_SO_14"), Err(ForgeRefusal::MaxLevel));
    }

    /// EVERY FORGE LEVEL with exactly its materials (KH3's rows, sheets/port/kh3-keyblade-levels.json): one short is
    /// refused, the exact set is spent to nothing, the level rises by one, and the stats are KH3's cumulative ones; a
    /// Keyblade not owned is not offered; no munny anywhere.
    #[test]
    fn every_forge_level_with_exact_materials() {
        let kh3: Value = serde_json::from_str(include_str!("../../../sheets/port/kh3-keyblade-levels.json")).unwrap();
        let col = |name: &str| kh3["columns"].as_array().unwrap().iter().position(|c| c.as_str() == Some(name)).unwrap();
        let (id, strength, magic, munny, materials, counts) = (col("id"), col("strength"), col("magic"), col("munny"), col("materials"), col("counts"));
        let row = |key: &str, level: u8| kh3["rows"].as_array().unwrap().iter().find(|r| r[id].as_str() == Some(&format!("{key}.lv{level}"))).unwrap().clone();
        let mut forged = 0;
        for k in &tables().keyblades {
            let mut bag = Bag::default();
            let mut ws = Workshop::default();
            assert_eq!(forge_check(&ws, &bag, &k.key), Err(ForgeRefusal::NotOwned));
            bag.give(&k.key, 1);
            assert_eq!(bag.keyblade_level(&k.key), Some(k.init));
            for level in k.init + 1..=k.max {
                let r = row(&k.key, level);
                assert_eq!(r[munny].as_u64(), Some(0));
                let need: Vec<(String, u32)> = r[materials].as_array().unwrap().iter().zip(r[counts].as_array().unwrap()).map(|(m, n)| (m.as_str().unwrap().to_string(), n.as_u64().unwrap() as u32)).collect();
                assert_eq!(forge_check(&ws, &bag, &k.key), Err(ForgeRefusal::LackMaterials), "{} +{level}", k.key);
                for (m, n) in &need {
                    ws.stock.insert(m.clone(), *n);
                }
                *ws.stock.get_mut(&need[0].0).unwrap() -= 1;
                assert_eq!(forge(&mut ws, &mut bag, &k.key), Err(ForgeRefusal::LackMaterials), "{} +{level}: one short", k.key);
                *ws.stock.get_mut(&need[0].0).unwrap() += 1;
                let done = forge(&mut ws, &mut bag, &k.key).unwrap();
                assert_eq!((done.level, bag.keyblade_level(&k.key)), (level, Some(level)));
                assert!(ws.stock.values().all(|n| *n == 0), "{} +{level}: exactly the level's materials", k.key);
                let stats = keyblade_stats(&k.key, level).unwrap();
                assert_eq!((i64::from(stats.strength), i64::from(stats.magic)), (r[strength].as_i64().unwrap(), r[magic].as_i64().unwrap()), "{} +{level}", k.key);
                forged += 1;
            }
            assert_eq!(forge_check(&ws, &bag, &k.key), Err(ForgeRefusal::MaxLevel));
            assert_eq!(forge_list(&ws, &bag).len(), 1);
        }
        // 170 rows less the init-achieved ones (Favorite Deputy / Ever After 1, Happy Gear / Crystal Snow / Grand Chef 2,
        // Hunny Spout / Nano Gear / Wheel of Fate 3, Ultima Weapon 10)
        assert_eq!(forged, 170 - (1 + 1 + 2 + 2 + 2 + 3 + 3 + 3 + 10));
        // the Kingdom Key's +4 adds Formchange Extender; the list shows the next level
        let mut bag = Bag::default();
        bag.give("WEP_KEYBLADE_SO_00", 1);
        bag.set_keyblade_level("WEP_KEYBLADE_SO_00", 4);
        assert!(keyblade_stats("WEP_KEYBLADE_SO_00", 4).unwrap().abilities.contains(&"FORM_TIME".to_string()));
        let list = forge_list(&Workshop::default(), &bag);
        assert_eq!((list[0].level, list[0].next.as_ref().map(|n| n.level), list[0].ready), (4, Some(5), false));
    }

    /// JOURNEY TO THE ERDTREE (the user 2026-11-07): all 20 listed from the start in the journey's order; each done at its
    /// boss's VERIFIED defeat flag on a visit (retroactively); the finale needs every shardbearer and the Elden Beast
    /// ("{0} / {1}") and gives the Petite Ribbon; recipes by tier; NEW until seen; every flag a verified boss row.
    #[test]
    fn photo_missions_are_er_milestones() {
        let t = tables();
        let mut ws = Workshop::default();
        let listed = |ws: &Workshop, flags: &dyn Fn(u32) -> bool| photo_list(ws, flags).into_iter().map(|p| p.id).collect::<Vec<_>>();
        // the journey, in order (a fresh save, no boss beaten): every mission listed, none done, all NEW
        let journey = [
            ("IM_12", "godrick"),
            ("IM_2", "rennala"),
            ("IM_17", "radahn"),
            ("IM_7", "mimic_tear"),
            ("IM_0", "rykard"),
            ("IM_16", "morgott"),
            ("IM_8", "fortissax"),
            ("IM_4", "mohg_lord"),
            ("IM_10", "fire_giant"),
            ("IM_9", "godskin_duo"),
            ("IM_5", "placidusax"),
            ("IM_3", "maliketh"),
            ("IM_1", "malenia"),
            ("IM_11", "hoarah_loux"),
            ("IM_14", "rellana"),
            ("IM_13", "messmer"),
            ("IM_15", "midra"),
            ("IM_18", "bayle"),
            ("IM_19", "elden_beast"),
        ];
        let fresh = photo_list(&ws, &no_flags);
        assert_eq!(fresh.len(), 20, "all 20 viewable from the start");
        assert!(fresh.iter().all(|p| !p.done && p.new));
        assert_eq!(listed(&ws, &no_flags), journey.iter().map(|j| j.0).chain(["IM_6"]).collect::<Vec<_>>());
        // each completes on its own verified flag, and only on it
        for (im, row) in journey {
            let flag = boss(row);
            assert_eq!(t.photo(im).unwrap().done, Milestone::Flag(flag), "{im} = boss.{row}");
            let mut ws = Workshop::default();
            let v = visit(&mut ws, &mut Bag::default(), &|f| f == flag);
            assert_eq!(v.photos, vec![im.to_string()], "{im}: {row}'s defeat");
            assert!(recipe_listed(&ws, t.recipe(&t.photo(im).unwrap().recipes[0]).unwrap()), "{im}: its recipe listed");
        }
        // the user's picks are in, the dropped ones are out
        let bosses: Vec<u32> = t.photos.iter().filter_map(|p| if let Milestone::Flag(f) = p.done { Some(f) } else { None }).collect();
        for row in ["godskin_duo", "placidusax", "malenia", "hoarah_loux", "fire_giant"] {
            assert!(bosses.contains(&boss(row)), "{row} in");
        }
        for row in ["leonine", "margit", "agheel", "ancestor_spirit", "regal_ancestor", "loretta_caria", "red_wolf", "borealis", "niall", "mohg_omen", "astel_stars", "draconic_tree_sentinel", "godfrey_shade", "consort_radahn"] {
            assert!(!bosses.contains(&boss(row)), "{row} out");
        }
        // the finale: every shardbearer + the Elden Beast, the Petite Ribbon, last
        let finale = t.photo("IM_6").unwrap();
        let mut need = t.great_rune_flags.clone();
        need.push(boss("elden_beast"));
        assert_eq!((&finale.done, finale.order, finale.recipes.as_slice()), (&Milestone::All(need.clone()), 20, ["IS_80".to_string()].as_slice()));
        let most: Vec<u32> = need.iter().take(7).copied().collect();
        let view = photo_list(&ws, &|f| most.contains(&f)).into_iter().find(|p| p.id == "IM_6").unwrap();
        assert_eq!((view.done, view.progress), (false, Some((7, 8))));
        let mut bag = Bag::default();
        assert!(visit(&mut ws, &mut bag, &|f| most.contains(&f)).photos.iter().all(|p| p != "IM_6"));
        assert!(visit(&mut ws, &mut bag, &|f| need.contains(&f)).photos.contains(&"IM_6".to_string()));
        assert!(finale.help.contains("{0} / {1}"));
        assert!(photo_list(&ws, &no_flags).iter().find(|p| p.id == "IM_12").is_some_and(|p| p.done && p.new));
        mark_photo_seen(&mut ws, "IM_12");
        assert!(photo_list(&ws, &no_flags).iter().find(|p| p.id == "IM_12").is_some_and(|p| !p.new));
        assert_eq!(log(&ws).photos, ws.photos.len() as u32);
        // recipes by tier: the n-th milestone gives the n-th recipe (KH3's mission recipes by material cost)
        let mut by_order: Vec<&Photo> = t.photos.iter().collect();
        by_order.sort_by_key(|p| p.order);
        let by_order: Vec<&str> = by_order.iter().map(|p| p.recipes[0].as_str()).collect();
        assert_eq!(by_order, ["IS_74", "IS_61", "IS_62", "IS_65", "IS_66", "IS_72", "IS_63", "IS_68", "IS_69", "IS_71", "IS_70", "IS_76", "IS_75", "IS_79", "IS_64", "IS_77", "IS_73", "IS_78", "IS_67", "IS_80"]);
        // every flag is a verified defeat flag of the boss rewards sheet
        let json: Value = serde_json::from_str(crate::bossrewards::SHEET).unwrap();
        let verified: Vec<u64> = json["rows"].as_array().unwrap().iter().filter(|r| r[1]["flag_conf"].as_str() == Some("verified")).filter_map(|r| r[1]["flag"].as_u64()).collect();
        for p in &t.photos {
            let flags = match &p.done {
                Milestone::Flag(f) => vec![*f],
                Milestone::All(list) => list.clone(),
                Milestone::Never => panic!("{}: no milestone", p.id),
            };
            assert!(flags.iter().all(|f| verified.contains(&u64::from(*f))), "{}", p.id);
            assert!((p.name.contains("kupo") || p.name_from.is_some()) && p.help.starts_with("Prove to the moogle that "), "{}", p.id);
            assert!(p.name.is_empty() == p.name_from.is_some(), "{}: a line of its own or the pack's", p.id);
        }
        let shop: Vec<u32> = t.shop_levels.iter().filter_map(|(_, s)| if let ShopStep::Flag(f) = s { Some(*f) } else { None }).collect();
        assert!(shop.iter().chain(&t.great_rune_flags).all(|f| verified.contains(&u64::from(*f))));
    }

    /// A SAVE FROM THE FIRST MILESTONE LIST (rev 1, 2026-11-03) on its first visit: a kept boss's completion stays even with
    /// its flag unread (New Game+); a replaced row's completion stays only if its new boss is beaten; the changed rows show NEW
    /// again; then it is never re-checked (rev 2 stored), and a beaten boss counts retroactively.
    #[test]
    fn a_rev1_save_is_brought_over_once() {
        let t = tables();
        assert_eq!(t.milestone_rev, 2);
        // rev 1: Godrick (IM_12, kept), Margit (IM_9, now the Godskin Duo), Agheel (IM_7, now the Mimic Tear), the seven (IM_6)
        let mut ws = Workshop { photos: ["IM_12", "IM_9", "IM_7", "IM_6"].map(String::from).to_vec(), ext: json!({ "photos_seen": ["IM_12", "IM_9", "IM_7"] }), ..Workshop::default() };
        let mimic = boss("mimic_tear");
        let flags = move |f: u32| f == mimic;
        let v = visit(&mut ws, &mut Bag::default(), &flags);
        assert_eq!(ws.photos, ["IM_12", "IM_7"], "Godrick kept; the Duo / the finale not beaten: dropped; the Mimic Tear beaten: kept");
        assert!(v.photos.is_empty(), "nothing newly done");
        assert_eq!(ws.ext["photos_seen"], json!(["IM_12"]), "the changed rows are NEW again");
        assert_eq!(ws.ext["milestone_rev"], json!(2));
        assert!(!migrate_milestones(&mut ws, &no_flags), "once");
        assert_eq!(ws.photos, ["IM_12", "IM_7"]);
        // a boss beaten long ago counts at the next visit (retroactively)
        let duo = boss("godskin_duo");
        assert_eq!(visit(&mut ws, &mut Bag::default(), &|f| f == duo).photos, ["IM_9"]);
        // a fresh save is stamped on its first visit and loses nothing
        let mut fresh = Workshop::default();
        visit(&mut fresh, &mut Bag::default(), &no_flags);
        assert_eq!((fresh.photos.len(), fresh.ext["milestone_rev"].clone()), (0, json!(2)));
    }

    /// The Material List's "? ? ?", the SYNTH LOG, and the `ext` lists kept through P1's save block.
    #[test]
    fn material_list_and_log() {
        let mut ws = Workshop::default();
        let list = material_list(&ws);
        assert_eq!(list.len(), 60);
        assert!(list.iter().all(|m| m.name == "? ? ?" && !m.known));
        count_gain(&mut ws, "MAT_ITEM05", 2);
        assert!(material_list(&ws).iter().any(|m| m.key == "MAT_ITEM05" && m.name == "Frost Shard" && m.known));
        let l = log(&ws);
        assert_eq!((l.created, l.recipes, l.photos, l.photo_total, l.goals, l.goal_total), (0, 88, 0, 20, 0, 78));
        assert_eq!(l.complete(), [false; 3]);
        let full = SynthLog { created: 88, photos: 20, goals: 78, ..l };
        assert_eq!(full.complete(), [true; 3]);
        mark_photo_seen(&mut ws, "IM_7");
        mark_photo_seen(&mut ws, "IM_7");
        assert_eq!(ws.ext, json!({ "photos_seen": ["IM_7"] }));
    }

    /// [`with_live`]: the running record with the camp menu's stock: the visit asks the HUD for the carried materials,
    /// the Forge raises the running Keyblade's level, and a material handed over reaches the door's queue counted once.
    #[test]
    fn the_live_host_uses_the_running_record() {
        let _door = crate::items::TEST_DOOR.lock().unwrap_or_else(|e| e.into_inner());
        crate::weapons::begin_new_game();
        crate::items::take_given();
        crate::items::take_requests();
        crate::items::publish_held(vec![("MAT_ITEM53".to_string(), 1), ("MAT_ITEM45".to_string(), 2)]);
        crate::items::on_material_gain("MAT_ITEM53", 1); // as the door counted them when they came
        crate::items::on_material_gain("MAT_ITEM45", 2);
        let level = with_live(|ws, host| {
            visit(ws, host, &no_flags);
            assert_eq!((stock_of(ws, "MAT_ITEM53"), host.carried("MAT_ITEM53")), (1, 0), "deposited");
            forge(ws, host, crate::weapons::KINGDOM_KEY).map(|f| f.level)
        });
        assert_eq!(level, Ok(1));
        assert_eq!(crate::weapons::level(crate::weapons::KINGDOM_KEY), Some(1), "the running record's Keyblade");
        let mut taken = crate::items::take_requests();
        taken.sort_unstable();
        assert_eq!(taken, vec![(0x72D, 2), (0x735, 1)], "the HUD is asked for the deposited materials");
        // IC_0 handed over its Ether through the door
        assert!(crate::items::take_given().contains(&(0x104, 1)));
        // a material handed over: onto the door's queue, counted once in the record
        crate::items::publish_held(Vec::new());
        with_live(|ws, host| hand_over(ws, host, "MAT_ITEM33", 2));
        assert_eq!(crate::items::take_given().into_iter().filter(|g| g.0 == 0x721).collect::<Vec<_>>(), vec![(0x721, 2)]);
        assert_eq!(crate::weapons::workshop().obtained_of("MAT_ITEM33"), 2);
        crate::weapons::begin_new_game();
    }
}
