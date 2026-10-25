//! The camp menu's Customize screen (docs/sora-ui-spec.md 10.6) and Game Help lists (10.9),
//! logic only. Customize edits what the command menu and the shortcut panel show: the six
//! magic slots (tier choice, reorder) and the three shortcut sets of four buttons. The host
//! applies the [`CampAction`]s to the command system's storage, so the changes show as soon as
//! the game resumes.
//!
//! NOT IN THE PORT: Links (no link is learned: the entry is hidden, as the game hides entries
//! without content) and the friends' layer (Sora alone).

use crate::camp::{CampAction, CampKey};
use crate::list::ListView;
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_OPEN, SE_MENU_REFUSED};
use crate::UiSound;

/// Command groups of the shortcut list's tabs.
pub const GROUP_MAGIC: u8 = 1;
pub const GROUP_LINK: u8 = 2;
pub const GROUP_ITEM: u8 = 3;

/// One assignable command (a learned magic kind, a link, an item command).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CommandEntry {
    /// `ETresCommandKind` value.
    pub kind: u8,
    pub name: String,
    pub group: u8,
    /// An item whose pouch slot has auto-reload.
    pub auto_reload: bool,
    pub help: String,
}

/// What the host supplies (again after every change).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CustomizeIn {
    /// `magicSlot[0 .. 5]` in command-menu order (0 = empty).
    pub magic_slots: [u8; 6],
    /// `shortcut[3][4]`: right, top, left, bottom face button.
    pub shortcuts: [[u8; 4]; 3],
    /// Every learned magic kind (asset order: Fire, Fira, Firaga, Blizzard ..), every item
    /// command whose item was ever obtained.
    pub entries: Vec<CommandEntry>,
    /// Display names of command kinds that are not in `entries`.
    pub names: Vec<(u8, String)>,
    /// (party) One entry per friend in the party, in party order (the top layer's Friends group).
    pub friends: Vec<FriendIn>,
    /// (party X2) A friend leader's page: the Magic entry's / page's / tab's label instead of KH3's "Magic" (Goofy's
    /// "Abilities"); None = the game's text.
    pub magic_label: Option<String>,
    /// (party X2) The tier buttons of kinds outside the magic range (Goofy's abilities: kh-core command/leader.rs
    /// `goofy::tiers`): each group lists its three kinds.
    pub tier_groups: Vec<[u8; 3]>,
    /// (XSI, 2026-10-16) The Shotlock choice of this character (XS 2.8: Spectral Rays as a second shotlock): shown as
    /// the top layer's entry under Magic (KH3's Links button, which the port never fills); None = no choice to make.
    pub shotlock: Option<ShotlockIn>,
}

/// (XSI) The Shotlock entry: the label ("Shotlocks"), the choices (shotlock kind, name) and the one in force.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ShotlockIn {
    pub label: String,
    pub choices: Vec<(u8, String)>,
    pub current: u8,
    pub help: String,
}

impl ShotlockIn {
    /// "Shotlocks: Spectral Rays".
    pub fn text(&self) -> String {
        let name = self.choices.iter().find(|c| c.0 == self.current).map_or("", |c| c.1.as_str());
        format!("{}: {name}", self.label)
    }
}

/// (party) A friend's AI settings as the Customize screen edits them (UI spec 10.9 "Friends"; the save record
/// bytes +0x158 Combat Style, +0x159 Abilities, +0x15A Recovery, +0x15B the three target bits [C]).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FriendIn {
    /// The party slot (1, 2 ...): what the action names.
    pub member: usize,
    pub name: String,
    /// Combat Style, Abilities, Recovery: the enum values.
    pub styles: [u8; 3],
    /// Bit 0 On Sora, 1 On Self, 2 On Others.
    pub targets: u8,
}

