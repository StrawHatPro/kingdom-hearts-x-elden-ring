//! (loot) ELDEN RING ITEM -> KH3 ITEM, AND THE LOT ROLL: the pure half of the loot replacement (no game access here;
//! `loot.rs` reads the game's params into these types). docs/sora-er-items.md is the text; every rule, pool, weight
//! and setting is a row of sheets/port/sora-er-items.json (read at run time, the compiled copy when the file is gone).
//!
//! THE COMBINED RARITY RULE. Elden Ring decides WHETHER and WHICH: a lot row is one weighted pick among its (up to
//! eight) slots by their base points; an empty slot is "nothing". KH3 decides THE LUCK: where Elden Ring multiplies
//! the points of its `enableLuck` slots by item discovery / 100, the points are multiplied by
//! `1 + 0.3 x LUCK_UP copies` (KH3's table spawn 0x141480420, rounded as there) on top of the base discovery of
//! the sheet (100: Sora has no arcane).
//!
//! THE VALUE MODEL (IMPL_ER28). An Elden Ring item gets POINTS: its rarity (the lot row's when that is set), where
//! it came from (`Source`: an enemy's drop < a fixed world pickup < a chest / a gift < a boss), how deep in the game
//! it was found (the map: Limgrave .. the DLC), its sell value, a somber (unique) armament. `tier = 1 + floor(points)`,
//! 1 ..= `tier.count`. KH3's equipment is ranked into the same tiers by a SCORE of its own numbers (Strength, Magic,
//! Defense, AP, resistances, abilities; weights in the sheet). An item Sora cannot use becomes a KH armour piece or
//! accessory OF ITS TIER, chosen by a stable hash of the Elden Ring id among the pieces whose kind fits (a staff -> a
//! Magic accessory, a shield -> armour, a fire piece -> fire, an Ash of War -> AP, a spirit ash -> a support ability):
//! the same Elden Ring item is always the same KH item. He never holds more than `equip.dup_max` of one piece: the
//! next of the pool is given, and `equip.exhausted` when the pool is used up.
//!
//! TWO GENERATIONS OF RULE ROWS: `r2.*` are this model's; `rule.*` are IMPL_ER25's and are only read when the sheet
//! has no `r2.*` row (the deployed build 4B6C2CFEF72E reads the same file and knows only `rule.*`).

use std::collections::HashMap;

/// The Elden Ring item tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cat {
    Weapon,
    Protector,
    Accessory,
    Goods,
    Gem,
}

impl Cat {
    pub fn name(self) -> &'static str {
        match self {
            Cat::Weapon => "weapon",
            Cat::Protector => "protector",
            Cat::Accessory => "accessory",
            Cat::Goods => "goods",
            Cat::Gem => "gem",
        }
    }

    pub fn from_name(name: &str) -> Option<Cat> {
        [Cat::Weapon, Cat::Protector, Cat::Accessory, Cat::Goods, Cat::Gem].into_iter().find(|c| c.name() == name)
    }
}

/// What using a goods item does, as far as the params tell (its `refId_default` SpEffect row).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Effect {
    #[default]
    None,
    Hp,
    Fp,
    Both,
    /// It gives runes (`SpEffectParam.soul`).
    Runes,
    /// A status cure. NOTHING SETS THIS YET (the cures are not recognisable from the fields read).
    Cure,
}

impl Effect {
    pub fn name(self) -> &'static str {
        match self {
            Effect::None => "none",
            Effect::Hp => "hp",
            Effect::Fp => "fp",
            Effect::Both => "both",
            Effect::Runes => "runes",
            Effect::Cure => "cure",
        }
    }

    /// From the use effect's row: negative HP / FP change = a restore (Elden Ring's convention, public documentation),
    /// a positive `soul` = runes.
    pub fn of_sp_effect(change_hp: f32, change_mp: f32, soul: i32) -> Effect {
        match (soul > 0, change_hp < 0.0, change_mp < 0.0) {
            (true, ..) => Effect::Runes,
            (_, true, true) => Effect::Both,
            (_, true, false) => Effect::Hp,
            (_, false, true) => Effect::Fp,
            _ => Effect::None,
        }
    }
}

/// The element a protector guards best against / an armament strikes with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Element {
    #[default]
    None,
    Magic,
    Fire,
    Lightning,
    Holy,
}

impl Element {
    pub fn name(self) -> &'static str {
        match self {
            Element::None => "none",
            Element::Magic => "magic",
            Element::Fire => "fire",
            Element::Lightning => "lightning",
            Element::Holy => "holy",
        }
    }

    /// The KH3 element in its place (KH3 has no "magic" and no light element on equipment: the choices of IMPL_ER25).
    pub fn kh_tag(self) -> Option<&'static str> {
        match self {
            Element::None => None,
            Element::Magic => Some("blizzard"),
            Element::Fire => Some("fire"),
            Element::Lightning => Some("thunder"),
            Element::Holy => Some("dark"),
        }
    }

    /// From the four elemental damage cut rates (1 = no protection, lower = better): the lowest, when it is lower
    /// than the mean of the other three by at least `margin`.
    pub fn of_cut_rates(magic: f32, fire: f32, lightning: f32, holy: f32, margin: f32) -> Element {
        let all = [(Element::Magic, magic), (Element::Fire, fire), (Element::Lightning, lightning), (Element::Holy, holy)];
        let best = all.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        let others = (magic + fire + lightning + holy - best.1) / 3.0;
        if others - best.1 >= margin {
            best.0
        } else {
            Element::None
        }
    }

    /// From an armament's base attack values: the largest elemental one (none when it has none).
    pub fn of_attack(magic: u16, fire: u16, lightning: u16, holy: u16) -> Element {
        let all = [(Element::Magic, magic), (Element::Fire, fire), (Element::Lightning, lightning), (Element::Holy, holy)];
        let best = all.iter().copied().max_by_key(|a| a.1).unwrap();
        if best.1 > 0 {
            best.0
        } else {
            Element::None
        }
    }
}

/// How an item reached the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Source {
    /// A killed enemy's repeatable drop.
    Enemy,
    /// A repeatable world pickup (a plant, a node: a map lot without an event flag).
    Gather,
    /// A fixed, one-time world pickup: an item on a placed corpse, on the ground (a flagged map lot).
    World,
    Chest,
    /// A gift or reward handed over by an event.
    Npc,
    Boss,
    Shop,
    #[default]
    Unknown,
}

impl Source {
    pub const ALL: [Source; 8] = [Source::Enemy, Source::Gather, Source::World, Source::Chest, Source::Npc, Source::Boss, Source::Shop, Source::Unknown];

    pub fn name(self) -> &'static str {
        match self {
            Source::Enemy => "enemy",
            Source::Gather => "gather",
            Source::World => "world",
            Source::Chest => "chest",
            Source::Npc => "npc",
            Source::Boss => "boss",
            Source::Shop => "shop",
            Source::Unknown => "unknown",
        }
    }

    pub fn from_name(name: &str) -> Option<Source> {
        Source::ALL.into_iter().find(|s| s.name() == name)
    }
}

/// What kind of armament a weapon row is (`EquipParamWeapon.wepType` through the sheet row `weapon.kinds`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WepKind {
    #[default]
    None,
    Melee,
    Staff,
    Seal,
    Shield,
    Bow,
    Ammo,
}

impl WepKind {
    pub fn name(self) -> &'static str {
        match self {
            WepKind::None => "none",
            WepKind::Melee => "melee",
            WepKind::Staff => "staff",
            WepKind::Seal => "seal",
            WepKind::Shield => "shield",
            WepKind::Bow => "bow",
            WepKind::Ammo => "ammo",
        }
    }

    pub fn from_name(name: &str) -> Option<WepKind> {
        [WepKind::Melee, WepKind::Staff, WepKind::Seal, WepKind::Shield, WepKind::Bow, WepKind::Ammo].into_iter().find(|k| k.name() == name)
    }
}

/// An Elden Ring item as the params describe it.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ErItem {
    pub cat: Option<Cat>,
    /// The param row id (no category bits).
    pub id: u32,
    /// `EquipParamGoods.goodsType` (goods only).
    pub goods_type: u8,
    /// `rarity` of its row, 0 .. 3; replaced by the lot row's when that is not negative.
    pub rarity: u8,
    pub sort_group: u8,
    /// `isConsume` (goods only).
    pub consumable: bool,
    pub sell_value: i32,
    pub effect: Effect,
    /// Runes its use gives (`Effect::Runes`).
    pub soul: i32,
    pub element: Element,
    /// Weapons only.
    pub wep_kind: WepKind,
    /// A weapon reinforced with somber stones (its `materialSetId` is in the sheet row `weapon.somber_material_sets`).
    pub somber: bool,
    /// False: the row was not found in the game's params (nothing but the id is known).
    pub known: bool,
}

impl ErItem {
    /// The id without what does not change the item: a weapon's affinity and reinforcement level (the last four
    /// digits of its id).
    pub fn base_id(&self) -> u32 {
        if self.cat == Some(Cat::Weapon) {
            self.id / 10000 * 10000
        } else {
            self.id
        }
    }
}

/// What the rules may ask of the moment: where the item came from, how deep in the game, and what Sora holds
/// (`state(KH item key)` = copies in stock and worn; `state("@acc_slots")` / `state("@def_slots")` = the slot
/// bonuses he already has).
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub source: Source,
    /// 0 Limgrave .. 4 the DLC (sheet rows `region.*`).
    pub depth: u8,
    pub state: &'a dyn Fn(&str) -> u32,
    /// A random number in [0, 1) for the `chance:` gives of this one item (0.999: every chance fails, the last
    /// alternative of the give text decides: the reading of a shop line, of a log-only examination).
    pub unit: f32,
}

fn holds_nothing(_: &str) -> u32 {
    0
}

impl Ctx<'static> {
    pub fn of(source: Source, depth: u8) -> Ctx<'static> {
        Ctx { source, depth, state: &holds_nothing, unit: 0.999 }
    }
}

/// What Sora gets for an Elden Ring item.
#[derive(Clone, Debug, PartialEq)]
pub enum Give {
    /// It stays Elden Ring's: untouched, with the game's own popup.
    Pass,
    /// A KH3 item (item id of sheets/port/sora-item-table.json).
    Kh { item_id: u32, key: String },
    Munny(i32),
    /// Something of the reward seam (`kh_core::status::Reward`): "acc_slot", "def_slot", "max_mp:5", "max_hp:10", "ap:1".
    Reward(String),
    /// It vanishes.
    Nothing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub give: Give,
    /// The row that decided ("r2.200.heal_hp", "id.goods.10010", "unknown").
    pub rule: String,
    /// The value tier 1 .. `tier.count` (0: the item passes).
    pub tier: u8,
    /// How the tier came about, for the log.
    pub why: String,
    /// The rule gives one KH item for the whole stack.
    pub one: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
struct Rule {
    id: String,
    cat: Vec<Cat>,
    goods_type: Vec<u8>,
    rarity: Vec<u8>,
    effect: Vec<String>,
    consumable: Option<bool>,
    ids: Vec<(u32, u32)>,
    source: Vec<Source>,
    tier: Vec<u8>,
    wep_kind: Vec<WepKind>,
    /// A proposal row: skipped while `progression.convert` is 0.
    proposal: bool,
    as_tier: Option<u8>,
    tier_min: Option<u8>,
    one: bool,
    /// (PU) `"any_source": true`: the rule's give stands from every source (an allowance of the user's: Golden Seed /
    /// Sacred Tear -> Megalixir); without it [`LootRules::restrict`] applies.
    any_source: bool,
    give: String,
}

/// A KH3 item with the numbers the ranking reads.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct KhItem {
    pub key: String,
    pub item_id: u32,
    /// "battle", "camp", "armour", "accessory", "material".
    pub class: &'static str,
    pub family: String,
    pub grade: u8,
    /// KH3's shop price (0: not sold).
    pub buy: u32,
    pub attack: i32,
    pub magic: i32,
    pub defense: i32,
    pub ap: i32,
    /// Percent by `ETresDamageAttribute` (1 fire, 2 blizzard, 3 thunder, 4 water, 5 aero, 6 dark).
    pub resist: [i32; 8],
    pub abilities: Vec<String>,
}

/// Every KH3 item of the item table with its equipment row.
pub fn kh_items() -> Vec<KhItem> {
    use kh_host::items::Class;
    kh_host::items::table()
        .iter()
        .map(|i| {
            let mut out = KhItem { key: i.key.clone(), item_id: i.item_id, family: i.family.clone(), grade: i.grade, buy: i.buy, ..KhItem::default() };
            out.class = match i.class {
                Class::Battle => "battle",
                Class::Camp => "camp",
                Class::Armour => "armour",
                Class::Accessory => "accessory",
                Class::Material => "material",
            };
            if let Some(row) = kh_core::status::tables::equipment_row(&i.key) {
                (out.attack, out.magic, out.defense, out.ap) = (row.attack, row.magic, row.defense, row.ap);
                for (k, r) in row.resist.iter().take(8).enumerate() {
                    out.resist[k] = *r;
                }
                out.abilities = row.abilities.iter().map(|a| a.to_string()).collect();
            }
            out
        })
        .collect()
}

/// A KH3 armour piece or accessory as the model ranks it.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    pub key: String,
    pub item_id: u32,
    pub armour: bool,
    pub score: f32,
    pub tier: u8,
    /// "strength", "magic", "balanced", "ap", "support", "defence", "all", "fire", "blizzard", "thunder", "water", "aero", "dark".
    pub tags: Vec<String>,
}

/// A merchant's line as `shop.kh` sells it.
#[derive(Clone, Debug, PartialEq)]
pub enum ShopLine {
    /// A key / progression item: the line stays Elden Ring's.
    Keep,
    Sell { item_id: u32, key: String, price: u32 },
    /// Nothing sensible to sell in its place (the text says why).
    Drop(String),
}

/// The sheet.
#[derive(Clone, Debug)]
pub struct LootRules {
    rules: Vec<Rule>,
    overrides: HashMap<(Cat, u32), String>,
    pools: HashMap<String, Vec<String>>,
    families: Vec<String>,
    /// KH item key -> item id; (family, grade) -> key.
    items: HashMap<String, u32>,
    materials: HashMap<(String, u8), String>,
    buy: HashMap<String, u32>,
    grades: HashMap<String, u8>,
    ranked: Vec<Ranked>,
    pub luck_per_copy: f32,
    pub base_discovery: f32,
    pub chain_max: u32,
    pub family_margin: f32,
    pub munny_per_sell: f32,
    lot_categories: HashMap<u32, Cat>,
    shop_categories: HashMap<u32, Cat>,
    // ---- the value model
    pub tier_count: u8,
    rarity_points: Vec<f32>,
    source_points: HashMap<Source, f32>,
    region_points: Vec<f32>,
    sell_points: Vec<(i32, f32)>,
    somber_points: f32,
    /// (IMPL_ER61) row `tier.equip_cap`: the highest tier of an armour piece or accessory by region depth (empty = none).
    equip_cap: Vec<u8>,
    pub dup_max: u32,
    exhausted: String,
    pub slot_max: u32,
    pub convert_progression: bool,
    wep_kinds: HashMap<u32, WepKind>,
    somber_sets: Vec<i32>,
    region_areas: HashMap<u32, u8>,
    region_boxes: Vec<[u32; 5]>,
    region_default: u8,
    callers: HashMap<u32, Source>,
    /// (PU) row `source.usable`: the sources that may give a usable item, equipment, an accessory or munny (`None`: the
    /// row is missing = every source, as before). Every other source gives a synthesis material only.
    usable_sources: Option<Vec<Source>>,
    /// (PU) KH item key -> its class ("battle", "camp", "armour", "accessory", "material").
    classes: HashMap<String, &'static str>,
    shop_tier_price: Vec<u32>,
    name_files: HashMap<Cat, Vec<u32>>,
    /// (IMPL_ER42) row `msg.texts`: the message files holding an item's DESCRIPTION (tried in order).
    text_files: HashMap<Cat, Vec<u32>>,
    /// (er30) The base consumable drop: rows `base.*`.
    pub base_on: bool,
    base_lines: Vec<BaseLine>,
    base_class_hp: Vec<i32>,
    base_class_level: Vec<u8>,
    /// 2: the rule rows in force are the `r2.*` generation; 1: the sheet has only IMPL_ER25's `rule.*` rows.
    pub generation: u8,
    /// (ER party S6, U4) the sheet row `party.weapons`: which armaments become a friend's weapon.
    #[cfg(feature = "party")]
    pub party_weapons: Option<PartyWeapons>,
    /// Rows that were not understood.
    pub problems: Vec<String>,
}

/// The compiled copy of the sheet.
pub const SHEET: &str = include_str!("../../../sheets/port/sora-er-items.json");

fn stable_pick(id: u32, count: usize) -> usize {
    if count == 0 {
        0
    } else {
        id as usize % count
    }
}

