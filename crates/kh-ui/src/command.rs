//! The command menu's DISPLAY SIDE (docs/sora-ui-spec.md section 5).
//!
//! In the game `UTresUIP_HudCommand` is both the command system (state machine, pad, lists,
//! execution) and its display. The LOGIC belongs to the command / magic / combat implementers
//! and will live in `kh-core`; this file is the seam and the display half:
//!
//! - [`CommandModel`]: plain data that says what the menu shows now: the state (the game's 13
//!   states), the main slots, the open list and its rows, the shortcut panel, the situation
//!   commands with their times, the shotlock panel, plus the one-shot facts of this frame
//!   ([`MenuEvent`]) that are not visible in the state itself (a refused confirm, how a list
//!   was closed, a situation command used or broken). A command system fills one per frame.
//! - [`CommandDisplay`]: compares the model with what it sent before and makes the ActionScript
//!   calls and UI sounds of the display contract (5.3 - 5.9), in the per-frame order of the
//!   part's update `0x14162FF80` (5.2).
//!
//! Display contract per state (5.2's table):
//! `In` (0): after 0.01 s `initMainCommand` + `setState(0 | 2 | 3, simpleHud)`; `MainSel` (1):
//! cursor, battle colour, slot enable; `SubSel` (2): the list (and the party target list);
//! `Shortcut` (3): the shortcut panel; `StyleWait` (4): as MainSel (the menu waits for a style
//! change, nothing is sent); `Attraction` (5): the mode panel through `setModeIn` / `setModeOut`;
//! `ShootFlow` (6): the shotlock panel; `Vehicle` (8): entered with `setState(3)`, then as
//! MainSel; `Out` (10) / `Wait` (11): root `off`; `Action` (7), `Trinity` (9), `Hologram` (12):
//! no display call is known (the spec did not read them): the display keeps what it shows.

use crate::frames::{form_gauge_frame, situation_digits, situation_gauge_frame};
use crate::hud_command::{self as hc, HudCommand};
use crate::UiSound;

/// Static SE indices of the command menu (spec section 9).
pub const SE_CURSOR: u32 = 0;
pub const SE_CANCEL: u32 = 3;
pub const SE_REFUSED: u32 = 4;
pub const SE_OPEN: u32 = 6;

/// The 13 states of the part (table built by `0x14161E540`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MenuState {
    In = 0,
    MainSel = 1,
    SubSel = 2,
    Shortcut = 3,
    StyleWait = 4,
    Attraction = 5,
    ShootFlow = 6,
    Action = 7,
    Vehicle = 8,
    Trinity = 9,
    Out = 10,
    /// The menu is hidden (the cockpit's hide puts it here).
    #[default]
    Wait = 11,
    Hologram = 12,
}

/// One row: a main slot, a list row, a shortcut entry.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct MenuRow {
    pub text: String,
    /// False: greyed (`disabled`).
    pub enabled: bool,
    /// The command costs more MP than he has (`col2`): it can still be cast.
    pub mp_short: bool,
    /// Pouch quantity of an item row (0 = not an item: no number is ever shown).
    pub quantity: u32,
    /// The row's change flash (Sneeze replaced it, a renamed slot).
    pub flash: bool,
    /// (GOOFY) A shortcut entry that is recharging (THE USER 2026-11-02: "...introduce a cooldown with a visible timer"):
    /// (seconds left, its full length); (0, 0) = ready. Drawn by [`CommandDisplay`] on the panel. `enabled` leaves the wait
    /// out, so a waiting entry stays lit over its gauge, as KH3 draws a timed command (grey text and a greyed count would not
    /// read on the gauge); whatever else greys it still does.
    pub cooldown: (f32, f32),
    /// (Summon) A list row's own number text, shown in KH3's sub-row number field (where an item's "x3" stands) instead
    /// of the quantity text when set (the sub lists only; the shortcut panel keeps its own numbers): a spirit ash's MP cost (THE USER 2026-11-02: "these equipped spirit ashes are what appears
    /// in the Summon command menu"; the cost there is COMPOSED, KH3's rows show no cost).
    pub number: Option<String>,
    /// (ASH-ROWS, THE USER 2026-11-05: "make the bubble dynamically extend to show the whole name (only for spirit ashes)")
    /// A row of the Summon list built from an equipped spirit ash (the host's flag, from the row's command kind): its
    /// plate grows to show the whole name before its number ([`ash_row_width`]) instead of the name being cut.
    pub spirit_ash: bool,
}

/// (GOOFY) The seconds of a recharging entry in its number field (where an item's "x3" stands), counted up to the whole
/// second as a cooldown is: "6" just used, "1" in its last second. Whole seconds fit the field; seconds and tenths overran
/// the entry's plate.
pub fn recharge_text(left: f32) -> String {
    format!("{}", left.max(0.0).ceil() as u32)
}

/// (GOOFY) The colour of KH3's command timer gauge on a recharging entry: `actGg`'s `magic` (KH3's violet of a grand-magic
/// situation command): the white name and the count read on it, and it is not the orange of a live situation command.
pub const RECHARGE_COLOR: &str = "magic";

impl MenuRow {
    pub fn new(text: &str) -> MenuRow {
        MenuRow { text: text.to_string(), enabled: true, ..MenuRow::default() }
    }
}

/// The open list's row texts; a row with its own number (MenuRow::number) fitted before it ([`fit_before_number`]).
/// (ASH-ROWS) A list with spirit-ash rows first widens them ([`ash_row_width`]): every ash row of the list takes the widest
/// one's width (KH3's lists are one column of equal plates, their numbers one column), its whole name shown; every other
/// row clip of the list is set back to its authored width (the clips are shared by all lists).
fn sub_texts(hud: &mut HudCommand, model: &CommandModel) -> Vec<String> {
    let list = model.list;
    let rows: Vec<&MenuRow> = model.lists[list].iter().take(hc::SUB_COMMAND_MAX).collect();
    let wide = rows.iter().enumerate().filter(|(_, r)| r.spirit_ash).filter_map(|(row, r)| r.number.as_deref().map(|n| ash_row_width(hud, list, row, &r.text, n))).fold(AshWidth::default(), |a, b| AshWidth { name: a.name.max(b.name), plate: a.plate.max(b.plate) });
    let room: Vec<Option<NameRoom>> = rows.iter().enumerate().map(|(row, r)| r.number.as_deref().and_then(|n| name_room(hud, list, row, n))).collect();
    for row in 0..hc::SUB_COMMAND_MAX {
        let by = if rows.get(row).is_some_and(|r| r.spirit_ash) { wide } else { AshWidth::default() };
        hud.set_sub_row_width(list as i32, row, by.plate, by.name);
    }
    rows.iter()
        .enumerate()
        .map(|(row, r)| match (r.number.as_deref(), room[row]) {
            (Some(_), Some(at)) => cut_to_room(hud, &r.text, at.room + if r.spirit_ash { wide.name / at.scale } else { 0.0 }),
            _ => r.text.clone(),
        })
        .collect()
}

