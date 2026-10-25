//! (DON1) DONALD'S SHARED COMBAT STATE: attunement, Resonance, full power and periods, the Release, Arcane Flow, the
//! cast rules (what is castable now and for how much MP) and the Y list (documentation/DONALD_OVERHAUL.md 4, 6, 7).
//! One [`DonCore`] per Donald (`FriendStatus::donald`), leader or AI. Engine-free and frame-rate independent: every
//! timer counts the seconds passed to [`DonCore::tick`].
//!
//! THE FLOW for a caster (DON-KIT / DON-MAGIC / AI):
//! 1. [`DonCore::check_cast`] -> a [`CastPlan`] (MP, price, cast time) or a [`Refusal`] (the menu's greyed reason);
//! 2. at the cast's START [`DonCore::commit_cast`] (5.3: "the MP is spent and lost" if he is hit while casting): the
//!    caller takes `plan.mp` from his MP; Resonance, the ultimate's period mark, Spell Echo, the three-element rule;
//! 3. every landed hit through `react::resolve` on the target's statuses, then [`DonCore::on_reaction`] /
//!    [`DonCore::on_missile_hit`] for the meters;
//! 4. [`DonCore::take_events`] each step: pips filled (refund `refund_pct` % of max MP), Releases to burst, Arcane
//!    Flow changes, full power reached / lost, attune casts done / interrupted.

use super::abilities::DonMods;
use super::arcane::{ArcaneFlow, ArcaneGain};
use super::attune::{AttuneCast, Attunement};
use super::element::{ElemSet, Element};
use super::learn::DonLearned;
use super::react::Reaction;
use super::spells::{mp_of_cost, mp_of_pct, price_pct, ArcaneLine, PriceCtx, SpellRef};
use super::tune::DonData;

/// Something the host shows or pays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DonEvent {
    /// The attune cast began (its length; 0 = instant).
    AttuneStarted { element: Element, seconds: f32 },
    /// The attune cast was interrupted (by a hit): nothing changed.
    AttuneInterrupted { element: Element },
    /// Attuned: a new period.
    Attuned { element: Element, period: u32, replaced: Option<Element> },
    /// The replaced element was at Resonance 3: cast its Release burst (`rel.<element>`, free) at the lock-on target.
    Released { element: Element },
    /// A pip filled: refund this share of max MP (percent).
    PipFilled { element: Element, pips: u8, refund_pct: f32 },
    /// A pip was lost (a heavy hit).
    PipLost { element: Element, pips: u8 },
    /// Full-power attunement reached (Resonance 3).
    FullPower { element: Element },
    /// Full power lost (a heavy hit, the drain, a switch).
    FullPowerLost { element: Element },
    /// Arcane Flow +1 (or a refresh at 4).
    ArcaneGained { reason: ArcaneGain, charges: u8 },
    /// Charges expired (in combat) or the meter emptied (out of combat / Zettaflare).
    ArcaneLost { charges: u8 },
    /// Spell Echo armed: the next base tier of this element is free.
    EchoArmed { element: Element },
    /// The fight ended (out of combat for `fight.end_delay`).
    FightEnded,
}

/// Why a spell cannot be cast now (the menu's greyed row; the Y row's MP text).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NotLearned,
    /// A -za / ultimate / Ultima: needs that element (Ultima: any) at full-power attunement.
    NeedsFullPower,
    /// The ultimate was already cast in this attunement period.
    UltimateSpent,
    /// A hybrid needs Dual Attunement and both its elements attuned.
    NeedsBothAttuned,
    NotEnoughMp { need: i32 },
    NotEnoughCharges { need: u8 },
    /// Zettaflare: not every learned element reached Resonance 3 in this fight.
    ZettaflareNotReady,
    /// Busy attuning.
    Attuning,
    /// A Release is never cast by hand.
    NotCastable,
    /// (DON-UI) An ultimate, Ultima or Zettaflare while he is airborne (the user, 2026-10-22: they "can only be cast on the
    /// ground"); the menu greys them with "On the ground only" (the cast gate itself is DON-SPELLB's / ZETTA's).
    NeedsGround,
}

/// A castable spell's price.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CastPlan {
    pub spell: SpellRef,
    pub mp: i32,
    /// The share of max MP (percent).
    pub pct: f32,
    pub attuned: bool,
    pub quick: bool,
    pub echo: bool,
    /// The cast's length (s, the row's `cast`).
    pub cast: f32,
    /// Y spells: the charges it spends.
    pub charges: u8,
}

