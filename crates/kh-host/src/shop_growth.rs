//! (SHOP-TIERS) THE KH SHOP'S STOCK GROWS WITH THE MERCHANT'S OWN ELDEN RING LINEUP, engine-free (THE USER, 2026-11-07: "for
//! merchants with shops that get upgraded in the base game, allow their shows to upgrade their stock selection as well when
//! their should would upgrade in elden ring. This means start them off with basic, limited selection"). State note:
//! build/sora_tmp/shop_tiers/STATE.md.
//!
//! - THE GROWTH OF A LINEUP: the rows of the range the merchant's talk script opened (OpenRegularShop(first, last)), each with
//!   its `eventFlag_forRelease` (0 = sold from the start). Its SHARE = rows released (no flag, or the flag on) / rows. A
//!   lineup without a release flag never grows: no share, the KH shop keeps its whole stock ([`lineup_share`]). The host reads
//!   the game's own rows and flags live (kh-er shopkh.rs); the sheet's `lineup.<first>-<last>` rows (generated from the same
//!   param by build/sora_tmp/shop_tiers/tools/make_shop_tiers.js) are the fallback and the record.
//! - THE KH STOCK BY THAT SHARE ([`GrowthRules::grown_stock`]): the offers of a class in `growth.always_classes` (the synthesis
//!   materials) and the host's own lines (`Growth::keep`: the merchant's own glovewort / Smithing Stone lines, which follow
//!   Elden Ring's flags line by line) always stay; the others (battle items, the Tent, armour, accessories, Donald's and
//!   Goofy's weapons) are ordered by their tier-1 buy value (`growth.order`: cheaper / weaker first) and the first
//!   ceil(share x n) of them are sold, never fewer than the start's count (at least `growth.min_offers`) plus one per release
//!   flag on, so every growth step of Elden Ring's lineup adds at least one offer ([`shown`]); the list keeps its own order. Nothing is shown
//!   for what is not sold yet (no "locked" rows: KH3's shop has none), nothing is taken from the player.
//!
//! Every number is a row of sheets/port/sora-er-shop-tiers.json (read from disk at run time by the Elden Ring host, the
//! compiled copy otherwise).

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::shop::{Offer, ShopRules};

/// The compiled copy of the sheet.
pub const SHEET: &str = include_str!("../../../sheets/port/sora-er-shop-tiers.json");

/// The sheet's place in the tree the binary was built from (read at run time, as `crate::shop::SHEET_PATH`).
pub const SHEET_PATH: &str = "sheets/port/sora-er-shop-tiers.json"; // (release) under the repository: kh_gendata::project_file

/// How far the merchant's own lineup has grown, for one opening of the shop.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Growth {
    /// Rows released / rows of the lineup the talk script opened (0 ..= 1).
    pub share: f32,
    /// The share at the start: rows without a release flag / rows.
    pub start: f32,
    /// Release flags on (the tiers reached beyond the start).
    pub reached: usize,
    /// Release flags of the lineup (its tiers beyond the start).
    pub tiers: usize,
    /// Keys never cut: the host's own lines (they follow Elden Ring's own release flags already).
    pub keep: Vec<String>,
}

/// One lineup of the sheet (`lineup.<first>-<last>`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lineup {
    pub who: Vec<String>,
    /// The talk scripts that open it.
    pub talks: Vec<i32>,
    pub rows: u32,
    /// Rows without a release flag.
    pub base: u32,
    /// (release flag, rows it releases).
    pub flags: Vec<(u32, u32)>,
}

impl Lineup {
    /// Every row's release flag (0 = none), as the param lists them (the order does not matter).
    pub fn release_flags(&self) -> Vec<u32> {
        let mut out = vec![0; self.base as usize];
        for (flag, n) in &self.flags {
            out.extend(std::iter::repeat_n(*flag, *n as usize));
        }
        out
    }

    /// Its tiers: 1 (the start) + one per release flag.
    pub fn tiers(&self) -> usize {
        1 + self.flags.len()
    }
}

/// The sheet's rules.
#[derive(Clone, Debug, PartialEq)]
pub struct GrowthRules {
    pub min_offers: usize,
    pub always_classes: Vec<String>,
    /// The lineups by (first row, last row).
    pub lineups: HashMap<(i32, i32), Lineup>,
    pub problems: Vec<String>,
}

impl Default for GrowthRules {
    fn default() -> GrowthRules {
        GrowthRules { min_offers: 3, always_classes: vec!["material".to_string()], lineups: HashMap::new(), problems: Vec::new() }
    }
}

