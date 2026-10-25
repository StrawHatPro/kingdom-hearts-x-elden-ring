//! (DON-SPELLB) THE CAST PRESENTATION OF DONALD'S OWN SPELLS (part B: line 2, the line-3 ultimates, the 15 hybrids, the Y
//! Arcane magic, the six Releases). The user (2026-10-22): "all of his spells need proper casting animations many of them
//! currently have none! And they are STILL missing effects! ... make all of the spells look like they were created and
//! polished for the real game!"
//!
//! WHAT A CAST LOOKS LIKE (a [`Look`] per spell, [`look_of`]):
//! - THE POSE: one of Donald's own KH3 clips (n_ex001), picked per spell for what it does: the quick wand flick
//!   (`MagicFire` 1321) for what is thrown, the wand thrust high (`MagicCast` 1341) for what is called onto a target, the
//!   leap and wand slam (`M_1311`) for what bursts out of the ground (Flood, Glacier), the long raised-wand call
//!   (`DeckActionB` 0977) for the big Arcane magic and the summons (Comet, Meteor, Ball Lightning), the wand twirl
//!   (`BattleStart` 1014) for the Release; in the air his air clips (`MagicAirFire` / `MagicAirCast`). The ULTIMATES take
//!   ZETTA's casting circle and Donald's Zettaflare channel and cast, tinted to the element (the user: "His ultimate level
//!   magic should have the same casting circle and Donald's casting animation as Zettaflare, without the cinematic camera";
//!   `zettaflare::ultimate_begin`), so they have no Look of their own here.
//! - THE TIMING: the clip is played so that its release frame (`shot`, clip seconds) comes exactly at the row's cast time;
//!   a cast longer than the clip's lead-in HOLDS the pose at `hold` (the wand raised, the spell gathering) instead of
//!   slowing the whole clip into slow motion; a shorter one plays the clip faster (`cast.rate_min` / `rate_max`).
//! - THE EFFECTS AT THE WAND: `charge` from the cast's start until the release (persistent, stopped at the release or when a
//!   hit interrupts the cast), `flash` at the release, all Donald's own KH3 cast / release effects of the element
//!   (`vn_ex001_*_cal00` / `*_fir00`, the unison calls `vn_ex001_uni_*_cal00` for the big ones; his `*_han*` effects are his
//!   KH3 spells themselves and are never used as a wand light).
//! - THE VOICE: his KH3 cast lines (`MagicCast` 1-13, the big callouts 101-103 for the ultimates, the Y magic and the
//!   hybrids). Clips with their own PlayVoice notify take the number through the voice group number; the others queue it.
//!
//! Everything here is presentation: the spells' rules (cost, timing, damage, statuses) stay where they were (`cast.rs`
//! `launch`, `magic_fx`, `allyfx`).

use super::element::Element;
use super::spells::{ArcaneLine, SpellRef};
use kh_core::host::Tint;

/// Where an effect sits on Donald.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum On {
    /// The wand's tip (`vfx_call`).
    Wand,
    /// His body's centre (`vfx_center`).
    Body,
    /// His feet (the actor's origin).
    Feet,
}

impl On {
    pub fn socket(self) -> Option<&'static str> {
        match self {
            On::Wand => Some("vfx_call"),
            On::Body => Some("vfx_center"),
            On::Feet => None,
        }
    }
}

/// One effect of a cast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub name: &'static str,
    pub on: On,
    pub scale: f32,
    pub tint: Option<Tint>,
}

const fn p(name: &'static str, on: On, scale: f32) -> Part {
    Part { name, on, scale, tint: None }
}

const fn pt(name: &'static str, on: On, scale: f32, tint: Tint) -> Part {
    Part { name, on, scale, tint: Some(tint) }
}

/// Which of his cast lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice {
    /// `MagicCast` 1-13 (a random one).
    Small,
    /// The big callouts `MagicCast` 101-103.
    Big,
}

/// A spell's cast presentation (see the module text).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// The ground clip and the air clip (logical names of n_ex001's anim set).
    pub clip: &'static str,
    pub air: &'static str,
    /// The clip time of the release (the spell leaves the wand), s.
    pub shot: f32,
    /// The clip time the pose is held at while a long cast gathers (None: the clip is slowed instead).
    pub hold: Option<f32>,
    /// The clip's own speed for this spell (before the fit to the cast time).
    pub rate: f32,
    pub charge: &'static [Part],
    pub flash: &'static [Part],
    pub voice: Voice,
}

