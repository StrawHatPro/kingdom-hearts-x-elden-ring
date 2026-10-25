//! Who heals whom with what: the friend's cure / item decision (`FUN_141264e30`, asked by the
//! MetaAI tick `FUN_14123e440`) and the MetaAI cure record that `IsCureInfoCommand` 0x141207870,
//! `SetCureTargetBB` 0x14121B340, `NpcCureAction` 0x14120DF60 and `NpcUseItem` 0x1412234A0 read
//! [C, transcribed 2026-10-15 by stream C2].
//!
//! The game, as read:
//! - `UTresNpcMetaAI` tick `FUN_14123e440`: with no record, every 2.0 s (+0x2AC accumulates dt,
//!   `> 2.0`) every NPC pawn of the coordinator builds its record (`FUN_141264e30`); the record whose
//!   curer-target distance (XY) is the smallest below 2000 cm wins; when none is that close, one of
//!   the far ones is taken at random (FRand xorshift, `(x >> 1) % n`). With a record: it is dropped
//!   when the curer is invalid / dying (`FUN_141229840`: state DIE now or before, vt+0x2540), when the
//!   need flag stored with it no longer holds (+0x2A5 HP `FUN_14126ac70`, else +0x2A6 MP
//!   `FUN_14126b550`, else +0x2A7 focus `FUN_14126a1f0`, else +0x2A8 bad status `FUN_141269db0`),
//!   when a Cure record's curer is recharging MP in IDLE / TURN / AI_MOVE (the Cure was cast), and
//!   out of battle unless the command is Cure.
//! - the pawn's record `FUN_141264e30`: no record when the pawn is disabled (+0xB95), moving with
//!   +0x518 bit 0, or in TSID_STATE_DIE. Candidates by the Recovery target bits (record +0x15B; the
//!   style component +0x460 vt+0x148 / +0x158 / +0x168): bit 0 the player, bit 1 itself, bit 2 the
//!   other party members (other uid, not +0xB94, not disabled). Commands: the item slots' kinds
//!   (`FUN_1405aa970`, slot order) and, when it can cast Cure (vt+0x2918; Donald 0x1411CA900: magic
//!   not restricted and ability 0xCD usable, i.e. not recharging MP), TRES_CMD_KIND_CURE last.
//!   Cure first: style OFTEN / PINCH, or ALWAYS out of battle, and somebody needs HP -> the best
//!   target for Cure. Otherwise, in battle only (out of battle the list is emptied), the command with
//!   the highest score over its best target. Score of (command, target) (`FUN_141263e10`): the
//!   `FriendEvaluationData` row of the command (m_SpecialCure from vt+0x2928, false for both
//!   friends) summed over the needs the target has: bad status + HP + MP + focus, + all-party when
//!   more than one candidate needs it (the row's m_AllCharaEvaluation). Ties keep the first.
//! - the needs per `ETresFNpcAIStyle_Cure` (record +0x15A, vt+0x138), "in battle" = the curer's
//!   battle flag with a hostile actor around (`FUN_141267740`):
//!   HP (`FUN_14126a870` / flag `FUN_14126ac70`): OFTEN HP% < 50 (35 in battle); PINCH HP <= 25 %
//!   (vt+0x2538, integer) or the target is in DIE; ALWAYS HP% < 80 (50 in battle) and, in battle,
//!   a fitting item / Cure exists (`FUN_14125f380`). Out of battle only Cure answers it.
//!   MP (`FUN_14126af20` / flag `FUN_14126b550`): OFTEN MP% <= 24 and not recharging or >= 8 s of
//!   recharge left (vt+0x17F0 = +0x7C4); PINCH recharging with >= 8 s left; ALWAYS MP% <= 24, not
//!   recharging with < 8 s left, and a fitting MP item (`FUN_14125f7f0`).
//!   Focus (`FUN_141269f60`): the player only; OFTEN / ALWAYS focus% <= 24, PINCH vt+0x1170.
//!   Bad status (`FUN_141269db0`): +0x85D set, or the state is BADSTAT_STUN / FREEZE / BURN / SNEEZE.
//! - the fitting item (`FUN_14125f380`, HP): missing = max HP - HP; Cure when it can cast it, the
//!   target's HP% < 50 and the Cure amount (vt+0x2920 + max(1, magic): `FUN_141263cc0`; Donald's
//!   m_AppendCure 120, ctor 0x1411BA820) covers it; else over the HP items / Cure: amount =
//!   m_RecoveryPoint (Cure: the Cure amount), rem = missing - amount; an item that covers (rem < 1)
//!   beats one that does not, two that cover: the smaller overshoot, two that do not: the larger
//!   amount. MP (`FUN_14125f7f0`): the same over ETHER .. LASTELIXIR on the MP missing.
//!
//! Fed by C3 through AiBody / AiWorld (fallbacks = the earlier behaviour): the command restriction
//! `FUN_1405ad320(6)` = "IsCommandRestricted:6", ITEM_UP = "GetAbilityNum:ITEM_UP", the player focus
//! = `AiWorld::player_focus`, the other members' MP = `AiWorld::member_mp`. Still [D]: restriction 5 (not seen in
//! the decision body), the curer's +0xB95 / +0x518 flags. Read by C4: the focus PINCH predicate
//! (player vt+0x1170 = +0x7C0 bit 0, the focus-break bit), the ALWAYS focus fit (`FUN_14125ef60`)
//! and the record's focus flag (`FUN_14126a1f0`, no fit).