/// The ultimate mark on the Resonance ring (7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UltMark {
    /// Not learned, or the element not attuned: no mark.
    Hidden,
    /// Unused in this period, not at full power yet (dim).
    Waiting,
    /// Castable now (lit).
    Ready,
    /// Cast in this period (dark).
    Spent,
}

/// One row of the Y list (6.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct YOffer {
    pub spell: SpellRef,
    pub charges: u8,
    pub mp: i32,
    /// He can pay it (else greyed with `mp` as the MP it needs).
    pub affordable: bool,
}

/// Donald's shared combat state.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DonCore {
    /// (ZETTA 2026-10-23, the user: "for Zettaflare, make it so that once you meet it's requirements it just needs 100% of your current MP to fire, and it makes your MP regen slower by 30%") Zettaflare fired: his MP refills `zetta.regen_mult` as fast until his MP
    /// bar has recharged (FriendStatus::don_mp_haste; cleared by hooks.rs once it is full).
    pub zetta_slow: bool,
    pub attune: Attunement,
    pub arcane: ArcaneFlow,
    /// Seconds since the core began (the timers' clock).
    pub clock: f32,
    pub in_combat: bool,
    /// Seconds out of combat (0 in combat).
    pub out_for: f32,
    fight_over: bool,
    /// The elements that reached Resonance 3 in this fight (the Zettaflare tracker, 6.5).
    pub fight_full: ElemSet,
    /// Recent casts (clock, elements) for the three-element rule.
    casts: Vec<(f32, ElemSet)>,
    /// Spell Echo: the element and the clock time it ends.
    pub echo: Option<(Element, f32)>,
    /// (target, reaction, clock) of the last rewarded reactions (the pip cooldown).
    rewarded: Vec<(u32, Reaction, f32)>,
    last_arcane_reaction: Option<f32>,
    events: Vec<DonEvent>,
}

impl DonCore {
    // ------------------------------------------------------------ reading
    /// Attuned to `e` with Resonance 3.
    pub fn full_power(&self, e: Element, d: &DonData) -> bool {
        self.attune.slot(e).is_some_and(|s| s.res >= d.tune.res_pips)
    }

    /// Full power in some element (Ultima's condition).
    pub fn full_power_any(&self, d: &DonData) -> bool {
        self.attune.slots.iter().flatten().any(|s| s.res >= d.tune.res_pips)
    }

    /// The element's Resonance (pips, fractional) when attuned.
    pub fn resonance(&self, e: Element) -> Option<f32> {
        self.attune.slot(e).map(|s| s.res)
    }

    pub fn attuned(&self, e: Element) -> bool {
        self.attune.attuned(e)
    }

    /// The attuned elements.
    pub fn elements(&self) -> ElemSet {
        self.attune.elements()
    }

    /// The element of the n-th missile (4.6: with two attunements they alternate; none = plain arcane).
    pub fn missile_element(&self, n: u32) -> Option<Element> {
        match (self.attune.slots[0], self.attune.slots[1]) {
            (Some(a), Some(b)) => Some(if n % 2 == 0 { a.element } else { b.element }),
            (Some(a), None) => Some(a.element),
            _ => None,
        }
    }

    /// Within `res.quick_window` of `e` reaching Resonance 3 (the quick price, 6.2).
    pub fn quick(&self, e: Element, d: &DonData) -> bool {
        self.full_power(e, d) && self.attune.slot(e).and_then(|s| s.full_at).is_some_and(|t| self.clock - t <= d.tune.res_quick_window)
    }

    /// The ultimate mark of an attuned element (7).
    pub fn ultimate_mark(&self, e: Element, learned: &DonLearned, d: &DonData) -> UltMark {
        let Some(s) = self.attune.slot(e) else { return UltMark::Hidden };
        if !learned.knows(SpellRef::Ultimate(e)) {
            return UltMark::Hidden;
        }
        if s.ult_used {
            UltMark::Spent
        } else if self.full_power(e, d) {
            UltMark::Ready
        } else {
            UltMark::Waiting
        }
    }

    /// The Zettaflare tracker (6.5): (the learned elements that reached Resonance 3 in this fight, the learned ones).
    pub fn zettaflare_progress(&self, learned: &DonLearned) -> (ElemSet, ElemSet) {
        let need = learned.elements();
        (ElemSet(self.fight_full.0 & need.0), need)
    }

