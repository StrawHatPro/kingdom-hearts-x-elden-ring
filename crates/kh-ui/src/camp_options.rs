//! (CAMP-OPTIONS, THE USER 2026-11-05: "change the camp's options menu so that they actually reflect Elden Ring's
//! settings, every Elden Ring settings option should be accessible from it, and remove any options that don't do
//! anything. Use tabs like Elden Ring but keep it all looking like KH") THE TABBED CONFIG SCREEN'S LOGIC, engine-free
//! and display-free like the rest of the camp menu (`camp.rs`): pages of items the host describes (Elden Ring's tabs
//! in its own order, with its own texts; the host's own page of KH3 rows), a cursor, a sub-page stack (Graphics >
//! Advanced Settings, HDR > Adjust Image Quality), and the events the host turns into the game's own writes.
//!
//! PAD (KH3's layout, the camp menu's keys): LB / RB switch the tab (Elden Ring's options switch tabs on the same
//! buttons; the camp's Items tab bar is drawn for it), Up / Down the row, Left / Right the value (every change is
//! sent AT ONCE, as Elden Ring's own options screen applies every change at once), A on a button row opens what it
//! names, LT / RT a page of rows (KH3's Config paging, moved off LB / RB), Y restores the shown page's defaults
//! (after KH3's question), B steps back out of a sub page, then leaves. The keyboard reaches the same keys through
//! the camp menu's own bindings.
//!
//! What a page shows is drawn by `camp_menu.rs` with KH3's Config rows (`setConfigValueN`: name, value with its
//! arrows or KH3's slider), the help line, the scroll bar and the tab bar.

use crate::camp::{move_wrapping, CampKey, OptionRow, CONFIG_VISIBLE};
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_REFUSED};
use crate::UiSound;

/// What a row offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OptKind {
    /// Choice texts in display order (the value is the index).
    Choice(Vec<String>),
    /// A slider over `min ..= max` (the value is the slider value).
    Slider { min: u8, max: u8 },
    /// A row that opens something (a sub page, a screen of the game's): the text shown as its value.
    Button(String),
}

/// One row, as the host describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptItem {
    /// The host's key of the row (sent back in the events).
    pub key: u32,
    pub label: String,
    /// The help line of the row.
    pub help: String,
    pub kind: OptKind,
    /// Choice index or slider value.
    pub value: u8,
    /// False: shown, not changeable now (Elden Ring greys such rows out: HDR's sliders while HDR is off).
    pub enabled: bool,
}

impl OptItem {
    /// The range Left / Right move the value in.
    pub fn range(&self) -> (u8, u8) {
        match &self.kind {
            OptKind::Choice(list) => (0, list.len().saturating_sub(1).min(255) as u8),
            OptKind::Slider { min, max } => (*min, (*max).max(*min)),
            OptKind::Button(_) => (0, 0),
        }
    }

    /// The row as the Config screen's drawing takes it.
    pub fn row(&self) -> OptionRow {
        let (choices, range) = match &self.kind {
            OptKind::Choice(list) => (list.clone(), self.range()),
            OptKind::Slider { min, max } => (Vec::new(), (*min, *max)),
            OptKind::Button(text) => (vec![text.clone()], (0, 0)),
        };
        let value = if matches!(self.kind, OptKind::Button(_)) { 0 } else { self.value };
        OptionRow { label: self.label.clone(), choices, range, value, stored: self.enabled }
    }
}

/// A tab (or a sub page) of rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptPage {
    pub key: u32,
    pub title: String,
    pub items: Vec<OptItem>,
}

/// What the screen asks of the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OptEvent {
    /// A row's value was changed (Left / Right): the host applies it now and answers with the pages again.
    Changed { page: u32, item: u32, value: u8 },
    /// A button row was chosen (A): the host pushes a sub page, opens a screen of the game's, or asks a question.
    Pressed { page: u32, item: u32 },
    /// Y + Yes: the page's rows go back to the game's defaults.
    Defaults { page: u32 },
    /// The answer to a question the host asked ([`OptionsScreen::ask`]).
    Answer { ask: u32, yes: bool },
    /// A sub page was left (B).
    SubLeft { page: u32 },
    /// The screen was left.
    Left,
}

/// A question the host asks over the screen (Yes / No): its text (the game's own) and its key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptAsk {
    pub key: u32,
    pub text: String,
    /// The cursor's first place: 0 = Yes, 1 = No.
    pub cursor: usize,
}

