//! THE CAMP MENU'S LOGIC (docs/sora-ui-spec.md 10.2 - 10.8), engine-free and display-free: what
//! the screens list, what the cursor does, what a decide changes, which sound plays. The host
//! supplies plain data (party values, the ability rows, the option rows) and applies the
//! [`CampAction`]s it gets back to the game's status and configuration. Drawing the screens
//! with the `CampMenu` movie is `camp_menu.rs`.
//!
//! The top screen (10.3), Abilities (10.4), Stats (10.7) and Config (10.8) are in this file;
//! Equipment and Items (10.5) in `camp_equip.rs`; Customize (10.6) and Game Help in
//! `camp_custom.rs`. Confirmations are [`Dialog`]s on top of the menu.
//! NOT implemented: Cuisine and the Gummiphone (their unlock flags are never set here), the
//! Abilities screen's "Show Next List" pages (Keyblade form actions and shotlocks: the
//! `TresKeybladeAbilityData` rows are not in the port's data), party members, character models.

use crate::camp_custom::{CustomLayer, CustomizeScreen, HelpLayer, HelpScreen};
use crate::camp_equip::{EquipLayer, EquipScreen, ItemsScreen};
use crate::frames::gauge_frame;
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_OPEN, SE_MENU_REFUSED};
use crate::list::ListView;
use crate::UiSound;

/// A menu event (spec 10.2's table), by meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampKey {
    Up,
    Down,
    Left,
    Right,
    /// Key 27.
    Decide,
    /// Key 28.
    Cancel,
    /// Key 22, the Menu button again.
    Menu,
    /// Key 0, the top face button.
    Triangle,
    /// Keys 6 / 7.
    L2,
    R2,
    /// Key 3, the left face button.
    Square,
    /// Keys 4 / 5.
    L1,
    R1,
}

/// The top screen's entries by id (table at `0x1486E0FF0`, in the code's order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopEntry {
    Equipment = 0,
    Abilities = 1,
    Cuisine = 2,
    Items = 3,
    Customize = 4,
    Stats = 5,
    Gummiphone = 6,
    GameHelp = 7,
    Config = 8,
    /// (er-host, IMPL_ER21) NOT AN ENTRY OF KH3'S CAMP MENU (its table has the nine above; KH3 reaches the title from the
    /// save-point menu): a host that wants a way out of the game without saving appends it after Config
    /// (`CampMenu::entries`). Deciding it asks KH3's own question (`kh3_menu.savepoint.dialog_0004`, cursor on No).
    TitleScreen = 9,
    /// (er-host, IMPL_ER59) NOT AN ENTRY OF KH3'S CAMP MENU: Sora's outfits (the user: "Boc is able to change Sora's
    /// appearance"); a host appends it when the outfits are open to the player ([`CampMenu::set_outfits`]).
    Outfits = 10,
    /// (party P) NOT AN ENTRY OF KH3'S CAMP MENU: the save point's "Party Formation" (the user's request 2026-10-15),
    /// shown as the camp menu's character step ([`CampMenu::open_formation`]).
    PartyFormation = 11,
}

impl TopEntry {
    pub const ORDER: [TopEntry; 9] = [TopEntry::Equipment, TopEntry::Abilities, TopEntry::Cuisine, TopEntry::Items, TopEntry::Customize, TopEntry::Stats, TopEntry::Gummiphone, TopEntry::GameHelp, TopEntry::Config];

    /// English text (`kh3_camp.top.menu_*`).
    pub fn text(self) -> &'static str {
        match self {
            TopEntry::Equipment => "Equipment",
            TopEntry::Abilities => "Abilities",
            TopEntry::Cuisine => "Cuisine",
            TopEntry::Items => "Items",
            TopEntry::Customize => "Customize",
            TopEntry::Stats => "Stats",
            TopEntry::Gummiphone => "Gummiphone",
            TopEntry::GameHelp => "Game Help",
            TopEntry::Config => "Config",
            TopEntry::TitleScreen => "Title Screen",
            TopEntry::Outfits => OUTFITS_TITLE,
            TopEntry::PartyFormation => FORMATION_TITLE,
        }
    }

    /// Equipment, Abilities and Stats first show the character list.
    pub fn needs_character(self) -> bool {
        matches!(self, TopEntry::Equipment | TopEntry::Abilities | TopEntry::Stats)
    }
}

/// The entries shown: hidden ones leave no gap; none is ever greyed.
pub fn top_entries(cuisine_unlocked: bool, smartphone_unlocked: bool) -> Vec<TopEntry> {
    TopEntry::ORDER.into_iter().filter(|e| match e {
        TopEntry::Cuisine => cuisine_unlocked,
        TopEntry::Gummiphone => smartphone_unlocked,
        _ => true,
    }).collect()
}

/// A vertical list of anchors chained with `toUp` / `toDown`, the two ends linked with
/// `repeatStop`: a fresh press wraps, a held direction stops at the end (10.2). Returns the
/// new index when the focus moved.
pub fn move_wrapping(cursor: usize, len: usize, down: bool, repeat: bool) -> Option<usize> {
    if len < 2 {
        return None;
    }
    let at_end = if down { cursor + 1 >= len } else { cursor == 0 };
    if at_end && repeat {
        return None;
    }
    Some(if down { (cursor + 1) % len } else { (cursor + len - 1) % len })
}

/// One owned ability as the host knows it (one per ability id).
#[derive(Clone, Debug, PartialEq)]
pub struct AbilityIn {
    pub id: i32,
    pub name: String,
    /// `ETresAbilityCategory`: 0 Actions, 1 Mobility, 2 Support.
    pub category: u8,
    /// `m_UseAP` of one equipped copy.
    pub ap: i32,
    /// `m_UIPriority` (ascending inside a category; equal priorities keep id order).
    pub priority: i32,
    /// Copies owned (1 ..= 3) and WHICH of them are equipped: bit n = copy n (fixes pass 1:
    /// it was a count, and the list then showed the first n copies as the equipped ones
    /// whichever row the player had toggled).
    pub owned: u8,
    pub equipped: u8,
    /// `m_HelpTextID`.
    pub help: String,
}

/// One row of the list: one owned copy.
#[derive(Clone, Debug, PartialEq)]
pub struct AbilityRow {
    pub id: i32,
    pub name: String,
    pub category: u8,
    pub ap: i32,
    /// Copy index 0 .. 2.
    pub copy: u8,
    pub equipped: bool,
    /// The first row of a category carries the category title.
    pub first_of_category: bool,
    pub help: String,
}

/// The ids of the reprisals (Counter Slash, Counter Impact, Counter Kick): one at a time.
pub const REPRISALS: [i32; 3] = [4, 5, 6];
/// Zero EXP: installing it asks for confirmation.
pub const ZERO_EXP: i32 = 26;

/// What a cursor position previews (event 20).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApPreview {
    /// The AP number shown.
    pub number: i32,
    /// `m_NowUseApAnim` label: `normal`, `equippable`, `lack`.
    pub label: &'static str,
    /// Frame of the increase (or lack) preview gauge; `None` = previews `off`.
    pub preview_frame: Option<i32>,
}

/// The Abilities screen of one character (`UTresUIP_CampMenuAbility`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AbilityScreen {
    pub rows: Vec<AbilityRow>,
    /// The list widget (`ability.abilityAnm.abilityList`): cursor and scroll.
    pub list: ListView,
    pub used_ap: i32,
    pub max_ap: i32,
}

impl AbilityScreen {
    /// The list builder `0x1415CD820` -> `0x1415CF300`: every ability once per owned copy, by
    /// category (Actions, Mobility, Support), inside a category by priority, then id; the
    /// copies of one ability are adjacent rows.
    pub fn build(abilities: &[AbilityIn], max_ap: i32) -> AbilityScreen {
        let mut sorted: Vec<&AbilityIn> = abilities.iter().filter(|a| a.owned > 0).collect();
        sorted.sort_by_key(|a| (a.category, a.priority, a.id));
        let mut rows = Vec::new();
        let mut last_category = None;
        for a in sorted {
            for copy in 0..a.owned.min(3) {
                rows.push(AbilityRow { id: a.id, name: a.name.clone(), category: a.category, ap: a.ap, copy, equipped: a.equipped & (1 << copy) != 0, first_of_category: last_category != Some(a.category), help: a.help.clone() });
                last_category = Some(a.category);
            }
        }
        let used_ap = rows.iter().filter(|r| r.equipped).map(|r| r.ap).sum();
        let mut list = ListView::new("ability.abilityAnm.abilityList", "AbilityListItem");
        list.set_count(rows.len());
        AbilityScreen { rows, list, used_ap, max_ap }
    }

