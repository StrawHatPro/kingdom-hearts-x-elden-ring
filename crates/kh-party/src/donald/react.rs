//! (DON1) THE ELEMENTAL REACTIONS (documentation/DONALD_OVERHAUL.md 4.5): a status on the target hit by another element.
//!
//! | status | hit with | reaction | effect |
//! |---|---|---|---|
//! | Wet | Thunder | Conduct | x2 Thunder damage, chains to every Wet enemy (Wet stays) |
//! | Wet | Blizzard | Flash Freeze | instant Frozen (Wet is used up) |
//! | Chill / Frozen | Fire | Steam Burst | big damage + Blind (the ice / Chill is used up) |
//! | Burn | Aero | Wildfire | the Burn spreads to every enemy near the target |
//! | Frozen | Aero, Thunder or a Charged Missile | Shatter | huge single-target damage, Frozen ends |
//! | Revealed | any | Exposed | +25 % damage from everyone in the party (for the rest of the Reveal) |
//! | Wet | Fire | Fizzle | Wet removed, weak damage: the bad combo (no Burn, no pip, no Arcane Flow) |
//!
//! READINGS (DON1, where the table leaves a case open; DON_PLAN.md "Contradictions"):
//! - one ELEMENTAL reaction per hit, by priority Shatter > Steam Burst > Conduct > Flash Freeze > Wildfire > Fizzle;
//!   Exposed is checked besides it. A hybrid counts as both elements (6.4), so Fire + Thunder (Plasma) on a Wet enemy
//!   Conducts instead of Fizzling, and a hit carrying Water never Fizzles (Scald, 6.4: "the one way to mix Fire and
//!   Water without a Fizzle").
//! - Exposed: the FIRST hit on a Revealed enemy that is not Light-only (any other element, a plain missile, a friend's
//!   hit) turns the Reveal into Exposed, once per Reveal; Light alone only Reveals.
//! - Frozen on a large enemy / boss is the slow of 6.1 (status.rs `freeze`).

use super::element::{ElemSet, Element};
use super::status::{Apply, FullFlags, StatusEvent, TargetStatus};
use super::tune::DonTune;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Reaction {
    Conduct,
    FlashFreeze,
    SteamBurst,
    Wildfire,
    Shatter,
    Exposed,
    Fizzle,
}

impl Reaction {
    pub const ALL: [Reaction; 7] = [Reaction::Conduct, Reaction::FlashFreeze, Reaction::SteamBurst, Reaction::Wildfire, Reaction::Shatter, Reaction::Exposed, Reaction::Fizzle];

    pub const fn label(self) -> &'static str {
        match self {
            Reaction::Conduct => "Conduct",
            Reaction::FlashFreeze => "Flash Freeze",
            Reaction::SteamBurst => "Steam Burst",
            Reaction::Wildfire => "Wildfire",
            Reaction::Shatter => "Shatter",
            Reaction::Exposed => "Exposed",
            Reaction::Fizzle => "Fizzle",
        }
    }

    /// A good reaction: it fills a Resonance pip and gives Arcane Flow (every one but Fizzle).
    pub const fn rewards(self) -> bool {
        !matches!(self, Reaction::Fizzle)
    }
}

/// One hit as the reaction rule sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HitTag {
    /// Its elements (a hybrid: both; a plain missile or a friend's physical hit: none).
    pub elements: ElemSet,
    /// A Charged Missile (it Shatters Frozen whatever its element, 4.5 / 6.1).
    pub breaker: bool,
}

impl HitTag {
    pub fn of(elements: ElemSet) -> HitTag {
        HitTag { elements, breaker: false }
    }
}

/// What the hit did.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct HitOutcome {
    /// The reactions it landed (at most one elemental one, plus Exposed).
    pub reactions: Vec<Reaction>,
    /// The hit's damage times this (the reaction's and Exposed's).
    pub damage_mult: f32,
    /// Conduct: chain to every Wet enemy within this (cm).
    pub conduct_radius: Option<f32>,
    /// Wildfire: Burn every enemy within this of the target (cm).
    pub wildfire_radius: Option<f32>,
    /// The status events (Froze, Launch ...).
    pub events: Vec<StatusEvent>,
}

/// THE RULE: resolves one hit on one enemy: the reaction(s) from the statuses BEFORE the hit, then the hit's own
/// statuses (`applies`). `reaction_plus`: the ability (+30 % on a reaction's bonus).
pub fn resolve(s: &mut TargetStatus, hit: HitTag, applies: &[Apply], full: FullFlags, t: &DonTune, reaction_plus: bool) -> HitOutcome {
    let e = hit.elements;
    let (fire, blizzard, thunder, water, aero) = (e.has(Element::Fire), e.has(Element::Blizzard), e.has(Element::Thunder), e.has(Element::Water), e.has(Element::Aero));
    let mut out = HitOutcome { damage_mult: 1.0, ..HitOutcome::default() };
    let was_frozen = s.is_frozen();
    let mut suppress_burn = false;
    let mut froze_now = false;
    let bonus = |m: f32| if reaction_plus && m > 1.0 { 1.0 + (m - 1.0) * t.react_plus_mult } else { m };

    let elemental = if was_frozen && (aero || thunder || hit.breaker) {
        s.unfreeze(t);
        out.damage_mult *= bonus(t.react_shatter_mult);
        Some(Reaction::Shatter)
    } else if (was_frozen || s.is_chilled()) && fire {
        s.unfreeze(t);
        s.chill = 0;
        s.chill_left = 0.0;
        s.blind = s.blind.max(t.blind_duration);
        out.damage_mult *= bonus(t.react_steam_mult);
        Some(Reaction::SteamBurst)
    } else if s.is_wet() && thunder {
        out.damage_mult *= bonus(t.react_conduct_mult);
        out.conduct_radius = Some(t.react_conduct_radius);
        Some(Reaction::Conduct)
    } else if s.is_wet() && blizzard {
        s.wet = 0.0;
        s.freeze(full, t, &mut out.events);
        froze_now = true;
        Some(Reaction::FlashFreeze)
    } else if s.is_burned() && aero {
        out.wildfire_radius = Some(t.react_wildfire_radius);
        Some(Reaction::Wildfire)
    } else if s.is_wet() && fire && !water {
        s.wet = 0.0;
        suppress_burn = true;
        out.damage_mult *= t.react_fizzle_mult;
        Some(Reaction::Fizzle)
    } else {
        None
    };
    out.reactions.extend(elemental);

    // Exposed: the first hit that is not Light-only on a Revealed enemy (once per Reveal)
    let light_only = !e.is_empty() && e.covers(ElemSet::one(Element::Light)) && e.len() == 1;
    if s.is_revealed() && !s.exposed && !light_only {
        s.exposed = true;
        out.reactions.push(Reaction::Exposed);
    }
    out.damage_mult *= s.damage_taken_mult(t);

    // the hit's own statuses
    for a in applies {
        match a {
            Apply::Burn if suppress_burn => {}
            Apply::Chill(_) | Apply::Frozen if froze_now => {}
            _ => s.apply(*a, full, t, &mut out.events),
        }
    }
    // a hit that neither broke nor melted the ice chips it (6.1)
    if was_frozen && s.is_frozen() && !matches!(elemental, Some(Reaction::Shatter | Reaction::SteamBurst)) {
        s.chip_ice(t);
    }
    out
}