/// THE GROWTH OF A LINEUP: `release_flags` = every row's release flag (0 = none); `released(flag)` = the flag is on. `None`:
/// the lineup never grows (no row, or no row with a release flag): the KH shop keeps its whole stock. (`keep` is the host's.)
pub fn lineup_share(release_flags: &[u32], released: &dyn Fn(u32) -> bool) -> Option<Growth> {
    if !release_flags.iter().any(|f| *f != 0) {
        return None;
    }
    let n = release_flags.len() as f32;
    let on = release_flags.iter().filter(|f| **f == 0 || released(**f)).count();
    let base = release_flags.iter().filter(|f| **f == 0).count();
    let mut flags: Vec<u32> = release_flags.iter().copied().filter(|f| *f != 0).collect();
    flags.sort_unstable();
    flags.dedup();
    let reached = flags.iter().filter(|f| released(**f)).count();
    Some(Growth { share: on as f32 / n, start: base as f32 / n, reached, tiers: flags.len(), keep: Vec::new() })
}

/// How many of `n` tiered offers a growth sells: ceil(share x n); never fewer than the start (ceil(start x n), at least
/// `min`, all when fewer) plus one per tier reached, so EVERY growth step of Elden Ring's lineup adds at least one offer
/// while any is left; all at share 1.
pub fn shown(g: &Growth, n: usize, min: usize) -> usize {
    if g.share >= 1.0 {
        return n;
    }
    let at = |s: f32| (s.max(0.0) * n as f32 - 1e-4).ceil().max(0.0) as usize;
    let start = at(g.start).max(min.min(n));
    at(g.share).max(start + g.reached).min(n)
}

impl GrowthRules {
    pub fn parse(text: &str) -> GrowthRules {
        let mut out = GrowthRules::default();
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
            out.problems.push("the sheet is not JSON".to_string());
            return out;
        };
        for row in json["rows"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            let (Some(id), value) = (row[0].as_str(), &row[1]) else { continue };
            match id {
                "growth.min_offers" => out.min_offers = value.as_u64().map_or(out.min_offers, |v| v as usize),
                "growth.always_classes" => out.always_classes = value.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default(),
                _ if id.starts_with("lineup.") => {
                    let range = id["lineup.".len()..].split_once('-').and_then(|(a, b)| Some((a.parse::<i32>().ok()?, b.parse::<i32>().ok()?)));
                    let (Some(range), Some(rows), Some(base)) = (range, value["rows"].as_u64(), value["base"].as_u64()) else {
                        out.problems.push(format!("{id}: needs lineup.<first>-<last> = {{rows, base, flags}}"));
                        continue;
                    };
                    let flags: Vec<(u32, u32)> = value["flags"].as_array().map(Vec::as_slice).unwrap_or(&[]).iter().filter_map(|p| Some((p[0].as_u64()? as u32, p[1].as_u64()? as u32))).collect();
                    if base + flags.iter().map(|f| f.1 as u64).sum::<u64>() != rows {
                        out.problems.push(format!("{id}: base + the flags' rows != rows"));
                    }
                    let who = value["who"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
                    let talks = value["talks"].as_array().map(|a| a.iter().filter_map(|x| x.as_i64().and_then(|t| i32::try_from(t).ok())).collect()).unwrap_or_default();
                    out.lineups.insert(range, Lineup { who, talks, rows: rows as u32, base: base as u32, flags });
                }
                _ => {}
            }
        }
        out
    }

    /// The sheet: `path` when it reads, else the compiled copy.
    pub fn load(path: Option<&std::path::Path>) -> GrowthRules {
        if let Some(rules) = path.and_then(|p| std::fs::read_to_string(p).ok()).map(|t| GrowthRules::parse(&t)).filter(|r| !r.lineups.is_empty()) {
            return rules;
        }
        GrowthRules::parse(SHEET)
    }

    /// THE GROWTH OF THE SHEET'S LINEUP `range` (the fallback when the game's own rows cannot be read).
    pub fn sheet_share(&self, range: (i32, i32), released: &dyn Fn(u32) -> bool) -> Option<Growth> {
        lineup_share(&self.lineups.get(&range)?.release_flags(), released)
    }

    /// THE MERCHANT'S OWN LINEUP OF A TALK SCRIPT when the script named no rows (its sell window, OpenSellShop(-1, -1): the
    /// player can still switch to BUY): the largest lineup of the sheet that script opens (the Twin Maiden Husks: their own
    /// 101800-101897, not a bell bearing's copy of another merchant's).
    pub fn lineup_of_talk(&self, talk_id: i32) -> Option<(i32, i32)> {
        self.lineups.iter().filter(|(_, l)| l.talks.contains(&talk_id)).max_by_key(|(range, l)| (l.rows, -range.0)).map(|(range, _)| *range)
    }

