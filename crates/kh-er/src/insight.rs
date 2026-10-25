//! (DON-PROG) ELEMENTAL INSIGHT IN ELDEN RING (documentation/DONALD_OVERHAUL.md 9: "Lock-on shows the target's weak and
//! resisted elements"; the coordinator 2026-10-22): an Elden Ring enemy's own numbers, its NpcParam row, read as Donald's
//! elements for `Targets::element_affinity` (Insight's marks, AI Donald's attunement by weakness). No damage rule reads
//! this: Sora's and the party's damage on Elden Ring enemies is unchanged.
//!
//! | Donald | Elden Ring | rule (sheet party-don-progression `insight.*`) |
//! |---|---|---|
//! | Fire | `fireDamageCutRate` | weak at >= `insight.cut_weak` (1.1), resisted at <= `insight.cut_resist` (0.85) |
//! | Thunder | `thunderDamageCutRate` (lightning) | the same |
//! | Light | `darkDamageCutRate` (Elden Ring's holy is "dark" in its params) | the same |
//! | Blizzard | `resistFreeze` (frostbite buildup: frost is Elden Ring's ice) | weak at <= `insight.frost_weak`, resisted at >= `insight.frost_resist`, 999 immune |
//! | Water, Aero | none in Elden Ring | neutral |
//!
//! The cut rate multiplies the damage taken (1 = neutral). The values are by `ETresDamageAttribute` (1 Fire, 2 Blizzard,
//! 3 Thunder; slot 7 carries Light: `Targets::element_affinity`), as resist % (negative = weak). UNTESTED IN GAME: the
//! thresholds are guesses until the regulation's values are surveyed.

use std::collections::HashMap;
use std::sync::Mutex;

use kh_party::donald::ability_info::ProgData;

/// One enemy row's numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ErElements {
    pub fire_cut: f32,
    pub thunder_cut: f32,
    pub holy_cut: f32,
    pub frost_resist: u16,
}

/// THE PURE MAPPING onto KH3's resist slots (resist %, negative = weak): Fire 1, Blizzard 2, Thunder 3, Light 7.
pub fn affinity(e: &ErElements, p: &ProgData) -> [i32; 8] {
    let (weak, resist) = (p.value("insight.cut_weak"), p.value("insight.cut_resist"));
    let cut = |rate: f32| -> i32 {
        if !rate.is_finite() {
            0
        } else if rate >= weak {
            -(((rate - 1.0) * 100.0).round() as i32).max(1)
        } else if rate <= resist {
            (((1.0 - rate) * 100.0).round() as i32).clamp(1, 100)
        } else {
            0
        }
    };
    let frost = if e.frost_resist >= 999 {
        100
    } else if e.frost_resist as f32 >= p.value("insight.frost_resist") {
        50
    } else if e.frost_resist as f32 <= p.value("insight.frost_weak") {
        -25
    } else {
        0
    };
    let mut out = [0; 8];
    out[1] = cut(e.fire_cut);
    out[2] = frost;
    out[3] = cut(e.thunder_cut);
    out[7] = cut(e.holy_cut);
    out
}

static CACHE: Mutex<Option<HashMap<i32, Option<ErElements>>>> = Mutex::new(None);

/// The NpcParam row's numbers (cached by row id; None when the row cannot be read).
#[cfg(windows)]
pub fn of_row(npc_param: i32) -> Option<ErElements> {
    use eldenring::cs::{NpcParam, SoloParamRepository};
    use fromsoftware_shared::FromStatic;
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(v) = cache.get(&npc_param) {
        return *v;
    }
    // SAFETY: the game's singleton; one row by id (the reader blood.rs uses).
    let row = unsafe { SoloParamRepository::instance() }.ok().and_then(|repo| repo.get::<NpcParam>(npc_param as u32).map(|r| ErElements { fire_cut: r.fire_damage_cut_rate(), thunder_cut: r.thunder_damage_cut_rate(), holy_cut: r.dark_damage_cut_rate(), frost_resist: r.resist_freeze() }));
    if let Some(e) = row {
        crate::erlog!("insight: NpcParam {npc_param}: fire x{:.2}, lightning x{:.2}, holy x{:.2}, frost resist {} -> {:?}", e.fire_cut, e.thunder_cut, e.holy_cut, e.frost_resist, affinity(&e, &kh_party::donald::ability_info::prog()));
    }
    cache.insert(npc_param, row);
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn don_prog_elden_rings_numbers_map_onto_donalds_elements() {
        let p = ProgData::sheet();
        let neutral = ErElements { fire_cut: 1.0, thunder_cut: 1.0, holy_cut: 1.0, frost_resist: 250 };
        assert_eq!(affinity(&neutral, &p), [0; 8], "average: nothing marked");
        // a fire-weak, lightning-resistant, holy-weak enemy, frost immune
        let e = ErElements { fire_cut: 1.3, thunder_cut: 0.6, holy_cut: 1.1, frost_resist: 999 };
        let a = affinity(&e, &p);
        assert_eq!((a[1], a[3], a[7], a[2]), (-30, 40, -10, 100));
        // just inside the neutral band: 1.05 / 0.9 are not marked; frost weak / resisted by buildup
        let near = ErElements { fire_cut: 1.05, thunder_cut: 0.9, holy_cut: 0.85, frost_resist: 100 };
        let a = affinity(&near, &p);
        assert_eq!((a[1], a[3], a[7], a[2]), (0, 0, 15, -25));
        assert_eq!(affinity(&ErElements { frost_resist: 450, ..neutral }, &p)[2], 50);
        // Water and Aero (slots 4 / 5): always neutral
        assert_eq!((a[4], a[5]), (0, 0));
        // read as Donald's elements: DON-UI's Insight and AI Donald's weakness
        let (weak, strong) = kh_party::donald::hud::affinity_of_resist(&affinity(&e, &p));
        use kh_party::donald::Element as E;
        assert!(weak.has(E::Fire) && weak.has(E::Light) && strong.has(E::Thunder) && strong.has(E::Blizzard));
    }
}
