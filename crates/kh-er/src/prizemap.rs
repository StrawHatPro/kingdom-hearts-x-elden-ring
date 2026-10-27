//! (prize) THE PURE HALF of the prizes in this host: THE ONE RULE for what a killed Elden Ring
//! enemy drops (rows `er.*` of sheets/port/sora-prizes.json, compiled in through kh-prize; control
//! keys override) and the WALLET (what a kill is worth = what the game itself awards for it; with
//! the mirror, Sora's munny shown as the game's rune counter, so that what costs runes costs
//! munny). No SDK, no game: unit-tested here. prize.rs is the game half.
//!
//! WHAT KH3 DOES AND WHAT IS SCALING. In KH3 an enemy's drop is its own fixed table (two prize
//! kinds with counts, three items with rates: docs/sora-prize-spec.md 2); NOTHING in it looks at
//! Sora's HP or MP. An Elden Ring enemy has no such table, so this host builds one from the two
//! numbers it can read: the enemy's maximum HP (a tier) and its rune value (the munny). Every
//! number of that construction is a row marked MARKED SCALING.

use kh_core::status::PrizeKind;
use kh_prize::params::P;
use kh_prize::Rng;

/// The rule's numbers (the sheet's, with the control file's overrides).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    pub tier_hp: [f32; 2],
    /// HP, MP, focus.
    pub slot1_weights: [f32; 3],
    pub slot1_count: [u32; 3],
    pub munny_count: [u32; 3],
    pub munny_per_rune: f32,
    pub munny_per_max_hp: f32,
    pub spawn_height: f32,
}

impl Default for Rule {
    fn default() -> Rule {
        Rule {
            tier_hp: P.er_tier_hp,
            slot1_weights: P.er_slot1_weights,
            slot1_count: P.er_slot1_count.map(|c| c as u32),
            munny_count: P.er_munny_count.map(|c| c as u32),
            munny_per_rune: P.er_munny_per_rune,
            munny_per_max_hp: P.er_munny_per_max_hp,
            spawn_height: P.er_spawn_height,
        }
    }
}

/// One slot of a kill's drop: `count` orbs of `kind`; for munny, what each is worth.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub kind: PrizeKind,
    pub count: u32,
    pub amounts: Vec<i32>,
}

/// What a kill drops.
#[derive(Clone, Debug, PartialEq)]
pub struct Drop {
    pub tier: usize,
    /// The munny of the kill and whether it came from the enemy's rune value.
    pub munny: i32,
    pub from_runes: bool,
    pub slots: Vec<Slot>,
}

impl Rule {
    /// `prize.weights = hp,mp,focus`, `prize.munny_rate = x`, `prize.munny_hp = x`.
    pub fn with_overrides(mut self, weights: Option<&str>, munny_rate: Option<f32>, munny_hp: Option<f32>) -> Rule {
        if let Some(text) = weights {
            let parts: Vec<f32> = text.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            if parts.len() == 3 && parts.iter().all(|w| *w >= 0.0) && parts.iter().sum::<f32>() > 0.0 {
                self.slot1_weights = [parts[0], parts[1], parts[2]];
            }
        }
        if let Some(rate) = munny_rate.filter(|r| *r >= 0.0) {
            self.munny_per_rune = rate;
        }
        if let Some(rate) = munny_hp.filter(|r| *r >= 0.0) {
            self.munny_per_max_hp = rate;
        }
        self
    }

    pub fn tier(&self, max_hp: i32) -> usize {
        match max_hp as f32 {
            hp if hp < self.tier_hp[0] => 0,
            hp if hp < self.tier_hp[1] => 1,
            _ => 2,
        }
    }

    /// THE RULE. `runes`: the enemy's rune value when it could be read (NpcParam getSoul), else
    /// the munny is `ceil(max HP x munny_per_max_hp)`. One draw of `rng` picks the first slot's
    /// family by `slot1_weights`.
    pub fn drop_for(&self, max_hp: i32, runes: Option<i32>, rng: &mut Rng) -> Drop {
        let tier = self.tier(max_hp);
        let large = tier == 2;
        let total: f32 = self.slot1_weights.iter().sum();
        let roll = rng.unit() * total;
        let family = if roll < self.slot1_weights[0] {
            0
        } else if roll < self.slot1_weights[0] + self.slot1_weights[1] {
            1
        } else {
            2
        };
        let first = match (family, large) {
            (0, false) => PrizeKind::HpSmall,
            (0, true) => PrizeKind::HpLarge,
            (1, false) => PrizeKind::MpSmall,
            (1, true) => PrizeKind::MpLarge,
            (_, false) => PrizeKind::FocusSmall,
            (_, true) => PrizeKind::FocusLarge,
        };
        let (munny, from_runes) = match runes.filter(|r| *r > 0) {
            Some(runes) => ((runes as f32 * self.munny_per_rune).round() as i32, true),
            None => ((max_hp.max(0) as f32 * self.munny_per_max_hp).ceil() as i32, false),
        };
        let mut slots = vec![Slot { kind: first, count: self.slot1_count[tier], amounts: Vec::new() }];
        slots.extend(self.munny_slot(tier, munny));
        Drop { tier, munny, from_runes, slots }
    }

    /// The munny orbs of a kill of this tier worth `munny` together (none when it is worth nothing).
    pub fn munny_slot(&self, tier: usize, munny: i32) -> Option<Slot> {
        if munny <= 0 {
            return None;
        }
        let tier = tier.min(2);
        let count = self.munny_count[tier].max(1).min(munny as u32);
        // Split evenly; the remainder goes on the first orbs.
        let (each, rest) = (munny / count as i32, munny % count as i32);
        let amounts = (0..count as i32).map(|k| each + (k < rest) as i32).collect();
        Some(Slot { kind: [PrizeKind::Munny1, PrizeKind::Munny5, PrizeKind::Munny10][tier], count, amounts })
    }
}

