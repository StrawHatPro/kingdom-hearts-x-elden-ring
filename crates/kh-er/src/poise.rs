//! (IMPL_ER45) ELDEN RING'S POISE DECIDES HOW HARD AN ORDINARY ENEMY IS TO STAGGER. The user (2026-10-13): "you need
//! to take poise into account when judging how easy it is to stunlock them. I'm stunlocking enemies in full plate
//! armor and it is wrong". Before this pass every body we throw about (kh-react Light / Medium: everything below
//! `target.poise_armoured` 120 that is not Large) reacted to every hit of Sora's: a knight of poise 65 flinched and
//! was juggled exactly like a soldier of poise 30.
//!
//! WHOSE METER: OURS, WITH THE GAME'S NUMBERS. The game's own meter (CSChrSuperArmorModule.sa_durability) is readable
//! and IS depleted by our bullets (ER5: atkSuperArmor 30 -> 20 -> 18; a soldier knelt at 0), but since ER5 our bullets
//! carry no poise damage to bodies we throw about, on purpose: the game answers its own poise break with the stance
//! break kneel (animation 8700, seconds long), which would play over KH3's launch / juggle animations; and KH3's
//! reaction must be decided when the hit is delivered, a frame before the game applies the bullet. So the meter is
//! kept here: its maximum IS the game's (the larger of sa_durability_max and toughness_max, read every frame; it is
//! NpcParam.superArmorDurability), its refill time is the game's (NpcParam.superArmorRecoverCorrection, seconds,
//! when it holds a usable number; else the sheet's `poise.recover`), hyper armour is the game's (the TAE action flag
//! SUPER_ARMOR of CSChrActionFlagModule, read every frame; else "it plays an attack animation"). Armoured bodies
//! (STRONG: Heavy / Giant, bosses) are not touched: bossguard.rs and the game's own meter govern them as before.
//!
//! THE RULE (every number a row `poise.*` of sheets/port/sora-er-host.json; control key `poise.kh = 0` = before):
//! - CLASS by the poise maximum: LOW up to `poise.class.low_max` (40: soldiers 30, dogs, rats): no meter, every hit
//!   reacts as before (KH3's juggles stay); MID up to `poise.class.mid_max` (70: c3020 50, c3010 / c4090 65); HIGH above.
//! - Every hit of Sora's on a MID / HIGH body deals its kind's poise number (the `react.kind.*` rows: basic hits 10,
//!   finishers / launchers / blows 60) x `damage.out.poise_scale` x `poise.factor`, x `poise.hyper_factor` while the
//!   body is in its own hyper armour.
//! - UNBROKEN (the meter above 0): HIGH bodies take no reaction at all; MID bodies take none from light hits and a
//!   flinch on the spot (no launch, no blow) from the big kinds (KH3's own STRONG-body rules, StrongBodyDamage1 / 2);
//!   in hyper armour, or while a reaction of ours still runs (in the air, lying), nothing.
//! - THE BREAK (the meter reaches 0): the hit's KH3 reaction in full (stagger / launch / blow); for `poise.window`
//!   seconds every hit reacts in full (the juggle); then THE BODY RECOVERS: its meter is full again and for
//!   `poise.lockout` seconds no hit deals poise damage or starts a reaction (it gets up and fights back); after that it
//!   must be broken again. Without poise damage for the refill time the meter is full again as well. Breaks are also
//!   counted by bossguard's chain cap (`react.chain.*`: at most 3 in 10 s, then none for 8 s; `react.guard = 0`
//!   lifts it).

use crate::bossguard::{Guard, GuardRules, Verdict};

/// The rows `poise.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoiseRules {
    /// `poise.kh`: the rules are on (the control key `poise.kh = 0` switches them off: every hit reacts, as before).
    pub on: bool,
    /// `poise.factor`: multiplies the poise number of every hit (after `damage.out.poise_scale`).
    pub factor: f32,
    /// `poise.class.low_max` / `.mid_max`: the class bounds by the body's poise maximum.
    pub low_max: f32,
    pub mid_max: f32,
    /// `poise.hyper_factor`: the share of its poise damage a hit deals to a body in its own hyper armour.
    pub hyper_factor: f32,
    /// `poise.window`: seconds after a break in which every hit reacts in full (the juggle).
    pub window: f32,
    /// `poise.lockout`: seconds after the window in which the body recovers: no poise damage, no reaction.
    pub lockout: f32,
    /// `poise.recover`: seconds without poise damage after which the meter is full again, when the game's own number
    /// (NpcParam.superArmorRecoverCorrection) is not usable.
    pub recover: f32,
    /// `poise.hyper_by_anim`: when the hyper-armour flag cannot be read, an attack animation (x3000..x3999) counts.
    pub hyper_by_anim: bool,
}

