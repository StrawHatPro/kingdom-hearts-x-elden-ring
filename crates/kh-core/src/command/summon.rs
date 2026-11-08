//! (THE USER 2026-11-02: "Give Sora the ability to 'Summon' from the command menu, but he uses Elden Ring's spirit ashes
//! instead of his KH3 summons. Use the summon animations for Sora and camera work from KH3 when initiating the summon. In
//! the equipment menu for Sora, make a new area for him to equip spirit ashes (4) and these equipped spirit ashes are what
//! appears in the Summon command menu. Summon is just greyed out if nothing is equipped. Summoning is also only available
//! when you get the spirit calling bell.") THE SUMMON COMMAND: the rules, as plain functions the menu, the host and the
//! tests share. Numbers: `sheets/port/sora-magic-mp.json` rows `summon.*` (PORT DESIGN, citing the user).
//!
//! THE SLOT: KH3's fourth main command, Link (`kh3_system.command.link`, list 3; the port had it hidden for Sora,
//! `link_none`). Its text here is "Summon" / "Unsummon" (COMPOSED: KH3 has no such word; its slot says "Link").
//! Command kinds 248 .. 251 (the equipped slots 0 .. 3) and 252 (Unsummon) are `ETresCommandKind::Reserve248 .. 252`,
//! values KH3 never uses.
//!
//! WHO HOLDS WHAT:
//! - the core (this file and `command/mod.rs`): the equipped ashes (`CommandState::spirit_ashes`, the base goods ids:
//!   saved with Sora's KH save), the menu's rows and greying, the MP payment (KH3's `UseMagicPoint`: an overspend goes
//!   into MP Charge), Sora's chant (`states/summon_chant.rs`: KH3's Link call `M_1571` with KH3's Link camera), the
//!   Unsummon refund;
//! - the host (Elden Ring: `kh-er` spirit.rs): the inventory (which ashes he owns, their level, cost, name), the Spirit
//!   Calling Bell, the spirit itself (ER's own buddy spawn), its timer, HP and leaving. It fills [`SummonView`] every
//!   frame and carries out the core's [`SummonPhase`] events.
//!
//! THE USER'S CHOICES (asked and answered, 2026-11-02): COST in MP converted from the ash's FP (or HP) cost ([`mp_cost`]);
//! anywhere (no Rebirth Monument); the spirit is an extra ally beside the party; one at a time; the upgrade level is the
//! ash's own; a timer ([`duration`]) that the upgrade lengthens; "Unsummon" refunds the unused share ([`refund`]).

use crate::magic::tables::Sheet;
use crate::status::Status;

/// The command kind of equipped slot 0 (slots 0 .. 3 = 248 .. 251).
pub const KIND_SUMMON: u8 = 248;
/// The command kind of "Unsummon".
pub const KIND_UNSUMMON: u8 = 252;
/// Equipped slots (THE USER: "a new area for him to equip spirit ashes (4)").
pub const SLOTS: usize = 4;
/// The main row's texts (COMPOSED; KH3's slot says "Link").
pub const TEXT_SUMMON: &str = "Summon";
pub const TEXT_UNSUMMON: &str = "Unsummon";

/// The slot of a summon command kind.
pub fn slot_of_kind(kind: u8) -> Option<usize> {
    (KIND_SUMMON..KIND_SUMMON + SLOTS as u8).contains(&kind).then(|| (kind - KIND_SUMMON) as usize)
}

/// A summon kind (an equipped slot or Unsummon).
pub fn is_summon_kind(kind: u8) -> bool {
    (KIND_SUMMON..=KIND_UNSUMMON).contains(&kind)
}

/// The ash's upgrade level from its goods id (Elden Ring: one goods row per level, `base + level`; the game's buddy
/// manager takes `level = id % 100`, build/sora_tmp/party/SUMMON/IMPL_SUMMON_RE.md section 3).
pub fn level_of(goods: u32) -> u8 {
    (goods % 100).min(10) as u8
}

