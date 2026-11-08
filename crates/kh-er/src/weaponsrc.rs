//! (P9b ACQUISITION, documentation/KH3_WEAPONS_PLAN.md 1.7, 2, 6b) WHERE KH3's WEAPONS COME FROM IN ELDEN RING: the game's
//! side. The rules and the sheets are kh-host's (`kh_host::bossrewards`: the `kb.*` boss Keyblades, P9a's `chest.*` /
//! `drop.*` rows of sheets/port/kh-weapon-sources.json; `kh_host::shop::kh3_stock`: the merchants' KH3 stock). UNTESTED IN
//! THE GAME: written by reading, the rules unit-tested.
//!
//! - BOSS KEYBLADES AND CHESTS: read off the game's event flags by the boss rewards' poll (bossreward.rs: the boss row's
//!   verified defeat flag; the chest lot's getItemFlagId, set by the game when the treasure is taken), once per save
//!   (records `kb:<boss>`, `wsrc:<flag>`), the save's catch-up included.
//! - RARE DROPS (this file): an enemy's REAL DEATH (sora.rs's kill handling, where the prizes and the loot boxes are thrown)
//!   rolls each `drop.*` row of its NpcParam row once (chance 0.02); what comes out is thrown as a KH3 prize box (a "rare"
//!   box: kh_prize `BoxKind::of_item` of a weapon id) whose arrival hands the weapon to the weapons' door (the OBTAINED
//!   popup then). WHAT IS NOT A REAL DEATH: a death reported after a load was asked for (kh-er prize.rs `LOAD_EPOCH`: the
//!   world of the abandoned session going away), one while the leader is dead (the reload of the respawn), the same target
//!   counted again within [`DEDUPE`] seconds. A Keyblade's row only while it is not owned; a friend's weapon may come again.
//!
//! TEST AIDS (one-shots of the TEST profile's ini, off unless written; never the user's):
//! | key | meaning |
//! |---|---|
//! | `test.wsrc_drop = N` | the next N real deaths of a `drop.*` kind roll a hit (no 2 % wait) |
//! | `test.wsrc_flag = F[,F..]` | the game's event flag F is set (a chest's getItemFlagId or a boss's defeat flag of the sheets): the next poll gives its weapon once |
//! | `test.wsrc_give = KEY` | the weapon KEY (`WEP_KEYBLADE_SO_05`, `WEP_DONALD_05` ...) through the weapons' door: the OBTAINED popup |
//!
//! THE LOG (prefix `weapons:`): the sheet's counts at the first kill; each roll of a drop kind with its unit; each refusal
//! (load / dead / again); each box thrown.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};

use glam::Vec3;
use kh_host::bossrewards::{self, WeaponSources};
use kh_host::prize::PrizeHost;
use kh_prize::Rng;

use crate::control::{Control, OneShot};
use crate::erlog;

/// Seconds within which the same target's death is not counted again.
pub const DEDUPE: f32 = 60.0;

/// `test.wsrc_drop`: forced hits left.
static FORCED: AtomicU32 = AtomicU32::new(0);

/// Whether a death counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Death {
    Real,
    /// A load was asked for since this Sora started.
    Load,
    /// The leader is dead (the respawn's reload).
    Dead,
    /// The same target was counted within [`DEDUPE`] seconds.
    Again,
}

/// One death as sora.rs's kill handling knows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kill {
    /// The target id.
    pub target: u32,
    /// Its NpcParam row.
    pub npc_param: i32,
    /// Where it fell (cm, KH frame).
    pub at: Vec3,
    /// The leader is dead (or his death is being handed to the game).
    pub dead: bool,
}

impl Default for ErWeaponSources {
    fn default() -> ErWeaponSources {
        ErWeaponSources::new()
    }
}

pub struct ErWeaponSources {
    /// `LOAD_EPOCH` when this Sora started.
    epoch: u32,
    /// (target, when it was counted: seconds of this session).
    recent: VecDeque<(u32, f32)>,
    rng: Rng,
    started: std::time::Instant,
    told: bool,
    /// The latest log lines (the tests read them).
    pub said: VecDeque<String>,
}

impl ErWeaponSources {
    /// A new Sora (a session start or a respawn): the loads asked for so far (kh-er prize.rs `load_epoch`).
    pub fn new() -> ErWeaponSources {
        ErWeaponSources::with_epoch(crate::prize::load_epoch())
    }

    /// A new Sora at this load epoch (the tests).
    pub fn with_epoch(epoch: u32) -> ErWeaponSources {
        ErWeaponSources { epoch, recent: VecDeque::new(), rng: Rng::new(0x5753_5243), started: std::time::Instant::now(), told: false, said: VecDeque::new() }
    }

