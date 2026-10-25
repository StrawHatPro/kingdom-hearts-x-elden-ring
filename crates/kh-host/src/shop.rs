//! (SHOP) THE KH SHOP'S RULES, engine-free (the user, 2026-10-23: "shops need to be replaced with our own Kingdom Heart's
//! styled shops. Right now make them sell useable items that are a appropriate for the area the merchant is in, and
//! reduce the amount of money most enemies give you. Make the prices for items in shops quite high. There also has to be
//! a sell menu that lets you sell non-key items and non-weapons"). State note: build/sora_tmp/party/IMPL_SHOP_STATE.md.
//!
//! EVERY NUMBER IS A ROW of sheets/port/sora-er-shop.json (Spreadsheet Method; read from disk at run time by the Elden Ring
//! host, the compiled copy otherwise):
//! - THE AREA TIER: `shop.area_tier` turns the region depth of the merchant's place (the loot sheet's `region.*` rows:
//!   0 Limgrave .. 4 the DLC) into a shop tier 1 ..; nothing is written per merchant.
//! - THE STOCK: `stock.<KH item key>` = {from: the first tier that sells it, base: its price at tier 1}; a merchant of
//!   tier T sells every row with `from <= T`, at `base x shop.tier_markup[T]`, rounded to `shop.round`.
//! - THE SELL PRICE: a share (`sell.share`) of the item's buy price at that merchant (a stocked item's own price; any other
//!   item: KH3's own shop price x `price.kh_factor`, or KH3's sell price x 4 x that factor when KH3 sells it nowhere).
//!   Never sold: weapons (`sell.never_prefix`: Keyblades, Donald's staves, Goofy's shields), key items (not in the stock),
//!   classes outside `sell.classes`.
//! - THE KILL MUNNY: `munny.kill_scale` (by the prize tier of the kill, 0 .. 2 by maximum HP) for an enemy whose row names a
//!   rune value; `munny.boss_scale` for a kill whose row names none (a boss: the game awards its runes by its own event).
//! - ELDEN RING'S ONLINE ITEMS: `online.sort_groups` (EquipParamGoods.sortGroupId of the game's "Multiplayer Items" group);
//!   (ONLINEITEMS 2026-10-25) they stay Elden Ring's items (kh-er lootmap.rs `online_item`).
//! - THE SMITHS: `smith.talk_ids` (talk scripts that open the game's reinforcement menu; generated from the game's own talk
//!   scripts) and the talk-script numbers the host reads (`talk.commands`).
//!
//! THE TRANSACTIONS ([`buy`], [`sell`], [`max_buy`]) are plain arithmetic on munny and a stock count with its maximum: the
//! HUD carries them out on Sora's status and the camp menu's stock (hud_shop.rs), the save keeps both.

use std::sync::OnceLock;

/// The compiled copy of the sheet.
pub const SHEET: &str = include_str!("../../../sheets/port/sora-er-shop.json");

/// One stock row.
#[derive(Clone, Debug, PartialEq)]
pub struct StockRow {
    pub key: String,
    /// The first shop tier that sells it.
    pub from: u8,
    /// Its price at tier 1 (munny).
    pub base: u32,
}

/// What a merchant of a tier offers: the item and its price there.
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub key: String,
    pub item_id: u32,
    pub name: String,
    pub price: u32,
    pub max: u32,
}

/// The talk-script numbers the host reads (Elden Ring's talk commands and queries; `talk.commands`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TalkNumbers {
    /// OpenRegularShop(first row, last row).
    pub open_buy: i32,
    /// OpenSellShop(-1, -1).
    pub open_sell: i32,
    /// OpenEnhanceShop(kind): the reinforcement menu of a smith.
    pub open_enhance: i32,
    /// CheckSpecificPersonMenuIsOpen's menu type of the buy and of the sell window.
    pub menu_buy: i32,
    pub menu_sell: i32,
    /// CheckActionButtonArea(action button param): the talk prompt of a character.
    pub action_button: i32,
}

