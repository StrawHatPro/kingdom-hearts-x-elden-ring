//! (party, stream F) DONALD AND GOOFY IN THE HUD AND THE CAMP MENU: the plain data a host hands the game HUD
//! ([`crate::hud::Hud::set_party`], [`crate::hud::Hud::apply_party_actions`]) and the friend AI settings of the camp
//! menu's Customize screen. Contract: docs/party-contracts.md, section "Stream F". No Bevy, no files.
//!
//! Evidence (docs/sora-ui-spec.md 4, 10.3 - 10.7, 10.9; docs/party-port-plan.md 2.8, 2.11, 2.12):
//! - status HUD slots: 0 Sora, 1 - 2 friends (`FriendRelationDataTable` index 0 Donald, 1 Goofy), 3 - 4 guests;
//!   the friends' faces are the movies `01_hud/face/FaceNpcExDonald` / `FaceNpcExGoofy` [A];
//! - the level-up window's `chara`: 0 Sora, 1 Donald, 2 Goofy (`0x141659CD0`) [C];
//! - the camp menu's party table `m_PartyParams[5]` (0x14154C710): slot 0 the player, then the friends [C];
//! - the friend AI settings are the save record's bytes +0x15A .. +0x15C (writer `0x1415EA860`) [C: the byte
//!   layout below is the port's reading, see [`FriendAiSettings`]].

/// Who a party slot is. The value is the level-up window's `chara` and the status HUD slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PartyChara {
    Sora = 0,
    Donald = 1,
    Goofy = 2,
}

impl PartyChara {
    pub const FRIENDS: [PartyChara; 2] = [PartyChara::Donald, PartyChara::Goofy];

    /// The status HUD slot (1 Donald, 2 Goofy; 0 Sora).
    pub fn slot(self) -> usize {
        self as usize
    }

    /// `ETresChrUniqueID`: Sora 0 (TRES_CHR_UID_SORA), Donald 11, Goofy 12 [A].
    pub fn unique_id(self) -> u8 {
        match self {
            PartyChara::Sora => 0,
            PartyChara::Donald => 11,
            PartyChara::Goofy => 12,
        }
    }

    /// The English party name (`kh3_system` character names as the camp menu shows them).
    pub fn name(self) -> &'static str {
        match self {
            PartyChara::Sora => "Sora",
            PartyChara::Donald => "Donald",
            PartyChara::Goofy => "Goofy",
        }
    }

    /// The face movie of the status HUD (UI pack movie name).
    pub fn face_movie(self) -> &'static str {
        match self {
            PartyChara::Sora => "FacePcExSora01",
            PartyChara::Donald => "FaceNpcExDonald",
            PartyChara::Goofy => "FaceNpcExGoofy",
        }
    }

    pub fn from_name(name: &str) -> Option<PartyChara> {
        match name.to_ascii_lowercase().as_str() {
            "sora" | "0" => Some(PartyChara::Sora),
            "donald" | "1" | "n_ex001" => Some(PartyChara::Donald),
            "goofy" | "2" | "n_ex002" => Some(PartyChara::Goofy),
            _ => None,
        }
    }
}

/// (GOOFY-GG) Goofy's GUARD bar: its values and look (kh_ui::hud_guard), for `Hud::set_guard`.
pub use kh_ui::hud_guard::{GuardLook, GuardValues};

/// One friend slot of the status HUD, as the native update reads the pawn (UI spec 4: the pawn virtuals).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartyHudSlot {
    pub chara: PartyChara,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    /// vt+0x17E0 / vt+0x17F8: the MP charge runs, and its value (the friends' gauges show it too).
    pub mp_charging: bool,
    pub mp_charge_value: f32,
    /// vt+0x1750: the friend pawn is in battle.
    pub in_battle: bool,
    /// (party X2-b) A LEADING friend's bar in the focus-gauge slot (value, max): Donald's Spell Volley focus; Goofy's
    /// shield gauge, his Discus Barrage focus while he aims. None = no bar (an AI friend; KH3 shows none for friends).
    pub focus: Option<(i32, i32)>,
}