    /// The row under the cursor.
    pub fn cursor(&self) -> usize {
        self.list.focus.max(0) as usize
    }

    pub fn set_cursor(&mut self, row: usize) {
        self.list.set_focus(row);
    }

    /// `m_ApGauge` (201 frames): `F(used, max, 200) + 1`.
    pub fn gauge_frame(&self) -> i32 {
        gauge_frame(self.used_ap, self.max_ap, 200) + 1
    }

    /// Whether an unequipped row may be equipped (`0x1415D2A00`): the cost fits and no other
    /// reprisal is equipped.
    pub fn may_equip(&self, row: usize) -> bool {
        let Some(r) = self.rows.get(row) else { return false };
        if r.equipped {
            return true;
        }
        let other_reprisal = REPRISALS.contains(&r.id) && self.rows.iter().any(|o| o.equipped && REPRISALS.contains(&o.id) && o.id != r.id);
        r.ap <= self.max_ap - self.used_ap && !other_reprisal
    }

    /// The preview of the row under the cursor.
    pub fn preview(&self, row: usize) -> ApPreview {
        match self.rows.get(row) {
            Some(r) if !r.equipped => {
                let would = self.used_ap + r.ap;
                ApPreview { number: would, label: if would > self.max_ap { "lack" } else { "equippable" }, preview_frame: Some(gauge_frame(would, self.max_ap, 200) + 1) }
            }
            _ => ApPreview { number: self.used_ap, label: "normal", preview_frame: None },
        }
    }

    /// Decide on the row under the cursor: the toggle. `Some((id, copy, on))` when the copy's
    /// equipped bit is to be written (`SetEquipped`), with static SE 1; `None` (static SE 4)
    /// when it is refused.
    pub fn toggle(&mut self, sounds: &mut Vec<UiSound>) -> Option<(i32, u8, bool)> {
        let row = self.cursor();
        if row >= self.rows.len() {
            return None;
        }
        if !self.may_equip(row) {
            sounds.push(UiSound::Play(SE_MENU_REFUSED));
            return None;
        }
        let r = &mut self.rows[row];
        r.equipped = !r.equipped;
        self.used_ap += if r.equipped { r.ap } else { -r.ap };
        sounds.push(UiSound::Play(SE_MENU_DECIDE));
        Some((r.id, r.copy, r.equipped))
    }
}

/// (GOOFY-GG) The name of the third gauge row of the Stats / party pages: Sora's Focus; Goofy's GUARD (the user,
/// 2026-10-16: his guard runs on a GUARD gauge instead of Focus). `name` is the snapshot's `StatsIn::name`.
pub fn third_gauge_label(name: &str) -> &'static str {
    if name == "Goofy" {
        "GUARD"
    } else {
        "Focus"
    }
}

/// The party slot's status snapshot as the Stats screen shows it (10.7).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StatsIn {
    pub name: String,
    pub level: i32,
    /// Total EXP, the next level's total (0 at level 99), this level's total.
    pub exp: i32,
    pub next_level_exp: i32,
    pub level_exp: i32,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    pub focus: i32,
    pub focus_max: i32,
    /// Game flag byte +0x95B0 bit 4: the focus gauge exists.
    pub focus_unlocked: bool,
    pub ap_max: i32,
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
    /// Fire, Blizzard, Thunder, Water, Aero, Dark, percent.
    pub resists: [i32; 6],
    pub munny: i32,
    /// Play time in seconds.
    pub play_seconds: u32,
}

impl StatsIn {
    /// "Next Level": the EXP still needed; 0 at level 99.
    pub fn next_level(&self) -> i32 {
        if self.level >= 99 || self.next_level_exp <= 0 {
            0
        } else {
            self.next_level_exp - self.exp
        }
    }

    /// The Stats screen's EXP gauge (101 frames): `F(exp - levelExp, next - levelExp, 100) + 1`;
    /// frame 101 at level 99.
    pub fn stats_exp_frame(&self) -> i32 {
        if self.level >= 99 || self.next_level_exp <= 0 {
            return 101;
        }
        gauge_frame(self.exp - self.level_exp, self.next_level_exp - self.level_exp, 100) + 1
    }

    /// The top screen panel's EXP gauge (151 frames): the same with 150 frames; at level 99
    /// `nextLevelExp` is 0, so the gauge shows frame 1 (empty).
    pub fn panel_exp_frame(&self) -> i32 {
        gauge_frame(self.exp - self.level_exp, self.next_level_exp - self.level_exp, 150) + 1
    }

    /// Play time as the frame shows it: `hours:minutes`, capped at 3,599,999 s.
    pub fn play_time(&self) -> String {
        let s = self.play_seconds.min(3_599_999);
        format!("{}:{:02}", s / 3600, s / 60 % 60)
    }

    /// The Stats screen's fields in the spec's order, as (label, text).
    pub fn fields(&self) -> Vec<(&'static str, String)> {
        let cap = |v: i32| v.min(999).to_string();
        let (focus, focus_label) = if self.focus_unlocked { (format!("{} / {}", cap(self.focus), cap(self.focus_max)), third_gauge_label(&self.name)) /* (GOOFY-GG) */ } else { ("— / —".to_string(), "? ? ?") };
        let mut out = vec![
            ("Level", self.level.to_string()),
            ("Total EXP", self.exp.to_string()),
            ("Next Level", self.next_level().to_string()),
            ("HP", format!("{} / {}", cap(self.hp), cap(self.hp_max))),
            ("MP", format!("{} / {}", cap(self.mp), cap(self.mp_max))),
            (focus_label, focus),
            ("AP", cap(self.ap_max)),
            ("Strength", cap(self.strength)),
            ("Magic", cap(self.magic)),
            ("Defense", cap(self.defense)),
        ];
        for (label, value) in ["Fire Resistance", "Blizzard Resistance", "Thunder Resistance", "Water Resistance", "Aero Resistance", "Dark Resistance"].into_iter().zip(self.resists) {
            out.push((label, format!("{}%", value.min(100))));
        }
        out
    }
}

/// One row of the Config screen as the host describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionRow {
    pub label: String,
    /// Choice texts; empty for a slider.
    pub choices: Vec<String>,
    /// Slider range (for a slider row).
    pub range: (u8, u8),
    /// Choice index or slider value.
    pub value: u8,
    /// False: the row is not stored on this platform (shown, not changeable).
    pub stored: bool,
}

impl OptionRow {
    pub fn value_text(&self) -> String {
        if self.choices.is_empty() {
            self.value.to_string()
        } else {
            self.choices.get(self.value as usize).cloned().unwrap_or_default()
        }
    }
}

/// (party P) The title of the Party Formation step (LOCAL text).
pub const FORMATION_TITLE: &str = "Party Formation";

/// (party P) THE PARTY FORMATION STEP's texts, the host's: one row per roster character (its name and its place:
/// "Leader", "Slot 2", "-"), the help line (the controls, or why an edit was refused).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FormationRows {
    pub rows: Vec<String>,
    pub help: String,
    /// Counts up when the texts change (the view redraws them).
    pub stamp: u32,
    /// (party PF) THE ICON SCREEN'S FEED, generic for every host (test level, Elden Ring): the roster in screen order
    /// (`rows` keeps the text form), the active members (roster indices, slot order), the leader (a roster index), and
    /// the line of a refused edit (`None` = accepted / no edit).
    pub members: Vec<FormationMember>,
    pub active: Vec<usize>,
    pub leader: usize,
    pub refused: Option<String>,
}