// ------------------------------------------------------------------------------------------- the clips (KH3 n_ex001)

/// The quick wand flick: crouch, then the thrust (Shot notify 0.31 s).
const FLICK: (&str, &str, f32, Option<f32>) = ("MagicFire", "MagicAirFire", 0.31, Some(0.2));
/// The wand thrust high (Shot notify 0.50 s); held with the wand up.
const THRUST: (&str, &str, f32, Option<f32>) = ("MagicCast", "MagicAirCast", 0.5, Some(0.42));
/// The leap and wand slam (the smash lands at 0.36 s).
const SLAM: (&str, &str, f32, Option<f32>) = ("M_1311", "MagicAirCast", 0.36, Some(0.17));
/// The long raised-wand call: the wand comes up by 0.45 s and stays up until 0.9 s.
const CALL: (&str, &str, f32, Option<f32>) = ("DeckActionB", "MagicAirCast", 0.62, Some(0.48));

const fn look(c: (&'static str, &'static str, f32, Option<f32>), charge: &'static [Part], flash: &'static [Part], voice: Voice) -> Look {
    Look { clip: c.0, air: c.1, shot: c.2, hold: c.3, rate: 1.0, charge, flash, voice }
}

// ------------------------------------------------------------------------------------------- the wand effects per element

/// Donald's own cast effect of an element at the wand (his `n_ex001_Magic_ead` rows; Water: Sora's Water cast, KH3 gives
/// Donald no Water: (KITFX) his vn_ex001_wbl_* effects are his UNDERWATER Blizzard, ice with Blizzard's SE).
pub const fn cal(e: Element) -> &'static str {
    match e {
        Element::Fire => "vn_ex001_fir_cal00",
        Element::Blizzard => "vn_ex001_blz_cal00",
        Element::Thunder => "vn_ex001_thd_cal00",
        Element::Water => "vm_wat_cal0", // (KITFX) Water: KH3 Water-family (vn_ex001_wbl_* is his UNDERWATER Blizzard: ice and the Blizzard SE)
        Element::Aero => "vn_ex001_air_cal00",
        Element::Light => "vn_ex001_cur_cal00",
    }
}

/// His release flash of an element at the wand (`vn_ex001_*_fir00`: the spell leaving the staff).
pub const fn fire(e: Element) -> &'static str {
    match e {
        Element::Fire => "vn_ex001_fir_fir00",
        Element::Blizzard => "vn_ex001_blz_fir00",
        Element::Thunder => "vn_ex001_thd_fir00",
        Element::Water => "vs_wat_hit1", // (KITFX) Water: KH3 Water-family (vn_ex001_wbl_* is his UNDERWATER Blizzard: ice and the Blizzard SE): Water's splash
        Element::Aero => "vn_ex001_air_fir00",
        Element::Light => "vn_ex001_cur_fir00",
    }
}

/// The light gathering at his wand while a big spell builds (his unison calls: a ground glow with sparks), never one of his
/// `*_han*` effects (those are his KH3 spells themselves: the tornado, the lightning, the ice dome).
pub const fn hand(e: Element) -> &'static str {
    match e {
        Element::Fire => "vn_ex001_uni_fir_cal00",
        Element::Blizzard => "vn_ex001_uni_blz_cal00",
        Element::Thunder => "vn_ex001_uni_thd_cal00",
        Element::Water => "vs_wat_hit1", // (KITFX) Water: KH3 Water-family (vn_ex001_wbl_* is his UNDERWATER Blizzard: ice and the Blizzard SE): Water's splash gathering
        Element::Aero => "vn_ex001_air_cal00",
        Element::Light => "vn_ex001_cur_cal00",
    }
}

/// The big unison call of an element (Fire / Blizzard / Thunder; the rest: the element's call).
pub const fn big_cal(e: Element) -> &'static str {
    match e {
        Element::Fire => "vn_ex001_uni_fir_cal00",
        Element::Blizzard => "vn_ex001_uni_blz_cal00",
        Element::Thunder => "vn_ex001_uni_thd_cal00",
        e => cal(e),
    }
}