/// `ETresFNpcAIStyle_Battle` [A: enum sheet]: Combat Style.
pub const BATTLE_STYLES: [&str; 4] = ["Stick By Sora", "Be Yourself", "Hands Off", "Finish the Job"];
/// `ETresFNpcAIStyle_Ability`: Abilities.
pub const ABILITY_STYLES: [&str; 3] = ["Go Wild", "Watch MP", "Conserve"];
/// `ETresFNpcAIStyle_Cure`: Recovery.
pub const CURE_STYLES: [&str; 3] = ["Use As Needed", "Use in Emergencies", "Use Like Crazy"];
/// The Recovery targets (bits 0, 1, 2).
pub const CURE_TARGETS: [&str; 3] = ["On Sora", "On Self", "On Others"];

/// THE FRIEND AI SETTINGS of the camp menu's Customize screen (UI spec 10.9 "Friends"): three choices and the
/// Recovery targets, as the friend's save record holds them [C]: +0x158 Combat Style (setter 0x140E97230), +0x159
/// Abilities (0x140E97220), +0x15A Recovery (0x140E97240), +0x15B bits 0 .. 2 the targets (writer 0x1415EA860). The
/// screen shows the label of the byte's value (0x1415E7E80 indexes its label tables by it); the labels
/// `kh3_camp.customize.bstyle_0101 ..` are taken in enum order [D: the label tables are built at run time].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FriendAiSettings {
    /// `ETresFNpcAIStyle_Battle`: TOGETHER 0 (Stick By Sora) / FREE 1 (Be Yourself) / SUPPORT 2 (Hands Off) /
    /// FOCUS 3 (Finish the Job).
    pub battle: u8,
    /// `ETresFNpcAIStyle_Ability`: FREE 0 (Go Wild) / BALANCE 1 (Watch MP) / LESS 2 (Conserve).
    pub ability: u8,
    /// `ETresFNpcAIStyle_Cure`: OFTEN 0 (Use As Needed) / PINCH 1 (Use in Emergencies) / ALWAYS 2 (Use Like Crazy).
    pub cure: u8,
    /// Bit 0 On Sora, bit 1 On Self, bit 2 On Others.
    pub cure_targets: u8,
}

impl Default for FriendAiSettings {
    /// THE GAME'S RESTORE DEFAULTS (`0x140E94550`, run by the Customize screen's Yes to "restore", 0x1415E16B0) [C]:
    /// +0x158 = +0x159 = 0, +0x15A = 1, targets |= 7. (The new-game init of the record bytes was not traced.)
    fn default() -> Self {
        FriendAiSettings { battle: 0, ability: 0, cure: 1, cure_targets: 0b111 }
    }
}

impl FriendAiSettings {
    /// The record bytes +0x158 .. +0x15B. Round-trips with [`FriendAiSettings::from_record`].
    pub fn to_record(&self) -> [u8; 4] {
        [self.battle, self.ability, self.cure, self.cure_targets & 7]
    }

    pub fn from_record(b: [u8; 4]) -> FriendAiSettings {
        FriendAiSettings { battle: b[0].min(3), ability: b[1].min(2), cure: b[2].min(2), cure_targets: b[3] & 7 }
    }

    /// One setting by the Customize screen's row (0 Combat Style, 1 Abilities, 2 Recovery).
    pub fn style(&self, row: usize) -> u8 {
        [self.battle, self.ability, self.cure][row.min(2)]
    }

    pub fn set_style(&mut self, row: usize, value: u8) {
        match row {
            0 => self.battle = value.min(3),
            1 => self.ability = value.min(2),
            _ => self.cure = value.min(2),
        }
    }

    /// One line for logs and the debug text.
    pub fn describe(&self) -> String {
        let targets: Vec<&str> = (0..3).filter(|b| self.cure_targets & (1 << b) != 0).map(|b| CURE_TARGETS[b]).collect();
        format!(
            "{} / {} / {} ({})",
            BATTLE_STYLES[self.battle.min(3) as usize],
            ABILITY_STYLES[self.ability.min(2) as usize],
            CURE_STYLES[self.cure.min(2) as usize],
            if targets.is_empty() { "no targets".to_string() } else { targets.join(", ") }
        )
    }
}

