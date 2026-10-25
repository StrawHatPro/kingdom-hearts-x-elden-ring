//! (DON1) ATTUNEMENT AND RESONANCE (documentation/DONALD_OVERHAUL.md 4.1 - 4.4, 4.6, 6.1).
//!
//! - One element at a time; with Dual Attunement (4.6) two, each with its own Resonance meter. `slots[0]` is the newest
//!   attunement (the primary), `slots[1]` the older one (Dual only). Attuning a third element replaces the OLDER one
//!   (DON1 reading: the doc does not say which; the newest stays).
//! - Resonance: 3 pips (fractional fill). Full power = attuned with Resonance 3 (4.4). A pip that fills refunds 2 % of
//!   max MP (the caller pays it). A heavy hit takes one pip off every meter (4.1). It drains only out of combat.
//! - The attunement PERIOD (6.1): from attuning to an element until he switches away. Its ultimate can be cast once in
//!   it; switching back later opens a fresh period.
//! - RELEASE (4.3): the element replaced while its meter is at 3 bursts (the caller casts its `rel.*` row, free) and
//!   the new element starts at `attune.release_start` (1) pips.

use super::element::{ElemSet, Element};

/// One attuned element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub element: Element,
    /// 0 ..= `res.pips`.
    pub res: f32,
    /// The attunement period's id (rises with every attune).
    pub period: u32,
    /// Its ultimate was cast in this period.
    pub ult_used: bool,
    /// The clock time it last reached full Resonance (the quick price window, 6.2).
    pub full_at: Option<f32>,
}

impl Slot {
    pub fn pips(&self) -> u8 {
        self.res.floor().max(0.0) as u8
    }
}

/// The attune cast under way (4.1: 0.6 s, interruptible).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttuneCast {
    pub element: Element,
    pub left: f32,
    pub total: f32,
}

/// The attunement state.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Attunement {
    pub slots: [Option<Slot>; 2],
    pub cast: Option<AttuneCast>,
    /// (DON-UI) The running cast is an UNATTUNE (the user, 2026-10-22: "a way ... to unattune and fire his default
    /// missiles again"): its end clears every attunement (`DonCore::begin_unattune`).
    pub unattuning: bool,
    next_period: u32,
}

/// What [`Attunement::attune`] did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Switched {
    pub element: Element,
    pub period: u32,
    /// The element it replaced, if any.
    pub replaced: Option<Element>,
    /// The replaced element was at full Resonance: its Release fires.
    pub released: Option<Element>,
}

impl Attunement {
    pub fn slot(&self, e: Element) -> Option<&Slot> {
        self.slots.iter().flatten().find(|s| s.element == e)
    }

    pub fn slot_mut(&mut self, e: Element) -> Option<&mut Slot> {
        self.slots.iter_mut().flatten().find(|s| s.element == e)
    }

    /// The newest attunement.
    pub fn primary(&self) -> Option<Element> {
        self.slots[0].map(|s| s.element)
    }

    pub fn attuned(&self, e: Element) -> bool {
        self.slot(e).is_some()
    }

    /// The attuned elements.
    pub fn elements(&self) -> ElemSet {
        let mut s = ElemSet::NONE;
        for slot in self.slots.iter().flatten() {
            s.insert(slot.element);
        }
        s
    }

    /// The element `e` would replace if attuned now (`dual`: Dual Attunement equipped).
    pub fn would_replace(&self, e: Element, dual: bool) -> Option<Element> {
        if self.attuned(e) {
            return None;
        }
        if dual {
            if self.slots[1].is_none() {
                return None;
            }
            self.slots[1].map(|s| s.element)
        } else {
            self.slots[0].map(|s| s.element)
        }
    }

    /// Attuning `e` now would trigger a Release (the Attunement submenu's mark, 7).
    pub fn would_release(&self, e: Element, dual: bool, pips: f32) -> Option<Element> {
        let r = self.would_replace(e, dual)?;
        self.slot(r).filter(|s| s.res >= pips).map(|s| s.element)
    }

    /// Completes an attunement to `e` (the cast has run): a new period, the replaced slot's Release. `None` when `e`
    /// is already attuned.
    pub fn attune(&mut self, e: Element, dual: bool, pips: f32, release_start: f32) -> Option<Switched> {
        if self.attuned(e) {
            return None;
        }
        let replaced_slot = if dual {
            if self.slots[1].is_none() {
                let old = self.slots[0];
                self.slots[1] = old;
                None
            } else {
                let older = self.slots[1];
                self.slots[1] = self.slots[0];
                older
            }
        } else {
            self.slots[0]
        };
        let released = replaced_slot.filter(|s| s.res >= pips).map(|s| s.element);
        self.next_period += 1;
        let period = self.next_period;
        self.slots[0] = Some(Slot { element: e, res: if released.is_some() { release_start.clamp(0.0, pips) } else { 0.0 }, period, ult_used: false, full_at: None });
        Some(Switched { element: e, period, replaced: replaced_slot.map(|s| s.element), released })
    }

    /// Dual Attunement taken off with two held: the older one is let go (no Release: nothing was switched).
    pub fn drop_second(&mut self) -> Option<Element> {
        self.slots[1].take().map(|s| s.element)
    }

    /// Clears every attunement (a reset / a new spawn).
    pub fn clear(&mut self) {
        self.slots = [None, None];
        self.cast = None;
        self.unattuning = false; // (DON-UI)
    }
}
