//! (THE USER 2026-11-02: "let Goofy use his last MP and go into MP reserve, make sure Donald and his spells (including
//! Zettaflare) have this as well. Zettaflare should instantly put Donald into MP reserve") THE PARTY'S MP RULE: one rule
//! for every MP move of every party member (Goofy's Shield Arts and Taunt, Donald's spells and Arcane spells, KH3's own
//! friend abilities, and whoever joins later). "MP reserve" is KH3's MP Charge.
//!
//! KH3's Sora (kh-core `Status::can_cast` / `use_mp`, `UseMagicPoint` 0x1405BCDC0) casts whenever his MP is not charging:
//! a spell dearer than what he has left takes all of it and starts MP Charge. The party's version:
//! - a move that costs MP may start when he has MP above 0 and MP Charge is not running (its other gates, cooldowns,
//!   learned, ground ... are the move's own);
//! - it spends its price; a price at or above what he has takes ALL of it: MP 0 and MP Charge starts at once. (Sora's
//!   exact landing on 0 waits for the next cast to start the charge; a friend's next cast needs MP above 0, so for him 0
//!   starts it, as the friends' KH3 gate always did, `FriendStatus::pay`);
//! - MP Charge: his MP stays 0 for [`charge_time`] (the character's own row: `n_ex00x.mp_charge_time`, 30 s, Donald's
//!   R7 length), then the bar is full again (`FriendStatus::tick`); nothing that costs MP starts meanwhile;
//! - so a short-MP refusal (KH3's refused / "error" sound) happens only in MP Charge or at 0 MP;
//! - a move whose price is 0 (a free Art, Spell Echo) is never refused by MP.

use crate::status::FriendStatus;

/// Why an MP move may not start now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Any price may be paid.
    Ready,
    /// MP Charge is running.
    Charging,
    /// MP 0 (and no charge running: a fresh status, a host that set it).
    Empty,
}

/// The MP gate now.
pub fn gate(s: &FriendStatus) -> Gate {
    if s.mp_charge_left > 0.0 {
        Gate::Charging
    } else if s.mp <= 0 {
        Gate::Empty
    } else {
        Gate::Ready
    }
}

/// He may pay any MP price now (MP above 0, no MP Charge).
pub fn can_spend(s: &FriendStatus) -> bool {
    gate(s) == Gate::Ready
}

/// A move of this price may start now by MP (`cost` 0: free, always; below 0: all of his MP, KH3's -1).
pub fn may_pay(s: &FriendStatus, cost: i32) -> bool {
    cost == 0 || can_spend(s)
}

/// A move of `cost` would take the last of `mp` and start MP Charge (`cost` below 0: all of it).
pub const fn drains(mp: i32, cost: i32) -> bool {
    cost < 0 || (cost > 0 && mp <= cost)
}

/// What a spend did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Spent {
    /// MP taken (at most what he had).
    pub mp: i32,
    /// MP Charge started with it.
    pub charge_started: bool,
}

/// PAYS `cost` by the party's MP rule: None = refused (MP Charge running, or MP 0 with a price above 0); the price 0 costs
/// nothing and is never refused. A price at or above his MP (or below 0: all of it) leaves 0 and starts MP Charge.
pub fn spend(s: &mut FriendStatus, cost: i32) -> Option<Spent> {
    if cost == 0 {
        return Some(Spent::default());
    }
    if !can_spend(s) {
        return None;
    }
    let before = s.mp;
    if drains(before, cost) {
        start_charge(s);
        return Some(Spent { mp: before, charge_started: true });
    }
    s.mp = before - cost;
    Some(Spent { mp: cost, charge_started: false })
}

/// (THE USER 2026-11-02: "Zettaflare should instantly put Donald into MP reserve") PAYS A MOVE THAT TAKES ALL OF HIS MP,
/// whatever is left: MP 0 and MP Charge at once. None = refused (MP Charge running, or MP 0).
pub fn spend_all(s: &mut FriendStatus) -> Option<Spent> {
    spend(s, -1)
}

/// MP 0 and MP Charge starts now ([`charge_time`]); a running charge is left as it is. With no charge time (a row of 0)
/// the bar refills at once (KH3's `SetMpCharge` 0x1405B9C60 takes its off path).
pub fn start_charge(s: &mut FriendStatus) {
    if s.mp_charge_left > 0.0 {
        s.mp = 0;
        return;
    }
    let t = charge_time(s);
    if t > 0.0 {
        s.mp = 0;
        s.mp_charge_left = t;
    } else {
        s.mp = s.max_mp();
    }
}

/// HIS MP CHARGE LENGTH (s, before MP Haste): Donald's R7 length (`pad::donald::mp_charge_time`: the friends' 30 s x
/// clamp(Sora's Magic / his, 0.6, 1)), anyone else his own row `n_ex00x.mp_charge_time` (KH3's `MyMpChargeTime`, 30 s).
pub fn charge_time(s: &FriendStatus) -> f32 {
    if s.donald.is_some() {
        crate::pad::donald::mp_charge_time(s)
    } else {
        crate::tables::cparam(s.chara, "mp_charge_time")
    }
}

/// MP Charge's progress 0..1 (0 when none runs): how far the bar has refilled.
pub fn charge_progress(s: &FriendStatus) -> f32 {
    if s.mp_charge_left <= 0.0 {
        return 0.0;
    }
    (1.0 - s.mp_charge_left / charge_time(s).max(1e-3)).clamp(0.0, 1.0)
}

/// THE HUD'S CHARGE VALUE (`vt+0x17F8`, kh-core `Status::mp_charge_gauge`: `timer / T x maxMP`, the MP still to refill)
/// from a member's charge progress (`MemberView::mp_charge`) and max MP; 0 when no charge runs. KH3's status HUD draws its
/// pink MP Charge gauge from this number on the leader's bar and on the friends' small ones alike.
pub fn hud_charge_value(charging: bool, progress: f32, mp_max: i32) -> f32 {
    if !charging {
        return 0.0;
    }
    (1.0 - progress.clamp(0.0, 1.0)) * mp_max.max(0) as f32
}

#[cfg(test)]
#[path = "mp_rule_tests.rs"]
mod tests;
