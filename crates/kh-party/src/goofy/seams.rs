//! (GOOFY) THE SEAMS OF THE LATER LANES (build/sora_tmp/party/GOOFY/CORE_SEAMS.md): the events the core emits and the views it
//! offers. GF-BREAK fills [`on_stagger`], GF-THREAT [`on_threat`], GF-UI reads [`hud`]. Each is one function so a lane changes
//! one place.

use super::parry::Tech;
use super::stagger::StaggerFacts;
use super::tune::K;
use super::{GoofyAbility, GoofyState};
use crate::host::PartyHost;
use crate::ids::MemberId;
use crate::party::Party;
use kh_core::host::{StaggerAsk, TargetId};

/// Why Stagger rose (5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaggerWhy {
    Basic,
    Deflect,
    /// The right technique against its attack.
    Technique,
    /// A hit of a Shield Art.
    Art,
    /// The debug word `x3g stagger` (test runs).
    Word,
}

/// Why threat rose (9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreatWhy {
    Basic,
    Guard,
    Deflect,
}

/// What Goofy's rules did this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GoofyEvent {
    /// Stagger `amount` on `target`.
    Stagger { target: TargetId, amount: f32, why: StaggerWhy },
    /// Threat `amount` on `target` (before [`threat_mult`]).
    Threat { target: TargetId, amount: f32, why: ThreatWhy },
    /// A parry worked (THE USER 2026-11-02: it opens no counter; its Stagger is the `Stagger` beside it).
    Parried { tech: Tech, right: bool, target: Option<TargetId> },
    /// GUARD broke.
    GuardBreak,
    /// A Stagger gauge filled and the host let the target break.
    Broken { target: TargetId },
    /// Goofy Breaker began: the target's BROKEN lasts `hold` seconds more at least.
    BreakerStarted { target: TargetId, hold: f32 },
    /// Party Breaker: the friends' turn.
    BreakerJoin { target: TargetId },
    /// The slam landed.
    BreakerLanded { target: TargetId, defeat: bool, launched: bool, rally: bool, dealt: f32 },
    /// (THE USER 2026-11-02: "make non-boss enemies bounce off of Goofy's shield ...") A perfect deflect turned `target`'s swing
    /// (`parry::bounces`): the party's step asks the host's rebound ([`on_bounce`]). In `Party::gof.events` only when the host
    /// took it.
    Bounce { target: TargetId },
}

/// GF-BREAK's seam: "Stagger N on target T". Only what the lock-on accepts has a gauge (a swing also breaks pots). A gauge that
/// fills asks the host (R6: its chain cap, its armour, an event may refuse); that one answer is the only way into BROKEN.
pub fn on_stagger(party: &mut Party, from: MemberId, target: TargetId, amount: f32, _why: StaggerWhy, host: &mut PartyHost) {
    let Some(info) = host.targets.info(target).filter(crate::pad::goofy_shots::lockable) else { return };
    let facts = StaggerFacts::of(&*host.targets, &info);
    if !party.gof.book.add(target, amount, facts) {
        return;
    }
    if !host.targets.stagger_ask(target, StaggerAsk::Break { window: facts.window() }) {
        party.gof.book.refused(target);
        return;
    }
    party.gof.book.open(target);
    party.gof.events.push((from, GoofyEvent::Broken { target }));
    if let Some(m) = party.member_mut(from) {
        crate::pad::goofy_shots::effect(m, host, K.stagger_break_effect, info.center, 1.0);
        crate::pad::goofy_shots::sound(m, host, K.stagger_break_se, info.center);
    }
}

/// (THE USER 2026-11-02: "make non-boss enemies bounce off of Goofy's shield if they're not doing a long combo string when Goofy
/// successfully performs a perfect deflect") THE BOUNCE'S SEAM: the swing of `target` that a perfect deflect turned is knocked back
/// by the host's own rebound (`StaggerAsk::Rebound`). Only on what the lock-on accepts; never on a boss (the host's word,
/// `Targets::size_class`: in Elden Ring the game's boss bar), nor on a body the Stagger book holds BROKEN (its kneel is the bigger
/// reaction, and this blow may have just filled the gauge). Bosses keep the old rules: the Stagger alone.
/// True: the host took it.
pub fn on_bounce(party: &Party, target: TargetId, host: &mut PartyHost) -> bool {
    let Some(info) = host.targets.info(target).filter(crate::pad::goofy_shots::lockable) else { return false };
    if StaggerFacts::of(&*host.targets, &info).boss || party.gof.book.view(target).is_some_and(|v| v.broken.is_some()) {
        return false;
    }
    host.targets.stagger_ask(target, StaggerAsk::Rebound { wait: K.deflect_bounce_wait })
}

/// GF-THREAT's seam: "threat N on target T", scaled by [`threat_mult`], on what the lock-on accepts (threat.rs). A guard or a
/// deflect also reaches the hosts' director as a zero-damage touch, and raises Donald's Taunt Pass.
pub fn on_threat(party: &mut Party, from: MemberId, target: TargetId, amount: f32, why: ThreatWhy, host: &mut PartyHost) {
    if host.targets.info(target).filter(crate::pad::goofy_shots::lockable).is_none() {
        return;
    }
    let mult = party.status(from).and_then(|s| s.goofy.as_ref()).map_or(1.0, threat_mult);
    party.gof.threat.add(target, from, amount * mult);
    if why != ThreatWhy::Basic {
        party.gof.threat.note_stance(from, target);
        crate::donald::ai::note_guard(party, from);
    }
}

/// R15's weight of his threat: x1.5, x3 at Resolve 2+, Loud Mouth on top.
pub fn threat_mult(s: &GoofyState) -> f32 {
    let base = if super::threat::strong(s) { K.threat_resolve_mult } else { K.threat_base_mult };
    if s.abilities.has(GoofyAbility::LoudMouth) {
        base * K.threat_loud_mouth
    } else {
        base
    }
}

/// Danger Sense: how much earlier the red "!" shows (0 without it).
pub fn danger_lead(s: &GoofyState) -> f32 {
    if s.abilities.has(GoofyAbility::DangerSense) {
        K.danger_lead
    } else {
        0.0
    }
}

/// Danger Sense: the red "!" also names the kind of attack under it (false without it: the plain "!" of 4.3, which never
/// says which). (THE USER 2026-11-02: "Danger Sense ability should indicate what kind of attack it will be")
pub fn danger_names_kind(s: &GoofyState) -> bool {
    s.abilities.has(GoofyAbility::DangerSense)
}

/// Breaker damage from Resolve (In the Zone: +50 %). GF-BREAK multiplies it in.
pub fn breaker_mult(s: &GoofyState) -> f32 {
    if s.core.resolve.in_zone() {
        K.resolve_zone_breaker
    } else {
        1.0
    }
}

/// What the HUD draws (Resolve pips). GF-UI reads it through `Party::goofy_hud`. (THE USER 2026-11-02: "Remove countering with
/// 'A' after a parry technique") no counter window, chain or free Art to show any more.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct GoofyHud {
    /// Resolve in thirds, 0..=9.
    pub thirds: u8,
    pub in_zone: bool,
    pub hop_cd: f32,
}

/// His HUD view.
pub fn hud(s: &GoofyState) -> GoofyHud {
    let c = &s.core;
    GoofyHud { thirds: c.resolve.thirds, in_zone: c.resolve.in_zone(), hop_cd: c.hop_cd }
}