use crate::api::{ActorRef, AiBody, AiWorld, CureStyle, CURE_TARGET_OTHERS, CURE_TARGET_PLAYER, CURE_TARGET_SELF};
use crate::pack::{pb, pf, ps, AiPack};
use glam::Vec3;

pub const CMD_NONE: &str = "TRES_CMD_KIND_NONE";
pub const CMD_CURE: &str = "TRES_CMD_KIND_CURE";

/// Kinds by `ETresCommandKind` value groups (the switch of `FUN_141264e30`).
fn kind_value(k: &str) -> i32 {
    match k {
        "TRES_CMD_KIND_CURE" => 0x31,
        "TRES_CMD_KIND_POTION" => 0x3A,
        "TRES_CMD_KIND_HIPOTION" => 0x3B,
        "TRES_CMD_KIND_MEGAPOTION" => 0x3C,
        "TRES_CMD_KIND_ETHER" => 0x3D,
        "TRES_CMD_KIND_HIETHER" => 0x3E,
        "TRES_CMD_KIND_MEGAETHER" => 0x3F,
        "TRES_CMD_KIND_ELIXIR" => 0x40,
        "TRES_CMD_KIND_LASTELIXIR" => 0x41,
        "TRES_CMD_KIND_FOCUSSUPPLEMENT" => 0x42,
        "TRES_CMD_KIND_HIFOCUSSUPPLEMENT" => 0x43,
        "TRES_CMD_KIND_ALLCURE" => 0x44,
        _ => 0,
    }
}

/// The MetaAI cure record (`UTresNpcMetaAI` +0x290 curer, +0x298 target, +0x2A0 score,
/// +0x2A4 command, +0x2A5..+0x2A8 the need flags HP / MP / focus / bad status).
#[derive(Clone, Debug, PartialEq)]
pub struct CureInfo {
    pub curer: u8,
    pub target: ActorRef,
    /// `ETresCommandKind` name ("TRES_CMD_KIND_CURE", "TRES_CMD_KIND_POTION" ...).
    pub command: String,
    /// +0x2A0: command index + score * 10.
    pub score: i32,
    /// +0x2A5 HP, +0x2A6 MP, +0x2A7 focus, +0x2A8 bad status.
    pub flags: [bool; 4],
    pub since: f64,
}