/// FNV-1a of the table and the id: the stable choice inside a pool (weapon ids are multiples of 10000: a plain
/// remainder would always land on the same few).
fn stable_hash(cat: Option<Cat>, id: u32) -> usize {
    let mut h: u32 = 0x811C_9DC5;
    let tag = cat.map_or(9u8, |c| c as u8);
    for b in std::iter::once(tag).chain(id.to_le_bytes()) {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    (h >> 8) as usize
}

/// `chance:<percent>:<give>` -> (percent, give).
fn chance_of(alt: &str) -> Option<(f32, &str)> {
    let (percent, rest) = alt.strip_prefix("chance:")?.split_once(':')?;
    Some((percent.parse::<f32>().ok()?, rest))
}

/// (er30) One line of the BASE CONSUMABLE DROP: an item line by level and a rate in percent by enemy class.
#[derive(Clone, Debug, PartialEq)]
struct BaseLine {
    name: String,
    /// KH item keys by level (0 = Limgrave's ordinary enemy).
    line: Vec<String>,
    /// Percent by class: ordinary, big, boss.
    rate: Vec<f32>,
}

/// A give text's outcome.
enum Res {
    /// Not understood.
    Bad,
    /// Understood, but nothing to give right now (no sell value, the slots are full, the pool is used up).
    Skip,
    Ok(Give),
}

impl LootRules {
    /// The sheet as it is on disk (the user edits it), else the compiled copy.
    pub fn load() -> LootRules {
        let path = kh_gendata::project_file("sheets/port/sora-er-items.json");
        let text = std::fs::read_to_string(path).unwrap_or_else(|_| SHEET.to_string());
        let items = kh_items();
        let rules = LootRules::parse(&text, &items);
        if rules.rules.is_empty() {
            return LootRules::parse(SHEET, &items);
        }
        rules
    }

    pub fn parse(text: &str, items: &[KhItem]) -> LootRules {
        let mut out = LootRules {
            rules: Vec::new(),
            overrides: HashMap::new(),
            pools: HashMap::new(),
            families: Vec::new(),
            items: HashMap::new(),
            materials: HashMap::new(),
            buy: HashMap::new(),
            grades: HashMap::new(),
            ranked: Vec::new(),
            luck_per_copy: 0.3,
            base_discovery: 100.0,
            chain_max: 16,
            family_margin: 0.02,
            munny_per_sell: 1.0,
            lot_categories: HashMap::new(),
            shop_categories: HashMap::new(),
            tier_count: 6,
            rarity_points: vec![0.0, 1.0, 2.0, 3.0],
            source_points: HashMap::new(),
            region_points: vec![0.0],
            sell_points: Vec::new(),
            somber_points: 0.0,
            equip_cap: Vec::new(),
            dup_max: 2,
            exhausted: "material|munny:sell".to_string(),
            slot_max: 2,
            convert_progression: true,
            wep_kinds: HashMap::new(),
            somber_sets: Vec::new(),
            region_areas: HashMap::new(),
            region_boxes: Vec::new(),
            region_default: 1,
            callers: HashMap::new(),
            usable_sources: None, // (PU)
            classes: HashMap::new(), // (PU)
            shop_tier_price: Vec::new(),
            name_files: HashMap::new(),
            // (IMPL_ER42) without the sheet row: GoodsCaption (the long text) 24, its DLC file 321, then GoodsInfo (the
            // short line) 20, its DLC file 320. FROM MEMORY of the public FMG lists (10 = GoodsName is proven by the log).
            text_files: HashMap::from([(Cat::Goods, vec![24, 321, 20, 320])]),
            base_on: false,
            base_lines: Vec::new(),
            base_class_hp: vec![700, 2500],
            base_class_level: vec![0, 1, 2],
            generation: 1,
            #[cfg(feature = "party")]
            party_weapons: None, // (ER party S6)
            problems: Vec::new(),
        };
        for i in items {
            out.items.insert(i.key.clone(), i.item_id);
            out.buy.insert(i.key.clone(), i.buy);
            out.grades.insert(i.key.clone(), i.grade);
            out.classes.insert(i.key.clone(), i.class); // (PU)
            if i.family != "-" && i.grade > 0 {
                out.materials.insert((i.family.clone(), i.grade), i.key.clone());
            }
        }
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
            out.problems.push("the sheet is not JSON".to_string());
            return out;
        };
        let strings = |v: &serde_json::Value| v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>()).unwrap_or_default();
        let numbers = |v: &serde_json::Value| v.as_array().map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u8)).collect::<Vec<_>>()).unwrap_or_default();
        let floats = |v: &serde_json::Value| v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64().map(|n| n as f32)).collect::<Vec<_>>()).unwrap_or_default();
        let hex = |k: &str| k.strip_prefix("0x").or_else(|| k.strip_prefix("0X")).map_or_else(|| k.parse::<u32>().ok(), |h| u32::from_str_radix(h, 16).ok());
        let map_f32 = |value: &serde_json::Value, into: &mut HashMap<String, f32>| {
            for (k, v) in value.as_object().into_iter().flatten() {
                if let Some(v) = v.as_f64() {
                    into.insert(k.clone(), v as f32);
                }
            }
        };
        // ---- the ranking's settings (collected first: the ranking is made after every row is read)
        let mut weights: HashMap<String, f32> = HashMap::new();
        let mut ability_scores: HashMap<String, f32> = HashMap::new();
        let mut support: Vec<String> = Vec::new();
        let (mut acc_min, mut arm_min): (Vec<f32>, Vec<f32>) = (Vec::new(), Vec::new());
        let mut tier_over: HashMap<String, u8> = HashMap::new();
        let mut excluded: Vec<String> = Vec::new();
        let (mut rules_1, mut rules_2) = (Vec::new(), Vec::new());
        for row in json["rows"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            let (Some(id), value) = (row[0].as_str(), &row[1]) else { continue };
            let number = value.as_f64().map(|v| v as f32);
            match id {
                "luck.per_copy" => out.luck_per_copy = number.unwrap_or(out.luck_per_copy),
                "lot.base_discovery" => out.base_discovery = number.unwrap_or(out.base_discovery),
                "lot.chain_max" => out.chain_max = number.map_or(out.chain_max, |v| v as u32),
                "armour.family_margin" => out.family_margin = number.unwrap_or(out.family_margin),
                "nothing.munny_per_sell_value" => out.munny_per_sell = number.unwrap_or(out.munny_per_sell),
                "material.families" => out.families = strings(value),
                "lot.categories" | "shop.categories" => {
                    for (k, v) in value.as_object().into_iter().flatten() {
                        if let (Ok(k), Some(cat)) = (k.parse::<u32>(), v.as_str().and_then(Cat::from_name)) {
                            if id == "lot.categories" { &mut out.lot_categories } else { &mut out.shop_categories }.insert(k, cat);
                        }
                    }
                }
                "tier.count" => out.tier_count = number.map_or(out.tier_count, |v| (v as u8).clamp(1, 9)),
                "tier.rarity_points" => out.rarity_points = floats(value),
                "tier.region_points" => out.region_points = floats(value),
                "tier.somber_points" => out.somber_points = number.unwrap_or(0.0),
                "tier.equip_cap" => out.equip_cap = numbers(value).into_iter().map(|t| t.max(1)).collect(), // (IMPL_ER61)
                "tier.source_points" => {
                    for (k, v) in value.as_object().into_iter().flatten() {
                        match (Source::from_name(k), v.as_f64()) {
                            (Some(s), Some(v)) => drop(out.source_points.insert(s, v as f32)),
                            _ => out.problems.push(format!("{id}: no source {k}")),
                        }
                    }
                }
                "tier.sell_points" => {
                    out.sell_points = value.as_array().map(|a| a.iter().filter_map(|p| Some((p[0].as_i64()? as i32, p[1].as_f64()? as f32))).collect()).unwrap_or_default();
                    out.sell_points.sort_by_key(|p| p.0);
                }
                "tier.accessory_min" => acc_min = floats(value),
                "tier.armour_min" => arm_min = floats(value),
                "score.weights" => map_f32(value, &mut weights),
                "score.abilities" => map_f32(value, &mut ability_scores),
                "affinity.support" => support = strings(value),
                "pool.exclude" => excluded = strings(value),
                "equip.dup_max" => out.dup_max = number.map_or(out.dup_max, |v| (v as u32).max(1)),
                "equip.exhausted" => out.exhausted = value.as_str().unwrap_or(&out.exhausted).to_string(),
                "reward.slot_max" => out.slot_max = number.map_or(out.slot_max, |v| v as u32),
                // (ER party S6, U4) the friends' weapons in the loot
                #[cfg(feature = "party")]
                "party.weapons" => match PartyWeapons::parse(value) {
                    Ok(p) => out.party_weapons = Some(p),
                    Err(e) => out.problems.push(e),
                },
                "progression.convert" => out.convert_progression = number.is_none_or(|v| v != 0.0),
                "weapon.kinds" => {
                    for (k, v) in value.as_object().into_iter().flatten() {
                        match WepKind::from_name(k) {
                            Some(kind) => {
                                for t in v.as_array().into_iter().flatten().filter_map(|x| x.as_u64()) {
                                    out.wep_kinds.insert(t as u32, kind);
                                }
                            }
                            None => out.problems.push(format!("{id}: no weapon kind {k}")),
                        }
                    }
                }
                "weapon.somber_material_sets" => out.somber_sets = value.as_array().map(|a| a.iter().filter_map(|x| x.as_i64().map(|n| n as i32)).collect()).unwrap_or_default(),
                "region.areas" => {
                    for (k, v) in value.as_object().into_iter().flatten() {
                        if let (Ok(k), Some(v)) = (k.parse::<u32>(), v.as_u64()) {
                            out.region_areas.insert(k, v as u8);
                        }
                    }
                }
                "region.overworld" => out.region_boxes = value.as_array().map(|a| a.iter().filter_map(|b| Some([b[0].as_u64()? as u32, b[1].as_u64()? as u32, b[2].as_u64()? as u32, b[3].as_u64()? as u32, b[4].as_u64()? as u32])).collect()).unwrap_or_default(),
                "region.default" => out.region_default = number.map_or(1, |v| v as u8),
                // (PU) the user, 2026-10-18: usable items, equipment and accessories only from enemies, corpses and chests
                "source.usable" => {
                    let mut list = Vec::new();
                    for name in strings(value) {
                        match Source::from_name(&name) {
                            Some(s) => list.push(s),
                            None => out.problems.push(format!("{id}: no source {name}")),
                        }
                    }
                    out.usable_sources = Some(list);
                }
                "gate.callers" => {
                    for (k, v) in value.as_object().into_iter().flatten() {
                        match (hex(k), v.as_str().and_then(Source::from_name)) {
                            (Some(rva), Some(s)) => drop(out.callers.insert(rva, s)),
                            _ => out.problems.push(format!("{id}: {k} is not an address = a source name")),
                        }
                    }
                }
                "msg.names" => {
                    for (k, v) in value.as_object().into_iter().flatten() {
                        if let Some(cat) = Cat::from_name(k) {
                            out.name_files.insert(cat, v.as_array().into_iter().flatten().filter_map(|x| x.as_u64().map(|n| n as u32)).collect());
                        }
                    }
                }
                "msg.texts" => {
                    for (k, v) in value.as_object().into_iter().flatten() {
                        match Cat::from_name(k) {
                            Some(cat) => drop(out.text_files.insert(cat, v.as_array().into_iter().flatten().filter_map(|x| x.as_u64().map(|n| n as u32)).collect())),
                            None => out.problems.push(format!("{id}: no table {k}")),
                        }
                    }
                }
                "base.on" => out.base_on = number.is_some_and(|v| v != 0.0),
                "base.class_hp" => out.base_class_hp = value.as_array().map(|a| a.iter().filter_map(|x| x.as_i64().map(|n| n as i32)).collect()).unwrap_or_default(),
                "base.class_level" => out.base_class_level = numbers(value),
                _ if id.starts_with("base.drop.") => {
                    let line = BaseLine { name: id["base.drop.".len()..].to_string(), line: strings(&value["line"]), rate: floats(&value["rate"]) };
                    for key in &line.line {
                        if !out.items.contains_key(key) {
                            out.problems.push(format!("{id}: no KH3 item {key}"));
                        }
                    }
                    if line.line.is_empty() || line.rate.is_empty() {
                        out.problems.push(format!("{id}: needs \"line\" (item keys by level) and \"rate\" (percent by class)"));
                    } else {
                        out.base_lines.push(line);
                    }
                }
                "shop.tier_price" => out.shop_tier_price = value.as_array().map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u32)).collect()).unwrap_or_default(),
                _ if id.starts_with("kh.tier.") => {
                    if let Some(t) = number {
                        tier_over.insert(id["kh.tier.".len()..].to_string(), t as u8);
                    }
                }
                _ if id.starts_with("pool.") => {
                    let keys = strings(value);
                    for key in &keys {
                        if !out.items.contains_key(key) {
                            out.problems.push(format!("{id}: no KH3 item {key}"));
                        }
                    }
                    out.pools.insert(id["pool.".len()..].to_string(), keys);
                }
                _ if id.starts_with("rule.") || id.starts_with("r2.") => {
                    let Some(give) = value["give"].as_str() else {
                        out.problems.push(format!("{id}: no give"));
                        continue;
                    };
                    let ids = value["ids"].as_array().map(|a| a.iter().filter_map(|p| Some((p[0].as_u64()? as u32, p[1].as_u64()? as u32))).collect()).unwrap_or_default();
                    let mut rule = Rule { id: id.to_string(), cat: strings(&value["cat"]).iter().filter_map(|c| Cat::from_name(c)).collect(), goods_type: numbers(&value["goods_type"]), rarity: numbers(&value["rarity"]), effect: strings(&value["effect"]), consumable: value["consumable"].as_bool(), ids, give: give.to_string(), ..Rule::default() };
                    for s in strings(&value["source"]) {
                        match Source::from_name(&s) {
                            Some(s) => rule.source.push(s),
                            None => out.problems.push(format!("{id}: no source {s}")),
                        }
                    }
                    for k in strings(&value["wep_kind"]) {
                        match WepKind::from_name(&k) {
                            Some(k) => rule.wep_kind.push(k),
                            None => out.problems.push(format!("{id}: no weapon kind {k}")),
                        }
                    }
                    rule.tier = numbers(&value["tier"]);
                    rule.proposal = value["proposal"].as_bool() == Some(true);
                    rule.as_tier = value["as_tier"].as_u64().map(|n| n as u8);
                    rule.tier_min = value["tier_min"].as_u64().map(|n| n as u8);
                    rule.one = value["count"].as_str() == Some("one");
                    rule.any_source = value["any_source"].as_bool() == Some(true); // (PU)
                    if id.starts_with("r2.") { &mut rules_2 } else { &mut rules_1 }.push(rule);
                }
                _ if id.starts_with("id.") => {
                    let mut parts = id.splitn(3, '.');
                    let (_, cat, param) = (parts.next(), parts.next().and_then(Cat::from_name), parts.next().and_then(|p| p.parse::<u32>().ok()));
                    match (cat, param, value.as_str()) {
                        (Some(cat), Some(param), Some(give)) => {
                            out.overrides.insert((cat, param), give.to_string());
                        }
                        _ => out.problems.push(format!("{id}: not id.<table>.<param id> = a give text")),
                    }
                }
                _ => {}
            }
        }
        out.generation = if rules_2.is_empty() { 1 } else { 2 };
        out.rules = if rules_2.is_empty() { rules_1 } else { rules_2 };
        out.rules.sort_by(|a, b| a.id.cmp(&b.id));
        // ---- KH3's equipment ranked into the tiers
        let w = |name: &str, default: f32| weights.get(name).copied().unwrap_or(default);
        let elements = [(1usize, "fire"), (2, "blizzard"), (3, "thunder"), (4, "water"), (5, "aero"), (6, "dark")];
        for i in items.iter().filter(|i| (i.class == "armour" || i.class == "accessory") && !excluded.contains(&i.key)) {
            let armour = i.class == "armour";
            let resist: i32 = i.resist.iter().sum();
            let abilities: f32 = i.abilities.iter().map(|a| ability_scores.get(a).copied().unwrap_or(w("ability", 8.0))).sum();
            let score = i.attack as f32 * w("attack", 10.0) + i.magic as f32 * w("magic", 10.0) + i.defense as f32 * w("defense", 10.0) + i.ap as f32 * w("ap", 1.0) + resist as f32 * w("resist", 0.25) + abilities;
            let mins = if armour { &arm_min } else { &acc_min };
            let by_score = mins.iter().rposition(|m| score >= *m).map_or(1, |k| k as u8 + 1);
            let tier = tier_over.get(&i.key).copied().unwrap_or(by_score).clamp(1, out.tier_count);
            let mut tags: Vec<String> = Vec::new();
            let mut tag = |t: &str| {
                if !tags.iter().any(|x| x == t) {
                    tags.push(t.to_string());
                }
            };
            if i.attack > i.magic {
                tag("strength");
            }
            if i.magic > i.attack {
                tag("magic");
            }
            if i.attack > 0 && i.attack == i.magic {
                tag("balanced");
            }
            if armour {
                match elements.iter().filter(|(k, _)| i.resist[*k] > 0).count() {
                    0 if i.defense > 0 => tag("defence"),
                    0 => {}
                    // (the element it guards best against; a Rosette's second, smaller one does not count)
                    1 | 2 => tag(elements.iter().rev().max_by_key(|(k, _)| i.resist[*k]).unwrap().1),
                    _ => tag("all"),
                }
            } else {
                if i.ap >= 20 {
                    tag("ap");
                }
                if i.abilities.iter().any(|a| support.contains(a)) {
                    tag("support");
                }
                for a in &i.abilities {
                    for (needle, name) in [("FIRE", "fire"), ("FIRA", "fire"), ("BLIZZA", "blizzard"), ("THUND", "thunder"), ("WATER", "water"), ("WATA", "water"), ("AERO", "aero"), ("DARK", "dark")] {
                        if a.contains(needle) {
                            tag(name);
                        }
                    }
                }
            }
            out.ranked.push(Ranked { key: i.key.clone(), item_id: i.item_id, armour, score, tier, tags });
        }
        // ---- every give text must be understood
        let mut gives: Vec<(String, String)> = out.rules.iter().map(|r| (r.id.clone(), r.give.clone())).chain(out.overrides.iter().map(|(k, g)| (format!("id.{}.{}", k.0.name(), k.1), g.clone()))).collect();
        gives.push(("equip.exhausted".to_string(), out.exhausted.clone()));
        let probe = ErItem { cat: Some(Cat::Goods), id: 1, known: true, ..ErItem::default() };
        for (id, give) in gives {
            if give.split('|').any(|alt| matches!(out.resolve_one(chance_of(alt.trim()).map_or(alt.trim(), |c| c.1), &probe, &Ctx::of(Source::Unknown, 0), 1), Res::Bad)) {
                out.problems.push(format!("{id}: give {give:?} is not understood"));
            }
        }
        out
    }

    pub fn rule_count(&self) -> (usize, usize) {
        (self.rules.len(), self.overrides.len())
    }

    /// `lotItemCategory` of a lot slot -> the table.
    pub fn lot_category(&self, raw: i32) -> Option<Cat> {
        self.lot_categories.get(&(raw as u32)).copied()
    }

    /// `ShopLineupParam.equipType` -> the table.
    pub fn shop_category(&self, raw: u8) -> Option<Cat> {
        self.shop_categories.get(&(raw as u32)).copied()
    }

    /// `EquipParamWeapon.wepType` -> the kind of armament (everything the sheet does not name is a melee weapon).
    pub fn wep_kind(&self, wep_type: u16) -> WepKind {
        self.wep_kinds.get(&(wep_type as u32)).copied().unwrap_or(WepKind::Melee)
    }

    /// The message files (FMG categories of the game's `MsgRepository`) that hold the names of a table's items.
    pub fn name_files(&self, cat: Cat) -> &[u32] {
        self.name_files.get(&cat).map_or(&[], Vec::as_slice)
    }

    /// (IMPL_ER42) The message files that hold the description of a table's items, in the order they are tried.
    pub fn text_files(&self, cat: Cat) -> &[u32] {
        self.text_files.get(&cat).map_or(&[], Vec::as_slice)
    }

    pub fn is_somber(&self, material_set_id: i32) -> bool {
        self.somber_sets.contains(&material_set_id)
    }

    /// KH3's equipment as ranked (table order).
    pub fn ranked(&self) -> &[Ranked] {
        &self.ranked
    }

    /// HOW DEEP IN THE GAME a map block lies: 0 Limgrave .. 4 the DLC. `None`: a block the sheet does not place
    /// (a cave, a catacomb: the caller keeps the depth of the last block it knew).
    pub fn depth_of_block(&self, block: i32) -> Option<u8> {
        if block <= 0 {
            return None;
        }
        let (area, x, z) = ((block as u32 >> 24) & 0xFF, (block as u32 >> 16) & 0xFF, (block as u32 >> 8) & 0xFF);
        if area == 60 {
            return Some(self.region_boxes.iter().find(|b| (b[0]..=b[1]).contains(&x) && (b[2]..=b[3]).contains(&z)).map_or(self.region_default, |b| b[4] as u8));
        }
        self.region_areas.get(&area).copied()
    }

    /// The source the sheet's `gate.callers` row gives one of these return addresses (RVAs).
    pub fn caller_source(&self, rvas: &[u32]) -> Option<Source> {
        rvas.iter().find_map(|r| self.callers.get(r).copied())
    }

    /// WHERE A GATE ITEM CAME FROM, as far as the lot tables tell: `caller` (the sheet's reading of the call site)
    /// first; else an item only enemies drop is an enemy's, an item of a flagged map lot is a fixed world pickup, an
    /// item of unflagged map lots only is gathered. An item both enemies and the world hold counts as the enemy's
    /// (the lower value). The second value: the reading is a guess between two.
    pub fn source_of(&self, caller: Option<Source>, in_enemy: bool, in_map_flagged: bool, in_map_plain: bool) -> (Source, bool) {
        if let Some(s) = caller {
            return (s, false);
        }
        match (in_enemy, in_map_flagged, in_map_plain) {
            (true, false, false) => (Source::Enemy, false),
            // (PU) enemies AND the world hold it: which one gave it cannot be told: THE SAFE SIDE (the user: only
            // enemies, corpses and chests give usable items): the world's reading, a synthesis material only. (Was:
            // the enemy's, the lower value: the user's Rowa Fruit gathered in Limgrave gave an Ether that way.)
            (true, true, _) => (Source::World, true),
            (true, false, true) => (Source::Gather, true),
            (false, true, _) => (Source::World, false),
            (false, false, true) => (Source::Gather, false),
            (false, false, false) => (Source::Unknown, false),
        }
    }

    /// (PU) WHERE A GATE ITEM CAME FROM, every reading in its order: (1) `chest`: the body played a chest animation just
    /// before (chest.rs, MEASURED: the chest's item reaches the gate when 60110 ends) = a chest; (2) the call site
    /// (`gate.callers`); (3) `er_menu`: a window of Elden Ring's has the pad (a merchant's: a purchase) = a shop;
    /// (4) the lot tables ([`Self::source_of`]). The text says which reading decided (for the log).
    pub fn gate_source(&self, chest: bool, caller: Option<Source>, er_menu: bool, in_enemy: bool, in_map_flagged: bool, in_map_plain: bool) -> (Source, &'static str) {
        if chest {
            return (Source::Chest, " by the body's chest animation");
        }
        if let Some(s) = caller {
            return (s, " by the call site");
        }
        if er_menu {
            return (Source::Shop, " (an Elden Ring window has the pad: a purchase)");
        }
        match self.source_of(None, in_enemy, in_map_flagged, in_map_plain) {
            (s, true) => (s, " (enemy and world lots hold it: the safe side taken)"),
            (s, false) => (s, ""),
        }
    }

    /// (PU) May this source give a usable item, equipment, an accessory or munny (row `source.usable`; none = all)?
    pub fn usable_from(&self, source: Source) -> bool {
        self.usable_sources.as_ref().is_none_or(|l| l.contains(&source))
    }

    /// (PU, the user 2026-10-18: "anything that isn't a dead body or an enemy or chest should not drop usable items,
    /// equipment or accessories unless I've specifically allowed it"; plants and animals: "only crafting materials")
    /// A DECISION FROM A SOURCE NOT IN `source.usable`: a KH item that is not a synthesis material (a battle / camp item,
    /// armour, an accessory, a friend's weapon) and munny become A SYNTHESIS MATERIAL of the same tier (family by the
    /// item, as the `material` give); a pass, a reward (a slot, max HP / MP, AP: no item), nothing and a material stand.
    pub fn restrict(&self, d: Decision, item: &ErItem, ctx: &Ctx) -> Decision {
        if self.usable_from(ctx.source) {
            return d;
        }
        let restricted = match &d.give {
            Give::Kh { key, .. } => self.classes.get(key).copied() != Some("material"),
            Give::Munny(_) => true,
            _ => false,
        };
        if !restricted {
            return d;
        }
        let give = match self.material(item, d.tier.max(1)) {
            Res::Ok(g) => g,
            _ => Give::Nothing,
        };
        let was = match &d.give {
            Give::Kh { key, .. } => key.clone(),
            other => format!("{other:?}"),
        };
        Decision { give, rule: format!("{}+source.usable", d.rule), tier: d.tier.max(1), why: format!("{}; from source {} only a synthesis material (was {was})", d.why, ctx.source.name()), one: d.one }
    }

    /// THE VALUE TIER of an Elden Ring item and how it came about.
    pub fn tier_of(&self, item: &ErItem, ctx: &Ctx) -> (u8, String) {
        let rarity = self.rarity_points.get(item.rarity.min(3) as usize).copied().unwrap_or(item.rarity as f32);
        let source = self.source_points.get(&ctx.source).copied().unwrap_or(0.0);
        let region = self.region_points.get((ctx.depth as usize).min(self.region_points.len().saturating_sub(1))).copied().unwrap_or(0.0);
        let sell = self.sell_points.iter().rev().find(|p| item.sell_value >= p.0).map_or(0.0, |p| p.1);
        let somber = if item.somber { self.somber_points } else { 0.0 };
        let points = rarity + source + region + sell + somber;
        let tier = (1.0 + (points + 1.0e-4).floor()).clamp(1.0, self.tier_count as f32) as u8;
        let mut why = format!("tier {tier} = 1 + rarity {} {rarity:+} + {} {source:+} + depth {} {region:+}", item.rarity, ctx.source.name(), ctx.depth);
        if sell != 0.0 {
            why += &format!(" + sell {} {sell:+}", item.sell_value);
        }
        if somber != 0.0 {
            why += &format!(" + somber {somber:+}");
        }
        (tier, why)
    }

    fn kh(&self, key: &str) -> Option<Give> {
        self.items.get(key).map(|id| Give::Kh { item_id: *id, key: key.to_string() })
    }

    /// (IMPL_ER61; the user, 2026-10-14: "lower quality of armour and accessories found in lower level areas") The tier
    /// of an armour piece or accessory: the item's value tier, at most `tier.equip_cap` of the region depth.
    pub fn equip_tier(&self, tier: u8, depth: u8) -> u8 {
        match self.equip_cap.get(depth as usize).or(self.equip_cap.last()) {
            Some(cap) => tier.min(*cap),
            None => tier,
        }
    }

    /// The kinds of KH equipment an Elden Ring item asks for: armour or accessory, and the tags in order of wish.
    fn wanted(item: &ErItem) -> (bool, Vec<&'static str>) {
        let element = item.element.kh_tag();
        match item.cat {
            Some(Cat::Protector) => (true, element.into_iter().chain(["defence"]).collect()),
            Some(Cat::Weapon) => match item.wep_kind {
                WepKind::Shield => (true, element.into_iter().chain(["defence"]).collect()),
                WepKind::Staff | WepKind::Seal => (false, vec!["magic"]),
                _ => match element {
                    Some(e) => (false, vec![e, "magic"]),
                    None => (false, vec!["strength"]),
                },
            },
            Some(Cat::Gem) => (false, vec!["ap", "strength"]),
            Some(Cat::Goods) => match item.goods_type {
                5 | 16 | 17 | 18 => (false, vec!["magic"]),
                7 | 8 => (false, vec!["support"]),
                _ => (false, Vec::new()),
            },
            _ => (false, Vec::new()),
        }
    }

    /// AN ARMOUR PIECE OR ACCESSORY OF THE TIER for this item: the stable choice among the pieces whose kind fits,
    /// stepping on past pieces he already holds `equip.dup_max` of. `armour`: the class the give text forces (else
    /// the item's own); `forced`: tags the give text names ("magic", "ap+strength") in the place of the item's.
    fn equip(&self, armour: Option<bool>, forced: Option<&str>, item: &ErItem, ctx: &Ctx, tier: u8) -> Res {
        let tier = self.equip_tier(tier, ctx.depth);
        let (by_item, own) = Self::wanted(item);
        let armour = armour.unwrap_or(by_item);
        let mut prefs: Vec<&str> = match forced {
            Some(tags) => tags.split('+').collect(),
            // (a text that forces the other class: of the item's wishes only an element still applies)
            None if armour != by_item => own.into_iter().filter(|p| !matches!(*p, "defence" | "strength" | "magic" | "ap" | "support")).collect(),
            None => own,
        };
        // (a piece that raises Strength and Magic alike stands in where the tier has no pure one)
        for k in (0..prefs.len()).rev() {
            if matches!(prefs[k], "strength" | "magic") {
                prefs.insert(k + 1, "balanced");
            }
        }
        let pool_of = |t: u8| self.ranked.iter().filter(|r| r.armour == armour && r.tier == t).collect::<Vec<_>>();
        // the tier's pool; an empty tier (none of this class ranks there) gives the next lower, then the next higher
        let Some(pool) = (1..=tier).rev().chain(tier + 1..=self.tier_count).map(pool_of).find(|p| !p.is_empty()) else { return Res::Skip };
        let fitting = prefs.iter().map(|p| pool.iter().copied().filter(|r| r.tags.iter().any(|t| t == p)).collect::<Vec<_>>()).find(|c| !c.is_empty());
        let start = stable_hash(item.cat, item.base_id());
        for list in fitting.iter().chain(std::iter::once(&pool)) {
            for k in 0..list.len() {
                let piece = list[(start + k) % list.len()];
                if (ctx.state)(&piece.key) < self.dup_max {
                    return Res::Ok(Give::Kh { item_id: piece.item_id, key: piece.key.clone() });
                }
            }
        }
        Res::Skip
    }

    fn material(&self, item: &ErItem, grade: u8) -> Res {
        let pick = if item.cat == Some(Cat::Goods) { stable_pick(item.id, self.families.len()) } else { stable_hash(item.cat, item.base_id()) % self.families.len().max(1) };
        match self.families.get(pick).and_then(|f| self.materials.get(&(f.clone(), grade.clamp(1, 4)))).and_then(|k| self.kh(k)) {
            Some(give) => Res::Ok(give),
            None => Res::Bad,
        }
    }

    /// One give text (no `|`). With IMPL_ER25's rows (`generation` 1) the rarity, not the tier, picks the grade.
    fn resolve_one(&self, give: &str, item: &ErItem, ctx: &Ctx, tier: u8) -> Res {
        let rarity = item.rarity.min(3) as usize;
        let munny = |runes: i32| if runes > 0 { Res::Ok(Give::Munny(runes)) } else { Res::Skip };
        let kh = |key: &str| self.kh(key).map_or(Res::Bad, Res::Ok);
        let nth = |keys: &str, at: usize| {
            let keys: Vec<&str> = keys.split(',').map(str::trim).collect();
            kh(keys[at.min(keys.len() - 1)])
        };
        match give {
            "pass" => Res::Ok(Give::Pass),
            "nothing" => Res::Ok(Give::Nothing),
            "munny:soul" => munny(item.soul),
            "munny:sell" => munny((item.sell_value.max(0) as f32 * self.munny_per_sell).round() as i32),
            "material" => self.material(item, if self.generation < 2 { rarity as u8 + 1 } else { tier }),
            "armour" => {
                let Some(pool) = self.pools.get(&format!("armour.{}", item.element.name())).or_else(|| self.pools.get("armour.none")).filter(|p| !p.is_empty()) else { return Res::Bad };
                kh(&pool[rarity.min(pool.len() - 1)])
            }
            "accessory" => {
                let Some(pool) = (0..=rarity).rev().find_map(|t| self.pools.get(&format!("accessory.{t}")).filter(|p| !p.is_empty())) else { return Res::Bad };
                kh(&pool[stable_pick(item.id, pool.len())])
            }
            "equip" => self.equip(None, None, item, ctx, tier),
            "equip:armour" => self.equip(Some(true), None, item, ctx, tier),
            "equip:accessory" => self.equip(Some(false), None, item, ctx, tier),
            "reward:acc_slot" | "reward:def_slot" | "reward:slot" => {
                let (acc, def) = ((ctx.state)("@acc_slots") < self.slot_max, (ctx.state)("@def_slots") < self.slot_max);
                match (give, acc, def) {
                    ("reward:acc_slot", true, _) | ("reward:slot", true, _) => Res::Ok(Give::Reward("acc_slot".to_string())),
                    ("reward:def_slot", _, true) | ("reward:slot", false, true) => Res::Ok(Give::Reward("def_slot".to_string())),
                    _ => Res::Skip,
                }
            }
            _ => {
                if let Some(key) = give.strip_prefix("item:") {
                    kh(key)
                } else if let Some(keys) = give.strip_prefix("tier:") {
                    nth(keys, rarity)
                } else if let Some(keys) = give.strip_prefix("bytier:") {
                    nth(keys, tier.max(1) as usize - 1)
                } else if let Some(n) = give.strip_prefix("material:") {
                    n.parse::<u8>().map_or(Res::Bad, |min| self.material(item, tier.max(min)))
                } else if let Some(tags) = give.strip_prefix("equip:accessory:") {
                    self.equip(Some(false), Some(tags), item, ctx, tier)
                } else if let Some(tags) = give.strip_prefix("equip:armour:") {
                    self.equip(Some(true), Some(tags), item, ctx, tier)
                } else if let Some(what) = give.strip_prefix("reward:") {
                    let (name, n) = what.split_once(':').unwrap_or((what, ""));
                    let ok = match (name, n.parse::<u32>()) {
                        ("max_hp", Ok(n)) => [3, 5, 10, 15, 30].contains(&n),
                        ("max_mp", Ok(n)) => [3, 5, 10, 20].contains(&n),
                        ("ap", Ok(n)) => (1..=10).contains(&n),
                        _ => false,
                    };
                    if ok {
                        Res::Ok(Give::Reward(what.to_string()))
                    } else {
                        Res::Bad
                    }
                } else if let Some(n) = give.strip_prefix("munny:") {
                    n.parse::<i32>().map_or(Res::Bad, munny)
                } else {
                    Res::Bad
                }
            }
        }
    }

    /// A give text for an item: alternatives separated by `|`, the first that has something to give; when none has,
    /// what `equip.exhausted` gives (after an `equip` text), else nothing. `None`: not understood (the caller passes
    /// the item through).
    fn resolve(&self, give: &str, item: &ErItem, ctx: &Ctx, tier: u8) -> Option<Give> {
        let mut equip = false;
        let mut unit = ctx.unit.clamp(0.0, 0.999_999) * 100.0;
        for alt in give.split('|').map(str::trim) {
            // `chance:<percent>:<give>`: taken with that chance; else the number is spread over what is left, so
            // `chance:30:A|chance:15:B|C` is 30 % A, 15 % B (of all), 55 % C
            let alt = match chance_of(alt) {
                Some((percent, rest)) => {
                    if unit >= percent {
                        unit -= percent;
                        continue;
                    }
                    rest
                }
                None => alt,
            };
            equip |= alt.starts_with("equip");
            match self.resolve_one(alt, item, ctx, tier) {
                Res::Bad => return None,
                Res::Skip => {}
                Res::Ok(give) => return Some(give),
            }
        }
        if equip {
            for alt in self.exhausted.split('|').map(str::trim) {
                if let Res::Ok(give) = self.resolve_one(alt, item, ctx, tier) {
                    return Some(give);
                }
            }
        }
        Some(Give::Nothing)
    }

    /// THE CLASSIFICATION. Fail-safe: an item whose row was not found, whose table is unknown, or that no rule
    /// names PASSES (it stays Elden Ring's) under the rule name "unknown".
    pub fn classify(&self, item: &ErItem, ctx: &Ctx) -> Decision {
        let d = self.classify_rules(item, ctx);
        // (SHOP-4) an item a game script takes or counts stays Elden Ring's, over the generic rules (the categories r2.1xx ..,
        // the online items); the user's own decisions stand (the explicit r2.0xx rules, id.* overrides, the old generation)
        let decided = d.rule.starts_with("id.") || d.rule.starts_with("r2.0") || d.rule.starts_with("rule.");
        if !matches!(d.give, Give::Pass) && !decided {
            if let Some(keep) = script_required(item) {
                return keep;
            }
        }
        d
    }

    /// The rules alone (no script requirement).
    fn classify_rules(&self, item: &ErItem, ctx: &Ctx) -> Decision {
        let unknown = || Decision { give: Give::Pass, rule: "unknown".to_string(), tier: 0, why: String::new(), one: false };
        let Some(cat) = item.cat else { return unknown() };
        let (tier, why) = self.tier_of(item, ctx);
        // (SHOP, the user 2026-10-23: "remove all online items, replace them with kingdom hearts consumable items")
        if let Some(d) = online_item(item, ctx) {
            return d;
        }
        let over = self.overrides.get(&(cat, item.id)).map(|g| (g, item.id)).or_else(|| self.overrides.get(&(cat, item.base_id())).map(|g| (g, item.base_id())));
        if let Some((give, id)) = over {
            return match self.resolve(give, item, ctx, tier) {
                Some(give) => Decision { give, rule: format!("id.{}.{}", cat.name(), id), tier, why, one: false },
                None => unknown(),
            };
        }
        if !item.known {
            return unknown();
        }
        // (ER party S6, U4) a staff / shield of the friends' share becomes a friend's weapon of its tier
        #[cfg(feature = "party")]
        if let Some(d) = self.party_weapon(item, ctx, &party_friends()) {
            return self.restrict(d, item, ctx); // (PU) a friend's weapon is equipment
        }
        for rule in &self.rules {
            if rule.proposal && !self.convert_progression {
                continue;
            }
            let holds = (rule.cat.is_empty() || rule.cat.contains(&cat))
                && (rule.goods_type.is_empty() || (cat == Cat::Goods && rule.goods_type.contains(&item.goods_type)))
                && (rule.rarity.is_empty() || rule.rarity.contains(&item.rarity))
                && (rule.effect.is_empty() || rule.effect.iter().any(|e| e == item.effect.name()))
                && rule.consumable.is_none_or(|c| cat == Cat::Goods && c == item.consumable)
                && (rule.ids.is_empty() || rule.ids.iter().any(|(lo, hi)| (*lo..=*hi).contains(&item.id)))
                && (rule.source.is_empty() || rule.source.contains(&ctx.source))
                && (rule.tier.is_empty() || rule.tier.contains(&tier))
                && (rule.wep_kind.is_empty() || (cat == Cat::Weapon && rule.wep_kind.contains(&item.wep_kind)));
            if holds {
                let (mut tier, mut why) = (tier, why);
                if let Some(t) = rule.as_tier {
                    tier = t.clamp(1, self.tier_count);
                    why = format!("tier {tier} by the rule");
                } else if let Some(t) = rule.tier_min.filter(|t| *t > tier) {
                    tier = t.clamp(1, self.tier_count);
                    why += &format!(", raised to {tier} by the rule");
                }
                return match self.resolve(&rule.give, item, ctx, tier) {
                    Some(give) => {
                        let pass = give == Give::Pass;
                        let d = Decision { give, rule: rule.id.clone(), tier: if pass { 0 } else { tier }, why: if pass { String::new() } else { why }, one: rule.one };
                        // (PU) a source the user did not allow usable items from gives a synthesis material
                        if rule.any_source {
                            d
                        } else {
                            self.restrict(d, item, ctx)
                        }
                    }
                    None => unknown(),
                };
            }
        }
        unknown()
    }

    /// A MERCHANT'S LINE under `shop.kh`: a key item's line stays; a line whose item maps to a KH item sells that
    /// item at KH3's own shop price (where KH3 sells it), else at the price of its tier (`shop.tier_price`);
    /// everything else (munny, a reward, nothing) is dropped.
    pub fn shop_line(&self, item: &ErItem, depth: u8) -> (ShopLine, Decision) {
        let decision = self.classify(item, &Ctx::of(Source::Shop, depth));
        let line = match &decision.give {
            Give::Pass => ShopLine::Keep,
            Give::Kh { item_id, key } => {
                let own = self.buy.get(key).copied().unwrap_or(0);
                let rank = self.ranked.iter().find(|r| &r.key == key).map(|r| r.tier).or_else(|| self.grades.get(key).copied().filter(|g| *g > 0)).unwrap_or(decision.tier).max(1);
                let by_tier = self.shop_tier_price.get(rank as usize - 1).or(self.shop_tier_price.last()).copied().unwrap_or(0);
                match if own > 0 { own } else { by_tier } {
                    0 => ShopLine::Drop("no price".to_string()),
                    price => ShopLine::Sell { item_id: *item_id, key: key.clone(), price },
                }
            }
            Give::Munny(_) => ShopLine::Drop("it maps to munny".to_string()),
            Give::Reward(_) => ShopLine::Drop("it maps to a reward".to_string()),
            Give::Nothing => ShopLine::Drop("it maps to nothing".to_string()),
        };
        (line, decision)
    }

    /// (er30) THE CLASS OF A KILLED ENEMY for the base drop, by its maximum HP: 0 ordinary, 1 big, 2 boss
    /// (`base.class_hp`: from how many HP the next class begins).
    pub fn base_class(&self, max_hp: i32) -> usize {
        self.base_class_hp.iter().filter(|from| max_hp >= **from).count()
    }

    /// (er30) THE BASE CONSUMABLE DROP OF ONE KILL, independent of the enemy's Elden Ring lot (whose rows hold no
    /// recovery item at all: the user's log of 2026-10-13): every `base.drop.*` line is rolled BY ITSELF with KH3's
    /// own item roll (`(int)(rand x 99 + 1) <= rate`, the rate with Lucky Strike `round(rate x (1 + 0.3 n))`; table
    /// spawn 0x141480420), its rate the one of the enemy's class; the item is the line's at the level
    /// `depth + base.class_level[class]`. Returns (item id, key, line name, the rate rolled against) per drop.
    pub fn base_drops(&self, class: usize, depth: u8, luck_up: u32, unit: &mut dyn FnMut() -> f32) -> Vec<(u32, String, String, u32)> {
        let mut out = Vec::new();
        if !self.base_on {
            return out;
        }
        let level = depth as usize + self.base_class_level.get(class).or(self.base_class_level.last()).copied().unwrap_or(0) as usize;
        for line in &self.base_lines {
            let rate = line.rate.get(class).or(line.rate.last()).copied().unwrap_or(0.0);
            let rate = (rate * (1.0 + self.luck_per_copy * luck_up as f32)).round() as u32;
            // (a line with rate 0 does not use a random number: adding a line does not move the others' rolls)
            if rate == 0 || (unit().clamp(0.0, 0.999_999) * 99.0) as u32 + 1 > rate {
                continue;
            }
            let key = &line.line[level.min(line.line.len() - 1)];
            if let Some(id) = self.items.get(key) {
                out.push((*id, key.clone(), line.name.clone(), rate));
            }
        }
        out
    }

    /// The base drop's lines for the log: (name, rates by class).
    pub fn base_summary(&self) -> String {
        self.base_lines.iter().map(|l| format!("{} {:?} %", l.name, l.rate)).collect::<Vec<_>>().join(", ")
    }

    /// KH3's Luck in the place of Elden Ring's discovery: the factor on the points of an `enableLuck` slot.
    pub fn luck_factor(&self, luck_up: u32) -> f32 {
        self.base_discovery / 100.0 * (1.0 + self.luck_per_copy * luck_up as f32)
    }
}

