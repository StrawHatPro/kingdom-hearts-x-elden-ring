//! (DON-UI) DONALD'S HUD SNAPSHOT (documentation/DONALD_OVERHAUL.md 7, 6.3, 6.5, 9 Scan / Elemental Insight; the plan
//! build/sora_tmp/party/DON_PLAN.md R17). Pure: what the HUD shows of one Donald, computed from his state and the
//! party's enemy statuses. kh-ui draws it (`hud_resonance`, `hud_arcane`, `hud_target`), kh-host carries it
//! (`Hud::set_don`), kh-er (Elden Ring) and kh-app (the test level) feed it through kh-partyhost.
//!
//! - THE RING (7): one per attunement (two with Dual Attunement, the primary first): the element, its colour (sheet
//!   `color.*`), the Resonance pips (3, fractional fill), full power, the quick window (the -za discount, 6.2), and the
//!   ultimate mark ([`UltMark`]: Hidden / Waiting dim / Ready lit / Spent dark, R17).
//! - THE ATTUNE CAST: the element being attuned and how far (the ring fills in its colour while he casts).
//! - THE STARS (6.3): the Arcane Flow charges (4) and the seconds left of the oldest (it blinks before it goes).
//! - THE ZETTAFLARE TRACKER (6.5): once he knows Zettaflare, a small icon per learned element, lit when it reached
//!   Resonance 3 in this fight.
//! - THE TARGET (7): the lock-on target's statuses as icons (Wet, Chill + stacks, Burn, Reveal, Frozen, plus Exposed
//!   and Blind), and the Scan gate (9: enemy HP only with Scan, KH3) / Elemental Insight (its weak / resisted elements).

use super::abilities::DonAbility;
use super::combat::UltMark;
use super::element::{ElemSet, Element};
use super::spells::ArcaneLine;
use super::status::{StatusBook, TargetStatus};
use super::tune::DonData;
use super::DonaldState;

/// One Resonance ring around the MP bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RingView {
    pub element: Element,
    /// 0xRRGGBB (sheet `color.<element>`).
    pub color: u32,
    /// The meter, 0 ..= `pips_max` (fractional: the pip being filled shows its share).
    pub pips: f32,
    pub pips_max: u8,
    /// Resonance 3: full-power attunement.
    pub full: bool,
    /// Within the quick window after reaching 3 (the -za tier / ultimate at the discount, 6.2).
    pub quick: bool,
    pub ult: UltMark,
}

impl RingView {
    /// The pips fully lit (0 ..= 3).
    pub fn lit(&self) -> u8 {
        (self.pips + 1e-4).floor().clamp(0.0, self.pips_max as f32) as u8
    }
}

/// A status icon over the lock-on target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StatusIcon {
    Wet,
    /// Chill with its stacks (1 ..= 4; 5 = Frozen).
    Chill(u8),
    Burn,
    Reveal,
    Frozen,
    /// The Reveal turned into Exposed (4.5).
    Exposed,
    Blind,
}

impl StatusIcon {
    /// The element whose colour tints the icon (Blind / Exposed: none, KH3's own look).
    pub fn element(self) -> Option<Element> {
        match self {
            StatusIcon::Wet => Some(Element::Water),
            StatusIcon::Chill(_) | StatusIcon::Frozen => Some(Element::Blizzard),
            StatusIcon::Burn => Some(Element::Fire),
            StatusIcon::Reveal | StatusIcon::Exposed => Some(Element::Light),
            StatusIcon::Blind => None,
        }
    }

    /// The text the icon carries (KH3's status-icon style: a short word).
    pub fn label(self) -> &'static str {
        match self {
            StatusIcon::Wet => "Wet",
            StatusIcon::Chill(_) => "Chill",
            StatusIcon::Burn => "Burn",
            StatusIcon::Reveal => "Reveal",
            StatusIcon::Frozen => "Frozen",
            StatusIcon::Exposed => "Exposed",
            StatusIcon::Blind => "Blind",
        }
    }
}

