//! (ENG, 2026-10-16) HOW MANY ELDEN RING ENEMIES ATTACK SORA AT ONCE: Elden Ring's own group limiter, set so that up to
//! `enemy.max_attackers` (default 5) members of a group take the attack role on one target at the same time.
//!
//! The user (2026-10-16): "keep 5 enemies attacking you at once but keep everything as it was on 'normal' mode". KH3's
//! coordinator (the 5-attacker rule of its Critical mode) does not reach Elden Ring's enemies (IMPL_DF_STATE.md 4), so
//! this uses Elden Ring's own. READ FROM eldenring.exe 2.7.1.0 (a copy, Ghidra; build/sora_tmp/er/IMPL_ENG_STATE.md):
//! - Every world area's AI (and the world AI manager) has a `CS::CSTeamAi` (RTTI; constructor 0x14034e3f0, which writes
//!   its ATTACK BUDGET +0x30 = 100 and its SURROUND BUDGET +0x34 = 100; nothing else writes them).
//! - Once a second (0x14034f5a0, timer 1.0 s from 0x143b19360) 0x14034ece0 hands out the roles: the members are grouped by
//!   their target, each group is sorted NEAREST FIRST (0x14034e920: distance to the target), and a running sum of each
//!   member's TEAM ATTACK EFFECTIVITY is taken: sum <= 100 -> role 0 (ROLE_TYPE_Attack), sum <= 200 -> role 1
//!   (ROLE_TYPE_Torimaki: circle around), else role 2 (ROLE_TYPE_Kankyaku: watch). The role is the Lua AI's
//!   `GetTeamOrder(ORDER_TYPE_Role)` (0x1402ffc30). (The other branch, a fixed 4 attackers / 5 surrounders, is off: its
//!   switch 0x143b19364 is 1 in the file and no code writes it.)
//! - The effectivity of a member (0x1402c2a60): an active SpEffect's `teamOffenseEffectivity` when it is not -1, else
//!   `NpcThinkParam.TeamAttackEffectivity` (+0x54) READ THROUGH THE ROW POINTER of the AI (+0x30) at every hand-out.
//!   So a value written in the loaded table counts within a second, for enemies already standing too.
//! THE MAPPING: a row's value is capped at floor(100 / N): N members with it fit the budget, N + 1 do not. 0 stays 0 (it
//! uses no budget: every such member already attacks). The real values (regulation 2.7.1.0, read offline from a copy):
//! 1727 of 2215 rows are above 20; most enemies have 30 - 100 (1 - 3 at once); the Godrick-soldier family c4300 already has
//! 20 (5 at once) and does not change.
//! IN MEMORY ONLY: regulation.bin is never touched; every value is written back when Sora stops, when this is dropped
//! (the payload replaced, the mode switched) and when the key goes to 0. Each changed row also carries its own value in
//! two of its padding bytes (`pad4`, zero in every row of the game), so a payload that starts over rows a crashed
//! payload left capped still finds the game's numbers.

/// The attack budget of a team (`CSTeamAi` +0x30, written 100 by its constructor 0x14034e3f0; no other writer).
pub const TEAM_BUDGET: u32 = 100;
/// `enemy.max_attackers` when the control file has none (the user's number).
pub const DEFAULT_MAX_ATTACKERS: u32 = 5;

/// The cap on a row's TeamAttackEffectivity that lets `n` members attack one target at once (`None`: `n` = 0, the game's
/// own values). floor(budget / n) = the largest value with which at least `n` fit (exactly `n` for 1-10 and 5 = 20; for
/// some `n`, e.g. 13, no integer value gives exactly `n` and the next step lets a few more); at least 1 (0 would mean
/// "no budget used at all") and at most 100 (the field's range).
pub fn cap_for(n: u32, budget: u32) -> Option<u8> {
    (n > 0).then(|| (budget / n).clamp(1, 100) as u8)
}

/// A row's value under the cap: 0 stays 0 (it already lets every such member attack), a smaller value stays.
pub fn capped(value: u8, cap: u8) -> u8 {
    if value == 0 { 0 } else { value.min(cap) }
}

