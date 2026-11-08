//! (party X2) CUSTOMIZE FOLLOWS THE LEADER: each friend leader's own Magic page (labelled "Abilities" for Goofy) and
//! shortcut sets A / B / C (build/sora_tmp/party/XD_PLAYABLE_KITS.md section 3, approved by the user 2026-10-15).
//! KH3 has no playable Donald / Goofy, so the STORAGE and the LISTS are the port's design; the RULES are Sora's
//! Customize rules (UI spec 10.6, magic spec J 2.2 / 3.1, `CommandState::fill` / `learn` / `set_magic_slot`,
//! `customize::assignable`) applied to the friend's own block:
//!
//! - Donald: six magic slots of SORA'S spells (what Sora has learned is what Donald casts while he leads: one shared
//!   progression, [`MagicBook`]); first filled as a copy of Sora's slot order; a newly learned tier replaces a lower
//!   one of its element unless the tier was chosen by hand; an element no slot holds goes into the first empty slot.
//!   Shortcut list: every learned tier of every element (each its own entry), then his pouch items. First-switch
//!   shortcut set A: the best Fire / Blizzard / Thunder / Cure (Circle, Triangle, Square, Cross), B and C empty.
//! - Goofy (GF-ARTS, documentation/GOOFY_OVERHAUL.md 8, PLAN.md R1, R19): two equipped Shield Arts on LB + A and LB + B, Taunt
//!   fixed on LB + X, the items on LB + Y. Goofy has no magic: his panel's Arts are the same on all three pages, and Y's item is
//!   each page's own. An Art he learns fills an empty slot. Ranks come from kh-party (`goofy::ArtBook`, copied in by the host).
//!
//! GOOFY'S IDS: his entries need command kinds for the menus and the save; `ETresCommandKind` has no free values, so
//! Goofy's list has its own id space [`goofy`] (`0xCA ..= 0xD0`), used ONLY inside Goofy's block and lists (never
//! executed as a Sora command). An old save's tiered ids `0xC1 ..= 0xC9` are migrated. Item entries are the item commands
//! 58 ..= 68 in every block.

use crate::magic::items::{command_of_item, item_of_command};
use crate::magic::spells::{self, Element};
use crate::status::{ItemKind, Status};

pub use super::customize::{Assignable, Tab};

/// Whose block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LeaderKind {
    Donald,
    Goofy,
}

impl LeaderKind {
    pub const ALL: [LeaderKind; 2] = [LeaderKind::Donald, LeaderKind::Goofy];

    /// The save key ("donald" / "goofy", as `kh_host::formation::key_of`).
    pub const fn key(self) -> &'static str {
        match self {
            LeaderKind::Donald => "donald",
            LeaderKind::Goofy => "goofy",
        }
    }

    /// The label of the Magic row / page / tab ("Abilities" for Goofy: the user's request).
    pub const fn list_label(self) -> &'static str {
        match self {
            LeaderKind::Donald => "Magic",
            LeaderKind::Goofy => "Abilities",
        }
    }

    /// The number of slots of the page.
    pub const fn slot_count(self) -> usize {
        match self {
            LeaderKind::Donald => 6,
            LeaderKind::Goofy => 2,
        }
    }
}

/// The asset order of the Customize lists (`CustomizeDataAsset.MagicCommands`, UI spec 10.6).
pub const ASSET_ORDER: [Element; 6] = [Element::Fire, Element::Blizzard, Element::Thunder, Element::Water, Element::Aero, Element::Cure];

/// (GF-ARTS) GOOFY'S OWN ID SPACE (port design, see the module text): one id per Art, the rank is the Art's own. His block
/// holds these and the item commands 58 ..= 68. kh-party's `pad::goofy_arts::Art` is the same numbering.
pub mod goofy {
    use super::{Assignable, Tab};
    use crate::magic::items::item_of_command;
    use crate::magic::spells;
    use crate::status::ItemKind;

    /// An old save's tiered abilities (index x 3 + tier - 1 + `OLD_FIRST`: Tornado, Turbo, Discus).
    pub const OLD_FIRST: u8 = 0xC1;
    pub const OLD_LAST: u8 = 0xC9;
    pub const CHARGE: u8 = 0xCA;
    pub const TORNADO: u8 = 0xCB;
    pub const BASH: u8 = 0xCC;
    pub const TURBO: u8 = 0xCD;
    pub const INTERPOSE: u8 = 0xCE;
    pub const SHIELD_TOSS: u8 = 0xCF;
    /// Fixed on LB + X: never one of the two equipped.
    pub const TAUNT: u8 = 0xD0;
    pub const FIRST: u8 = CHARGE;
    pub const LAST: u8 = TAUNT;
    /// The Arts he may equip, in the doc's order.
    pub const ARTS: [u8; 6] = [CHARGE, TORNADO, BASH, TURBO, INTERPOSE, SHIELD_TOSS];

    /// The panel's slots (`shortcut[page][slot]`, Sora's order: B, Y, X, A).
    pub const SLOT_B: usize = 0;
    pub const SLOT_ITEM: usize = 1;
    pub const SLOT_TAUNT: usize = 2;
    pub const SLOT_A: usize = 3;

    pub const fn is_goofy(kind: u8) -> bool {
        kind >= FIRST && kind <= LAST
    }

    /// One of the two-slot Arts (Taunt is not).
    pub const fn is_art(kind: u8) -> bool {
        kind >= FIRST && kind < TAUNT
    }

    const fn is_old(kind: u8) -> bool {
        kind >= OLD_FIRST && kind <= OLD_LAST
    }

    /// AN OLD SAVE'S ENTRY: Tornado and Turbo at any tier are those Arts, Discus is Shield Toss; his own ids and the rest unchanged.
    pub const fn migrate(kind: u8) -> u8 {
        match kind {
            0xC1..=0xC3 => TORNADO,
            0xC4..=0xC6 => TURBO,
            0xC7..=0xC9 => SHIELD_TOSS,
            _ => kind,
        }
    }