/// The colour of each element (the ultimates' circle tint, the Arcane magic's call).
pub const fn element_tint(e: Element) -> Tint {
    match e {
        Element::Fire => Tint { color: [1.0, 0.42, 0.12], intensity: 1.3 },
        Element::Blizzard => Tint { color: [0.55, 0.85, 1.0], intensity: 1.25 },
        Element::Thunder => Tint { color: [1.0, 0.9, 0.3], intensity: 1.35 },
        Element::Water => Tint { color: [0.25, 0.6, 1.0], intensity: 1.25 },
        Element::Aero => Tint { color: [0.45, 1.0, 0.6], intensity: 1.2 },
        Element::Light => Tint { color: [1.0, 0.85, 0.45], intensity: 1.4 },
    }
}

/// The non-elemental Arcane colour of the Y magic's wand light (blue-white).
pub const ARCANE: Tint = Tint { color: [0.62, 0.72, 1.0], intensity: 1.35 };

macro_rules! parts {
    ($($x:expr),* $(,)?) => {{ const PARTS: &[Part] = &[$($x),*]; PARTS }};
}

/// THE LOOK OF A SPELL (None: line 1 is part A's, Zettaflare is ZETTA's, the ultimates take ZETTA's presentation).
pub fn look_of(spell: SpellRef) -> Option<Look> {
    use Element::*;
    use On::*;
    use Voice::*;
    Some(match spell {
        SpellRef::Line1(..) | SpellRef::Ultimate(..) => return None,
        SpellRef::Arcane(ArcaneLine::Zettaflare, _) => return None,
        // ---- line 2
        // Flare: the coil of the flick held while the white-hot star gathers at the wand, then the throw
        SpellRef::Line2(Fire, _) => look(FLICK, parts![p("vn_ex001_fir_cal00", Wand, 1.0), p("ve_ex773_flre_cal0", Wand, 1.2), pt("vn_ex001_fff_wep00", Wand, 0.8, element_tint(Fire))], parts![p("vn_ex001_fir_fir00", Wand, 1.4), pt("vn_ex001_fff_hit00", Wand, 0.6, element_tint(Fire))], Small),
        // Freeze: the wand thrust at the target (the frost gathers there)
        SpellRef::Line2(Blizzard, _) => look(THRUST, parts![p("vn_ex001_blz_cal00", Wand, 1.0), p("vn_ex001_uni_blz_cal00", Wand, 0.9)], parts![p("vn_ex001_blz_fir00", Wand, 1.4), pt("vn_ex001_fff_hit00", Wand, 0.5, element_tint(Blizzard))], Small),
        // Ball Lightning: the wand raised to call the balls round the wearer
        SpellRef::Line2(Thunder, _) => look(CALL, parts![p("vn_ex001_thd_cal00", Wand, 1.0), p("vn_ex001_uni_thd_cal00", Wand, 0.9)], parts![p("vn_ex001_thd_fir00", Wand, 1.4), pt("vn_ex001_fff_hit00", Wand, 0.6, element_tint(Thunder))], Small),
        // Flood: the slam (0.3 s)
        SpellRef::Line2(Water, _) => look(SLAM, parts![p("vs_wat_hit1", Wand, 0.7)], parts![p("vs_wat_hit1", Wand, 1.4), p("vm_wat_cal0", Feet, 1.0)], Small), // (KITFX) Water: KH3 Water-family (vn_ex001_wbl_* is his UNDERWATER Blizzard: ice and the Blizzard SE)
        // Aero Barrier: the wand thrust up, the wind gathering in his hand
        SpellRef::Line2(Aero, _) => look(THRUST, parts![p("vn_ex001_air_cal00", Wand, 1.0), pt("vn_ex001_fff_wep00", Wand, 0.8, Tint { color: [0.75, 1.0, 0.9], intensity: 1.2 })], parts![p("vn_ex001_air_fir00", Wand, 1.4), pt("vn_ex001_fff_hit00", Wand, 0.5, Tint { color: [0.8, 1.0, 0.95], intensity: 1.2 })], Small),
        // Banish: the wand thrust high, the holy light calling the ray down
        SpellRef::Line2(Light, _) => look(THRUST, parts![p("vn_ex001_cur_cal00", Wand, 1.0), pt("vn_ex001_fff_wep00", Wand, 0.8, element_tint(Light))], parts![p("vn_ex001_cur_fir00", Wand, 0.9)], Small),
        // ---- the hybrids: what is thrown is flicked, what is called is thrust, the ice wall is slammed up
        SpellRef::Hybrid(a, b) => {
            let thrown = matches!((a, b), (Fire, Blizzard) | (Fire, Thunder) | (Blizzard, Light) | (Thunder, Light) | (Aero, Light) | (Thunder, Aero));
            let slam = matches!((a, b), (Blizzard, Water));
            let around = matches!((a, b), (Blizzard, Aero));
            let c = if slam {
                SLAM
            } else if thrown {
                FLICK
            } else if around {
                CALL
            } else {
                THRUST
            };
            Look { clip: c.0, air: c.1, shot: c.2, hold: c.3, rate: 1.0, charge: hybrid_charge(a, b), flash: hybrid_flash(a, b), voice: Big }
        }
        // ---- Y Arcane: the long raised-wand call for what falls from the sky, the thrust for Gravity and Ultima
        SpellRef::Arcane(ArcaneLine::Comet, _) => look(CALL, parts![pt("vn_ex001_fff_wep00", Wand, 1.0, ARCANE), pt("vn_ex001_fcm_emi00", Wand, 0.45, ARCANE)], parts![pt("vn_ex001_fff_hit00", Wand, 1.2, ARCANE)], Big),
        SpellRef::Arcane(ArcaneLine::Gravity, _) => look(THRUST, parts![pt("vn_ex001_fff_wep00", Wand, 1.0, Tint { color: [0.6, 0.35, 1.0], intensity: 1.3 }), pt("vn_ex001_uni_thd_cal00", Wand, 0.9, Tint { color: [0.55, 0.3, 1.0], intensity: 1.2 })], parts![pt("vn_ex001_fff_hit00", Wand, 1.2, Tint { color: [0.6, 0.35, 1.0], intensity: 1.3 })], Big),
        SpellRef::Arcane(ArcaneLine::Meteor, _) => look(CALL, parts![pt("vn_ex001_fff_wep00", Wand, 1.2, ARCANE), p("vn_ex001_fcm_emi00", Wand, 0.7), p("vn_ex001_uni_fir_cal00", Wand, 1.0)], parts![p("vn_ex001_fff_hit00", Wand, 1.4)], Big),
        SpellRef::Arcane(ArcaneLine::Ultima, _) => look(CALL, parts![pt("vn_ex001_fff_wep00", Wand, 1.4, ARCANE), pt("vn_ex001_cur_cal00", Wand, 1.2, ARCANE)], parts![pt("vn_ex001_fff_hit00", Wand, 1.8, ARCANE)], Big),
        // ---- the Releases: the wand twirl (the switch is a move); cast by `release_gesture`, not through `begin`
        SpellRef::Release(e) => Look { clip: "BattleStart", air: "MagicAirCast", shot: 0.18, hold: None, rate: 1.0, charge: &[], flash: release_flash(e), voice: Small },
    })
}