    /// The Zettaflare condition but the MP (4 charges, every learned element at Resonance 3 in this fight).
    pub fn zettaflare_ready(&self, learned: &DonLearned, d: &DonData) -> bool {
        let (got, need) = self.zettaflare_progress(learned);
        !need.is_empty() && got.covers(need) && self.arcane.charges() as f32 >= d.tune.arcane_max
    }

    /// What the line-1 entry of an element's menu casts now (6.1): the -za tier at full power, else the highest tier
    /// up to -ga he knows.
    pub fn line1_pick(&self, e: Element, learned: &DonLearned, d: &DonData) -> Option<SpellRef> {
        let t = learned.tier(e);
        if t == 0 {
            return None;
        }
        if t >= 4 && self.full_power(e, d) {
            return Some(SpellRef::Line1(e, 4));
        }
        Some(SpellRef::Line1(e, t.min(3)))
    }

    /// A shortcut's line-1 tier as cast now: the -za tier falls back to -ga when not at full power (DON1 reading of 6.1).
    pub fn shortcut_tier(&self, s: SpellRef, learned: &DonLearned, d: &DonData) -> SpellRef {
        match s {
            SpellRef::Line1(e, 4) if !self.full_power(e, d) && learned.tier(e) >= 3 => SpellRef::Line1(e, 3),
            other => other,
        }
    }

    // ------------------------------------------------------------ casting
    fn arcane_charges(spell: SpellRef, d: &DonData) -> u8 {
        spell.row(d).map_or(0, |r| r.arcane as u8)
    }

    /// THE CAST RULE: castable now, and its price; `mp` / `max_mp`: his MP now and his max; `dual`: Dual Attunement worn.
    pub fn check_cast(&self, spell: SpellRef, learned: &DonLearned, mp: i32, max_mp: i32, dual: bool, d: &DonData) -> Result<CastPlan, Refusal> {
        if matches!(spell, SpellRef::Release(_)) {
            return Err(Refusal::NotCastable);
        }
        if self.attune.cast.is_some() {
            return Err(Refusal::Attuning);
        }
        if !learned.knows(spell) {
            return Err(Refusal::NotLearned);
        }
        let row = spell.row(d).ok_or(Refusal::NotLearned)?;
        let scale = d.tune.cost_scale;
        if let SpellRef::Arcane(line, _) = spell {
            let charges = row.arcane as u8;
            if self.arcane.charges() < charges {
                return Err(Refusal::NotEnoughCharges { need: charges });
            }
            if line == ArcaneLine::Ultima && !self.full_power_any(d) {
                return Err(Refusal::NeedsFullPower);
            }
            if line == ArcaneLine::Zettaflare {
                if !self.zettaflare_ready(learned, d) {
                    return Err(Refusal::ZettaflareNotReady);
                }
                // (ZETTA 2026-10-23, the user: "for Zettaflare, make it so that once you meet it's requirements it just needs 100% of your current MP to fire, and it makes your MP regen slower by 30%") ALL of his current MP, any above 0 (it replaces the 300 MP of
                // 2026-10-22); the slower MP regen follows its cast (commit_cast)
                // (ZETTA 2026-10-23, the user: "Zettaflare needs to use up all of your magic (it needs all of your current MP to be useable) and then put you into the red MP bar mode with a 30% slower recharge rate"; asked, the user chose "Requires a full MP bar") A FULL MP BAR: castable only with all of his MP (MP == max MP), all of it spent: his MP
                // Charge (the red bar, as Cure's) runs, `zetta.regen_mult` as fast
                if !zetta_mp_ok(mp, max_mp) {
                    return Err(Refusal::NotEnoughMp { need: max_mp.max(1) });
                }
                return Ok(CastPlan { spell, mp, pct: mp as f32, attuned: false, quick: false, echo: false, cast: row.cast, charges });
            }
            let need = mp_of_cost(row.cost_att, scale); // a set MP cost like the rest of his magic (user, 2026-10-22)
            if mp < need {
                return Err(Refusal::NotEnoughMp { need });
            }
            return Ok(CastPlan { spell, mp: need, pct: row.cost_att, attuned: false, quick: false, echo: false, cast: row.cast, charges });
        }
        let elems = spell.elements();
        let attuned = !elems.is_empty() && self.elements().covers(elems);
        if let SpellRef::Hybrid(..) = spell {
            if !dual || !attuned {
                return Err(Refusal::NeedsBothAttuned);
            }
        }
        let e = spell.element().unwrap_or(Element::Fire);
        if row.full_power && !self.full_power(e, d) {
            return Err(Refusal::NeedsFullPower);
        }
        if spell.is_ultimate() && self.attune.slot(e).is_some_and(|s| s.ult_used) {
            return Err(Refusal::UltimateSpent);
        }
        let quick = attuned && (row.full_power || spell.is_ultimate()) && self.quick(e, d);
        let echo = spell.is_base() && self.echo.is_some_and(|(ee, until)| ee == e && self.clock <= until);
        let pct = price_pct(row, PriceCtx { attuned, quick, echo_free: echo }).ok_or(Refusal::NeedsFullPower)?;
        let need = mp_of_cost(pct, scale); // fixed MP like Sora (user, 2026-10-22); `pct` now holds that cost
        if need > 0 && mp < need {
            return Err(Refusal::NotEnoughMp { need });
        }
        Ok(CastPlan { spell, mp: need, pct, attuned, quick, echo, cast: row.cast, charges: 0 })
    }