    /// The display name (the doc's; KH3's own for Tornado and Turbo).
    pub const fn name(kind: u8) -> &'static str {
        match kind {
            CHARGE => "Goofy Charge",
            TORNADO => "Goofy Tornado",
            BASH => "Goofy Bash",
            TURBO => "Goofy Turbo",
            INTERPOSE => "Interpose",
            SHIELD_TOSS => "Shield Toss",
            TAUNT => "Taunt",
            _ => "",
        }
    }

    /// The Customize help line. "{0}" is the MP; "{1}" the Art's cooldown in seconds (COMPOSED, no KH3 string; THE USER
    /// 2026-11-02: "Goofy's abilities (except taunt) should cost more MP, and introduce a cooldown with a visible timer"). Taunt
    /// has none.
    pub const fn help(kind: u8) -> &'static str {
        match kind {
            CHARGE => "A shield-first rush that pushes enemies along and ends in a guard stance. MP Cost: {0}  Cooldown: {1} s",
            TORNADO => "A spinning shield whirl that hits all around and deflects projectiles while it spins. MP Cost: {0}  Cooldown: {1} s",
            BASH => "An overhead slam that staggers the enemy badly. MP Cost: {0}  Cooldown: {1} s",
            TURBO => "Rides the shield forward like a sled. X cancels it into Guard. MP Cost: {0}  Cooldown: {1} s",
            INTERPOSE => "Dashes in front of the nearest threatened friend and deflects the next hit aimed at them. MP Cost: {0}  Cooldown: {1} s",
            SHIELD_TOSS => "Throws the shield, which returns. While it is out he can only guard bare-handed. MP Cost: {0}  Cooldown: {1} s",
            TAUNT => "Hollers and bangs the shield: nearby enemies turn on him. Always on <icon(L1)> + <icon(Square)>. MP Cost: {0}", // (BUTTON-ICONS) icons
            _ => "",
        }
    }

    /// WHAT GOOFY HAS LEARNED as his block reads it (kh-party's ranks, copied in by the host every step: kh-partyhost custom.rs).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ArtBook {
        /// Rank per Art, index `id - FIRST` (0 = not learned).
        pub ranks: [u8; 7],
        /// The ranks are his (the host fed them): an Art above them may be cleared. Unfed, the block only keeps what it holds.
        pub fed: bool,
    }

    impl Default for ArtBook {
        fn default() -> ArtBook {
            ArtBook::START
        }
    }

    impl ArtBook {
        /// The start (the doc's 11.2): Goofy Charge and Taunt.
        pub const START: ArtBook = ArtBook { ranks: [1, 0, 0, 0, 0, 0, 1], fed: false };

        pub fn rank(&self, kind: u8) -> u8 {
            if is_goofy(kind) { self.ranks[(kind - FIRST) as usize] } else { 0 }
        }

        pub fn knows(&self, kind: u8) -> bool {
            self.rank(kind) > 0
        }
    }

    /// THE ARTS HE MAY EQUIP NOW: the learned ones, Taunt not among them (Customize's list).
    pub fn arts(book: &ArtBook) -> Vec<Assignable> {
        ARTS.into_iter().filter(|k| book.knows(*k)).map(|kind| Assignable { kind, name: name(kind), tab: Tab::Magic, auto_reload: false }).collect()
    }

    /// The list of a slot of his panel: the Arts, then the items (`items`: his pouch and the shared stock).
    pub fn assignable(book: &ArtBook, items: &[ItemKind]) -> Vec<Assignable> {
        let mut out = arts(book);
        for cmd in 58u8..=68 {
            if item_of_command(cmd).is_some_and(|i| items.contains(&i)) {
                out.push(Assignable { kind: cmd, name: spells::command_name(cmd), tab: Tab::Items, auto_reload: false });
            }
        }
        out
    }

    /// The first two Arts he knows: [LB + A, LB + B] of a block never filled (0 = empty).
    pub fn default_arts(book: &ArtBook) -> [u8; 2] {
        let mut known = ARTS.into_iter().filter(|k| book.knows(*k));
        [known.next().unwrap_or(0), known.next().unwrap_or(0)]
    }

    /// The page holds what the old layout allowed: an old id, an item off Y's slot, an Art on it.
    pub(super) fn old_layout(page: &[u8; 4]) -> bool {
        page.iter().enumerate().any(|(slot, k)| is_old(*k) || (slot == SLOT_ITEM && is_goofy(*k)) || (slot != SLOT_ITEM && item_of_command(*k).is_some()))
    }
}

/// (DON-UI) DONALD'S OWN ID SPACE (the Donald overhaul, documentation/DONALD_OVERHAUL.md 7; kh-party `donald::DonCmd`
/// is the same numbering, its tests check the two agree): his block holds these ids and the item commands 58 ..= 68,
/// never Sora's spell commands (an old save's are migrated by [`donald::migrate`]). Element index = Fire, Blizzard,
/// Thunder, Water, Aero, Light (Cure), as [`ASSET_ORDER`].
/// - `0x80 + 4 x element + tier - 1`: a line-1 tier (Fire .. Firaza, Cure .. Curaza);
/// - `0x98 + element`: the element's line-2 spell (Flare, Freeze, Ball Lightning, Flood, Aero Barrier, Banish), cast at
///   the level he knows;
/// - `0x9E + element`: the ultimate (Inferno ...);
/// - `0xA4 + n`: the n-th hybrid (pairs in menu order: Fire + Blizzard, Fire + Thunder ... Aero + Light);
/// - `0xB3 + element`: Attune to that element.
pub mod donald {
    use super::{Assignable, Tab};
    use crate::magic::items::item_of_command;
    use crate::magic::spells;
    use crate::status::ItemKind;

    pub const FIRST: u8 = 0x80;
    pub const LINE2: u8 = 0x98;
    pub const ULTIMATE: u8 = 0x9E;
    pub const HYBRID: u8 = 0xA4;
    pub const ATTUNE: u8 = 0xB3;
    /// Unattune: back to his plain missiles (the user, 2026-10-22); not a kh-party `DonCmd` (the menu's own action).
    pub const UNATTUNE: u8 = 0xB9;
    pub const LAST: u8 = 0xB9;
    pub const ELEMENTS: usize = 6;