impl Default for TalkNumbers {
    fn default() -> TalkNumbers {
        TalkNumbers { open_buy: 22, open_sell: 46, open_enhance: 24, menu_buy: 5, menu_sell: 6, action_button: 56 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShopRules {
    pub area_tier: Vec<u8>,
    pub markup: Vec<f32>,
    pub round: u32,
    pub stock: Vec<StockRow>,
    pub sell_share: f32,
    pub kh_factor: f32,
    pub sell_min: u32,
    pub never_prefix: Vec<String>,
    pub sell_classes: Vec<String>,
    /// Munny scale of a priced kill by its prize tier (0 .. 2).
    pub kill_scale: [f32; 3],
    /// Munny scale of a kill whose row names no rune value (a boss).
    pub boss_scale: f32,
    /// (ONLINEITEMS) the "Multiplayer Items" sort groups: their goods stay Elden Ring's.
    pub online_groups: Vec<u8>,
    /// (SHOP-4, the user: zero progression blockers) row `required.keep`: items (event type, id: 0 weapon, 1 protector,
    /// 2 accessory, 3 goods, 4 gem) a script takes or counts: they stay Elden Ring's.
    pub required_keep: std::collections::HashSet<(u8, u32)>,
    /// row `required.satisfy`: items only whose "obtained" flag a script waits on: converted, the flag set.
    pub satisfy: std::collections::HashMap<(u8, u32), SatisfyRow>,
    pub smith_talks: Vec<i32>,
    pub talk: TalkNumbers,
    /// The most of one item a purchase may bring (the stock's own maximum caps it too).
    pub qty_max: u32,
    /// Rows that were not understood.
    pub problems: Vec<String>,
}

impl Default for ShopRules {
    fn default() -> ShopRules {
        ShopRules { area_tier: vec![1], markup: vec![1.0], round: 10, stock: Vec::new(), sell_share: 0.25, kh_factor: 10.0, sell_min: 1, never_prefix: vec!["WEP_".to_string()], sell_classes: Vec::new(), kill_scale: [1.0; 3], boss_scale: 1.0, online_groups: Vec::new(), required_keep: Default::default(), satisfy: Default::default(), smith_talks: Vec::new(), talk: TalkNumbers::default(), qty_max: 99, problems: Vec::new() }
    }
}

/// (SHOP-4) An item whose "obtained" flag a script waits on (row `required.satisfy`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SatisfyRow {
    /// Every flag that says it was obtained, with the map of that lot (`mAA_BB_CC_00`; none: an enemy's / a gift's).
    pub flags: Vec<(u32, Option<String>)>,
    /// The flags a script waits on.
    pub waited: Vec<u32>,
    /// Lots in the map of an event that waits on them (flag, map): the chapel's exit.
    pub local: Vec<(u32, String)>,
}

impl SatisfyRow {
    /// THE FLAG OF THIS PICKUP when the gate turns it into a KH item in `map_now`: the one lot of the item in that map,
    /// else its only flag. `None`: it cannot be told (several lots there, or several elsewhere): the item stays Elden Ring's.
    pub fn pickup_flags(&self, map_now: &str) -> Option<Vec<u32>> {
        let here: Vec<u32> = self.flags.iter().filter(|(_, m)| m.as_deref() == Some(map_now)).map(|(f, _)| *f).collect();
        match (here.len(), self.flags.len()) {
            (1, _) => Some(here),
            (0, 1) => Some(vec![self.flags[0].0]),
            _ => None,
        }
    }
}

/// The event / talk numbering of an item table (0 weapon, 1 protector, 2 accessory, 3 goods, 4 gem).
pub fn item_key(text: &str) -> Option<(u8, u32)> {
    let (t, id) = text.split_once(':')?;
    Some((t.parse().ok()?, id.parse().ok()?))
}

/// Why a trade cannot be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Buying: the munny does not reach.
    Munny,
    /// Buying: the stock already holds its maximum.
    Full,
    /// Selling: none (or fewer) held.
    NoneHeld,
    /// A quantity of 0.
    Nothing,
}

impl Refusal {
    /// The words the shop shows.
    pub fn text(self) -> &'static str {
        match self {
            Refusal::Munny => kh_gendata::line("kh3_menu.shop.dialog_0020"),
            Refusal::Full => kh_gendata::line("kh3_menu.shop.dialog_0010"),
            Refusal::NoneHeld => "You don't have any to sell.",
            Refusal::Nothing => "Nothing to trade.",
        }
    }
}