impl PoiseRules {
    /// The sheet's rows.
    pub const DEFAULT: PoiseRules = PoiseRules { on: true, factor: 1.0, low_max: 40.0, mid_max: 70.0, hyper_factor: 0.5, window: 3.0, lockout: 4.0, recover: 5.0, hyper_by_anim: true };
    /// `poise.kh = 0`: as before this pass.
    pub const OFF: PoiseRules = PoiseRules { on: false, ..PoiseRules::DEFAULT };

    /// One row of the sheet; false: not understood.
    pub fn apply_row(&mut self, key: &str, v: f32) -> bool {
        match key {
            "poise.kh" => self.on = v != 0.0,
            "poise.factor" if (0.0..=10.0).contains(&v) => self.factor = v,
            "poise.class.low_max" if v >= 0.0 => self.low_max = v,
            "poise.class.mid_max" if v >= 0.0 => self.mid_max = v,
            "poise.hyper_factor" if (0.0..=1.0).contains(&v) => self.hyper_factor = v,
            "poise.window" if (0.0..=30.0).contains(&v) => self.window = v,
            "poise.lockout" if (0.0..=60.0).contains(&v) => self.lockout = v,
            "poise.recover" if (0.1..=120.0).contains(&v) => self.recover = v,
            "poise.hyper_by_anim" => self.hyper_by_anim = v != 0.0,
            _ => return false,
        }
        true
    }

    /// The refill time of a body: the game's own number when it is usable, else the sheet's.
    pub fn recover_of(&self, game_seconds: Option<f32>) -> (f32, bool) {
        match game_seconds {
            Some(s) if s > 0.0 && s <= 60.0 => (s, true),
            _ => (self.recover, false),
        }
    }
}

/// The poise class of a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoiseClass {
    /// Soldiers, dogs, rats: every hit reacts (no meter).
    Low,
    /// Light hits do nothing, big ones flinch, until the meter breaks.
    Mid,
    /// Nothing reacts until the meter breaks.
    High,
}

impl PoiseClass {
    pub fn of(rules: &PoiseRules, poise_max: f32) -> PoiseClass {
        if !(poise_max > rules.low_max) {
            PoiseClass::Low
        } else if poise_max <= rules.mid_max {
            PoiseClass::Mid
        } else {
            PoiseClass::High
        }
    }
}

/// What a hit may start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// KH3's reaction as the hit asks.
    Full,
    /// A flinch on the spot (the kind becomes SMALL).
    Flinch,
    /// No reaction (damage only).
    None,
}

/// The meter's answer to one hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outcome {
    pub gate: Gate,
    pub class: PoiseClass,
    /// Poise damage dealt to the meter, and the meter before / after.
    pub damage: f32,
    pub before: f32,
    pub after: f32,
    /// This hit broke the meter.
    pub broke: bool,
    /// The meter reached 0 but the chain cap refused the break (the meter is full again).
    pub chain_refused: bool,
    /// The body is in its break window (every hit reacts).
    pub in_window: bool,
    /// The body recovers after its window (no poise damage, no reaction).
    pub recovering: bool,
}

/// What a step of the meter's clock did (for the log).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tick {
    Nothing,
    /// The break window closed: the meter is full again.
    WindowClosed,
    /// No poise damage for the refill time: the meter is full again.
    Recovered,
}

/// ONE BODY'S POISE METER.
#[derive(Clone, Copy, Debug)]
pub struct Meter {
    value: f32,
    max: f32,
    clock: f32,
    last_hit: f32,
    broken_until: f32,
    lock_until: f32,
    lockout: f32,
    recover: f32,
    /// bossguard's chain cap, counting breaks.
    chain: Guard,
    pub breaks: u32,
    pub refused: u32,
}