    /// One entry of his block.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum Entry {
        /// (element, tier 1 ..= 4)
        Line1(usize, u8),
        Line2(usize),
        Ultimate(usize),
        /// (first element, second element), first < second
        Hybrid(usize, usize),
        Attune(usize),
        Unattune,
    }

    pub const fn is_donald(kind: u8) -> bool {
        kind >= FIRST && kind <= LAST
    }

    pub const fn line1(element: usize, tier: u8) -> u8 {
        let t = if tier < 1 { 1 } else if tier > 4 { 4 } else { tier };
        FIRST + 4 * element as u8 + t - 1
    }

    pub const fn line2(element: usize) -> u8 {
        LINE2 + element as u8
    }

    pub const fn ultimate(element: usize) -> u8 {
        ULTIMATE + element as u8
    }

    pub const fn attune(element: usize) -> u8 {
        ATTUNE + element as u8
    }

    /// The 15 pairs in menu order.
    pub fn pairs() -> impl Iterator<Item = (usize, usize)> {
        (0..ELEMENTS).flat_map(|a| (a + 1..ELEMENTS).map(move |b| (a, b)))
    }

    /// The hybrid id of two different elements (either order).
    pub fn hybrid(a: usize, b: usize) -> Option<u8> {
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        pairs().position(|p| p == (a, b)).map(|n| HYBRID + n as u8)
    }

    pub fn split(kind: u8) -> Option<Entry> {
        Some(match kind {
            0x80..=0x97 => Entry::Line1(((kind - FIRST) / 4) as usize, (kind - FIRST) % 4 + 1),
            0x98..=0x9D => Entry::Line2((kind - LINE2) as usize),
            0x9E..=0xA3 => Entry::Ultimate((kind - ULTIMATE) as usize),
            0xA4..=0xB2 => {
                let (a, b) = pairs().nth((kind - HYBRID) as usize)?;
                Entry::Hybrid(a, b)
            }
            0xB3..=0xB8 => Entry::Attune((kind - ATTUNE) as usize),
            UNATTUNE => Entry::Unattune,
            _ => return None,
        })
    }

    /// The element an entry belongs to (a hybrid: its first).
    pub fn element_of(kind: u8) -> Option<usize> {
        match split(kind)? {
            Entry::Line1(e, _) | Entry::Line2(e) | Entry::Ultimate(e) | Entry::Attune(e) | Entry::Hybrid(e, _) => Some(e),
            Entry::Unattune => None,
        }
    }

    const LINE1_NAMES: [[&str; 4]; ELEMENTS] = [
        ["Fire", "Fira", "Firaga", "Firaza"],
        ["Blizzard", "Blizzara", "Blizzaga", "Blizzaza"],
        ["Thunder", "Thundara", "Thundaga", "Thundaza"],
        ["Water", "Watera", "Waterga", "Waterza"],
        ["Aero", "Aerora", "Aeroga", "Aeroza"],
        ["Cure", "Cura", "Curaga", "Curaza"],
    ];
    const LINE2_NAMES: [&str; ELEMENTS] = ["Flare", "Freeze", "Ball Lightning", "Flood", "Aero Barrier", "Banish"];
    const ULTIMATE_NAMES: [&str; ELEMENTS] = ["Inferno", "Diamond Dust", "Judgment Bolt", "Tsunami", "Aerial Blast", "Holy"];
    const HYBRID_NAMES: [&str; 15] = [
        "Thermal Shock",
        "Plasma",
        "Scald",
        "Fire Whirl",
        "Sunburst",
        "Hailstorm",
        "Glacier",
        "Whiteout",
        "Prism Frost",
        "Thunderstorm",
        "Tempest",
        "Divine Spark",
        "Maelstrom",
        "Shining Rain",
        "Radiant Gale",
    ];
    /// The element names (the Attunement submenu's rows).
    pub const ELEMENT_NAMES: [&str; ELEMENTS] = ["Fire", "Blizzard", "Thunder", "Water", "Aero", "Light"];
    const ATTUNE_NAMES: [&str; ELEMENTS] = ["Attune Fire", "Attune Blizzard", "Attune Thunder", "Attune Water", "Attune Aero", "Attune Light"];

    /// The display name of an id (the sheet `party-don-spells` names; a line-2 entry without its level).
    pub fn name(kind: u8) -> &'static str {
        match split(kind) {
            Some(Entry::Line1(e, t)) => LINE1_NAMES[e][t as usize - 1],
            Some(Entry::Line2(e)) => LINE2_NAMES[e],
            Some(Entry::Ultimate(e)) => ULTIMATE_NAMES[e],
            Some(Entry::Hybrid(a, b)) => hybrid(a, b).map_or("", |id| HYBRID_NAMES[(id - HYBRID) as usize]),
            Some(Entry::Attune(e)) => ATTUNE_NAMES[e],
            Some(Entry::Unattune) => "Unattune",
            None => "",
        }
    }

    const LINE2_HELP: [&str; ELEMENTS] = [
        "A white-hot star that detonates on the target. Consumes Burn for more damage.",
        "Encases the target in ice. Large foes are slowed instead.",
        "Orbs of lightning circle a party member and strike nearby foes.",
        "A wave that pushes foes away and leaves them Wet.",
        "A wind barrier that cuts the damage a party member takes.",
        "A ray of holy light that Reveals the target.",
    ];

    /// The Customize help line of an id (DONALD_OVERHAUL.md 4, 6).
    pub fn help(kind: u8) -> String {
        let el = |e: usize| ELEMENT_NAMES[e];
        match split(kind) {
            Some(Entry::Line1(5, 4)) => "The strongest healing magic. Castable only at full-power Light attunement.".to_string(),
            Some(Entry::Line1(5, _)) => format!("{}: restores HP. Cheaper while attuned to Light.", name(kind)),
            Some(Entry::Line1(e, 4)) => format!("The strongest {} magic. Castable only at full-power attunement (Resonance 3).", el(e)),
            Some(Entry::Line1(e, _)) => format!("{} magic. Cheaper while attuned to {}.", el(e), el(e)),
            Some(Entry::Line2(e)) => LINE2_HELP[e].to_string(),
            Some(Entry::Ultimate(e)) => format!("{}'s ultimate. Full-power attunement only, once per attunement period.", el(e)),
            Some(Entry::Hybrid(a, b)) => format!("Hybrid of {} and {}. Needs Dual Attunement with both attuned.", el(a), el(b)),
            Some(Entry::Attune(e)) => format!("Attune to {}: it changes his missiles, dodge, finisher, spell costs and party aura.", el(e)),
            Some(Entry::Unattune) => "Let go of every attunement: plain arcane missiles again. An element at Resonance 3 is Released.".to_string(),
            None => String::new(),
        }
    }

    /// AN OLD SAVE'S ENTRY (stream X2 stored Sora's spell commands 29 ..= 52 and the grand ids 110 ..= 125 in Donald's
    /// block): the same element and tier as his line-1 id (kh-party `DonCmd::from_sora_cmd`); his own ids and the item
    /// commands unchanged; anything else 0.
    pub fn migrate(kind: u8) -> u8 {
        if kind == 0 || is_donald(kind) || item_of_command(kind).is_some() {
            return kind;
        }
        if (29..=52).contains(&kind) {
            let n = kind - 29;
            return line1((n / 4) as usize, n % 4 + 1);
        }
        match kind {
            110..=112 => line1(0, kind - 108),
            113..=115 => line1(1, kind - 111),
            116..=118 => line1(2, kind - 114),
            119..=121 => line1(3, kind - 117),
            122..=124 => line1(4, kind - 120),
            125 => line1(5, 4),
            _ => 0,
        }
    }

    /// WHAT DONALD HAS LEARNED as his block reads it (kh-party `DonLearned` + the Dual Attunement ability, copied in by
    /// the host every step: kh-partyhost custom.rs).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct DonBook {
        /// Line-1 tier per element (0 none, 4 = -za + the ultimate).
        pub tier: [u8; ELEMENTS],
        /// Line-2 level per element (0 none).
        pub line2: [u8; ELEMENTS],
        /// Dual Attunement equipped (the hybrids).
        pub dual: bool,
    }

    impl Default for DonBook {
        fn default() -> DonBook {
            DonBook::START
        }
    }

    impl DonBook {
        /// The start (DONALD_OVERHAUL.md 10): Fire and Water tier 1, Flare and Flood Lv 1.
        pub const START: DonBook = DonBook { tier: [1, 0, 0, 1, 0, 0], line2: [1, 0, 0, 1, 0, 0], dual: false };

        /// The element's Attunement (any spell of it).
        pub fn unlocked(&self, e: usize) -> bool {
            e < ELEMENTS && (self.tier[e] > 0 || self.line2[e] > 0)
        }

        /// He knows the entry (a hybrid needs Dual Attunement and both elements; items are not his book's).
        pub fn knows(&self, kind: u8) -> bool {
            match split(kind) {
                Some(Entry::Line1(e, t)) => self.tier[e] >= t,
                Some(Entry::Line2(e)) => self.line2[e] > 0,
                Some(Entry::Ultimate(e)) => self.tier[e] >= 4,
                Some(Entry::Hybrid(a, b)) => self.dual && self.unlocked(a) && self.unlocked(b),
                Some(Entry::Attune(e)) => self.unlocked(e),
                Some(Entry::Unattune) => (0..ELEMENTS).any(|e| self.unlocked(e)),
                None => false,
            }
        }

        /// The best line-1 tier he may put on a slot by default (up to -ga: the -za needs full power, R14).
        pub fn best(&self, e: usize) -> Option<u8> {
            let t = self.tier[e].min(3);
            (t > 0).then(|| line1(e, t))
        }
    }

    /// THE SHORTCUT LIST (Customize): per element, every learned line-1 tier, the line-2 spell, the ultimate, then the
    /// hybrids (with Dual Attunement), then the six Attunements he has, then the items (`items`: his pouch / the shared
    /// stock).
    pub fn assignable(book: &DonBook, items: &[ItemKind]) -> Vec<Assignable> {
        let mut out = Vec::new();
        let mut push = |kind: u8| out.push(Assignable { kind, name: name(kind), tab: Tab::Magic, auto_reload: false });
        for e in 0..ELEMENTS {
            for t in 1..=book.tier[e].min(4) {
                push(line1(e, t));
            }
            if book.line2[e] > 0 {
                push(line2(e));
            }
            if book.tier[e] >= 4 {
                push(ultimate(e));
            }
        }
        for (a, b) in pairs() {
            if let Some(id) = hybrid(a, b).filter(|id| book.knows(*id)) {
                push(id);
            }
        }
        for e in 0..ELEMENTS {
            if book.unlocked(e) {
                push(attune(e));
            }
        }
        if book.knows(UNATTUNE) {
            push(UNATTUNE); // (DON-UI) Unattune, after the Attunements
        }
        for cmd in 58u8..=68 {
            if item_of_command(cmd).is_some_and(|i| items.contains(&i)) {
                out.push(Assignable { kind: cmd, name: spells::command_name(cmd), tab: Tab::Items, auto_reload: false });
            }
        }
        out
    }

    /// The first-switch palettes (KH3 has three from the start): A = the first two elements' best line-1, the first
    /// line-2 spell, Cure (else the second element's line-2); B = the Attunements of the first four elements he has;
    /// C empty.
    pub fn default_shortcuts(book: &DonBook) -> [[u8; 4]; 3] {
        let known: Vec<usize> = (0..ELEMENTS).filter(|e| book.unlocked(*e)).collect();
        let bests: Vec<u8> = known.iter().filter_map(|e| book.best(*e)).filter(|id| element_of(*id) != Some(5)).collect();
        let line2s: Vec<u8> = known.iter().filter(|e| book.line2[**e] > 0).map(|e| line2(*e)).collect();
        let mut a = [bests.first().copied().unwrap_or(0), line2s.first().copied().unwrap_or(0), bests.get(1).copied().unwrap_or(0), book.best(5).unwrap_or(line2s.get(1).copied().unwrap_or(0))];
        // never the same entry twice on a palette
        for i in 1..4 {
            if a[i] != 0 && a[..i].contains(&a[i]) {
                a[i] = 0;
            }
        }
        let mut b = [0u8; 4];
        for (slot, e) in known.iter().take(4).enumerate() {
            b[slot] = attune(*e);
        }
        [a, b, [0; 4]]
    }
}