/// The icons of one enemy's statuses, in the HUD's order (Frozen first: it is the one that matters most).
pub fn icons_of(s: &TargetStatus) -> Vec<StatusIcon> {
    let mut out = Vec::new();
    if s.is_frozen() {
        out.push(StatusIcon::Frozen);
    } else if s.is_chilled() {
        out.push(StatusIcon::Chill(s.chill));
    }
    if s.is_wet() {
        out.push(StatusIcon::Wet);
    }
    if s.is_burned() {
        out.push(StatusIcon::Burn);
    }
    if s.is_revealed() {
        out.push(if s.exposed { StatusIcon::Exposed } else { StatusIcon::Reveal });
    }
    if s.is_blind() {
        out.push(StatusIcon::Blind);
    }
    out
}

/// What the HUD shows over / for the lock-on target.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TargetHud {
    pub id: u32,
    pub icons: Vec<StatusIcon>,
    /// Scan (9): the target's HP gauge is shown (KH3: without Scan an enemy's HP is not shown).
    pub scan: bool,
    /// Elemental Insight (9): the target's weak and resisted elements (None: not worn, or the host does not know them).
    pub insight: Option<(ElemSet, ElemSet)>,
}

/// The Zettaflare tracker (6.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ZettaView {
    /// The learned elements (one icon each).
    pub need: ElemSet,
    /// The ones that reached Resonance 3 in this fight (lit).
    pub got: ElemSet,
}

/// One Donald's HUD.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DonHud {
    /// The attunement rings, primary first (empty: not attuned).
    pub rings: Vec<RingView>,
    /// Attuning now: the element, its colour and the share done (0 ..= 1).
    pub attuning: Option<(Element, u32, f32)>,
    /// Arcane Flow (6.3): charges now and the meter's size.
    pub stars: u8,
    pub stars_max: u8,
    /// Seconds left of the oldest charge.
    pub star_left: Option<f32>,
    /// The Zettaflare tracker, once Zettaflare is learned.
    pub zetta: Option<ZettaView>,
    pub target: Option<TargetHud>,
    /// Elemental Insight's best element against the lock-on target (the Attunement submenu's mark too).
    pub best: Option<Element>,
}

/// The element Elemental Insight marks as best against a target: a weak element he has unlocked, in menu order;
/// None when nothing is known.
pub fn best_element(weak: ElemSet, resist: ElemSet, unlocked: ElemSet) -> Option<Element> {
    weak.iter().find(|e| unlocked.has(*e) && !resist.has(*e))
}

/// A target's (weak, resisted) elements from KH3's attribute resists (`TargetDefense::resist`, percent by
/// `ETresDamageAttribute`: 1 Fire, 2 Blizzard, 3 Thunder, 4 Water, 5 Aero; negative = weak, positive = resisted). Light
/// has no KH3 attribute: never marked.
pub fn affinity_of_resist(resist: &[i32; 8]) -> (ElemSet, ElemSet) {
    let (mut weak, mut strong) = (ElemSet::NONE, ElemSet::NONE);
    for (i, e) in [Element::Fire, Element::Blizzard, Element::Thunder, Element::Water, Element::Aero].into_iter().enumerate() {
        let r = resist[i + 1];
        if r < 0 {
            weak.insert(e);
        } else if r > 0 {
            strong.insert(e);
        }
    }
    // (DON-PROG) Light (holy) in slot 7 (Targets::element_affinity)
    match resist[7] {
        r if r < 0 => weak.insert(Element::Light),
        r if r > 0 => strong.insert(Element::Light),
        _ => {}
    }
    (weak, strong)
}

impl DonHud {
    /// THE SNAPSHOT of one Donald: `book` the party's enemy statuses, `lock` the lock-on target (its id), `affinity`
    /// the host's (weak, resisted) elements of that target when it knows them.
    pub fn of(st: &DonaldState, book: &StatusBook, lock: Option<u32>) -> DonHud {
        Self::with(st, book, lock, None, &super::tune::data())
    }