impl Default for Meter {
    fn default() -> Meter {
        Meter::new()
    }
}

impl Meter {
    pub fn new() -> Meter {
        Meter { value: -1.0, max: 0.0, clock: 1.0, last_hit: -1000.0, broken_until: 0.0, lock_until: 0.0, lockout: PoiseRules::DEFAULT.lockout, recover: PoiseRules::DEFAULT.recover, chain: Guard::new(), breaks: 0, refused: 0 }
    }

    /// The meter now (-1 before its first hit) and its maximum.
    pub fn value(&self) -> (f32, f32) {
        (self.value, self.max)
    }

    /// In the window after a break.
    pub fn broken(&self) -> bool {
        self.clock < self.broken_until
    }

    /// Seconds of the break window / of the recovery after it left.
    pub fn window_left(&self) -> f32 {
        (self.broken_until - self.clock).max(0.0)
    }
    pub fn recovery_left(&self) -> f32 {
        (self.lock_until - self.clock).max(0.0)
    }

    pub fn tick(&mut self, dt: f32) -> Tick {
        self.clock += dt;
        self.chain.tick(dt);
        if self.broken_until > 0.0 && !self.broken() {
            self.broken_until = 0.0;
            self.value = self.max;
            self.lock_until = self.clock + self.lockout;
            return Tick::WindowClosed;
        }
        if self.broken_until == 0.0 && self.value >= 0.0 && self.value < self.max && self.clock - self.last_hit >= self.recover {
            self.value = self.max;
            return Tick::Recovered;
        }
        Tick::Nothing
    }

    /// ONE HIT. `poise_max` = the game's maximum of the body; `recover` = its refill time; `row_poise` = the poise
    /// number of the hit's kind (scaled by `damage.out.poise_scale`); `big` = a kind that throws (KH3's "big" kinds);
    /// `hyper` = the body is in its own hyper armour; `in_reaction` = a reaction of ours still runs on it.
    #[allow(clippy::too_many_arguments)]
    pub fn hit(&mut self, rules: &PoiseRules, guard: &GuardRules, poise_max: f32, recover: f32, row_poise: f32, big: bool, hyper: bool, in_reaction: bool) -> Outcome {
        let class = PoiseClass::of(rules, poise_max);
        // the maximum follows the game's (a meter not hit yet, or full, starts full)
        if (poise_max - self.max).abs() > 0.01 {
            self.value = if self.value < 0.0 || self.value >= self.max { poise_max } else { self.value.min(poise_max) };
            self.max = poise_max;
        }
        if self.value < 0.0 {
            self.value = self.max;
        }
        self.recover = recover.max(0.1);
        self.lockout = rules.lockout;
        let mut out = Outcome { gate: Gate::Full, class, damage: 0.0, before: self.value, after: self.value, broke: false, chain_refused: false, in_window: false, recovering: false };
        if !rules.on || class == PoiseClass::Low {
            return out;
        }
        if self.broken() {
            out.in_window = true;
            return out;
        }
        if self.clock < self.lock_until {
            out.recovering = true;
            out.gate = Gate::None;
            return out;
        }
        let damage = (row_poise * rules.factor * if hyper { rules.hyper_factor } else { 1.0 }).max(0.0);
        self.value -= damage;
        self.last_hit = self.clock;
        out.damage = damage;
        if self.value <= 0.0 && damage > 0.0 {
            if self.chain.reaction(guard) != Verdict::Allow {
                self.refused += 1;
                self.value = self.max;
                out.after = self.value;
                out.gate = Gate::None;
                out.chain_refused = true;
                return out;
            }
            self.breaks += 1;
            self.value = 0.0;
            self.broken_until = self.clock + rules.window;
            out.after = 0.0;
            out.broke = true;
            out.in_window = rules.window > 0.0;
            return out;
        }
        out.after = self.value;
        out.gate = match class {
            _ if hyper || in_reaction => Gate::None,
            PoiseClass::Mid if big => Gate::Flinch,
            _ => Gate::None,
        };
        out
    }
}

/// KH3's "big" damage kinds (they throw a body; a STRONG body turns them into a flinch): mask 0x20000F9C of the
/// enemy's reaction gate (kh-react `EnemyReaction::hit`).
pub fn big_kind(kind: u8) -> bool {
    matches!(kind, 2 | 3 | 4 | 7 | 8 | 9 | 10 | 11 | 29)
}