/// The tabbed screen while it is open.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct OptionsScreen {
    pub tabs: Vec<OptPage>,
    pub tab: usize,
    /// Sub pages over the tab (last = shown), each with the cursor and scroll of the page under it.
    pub subs: Vec<(OptPage, usize, usize)>,
    pub cursor: usize,
    pub scroll: usize,
    /// The cursor and scroll of every tab (a tab comes back where it was left).
    places: Vec<(usize, usize)>,
    /// The last tab switch (true = to the right): the view flashes that arrow once.
    pub tab_flash: Option<bool>,
    /// A question of the host's, waiting for the camp menu's dialog.
    pub ask: Option<OptAsk>,
    /// (CAMP-OPTIONS) the host captures a button for a row: the screen takes no keys until it is done.
    pub capturing: bool,
}

impl OptionsScreen {
    pub fn new(tabs: Vec<OptPage>) -> OptionsScreen {
        let places = vec![(0, 0); tabs.len()];
        OptionsScreen { tabs, places, ..OptionsScreen::default() }
    }

    /// The page shown (the top sub page, else the tab).
    pub fn page(&self) -> Option<&OptPage> {
        self.subs.last().map(|s| &s.0).or_else(|| self.tabs.get(self.tab))
    }

    fn page_mut(&mut self) -> Option<&mut OptPage> {
        match self.subs.last_mut() {
            Some(s) => Some(&mut s.0),
            None => self.tabs.get_mut(self.tab),
        }
    }

    pub fn item(&self) -> Option<&OptItem> {
        self.page().and_then(|p| p.items.get(self.cursor))
    }

    /// The help line of the focused row.
    pub fn help(&self) -> String {
        self.item().map(|i| i.help.clone()).unwrap_or_default()
    }

    /// The rows of the shown page, as the Config screen draws them.
    pub fn rows(&self) -> Vec<OptionRow> {
        self.page().map(|p| p.items.iter().map(OptItem::row).collect()).unwrap_or_default()
    }

    pub fn tab_titles(&self) -> Vec<String> {
        self.tabs.iter().map(|t| t.title.clone()).collect()
    }

    /// True while a sub page is shown (the tab bar does not switch).
    pub fn in_sub(&self) -> bool {
        !self.subs.is_empty()
    }

    fn len(&self) -> usize {
        self.page().map_or(0, |p| p.items.len())
    }

    fn scroll_max(&self) -> usize {
        self.len().saturating_sub(CONFIG_VISIBLE)
    }

    fn focus(&mut self, row: usize) {
        self.cursor = row;
        self.scroll = crate::list::window_scroll(row, self.scroll, CONFIG_VISIBLE, 0, self.scroll_max());
    }

    fn clamp(&mut self) {
        let len = self.len();
        self.cursor = self.cursor.min(len.saturating_sub(1));
        self.scroll = self.scroll.min(self.scroll_max());
        let cursor = self.cursor;
        self.focus(cursor);
    }

    /// THE HOST'S ANSWER (to an event, or a refresh of its own): the tabs and every sub page again with their values
    /// now; a sub page on the stack is replaced by the one of the same key in `subs` (one the host no longer has is
    /// left as it was). The cursors stay where they are.
    pub fn update(&mut self, tabs: Vec<OptPage>, subs: &[OptPage]) {
        if tabs.len() != self.tabs.len() {
            self.places.resize(tabs.len(), (0, 0));
            self.tab = self.tab.min(tabs.len().saturating_sub(1));
        }
        self.tabs = tabs;
        for (page, _, _) in self.subs.iter_mut() {
            if let Some(new) = subs.iter().find(|s| s.key == page.key) {
                *page = new.clone();
            }
        }
        self.clamp();
    }

    /// A sub page over the shown one (the host's answer to a [`OptEvent::Pressed`]); the cursor starts on its first row.
    pub fn push(&mut self, page: OptPage) {
        self.subs.push((page, self.cursor, self.scroll));
        (self.cursor, self.scroll) = (0, 0);
    }

    /// The tab shown now (a host that needs to know: what Y restores).
    pub fn page_key(&self) -> Option<u32> {
        self.page().map(|p| p.key)
    }

    fn switch_tab(&mut self, right: bool, sounds: &mut Vec<UiSound>) {
        let count = self.tabs.len();
        if count < 2 || self.in_sub() {
            return;
        }
        if let Some(place) = self.places.get_mut(self.tab) {
            *place = (self.cursor, self.scroll);
        }
        self.tab = if right { (self.tab + 1) % count } else { (self.tab + count - 1) % count };
        (self.cursor, self.scroll) = self.places.get(self.tab).copied().unwrap_or((0, 0));
        self.clamp();
        self.tab_flash = Some(right);
        sounds.push(UiSound::Play(SE_MENU_CURSOR));
    }

