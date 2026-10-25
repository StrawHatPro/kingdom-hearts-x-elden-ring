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
//! - Goofy: three ability slots (Goofy Tornado / Turbo / Discus) with a tier pick like magic. His tiers follow the
//!   party's magic (decision 6): tier 2 with the first second-tier spell Sora has, tier 3 with the first third-tier one.
//!   Shortcut list: each ability at each tier he has, then his pouch items. First-switch set A: Tornado / Turbo /
//!   Discus at the best tier, Potion.
//!
//! GOOFY'S IDS: his entries need command kinds for the menus and the save; `ETresCommandKind` has no free values, so
//! Goofy's list has its own id space [`goofy`] (`0xC1 ..= 0xC9`), used ONLY inside Goofy's block and lists (never
//! executed as a Sora command). Item entries are the item commands 58 ..= 68 in every block.

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
            LeaderKind::Goofy => 3,
        }
    }
}

/// The asset order of the Customize lists (`CustomizeDataAsset.MagicCommands`, UI spec 10.6).
pub const ASSET_ORDER: [Element; 6] = [Element::Fire, Element::Blizzard, Element::Thunder, Element::Water, Element::Aero, Element::Cure];

/// GOOFY'S OWN ID SPACE (port design, see the module text): ability index x 3 + tier - 1 + `BASE`.
pub mod goofy {
    pub const BASE: u8 = 0xC1;
    pub const TORNADO: u8 = BASE;
    pub const TURBO: u8 = BASE + 3;
    pub const DISCUS: u8 = BASE + 6;
    pub const LAST: u8 = BASE + 8;
    /// The `TresAbilityData` rows of the three abilities (`GOOFY_TORNADE`, `GOOFY_TURBO`, `GOOFY_FLIP`).
    pub const ABILITY_ROWS: [&str; 3] = ["GOOFY_TORNADE", "GOOFY_TURBO", "GOOFY_FLIP"];

    pub const fn is_goofy(kind: u8) -> bool {
        kind >= BASE && kind <= LAST
    }

    /// The id of ability `index` (0 Tornado, 1 Turbo, 2 Discus) at `tier` (1 ..= 3).
    pub const fn id(index: u8, tier: u8) -> u8 {
        let t = if tier < 1 { 1 } else if tier > 3 { 3 } else { tier };
        BASE + index * 3 + t - 1
    }

    /// (ability index 0 ..= 2, tier 1 ..= 3) of an id.
    pub const fn split(kind: u8) -> Option<(u8, u8)> {
        if !is_goofy(kind) {
            return None;
        }
        let k = kind - BASE;
        Some((k / 3, k % 3 + 1))
    }

    /// The three tiers of an id's ability.
    pub fn tiers(kind: u8) -> Option<[u8; 3]> {
        let (index, _) = split(kind)?;
        Some([id(index, 1), id(index, 2), id(index, 3)])
    }

    /// The display name (KH3's "Goofy Tornado" / "Goofy Turbo" / "Goofy Discus" [kh3_system]; the tier marks II / III
    /// are ours: KH3's friends have one tier).
    pub const fn name(kind: u8) -> &'static str {
        match kind {
            0xC1 => "Goofy Tornado",
            0xC2 => "Goofy Tornado II",
            0xC3 => "Goofy Tornado III",
            0xC4 => "Goofy Turbo",
            0xC5 => "Goofy Turbo II",
            0xC6 => "Goofy Turbo III",
            0xC7 => "Goofy Discus",
            0xC8 => "Goofy Discus II",
            0xC9 => "Goofy Discus III",
            _ => "",
        }
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