/// (Summon, ASH-ROWS) THE ROOM A ROW'S NAME HAS BEFORE ITS NUMBER, at the row's authored width ([`NameRoom`]), measured from the movie's own fields (the row clip's `cmdText1.txCmdName...coCmdName`
/// and `cmdText1.cmdNumber...cmdNumberTxt`, their matrices and the fonts): from the name's left edge to a space before the
/// number's text (its left edge by the field's alignment, 2 px gutter as Flash lays a field out). None without the fields
/// or the fonts.
fn name_room(hud: &HudCommand, list: usize, row: usize, number: &str) -> Option<NameRoom> {
    let clip = hud.sub_row_clip(list as i32, row);
    // the matrix from the row's text clip (`cmdText1`) down to a field, and the field's definition
    let field = |rel: &str| -> Option<(crate::geom::Matrix, std::sync::Arc<crate::def::TextDef>)> {
        let mut path = format!("{clip}.cmdText1");
        let mut m = crate::geom::Matrix::IDENTITY;
        // (both are KH3's `TextFieldAdjust` clips: the text is their child `textField`)
        for part in rel.split('.').chain(["textField"]) {
            path = format!("{path}.{part}");
            let node = hud.movie.node(&path)?;
            m = m.mul(&node.matrix);
            if let crate::player::Kind::Text(t) = &node.kind {
                return Some((m, t.def.clone()));
            }
        }
        None
    };
    let ((name_m, name_def), (num_m, num_def)) = (field("txCmdName.txCmdNameCol.coCmdName")?, field("cmdNumber.cmdNumCol.txCmdNumber.cmdNumberTxt")?);
    let (name_font, num_font) = (hud.fonts.get(name_def.font_class.as_deref())?, hud.fonts.get(num_def.font_class.as_deref())?);
    let num_w = num_font.text_width(number, num_def.size);
    let num_x = match num_def.align {
        crate::def::Align::Left => num_def.bounds.x0 + 2.0,
        crate::def::Align::Right => num_def.bounds.x1 - 2.0 - num_w,
        crate::def::Align::Center => (num_def.bounds.x0 + num_def.bounds.x1 - num_w) / 2.0,
    };
    // (ASH-ROWS) at the authored width: a widened row's number stands `sub_row_number_shift` further right
    let shift = hud.sub_row_number_shift(list as i32, row);
    let num_left = num_m.apply(num_x, 0.0).0 - shift;
    let name_left = name_m.apply(name_def.bounds.x0 + 2.0, 0.0).0;
    let scale = name_m.a.abs().max(1.0e-3);
    let gap = name_font.text_width(" ", name_def.size);
    // the field's right end (its 2 px gutter): an "x3" ends well inside it, a three-digit cost runs past
    let overflow = (num_m.apply(num_x + num_w, 0.0).0 - num_m.apply(num_def.bounds.x1 - 2.0, 0.0).0).max(0.0);
    Some(NameRoom { room: (num_left - name_left) / scale - gap, scale, overflow })
}

/// (ASH-ROWS) What [`name_room`] measures: the room in the name field's own units, the field's x scale, and how far the
/// number's text runs past its field's right end (row pixels).
#[derive(Clone, Copy, Debug)]
struct NameRoom {
    room: f32,
    scale: f32,
    overflow: f32,
}

/// (ASH-ROWS) How much wider a spirit-ash row is drawn ([`ash_row_width`]), row pixels: `name` = the name's growth (the
/// number moves right by it), `plate` = the plate's (the name's growth and the count's overrun of its field).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AshWidth {
    pub name: f32,
    pub plate: f32,
}

/// The name field's text width (its font and size; row 0's field stands for every row: they are one symbol).
fn name_width(hud: &HudCommand, name: &str) -> Option<f32> {
    let clip = hud.sub_row_clip(1, 0);
    let Some(crate::player::Node { kind: crate::player::Kind::Text(t), .. }) = hud.movie.node(&format!("{clip}.cmdText1.txCmdName.txCmdNameCol.coCmdName.textField")) else { return None };
    hud.fonts.get(t.def.font_class.as_deref()).map(|font| font.text_width(name, t.def.size))
}

/// A name cut with "..." (as `render::fit_text` ends a cut text) so it fits `room`, its upgrade level (" +5") kept whole.
fn cut_to_room(hud: &HudCommand, name: &str, room: f32) -> String {
    let width = |t: &str| name_width(hud, t).unwrap_or(0.0);
    if room <= 0.0 || width(name) <= room {
        return name.to_string();
    }
    // the upgrade level (" +5") is kept whole: the cut falls in the name before it
    let (head, tail) = match name.rsplit_once(" +") {
        Some((head, level)) if !level.is_empty() && level.chars().all(|c| c.is_ascii_digit()) => (head, format!(" +{level}")),
        _ => (name, String::new()),
    };
    let mut chars: Vec<char> = head.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let cut = format!("{}...{tail}", chars.iter().collect::<String>().trim_end());
        if width(&cut) <= room {
            return cut;
        }
    }
    name.to_string()
}

/// (Summon) A row name that would run into the row's number: KH3's number field sits at a fixed place on the plate and
/// KH3's names with a count are short ("Potion" x3), but Elden Ring's spirit-ash names are not ("Black Knife Tiche +5"
/// ran into its "66" and read "+566"; KH3's `TextFieldAdjust` squeeze to 0.8 only fits the name to its own field, which
/// reaches under the number). The name is cut with "..." (as `render::fit_text` ends a cut text), its " +N" level kept,
/// so it ends a space before the number's text ([`name_room`]). LOCAL: KH3's native code was not read for such a case.
/// Without the fields or the font: the name unchanged. (ASH-ROWS) Spirit-ash rows are widened instead
/// ([`ash_row_width`]); this stays for any other row with its own number.
pub fn fit_before_number(hud: &HudCommand, list: usize, row: usize, name: &str, number: &str) -> String {
    name_room(hud, list, row, number).map_or_else(|| name.to_string(), |at| cut_to_room(hud, name, at.room))
}