    /// One key (`repeat` = held). Returns the events for the host; [`OptEvent::Left`] = the screen was left.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) -> Vec<OptEvent> {
        let mut events = Vec::new();
        if self.capturing {
            return events;
        }
        let page_key = self.page_key().unwrap_or(0);
        match key {
            CampKey::Up | CampKey::Down => {
                if let Some(to) = move_wrapping(self.cursor, self.len(), key == CampKey::Down, repeat) {
                    self.focus(to);
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                }
            }
            CampKey::L1 | CampKey::R1 => {
                if !repeat {
                    self.switch_tab(key == CampKey::R1, sounds);
                }
            }
            // KH3's paging (`0x141541A20`, events 9 / 10), on the triggers here: a page of rows; at the ends a fresh press
            // wraps, a held key stops.
            CampKey::L2 | CampKey::R2 => {
                let (count, forward) = (self.len(), key == CampKey::R2);
                if count == 0 || (repeat && if forward { self.cursor + 1 >= count } else { self.cursor == 0 }) {
                    return events;
                }
                let before = self.cursor;
                if self.cursor == 0 && !forward {
                    self.focus(count - 1);
                } else if self.cursor + 1 == count && forward {
                    self.focus(0);
                } else {
                    let old = self.scroll as i64;
                    let wanted = old + if forward { CONFIG_VISIBLE as i64 } else { -(CONFIG_VISIBLE as i64) };
                    let scroll = wanted.clamp(0, self.scroll_max() as i64);
                    self.scroll = scroll as usize;
                    let mut row = (self.cursor as i64 - old + scroll).max(0) as usize;
                    if scroll == old {
                        row = if forward { count - 1 } else { 0 };
                    }
                    self.focus(row.min(count - 1));
                }
                if self.cursor != before {
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                }
            }
            CampKey::Left | CampKey::Right => {
                let cursor = self.cursor;
                let Some(item) = self.page_mut().and_then(|p| p.items.get_mut(cursor)) else { return events };
                if !item.enabled || matches!(item.kind, OptKind::Button(_)) {
                    return events;
                }
                let (min, max) = item.range();
                let to = if key == CampKey::Right { item.value.saturating_add(1).min(max) } else { item.value.saturating_sub(1).max(min) };
                if to != item.value {
                    item.value = to;
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    events.push(OptEvent::Changed { page: page_key, item: item.key, value: to });
                }
            }
            CampKey::Decide => match self.item() {
                Some(item) if matches!(item.kind, OptKind::Button(_)) && item.enabled => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    events.push(OptEvent::Pressed { page: page_key, item: item.key });
                }
                Some(item) if matches!(item.kind, OptKind::Button(_)) => sounds.push(UiSound::Play(SE_MENU_REFUSED)),
                _ => {}
            },
            CampKey::Cancel => {
                sounds.push(UiSound::Play(SE_MENU_CLOSE));
                match self.subs.pop() {
                    Some((page, cursor, scroll)) => {
                        (self.cursor, self.scroll) = (cursor, scroll);
                        self.clamp();
                        events.push(OptEvent::SubLeft { page: page.key });
                    }
                    None => events.push(OptEvent::Left),
                }
            }
            _ => {}
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(key: u32, n: usize) -> OptPage {
        OptPage {
            key,
            title: format!("Tab {key}"),
            items: (0..n)
                .map(|k| OptItem {
                    key: key * 100 + k as u32,
                    label: format!("Row {k}"),
                    help: format!("Help {k}"),
                    kind: match k % 3 {
                        0 => OptKind::Choice(vec!["On".into(), "Off".into()]),
                        1 => OptKind::Slider { min: 0, max: 10 },
                        _ => OptKind::Button("Open".into()),
                    },
                    value: if k % 3 == 1 { 5 } else { 0 },
                    enabled: k != 4,
                })
                .collect(),
        }
    }