/// (IMPL_ER20) THE HOST'S VALUE OF AN HP ORB: the slot's orbs carry `small` / `large` hit points instead of KH3's
/// 1 / 5 (kh-core applies an HP orb's amount when it is not 0). KH3's own values leave the slot as it is; MP and
/// focus orbs are never touched.
pub fn hp_amounts(slot: &mut Slot, small: i32, large: i32) {
    let (own, value) = match slot.kind {
        PrizeKind::HpSmall => (1, small),
        PrizeKind::HpLarge => (5, large),
        _ => return,
    };
    if value > 0 && value != own {
        slot.amounts = vec![value; slot.count as usize];
    }
}

// ------------------------------------------------------------------------------- the wallet

/// The game's rune counter cannot hold more.
pub const RUNE_MAX: u32 = 999_999_999;
/// Seconds a kill waits for the game's own rune award before its fallback value is paid.
/// MEASURED (IMPL_ER9_STATE.md): the game adds a sheep's runes 0.85 s after the blow that killed it (when its
/// death has played), so the wait must cover a death animation.
pub const KILL_WAIT: f32 = 5.0;
/// (IMPL_ER19) Seconds an UNPRICED kill (its row has no rune value: a boss, whose runes the game awards by its own
/// event, or an enemy the game pays nothing for) waits for the game's award before it is closed WITHOUT munny.
/// MEASURED in the user's session of 2026-10-13: the Soldier of Godrick's 400 came 7.7 s after the killing blow, the
/// Tree Sentinel's 3200 after 6.2 s; nine rune-less enemies (NpcParam 41000910 / 41002910 / 41200910) got nothing.
pub const UNPRICED_WAIT: f32 = 20.0;
/// (IMPL_ER19) An award more than this many times what ALL the waiting priced kills are worth together is not
/// theirs when an unpriced kill waits too (a boss's runes arriving while a small enemy's are still due).
pub const UNPRICED_RATIO: f64 = 3.0;
/// Seconds a rune gain waits for a kill to claim it before it counts as a gain of another kind
/// (a rune item used, something sold).
pub const GAIN_WAIT: f32 = 0.5;
/// Seconds a fallback payment is remembered: a late award of the game for that kill (a boss's
/// runes come seconds after its death) is not paid a second time.
pub const ADVANCE_LIFE: f32 = 30.0;