impl LootRules {
    /// (IMPL_ER31, small bosses: bossreward.rs) AN ARMOUR PIECE OR ACCESSORY OF AT MOST `tier`: the tier's pool, then
    /// the lower ones, NEVER a higher one (the caller's tier is a cap); within a pool the stable choice by `seed`,
    /// stepping on past pieces he holds `equip.dup_max` of. Returns (item id, key, the piece's tier).
    pub fn capped_piece(&self, tier: u8, armour: bool, seed: u32, held: &dyn Fn(&str) -> u32) -> Option<(u32, String, u8)> {
        for t in (1..=tier.min(self.tier_count)).rev() {
            let pool: Vec<&Ranked> = self.ranked.iter().filter(|r| r.armour == armour && r.tier == t).collect();
            for k in 0..pool.len() {
                let piece = pool[(seed as usize + k) % pool.len()];
                if held(&piece.key) < self.dup_max {
                    return Some((piece.item_id, piece.key.clone(), t));
                }
            }
        }
        None
    }
}

/// One slot of an ItemLotParam row.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct LotSlot {
    /// `lotItemCategory`: 0 = empty.
    pub category: i32,
    pub item: i32,
    /// `lotItemBasePoint`.
    pub points: u32,
    /// `lotItemNum`.
    pub count: u8,
    /// `enableLuck`.
    pub luck: bool,
    /// `getItemFlagId`: not 0 = a one-time drop the game tracks by an event flag.
    pub flag: u32,
}