/// The most of an item that can be bought now: what the munny reaches, what the stock has room for, `qty_max`.
pub fn max_buy(munny: i64, price: u32, owned: u32, max: u32, qty_max: u32) -> u32 {
    let room = max.saturating_sub(owned);
    let afford = if price == 0 { u32::MAX } else { (munny.max(0) / price as i64).min(u32::MAX as i64) as u32 };
    room.min(afford).min(qty_max)
}

/// A PURCHASE of `qty` at `price` each: (munny after, owned after).
pub fn buy(munny: i64, owned: u32, max: u32, price: u32, qty: u32) -> Result<(i64, u32), Refusal> {
    if qty == 0 {
        return Err(Refusal::Nothing);
    }
    if owned.saturating_add(qty) > max {
        return Err(Refusal::Full);
    }
    let total = price as i64 * qty as i64;
    if total > munny {
        return Err(Refusal::Munny);
    }
    Ok((munny - total, owned + qty))
}

/// A SALE of `qty` at `price` each: (munny after, owned after).
pub fn sell(munny: i64, owned: u32, price: u32, qty: u32) -> Result<(i64, u32), Refusal> {
    if qty == 0 {
        return Err(Refusal::Nothing);
    }
    if qty > owned {
        return Err(Refusal::NoneHeld);
    }
    Ok((munny.saturating_add(price as i64 * qty as i64), owned - qty))
}

