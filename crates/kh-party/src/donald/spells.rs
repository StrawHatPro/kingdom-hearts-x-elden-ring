//! (DON1) WHAT DONALD CASTS (documentation/DONALD_OVERHAUL.md 4.3, 6): the spell references, their sheet rows
//! (`party-don-spells`), the MP price rule (6.2), and the command ids his menu / shortcut blocks store (7).
//!
//! Lines per element (6.1): line 1 = four tiers (Fire, Fira, Firaga, Firaza; Light: Cure .. Curaza), line 2 = three
//! levels (Flare, Freeze, Ball Lightning, Flood, Aero Barrier, Banish), line 3 = the ultimate (Inferno ...). Plus the 15
//! hybrids (6.4), the Y Arcane lines (6.3, 6.5) and the six Release bursts (4.3, free, never cast from a menu).

use super::element::{ElemSet, Element};
use super::tune::{DonData, DonSpellRow};

/// A Y Arcane line (6.3) and Zettaflare (6.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ArcaneLine {
    Comet,
    Gravity,
    Meteor,
    Ultima,
    Zettaflare,
}

impl ArcaneLine {
    /// The order of the Y list (cheapest first).
    pub const ALL: [ArcaneLine; 5] = [ArcaneLine::Comet, ArcaneLine::Gravity, ArcaneLine::Meteor, ArcaneLine::Ultima, ArcaneLine::Zettaflare];

    pub const fn key(self) -> &'static str {
        match self {
            ArcaneLine::Comet => "comet",
            ArcaneLine::Gravity => "gravity",
            ArcaneLine::Meteor => "meteor",
            ArcaneLine::Ultima => "ultima",
            ArcaneLine::Zettaflare => "zettaflare",
        }
    }

    pub fn from_key(k: &str) -> Option<ArcaneLine> {
        ArcaneLine::ALL.into_iter().find(|l| l.key().eq_ignore_ascii_case(k.trim()))
    }

    /// The top level (6.3 table): Comet 2, Gravity 3, Meteor 3, Ultima 1, Zettaflare 1.
    pub const fn max_level(self) -> u8 {
        match self {
            ArcaneLine::Comet => 2,
            ArcaneLine::Gravity | ArcaneLine::Meteor => 3,
            ArcaneLine::Ultima | ArcaneLine::Zettaflare => 1,
        }
    }
}

/// One castable spell (and its level).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpellRef {
    /// Line 1, tier 1..=4 (4 = the -za tier).
    Line1(Element, u8),
    /// Line 2, level 1..=3.
    Line2(Element, u8),
    /// Line 3: the element's ultimate.
    Ultimate(Element),
    /// A hybrid (6.4); always stored with the elements in menu order.
    Hybrid(Element, Element),
    /// A Y Arcane spell at its level.
    Arcane(ArcaneLine, u8),
    /// The Release burst of an element (4.3).
    Release(Element),
}

/// The 15 pairs in menu order (6.4).
pub fn hybrid_pairs() -> impl Iterator<Item = (Element, Element)> {
    Element::ALL.into_iter().enumerate().flat_map(|(i, a)| Element::ALL.into_iter().skip(i + 1).map(move |b| (a, b)))
}

impl SpellRef {
    /// The hybrid of two different elements (either order).
    pub fn hybrid(a: Element, b: Element) -> Option<SpellRef> {
        match a.index().cmp(&b.index()) {
            std::cmp::Ordering::Less => Some(SpellRef::Hybrid(a, b)),
            std::cmp::Ordering::Greater => Some(SpellRef::Hybrid(b, a)),
            std::cmp::Ordering::Equal => None,
        }
    }

    /// The sheet row id.
    pub fn id(&self) -> String {
        match *self {
            SpellRef::Line1(e, t) => format!("l1.{}.{}", e.key(), t.clamp(1, 4)),
            SpellRef::Line2(e, l) => format!("l2.{}.{}", e.key(), l.clamp(1, 3)),
            SpellRef::Ultimate(e) => format!("l3.{}", e.key()),
            SpellRef::Hybrid(a, b) => {
                let (a, b) = if a.index() <= b.index() { (a, b) } else { (b, a) };
                format!("hy.{}_{}", a.key(), b.key())
            }
            SpellRef::Arcane(l, lv) => format!("y.{}.{}", l.key(), lv.clamp(1, l.max_level())),
            SpellRef::Release(e) => format!("rel.{}", e.key()),
        }
    }