/// What the game half reads for one frame of the wallet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalletIn {
    /// The game's rune counter and the Tarnished's level now.
    pub runes: u32,
    pub level: u32,
    /// Sora's munny now.
    pub munny: i32,
    /// The game's player is dead or dying.
    pub dead: bool,
    /// `munny.mirror`: the rune counter is the picture of the munny.
    pub mirror: bool,
    /// `munny.mirror.gains`: a rune gain that no kill claims becomes munny.
    pub accept_gains: bool,
    /// He rests at a site of grace (the game's grace menu is up): the counter shows 0, so that its
    /// Level Up can raise nothing.
    pub resting: bool,
    /// The first frame of a session (not a respawn): the counter of the game's save IS the munny.
    pub adopt: bool,
    pub dt: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalletEvent {
    /// The session's first frame: Sora's munny became what the game's save held.
    Adopted { runes: u32, munny_before: i32 },
    /// The first frame after a respawn (or of a session without adoption): the counter was set to the munny.
    Synced { munny: u32, held: u32 },
    /// A kill's munny is known: the host throws its munny orbs. `awarded`: it is what the game
    /// itself added to its counter for the kill; else the fallback (the rune value read from the
    /// enemy's row, or the max-HP rule) because the game added nothing in time.
    KillPaid { key: u32, munny: i32, awarded: bool, fallback: i32 },
    /// A gain no kill claimed became munny / was taken away again (`munny.mirror.gains = 0`) /
    /// stays in the game's counter (no mirror).
    GainTaken(u32),
    GainDropped(u32),
    GainLeft(u32),
    /// The game took runes: the same munny is spent.
    Spent(u32),
    /// The Tarnished's level rose: NOT charged; the host puts the level and the attributes back.
    LevelUp { from: u32, to: u32, runes: u32 },
    /// The player died holding this many runes in the counter: it was emptied before the game could drop them.
    DeathHeld(u32),
    /// The counter was not 0 on a later frame of the death (the game wrote it): emptied again.
    DeathAgain(u32),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WalletOut {
    /// Write this into the game's rune counter.
    pub write_runes: Option<u32>,
    /// Add this to Sora's munny (negative: something was bought). Kills are NOT in here: their
    /// munny comes with the orbs.
    pub munny_delta: i32,
    pub events: Vec<WalletEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PendingKill {
    key: u32,
    weight: f32,
    fallback: i32,
    age: f32,
    /// Its row names a rune value: the fallback is that value. Not priced: nothing is ever paid for it but what
    /// the game itself awards.
    priced: bool,
}

/// THE WALLET: Sora's munny against the game's rune counter.
///
/// WHAT A KILL IS WORTH. The game adds a kill's runes to its counter itself, with every rate it
/// knows (the area's scaling, the journey number, rune-gain effects). The wallet watches the
/// counter: a rise that comes within [`KILL_WAIT`] of a kill IS that kill's value, and the kill's
/// munny orbs are made worth exactly that (several kills in the window share it by their rune
/// values). Only when the game adds nothing in time is the fallback paid.
///
/// THE MIRROR (`mirror`). Sora's munny is the truth and the counter its picture, so that vendors,
/// smiths and everything else that asks "has he N runes?" asks about munny: what the game adds
/// is taken out again (it arrives as orbs, or directly when no kill claims it), what the game
/// takes is spent. While he rests the counter shows 0 (nothing in the grace menu costs runes
/// except Level Up, which is not part of this game). While the player is dead the counter is held
/// at 0: THE GAME HAS NOTHING TO DROP, and the munny is not touched by the death.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wallet {
    /// What the counter held after our last look (and write).
    shown: Option<u32>,
    level: u32,
    pending: Vec<PendingKill>,
    /// A gain no kill has claimed yet, and its age.
    pool: (u32, f32),
    /// Fallback payments a late award may still cancel against, and the age of the latest.
    advance: (i32, f32),
    dead_told: bool,
    /// The mirror was on at our last look (switching it starts over with a sync).
    mirrored: bool,
    /// (IMPL_ER19) The waits of a priced / an unpriced kill when the host set them (`set_waits`).
    waits: Option<(f32, f32)>,
}

impl Wallet {
    /// An enemy of Sora's died. `weight`: its share when several kills split one award (its rune
    /// value); `fallback`: the munny paid when the game awards nothing in time.
    pub fn kill(&mut self, key: u32, weight: f32, fallback: i32) {
        self.pending.push(PendingKill { key, weight: weight.max(0.0), fallback: fallback.max(0), age: 0.0, priced: true });
    }

    /// (IMPL_ER19) An enemy of Sora's died WHOSE ROW NAMES NO RUNE VALUE (a boss: the game awards its runes by an
    /// event of its own, seconds later; or an enemy the game pays nothing for). It waits [`UNPRICED_WAIT`] for the
    /// game's award and is worth exactly that; without one it is worth NOTHING (no guess is ever paid: with the
    /// mirror a guess would be runes the game never gave). `weight`: its size among unpriced kills (its maximum
    /// HP): the biggest waiting one takes an award.
    pub fn kill_unpriced(&mut self, key: u32, weight: f32) {
        self.pending.push(PendingKill { key, weight: weight.max(0.0), fallback: 0, age: 0.0, priced: false });
    }

    /// (IMPL_ER19) The seconds a priced / an unpriced kill waits for the game's award (host rows; the constants else).
    pub fn set_waits(&mut self, priced: f32, unpriced: f32) {
        self.waits = Some((priced.clamp(0.5, 60.0), unpriced.clamp(0.5, 120.0)));
    }

    fn wait_of(&self, p: &PendingKill) -> f32 {
        let (priced, unpriced) = self.waits.unwrap_or((KILL_WAIT, UNPRICED_WAIT));
        if p.priced { priced } else { unpriced }
    }

    pub fn pending_kills(&self) -> usize {
        self.pending.len()
    }

    fn pay_fallbacks(&mut self, all: bool, out: &mut WalletOut) {
        let mut k = 0;
        while k < self.pending.len() {
            if all || self.pending[k].age >= self.wait_of(&self.pending[k]) {
                let p = self.pending.remove(k);
                // (an unpriced kill: fallback 0 = no munny, nothing advanced)
                out.events.push(WalletEvent::KillPaid { key: p.key, munny: p.fallback, awarded: false, fallback: p.fallback });
                if p.fallback > 0 {
                    self.advance = (self.advance.0 + p.fallback, 0.0);
                }
            } else {
                k += 1;
            }
        }
    }

    /// One game frame.
    pub fn frame(&mut self, i: WalletIn) -> WalletOut {
        let mut out = WalletOut::default();
        let munny = i.munny.max(0);
        if i.dead {
            // (kills still waiting: the game awards nothing to a dead player's wallet we could read)
            self.pay_fallbacks(true, &mut out);
            self.pool = (0, 0.0);
            if i.mirror && i.runes != 0 {
                out.write_runes = Some(0);
                out.events.push(if self.dead_told { WalletEvent::DeathAgain(i.runes) } else { WalletEvent::DeathHeld(i.runes) });
            }
            self.dead_told = true;
            self.shown = None;
            return out;
        }
        self.dead_told = false;
        if i.mirror != self.mirrored {
            self.mirrored = i.mirror;
            self.shown = None;
        }
        let Some(shown) = self.shown else {
            self.level = i.level;
            if !i.mirror {
                self.shown = Some(i.runes);
                return out;
            }
            if i.adopt {
                out.munny_delta = i.runes.min(i32::MAX as u32) as i32 - munny;
                out.events.push(WalletEvent::Adopted { runes: i.runes, munny_before: munny });
                self.shown = Some(i.runes);
            } else {
                let want = (munny as u32).min(RUNE_MAX);
                out.write_runes = (i.runes != want).then_some(want);
                out.events.push(WalletEvent::Synced { munny: want, held: i.runes });
                self.shown = Some(want);
            }
            return out;
        };
        // ---- what the game itself did to its counter since our last look
        let level_up = i.level > self.level;
        if i.runes > shown {
            if self.pool.0 == 0 {
                self.pool.1 = 0.0;
            }
            self.pool.0 = self.pool.0.saturating_add(i.runes - shown);
        } else if i.runes < shown {
            let taken = shown - i.runes;
            if level_up {
                out.events.push(WalletEvent::LevelUp { from: self.level, to: i.level, runes: taken });
            } else if i.mirror {
                let spent = taken.min(munny as u32);
                out.munny_delta -= spent as i32;
                out.events.push(WalletEvent::Spent(taken));
            }
        } else if level_up {
            out.events.push(WalletEvent::LevelUp { from: self.level, to: i.level, runes: 0 });
        }
        // (the host puts a raised level back: the level it must return to stays ours)
        if !level_up {
            self.level = i.level;
        }
        // ---- kills and gains find each other
        for p in &mut self.pending {
            p.age += i.dt;
        }
        if self.pool.0 > 0 {
            self.pool.1 += i.dt;
        }
        if self.advance.0 > 0 {
            self.advance.1 += i.dt;
            if self.advance.1 > ADVANCE_LIFE {
                self.advance = (0, 0.0);
            }
        }
        if self.pool.0 > 0 && !self.pending.is_empty() {
            // WHOSE AWARD IT IS. Among the PRICED kills: the oldest kill's; and the kills after it for as long as their
            // fallback values together still fit into the award (several enemies whose runes arrived in one frame).
            // Within that group it is shared by weight (the first take the remainder).
            // (IMPL_ER19) UNPRICED kills (bosses, rune-less enemies) stand aside while a priced kill waits, unless the
            // award is far more than everything the priced ones are worth; when only unpriced kills wait, the BIGGEST
            // takes the award whole (a boss among its rune-less company).
            let total = self.pool.0 as i64;
            let priced: Vec<usize> = (0..self.pending.len()).filter(|k| self.pending[*k].priced).collect();
            let unpriced: Vec<usize> = (0..self.pending.len()).filter(|k| !self.pending[*k].priced).collect();
            let priced_worth: i64 = priced.iter().map(|k| self.pending[*k].fallback as i64).sum();
            let to_unpriced = !unpriced.is_empty() && (priced.is_empty() || total as f64 > priced_worth as f64 * UNPRICED_RATIO);
            let taken: Vec<usize> = if to_unpriced {
                let mut best = unpriced[0];
                for k in &unpriced[1..] {
                    if self.pending[*k].weight > self.pending[best].weight {
                        best = *k;
                    }
                }
                vec![best]
            } else {
                let mut count = 1;
                let mut sum = self.pending[priced[0]].fallback as i64;
                while count < priced.len() && sum + self.pending[priced[count]].fallback as i64 <= (total as f64 * 1.05) as i64 {
                    sum += self.pending[priced[count]].fallback as i64;
                    count += 1;
                }
                priced[..count].to_vec()
            };
            let mut group: Vec<PendingKill> = Vec::with_capacity(taken.len());
            for k in taken.iter().rev() {
                group.insert(0, self.pending.remove(*k));
            }
            let weights: f32 = group.iter().map(|p| p.weight).sum();
            let n = group.len() as i64;
            let mut shares: Vec<i64> = group.iter().map(|p| if weights > 0.0 { (total as f64 * (p.weight / weights) as f64).floor() as i64 } else { total / n }).collect();
            let mut rest = total - shares.iter().sum::<i64>();
            let mut k = 0;
            while rest > 0 {
                let slot = k % shares.len();
                shares[slot] += 1;
                rest -= 1;
                k += 1;
            }
            for (p, share) in group.into_iter().zip(shares) {
                out.events.push(WalletEvent::KillPaid { key: p.key, munny: share.min(i32::MAX as i64) as i32, awarded: true, fallback: p.fallback });
            }
            self.pool = (0, 0.0);
        } else {
            self.pay_fallbacks(false, &mut out);
            if self.pool.0 > 0 && self.pool.1 >= GAIN_WAIT {
                // (a late award for a kill whose fallback was paid already is not paid twice)
                let cancelled = self.pool.0.min(self.advance.0.max(0) as u32);
                self.advance.0 -= cancelled as i32;
                let gain = self.pool.0 - cancelled;
                self.pool = (0, 0.0);
                if gain > 0 {
                    if !i.mirror {
                        out.events.push(WalletEvent::GainLeft(gain));
                    } else if i.accept_gains {
                        out.munny_delta += gain.min(i32::MAX as u32) as i32;
                        out.events.push(WalletEvent::GainTaken(gain));
                    } else {
                        out.events.push(WalletEvent::GainDropped(gain));
                    }
                }
            }
        }
        // ---- what the counter shows now
        if i.mirror {
            let after = (munny as i64 + out.munny_delta as i64).clamp(0, RUNE_MAX as i64) as u32;
            let want = if i.resting { 0 } else { after };
            out.write_runes = (i.runes != want).then_some(want);
            self.shown = Some(want);
        } else {
            self.shown = Some(i.runes);
        }
        out
    }
}

/// (MUNNYSE, the user 2026-10-27: "make picking up munny from the world give a sound") THE MUNNY ORBS' GET SOUND, PACED.
/// KH3 plays its `m_SoundGet` once per orb (TresPrizeBase, se00000_024 for all three munny sizes, 0.2 s before it
/// arrives); a burst of a dozen orbs reaching him in the same frame would play a dozen at once (a blare). Here every orb
/// asks one; they come out one at a time, at least `gap` seconds apart, and at most `most` wait (more in a burst are
/// dropped: the ones that play already say "munny"). Orbs that come in a trickle play one each, as in KH3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MunnyChime {
    pub gap: f32,
    pub most: u32,
    waiting: u32,
    wait: f32,
    at: glam::Vec3,
}