/// (ASH-ROWS) The most a spirit-ash row's plate grows past its authored 246 px (a name longer than that is cut with "..."
/// before the number as before; none of Elden Ring's ashes is: "Nightmaiden & Swordstress Puppets +10", the longest,
/// needs far less). Chosen so the widest plate stays left of the stage's middle at the Summon list's place, clear of the
/// party's and the spirit's gauges in the bottom right at every aspect ratio (the stage keeps 16:9).
pub const ASH_ROW_MAX_EXTRA: f32 = 320.0;

/// (ASH-ROWS, THE USER 2026-11-05: "For the ash summons, make the bubble dynamically extend to show the whole name (only
/// for spirit ashes)") HOW MUCH WIDER a spirit-ash row is drawn (row pixels, whole; 0 = the name fits the authored plate):
/// what the whole name needs past its room before the number ([`name_room`]): the number stands a space after the name
/// and the plate's end as far after the number as KH3's; a count wider than its field (a three-digit cost) widens the
/// plate by its overrun as well. Held to [`ASH_ROW_MAX_EXTRA`]. LOCAL: KH3's rows have fixed plates.
pub fn ash_row_width(hud: &HudCommand, list: usize, row: usize, name: &str, number: &str) -> AshWidth {
    let (Some(at), Some(width)) = (name_room(hud, list, row, number), name_width(hud, name)) else { return AshWidth::default() };
    let grow = ((width - at.room) * at.scale).max(0.0).ceil().min(ASH_ROW_MAX_EXTRA);
    AshWidth { name: grow, plate: grow + at.overflow.ceil() }
}

// How a sub list was left (decides the call and the sound).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseKind {
    /// Cancel: `setSubCommand(2, 0)`, the main cursor returns to Attack, static SE 3.
    Cancel,
    /// Left: `setSubCommand(1, 0)`, the main cursor stays, static SE 3.
    Back,
    /// A command was executed from it: `setSubCommand(3, 0)`, no UI sound.
    Decide,
}

/// One-shot facts of a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuEvent {
    /// A refused confirm / empty shortcut entry: static SE 4.
    Refused,
    SubClosed(CloseKind),
    /// The situation command with this serial was used (triangle).
    SituationUsed { serial: u32, form_change: bool },
    /// Its time was drained (`breakActionSelectButton`).
    SituationBroken { serial: u32 },
    /// A shortcut entry was executed; `style_change` = the command changes the menu style.
    ShortcutExecuted { style_change: bool },
    /// The event button was used / removed.
    EventButtonUsed { form_change: bool },
    /// The keyblade was switched on Attack (`setMainCommand(10)`).
    AttackTextChange,
}

/// A situation command in the list (at most 6).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Situation {
    /// Unique per entry.
    pub serial: u32,
    pub name: String,
    /// 0 none ("?"), 1 wep (form change), 2 atf (attraction), 3 magic (grand magic), 4 friend,
    /// 5 link, 100 = the style-point gauge.
    pub icon_type: i32,
    /// 0 .. 3 = `in` .. `in4`.
    pub in_effect: i32,
    /// Seconds; 0 = no timer.
    pub total: f32,
    pub remaining: f32,
    pub enabled: bool,
    /// Counts up when the same kind is added again (the flash replays).
    pub refreshed: u32,
}

/// The event button (slot A: reaction commands, slot B: finish commands).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EventButton {
    pub name: String,
    pub has_gauge: bool,
    pub slot_b: bool,
}

/// The party target list (second level).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TargetList {
    pub names: Vec<String>,
    pub cursor: usize,
}

/// The shotlock panel (state 6).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ShootFlow {
    /// (text, pad button text) per row, from `MainCommandData` row `SHOOTFLOW` / `FPCHARGE`.
    pub rows: Vec<(String, String)>,
    /// Can shoot / can airstep.
    pub enabled: [bool; 2],
    pub from_shortcut: bool,
}

/// A form / attraction mode panel (state 5; `setModeIn`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ModePanel {
    pub style: i32,
    pub icon_type: i32,
    pub has_timer: bool,
    pub rows: Vec<(String, String)>,
}

/// What the command menu shows.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandModel {
    pub state: MenuState,
    /// Byte +0x6E0.
    pub simple_hud: bool,
    /// 0 field (blue), 1 yellow, 2 battle (red).
    pub battle_mode: i32,
    /// Attack, Magic, Items, Link: text and whether the slot's list has an enabled row.
    pub main: [MenuRow; 4],
    /// The Link list is empty: three slots.
    pub link_none: bool,
    pub main_cursor: usize,
    /// The right arrow of the Attack slot (a second action exists).
    pub attack_right_icon: bool,
    /// (cleanup pass) A bad status has replaced the Attack command (SNEEZE, command kind 21): the
    /// text to show. UI spec 5.3: `setMainCommandTextBadStatus(0, text, on)` (0x141618290).
    pub attack_bad_status: Option<String>,
    /// The open list in `SubSel`: 1 Magic, 2 Items, 3 Link.
    pub list: usize,
    /// Rows of the lists 1 .. 3 (index 0 unused); at most 9 are shown.
    pub lists: [Vec<MenuRow>; 4],
    pub row_cursor: usize,
    pub target: Option<TargetList>,
    /// 0 = A, 1 = B, 2 = C.
    pub shortcut_page: usize,
    /// Rows in the order right, top, left, bottom face button.
    pub shortcut: [MenuRow; 4],
    /// Bottom first.
    pub situations: Vec<Situation>,
    /// Index into `situations` of the active entry; `None` with an event button = the event button.
    pub situation_selected: Option<usize>,
    pub situation_restricted: bool,
    pub situation_show_triangle: bool,
    pub event_button: Option<EventButton>,
    /// Form-change point ratio 0 .. 1 (the style gauge); `None` = no gauge entry.
    pub style_ratio: Option<f32>,
    pub shoot_flow: Option<ShootFlow>,
    pub mode: Option<ModePanel>,
    /// (remaining, total) seconds of the form gauge.
    pub form_gauge: Option<(f32, f32)>,
    /// (F-HUD) The style of the form that is on (its logo on the title plate, the logo gauge): `setStyle` in when it
    /// appears or changes kind, out when it goes ([`crate::forms_hud_style`]).
    pub form_style: Option<crate::forms_hud_style::FormStyle>,
    pub legacy_mode: bool,
    pub events: Vec<MenuEvent>,
}