/// (HYB) the Light half of a hybrid at the wand: golden holy light (his Arcane wand light and flash in Light's gold), never
/// his Cure call (Light hybrids never heal: holy damage plus Reveal).
const HOLY_WAND: Part = pt("vn_ex001_fff_wep00", On::Wand, 0.9, element_tint(Element::Light));
const HOLY_FLASH: Part = pt("vn_ex001_fff_hit00", On::Wand, 0.45, element_tint(Element::Light));

fn hybrid_charge(a: Element, b: Element) -> &'static [Part] {
    use Element::*;
    use On::*;
    match (a, b) {
        (Fire, Blizzard) => parts![p("vn_ex001_fir_cal00", Wand, 0.9), p("vn_ex001_blz_cal00", Wand, 0.9)],
        (Fire, Thunder) => parts![p("vn_ex001_fir_cal00", Wand, 1.0), p("vn_ex001_thd_cal00", Wand, 1.0)],
        (Fire, Water) => parts![p("vn_ex001_wfi_cal00", Wand, 1.0), p("vn_ex001_uni_fir_cal00", Wand, 0.9)],
        (Fire, Aero) => parts![p("vn_ex001_fir_cal00", Wand, 1.0), p("vn_ex001_air_cal00", Wand, 1.0)],
        (Fire, Light) => parts![p("vn_ex001_uni_fir_cal00", Wand, 0.8), HOLY_WAND],
        (Blizzard, Thunder) => parts![p("vn_ex001_blz_cal00", Wand, 1.0), p("vn_ex001_thd_cal00", Wand, 1.0)],
        (Blizzard, Water) => parts![p("vn_ex001_uni_blz_cal00", Wand, 0.9), p("vm_wat_cal0", Wand, 1.0)], // (KITFX) Water: KH3 Water-family (vn_ex001_wbl_* is his UNDERWATER Blizzard: ice and the Blizzard SE)
        (Blizzard, Aero) => parts![p("vn_ex001_uni_blz_cal00", Wand, 0.8), p("vn_ex001_air_cal00", Wand, 1.0)],
        (Blizzard, Light) => parts![p("vn_ex001_blz_cal00", Wand, 1.0), HOLY_WAND],
        (Thunder, Water) => parts![p("vn_ex001_wth_cal00", Wand, 1.0), p("vn_ex001_uni_thd_cal00", Wand, 0.9)],
        (Thunder, Aero) => parts![p("vn_ex001_thd_cal00", Wand, 1.0), p("vn_ex001_air_cal00", Wand, 1.0)],
        (Thunder, Light) => parts![p("vn_ex001_uni_thd_cal00", Wand, 0.9), HOLY_WAND],
        (Water, Aero) => parts![p("vn_ex001_wai_cal00", Wand, 1.0), p("vn_ex001_air_cal00", Wand, 1.0)],
        (Water, Light) => parts![p("vm_wat_cal0", Wand, 1.0), HOLY_WAND], // (KITFX) Water: KH3 Water-family (vn_ex001_wbl_* is his UNDERWATER Blizzard: ice and the Blizzard SE)
        (Aero, Light) => parts![p("vn_ex001_air_cal00", Wand, 1.0), HOLY_WAND],
        _ => &[],
    }
}

