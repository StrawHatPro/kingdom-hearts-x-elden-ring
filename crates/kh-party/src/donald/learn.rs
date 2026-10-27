//! (DON1) WHAT DONALD HAS LEARNED (documentation/DONALD_OVERHAUL.md 4.1, 6.3, 9, 10) and the boss-step rules.
//!
//! - Elements: each boss of an element gives ONE TIER MORE, in any order, never skipping (Sora's rule, SORA_CURRENT 5).
//!   Tier 1: the base spell and, when new, the element's Attunement; tiers 2 / 3: the next tier; tier 4: the -za tier
//!   and the ultimate. A step past the top gives Max MP +3. (DONLINE2) A tier step gives NO line-2 level any more.
//!   (DON-PROG, the user 2026-10-22: "Donald's ZA magic should be unlocked in Elden Ring's DLC") A BASE-GAME boss
//!   ([`DonStep::Element`]) gives tiers up to -ga only, Max MP +3 past it; the element's DLC boss
//!   ([`DonStep::ElementDlc`]) gives the -za tier and the ultimate with every missing lower tier first.
//! - (DONLINE2, the user 2026-10-27: "Tier 2 magic should be given by different bosses, seperate from tier 1 bosses,
//!   following the same rules as tier 1 spell aquirements") LINE 2 (Flare, Freeze, Ball Lightning, Flood, Aero Barrier,
//!   Banish) has bosses of its own ([`DonStep::Line2`], sheet token `line2:<element>`): each gives ONE LEVEL MORE of
//!   that element's line-2 spell, in any order, never skipping; past Lv 3 Max MP +3; the element's Attunement when it
//!   is his first spell of it. Light's line 2 is Banish (the old `banish` token = `line2:light`, the Tree Sentinel's).
//! - Light: tier 1 Cure; tiers 2 / 3 Cura / Curaga; tier 4 Curaza + Holy (Banish only from its line-2 bosses).
//! - Attunement: an element's Attunement is unlocked as soon as he has ANY spell of it (Banish unlocks Light).
//! - Arcane (Y): Gravity Lv 1 known from the start; each boss of a line gives its next level; past the top Max MP +3.
//!   Zettaflare: one boss (Messmer); again = Max MP +3.
//! - Start (10): Fire and Water at tier 1 (with their Attunements), Gravity Lv 1. (DONLINE2, the user 2026-10-27: "Donald
//!   should not start with access to Flood or Flare": no line-2 level at the start.)
//! - Sora: a step of an element gives HIM his next tier too (up to -ga, tier 3); with -ga already, Max MP +3
//!   ([`sora_gain`]). Line-2 (Banish included), Arcane and Zettaflare steps give Sora nothing.
//!
//! The state is the ROSTER's (kh-partyhost keeps one, saved in the party record as `donald_magic`): Donald gets every
//! step even before he joins, and joins with it.

use super::element::{ElemSet, Element};
use super::spells::{ArcaneLine, SpellRef};
use super::tune::DonData;

/// One boss's step for Donald.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DonStep {
    /// One tier more of an element, UP TO -GA (a base-game boss; past -ga: Max MP +3). (DON-PROG, the user 2026-10-22:
    /// "Donald's ZA magic should be unlocked in Elden Ring's DLC")
    Element(Element),
    /// (DON-PROG) THE ELEMENT'S DLC BOSS: the -za tier and the ultimate, with every lower tier still missing first (nothing
    /// skipped); at -za already: Max MP +3.
    ElementDlc(Element),
    /// (DONLINE2) The next level of the element's LINE-2 spell (Flare .. Banish), from that line's own bosses; at Lv 3
    /// already: Max MP +3. Light's = Banish (the sheet's old `banish` token, the Tree Sentinel).
    Line2(Element),
    /// The next level of a Y line.
    Arcane(ArcaneLine),
}

impl DonStep {
    /// The sheet token (`magic:fire`, `line2:fire`, `banish` = `line2:light`, `arcane:meteor`, `arcane:zettaflare`).
    pub fn parse(token: &str) -> Option<DonStep> {
        let t = token.trim();
        if t.eq_ignore_ascii_case("banish") {
            return Some(DonStep::Line2(Element::Light)); // (DONLINE2) the old token: Light's line 2
        }
        let (k, v) = t.split_once(':')?;
        match k.trim() {
            "magic" | "element" => Element::from_key(v).map(DonStep::Element),
            "dlc" | "magic_dlc" => Element::from_key(v).map(DonStep::ElementDlc), // (DON-PROG)
            "line2" | "l2" => Element::from_key(v).map(DonStep::Line2), // (DONLINE2)
            "arcane" | "y" => ArcaneLine::from_key(v).map(DonStep::Arcane),
            _ => None,
        }
    }

    pub fn token(&self) -> String {
        match self {
            DonStep::Element(e) => format!("magic:{}", e.key()),
            DonStep::ElementDlc(e) => format!("dlc:{}", e.key()), // (DON-PROG)
            DonStep::Line2(e) => format!("line2:{}", e.key()), // (DONLINE2)
            DonStep::Arcane(l) => format!("arcane:{}", l.key()),
        }
    }
}