/// SORA'S MAGIC as the friends share it: the learned tier per element (asset order: Fire, Blizzard, Thunder, Water,
/// Aero, Cure; 0 = not learned) and his own magic slots (the copy source of Donald's first fill).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MagicBook {
    pub tiers: [u8; 6],
    pub sora_slots: [u8; 6],
}

impl MagicBook {
    /// Sora's book now (his status's `magic` list, his command slots).
    pub fn of(status: &Status, sora_slots: [u8; 6]) -> MagicBook {
        let mut tiers = [0u8; 6];
        for (i, e) in ASSET_ORDER.iter().enumerate() {
            tiers[i] = status.magic.iter().find(|m| Element::from_name(&m.name) == Some(*e)).map_or(0, |m| m.tier.min(3) as u8);
        }
        MagicBook { tiers, sora_slots }
    }

    /// The learned tier of an element (0 = not learned).
    pub fn tier(&self, element: Element) -> u8 {
        ASSET_ORDER.iter().position(|e| *e == element).map_or(0, |i| self.tiers[i])
    }

    /// A normal spell command whose tier is learned.
    pub fn learned(&self, cmd: u8) -> bool {
        spells::is_magic(cmd) && spells::unit(cmd).is_some_and(|u| !u.grand && u.tier <= self.tier(u.element))
    }

    /// The best learned tier of an element as a command (Firaga ...), None if not learned.
    pub fn best(&self, element: Element) -> Option<u8> {
        let t = self.tier(element);
        (t > 0).then(|| element.command(t))
    }
}

/// ONE LEADER'S CUSTOMIZE BLOCK (Sora's `CommandState` storage shape: `magicSlot[6]`, the hand-chosen bits S+0x5C,
/// `shortcut[3][4]` right / top / left / bottom face button; 0 = empty).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LeaderCustom {
    pub magic_slots: [u8; 6],
    pub tier_chosen: [bool; 6],
    pub shortcuts: [[u8; 4]; 3],
    /// Filled once (the first time he leads, or from the save).
    pub filled: bool,
    /// (DON-UI) Donald's learned magic as his block reads it (the host copies it in before each sync; not saved here:
    /// the roster's `donald_magic` record is the source). Unused in Goofy's block.
    pub don: donald::DonBook,
    /// (GF-ARTS) Goofy's learned Arts as his block reads it (the same; his ranks are the roster's). Unused in Donald's block.
    pub gof: goofy::ArtBook,
}

impl LeaderCustom {
    /// The block as the book stands: the first fill, then Sora's learn rule (0x14159EE70) for every learned tier.
    /// Returns true when anything changed.
    pub fn sync(&mut self, kind: LeaderKind, book: &MagicBook) -> bool {
        let before = *self;
        match kind {
            // (DON-UI) Donald's own block (the overhaul): his ids, his book (`self.don`), the old save's Sora ids migrated
            LeaderKind::Donald => self.sync_donald(),
            // (GF-ARTS) Goofy's own block: his Art ids, his book (`self.gof`), the old save's tiers migrated
            LeaderKind::Goofy => {
                let _ = book;
                self.sync_goofy();
            }
        }
        *self != before
    }