fn hybrid_flash(a: Element, b: Element) -> &'static [Part] {
    use Element::*;
    use On::*;
    match (a, b) {
        (Fire, Blizzard) => parts![p("vn_ex001_fir_fir00", Wand, 1.0), p("vn_ex001_blz_fir00", Wand, 1.0)],
        (Fire, Thunder) => parts![p("vn_ex001_fir_fir00", Wand, 1.1), p("vn_ex001_thd_fir00", Wand, 1.0)],
        (Fire, Water) => parts![p("vn_ex001_wfi_cal00", Wand, 1.2)],
        (Fire, Aero) => parts![p("vn_ex001_fir_fir00", Wand, 1.0), p("vn_ex001_air_fir00", Wand, 1.0)],
        (Fire, Light) => parts![p("vn_ex001_fir_fir00", Wand, 1.0), HOLY_FLASH],
        (Blizzard, Thunder) => parts![p("vn_ex001_blz_fir00", Wand, 1.0), p("vn_ex001_thd_fir00", Wand, 1.0)],
        (Blizzard, Water) => parts![p("vn_ex001_blz_fir00", Wand, 1.2), p("vn_ex001_uni_blz_cal00", Wand, 1.0)],
        (Blizzard, Aero) => parts![p("vn_ex001_blz_fir00", Wand, 1.0), p("vn_ex001_air_fir00", Wand, 1.0)],
        (Blizzard, Light) => parts![p("vn_ex001_blz_fir00", Wand, 1.0), HOLY_FLASH],
        (Thunder, Water) => parts![p("vn_ex001_wth_fir00", Wand, 1.2)],
        (Thunder, Aero) => parts![p("vn_ex001_thd_fir00", Wand, 1.0), p("vn_ex001_air_fir00", Wand, 1.0)],
        (Thunder, Light) => parts![p("vn_ex001_thd_fir00", Wand, 1.0), HOLY_FLASH],
        (Water, Aero) => parts![p("vn_ex001_wai_cal00", Wand, 1.2)],
        (Water, Light) => parts![p("vs_wat_hit1", Wand, 1.0), HOLY_FLASH], // (KITFX) Water: KH3 Water-family (vn_ex001_wbl_* is his UNDERWATER Blizzard: ice and the Blizzard SE)
        (Aero, Light) => parts![p("vn_ex001_air_fir00", Wand, 1.0), HOLY_FLASH],
        _ => &[],
    }
}