/// (party) How many choices each style row has (`ETresFNpcAIStyle_*` without MAX) and their label keys
/// (`kh3_camp.customize.bstyle_0101 ..`, in enum order [D]).
pub const STYLE_CHOICES: [usize; 3] = [4, 3, 3];
pub const STYLE_LABELS: [&[&str]; 3] = [
    &["kh3_camp.customize.bstyle_0101", "kh3_camp.customize.bstyle_0102", "kh3_camp.customize.bstyle_0103", "kh3_camp.customize.bstyle_0104"],
    &["kh3_camp.customize.bstyle_0105", "kh3_camp.customize.bstyle_0106", "kh3_camp.customize.bstyle_0107"],
    &["kh3_camp.customize.bstyle_0108", "kh3_camp.customize.bstyle_0109", "kh3_camp.customize.bstyle_0110"],
];
/// Help lines of the choices (`detail_0105 ..`) and of the rows (`detail_0101 ..`, targets ON / OFF `0115 ..`).
pub const STYLE_HELP: [&[&str]; 3] = [
    &["kh3_camp.customize.detail_0105", "kh3_camp.customize.detail_0106", "kh3_camp.customize.detail_0107", "kh3_camp.customize.detail_0108"],
    &["kh3_camp.customize.detail_0109", "kh3_camp.customize.detail_0110", "kh3_camp.customize.detail_0111"],
    &["kh3_camp.customize.detail_0112", "kh3_camp.customize.detail_0113", "kh3_camp.customize.detail_0114"],
];
pub const ROW_HELP: [&str; 3] = ["kh3_camp.customize.detail_0101", "kh3_camp.customize.detail_0102", "kh3_camp.customize.detail_0103"];
/// (target, on) -> help: On Sora 0115 / 0116, On Self 0117 / 0118, On Others 0119 / 0120.
pub fn target_help(target: usize, on: bool) -> &'static str {
    ["kh3_camp.customize.detail_0115", "kh3_camp.customize.detail_0116", "kh3_camp.customize.detail_0117", "kh3_camp.customize.detail_0118", "kh3_camp.customize.detail_0119", "kh3_camp.customize.detail_0120"][target.min(2) * 2 + (!on) as usize]
}
/// The friend layer's rows: three style buttons, then the three Recovery targets.
pub const FRIEND_ROWS: usize = 6;

impl CustomizeIn {
    pub fn name(&self, kind: u8) -> String {
        if kind == 0 {
            return String::new();
        }
        self.entries.iter().find(|e| e.kind == kind).map(|e| e.name.clone()).or_else(|| self.names.iter().find(|n| n.0 == kind).map(|n| n.1.clone())).unwrap_or_default()
    }

    pub fn entry(&self, kind: u8) -> Option<&CommandEntry> {
        self.entries.iter().find(|e| e.kind == kind)
    }

    /// (party X2) The three tiers of a slot's kind: a host group first, else the magic rule (`tiers_of`).
    pub fn tiers_of(&self, kind: u8) -> Option<[u8; 3]> {
        self.tier_groups.iter().find(|g| g.contains(&kind)).copied().or_else(|| tiers_of(kind))
    }
}

/// The three tiers of a magic kind's element (`ETresCommandKind`: 29 Fire .. 31 Firaga, then
/// Blizzard 33, Thunder 37, Water 41, Aero 45, Cure 49; the fourth of each group is "-gan").
pub fn tiers_of(kind: u8) -> Option<[u8; 3]> {
    if !(29..=51).contains(&kind) {
        return None;
    }
    let base = 29 + (kind - 29) / 4 * 4;
    Some([base, base + 1, base + 2])
}

/// The top layer's entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopItem {
    Magic,
    /// (XSI) The Shotlock choice (Decide steps to the next choice).
    Shotlock,
    Shortcut(usize),
    /// (party) The friend at this index of `CustomizeIn::friends` (entry text `custom_0006 ..` = "{0}", the name).
    Friend(usize),
}

pub const TOP_ITEMS: [TopItem; 4] = [TopItem::Magic, TopItem::Shortcut(0), TopItem::Shortcut(1), TopItem::Shortcut(2)];

