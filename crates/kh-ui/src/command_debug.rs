//! DEBUG stand-in for the command system: fills a [`CommandModel`] from debug inputs so that the
//! command menu's display can be exercised before the real command system exists. NOTHING here
//! is a gameplay rule to build on: the list CONTENTS are made up (a few spells, items with
//! chosen quantities), nothing executes, nothing costs. What it does take from the game is what
//! the display code itself reveals (docs/sora-ui-spec.md 5.3 - 5.6, 5.10): which pad key does
//! what in which state, the wrap of the cursors, what greys a row, the situation timers' start
//! delay, the shortcut pages.

use crate::command::{CloseKind, CommandModel, EventButton, MenuEvent, MenuRow, MenuState, ModePanel, ShootFlow, Situation, TargetList};
use crate::hud_command::{ACTION_COMMAND_TYPE_STYLE_POINT_GAUGE, BUTTON_TEXT_DOWN, BUTTON_TEXT_L1, BUTTON_TEXT_LEFT, BUTTON_TEXT_UP};

/// The pad keys of the menu (5.10 item 1), by meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    Left,
    Right,
    /// Pad key 27.
    Confirm,
    /// Pad key 28.
    Cancel,
    /// Pad key 4 (L1) went down / came up.
    ShortcutDown,
    ShortcutUp,
    /// The face buttons in the shortcut panel: 0 right, 1 top, 2 left, 3 bottom.
    Face(usize),
    /// Pad key 0 (top face button): use the situation command.
    SituationUse,
    /// Pad key 6 (L2): switch the situation command.
    SituationSwitch,
}

#[derive(Clone, Debug)]
struct Entry {
    name: String,
    /// MP cost (255 = all of it, as Cure's row of the command table says).
    mp: i32,
    quantity: u32,
    /// Opens the party target list (Cure, recovery items).
    targeted: bool,
}

/// Seconds a situation command waits before its timer runs (`0x141630AF0`).
pub const SITUATION_START_DELAY: f32 = 0.5;
/// At most 6 list entries.
pub const SITUATION_MAX: usize = 6;

pub struct DebugCommandSystem {
    pub model: CommandModel,
    lists: [Vec<Entry>; 4],
    shortcuts: [[Option<(usize, usize)>; 4]; 3],
    remembered: [usize; 4],
    mp: i32,
    mp_max: i32,
    mp_charging: bool,
    party: Vec<String>,
    next_serial: u32,
    /// (serial, start delay left).
    delays: Vec<(u32, f32)>,
    selected_serial: Option<u32>,
    return_state: MenuState,
    shoot_rows: Vec<(String, String)>,
}

impl DebugCommandSystem {
    /// The shotlock panel's rows (text, pad button text).
    pub fn shoot_rows(&self) -> &[(String, String)] {
        &self.shoot_rows
    }