impl ShopRules {
    pub fn parse(text: &str) -> ShopRules {
        let mut out = ShopRules::default();
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
            out.problems.push("the sheet is not JSON".to_string());
            return out;
        };
        let floats = |v: &serde_json::Value| v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64().map(|n| n as f32)).collect::<Vec<_>>()).unwrap_or_default();
        let strings = |v: &serde_json::Value| v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>()).unwrap_or_default();
        for row in json["rows"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            let (Some(id), value) = (row[0].as_str(), &row[1]) else { continue };
            let number = value.as_f64();
            match id {
                "shop.area_tier" => out.area_tier = value.as_array().map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| (n as u8).max(1))).collect()).unwrap_or_default(),
                "shop.tier_markup" => out.markup = floats(value),
                "shop.round" => out.round = number.map_or(out.round, |v| (v as u32).max(1)),
                "shop.qty_max" => out.qty_max = number.map_or(out.qty_max, |v| (v as u32).max(1)),
                "sell.share" => out.sell_share = number.map_or(out.sell_share, |v| v as f32),
                "price.kh_factor" => out.kh_factor = number.map_or(out.kh_factor, |v| v as f32),
                "sell.min" => out.sell_min = number.map_or(out.sell_min, |v| v as u32),
                "sell.never_prefix" => out.never_prefix = strings(value),
                "sell.classes" => out.sell_classes = strings(value),
                "munny.kill_scale" => {
                    let v = floats(value);
                    if v.len() == 3 {
                        out.kill_scale = [v[0], v[1], v[2]];
                    } else {
                        out.problems.push(format!("{id}: needs three numbers (prize tier 0, 1, 2)"));
                    }
                }
                "munny.boss_scale" => out.boss_scale = number.map_or(out.boss_scale, |v| v as f32),
                "online.sort_groups" => out.online_groups = value.as_array().map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u8)).collect()).unwrap_or_default(),
                "required.keep" => out.required_keep = value.as_array().map(|a| a.iter().filter_map(|x| x.as_str().and_then(item_key)).collect()).unwrap_or_default(),
                "required.satisfy" => {
                    for (k, v) in value.as_object().into_iter().flatten() {
                        let Some(key) = item_key(k) else { continue };
                        let flags = v["flags"].as_array().map(|a| a.iter().filter_map(|p| Some((p[0].as_u64()? as u32, p[1].as_str().map(str::to_string)))).collect()).unwrap_or_default();
                        let waited = v["waited"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u32)).collect()).unwrap_or_default();
                        let local = v["local"].as_array().map(|a| a.iter().filter_map(|p| Some((p[0].as_u64()? as u32, p[1].as_str()?.to_string()))).collect()).unwrap_or_default();
                        out.satisfy.insert(key, SatisfyRow { flags, waited, local });
                    }
                }
                "smith.talk_ids" => out.smith_talks = value.as_array().map(|a| a.iter().filter_map(|x| x.as_i64().map(|n| n as i32)).collect()).unwrap_or_default(),
                "talk.commands" => {
                    let n = |k: &str, d: i32| value[k].as_i64().map_or(d, |v| v as i32);
                    let d = TalkNumbers::default();
                    out.talk = TalkNumbers { open_buy: n("open_buy", d.open_buy), open_sell: n("open_sell", d.open_sell), open_enhance: n("open_enhance", d.open_enhance), menu_buy: n("menu_buy", d.menu_buy), menu_sell: n("menu_sell", d.menu_sell), action_button: n("action_button", d.action_button) };
                }
                _ if id.starts_with("stock.") => {
                    let key = id["stock.".len()..].to_string();
                    match (value["from"].as_u64(), value["base"].as_u64()) {
                        (Some(from), Some(base)) if crate::items::by_key(&key).is_some() => out.stock.push(StockRow { key, from: (from as u8).max(1), base: base as u32 }),
                        (Some(_), Some(_)) => out.problems.push(format!("{id}: no KH3 item {key}")),
                        _ => out.problems.push(format!("{id}: needs {{\"from\": tier, \"base\": munny}}")),
                    }
                }
                _ => {}
            }
        }
        if out.area_tier.is_empty() {
            out.problems.push("shop.area_tier is empty".to_string());
            out.area_tier = vec![1];
        }
        if out.markup.is_empty() {
            out.markup = vec![1.0];
        }
        out
    }

    /// The sheet: `path` when it reads (the user edits it), else the compiled copy.
    pub fn load(path: Option<&std::path::Path>) -> ShopRules {
        if let Some(text) = path.and_then(|p| std::fs::read_to_string(p).ok()) {
            let rules = ShopRules::parse(&text);
            if !rules.stock.is_empty() {
                return rules;
            }
        }
        ShopRules::parse(SHEET)
    }

    /// The shop tier of a region depth (0 Limgrave .. 4 the DLC; deeper than the row: its last tier).
    pub fn tier_of_depth(&self, depth: u8) -> u8 {
        self.area_tier.get(depth as usize).or(self.area_tier.last()).copied().unwrap_or(1).max(1)
    }

    /// The highest tier the sheet knows.
    pub fn top_tier(&self) -> u8 {
        self.area_tier.iter().copied().max().unwrap_or(1)
    }

    pub fn markup(&self, tier: u8) -> f32 {
        self.markup.get(tier.max(1) as usize - 1).or(self.markup.last()).copied().unwrap_or(1.0)
    }

    fn rounded(&self, value: f32) -> u32 {
        let r = self.round.max(1) as f32;
        ((value / r).round() * r).max(0.0) as u32
    }

    /// WHAT A MERCHANT OF THIS TIER SELLS, in the sheet's order.
    pub fn stock_for(&self, tier: u8) -> Vec<Offer> {
        self.stock
            .iter()
            .filter(|s| s.from <= tier)
            .filter_map(|s| {
                let item = crate::items::by_key(&s.key)?;
                Some(Offer { key: s.key.clone(), item_id: item.item_id, name: item.name.clone(), price: self.rounded(s.base as f32 * self.markup(tier)).max(1), max: item.max })
            })
            .collect()
    }

    /// The item's buy value at tier 1: a stocked item's base, else KH3's own prices x `price.kh_factor`.
    pub fn buy_value(&self, key: &str) -> u32 {
        if let Some(s) = self.stock.iter().find(|s| s.key == key) {
            return s.base;
        }
        match crate::items::by_key(key) {
            Some(item) if item.buy > 0 => (item.buy as f32 * self.kh_factor).round() as u32,
            Some(item) => (item.sell as f32 * 4.0 * self.kh_factor).round() as u32,
            None => 0,
        }
    }

    /// May this item be sold at all (not a weapon, a class the sheet allows, a row of the item table)?
    pub fn sellable(&self, key: &str) -> bool {
        if self.never_prefix.iter().any(|p| key.starts_with(p.as_str())) {
            return false;
        }
        let Some(item) = crate::items::by_key(key) else { return false };
        let class = match item.class {
            crate::items::Class::Battle => "battle",
            crate::items::Class::Camp => "camp",
            crate::items::Class::Armour => "armour",
            crate::items::Class::Accessory => "accessory",
            crate::items::Class::Material => "material",
        };
        self.sell_classes.is_empty() || self.sell_classes.iter().any(|c| c == class)
    }

    /// The price one of these brings at a merchant of this tier (`None`: never sold).
    pub fn sell_price(&self, key: &str, tier: u8) -> Option<u32> {
        if !self.sellable(key) {
            return None;
        }
        let value = self.buy_value(key) as f32 * self.markup(tier) * self.sell_share;
        Some(self.rounded(value).max(self.sell_min))
    }

    /// THE MUNNY SCALE of a kill: `priced` = its row names a rune value; `tier` = its prize tier (0 .. 2 by maximum HP).
    pub fn kill_scale(&self, tier: usize, priced: bool) -> f32 {
        if priced {
            self.kill_scale[tier.min(2)]
        } else {
            self.boss_scale
        }
    }

    /// An Elden Ring goods row of the "Multiplayer Items" group (its sortGroupId).
    pub fn is_online_group(&self, sort_group: u8) -> bool {
        self.online_groups.contains(&sort_group)
    }

    /// A talk script that opens the game's reinforcement menu (a smith).
    pub fn is_smith_talk(&self, talk_id: i32) -> bool {
        self.smith_talks.contains(&talk_id)
    }

}