impl LotSlot {
    pub fn empty(&self) -> bool {
        self.category == 0 || self.item <= 0 || self.count == 0
    }
}

/// One ItemLotParam row.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct LotRow {
    pub id: u32,
    pub slots: [LotSlot; 8],
    /// `lotItem_Rarity`: not negative = the rarity of what this row drops.
    pub rarity: i8,
    /// `getItemFlagId` of the row.
    pub flag: u32,
    /// `cumulateNumMax` > 0: the row has a pity counter (NOT MODELLED: the base points are used as they are).
    pub cumulate: bool,
}

impl LotRow {
    /// The weights of the eight slots with a luck factor: `round(points x factor)` where `enableLuck` is set
    /// (KH3 rounds its lucky rate the same way), the points themselves elsewhere.
    pub fn weights(&self, luck_factor: f32) -> [u32; 8] {
        self.slots.map(|s| if s.luck { (s.points as f32 * luck_factor).round() as u32 } else { s.points })
    }

    /// The chance of each slot (0 when the row has no points).
    pub fn chances(&self, luck_factor: f32) -> [f32; 8] {
        let w = self.weights(luck_factor);
        let total: u32 = w.iter().sum();
        w.map(|x| if total == 0 { 0.0 } else { x as f32 / total as f32 })
    }

    /// ONE ROLL: `unit` uniform in [0, 1). The slot picked (it may be an empty one = nothing), `None` when the row
    /// has no points at all.
    pub fn roll(&self, luck_factor: f32, unit: f32) -> Option<usize> {
        let w = self.weights(luck_factor);
        let total: u32 = w.iter().sum();
        if total == 0 {
            return None;
        }
        let mut at = ((unit.clamp(0.0, 0.999_999) * total as f32) as u32).min(total - 1);
        for (k, weight) in w.iter().enumerate() {
            if at < *weight {
                return Some(k);
            }
            at -= weight;
        }
        None
    }