/// What a step gave Donald (the BONUS! panel's lines).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Grant {
    /// A line-1 tier (1..=4).
    Tier(Element, u8),
    /// A line-2 level (1..=3).
    Line2(Element, u8),
    Ultimate(Element),
    /// The element's Attunement became available.
    Attunement(Element),
    /// A Y line's level (Zettaflare: level 1).
    Arcane(ArcaneLine, u8),
    /// Max MP +n (a step at the top).
    MaxMp(i32),
}

impl Grant {
    /// The panel's words ("Firaza", "Flare Lv 2", "Banish II", "Fire Attunement", "Graviga", "Max MP +3").
    pub fn label(&self, d: &DonData) -> String {
        match *self {
            Grant::Tier(e, t) => SpellRef::Line1(e, t).name(d).to_string(),
            Grant::Line2(e, l) => SpellRef::Line2(e, l).name(d).to_string(),
            Grant::Ultimate(e) => SpellRef::Ultimate(e).name(d).to_string(),
            Grant::Attunement(e) => format!("{} Attunement", e.label()),
            Grant::Arcane(l, lv) => SpellRef::Arcane(l, lv).name(d).to_string(),
            Grant::MaxMp(n) => format!("Max MP +{n}"),
        }
    }

    /// A magic grant (KH3's magic icon on the panel, ER_PHASE_NOTES 2026-10-19), not a stat line.
    pub fn is_magic(&self) -> bool {
        !matches!(self, Grant::MaxMp(_))
    }
}

/// Sora's share of a step (SORA_CURRENT 5, "Planned").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoraGain {
    /// His next tier of the element (1..=3).
    Tier(u8),
    /// He already has -ga: Max MP +n.
    MaxMp(i32),
    Nothing,
}

/// THE MP STEP of a step at the top (Donald's and Sora's): Max MP +3 (9).
pub const MAX_MP_STEP: i32 = 3;

/// Sora's gain from a step, given his current tier of that element (0..=3).
pub fn sora_gain(step: DonStep, sora_tier: u8) -> SoraGain {
    match step {
        DonStep::Element(_) | DonStep::ElementDlc(_) if sora_tier < 3 => SoraGain::Tier(sora_tier + 1), // (DON-PROG) the DLC boss too
        DonStep::Element(_) | DonStep::ElementDlc(_) => SoraGain::MaxMp(MAX_MP_STEP),
        DonStep::Line2(_) | DonStep::Arcane(_) => SoraGain::Nothing, // (DONLINE2) Banish included
    }
}

/// The learned state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DonLearned {
    /// Line-1 tier per element (0 = none, 4 = -za + the ultimate).
    pub tier: [u8; 6],
    /// Line-2 level per element (0..=3; Light's = Banish).
    pub line2: [u8; 6],
    /// Comet, Gravity, Meteor, Ultima, Zettaflare levels (ArcaneLine order).
    pub arcane: [u8; 5],
    /// Max MP +3 steps received (steps past a line's top).
    pub mp_steps: u8,
}

impl Default for DonLearned {
    fn default() -> DonLearned {
        DonLearned::start()
    }
}

impl DonLearned {
    /// Nothing at all (a test's blank).
    pub const EMPTY: DonLearned = DonLearned { tier: [0; 6], line2: [0; 6], arcane: [0; 5], mp_steps: 0 };

    /// THE START (10): Fire and Water at tier 1 (Fire, Water, their Attunements), Gravity Lv 1. (DONLINE2, the user
    /// 2026-10-27: "Donald should not start with access to Flood or Flare": no line-2 level.)
    pub fn start() -> DonLearned {
        let mut l = DonLearned::EMPTY;
        l.tier[Element::Fire.index()] = 1;
        l.tier[Element::Water.index()] = 1;
        l.arcane[ArcaneLine::Gravity as usize] = 1;
        l
    }

    /// Every spell of every line at the top (the test level's loadout).
    pub fn all() -> DonLearned {
        DonLearned { tier: [4; 6], line2: [3; 6], arcane: [2, 3, 3, 1, 1], mp_steps: 0 }
    }

    pub fn tier(&self, e: Element) -> u8 {
        self.tier[e.index()]
    }

    pub fn line2(&self, e: Element) -> u8 {
        self.line2[e.index()]
    }

    pub fn arcane(&self, l: ArcaneLine) -> u8 {
        self.arcane[l as usize]
    }

    /// The element's Attunement is unlocked (any spell of it, 4.1).
    pub fn attune_unlocked(&self, e: Element) -> bool {
        self.tier(e) > 0 || self.line2(e) > 0
    }

    /// The elements he has any spell of (the Attunement menu, the Zettaflare tracker's list).
    pub fn elements(&self) -> ElemSet {
        let mut s = ElemSet::NONE;
        for e in Element::ALL.into_iter().filter(|e| self.attune_unlocked(*e)) {
            s.insert(e);
        }
        s
    }