    /// GOOFY'S TIER (decision 6): the highest tier of any spell the party has (tier 2 with the first -ra, 3 with the
    /// first -ga); 1 with none.
    pub fn goofy_tier(&self) -> u8 {
        self.tiers.iter().copied().max().unwrap_or(0).clamp(1, 3)
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
}

impl LeaderCustom {
    /// The block as the book stands: the first fill, then Sora's learn rule (0x14159EE70) for every learned tier.
    /// Returns true when anything changed.
    pub fn sync(&mut self, kind: LeaderKind, book: &MagicBook) -> bool {
        let before = *self;
        match kind {
            // (DON-UI) Donald's own block (the overhaul): his ids, his book (`self.don`), the old save's Sora ids migrated
            LeaderKind::Donald => self.sync_donald(),
            LeaderKind::Goofy => {
                let t = book.goofy_tier();
                if !self.filled {
                    self.filled = true;
                    self.tier_chosen = [false; 6];
                    self.magic_slots = [goofy::id(0, t), goofy::id(1, t), goofy::id(2, t), 0, 0, 0];
                    self.shortcuts = [[goofy::id(0, t), goofy::id(1, t), goofy::id(2, t), spells::cmd::POTION], [0; 4], [0; 4]];
                }
                for (slot, s) in self.magic_slots.iter_mut().enumerate() {
                    if let Some((index, tier)) = goofy::split(*s) {
                        let want = if self.tier_chosen[slot] { tier.min(t) } else { t };
                        *s = goofy::id(index, want);
                    }
                }
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
            LeaderKind::Goofy => match (goofy::split(cmd), goofy::split(old)) {
                (Some((i, t)), Some((j, _))) => i == j && t <= book.goofy_tier(),
                _ => false,
            },
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
        true
    }

    /// `shortcut[page][slot] = cmd` (0 = Remove); the command must be in the leader's list.
    pub fn set_shortcut(&mut self, kind: LeaderKind, book: &MagicBook, items: &[ItemKind], page: usize, slot: usize, cmd: u8) -> bool {
        if page >= 3 || slot >= 4 || (cmd != 0 && !self.list(kind, book, items).iter().any(|a| a.kind == cmd)) {
            return false;
        }
        self.shortcuts[page][slot] = cmd;
        true
    }

    /// (DON-UI) The leader's shortcut list (Customize): Donald's from his own book (`self.don`), Goofy's [`assignable`].
    pub fn list(&self, kind: LeaderKind, book: &MagicBook, items: &[ItemKind]) -> Vec<Assignable> {
        match kind {
            LeaderKind::Donald => donald::assignable(&self.don, items),
            LeaderKind::Goofy => assignable(kind, book, items),
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
        for e in 0..donald::ELEMENTS {
            if book.unlocked(e) && !seen[e] {
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
    let mut out = Vec::new();
    match kind {
        // (DON-UI) Donald's list is his own book's: [`LeaderCustom::list`]; here the start's (no block at hand)
        LeaderKind::Donald => {
            let _ = book;
            return donald::assignable(&donald::DonBook::START, items);
        }
        LeaderKind::Goofy => {
            for index in 0..3u8 {
                for tier in 1..=book.goofy_tier() {
                    let id = goofy::id(index, tier);
                    out.push(Assignable { kind: id, name: goofy::name(id), tab: Tab::Magic, auto_reload: false });
                }
            }
        }
    }
    for cmd in 58u8..=68 {
        if item_of_command(cmd).is_some_and(|i| items.contains(&i)) {
            out.push(Assignable { kind: cmd, name: spells::command_name(cmd), tab: Tab::Items, auto_reload: false });
        }
    }
    out
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
        let mut c = LeaderCustom::default();
        c.don = DonBook::START;
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

    #[test]
    fn goofy_s_tiers_follow_the_party_s_magic() {
        assert_eq!(book([1, 0, 0, 0, 0, 0]).goofy_tier(), 1);
        assert_eq!(book([1, 2, 0, 0, 0, 0]).goofy_tier(), 2);
        assert_eq!(book([0; 6]).goofy_tier(), 1);
        let mut c = LeaderCustom::default();
        c.sync(LeaderKind::Goofy, &book([1, 2, 0, 0, 0, 0]));
        assert_eq!(c.slots(LeaderKind::Goofy), vec![goofy::id(0, 2), goofy::id(1, 2), goofy::id(2, 2)]);
        assert_eq!(c.shortcuts[0], [goofy::TORNADO + 1, goofy::TURBO + 1, goofy::DISCUS + 1, cmd::POTION]);
        assert!(c.set_slot(LeaderKind::Goofy, &book([1, 2, 0, 0, 0, 0]), 1, goofy::TURBO));
        c.sync(LeaderKind::Goofy, &book([3; 6]));
        assert_eq!(c.slots(LeaderKind::Goofy), vec![goofy::id(0, 3), goofy::TURBO, goofy::id(2, 3)], "a hand-chosen tier stays");
        assert_eq!(goofy::split(goofy::id(2, 3)), Some((2, 3)));
        assert_eq!(goofy::tiers(goofy::id(1, 2)), Some([0xC4, 0xC5, 0xC6]));
        assert_eq!(assignable(LeaderKind::Goofy, &book([3; 6]), &[]).len(), 9);
        assert!(!goofy::is_goofy(cmd::POTION) && !goofy::is_goofy(231));
    }

    #[test]
    fn the_save_shape_round_trips() {
        let mut c = LeaderCustom::default();
        c.don = donald::DonBook { tier: [3; 6], line2: [1; 6], dual: false };
        c.sync(LeaderKind::Donald, &book([3; 6]));
        c.tier_chosen[2] = true;
        let s = c.to_json();
        assert!(s.starts_with("{\"magic\":[128,132,136,140,144,148]"), "{s}");
        let back = LeaderCustom::from_parts(&c.magic_slots.map(|x| x as u64), &c.tier_chosen, &c.shortcuts.iter().map(|p| p.iter().map(|x| *x as u64).collect()).collect::<Vec<_>>(), true);
        assert_eq!(LeaderCustom { don: c.don, ..back }, c);
    }
}