impl Default for CommandModel {
    fn default() -> Self {
        CommandModel {
            state: MenuState::Wait,
            simple_hud: true,
            battle_mode: 0,
            main: Default::default(),
            link_none: false,
            main_cursor: 0,
            attack_right_icon: false,
            attack_bad_status: None,
            list: 0,
            lists: Default::default(),
            row_cursor: 0,
            target: None,
            shortcut_page: 0,
            shortcut: Default::default(),
            situations: Vec::new(),
            situation_selected: None,
            situation_restricted: false,
            situation_show_triangle: true,
            event_button: None,
            style_ratio: None,
            shoot_flow: None,
            mode: None,
            form_gauge: None,
            form_style: None, // (F-HUD)
            legacy_mode: false,
            events: Vec::new(),
        }
    }
}

/// Texts the display needs from the string tables.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandStrings {
    /// `kh3_system.cockpit.cockpit_0001` = "x{0}".
    pub quantity: String,
    /// `kh3_system.general.general_0001`: an empty shortcut entry.
    pub empty_shortcut: String,
    /// `kh3_system.command.unknown` = "? ? ?".
    pub unknown: String,
}

impl Default for CommandStrings {
    fn default() -> Self {
        CommandStrings { quantity: "x{0}".into(), empty_shortcut: "-".into(), unknown: "? ? ?".into() }
    }
}

impl CommandStrings {
    pub fn load(pack: &mut crate::def::Pack) -> CommandStrings {
        let d = CommandStrings::default();
        CommandStrings {
            quantity: pack.string("kh3_system.cockpit.cockpit_0001").unwrap_or(d.quantity),
            empty_shortcut: pack.string("kh3_system.general.general_0001").unwrap_or(d.empty_shortcut),
            unknown: pack.string("kh3_system.command.unknown").unwrap_or(d.unknown),
        }
    }

    /// The number shown beside an item row (`0x141633B60`): "x{0}" for 2 or more, capped at 9;
    /// nothing for 1 or 0.
    pub fn quantity_text(&self, quantity: u32) -> String {
        if quantity >= 2 {
            self.quantity.replace("{0}", &quantity.min(9).to_string())
        } else {
            String::new()
        }
    }
}