/// The ash's level-0 goods id: what the equipped slot keeps, so an upgrade at Roderika (the goods row of the next level
/// replaces the old one) keeps the slot.
pub fn base_of(goods: u32) -> u32 {
    goods - goods % 100
}

/// One equipped ash as the host resolved it from the inventory this frame.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AshSlot {
    /// The goods id he owns now (its level included).
    pub goods: u32,
    pub level: u8,
    /// The game's name, "+N" added by the host when N > 0.
    pub name: String,
    /// The MP it costs ([`mp_cost`] of its Elden Ring cost).
    pub mp: i32,
    /// A renowned ash (Elden Ring goods type 8, upgraded with Ghost Gloveworts): the shorter timer rows.
    pub renowned: bool,
    /// The Mimic Tear: summons Mimic Sora (kh-er), not Elden Ring's copy.
    pub mimic: bool,
    /// The host can summon it now (`false`: an ash the game would only call beside a monument, the multiplayer rule).
    pub usable: bool,
    /// (THE USER 2026-11-03: "For the spirit ashes HUD picture, use the Elden Ring picture of the spirit ash.") Its Elden Ring
    /// icon (`EquipParamGoods.iconId`; the shared route: `kh_ui::item_art::er_icon_key`); `None`: not known.
    pub icon: Option<u32>,
}

/// The spirit that is out.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SpiritOut {
    pub slot: u8,
    pub name: String,
    /// The MP paid for it (the refund's base).
    pub cost: i32,
    /// Seconds left of its timer, and the timer's full length.
    pub left: f32,
    pub full: f32,
    pub hp: i32,
    pub hp_max: i32,
    pub mimic: bool,
    /// (THE USER 2026-11-03) the ash's Elden Ring icon (as `AshSlot::icon`; Mimic Sora: the Mimic Tear's).
    pub icon: Option<u32>,
}

/// What the host tells the core every frame.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SummonView {
    /// This Sora may summon at all (the player's Sora with a host that has spirits; AI Sora and Mimic Sora: false, no row).
    pub allowed: bool,
    /// The Spirit Calling Bell is owned (Elden Ring's key item, read at run time).
    pub bell: bool,
    /// The equipped slots, resolved (`None`: empty, or an ash he no longer owns).
    pub slots: [Option<AshSlot>; SLOTS],
    /// The spirit that is out (one at a time, as Elden Ring).
    pub out: Option<SpiritOut>,
    /// The chant runs (from the press to the spawn): no second summon, no Unsummon.
    pub calling: bool,
    /// Every spirit ash he owns now (one entry per goods row he has; the camp Equipment screen's stock for the four
    /// spirit-ash slots).
    pub owned: Vec<AshSlot>,
    /// (THE USER 2026-11-03: "have the spirit ashes HUD element fade away after they either die or time out") THE SPIRIT
    /// THAT JUST LEFT and why, with its final values ([`final_values`]: HP 0 when it died, no time when it timed out),
    /// kept by the host for [`LEAVING_HOLD`] seconds after the leave (or until a new spirit is out) so the HUD's gauge
    /// can fade out on them. `None` for a leave with nothing to show (a load, the mode's end: `LeaveReason::Gone`).
    pub leaving: Option<(SpiritOut, LeaveReason)>,
}

/// How long the host keeps [`SummonView::leaving`] (seconds; longer than the HUD's fade).
pub const LEAVING_HOLD: f32 = 1.5;

/// The values the gauge fades out on: a spirit that died shows no HP, one that timed out no time; Unsummon and the save
/// point keep what they had (PORT DESIGN: those fade the same way, for one KH3 look of a spirit leaving). `None`: a leave
/// that shows nothing (`Gone`: a load, a map change, the mode's end; the gauge goes at once with the HUD's reset).
pub fn final_values(out: &SpiritOut, reason: LeaveReason) -> Option<SpiritOut> {
    let mut last = out.clone();
    match reason {
        LeaveReason::Died => last.hp = 0,
        LeaveReason::Timeout => last.left = 0.0,
        LeaveReason::Unsummon | LeaveReason::SavePoint => {}
        LeaveReason::Gone => return None,
    }
    Some(last)
}