    #[test]
    fn co_tabs_switch_on_lb_rb_and_remember_their_rows() {
        let mut s = OptionsScreen::new(vec![page(1, 7), page(2, 3), page(3, 14)]);
        let mut sounds = Vec::new();
        s.key(CampKey::Down, false, &mut sounds);
        s.key(CampKey::Down, false, &mut sounds);
        assert_eq!(s.cursor, 2);
        s.key(CampKey::R1, false, &mut sounds);
        assert_eq!((s.tab, s.cursor, s.tab_flash), (1, 0, Some(true)));
        s.key(CampKey::L1, false, &mut sounds);
        assert_eq!((s.tab, s.cursor), (0, 2), "the tab comes back where it was left");
        s.key(CampKey::L1, false, &mut sounds);
        assert_eq!(s.tab, 2, "LB on the first tab wraps to the last");
        s.key(CampKey::R1, false, &mut sounds);
        assert_eq!(s.tab, 0, "RB on the last wraps to the first");
        // a held LB / RB does not race through the tabs
        s.key(CampKey::R1, true, &mut sounds);
        assert_eq!(s.tab, 0);
        assert_eq!(s.help(), "Help 2");
        assert_eq!(s.tab_titles(), vec!["Tab 1", "Tab 2", "Tab 3"]);
    }

    #[test]
    fn co_changes_are_sent_at_once_and_stay_in_range() {
        let mut s = OptionsScreen::new(vec![page(1, 7)]);
        let mut sounds = Vec::new();
        // row 0: a choice of two
        assert_eq!(s.key(CampKey::Right, false, &mut sounds), vec![OptEvent::Changed { page: 1, item: 100, value: 1 }]);
        assert!(s.key(CampKey::Right, false, &mut sounds).is_empty(), "no wrap past the last choice");
        assert_eq!(s.key(CampKey::Left, false, &mut sounds), vec![OptEvent::Changed { page: 1, item: 100, value: 0 }]);
        // row 1: the slider 0 ..= 10 at 5
        s.key(CampKey::Down, false, &mut sounds);
        for _ in 0..8 {
            s.key(CampKey::Right, true, &mut sounds);
        }
        assert_eq!(s.item().unwrap().value, 10);
        // row 2: a button: A presses it, Left / Right do nothing
        s.key(CampKey::Down, false, &mut sounds);
        assert!(s.key(CampKey::Right, false, &mut sounds).is_empty());
        assert_eq!(s.key(CampKey::Decide, false, &mut sounds), vec![OptEvent::Pressed { page: 1, item: 102 }]);
        // row 4: disabled: no change
        s.key(CampKey::Down, false, &mut sounds);
        s.key(CampKey::Down, false, &mut sounds);
        assert!(s.key(CampKey::Right, false, &mut sounds).is_empty());
        let rows = s.rows();
        assert_eq!(rows.len(), 7);
        assert!(!rows[4].stored && rows[2].choices == vec!["Open".to_string()]);
    }

    #[test]
    fn co_sub_pages_and_the_way_out() {
        let mut s = OptionsScreen::new(vec![page(1, 7), page(2, 3)]);
        let mut sounds = Vec::new();
        s.key(CampKey::Down, false, &mut sounds);
        s.key(CampKey::Down, false, &mut sounds);
        s.push(page(9, 14));
        assert!(s.in_sub() && s.cursor == 0 && s.page_key() == Some(9));
        s.key(CampKey::R1, false, &mut sounds);
        assert_eq!((s.tab, s.page_key()), (0, Some(9)), "no tab switch inside a sub page");
        // the host's refresh replaces the sub page of the same key
        let mut new = page(9, 14);
        new.items[0].value = 1;
        s.update(vec![page(1, 7), page(2, 3)], &[new]);
        assert_eq!(s.item().unwrap().value, 1);
        assert_eq!(s.key(CampKey::Cancel, false, &mut sounds), vec![OptEvent::SubLeft { page: 9 }]);
        assert_eq!((s.page_key(), s.cursor), (Some(1), 2), "back on the row that opened it");
        assert_eq!(s.key(CampKey::Cancel, false, &mut sounds), vec![OptEvent::Left]);
    }

    #[test]
    fn co_paging_on_the_triggers() {
        let mut s = OptionsScreen::new(vec![page(1, 25)]);
        let mut sounds = Vec::new();
        s.key(CampKey::R2, false, &mut sounds);
        assert_eq!((s.cursor, s.scroll), (10, 10));
        s.key(CampKey::R2, false, &mut sounds);
        assert_eq!((s.cursor, s.scroll), (15, 15));
        s.key(CampKey::R2, false, &mut sounds);
        assert_eq!(s.cursor, 24);
        s.key(CampKey::R2, false, &mut sounds);
        assert_eq!((s.cursor, s.scroll), (0, 0), "a fresh press at the end wraps");
        // a refresh with fewer rows keeps the cursor inside
        s.focus(24);
        s.update(vec![page(1, 5)], &[]);
        assert_eq!(s.cursor, 4);
    }
}