    /// `CommandState::learn` (0x14159EE70) on this block.
    pub fn learn(&mut self, cmd: u8) -> Option<usize> {
        let new = spells::unit(cmd).filter(|u| !u.grand)?;
        if let Some(slot) = self.magic_slots.iter().position(|k| spells::unit(*k).is_some_and(|u| u.element == new.element)) {
            let old = spells::unit(self.magic_slots[slot])?;
            if self.tier_chosen[slot] || old.tier >= new.tier {
                return None;
            }
            self.magic_slots[slot] = cmd;
            return Some(slot);
        }
        let slot = self.magic_slots.iter().position(|k| *k == 0)?;
        self.magic_slots[slot] = cmd;
        Some(slot)
    }

    /// The Customize tier choice (`set_magic_slot` 1:1): the slot must hold that element / ability, the tier learned.
    pub fn set_slot(&mut self, kind: LeaderKind, book: &MagicBook, slot: usize, cmd: u8) -> bool {
        let Some(&old) = self.magic_slots.get(slot) else { return false };
        let ok = match kind {
            // (DON-UI) his Magic page holds his elements' sub-lists: the entry casts the best tier itself (R14), so there is
            // no tier to choose (the order can still be changed: Reorder)
            LeaderKind::Donald => {
                let _ = (cmd, old, book);
                false
            }
            // (GF-ARTS) his page is the two equipped Arts: no tiers, the slot takes any learned Art
            LeaderKind::Goofy => {
                let _ = (old, book);
                return self.equip_art(slot, cmd);
            }
        };
        if !ok {
            return false;
        }
        if self.magic_slots[slot] != cmd {
            self.tier_chosen[slot] = true;
        }
        self.magic_slots[slot] = cmd;
        true
    }

    /// Triangle "Reorder": the two slots swap, kind and bit.
    pub fn swap(&mut self, kind: LeaderKind, a: usize, b: usize) -> bool {
        let n = kind.slot_count();
        if a >= n || b >= n {
            return false;
        }
        self.magic_slots.swap(a, b);
        self.tier_chosen.swap(a, b);
        if kind == LeaderKind::Goofy {
            let [x, y] = self.arts(); // (GF-ARTS) his page is the two Arts: the buttons swap with it
            self.write_arts([y, x]);
        }
        true
    }

    /// `shortcut[page][slot] = cmd` (0 = Remove); the command must be in the leader's list.
    pub fn set_shortcut(&mut self, kind: LeaderKind, book: &MagicBook, items: &[ItemKind], page: usize, slot: usize, cmd: u8) -> bool {
        if page >= 3 || slot >= 4 || (cmd != 0 && !self.list(kind, book, items).iter().any(|a| a.kind == cmd)) {
            return false;
        }
        // (GF-ARTS) Goofy's panel: A and B are the two Arts (on every page), X is Taunt, Y is an item
        if kind == LeaderKind::Goofy {
            return match slot {
                goofy::SLOT_A => self.equip_art(0, cmd),
                goofy::SLOT_B => self.equip_art(1, cmd),
                goofy::SLOT_ITEM if cmd == 0 || item_of_command(cmd).is_some() => {
                    self.shortcuts[page][slot] = cmd;
                    true
                }
                _ => false,
            };
        }
        self.shortcuts[page][slot] = cmd;
        true
    }

    /// (GF-ARTS) GOOFY'S TWO EQUIPPED ARTS: [LB + A, LB + B].
    pub fn arts(&self) -> [u8; 2] {
        [self.shortcuts[0][goofy::SLOT_A], self.shortcuts[0][goofy::SLOT_B]]
    }

    /// (GF-ARTS) EQUIPS A LEARNED ART on `slot` (0 LB + A, 1 LB + B, on every page). The Art already on the other slot swaps
    /// over. False: not an Art he knows (Taunt never is), or no such slot.
    pub fn equip_art(&mut self, slot: usize, art: u8) -> bool {
        if slot >= 2 || !goofy::is_art(art) || !self.gof.knows(art) {
            return false;
        }
        let mut arts = self.arts();
        if arts[1 - slot] == art {
            arts[1 - slot] = arts[slot];
        }
        arts[slot] = art;
        self.write_arts(arts);
        true
    }

    fn write_arts(&mut self, arts: [u8; 2]) {
        for page in self.shortcuts.iter_mut() {
            page[goofy::SLOT_A] = arts[0];
            page[goofy::SLOT_B] = arts[1];
            page[goofy::SLOT_TAUNT] = goofy::TAUNT;
        }
        self.magic_slots = [arts[0], arts[1], 0, 0, 0, 0];
    }

    /// (GF-ARTS) GOOFY'S BLOCK SYNC:
    /// - an old save's block (tiered ids, an item on any slot): its Arts in the order A, B, Y, X over the pages, its items onto
    ///   the pages' Y slots in order;
    /// - an Art he does not know (an older progression loaded) is cleared; an empty slot takes the first Art he knows that the
    ///   other slot does not hold;
    /// - the first fill: the first two Arts he knows and a Potion on Y.
    fn sync_goofy(&mut self) {
        let book = self.gof;
        let fresh = !self.filled;
        self.filled = true;
        self.tier_chosen = [false; 6];
        let mut arts = self.arts();
        if fresh {
            self.shortcuts[0][goofy::SLOT_ITEM] = spells::cmd::POTION;
        } else if self.shortcuts.iter().any(goofy::old_layout) {
            let (mut found, mut items) = (Vec::new(), Vec::new());
            for page in self.shortcuts.iter() {
                for slot in [goofy::SLOT_A, goofy::SLOT_B, goofy::SLOT_ITEM, goofy::SLOT_TAUNT] {
                    let k = goofy::migrate(page[slot]);
                    if goofy::is_art(k) && !found.contains(&k) {
                        found.push(k);
                    } else if item_of_command(k).is_some() {
                        items.push(k);
                    }
                }
            }
            arts = [found.first().copied().unwrap_or(0), found.get(1).copied().unwrap_or(0)];
            for (p, page) in self.shortcuts.iter_mut().enumerate() {
                page[goofy::SLOT_ITEM] = items.get(p).copied().unwrap_or(0);
            }
        }
        for a in arts.iter_mut() {
            if !goofy::is_art(*a) || (book.fed && !book.knows(*a)) {
                *a = 0;
            }
        }
        if arts[0] == arts[1] {
            arts[1] = 0;
        }
        for i in 0..2 {
            if arts[i] == 0 {
                arts[i] = goofy::ARTS.into_iter().find(|k| book.knows(*k) && !arts.contains(k)).unwrap_or(0);
            }
        }
        self.write_arts(arts);
    }

    /// (DON-UI) The leader's shortcut list (Customize): Donald's from his own book (`self.don`), Goofy's [`assignable`].
    pub fn list(&self, kind: LeaderKind, book: &MagicBook, items: &[ItemKind]) -> Vec<Assignable> {
        match kind {
            LeaderKind::Donald => donald::assignable(&self.don, items),
            LeaderKind::Goofy => {
                let _ = book;
                goofy::assignable(&self.gof, items) // (GF-ARTS) his own book
            }
        }
    }