static RULES: OnceLock<ShopRules> = OnceLock::new();

/// The sheet's place in the tree the binary was built from (the Elden Ring mod reads its sheets there at run time, as
/// lootmap.rs does with sora-er-items).
pub const SHEET_PATH: &str = "sheets/port/sora-er-shop.json"; // (release) under the repository: kh_gendata::project_file

/// The rules of this process: the sheet on disk ([`SHEET_PATH`]) when it reads, else the compiled copy (or what the host
/// set first with [`set_rules`]).
pub fn rules() -> &'static ShopRules {
    RULES.get_or_init(|| ShopRules::load(Some(std::path::Path::new(kh_gendata::project_file(SHEET_PATH)))))
}

/// The host's own reading of the sheet (from disk); only the first call counts.
pub fn set_rules(rules: ShopRules) -> bool {
    RULES.set(rules).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> ShopRules {
        let r = ShopRules::parse(SHEET);
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        r
    }

    /// The sheet as shipped reads without a problem; every stock row is a KH3 battle or camp item a Sora can use.
    #[test]
    fn shop_the_sheet_reads_and_sells_only_usable_items() {
        let r = sheet();
        assert!(r.stock.len() >= 10);
        for s in &r.stock {
            let item = crate::items::by_key(&s.key).unwrap();
            assert!(crate::items::battle_kind(item.item_id).is_some(), "{} is not an item a Sora uses", s.key);
            assert!(s.base > 0 && (1..=r.top_tier()).contains(&s.from), "{s:?}");
        }
        assert_eq!(r.talk, TalkNumbers::default(), "the talk numbers read in the game's scripts");
        assert_eq!(r.online_groups, vec![120]);
        assert_eq!(r.smith_talks.len(), 4);
    }

    /// AREA TIERS: Limgrave sells the basics only, every deeper area what the one before sells and more; the DLC
    /// sells the Megalixir; prices rise with the tier and stay "quite high" (a Potion costs ten times KH3's).
    #[test]
    fn shop_the_stock_follows_the_area_tier() {
        let r = sheet();
        let keys = |t: u8| r.stock_for(t).iter().map(|o| o.key.clone()).collect::<Vec<_>>();
        assert_eq!(r.tier_of_depth(0), 1);
        assert_eq!(r.tier_of_depth(4), r.top_tier());
        assert_eq!(r.tier_of_depth(9), r.top_tier(), "deeper than the row: the last tier");
        let t1 = keys(1);
        assert!(t1.contains(&"ITEM_POTION".to_string()) && t1.contains(&"ITEM_ETHER".to_string()) && t1.contains(&"ITEM_ALLCURE".to_string()));
        assert!(!t1.contains(&"ITEM_ELIXIR".to_string()) && !t1.contains(&"ITEM_LASTELIXIR".to_string()), "no Elixir in Limgrave");
        for t in 2..=r.top_tier() {
            let (before, now) = (keys(t - 1), keys(t));
            assert!(before.iter().all(|k| now.contains(k)) && now.len() > before.len(), "tier {t} keeps tier {}'s stock and adds to it", t - 1);
        }
        assert!(keys(r.top_tier()).contains(&"ITEM_LASTELIXIR".to_string()));
        let price = |t: u8, key: &str| r.stock_for(t).into_iter().find(|o| o.key == key).map(|o| o.price);
        let potion: Vec<u32> = (1..=r.top_tier()).map(|t| price(t, "ITEM_POTION").unwrap()).collect();
        assert!(potion.windows(2).all(|w| w[1] > w[0]), "{potion:?}");
        assert!(potion[0] >= 10 * crate::items::by_key("ITEM_POTION").unwrap().buy, "quite high: {}", potion[0]);
        assert!(price(4, "ITEM_ELIXIR").unwrap() > price(4, "ITEM_HIGHPOTION").unwrap() * 5);
        assert!(r.stock_for(1).iter().all(|o| o.price % r.round == 0));
    }

    /// THE SELL RULE: weapons (Keyblades, staves, shields) are never sold; an item sells for a quarter of what this
    /// merchant asks for it; an item the merchant does not stock by KH3's own price.
    #[test]
    fn shop_sell_prices_and_the_never_sold() {
        let r = sheet();
        for key in ["WEP_KEYBLADE_SO_00", "WEP_DONALD_03", "WEP_GOOFY_10", "KEY_ITEM02", "HOSTKEY_0"] {
            assert!(!r.sellable(key) && r.sell_price(key, 1).is_none(), "{key} must never be sold");
        }
        for key in ["ITEM_POTION", "ITEM_TENT", "PRT_ITEM01", "ACC_ITEM01", "MAT_ITEM01"] {
            assert!(r.sellable(key), "{key}");
        }
        let buy = r.stock_for(2).into_iter().find(|o| o.key == "ITEM_POTION").unwrap().price;
        let sold = r.sell_price("ITEM_POTION", 2).unwrap();
        assert!(sold < buy && (sold as f32 - buy as f32 * r.sell_share).abs() <= r.round as f32, "{sold} of {buy}");
        let armour = r.sell_price("PRT_ITEM01", 1).unwrap();
        assert!(armour >= r.sell_min);
        assert_eq!(r.buy_value("PRT_ITEM01"), (crate::items::by_key("PRT_ITEM01").unwrap().buy as f32 * r.kh_factor).round() as u32);
    }

    /// THE TRANSACTIONS: munny and the stock's maximum are respected both ways.
    #[test]
    fn shop_transactions() {
        assert_eq!(buy(1000, 2, 99, 300, 3), Ok((100, 5)));
        assert_eq!(buy(899, 2, 99, 300, 3), Err(Refusal::Munny));
        assert_eq!(buy(10_000, 98, 99, 300, 2), Err(Refusal::Full));
        assert_eq!(buy(10_000, 0, 99, 300, 0), Err(Refusal::Nothing));
        assert_eq!(sell(100, 5, 75, 5), Ok((475, 0)));
        assert_eq!(sell(100, 5, 75, 6), Err(Refusal::NoneHeld));
        assert_eq!(max_buy(1000, 300, 2, 99, 99), 3);
        assert_eq!(max_buy(1_000_000, 300, 95, 99, 99), 4);
        assert_eq!(max_buy(1_000_000, 300, 0, 99, 10), 10);
        assert_eq!(max_buy(0, 300, 0, 99, 99), 0);
        assert_eq!(max_buy(-5, 300, 0, 99, 99), 0);
    }

    /// LESS MUNNY FROM MOST ENEMIES: an ordinary enemy's award is cut; a boss keeps its whole award.
    #[test]
    fn shop_kill_munny_is_cut_for_ordinary_enemies() {
        let r = sheet();
        assert!(r.kill_scale(0, true) < 0.5 && r.kill_scale(1, true) < 1.0);
        assert!(r.kill_scale(0, true) <= r.kill_scale(1, true) && r.kill_scale(1, true) <= r.kill_scale(2, true));
        assert_eq!(r.kill_scale(0, false), r.boss_scale);
        assert!(r.boss_scale >= 1.0, "bosses keep their reward");
    }

    /// (ONLINEITEMS) The online group is read (its items stay Elden Ring's: kh-er lootmap); the script-required rows of SHOP-4
    /// stand for the items the gate still converts.
    #[test]
    fn shop_online_group_and_the_script_required_rows() {
        let r = sheet();
        assert!(r.is_online_group(120) && !r.is_online_group(10));
        // (SHOP-4) the satisfy rule of the gate (applies to converted items only; an online item is never converted)
        let chapel = r.satisfy.get(&(3, 106)).expect("the scan's row of the chapel's finger");
        assert_eq!(chapel.pickup_flags("m10_01_00_00"), Some(vec![60210]));
        // an item a script takes or counts stays Elden Ring's; an ambiguous pickup cannot be satisfied
        assert!(r.required_keep.contains(&(0, 2_540_000)), "the Stone-Sheathed Sword (an event takes it)");
        assert!(r.required_keep.len() > 100 && r.satisfy.len() > 100);
        let two = SatisfyRow { flags: vec![(1, Some("m10_00_00_00".into())), (2, Some("m10_00_00_00".into()))], ..SatisfyRow::default() };
        assert_eq!(two.pickup_flags("m10_00_00_00"), None, "two lots of it here: cannot be told");
        assert_eq!(SatisfyRow { flags: vec![(5, None)], ..SatisfyRow::default() }.pickup_flags("m60_42_36_00"), Some(vec![5]), "its only flag");
        // the retired rows of the online conversion are gone from the sheet
        for gone in ["online.quest_keep", "online.quest_keep2", "online.wait_flags"] {
            assert!(!SHEET.contains(&format!("\"{gone}\"")), "{gone}");
        }
        assert!(r.problems.is_empty(), "{:?}", r.problems);
    }
}