/// A SpEffect's override (`teamOffenseEffectivity`, -1 = none) under the cap.
pub fn capped_override(value: i8, cap: u8) -> i8 {
    if value <= 0 { value } else { (value as u8).min(cap) as i8 }
}

/// The roles of one target's members, nearest first, as 0x14034ece0 hands them out (0 attack, 1 surround, 2 watch).
pub fn roles(values: &[u8], budget: u32, surround: u32) -> Vec<u8> {
    let mut sum = 0u32;
    values.iter().map(|v| {
        sum += *v as u32;
        if sum <= budget { 0 } else if sum <= budget + surround { 1 } else { 2 }
    }).collect()
}

/// How many of `count` members of one value take the attack role at once (the game's own rule).
pub fn at_once(value: u8, count: usize, budget: u32) -> usize {
    roles(&vec![value; count], budget, budget).iter().filter(|r| **r == 0).count()
}

/// The attack-animation meter of a fight, for the log: the most hostiles near Sora in an attack animation at once.
#[derive(Default, Debug)]
pub struct Meter {
    pub peak: usize,
    pub near_peak: usize,
    pub quiet: f32,
    pub seconds: f32,
}

impl Meter {
    /// One frame: `attacking` hostiles in an attack animation within reach, `near` hostiles within reach. `Some(peak,
    /// near peak, length)` when a fight ended (8 s without one in an attack animation).
    pub fn frame(&mut self, attacking: usize, near: usize, dt: f32) -> Option<(usize, usize, f32)> {
        if attacking > 0 {
            self.peak = self.peak.max(attacking);
            self.quiet = 0.0;
        } else if self.peak > 0 {
            self.quiet += dt;
        }
        if self.peak > 0 {
            self.near_peak = self.near_peak.max(near);
            self.seconds += dt;
        }
        if self.peak > 0 && self.quiet >= 8.0 {
            let out = (self.peak, self.near_peak, self.seconds - self.quiet);
            *self = Meter::default();
            return Some(out);
        }
        None
    }
}

#[cfg(windows)]
pub use game::ErAttackers;

#[cfg(windows)]
mod game {
    use super::*;
    use crate::control::Control;
    use crate::erlog;
    use eldenring::cs::{NpcThinkParam, SoloParamRepository, SpEffectParam};
    use eldenring::param::NPC_THINK_PARAM_ST;
    use std::collections::HashSet;

    /// The row's field the game reads (0x1402c2a60: `movzx ebx, byte [rax+54h]`); checked against the SDK's getter.
    const FIELD: usize = 0x54;
    /// `pad4` (16 bytes at 0x7E, zero in all 2215 rows of regulation 2.7.1.0): [0x7E] = MARK when [0x7F] holds the game's value.
    const PAD_MARK: usize = 0x7E;
    const PAD_VALUE: usize = 0x7F;
    const MARK: u8 = 0xE5;
    const _: () = assert!(std::mem::size_of::<NPC_THINK_PARAM_ST>() == 0xE4); // the row stride of the regulation's table
    /// The rows named in the log (the families of the user's log: c4300, c4161, c4080, c4050, c4440, c3000, c2271).
    const SAMPLES: [(u32, &str); 7] = [(43000000, "c4300"), (41610000, "c4161"), (40800000, "c4080"), (40500000, "c4050"), (44400000, "c4440"), (30000000, "c3000"), (22710000, "c2271")];
    /// Hostiles within this distance of Sora count for the meter (cm).
    const NEAR_CM: f32 = 1500.0;

    pub struct ErAttackers {
        /// The changed think rows: (row id, the game's value).
        think: Vec<(u32, u8)>,
        /// The changed SpEffect overrides: (row id, the game's value).
        speffect: Vec<(u32, i8)>,
        /// The cap in force (`None`: the game's own values).
        cap: Option<u8>,
        /// The control file's generation last looked at.
        generation: u64,
        looked: bool,
        refused: bool,
        seen: HashSet<i32>,
        meter: Meter,
    }