    /// THE CAST BEGINS (the caller has taken `plan.mp`): Resonance for attuned elements, the period's ultimate mark,
    /// Spell Echo, the three-element rule, the Y charges (Zettaflare empties Resonance and Arcane Flow).
    pub fn commit_cast(&mut self, plan: &CastPlan, m: &DonMods, d: &DonData) {
        let spell = plan.spell;
        if let SpellRef::Arcane(line, _) = spell {
            if line == ArcaneLine::Zettaflare {
                self.zetta_slow = true; // (ZETTA 2026-10-23) his MP regen is slower until his MP has recharged
                let n = self.arcane.charges();
                self.arcane.clear();
                if n > 0 {
                    self.events.push(DonEvent::ArcaneLost { charges: n });
                }
                for e in self.attune.elements().iter() {
                    self.set_res(e, 0.0, d);
                }
            } else if self.arcane.spend(plan.charges) {
                self.events.push(DonEvent::ArcaneLost { charges: plan.charges });
            }
            return;
        }
        // Spell Echo: used, or cleared by any other cast; armed again by a -za
        if plan.echo || self.echo.is_some() {
            self.echo = None;
        }
        if m.spell_echo && spell.is_za() {
            if let Some(e) = spell.element() {
                self.echo = Some((e, self.clock + d.tune.echo_window));
                self.events.push(DonEvent::EchoArmed { element: e });
            }
        }
        if spell.is_ultimate() {
            if let Some(s) = spell.element().and_then(|e| self.attune.slot_mut(e)) {
                s.ult_used = true;
            }
        }
        for e in spell.elements().iter() {
            if self.attuned(e) {
                self.gain(e, d.tune.res_gain_spell, d);
            }
        }
        self.note_elements(spell.elements(), m, d);
    }

    /// The three-elements-in-10-s rule (6.3): this cast's elements into the window; +1 Arcane Flow at three, the window
    /// starts over.
    fn note_elements(&mut self, elems: ElemSet, m: &DonMods, d: &DonData) {
        if elems.is_empty() {
            return;
        }
        let now = self.clock;
        self.casts.retain(|(t, _)| now - *t <= d.tune.arcane_elements_window);
        self.casts.push((now, elems));
        let seen = self.casts.iter().fold(ElemSet::NONE, |s, (_, e)| s.union(*e));
        if seen.len() as f32 >= d.tune.arcane_elements_needed {
            self.casts.clear();
            self.arcane_gain(ArcaneGain::ThreeElements, m, d);
        }
    }

    // ------------------------------------------------------------ attuning
    /// Starts attuning to `e` (4.1): the cast length (0 with Instant Attune: done at once). Refused when the element is
    /// locked (no spell of it), already attuned, or a cast runs.
    pub fn begin_attune(&mut self, e: Element, learned: &DonLearned, m: &DonMods, d: &DonData) -> Result<f32, Refusal> {
        if !learned.attune_unlocked(e) {
            return Err(Refusal::NotLearned);
        }
        if self.attuned(e) {
            return Err(Refusal::NotCastable);
        }
        if self.attune.cast.is_some() {
            return Err(Refusal::Attuning);
        }
        let mult = if m.instant_attune {
            d.tune.attune_instant_mult
        } else if m.swift_attune {
            d.tune.attune_swift_mult
        } else {
            1.0
        };
        let total = (d.tune.attune_cast * mult).max(0.0);
        self.events.push(DonEvent::AttuneStarted { element: e, seconds: total });
        if total <= 0.0 {
            self.finish_attune(e, m, d);
        } else {
            self.attune.cast = Some(AttuneCast { element: e, left: total, total });
        }
        Ok(total)
    }