/// One candidate target as the decision reads it.
#[derive(Clone, Copy, Debug)]
pub struct TargetStat {
    pub actor: ActorRef,
    pub position: Vec3,
    pub hp: i32,
    pub hp_max: i32,
    /// None when the view does not carry MP (other members).
    pub mp: Option<(i32, i32)>,
    pub mp_charging: bool,
    /// Seconds of MP recharge left (vt+0x17F0); unknown = infinity.
    pub mp_charge_left: f32,
    pub dying: bool,
    pub bad_status: bool,
    pub is_player: bool,
    /// The player's focus gauge (current, max) when the host knows it (`AiWorld::player_focus`).
    pub focus: Option<(i32, i32)>,
}

impl TargetStat {
    /// vt+0x17B8 `FUN_1405aadc0`: int(hp / max * 100) clamped to 0..100, 0 without a max.
    pub fn hp_pct(&self) -> i32 {
        pct(self.hp, self.hp_max)
    }
    /// vt+0x2538 `FUN_1405ad600`: hp <= max * 25 / 100 (integers).
    pub fn pinch(&self) -> bool {
        self.hp <= self.hp_max * 25 / 100
    }
}

fn pct(v: i32, max: i32) -> i32 {
    if max <= 0 {
        return 0;
    }
    let p = ((v as f32 / max as f32) * 100.0) as i32;
    if p < 0 {
        0
    } else {
        p.min(100)
    }
}

/// `FriendEvaluationData` row (`vt+0x250` of the table holder, by command kind and m_SpecialCure).
#[derive(Clone, Copy, Debug, Default)]
pub struct EvalRow {
    pub bad: i32,
    pub hp: i32,
    pub mp: i32,
    pub focus: i32,
    pub all: i32,
    pub recovery: i32,
}

pub fn eval_row(pack: &AiPack, kind: &str, special: bool) -> Option<EvalRow> {
    let t = pack.tables.get("FriendEvaluationData")?;
    t.rows.iter().map(|(_, r)| r).find(|r| ps(r, "m_CommandKind") == Some(kind) && pb(r, "m_SpecialCure", false) == special).map(|r| EvalRow {
        bad: pf(r, "m_BadStatEvaluation", 0.0) as i32,
        hp: pf(r, "m_HitPointEvaluation", 0.0) as i32,
        mp: pf(r, "m_MagicPointEvaluation", 0.0) as i32,
        focus: pf(r, "m_FocusPointEvaluation", 0.0) as i32,
        all: pf(r, "m_AllCharaEvaluation", 0.0) as i32,
        recovery: pf(r, "m_RecoveryPoint", 0.0) as i32,
    })
}

/// The curer's side of the decision.
pub struct Curer<'a> {
    pub pack: &'a AiPack,
    pub style: CureStyle,
    /// `FUN_141267740`: battle flag and a hostile actor around.
    pub in_battle: bool,
    /// vt+0x2918.
    pub can_cure: bool,
    /// `FUN_141263cc0` (vt+0x2920 + max(1, magic)); 0 when it cannot cast Cure.
    pub cure_amount: i32,
    /// The command list: item kinds in slot order, then Cure when it can cast it.
    pub commands: Vec<String>,
    /// ITEM_UP (ETresAbilityKind 0x28) equipped count (pawn vt+0x1500), for FUN_141263950.
    pub item_up: i32,
    /// The player's focus-break bit (pawn +0x7C0 bit 0 = vt+0x1170 0x140545D70) [C4]: the PINCH
    /// focus need (`controller::FocusBreak`).
    pub focus_break: bool,
}

