//! EXP and levelling (docs/sora-stats-spec.md section 4).
//!
//! `GiveExp` 0x140E9E830 -> `AdjustExp` 0x140632CB0 -> `AddExp` 0x140E864B0 -> `CheckLevelUp`
//! 0x140EBE400. EXP accrues at the moment of the kill, also beyond level 99 (up to the cap);
//! with Zero EXP nothing accrues at all. One award can raise several levels with ONE level-up
//! panel. A level-up refills nothing and equips nothing.

use super::math::ue_ceil;
use super::tables::{self, GameLevel};
use super::Status;
use crate::enums::ETresAbilityKind;

/// At most this much EXP per award (`AdjustExp`, [I] 0x140632E37).
pub const EXP_AWARD_CAP: i32 = 999;

/// The lines of the level-up panel (`LevelUpDelta` 0x140583470): the new level and
/// `row(new) - row(new - 1)` for Strength, Magic, Defense, AP, and the ability of the NEW row on
/// the current path. Only non-zero entries are shown by the HUD (kinds 1 Strength, 2 Magic,
/// 3 Defense, 4 AP, 7 ability). With an award that crosses several levels the deltas are still
/// those of the final level's row alone (the game calls `LevelUpDelta(bp, newLevel)` once).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelUp {
    pub level: u32,
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
    pub ap: i32,
    pub ability: Option<ETresAbilityKind>,
}

/// What one award did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpAward {
    /// The enemy's reward before `AdjustExp`: `ceil(rate * base_exp)`.
    pub reward: i32,
    /// After `AdjustExp` (difficulty, Zero EXP, EXP Incentive, the 999 cap).
    pub adjusted: i32,
    /// What was really added to the total (the 9,999,999 cap).
    pub added: i32,
    pub total: i32,
    pub level_before: u32,
    pub level_after: u32,
    /// The next-level EXP before the add was > 0: the HUD shows the floating EXP number (when
    /// the award has a position).
    pub show_exp_point: bool,
    /// The one level-up panel of this award, if a level was gained.
    pub level_up: Option<LevelUp>,
    /// "Next LV": EXP still needed (`next-level EXP - total`), `None` at level 99 or when
    /// nothing was added.
    pub next_lv: Option<i32>,
}

/// (party P2) The pawn `AdjustExp` asks ("player 0", 0x140EA8DD0): alive (HP above 0), his ZERO EXP (26) and EXP
/// INCENTIVE (174) counts, his level (EXP Incentive's level gap).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpPlayer {
    pub alive: bool,
    pub exp_zero: u32,
    pub exp_bargain: u32,
    pub level: u32,
}

impl Status {
    /// The reward of an enemy (spec 4.1, 0x140576130; sheet row `exp.enemy_reward`):
    /// `ceil(m_ExpRate * EnemyBaseStatus[battle level].m_BaseExp)`.
    pub fn enemy_reward(base_exp: i32, rate: f32) -> i32 {
        ue_ceil(rate * base_exp as f32)
    }

    /// `AdjustExp` 0x140632CB0 (sheet row `exp.award`): 0 when dead or with Zero EXP; Beginner
    /// `e + ceil(0.25 e)`; Critical `ceil(0.75 e)`; EXP Incentive `+ ceil((0.1 d + 0.5) gain)`
    /// with `d = max(battle level - level, 0)`; at most 999. [I] 0x140632D79, 0x140632DA1,
    /// 0x140632E0A, 0x140632E12, 0x140632E37. (The form ids 5, 7, 11, 12 that also give 0 are
    /// formchange states: none exists in the port yet.)
    pub fn adjust_exp(&self, exp: i32) -> i32 {
        self.adjust_exp_for(exp, ExpPlayer { alive: self.hp > 0, exp_zero: self.count(ETresAbilityKind::Expzero), exp_bargain: self.count(ETresAbilityKind::ExpBargain), level: self.level })
    }

    /// (party P2) `AdjustExp` with ANOTHER PAWN as "player 0" (spec 4.2 step 2: the pawn whose HP, Zero EXP and EXP
    /// Incentive gate the award is player 0, 0x140EA8DD0; a host whose controlled pawn is a friend passes his). The
    /// difficulty and the battle level are this status's (game-wide).
    pub fn adjust_exp_for(&self, exp: i32, player: ExpPlayer) -> i32 {
        if self.placeholder || !player.alive || player.exp_zero > 0 {
            return 0;
        }
        let mut gain = exp;
        match self.game_level {
            GameLevel::Beginner => gain = exp + ue_ceil(0.25 * exp as f32),
            GameLevel::Critical => gain = ue_ceil(0.75 * exp as f32),
            _ => {}
        }
        if player.exp_bargain > 0 {
            let d = (self.battle_level as i32 - player.level as i32).max(0);
            gain += ue_ceil((0.1 * d as f32 + 0.5) * gain as f32);
        }
        gain.min(EXP_AWARD_CAP)
    }