/// An attack animation of an Elden Ring character's set (x3000..x3999).
pub fn attack_anim(anim: i32) -> bool {
    (3000..4000).contains(&anim.rem_euclid(1_000_000))
}

// ---------------------------------------------------------------------------------------------------------------
// The game half: the numbers of a kind of character (NpcParam), read once per NpcParam row and logged.
// ---------------------------------------------------------------------------------------------------------------

/// What the game says about the poise of one NpcParam row.
#[derive(Clone, Copy, Debug, Default)]
pub struct TypeInfo {
    /// The refill time used (seconds).
    pub recover: f32,
    pub hyper_seen: bool,
}

/// One line per kind of character at first sight (`poise:`), and the refill time per kind.
#[derive(Default)]
pub struct Book {
    types: std::collections::HashMap<i32, TypeInfo>,
}

#[cfg(windows)]
impl Book {
    /// The refill time of this character's kind (read and logged at its first sight). `poise_max` / `poise`: what the
    /// game's modules hold now; `toughness`: (now, max) of its toughness module when readable.
    pub fn see(&mut self, chr: &eldenring::cs::ChrIns, poise_max: f32, sa: Option<(f32, f32, f32)>, toughness: Option<(f32, f32)>, rules: &PoiseRules) -> f32 {
        let key = chr.npc_param_id;
        if let Some(t) = self.types.get(&key) {
            return t.recover;
        }
        let row = npc_row(key);
        let (recover, from_game) = rules.recover_of(row.map(|r| r.1));
        let name = row.and_then(|r| npc_name(r.4)).map_or(String::new(), |n| format!(" \"{n}\""));
        let class = PoiseClass::of(rules, poise_max);
        let light = PoiseRules::DEFAULT.factor * 10.0;
        let rule = match class {
            PoiseClass::Low => "every hit reacts (no meter: KH3's juggles as before)".to_string(),
            PoiseClass::Mid => format!("unbroken: no reaction from light hits, a flinch on the spot from finishers / launchers; broken by {} light hit(s) of {light:.0} or a finisher (60) and {} light", (poise_max / light).ceil(), ((poise_max - 60.0) / light).ceil().max(0.0)),
            PoiseClass::High => format!("unbroken: NO reaction at all; broken by {} light hit(s) of {light:.0} or a finisher (60) and {} light", (poise_max / light).ceil(), ((poise_max - 60.0) / light).ceil().max(0.0)),
        };
        crate::erlog!(
            "poise: c{:04} NpcParam {key}{name}: poise max {poise_max:.0} (the game's super armour {}, toughness {}; NpcParam superArmorDurability {}, superArmorRecoverCorrection {}, saRecoveryRate {}, toughness {}, toughnessRecoverCorrection {}); class {class:?} (low <= {:.0} < mid <= {:.0} < high): {rule}; the meter refills after {recover:.1} s without poise damage ({}); break window {:.1} s; hyper armour x{:.2}{}",
            chr.npc_id,
            sa.map_or("not readable".to_string(), |(now, max, t)| format!("{now:.0}/{max:.0}, recover_time {t:.2}")),
            toughness.map_or("not readable".to_string(), |(now, max)| format!("{now:.0}/{max:.0}")),
            row.map_or("-".to_string(), |r| format!("{:.0}", r.0)),
            row.map_or("-".to_string(), |r| format!("{:.2}", r.1)),
            row.map_or("-".to_string(), |r| format!("{:.2}", r.2)),
            row.map_or("-".to_string(), |r| format!("{}", r.3)),
            row.map_or("-".to_string(), |r| format!("{:.2}", r.5)),
            rules.low_max, rules.mid_max,
            if from_game { "the game's NpcParam number" } else { "NOT the game's: sheet row poise.recover" },
            rules.window, rules.hyper_factor,
            if rules.on { "" } else { " (poise.kh = 0: NOT APPLIED)" },
        );
        self.types.insert(key, TypeInfo { recover, hyper_seen: false });
        recover
    }

