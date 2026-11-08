//! (REVENGE, 2026-11-07) A HOST-ASKED COUNTER: attack kinds the host wants played one after the other at a target, the
//! Mimic Tear boss copies' answer to a full revenge value (kh-er mimicrevenge.rs; the user: "Mimic Sora should counter with
//! Sonic Blade and zantetsuken, unless he's in a form change where the keyblade has been turned into a completely different
//! weapon than he does an appropriate move from the form's moveset"). Nothing here runs unless a host asks
//! ([`crate::Sora::request_counter`]); KH3's own rules are untouched.
//!
//! HOW IT PLAYS (no new attack code: KH3's own request path):
//! - Each step while a kind is waiting and Sora is in a FREE state (idle, run, jump, glide, floating, dodge roll, guard,
//!   air slide: never in an attack, a reaction or a command), the controller update makes the attack request
//!   (`TryAttack`) at the counter's target, as an Attack press would.
//! - The request takes the waiting kind as its FORCED kind ([`take`], from `combat::guard::forced_attack`, the slot KH3's
//!   own guard / blow counters use), so the attack class of the held form plays it through its own tables
//!   (`class_state(cmd, Attack { forced_kind })`): a form's own move (`own_form`).
//! - A BLADE counter (`own_form` false: the XS kinds of Sonic Blade 0xE0 / 0xE1 and Zantetsuken 0xE4) skips the held form's
//!   own request ([`skips_form`]): the plain attack state plays it (PlayAttack's XS tail, abilities/xs).
//! - Zantetsuken waits for the ground (its finisher is a ground one). Without progress for [`GIVE_UP`] seconds (refused
//!   requests, a reaction, a KO) the rest is dropped.

use crate::combat::guard::CounterOffer;
use crate::host::TargetId;
use crate::pawn::{Ctx, Pawn};
use crate::states::common::on_ground;
use crate::states::{StateData, StateId};

/// Seconds without a counter attack running before the rest of the counter is dropped.
pub const GIVE_UP: f32 = 3.0;

/// The counter waiting to be played.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CounterAsk {
    /// The kinds still to play, the first next.
    pub kinds: Vec<u8>,
    pub target: TargetId,
    /// The kinds are the held form's own (played by its attack class); false: blade kinds of the plain attack state.
    pub own_form: bool,
    /// Seconds without one of its attacks running.
    pub waited: f32,
    /// Kinds started so far (for the host's log and tests).
    pub played: u32,
}

/// A kind that only plays on the ground (Zantetsuken, XS 0xE4).
fn needs_ground(kind: u8) -> bool {
    kind == super::finishers::ZANTETSUKEN
}

/// The states a counter attack may start out of.
fn free(id: StateId) -> bool {
    matches!(id, StateId::Idle | StateId::Run | StateId::Jump | StateId::Glide | StateId::Floating | StateId::Dodgeroll | StateId::Guard | StateId::AirSlide)
}

/// A blade counter waits: the held form's own attack request is skipped (`states::attack::try_attack`).
pub(crate) fn skips_form(s: &Pawn) -> bool {
    s.combat.xs.counter.as_ref().is_some_and(|c| !c.own_form && !c.kinds.is_empty())
}

/// THE FORCED KIND of an attack request out of state `id` (`combat::guard::forced_attack`'s first question): the counter's
/// next kind, taken.
pub(crate) fn take(id: StateId, s: &mut Pawn) -> Option<CounterOffer> {
    if !free(id) {
        return None;
    }
    let grounded = on_ground(s);
    let c = s.combat.xs.counter.as_mut()?;
    let kind = *c.kinds.first()?;
    if needs_ground(kind) && !grounded {
        return None;
    }
    c.kinds.remove(0);
    c.played += 1;
    c.waited = 0.0;
    Some(CounterOffer { source: id, kind, target: c.target, just_guard: false, push: id == StateId::AirSlide })
}

/// The per-step part (controller update): the request made while a kind waits and Sora is free; the give-up clock.
pub(crate) fn tick(state: &mut StateData, cx: &mut Ctx, dt: f32) {
    let attacking = state.attack().is_some();
    let Some(c) = cx.pawn.combat.xs.counter.as_mut() else { return };
    c.waited = if attacking { 0.0 } else { c.waited + dt };
    if (c.kinds.is_empty() && !attacking) || c.waited > GIVE_UP {
        cx.pawn.combat.xs.counter = None;
        return;
    }
    let (kind, target) = match c.kinds.first() {
        Some(k) => (*k, c.target),
        None => return,
    };
    if !free(state.id()) || (needs_ground(kind) && !on_ground(cx.pawn)) {
        return;
    }
    crate::states::attack::try_attack(state, cx, target);
}