/// (party PF) One roster character of the Party Formation screen: its key (`sora`, `donald`, `goofy` ...: which
/// pictures), its shown name, its level (0 = not shown) and whether it is still locked (not unlocked yet: a silhouette).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FormationMember {
    pub key: String,
    pub name: String,
    pub level: u32,
    pub locked: bool,
}

/// Which screen of the menu is up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CampScreen {
    #[default]
    Top,
    /// The character list of an entry that needs one.
    Character(TopEntry),
    Equipment,
    Abilities,
    Items,
    Customize,
    Stats,
    GameHelp,
    Config,
    /// A screen the port does not have (Cuisine, Gummiphone: their unlock flags are never set
    /// in the test level): a notice; cancel returns.
    Unavailable(TopEntry),
    /// (er-host, IMPL_ER59) Sora's outfits: one list (drawn in the Game Help screen's category list).
    Outfits,
}

/// What the host has to do for the menu.
#[derive(Clone, Debug, PartialEq)]
pub enum CampAction {
    /// `SetEquipped(record, id, copy, on)`: in force at once.
    SetAbility { id: i32, copy: u8, on: bool },
    /// The Config screen was left with changes: write the rows' values back, apply, save.
    ApplyConfig(Vec<u8>),
    /// Triangle + Yes on the Config screen: restore the defaults (the host answers with new rows).
    RestoreConfigDefaults,
    /// A sub screen opens: the host supplies its data.
    Opened(CampScreen),
    /// Equipment: the piece `id` of the stock goes into the slot (`None`: the slot is emptied);
    /// the old piece goes back to the stock; the status is rebuilt. In force at once.
    Equip { kind: u8, slot: usize, id: Option<String> },
    /// Equipment: Auto-Reload for every pouch slot holding the item.
    AutoReload { item: String, on: bool },
    /// Items: one of the item is used on Sora (stock - 1, the item command's effect).
    UseItem { id: String },
    /// Items: every copy of the entry is discarded.
    Discard { id: String },
    /// Customize: a tier is written into a magic slot; two slots swap; a shortcut is written.
    SetMagicSlot { slot: usize, kind: u8 },
    SwapMagicSlots(usize, usize),
    SetShortcut { page: usize, slot: usize, kind: u8 },
    /// (XSI, 2026-10-16) Customize: the Shotlock choice (`ETresShootFlowKind`: 0 the Keyblade's own, 11 Spectral Rays).
    SetShotlock(u8),
    /// The whole menu closes (root `out`, static SE 3): unpause when the animation ends.
    Close,
    /// (er-host, IMPL_ER21) "Title Screen" was chosen and confirmed: the host leaves for its title screen (the menu
    /// closes with it: a `Close` follows).
    TitleScreen,
    /// (er-host, IMPL_ER59) An outfit was chosen (its id, as the host gave it in [`CampMenu::set_outfits`]).
    SetOutfit(String),
    /// (party) Customize -> a friend: its AI settings changed (party slot `member`; Combat Style, Abilities,
    /// Recovery as enum values; the three target bits). In force at once (the save record bytes +0x158 ..).
    SetFriendAi { member: usize, styles: [u8; 3], targets: u8 },
    /// (party P) Party Formation: roster row `row` slotted in / out (A) or made the leader (Y, `lead`); the host applies
    /// the rules and answers with [`CampMenu::formation_answer`] (its sound, the new texts).
    Formation { row: usize, lead: bool },
}

/// Dialog kinds of `0x1415A1C70(mgr, kind, text, ...)`: 1 = message with OK, 2 = Yes / No.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    Ok,
    YesNo,
}

/// What a "Yes" does.
#[derive(Clone, Debug, PartialEq)]
pub enum Pending {
    None,
    /// Install Zero EXP (`kh3_camp.ability.dialog_0001`).
    ZeroExp,
    RestoreConfig,
    Discard(String),
    AutoReloadOff(String),
    /// (er-host, IMPL_ER21) Return to the title screen (`kh3_menu.savepoint.dialog_0004`).
    TitleScreen,
    /// (party) Restore the focused friend's AI settings (Customize, `0x1415E16B0`).
    FriendDefaults,
}

/// A confirmation on top of the menu (`00_system/Dialog`).
#[derive(Clone, Debug, PartialEq)]
pub struct Dialog {
    pub text_key: &'static str,
    /// Replaces `{0}` of the text.
    pub argument: String,
    pub kind: DialogKind,
    /// Yes / No: 0 = Yes, 1 = No.
    pub cursor: usize,
    pub pending: Pending,
}

impl Dialog {
    pub fn message(text_key: &'static str) -> Dialog {
        Dialog { text_key, argument: String::new(), kind: DialogKind::Ok, cursor: 0, pending: Pending::None }
    }
}

/// The Config screen's window: 10 visible rows, no margin (`UTresASProxyRSLConfig` +0xD0, +0xD4).
pub const CONFIG_VISIBLE: usize = 10;

/// The camp menu while it is open.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CampMenu {
    pub screen: CampScreen,
    pub entries: Vec<TopEntry>,
    pub top_cursor: usize,
    pub abilities: AbilityScreen,
    pub stats: StatsIn,
    pub options: Vec<OptionRow>,
    pub option_cursor: usize,
    /// The Config window's first row (native +0x11C).
    pub option_scroll: usize,
    options_before: Vec<u8>,
    pub equipment: EquipScreen,
    pub items: ItemsScreen,
    pub customize: CustomizeScreen,
    pub help: HelpScreen,
    pub dialog: Option<Dialog>,
    /// (er-host, IMPL_ER59) Sora's outfits.
    pub outfits: OutfitScreen,
    /// (party) THE PARTY TABLE (`m_PartyParams[5]`, 0x14154C710): slot 0 Sora, then the friends; `stats` is the
    /// chosen member's copy. Set by [`CampMenu::set_party`]; Sora alone = one entry.
    pub party: Vec<StatsIn>,
    /// (party) The party slot the sub screen shows (sub menu +0x268 / +0x2A8), the character list's cursor, and the
    /// last member switch (true = next: the view plays its clip once).
    pub member: usize,
    pub chara_cursor: usize,
    pub member_flash: Option<bool>,
    /// (party P) Party Formation's texts while the menu is that step (`None` otherwise).
    pub formation: Option<FormationRows>,
}

impl CampMenu {
    /// Camp open (`0x1415C8DD0`): static SE 2, focus on entry 0.
    pub fn open(cuisine_unlocked: bool, smartphone_unlocked: bool, stats: StatsIn, sounds: &mut Vec<UiSound>) -> CampMenu {
        sounds.push(UiSound::Play(SE_MENU_OPEN));
        CampMenu { entries: top_entries(cuisine_unlocked, smartphone_unlocked), party: vec![stats.clone()], stats, equipment: EquipScreen::new(), items: ItemsScreen::new(), customize: CustomizeScreen::new(), help: HelpScreen::new(), ..CampMenu::default() }
    }

    /// (party P) THE SAVE POINT'S PARTY FORMATION: the camp menu opened on its character step, one row per roster
    /// character (`rows`), the panels of the ACTIVE members (`party`, slot order); A slots the row's character in /
    /// out, Y (Triangle) makes it the leader, B / Menu closes. Static SE 2 as the camp's open.
    pub fn open_formation(party: Vec<StatsIn>, rows: FormationRows, sounds: &mut Vec<UiSound>) -> CampMenu {
        sounds.push(UiSound::Play(SE_MENU_OPEN));
        let stats = party.first().cloned().unwrap_or_default();
        CampMenu { entries: vec![TopEntry::PartyFormation], party, stats, screen: CampScreen::Character(TopEntry::PartyFormation), formation: Some(rows), equipment: EquipScreen::new(), items: ItemsScreen::new(), customize: CustomizeScreen::new(), help: HelpScreen::new(), ..CampMenu::default() }
    }