fn release_flash(e: Element) -> &'static [Part] {
    use On::*;
    match e {
        Element::Fire => parts![p("vn_ex001_uni_fir_cal00", Wand, 1.0)],
        Element::Blizzard => parts![p("vn_ex001_uni_blz_cal00", Wand, 1.0)],
        Element::Thunder => parts![p("vn_ex001_uni_thd_cal00", Wand, 1.0)],
        Element::Water => parts![p("vs_wat_hit1", Wand, 1.3)], // (KITFX) Water: KH3 Water-family (vn_ex001_wbl_* is his UNDERWATER Blizzard: ice and the Blizzard SE)
        Element::Aero => parts![p("vn_ex001_air_fir00", Wand, 1.3)],
        Element::Light => parts![p("vn_ex001_cur_fir00", Wand, 1.3)],
    }
}

/// THE TIMING (see the module text): the clip's speed, and the hold (start, length in seconds of the cast) so that the clip
/// time `shot` comes at `cast` seconds. `rate_min` / `rate_max`: the play-rate clamp.
pub fn fit(l: &Look, cast: f32, rate_min: f32, rate_max: f32) -> (f32, Option<(f32, f32)>) {
    let cast = cast.max(0.05);
    let lead = l.shot / l.rate.max(0.05);
    if cast > lead + 0.02 {
        if let Some(h) = l.hold.filter(|h| *h <= l.shot) {
            return (l.rate, Some((h / l.rate, cast - lead)));
        }
    }
    ((l.shot / cast).clamp(rate_min, rate_max), None)
}

/// Every effect name the presentation spawns (the pack harness checks each one).
pub fn all_names() -> Vec<&'static str> {
    let mut v = Vec::new();
    for s in part_b() {
        if let Some(l) = look_of(s) {
            v.extend(l.charge.iter().chain(l.flash.iter()).map(|p| p.name));
        }
    }
    v.sort_unstable();
    v.dedup();
    v
}

/// Every part-B spell (line 2 at each level, the ultimates, the hybrids, the Y Arcane magic but Zettaflare, the Releases).
pub fn part_b() -> Vec<SpellRef> {
    let mut v = Vec::new();
    for e in Element::ALL {
        for lv in 1..=3 {
            v.push(SpellRef::Line2(e, lv));
        }
        v.push(SpellRef::Ultimate(e));
        v.push(SpellRef::Release(e));
    }
    v.extend(super::spells::hybrid_pairs().map(|(a, b)| SpellRef::Hybrid(a, b)));
    for l in ArcaneLine::ALL.into_iter().filter(|l| *l != ArcaneLine::Zettaflare) {
        for lv in 1..=l.max_level() {
            v.push(SpellRef::Arcane(l, lv));
        }
    }
    v
}

// ============================================================================================== the runtime

use super::cast::{CastRun, CastTarget};
use super::magic_fx::magic;
use crate::host::PartyHost;
use crate::pawn::NpcPawn;
use glam::Vec3;
use kh_core::host::party::{VoiceGroup, VoiceMode};
use kh_core::host::{Attach, EffectHandle, EffectId, EffectSpawn, PartyMember};

/// The presentation of the cast under way (`PawnCast::look`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct LookRun {
    /// The spell it belongs to (None: no part-B cast).
    pub spell: Option<SpellRef>,
    /// The clip's speed outside the hold.
    pub speed: f32,
    /// The hold: (from, length) in seconds of the cast.
    pub hold: Option<(f32, f32)>,
    /// The charge effects (stopped at the release).
    pub charge: Vec<EffectHandle>,
    /// The ultimate's presentation is ZETTA's (no clip of ours).
    pub ult: bool,
    /// The release flash has been spawned.
    pub flashed: bool,
}

/// The clips whose own PlayVoice notify plays the voice group number (the others get the line queued).
fn clip_voices(clip: &str) -> bool {
    matches!(clip, "MagicFire" | "MagicAirFire" | "MagicCast" | "MagicAirCast" | "M_1311")
}

/// The clip time of the release of an air clip, and its hold.
fn air_shot(clip: &str) -> (f32, Option<f32>) {
    match clip {
        "MagicAirFire" => (0.29, Some(0.18)),
        _ => (THRUST.2, Some(0.42)),
    }
}