    /// (DON-UI) DONALD'S BLOCK SYNC (the overhaul, DONALD_OVERHAUL.md 7):
    /// - an old save's Sora spell ids become his line-1 ids (`donald::migrate`), on the Magic page as the element's
    ///   entry; anything that is not his (or an item) is cleared;
    /// - the Magic page lists the elements he has (one entry each, its line-1 base id; the menu's sub-list order),
    ///   a new element in the first empty slot, an element he lost (an older progression loaded) removed;
    /// - the first fill: the three palettes of [`donald::default_shortcuts`];
    /// - a shortcut above what he knows: a line-1 tier falls to the learned one (else empty), another entry he does not
    ///   know is cleared (a hybrid stays while both elements are his: Dual Attunement may be taken off for a while).
    fn sync_donald(&mut self) {
        let book = self.don;
        let fresh = !self.filled;
        if fresh {
            self.filled = true;
            self.magic_slots = [0; 6];
            self.shortcuts = donald::default_shortcuts(&book);
        }
        self.tier_chosen = [false; 6];
        // the Magic page: one entry per element, migrated and normalised
        let mut seen = [false; donald::ELEMENTS];
        for s in self.magic_slots.iter_mut() {
            let e = donald::element_of(donald::migrate(*s)).filter(|e| book.unlocked(*e) && !seen[*e]);
            *s = match e {
                Some(e) => {
                    seen[e] = true;
                    donald::line1(e, 1)
                }
                None => 0,
            };
        }
        for (e, &placed) in seen.iter().enumerate() {
            if book.unlocked(e) && !placed {
                if let Some(slot) = self.magic_slots.iter().position(|k| *k == 0) {
                    self.magic_slots[slot] = donald::line1(e, 1);
                }
            }
        }
        // the shortcuts
        for page in self.shortcuts.iter_mut() {
            for k in page.iter_mut() {
                let m = donald::migrate(*k);
                *k = match donald::split(m) {
                    None => m,
                    Some(donald::Entry::Line1(e, t)) => {
                        let have = book.tier[e].min(t);
                        if have == 0 { 0 } else { donald::line1(e, have) }
                    }
                    Some(donald::Entry::Hybrid(a, b)) if book.unlocked(a) && book.unlocked(b) => m,
                    Some(_) if book.knows(m) => m,
                    Some(_) => 0,
                };
            }
        }
    }

    /// The slots that hold something, in page order.
    pub fn slots(&self, kind: LeaderKind) -> Vec<u8> {
        self.magic_slots.iter().take(kind.slot_count()).copied().filter(|k| *k != 0).collect()
    }

    /// The save shape (`party.custom.<key>`).
    pub fn to_json(&self) -> String {
        let n = |v: &[u8]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
        let b = |v: &[bool]| v.iter().map(|x| if *x { "1" } else { "0" }).collect::<Vec<_>>().join(",");
        format!("{{\"magic\":[{}],\"chosen\":[{}],\"shortcuts\":[[{}],[{}],[{}]],\"filled\":{}}}", n(&self.magic_slots), b(&self.tier_chosen), n(&self.shortcuts[0]), n(&self.shortcuts[1]), n(&self.shortcuts[2]), self.filled)
    }

    /// From the save shape (numbers as a host's JSON reader gives them): missing parts stay empty / unfilled, so an old
    /// save fills from the defaults at the next `sync`.
    pub fn from_parts(magic: &[u64], chosen: &[bool], shortcuts: &[Vec<u64>], filled: bool) -> LeaderCustom {
        let mut c = LeaderCustom { filled, ..LeaderCustom::default() };
        for (i, v) in magic.iter().take(6).enumerate() {
            c.magic_slots[i] = (*v).min(255) as u8;
        }
        for (i, v) in chosen.iter().take(6).enumerate() {
            c.tier_chosen[i] = *v;
        }
        for (p, page) in shortcuts.iter().take(3).enumerate() {
            for (s, v) in page.iter().take(4).enumerate() {
                c.shortcuts[p][s] = (*v).min(255) as u8;
            }
        }
        c
    }
}

/// The display name of an entry of any leader's lists (Sora's command names, Goofy's ids).
pub fn entry_name(kind: u8) -> &'static str {
    if goofy::is_goofy(kind) {
        goofy::name(kind)
    } else if donald::is_donald(kind) {
        donald::name(kind) // (DON-UI)
    } else {
        spells::command_name(kind)
    }
}

/// THE SHORTCUT LIST of a friend leader (the list a shortcut button opens; `customize::assignable` for Sora): his
/// entries in asset order, then the item commands of the items in his pouch (`items`: his item slots' items).
pub fn assignable(kind: LeaderKind, book: &MagicBook, items: &[ItemKind]) -> Vec<Assignable> {
    let _ = book;
    match kind {
        // (DON-UI) Donald's list is his own book's (LeaderCustom::list); here the start's (no block at hand)
        LeaderKind::Donald => donald::assignable(&donald::DonBook::START, items),
        // (GF-ARTS) the same for Goofy
        LeaderKind::Goofy => goofy::assignable(&goofy::ArtBook::START, items),
    }
}