    /// `main` = the four slot texts (`MainCommandData` row `NORMAL`); the debug lists' names and
    /// MP costs come from the pack's command table when it is at hand.
    pub fn new(pack: Option<&mut crate::def::Pack>) -> DebugCommandSystem {
        let mut main = ["Attack".to_string(), "Magic".to_string(), "Items".to_string(), "Link".to_string()];
        let mut shoot_rows = vec![("Shotlocks".to_string(), BUTTON_TEXT_DOWN.to_string()), ("Airstep".to_string(), BUTTON_TEXT_LEFT.to_string())];
        let mut costs: std::collections::HashMap<i64, (String, i32)> = std::collections::HashMap::new();
        if let Some(pack) = pack {
            if let Ok(styles) = pack.data("main_command_styles") {
                for row in styles["rows"].as_array().into_iter().flatten() {
                    let names: Vec<String> = row["buttons_en"].as_array().into_iter().flatten().filter_map(|b| b.as_str().map(str::to_string)).collect();
                    match row["id"].as_str() {
                        Some("NORMAL") if names.len() == 4 => main = [names[0].clone(), names[1].clone(), names[2].clone(), names[3].clone()],
                        Some("SHOOTFLOW") if names.len() >= 2 => {
                            shoot_rows[0].0 = names[0].clone();
                            shoot_rows[1].0 = names[1].clone();
                        }
                        _ => {}
                    }
                }
            }
            if let Ok(commands) = pack.data("commands") {
                for row in commands["rows"].as_array().into_iter().flatten() {
                    if let (Some(id), Some(name)) = (row["id"].as_i64(), row["name_en"].as_str()) {
                        costs.insert(id, (name.to_string(), row["mp"].as_i64().unwrap_or(0) as i32));
                    }
                }
            }
        }
        let entry = |id: i64, fallback: &str, mp: i32, quantity: u32, targeted: bool| {
            let (name, mp) = costs.get(&id).cloned().unwrap_or((fallback.to_string(), mp));
            Entry { name, mp, quantity, targeted }
        };
        // DEBUG CONTENT (not his real loadout): command kinds 29 / 33 / 37 / 41 / 45 / 49 and
        // the items 58 / 61 / 59 / 64 with quantities chosen to show "x3", nothing, "x9", greyed.
        let lists = [
            Vec::new(),
            vec![entry(29, "Fire", 10, 0, false), entry(33, "Blizzard", 12, 0, false), entry(37, "Thunder", 14, 0, false), entry(41, "Water", 11, 0, false), entry(45, "Aero", 13, 0, false), entry(49, "Cure", 255, 0, true)],
            vec![entry(58, "Potion", 0, 3, true), entry(61, "Ether", 0, 1, true), entry(59, "Hi-Potion", 0, 12, true), entry(64, "Elixir", 0, 0, true)],
            vec![Entry { name: "Meow Wow Balloon".into(), mp: 0, quantity: 0, targeted: false }, Entry { name: "King's Flare".into(), mp: 0, quantity: 0, targeted: false }],
        ];
        let shortcuts = [[Some((1, 0)), Some((1, 2)), Some((1, 5)), Some((2, 0))], [Some((1, 1)), Some((1, 3)), None, Some((2, 2))], [Some((1, 4)), None, None, Some((2, 1))]];
        let mut system = DebugCommandSystem {
            model: CommandModel::default(),
            lists,
            shortcuts,
            remembered: [0; 4],
            mp: 100,
            mp_max: 100,
            mp_charging: false,
            party: vec!["Sora".into()],
            next_serial: 1,
            delays: Vec::new(),
            selected_serial: None,
            return_state: MenuState::MainSel,
            shoot_rows,
        };
        for (slot, text) in main.iter().enumerate() {
            system.model.main[slot] = MenuRow::new(text);
        }
        system.refresh();
        system
    }

    /// What greys and recolours rows comes from his MP state (5.4).
    pub fn set_status(&mut self, mp: i32, mp_max: i32, mp_charging: bool) {
        (self.mp, self.mp_max, self.mp_charging) = (mp, mp_max, mp_charging);
    }

    /// Replaces a debug list (1 Magic, 2 Items, 3 Link): (name, MP cost, quantity, targeted).
    pub fn set_list(&mut self, list: usize, rows: &[(&str, i32, u32, bool)]) {
        if let Some(slot) = self.lists.get_mut(list.clamp(1, 3)) {
            *slot = rows.iter().map(|r| Entry { name: r.0.to_string(), mp: r.1, quantity: r.2, targeted: r.3 }).collect();
        }
    }

    fn row(&self, list: usize, e: &Entry) -> MenuRow {
        let (enabled, mp_short) = match list {
            // Magic: disabled while MP Charge runs; low MP only recolours.
            1 => (!self.mp_charging, e.mp != 255 && e.mp > self.mp),
            // An item row with quantity 0 is greyed.
            2 => (e.quantity > 0, false),
            // Link rows need full MP.
            _ => (self.mp >= self.mp_max && !self.mp_charging, false),
        };
        MenuRow { text: e.name.clone(), enabled, mp_short, quantity: if list == 2 { e.quantity } else { 0 }, flash: false }
    }