impl MunnyChime {
    /// The default pacing: 0.06 s apart, 4 waiting at most.
    pub const GAP: f32 = 0.06;
    pub const MOST: u32 = 4;

    pub fn new(gap: f32, most: u32) -> MunnyChime {
        MunnyChime { gap: gap.max(0.0), most: most.max(1), waiting: 0, wait: 0.0, at: glam::Vec3::ZERO }
    }

    /// An orb's get sound asked for at `at` (the newest place is where the next one sounds).
    pub fn ask(&mut self, at: glam::Vec3) {
        self.waiting = (self.waiting + 1).min(self.most);
        self.at = at;
    }

    /// One step of `dt` seconds: the place of the sound to play now, if one is due.
    pub fn step(&mut self, dt: f32) -> Option<glam::Vec3> {
        self.wait = (self.wait - dt).max(0.0);
        if self.waiting == 0 || self.wait > 0.0 {
            return None;
        }
        self.waiting -= 1;
        self.wait = self.gap;
        Some(self.at)
    }

    /// Sounds still to come.
    pub fn waiting(&self) -> u32 {
        self.waiting
    }

    /// Nothing waits (a load, a cutscene).
    pub fn clear(&mut self) {
        self.waiting = 0;
    }
}

#[cfg(test)]
mod munnyse_tests {
    use super::MunnyChime;
    use glam::Vec3;