    /// (party P) The host's answer to `CampAction::Formation`: accepted (static SE decide) or refused (the error buzz),
    /// the new texts and the active members' panels.
    pub fn formation_answer(&mut self, ok: bool, party: Vec<StatsIn>, mut rows: FormationRows, sounds: &mut Vec<UiSound>) {
        sounds.push(UiSound::Play(if ok { SE_MENU_DECIDE } else { SE_MENU_REFUSED }));
        rows.stamp = self.formation.as_ref().map_or(0, |f| f.stamp) + 1;
        self.formation = Some(rows);
        if !party.is_empty() {
            self.party = party;
            self.stats = self.party[0].clone();
        }
    }

    /// (party) The party table: Sora's status first, then the friends' (the host's answer, again after a change).
    /// The chosen member's copy becomes `stats`.
    pub fn set_party(&mut self, party: Vec<StatsIn>) {
        if party.is_empty() {
            return;
        }
        self.party = party;
        self.member = self.member.min(self.party.len() - 1);
        self.chara_cursor = self.chara_cursor.min(self.party.len() - 1);
        self.stats = self.party[self.member].clone();
        self.items.party_size = self.party.len();
    }

    /// (party) L2 / R2 on Equipment, Abilities and Stats: the previous / next party slot that holds a member,
    /// wrapping (`0x1415BC3D0` / `0x1415D6540` / `0x1415D70D0`: search away from the current slot, then from the other
    /// end); true when the member changed (the screen then plays its clip and static SE 2; nothing otherwise).
    pub fn switch_member(&mut self, next: bool, sounds: &mut Vec<UiSound>, actions: &mut Vec<CampAction>) -> bool {
        let n = self.party.len();
        if n < 2 {
            return false;
        }
        self.member = if next { (self.member + 1) % n } else { (self.member + n - 1) % n };
        self.stats = self.party[self.member].clone();
        self.member_flash = Some(next);
        sounds.push(UiSound::Play(SE_MENU_OPEN));
        match self.screen {
            CampScreen::Equipment => self.equipment = EquipScreen::new(),
            CampScreen::Abilities => self.abilities = AbilityScreen::default(),
            _ => {}
        }
        actions.push(CampAction::Opened(self.screen));
        true
    }

    pub fn focused_entry(&self) -> Option<TopEntry> {
        self.entries.get(self.top_cursor).copied()
    }

    /// The host's answer to `Opened(Abilities)`.
    pub fn set_abilities(&mut self, abilities: &[AbilityIn], max_ap: i32) {
        // The cursor and the window stay where they are (a toggle redraws the list in place).
        let (focus, scroll) = (self.abilities.list.focus, self.abilities.list.scroll);
        self.abilities = AbilityScreen::build(abilities, max_ap);
        if focus >= 0 && (focus as usize) < self.abilities.rows.len() {
            self.abilities.list.focus = focus;
            self.abilities.list.scroll = scroll.min(self.abilities.list.scroll_max());
            self.abilities.list.adjust_display();
        }
    }

    /// The host's answer to `Opened(Config)` and to `RestoreConfigDefaults`.
    pub fn set_options(&mut self, rows: Vec<OptionRow>, remember: bool) {
        if remember {
            self.options_before = rows.iter().map(|r| r.value).collect();
        }
        self.options = rows;
        self.option_cursor = self.option_cursor.min(self.options.len().saturating_sub(1));
    }

    fn config_scroll_max(&self) -> usize {
        self.options.len().saturating_sub(CONFIG_VISIBLE)
    }

    fn config_focus(&mut self, row: usize) {
        self.option_cursor = row;
        self.option_scroll = crate::list::window_scroll(row, self.option_scroll, CONFIG_VISIBLE, 0, self.config_scroll_max());
    }

    fn open_sub(&mut self, entry: TopEntry, sounds: &mut Vec<UiSound>, actions: &mut Vec<CampAction>) {
        self.screen = match entry {
            TopEntry::Equipment => CampScreen::Equipment,
            TopEntry::Abilities => CampScreen::Abilities,
            TopEntry::Items => CampScreen::Items,
            TopEntry::Customize => CampScreen::Customize,
            TopEntry::Stats => CampScreen::Stats,
            TopEntry::GameHelp => CampScreen::GameHelp,
            TopEntry::Config => CampScreen::Config,
            TopEntry::Outfits => CampScreen::Outfits,
            other => CampScreen::Unavailable(other),
        };
        match self.screen {
            CampScreen::Equipment => self.equipment = EquipScreen::new(),
            CampScreen::Items => self.items = ItemsScreen::new(),
            CampScreen::Customize => self.customize = CustomizeScreen::new(),
            CampScreen::GameHelp => self.help = HelpScreen::new(),
            CampScreen::Config => (self.option_cursor, self.option_scroll) = (0, 0),
            CampScreen::Outfits => self.outfits.opened(),
            _ => {}
        }
        sounds.push(UiSound::Play(SE_MENU_OPEN));
        actions.push(CampAction::Opened(self.screen));
    }

    /// A dialog's answer. Returns the actions of a "Yes".
    fn dialog_key(&mut self, key: CampKey, sounds: &mut Vec<UiSound>) -> Vec<CampAction> {
        let mut actions = Vec::new();
        let Some(dialog) = self.dialog.as_mut() else { return actions };
        match key {
            CampKey::Left | CampKey::Right | CampKey::Up | CampKey::Down if dialog.kind == DialogKind::YesNo => {
                dialog.cursor ^= 1;
                sounds.push(UiSound::Play(SE_MENU_CURSOR));
            }
            CampKey::Decide => {
                let dialog = self.dialog.take().unwrap();
                let yes = dialog.kind == DialogKind::YesNo && dialog.cursor == 0;
                sounds.push(UiSound::Play(if yes || dialog.kind == DialogKind::Ok { SE_MENU_DECIDE } else { SE_MENU_CLOSE }));
                if yes {
                    match dialog.pending {
                        Pending::ZeroExp => {
                            if let Some((id, copy, on)) = self.abilities.toggle(&mut Vec::new()) {
                                actions.push(CampAction::SetAbility { id, copy, on });
                            }
                        }
                        Pending::RestoreConfig => actions.push(CampAction::RestoreConfigDefaults),
                        Pending::Discard(id) => actions.push(CampAction::Discard { id }),
                        Pending::AutoReloadOff(item) => actions.push(CampAction::AutoReload { item, on: false }),
                        Pending::TitleScreen => {
                            actions.push(CampAction::TitleScreen);
                            actions.push(CampAction::Close);
                        }
                        Pending::FriendDefaults => self.customize.restore_friend_defaults(&mut actions),
                        Pending::None => {}
                    }
                }
            }
            CampKey::Cancel => {
                self.dialog = None;
                sounds.push(UiSound::Play(SE_MENU_CLOSE));
            }
            _ => {}
        }
        actions
    }

    /// The sub screen is at its own top level (the Menu key then closes the whole menu;
    /// otherwise it steps back like cancel: vt+0x2A0 / +0x2A8).
    fn sub_at_top(&self) -> bool {
        match self.screen {
            CampScreen::Equipment => self.equipment.layer == EquipLayer::Slots,
            CampScreen::Items => !self.items.target,
            CampScreen::Customize => self.customize.layer == CustomLayer::Top,
            CampScreen::GameHelp => self.help.layer == HelpLayer::Categories,
            _ => true,
        }
    }

    fn leave_config(&mut self, actions: &mut Vec<CampAction>) {
        // Leaving applies: the copy is written back when it differs.
        let now: Vec<u8> = self.options.iter().map(|r| r.value).collect();
        if now != self.options_before {
            actions.push(CampAction::ApplyConfig(now));
        }
    }