    /// The hyper-armour flag was seen set on a character of this kind: logged once per kind (evidence that the flag
    /// is read where the SDK says).
    pub fn hyper_seen(&mut self, chr: &eldenring::cs::ChrIns, anim: i32, multiplier: f32) {
        if let Some(t) = self.types.get_mut(&chr.npc_param_id) {
            if !t.hyper_seen {
                t.hyper_seen = true;
                crate::erlog!("poise: c{:04} NpcParam {}: the game's HYPER ARMOUR flag (TAE ChrActionFlag 24 SUPER_ARMOR) seen set, animation {anim}{}, SaDurabilityMultiplier {multiplier:.2}: light hits do not interrupt it unless its poise breaks", chr.npc_id, chr.npc_param_id, if attack_anim(anim) { " (an attack)" } else { " (NOT an attack animation)" });
            }
        }
    }
}

/// NpcParam: (superArmorDurability, superArmorRecoverCorrection, saRecoveryRate, toughness, nameId,
/// toughnessRecoverCorrection). `None`: the repository or the row is not there.
#[cfg(windows)]
fn npc_row(id: i32) -> Option<(f32, f32, f32, u32, i32, f32)> {
    use fromsoftware_shared::FromStatic;
    if id < 0 {
        return None;
    }
    // SAFETY: game thread; the singleton lives as long as the game; the SDK checks its holder table.
    let repo = unsafe { eldenring::cs::SoloParamRepository::instance() }.ok()?;
    let r = repo.get::<eldenring::cs::NpcParam>(id as u32)?;
    Some((r.super_armor_durability(), r.super_armor_recover_correction(), r.sa_recovery_rate(), r.toughness(), r.name_id(), r.toughness_recover_correction()))
}