    /// Rebuilds the rows from the debug lists and the MP state.
    fn refresh(&mut self) {
        for list in 1..=3 {
            let rows: Vec<MenuRow> = self.lists[list].iter().map(|e| self.row(list, e)).collect();
            // A main slot whose list has no enabled row is greyed.
            self.model.main[list].enabled = rows.iter().any(|r| r.enabled);
            self.model.lists[list] = rows;
        }
        self.model.main[0].enabled = true;
        self.model.link_none = self.lists[3].is_empty();
        let page = self.model.shortcut_page.min(2);
        for (slot, entry) in self.shortcuts[page].iter().enumerate() {
            self.model.shortcut[slot] = match entry.and_then(|(list, index)| self.lists[list].get(index).map(|e| (list, e))) {
                Some((list, e)) => self.row(list, e),
                None => MenuRow::default(),
            };
        }
        // The active situation command: the chosen one, else the oldest (5.10 item 6).
        self.model.situation_selected = match self.selected_serial.and_then(|s| self.model.situations.iter().position(|x| x.serial == s)) {
            Some(i) => Some(i),
            None if self.model.event_button.is_some() => None,
            None => (!self.model.situations.is_empty()).then_some(0),
        };
    }

    /// Start of a frame: last frame's one-shot events are gone.
    pub fn begin_frame(&mut self) {
        self.model.events.clear();
    }

    /// The system's own per-frame work: state In -> MainSel, situation timers.
    pub fn step(&mut self, dt: f32) {
        if self.model.state == MenuState::In {
            self.model.state = MenuState::MainSel;
        }
        // Timers stop while a command executes; nothing executes here.
        let mut expired = Vec::new();
        for s in &mut self.model.situations {
            if s.total <= 0.0 {
                continue;
            }
            match self.delays.iter_mut().find(|d| d.0 == s.serial) {
                Some(delay) if delay.1 > 0.0 => delay.1 -= dt,
                _ => {
                    s.remaining -= dt;
                    if s.remaining <= 0.0 {
                        s.remaining = 0.0;
                        expired.push(s.serial);
                    }
                }
            }
        }
        self.model.situations.retain(|s| !expired.contains(&s.serial));
        self.delays.retain(|d| !expired.contains(&d.0));
        self.refresh();
    }

    fn cursor_span(&self) -> usize {
        if self.model.link_none {
            3
        } else {
            4
        }
    }