fn spawn_part(p: &mut NpcPawn, host: &mut PartyHost, part: &Part, persistent: bool) -> EffectHandle {
    let pm = PartyMember(p.id.0);
    let at = match part.on.socket() {
        Some(s) => Attach::party(pm, Some(s)),
        None => Attach::World(p.position()),
    };
    let h = p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(part.name)), name: part.name, at, rotation: Vec3::ZERO, scale: part.scale, persistent });
    if let Some(t) = part.tint {
        host.effects.tint(h, t);
    }
    if let Some(se) = super::magic_fx::fx::extra_se(part.name) {
        p.seam.play(host.audio, &kh_core::host::SoundPlay { id: kh_core::host::SoundId(kh_core::notify::notify_asset_id(se)), name: se, at, volume: 1.0, pitch: 1.0, looping: false, spatial: true });
    }
    h
}

fn voice(p: &mut NpcPawn, host: &mut PartyHost, v: Voice, clip: &str) {
    let pm = PartyMember(p.id.0);
    let r = p.rng.next_u32();
    // (the user, 2026-10-22: "while using damaging holy magic like banish and holy, donald is yelling as if he is healing, make
    // him use his attack quotes") MagicCast 1-13 are his lines for a member he casts on (KH3's target voice: "Sora!",
    // "Goofy!" ... with the Cure); every part-B spell is an attack or a ward and takes his attack shouts 101-103 (KH3's magic
    // voice number), as his MagicFire throw does (`Voice::Small` / `Voice::Big` differ only in which clip carries them).
    let _ = v;
    let n = kh_core::host::party::magic_voice_number(pm, r);
    if clip_voices(clip) {
        host.audio.party_voice_group_no(pm, n);
    } else {
        p.queue_voice(VoiceGroup::MagicCast, n, VoiceMode::Replace);
    }
}

/// An AI Donald holds the play-motion state for the clip just started, so no move / turn of his runs over the pose (part
/// A's `sora_magic::hold_pose`, for every spell; a pad Donald's own state machine holds it already).
pub fn hold_pose(p: &mut NpcPawn) {
    if p.pad.is_some() {
        return;
    }
    if p.state != crate::states::NpcStateId::PlayMotion {
        let (clip, time, speed) = (p.anim.clip, p.anim.time, p.anim.speed);
        p.enter(crate::states::NpcStateId::PlayMotion);
        if p.anim.clip.sequence != clip.sequence {
            p.anim.play(clip, 0.1);
            p.anim.time = time;
        }
        p.anim.speed = speed;
    }
}

/// THE START OF A PART-B CAST (`cast::begin`): the pose, the wand's charge, the voice; Some((clip, shot_at, end_at)), or
/// None for a spell that is not part B's (line 1: part A's path).
pub fn begin_look(p: &mut NpcPawn, spell: SpellRef, cast: f32, target: CastTarget, air: bool, host: &mut PartyHost) -> Option<(&'static str, f32, f32)> {
    let m = magic();
    stop_charge(p, host);
    let ult = match spell {
        SpellRef::Ultimate(e) => Some(element_tint(e)),
        SpellRef::Arcane(ArcaneLine::Ultima, _) => Some(ULTIMA_TINT),
        _ => None,
    };
    // THE ULTIMATES (and Ultima): ZETTA's casting circle and Donald's Zettaflare channel and cast, tinted, rooted, no camera
    if let Some(tint) = ult {
        if let Some(t) = super::zettaflare::ultimate_begin(p, tint, target, host) {
            // (ZETTA's presentation plays his effort; the big callout comes at the release, [`at_shot`])
            p.don_cast.look = LookRun { spell: Some(spell), speed: 1.0, hold: None, charge: Vec::new(), ult: true, flashed: false };
            return Some(("ZettaChannel", t, t + m.cast_recover));
        }
    }
    let l = match spell {
        // refused there (should not happen): the held wand thrust with the element's big call, so it still reads
        SpellRef::Ultimate(_) => Look { clip: THRUST.0, air: THRUST.1, shot: THRUST.2, hold: THRUST.3, rate: 1.0, charge: &[], flash: &[], voice: Voice::Big },
        s => look_of(s)?,
    };
    let charge: Vec<Part> = match spell {
        SpellRef::Ultimate(e) => vec![Part { name: big_cal(e), on: On::Wand, scale: 1.2, tint: None }, Part { name: hand(e), on: On::Wand, scale: 1.0, tint: None }],
        _ => l.charge.to_vec(),
    };
    let r = start(p, host, &l, &charge, cast, air);
    p.don_cast.look.spell = Some(spell);
    Some(r)
}