/// One friend as the camp menu shows it (the host's snapshot of B's `FriendStatus`; contract F.2). Friends in party
/// order (Donald, Goofy).
#[derive(Clone, Debug, PartialEq)]
pub struct PartyMenuMember {
    pub chara: PartyChara,
    pub level: i32,
    /// Total EXP (shared with Sora), this level's total, the next level's total (0 at level 99).
    pub exp: i32,
    pub exp_level: i32,
    pub exp_next: i32,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    pub ap_max: i32,
    pub ap_in_use: i32,
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
    /// Fire, Blizzard, Thunder, Water, Aero, Dark, percent.
    pub resists: [i32; 6],
    /// The equip item id of the weapon ("WEP_DONALD00" ...).
    pub weapon: String,
    /// One entry per OPEN slot.
    pub armour: Vec<Option<String>>,
    pub accessories: Vec<Option<String>>,
    pub items: Vec<Option<kh_core::status::ItemKind>>,
    /// (ability, owned copies, equipped-copies mask), every owned ability.
    pub abilities: Vec<(kh_core::enums::ETresAbilityKind, u8, u8)>,
    pub ai: FriendAiSettings,
    /// (party X2) His own Customize block (Magic / Abilities page, shortcut sets A / B / C: kh-core command/leader.rs);
    /// the camp menu's Customize shows it while he LEADS. None = the host keeps none (Sora's page as before).
    pub custom: Option<kh_core::command::leader::LeaderCustom>,
    /// (party X2) The MP cost of each of his own list's entries that has one (Goofy's abilities: their `m_UseMP`), for
    /// the Customize help text's "MP Cost: {0}".
    pub custom_costs: Vec<(u8, i32)>,
    /// (party X2-b) His kit abilities (the port's own, XD_PLAYABLE_KITS.md 1.7 / 2.6 and the Unisons / Fusions he
    /// wears): owned ones, for his Abilities page.
    pub kit: Vec<KitRow>,
    /// (GOOFY-GG) Goofy's GUARD (value, max) for the Stats / party pages' third gauge row (where Sora's Focus is); None
    /// for Donald.
    pub guard: Option<(i32, i32)>,
    /// (EQ) Per open item slot (as `items`): Auto-Reload on, and the item of a USED auto-reload slot waiting for its
    /// refill (None otherwise): the Equipment screen shows it greyed as Sora's, the menu's close refills it.
    pub item_reload: Vec<bool>,
    pub item_spent: Vec<Option<kh_core::status::ItemKind>>,
}

/// (party X2-b) One owned kit ability of a friend (kh-party `pad::donald_kit` / `pad::goofy_kit`).
#[derive(Clone, Debug, PartialEq)]
pub struct KitRow {
    /// The save name (`KIT_SPELL_WEAVER` ...): what `PartyMenuEdit::Kit` names.
    pub key: String,
    pub name: String,
    pub ap: i32,
    /// `ETresAbilityCategory`: 0 Actions, 1 Mobility, 2 Support.
    pub category: u8,
    pub equipped: bool,
    pub help: String,
    /// The KH3 ability it stands for (Unison / Fusion: KH3's own name and help text).
    pub kh3: Option<kh_core::enums::ETresAbilityKind>,
}

/// (party X2) A Customize edit of a friend leader's own block (`PartyMenuEdit::Custom`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomEdit {
    SetMagicSlot { slot: usize, kind: u8 },
    SwapMagicSlots(usize, usize),
    /// kind 0 = Remove.
    SetShortcut { page: usize, slot: usize, kind: u8 },
}

/// What the camp menu changed for a friend: the host applies each to B's `FriendStatus` (which rebuilds the
/// stats) and hands the new snapshot back with `Hud::set_party_menu`.
#[derive(Clone, Debug, PartialEq)]
pub enum PartyMenuEdit {
    Weapon { chara: PartyChara, id: String },
    Armour { chara: PartyChara, slot: usize, id: Option<String> },
    Accessory { chara: PartyChara, slot: usize, id: Option<String> },
    ItemSlot { chara: PartyChara, slot: usize, item: Option<kh_core::status::ItemKind> },
    Ability { chara: PartyChara, kind: kh_core::enums::ETresAbilityKind, copy: u8, on: bool },
    /// An item of the stock used on the friend from the Items screen (the stock count already dropped).
    UseItem { chara: PartyChara, item: kh_core::status::ItemKind },
    Ai { chara: PartyChara, settings: FriendAiSettings },
    /// (party X2) The leader's own Magic / Abilities page or shortcut sets.
    Custom { chara: PartyChara, edit: CustomEdit },
    /// (party X2-b) A kit ability worn / taken off (its save name).
    Kit { chara: PartyChara, key: String, on: bool },
    /// (EQ) Equipment's Triangle on an item row: Auto-Reload of that item on every slot holding it (Sora's rule).
    AutoReload { chara: PartyChara, item: kh_core::status::ItemKind, on: bool },
}