    /// A hit interrupts the attune cast (nothing changes).
    pub fn interrupt_attune(&mut self) -> bool {
        match self.attune.cast.take() {
            Some(c) => {
                self.attune.unattuning = false; // (DON-UI) an interrupted unattune changes nothing either
                self.events.push(DonEvent::AttuneInterrupted { element: c.element });
                true
            }
            None => false,
        }
    }

    // ------------------------------------------------------------ (DON-UI) unattuning
    /// (DON-UI) UNATTUNE (the user, 2026-10-22: "Donald needs a way at the bottom of his attunement submenu to unattune
    /// and fire his default missiles again"; the coordinator's readings): a cast like attuning (the same length, Swift /
    /// Instant Attune, interruptible); at its end every attunement goes (both with Dual), each one at Resonance 3 is
    /// Released as switching away does, the periods end. Refused when he is not attuned or a cast runs.
    pub fn begin_unattune(&mut self, m: &DonMods, d: &DonData) -> Result<f32, Refusal> {
        let Some(primary) = self.attune.primary() else { return Err(Refusal::NotCastable) };
        if self.attune.cast.is_some() {
            return Err(Refusal::Attuning);
        }
        let mult = if m.instant_attune {
            d.tune.attune_instant_mult
        } else if m.swift_attune {
            d.tune.attune_swift_mult
        } else {
            1.0
        };
        let total = (d.tune.attune_cast * mult).max(0.0);
        self.events.push(DonEvent::AttuneStarted { element: primary, seconds: total });
        if total <= 0.0 {
            self.finish_unattune(m, d);
        } else {
            self.attune.cast = Some(AttuneCast { element: primary, left: total, total });
            self.attune.unattuning = true;
        }
        Ok(total)
    }

    /// (DON-UI) Unattuning now would Release these elements (the Attunement submenu's mark on Unattune).
    pub fn would_release_unattune(&self, d: &DonData) -> ElemSet {
        let mut out = ElemSet::NONE;
        for s in self.attune.slots.iter().flatten().filter(|s| s.res >= d.tune.res_pips) {
            out.insert(s.element);
        }
        out
    }

    fn finish_unattune(&mut self, m: &DonMods, d: &DonData) {
        let released = self.would_release_unattune(d);
        for e in released.iter() {
            self.events.push(DonEvent::FullPowerLost { element: e });
            self.events.push(DonEvent::Released { element: e });
            self.arcane_gain(ArcaneGain::Release, m, d);
        }
        self.attune.slots = [None, None];
        self.attune.cast = None;
        self.attune.unattuning = false;
    }

    /// Attuning now would Release (the Attunement submenu's mark, 7); the element released.
    pub fn would_release(&self, e: Element, m: &DonMods, d: &DonData) -> Option<Element> {
        self.attune.would_release(e, m.dual, d.tune.res_pips)
    }

    fn finish_attune(&mut self, e: Element, m: &DonMods, d: &DonData) {
        let before: Vec<(Element, bool)> = self.attune.slots.iter().flatten().map(|s| (s.element, s.res >= d.tune.res_pips)).collect();
        let Some(sw) = self.attune.attune(e, m.dual, d.tune.res_pips, d.tune.attune_release_start) else { return };
        if let Some(r) = sw.replaced {
            if before.iter().any(|(el, full)| *el == r && *full) {
                self.events.push(DonEvent::FullPowerLost { element: r });
            }
        }
        self.events.push(DonEvent::Attuned { element: e, period: sw.period, replaced: sw.replaced });
        if let Some(r) = sw.released {
            self.events.push(DonEvent::Released { element: r });
            self.arcane_gain(ArcaneGain::Release, m, d);
        }
    }

    /// Dual Attunement taken off: the older attunement is let go.
    pub fn sync_dual(&mut self, m: &DonMods) {
        if !m.dual {
            self.attune.drop_second();
        }
    }