    /// One menu event. `repeat` = the key is held (auto-repeat).
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) -> Vec<CampAction> {
        if self.dialog.is_some() {
            return self.dialog_key(key, sounds);
        }
        let mut actions = Vec::new();
        // The Menu key in a sub screen.
        let mut key = key;
        if key == CampKey::Menu && !matches!(self.screen, CampScreen::Top | CampScreen::Character(_)) {
            if self.sub_at_top() {
                if self.screen == CampScreen::Config {
                    self.leave_config(&mut actions);
                }
                sounds.push(UiSound::Play(SE_MENU_CLOSE));
                actions.push(CampAction::Close);
                return actions;
            }
            key = CampKey::Cancel;
        }
        match self.screen {
            CampScreen::Top => match key {
                CampKey::Up | CampKey::Down => {
                    if let Some(to) = move_wrapping(self.top_cursor, self.entries.len(), key == CampKey::Down, repeat) {
                        self.top_cursor = to;
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                CampKey::Decide => {
                    // (party) Items and Customize work on the whole party: the slot is Sora's for them.
                    self.member = 0;
                    self.chara_cursor = 0;
                    if let Some(stats) = self.party.first() {
                        self.stats = stats.clone();
                    }
                    if let Some(entry) = self.focused_entry() {
                        sounds.push(UiSound::Play(SE_MENU_DECIDE));
                        if entry == TopEntry::TitleScreen {
                            // (er-host, IMPL_ER21) KH3's question, the cursor on No
                            self.dialog = Some(Dialog { text_key: "kh3_menu.savepoint.dialog_0004", argument: String::new(), kind: DialogKind::YesNo, cursor: 1, pending: Pending::TitleScreen });
                        } else if entry.needs_character() {
                            self.screen = CampScreen::Character(entry);
                        } else {
                            self.open_sub(entry, sounds, &mut actions);
                        }
                    }
                }
                CampKey::Cancel | CampKey::Menu => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    actions.push(CampAction::Close);
                }
                _ => {}
            },
            // The character list (`charaList.btnChara1 .. 5`, one per party slot; shown for Sora alone too).
            // (party) up / down walk the members ([?] wrapping as the top list); decide opens the sub menu for that
            // party slot (sub menu vt+0x280).
            // (party P) the Party Formation step: A in / out, Y leader, B closes the menu
            CampScreen::Character(TopEntry::PartyFormation) => {
                // (party PF) the roster is a ROW of icons: left / right walk it (up / down too, the same way)
                let n = self.formation.as_ref().map_or(0, |f| f.members.len().max(f.rows.len())).max(1);
                match key {
                    CampKey::Up | CampKey::Down | CampKey::Left | CampKey::Right => {
                        if let Some(to) = move_wrapping(self.chara_cursor, n, matches!(key, CampKey::Down | CampKey::Right), repeat) {
                            self.chara_cursor = to;
                            sounds.push(UiSound::Play(SE_MENU_CURSOR));
                        }
                    }
                    CampKey::Decide => actions.push(CampAction::Formation { row: self.chara_cursor, lead: false }),
                    CampKey::Triangle => actions.push(CampAction::Formation { row: self.chara_cursor, lead: true }),
                    CampKey::Cancel | CampKey::Menu => {
                        sounds.push(UiSound::Play(SE_MENU_CLOSE));
                        actions.push(CampAction::Close);
                    }
                    _ => {}
                }
            }
            CampScreen::Character(entry) => match key {
                CampKey::Up | CampKey::Down => {
                    if let Some(to) = move_wrapping(self.chara_cursor, self.party.len().max(1), key == CampKey::Down, repeat) {
                        self.chara_cursor = to;
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                CampKey::Decide => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    self.member = self.chara_cursor.min(self.party.len().saturating_sub(1));
                    if let Some(stats) = self.party.get(self.member) {
                        self.stats = stats.clone();
                    }
                    self.open_sub(entry, sounds, &mut actions);
                }
                CampKey::Cancel => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.screen = CampScreen::Top;
                }
                // (DON-UI) the Menu key closes the whole menu, as from any screen at its own top (B is the one-level back)
                CampKey::Menu => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    actions.push(CampAction::Close);
                }
                _ => {}
            },
            CampScreen::Abilities => match key {
                CampKey::Up | CampKey::Down => {
                    if self.abilities.list.move_anchor(key == CampKey::Down, repeat) {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                // Page up / down (the lists' `prevPage` / `nextPage`).
                CampKey::L1 | CampKey::R1 => {
                    let moved = if key == CampKey::R1 { self.abilities.list.next_page(repeat) } else { self.abilities.list.prev_page(repeat) };
                    if moved {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                CampKey::Decide => {
                    // Zero EXP (id 26): equipping asks first (`0x1415C50C0`).
                    let row = self.abilities.cursor();
                    let asks = self.abilities.rows.get(row).is_some_and(|r| r.id == ZERO_EXP && !r.equipped) && self.abilities.may_equip(row);
                    if asks {
                        self.dialog = Some(Dialog { text_key: "kh3_camp.ability.dialog_0001", argument: String::new(), kind: DialogKind::YesNo, cursor: 1, pending: Pending::ZeroExp });
                    } else if let Some((id, copy, on)) = self.abilities.toggle(sounds) {
                        actions.push(CampAction::SetAbility { id, copy, on });
                    }
                }
                // (party) previous / next member (`0x1415C59B0` events 11 / 12)
                CampKey::L2 | CampKey::R2 => drop(self.switch_member(key == CampKey::R2, sounds, &mut actions)),
                CampKey::Cancel => self.back(sounds),
                _ => {}
            },
            // (party) Stats: L2 / R2 previous / next member (`0x1415E4340`).
            CampScreen::Stats if matches!(key, CampKey::L2 | CampKey::R2) => drop(self.switch_member(key == CampKey::R2, sounds, &mut actions)),
            CampScreen::Stats | CampScreen::Unavailable(_) => {
                if key == CampKey::Cancel {
                    self.back(sounds);
                }
            }
            // (party) Equipment: L2 / R2 only on the equipped list (`0x1415E30D0`: not while the stock list is up).
            CampScreen::Equipment if matches!(key, CampKey::L2 | CampKey::R2) && self.equipment.layer == EquipLayer::Slots => drop(self.switch_member(key == CampKey::R2, sounds, &mut actions)),
            CampScreen::Equipment => {
                let stats = self.stats.clone();
                if self.equipment.key(key, repeat, &stats, sounds, &mut actions, &mut self.dialog) {
                    self.back(sounds);
                }
            }
            CampScreen::Items => {
                if self.items.key(key, repeat, sounds, &mut actions, &mut self.dialog) {
                    self.back(sounds);
                }
            }
            CampScreen::Customize => {
                if self.customize.key(key, repeat, sounds, &mut actions) {
                    self.back(sounds);
                }
                // (party) Triangle on a friend's layer: "restore the defaults?", Yes / No with the cursor on No
                // (0x1415E1FA0 -> dialog kind 2 -> 0x1415E16B0). [?] The dialog's text key is a run-time table entry
                // (DAT_1486E3020) not read: the Config screen's restore question stands in.
                if std::mem::take(&mut self.customize.restore_request) {
                    self.dialog = Some(Dialog { text_key: "kh3_camp.config.dialog_0010", argument: String::new(), kind: DialogKind::YesNo, cursor: 1, pending: Pending::FriendDefaults });
                }
            }
            CampScreen::GameHelp => {
                if self.help.key(key, repeat, sounds) {
                    self.back(sounds);
                }
            }
            CampScreen::Outfits => {
                if self.outfits.key(key, repeat, sounds, &mut actions) {
                    self.back(sounds);
                }
            }
            CampScreen::Config => match key {
                CampKey::Up | CampKey::Down => {
                    if let Some(to) = move_wrapping(self.option_cursor, self.options.len(), key == CampKey::Down, repeat) {
                        self.config_focus(to);
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                // Events 9 / 10 (`0x141541A20`): a page of rows; at the ends a fresh press
                // wraps, a held key stops.
                CampKey::L1 | CampKey::R1 => {
                    let (count, forward) = (self.options.len(), key == CampKey::R1);
                    if count == 0 || (repeat && if forward { self.option_cursor + 1 >= count } else { self.option_cursor == 0 }) {
                        return actions;
                    }
                    let before = self.option_cursor;
                    if self.option_cursor == 0 && !forward {
                        self.config_focus(count - 1);
                    } else if self.option_cursor + 1 == count && forward {
                        self.config_focus(0);
                    } else {
                        let old = self.option_scroll as i64;
                        let wanted = old + if forward { CONFIG_VISIBLE as i64 } else { -(CONFIG_VISIBLE as i64) };
                        let scroll = wanted.clamp(0, self.config_scroll_max() as i64);
                        self.option_scroll = scroll as usize;
                        let mut row = (self.option_cursor as i64 - old + scroll) as usize;
                        if scroll == old {
                            row = if forward { count - 1 } else { 0 };
                        }
                        self.config_focus(row.min(count - 1));
                    }
                    if self.option_cursor != before {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                CampKey::Left | CampKey::Right => {
                    if let Some(row) = self.options.get_mut(self.option_cursor).filter(|r| r.stored) {
                        let (min, max) = if row.choices.is_empty() { row.range } else { (0, row.choices.len().saturating_sub(1) as u8) };
                        let to = if key == CampKey::Right { row.value.saturating_add(1).min(max) } else { row.value.saturating_sub(1).max(min) };
                        if to != row.value {
                            row.value = to;
                            sounds.push(UiSound::Play(SE_MENU_CURSOR));
                        }
                    }
                }
                // "Restore Defaults" after `kh3_camp.config.dialog_0010`.
                CampKey::Triangle => {
                    self.dialog = Some(Dialog { text_key: "kh3_camp.config.dialog_0010", argument: String::new(), kind: DialogKind::YesNo, cursor: 1, pending: Pending::RestoreConfig });
                }
                CampKey::Cancel => {
                    self.leave_config(&mut actions);
                    self.back(sounds);
                }
                _ => {}
            },
        }
        actions
    }

    fn back(&mut self, sounds: &mut Vec<UiSound>) {
        sounds.push(UiSound::Play(SE_MENU_CLOSE));
        // (DON-UI) B goes back exactly one level (the user, 2026-10-22, for ALL menus: "when you press B it should bring
        // you back to the previous menu, not to the origin menu"): a screen reached through the character list returns
        // to that list (the movie's own `backChara`, the cursor on the member), the others to the top
        let entry = match self.screen {
            CampScreen::Equipment => Some(TopEntry::Equipment),
            CampScreen::Abilities => Some(TopEntry::Abilities),
            CampScreen::Stats => Some(TopEntry::Stats),
            CampScreen::Unavailable(e) => Some(e),
            _ => None,
        };
        self.screen = match entry.filter(|e| e.needs_character()) {
            Some(e) => CampScreen::Character(e),
            None => CampScreen::Top,
        };
    }

    /// (er-host, IMPL_ER59) The outfits the host offers (id, shown name) and the one worn; the entry is appended
    /// after the others (before "Title Screen" when that one is there) when it is not there yet.
    pub fn set_outfits(&mut self, outfits: Vec<(String, String)>, worn: &str) {
        self.outfits.set(outfits, worn);
        if !self.entries.contains(&TopEntry::Outfits) {
            let at = self.entries.iter().position(|e| *e == TopEntry::TitleScreen).unwrap_or(self.entries.len());
            self.entries.insert(at, TopEntry::Outfits);
        }
    }
}

/// (er-host, IMPL_ER59) LOCAL TEXT (KH3 has no outfit menu, so no string of its own): the entry's name and help line.
pub const OUTFITS_TITLE: &str = "Outfits";
pub const OUTFITS_DETAIL: &str = "Change Sora's outfit.";

/// (er-host, IMPL_ER59) THE OUTFIT LIST: up / down (wrapping like the game's lists), Decide wears the outfit under the
/// cursor (the screen stays: another can be tried at once), Cancel goes back.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct OutfitScreen {
    /// (id, shown name).
    pub outfits: Vec<(String, String)>,
    /// The index of the one worn.
    pub worn: usize,
    /// Drawn in the Game Help screen's category list.
    pub list: ListView,
}

impl OutfitScreen {
    pub fn set(&mut self, outfits: Vec<(String, String)>, worn: &str) {
        self.worn = outfits.iter().position(|(id, _)| id == worn).unwrap_or(0);
        self.outfits = outfits;
        if self.list.path.is_empty() {
            self.list = ListView::new("gameHelp.gameHelpAnm.gameHelpList", "CoGameHelpCategoryListItem");
        }
        self.list.set_count(self.outfits.len());
    }

    /// The screen opens: the cursor on the one worn.
    pub fn opened(&mut self) {
        if self.list.path.is_empty() {
            self.list = ListView::new("gameHelp.gameHelpAnm.gameHelpList", "CoGameHelpCategoryListItem");
        }
        self.list.set_count(self.outfits.len());
        if !self.outfits.is_empty() {
            self.list.set_focus(self.worn.min(self.outfits.len() - 1));
        }
        self.list.has_focus = true;
        self.list.invalidate();
    }

    pub fn cursor(&self) -> Option<usize> {
        usize::try_from(self.list.focus).ok().filter(|i| *i < self.outfits.len())
    }

    /// One event; true = leave the screen.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>, actions: &mut Vec<CampAction>) -> bool {
        match key {
            CampKey::Up | CampKey::Down => {
                if self.list.move_anchor(key == CampKey::Down, repeat) {
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                }
            }
            CampKey::Decide => match self.cursor() {
                Some(i) => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    if i != self.worn {
                        self.worn = i;
                        actions.push(CampAction::SetOutfit(self.outfits[i].0.clone()));
                    }
                }
                None => sounds.push(UiSound::Play(SE_MENU_REFUSED)),
            },
            CampKey::Cancel => return true,
            _ => {}
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats_99() -> StatsIn {
        StatsIn { name: "Sora".into(), level: 99, exp: 1_527_640, next_level_exp: 0, level_exp: 1_527_640, hp: 265, hp_max: 265, mp: 169, mp_max: 169, focus: 100, focus_max: 100, focus_unlocked: true, ap_max: 180, strength: 66, magic: 67, defense: 59, resists: [0; 6], munny: 0, play_seconds: 3725 }
    }

    #[test]
    fn spec_11b_test_11_top_screen() {
        let mut sounds = Vec::new();
        let mut menu = CampMenu::open(false, false, stats_99(), &mut sounds);
        assert_eq!(sounds, vec![UiSound::Play(2)]);
        let texts: Vec<&str> = menu.entries.iter().map(|e| e.text()).collect();
        assert_eq!(texts, ["Equipment", "Abilities", "Items", "Customize", "Stats", "Game Help", "Config"]);
        assert_eq!(menu.focused_entry(), Some(TopEntry::Equipment));
        // Down from Config (fresh press) wraps to Equipment; holding down stops at Config.
        sounds.clear();
        for _ in 0..6 {
            menu.key(CampKey::Down, true, &mut sounds);
        }
        assert_eq!(menu.focused_entry(), Some(TopEntry::Config));
        menu.key(CampKey::Down, true, &mut sounds);
        assert_eq!((menu.focused_entry(), sounds.len()), (Some(TopEntry::Config), 6), "held: stops, no sound");
        menu.key(CampKey::Down, false, &mut sounds);
        assert_eq!(menu.focused_entry(), Some(TopEntry::Equipment));
        assert_eq!(sounds, vec![UiSound::Play(0); 7]);
        // The panel and the frame.
        assert_eq!((menu.stats.panel_exp_frame(), menu.stats.play_time()), (1, "1:02".to_string()));
        // Cancel: static SE 3, close.
        sounds.clear();
        assert_eq!(menu.key(CampKey::Cancel, false, &mut sounds), vec![CampAction::Close]);
        assert_eq!(sounds, vec![UiSound::Play(3)]);
        assert_eq!(top_entries(true, true).len(), 9);
    }

    fn ability(id: i32, name: &str, category: u8, ap: i32, owned: u8, equipped: u8) -> AbilityIn {
        // (`equipped` here: how many copies, the lowest ones.)
        AbilityIn { id, name: name.into(), category, ap, priority: id, owned, equipped: (1u8 << equipped) - 1, help: String::new() }
    }

    #[test]
    fn a_copy_under_another_copy_of_the_same_ability_is_its_own_row() {
        // Fixes pass 1 (user report: "equipping an ability under an ability with the same name
        // equips the ability above it"): three Air Slides, none equipped; the THIRD row is
        // decided on. The action names copy 2, and after the host has written exactly that
        // copy (its mask is the screen's new data) the third row is the equipped one.
        let mut sounds = Vec::new();
        let mut menu = CampMenu::open(false, false, stats_99(), &mut sounds);
        menu.set_abilities(&[AbilityIn { equipped: 0, ..ability(23, "Air Slide", 1, 2, 3, 0) }, AbilityIn { equipped: 0b01, ..ability(28, "Combo Plus", 2, 4, 2, 0) }], 50);
        menu.screen = CampScreen::Abilities;
        menu.abilities.set_cursor(2);
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![CampAction::SetAbility { id: 23, copy: 2, on: true }]);
        menu.set_abilities(&[AbilityIn { equipped: 0b100, ..ability(23, "Air Slide", 1, 2, 3, 0) }, AbilityIn { equipped: 0b01, ..ability(28, "Combo Plus", 2, 4, 2, 0) }], 50);
        let shown: Vec<bool> = menu.abilities.rows.iter().map(|r| r.equipped).collect();
        assert_eq!(shown, [false, false, true, true, false], "rows: Air Slide x3, Combo Plus x2");
        // The lower Combo Plus on, then the upper one off: each row is its own copy.
        menu.abilities.set_cursor(4);
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![CampAction::SetAbility { id: 28, copy: 1, on: true }]);
        menu.abilities.set_cursor(3);
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![CampAction::SetAbility { id: 28, copy: 0, on: false }]);
        assert_eq!(menu.abilities.rows.iter().map(|r| r.equipped).collect::<Vec<_>>(), [false, false, true, false, true]);
        assert_eq!(menu.abilities.used_ap, 2 + 4);
    }

    #[test]
    fn spec_11b_test_12_abilities() {
        // 180 / 180 with Counter Slash installed and the other reprisals not.
        let list = [
            ability(1, "Aerial Recovery", 0, 2, 1, 1),
            ability(2, "Payback Strike", 0, 5, 1, 1),
            ability(3, "Block", 0, 2, 1, 1),
            ability(4, "Counter Slash", 0, 4, 1, 1),
            ability(5, "Counter Impact", 0, 4, 1, 0),
            ability(6, "Counter Kick", 0, 4, 1, 0),
            ability(23, "Air Slide", 1, 2, 3, 3),
            ability(28, "Combo Plus", 2, 4, 2, 2),
            ability(200, "Filler", 2, 153, 1, 1),
        ];
        let mut sounds = Vec::new();
        let mut menu = CampMenu::open(false, false, stats_99(), &mut sounds);
        menu.key(CampKey::Down, false, &mut sounds);
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![]);
        assert_eq!(menu.screen, CampScreen::Character(TopEntry::Abilities));
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![CampAction::Opened(CampScreen::Abilities)]);
        menu.set_abilities(&list, 180);
        let a = &menu.abilities;
        let names: Vec<&str> = a.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(&names[..6], ["Aerial Recovery", "Payback Strike", "Block", "Counter Slash", "Counter Impact", "Counter Kick"]);
        // Stackable abilities are adjacent rows; categories carry their title on the first row.
        assert_eq!(&names[6..11], ["Air Slide", "Air Slide", "Air Slide", "Combo Plus", "Combo Plus"]);
        assert_eq!(a.rows.iter().filter(|r| r.first_of_category).count(), 3);
        assert_eq!((a.used_ap, a.max_ap, a.gauge_frame()), (180, 180, 201));
        // Cursor on Counter Impact (not installed): 184, `lack`; decide: static SE 4.
        menu.abilities.set_cursor(4);
        assert_eq!(menu.abilities.preview(4), ApPreview { number: 184, label: "lack", preview_frame: Some(200) });
        sounds.clear();
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![]);
        assert_eq!(sounds, vec![UiSound::Play(4)]);
        // Uninstall Counter Slash: static SE 1, 176 / 180, gauge frame 197.
        menu.abilities.set_cursor(3);
        sounds.clear();
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![CampAction::SetAbility { id: 4, copy: 0, on: false }]);
        assert_eq!(sounds, vec![UiSound::Play(1)]);
        assert_eq!((menu.abilities.used_ap, menu.abilities.gauge_frame()), (176, 197));
        // Counter Impact now previews 180 with `equippable`; install it: 180 / 180.
        assert_eq!(menu.abilities.preview(4), ApPreview { number: 180, label: "equippable", preview_frame: Some(201) });
        menu.abilities.set_cursor(4);
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![CampAction::SetAbility { id: 5, copy: 0, on: true }]);
        assert_eq!(menu.abilities.used_ap, 180);
        // Counter Kick: refused (one reprisal), also with AP to spare.
        menu.abilities.max_ap = 200;
        menu.abilities.set_cursor(5);
        sounds.clear();
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![]);
        assert_eq!(sounds, vec![UiSound::Play(4)]);
        // An equipped row can always be removed.
        menu.abilities.set_cursor(0);
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![CampAction::SetAbility { id: 1, copy: 0, on: false }]);
        // Cancel: (DON-UI) back one level, the user's rule for all menus: the character list, then the top screen
        menu.key(CampKey::Cancel, false, &mut sounds);
        assert_eq!(menu.screen, CampScreen::Character(TopEntry::Abilities));
        menu.key(CampKey::Cancel, false, &mut sounds);
        assert_eq!(menu.screen, CampScreen::Top);
    }

    #[test]
    fn spec_11b_test_16_stats() {
        let s = stats_99();
        let fields = s.fields();
        let get = |label: &str| fields.iter().find(|f| f.0 == label).map(|f| f.1.clone()).unwrap();
        assert_eq!((get("Level"), get("Total EXP"), get("Next Level")), ("99".into(), "1527640".into(), "0".into()));
        assert_eq!(s.stats_exp_frame(), 101);
        assert_eq!((get("HP"), get("MP"), get("Focus"), get("AP")), ("265 / 265".into(), "169 / 169".into(), "100 / 100".into(), "180".into()));
        assert_eq!((get("Strength"), get("Magic"), get("Defense"), get("Fire Resistance")), ("66".into(), "67".into(), "59".into(), "0%".into()));
        // Without the focus flag: "—" and "? ? ?".
        let locked = StatsIn { focus_unlocked: false, ..stats_99() };
        assert!(locked.fields().iter().any(|f| f.0 == "? ? ?" && f.1 == "— / —"));
        // (IMPL_ER21) THE USER'S LEVEL 5 (the game's table: level 5 at 1120, level 6 at 1680 total EXP): the gauges show the
        // way from this level's threshold to the next one's. 1136 (his session's last value) is 2.9 %: frame 5 of 151.
        let at = |exp: i32| StatsIn { level: 5, exp, next_level_exp: 1680, level_exp: 1120, ..stats_99() };
        assert_eq!([1120, 1122, 1136, 1400, 1674, 1679].map(|e| at(e).panel_exp_frame()), [1, 2, 5, 76, 149, 150]);
        assert_eq!([1120, 1136, 1400, 1674].map(|e| at(e).stats_exp_frame()), [1, 4, 51, 100]);
        assert_eq!((at(1136).next_level(), at(1400).next_level()), (544, 280));
        // Level 1 with 120 of 80 .. 280 EXP: next level 160, gauges part way.
        let low = StatsIn { level: 2, exp: 120, next_level_exp: 280, level_exp: 80, ..stats_99() };
        assert_eq!((low.next_level(), low.stats_exp_frame(), low.panel_exp_frame()), (160, 21, 31));
    }

    #[test]
    fn spec_11b_test_17_config_screen() {
        let rows = vec![
            OptionRow { label: "Camera Work".into(), choices: vec!["Auto".into(), "Manual".into()], range: (0, 1), value: 0, stored: true },
            OptionRow { label: "Vertical Camera Sensitivity (Gamepad)".into(), choices: vec![], range: (1, 100), value: 50, stored: true },
            OptionRow { label: "Graphics".into(), choices: vec!["Default Mode".into(), "Stable Mode".into()], range: (0, 1), value: 0, stored: false },
        ];
        let mut sounds = Vec::new();
        let mut menu = CampMenu::open(false, false, stats_99(), &mut sounds);
        menu.top_cursor = 6;
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![CampAction::Opened(CampScreen::Config)]);
        menu.set_options(rows, true);
        // Leaving without a change applies nothing.
        assert_eq!(menu.key(CampKey::Cancel, false, &mut sounds), vec![]);
        menu.screen = CampScreen::Config;
        menu.key(CampKey::Right, false, &mut sounds);
        assert_eq!(menu.options[0].value_text(), "Manual");
        menu.key(CampKey::Right, false, &mut sounds);
        assert_eq!(menu.options[0].value, 1, "no wrap");
        menu.key(CampKey::Down, false, &mut sounds);
        menu.key(CampKey::Left, false, &mut sounds);
        assert_eq!(menu.options[1].value_text(), "49");
        menu.key(CampKey::Down, false, &mut sounds);
        menu.key(CampKey::Right, false, &mut sounds);
        assert_eq!(menu.options[2].value, 0, "a row that is not stored does not change");
        // Triangle asks first (default No); Yes restores.
        assert_eq!(menu.key(CampKey::Triangle, false, &mut sounds), vec![]);
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![]);
        menu.key(CampKey::Triangle, false, &mut sounds);
        menu.key(CampKey::Left, false, &mut sounds);
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![CampAction::RestoreConfigDefaults]);
        assert_eq!(menu.key(CampKey::Cancel, false, &mut sounds), vec![CampAction::ApplyConfig(vec![1, 49, 0])]);
        assert_eq!(menu.screen, CampScreen::Top);
    }

    /// (party) The character list, L2 / R2 member switching and the friends' Customize layer.
    #[test]
    fn party_character_select_member_switch_and_friend_settings() {
        let mut sounds = Vec::new();
        let mut menu = CampMenu::open(false, false, stats_99(), &mut sounds);
        let friend = |name: &str, level: i32| StatsIn { name: name.into(), level, hp: 200, hp_max: 200, ..StatsIn::default() };
        menu.set_party(vec![stats_99(), friend("Donald", 99), friend("Goofy", 99)]);
        // Equipment -> the character list -> Goofy (down twice).
        menu.key(CampKey::Decide, false, &mut sounds);
        assert_eq!(menu.screen, CampScreen::Character(TopEntry::Equipment));
        menu.key(CampKey::Down, false, &mut sounds);
        menu.key(CampKey::Down, false, &mut sounds);
        let actions = menu.key(CampKey::Decide, false, &mut sounds);
        assert_eq!((menu.screen, menu.member, menu.stats.name.as_str()), (CampScreen::Equipment, 2, "Goofy"));
        assert_eq!(actions, vec![CampAction::Opened(CampScreen::Equipment)]);
        // R2 wraps to Sora, L2 back to Goofy then Donald; static SE 2 each time.
        sounds.clear();
        assert_eq!(menu.key(CampKey::R2, false, &mut sounds), vec![CampAction::Opened(CampScreen::Equipment)]);
        assert_eq!((menu.member, menu.stats.name.as_str()), (0, "Sora"));
        menu.key(CampKey::L2, false, &mut sounds);
        menu.key(CampKey::L2, false, &mut sounds);
        assert_eq!(menu.stats.name, "Donald");
        assert_eq!(sounds, vec![UiSound::Play(SE_MENU_OPEN); 3]);
        // Sora alone: L2 / R2 do nothing (no sound).
        let mut solo = CampMenu::open(false, false, stats_99(), &mut sounds);
        solo.screen = CampScreen::Stats;
        sounds.clear();
        assert!(solo.key(CampKey::R2, false, &mut sounds).is_empty() && sounds.is_empty());
        // Customize: the friend entries follow Shortcut Set C; Combat Style -> Hands Off; On Self off; restore.
        menu.key(CampKey::Cancel, false, &mut sounds);
        menu.screen = CampScreen::Top;
        menu.customize.set(crate::camp_custom::CustomizeIn { friends: vec![crate::camp_custom::FriendIn { member: 1, name: "Donald".into(), styles: [0, 0, 1], targets: 7 }], ..Default::default() });
        menu.screen = CampScreen::Customize;
        for _ in 0..4 {
            menu.key(CampKey::Down, false, &mut sounds);
        }
        menu.key(CampKey::Decide, false, &mut sounds);
        assert_eq!(menu.customize.layer, crate::camp_custom::CustomLayer::Friend);
        menu.key(CampKey::Decide, false, &mut sounds);
        menu.key(CampKey::Down, false, &mut sounds);
        menu.key(CampKey::Down, false, &mut sounds);
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![CampAction::SetFriendAi { member: 1, styles: [2, 0, 1], targets: 7 }]);
        for _ in 0..4 {
            menu.key(CampKey::Down, false, &mut sounds);
        }
        assert_eq!(menu.customize.friend_row, 4);
        assert_eq!(menu.key(CampKey::Left, false, &mut sounds), vec![CampAction::SetFriendAi { member: 1, styles: [2, 0, 1], targets: 5 }]);
        sounds.clear();
        assert!(menu.key(CampKey::Left, false, &mut sounds).is_empty());
        assert_eq!(sounds, vec![UiSound::Play(SE_MENU_REFUSED)]);
        menu.key(CampKey::Triangle, false, &mut sounds);
        assert!(menu.dialog.is_some());
        menu.key(CampKey::Left, false, &mut sounds);
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![CampAction::SetFriendAi { member: 1, styles: [0, 0, 1], targets: 7 }]);
    }
}