    /// (MUNNYSE) One orb: its sound at once. A burst of 12 in one step: 4 sounds, 0.06 s apart, the rest dropped. A
    /// trickle (one orb every 0.1 s): one sound each.
    #[test]
    fn munnyse_the_munny_sound_is_paced() {
        let dt = 1.0 / 60.0;
        let mut c = MunnyChime::new(MunnyChime::GAP, MunnyChime::MOST);
        c.ask(Vec3::X);
        assert_eq!(c.step(dt), Some(Vec3::X), "a single orb sounds at once");
        assert_eq!(c.step(dt), None);
        let mut c = MunnyChime::new(MunnyChime::GAP, MunnyChime::MOST);
        for _ in 0..12 {
            c.ask(Vec3::Y);
        }
        let played: Vec<usize> = (0..120).filter(|_| c.step(dt).is_some()).collect();
        assert_eq!(played.len(), 4, "a burst plays MOST sounds, not 12: {played:?}");
        assert!(played.windows(2).all(|w| (w[1] - w[0]) as f32 * dt >= MunnyChime::GAP - 1e-4), "never closer than GAP: {played:?}");
        let mut c = MunnyChime::new(MunnyChime::GAP, MunnyChime::MOST);
        let mut n = 0;
        for k in 0..60 {
            if k % 6 == 0 {
                c.ask(Vec3::Z);
            }
            n += c.step(dt).is_some() as u32;
        }
        assert_eq!(n, 10, "a trickle: one each");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paid(out: &WalletOut) -> Vec<(u32, i32, bool)> {
        out.events.iter().filter_map(|e| if let WalletEvent::KillPaid { key, munny, awarded, .. } = e { Some((*key, *munny, *awarded)) } else { None }).collect()
    }

    /// One frame of a mirrored wallet with the counter reading `runes`; the munny follows what the wallet says
    /// (kills are NOT in its delta: their munny comes with the orbs, which this harness adds when told).
    fn frame(wallet: &mut Wallet, runes: u32, munny: &mut i32, dt: f32) -> (WalletOut, u32) {
        let out = wallet.frame(WalletIn { runes, level: 1, munny: *munny, dead: false, mirror: true, accept_gains: true, resting: false, adopt: false, dt });
        *munny += out.munny_delta;
        let counter = out.write_runes.unwrap_or(runes);
        (out, counter)
    }

    /// (IMPL_ER19, the user's session 2026-10-13) AN ENEMY WHOSE ROW HAS NO RUNE VALUE AND THAT THE GAME PAYS NOTHING
    /// FOR (NpcParam 41000910: nine of them were paid 54 / 488 munny by the max-HP guess, 1354 in all, and the mirror
    /// wrote them into the game's counter): it waits, is closed with NO munny, and neither the munny nor the counter move.
    #[test]
    fn a_runeless_enemy_the_game_pays_nothing_for_gives_no_munny() {
        let mut wallet = Wallet::default();
        let mut munny = 8406;
        let (_, mut counter) = frame(&mut wallet, 8406, &mut munny, 1.0 / 60.0);
        wallet.kill_unpriced(1, 135.0);
        wallet.kill_unpriced(2, 1218.0);
        let mut events = Vec::new();
        for _ in 0..(25 * 60) {
            let (out, c) = frame(&mut wallet, counter, &mut munny, 1.0 / 60.0);
            counter = c;
            events.extend(paid(&out));
        }
        assert_eq!(events, vec![(1, 0, false), (2, 0, false)], "closed without munny after the wait");
        assert_eq!((munny, counter, wallet.pending_kills()), (8406, 8406, 0));
        // (and nothing was advanced: a later gain is a gain in full)
        let (out, c) = frame(&mut wallet, counter + 200, &mut munny, 1.0 / 60.0);
        assert!(out.events.is_empty());
        counter = c;
        let mut gain = 0;
        for _ in 0..60 {
            let (out, c) = frame(&mut wallet, counter, &mut munny, 1.0 / 60.0);
            counter = c;
            for e in &out.events {
                if let WalletEvent::GainTaken(n) = e {
                    gain += *n;
                }
            }
        }
        assert_eq!((gain, munny, counter), (200, 8606, 8606));
    }

    /// (IMPL_ER19, the same session) A BOSS: its row has no rune value and the game awards its runes by itself
    /// seconds later (Tree Sentinel: 3200 after 6.2 s; before, 1156 of them came as orbs of the max-HP guess after 5 s
    /// and the other 2044 without orbs). Now the kill waits and its munny orbs are worth exactly the award; the
    /// counter shows the munny (not the award) until the orbs arrive; nothing is counted twice.
    #[test]
    fn a_bosss_late_award_is_its_munny_in_orbs_once() {
        let mut wallet = Wallet::default();
        let mut munny = 3786;
        let (_, mut counter) = frame(&mut wallet, 3786, &mut munny, 1.0 / 60.0);
        wallet.kill_unpriced(7, 2889.0);
        let mut events = Vec::new();
        for step in 0..(8 * 60) {
            // the game adds 3200 at 6.2 s
            let seen = if step == 372 { counter + 3200 } else { counter };
            let (out, c) = frame(&mut wallet, seen, &mut munny, 1.0 / 60.0);
            counter = c;
            events.extend(paid(&out));
            assert!(!out.events.iter().any(|e| matches!(e, WalletEvent::GainTaken(_))), "not a gain outside a kill");
        }
        assert_eq!(events, vec![(7, 3200, true)]);
        // the award was taken out of the counter again: it arrives with the orbs
        assert_eq!((munny, counter), (3786, 3786));
        munny += 3200; // the orbs arrive
        let (out, counter) = frame(&mut wallet, counter, &mut munny, 1.0 / 60.0);
        assert!(out.events.is_empty());
        assert_eq!((munny, counter), (6986, 6986));
    }

    /// (IMPL_ER19) Priced and unpriced kills waiting together: a small enemy's own award goes to it (the boss keeps
    /// waiting); an award far above everything the priced kills are worth is the boss's; among unpriced kills the
    /// biggest takes it (a boss among rune-less company), the others close without munny.
    #[test]
    fn awards_find_their_kills_among_priced_and_unpriced() {
        let mut wallet = Wallet::default();
        let mut munny = 100;
        let (_, mut counter) = frame(&mut wallet, 100, &mut munny, 1.0 / 60.0);
        wallet.kill_unpriced(1, 135.0); // rune-less company
        wallet.kill_unpriced(2, 2889.0); // the boss
        wallet.kill(3, 11.0, 11); // a soldier
        let (out, c) = frame(&mut wallet, counter + 11, &mut munny, 1.0 / 60.0);
        counter = c;
        assert_eq!(paid(&out), vec![(3, 11, true)]);
        wallet.kill(4, 38.0, 38);
        let (out, c) = frame(&mut wallet, counter + 3200, &mut munny, 1.0 / 60.0);
        counter = c;
        assert_eq!(paid(&out), vec![(2, 3200, true)], "3200 is not the 38-rune soldier's");
        let (out, c) = frame(&mut wallet, counter + 38, &mut munny, 1.0 / 60.0);
        counter = c;
        assert_eq!(paid(&out), vec![(4, 38, true)]);
        let mut events = Vec::new();
        for _ in 0..(21 * 60) {
            let (out, c) = frame(&mut wallet, counter, &mut munny, 1.0 / 60.0);
            counter = c;
            events.extend(paid(&out));
        }
        assert_eq!(events, vec![(1, 0, false)]);
        assert_eq!((munny, counter), (100, 100), "every award was taken out again for its orbs");
        // A priced kill the game pays nothing for in time still gets its row's value (unchanged), and a late award
        // for it is not paid twice.
        wallet.kill(5, 42.0, 42);
        let mut events = Vec::new();
        for _ in 0..(6 * 60) {
            let (out, c) = frame(&mut wallet, counter, &mut munny, 1.0 / 60.0);
            counter = c;
            events.extend(paid(&out));
        }
        assert_eq!(events, vec![(5, 42, false)]);
        munny += 42; // its orb arrives
        let (_, c) = frame(&mut wallet, counter, &mut munny, 1.0 / 60.0);
        counter = c;
        let (_, c) = frame(&mut wallet, counter + 42, &mut munny, 1.0 / 60.0);
        counter = c;
        for _ in 0..60 {
            let (out, c) = frame(&mut wallet, counter, &mut munny, 1.0 / 60.0);
            counter = c;
            assert!(!out.events.iter().any(|e| matches!(e, WalletEvent::GainTaken(_))));
        }
        assert_eq!((munny, counter), (142, 142));
        // The host's waits.
        let mut quick = Wallet::default();
        quick.set_waits(1.0, 2.0);
        let mut m = 0;
        let (_, mut c0) = frame(&mut quick, 0, &mut m, 1.0 / 60.0);
        quick.kill_unpriced(9, 100.0);
        let mut closed_at = None;
        for step in 0..300 {
            let (out, c) = frame(&mut quick, c0, &mut m, 1.0 / 60.0);
            c0 = c;
            if !paid(&out).is_empty() && closed_at.is_none() {
                closed_at = Some(step);
            }
        }
        assert!(matches!(closed_at, Some(118..=122)), "{closed_at:?}");
    }

    /// (IMPL_ER20) The host's HP values go on HP slots alone: 4 per small orb, 20 per large one (the sheet's rows);
    /// KH3's own 1 / 5 leave the slot without amounts (the core's own values); MP, focus and munny slots are not touched.
    #[test]
    fn hp_slots_carry_the_hosts_hit_points() {
        let p = crate::ermap::PrizeRules::DEFAULT;
        assert_eq!((p.hp_small, p.hp_large, p.hp_large / p.hp_small), (4, 20, 5));
        let mut small = Slot { kind: PrizeKind::HpSmall, count: 2, amounts: Vec::new() };
        hp_amounts(&mut small, p.hp_small, p.hp_large);
        assert_eq!(small.amounts, vec![4, 4]);
        let mut large = Slot { kind: PrizeKind::HpLarge, count: 3, amounts: Vec::new() };
        hp_amounts(&mut large, p.hp_small, p.hp_large);
        assert_eq!(large.amounts, vec![20, 20, 20]);
        let mut kh = Slot { kind: PrizeKind::HpLarge, count: 2, amounts: Vec::new() };
        hp_amounts(&mut kh, 1, 5);
        assert!(kh.amounts.is_empty());
        for kind in [PrizeKind::MpSmall, PrizeKind::MpLarge, PrizeKind::FocusSmall, PrizeKind::FocusLarge] {
            let mut other = Slot { kind, count: 2, amounts: Vec::new() };
            hp_amounts(&mut other, 4, 20);
            assert!(other.amounts.is_empty());
        }
        let mut munny = Slot { kind: PrizeKind::Munny5, count: 2, amounts: vec![100, 100] };
        hp_amounts(&mut munny, 4, 20);
        assert_eq!(munny.amounts, vec![100, 100]);
        // What the log says when orbs are taken back.
        assert!(crate::prize::settle_line("Sora's death", 3, 42, true, 142).contains("the 42 munny they held are Sora's at once") && crate::prize::settle_line("Sora stops", 2, 42, false, 100).contains("NOT given") && !crate::prize::settle_line("a cutscene starts", 2, 0, true, 0).contains("munny"));
    }

    /// The rule: tiers by maximum HP, the first slot's family by the KH3 shares, munny from the
    /// rune value (split evenly over the munny orbs) or from the maximum HP.
    #[test]
    fn a_kill_drops_one_stat_slot_and_the_enemys_runes_as_munny() {
        let rule = Rule::default();
        assert_eq!((rule.tier(55), rule.tier(299), rule.tier(300), rule.tier(1499), rule.tier(2889)), (0, 0, 1, 1, 2));
        let mut rng = Rng::new(1);
        // A weak enemy worth 77 runes: 2 small orbs of one family, 1 small munny orb worth 77.
        let d = rule.drop_for(192, Some(77), &mut rng);
        assert_eq!((d.tier, d.munny, d.from_runes, d.slots.len()), (0, 77, true, 2));
        assert!(matches!(d.slots[0].kind, PrizeKind::HpSmall | PrizeKind::MpSmall | PrizeKind::FocusSmall) && d.slots[0].count == 2);
        assert_eq!(d.slots[1], Slot { kind: PrizeKind::Munny1, count: 1, amounts: vec![77] });
        // A strong one worth 3201: large orbs; two large munny orbs worth 1601 + 1600.
        let d = rule.drop_for(2889, Some(3201), &mut rng);
        assert!(matches!(d.slots[0].kind, PrizeKind::HpLarge | PrizeKind::MpLarge | PrizeKind::FocusLarge) && d.slots[0].count == 2);
        assert_eq!(d.slots[1], Slot { kind: PrizeKind::Munny10, count: 2, amounts: vec![1601, 1600] });
        assert_eq!(d.slots[1].amounts.iter().sum::<i32>(), 3201);
        // No rune value: ceil(max HP x 0.4), like the EXP rule.
        let d = rule.drop_for(500, None, &mut rng);
        assert_eq!((d.tier, d.munny, d.from_runes), (1, 200, false));
        assert_eq!(d.slots[1], Slot { kind: PrizeKind::Munny5, count: 2, amounts: vec![100, 100] });
        // An enemy worth nothing drops no munny orb; one worth 1 drops one orb, not two.
        assert_eq!(rule.with_overrides(None, None, Some(0.0)).drop_for(500, None, &mut rng).slots.len(), 1);
        assert_eq!(rule.drop_for(500, Some(1), &mut rng).slots[1].amounts, vec![1]);
        // The first slot's family follows the weights (67 : 36 : 10 of KH3's enemy rows).
        let mut seen = [0u32; 3];
        for _ in 0..11_300 {
            match rule.drop_for(100, None, &mut rng).slots[0].kind {
                PrizeKind::HpSmall => seen[0] += 1,
                PrizeKind::MpSmall => seen[1] += 1,
                PrizeKind::FocusSmall => seen[2] += 1,
                other => panic!("{other:?}"),
            }
        }
        assert!((6300..7100).contains(&seen[0]) && (3250..3950).contains(&seen[1]) && (800..1200).contains(&seen[2]), "{seen:?}");
        // The control keys.
        let only_hp = rule.with_overrides(Some("1, 0, 0"), Some(0.5), None);
        let d = only_hp.drop_for(100, Some(100), &mut rng);
        assert_eq!((d.slots[0].kind, d.munny), (PrizeKind::HpSmall, 50));
        assert_eq!(rule.with_overrides(Some("nonsense"), Some(-1.0), None), rule);
    }

    fn wi(runes: u32, munny: i32) -> WalletIn {
        WalletIn { runes, level: 10, munny, dead: false, mirror: true, accept_gains: true, resting: false, adopt: false, dt: 1.0 / 60.0 }
    }

    /// The wallet with the mirror: the session starts with what the game's save held; a kill's munny is
    /// what the game itself awards; purchases cost munny; gains without a kill become munny; resting
    /// shows 0; a level-up is not charged; a death drops nothing.
    #[test]
    fn the_rune_counter_shows_munny_and_a_kill_is_worth_what_the_game_awards() {
        let mut w = Wallet::default();
        let mut munny = 500;
        // The session's first frame: the Tarnished's save holds 1234 runes: that is Sora's munny.
        let o = w.frame(WalletIn { adopt: true, ..wi(1234, munny) });
        assert_eq!((o.write_runes, o.munny_delta, o.events.clone()), (None, 734, vec![WalletEvent::Adopted { runes: 1234, munny_before: 500 }]));
        munny += o.munny_delta;
        assert_eq!(w.frame(wi(1234, munny)), WalletOut::default());
        // A kill worth 60 by its row; three frames later the game adds 77 (its rates): the orbs are worth 77,
        // and the counter goes back to the munny (the orbs bring it).
        w.kill(7, 60.0, 60);
        assert_eq!(w.frame(wi(1234, munny)), WalletOut::default());
        let o = w.frame(wi(1311, munny));
        assert_eq!(o.events, vec![WalletEvent::KillPaid { key: 7, munny: 77, awarded: true, fallback: 60 }]);
        assert_eq!((o.write_runes, o.munny_delta), (Some(1234), 0));
        // The orb arrives: the counter follows.
        munny += 77;
        assert_eq!(w.frame(wi(1234, munny)).write_runes, Some(1311));
        // The award BEFORE the kill is known to us (the same frame order the other way round).
        let o = w.frame(wi(1311 + 40, munny));
        assert!(o.events.is_empty() && o.write_runes == Some(1311));
        w.kill(8, 30.0, 30);
        w.kill(9, 10.0, 10);
        let o = w.frame(wi(1311, munny));
        assert_eq!(o.events, vec![WalletEvent::KillPaid { key: 8, munny: 30, awarded: true, fallback: 30 }, WalletEvent::KillPaid { key: 9, munny: 10, awarded: true, fallback: 10 }]);
        munny += 40;
        w.frame(wi(1311, munny));
        // A vendor takes 300.
        let o = w.frame(wi(1051, munny));
        assert_eq!((o.write_runes, o.munny_delta, o.events.clone()), (None, -300, vec![WalletEvent::Spent(300)]));
        munny += o.munny_delta;
        assert_eq!(w.frame(wi(1051, munny)), WalletOut::default());
        // A rune item used (no kill near): after the wait it is munny.
        let mut o = w.frame(wi(1251, munny));
        assert_eq!((o.write_runes, o.munny_delta), (Some(1051), 0));
        for _ in 0..40 {
            o = w.frame(wi(1051, munny));
            if o.munny_delta != 0 {
                break;
            }
        }
        assert_eq!((o.munny_delta, o.write_runes, o.events.clone()), (200, Some(1251), vec![WalletEvent::GainTaken(200)]));
        munny += 200;
        // A kill the game awards nothing for: its fallback after the wait.
        w.kill(10, 5.0, 5);
        let mut paid = Vec::new();
        for _ in 0..330 {
            paid.extend(w.frame(wi(1251, munny)).events);
        }
        assert_eq!(paid, vec![WalletEvent::KillPaid { key: 10, munny: 5, awarded: false, fallback: 5 }]);
        // ... and the game's late award for it (a boss) is not paid a second time: only what exceeds it.
        w.frame(wi(1251 + 12, munny));
        let mut late = Vec::new();
        for _ in 0..40 {
            let o = w.frame(wi(1251, munny));
            late.extend(o.events);
            if o.munny_delta != 0 {
                assert_eq!((o.munny_delta, o.write_runes), (7, Some(1258)));
                break;
            }
        }
        assert_eq!(late, vec![WalletEvent::GainTaken(7)]);
        munny += 7;
        assert_eq!(w.frame(wi(1258, munny)), WalletOut::default());
        // Resting at a grace: the counter shows 0 (Level Up can raise nothing), and the munny again afterwards.
        assert_eq!(w.frame(WalletIn { resting: true, ..wi(1258, munny) }).write_runes, Some(0));
        assert_eq!(w.frame(WalletIn { resting: true, ..wi(0, munny) }), WalletOut::default());
        assert_eq!(w.frame(wi(0, munny)).write_runes, Some(1258));
        // A level-up that happens all the same: not charged, reported, the counter is the munny again.
        let o = w.frame(WalletIn { level: 11, ..wi(1000, munny) });
        assert_eq!((o.munny_delta, o.write_runes, o.events.clone()), (0, Some(1258), vec![WalletEvent::LevelUp { from: 10, to: 11, runes: 258 }]));
        // Death: the counter is emptied at once (nothing to drop), the munny is untouched, and it is
        // the counter again after the respawn.
        let o = w.frame(WalletIn { dead: true, ..wi(1258, munny) });
        assert_eq!((o.write_runes, o.munny_delta, o.events.clone()), (Some(0), 0, vec![WalletEvent::DeathHeld(1258)]));
        assert_eq!(w.frame(WalletIn { dead: true, ..wi(0, munny) }), WalletOut::default());
        let o = w.frame(wi(0, munny));
        assert_eq!((o.write_runes, o.munny_delta, o.events.clone()), (Some(1258), 0, vec![WalletEvent::Synced { munny: 1258, held: 0 }]));
        assert_eq!(munny, 1258);
        // A payment larger than the munny (cannot happen while the counter is the munny) never goes below 0.
        let mut w = Wallet::default();
        w.frame(wi(50, 50));
        assert_eq!(w.frame(wi(0, 20)).munny_delta, -20);
    }

    /// Without the mirror nothing is written: the game keeps its runes, and a kill's orbs are still
    /// worth what it awarded. With the mirror but `munny.mirror.gains = 0` other gains are taken away.
    #[test]
    fn without_the_mirror_the_counter_is_only_read() {
        let off = |runes: u32, munny: i32| WalletIn { mirror: false, ..wi(runes, munny) };
        let mut w = Wallet::default();
        assert_eq!(w.frame(off(100, 0)), WalletOut::default());
        w.kill(1, 4.0, 4);
        let o = w.frame(off(104, 0));
        assert_eq!((o.write_runes, o.munny_delta, o.events.clone()), (None, 0, vec![WalletEvent::KillPaid { key: 1, munny: 4, awarded: true, fallback: 4 }]));
        assert_eq!(w.frame(off(54, 4)), WalletOut::default());
        // Switching the mirror on mid-session: a sync.
        let o = w.frame(wi(54, 4));
        assert_eq!((o.write_runes, o.events.clone()), (Some(4), vec![WalletEvent::Synced { munny: 4, held: 54 }]));
        let mut events = Vec::new();
        w.frame(WalletIn { accept_gains: false, ..wi(304, 4) });
        for _ in 0..40 {
            let o = w.frame(WalletIn { accept_gains: false, ..wi(4, 4) });
            assert_eq!(o.munny_delta, 0);
            events.extend(o.events);
        }
        assert_eq!(events, vec![WalletEvent::GainDropped(300)]);
    }
}