    // ------------------------------------------------------------ the meters
    fn set_res(&mut self, e: Element, v: f32, d: &DonData) {
        let pips = d.tune.res_pips;
        let Some(s) = self.attune.slot_mut(e) else { return };
        let was_full = s.res >= pips;
        s.res = v.clamp(0.0, pips);
        if was_full && s.res < pips {
            self.events.push(DonEvent::FullPowerLost { element: e });
        }
    }

    /// Fills `e`'s meter by `amount` pips: refunds per pip filled, full power reached.
    fn gain(&mut self, e: Element, amount: f32, d: &DonData) {
        let (pips, refund, now) = (d.tune.res_pips, d.tune.res_refund_pct, self.clock);
        let Some(s) = self.attune.slot_mut(e) else { return };
        if amount <= 0.0 || s.res >= pips {
            return;
        }
        let before = s.res.floor() as i32;
        s.res = (s.res + amount).min(pips);
        let after = s.res.floor() as i32;
        let full = s.res >= pips;
        if full {
            s.full_at = Some(now);
        }
        for p in (before + 1)..=after {
            self.events.push(DonEvent::PipFilled { element: e, pips: p as u8, refund_pct: refund });
        }
        if full {
            self.fight_full.insert(e);
            self.events.push(DonEvent::FullPower { element: e });
        }
    }

    /// A missile (or the Charged Missile) of an attuned element landed (4.1 small gain).
    pub fn on_missile_hit(&mut self, e: Option<Element>, d: &DonData) {
        if let Some(e) = e {
            self.gain(e, d.tune.res_gain_missile, d);
        }
    }

    /// A reaction landed on `target` by a hit of `elems` (4.5, 6.3): a full pip to the attuned meter of the hit's
    /// element (else the primary meter), and +1 Arcane Flow; once per `res.reaction_cooldown` for the same reaction on
    /// the same target, Arcane Flow at most once per `arcane.reaction_cooldown`. Fizzle gives nothing.
    pub fn on_reaction(&mut self, r: Reaction, target: u32, elems: ElemSet, m: &DonMods, d: &DonData) {
        if !r.rewards() {
            return;
        }
        let now = self.clock;
        self.rewarded.retain(|(_, _, t)| now - *t < d.tune.res_reaction_cooldown);
        if self.rewarded.iter().any(|(id, rr, _)| *id == target && *rr == r) {
            return;
        }
        self.rewarded.push((target, r, now));
        let to = elems.iter().find(|e| self.attuned(*e)).or_else(|| self.attune.primary());
        if let Some(e) = to {
            self.gain(e, d.tune.res_gain_reaction, d);
        }
        if self.last_arcane_reaction.is_none_or(|t| now - t >= d.tune.arcane_reaction_cooldown) {
            self.last_arcane_reaction = Some(now);
            self.arcane_gain(ArcaneGain::Reaction, m, d);
        }
    }

    /// A heavy hit on Donald (4.1): one pip off every meter, and the attune cast is interrupted.
    pub fn on_heavy_hit(&mut self, d: &DonData) {
        self.interrupt_attune();
        let loss = d.tune.res_heavy_loss;
        let list: Vec<(Element, f32)> = self.attune.slots.iter().flatten().map(|s| (s.element, s.res)).collect();
        for (e, res) in list {
            if res <= 0.0 {
                continue;
            }
            let v = (res - loss).max(0.0);
            self.set_res(e, v, d);
            self.events.push(DonEvent::PipLost { element: e, pips: v.floor() as u8 });
        }
    }

    /// A perfect Magic Block (5.2): a pip to the primary meter, +1 Arcane Flow.
    pub fn on_perfect_block(&mut self, m: &DonMods, d: &DonData) {
        if let Some(e) = self.attune.primary() {
            self.gain(e, d.tune.res_gain_perfect_block, d);
        }
        self.arcane_gain(ArcaneGain::PerfectBlock, m, d);
    }

    fn arcane_gain(&mut self, reason: ArcaneGain, m: &DonMods, d: &DonData) {
        let life = if m.arcane_flow_plus { d.tune.arcane_life_plus } else { d.tune.arcane_life };
        self.arcane.gain(self.clock, life, d.tune.arcane_max as u8);
        self.events.push(DonEvent::ArcaneGained { reason, charges: self.arcane.charges() });
    }

