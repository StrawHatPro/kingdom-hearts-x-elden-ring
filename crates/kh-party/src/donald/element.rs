//! (DON1) Donald's six elements (documentation/DONALD_OVERHAUL.md 4.1: Fire, Blizzard, Thunder, Water, Aero, Light) and a
//! small set of them.

use super::tune::DonTune;

/// One of Donald's elements. Light covers healing and holy magic (the Cure line, Banish, Holy).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Element {
    Fire,
    Blizzard,
    Thunder,
    Water,
    Aero,
    Light,
}

impl Element {
    /// The menu's order (6.1, 7).
    pub const ALL: [Element; 6] = [Element::Fire, Element::Blizzard, Element::Thunder, Element::Water, Element::Aero, Element::Light];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Option<Element> {
        Element::ALL.get(i).copied()
    }

    /// The sheet's / the save's key ("fire" ... "light").
    pub const fn key(self) -> &'static str {
        match self {
            Element::Fire => "fire",
            Element::Blizzard => "blizzard",
            Element::Thunder => "thunder",
            Element::Water => "water",
            Element::Aero => "aero",
            Element::Light => "light",
        }
    }

    pub fn from_key(key: &str) -> Option<Element> {
        let k = key.trim();
        Element::ALL.into_iter().find(|e| e.key().eq_ignore_ascii_case(k)).or_else(|| k.eq_ignore_ascii_case("cure").then_some(Element::Light))
    }

    /// The menu's English name ("Fire" ... "Light").
    pub const fn label(self) -> &'static str {
        match self {
            Element::Fire => "Fire",
            Element::Blizzard => "Blizzard",
            Element::Thunder => "Thunder",
            Element::Water => "Water",
            Element::Aero => "Aero",
            Element::Light => "Light",
        }
    }

    /// Its colour (0xRRGGBB, sheet `color.<key>`): the Resonance ring, missile tints, status icons.
    pub fn color(self, t: &DonTune) -> u32 {
        (match self {
            Element::Fire => t.color_fire,
            Element::Blizzard => t.color_blizzard,
            Element::Thunder => t.color_thunder,
            Element::Water => t.color_water,
            Element::Aero => t.color_aero,
            Element::Light => t.color_light,
        }) as u32
    }

    /// kh-core's element (Sora's spells; Cure = Light).
    pub fn of_sora(e: kh_core::magic::spells::Element) -> Element {
        use kh_core::magic::spells::Element as S;
        match e {
            S::Fire => Element::Fire,
            S::Blizzard => Element::Blizzard,
            S::Thunder => Element::Thunder,
            S::Water => Element::Water,
            S::Aero => Element::Aero,
            S::Cure => Element::Light,
        }
    }

    /// kh-core's element of the same family (Light = Cure).
    pub fn to_sora(self) -> kh_core::magic::spells::Element {
        use kh_core::magic::spells::Element as S;
        match self {
            Element::Fire => S::Fire,
            Element::Blizzard => S::Blizzard,
            Element::Thunder => S::Thunder,
            Element::Water => S::Water,
            Element::Aero => S::Aero,
            Element::Light => S::Cure,
        }
    }

    pub const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// A set of elements (a hit's elements, the attuned pair, the elements that reached Resonance 3 ...).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct ElemSet(pub u8);

impl ElemSet {
    pub const NONE: ElemSet = ElemSet(0);
    pub const ALL: ElemSet = ElemSet(0b11_1111);

    pub fn one(e: Element) -> ElemSet {
        ElemSet(e.bit())
    }

    pub fn of(list: &[Element]) -> ElemSet {
        ElemSet(list.iter().fold(0, |m, e| m | e.bit()))
    }

    pub fn has(self, e: Element) -> bool {
        self.0 & e.bit() != 0
    }

    pub fn insert(&mut self, e: Element) {
        self.0 |= e.bit();
    }

    pub fn remove(&mut self, e: Element) {
        self.0 &= !e.bit();
    }

    pub fn union(self, o: ElemSet) -> ElemSet {
        ElemSet(self.0 | o.0)
    }

    /// Every element of `o` is in this set.
    pub fn covers(self, o: ElemSet) -> bool {
        self.0 & o.0 == o.0
    }

    pub fn is_empty(self) -> bool {
        self.0 & 0b11_1111 == 0
    }

    pub fn len(self) -> usize {
        (self.0 & 0b11_1111).count_ones() as usize
    }

    /// The elements in menu order.
    pub fn iter(self) -> impl Iterator<Item = Element> {
        Element::ALL.into_iter().filter(move |e| self.has(*e))
    }
}