    /// The reference of a sheet row id (the inverse of [`SpellRef::id`]).
    pub fn from_id(id: &str) -> Option<SpellRef> {
        let mut it = id.trim().split('.');
        let line = it.next()?;
        let a = it.next()?;
        let n = it.next().and_then(|n| n.parse::<u8>().ok());
        Some(match line {
            "l1" => SpellRef::Line1(Element::from_key(a)?, n.filter(|t| (1..=4).contains(t))?),
            "l2" => SpellRef::Line2(Element::from_key(a)?, n.filter(|t| (1..=3).contains(t))?),
            "l3" => SpellRef::Ultimate(Element::from_key(a)?),
            "hy" => {
                let (x, y) = a.split_once('_')?;
                SpellRef::hybrid(Element::from_key(x)?, Element::from_key(y)?)?
            }
            "y" => {
                let l = ArcaneLine::from_key(a)?;
                SpellRef::Arcane(l, n.filter(|t| (1..=l.max_level()).contains(t))?)
            }
            "rel" => SpellRef::Release(Element::from_key(a)?),
            _ => return None,
        })
    }

    /// Its sheet row.
    pub fn row<'a>(&self, d: &'a DonData) -> Option<&'a DonSpellRow> {
        d.row(&self.id())
    }

    /// The menu's name ("Firaza", "Flare", "Banish II", "Thermal Shock", "Graviga").
    pub fn name(&self, d: &DonData) -> &'static str {
        self.row(d).map_or("?", |r| r.name)
    }

    /// The elements it counts as (a hybrid: both, 6.4; the Y magic and nothing else: none).
    pub fn elements(&self) -> ElemSet {
        match *self {
            SpellRef::Line1(e, _) | SpellRef::Line2(e, _) | SpellRef::Ultimate(e) | SpellRef::Release(e) => ElemSet::one(e),
            SpellRef::Hybrid(a, b) => ElemSet::of(&[a, b]),
            SpellRef::Arcane(..) => ElemSet::NONE,
        }
    }

    /// Its own element (a hybrid: the first of its pair).
    pub fn element(&self) -> Option<Element> {
        match *self {
            SpellRef::Line1(e, _) | SpellRef::Line2(e, _) | SpellRef::Ultimate(e) | SpellRef::Release(e) | SpellRef::Hybrid(e, _) => Some(e),
            SpellRef::Arcane(..) => None,
        }
    }

    /// A -za tier.
    pub fn is_za(&self) -> bool {
        matches!(self, SpellRef::Line1(_, 4))
    }

    /// A base tier (Fire, Blizzard ... Cure).
    pub fn is_base(&self) -> bool {
        matches!(self, SpellRef::Line1(_, 1))
    }

    pub fn is_ultimate(&self) -> bool {
        matches!(self, SpellRef::Ultimate(_))
    }

    /// Cast on a party member through the target list (6.1: Aero Barrier, Ball Lightning).
    /// (DON-UI) Cast on the ground only (the user, 2026-10-22): the ultimates, Ultima and Zettaflare.
    pub fn ground_only(&self) -> bool {
        matches!(self, SpellRef::Ultimate(_) | SpellRef::Arcane(ArcaneLine::Ultima | ArcaneLine::Zettaflare, _))
    }

    pub fn ally_target(&self) -> bool {
        matches!(self, SpellRef::Line2(Element::Aero | Element::Thunder, _))
    }
}

/// What the price depends on (6.2, 4.4, Spell Echo).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PriceCtx {
    /// Its element (a hybrid: both) is attuned now.
    pub attuned: bool,
    /// Within `res.quick_window` of that element reaching Resonance 3.
    pub quick: bool,
    /// Spell Echo makes it free.
    pub echo_free: bool,
}

/// THE PRICE RULE (6.2): the fixed MP cost before `cost.scale` (fixed MP since 2026-10-22; the Y spells excepted), or None when the spell cannot be cast in this state (a -za
/// tier / ultimate / hybrid unattuned). Attuned: the quick price when it has one and `quick`, else the attuned price.
/// Not attuned: the full price ("off-element casting costs full MP", 4.4).
pub fn price_pct(row: &DonSpellRow, ctx: PriceCtx) -> Option<f32> {
    if ctx.echo_free {
        return Some(0.0);
    }
    let p = if ctx.attuned {
        if ctx.quick && row.cost_quick >= 0.0 {
            row.cost_quick
        } else {
            row.cost_att
        }
    } else {
        row.cost_free
    };
    (p >= 0.0).then_some(p)
}

/// THE MP OF A SPELL (user, 2026-10-22: "donald should work like Sora where spells cost MP"): every spell but the Y
/// Arcane ones costs a fixed MP number from its row, like Sora's KH3 spells, times `cost.scale` (the host sets it to
/// Sora's difficulty multiplier, `magic.mp_cost_scale`). At least 1 for a priced spell, 0 for a free one.
pub fn mp_of_cost(cost: f32, scale: f32) -> i32 {
    if cost <= 0.0 {
        return 0;
    }
    ((cost * scale) - 1e-4).ceil().max(1.0) as i32
}