    // ------------------------------------------------------------ the clock
    /// One step: `in_combat` is the host's battle state for Donald.
    pub fn tick(&mut self, dt: f32, in_combat: bool, m: &DonMods, d: &DonData) {
        let dt = dt.max(0.0);
        self.clock += dt;
        self.sync_dual(m);
        // the attune cast
        if let Some(mut c) = self.attune.cast {
            c.left -= dt;
            if c.left <= 0.0 {
                self.attune.cast = None;
                if std::mem::take(&mut self.attune.unattuning) {
                    self.finish_unattune(m, d); // (DON-UI)
                } else {
                    self.finish_attune(c.element, m, d);
                }
            } else {
                self.attune.cast = Some(c);
            }
        }
        // combat and the fight
        self.in_combat = in_combat;
        if in_combat {
            self.out_for = 0.0;
            self.fight_over = false;
            let lost = self.arcane.expire(self.clock);
            if lost > 0 {
                self.events.push(DonEvent::ArcaneLost { charges: lost });
            }
        } else {
            self.out_for += dt;
            if !self.fight_over && self.out_for >= d.tune.fight_end_delay {
                self.fight_over = true;
                let n = self.arcane.charges();
                self.arcane.clear();
                if n > 0 {
                    self.events.push(DonEvent::ArcaneLost { charges: n });
                }
                self.fight_full = ElemSet::NONE;
                self.casts.clear();
                self.events.push(DonEvent::FightEnded);
            }
            // the drain (out of combat only, 4.1)
            if self.out_for > d.tune.res_drain_delay {
                let rate = d.tune.res_drain * if m.resonance_keeper { d.tune.res_keeper_mult } else { 1.0 };
                let live = dt.min(self.out_for - d.tune.res_drain_delay);
                let list: Vec<(Element, f32)> = self.attune.slots.iter().flatten().map(|s| (s.element, s.res)).collect();
                for (e, res) in list {
                    if res > 0.0 {
                        self.set_res(e, res - rate * live, d);
                    }
                }
            }
        }
        if let Some((_, until)) = self.echo {
            if self.clock > until {
                self.echo = None;
            }
        }
    }

    /// The events since the last call.
    pub fn take_events(&mut self) -> Vec<DonEvent> {
        std::mem::take(&mut self.events)
    }

    // ------------------------------------------------------------ the Y list
    /// THE Y LIST (6.3, 6.5): every learned Y spell whose charge cost is at or below his charges (Ultima only at full
    /// power in some element, Zettaflare only with its condition), cheapest first; each greyed when he cannot pay its MP.
    pub fn y_offers(&self, learned: &DonLearned, mp: i32, max_mp: i32, d: &DonData) -> Vec<YOffer> {
        let mut out = Vec::new();
        for line in ArcaneLine::ALL {
            let Some(spell) = learned.arcane_spell(line) else { continue };
            let charges = Self::arcane_charges(spell, d);
            if self.arcane.charges() < charges {
                continue;
            }
            if line == ArcaneLine::Ultima && !self.full_power_any(d) {
                continue;
            }
            if line == ArcaneLine::Zettaflare && !self.zettaflare_ready(learned, d) {
                continue;
            }
            // (ZETTA 2026-10-23, the user: "Zettaflare needs to use up all of your magic (it needs all of your current MP to be useable) and then put you into the red MP bar mode with a 30% slower recharge rate"; asked, the user chose "Requires a full MP bar") offered only with a full MP bar (it spends all of it)
            if line == ArcaneLine::Zettaflare && !zetta_mp_ok(mp, max_mp) {
                continue;
            }
            // set MP costs (user, 2026-10-22); (ZETTA 2026-10-23) Zettaflare: his whole (full) bar
            let need = if line == ArcaneLine::Zettaflare { max_mp.max(1) } else { spell.row(d).map_or(0, |r| mp_of_cost(r.cost_att, d.tune.cost_scale)) };
            out.push(YOffer { spell, charges, mp: need, affordable: mp >= need });
        }
        out
    }
}

/// (ZETTA 2026-10-23, the user: "Zettaflare needs to use up all of your magic (it needs all of your current MP to be useable) and then put you into the red MP bar mode with a 30% slower recharge rate"; asked, the user chose "Requires a full MP bar") ZETTAFLARE'S MP RULE: a FULL MP bar (his MP at its max, above 0).
pub fn zetta_mp_ok(mp: i32, max_mp: i32) -> bool {
    mp > 0 && mp >= max_mp
}