    /// GIVES THE EXP OF A DEFEATED ENEMY: `give_exp(enemy base exp, rate)` (spec 4.1 - 4.3).
    /// `base_exp` = `EnemyBaseStatus[battle level].m_BaseExp`, `rate` = the enemy row's
    /// `m_ExpRate`. Returns what happened; the caller that owns the HUD seam raises the popups
    /// from it (`Sora::give_exp` does, in the UI spec's order).
    pub fn give_exp(&mut self, base_exp: i32, rate: f32) -> ExpAward {
        let reward = Self::enemy_reward(base_exp, rate);
        let level_before = self.level;
        let mut award = ExpAward { reward, adjusted: 0, added: 0, total: self.exp, level_before, level_after: level_before, show_exp_point: false, level_up: None, next_lv: None };
        // 1. Nothing when the reward is 0.
        if reward == 0 || self.placeholder {
            return award;
        }
        // 2. AdjustExp.
        award.adjusted = self.adjust_exp(reward);
        self.add_exp_award(award)
    }

    /// (party P2) GIVES AN AWARD ALREADY ADJUSTED (`adjust_exp_for` with the host's player 0): steps 3 .. 5 of
    /// `give_exp` with `gain` as the adjusted EXP.
    pub fn give_exp_gain(&mut self, reward: i32, gain: i32) -> ExpAward {
        let level_before = self.level;
        let award = ExpAward { reward, adjusted: gain, added: 0, total: self.exp, level_before, level_after: level_before, show_exp_point: false, level_up: None, next_lv: None };
        if reward == 0 || self.placeholder {
            return award;
        }
        self.add_exp_award(award)
    }

    /// Steps 3 .. 5 of `give_exp` (AddExp, CheckLevelUp, the HUD values) for an award whose `adjusted` is set.
    fn add_exp_award(&mut self, mut award: ExpAward) -> ExpAward {
        let next_before = self.exp_next;
        // 3. AddExp 0x140E864B0: total = clamp(total + gain, 0, 9,999,999); the amount really added.
        let cap = tables::RULES.exp_total_cap as i32;
        let total = (self.exp as i64 + award.adjusted as i64).clamp(0, cap as i64) as i32;
        award.added = total - self.exp;
        self.exp = total;
        award.total = total;
        if award.added <= 0 {
            return award;
        }
        // 4. CheckLevelUp 0x140EBE400: if 0 < next-level EXP <= total, rebuild (the level is
        //    recomputed from the EXP, so several levels can be gained at once, with one panel).
        if next_before > 0 && next_before <= total {
            self.rebuild();
            let (new, old) = (tables::level_row(self.level), tables::level_row(self.level - 1));
            award.level_up = Some(LevelUp {
                level: self.level,
                strength: new.attack - old.attack,
                magic: new.magic - old.magic,
                defense: new.defense - old.defense,
                ap: new.ap - old.ap,
                ability: tables::level_ability(self.level, self.ability_path),
            });
        }
        award.level_after = self.level;
        // 5. HUD: the EXP number when a next level existed before the add; "Next LV" when one
        //    exists after it.
        award.show_exp_point = next_before > 0;
        if self.exp_next > 0 {
            award.next_lv = Some(self.exp_next - total);
        }
        award
    }

    /// Fraction of the current level's EXP gauge, 0..1 (spec 4.4:
    /// `(total - this level) / (next level - this level)`); 1 at level 99.
    pub fn exp_fraction(&self) -> f32 {
        if self.exp_next <= self.exp_level {
            return 1.0;
        }
        ((self.exp - self.exp_level) as f32 / (self.exp_next - self.exp_level) as f32).clamp(0.0, 1.0)
    }

    /// EXP still needed for the next level; 0 at level 99.
    pub fn exp_to_next(&self) -> i32 {
        if self.exp_next > 0 {
            (self.exp_next - self.exp).max(0)
        } else {
            0
        }
    }
}