/// The tabs of the shortcut list (`kh3_camp.customize.tab_*`): only tabs that have entries.
/// (IMPL_ER23B) The tabs' icons (cells of `ItemListLibrary_bmpIcon`): the native table at 0x146A80148 indexed by the
/// tab's type (0x1415EA9C0) holds 0, 8, 9, 1 for types 1 .. 4 [decompiled C + the exe's data; that the four types
/// are these four tabs in this order is read off the icons' art: all, a wand, a link, a bottle].
pub const TAB_ICONS: [i32; 4] = [0, 8, 9, 1];
pub const TAB_KEYS: [(&str, Option<u8>); 4] = [("kh3_camp.customize.tab_0001", None), ("kh3_camp.customize.tab_0002", Some(GROUP_MAGIC)), ("kh3_camp.customize.tab_0003", Some(GROUP_LINK)), ("kh3_camp.customize.tab_0004", Some(GROUP_ITEM))];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CustomLayer {
    #[default]
    Top,
    /// The six magic slots; `picked` = the slot picked up for reordering.
    Magic,
    /// The tier buttons of the focused magic slot.
    Tier,
    /// The four buttons of a shortcut set.
    Set,
    /// The assignable list of a shortcut button.
    List,
    /// (party) A friend's settings (`battleStyle2ndLayer`): Combat Style, Abilities, Recovery, the three targets.
    Friend,
    /// (party) The choices of the focused style row (`battleStyle3rdLayer`).
    FriendList,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct CustomizeScreen {
    pub data: CustomizeIn,
    pub layer: CustomLayer,
    pub top_cursor: usize,
    pub magic_cursor: usize,
    pub picked: Option<usize>,
    pub tier_cursor: usize,
    pub page: usize,
    pub set_cursor: usize,
    pub tab: usize,
    pub tab_flash: Option<bool>,
    pub list: ListView,
    /// (party) The friend whose layer is up (index into `data.friends`), its focused row, the focused choice.
    pub friend: usize,
    pub friend_row: usize,
    pub choice_cursor: usize,
    /// (party) Triangle on the friend layer: the menu asks "restore the defaults?" (CampMenu opens the dialog).
    pub restore_request: bool,
}

impl CustomizeScreen {
    pub fn new() -> CustomizeScreen {
        CustomizeScreen { list: ListView::new("customize.shortcut3rdLayer.shortcutChangeListAnm.shortcutChangeList", "CoBtnSelectShortCmd"), ..CustomizeScreen::default() }
    }

    pub fn set(&mut self, data: CustomizeIn) {
        self.data = data;
        if self.layer == CustomLayer::List {
            let count = self.list_rows().len();
            self.list.set_count(count);
            self.list.invalidate();
        }
    }

    /// The top layer's entries: Magic, Shortcut Set A / B / C, then one per friend (Links is hidden: no link learned).
    pub fn top_items(&self) -> Vec<TopItem> {
        let mut items = TOP_ITEMS.to_vec();
        if self.data.shotlock.as_ref().is_some_and(|s| !s.choices.is_empty()) {
            items.insert(1, TopItem::Shotlock); // (XSI)
        }
        items.extend((0..self.data.friends.len().min(4)).map(TopItem::Friend));
        items
    }

    /// (party) The friend whose layer is up.
    pub fn friend_in(&self) -> Option<&FriendIn> {
        self.data.friends.get(self.friend)
    }

    /// The magic slots that hold a command (the buttons shown).
    pub fn magic_count(&self) -> usize {
        self.data.magic_slots.iter().filter(|k| **k != 0).count()
    }

    /// The tabs that have entries, as indices into [`TAB_KEYS`].
    pub fn tabs(&self) -> Vec<usize> {
        (0..TAB_KEYS.len()).filter(|t| TAB_KEYS[*t].1.is_none_or(|g| self.data.entries.iter().any(|e| e.group == g))).collect()
    }

    pub fn list_rows(&self) -> Vec<&CommandEntry> {
        let tabs = self.tabs();
        let filter = tabs.get(self.tab).and_then(|t| TAB_KEYS[*t].1);
        self.data.entries.iter().filter(|e| filter.is_none_or(|g| e.group == g)).collect()
    }

    /// The tier buttons of the focused magic slot: (kind, learned).
    pub fn tiers(&self) -> Vec<(u8, bool)> {
        let kind = self.data.magic_slots.get(self.magic_cursor).copied().unwrap_or(0);
        self.data.tiers_of(kind).map_or(Vec::new(), |t| t.iter().map(|k| (*k, self.data.entry(*k).is_some())).collect()) // (party X2) host groups
    }

    fn step(cursor: &mut usize, len: usize, down: bool, repeat: bool, sounds: &mut Vec<UiSound>) -> bool {
        match crate::camp::move_wrapping(*cursor, len, down, repeat) {
            Some(to) => {
                *cursor = to;
                sounds.push(UiSound::Play(SE_MENU_CURSOR));
                true
            }
            None => false,
        }
    }

    /// One event; true = leave the screen.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>, actions: &mut Vec<CampAction>) -> bool {
        let vertical = matches!(key, CampKey::Up | CampKey::Down);
        let down = key == CampKey::Down;
        match self.layer {
            CustomLayer::Top => match key {
                _ if vertical => {
                    let n = self.top_items().len();
                    Self::step(&mut self.top_cursor, n, down, repeat, sounds);
                }
                CampKey::Decide => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    let items = self.top_items();
                    match items[self.top_cursor.min(items.len() - 1)] {
                        // (XSI) the next shotlock of the choices, in force at once
                        TopItem::Shotlock => {
                            if let Some(s) = self.data.shotlock.as_mut() {
                                let at = s.choices.iter().position(|c| c.0 == s.current).map_or(0, |i| (i + 1) % s.choices.len().max(1));
                                if let Some(&(kind, _)) = s.choices.get(at) {
                                    s.current = kind;
                                    actions.push(CampAction::SetShotlock(kind));
                                }
                            }
                        }
                        TopItem::Friend(i) => {
                            self.layer = CustomLayer::Friend;
                            self.friend = i;
                            self.friend_row = 0;
                        }
                        TopItem::Magic => {
                            if self.magic_count() == 0 {
                                sounds.push(UiSound::Play(SE_MENU_REFUSED));
                            } else {
                                self.layer = CustomLayer::Magic;
                                self.magic_cursor = self.magic_cursor.min(self.magic_count() - 1);
                                self.picked = None;
                            }
                        }
                        TopItem::Shortcut(page) => {
                            self.layer = CustomLayer::Set;
                            self.page = page;
                            self.set_cursor = 0;
                        }
                    }
                }
                CampKey::Cancel => return true,
                _ => {}
            },
            CustomLayer::Magic => match key {
                _ if vertical => {
                    let (before, count) = (self.magic_cursor, self.magic_count());
                    if Self::step(&mut self.magic_cursor, count, down, repeat, sounds) {
                        // While a slot is picked up, moving the focus swaps the two slots.
                        if self.picked.is_some() {
                            actions.push(CampAction::SwapMagicSlots(before, self.magic_cursor));
                            self.data.magic_slots.swap(before, self.magic_cursor);
                            self.picked = Some(self.magic_cursor);
                        }
                    }
                }
                CampKey::Triangle => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    self.picked = if self.picked.is_some() { None } else { Some(self.magic_cursor) };
                }
                CampKey::Decide if self.picked.is_none() => {
                    let kind = self.data.magic_slots[self.magic_cursor];
                    if let Some(tiers) = self.data.tiers_of(kind) { // (party X2) host groups
                        sounds.push(UiSound::Play(SE_MENU_DECIDE));
                        self.layer = CustomLayer::Tier;
                        self.tier_cursor = tiers.iter().position(|k| *k == kind).unwrap_or(0);
                    }
                }
                CampKey::Cancel => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    if self.picked.take().is_none() {
                        self.layer = CustomLayer::Top;
                    }
                }
                _ => {}
            },
            CustomLayer::Tier => match key {
                _ if vertical => drop(Self::step(&mut self.tier_cursor, 3, down, repeat, sounds)),
                CampKey::Decide => match self.tiers().get(self.tier_cursor).copied() {
                    Some((kind, true)) => {
                        sounds.push(UiSound::Play(SE_MENU_DECIDE));
                        if self.data.magic_slots[self.magic_cursor] != kind {
                            self.data.magic_slots[self.magic_cursor] = kind;
                            actions.push(CampAction::SetMagicSlot { slot: self.magic_cursor, kind });
                        }
                        self.layer = CustomLayer::Magic;
                    }
                    _ => sounds.push(UiSound::Play(SE_MENU_REFUSED)),
                },
                CampKey::Cancel => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.layer = CustomLayer::Magic;
                }
                _ => {}
            },
            CustomLayer::Set => match key {
                _ if vertical => drop(Self::step(&mut self.set_cursor, 4, down, repeat, sounds)),
                CampKey::Decide => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    self.layer = CustomLayer::List;
                    self.tab = 0;
                    self.tab_flash = None;
                    let count = self.list_rows().len();
                    self.list.focus = 0;
                    self.list.scroll = 0;
                    self.list.set_count(count);
                    self.list.invalidate();
                }
                // "Remove": the slot is cleared (kind 0).
                CampKey::Square => {
                    if self.data.shortcuts[self.page][self.set_cursor] != 0 {
                        sounds.push(UiSound::Play(SE_MENU_CLOSE));
                        self.data.shortcuts[self.page][self.set_cursor] = 0;
                        actions.push(CampAction::SetShortcut { page: self.page, slot: self.set_cursor, kind: 0 });
                    }
                }
                CampKey::Cancel => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.layer = CustomLayer::Top;
                }
                _ => {}
            },
            CustomLayer::List => match key {
                _ if vertical => {
                    if self.list.move_anchor(down, repeat) {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                CampKey::L1 | CampKey::R1 => {
                    let moved = if key == CampKey::R1 { self.list.next_page(repeat) } else { self.list.prev_page(repeat) };
                    if moved {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                CampKey::L2 | CampKey::R2 => {
                    let n = self.tabs().len();
                    if n < 2 {
                        return false;
                    }
                    let at_end = if key == CampKey::R2 { self.tab + 1 >= n } else { self.tab == 0 };
                    if at_end && repeat {
                        return false;
                    }
                    self.tab = if key == CampKey::R2 { (self.tab + 1) % n } else { (self.tab + n - 1) % n };
                    self.tab_flash = Some(key == CampKey::R2);
                    sounds.push(UiSound::Play(SE_MENU_OPEN));
                    let count = self.list_rows().len();
                    self.list.focus = 0;
                    self.list.scroll = 0;
                    self.list.set_count(count);
                    self.list.invalidate();
                }
                CampKey::Decide => match usize::try_from(self.list.focus).ok().and_then(|i| self.list_rows().get(i).map(|e| e.kind)) {
                    // The same command may sit in several slots and sets.
                    Some(kind) => {
                        sounds.push(UiSound::Play(SE_MENU_DECIDE));
                        self.data.shortcuts[self.page][self.set_cursor] = kind;
                        actions.push(CampAction::SetShortcut { page: self.page, slot: self.set_cursor, kind });
                        self.layer = CustomLayer::Set;
                    }
                    None => sounds.push(UiSound::Play(SE_MENU_REFUSED)),
                },
                CampKey::Cancel => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.layer = CustomLayer::Set;
                }
                _ => {}
            },
            // (party) A FRIEND'S SETTINGS (input 0x1415E1FA0) [C]: up / down walk the six rows; decide on a style
            // row opens its choices with the cursor on the value held; left / right on a target row: left = OFF,
            // right = ON (direction 2 writes 0, direction 3 writes 1 through 0x1415EA860), static SE 0 when it
            // changed, else SE 4; Triangle asks to restore the defaults (Yes / No, cursor No: 0x1415E16B0).
            CustomLayer::Friend => match key {
                _ if vertical => drop(Self::step(&mut self.friend_row, FRIEND_ROWS, down, repeat, sounds)),
                CampKey::Decide if self.friend_row < 3 => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    let held = self.friend_in().map_or(0, |f| f.styles[self.friend_row]) as usize;
                    self.choice_cursor = held.min(STYLE_CHOICES[self.friend_row] - 1);
                    self.layer = CustomLayer::FriendList;
                }
                CampKey::Left | CampKey::Right if self.friend_row >= 3 => {
                    let bit = 1u8 << (self.friend_row - 3);
                    let on = key == CampKey::Right;
                    let friend = self.friend;
                    match self.data.friends.get_mut(friend) {
                        Some(f) if (f.targets & bit != 0) != on => {
                            f.targets = if on { f.targets | bit } else { f.targets & !bit };
                            sounds.push(UiSound::Play(SE_MENU_CURSOR));
                            actions.push(CampAction::SetFriendAi { member: f.member, styles: f.styles, targets: f.targets });
                        }
                        _ => sounds.push(UiSound::Play(SE_MENU_REFUSED)),
                    }
                }
                CampKey::Triangle => self.restore_request = true,
                CampKey::Cancel => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.layer = CustomLayer::Top;
                }
                _ => {}
            },
            CustomLayer::FriendList => match key {
                _ if vertical => {
                    let n = STYLE_CHOICES[self.friend_row.min(2)];
                    Self::step(&mut self.choice_cursor, n, down, repeat, sounds);
                }
                CampKey::Decide => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    let (row, value, friend) = (self.friend_row.min(2), self.choice_cursor as u8, self.friend);
                    if let Some(f) = self.data.friends.get_mut(friend) {
                        if f.styles[row] != value {
                            f.styles[row] = value;
                            actions.push(CampAction::SetFriendAi { member: f.member, styles: f.styles, targets: f.targets });
                        }
                    }
                    self.layer = CustomLayer::Friend;
                }
                CampKey::Cancel => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.layer = CustomLayer::Friend;
                }
                _ => {}
            },
        }
        false
    }

    /// (party) The Yes of "restore the defaults?" on the friend layer: the game's `0x140E94550` (Combat Style 0,
    /// Abilities 0, Recovery 1, all three targets on).
    pub fn restore_friend_defaults(&mut self, actions: &mut Vec<CampAction>) {
        let friend = self.friend;
        if let Some(f) = self.data.friends.get_mut(friend) {
            f.styles = [0, 0, 1];
            f.targets |= 7;
            actions.push(CampAction::SetFriendAi { member: f.member, styles: f.styles, targets: f.targets });
        }
    }

    /// The help line (`kh3_camp.customize.detail_*`), or the focused command's own text.
    pub fn help(&self) -> (Option<&'static str>, String) {
        match self.layer {
            CustomLayer::Top => {
                let items = self.top_items();
                // (XSI) the Shotlock entry: its own line
                if items[self.top_cursor.min(items.len() - 1)] == TopItem::Shotlock {
                    return (None, self.data.shotlock.as_ref().map(|s| s.help.clone()).unwrap_or_default());
                }
                (
                    Some(match items[self.top_cursor.min(items.len() - 1)] {
                        TopItem::Magic => "kh3_camp.customize.detail_0001",
                        TopItem::Shotlock => "kh3_camp.customize.detail_0001",
                        TopItem::Shortcut(0) => "kh3_camp.customize.detail_0003",
                        TopItem::Shortcut(1) => "kh3_camp.customize.detail_0004",
                        TopItem::Shortcut(_) => "kh3_camp.customize.detail_0005",
                        TopItem::Friend(_) => "kh3_camp.customize.detail_0006",
                    }),
                    String::new(),
                )
            }
            CustomLayer::Friend => {
                let on = self.friend_in().is_some_and(|f| self.friend_row >= 3 && f.targets & (1 << (self.friend_row - 3)) != 0);
                (Some(if self.friend_row < 3 { ROW_HELP[self.friend_row] } else { target_help(self.friend_row - 3, on) }), String::new())
            }
            CustomLayer::FriendList => (STYLE_HELP[self.friend_row.min(2)].get(self.choice_cursor).copied(), String::new()),
            CustomLayer::Magic => (None, self.data.entry(self.data.magic_slots[self.magic_cursor.min(5)]).map(|e| e.help.clone()).unwrap_or_default()),
            CustomLayer::Tier => (None, self.tiers().get(self.tier_cursor).and_then(|t| self.data.entry(t.0)).map(|e| e.help.clone()).unwrap_or_default()),
            CustomLayer::Set => (None, self.data.entry(self.data.shortcuts[self.page][self.set_cursor.min(3)]).map(|e| e.help.clone()).unwrap_or_default()),
            CustomLayer::List => (None, usize::try_from(self.list.focus).ok().and_then(|i| self.list_rows().get(i).map(|e| e.help.clone())).unwrap_or_default()),
        }
    }

    /// The button guide (`kh3_camp.customize.button_*`).
    pub fn guide_key(&self) -> &'static str {
        match self.layer {
            CustomLayer::Magic => "kh3_camp.customize.button_0001",
            CustomLayer::Set => "kh3_camp.customize.button_0002",
            CustomLayer::Friend => "kh3_camp.customize.button_0003",
            _ => "",
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Game Help (`UTresUIP_CampMenuGameHelp`): two lists, categories and topics; decide on a topic
// shows its pages.

#[derive(Clone, Debug, PartialEq, Default)]
pub struct HelpTopic {
    pub title: String,
    pub pages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct HelpCategory {
    pub name: String,
    pub topics: Vec<HelpTopic>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HelpLayer {
    #[default]
    Categories,
    Topics,
    /// A topic's page is up.
    Page(usize),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct HelpScreen {
    pub categories: Vec<HelpCategory>,
    pub layer: HelpLayer,
    pub cats: ListView,
    pub topics: ListView,
}

impl HelpScreen {
    pub fn new() -> HelpScreen {
        HelpScreen { cats: ListView::new("gameHelp.gameHelpAnm.gameHelpList", "CoGameHelpCategoryListItem"), topics: ListView::new("gameHelp.gameHelpAnm.gameHelpItemList", "CoGameHelpCategoryListItem"), ..HelpScreen::default() }
    }

    pub fn set(&mut self, categories: Vec<HelpCategory>) {
        self.categories = categories;
        self.cats.set_count(self.categories.len());
        self.sync_topics();
    }

    pub fn category(&self) -> Option<&HelpCategory> {
        usize::try_from(self.cats.focus).ok().and_then(|i| self.categories.get(i))
    }

    pub fn topic(&self) -> Option<&HelpTopic> {
        let category = self.category()?;
        usize::try_from(self.topics.focus).ok().and_then(|i| category.topics.get(i))
    }

    fn sync_topics(&mut self) {
        let count = self.category().map_or(0, |c| c.topics.len());
        self.topics.focus = 0;
        self.topics.scroll = 0;
        self.topics.set_count(count);
        self.topics.invalidate();
    }

    /// One event; true = leave the screen.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) -> bool {
        let vertical = matches!(key, CampKey::Up | CampKey::Down);
        let paging = matches!(key, CampKey::L1 | CampKey::R1);
        match self.layer {
            HelpLayer::Categories => match key {
                _ if vertical => {
                    if self.cats.move_anchor(key == CampKey::Down, repeat) {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                        self.sync_topics();
                    }
                }
                CampKey::Decide | CampKey::Right => {
                    if self.topics.count > 0 {
                        sounds.push(UiSound::Play(SE_MENU_DECIDE));
                        self.layer = HelpLayer::Topics;
                    } else if key == CampKey::Decide {
                        sounds.push(UiSound::Play(SE_MENU_REFUSED));
                    }
                }
                CampKey::Cancel => return true,
                _ => {}
            },
            HelpLayer::Topics => match key {
                _ if vertical => {
                    if self.topics.move_anchor(key == CampKey::Down, repeat) {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                _ if paging => {
                    let moved = if key == CampKey::R1 { self.topics.next_page(repeat) } else { self.topics.prev_page(repeat) };
                    if moved {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                CampKey::Decide => {
                    if self.topic().is_some_and(|t| !t.pages.is_empty()) {
                        sounds.push(UiSound::Play(SE_MENU_OPEN));
                        self.layer = HelpLayer::Page(0);
                    } else {
                        sounds.push(UiSound::Play(SE_MENU_REFUSED));
                    }
                }
                CampKey::Cancel | CampKey::Left => {
                    if key == CampKey::Cancel {
                        sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    }
                    self.layer = HelpLayer::Categories;
                }
                _ => {}
            },
            HelpLayer::Page(page) => {
                let pages = self.topic().map_or(0, |t| t.pages.len());
                match key {
                    // Decide: the next page, back to the first after the last
                    // (`kh3_gamehelp.general.button_002 / 003`).
                    CampKey::Decide | CampKey::Right if pages > 1 => {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                        self.layer = HelpLayer::Page(if page + 1 >= pages { 0 } else { page + 1 });
                    }
                    CampKey::Left if page > 0 => {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                        self.layer = HelpLayer::Page(page - 1);
                    }
                    CampKey::Cancel => {
                        sounds.push(UiSound::Play(SE_MENU_CLOSE));
                        self.layer = HelpLayer::Topics;
                    }
                    _ => {}
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: u8, name: &str, group: u8) -> CommandEntry {
        CommandEntry { kind, name: name.into(), group, ..CommandEntry::default() }
    }

    /// Level 99: six elements at tier 3 (18 magic entries in asset order), three item commands.
    fn level_99() -> CustomizeIn {
        let mut entries = Vec::new();
        for (base, names) in [(29, ["Fire", "Fira", "Firaga"]), (33, ["Blizzard", "Blizzara", "Blizzaga"]), (37, ["Thunder", "Thundara", "Thundaga"]), (41, ["Water", "Watera", "Waterga"]), (45, ["Aero", "Aerora", "Aeroga"]), (49, ["Cure", "Cura", "Curaga"])] {
            for (tier, name) in names.into_iter().enumerate() {
                entries.push(entry(base + tier as u8, name, GROUP_MAGIC));
            }
        }
        for (kind, name) in [(58, "Potion"), (59, "Hi-Potion"), (61, "Ether")] {
            entries.push(entry(kind, name, GROUP_ITEM));
        }
        CustomizeIn { magic_slots: [31, 43, 51, 35, 39, 47], shortcuts: [[0; 4]; 3], entries, names: Vec::new(), friends: Vec::new(), ..CustomizeIn::default() }
    }

    #[test]
    fn spec_11b_test_15_customize() {
        let (mut sounds, mut actions) = (Vec::new(), Vec::new());
        let mut screen = CustomizeScreen::new();
        screen.set(level_99());
        let names: Vec<String> = screen.data.magic_slots.iter().map(|k| screen.data.name(*k)).collect();
        assert_eq!(names, ["Firaga", "Waterga", "Curaga", "Blizzaga", "Thundaga", "Aeroga"]);
        // Magic -> slot 0 -> tiers Fire / Fira / Firaga; choose Fire: slot 0 = kind 29.
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions);
        assert_eq!(screen.layer, CustomLayer::Magic);
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions);
        assert_eq!((screen.layer, screen.tiers(), screen.tier_cursor), (CustomLayer::Tier, vec![(29, true), (30, true), (31, true)], 2));
        screen.key(CampKey::Down, false, &mut sounds, &mut actions);
        assert_eq!(screen.tier_cursor, 0, "wraps");
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions);
        assert_eq!(actions, vec![CampAction::SetMagicSlot { slot: 0, kind: 29 }]);
        assert_eq!(screen.data.name(screen.data.magic_slots[0]), "Fire");
        // Triangle on slot 0, down, Triangle: slots 0 and 1 swapped.
        actions.clear();
        screen.key(CampKey::Triangle, false, &mut sounds, &mut actions);
        screen.key(CampKey::Down, false, &mut sounds, &mut actions);
        screen.key(CampKey::Triangle, false, &mut sounds, &mut actions);
        assert_eq!(actions, vec![CampAction::SwapMagicSlots(0, 1)]);
        assert_eq!((screen.data.magic_slots[0], screen.data.magic_slots[1], screen.picked), (43, 29, None));
        // Shortcut Set A, button 0, decide: tabs All / Magic / Items (no Links tab), Magic tab 18 entries.
        screen.key(CampKey::Cancel, false, &mut sounds, &mut actions);
        screen.key(CampKey::Down, false, &mut sounds, &mut actions);
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions);
        assert_eq!((screen.layer, screen.page), (CustomLayer::Set, 0));
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions);
        assert_eq!(screen.tabs(), vec![0, 1, 3]);
        assert_eq!(screen.list_rows().len(), 21);
        screen.key(CampKey::R2, false, &mut sounds, &mut actions);
        assert_eq!(screen.list_rows().len(), 18);
        // Choose Curaga (the last magic entry): page A, slot 0.
        screen.key(CampKey::Up, false, &mut sounds, &mut actions);
        actions.clear();
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions);
        assert_eq!(actions, vec![CampAction::SetShortcut { page: 0, slot: 0, kind: 51 }]);
        assert_eq!(screen.layer, CustomLayer::Set);
        // Square on the button: empty again.
        actions.clear();
        screen.key(CampKey::Square, false, &mut sounds, &mut actions);
        assert_eq!(actions, vec![CampAction::SetShortcut { page: 0, slot: 0, kind: 0 }]);
        // Cancel steps back one layer at a time, then leaves.
        assert!(!screen.key(CampKey::Cancel, false, &mut sounds, &mut actions));
        assert!(screen.key(CampKey::Cancel, false, &mut sounds, &mut actions));
    }

    /// (party X2) A friend leader's page: Goofy's three abilities with the host's tier groups and his label.
    #[test]
    fn goofys_abilities_page_picks_a_tier_from_the_host_groups() {
        let (mut sounds, mut actions) = (Vec::new(), Vec::new());
        let mut entries = Vec::new();
        for (base, name) in [(0xC1u8, "Goofy Tornado"), (0xC4, "Goofy Turbo"), (0xC7, "Goofy Discus")] {
            entries.push(entry(base, name, GROUP_MAGIC));
            entries.push(entry(base + 1, &format!("{name} II"), GROUP_MAGIC));
        }
        let data = CustomizeIn { magic_slots: [0xC2, 0xC5, 0xC8, 0, 0, 0], entries, magic_label: Some("kh3_camp.customize.group_0010".into()), tier_groups: vec![[0xC1, 0xC2, 0xC3], [0xC4, 0xC5, 0xC6], [0xC7, 0xC8, 0xC9]], ..CustomizeIn::default() };
        let mut screen = CustomizeScreen::new();
        screen.set(data);
        assert_eq!(screen.magic_count(), 3);
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions);
        screen.key(CampKey::Down, false, &mut sounds, &mut actions);
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions);
        assert_eq!((screen.layer, screen.tiers()), (CustomLayer::Tier, vec![(0xC4, true), (0xC5, true), (0xC6, false)]));
        screen.key(CampKey::Up, false, &mut sounds, &mut actions);
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions);
        assert_eq!(actions, vec![CampAction::SetMagicSlot { slot: 1, kind: 0xC4 }]);
        assert_eq!(screen.data.name(screen.data.magic_slots[1]), "Goofy Turbo");
    }

    #[test]
    fn a_tier_that_is_not_learned_is_refused() {
        let mut data = level_99();
        data.entries.retain(|e| e.kind != 30);
        let (mut sounds, mut actions) = (Vec::new(), Vec::new());
        let mut screen = CustomizeScreen::new();
        screen.set(data);
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions);
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions);
        screen.key(CampKey::Up, false, &mut sounds, &mut actions);
        assert_eq!(screen.tiers()[screen.tier_cursor], (30, false));
        sounds.clear();
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions);
        assert_eq!((sounds, actions.len()), (vec![UiSound::Play(SE_MENU_REFUSED)], 0));
    }

    #[test]
    fn game_help_lists_and_pages() {
        let mut screen = HelpScreen::new();
        screen.set(vec![
            HelpCategory { name: "Mechanics".into(), topics: vec![HelpTopic { title: "Save Points".into(), pages: vec!["a".into()] }] },
            HelpCategory { name: "Combat".into(), topics: vec![HelpTopic { title: "Combat Basics".into(), pages: vec!["a".into(), "b".into()] }, HelpTopic { title: "Reprisals".into(), pages: vec!["c".into()] }] },
        ]);
        let mut sounds = Vec::new();
        screen.key(CampKey::Down, false, &mut sounds);
        assert_eq!(screen.topics.count, 2);
        screen.key(CampKey::Decide, false, &mut sounds);
        screen.key(CampKey::Decide, false, &mut sounds);
        assert_eq!((screen.layer, screen.topic().map(|t| t.title.as_str())), (HelpLayer::Page(0), Some("Combat Basics")));
        screen.key(CampKey::Decide, false, &mut sounds);
        assert_eq!(screen.layer, HelpLayer::Page(1));
        screen.key(CampKey::Decide, false, &mut sounds);
        assert_eq!(screen.layer, HelpLayer::Page(0), "back to the first page");
        assert!(!screen.key(CampKey::Cancel, false, &mut sounds));
        assert!(!screen.key(CampKey::Cancel, false, &mut sounds));
        assert!(screen.key(CampKey::Cancel, false, &mut sounds));
    }
}