/// Style-gauge phase of a ratio (`0x14162DC40`): 0 below 0.25, 1 below 0.5, 2 below 0.75, 3 below 1.
pub fn style_point_phase(ratio: f32) -> i32 {
    if ratio < 0.25 {
        0
    } else if ratio < 0.5 {
        1
    } else if ratio < 0.75 {
        2
    } else {
        3
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct ShownSituation {
    serial: u32,
    /// AS button index (`addActionSelectButton`'s return).
    button: i32,
    refreshed: u32,
    text_label: &'static str,
}

/// (GOOFY, R19) THE TWO-ROW LIST (Attack / Items): a nameless Magic row under a named Items row. (LEADER RESET, THE USER
/// 2026-11-02) a model with no names at all (the title's frames run the HUD on an empty snapshot) is no two-row list: before,
/// it moved the Items row up for whoever was loaded next.
fn two_rows(model: &CommandModel) -> bool {
    model.main[1].text.is_empty() && !model.main[2].text.is_empty()
}

/// The display half of `UTresUIP_HudCommand`.
#[derive(Clone, Debug, Default)]
pub struct CommandDisplay {
    /// The model as last shown.
    shown: Option<CommandModel>,
    /// Seconds in state In.
    in_time: f32,
    inited: bool,
    main_label: [&'static str; 4],
    row_label: [(&'static str, &'static str); hc::SUB_COMMAND_MAX],
    rows_list: usize,
    numbers: Vec<String>,
    numbers_battle: bool,
    shortcut_label: [(&'static str, &'static str); 4],
    shortcut_numbers: Vec<String>,
    situations: Vec<ShownSituation>,
    mark: Option<(i32, bool, bool, bool)>,
    shown_event: Option<EventButton>,
    style_phase: i32,
    form_gauge_shown: bool,
    hidden: bool,
    /// The bad-status text on the Attack slot (`setMainCommandTextBadStatus`), when shown.
    attack_bad: Option<String>,
    /// (F-HUD) The form style sent with `setStyle` (None = NORMAL).
    style_shown: Option<crate::forms_hud_style::FormStyle>,
}

impl CommandDisplay {
    /// (DON-UI) The situation buttons on show: (serial, AS button slot).
    pub fn situation_buttons(&self) -> Vec<(u32, i32)> {
        self.situations.iter().map(|s| (s.serial, s.button)).collect()
    }

    pub fn new() -> CommandDisplay {
        CommandDisplay { style_phase: -1, hidden: true, ..CommandDisplay::default() }
    }

    fn row_text_label(row: &MenuRow, selected: bool) -> &'static str {
        if !row.enabled {
            "disabled"
        } else if selected {
            "on"
        } else {
            "off"
        }
    }

    /// One update of the part: `dt` seconds, the model of this frame.
    pub fn update(&mut self, hud: &mut HudCommand, model: &CommandModel, strings: &CommandStrings, dt: f32, sounds: &mut Vec<UiSound>) {
        let before = self.shown.take().unwrap_or_default();
        let (was, now) = (before.state, model.state);

        // (1) setLegacyMode when it changed.
        if model.legacy_mode != before.legacy_mode {
            hud.set_legacy_mode(model.legacy_mode);
        }

        // Hide / show (the cockpit's 0x1415849A0): root `off` / `loop`.
        let hide = matches!(now, MenuState::Wait | MenuState::Out);
        if hide != self.hidden {
            self.hidden = hide;
            hud.native_root(if hide { "off" } else { "loop" });
        }

        // (4) the state.
        if now == MenuState::In || (!self.inited && !hide) {
            // State In: after 0.01 s the texts and the first-in animation; then MainSel.
            if was != MenuState::In && self.inited {
                self.in_time = 0.0;
                self.inited = false;
            }
            self.in_time += dt;
            if !self.inited && self.in_time >= 0.01 {
                self.inited = true;
                let t: Vec<&str> = model.main.iter().map(|r| if r.text.is_empty() { strings.unknown.as_str() } else { r.text.as_str() }).collect();
                hud.init_main_command(t[0], t[1], t[2], t[3]);
                hud.set_main_two_rows(two_rows(model)); // (LEADER RESET) the list's shape from its first frame
                let mode = match (now, model.mode.is_some() || model.form_style.is_some()) { // (F-HUD) a form is on: style in
                    (MenuState::Vehicle, _) => hc::STATE_MODE_VEHICLE_IN,
                    (_, true) => hc::STATE_MODE_STYLE_IN,
                    _ => hc::STATE_MODE_IN,
                };
                hud.set_state(mode, model.simple_hud);
                self.main_label = ["", "", "", ""];
                // setState selects slot 0; a model whose cursor is elsewhere moves it next.
                if model.main_cursor != 0 {
                    hud.select_main_command(model.main_cursor as i32, -1, false);
                }
                // A model that starts without a link (Sora alone): `SetLinkCommandNone(true)`
                // hides slot 4 and the frame shows three rows (spec 5.3).
                if model.link_none {
                    hud.set_link_command_none(true);
                }
            }
        } else if self.inited {
            for (slot, (a, b)) in before.main.iter().zip(&model.main).enumerate() {
                if a.text != b.text {
                    hud.set_main_command_text(slot as i32, &b.text, b.flash);
                }
            }
            hud.set_main_two_rows(two_rows(model)); // (GOOFY, R19) Attack / Items: a nameless Magic row is hidden
            if model.link_none != before.link_none {
                hud.set_link_command_none(model.link_none);
            }
            // Bad status on Attack (spec 5.3, 0x141618290): `on` = 1 plays `in2` on the slot's flash
            // clip with the status command's name, 0 plays `out` and the slot's own text returns.
            if model.attack_bad_status != self.attack_bad {
                match &model.attack_bad_status {
                    Some(text) => hud.set_main_command_text_bad_status(0, text, 1),
                    None => hud.set_main_command_text_bad_status(0, &model.main[0].text, 0),
                };
                self.attack_bad = model.attack_bad_status.clone();
            }
            for event in &model.events {
                match event {
                    MenuEvent::Refused => sounds.push(UiSound::Play(SE_REFUSED)),
                    MenuEvent::AttackTextChange => hud.set_main_command(hc::MAIN_COMMAND_ATTACK_TEXT_CHANGE),
                    _ => {}
                }
            }
            self.sub_lists(hud, &before, model, sounds);
            self.shortcuts(hud, &before, model, strings, sounds);
            self.shoot_flow(hud, &before, model);
            self.mode_panel(hud, &before, model);
            // Main cursor (0x141617D40): only while no list hides the slots.
            if model.main_cursor != before.main_cursor && matches!(now, MenuState::MainSel | MenuState::StyleWait | MenuState::Vehicle) && hud.level() == 0 {
                hud.select_main_command(model.main_cursor as i32, -1, false);
                if was == now {
                    sounds.push(UiSound::Play(SE_CURSOR));
                }
            } else if hud.level() == 0 && hud.main_cursor() != model.main_cursor as i32 && matches!(now, MenuState::MainSel) {
                hud.select_main_command(model.main_cursor as i32, -1, false);
            }
            if model.attack_right_icon != before.attack_right_icon {
                hud.set_main_command_right_icon(0, model.attack_right_icon);
            }
        }

        if self.inited {
            // (F-HUD) the form style in / out: the formchange (or its end) the state update of this frame carried out
            // (0x14162EF30 / 0x14161C3F0, called from the command's execution inside step (4)).
            self.form_style(hud, model);
            // (5) setBattleMode: every change; keepTitle while the title shows something else.
            if model.battle_mode != before.battle_mode || before.state == MenuState::In || (was == MenuState::Wait && !hide) {
                let keep_title = matches!(now, MenuState::Shortcut | MenuState::ShootFlow) || model.mode.is_some();
                hud.set_battle_mode(model.battle_mode, keep_title);
                self.numbers.clear();
                self.shortcut_numbers.clear();
            }
            // (6) main and sub row enable / colour (0x141632660).
            self.row_states(hud, model);
            // (7) shortcut rows while the panel is open (0x141632AB0).
            if now == MenuState::Shortcut {
                self.shortcut_rows(hud, model, strings);
            }
            // (8) situation commands: list, timers, (10) the mark.
            self.situation_commands(hud, model, sounds);
            // (9) form gauge (0x1416318A0).
            match model.form_gauge {
                Some((remaining, total)) => {
                    hud.native_form_gauge(form_gauge_frame(remaining, total), if remaining < 10.0 { "alert" } else { "loop" });
                    self.form_gauge_shown = true;
                }
                None if self.form_gauge_shown => {
                    // (F-HUD) not when the style's end already took the gauge out (`setStyle(0, ...)`: out / off)
                    if !matches!(hud.movie.current_label("cmdMenu.cmdActLogoGg.cmdActLogoGgShort"), Some("out" | "off")) {
                        hud.hide_logo_gauge(true);
                    }
                    self.form_gauge_shown = false;
                }
                None => {}
            }
            // (11) numbers (0x141633B60) when dirty.
            self.sub_numbers(hud, model, strings);
        }
        let mut shown = model.clone();
        shown.events.clear();
        self.shown = Some(shown);
    }

    /// (F-HUD) The form style (docs/sora-ui-spec.md 5.8, `forms_hud_style` for the trace). In: `setStyle(styleIn,
    /// styleOut, 1, iconType, total > 0, logoPath)` (styleOut = the form it replaces, else 0 = NORMAL), then
    /// `setActionSelect(0 | 12)`, `(10)` and, for a first style, `(2)` (0x14162EF30). Out: `setStyle(0, styleOut, mode)`
    /// (0x14161C3F0); the logo gauge goes with it (so step (9) does not hide it again).
    fn form_style(&mut self, hud: &mut HudCommand, model: &CommandModel) {
        if self.style_shown.as_ref().map(|s| &s.kind) == model.form_style.as_ref().map(|s| &s.kind) {
            return;
        }
        let before = self.style_shown.take();
        match &model.form_style {
            Some(style) => {
                if let Some(logo) = &style.logo {
                    hud.provide_logo(logo.def.clone());
                }
                let out = before.as_ref().map_or(0, |s| s.style);
                hud.set_style(style.style, out, hc::StyleArgs { a3: 1, a4: style.icon_type, a5: style.total > 0.0, a6: style.logo_path.as_deref(), a7: 0 });
                hud.set_action_select(if model.situation_restricted { hc::ACTION_COMMAND_DISABLE_IN } else { hc::ACTION_COMMAND_IN }, 0, 0);
                hud.set_action_select(hc::ACTION_COMMAND_FINISH_IN, 0, 0);
                if before.is_none() {
                    hud.set_action_select(hc::ACTION_COMMAND_PUSH_UP, 0, 0);
                }
                self.style_shown = Some(style.clone());
            }
            None => {
                if let Some(old) = before {
                    hud.set_style(0, old.style, hc::StyleArgs { a3: crate::forms_hud_style::style_out_mode(model.state), ..hc::StyleArgs::default() });
                }
                self.form_gauge_shown = false;
            }
        }
    }

    fn sub_lists(&mut self, hud: &mut HudCommand, before: &CommandModel, model: &CommandModel, sounds: &mut Vec<UiSound>) {
        let (was, now) = (before.state == MenuState::SubSel, model.state == MenuState::SubSel);
        let rows = |m: &CommandModel| m.lists.get(m.list).map_or(0, |l| l.len().min(hc::SUB_COMMAND_MAX));
        if now && (!was || model.list != before.list) {
            // State 1 -> 2 (0x14162DDB0): the list's size, its texts, then the in animation.
            if hud.main_cursor() != model.list as i32 {
                hud.select_main_command(model.list as i32, -1, false);
            }
            hud.init_sub_command(model.list as i32, rows(model) as i32);
            let texts = sub_texts(hud, model);
            let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
            hud.set_sub_command_text(&texts);
            hud.set_sub_command(hc::SUB_COMMAND_IN, model.row_cursor as i32);
            sounds.push(UiSound::Play(SE_OPEN));
            self.row_label = [("", ""); hc::SUB_COMMAND_MAX];
            self.rows_list = model.list;
            self.numbers.clear();
            return;
        }
        if was && !now {
            let kind = model.events.iter().find_map(|e| if let MenuEvent::SubClosed(k) = e { Some(*k) } else { None }).unwrap_or(CloseKind::Cancel);
            if before.target.is_some() && kind != CloseKind::Decide {
                hud.set_sub_command(hc::SUB_COMMAND_OUT, 0);
            }
            match kind {
                CloseKind::Cancel => {
                    hud.set_sub_command(hc::SUB_COMMAND_CANCEL_OUT, 0);
                    sounds.push(UiSound::Play(SE_CANCEL));
                }
                CloseKind::Back => {
                    hud.set_sub_command(hc::SUB_COMMAND_OUT, 0);
                    sounds.push(UiSound::Play(SE_CANCEL));
                }
                CloseKind::Decide => hud.set_sub_command(hc::SUB_COMMAND_DECIDE_OUT, 0),
            }
            self.main_label = ["", "", "", ""];
            return;
        }
        if !now {
            return;
        }
        // The party target list (second level).
        match (&before.target, &model.target) {
            (None, Some(target)) => {
                hud.init_sub_command2(hc::SUB_COMMAND_INDEX as i32, target.names.len().min(hc::SUB_COMMAND2_MAX) as i32);
                let row_text = model.lists[model.list].get(model.row_cursor).map_or("", |r| r.text.as_str());
                let names: Vec<&str> = target.names.iter().take(hc::SUB_COMMAND2_MAX).map(String::as_str).collect();
                hud.set_sub_command2_text(row_text, &names);
                // The movie reaches its second level through SUB_COMMAND_IN from level 1 (the
                // AS3 case 4, FINISH_IN, only re-selects the remembered row).
                hud.set_sub_command(hc::SUB_COMMAND_IN, target.cursor as i32);
                sounds.push(UiSound::Play(SE_OPEN));
            }
            (Some(_), None) => {
                hud.set_sub_command(hc::SUB_COMMAND_OUT, 0);
                sounds.push(UiSound::Play(SE_CANCEL));
                self.row_label = [("", ""); hc::SUB_COMMAND_MAX];
            }
            (Some(a), Some(b)) if a.cursor != b.cursor => {
                hud.select_sub_command(b.cursor as i32);
                sounds.push(UiSound::Play(SE_CURSOR));
            }
            (None, None) => {
                if model.lists[model.list] .iter().map(|r| &r.text).ne(before.lists[before.list].iter().map(|r| &r.text)) {
                    // The list was rebuilt while open (0x14161F300).
                    hud.init_sub_command(model.list as i32, rows(model) as i32);
                    let texts = sub_texts(hud, model);
                    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
                    hud.set_sub_command_text(&texts);
                    self.row_label = [("", ""); hc::SUB_COMMAND_MAX];
                    self.numbers.clear();
                }
                if model.row_cursor != before.row_cursor && rows(model) > 1 {
                    // Row cursor (0x141627CF0): wraps both ways; nothing with a single row.
                    hud.select_sub_command(model.row_cursor as i32);
                    sounds.push(UiSound::Play(SE_CURSOR));
                }
            }
            _ => {}
        }
    }

    fn row_states(&mut self, hud: &mut HudCommand, model: &CommandModel) {
        if hud.level() == 0 && !matches!(model.state, MenuState::Shortcut | MenuState::ShootFlow) {
            for slot in 0..4 {
                if slot == 3 && model.link_none {
                    continue;
                }
                let label = Self::row_text_label(&model.main[slot], model.main_cursor == slot);
                if self.main_label[slot] != label {
                    self.main_label[slot] = label;
                    hud.native_main_text(slot as i32, label);
                }
            }
        }
        if model.state == MenuState::SubSel && model.target.is_none() && self.rows_list == model.list {
            for (row, r) in model.lists[model.list].iter().take(hc::SUB_COMMAND_MAX).enumerate() {
                let labels = (Self::row_text_label(r, model.row_cursor == row), if r.mp_short { "col2" } else { "col1" });
                let shown = self.row_label[row];
                // The driver sends when `enabled` or `mpShort` changed (the movie itself moves
                // on / off with the cursor).
                if (shown.0 == "disabled") != (labels.0 == "disabled") || shown.0.is_empty() {
                    hud.native_sub_text(model.list as i32, row, labels.0);
                }
                if shown.1 != labels.1 {
                    hud.native_sub_name(model.list as i32, row, labels.1);
                }
                self.row_label[row] = labels;
            }
        }
    }

    fn sub_numbers(&mut self, hud: &mut HudCommand, model: &CommandModel, strings: &CommandStrings) {
        if model.state != MenuState::SubSel {
            return;
        }
        let rows = &model.lists[model.list];
        // (Summon) a row's own number (a spirit ash's MP cost) before the quantity text
        let numbers: Vec<String> = rows.iter().take(hc::SUB_COMMAND_MAX).map(|r| r.number.clone().unwrap_or_else(|| strings.quantity_text(r.quantity))).collect();
        let battle = model.battle_mode == 2;
        if numbers != self.numbers || battle != self.numbers_battle {
            let refs: Vec<&str> = numbers.iter().map(String::as_str).collect();
            hud.set_sub_command_num_text(model.list as i32, refs.len() as i32, battle, &refs);
            if model.target.is_some() {
                hud.set_sub_command_next_num_text(battle, numbers.get(model.row_cursor).map_or("", String::as_str));
            }
            self.numbers = numbers;
            self.numbers_battle = battle;
        }
    }

    fn shortcut_texts(hud: &mut HudCommand, model: &CommandModel, strings: &CommandStrings) {
        let t: Vec<&str> = model.shortcut.iter().map(|r| if r.text.is_empty() { strings.empty_shortcut.as_str() } else { r.text.as_str() }).collect();
        hud.set_shortcut_text(t[0], t[1], t[2], t[3]);
        let e: Vec<i32> = model.shortcut.iter().map(|r| r.flash as i32).collect();
        hud.set_shortcut_text_status(e[0], e[1], e[2], e[3]);
    }

    fn shortcuts(&mut self, hud: &mut HudCommand, before: &CommandModel, model: &CommandModel, strings: &CommandStrings, sounds: &mut Vec<UiSound>) {
        let (was, now) = (before.state == MenuState::Shortcut, model.state == MenuState::Shortcut);
        if now && !was {
            // Enter (0x14161A830 -> 0x14162CAE0(0)).
            Self::shortcut_texts(hud, model, strings);
            hud.set_mode_shortcut(hc::SHORTCUT_COMMAND_IN, model.shortcut_page as i32);
            hud.set_action_select(hc::ACTION_COMMAND_DISABLE, 0, 0);
            self.shortcut_label = [("", ""); 4];
            self.shortcut_numbers.clear();
        } else if now && model.shortcut_page != before.shortcut_page {
            // D-pad down = page + 1 (op 3), up = page - 1 (op 4), wrapping over 0 .. 2.
            Self::shortcut_texts(hud, model, strings);
            let down = (before.shortcut_page + 1) % 3 == model.shortcut_page;
            hud.set_mode_shortcut(if down { hc::SHORTCUT_COMMAND_PAGE_DOWN } else { hc::SHORTCUT_COMMAND_PAGE_UP }, model.shortcut_page as i32);
            sounds.push(UiSound::Play(SE_OPEN));
            self.shortcut_label = [("", ""); 4];
            self.shortcut_numbers.clear();
        } else if was && !now {
            let executed = model.events.iter().find_map(|e| if let MenuEvent::ShortcutExecuted { style_change } = e { Some(*style_change) } else { None });
            hud.set_mode_shortcut(if executed == Some(true) { hc::SHORTCUT_COMMAND_OUT_EXEC } else { hc::SHORTCUT_COMMAND_OUT }, before.shortcut_page as i32);
            if !model.situation_restricted {
                hud.set_action_select(hc::ACTION_COMMAND_ENABLE, 0, 0);
            }
            self.main_label = ["", "", "", ""];
        }
    }

    fn shortcut_rows(&mut self, hud: &mut HudCommand, model: &CommandModel, strings: &CommandStrings) {
        for (row, r) in model.shortcut.iter().enumerate() {
            let labels = (if r.enabled && !r.text.is_empty() { "on" } else { "disabled" }, if r.mp_short { "col2" } else { "col1" });
            if self.shortcut_label[row].0 != labels.0 {
                hud.native_shortcut_text(row, labels.0);
            }
            if self.shortcut_label[row].1 != labels.1 {
                hud.native_shortcut_name(row, labels.1);
            }
            self.shortcut_label[row] = labels;
        }
        // (GOOFY, THE USER 2026-11-02: "...a cooldown with a visible timer") a recharging entry: KH3's situation timer gauge
        // draining on its plate and its seconds in the entry's own number field (where an item's count stands)
        for (row, r) in model.shortcut.iter().enumerate() {
            let frame = (r.cooldown.0 > 0.0).then(|| situation_gauge_frame(r.cooldown.0, r.cooldown.1.max(r.cooldown.0)));
            hud.set_shortcut_recharge(row, frame, RECHARGE_COLOR);
        }
        let numbers: Vec<String> = model.shortcut.iter().map(|r| if r.cooldown.0 > 0.0 { recharge_text(r.cooldown.0) } else { strings.quantity_text(r.quantity) }).collect();
        if numbers != self.shortcut_numbers {
            let refs: Vec<&str> = numbers.iter().map(String::as_str).collect();
            hud.set_shortcut_command_num_text(model.battle_mode == 2, &refs);
            self.shortcut_numbers = numbers;
        }
    }

    fn shoot_flow(&mut self, hud: &mut HudCommand, before: &CommandModel, model: &CommandModel) {
        match (&before.shoot_flow, &model.shoot_flow) {
            (None, Some(flow)) => {
                hud.set_action_select(hc::ACTION_COMMAND_DISABLE, 0, 0);
                let rows: Vec<(&str, &str)> = flow.rows.iter().map(|(t, b)| (t.as_str(), b.as_str())).collect();
                hud.set_shoot_flow(hc::SHOOT_FLOW_COMMAND_IN, rows.len() as i32, 0, flow.from_shortcut, &rows);
                for (row, enabled) in flow.enabled.iter().enumerate() {
                    if !enabled {
                        hud.set_enable_command_ex_text(row as i32, false, false);
                    }
                }
            }
            (Some(flow), None) => {
                hud.set_shoot_flow(hc::SHOOT_FLOW_COMMAND_OUT, 0, 0, flow.from_shortcut, &[]);
                hud.set_action_select(hc::ACTION_COMMAND_ENABLE, 0, 0);
                self.main_label = ["", "", "", ""];
            }
            (Some(a), Some(b)) => {
                for row in 0..2 {
                    if a.enabled[row] != b.enabled[row] {
                        hud.set_enable_command_ex_text(row as i32, b.enabled[row], false);
                    }
                    if a.rows.get(row).map(|r| &r.0) != b.rows.get(row).map(|r| &r.0) {
                        // MAX reached: the row is renamed with the change flash (0x14158C380).
                        if let Some((text, _)) = b.rows.get(row) {
                            hud.set_mode_command_text(row as i32, text, true);
                        }
                    }
                }
            }
            (None, None) => {}
        }
    }

    fn mode_panel(&mut self, hud: &mut HudCommand, before: &CommandModel, model: &CommandModel) {
        match (&before.mode, &model.mode) {
            (None, Some(mode)) => {
                let rows: Vec<(&str, &str)> = mode.rows.iter().map(|(t, b)| (t.as_str(), b.as_str())).collect();
                hud.set_mode_in(mode.style, rows.len() as i32, mode.icon_type, mode.has_timer, "", &rows);
            }
            (Some(mode), None) => {
                hud.set_mode_out(mode.style);
                hud.set_mode_in(0, 0, 0, false, "", &[]);
            }
            _ => {}
        }
    }

    fn situation_commands(&mut self, hud: &mut HudCommand, model: &CommandModel, sounds: &mut Vec<UiSound>) {
        // Removed entries first (used: `down`, broken: `break`, else expired: `out`).
        let mut position = 0;
        while position < self.situations.len() {
            let serial = self.situations[position].serial;
            if model.situations.iter().any(|s| s.serial == serial) {
                position += 1;
                continue;
            }
            let used = model.events.iter().find_map(|e| match e {
                MenuEvent::SituationUsed { serial: s, form_change } if *s == serial => Some(*form_change),
                _ => None,
            });
            if model.events.iter().any(|e| matches!(e, MenuEvent::SituationBroken { serial: s } if *s == serial)) {
                hud.break_action_select_button(position as i32);
            }
            hud.delete_action_select_button(position as i32, used.is_some(), used.unwrap_or(false));
            self.situations.remove(position);
            self.mark = None;
        }
        // Added entries, at their list position.
        for (position, s) in model.situations.iter().enumerate() {
            match self.situations.iter().position(|shown| shown.serial == s.serial) {
                Some(at) => {
                    if self.situations[at].refreshed != s.refreshed {
                        // The same kind was added again: the time is refreshed, the flash replays.
                        self.situations[at].refreshed = s.refreshed;
                        hud.play_action_command_effect(at as i32);
                    }
                }
                None => {
                    let at = position.min(self.situations.len());
                    let button = hud.add_action_select_button(at as i32, &s.name, s.icon_type, s.in_effect, s.total > 0.0);
                    self.situations.insert(at, ShownSituation { serial: s.serial, button, refreshed: s.refreshed, text_label: "" });
                    self.mark = None;
                    if s.icon_type == hc::ACTION_COMMAND_TYPE_STYLE_POINT_GAUGE {
                        self.style_phase = -1;
                    }
                }
            }
        }
        // Per frame per timed entry (0x141630ED0).
        for (position, shown) in self.situations.iter_mut().enumerate() {
            let Some(s) = model.situations.iter().find(|s| s.serial == shown.serial) else { continue };
            if shown.button < 0 || s.icon_type == hc::ACTION_COMMAND_TYPE_STYLE_POINT_GAUGE {
                continue;
            }
            if s.total > 0.0 {
                let (seconds, tenths) = situation_digits(s.remaining);
                let label = if s.remaining.min(99.9) < 10.0 { "red" } else { "loop" };
                hud.native_action_timer(shown.button, seconds, tenths, label, situation_gauge_frame(s.remaining, s.total));
            }
            let selected = model.situation_selected == Some(position) && !model.situation_restricted;
            let label = if !s.enabled {
                "disabled"
            } else if selected {
                "on"
            } else {
                "off"
            };
            if (shown.text_label == "disabled") != (label == "disabled") {
                hud.native_action_text(shown.button, label);
            }
            shown.text_label = label;
        }
        // The event button.
        let before_event = self.shown_event.clone();
        if before_event != model.event_button {
            if before_event.is_some() {
                let used = model.events.iter().find_map(|e| if let MenuEvent::EventButtonUsed { form_change } = e { Some(*form_change) } else { None });
                hud.delete_action_select_event_button(used.is_some(), used.unwrap_or(false), false);
            }
            if let Some(event) = &model.event_button {
                hud.add_action_select_event_button(&event.name, event.has_gauge, if event.slot_b { 1 } else { 2 });
            }
            self.shown_event = model.event_button.clone();
            self.mark = None;
        }
        // Style gauge (0x14162DC40): a phase change.
        if let Some(ratio) = model.style_ratio {
            let phase = style_point_phase(ratio);
            if phase != self.style_phase && self.situations.iter().zip(&model.situations).any(|(_, s)| s.icon_type == hc::ACTION_COMMAND_TYPE_STYLE_POINT_GAUGE) {
                hud.set_style_point_phase(phase);
                self.style_phase = phase;
            }
        }
        // (10) setActionCommandMark when dirty (0x141631110).
        let index = match (model.situation_selected, &model.event_button) {
            (Some(i), _) => i as i32,
            (None, Some(_)) => -1,
            (None, None) => -2,
        };
        let restricted = model.situation_restricted || matches!(model.state, MenuState::Shortcut | MenuState::ShootFlow);
        let mark = (index, index != -2, restricted, model.situation_show_triangle);
        if self.mark != Some(mark) && (!self.situations.is_empty() || model.event_button.is_some()) {
            let switched = self.mark.is_some_and(|m| m.0 != mark.0 && m.0 >= 0 && mark.0 >= 0);
            hud.set_action_command_mark(mark.0, mark.1, mark.2, mark.3);
            if switched {
                // Switch with the left trigger (0x14162BAF0): only when the entry changed.
                sounds.push(UiSound::Play(SE_CURSOR));
            }
            self.mark = Some(mark);
        }
    }
}