    pub fn key(&mut self, key: MenuKey) {
        let m = &mut self.model;
        match (m.state, key) {
            (MenuState::MainSel, MenuKey::Up | MenuKey::Down) => {
                let n = self.cursor_span();
                let m = &mut self.model;
                m.main_cursor = if key == MenuKey::Up { (m.main_cursor + n - 1) % n } else { (m.main_cursor + 1) % n };
            }
            (MenuState::MainSel, MenuKey::Confirm | MenuKey::Right) => {
                let list = m.main_cursor;
                if list == 0 {
                    // Attack executes: the menu does not change (and Right does nothing).
                } else if m.lists[list].is_empty() || !m.main[list].enabled {
                    m.events.push(MenuEvent::Refused);
                } else {
                    m.list = list;
                    m.row_cursor = self.remembered[list].min(m.lists[list].len() - 1);
                    m.target = None;
                    m.state = MenuState::SubSel;
                }
            }
            (MenuState::MainSel, MenuKey::ShortcutDown) => {
                self.return_state = MenuState::MainSel;
                m.state = MenuState::Shortcut;
            }
            (MenuState::SubSel, MenuKey::Up | MenuKey::Down) => {
                let up = key == MenuKey::Up;
                let step = |cursor: usize, n: usize| if n == 0 { 0 } else if up { (cursor + n - 1) % n } else { (cursor + 1) % n };
                match &mut m.target {
                    Some(target) => target.cursor = step(target.cursor, target.names.len()),
                    None => {
                        m.row_cursor = step(m.row_cursor, m.lists[m.list].len().min(9));
                        self.remembered[m.list] = m.row_cursor;
                    }
                }
            }
            (MenuState::SubSel, MenuKey::Confirm | MenuKey::Right) => {
                let row = &m.lists[m.list][m.row_cursor];
                if !row.enabled {
                    m.events.push(MenuEvent::Refused);
                } else if m.target.is_none() && self.lists[m.list][m.row_cursor].targeted && self.party.len() >= 2 {
                    m.target = Some(TargetList { names: self.party.clone(), cursor: 0 });
                } else if key == MenuKey::Confirm || m.target.is_none() {
                    // "Executed": the list closes, both cursors go back to Attack.
                    m.events.push(MenuEvent::SubClosed(CloseKind::Decide));
                    m.target = None;
                    m.state = MenuState::MainSel;
                    m.main_cursor = 0;
                    self.remembered[m.list] = 0;
                }
            }
            (MenuState::SubSel, MenuKey::Cancel | MenuKey::Left) => {
                if m.target.is_some() {
                    m.target = None;
                } else if key == MenuKey::Cancel {
                    m.events.push(MenuEvent::SubClosed(CloseKind::Cancel));
                    m.state = MenuState::MainSel;
                    m.main_cursor = 0;
                } else {
                    m.events.push(MenuEvent::SubClosed(CloseKind::Back));
                    m.state = MenuState::MainSel;
                }
            }
            (MenuState::Shortcut, MenuKey::Down) => m.shortcut_page = (m.shortcut_page + 1) % 3,
            (MenuState::Shortcut, MenuKey::Up) => m.shortcut_page = (m.shortcut_page + 2) % 3,
            (MenuState::Shortcut, MenuKey::Face(slot)) => {
                let row = &m.shortcut[slot.min(3)];
                if row.text.is_empty() || !row.enabled {
                    m.events.push(MenuEvent::Refused);
                } else {
                    m.events.push(MenuEvent::ShortcutExecuted { style_change: false });
                    m.state = self.return_state;
                }
            }
            (MenuState::Shortcut, MenuKey::ShortcutUp) => m.state = self.return_state,
            (_, MenuKey::SituationSwitch) => {
                if m.situations.len() > 1 {
                    let at = m.situation_selected.unwrap_or(0);
                    self.selected_serial = Some(m.situations[(at + 1) % m.situations.len()].serial);
                }
            }
            (_, MenuKey::SituationUse) => {
                if m.event_button.is_some() && m.situation_selected.is_none() {
                    m.events.push(MenuEvent::EventButtonUsed { form_change: false });
                    m.event_button = None;
                } else if let Some(at) = m.situation_selected.filter(|i| *i < m.situations.len()) {
                    let used = m.situations.remove(at);
                    m.events.push(MenuEvent::SituationUsed { serial: used.serial, form_change: used.icon_type == 1 });
                    self.selected_serial = None;
                }
            }
            _ => {}
        }
        self.refresh();
    }

    /// Adds a situation command (new entries go above the older ones); a 7th first removes the
    /// oldest timed one. Returns its serial.
    pub fn add_situation(&mut self, name: &str, seconds: f32, icon_type: i32) -> u32 {
        if let Some(same) = self.model.situations.iter_mut().find(|s| s.name == name && s.icon_type == icon_type) {
            // The same kind again: the time is refreshed and the flash replays.
            same.remaining = seconds;
            same.total = seconds;
            same.refreshed += 1;
            return same.serial;
        }
        if self.model.situations.len() >= SITUATION_MAX {
            if let Some(oldest) = self.model.situations.iter().position(|s| s.total > 0.0) {
                self.model.situations.remove(oldest);
            } else {
                return 0;
            }
        }
        let serial = self.next_serial;
        self.next_serial += 1;
        self.model.situations.push(Situation { serial, name: name.to_string(), icon_type, in_effect: 0, total: seconds, remaining: seconds, enabled: true, refreshed: 0 });
        self.delays.push((serial, SITUATION_START_DELAY));
        self.refresh();
        serial
    }