/// What the core asks of the host (an [`crate::host::EventKind::Summon`] event).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SummonPhase {
    /// The command was accepted and paid: Sora's chant begins (KH3's Link camera starts on it).
    Chant,
    /// The chant reached its call: the host summons the slot's ash now.
    Spawn,
    /// "Unsummon": the host ends the spirit now; `refund` MP were given back already.
    Unsummon,
}

/// Why a spirit left (the host's; only Unsummon refunds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaveReason {
    Unsummon,
    Timeout,
    Died,
    /// The KH save point's menu (THE USER 2026-11-02: "going into the save point menu instantly ends the summon"), or Elden
    /// Ring's own rest at a grace.
    SavePoint,
    /// A load, a map change, the mode's end, the game ending it by its own rules (distance, area).
    Gone,
}

impl LeaveReason {
    pub fn refunds(self) -> bool {
        self == LeaveReason::Unsummon
    }

    pub fn name(self) -> &'static str {
        match self {
            LeaveReason::Unsummon => "Unsummon",
            LeaveReason::Timeout => "its time ran out",
            LeaveReason::Died => "it died",
            LeaveReason::SavePoint => "the save point",
            LeaveReason::Gone => "it left",
        }
    }
}

// ============================================================================================ the numbers

/// THE MP CONVERSION (THE USER 2026-11-02, asked: "MP, converted from each ash's Elden Ring FP cost (cheap ashes cheap,
/// Mimic Tear expensive)"): an FP ash costs `ceil(FP x summon.mp.per_fp)`, an HP ash (`consumeHP`, the Mimic Tear's 660
/// and the jellyfish-like few) `ceil(HP x summon.mp.per_hp)`, at least `summon.mp.min`. Elden Ring's FP and Sora's MP
/// are both "about 100 early, a few hundred late"; half the FP keeps a spell-sized cheap ash (11 FP -> 6 MP) and makes a
/// renowned one a large part of his bar (Black Knife Tiche 132 FP -> 66 MP); the Mimic Tear's 660 HP -> 99 MP.
/// `fp` / `hp` < 1 = the game has no such cost (`-1` in EquipParamGoods).
pub fn mp_cost(fp: i32, hp: i32) -> i32 {
    let min = Sheet::Mp.int("summon.mp.min");
    let raw = if fp > 0 {
        (fp as f32 * Sheet::Mp.num("summon.mp.per_fp") - 1.0e-3).ceil() as i32
    } else if hp > 0 {
        (hp as f32 * Sheet::Mp.num("summon.mp.per_hp") - 1.0e-3).ceil() as i32
    } else {
        0
    };
    raw.max(min)
}

/// (THE USER 2026-11-03: "mimic tear should make Sora instantly go into MP recharge") THE MIMIC TEAR TAKES ALL HIS MP: its
/// summon empties the bar and starts MP Charge at once, whatever he had (as Donald's Zettaflare does), instead of a
/// converted cost. It needs MP above 0 and no MP Charge running; MP Safety never refuses it (an "all MP" cost is never
/// short, as KH3's Cure, cost 255). The menus show "All" for it (COMPOSED: [`TEXT_ALL_MP`]).
pub fn takes_all(ash: &AshSlot) -> bool {
    ash.mimic
}

/// The cost text of an "all MP" ash (COMPOSED).
pub const TEXT_ALL_MP: &str = "All";

/// What a summon of `ash` pays now: (MP taken, the refund's base). An ordinary ash: its cost after MP Thrift (KH3's
/// `EffCost`), both the same. The Mimic Tear: everything; PORT DESIGN (the coordinator's question): Unsummon refunds the
/// share of the time left of THE MP HE HAD when he called it (100 % time left = that MP back), given as charge progress
/// since he is always in MP Charge then ([`give_refund`]): calling it and sending it straight back costs nothing but the
/// chant, as for every other ash.
pub fn pay_now(ash: &AshSlot, status: &Status) -> (i32, i32) {
    if takes_all(ash) {
        (status.mp, status.mp)
    } else {
        let cost = status.effective_mp_cost(ash.mp, 0);
        (cost, cost)
    }
}