/// Ultima's tint (white-violet pure magic).
pub const ULTIMA_TINT: Tint = Tint { color: [0.82, 0.7, 1.0], intensity: 1.5 };

fn start(p: &mut NpcPawn, host: &mut PartyHost, l: &Look, charge: &[Part], cast: f32, air: bool) -> (&'static str, f32, f32) {
    let m = magic();
    let clip = if air { l.air } else { l.clip };
    let fitted = if air {
        let (shot, hold) = air_shot(clip);
        Look { clip, air: clip, shot, hold, rate: 1.0, ..*l }
    } else {
        *l
    };
    p.play(clip, 0.1);
    let (speed, hold) = fit(&fitted, cast, m.cast_rate_min, m.cast_rate_max);
    p.anim.speed = speed;
    let handles: Vec<EffectHandle> = charge.iter().map(|c| spawn_part(p, host, c, true)).collect();
    voice(p, host, l.voice, clip);
    hold_pose(p);
    p.don_cast.look = LookRun { spell: None, speed, hold, charge: handles, ult: false, flashed: false };
    let shot_at = cast.max(0.05);
    (clip, shot_at, shot_at + m.cast_recover)
}

/// The charge effects end (the release, an interruption, a cast replaced).
pub fn stop_charge(p: &mut NpcPawn, host: &mut PartyHost) {
    for h in std::mem::take(&mut p.don_cast.look.charge) {
        host.effects.stop(h, false);
    }
}

/// EVERY STEP of a member (`cast::party_step`, before the cast's clock moves): the hold of a long cast, and the charge of a
/// cast that is gone (interrupted) stopped.
pub fn step(p: &mut NpcPawn, host: &mut PartyHost) {
    match p.don_cast.run {
        Some(r) if !r.shot && p.don_cast.look.spell == Some(r.spell) && !p.don_cast.look.ult => {
            if let Some((from, len)) = p.don_cast.look.hold {
                p.anim.speed = if r.t >= from && r.t < from + len { 0.0 } else { p.don_cast.look.speed };
            }
            hold_pose(p);
        }
        Some(_) => {}
        None => {
            if !p.don_cast.look.charge.is_empty() {
                stop_charge(p, host);
            }
            p.don_cast.look.spell = None;
        }
    }
}

/// THE RELEASE of a part-B cast (`cast::party_step` at the shot): the charge ends, the flash at the wand.
pub fn at_shot(p: &mut NpcPawn, run: &CastRun, host: &mut PartyHost) {
    if p.don_cast.look.spell != Some(run.spell) || p.don_cast.look.flashed {
        return;
    }
    p.don_cast.look.flashed = true;
    stop_charge(p, host);
    if p.don_cast.look.ult {
        // ZETTA's presentation has its own release flash at the staff; the big callout
        voice(p, host, Voice::Big, "");
        return;
    }
    if let Some(l) = look_of(run.spell) {
        for part in l.flash {
            let _ = spawn_part(p, host, part, false);
        }
    }
}

/// THE RELEASE'S GESTURE (4.3, "the switch itself is a move"): Donald twirls the wand as the old element bursts at the
/// target (a short hold, cancelable like a cast's recovery), with the element's flash and a short cast line. Not while a
/// spell of his is before its shot.
pub fn release_gesture(p: &mut NpcPawn, e: Element, host: &mut PartyHost) {
    if p.status.ko() || p.don_cast.run.is_some_and(|r| !r.shot) {
        return;
    }
    let Some(l) = look_of(SpellRef::Release(e)) else { return };
    let air = !p.on_ground();
    let clip = if air { l.air } else { l.clip };
    p.play(clip, 0.06);
    p.anim.speed = 1.0;
    if air {
        // past the air cast's wind-up: the wand already out
        p.anim.time = 0.3;
    }
    for part in l.flash {
        let _ = spawn_part(p, host, part, false);
    }
    voice(p, host, Voice::Small, clip);
    hold_pose(p);
    let spell = SpellRef::Release(e);
    let plan = super::combat::CastPlan { spell, mp: 0, pct: 0.0, attuned: true, quick: false, echo: false, cast: 0.0, charges: 0 };
    let end = magic().release_gesture.max(0.1);
    p.don_cast.run = Some(CastRun { spell, plan, target: CastTarget::Point(p.position()), t: 0.0, shot_at: 0.0, end_at: end, shot: true, hover: false, clip });
    p.don_cast.look = LookRun::default();
}