    /// The row is a plain repeatable drop: no event flag anywhere (a flagged drop is a one-time or quest drop and
    /// stays the game's).
    pub fn plain(&self) -> bool {
        self.flag == 0 && self.slots.iter().all(|s| s.empty() || s.flag == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE VALUE MODEL ALONE (PU: without the row `source.usable`, whose own tests are `pu_*` below: these tests read
    /// what an item is worth from each source; the user's 2026-10-18 rule then keeps usable items to some sources).
    fn rules() -> LootRules {
        model_of(SHEET)
    }

    /// (PU) A sheet text read without its row `source.usable` (the value model alone).
    fn model_of(text: &str) -> LootRules {
        let mut json: serde_json::Value = serde_json::from_str(text).unwrap();
        json["rows"].as_array_mut().unwrap().retain(|r| r[0].as_str() != Some("source.usable"));
        let rules = LootRules::parse(&json.to_string(), &kh_items());
        assert!(rules.problems.is_empty(), "{:?}", rules.problems);
        assert!(Source::ALL.iter().all(|s| rules.usable_from(*s)));
        rules
    }

    /// (PU) The sheet as shipped, with the user's rule on the sources.
    fn sheet() -> LootRules {
        let rules = LootRules::parse(SHEET, &kh_items());
        assert!(rules.problems.is_empty(), "{:?}", rules.problems);
        rules
    }

    fn class_of(r: &LootRules, d: &Decision) -> &'static str {
        match &d.give {
            Give::Kh { key, .. } => r.classes.get(key).copied().unwrap_or("?"),
            Give::Munny(_) => "munny",
            Give::Pass => "pass",
            Give::Reward(_) => "reward",
            Give::Nothing => "nothing",
        }
    }

    /// (PU, the user 2026-10-18: "remove the two items you get replacing the flask of crimson and cerulean tears. You get
    /// nothing instead") EVERY FLASK GIVES NOTHING (no KH item, so no popup) from every source; the gate still takes it out
    /// of the buffer (Give::Nothing is swallowed: loot.rs); as the user's log had them (goods type 0, consumable, effect
    /// none) and with a heal effect; the Flask of Wondrous Physick is unchanged.
    #[test]
    fn pu_the_flasks_give_nothing() {
        let r = sheet();
        for id in [1000u32, 1001, 1024, 1025, 1050, 1051, 1074, 1075] {
            for effect in [Effect::None, Effect::Hp, Effect::Fp] {
                for source in Source::ALL {
                    let d = r.classify(&ErItem { effect, sell_value: -1, ..goods(id, 0, 3) }, &Ctx { unit: 0.0, ..Ctx::of(source, 0) });
                    assert_eq!((d.give.clone(), d.rule.as_str()), (Give::Nothing, "r2.015.flask"), "flask {id} {effect:?} from {}", source.name());
                }
            }
        }
        assert_eq!(name(&r.classify(&ErItem { consumable: false, ..goods(250, 9, 2) }, &Ctx::of(Source::Shop, 0))), "Elixir", "the physick flask");
        assert_ne!(r.classify(&goods(1100, 0, 0), &Ctx { unit: 0.0, ..ENEMY }).rule, "r2.015.flask", "outside the range");
    }

    /// (PU, the user 2026-10-18: "anything that isn't a dead body or an enemy or chest should not drop usable items,
    /// equipment or accessories unless I've specifically allowed it") THE SOURCES: a crafting material (Rowa Fruit,
    /// Gold-Tinged Excrement: goods type 2), a consumable, a weapon, a shield, armour, a talisman, an Ash of War, a spirit
    /// ash: from an enemy or a chest what the model gives; gathered, a world pickup, unknown: ONLY a synthesis material,
    /// never munny, whatever the roll.
    #[test]
    fn pu_usable_items_come_only_from_the_allowed_sources() {
        let r = sheet();
        let allowed = [Source::Enemy, Source::Chest, Source::Npc, Source::Boss, Source::Shop, Source::World]; // (coordinator 2026-10-18, user: "yes ground items can contain the other stuff")
        let barred = [Source::Gather, Source::Unknown];
        assert!(allowed.iter().all(|s| r.usable_from(*s)) && !barred.iter().any(|s| r.usable_from(*s)));
        let crafting = ErItem { sell_value: 10, ..goods(20720, 2, 0) };
        let consumable = ErItem { sell_value: 50, ..goods(1700, 0, 1) };
        let protector = ErItem { cat: Some(Cat::Protector), id: 40000, rarity: 1, sell_value: 100, known: true, ..ErItem::default() };
        let talisman = ErItem { cat: Some(Cat::Accessory), id: 1000, rarity: 2, sell_value: 100, known: true, ..ErItem::default() };
        let ash_of_war = ErItem { cat: Some(Cat::Gem), id: 11000, rarity: 1, sell_value: 100, known: true, ..ErItem::default() };
        let items = [crafting, consumable, weapon(1000000, 1, WepKind::Melee), weapon(30000000, 1, WepKind::Shield), protector, talisman, ash_of_war, ErItem { consumable: false, ..goods(200000, 7, 1) }];
        for item in &items {
            for source in barred {
                for k in 0..50 {
                    let d = r.classify(item, &Ctx { unit: k as f32 / 50.0, ..Ctx::of(source, 1) });
                    assert_eq!(class_of(&r, &d), "material", "{:?} {} from {} at {k}: {:?}", item.cat, item.id, source.name(), d);
                }
            }
        }
        // the same rolls from the allowed sources still give usable items, equipment and munny
        let from = |item: &ErItem, source: Source| (0..50).map(|k| class_of(&r, &r.classify(item, &Ctx { unit: k as f32 / 50.0, ..Ctx::of(source, 1) }))).collect::<std::collections::HashSet<_>>();
        for source in [Source::Enemy, Source::Chest] {
            let got = from(&crafting, source);
            assert!(got.contains("battle") && got.contains("munny") && got.contains("material"), "{}: {got:?}", source.name());
        }
        assert!(from(&items[2], Source::Chest).contains("accessory") && from(&protector, Source::Chest).contains("armour"));
        // GOLD-TINGED EXCREMENT (a crafting material, goods type 2; the id here is a stand-in): gathered or picked up =
        // a material at every roll (before: 25 % a Potion-line item, 12 % an Ether-line item, 23 % munny, else a material)
        let excrement = ErItem { sell_value: 30, ..goods(15400, 2, 1) };
        let only_material: std::collections::HashSet<&str> = ["material"].into_iter().collect();
        assert_eq!(from(&excrement, Source::Gather), only_material);
        assert!(from(&excrement, Source::World).len() > 1, "a ground pickup may give more than a material (user 2026-10-18)");
        let before = rules().classify(&excrement, &Ctx { unit: 0.0, ..Ctx::of(Source::Gather, 0) });
        assert_eq!(class_of(&r, &before), "battle", "before: a Potion-line item at this roll");
    }

    /// (PU) THE USER'S EARLIER ALLOWANCES STAND FROM EVERY SOURCE: Golden Seed / Sacred Tear (id rows and r2.030) ->
    /// Megalixir; the rewards (a slot, max HP / MP, AP: the stat boosts the roster shares); key items and tools pass.
    #[test]
    fn pu_the_users_allowances_stand_everywhere() {
        let r = sheet();
        for source in Source::ALL {
            let c = |item: &ErItem| r.classify(item, &Ctx { unit: 0.0, ..Ctx::of(source, 0) });
            assert_eq!(name(&c(&goods(10010, 14, 2))), "Megalixir", "Golden Seed from {}", source.name());
            assert_eq!(name(&c(&goods(10020, 14, 2))), "Megalixir", "Sacred Tear from {}", source.name());
            assert_eq!(name(&c(&goods(10011, 11, 2))), "Megalixir", "a flask upgrade (r2.030) from {}", source.name());
            assert_eq!(c(&goods(10030, 1, 2)).give, Give::Reward("max_mp:5".to_string()), "a Memory Stone from {}", source.name());
            assert_eq!(c(&ErItem { consumable: false, ..goods(8000, 1, 0) }).give, Give::Pass, "the Stonesword Key from {}", source.name());
            assert_eq!(c(&ErItem { consumable: false, ..goods(130, 0, 3) }).give, Give::Pass, "the whistle (a tool) from {}", source.name());
        }
    }

    /// (PU) WHERE A GATE ITEM CAME FROM: the chest animation first, then the call sites the user's logs showed, then an
    /// Elden Ring window (a purchase), then the lot tables, where "enemies and the world" is the safe side (the world).
    #[test]
    fn pu_the_gate_tells_the_source_in_order() {
        let r = sheet();
        assert_eq!(r.gate_source(true, Some(Source::World), true, true, true, true).0, Source::Chest);
        assert_eq!(r.gate_source(false, Some(Source::Gather), true, true, false, false).0, Source::Gather);
        assert_eq!(r.gate_source(false, None, true, false, true, false).0, Source::Shop);
        assert_eq!(r.gate_source(false, None, false, true, false, false).0, Source::Enemy, "only enemies drop it");
        assert_eq!(r.gate_source(false, None, false, true, true, false).0, Source::World, "enemies and the world: the safe side");
        assert_eq!(r.gate_source(false, None, false, true, false, true).0, Source::Gather);
        assert_eq!(r.gate_source(false, None, false, false, false, false).0, Source::Unknown);
        // the call sites of the user's logs (2.7.1.0)
        assert_eq!(r.caller_source(&[0x563f6c, 0x6e8590, 0x6e83a1, 0x6e80f9, 0x6c2c08]), Some(Source::Gather), "gathering (Rowa Fruit, Erdleaf Flower)");
        assert_eq!(r.caller_source(&[0x55c97e, 0x5acd58, 0x5ac9b2, 0x591780, 0x58f27e]), Some(Source::World), "the treasure pickup (corpse or ground)");
        assert_eq!(r.caller_source(&[0x563f6c, 0x56db3d, 0x568e38, 0x583e57, 0x58a2ac]), Some(Source::Npc), "an event's award");
        assert_eq!(r.caller_source(&[0x563f6c, 0xea8f99, 0x208a781]), Some(Source::Npc), "a talk's gift");
        assert_eq!(r.caller_source(&[0x563f6c, 0x6e4473, 0x6e41db]), None, "not named: the lot tables decide");
    }

    /// The sheet as the deployed build 4B6C2CFEF72E reads it: without the rows it does not know.
    fn legacy() -> LootRules {
        let mut json: serde_json::Value = serde_json::from_str(SHEET).unwrap();
        json["rows"].as_array_mut().unwrap().retain(|r| !r[0].as_str().unwrap().starts_with("r2."));
        let rules = LootRules::parse(&json.to_string(), &kh_items());
        assert!(rules.problems.is_empty(), "{:?}", rules.problems);
        assert_eq!(rules.generation, 1);
        rules
    }

    fn goods(id: u32, goods_type: u8, rarity: u8) -> ErItem {
        ErItem { cat: Some(Cat::Goods), id, goods_type, rarity, consumable: true, known: true, ..ErItem::default() }
    }

    fn weapon(id: u32, rarity: u8, wep_kind: WepKind) -> ErItem {
        ErItem { cat: Some(Cat::Weapon), id, rarity, wep_kind, sell_value: 100, known: true, ..ErItem::default() }
    }

    fn name(d: &Decision) -> String {
        match &d.give {
            Give::Kh { item_id, .. } => kh_host::items::by_id(*item_id).map_or("?".to_string(), |i| i.name.clone()),
            Give::Pass => "PASS".to_string(),
            Give::Munny(n) => format!("{n} munny"),
            Give::Reward(r) => format!("reward {r}"),
            Give::Nothing => "nothing".to_string(),
        }
    }

    fn tags(r: &LootRules, d: &Decision) -> Vec<String> {
        match &d.give {
            Give::Kh { key, .. } => r.ranked().iter().find(|x| &x.key == key).map_or(Vec::new(), |x| x.tags.clone()),
            _ => Vec::new(),
        }
    }

    const ENEMY: Ctx<'static> = Ctx { source: Source::Enemy, depth: 0, state: &holds_nothing, unit: 0.999 };

    /// The sheet as shipped parses without a problem; it is generation 2; a broken give text is reported.
    #[test]
    fn the_sheet_parses_and_names_only_real_kh3_items() {
        let r = rules();
        assert_eq!(r.generation, 2);
        assert!(r.rule_count().0 >= 30 && r.rule_count().1 >= 2, "{:?}", r.rule_count());
        assert_eq!((r.luck_per_copy, r.base_discovery, r.chain_max, r.tier_count, r.dup_max), (0.3, 100.0, 16, 6, 2));
        assert_eq!(r.lot_category(1), Some(Cat::Goods));
        assert_eq!(r.lot_category(3), Some(Cat::Protector));
        assert_eq!(r.lot_category(0), None);
        assert_eq!((r.shop_category(3), r.shop_category(0), r.shop_category(9)), (Some(Cat::Goods), Some(Cat::Weapon), None));
        assert_eq!([r.wep_kind(57), r.wep_kind(61), r.wep_kind(67), r.wep_kind(81), r.wep_kind(3)], [WepKind::Staff, WepKind::Seal, WepKind::Shield, WepKind::Ammo, WepKind::Melee]);
        let broken = SHEET.replace("\"give\":\"item:ITEM_ALLCURE\"},\"rule\",\"HOST CHOICE (2026-10-13, IMPL_ER28", "\"give\":\"item:ITEM_NO_SUCH\"},\"rule\",\"HOST CHOICE (2026-10-13, IMPL_ER28");
        assert_ne!(broken, SHEET);
        assert!(LootRules::parse(&broken, &kh_items()).problems.iter().any(|p| p.contains("r2.203")));
    }

    /// THE DEPLOYED BUILD'S READING STANDS: without the `r2.*` rows the IMPL_ER25 rules decide exactly as before
    /// (rarity picks the grade, weapons are munny, talismans come from the hand-made pools).
    #[test]
    fn the_first_generation_of_rules_still_reads_as_it_did() {
        let r = legacy();
        let c = |item: &ErItem| r.classify(item, &Ctx::of(Source::Chest, 4));
        for goods_type in [1u8, 3, 12, 15] {
            assert_eq!((c(&goods(8000, goods_type, 0)).give, c(&goods(8000, goods_type, 0)).rule), (Give::Pass, "rule.010.key".to_string()));
        }
        let m = |id: u32, rarity: u8| name(&c(&goods(id, 2, rarity)));
        assert_eq!([m(20650, 0), m(20650, 1), m(20650, 2), m(20650, 3)], ["Betwixt Shard", "Betwixt Stone", "Betwixt Gem", "Betwixt Crystal"], "the grade by rarity, whatever the source");
        assert_eq!(c(&weapon(1000000, 0, WepKind::Melee)).give, Give::Munny(100));
        let acc = |id: u32, rarity: u8| name(&c(&ErItem { cat: Some(Cat::Accessory), id, rarity, known: true, ..ErItem::default() }));
        assert_eq!([acc(7, 0), acc(8, 3)], ["Power Ring", "Phantom Ring"]);
        let armour = |element: Element, rarity: u8| name(&c(&ErItem { cat: Some(Cat::Protector), id: 40000, rarity, element, known: true, ..ErItem::default() }));
        assert_eq!([armour(Element::None, 0), armour(Element::Fire, 2), armour(Element::Magic, 3)], ["Shield Belt", "Firaga Bangle", "Blizzaza Choker"]);
        let heal = |effect: Effect, rarity: u8| name(&c(&ErItem { effect, ..goods(900, 0, rarity) }));
        assert_eq!([heal(Effect::Hp, 0), heal(Effect::Hp, 2), heal(Effect::Both, 3)], ["Potion", "Mega-Potion", "Megalixir"]);
    }

    /// KH3'S EQUIPMENT RANKED: all 52 armour pieces and 112 accessories have a tier; the cheap pieces are low, the
    /// best are at the top; every tier has accessories; the tags say what a piece is.
    #[test]
    fn kh3s_equipment_is_ranked_into_the_tiers_by_its_own_numbers() {
        let r = rules();
        assert_eq!((r.ranked().iter().filter(|x| x.armour).count(), r.ranked().iter().filter(|x| !x.armour).count()), (52, 52), "the 60 synthesis variants are excluded by the sheet");
        let of = |name: &str| {
            let item = kh_host::items::table().iter().find(|i| i.name == name).unwrap();
            r.ranked().iter().find(|x| x.key == item.key).unwrap().clone()
        };
        let tiers = |names: &[&str]| names.iter().map(|n| of(n).tier).collect::<Vec<_>>();
        assert_eq!(tiers(&["Power Ring", "Buster Ring", "Valor Ring", "Phantom Ring", "Breakthrough", "Crystal Regalia+"]), [1, 2, 3, 4, 5, 6]);
        assert_eq!(tiers(&["Magic Ring", "Rune Ring", "Force Ring", "Sorcerer's Ring", "Cosmic Arts", "Mickey Clasp"]), [1, 2, 3, 4, 5, 6]);
        assert_eq!(tiers(&["Ability Ring", "Technician's Ring", "Skill Ring", "Skill Ring+", "Cosmic Ring"]), [1, 2, 3, 4, 6]);
        assert_eq!(tiers(&["Shield Belt", "Guardian's Belt", "Power Band", "Cosmic Belt", "Master Belt", "Royal Ribbon"]), [1, 2, 3, 4, 5, 6]);
        assert_eq!(tiers(&["Fire Bangle", "Fira Bangle", "Firaga Bangle", "Firaza Bangle"]), [2, 3, 4, 5]);
        assert_eq!(tiers(&["Lucky Ring", "Laughter Pin", "Forest Clasp"]), [3, 3, 4]);
        for tier in 1..=6 {
            assert!(r.ranked().iter().any(|x| !x.armour && x.tier == tier), "no accessory of tier {tier}");
            assert!(r.ranked().iter().any(|x| x.armour && x.tier == tier), "no armour of tier {tier}");
        }
        let has = |name: &str, tag: &str| of(name).tags.iter().any(|t| t == tag);
        assert!(has("Power Ring", "strength") && has("Magic Ring", "magic") && has("Skill Ring", "ap") && has("Lucky Ring", "support"));
        assert!(has("Fire Bangle", "fire") && has("Shield Belt", "defence") && has("Ribbon", "all") && has("Dark Anklet", "dark") && has("Firefighter Rosette", "fire"));
        assert!(has("Fire Cufflink", "fire") && has("Soldier's Earring", "support"));
        // a sheet row moves a single piece
        let moved = SHEET.replace("[\"schema\",2,", "[\"kh.tier.ACC_ITEM12\",5,\"-\",\"-\",\"-\",\"-\",\"guess\",\"-\"],\n[\"schema\",2,");
        assert_eq!(LootRules::parse(&moved, &kh_items()).ranked().iter().find(|x| x.key == "ACC_ITEM12").unwrap().tier, 5);
    }

    /// THE TIER of an Elden Ring item: rarity + source + region depth + sell value + somber, clamped to 1 .. 6.
    #[test]
    fn the_tier_is_rarity_plus_source_plus_depth() {
        let r = rules();
        let t = |item: &ErItem, source: Source, depth: u8| r.tier_of(item, &Ctx::of(source, depth)).0;
        let common = goods(20650, 2, 0);
        assert_eq!([t(&common, Source::Enemy, 0), t(&common, Source::World, 0), t(&common, Source::Chest, 0), t(&common, Source::Boss, 0)], [1, 1, 2, 3]);
        assert_eq!([t(&common, Source::Enemy, 1), t(&common, Source::Enemy, 2), t(&common, Source::Enemy, 4)], [1, 2, 3], "a deeper region");
        // (IMPL_ER61: rarity points 0 / 0.5 / 1.5 / 2.5, region points 0 / 0.75 / 1.5 / 2 / 2.5: lower early, as high late)
        assert_eq!([t(&common, Source::World, 1), t(&common, Source::Chest, 3)], [2, 4], "half points add up");
        let rare = goods(1, 0, 2);
        assert_eq!([t(&rare, Source::Enemy, 0), t(&rare, Source::Chest, 0), t(&rare, Source::Chest, 2)], [2, 3, 5]);
        let legendary = ErItem { somber: true, rarity: 3, ..weapon(9000000, 3, WepKind::Melee) };
        assert_eq!([t(&legendary, Source::Enemy, 0), t(&legendary, Source::World, 0), t(&legendary, Source::Chest, 3), t(&legendary, Source::Boss, 4)], [4, 4, 6, 6], "never above the top tier");
        // (IMPL_ER61) an armour piece / accessory is at most `tier.equip_cap` of the region: 2 in Limgrave .. 6 in the DLC
        assert_eq!((0..=5).map(|d| r.equip_tier(6, d)).collect::<Vec<_>>(), [2, 3, 4, 5, 6, 6]);
        assert_eq!((r.equip_tier(1, 0), r.equip_tier(3, 2)), (1, 3));
        assert_eq!(t(&ErItem { sell_value: 1000, ..common }, Source::World, 0), 2, "the sell value");
        assert!(r.tier_of(&legendary, &Ctx::of(Source::Chest, 3)).1.contains("somber"));
        // ---- the depth of a map block
        let block = |area: u32, x: u32, z: u32| ((area << 24) | (x << 16) | (z << 8)) as i32;
        assert_eq!(r.depth_of_block(block(60, 42, 36)), Some(0), "the First Step");
        assert_eq!([r.depth_of_block(block(60, 36, 44)), r.depth_of_block(block(60, 50, 38)), r.depth_of_block(block(60, 40, 52)), r.depth_of_block(block(60, 50, 55))], [Some(1), Some(2), Some(2), Some(3)]);
        assert_eq!([r.depth_of_block(block(10, 0, 0)), r.depth_of_block(block(13, 0, 0)), r.depth_of_block(block(61, 45, 40)), r.depth_of_block(block(18, 0, 0))], [Some(1), Some(3), Some(4), Some(0)]);
        assert_eq!([r.depth_of_block(block(31, 0, 0)), r.depth_of_block(-1), r.depth_of_block(0)], [None, None, None], "a cave keeps the last depth");
        // ---- the source, as far as the lot tables tell
        assert_eq!(r.source_of(None, true, false, false), (Source::Enemy, false));
        assert_eq!(r.source_of(None, false, true, true), (Source::World, false));
        assert_eq!(r.source_of(None, false, false, true), (Source::Gather, false));
        assert_eq!(r.source_of(None, true, true, false), (Source::World, true), "(PU) both: the safe side (the world's), marked as a guess");
        assert_eq!(r.source_of(None, false, false, false), (Source::Unknown, false));
        assert_eq!(r.source_of(Some(Source::Chest), true, true, true), (Source::Chest, false), "the call site wins");
        // (PU: the shipped row names the call sites of the user's logs; replaced here by two made-up ones)
        let named = SHEET.replace("[\"gate.callers\",{\"0x6e8590\":\"gather\",\"0x56db3d\":\"npc\",\"0xea8f99\":\"npc\",\"0x55c97e\":\"world\"},", "[\"gate.callers\",{\"0x5612AB\":\"chest\",\"7654321\":\"npc\"},");
        assert_ne!(named, SHEET);
        let n = LootRules::parse(&named, &kh_items());
        assert!(n.problems.is_empty(), "{:?}", n.problems);
        assert_eq!([n.caller_source(&[0x1, 0x5612AB]), n.caller_source(&[7654321]), n.caller_source(&[5])], [Some(Source::Chest), Some(Source::Npc), None]);
    }

    /// WHAT PASSES: key items (a Stonesword Key by its own row, a bell bearing by the key rule), tools, anything unknown.
    #[test]
    fn keys_tools_and_unknowns_pass() {
        let r = rules();
        let c = |item: &ErItem| r.classify(item, &Ctx::of(Source::Chest, 2));
        let key = |id: u32| ErItem { consumable: false, ..goods(id, 1, 0) };
        assert_eq!((c(&key(8000)).give, c(&key(8000)).rule.as_str(), c(&key(8000)).tier), (Give::Pass, "r2.010.stonesword_key", 0), "a Stonesword Key");
        assert_eq!((c(&key(8951)).give, c(&key(8951)).rule.as_str()), (Give::Pass, "r2.040.key"), "a bell bearing (a key item)");
        assert_eq!((c(&key(8105)).give, c(&key(8600)).give), (Give::Pass, Give::Pass), "a medallion half, a map");
        for goods_type in [3u8, 12, 15] {
            assert_eq!(c(&goods(2950, goods_type, 3)).give, Give::Pass, "goods type {goods_type}");
        }
        assert_eq!(c(&ErItem { consumable: false, ..goods(130, 0, 0) }).rule, "r2.050.tool");
        assert_eq!(c(&ErItem { cat: Some(Cat::Goods), id: 4242, known: false, ..ErItem::default() }).rule, "unknown", "a row that was not found passes");
        assert_eq!(c(&ErItem { cat: None, id: 1, known: true, ..ErItem::default() }).give, Give::Pass, "an unknown table passes");
        assert_eq!(c(&goods(1, 4, 0)).give, Give::Pass, "a goods type no rule names passes");
    }

    /// THE PROGRESSION ITEMS SORA CANNOT USE (proposal rows): each becomes its KH equivalent; with
    /// `progression.convert` 0 they stay Elden Ring's.
    #[test]
    fn progression_items_become_their_kh_equivalents() {
        let r = rules();
        let key = |id: u32| ErItem { consumable: false, ..goods(id, 1, 1) };
        let with = |item: &ErItem, acc: u32, def: u32| {
            let state = move |k: &str| match k {
                "@acc_slots" => acc,
                "@def_slots" => def,
                _ => 0,
            };
            name(&r.classify(item, &Ctx { source: Source::World, depth: 0, state: &state, unit: 0.999 }))
        };
        // Talisman Pouch: accessory slots first, then an armour slot, then a Megalixir
        assert_eq!([with(&key(10040), 0, 0), with(&key(10040), 1, 0), with(&key(10040), 2, 0), with(&key(10040), 2, 2)], ["reward acc_slot", "reward acc_slot", "reward def_slot", "Megalixir"]);
        // Larval Tear: an armour slot first
        assert_eq!([with(&key(8185), 2, 0), with(&key(8185), 0, 2), with(&key(8185), 2, 2)], ["reward def_slot", "reward acc_slot", "Orichalcum+"]);
        assert_eq!(with(&key(10030), 0, 0), "reward max_mp:5", "Memory Stone");
        assert_eq!([with(&key(8590), 0, 0), with(&key(8972), 0, 0)], ["reward ap:1", "reward ap:1"], "Whetstone Knife, a Whetblade");
        assert!(with(&key(9300), 0, 0).ends_with(" Gem"), "a cookbook: {}", with(&key(9300), 0, 0));
        let c = |item: &ErItem| r.classify(item, &Ctx::of(Source::World, 0));
        let (heart, root, lost) = (c(&key(10060)), c(&key(2090)), c(&key(10070)));
        assert!(heart.tier == 4 && tags(&r, &heart).contains(&"magic".to_string()), "{heart:?}");
        assert!(root.tier == 4 && tags(&r, &root).contains(&"strength".to_string()), "{root:?}");
        assert!(lost.tier == 3 && tags(&r, &lost).contains(&"ap".to_string()), "{lost:?}");
        assert_eq!([name(&c(&goods(11000, 10, 1))), name(&c(&goods(11000, 10, 3)))], ["Mega-Ether", "Elixir"], "a crystal tear by tier");
        assert_eq!(name(&c(&ErItem { consumable: false, ..goods(250, 9, 2) })), "Elixir", "the physick flask");
        assert_eq!(name(&c(&goods(10010, 11, 2))), "Megalixir");
        // switched off: they stay Elden Ring's (the key rule passes them)
        let off = LootRules::parse(&SHEET.replace("[\"progression.convert\",1,", "[\"progression.convert\",0,"), &kh_items());
        assert!(!off.convert_progression);
        for id in [10040, 10030, 8590, 9300, 8185, 10060, 2090, 10070] {
            assert_eq!(off.classify(&key(id), &Ctx::of(Source::World, 0)).give, Give::Pass, "{id}");
        }
    }

    /// CONSUMABLES AND MATERIALS follow their rules with the tier applied.
    #[test]
    fn consumables_and_materials_take_the_tier() {
        let r = rules();
        let at = |item: &ErItem, source: Source, depth: u8| name(&r.classify(item, &Ctx::of(source, depth)));
        // a Limgrave soldier's common material: a Shard; the same material from a chest: a Stone; late: a Gem / Crystal
        let material = goods(20650, 2, 0);
        assert_eq!([at(&material, Source::Enemy, 0), at(&material, Source::Chest, 0), at(&material, Source::Chest, 2), at(&material, Source::Chest, 4), at(&material, Source::Boss, 4)], ["Betwixt Shard", "Betwixt Stone", "Betwixt Gem", "Betwixt Crystal", "Betwixt Crystal"]);
        assert_eq!(at(&goods(20651, 2, 0), Source::Enemy, 0), "Twilight Shard", "one family per Elden Ring id");
        let heal = |effect: Effect, rarity: u8, source: Source| at(&ErItem { effect, ..goods(900, 0, rarity) }, source, 0);
        assert_eq!([heal(Effect::Hp, 0, Source::Enemy), heal(Effect::Hp, 1, Source::Enemy), heal(Effect::Hp, 0, Source::Chest), heal(Effect::Hp, 2, Source::Chest), heal(Effect::Hp, 3, Source::Boss)], ["Potion", "Potion", "Hi-Potion", "Mega-Potion", "Elixir"]);
        assert_eq!([heal(Effect::Fp, 0, Source::Enemy), heal(Effect::Fp, 1, Source::Enemy), heal(Effect::Fp, 2, Source::Enemy)], ["Ether", "Ether", "Hi-Ether"]);
        assert_eq!([heal(Effect::Both, 0, Source::Enemy), heal(Effect::Both, 3, Source::Boss)], ["Elixir", "Megalixir"]);
        assert_eq!(heal(Effect::Cure, 0, Source::Enemy), "Panacea");
        assert_eq!(r.classify(&ErItem { effect: Effect::Runes, soul: 800, ..goods(2903, 0, 0) }, &Ctx::of(Source::World, 0)).give, Give::Munny(800), "a rune item: its runes");
        let ore = |id: u32| at(&goods(id, 14, 0), Source::Chest, 4);
        assert_eq!([ore(10100), ore(10103), ore(10105), ore(10107), ore(10140), ore(10200)], ["Fluorite", "Damascus", "Adamantite", "Electrum", "Orichalcum", "Orichalcum+"], "a stone's own grade, whatever the source");
        assert_eq!([at(&goods(10900, 14, 0), Source::Enemy, 0), at(&goods(10900, 14, 1), Source::Chest, 0), at(&goods(10900, 14, 2), Source::Chest, 0)], ["Fluorite", "Damascus", "Adamantite"], "another reinforcement material: by tier");
        assert!(at(&goods(1100, 0, 0), Source::Enemy, 0).ends_with("Shard"), "another consumable");
    }

    /// WHAT SORA CANNOT USE BECOMES EQUIPMENT OF ITS TIER, by the kind of thing it is; always the same piece for the
    /// same Elden Ring item.
    #[test]
    fn unusable_things_become_equipment_of_their_tier() {
        let r = rules();
        let c = |item: &ErItem, source: Source, depth: u8| r.classify(item, &Ctx::of(source, depth));
        let tier_of_give = |d: &Decision| match &d.give {
            Give::Kh { key, .. } => r.ranked().iter().find(|x| &x.key == key).map(|x| x.tier),
            _ => None,
        };
        let has = |d: &Decision, tag: &str| tags(&r, d).iter().any(|t| t == tag);
        let armour = |d: &Decision| matches!(&d.give, Give::Kh { key, .. } if key.starts_with("PRT_"));
        // ---- a chest's talisman in Limgrave (rarity 1): tier 2 (IMPL_ER61: rarity 1 = 0.5 points), an accessory of tier 2
        let talisman = ErItem { cat: Some(Cat::Accessory), id: 1040, rarity: 1, known: true, ..ErItem::default() };
        let d = c(&talisman, Source::Chest, 0);
        assert_eq!((d.tier, tier_of_give(&d), d.rule.as_str(), armour(&d)), (2, Some(2), "r2.910.accessory", false), "{d:?}");
        assert!(tier_of_give(&c(&talisman, Source::Chest, 3)) > tier_of_give(&d), "the same talisman found late is a better accessory");
        // ---- a legendary armament (rarity 3, somber) from a DLC chest: one of the best accessories
        let legend = ErItem { somber: true, ..weapon(9000000, 3, WepKind::Melee) };
        let d = c(&legend, Source::Chest, 4);
        assert_eq!((d.tier, tier_of_give(&d), armour(&d)), (6, Some(6), false), "{d:?}");
        assert!(["Crystal Regalia+", "Cosmic Ring", "Crystal Regalia", "Mickey Clasp"].contains(&name(&d).as_str()), "{}", name(&d));
        // (IMPL_ER61) the same armament from a Limgrave chest (value tier 5): an accessory of tier 2 at most (`tier.equip_cap`)
        let early = c(&legend, Source::Chest, 0);
        assert_eq!((early.tier, tier_of_give(&early)), (5, Some(2)), "{early:?}");
        // ---- determinism: the same item, the same piece, at every reinforcement level and affinity; another item, maybe another
        assert_eq!(c(&legend, Source::Chest, 4), c(&legend, Source::Chest, 4));
        assert_eq!(c(&ErItem { id: 9000000 + 110, ..legend }, Source::Chest, 4).give, d.give, "+10 with an affinity is the same armament");
        let sword = weapon(2000000, 1, WepKind::Melee);
        let picks: std::collections::HashSet<String> = (0..40u32).map(|k| name(&c(&ErItem { id: 1000000 + k * 10000, rarity: 2, ..sword }, Source::World, 1))).collect();
        assert!(picks.len() >= 3, "different armaments spread over the pool: {picks:?}");
        // ---- by kind: a sword Strength, a staff Magic, a fire sword fire or Magic, a shield armour, a fire shield fire armour
        let d = c(&sword, Source::World, 1);
        assert!(d.tier == 2 && has(&d, "strength") && !armour(&d), "{d:?}");
        let d = c(&weapon(33000000, 1, WepKind::Staff), Source::World, 1);
        assert!(has(&d, "magic") && !armour(&d), "{d:?}");
        let d = c(&ErItem { element: Element::Fire, ..sword }, Source::Chest, 1);
        assert!((has(&d, "fire") || has(&d, "magic")) && !armour(&d), "{d:?}");
        let d = c(&weapon(30000000, 1, WepKind::Shield), Source::World, 1);
        assert!(armour(&d) && has(&d, "defence") && d.rule == "r2.720.shield", "{d:?}");
        let d = c(&ErItem { element: Element::Fire, ..weapon(30000000, 1, WepKind::Shield) }, Source::World, 2);
        assert_eq!((name(&d), d.tier), ("Fira Bangle".to_string(), 3), "the fire armour of tier 3");
        // ---- an armour piece: its element at its tier (IMPL_ER61: tiers 2, 3, 4, 5 = Liurnia enemy, Liurnia chest, Caelid
        // chest, a rare piece from a Mountaintops chest)
        let robe = |element: Element, rarity: u8, source: Source, depth: u8| name(&c(&ErItem { cat: Some(Cat::Protector), id: 40000, rarity, element, known: true, ..ErItem::default() }, source, depth));
        assert_eq!([robe(Element::Fire, 1, Source::Enemy, 1), robe(Element::Fire, 1, Source::Chest, 1), robe(Element::Fire, 1, Source::Chest, 2), robe(Element::Fire, 2, Source::Chest, 3)], ["Fire Bangle", "Fira Bangle", "Firaga Bangle", "Firaza Bangle"]);
        assert_eq!([robe(Element::Holy, 1, Source::Chest, 1), robe(Element::Magic, 1, Source::Chest, 1), robe(Element::Lightning, 1, Source::Chest, 1)], ["Dark Anklet", "Blizzara Choker", "Thundara Trinket"]);
        assert!(["Shield Belt", "Defense Belt"].contains(&robe(Element::None, 0, Source::Enemy, 0).as_str()));
        // (IMPL_ER61) in Limgrave the best of them is a tier-2 piece
        assert_eq!(name(&c(&ErItem { cat: Some(Cat::Protector), id: 40000, rarity: 3, element: Element::Fire, known: true, ..ErItem::default() }, Source::Chest, 0)), "Fire Bangle");
        // ---- a spirit ash: a support accessory; an Ash of War: an AP accessory; a sorcery: a Magic accessory
        let d = c(&ErItem { consumable: false, ..goods(200000, 7, 1) }, Source::World, 0);
        assert!(d.rule == "r2.510.spirit_ash" && d.tier == 2 && has(&d, "support"), "{d:?} {:?}", tags(&r, &d));
        let d = c(&ErItem { cat: Some(Cat::Gem), id: 10000, rarity: 1, known: true, ..ErItem::default() }, Source::Chest, 0);
        assert!(d.rule == "r2.800.gem" && d.tier == 2 && has(&d, "ap"), "{d:?} {:?}", tags(&r, &d));
        let d = c(&ErItem { consumable: false, ..goods(21, 5, 1) }, Source::Shop, 0); // (SHOP-4: real sorceries a teacher's talk counts stay Elden Ring's: a free id)
        assert!(d.rule == "r2.500.spell" && has(&d, "magic"), "{d:?}");
        // ---- where equipment makes no sense: ammunition and an enemy's tier-1 armament are munny
        assert_eq!(c(&ErItem { sell_value: 2, ..weapon(50000000, 0, WepKind::Ammo) }, Source::Chest, 3).give, Give::Munny(2));
        assert_eq!(c(&ErItem { sell_value: 0, ..weapon(50000000, 0, WepKind::Ammo) }, Source::Chest, 3).give, Give::Nothing);
        assert_eq!((c(&weapon(1000000, 0, WepKind::Melee), Source::Enemy, 0).give, c(&weapon(1000000, 0, WepKind::Melee), Source::Enemy, 0).rule), (Give::Munny(100), "r2.710.junk_weapon".to_string()));
        assert!(name(&c(&ErItem { sell_value: 0, ..weapon(1000000, 0, WepKind::Melee) }, Source::Enemy, 0)).ends_with("Shard"));
        assert!(matches!(c(&weapon(1000000, 0, WepKind::Melee), Source::World, 0).give, Give::Kh { .. }), "the same sword from a corpse is equipment");
        assert_eq!(c(&weapon(1000000, 1, WepKind::Melee), ENEMY.source, 0).rule, "r2.710.junk_weapon", "rarity 1 is every soldier's armament (the user's log): tier 2 from an enemy is junk too");
        // (IMPL_ER61: rarity 2 = 1.5 points) a rare one from a Limgrave enemy is junk too (tier 2); from Caelid on equipment
        assert_eq!(c(&weapon(1000000, 2, WepKind::Melee), ENEMY.source, 0).rule, "r2.710.junk_weapon");
        assert!(matches!(c(&weapon(1000000, 2, WepKind::Melee), ENEMY.source, 2).give, Give::Kh { .. }), "a rare one from an enemy is equipment");
        // (IMPL_ER61) FEWER PIECES: with a random number the equipment rules give a material or munny first; the
        // last alternative (the reading of a shop line, unit 0.999) is still the piece
        let at = |item: &ErItem, unit: f32| r.classify(item, &Ctx { unit, ..Ctx::of(Source::World, 1) }).give;
        let sword = ErItem { sell_value: 100, ..weapon(2000000, 1, WepKind::Melee) };
        assert!(matches!(at(&sword, 0.10), Give::Kh { ref key, .. } if key.starts_with("MAT_")), "45 % a material");
        assert_eq!(at(&sword, 0.50), Give::Munny(100), "25 % munny");
        assert!(matches!(at(&sword, 0.80), Give::Kh { ref key, .. } if key.starts_with("ACC_")), "30 % the accessory");
        let mut rng = kh_prize::Rng::new(5);
        let pieces = |item: &ErItem, rng: &mut kh_prize::Rng| (0..10_000).filter(|_| matches!(at(item, rng.unit()), Give::Kh { ref key, .. } if key.starts_with("ACC_") || key.starts_with("PRT_"))).count() as f32 / 10_000.0;
        let robe = ErItem { cat: Some(Cat::Protector), id: 40000, rarity: 1, sell_value: 100, known: true, ..ErItem::default() };
        let talisman = ErItem { sell_value: 100, ..talisman };
        let (w, p, t) = (pieces(&sword, &mut rng), pieces(&robe, &mut rng), pieces(&talisman, &mut rng));
        assert!((w - 0.30).abs() < 0.02 && (p - 0.35).abs() < 0.02 && (t - 0.70).abs() < 0.02, "weapon {w} armour {p} talisman {t}");
    }

    /// NO FLOOD OF DUPLICATES: at two copies the next piece of the pool is given; a pool used up gives a material
    /// of the tier's grade.
    #[test]
    fn at_two_copies_the_next_piece_of_the_pool_is_given() {
        let r = rules();
        // (IMPL_ER61: rarity 2, so that it is still tier 3 on a Liurnia corpse)
        let sword = weapon(2000000, 2, WepKind::Melee);
        let key_of = |d: &Decision| match &d.give {
            Give::Kh { key, .. } => key.clone(),
            other => panic!("{other:?}"),
        };
        let first = key_of(&r.classify(&sword, &Ctx::of(Source::World, 1)));
        let one = |k: &str| if k == first { 1 } else { 0 };
        assert_eq!(key_of(&r.classify(&sword, &Ctx { source: Source::World, depth: 1, state: &one, unit: 0.999 })), first, "one copy held: the same piece again");
        let two = |k: &str| if k == first { 2 } else { 0 };
        let second = r.classify(&sword, &Ctx { source: Source::World, depth: 1, state: &two, unit: 0.999 });
        assert_ne!(key_of(&second), first);
        assert_eq!(r.ranked().iter().find(|x| x.key == key_of(&second)).unwrap().tier, 3, "the next piece is of the same tier");
        assert!(tags(&r, &second).contains(&"strength".to_string()), "and of the same kind while there is one");
        // every Strength piece of the tier held twice: any other piece of the tier
        let strength: Vec<String> = r.ranked().iter().filter(|x| !x.armour && x.tier == 3 && x.tags.iter().any(|t| t == "strength")).map(|x| x.key.clone()).collect();
        let all_strength = |k: &str| if strength.iter().any(|s| s == k) { 2 } else { 0 };
        let other = r.classify(&sword, &Ctx { source: Source::World, depth: 1, state: &all_strength, unit: 0.999 });
        assert!(!tags(&r, &other).contains(&"strength".to_string()) && r.ranked().iter().find(|x| x.key == key_of(&other)).unwrap().tier == 3);
        // the whole tier held twice: a material of the tier's grade (a Gem at tier 3); `equip.dup_max` is the sheet's
        let all = |k: &str| if k.starts_with("ACC_") { 2 } else { 0 };
        assert!(name(&r.classify(&sword, &Ctx { source: Source::World, depth: 1, state: &all, unit: 0.999 })).ends_with(" Gem"));
        let three = model_of(&SHEET.replace("[\"equip.dup_max\",2,", "[\"equip.dup_max\",3,"));
        assert_eq!(key_of(&three.classify(&sword, &Ctx { source: Source::World, depth: 1, state: &two, unit: 0.999 })), first);
    }

    /// A MERCHANT'S LINE under `shop.kh`: key items stay, mapped items are sold at KH3's price (or the tier's), the
    /// rest is dropped.
    #[test]
    fn a_shop_sells_kh_items_at_kh3s_prices() {
        let r = rules();
        assert_eq!(r.shop_line(&ErItem { consumable: false, ..goods(8000, 1, 0) }, 0).0, ShopLine::Keep, "a Stonesword Key");
        let price = |item: &ErItem| match r.shop_line(item, 0).0 {
            ShopLine::Sell { key, price, .. } => (kh_host::items::by_key(&key).unwrap().name.clone(), price),
            other => panic!("{other:?}"),
        };
        assert_eq!(price(&ErItem { effect: Effect::Hp, ..goods(900, 0, 0) }), ("Potion".to_string(), 40), "KH3's own price");
        assert_eq!(price(&goods(10100, 14, 0)).0, "Fluorite");
        assert_eq!(price(&goods(20650, 2, 0)), ("Betwixt Shard".to_string(), 100), "KH3 sells its materials too");
        let (sold, cost) = price(&weapon(2000000, 1, WepKind::Melee));
        let item = kh_host::items::table().iter().find(|i| i.name == sold).unwrap();
        // (IMPL_ER61) a Limgrave shop's rarity-1 sword is tier 1 now: the tier-1 price
        assert_eq!(cost, if item.buy > 0 { item.buy } else { 300 }, "{sold}");
        assert!(matches!(r.shop_line(&ErItem { sell_value: 2, ..weapon(50000000, 0, WepKind::Ammo) }, 0).0, ShopLine::Drop(_)), "arrows: munny is not sold");
        assert!(matches!(r.shop_line(&ErItem { consumable: false, ..goods(10030, 1, 1) }, 0).0, ShopLine::Drop(_)), "a Memory Stone: a reward is not sold");
    }

    /// The worked examples of docs/sora-er-items.md (printed: `cargo test -p kh-er --lib lootmap::tests::worked -- --nocapture`).
    #[test]
    fn worked_examples() {
        let r = rules();
        let show = |what: &str, item: &ErItem, source: Source, depth: u8| {
            let d = r.classify(item, &Ctx::of(source, depth));
            println!("{what}: {} | {} | {} | {:?}", name(&d), d.why, d.rule, tags(&r, &d));
        };
        show("a Limgrave soldier's common crafting material", &goods(20650, 2, 0), Source::Enemy, 0);
        show("the same material from a Limgrave chest", &goods(20650, 2, 0), Source::Chest, 0);
        show("a talisman (rarity 1) from a Limgrave chest", &ErItem { cat: Some(Cat::Accessory), id: 1040, rarity: 1, known: true, ..ErItem::default() }, Source::Chest, 0);
        show("a legendary somber armament from a Mountaintops chest", &ErItem { somber: true, ..weapon(9000000, 3, WepKind::Melee) }, Source::Chest, 3);
        show("a spirit ash (rarity 1) on a Limgrave corpse", &ErItem { consumable: false, ..goods(200000, 7, 1) }, Source::World, 0);
        show("an Ash of War (rarity 1) from a Limgrave scarab", &ErItem { cat: Some(Cat::Gem), id: 10000, rarity: 1, known: true, ..ErItem::default() }, Source::Enemy, 0);
        show("a soldier's sword (rarity 1, as the user's log shows them) dropped in Limgrave", &weapon(2040000, 1, WepKind::Melee), Source::Enemy, 0);
        show("an uncommon sword on a Liurnia corpse", &weapon(2000000, 1, WepKind::Melee), Source::World, 1);
        show("a staff (rarity 1) on a Liurnia corpse", &weapon(33000000, 1, WepKind::Staff), Source::World, 1);
        show("a fire-guarding armour piece (rarity 2) from a Caelid chest", &ErItem { cat: Some(Cat::Protector), id: 40000, rarity: 2, element: Element::Fire, known: true, ..ErItem::default() }, Source::Chest, 2);
        show("a medium shield (rarity 1) on a Limgrave corpse", &weapon(30000000, 1, WepKind::Shield), Source::World, 0);
        show("a sorcery (rarity 2) from an Altus chest", &ErItem { consumable: false, ..goods(4000, 5, 2) }, Source::Chest, 2);
        show("a Stonesword Key", &ErItem { consumable: false, ..goods(8000, 1, 0) }, Source::World, 0);
        show("a bell bearing", &ErItem { consumable: false, ..goods(8951, 1, 0) }, Source::Boss, 0);
        show("a Talisman Pouch", &ErItem { consumable: false, ..goods(10040, 1, 1) }, Source::Boss, 1);
        show("a stack of arrows", &ErItem { sell_value: 2, ..weapon(50000000, 0, WepKind::Ammo) }, Source::World, 0);
        for tier in 1..=6u8 {
            let of = |armour: bool| r.ranked().iter().filter(|x| x.armour == armour && x.tier == tier).map(|x| kh_host::items::by_key(&x.key).unwrap().name.clone()).collect::<Vec<_>>();
            println!("TIER {tier} accessories ({}): {}", of(false).len(), of(false).join(", "));
            println!("TIER {tier} armour ({}): {}", of(true).len(), of(true).join(", "));
        }
    }

    /// (er30) POTIONS AND ETHERS LEAN IN: a crafting material is 25 % a Potion-line item, 12 % an Ether-line item,
    /// 23 % its sell value in munny (IMPL_ER61: fewer items), else its synthesis material; another consumable 30 % / 15 %
    /// / 20 %; by tier; the same number gives the same item. (A thing without a sell value skips the munny.)
    #[test]
    fn a_material_is_often_a_potion_or_an_ether() {
        let r = rules();
        let at = |item: &ErItem, unit: f32, source: Source, depth: u8| name(&r.classify(item, &Ctx { unit, ..Ctx::of(source, depth) }));
        let material = goods(15340, 2, 0);
        assert_eq!([at(&material, 0.0, Source::Enemy, 0), at(&material, 0.249, Source::Enemy, 0), at(&material, 0.251, Source::Enemy, 0), at(&material, 0.369, Source::Enemy, 0), at(&material, 0.371, Source::Enemy, 0), at(&material, 0.999, Source::Enemy, 0)], ["Potion", "Potion", "Ether", "Ether", "Blazing Shard", "Blazing Shard"]);
        let sold = ErItem { sell_value: 10, ..material };
        assert_eq!([at(&sold, 0.371, Source::Enemy, 0), at(&sold, 0.599, Source::Enemy, 0), at(&sold, 0.601, Source::Enemy, 0)], ["10 munny", "10 munny", "Blazing Shard"]);
        assert_eq!([at(&material, 0.1, Source::Chest, 0), at(&material, 0.3, Source::Chest, 0), at(&material, 0.1, Source::Chest, 2), at(&material, 0.3, Source::Chest, 4)], ["Hi-Potion", "Hi-Ether", "Mega-Potion", "Mega-Ether"], "by tier");
        let other = goods(1730, 0, 0);
        assert_eq!([at(&other, 0.299, Source::World, 0), at(&other, 0.301, Source::World, 0), at(&other, 0.449, Source::World, 0), at(&other, 0.451, Source::World, 0)].map(|n| n.ends_with("Shard")), [false, false, false, true]);
        assert_eq!([at(&other, 0.299, Source::World, 0), at(&other, 0.449, Source::World, 0)], ["Potion", "Ether"]);
        // measured with the game's generator
        let mut rng = kh_prize::Rng::new(11);
        let (mut potion, mut ether, mut shard) = (0, 0, 0);
        for _ in 0..20_000 {
            match at(&material, rng.unit(), Source::Enemy, 0).as_str() {
                "Potion" => potion += 1,
                "Ether" => ether += 1,
                _ => shard += 1,
            }
        }
        assert!((potion as f32 / 20_000.0 - 0.25).abs() < 0.012 && (ether as f32 / 20_000.0 - 0.12).abs() < 0.01 && (shard as f32 / 20_000.0 - 0.63).abs() < 0.012, "{potion} {ether} {shard}");
        // a smithing stone and equipment are not touched by the split
        assert_eq!(at(&goods(10100, 14, 1), 0.0, Source::Enemy, 0), "Fluorite");
        // a broken chance text is reported
        assert!(LootRules::parse(&SHEET.replace("chance:25:bytier:ITEM_POTION", "chance:25:bytier:ITEM_NO_SUCH"), &kh_items()).problems.iter().any(|p| p.contains("r2.400")));
    }

    /// (er30) THE BASE CONSUMABLE DROP: the rates over many seeded kills (KH3's roll: rate / 99), the class by
    /// maximum HP, the better lines by class and region, Lucky Strike, the same seed the same drops.
    #[test]
    fn every_kill_rolls_a_potion_and_an_ether() {
        let r = rules();
        assert!(r.base_on);
        assert_eq!([r.base_class(150), r.base_class(699), r.base_class(700), r.base_class(2499), r.base_class(2500), r.base_class(40000)], [0, 0, 1, 1, 2, 2]);
        let run = |class: usize, depth: u8, luck: u32, kills: u32, seed: u32| {
            let mut rng = kh_prize::Rng::new(seed);
            let mut counts: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
            for _ in 0..kills {
                for (id, ..) in r.base_drops(class, depth, luck, &mut || rng.unit()) {
                    *counts.entry(kh_host::items::by_id(id).unwrap().name.clone()).or_default() += 1;
                }
            }
            counts
        };
        let share = |counts: &std::collections::BTreeMap<String, u32>, name: &str, kills: u32| counts.get(name).copied().unwrap_or(0) as f32 / kills as f32;
        // an ordinary Limgrave enemy: Potion 12 / 99, Ether 6 / 99, nothing else (IMPL_ER61: was 15 and 8)
        let c = run(0, 0, 0, 40_000, 7);
        assert!((share(&c, "Potion", 40_000) - 12.0 / 99.0).abs() < 0.006 && (share(&c, "Ether", 40_000) - 6.0 / 99.0).abs() < 0.005, "{c:?}");
        assert_eq!(c.keys().map(String::as_str).collect::<Vec<_>>(), ["Ether", "Potion"]);
        // Lucky Strike: round(12 x 1.3) = 16, round(6 x 1.3) = 8; three copies: round(12 x 1.9) = 23
        let c = run(0, 0, 1, 40_000, 7);
        assert!((share(&c, "Potion", 40_000) - 16.0 / 99.0).abs() < 0.006 && (share(&c, "Ether", 40_000) - 8.0 / 99.0).abs() < 0.005, "{c:?}");
        let c = run(0, 0, 3, 40_000, 7);
        assert!((share(&c, "Potion", 40_000) - 23.0 / 99.0).abs() < 0.007, "{c:?}");
        // a big enemy: 28 % and 15 %, a rare Elixir (2 %); still Potion / Ether in Limgrave (level 1)
        let c = run(1, 0, 0, 40_000, 9);
        assert!((share(&c, "Potion", 40_000) - 28.0 / 99.0).abs() < 0.008 && (share(&c, "Ether", 40_000) - 15.0 / 99.0).abs() < 0.007 && (share(&c, "Elixir", 40_000) - 2.0 / 99.0).abs() < 0.004, "{c:?}");
        // a boss: always the better items (Limgrave: level 2 = Hi-Potion and Hi-Ether), an Elixir from a fifth
        let c = run(2, 0, 0, 4_000, 3);
        assert_eq!((c["Hi-Potion"], c["Hi-Ether"]), (4_000, 4_000));
        assert!((share(&c, "Elixir", 4_000) - 20.0 / 99.0).abs() < 0.025 && !c.contains_key("Potion"), "{c:?}");
        // the region: ordinary enemies of Caelid / Altus drop Hi-Potions, of the DLC Mega-Potions; a DLC boss a Megalixir at times
        assert!(run(0, 2, 0, 2_000, 5).contains_key("Hi-Potion") && !run(0, 2, 0, 2_000, 5).contains_key("Potion"));
        assert!(run(0, 4, 0, 2_000, 5).contains_key("Mega-Potion") && run(0, 4, 0, 2_000, 5).contains_key("Mega-Ether"));
        assert!(run(2, 4, 0, 2_000, 5).contains_key("Megalixir") && run(2, 3, 0, 2_000, 5).contains_key("Mega-Potion"));
        // the same seed, the same drops; another seed, others
        assert_eq!(run(0, 0, 0, 500, 21), run(0, 0, 0, 500, 21));
        assert_ne!(run(0, 0, 0, 500, 21), run(0, 0, 0, 500, 22));
        // switched off by the sheet; the first-generation sheet has none
        assert!(LootRules::parse(&SHEET.replace("[\"base.on\",1,", "[\"base.on\",0,"), &kh_items()).base_drops(2, 0, 0, &mut || 0.0).is_empty());
        assert_eq!(r.base_drops(0, 0, 0, &mut || 0.0).len(), 2, "the lowest roll drops every line with a rate");
        assert!(r.base_drops(0, 0, 0, &mut || 0.999).is_empty());
    }

    #[test]
    fn effects_and_elements_are_read_from_the_param_values() {
        assert_eq!(Effect::of_sp_effect(-300.0, 0.0, 0), Effect::Hp);
        assert_eq!(Effect::of_sp_effect(0.0, -80.0, 0), Effect::Fp);
        assert_eq!(Effect::of_sp_effect(-1.0, -1.0, 0), Effect::Both);
        assert_eq!(Effect::of_sp_effect(0.0, 0.0, 200), Effect::Runes);
        assert_eq!(Effect::of_sp_effect(5.0, 0.0, 0), Effect::None, "a positive change is damage");
        assert_eq!(Element::of_cut_rates(0.9, 0.8, 0.9, 0.9, 0.02), Element::Fire);
        assert_eq!(Element::of_cut_rates(0.9, 0.9, 0.9, 0.9, 0.02), Element::None);
        assert_eq!(Element::of_cut_rates(0.90, 0.89, 0.90, 0.90, 0.02), Element::None, "inside the margin");
        assert_eq!(Element::of_cut_rates(0.95, 0.95, 0.95, 0.7, 0.02), Element::Holy);
        assert_eq!([Element::of_attack(0, 0, 0, 0), Element::of_attack(0, 80, 0, 0), Element::of_attack(60, 0, 0, 70)], [Element::None, Element::Fire, Element::Holy]);
        assert_eq!(ErItem { cat: Some(Cat::Weapon), id: 9000110, ..ErItem::default() }.base_id(), 9000000);
        assert_eq!(ErItem { cat: Some(Cat::Goods), id: 9000110, ..ErItem::default() }.base_id(), 9000110);
    }

    fn slot(category: i32, item: i32, points: u32, luck: bool) -> LotSlot {
        LotSlot { category, item, points, count: 1, luck, flag: 0 }
    }

    /// THE ROLL ARITHMETIC of docs/sora-er-items.md section 4 (the three worked examples).
    #[test]
    fn the_lot_roll_is_elden_rings_pick_with_kh3s_luck() {
        let r = rules();
        assert!((r.luck_factor(0) - 1.0).abs() < 1.0e-6 && (r.luck_factor(1) - 1.3).abs() < 1.0e-6 && (r.luck_factor(3) - 1.9).abs() < 1.0e-6);
        // EXAMPLE 1: a soldier's lot: nothing 920, a crafting material 50 (luck), a consumable 30 (luck)
        let mut row = LotRow { id: 431000000, ..LotRow::default() };
        row.slots[0] = slot(0, 0, 920, false);
        row.slots[1] = slot(1, 20650, 50, true);
        row.slots[2] = slot(1, 1100, 30, true);
        assert_eq!(row.weights(1.0), [920, 50, 30, 0, 0, 0, 0, 0]);
        let c = row.chances(1.0);
        assert!((c[0] - 0.92).abs() < 1.0e-6 && (c[1] - 0.05).abs() < 1.0e-6 && (c[2] - 0.03).abs() < 1.0e-6);
        // one Lucky Strike: 50 -> 65, 30 -> 39; nothing stays 920: total 1024
        assert_eq!(row.weights(r.luck_factor(1)), [920, 65, 39, 0, 0, 0, 0, 0]);
        let c = row.chances(r.luck_factor(1));
        assert!((c[1] - 65.0 / 1024.0).abs() < 1.0e-6 && (c[1] + c[2] - 104.0 / 1024.0).abs() < 1.0e-6);
        // three copies: 95 and 57: an item from 14.2 % of the kills instead of 8 %
        let c = row.chances(r.luck_factor(3));
        assert!((c[1] + c[2] - 152.0 / 1072.0).abs() < 1.0e-6, "{c:?}");
        // the roll walks the slots in order
        assert_eq!([row.roll(1.0, 0.0), row.roll(1.0, 0.9199), row.roll(1.0, 0.9201), row.roll(1.0, 0.9699), row.roll(1.0, 0.9701), row.roll(1.0, 0.9999)], [Some(0), Some(0), Some(1), Some(1), Some(2), Some(2)]);
        // measured over many rolls with the game's generator
        let mut rng = kh_prize::Rng::new(5);
        let mut hits = [0u32; 8];
        for _ in 0..100_000 {
            hits[row.roll(r.luck_factor(1), rng.unit()).unwrap()] += 1;
        }
        assert!((hits[1] as f32 / 100_000.0 - 65.0 / 1024.0).abs() < 0.004 && (hits[2] as f32 / 100_000.0 - 39.0 / 1024.0).abs() < 0.004, "{hits:?}");
        // EXAMPLE 2: a rare armour piece: nothing 980, the piece 20 (luck): 2 % -> 2.58 % -> 3.73 %
        let mut rare = LotRow::default();
        rare.slots[0] = slot(0, 0, 980, false);
        rare.slots[1] = slot(3, 40000, 20, true);
        let chance = |n: u32| rare.chances(r.luck_factor(n))[1];
        assert!((chance(0) - 0.02).abs() < 1.0e-6 && (chance(1) - 26.0 / 1006.0).abs() < 1.0e-6 && (chance(3) - 38.0 / 1018.0).abs() < 1.0e-6);
        // EXAMPLE 3: a guaranteed drop (one slot with all the points) stays guaranteed whatever the luck
        let mut sure = LotRow::default();
        sure.slots[0] = slot(1, 10100, 1000, false);
        for n in 0..4 {
            assert_eq!(sure.chances(r.luck_factor(n))[0], 1.0);
            assert_eq!(sure.roll(r.luck_factor(n), 0.73), Some(0));
        }
        // a row without points drops nothing; a flagged row is not a plain drop
        assert_eq!(LotRow::default().roll(1.0, 0.5), None);
        assert!(row.plain());
        let mut flagged = row;
        flagged.slots[1].flag = 400123;
        assert!(!flagged.plain());
        assert!(!LotRow { flag: 5, ..row }.plain());
    }
}

// ------------------------------------------------------------------------------- (ER party S6, U4) the friends' gear

/// (ER party S6, U4: "the friends' 14 staves and 14 shields (and their accessories) ... mixed into the KH loot / chest
/// pools by rarity, as Sora's gear is") THE FRIENDS' WEAPONS IN THE LOOT. The sheet row `party.weapons` (sheets/port/
/// sora-er-items.json; the deployed builds before S6 do not read it) names which Elden Ring armaments become whose
/// weapon (a staff or a sacred seal -> Donald's staff, a shield -> Goofy's shield) and which share of them (a stable hash
/// of the Elden Ring id: the same item is always the same answer); the rest stay what the model made of them (a Magic
/// accessory, an armour piece). The weapon is the friend's of the item's value TIER (the model's tier 1..T over his 14
/// weapons in KH3's order, weakest first), the stable pick inside the tier's band, a held one stepped past
/// (`dup_max`). Not in a shop (KH3 sells no friend weapons). Only for a friend the party has unlocked
/// ([`set_party_friends`], the host's every frame); the friends' accessories and armour are the shared KH3 pieces the
/// model already gives (they wear what Sora wears).
#[cfg(feature = "party")]
#[derive(Clone, Debug, PartialEq)]
pub struct PartyWeapons {
    /// (Elden Ring armament kind, the friend).
    pub kinds: Vec<(WepKind, kh_host::party::PartyChara)>,
    /// Percent of those items that become the friend's weapon.
    pub share: u32,
    /// At most this many of one weapon (in the stock plus queued).
    pub dup_max: u32,
}

#[cfg(feature = "party")]
impl PartyWeapons {
    pub fn parse(value: &serde_json::Value) -> Result<PartyWeapons, String> {
        let mut kinds = Vec::new();
        for (k, v) in value["kinds"].as_object().into_iter().flatten() {
            let kind = WepKind::from_name(k).ok_or_else(|| format!("party.weapons: no weapon kind {k}"))?;
            let who = v.as_str().and_then(kh_host::formation::chara_of).filter(|c| *c != kh_host::party::PartyChara::Sora).ok_or_else(|| format!("party.weapons: {k} -> {v} is not donald | goofy"))?;
            kinds.push((kind, who));
        }
        Ok(PartyWeapons { kinds, share: value["share"].as_u64().map_or(50, |s| s.min(100) as u32), dup_max: value["dup_max"].as_u64().map_or(1, |d| d.max(1) as u32) })
    }
}

/// (ER party S6) The friends the party has unlocked, for the loot (the host sets it every frame; empty = none: no friend
/// weapon drops).
#[cfg(feature = "party")]
static PARTY_FRIENDS: std::sync::Mutex<Vec<kh_host::party::PartyChara>> = std::sync::Mutex::new(Vec::new());

#[cfg(feature = "party")]
pub fn set_party_friends(friends: &[kh_host::party::PartyChara]) {
    if let Ok(mut f) = PARTY_FRIENDS.lock() {
        if f.as_slice() != friends {
            *f = friends.to_vec();
        }
    }
}

#[cfg(feature = "party")]
fn party_friends() -> Vec<kh_host::party::PartyChara> {
    PARTY_FRIENDS.lock().map(|f| f.clone()).unwrap_or_default()
}

/// The friend's weapon keys in KH3's order (weakest first): `WEP_DONALD_00` .. `_13`.
#[cfg(feature = "party")]
pub fn party_weapon_keys(c: kh_host::party::PartyChara) -> Vec<String> {
    let prefix = if c == kh_host::party::PartyChara::Donald { "WEP_DONALD" } else { "WEP_GOOFY" };
    (0..14).map(|n| format!("{prefix}_{n:02}")).collect()
}

#[cfg(feature = "party")]
impl LootRules {
    /// THE RULE (module text of [`PartyWeapons`]): `Some` = this item becomes a friend's weapon.
    pub fn party_weapon(&self, item: &ErItem, ctx: &Ctx, friends: &[kh_host::party::PartyChara]) -> Option<Decision> {
        let rule = self.party_weapons.as_ref()?;
        if item.cat != Some(Cat::Weapon) || ctx.source == Source::Shop || !item.known {
            return None;
        }
        let who = rule.kinds.iter().find(|k| k.0 == item.wep_kind)?.1;
        if !friends.contains(&who) {
            return None;
        }
        let h = stable_hash(item.cat, item.base_id() ^ 0x5A17_0000);
        if (h % 100) as u32 >= rule.share {
            return None;
        }
        let (tier, why) = self.tier_of(item, ctx);
        let keys = party_weapon_keys(who);
        let t = self.tier_count.max(1) as usize;
        let tier = tier.clamp(1, self.tier_count.max(1));
        let lo = (tier as usize - 1) * keys.len() / t;
        let hi = (tier as usize * keys.len() / t).max(lo + 1).min(keys.len());
        let band = hi - lo;
        // the stable pick in the band; held ones stepped past (to stronger ones of the band, then the band below)
        let order: Vec<usize> = (0..band).map(|k| lo + (h / 100 + k) % band).chain((0..lo).rev()).collect();
        let key = order.into_iter().map(|i| &keys[i]).find(|k| (ctx.state)(k) < rule.dup_max)?;
        let item_id = kh_host::items::party_weapon_id(key)?;
        Some(Decision { give: Give::Kh { item_id, key: key.clone() }, rule: format!("party.weapons.{}", who.name().to_ascii_lowercase()), tier, why: format!("{why}; {}'s weapon of the band {}..{} (of 14)", who.name(), lo, hi - 1), one: true })
    }
}

#[cfg(all(test, feature = "party"))]
mod party_tests {
    use super::*;
    use kh_host::party::PartyChara;

    fn rules() -> LootRules {
        LootRules::parse(SHEET, &kh_items())
    }

    fn staff(id: u32, rarity: u8) -> ErItem {
        ErItem { cat: Some(Cat::Weapon), id, rarity, known: true, wep_kind: WepKind::Staff, ..ErItem::default() }
    }

    #[test]
    fn staves_and_shields_become_the_friends_weapons_by_tier() {
        let r = rules();
        assert!(r.party_weapons.is_some(), "the sheet row party.weapons");
        let friends = [PartyChara::Donald, PartyChara::Goofy];
        let ctx = Ctx::of(Source::Chest, 0);
        let mut donald = 0;
        let mut kept = 0;
        for k in 0..200u32 {
            let item = staff(33_000_000 + k * 10_000, 0);
            match r.party_weapon(&item, &ctx, &friends) {
                Some(d) => {
                    donald += 1;
                    let Give::Kh { item_id, key } = &d.give else { panic!("{d:?}") };
                    assert!(key.starts_with("WEP_DONALD_"), "{key}");
                    assert_eq!(kh_host::items::party_weapon_key(*item_id).as_deref(), Some(key.as_str()));
                    // the same item, the same answer
                    assert_eq!(r.party_weapon(&item, &ctx, &friends), Some(d));
                }
                None => kept += 1,
            }
        }
        assert!(donald > 50 && kept > 50, "about half of the staves: {donald} weapons, {kept} kept as before");
        // a shield is Goofy's; a sword nobody's; not in a shop; not for a friend not unlocked
        let shield = ErItem { wep_kind: WepKind::Shield, ..staff(0, 0) };
        let goofy = (0..50u32).filter_map(|k| r.party_weapon(&ErItem { id: 30_000_000 + k * 10_000, ..shield }, &ctx, &friends)).next().expect("some shields");
        assert!(matches!(&goofy.give, Give::Kh { key, .. } if key.starts_with("WEP_GOOFY_")));
        assert!((0..50u32).all(|k| r.party_weapon(&ErItem { id: 30_000_000 + k * 10_000, ..shield }, &ctx, &[PartyChara::Donald]).is_none()));
        assert!((0..50u32).all(|k| r.party_weapon(&ErItem { wep_kind: WepKind::Melee, ..staff(1_000_000 + k * 10_000, 0) }, &ctx, &friends).is_none()));
        assert!((0..50u32).all(|k| r.party_weapon(&staff(33_000_000 + k * 10_000, 0), &Ctx::of(Source::Shop, 0), &friends).is_none()));
        // deeper / rarer = a stronger staff; a held one is stepped past
        let tier_of = |d: &Decision| d.tier;
        let weak = (0..200u32).filter_map(|k| r.party_weapon(&staff(33_000_000 + k * 10_000, 0), &Ctx::of(Source::Enemy, 0), &friends)).map(|d| tier_of(&d)).max().unwrap();
        let strong = (0..200u32).filter_map(|k| r.party_weapon(&staff(33_000_000 + k * 10_000, 3), &Ctx::of(Source::Boss, 4), &friends)).map(|d| tier_of(&d)).max().unwrap();
        assert!(strong > weak, "{strong} > {weak}");
        let item = (0..200u32).map(|k| staff(33_000_000 + k * 10_000, 0)).find(|i| r.party_weapon(i, &ctx, &friends).is_some()).unwrap();
        let first = r.party_weapon(&item, &ctx, &friends).unwrap();
        let Give::Kh { key: first_key, .. } = &first.give else { unreachable!() };
        let held = |k: &str| u32::from(k == first_key);
        let next = r.party_weapon(&item, &Ctx { state: &held, ..ctx }, &friends);
        assert!(next.is_none_or(|d| !matches!(&d.give, Give::Kh { key, .. } if key == first_key)), "never a second copy of a held one");
    }
}

// ------------------------------------------------------------------------------------------------ (SHOP) online items

/// (SHOP-4) The event / talk numbering of an item table (0 weapon, 1 protector, 2 accessory, 3 goods, 4 gem).
pub fn item_type(cat: Cat) -> u8 {
    match cat {
        Cat::Weapon => 0,
        Cat::Protector => 1,
        Cat::Accessory => 2,
        Cat::Goods => 3,
        Cat::Gem => 4,
    }
}

/// (SHOP-4, the user 2026-10-24: zero progression blockers) AN ITEM A GAME SCRIPT TAKES OR COUNTS (sheet sora-er-shop
/// `required.keep`, from every talk and event script of the game) STAYS ELDEN RING'S, from every source: the script finds
/// it in the inventory. (Items only whose "obtained" flag a script waits on are converted and the flag is set: loot.rs.)
pub fn script_required(item: &ErItem) -> Option<Decision> {
    let cat = item.cat?;
    let keep = &kh_host::shop::rules().required_keep;
    (keep.contains(&(item_type(cat), item.id)) || keep.contains(&(item_type(cat), item.base_id()))).then(|| Decision { give: Give::Pass, rule: "shop.script_keep".to_string(), tier: 0, why: String::new(), one: false })
}

/// (SHOP, the user 2026-10-23: "remove all online items, replace them with kingdom hearts consumable items") AN ELDEN RING
/// ONLINE ITEM (a goods row of the game's "Multiplayer Items" group: `EquipParamGoods.sortGroupId` in the sheet
/// sora-er-shop `online.sort_groups`, 120: the furled fingers, the Finger Severer, the bloody / recusant fingers, the cipher
/// rings, the effigies, the Taunter's Tongue, the Furlcalling Finger Remedy, the Wizened Finger, the phantom items) is
/// NEVER Elden Ring's: wherever it comes from (a pickup, a corpse, a gift, the start's kit, a reward) it becomes a KH
/// consumable that a merchant of the area sells (the shop's own area-tier rule: `shop.area_tier` of the region depth,
/// one of `stock_for(tier)` chosen by the item's id), from every source (the user's own replacement rule).
pub fn online_item(item: &ErItem, ctx: &Ctx) -> Option<Decision> {
    let shop = kh_host::shop::rules();
    if item.cat != Some(Cat::Goods) || !item.known || !shop.is_online_group(item.sort_group) {
        return None;
    }
    // (the user: "keep quest items only") an online item a quest needs (sheet `online.quest_keep`, from the game's scripts)
    // stays Elden Ring's: the gate passes it, the KH3 Items screen lists it with the key items
    if shop.is_quest_online(item.id) {
        return Some(Decision { give: Give::Pass, rule: "shop.online_quest".to_string(), tier: 0, why: String::new(), one: false });
    }
    let tier = shop.tier_of_depth(ctx.depth);
    let why = format!("an online item (sortGroupId {}): the area's shop tier {tier} (depth {})", item.sort_group, ctx.depth);
    Some(match shop.replacement(tier, item.id) {
        Some(o) => Decision { give: Give::Kh { item_id: o.item_id, key: o.key }, rule: "shop.online".to_string(), tier, why, one: true },
        None => Decision { give: Give::Nothing, rule: "shop.online".to_string(), tier, why: format!("{why}; the shop sheet has no stock: nothing"), one: true },
    })
}

#[cfg(test)]
mod shop_online_tests {
    use super::*;

    fn online(id: u32) -> ErItem {
        ErItem { cat: Some(Cat::Goods), id, goods_type: 0, sort_group: 120, consumable: false, known: true, ..ErItem::default() }
    }

    /// (SHOP) Every online item becomes ONE KH consumable of the area's shop stock, from every source (a pickup, the
    /// start's gift, a gather); an ordinary goods item is not touched by the rule.
    #[test]
    fn shop_online_items_become_kh_consumables_everywhere() {
        let rules = LootRules::load();
        let shop = kh_host::shop::rules();
        for (id, source, depth) in [(100u32, Source::Npc, 0u8), (101, Source::World, 0), (150, Source::Gather, 1), (109, Source::Unknown, 4), (182, Source::Enemy, 2)] {
            let d = rules.classify(&online(id), &Ctx::of(source, depth));
            let Give::Kh { key, .. } = &d.give else { panic!("{id} from {source:?}: {d:?}") };
            assert_eq!(d.rule, "shop.online");
            assert!(d.one, "one item for the stack");
            assert!(shop.stock_for(shop.tier_of_depth(depth)).iter().any(|o| &o.key == key), "{key} is sold in that area");
            assert!(kh_host::items::battle_kind(kh_host::items::by_key(key).unwrap().item_id).is_some(), "a usable item");
        }
        // the shop's stock: never an online item there either (only KH items)
        assert!(shop.stock_for(shop.top_tier()).iter().all(|o| o.key.starts_with("ITEM_")));
        // an ordinary reusable tool (sort group 10) still passes by its own rule
        // (the user: "keep quest items only") Varré's fingers stay Elden Ring's from every source
        // (SHOP-4) Varré's fingers stay Elden Ring's; the Wizened Finger is converted (its flag is set at the gate)
        let finger = rules.classify(&online(106), &Ctx::of(Source::World, 0));
        assert!(matches!(finger.give, Give::Kh { .. }) && finger.rule == "shop.online", "{finger:?}");
        // an item a script takes (the Stone-Sheathed Sword) stays Elden Ring's, whatever its upgrade
        for id in [2_540_000u32, 2_540_003] {
            let sword = ErItem { cat: Some(Cat::Weapon), id, known: true, rarity: 2, ..ErItem::default() };
            assert_eq!(rules.classify(&sword, &Ctx::of(Source::World, 3)).rule, "shop.script_keep");
        }
        for id in [102u32, 111] {
            let d = rules.classify(&online(id), &Ctx::of(Source::Npc, 1));
            assert_eq!((d.give, d.rule.as_str()), (Give::Pass, "shop.online_quest"), "{id}");
        }
        let tool = ErItem { sort_group: 10, ..online(8000) };
        assert_ne!(rules.classify(&tool, &Ctx::of(Source::World, 0)).rule, "shop.online");
    }
}