/// The MP of a share of max MP (only the Y Arcane spells and the Resonance refunds): `ceil(max MP x pct / 100 x scale)`.
pub fn mp_of_pct(max_mp: i32, pct: f32, scale: f32) -> i32 {
    if pct <= 0.0 || max_mp <= 0 {
        return 0;
    }
    ((max_mp as f32 * pct * scale / 100.0) - 1e-4).ceil().max(1.0) as i32
}

/// THE COMMAND IDS of Donald's menu and shortcut blocks (7). `ETresCommandKind` has no free values, so Donald's block
/// has its own id space (as Goofy's `0xC1..`, kh-core command/leader.rs), used ONLY inside Donald's block and lists:
/// - `0x80 + 4 x element + (tier - 1)`: a line-1 tier (24 ids, 0x80..=0x97);
/// - `0x98 + element`: the element's line-2 spell, cast at the level he knows (0x98..=0x9D);
/// - `0x9E + element`: the ultimate (0x9E..=0xA3);
/// - `0xA4 + n`: the n-th hybrid in [`hybrid_pairs`] order (0xA4..=0xB2);
/// - `0xB3 + element`: Attune to that element (0xB3..=0xB8).
/// The item commands 58..=68 stay as in every block. The Y Arcane spells and the Releases have no id (Y / automatic).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DonCmd {
    Spell(SpellRef),
    Attune(Element),
}

pub const CMD_FIRST: u8 = 0x80;
pub const CMD_LAST: u8 = 0xB8;

impl DonCmd {
    /// The id (None for a spell with no menu id: Y, Release). A line-2 spell's id ignores its level.
    pub fn id(&self) -> Option<u8> {
        Some(match *self {
            DonCmd::Spell(SpellRef::Line1(e, t)) => 0x80 + 4 * e.index() as u8 + t.clamp(1, 4) - 1,
            DonCmd::Spell(SpellRef::Line2(e, _)) => 0x98 + e.index() as u8,
            DonCmd::Spell(SpellRef::Ultimate(e)) => 0x9E + e.index() as u8,
            DonCmd::Spell(SpellRef::Hybrid(a, b)) => {
                let s = SpellRef::hybrid(a, b)?;
                0xA4 + hybrid_pairs().position(|p| SpellRef::Hybrid(p.0, p.1) == s)? as u8
            }
            DonCmd::Attune(e) => 0xB3 + e.index() as u8,
            DonCmd::Spell(SpellRef::Arcane(..) | SpellRef::Release(_)) => return None,
        })
    }

    /// The command of an id; a line-2 id comes back at level 1 (the caster picks the level he knows).
    pub fn from_id(id: u8) -> Option<DonCmd> {
        Some(match id {
            0x80..=0x97 => {
                let n = id - 0x80;
                DonCmd::Spell(SpellRef::Line1(Element::from_index((n / 4) as usize)?, n % 4 + 1))
            }
            0x98..=0x9D => DonCmd::Spell(SpellRef::Line2(Element::from_index((id - 0x98) as usize)?, 1)),
            0x9E..=0xA3 => DonCmd::Spell(SpellRef::Ultimate(Element::from_index((id - 0x9E) as usize)?)),
            0xA4..=0xB2 => {
                let (a, b) = hybrid_pairs().nth((id - 0xA4) as usize)?;
                DonCmd::Spell(SpellRef::Hybrid(a, b))
            }
            0xB3..=0xB8 => DonCmd::Attune(Element::from_index((id - 0xB3) as usize)?),
            _ => return None,
        })
    }

    /// AN OLD SAVE'S DONALD BLOCK (stream X2 stored Sora's spell commands, 29..=51 and the S_ grand ids): the same
    /// element and tier as Donald's line-1 command (Sora's -za / "gan" ids = the -za tier).
    pub fn from_sora_cmd(cmd: u8) -> Option<DonCmd> {
        if (29..=52).contains(&cmd) {
            let n = cmd - 29;
            return Some(DonCmd::Spell(SpellRef::Line1(Element::from_index((n / 4) as usize)?, n % 4 + 1)));
        }
        let (e, t) = match cmd {
            110..=112 => (Element::Fire, cmd - 108),
            113..=115 => (Element::Blizzard, cmd - 111),
            116..=118 => (Element::Thunder, cmd - 114),
            119..=121 => (Element::Water, cmd - 117),
            122..=124 => (Element::Aero, cmd - 120),
            125 => (Element::Light, 4),
            _ => return None,
        };
        Some(DonCmd::Spell(SpellRef::Line1(e, t)))
    }
}