/// THE TIMER (THE USER 2026-11-02: "Give spirit ashes a timer, upgrading the spirit ash increases the timer as well as
/// whatever else gets upgraded normally"): `base + level x per_level` seconds, by the ash's tier (an ordinary ash:
/// `summon.time.*.normal`; a renowned one, Ghost-Glovewort upgraded, Elden Ring goods type 8: `summon.time.*.renowned`).
/// The level's other gains are Elden Ring's own (the buddy's `dopingSpEffect_lv<N>`).
pub fn duration(renowned: bool, level: u8) -> f32 {
    let tier = if renowned { "renowned" } else { "normal" };
    Sheet::Mp.num(&format!("summon.time.base.{tier}")) + level.min(10) as f32 * Sheet::Mp.num(&format!("summon.time.per_level.{tier}"))
}

/// THE REFUND (THE USER 2026-11-02: "pressing it just immediately ends the summon and gives back a refund of MP for how
/// long the timer had left (if 100% of the timer remained then Sora would get back 100% MP, if he ended it at 0% of timer
/// remaining he gets 0% MP back)"): read literally, that share of the summon's MP cost, rounded down.
pub fn refund(cost: i32, left: f32, full: f32) -> i32 {
    if cost <= 0 || full <= 0.0 {
        return 0;
    }
    (cost as f32 * (left / full).clamp(0.0, 1.0) + 1.0e-4).floor() as i32
}

/// What a refund did to his MP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refunded {
    /// MP rose by this much (capped at his maximum).
    Mp(i32),
    /// He was in MP Charge: the charge was shortened by this many MP units (its gauge counts MP still to refill); `ended`:
    /// that finished the charge (full MP, as the charge's own end).
    Charge { units: i32, ended: bool },
    Nothing,
}

/// Gives a refund back. PORT DESIGN (the coordinator's question "if the summon put him into MP Charge"): KH3's MP Charge
/// ignores MP writes (`SetMagicPoint` does nothing while charging) and refills everything at its end; the refund is
/// given as charge progress instead: the timer loses `refund / max MP` of the charge time (the HUD's charge gauge counts
/// MP units still to refill, `mp_charge_gauge`), so the MP he gets back is worth the same either way, and a large refund
/// can end the charge at once (full MP).
pub fn give_refund(status: &mut Status, mp: i32) -> Refunded {
    if mp <= 0 || status.placeholder {
        return Refunded::Nothing;
    }
    let max = status.max_mp().max(1);
    if status.mp_charging {
        let units = mp.min(max);
        status.mp_charge_timer -= units as f32 / max as f32 * status.mp_charge_time();
        let ended = status.mp_charge_timer <= 0.0;
        if ended {
            status.set_mp_charge(false, 1.0);
        }
        return Refunded::Charge { units, ended };
    }
    let before = status.mp;
    status.set_mp(before + mp);
    status.refresh();
    Refunded::Mp(status.mp - before)
}

/// The spirit's timer (the host runs it; `tick` true = its time ran out this step).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Timer {
    pub full: f32,
    pub left: f32,
}

impl Timer {
    pub fn new(full: f32) -> Timer {
        Timer { full, left: full }
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        let was = self.left;
        self.left = (self.left - dt.max(0.0)).max(0.0);
        was > 0.0 && self.left <= 0.0
    }

    /// The share left, 0 .. 1 (the HUD's draining gauge).
    pub fn ratio(&self) -> f32 {
        if self.full <= 0.0 {
            0.0
        } else {
            (self.left / self.full).clamp(0.0, 1.0)
        }
    }
}

// ============================================================================================ the menu's rules