    fn repo() -> Option<&'static mut SoloParamRepository> {
        use fromsoftware_shared::FromStatic;
        // SAFETY: game thread; the singleton lives as long as the game; the SDK checks its holder table.
        unsafe { SoloParamRepository::instance_mut() }.ok()
    }

    fn raw(row: &mut NPC_THINK_PARAM_ST) -> *mut u8 {
        row as *mut NPC_THINK_PARAM_ST as *mut u8
    }

    /// The game's own value of a row: its padding's copy when an earlier payload left one, else the field.
    fn own_value(row: &mut NPC_THINK_PARAM_ST) -> u8 {
        let p = raw(row);
        // SAFETY: both offsets lie inside the 0xE4-byte row (asserted above).
        unsafe { if *p.add(PAD_MARK) == MARK { *p.add(PAD_VALUE) } else { row.team_attack_effectivity() } }
    }

    fn put(row: &mut NPC_THINK_PARAM_ST, value: u8, own: u8) {
        row.set_team_attack_effectivity(value);
        let p = raw(row);
        // SAFETY: inside the row; the padding is unused by the game (zero in every row) and cleared again on restore.
        unsafe {
            if value == own {
                if *p.add(PAD_MARK) == MARK {
                    *p.add(PAD_MARK) = 0;
                    *p.add(PAD_VALUE) = 0;
                }
            } else if *p.add(PAD_MARK) == 0 || *p.add(PAD_MARK) == MARK {
                *p.add(PAD_MARK) = MARK;
                *p.add(PAD_VALUE) = own;
            }
        }
    }

    impl Default for ErAttackers {
        fn default() -> Self {
            Self::new()
        }
    }

    impl ErAttackers {
        pub fn new() -> Self {
            Self { think: Vec::new(), speffect: Vec::new(), cap: None, generation: 0, looked: false, refused: false, seen: HashSet::new(), meter: Meter::default() }
        }

        fn wanted(control: &Control) -> (u32, u32) {
            let n = control.i64("enemy.max_attackers").map_or(DEFAULT_MAX_ATTACKERS, |v| v.clamp(0, 100) as u32);
            let budget = control.i64("enemy.attack_slots_budget").map_or(TEAM_BUDGET, |v| v.clamp(1, 1000) as u32);
            (n, budget)
        }

        /// Every frame of Sora: the cap is (re)applied at the start and whenever the control file changes; `hostiles` =
        /// (chr id, think row, distance cm, in an attack animation) of the hostiles around him, for the log.
        pub fn frame(&mut self, control: &Control, hostiles: &[(i32, i32, f32, bool)], dt: f32) {
            if !self.looked || control.generation != self.generation {
                self.looked = true;
                self.generation = control.generation;
                let (n, budget) = Self::wanted(control);
                let cap = cap_for(n, budget);
                if cap != self.cap {
                    match cap {
                        Some(c) => self.apply(c, n, budget),
                        None => {
                            self.restore("enemy.max_attackers = 0: the game's own values");
                            self.cap = None;
                        }
                    }
                }
            }
            for (chr, think, _, _) in hostiles {
                if *think >= 0 && self.seen.insert(*think) {
                    self.tell_kind(*chr, *think);
                }
            }
            let near = hostiles.iter().filter(|h| h.2 <= NEAR_CM).count();
            let attacking = hostiles.iter().filter(|h| h.2 <= NEAR_CM && h.3).count();
            if let Some((peak, near_peak, seconds)) = self.meter.frame(attacking, near, dt) {
                erlog!("enemy: fight over ({seconds:.0} s): at most {peak} enemies in an attack animation at once, {near_peak} hostiles within {:.0} m at most (enemy.max_attackers {})", NEAR_CM / 100.0, self.cap.map_or("0: the game's own values".to_string(), |c| format!("cap {c}")));
            }
        }

        fn tell_kind(&self, chr: i32, think: i32) {
            let Some(repo) = repo() else { return };
            let Some(row) = repo.get_mut::<NpcThinkParam>(think as u32) else { return };
            let now = row.team_attack_effectivity();
            let own = own_value(row);
            let line = |v: u8| if v == 0 { "every one".to_string() } else { format!("{}", at_once(v, 100, TEAM_BUDGET)) };
            erlog!("enemy: c{chr:04} NpcThinkParam {think}: TeamAttackEffectivity {now} (the game's {own}): {} of a group take the attack role on Sora at once (the game: {})", line(now), line(own));
        }

        fn apply(&mut self, cap: u8, n: u32, budget: u32) {
            let Some(repo) = repo() else {
                erlog!("enemy: team attack: the param repository is not there: NOT APPLIED");
                return;
            };
            // the offset check: the byte the game reads must be the SDK's field
            if let Some((_, row)) = repo.rows_mut::<NpcThinkParam>().next() {
                // SAFETY: inside the row.
                let byte = unsafe { *raw(row).add(FIELD) };
                if byte != row.team_attack_effectivity() {
                    if !self.refused {
                        erlog!("enemy: team attack: THE ROW LAYOUT IS NOT THE ONE READ FROM THE EXE (byte +0x54 {byte}, field {}): NOT APPLIED", row.team_attack_effectivity());
                    }
                    self.refused = true;
                    return;
                }
            }
            let mut before = std::collections::BTreeMap::<u8, usize>::new();
            let (mut rows, mut changed) = (0, 0);
            let mut think = Vec::new();
            for (id, row) in repo.rows_mut::<NpcThinkParam>() {
                rows += 1;
                let own = own_value(row);
                *before.entry(own).or_default() += 1;
                let value = capped(own, cap);
                if value != row.team_attack_effectivity() || value != own {
                    put(row, value, own);
                }
                if value != own {
                    changed += 1;
                    think.push((id, own));
                }
            }
            // rows of the earlier cap that are no longer above this one are already back (value == own above)
            self.think = think;
            let mut speffect = self.speffect.drain(..).collect::<std::collections::HashMap<u32, i8>>();
            let mut sp_changed = 0;
            for (id, row) in repo.rows_mut::<SpEffectParam>() {
                let own = speffect.remove(&id).unwrap_or(row.team_offense_effectivity());
                let value = capped_override(own, cap);
                if value != row.team_offense_effectivity() {
                    row.set_team_offense_effectivity(value);
                }
                if value != own {
                    sp_changed += 1;
                    self.speffect.push((id, own));
                }
            }
            self.cap = Some(cap);
            let samples: Vec<String> = SAMPLES.iter().filter_map(|(id, chr)| {
                let own = self.think.iter().find(|(t, _)| t == id).map(|(_, v)| *v);
                let row = repo.get::<NpcThinkParam>(*id)?;
                let now = row.team_attack_effectivity();
                let own = own.unwrap_or(now);
                Some(if own == now { format!("{chr} ({id}) {own} unchanged ({} at once)", at_once(own, 100, budget)) } else { format!("{chr} ({id}) {own} -> {now} ({} -> {} at once)", at_once(own, 100, budget), at_once(now, 100, budget)) })
            }).collect();
            let hist: Vec<String> = before.iter().map(|(v, c)| format!("{v}: {c}")).collect();
            erlog!("enemy: team attack: {changed} of {rows} NpcThinkParam rows capped at {cap} (enemy.max_attackers = {n}, team budget {budget}: up to {n} members of a group take the attack role on one target at once; Elden Ring's own limiter CSTeamAi, in memory only, regulation.bin untouched), {sp_changed} SpEffect override(s) capped; before -> after: {}; the game's values (value: rows): {}", samples.join(", "), hist.join(", "));
        }

        /// Every changed row gets the game's value back.
        pub fn restore(&mut self, why: &str) {
            if self.think.is_empty() && self.speffect.is_empty() {
                self.cap = None;
                return;
            }
            let Some(repo) = repo() else { return };
            let n = self.think.len();
            for (id, own) in self.think.drain(..) {
                if let Some(row) = repo.get_mut::<NpcThinkParam>(id) {
                    put(row, own, own);
                }
            }
            for (id, own) in self.speffect.drain(..) {
                if let Some(row) = repo.get_mut::<SpEffectParam>(id) {
                    row.set_team_offense_effectivity(own);
                }
            }
            self.cap = None;
            self.looked = false;
            erlog!("enemy: team attack: {n} NpcThinkParam rows got the game's own TeamAttackEffectivity back ({why})");
        }
    }

    impl Drop for ErAttackers {
        fn drop(&mut self) {
            self.restore("dropped: Sora stopped or the payload is replaced");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cap_lets_exactly_n_attack() {
        // the largest value that lets at least n attack: exactly n where an integer value can (1-10, 12, 14, 16, 20,
        // 25, 33, 50, 100), else the few more of the nearest step (13 -> cap 7 -> 14)
        for n in 1..=50u32 {
            let cap = cap_for(n, TEAM_BUDGET).unwrap();
            let k = at_once(cap, 200, TEAM_BUDGET);
            assert!(k >= n as usize, "n {n} cap {cap}: {k}");
            assert!(at_once(cap + 1, 200, TEAM_BUDGET) < n as usize, "n {n}: cap {cap} is not the largest");
            if TEAM_BUDGET / (TEAM_BUDGET / n) == n {
                assert_eq!(k, n as usize, "n {n}");
            }
        }
        for n in 1..=10 {
            assert_eq!(at_once(cap_for(n, 100).unwrap(), 50, 100), n as usize);
        }
        assert_eq!(cap_for(5, 100), Some(20));
        assert_eq!(cap_for(3, 100), Some(33));
        assert_eq!(cap_for(0, 100), None);
        assert_eq!(cap_for(1000, 100), Some(1)); // never 0: that is "unlimited"
        assert_eq!(cap_for(1, 100), Some(100));
    }

    #[test]
    fn the_games_own_values_and_what_five_makes_of_them() {
        // the field's default 25 -> 4 at once; the common values of the regulation
        assert_eq!(at_once(25, 10, 100), 4);
        assert_eq!(at_once(20, 10, 100), 5);
        assert_eq!(at_once(35, 10, 100), 2);
        assert_eq!(at_once(50, 10, 100), 2);
        assert_eq!(at_once(60, 10, 100), 1);
        assert_eq!(at_once(80, 10, 100), 1);
        assert_eq!(at_once(100, 10, 100), 1);
        assert_eq!(at_once(0, 10, 100), 10); // 0 uses no budget
        let cap = cap_for(DEFAULT_MAX_ATTACKERS, TEAM_BUDGET).unwrap();
        for v in [25u8, 30, 34, 35, 40, 50, 60, 80, 100] {
            assert_eq!(at_once(capped(v, cap), 10, 100), 5, "value {v}");
        }
        // smaller values keep theirs (more at once already), 0 stays 0
        assert_eq!(capped(10, cap), 10);
        assert_eq!(capped(20, cap), 20);
        assert_eq!(capped(0, cap), 0);
        assert_eq!(capped(1, cap), 1);
    }

    #[test]
    fn the_roles_follow_the_running_sum_nearest_first() {
        // a mixed group: 20, 35, 35, 20, 50 -> sums 20 55 90 110 160: three attack, two surround
        assert_eq!(roles(&[20, 35, 35, 20, 50], 100, 100), vec![0, 0, 0, 1, 1]);
        // the same group capped at 20: five attack
        assert_eq!(roles(&[20, 35, 35, 20, 50].map(|v| capped(v, 20)), 100, 100), vec![0, 0, 0, 0, 0]);
        // past the surround budget: watch
        assert_eq!(roles(&[80, 80, 80], 100, 100), vec![0, 1, 2]);
        // exactly at the budget still attacks (the game's test is sum > budget)
        assert_eq!(roles(&[50, 50, 1], 100, 100), vec![0, 0, 1]);
    }

    #[test]
    fn speffect_overrides() {
        assert_eq!(capped_override(-1, 20), -1);
        assert_eq!(capped_override(0, 20), 0);
        assert_eq!(capped_override(20, 20), 20);
        assert_eq!(capped_override(20, 14), 14);
        assert_eq!(capped_override(100, 20), 20);
    }

    #[test]
    fn the_meter_reports_a_fight_after_eight_quiet_seconds() {
        let mut m = Meter::default();
        assert_eq!(m.frame(0, 3, 0.1), None);
        for k in 0..30 {
            assert_eq!(m.frame(if k % 3 == 0 { 4 } else { 2 }, 6, 0.1), None);
        }
        let mut out = None;
        for _ in 0..100 {
            if let Some(o) = m.frame(0, 6, 0.1) {
                out = Some(o);
                break;
            }
        }
        let (peak, near, seconds) = out.unwrap();
        assert_eq!((peak, near), (4, 6));
        assert!((seconds - 3.0).abs() < 0.2, "{seconds}");
        assert_eq!(m.peak, 0);
    }
}
