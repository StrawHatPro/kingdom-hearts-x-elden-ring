//! (DON1) THE PARTY'S SIDE OF THE OVERHAUL: the enemies' statuses (party-wide), every Donald's clock, the shared MP
//! refund rule, and one route for a hit through the reaction rule. `Party::step` calls [`step`] first; the streams and
//! the hosts read the step's outbox ([`PartyDon::events`], [`PartyDon::status_events`]) after it.

use super::combat::DonEvent;
use super::react::{resolve, HitOutcome, HitTag};
use super::spells::mp_of_pct;
use super::status::{Apply, Size, StatusBook, StatusEvent};
use super::tune::data;
use crate::ids::MemberId;
use crate::party::Party;

/// The party's overhaul state.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PartyDon {
    /// Every enemy's statuses (Exposed counts for everyone's hits; Sora's Thunder conducts too).
    pub status: StatusBook,
    /// The last step's events of every Donald (pips, Releases to burst, Arcane Flow, attunes ...).
    pub events: Vec<(MemberId, DonEvent)>,
    /// The last step's status events (Burn ticks to deal, thaws, launches).
    pub status_events: Vec<(u32, StatusEvent)>,
}

/// ONCE A STEP (`Party::step`, first): every Donald's core ticks (in combat = his battle flag), the refunds are paid
/// (4.4: a pip filled refunds 2 % of max MP; none while his MP charges), the enemies' statuses tick.
pub fn step(party: &mut Party, dt: f32) {
    let d = data();
    party.don.events.clear();
    let ids: Vec<MemberId> = party.members().collect();
    for id in ids {
        let in_battle = party.member(id).is_some_and(|m| m.in_battle);
        let Some(s) = party.status_mut(id) else { continue };
        let max_mp = s.max_mp();
        let charging = s.mp_charge_left > 0.0;
        let full = !charging && s.mp >= max_mp;
        let Some(st) = s.donald.as_mut() else { continue };
        if full {
            st.core.zetta_slow = false; // (ZETTA 2026-10-23) his MP has recharged: his regen is normal again
        }
        let mods = st.abilities.mods();
        st.core.tick(dt, in_battle, &mods, &d);
        let events = st.core.take_events();
        let mut refund = 0;
        for e in &events {
            if let DonEvent::PipFilled { refund_pct, .. } = e {
                refund += mp_of_pct(max_mp, *refund_pct, 1.0);
            }
        }
        if refund > 0 && !charging {
            s.mp = (s.mp + refund).min(max_mp);
        }
        party.don.events.extend(events.into_iter().map(|e| (id, e)));
    }
    party.don.status_events = party.don.status.tick(dt, &d.tune);
}

impl Party {
    /// ONE HIT ON AN ENEMY through the reaction rule (4.5): `caster` = the Donald whose attunement lengthens the
    /// statuses and whose meters the reactions fill (None: Sora's / Goofy's hit, which still reacts and Exposes);
    /// `applies` = the hit's own statuses (a spell row's `applies`, a missile's element). Returns the reactions and the
    /// damage multiplier for the caller's damage route.
    pub fn don_hit(&mut self, caster: Option<MemberId>, target: u32, size: Size, hit: HitTag, applies: &[Apply]) -> HitOutcome {
        let d = data();
        let (full, mods) = caster.and_then(|id| self.status(id)).and_then(|s| s.donald.as_ref()).map(|st| (st.full_flags(&d), st.abilities.mods())).unwrap_or_default();
        let out = resolve(self.don.status.entry(target, size), hit, applies, full, &d.tune, mods.reaction_plus);
        if let Some(st) = caster.and_then(|id| self.status_mut(id)).and_then(|s| s.donald.as_mut()) {
            for r in &out.reactions {
                st.core.on_reaction(*r, target, hit.elements, &mods, &d);
            }
        }
        out
    }

    /// An enemy's damage multiplier from everyone (Exposed, 4.5) without a reaction (e.g. Sora's melee hits).
    pub fn don_damage_taken(&self, target: u32) -> f32 {
        self.don.status.get(target).map_or(1.0, |s| s.damage_taken_mult(&data().tune))
    }

    /// An enemy gone: its statuses too.
    pub fn don_forget(&mut self, target: u32) {
        self.don.status.forget(target);
    }
}