    fn say(&mut self, line: String) {
        erlog!("weapons: {line}");
        if self.said.len() >= 64 {
            self.said.pop_front();
        }
        self.said.push_back(line);
    }

    /// THE RULE OF A REAL DEATH (pure): `now` = seconds of this session; a real one is remembered for [`DEDUPE`] seconds.
    pub fn count(&mut self, target: u32, epoch_now: u32, dead: bool, now: f32) -> Death {
        while self.recent.front().is_some_and(|(_, at)| now - *at > DEDUPE) {
            self.recent.pop_front();
        }
        if epoch_now != self.epoch {
            return Death::Load;
        }
        if dead {
            return Death::Dead;
        }
        if self.recent.iter().any(|(t, _)| *t == target) {
            return Death::Again;
        }
        self.recent.push_back((target, now));
        Death::Real
    }

    /// THE DROPS OF ONE DEATH (pure): the rows rolled (a hit below their chance, or a forced hit), with the unit rolled.
    pub fn roll<'a>(&mut self, sources: &'a WeaponSources, npc_param: i32, forced: bool, owns: &dyn Fn(&str) -> bool) -> Vec<(&'a bossrewards::DropSource, f32)> {
        let mut units = Vec::new();
        let rng = &mut self.rng;
        let hits = bossrewards::drop_roll(sources, npc_param, &mut || {
            let u = if forced { 0.0 } else { rng.unit() };
            units.push(u);
            u
        }, owns);
        hits.into_iter().zip(units).collect()
    }

    /// AN ENEMY'S DEATH (sora.rs's kill handling, after the prizes and the loot boxes). A real death of a `drop.*` kind
    /// rolls; a hit is a KH3 prize box at the body.
    pub fn on_kill(&mut self, control: &Control, sora: &mut kh_core::Sora, host: &mut PrizeHost, kill: Kill) {
        let Kill { target, npc_param, at, dead } = kill;
        let epoch_now = crate::prize::load_epoch();
        let sources = bossrewards::weapon_sources();
        if !self.told {
            self.told = true;
            self.say(format!("sheet kh-weapon-sources: {} chest row(s) ({} verified), {} drop row(s) ({} verified){}", sources.chests.len(), sources.chests.iter().filter(|c| c.verified).count(), sources.drops.len(), sources.drops.iter().filter(|d| d.verified).count(), if sources.problems.is_empty() { String::new() } else { format!(" | PROBLEM(S): {}", sources.problems.join("; ")) }));
        }
        if sources.drops_of(npc_param).next().is_none() {
            return;
        }
        let now = self.started.elapsed().as_secs_f32();
        match self.count(target, epoch_now, dead, now) {
            Death::Real => {}
            why => {
                self.say(format!("target {target} (NpcParam {npc_param}) of a drop kind died: NOT COUNTED ({why:?}: {})", match why {
                    Death::Load => "a load was asked for since this Sora started",
                    Death::Dead => "the leader is dead",
                    _ => "counted already",
                }));
                return;
            }
        }
        let forced = FORCED.load(Ordering::Relaxed) > 0;
        if forced {
            FORCED.fetch_sub(1, Ordering::Relaxed);
        }
        let hits = self.roll(sources, npc_param, forced, &kh_host::weapons::owns);
        let rows: Vec<String> = sources.drops_of(npc_param).map(|d| format!("{} ({}, {} %)", d.id, d.kind, d.chance * 100.0)).collect();
        if hits.is_empty() {
            self.say(format!("target {target} (NpcParam {npc_param}) died: rolled {}: no drop", rows.join(", ")));
            return;
        }
        let height = control.f32("prize.height").filter(|v| (-200.0..=500.0).contains(v)).unwrap_or(80.0);
        for (d, unit) in hits {
            let what = match host.prizes.drop_item_box(at + Vec3::Z * height, d.item_id, false) {
                Some(id) => format!("KH3 prize box {id} thrown (its arrival: the weapons' door, the OBTAINED popup)"),
                None => "NO BOX (item id 0)".to_string(),
            };
            self.say(format!("RARE DROP: target {target} (NpcParam {npc_param}, {}): {} {} (rolled {unit:.4} < {}{}): {what}", d.kind, d.key, kh_host::weapons::by_key(&d.key).map_or("?", |w| w.name.as_str()), d.chance, if forced { ", forced by test.wsrc_drop" } else { "" }));
        }
        host.take(sora);
    }
}

// --------------------------------------------------------------------------------------------------------- the test aids

struct Aids {
    drop: OneShot,
    flag: OneShot,
    give: OneShot,
}