impl Curer<'_> {
    fn row(&self, kind: &str) -> Option<EvalRow> {
        // m_SpecialCure = vt+0x2928 (0x14041F340 / 0x14041F1E0 in the friends' vtables: false)
        eval_row(self.pack, kind, false)
    }

    /// `FUN_14126ac70`: the HP need flag stored with a record (no command involved).
    pub fn hp_flag(&self, t: &TargetStat) -> bool {
        match self.style {
            CureStyle::Often => t.hp_pct() < if self.in_battle { 35 } else { 50 },
            CureStyle::Pinch => t.pinch() || t.dying,
            CureStyle::Always => t.hp_pct() < if self.in_battle { 50 } else { 80 },
        }
    }

    /// `FUN_14126a870`: target `t` needs HP for command `cmd` (CMD_NONE = any); `all` = the
    /// all-party clause (param_5 > 0).
    pub fn hp_need(&self, t: &TargetStat, cmd: &str, all: bool) -> bool {
        let out_of_battle_ok = cmd == CMD_NONE || cmd == CMD_CURE;
        match self.style {
            CureStyle::Often => {
                if t.hp_pct() >= if self.in_battle { 35 } else { 50 } {
                    return false;
                }
                self.in_battle || out_of_battle_ok
            }
            CureStyle::Pinch => {
                if !(t.pinch() || t.dying) {
                    return false;
                }
                self.in_battle || out_of_battle_ok
            }
            CureStyle::Always => {
                if t.hp_pct() >= if self.in_battle { 50 } else { 80 } {
                    return false;
                }
                if !self.in_battle {
                    return out_of_battle_ok;
                }
                let pick = self.hp_pick(t);
                if pick == CMD_NONE {
                    return false;
                }
                if cmd == CMD_NONE || cmd == pick {
                    return true;
                }
                all && self.row(cmd).map(|r| r.all > 0).unwrap_or(false)
            }
        }
    }

    fn mp_basic(&self, t: &TargetStat) -> Option<bool> {
        let (mp, max) = t.mp?;
        let low = pct(mp, max) <= 24;
        Some(match self.style {
            CureStyle::Often => low && (!t.mp_charging || t.mp_charge_left >= 8.0),
            CureStyle::Pinch => t.mp_charging && t.mp_charge_left >= 8.0,
            CureStyle::Always => low && !(t.mp_charging && t.mp_charge_left < 8.0),
        })
    }

    /// `FUN_14126b550`: the MP need flag stored with a record.
    pub fn mp_flag(&self, t: &TargetStat) -> bool {
        self.mp_basic(t).unwrap_or(false)
    }

    /// `FUN_14126af20`.
    pub fn mp_need(&self, t: &TargetStat, cmd: &str, all: bool) -> bool {
        if !self.mp_basic(t).unwrap_or(false) {
            return false;
        }
        if self.style != CureStyle::Always {
            return true;
        }
        let pick = self.mp_pick(t);
        if pick == CMD_NONE {
            return false;
        }
        cmd == CMD_NONE || cmd == pick || all && self.row(cmd).map(|r| r.all > 0).unwrap_or(false)
    }

    /// The focus percentage of 0x141269f60 / 0x14126a1f0: int(focus / max x 100), None when it
    /// is negative (the game's `-1 < pct` guard: a need) .
    fn focus_low(f: i32, max: i32) -> bool {
        let pct = ((f as f32 / max as f32) * 100.0) as i32;
        pct < 0 || pct.min(100) <= 24
    }

    /// `FUN_14126a1f0` (the record's focus flag +0x2A7, C4): the player only (unique id match);
    /// OFTEN / ALWAYS: focus% <= 24; PINCH: the focus-break bit. Unknown gauge (host None): false.
    pub fn focus_flag(&self, t: &TargetStat) -> bool {
        let Some((f, max)) = t.focus.filter(|_| t.is_player) else { return false };
        if self.style == CureStyle::Pinch {
            return self.focus_break;
        }
        max > 0 && Self::focus_low(f, max)
    }

    /// `FUN_141269f60` (C3 / C4): as `focus_flag`, and ALWAYS also needs the fitting focus item
    /// (`focus_pick`, FUN_14125ef60) to exist and to be `cmd` when one is asked.
    pub fn focus_need(&self, t: &TargetStat, cmd: &str) -> bool {
        let Some((f, max)) = t.focus.filter(|_| t.is_player) else { return false };
        match self.style {
            CureStyle::Pinch => self.focus_break,
            CureStyle::Often => max > 0 && Self::focus_low(f, max),
            CureStyle::Always => {
                if !(max > 0 && Self::focus_low(f, max)) {
                    return false;
                }
                let pick = self.focus_pick(t);
                pick != CMD_NONE && (cmd == CMD_NONE || cmd == pick)
            }
        }
    }

    /// `FUN_14125ef60` (C4): over the commands of kinds 0x42..0x43 (FOCUSSUPPLEMENT /
    /// HIFOCUSSUPPLEMENT): amount = CeilToInt(float(max focus x m_RecoveryPoint / 100)) (integer
    /// division first), x the ITEM_UP rule FUN_141263950 when the curer has ITEM_UP (vt+0x14E0
    /// 0x28), halved (truncated) when the target is in TSID_STATE_DIE; the same fit as the HP /
    /// MP items on missing = max - focus.
    pub fn focus_pick(&self, t: &TargetStat) -> String {
        let Some((f, max)) = t.focus else { return CMD_NONE.to_string() };
        let missing = max - f;
        let mut best = CMD_NONE.to_string();
        let mut best_rem = missing;
        for cmd in &self.commands {
            if !(0x42..=0x43).contains(&kind_value(cmd)) {
                continue;
            }
            let pct = self.row(cmd).map(|r| r.recovery).unwrap_or(0);
            let mut amount = (((max * pct) / 100) as f32).ceil() as i32;
            if self.item_up > 0 {
                amount = (amount as f32 * (1.0 + 0.5 * self.item_up as f32)).ceil() as i32;
            }
            if t.dying {
                amount = (amount as f32 * 0.5) as i32;
            }
            let rem = missing - amount;
            let take = if rem < 1 {
                if best_rem < 1 {
                    rem.abs() < best_rem.abs()
                } else {
                    true
                }
            } else {
                rem < best_rem
            };
            if take {
                best_rem = rem;
                best = cmd.clone();
            }
        }
        best
    }

    /// `FUN_141269db0`.
    pub fn bad_need(&self, t: &TargetStat) -> bool {
        t.bad_status
    }

    fn fit(&self, missing: i32, kinds: impl Fn(i32) -> bool, cure: bool) -> String {
        let mut best = CMD_NONE.to_string();
        let mut best_rem = missing;
        for cmd in &self.commands {
            let v = kind_value(cmd);
            if !(kinds(v) || cure && v == 0x31) {
                continue;
            }
            let amount = match self.row(cmd) {
                None => 0,
                // FUN_141263950: items 0x3A..0x3D / 0x42..0x43 x (1 + 0.5 x ITEM_UP count) rounded up
                Some(r) if r.recovery >= 0 && ((0x3A..=0x3D).contains(&v) || (0x42..=0x43).contains(&v)) => (r.recovery as f32 * (1.0 + 0.5 * self.item_up as f32)).ceil() as i32,
                Some(r) if r.recovery >= 0 => r.recovery,
                Some(_) => self.cure_amount,
            };
            let rem = missing - amount;
            let take = if rem < 1 {
                if best_rem < 1 {
                    rem.abs() < best_rem.abs()
                } else {
                    true
                }
            } else {
                rem < best_rem
            };
            if take {
                best_rem = rem;
                best = cmd.clone();
            }
        }
        best
    }

    /// `FUN_14125f380`.
    pub fn hp_pick(&self, t: &TargetStat) -> String {
        let missing = t.hp_max - t.hp;
        if self.can_cure && t.hp_pct() < 50 && missing <= self.cure_amount {
            return CMD_CURE.to_string();
        }
        self.fit(missing, |v| (0x3A..=0x3C).contains(&v) || (0x40..=0x41).contains(&v), true)
    }

    /// `FUN_14125f7f0`.
    pub fn mp_pick(&self, t: &TargetStat) -> String {
        let Some((mp, max)) = t.mp else { return CMD_NONE.to_string() };
        self.fit(max - mp, |v| (0x3D..=0x41).contains(&v), false)
    }

    /// `FUN_141263e10`: the best target index and its score for `cmd`.
    pub fn select(&self, cmd: &str, targets: &[TargetStat], all: bool) -> Option<(usize, i32)> {
        let row = self.row(cmd);
        let mut best: Option<(usize, i32)> = None;
        let mut best_score = 0;
        for (i, t) in targets.iter().enumerate() {
            let mut s = 0;
            if let Some(r) = row {
                if self.bad_need(t) {
                    s = r.bad;
                }
                if self.hp_need(t, cmd, all) {
                    s += r.hp;
                }
                if self.mp_need(t, cmd, all) {
                    s += r.mp;
                }
                if self.focus_need(t, cmd) {
                    s += r.focus;
                }
                if all {
                    s += r.all;
                }
            }
            if best_score < s {
                best_score = s;
                best = Some((i, s));
            }
        }
        best
    }

    /// `FUN_141264e30` after the candidate list: (target index, command, score field, flags).
    pub fn decide(&self, targets: &[TargetStat]) -> Option<(usize, String, i32, [bool; 4])> {
        let n_hp = targets.iter().filter(|t| self.hp_need(t, CMD_NONE, false)).count();
        let n_mp = targets.iter().filter(|t| self.mp_need(t, CMD_NONE, false)).count();
        let focus = targets.iter().any(|t| self.focus_need(t, CMD_NONE));
        let bad = targets.iter().any(|t| self.bad_need(t));
        // vt+0x2928 (special cure) is false for both friends: bad status never asks for Cure
        let special = false;
        let style_ok = match self.style {
            CureStyle::Often | CureStyle::Pinch => true,
            CureStyle::Always => !self.in_battle,
        };
        if self.can_cure && style_ok && (n_hp > 0 || bad && special) {
            if let Some((i, s)) = self.select(CMD_CURE, targets, false) {
                let f = [self.hp_flag(&targets[i]), false, false, false];
                return Some((i, CMD_CURE.to_string(), s * 10, f));
            }
        }
        let commands: &[String] = if self.in_battle { &self.commands } else { &[] };
        let mut best: Option<(usize, usize, i32)> = None; // (command index, target, score)
        let mut best_score = 0;
        for (ci, cmd) in commands.iter().enumerate() {
            let v = kind_value(cmd);
            let all = match v {
                0x31 | 0x3A..=0x3C => {
                    if !(n_hp > 0 || bad && special) {
                        continue;
                    }
                    n_hp > 1
                }
                0x3D..=0x3F => {
                    if n_mp < 1 {
                        continue;
                    }
                    n_mp > 1
                }
                0x40 | 0x41 => {
                    if !(n_hp > 0 || n_mp > 0) {
                        continue;
                    }
                    n_hp >= 2 || n_mp >= 2
                }
                0x42 | 0x43 => {
                    if !focus {
                        continue;
                    }
                    false
                }
                0x44 => {
                    if !bad {
                        continue;
                    }
                    false
                }
                _ => continue,
            };
            if let Some((ti, s)) = self.select(cmd, targets, all) {
                if best_score < s {
                    best_score = s;
                    best = Some((ci, ti, s));
                }
            }
        }
        let (ci, ti, s) = best?;
        let cmd = commands[ci].clone();
        let t = &targets[ti];
        let mut f = [false; 4];
        match kind_value(&cmd) {
            0x31 | 0x3A..=0x3C => f[0] = self.hp_flag(t),
            0x40 | 0x41 => {
                f[0] = self.hp_flag(t);
                f[1] = self.mp_flag(t);
            }
            0x3D..=0x3F => f[1] = self.mp_flag(t),
            0x42 | 0x43 => f[2] = self.focus_flag(t),
            0x44 => f[3] = self.bad_need(t),
            _ => {}
        }
        Some((ti, cmd, ci as i32 + s * 10, f))
    }

    /// The record's flags still hold (the MetaAI tick's else-if chain).
    pub fn still_needed(&self, info: &CureInfo, t: &TargetStat) -> bool {
        if info.flags[0] {
            self.hp_flag(t)
        } else if info.flags[1] {
            self.mp_flag(t)
        } else if info.flags[2] {
            self.focus_flag(t)
        } else if info.flags[3] {
            self.bad_need(t)
        } else {
            true
        }
    }
}