    /// Is this offer never cut (an always class, a key of the host's own lines)?
    fn always(&self, offer: &Offer, growth: &Growth) -> bool {
        if growth.keep.iter().any(|k| k == &offer.key) {
            return true;
        }
        let class = crate::items::by_key(&offer.key).filter(|_| crate::weapons::by_key(&offer.key).is_none()).map(|i| match i.class {
            crate::items::Class::Battle => "battle",
            crate::items::Class::Camp => "camp",
            crate::items::Class::Armour => "armour",
            crate::items::Class::Accessory => "accessory",
            crate::items::Class::Material => "material",
        });
        class.is_some_and(|c| self.always_classes.iter().any(|a| a == c))
    }

    /// WHAT THE MERCHANT SELLS OF `offers` AT THIS GROWTH (module text), in the offers' own order.
    pub fn grown_stock(&self, offers: Vec<Offer>, growth: &Growth, shop: &ShopRules) -> Vec<Offer> {
        let mut tiered: Vec<(u32, usize)> = offers.iter().enumerate().filter(|(_, o)| !self.always(o, growth)).map(|(k, o)| (strength(shop, o), k)).collect();
        tiered.sort();
        let n = shown(growth, tiered.len(), self.min_offers);
        let cut: std::collections::HashSet<usize> = tiered[n..].iter().map(|t| t.1).collect();
        offers.into_iter().enumerate().filter(|(k, _)| !cut.contains(k)).map(|(_, o)| o).collect()
    }
}

/// `growth.order`: the tier-1 buy value (a stocked item's base, else KH3's prices x `price.kh_factor`); a weapon: its price.
fn strength(shop: &ShopRules, offer: &Offer) -> u32 {
    match shop.buy_value(&offer.key) {
        0 => offer.price,
        v => v,
    }
}

static RULES: OnceLock<GrowthRules> = OnceLock::new();

