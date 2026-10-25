//! (DON1) THE NUMBERS: the two shared sheets of the overhaul, compiled in (tools/gen_port.js) and tunable WITHOUT A
//! REBUILD (the doc, section 2: "Every stat is a sheet row, so it can be tuned without a rebuild"):
//!
//! - `sheets/port/party-don-core.json` -> [`DonTune`] (attunement, Resonance, Arcane Flow, statuses, reactions, colours);
//! - `sheets/port/party-don-spells.json` -> [`DonSpellRow`] rows (every castable spell and level).
//!
//! The compiled values are the defaults. A host that reads the sheet files again at run time (kh-partyhost
//! `don_sheets`, the established route of kh-er's `bossreward.rs`: the repo's sheet next to the build) or a ctl key
//! (`don.<row id> = value`, `don.spell.<spell id>.<column> = value`) applies them with [`DonData::set`] and publishes the
//! result with [`install`]; everything in `crate::donald` reads [`data`]. Engine-free: no file is read here.

use std::sync::{Arc, RwLock};

include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_don_core.rs"));
include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_don_spells.rs"));

/// The core sheet's numbers (one field per row of party-don-core).
pub type DonTune = DonCoreParams;

/// Everything tunable of the overhaul's shared rules.
#[derive(Clone, Debug, PartialEq)]
pub struct DonData {
    pub tune: DonTune,
    pub spells: Vec<DonSpellRow>,
}

impl Default for DonData {
    fn default() -> DonData {
        DonData::sheet()
    }
}

/// Sets one field of the core sheet by its row id (`res.gain_spell`) or field name (`res_gain_spell`).
macro_rules! tune_fields {
    ($($f:ident),* $(,)?) => {
        fn set_tune(t: &mut DonTune, field: &str, v: f32) -> bool {
            match field {
                $(stringify!($f) => { t.$f = v; true })*
                _ => false,
            }
        }
        fn get_tune(t: &DonTune, field: &str) -> Option<f32> {
            match field {
                $(stringify!($f) => Some(t.$f),)*
                _ => None,
            }
        }
    };
}

tune_fields!(
    attune_cast, attune_swift_mult, attune_instant_mult, attune_release_start, res_pips, res_gain_spell, res_gain_missile, res_gain_reaction,
    res_gain_perfect_block, res_heavy_loss, res_drain, res_drain_delay, res_keeper_mult, res_refund_pct, res_quick_window, res_reaction_cooldown,
    arcane_max, arcane_life, arcane_life_plus, arcane_elements_window, arcane_elements_needed, arcane_reaction_cooldown, arcane_mp_pct,
    arcane_zetta_mp_pct, fight_end_delay, wet_duration, wet_full_mult, chill_duration, chill_slow_per_stack, chill_freeze_at, frozen_duration,
    frozen_full_duration, frozen_hit_shorten, frozen_rearm, big_chill_stacks, big_slow, big_slow_duration, burn_duration, burn_tick, burn_tick_power,
    reveal_duration, exposed_mult, blind_duration, blind_miss, launch_hits, react_conduct_mult, react_conduct_radius, react_steam_mult,
    react_shatter_mult, react_wildfire_radius, react_fizzle_mult, react_plus_mult, echo_window, cost_scale, color_fire, color_blizzard,
    color_thunder, color_water, color_aero, color_light, color_arcane,
);

fn field_of(key: &str) -> String {
    key.trim().replace(['.', '-'], "_").to_ascii_lowercase()
}

impl DonData {
    /// The compiled sheets.
    pub fn sheet() -> DonData {
        DonData { tune: DON_CORE_PARAMS, spells: DON_SPELLS.to_vec() }
    }

    /// One value by its key: a core row (`res.gain_spell`), or a spell's numeric column (`spell.l1.fire.4.cost_att`;
    /// the columns cost_free, cost_att, cost_quick, arcane, cast, power, extra, radius, range, angle, push, duration,
    /// count, full_power / once_per_period as 0 / 1). False when the key names nothing.
    pub fn set(&mut self, key: &str, v: f32) -> bool {
        let key = key.trim();
        let key = key.strip_prefix("don.").unwrap_or(key);
        if let Some(rest) = key.strip_prefix("spell.") {
            let Some((id, col)) = rest.rsplit_once('.') else { return false };
            let Some(row) = self.spells.iter_mut().find(|r| r.id.eq_ignore_ascii_case(id)) else { return false };
            match col {
                "cost_free" => row.cost_free = v,
                "cost_att" => row.cost_att = v,
                "cost_quick" => row.cost_quick = v,
                "arcane" => row.arcane = v.max(0.0) as u32,
                "cast" => row.cast = v,
                "power" => row.power = v,
                "extra" => row.extra = v,
                "radius" => row.radius = v,
                "range" => row.range = v,
                "angle" => row.angle = v,
                "push" => row.push = v,
                "duration" => row.duration = v,
                "count" => row.count = v.max(0.0) as u32,
                "full_power" => row.full_power = v != 0.0,
                "once_per_period" => row.once_per_period = v != 0.0,
                _ => return false,
            }
            return true;
        }
        set_tune(&mut self.tune, &field_of(key), v)
    }

    /// A core value by its row id or field name.
    pub fn get(&self, key: &str) -> Option<f32> {
        let key = key.trim();
        get_tune(&self.tune, &field_of(key.strip_prefix("don.").unwrap_or(key)))
    }

    /// Every (key, value) of a list onto the data; returns the keys that named nothing.
    pub fn apply<'a>(&mut self, pairs: impl IntoIterator<Item = (&'a str, f32)>) -> Vec<&'a str> {
        pairs.into_iter().filter(|(k, v)| !self.set(k, *v)).map(|(k, _)| k).collect()
    }

    /// A spell row by its id (`l1.fire.4`, `l2.light.2`, `hy.fire_water`, `y.meteor.3`, `rel.aero`).
    pub fn row(&self, id: &str) -> Option<&DonSpellRow> {
        self.spells.iter().find(|r| r.id == id)
    }
}

static DATA: RwLock<Option<Arc<DonData>>> = RwLock::new(None);

/// The data every rule reads (the compiled sheets until a host [`install`]s its own).
pub fn data() -> Arc<DonData> {
    if let Some(d) = DATA.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return d.clone();
    }
    let mut w = DATA.write().unwrap_or_else(|e| e.into_inner());
    w.get_or_insert_with(|| Arc::new(DonData::sheet())).clone()
}

/// The host's data (the sheets read again, the ctl overrides) for every later [`data`].
pub fn install(d: DonData) {
    *DATA.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(d));
}

/// Every core row id the sheet has (for a host's key listing and the coverage test).
pub fn core_keys() -> impl Iterator<Item = &'static str> {
    DON_CORE_PARAM_ROWS.iter().map(|r| r.1)
}