const BAD_STATES: [&str; 4] = ["TSID_STATE_BADSTAT_STUN", "TSID_STATE_BADSTAT_FREEZE", "TSID_STATE_BADSTAT_BURN", "TSID_STATE_BADSTAT_SNEEZE"];

/// The stats of a candidate (self from the body, the player from the player view, the others from
/// their actor views).
pub fn target_stat(a: ActorRef, me: u8, body: &dyn AiBody, world: &dyn AiWorld) -> Option<TargetStat> {
    match a {
        ActorRef::Player => {
            let p = world.player();
            Some(TargetStat {
                actor: a,
                position: p.actor.position,
                hp: p.hp,
                hp_max: p.hp_max,
                mp: Some((p.mp, p.mp_max)),
                // the player view carries no recharge state: MP 0 = recharging, time unknown [D]
                mp_charging: p.mp <= 0 && p.mp_max > 0,
                mp_charge_left: f32::INFINITY,
                dying: p.dead,
                bad_status: BAD_STATES.contains(&p.actor.state),
                is_player: true,
                focus: world.player_focus(),
            })
        }
        ActorRef::Member(m) if m == me => {
            let b = body.view();
            let left = body.function("GetMPChargeTimeLeft").unwrap_or(f32::INFINITY);
            Some(TargetStat {
                actor: a,
                position: b.position,
                hp: b.hp,
                hp_max: b.hp_max,
                mp: Some((b.mp, b.mp_max)),
                mp_charging: b.mp_charging,
                mp_charge_left: left,
                dying: b.ko || b.state == "TSID_STATE_DIE",
                bad_status: BAD_STATES.contains(&b.state),
                is_player: false,
                focus: None,
            })
        }
        a => {
            let v = world.actor(a)?;
            // another member's MP (C3): AiWorld::member_mp (None keeps "no MP need")
            let mm = match a {
                ActorRef::Member(m) => world.member_mp(m),
                _ => None,
            };
            Some(TargetStat {
                actor: a,
                position: v.position,
                hp: v.hp.round() as i32,
                hp_max: v.hp_max.round() as i32,
                mp: mm.map(|(mp, max, _, _)| (mp, max)),
                mp_charging: mm.map(|x| x.2).unwrap_or(false),
                mp_charge_left: mm.map(|x| x.3).unwrap_or(f32::INFINITY),
                dying: !v.alive || v.state == "TSID_STATE_DIE",
                bad_status: BAD_STATES.contains(&v.state),
                is_player: false,
                focus: None,
            })
        }
    }
}