    /// He knows this spell (a hybrid: both elements unlocked; the Dual Attunement ability is the caller's rule).
    pub fn knows(&self, s: SpellRef) -> bool {
        match s {
            SpellRef::Line1(e, t) => t >= 1 && self.tier(e) >= t,
            SpellRef::Line2(e, l) => l >= 1 && self.line2(e) >= l,
            SpellRef::Ultimate(e) => self.tier(e) >= 4,
            SpellRef::Hybrid(a, b) => a != b && self.attune_unlocked(a) && self.attune_unlocked(b),
            SpellRef::Arcane(l, lv) => lv >= 1 && self.arcane(l) >= lv,
            SpellRef::Release(e) => self.attune_unlocked(e),
        }
    }

    /// The level of the element's line-2 spell he casts (None: not learned).
    pub fn line2_spell(&self, e: Element) -> Option<SpellRef> {
        let l = self.line2(e);
        (l > 0).then_some(SpellRef::Line2(e, l))
    }

    /// The Y line's spell at the level he knows.
    pub fn arcane_spell(&self, l: ArcaneLine) -> Option<SpellRef> {
        let lv = self.arcane(l);
        (lv > 0).then_some(SpellRef::Arcane(l, lv))
    }

    /// Donald's extra max MP from steps at the top (`mp_steps` x 3): DON-KIT's `max_mp` adds it.
    pub fn mp_bonus(&self) -> i32 {
        self.mp_steps as i32 * MAX_MP_STEP
    }

    fn mp_step(&mut self, out: &mut Vec<Grant>) {
        self.mp_steps = self.mp_steps.saturating_add(1);
        out.push(Grant::MaxMp(MAX_MP_STEP));
    }

    /// (DONLINE2) One level more of `e`'s line-2 spell (Banish for Light), the Attunement when it is his first spell of
    /// `e`; at Lv 3 already: Max MP +3 (the tier steps' rule).
    fn line2_up(&mut self, e: Element, out: &mut Vec<Grant>) {
        let i = e.index();
        if self.line2[i] >= 3 {
            self.mp_step(out);
            return;
        }
        let unlocked = self.attune_unlocked(e);
        self.line2[i] += 1;
        out.push(Grant::Line2(e, self.line2[i]));
        if !unlocked {
            out.push(Grant::Attunement(e));
        }
    }

    /// One tier more of `e` (below 4): its spell, the Attunement when new, the ultimate at 4. (DONLINE2) Line 2 / Banish
    /// is no longer raised here: its own bosses give it (`DonStep::Line2`).
    fn tier_up(&mut self, e: Element, out: &mut Vec<Grant>) {
        let i = e.index();
        let unlocked = self.attune_unlocked(e);
        self.tier[i] += 1;
        let t = self.tier[i];
        out.push(Grant::Tier(e, t));
        if !unlocked {
            out.push(Grant::Attunement(e));
        }
        if t == 4 {
            out.push(Grant::Ultimate(e));
        }
    }

    /// ONE BOSS STEP (9): what it gave, in panel order.
    pub fn apply(&mut self, step: DonStep) -> Vec<Grant> {
        let mut out = Vec::new();
        match step {
            DonStep::Line2(e) => self.line2_up(e, &mut out), // (DONLINE2)
            DonStep::Arcane(l) => {
                let i = l as usize;
                if self.arcane[i] >= l.max_level() {
                    self.mp_step(&mut out);
                } else {
                    self.arcane[i] += 1;
                    out.push(Grant::Arcane(l, self.arcane[i]));
                }
            }
            // (DON-PROG) a base-game boss: up to -ga; the DLC boss: -za + the ultimate, the missing tiers first
            DonStep::Element(e) => {
                if self.tier[e.index()] >= 3 {
                    self.mp_step(&mut out);
                    return out;
                }
                self.tier_up(e, &mut out);
            }
            DonStep::ElementDlc(e) => {
                if self.tier[e.index()] >= 4 {
                    self.mp_step(&mut out);
                    return out;
                }
                while self.tier[e.index()] < 4 {
                    self.tier_up(e, &mut out);
                }
            }
        }
        out
    }

    /// The record's parts (kh-partyhost writes them as `donald_magic`): tiers, line-2 levels, Arcane levels, MP steps.
    pub fn parts(&self) -> ([u8; 6], [u8; 6], [u8; 5], u8) {
        (self.tier, self.line2, self.arcane, self.mp_steps)
    }

    /// From a record's parts, clamped to the lines' ranges; missing entries are 0 (Gravity at least 1: he always knows it).
    pub fn from_parts(tier: &[u64], line2: &[u64], arcane: &[u64], mp_steps: u64) -> DonLearned {
        let mut l = DonLearned::EMPTY;
        for i in 0..6 {
            l.tier[i] = tier.get(i).copied().unwrap_or(0).min(4) as u8;
            l.line2[i] = line2.get(i).copied().unwrap_or(0).min(3) as u8;
        }
        for line in ArcaneLine::ALL {
            let i = line as usize;
            l.arcane[i] = arcane.get(i).copied().unwrap_or(0).min(line.max_level() as u64) as u8;
        }
        l.arcane[ArcaneLine::Gravity as usize] = l.arcane[ArcaneLine::Gravity as usize].max(1);
        l.mp_steps = mp_steps.min(255) as u8;
        l
    }
}