/// The item command of an item (58 Potion ...), for the friends' pouch rows.
pub fn item_command(item: ItemKind) -> Option<u8> {
    command_of_item(item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::magic::spells::cmd;

    fn book(tiers: [u8; 6]) -> MagicBook {
        MagicBook { tiers, sora_slots: [cmd::FIRAGA, cmd::WATAGA, cmd::CURAGA, cmd::BLIZZAGA, cmd::THUNDAGA, cmd::AEROGA] }
    }

    // (DON-UI) Donald's block is his own since the overhaul (his ids, his book); X2's Sora-spell tests replaced.
    fn don(tier: [u8; 6], line2: [u8; 6], dual: bool) -> donald::DonBook {
        donald::DonBook { tier, line2, dual }
    }

    #[test]
    fn donald_s_ids_names_and_split() {
        use donald::*;
        assert_eq!((line1(0, 1), line1(5, 4), line2(0), line2(5), ultimate(0), ultimate(5), attune(0), attune(5)), (0x80, 0x97, 0x98, 0x9D, 0x9E, 0xA3, 0xB3, 0xB8));
        assert_eq!(hybrid(0, 1), Some(0xA4));
        assert_eq!(hybrid(5, 4), Some(0xB2), "Aero + Light, either order");
        assert_eq!(hybrid(2, 2), None);
        assert_eq!(pairs().count(), 15);
        for k in FIRST..=LAST {
            assert!(split(k).is_some() && !name(k).is_empty(), "{k:#x}");
        }
        assert_eq!((name(0x83), name(0x95), name(0x99), name(0xA1), name(0xA4), name(0xB2), name(0xB8)), ("Firaza", "Cura", "Freeze", "Tsunami", "Thermal Shock", "Radiant Gale", "Attune Light"));
        assert_eq!(entry_name(0x9A), "Ball Lightning");
        assert_eq!(entry_name(cmd::POTION), "Potion");
        assert!(split(0x7F).is_none() && split(0xBA).is_none());
        assert_eq!((split(UNATTUNE), name(UNATTUNE)), (Some(Entry::Unattune), "Unattune"), "(DON-UI) Unattune");
    }

    #[test]
    fn an_old_save_s_sora_ids_migrate_to_donald_s() {
        use donald::*;
        assert_eq!(migrate(cmd::FIRE), line1(0, 1));
        assert_eq!(migrate(cmd::FIRAGA), line1(0, 3));
        assert_eq!(migrate(cmd::CURAGA), line1(5, 3));
        assert_eq!(migrate(cmd::WATAGA), line1(3, 3));
        assert_eq!(migrate(112), line1(0, 4), "the grand ids as DonCmd::from_sora_cmd");
        assert_eq!(migrate(125), line1(5, 4));
        assert_eq!(migrate(cmd::POTION), cmd::POTION, "items stay");
        assert_eq!(migrate(0x99), 0x99, "his own stay");
        assert_eq!(migrate(0), 0);
        // an X2 block (Sora ids, filled) synced against his book: the ids become his, above-learned tiers fall
        let mut c = LeaderCustom { filled: true, magic_slots: [cmd::FIRAGA, cmd::WATAGA, cmd::CURAGA, cmd::BLIZZAGA, 0, 0], shortcuts: [[cmd::FIRAGA, cmd::BLIZZAGA, cmd::THUNDAGA, cmd::CURAGA], [cmd::POTION, 0, 0, 0], [0; 4]], ..LeaderCustom::default() };
        c.don = don([2, 0, 0, 1, 0, 0], [1, 0, 0, 1, 0, 0], false);
        c.sync(LeaderKind::Donald, &MagicBook::default());
        assert_eq!(c.shortcuts[0], [line1(0, 2), 0, 0, 0], "Firaga -> Fira (he knows Fira), Blizzaga / Thundaga / Curaga unknown -> empty");
        assert_eq!(c.shortcuts[1][0], cmd::POTION);
        assert_eq!(c.slots(LeaderKind::Donald), vec![line1(0, 1), line1(3, 1)], "the Magic page: his elements");
    }

    #[test]
    fn donald_s_block_fills_learns_and_lists() {
        use donald::*;
        let mut c = LeaderCustom { don: DonBook::START, ..LeaderCustom::default() };
        assert!(c.sync(LeaderKind::Donald, &MagicBook::default()));
        assert_eq!(c.slots(LeaderKind::Donald), vec![line1(0, 1), line1(3, 1)]);
        assert_eq!(c.shortcuts, [[line1(0, 1), line2(0), line1(3, 1), line2(3)], [attune(0), attune(3), 0, 0], [0; 4]], "three palettes from the start");
        // Thunder learned: its entry goes in the first empty slot; the order is his (Reorder works, no tier choice)
        c.don = don([2, 0, 1, 1, 0, 0], [1, 0, 0, 1, 0, 0], false);
        c.sync(LeaderKind::Donald, &MagicBook::default());
        assert_eq!(c.slots(LeaderKind::Donald), vec![line1(0, 1), line1(3, 1), line1(2, 1)]);
        assert!(c.swap(LeaderKind::Donald, 0, 2));
        c.sync(LeaderKind::Donald, &MagicBook::default());
        assert_eq!(c.slots(LeaderKind::Donald)[0], line1(2, 1), "the order stays");
        assert!(!c.set_slot(LeaderKind::Donald, &MagicBook::default(), 0, line1(2, 1)), "no tier choice: the entry picks (R14)");
        // the shortcut list: tiers, line 2, ultimate, hybrids with Dual, attunements, items
        let full = don([4; 6], [3; 6], true);
        let list = assignable(&full, &[ItemKind::Potion]);
        assert_eq!(list.iter().filter(|a| a.tab == Tab::Magic).count(), 6 * 6 + 15 + 6 + 1, "(DON-UI) + Unattune");
        assert_eq!(list.last().map(|a| a.kind), Some(cmd::POTION));
        assert_eq!(assignable(&don([4; 6], [3; 6], false), &[]).len(), 6 * 6 + 6 + 1, "no hybrids without Dual Attunement");
        c.don = full;
        c.sync(LeaderKind::Donald, &MagicBook::default());
        assert!(c.set_shortcut(LeaderKind::Donald, &MagicBook::default(), &[], 2, 0, line1(1, 4)), "Blizzaza on a shortcut");
        assert!(c.set_shortcut(LeaderKind::Donald, &MagicBook::default(), &[], 2, 1, attune(4)), "an Attunement on a shortcut");
        assert!(c.set_shortcut(LeaderKind::Donald, &MagicBook::default(), &[], 2, 2, hybrid(0, 3).unwrap()));
        assert!(!c.set_shortcut(LeaderKind::Donald, &MagicBook::default(), &[], 2, 3, cmd::FIRAGA), "Sora's spells are not his");
        // an older progression loaded: Blizzaza falls to the tier he has, the Aero attune is cleared, the hybrid stays
        c.don = don([4, 2, 0, 1, 0, 0], [1, 0, 0, 1, 0, 0], false);
        c.sync(LeaderKind::Donald, &MagicBook::default());
        assert_eq!(c.shortcuts[2][..3], [line1(1, 2), 0, hybrid(0, 3).unwrap()]);
    }

    fn gof(ranks: [u8; 7]) -> goofy::ArtBook {
        goofy::ArtBook { ranks, fed: true }
    }

    fn goofy_block(book: goofy::ArtBook, shortcuts: [[u8; 4]; 3]) -> LeaderCustom {
        LeaderCustom { filled: true, shortcuts, gof: book, ..LeaderCustom::default() }
    }

    #[test]
    fn goofy_s_ids_names_and_the_first_fill() {
        use goofy::*;
        assert_eq!(ARTS.len(), 6, "six to equip; Taunt is fixed");
        for k in ARTS {
            assert!(is_art(k) && is_goofy(k) && !name(k).is_empty() && !help(k).is_empty(), "{k:#x}");
            assert!(item_of_command(k).is_none() && !spells::is_magic(k) && !donald::is_donald(k), "{k:#x} is his alone");
            assert_eq!(entry_name(k), name(k));
        }
        assert!(is_goofy(TAUNT) && !is_art(TAUNT));
        assert!(!is_goofy(cmd::POTION) && !is_goofy(OLD_FIRST) && !is_goofy(OLD_LAST) && !is_goofy(0xD1));
        // level 1: Charge and Taunt; the second Art slot waits
        let mut c = LeaderCustom::default();
        c.sync(LeaderKind::Goofy, &book([1; 6]));
        assert!(c.filled);
        assert_eq!(c.arts(), [CHARGE, 0]);
        assert_eq!(c.shortcuts, [[0, cmd::POTION, TAUNT, CHARGE], [0, 0, TAUNT, CHARGE], [0, 0, TAUNT, CHARGE]], "Arts and Taunt on every page, the Potion on Y");
        // an Art learned later fills the empty slot, and stays out of the taken one
        c.gof = gof([1, 1, 0, 0, 0, 0, 1]);
        c.sync(LeaderKind::Goofy, &book([1; 6]));
        assert_eq!(c.arts(), [CHARGE, TORNADO]);
        c.gof = gof([1, 1, 1, 0, 0, 0, 1]);
        assert!(!c.sync(LeaderKind::Goofy, &book([1; 6])), "both taken: nothing moves");
    }

    #[test]
    fn equip_art_takes_a_learned_art_and_swaps_a_duplicate() {
        use goofy::*;
        let mut c = LeaderCustom { gof: gof([1, 1, 1, 0, 0, 2, 1]), ..LeaderCustom::default() };
        c.sync(LeaderKind::Goofy, &book([1; 6]));
        assert_eq!(c.arts(), [CHARGE, TORNADO]);
        assert!(c.equip_art(1, SHIELD_TOSS));
        assert_eq!(c.arts(), [CHARGE, SHIELD_TOSS]);
        assert!(c.shortcuts.iter().all(|p| p[SLOT_B] == SHIELD_TOSS), "every page");
        assert!(c.equip_art(0, SHIELD_TOSS), "the Art on the other slot swaps over");
        assert_eq!(c.arts(), [SHIELD_TOSS, CHARGE]);
        assert!(!c.equip_art(0, TURBO), "not learned");
        assert!(!c.equip_art(0, TAUNT), "Taunt is fixed");
        assert!(!c.equip_art(2, BASH), "no third slot");
        assert_eq!(arts(&c.gof).iter().map(|a| a.kind).collect::<Vec<_>>(), [CHARGE, TORNADO, BASH, SHIELD_TOSS], "the list: learned, no Taunt");
        // the shortcut route of Customize lands on the same rules
        let items = [ItemKind::Potion];
        assert!(c.set_shortcut(LeaderKind::Goofy, &book([1; 6]), &items, 2, SLOT_A, BASH));
        assert_eq!(c.arts()[0], BASH, "the Art of any page is the Art of every page");
        assert!(!c.set_shortcut(LeaderKind::Goofy, &book([1; 6]), &items, 0, SLOT_TAUNT, CHARGE), "X stays Taunt");
        assert!(!c.set_shortcut(LeaderKind::Goofy, &book([1; 6]), &items, 0, SLOT_A, cmd::POTION), "an item is not an Art");
        assert!(!c.set_shortcut(LeaderKind::Goofy, &book([1; 6]), &items, 0, SLOT_ITEM, BASH), "Y holds items");
        assert!(c.set_shortcut(LeaderKind::Goofy, &book([1; 6]), &items, 1, SLOT_ITEM, cmd::POTION), "Y is each page's own");
        assert_eq!((c.shortcuts[1][SLOT_ITEM], c.shortcuts[2][SLOT_ITEM]), (cmd::POTION, 0));
        assert!(c.swap(LeaderKind::Goofy, 0, 1));
        assert_eq!(c.arts(), [CHARGE, BASH], "the page's swap swaps the buttons");
        assert_eq!(c.slots(LeaderKind::Goofy), vec![CHARGE, BASH]);
        assert!(c.set_slot(LeaderKind::Goofy, &book([1; 6]), 1, SHIELD_TOSS), "his page's slot equips");
        assert_eq!(c.arts(), [CHARGE, SHIELD_TOSS]);
    }

    #[test]
    fn an_old_save_s_tiered_block_migrates_to_arts() {
        use goofy::*;
        // X2's default block (slots B, Y, X, A: Tornado II, Turbo II, Discus II, Potion), everything learned
        let all = gof([1; 7]);
        let mut c = goofy_block(all, [[0xC2, 0xC5, 0xC8, cmd::POTION], [0; 4], [0; 4]]);
        c.magic_slots = [0xC2, 0xC5, 0xC8, 0, 0, 0];
        c.tier_chosen[1] = true;
        assert!(c.sync(LeaderKind::Goofy, &book([1; 6])));
        assert_eq!(c.arts(), [TORNADO, TURBO], "in panel order");
        assert_eq!(c.shortcuts[0], [TURBO, cmd::POTION, TAUNT, TORNADO], "the item moved to Y");
        assert_eq!(c.magic_slots[..3], [TORNADO, TURBO, 0]);
        assert_eq!(c.tier_chosen, [false; 6]);
        let settled = c;
        assert!(!c.sync(LeaderKind::Goofy, &book([1; 6])), "once");
        assert_eq!(c, settled);
        // a Discus with a low-level Goofy: not learned yet -> his Charge; the items spread over the pages
        let mut c = goofy_block(gof([1, 0, 0, 0, 0, 0, 1]), [[0xC9, cmd::POTION, cmd::ETHER, 0], [0xC1, 0, 0, cmd::POTION], [0; 4]]);
        c.sync(LeaderKind::Goofy, &book([1; 6]));
        assert_eq!(c.arts(), [CHARGE, 0]);
        assert_eq!((c.shortcuts[0][SLOT_ITEM], c.shortcuts[1][SLOT_ITEM], c.shortcuts[2][SLOT_ITEM]), (cmd::POTION, cmd::ETHER, cmd::POTION));
        // before the host fed his ranks the old Arts are kept
        let mut c = goofy_block(ArtBook::START, [[0xC3, 0xC6, 0xC9, 0], [0; 4], [0; 4]]);
        c.sync(LeaderKind::Goofy, &book([1; 6]));
        assert_eq!(c.arts(), [TORNADO, TURBO], "unfed: nothing is thrown away");
        // an older progression loaded: the Art above his ranks goes, the free slot takes one he knows
        let mut c = goofy_block(gof([1, 1, 0, 0, 0, 0, 1]), [[SHIELD_TOSS, 0, TAUNT, TORNADO]; 3]);
        c.sync(LeaderKind::Goofy, &book([1; 6]));
        assert_eq!(c.arts(), [TORNADO, CHARGE]);
    }

    #[test]
    fn the_save_shape_round_trips() {
        let mut c = LeaderCustom { don: donald::DonBook { tier: [3; 6], line2: [1; 6], dual: false }, ..LeaderCustom::default() };
        c.sync(LeaderKind::Donald, &book([3; 6]));
        c.tier_chosen[2] = true;
        let s = c.to_json();
        assert!(s.starts_with("{\"magic\":[128,132,136,140,144,148]"), "{s}");
        let back = LeaderCustom::from_parts(&c.magic_slots.map(|x| x as u64), &c.tier_chosen, &c.shortcuts.iter().map(|p| p.iter().map(|x| *x as u64).collect()).collect::<Vec<_>>(), true);
        assert_eq!(LeaderCustom { don: c.don, ..back }, c);
    }
}