    /// A debug verb (`hud-cmd ...` in the test app). Words:
    /// `show` / `hide`; `up` `down` `left` `right` `confirm` `cancel`; `open magic|items|link`;
    /// `shortcut on|off`, `page down|up`, `face right|top|left|bottom`;
    /// `situation add NAME [SECONDS [ICON]]`, `situation use|switch|break|clear`;
    /// `event add NAME` / `event use`; `battle 0|1|2`; `link on|off`; `party on|off`;
    /// `mp N [MAX]`, `mpcharge on|off`; `style RATIO|off`; `shotlock on|off`,
    /// `shotlock enable ROW on|off`; `form on|off`; `simple on|off`.
    pub fn command(&mut self, text: &str) -> Result<(), String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let on = |w: Option<&&str>| -> Result<bool, String> {
            match w.copied() {
                Some("on") | Some("1") | Some("true") => Ok(true),
                Some("off") | Some("0") | Some("false") => Ok(false),
                other => Err(format!("hud-cmd {text}: expected on / off, found {other:?}")),
            }
        };
        let bad = || Err(format!("hud-cmd: unknown words \"{text}\""));
        match words.as_slice() {
            ["show"] => {
                if matches!(self.model.state, MenuState::Wait | MenuState::Out) {
                    self.model.state = MenuState::In;
                }
            }
            ["hide"] => {
                self.model.state = MenuState::Wait;
                self.model.target = None;
            }
            ["up"] => self.key(MenuKey::Up),
            ["down"] => self.key(MenuKey::Down),
            ["left"] => self.key(MenuKey::Left),
            ["right"] => self.key(MenuKey::Right),
            ["confirm"] => self.key(MenuKey::Confirm),
            ["cancel"] => self.key(MenuKey::Cancel),
            ["open", list] => {
                let list = match *list {
                    "magic" => 1,
                    "items" | "item" => 2,
                    "link" => 3,
                    _ => return bad(),
                };
                if self.model.state == MenuState::SubSel {
                    self.key(MenuKey::Cancel);
                }
                self.model.state = MenuState::MainSel;
                self.model.main_cursor = list;
                self.key(MenuKey::Confirm);
            }
            ["shortcut", w] => self.key(if on(Some(w))? { MenuKey::ShortcutDown } else { MenuKey::ShortcutUp }),
            ["page", "down"] => self.key(MenuKey::Down),
            ["page", "up"] => self.key(MenuKey::Up),
            ["face", which] => self.key(MenuKey::Face(match *which {
                "right" => 0,
                "top" => 1,
                "left" => 2,
                "bottom" => 3,
                _ => return bad(),
            })),
            ["situation", "add", name, rest @ ..] => {
                let seconds = rest.first().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let icon = rest.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
                self.add_situation(&name.replace('_', " "), seconds, icon);
            }
            ["situation", "use"] => self.key(MenuKey::SituationUse),
            ["situation", "switch"] => self.key(MenuKey::SituationSwitch),
            ["situation", "break"] => {
                if let Some(at) = self.model.situation_selected.filter(|i| *i < self.model.situations.len()) {
                    let broken = self.model.situations.remove(at);
                    self.model.events.push(MenuEvent::SituationBroken { serial: broken.serial });
                }
            }
            ["situation", "clear"] => self.model.situations.clear(),
            ["event", "add", name] => self.model.event_button = Some(EventButton { name: name.replace('_', " "), has_gauge: false, slot_b: false }),
            ["event", "use"] => {
                if self.model.event_button.take().is_some() {
                    self.model.events.push(MenuEvent::EventButtonUsed { form_change: false });
                }
            }
            ["battle", n] => self.model.battle_mode = n.parse::<i32>().map_err(|e| e.to_string())?.clamp(0, 2),
            ["link", w] => {
                if on(Some(w))? {
                    if self.lists[3].is_empty() {
                        self.lists[3].push(Entry { name: "Meow Wow Balloon".into(), mp: 0, quantity: 0, targeted: false });
                    }
                } else {
                    self.lists[3].clear();
                    self.model.main_cursor = self.model.main_cursor.min(2);
                }
            }
            ["party", w] => self.party = if on(Some(w))? { vec!["Sora".into(), "Donald".into(), "Goofy".into()] } else { vec!["Sora".into()] },
            ["mp", n, rest @ ..] => {
                self.mp = n.parse().map_err(|_| format!("hud-cmd {text}: MP is a number"))?;
                if let Some(max) = rest.first().and_then(|m| m.parse().ok()) {
                    self.mp_max = max;
                }
            }
            ["mpcharge", w] => self.mp_charging = on(Some(w))?,
            ["style", "off"] => {
                self.model.style_ratio = None;
                self.model.situations.retain(|s| s.icon_type != ACTION_COMMAND_TYPE_STYLE_POINT_GAUGE);
            }
            ["style", ratio] => {
                let ratio: f32 = ratio.parse().map_err(|_| format!("hud-cmd {text}: the ratio is a number"))?;
                if ratio >= 1.0 {
                    // Full: the gauge entry is removed (the form's situation command takes over).
                    self.model.style_ratio = None;
                    self.model.situations.retain(|s| s.icon_type != ACTION_COMMAND_TYPE_STYLE_POINT_GAUGE);
                } else {
                    if !self.model.situations.iter().any(|s| s.icon_type == ACTION_COMMAND_TYPE_STYLE_POINT_GAUGE) {
                        // Style-gauge entries sit below the others.
                        let serial = self.next_serial;
                        self.next_serial += 1;
                        self.model.situations.insert(0, Situation { serial, icon_type: ACTION_COMMAND_TYPE_STYLE_POINT_GAUGE, enabled: true, ..Situation::default() });
                    }
                    self.model.style_ratio = Some(ratio.max(0.0));
                }
            }
            ["shotlock", "enable", row, w] => {
                let row: usize = row.parse().map_err(|_| format!("hud-cmd {text}: the row is 0 or 1"))?;
                let value = on(Some(w))?;
                if let Some(flow) = &mut self.model.shoot_flow {
                    flow.enabled[row.min(1)] = value;
                }
            }
            ["shotlock", w] => {
                if on(Some(w))? {
                    if self.model.shoot_flow.is_none() {
                        self.return_state = if self.model.state == MenuState::ShootFlow { self.return_state } else { MenuState::MainSel };
                        self.model.shoot_flow = Some(ShootFlow { rows: self.shoot_rows.clone(), enabled: [true, true], from_shortcut: false });
                        self.model.state = MenuState::ShootFlow;
                    }
                } else if self.model.shoot_flow.take().is_some() {
                    self.model.state = MenuState::MainSel;
                }
            }
            ["form", w] => {
                self.model.mode = on(Some(w))?.then(|| ModePanel { style: 1, icon_type: 1, has_timer: true, rows: vec![("Finish".to_string(), BUTTON_TEXT_UP.to_string()), ("Cancel".to_string(), BUTTON_TEXT_L1.to_string())] });
                self.model.form_gauge = self.model.mode.as_ref().map(|_| (30.0, 30.0));
            }
            ["simple", w] => self.model.simple_hud = on(Some(w))?,
            _ => return bad(),
        }
        self.refresh();
        Ok(())
    }
}