/// The candidate list of `FUN_141264e30` by the Recovery target bits.
pub fn candidates(me: u8, bits: u8, body: &dyn AiBody, world: &dyn AiWorld) -> Vec<TargetStat> {
    let mut out = Vec::new();
    if bits & CURE_TARGET_PLAYER != 0 {
        out.extend(target_stat(ActorRef::Player, me, body, world));
    }
    if bits & CURE_TARGET_SELF != 0 {
        out.extend(target_stat(ActorRef::Member(me), me, body, world));
    }
    if bits & CURE_TARGET_OTHERS != 0 {
        let mut al = Vec::new();
        world.allies(&mut al);
        for v in al.iter().filter(|v| v.actor != ActorRef::Member(me)).take(6) {
            out.extend(target_stat(v.actor, me, body, world));
        }
    }
    // (party AIX) a target without an HP pool (max HP 0: a host's stand-in, e.g. kh-core's placeholder status) is no
    // one to heal: it read as "dying" and drew a field Cure (the game has no such pawn)
    out.retain(|t| t.hp_max > 0);
    out
}

/// `FUN_141267740`: the battle flag with a hostile actor around (vt+0x28E8 0x141264840 lists the
/// hostile pawns).
pub fn in_battle(body: &dyn AiBody, world: &dyn AiWorld) -> bool {
    if !body.view().in_battle {
        return false;
    }
    let mut e = Vec::new();
    world.enemies(&mut e);
    e.iter().any(|v| v.alive && v.hostile)
}