static AIDS: std::sync::Mutex<Option<Aids>> = std::sync::Mutex::new(None);

/// Once a frame (bossreward.rs): the one-shots `test.wsrc_*` of the test profile.
pub fn aids(control: &Control) {
    let mut g = AIDS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let a = g.get_or_insert_with(|| Aids { drop: OneShot::new("test.wsrc_drop", control), flag: OneShot::new("test.wsrc_flag", control), give: OneShot::new("test.wsrc_give", control) });
    if let Some(n) = a.drop.fired(control).and_then(|v| v.trim().parse::<u32>().ok()) {
        FORCED.store(n.min(100), Ordering::Relaxed);
        erlog!("weapons: test.wsrc_drop = {n}: the next {} real death(s) of a drop kind roll a hit", n.min(100));
    }
    if let Some(text) = a.flag.fired(control) {
        for f in text.split([',', ' ']).filter_map(|s| s.trim().parse::<u32>().ok()) {
            erlog!("weapons: test.wsrc_flag: event flag {f} {}", if set_flag(f) { "SET (the next poll gives its weapon once)" } else { "NOT set (no flag manager)" });
        }
    }
    if let Some(key) = a.give.fired(control) {
        match kh_host::weapons::find(key.trim()) {
            Some(w) => erlog!("weapons: test.wsrc_give = {key}: {} {:?}", w.name, kh_host::weapons::give(w.item_id, 1)),
            None => erlog!("weapons: test.wsrc_give = {key}: no such weapon"),
        }
    }
}

fn set_flag(flag: u32) -> bool {
    use eldenring::cs::CSEventFlagMan;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the game's own flag tables (the SDK's writer looks the block up and does nothing without one).
    let Ok(man) = (unsafe { CSEventFlagMan::instance_mut() }) else { return false };
    if man.virtual_memory_flag.flag_blocks.is_null() {
        return false;
    }
    man.virtual_memory_flag.set_flag(flag, true);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DROPS COUNTED ONCE: the same target again (the death reported twice) is not a second roll; a death after a load was
    /// asked for, or while the leader is dead (the respawn's reload), is no roll; a new session (a new epoch: the next Sora)
    /// rolls again; the same body much later (the enemies respawned at a rest) is a new death.
    #[test]
    fn a_death_counts_once_and_never_through_a_load_or_a_respawn() {
        let mut w = ErWeaponSources::with_epoch(7);
        assert_eq!(w.count(11, 7, false, 1.0), Death::Real);
        assert_eq!(w.count(11, 7, false, 2.0), Death::Again, "the same death reported again");
        assert_eq!(w.count(12, 7, false, 2.0), Death::Real, "another enemy");
        assert_eq!(w.count(13, 7, true, 3.0), Death::Dead, "the leader is dead: the respawn's reload");
        assert_eq!(w.count(13, 8, false, 4.0), Death::Load, "a load was asked for since this Sora started");
        assert_eq!(w.count(11, 7, false, 1.0 + DEDUPE + 1.0), Death::Real, "the enemy came back (a rest): a new death");
        let mut next = ErWeaponSources::with_epoch(8);
        assert_eq!(next.count(13, 8, false, 0.5), Death::Real, "the loaded session's Sora counts again");
    }

    /// THE ROLL: a Raya Lucaria sorcerer's row (NpcParam 37020020) gives Mage's Staff+ on a hit; a forced roll always hits;
    /// an enemy of no drop kind rolls nothing; the dedupe keeps a second report of the death from a second roll.
    #[test]
    fn the_rare_drop_rolls_per_real_death() {
        let src = WeaponSources::parse(bossrewards::WEAPON_SOURCES_SHEET);
        assert!(src.problems.is_empty(), "{:?}", src.problems);
        let mut w = ErWeaponSources::with_epoch(0);
        let none = |_: &str| false;
        let hit = w.roll(&src, 37020020, true, &none);
        assert_eq!(hit.iter().map(|(d, _)| d.key.as_str()).collect::<Vec<_>>(), ["WEP_DONALD_01"]);
        assert!(w.roll(&src, 10000000, true, &none).is_empty(), "no drop kind");
        // 2 %: over many real deaths about 2 in 100 (the seeded generator)
        let n = (0..5000).filter(|_| !w.roll(&src, 37020020, false, &none).is_empty()).count();
        assert!((50..=150).contains(&n), "{n} of 5000");
        // the same target reported twice: one roll only
        let mut rolls = 0;
        for _ in 0..2 {
            if w.count(99, 0, false, 5.0) == Death::Real {
                rolls += w.roll(&src, 37020020, true, &none).len();
            }
        }
        assert_eq!(rolls, 1);
    }
}