impl PartyMenuEdit {
    pub fn chara(&self) -> PartyChara {
        match self {
            PartyMenuEdit::Weapon { chara, .. } | PartyMenuEdit::Armour { chara, .. } | PartyMenuEdit::Accessory { chara, .. } | PartyMenuEdit::ItemSlot { chara, .. } | PartyMenuEdit::Ability { chara, .. } | PartyMenuEdit::UseItem { chara, .. } | PartyMenuEdit::Ai { chara, .. } | PartyMenuEdit::Custom { chara, .. } | PartyMenuEdit::Kit { chara, .. } | PartyMenuEdit::AutoReload { chara, .. } /* (EQ) */ => *chara,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_settings_round_trip_the_record_bytes() {
        for battle in 0..4 {
            for ability in 0..3 {
                for cure in 0..3 {
                    for targets in 0..8 {
                        let s = FriendAiSettings { battle, ability, cure, cure_targets: targets };
                        assert_eq!(FriendAiSettings::from_record(s.to_record()), s);
                    }
                }
            }
        }
        assert_eq!(FriendAiSettings::default().to_record(), [0, 0, 1, 7]);
        assert_eq!(FriendAiSettings::default().describe(), "Stick By Sora / Go Wild / Use in Emergencies (On Sora, On Self, On Others)");
    }

    #[test]
    fn chara_values_are_the_level_up_window_and_hud_slots() {
        assert_eq!(PartyChara::Donald.slot(), 1);
        assert_eq!(PartyChara::Goofy.slot(), 2);
        assert_eq!(PartyChara::from_name("Goofy"), Some(PartyChara::Goofy));
        assert_eq!(PartyChara::Donald.unique_id(), 11);
    }
}

// ------------------------------------------------------------------------------------------- (DON-UI) Donald's HUD

/// (DON-UI) Donald's HUD pieces (kh_ui::hud_resonance / hud_arcane): the views and their looks.
pub use kh_ui::hud_arcane::{ArcaneLayout, ArcaneView, TargetIconsView};
pub use kh_ui::hud_resonance::{ResonanceLayout, ResonanceView, RingLook, UltLook};

/// (DON-UI) DONALD'S HUD THIS FRAME (documentation/DONALD_OVERHAUL.md 6.1, 6.3, 6.5, 7, 9), for `Hud::set_don`: what
/// kh-partyhost's `PartyRuntime::don_hud_view` makes of kh-party's `DonHud` and his menu's side channel. Only while
/// Donald is the controlled leader.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DonHudView {
    /// The status HUD: the Resonance ring(s), pips, ultimate mark, the attune cast, the worn spells by the portraits.
    pub ring: ResonanceView,
    /// The command HUD: the Arcane Flow stars and the Zettaflare tracker.
    pub arcane: ArcaneView,
    /// Over the lock-on target: its statuses and Elemental Insight's elements.
    pub target: Option<TargetIconsView>,
    /// Scan (9): the lock-on target's HP gauge is shown (KH3: without Scan an enemy's HP is not shown).
    pub scan: bool,
    /// The ally target list (6.1): the rows' texts and the cursor (KH3's party target list, the menu's second level).
    pub targets: Option<(Vec<String>, usize)>,
    /// Quick Cast (9): the shortcut slot whose spell would trigger a Reaction (its name lit).
    pub quick_glow: Option<usize>,
    /// (DON-UI) His Y rows' star cost: (situation serial, stars, enabled).
    pub y_costs: Vec<(u32, u8, bool)>,
    /// The ids of Sora's team-attack rows on Y (their names come from the pack, as Sora's own).
    pub team_serials: Vec<(u32, i64)>,
    /// Which of his lists is open (kh-party `DonMenuView::list`): Magic's list changing in place (the elements <-> an
    /// element's spells) replays KH3's sub-list opening so the rows match their count.
    pub list: u8,
    /// Members wearing Aero Barrier / Ball Lightning: (who, barrier, orbs left); the HUD puts each by his portrait's seat.
    pub worn: Vec<(PartyChara, bool, u8)>,
    /// The look (sheet `party-don-hud`): applied when given.
    pub look: Option<DonLook>,
}

/// (DON-UI) The look of Donald's HUD pieces (sheet `party-don-hud`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DonLook {
    pub ring: ResonanceLayout,
    pub arcane: ArcaneLayout,
}