    /// [`DonHud::of`] with the target's elements and the numbers given.
    pub fn with(st: &DonaldState, book: &StatusBook, lock: Option<u32>, affinity: Option<(ElemSet, ElemSet)>, d: &DonData) -> DonHud {
        let core = &st.core;
        let pips_max = d.tune.res_pips.round().clamp(1.0, 8.0) as u8;
        let mut rings: Vec<RingView> = Vec::new();
        // the primary (the newest, R13) first
        let primary = core.attune.primary();
        let mut slots: Vec<_> = core.attune.slots.iter().flatten().copied().collect();
        slots.sort_by_key(|s| if Some(s.element) == primary { 0 } else { 1 });
        for s in slots {
            rings.push(RingView {
                element: s.element,
                color: s.element.color(&d.tune),
                pips: s.res.clamp(0.0, pips_max as f32),
                pips_max,
                full: core.full_power(s.element, d),
                quick: core.quick(s.element, d),
                ult: core.ultimate_mark(s.element, &st.learned, d),
            });
        }
        let attuning = core.attune.cast.map(|c| (c.element, c.element.color(&d.tune), if c.total > 0.0 { (1.0 - c.left / c.total).clamp(0.0, 1.0) } else { 1.0 }));
        let zetta = (st.learned.arcane(ArcaneLine::Zettaflare) > 0).then(|| {
            let (got, need) = core.zettaflare_progress(&st.learned);
            ZettaView { need, got }
        });
        let ab = &st.abilities;
        let insight_on = ab.has(DonAbility::ElementalInsight);
        let target = lock.map(|id| TargetHud {
            id,
            icons: book.get(id).map(icons_of).unwrap_or_default(),
            scan: ab.has(DonAbility::Scan),
            insight: if insight_on { affinity } else { None },
        });
        let best = if insight_on { affinity.and_then(|(w, r)| best_element(w, r, st.learned.elements())) } else { None };
        DonHud {
            rings,
            attuning,
            stars: core.arcane.charges(),
            stars_max: d.tune.arcane_max.round().clamp(1.0, 8.0) as u8,
            star_left: core.arcane.next_expiry(core.clock),
            zetta,
            target,
            best,
        }
    }

    /// Nothing to draw of the ring / stars (not attuned, no charges, no tracker).
    pub fn is_idle(&self) -> bool {
        self.rings.is_empty() && self.attuning.is_none() && self.stars == 0 && self.zetta.is_none()
    }
}

// ------------------------------------------------------------------------------------------------ the sheet's numbers

include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_don_hud.rs"));

macro_rules! hud_fields {
    ($($f:ident),* $(,)?) => {
        /// Sets one row of `party-don-hud` by its id (`pip.x`) or field name (`pip_x`); false for an unknown one.
        pub fn set_hud_param(p: &mut DonHudParams, key: &str, v: f32) -> bool {
            let field = key.trim().replace('.', "_");
            match field.as_str() {
                $(stringify!($f) => { p.$f = v; true })*
                _ => false,
            }
        }
    };
}

hud_fields!(
    menu_keep_cursor, menu_reasons, menu_icons, badge_right, badge_y, badge_row_step, badge_plate_h, badge_plate, badge_plate_alpha, badge_pad,
    badge_rim, badge_rim_hz, pip_w, pip_h, pip_step, pip_off, pip_off_alpha, icon_size, icon_gap, ult_size, ult_gap, ult_dark, ult_dim, ult_hot,
    ult_glow, ult_glow_min, ult_pulse_hz, ult_ready_scale, ult_core, ult_glow_h, ult_glow_gain, ult_tint, ult_flare_w,
    ult_flare_h, ult_burst_s, ult_burst_scale, worn_pc_x, worn_pc_y, worn_size_pc, worn_npc_x, worn_npc_y,
    worn_size_npc, blink_hz, star_size, star_step, star_off, star_warn, star_blink_hz, zetta_gap, zetta_size, zetta_step, zetta_off, status_x,
    status_y, status_size, status_step, status_insight_dy, status_insight_size, y_star_size, y_star_step, y_star_x, y_star_y, y_star_off,
);

/// The menu's switches of the sheet.
pub fn menu_tune_of(p: &DonHudParams) -> crate::pad::menu::DonMenuTune {
    crate::pad::menu::DonMenuTune { keep_cursor: p.menu_keep_cursor != 0.0, reasons: p.menu_reasons != 0.0, icons: p.menu_icons != 0.0 }
}