/// Why the Summon row is greyed (`None`: it is lit).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grey {
    NotAllowed,
    /// THE USER: "Summoning is also only available when you get the spirit calling bell."
    NoBell,
    /// THE USER: "Summon is just greyed out if nothing is equipped."
    NothingEquipped,
    /// MP Charge runs (KH3: no MP command while charging).
    MpCharge,
    /// The chant runs.
    Calling,
}

/// The main row's state: its text and why it is grey. While a spirit is out the row reads "Unsummon" and is lit
/// (THE USER 2026-11-02: "While a spirit ash is summoned, the 'Summon' command menu changes to 'Unsummon'"), also in MP
/// Charge (it gives MP back).
pub fn main_row(view: &SummonView, status: &Status) -> (&'static str, Option<Grey>) {
    if !view.allowed {
        return (TEXT_SUMMON, Some(Grey::NotAllowed));
    }
    if view.out.is_some() {
        return (TEXT_UNSUMMON, if view.calling { Some(Grey::Calling) } else { None });
    }
    let grey = if !view.bell {
        Some(Grey::NoBell)
    } else if view.slots.iter().all(Option::is_none) {
        Some(Grey::NothingEquipped)
    } else if view.calling {
        Some(Grey::Calling)
    } else if status.mp_charging {
        Some(Grey::MpCharge)
    } else {
        None
    };
    (TEXT_SUMMON, grey)
}

/// One row of the Summon list: lit unless MP Charge runs or the host cannot summon it; `mp_short` (recoloured, KH3's
/// "col2") when the cost is above his MP (it can still be called: the overspend starts MP Charge, as a spell's would,
/// unless MP Safety refuses it).
pub fn list_row(ash: &AshSlot, status: &Status) -> (bool, bool) {
    if takes_all(ash) {
        // (THE USER 2026-11-03) the Mimic Tear: any MP above 0, never "short"
        return (ash.usable && !status.mp_charging && status.mp >= 1, false);
    }
    let short = ash.mp > status.mp;
    let safety = status.has(crate::enums::ETresAbilityKind::MpSafety) && short && !(status.has(crate::enums::ETresAbilityKind::MpLeave) && status.mp >= 1);
    (ash.usable && !status.mp_charging && !safety, short)
}

// ============================================================================================ out of combat: follow

/// What a summoned spirit does this frame (THE USER 2026-11-02: "When summoned and out of combat, the spirit ashes should
/// follow Sora like his party allies.").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowPlan {
    /// In battle: its own Elden Ring AI fights.
    Fight,
    /// Out of battle and near its place: nothing to do.
    Stay,
    /// Out of battle and away from its place behind the leader: go there (the game's own move order).
    Follow,
    /// Too far (KH3's `m_WarpStartDistance`), or no path for a while: the game's own teleport to its place.
    Warp,
}

/// The follow decision. `in_battle`: the HUD's battle rule (an engaged enemy, with its linger); `dist`: metres from the
/// spirit to its place behind the leader; `stuck_s`: seconds it has made no progress toward it. Rows `summon.follow.*`:
/// KH3's friends' follow definition (`m_RangeSamePlayerSpeed` 300 cm: the stop band; `m_WarpStartDistance` 3000 cm).
pub fn follow_plan(in_battle: bool, dist: f32, stuck_s: f32) -> FollowPlan {
    if in_battle {
        return FollowPlan::Fight;
    }
    if dist >= Sheet::Mp.num("summon.follow.warp_m") || (dist > Sheet::Mp.num("summon.follow.start_m") && stuck_s >= Sheet::Mp.num("summon.follow.stuck_s")) {
        FollowPlan::Warp
    } else if dist > Sheet::Mp.num("summon.follow.start_m") {
        FollowPlan::Follow
    } else {
        FollowPlan::Stay
    }
}