/// (party P) The save point's Party Formation step: keys, actions, the answer's sounds and texts.
#[cfg(test)]
mod party_formation_tests {
    use super::*;

    fn rows(n: u32) -> FormationRows {
        FormationRows { rows: vec!["Sora    Leader".into(), "Donald  Slot 2".into(), "Goofy   -".into()], help: "help".into(), stamp: n, ..FormationRows::default() }
    }

    #[test]
    fn a_toggles_y_leads_b_closes_and_the_answer_sounds() {
        let mut sounds = Vec::new();
        let mut menu = CampMenu::open_formation(vec![StatsIn { name: "Sora".into(), ..StatsIn::default() }], rows(0), &mut sounds);
        assert_eq!(menu.screen, CampScreen::Character(TopEntry::PartyFormation));
        assert_eq!(sounds, vec![UiSound::Play(SE_MENU_OPEN)]);
        // the cursor walks the three roster rows, wrapping
        sounds.clear();
        menu.key(CampKey::Down, false, &mut sounds);
        menu.key(CampKey::Down, false, &mut sounds);
        assert_eq!(menu.chara_cursor, 2);
        assert_eq!(sounds, vec![UiSound::Play(SE_MENU_CURSOR); 2]);
        // (party PF) the icon row: right wraps to the first, left back to the last
        menu.key(CampKey::Right, false, &mut sounds);
        assert_eq!(menu.chara_cursor, 0);
        menu.key(CampKey::Left, false, &mut sounds);
        assert_eq!(menu.chara_cursor, 2);
        assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![CampAction::Formation { row: 2, lead: false }]);
        assert_eq!(menu.key(CampKey::Triangle, false, &mut sounds), vec![CampAction::Formation { row: 2, lead: true }]);
        // the host's answers: decide SE when accepted, the error buzz when refused; the texts count up
        sounds.clear();
        menu.formation_answer(true, Vec::new(), rows(0), &mut sounds);
        menu.formation_answer(false, Vec::new(), rows(0), &mut sounds);
        assert_eq!(sounds, vec![UiSound::Play(SE_MENU_DECIDE), UiSound::Play(SE_MENU_REFUSED)]);
        assert_eq!(menu.formation.as_ref().unwrap().stamp, 2);
        sounds.clear();
        assert_eq!(menu.key(CampKey::Cancel, false, &mut sounds), vec![CampAction::Close]);
        assert_eq!(sounds, vec![UiSound::Play(SE_MENU_CLOSE)]);
    }
}