/// The rules of this process: the sheet on disk ([`SHEET_PATH`]) when it reads, else the compiled copy.
pub fn rules() -> &'static GrowthRules {
    RULES.get_or_init(|| GrowthRules::load(Some(std::path::Path::new(kh_gendata::project_file(SHEET_PATH)))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> GrowthRules {
        let r = GrowthRules::parse(SHEET);
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        r
    }

    /// THE GENERATED SHEET READS: its rules, and the lineups Elden Ring grows (the Twin Maiden Husks' own range by the bell
    /// bearings, Corhyn's by the prayerbooks, Miriel's, Sellen's, Patches's quest, the DLC's Moore / Thiollier / Ymir) next
    /// to the ones it never grows (Kalé, the Nomadic Merchants without a flag, D).
    #[test]
    fn shop_tiers_the_sheet_reads_the_growing_and_the_fixed_lineups() {
        let r = sheet();
        assert_eq!((r.min_offers, r.always_classes.as_slice()), (3, ["material".to_string()].as_slice()));
        let husks = &r.lineups[&(101_800, 101_897)];
        assert_eq!((husks.rows, husks.base, husks.tiers()), (94, 21, 39), "the Husks: 21 rows from the start, 38 release flags");
        assert!(husks.flags.iter().any(|f| f.0 == 11_109_752), "a Smithing-Stone Miner's Bell Bearing tier");
        for (range, who) in [((100_350, 100_399), "Corhyn"), ((100_425, 100_474), "Miriel"), ((100_050, 100_074), "Sellen"), ((100_100, 100_124), "Patches"), ((102_250, 102_289), "Moore"), ((102_270, 102_289), "Thiollier"), ((102_300, 102_310), "Ymir")] {
            let l = &r.lineups[&range];
            assert!(l.tiers() > 1 && l.who.iter().any(|w| w.contains(who)), "{who} grows: {l:?}");
        }
        for (range, who) in [((100_500, 100_524), "Kalé"), ((100_525, 100_549), "Nomadic Merchant"), ((100_125, 100_149), "D, Hunter of the Dead")] {
            let l = &r.lineups[&range];
            assert!(l.flags.is_empty() && l.who.iter().any(|w| w.contains(who)), "{who} never grows: {l:?}");
            assert_eq!(r.sheet_share(range, &|_| false), None, "{who}: no share, the whole stock");
        }
        assert_eq!([r.lineup_of_talk(600_001_110), r.lineup_of_talk(330_006_000), r.lineup_of_talk(800_006_000), r.lineup_of_talk(1)], [Some((101_800, 101_897)), Some((100_425, 100_474)), Some((100_500, 100_524)), None], "a sell window's merchant: the Husks' own range, Miriel's larger one, Kalé's");
    }

    fn g(share: f32, start: f32, reached: usize) -> Growth {
        Growth { share, start, reached, tiers: 0, keep: Vec::new() }
    }

    /// THE SHARE: base rows / rows at the start, each flag adds its rows, every flag on = 1; a lineup without a flag never
    /// grows (no share). THE COUNT: the share's, never below the start + one per tier reached (every growth step shows).
    #[test]
    fn shop_tiers_the_share_follows_the_release_flags() {
        let share = |flags: &[u32], on: &dyn Fn(u32) -> bool| lineup_share(flags, on).map(|g| (g.share, g.start, g.reached, g.tiers));
        assert_eq!(share(&[0, 0, 7, 7, 9], &|_| false), Some((0.4, 0.4, 0, 2)));
        assert_eq!(share(&[0, 0, 7, 7, 9], &|f| f == 7), Some((0.8, 0.4, 1, 2)));
        assert_eq!(share(&[0, 0, 7, 7, 9], &|_| true), Some((1.0, 0.4, 2, 2)));
        assert_eq!(share(&[0, 0, 0], &|_| false), None);
        assert_eq!(share(&[], &|_| true), None);
        assert_eq!((shown(&g(0.0, 0.0, 0), 10, 3), shown(&g(0.22, 0.22, 0), 10, 3), shown(&g(0.5, 0.22, 1), 10, 3), shown(&g(0.99, 0.22, 9), 10, 3), shown(&g(1.0, 0.22, 9), 10, 3), shown(&g(0.1, 0.1, 0), 2, 3)), (3, 3, 5, 10, 10, 2));
        assert_eq!((shown(&g(0.23, 0.22, 1), 10, 3), shown(&g(0.24, 0.22, 2), 10, 3), shown(&g(0.24, 0.22, 20), 10, 3)), (4, 5, 10), "a one-row step still adds one; never more than all");
    }

    fn offer(key: &str, price: u32) -> Offer {
        let (item_id, name, max) = match (crate::items::by_key(key), crate::weapons::by_key(key)) {
            (Some(i), _) => (i.item_id, i.name.clone(), i.max),
            (None, Some(w)) => (w.item_id, w.name.clone(), w.max),
            (None, None) => panic!("{key}"),
        };
        Offer { key: key.into(), item_id, name, price, max }
    }

    /// THE KH STOCK BY THE SHARE: the cheapest first, at least growth.min_offers, materials and the host's own lines always,
    /// the list's own order kept, the whole list at share 1.
    #[test]
    fn shop_tiers_the_stock_is_cut_cheapest_first_and_keeps_the_always_rows() {
        let r = sheet();
        let shop = crate::shop::ShopRules::parse(crate::shop::SHEET);
        let mut offers = shop.stock_for(5);
        offers.push(offer("WEP_DONALD_02", 4000));
        offers.push(offer("MAT_ITEM53", 5000));
        offers.push(offer("MAT_GLOVE_GRAVE_01", 800));
        let keep = vec!["MAT_GLOVE_GRAVE_01".to_string()];
        let keys = |share: f32| r.grown_stock(offers.clone(), &Growth { share, keep: keep.clone(), ..Growth::default() }, &shop).into_iter().map(|o| o.key).collect::<Vec<_>>();
        let all: Vec<String> = offers.iter().map(|o| o.key.clone()).collect();
        assert_eq!(keys(1.0), all, "share 1: everything");
        let start = keys(0.0);
        assert_eq!(start, ["ITEM_POTION", "ITEM_ETHER", "ITEM_ALLCURE", "MAT_ITEM53", "MAT_GLOVE_GRAVE_01"], "the three cheapest (Panacea 250, Potion 400, Ether 800) in the list's order, the material and the host's line");
        let half = keys(0.5);
        assert_eq!(half.len(), 2 + 7, "13 tiered offers x 0.5 = 7, plus the two kept");
        assert!(half.iter().all(|k| all.contains(k)) && !half.contains(&"ITEM_LASTELIXIR".to_string()) && !half.contains(&"WEP_DONALD_02".to_string()), "Megalixir and the staff wait: {half:?}");
        let more = keys(0.75);
        assert!(half.iter().all(|k| more.contains(k)), "growth only adds");
    }
}