/// The spirit's place behind the leader (leader position, leader yaw in radians, Elden Ring's up = +y): straight behind,
/// `summon.follow.back_m` back (the friends take the leader's sides in KH3's formation; behind is free of them).
pub fn follow_place(leader: glam::Vec3, yaw: f32) -> glam::Vec3 {
    let back = Sheet::Mp.num("summon.follow.back_m");
    leader - glam::Vec3::new(yaw.sin(), 0.0, yaw.cos()) * back
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summon_the_mp_conversion_is_half_the_fp_and_the_mimic_is_expensive() {
        // Elden Ring 2.7.2.0 regulation, EquipParamGoods consumeMP / consumeHP (read offline, build/sora_tmp/party/SUMMON)
        assert_eq!(mp_cost(11, -1), 6, "the cheapest FP ash (row 241000, 11 FP)");
        assert_eq!(mp_cost(132, -1), 66, "Black Knife Tiche (row 200000, 132 FP)");
        assert_eq!(mp_cost(-1, 660), 99, "the Mimic Tear (row 207000, 660 HP)");
        assert_eq!(mp_cost(0, -1), 1, "a cost of 0 still costs the minimum");
    }

    #[test]
    fn summon_the_timer_grows_with_the_level_by_tier() {
        assert_eq!(duration(false, 0), 60.0);
        assert_eq!(duration(false, 10), 120.0);
        assert_eq!(duration(true, 0), 45.0);
        assert_eq!(duration(true, 10), 90.0);
        assert_eq!(duration(true, 99), duration(true, 10), "levels stop at +10");
        assert!(duration(false, 5) > duration(false, 4));
        let mut t = Timer::new(2.0);
        assert!(!t.tick(1.0) && (t.ratio() - 0.5).abs() < 1e-6);
        assert!(t.tick(1.5), "the step that empties it says so");
        assert!(!t.tick(1.0), "only once");
    }

    #[test]
    fn summon_refund_at_full_half_and_no_time_left() {
        assert_eq!(refund(40, 60.0, 60.0), 40);
        assert_eq!(refund(40, 30.0, 60.0), 20);
        assert_eq!(refund(41, 30.0, 60.0), 20, "rounded down");
        assert_eq!(refund(40, 0.0, 60.0), 0);
        assert_eq!(level_of(200005), 5);
        assert_eq!(base_of(200005), 200000);
        assert_eq!(slot_of_kind(250), Some(2));
        assert_eq!(slot_of_kind(KIND_UNSUMMON), None);
    }

    #[test]
    fn summon_the_gauge_fades_out_on_the_final_values() {
        let out = SpiritOut { hp: 300, hp_max: 500, left: 12.0, full: 60.0, ..SpiritOut::default() };
        assert_eq!(final_values(&out, LeaveReason::Died).map(|o| (o.hp, o.left)), Some((0, 12.0)), "death: no HP");
        assert_eq!(final_values(&out, LeaveReason::Timeout).map(|o| (o.hp, o.left)), Some((300, 0.0)), "timeout: no time");
        assert_eq!(final_values(&out, LeaveReason::Unsummon).map(|o| (o.hp, o.left)), Some((300, 12.0)));
        assert!(final_values(&out, LeaveReason::Gone).is_none(), "a load shows nothing");
    }

    #[test]
    fn summon_follow_plan_in_and_out_of_battle() {
        assert_eq!(follow_plan(true, 100.0, 10.0), FollowPlan::Fight);
        assert_eq!(follow_plan(false, 1.0, 0.0), FollowPlan::Stay);
        assert_eq!(follow_plan(false, 8.0, 0.0), FollowPlan::Follow);
        assert_eq!(follow_plan(false, 31.0, 0.0), FollowPlan::Warp, "KH3's warp start distance, 30 m");
        assert_eq!(follow_plan(false, 8.0, 6.0), FollowPlan::Warp, "no progress for a while");
        let p = follow_place(glam::Vec3::ZERO, 0.0);
        assert!((p.z + 3.5).abs() < 1e-5 && p.x.abs() < 1e-5, "behind the leader: {p}");
    }
}