/// The name of an NpcName id (message file 18 by the public documentation; NOT VERIFIED IN THE GAME).
#[cfg(windows)]
fn npc_name(name_id: i32) -> Option<String> {
    use fromsoftware_shared::FromStatic;
    if name_id <= 0 {
        return None;
    }
    // SAFETY: game thread; the singleton lives as long as the game; the SDK checks the file table's bounds.
    let msgs = unsafe { eldenring::cs::MsgRepository::instance() }.ok()?;
    let text = msgs.get_msg(18, name_id as u32)?;
    let end = text.iter().position(|c| *c == 0).unwrap_or(text.len()).min(80);
    let name = String::from_utf16_lossy(&text[..end]).trim().to_string();
    (!name.is_empty() && !name.contains("[ERROR]")).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: PoiseRules = PoiseRules::DEFAULT;
    const G: GuardRules = GuardRules::DEFAULT;
    /// The poise numbers of the react.kind rows: basic hits (Small) 10, finishers / launchers 60.
    const LIGHT: f32 = 10.0;
    const FINISHER: f32 = 60.0;

    fn run(m: &mut Meter, seconds: f32) -> Vec<Tick> {
        (0..(seconds * 60.0).round() as u32).map(|_| m.tick(1.0 / 60.0)).filter(|t| *t != Tick::Nothing).collect()
    }

    /// A light hit on a standing body, out of hyper armour.
    fn light(m: &mut Meter, poise_max: f32) -> Outcome {
        m.hit(&R, &G, poise_max, 5.0, LIGHT, false, false, false)
    }

    #[test]
    fn a_plate_knight_takes_seven_light_hits_before_he_staggers_a_soldier_at_once() {
        // c3010 (poise 65, the user's log): six light hits do nothing, the seventh breaks him
        let mut knight = Meter::new();
        for n in 1..=6 {
            let o = light(&mut knight, 65.0);
            assert_eq!((o.gate, o.class, o.broke), (Gate::None, PoiseClass::Mid, false), "hit {n}");
            run(&mut knight, 0.3);
        }
        let o = light(&mut knight, 65.0);
        assert_eq!((o.gate, o.broke, o.after), (Gate::Full, true, 0.0));
        // the basic ground combo (3 light + the finisher) breaks him on the finisher
        let mut knight = Meter::new();
        for _ in 0..3 {
            assert_eq!(light(&mut knight, 65.0).gate, Gate::None);
        }
        assert!(knight.hit(&R, &G, 65.0, 5.0, FINISHER, true, false, false).broke);
        // a finisher on a fresh MID knight only flinches him; on a HIGH body (80) it does nothing
        let mut knight = Meter::new();
        assert_eq!(knight.hit(&R, &G, 65.0, 5.0, FINISHER, true, false, false).gate, Gate::Flinch);
        let mut plate = Meter::new();
        let o = plate.hit(&R, &G, 80.0, 5.0, FINISHER, true, false, false);
        assert_eq!((o.gate, o.class), (Gate::None, PoiseClass::High));
        // c2500 (poise 80): eight light hits
        let mut plate = Meter::new();
        assert_eq!((1..=8).position(|_| light(&mut plate, 80.0).broke), Some(7));
        // c3000 soldier (poise 30): every hit reacts, no meter at all
        let mut soldier = Meter::new();
        for _ in 0..20 {
            let o = light(&mut soldier, 30.0);
            assert_eq!((o.gate, o.class, o.damage), (Gate::Full, PoiseClass::Low, 0.0));
        }
        assert_eq!(soldier.hit(&R, &G, 30.0, 5.0, FINISHER, true, false, true).gate, Gate::Full);
        // switched off: the knight reacts to everything, as before
        let mut knight = Meter::new();
        assert_eq!(knight.hit(&PoiseRules::OFF, &G, 65.0, 5.0, LIGHT, false, false, false).gate, Gate::Full);
    }

    #[test]
    fn hyper_armour_halves_the_poise_damage_and_nothing_interrupts_it() {
        let mut knight = Meter::new();
        // 65 / 5 = 13 light hits while he attacks; even a finisher does not flinch him
        assert_eq!(knight.hit(&R, &G, 65.0, 5.0, FINISHER, true, true, false).gate, Gate::None);
        assert_eq!(knight.value().0, 35.0);
        for _ in 0..6 {
            assert_eq!(knight.hit(&R, &G, 65.0, 5.0, LIGHT, false, true, false).gate, Gate::None);
        }
        let o = knight.hit(&R, &G, 65.0, 5.0, LIGHT, false, true, false);
        assert!(o.broke && o.gate == Gate::Full, "the break interrupts the attack: {o:?}");
        let mut fresh = Meter::new();
        assert_eq!((1..=13).position(|_| fresh.hit(&R, &G, 65.0, 5.0, LIGHT, false, true, false).broke), Some(12));
    }

    #[test]
    fn the_break_window_then_the_meter_is_full_and_must_be_broken_again() {
        let mut knight = Meter::new();
        for _ in 0..7 {
            light(&mut knight, 65.0);
        }
        assert!(knight.broken());
        // the juggle: every hit reacts in full for the window, also in the air
        run(&mut knight, 1.0);
        let o = knight.hit(&R, &G, 65.0, 5.0, LIGHT, false, false, true);
        assert_eq!((o.gate, o.in_window, o.damage), (Gate::Full, true, 0.0));
        assert_eq!(run(&mut knight, 2.1), vec![Tick::WindowClosed]);
        assert_eq!(knight.value(), (65.0, 65.0));
        // he recovers: for 4 s nothing reacts and nothing takes poise (a body still in the air is not juggled on)
        let o = knight.hit(&R, &G, 65.0, 5.0, FINISHER, true, false, true);
        assert_eq!((o.gate, o.recovering, o.damage), (Gate::None, true, 0.0));
        assert_eq!(knight.hit(&R, &G, 65.0, 5.0, FINISHER, true, false, false).gate, Gate::None);
        run(&mut knight, 4.0);
        // then he must be broken again: a finisher flinches him on the spot (65 -> 5), a light hit after it breaks him
        assert_eq!(knight.hit(&R, &G, 65.0, 5.0, FINISHER, true, false, false).gate, Gate::Flinch);
        assert!(light(&mut knight, 65.0).broke);
        // THE GAME'S REFILL TIME: 3 light hits, then 5 s without poise damage: full again
        let mut knight = Meter::new();
        for _ in 0..3 {
            light(&mut knight, 65.0);
        }
        assert_eq!(knight.value().0, 35.0);
        assert_eq!(run(&mut knight, 4.9), vec![]);
        assert_eq!(run(&mut knight, 0.2), vec![Tick::Recovered]);
        assert_eq!(knight.value().0, 65.0);
        // the refill time comes from the game when its number is usable
        assert_eq!(R.recover_of(Some(3.0)), (3.0, true));
        assert_eq!(R.recover_of(Some(-1.0)), (5.0, false));
        assert_eq!(R.recover_of(None), (5.0, false));
    }

    /// A player who never stops hitting (a light hit every 0.25 s, a finisher every fourth): the knight reacts only
    /// in his break windows, and the chain cap keeps them apart.
    #[test]
    fn no_infinite_stunlock_on_a_knight() {
        let mut knight = Meter::new();
        let (mut full, mut hits, mut breaks) = (0, 0, 0);
        let mut longest_calm = 0.0f32;
        let mut calm = 0.0f32;
        for n in 0..240 {
            let finisher = n % 4 == 3;
            let o = knight.hit(&R, &G, 65.0, 5.0, if finisher { FINISHER } else { LIGHT }, finisher, false, false);
            hits += 1;
            if o.gate != Gate::None {
                full += 1;
                calm = 0.0;
            } else {
                calm += 0.25;
                longest_calm = longest_calm.max(calm);
            }
            breaks += o.broke as u32;
            run(&mut knight, 0.25);
        }
        // 60 s of hitting: a break about every 7.5 s (3 s window + 4 s recovery + the combo that breaks him again);
        // the knight stands unflinching through more than half of it, at least 4 s at a time
        assert!((6..=10).contains(&breaks), "breaks {breaks}");
        assert!(full * 2 < hits, "reactions {full} of {hits} hits");
        assert!(longest_calm >= 3.5, "a stretch of {longest_calm} s without any reaction");
        // a soldier under the same hail reacts every time (KH3's juggles stay)
        let mut soldier = Meter::new();
        assert!((0..240).all(|n| soldier.hit(&R, &G, 30.0, 5.0, if n % 4 == 3 { FINISHER } else { LIGHT }, n % 4 == 3, false, false).gate == Gate::Full));
    }

    /// THE CHAIN CAP (bossguard's react.chain rows) still holds when the sheet gives no window and no recovery.
    #[test]
    fn the_chain_cap_counts_breaks() {
        let fast = PoiseRules { window: 0.0, lockout: 0.0, ..R };
        let mut knight = Meter::new();
        let mut breaks = 0;
        for _ in 0..40 {
            breaks += knight.hit(&fast, &G, 65.0, 5.0, FINISHER, true, false, false).broke as u32;
            breaks += knight.hit(&fast, &G, 65.0, 5.0, LIGHT, false, false, false).broke as u32;
            run(&mut knight, 0.6);
        }
        // 24 s of a break every 0.6 s: 3, the cool-down of 8 s, 3 more ...
        assert!((6..=9).contains(&breaks) && knight.refused > 0, "breaks {breaks}, refused {}", knight.refused);
        // react.guard = 0: no cap
        let mut knight = Meter::new();
        let n: u32 = (0..40).map(|_| { let b = knight.hit(&fast, &GuardRules::OFF, 65.0, 5.0, FINISHER, true, false, false).broke as u32 + knight.hit(&fast, &GuardRules::OFF, 65.0, 5.0, LIGHT, false, false, false).broke as u32; run(&mut knight, 0.6); b }).sum();
        assert_eq!(n, 40);
    }

    #[test]
    fn the_rows() {
        let mut r = PoiseRules::DEFAULT;
        assert!(r.apply_row("poise.kh", 0.0) && !r.on);
        assert!(r.apply_row("poise.factor", 2.0) && r.factor == 2.0);
        assert!(r.apply_row("poise.lockout", 6.0) && r.lockout == 6.0);
        assert!(!r.apply_row("poise.hyper_factor", 2.0));
        assert!(!r.apply_row("poise.nothing", 1.0));
        assert_eq!(PoiseClass::of(&R, 0.0), PoiseClass::Low);
        assert_eq!(PoiseClass::of(&R, 40.0), PoiseClass::Low);
        assert_eq!(PoiseClass::of(&R, 50.0), PoiseClass::Mid);
        assert_eq!(PoiseClass::of(&R, 70.0), PoiseClass::Mid);
        assert_eq!(PoiseClass::of(&R, 80.0), PoiseClass::High);
        assert!(big_kind(8) && big_kind(2) && !big_kind(1));
        assert!(attack_anim(3003001) && !attack_anim(3008100));
    }
}
