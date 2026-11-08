//! (F-HUD) The command menu's FORM STYLE: KH3's style in / out with the form logo (`act_logo/*`) on the title plate and
//! the logo gauge (the form's time), for any formchange (the `CHANGE_*` rows of `MainCommandData`).
//!
//! Traced from the native part `UTresUIP_HudCommand` (docs/sora-ui-spec.md 5.8):
//! - STYLE IN `0x14162EF30(part, kind, total, remaining, exKind)` (the cockpit's `0x14158D7D0` with the command kind of the
//!   form being entered): `0x141619E60(part, 1, kind, ...)` gives the entry type (3 = a form: TresCmdData Type 4 / Cate 14)
//!   and the icon type (1 `wep`; 4 `friend` for kind 0x9A, which has no MainCommandData row); an entry (kind +8, type +4,
//!   total +0x14, remaining +0x18, vehicle flag +0x39 = 0) is added to the style list +0x820 (a form already there is
//!   replaced: its kind gives the style out); then `setStyle(styleIn, styleOut, 1, iconType, total > 0, logoPath)` where
//!   `styleIn` / `styleOut` = `+0x50` of the MainCommandData rows of the two kinds (`0x14155DBC0`: the map kind -> row;
//!   no row = 0 = NORMAL) and `logoPath` = the row's `m_SwfAsset` (row +0x20; only passed when not empty); then
//!   `setActionSelect(0 | 12)` (12 while the situation commands are restricted, +0x9EB, `0x141624D70`),
//!   `setActionSelect(10)` and, for a first style (no form was on), `setActionSelect(2)`.
//! - THE STYLE ID `+0x50` is written by the table loader `0x14155F700`: the row's INDEX in the DataTable (row order of
//!   `MainCommandData.uasset`: NORMAL 0, SHOOTFLOW 1 ... CHANGE_DW 12 ...), the map key being the row's command kind.
//! - STYLE OUT `0x14161C3F0(part, kind, exKind)` (from `0x141627870` when the list runs dry, and the cockpit's end calls;
//!   `0x14157B000` is the same for kind 0xE6): the entry is removed and, for a non-vehicle entry,
//!   `setStyle(0, styleOut, mode)` with `styleOut` = the ending form's `+0x50` and `mode` = the menu state (0 unless the
//!   menu is up, +0x738; states 7 and 11 give 1), so that MainSel (1) and SubSel (2) bring "COMMANDS" back
//!   (`setStyleNormal(0)`). (The vehicle branch alone calls `setActionSelect(3)`, `(0)`, `(10)`.)
//! - The logo movie is loaded by the movie itself (`m_actLogoLoader`, `onActLogoLoadComplete`): see
//!   [`crate::hud_command::HudCommand::provide_logo`].
//!
//! Attractions (`AF_*`), friend and Dream Eater rows and vehicles are NOT served here (the user excluded attractions;
//! vehicles are not needed): [`FormStyles::style_of`] answers only `TRES_CMD_KIND_CHANGE_*` kinds.

use std::collections::HashMap;
use std::sync::Arc;

use crate::command::MenuState;
use crate::def::{MovieDef, Pack};

/// The command kinds this module serves: `ETresCommandKind::game_name()` of a formchange starts with it.
pub const FORM_KIND_PREFIX: &str = "TRES_CMD_KIND_CHANGE_";

/// `0x141619E60`: the icon type of a form's style (`LABELS_ACTION_COMMAND_TYPE`): Type 4 / Cate 14 gives 1 (`wep`),
/// kind 0x9A (`CHANGE_A2`) 4 (`friend`).
pub fn icon_type_of(kind: &str) -> i32 {
    if kind == "TRES_CMD_KIND_CHANGE_A2" {
        4
    } else {
        1
    }
}

/// `setStyle`'s third argument when a style ends (`0x14161C3F0`): the menu state, 7 / 11 as 1; 0 while the menu is not
/// up (In, Out, Wait: the part's +0x738 is clear).
pub fn style_out_mode(state: MenuState) -> i32 {
    match state {
        MenuState::In | MenuState::Out | MenuState::Wait => 0,
        MenuState::Action => 1,
        s => s as i32,
    }
}

/// The movie name of a logo asset path (`/Game/UI/Scaleform/01_hud/act_logo/ActLogoK19.ActLogoK19` -> `ActLogoK19`).
pub fn logo_movie_name(path: &str) -> Option<&str> {
    let last = path.rsplit('/').next()?;
    let name = last.split('.').next()?;
    (!name.is_empty()).then_some(name)
}

/// A loaded logo movie (`act_logo/ActLogoKNN`): what the movie's `Loader` gets for the logo path.
#[derive(Clone)]
pub struct LogoMovie {
    /// The pack's movie name (`ActLogoK19`).
    pub name: String,
    pub def: Arc<MovieDef>,
}

impl PartialEq for LogoMovie {
    fn eq(&self, other: &LogoMovie) -> bool {
        self.name == other.name && Arc::ptr_eq(&self.def, &other.def)
    }
}

impl std::fmt::Debug for LogoMovie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LogoMovie({})", self.name)
    }
}

/// The style of the form that is on (a `CommandModel` field): the `MainCommandData` row of its command kind.
#[derive(Clone, Debug, PartialEq)]
pub struct FormStyle {
    /// The command kind, `ETresCommandKind::game_name()`: `TRES_CMD_KIND_CHANGE_L1` (Second Form).
    pub kind: String,
    /// The row name (`CHANGE_L1`).
    pub row: String,
    /// The style id (row +0x50 = the row's index in the table).
    pub style: i32,
    /// `setStyle`'s a4 (1 = `wep`).
    pub icon_type: i32,
    /// The row's `m_SwfAsset` (`setStyle`'s a6); None for a row without a logo.
    pub logo_path: Option<String>,
    /// The logo movie, when the pack holds it.
    pub logo: Option<LogoMovie>,
    /// The form's full time in seconds (`setStyle`'s a5 = `total > 0`: the logo gauge).
    pub total: f32,
}

#[derive(Clone, Debug)]
struct StyleRow {
    row: String,
    kind: String,
    style: i32,
    logo_path: Option<String>,
}

/// The `CHANGE_*` rows of `data/main_command_styles.json` and their logo movies.
#[derive(Clone, Default)]
pub struct FormStyles {
    rows: Vec<StyleRow>,
    logos: HashMap<String, Arc<MovieDef>>,
    /// Logo movies the pack did not have (names), for the host's log.
    pub missing: Vec<String>,
}

impl std::fmt::Debug for FormStyles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FormStyles").field("rows", &self.rows.len()).field("logos", &self.logos.len()).field("missing", &self.missing).finish()
    }
}

impl FormStyles {
    /// The table only (no logo movie is read): [`FormStyles::load_logos`] or [`FormStyles::load`] add them.
    pub fn table(pack: &Pack) -> FormStyles {
        let mut out = FormStyles::default();
        let Ok(data) = pack.data("main_command_styles") else { return out };
        for (index, row) in data["rows"].as_array().into_iter().flatten().enumerate() {
            let kind = row["command_kind"].as_str().unwrap_or("");
            if !kind.starts_with(FORM_KIND_PREFIX) {
                continue;
            }
            let logo_path = (row["title_kind"].as_str() == Some("Logo")).then(|| row["logo_movie"].as_str().map(str::to_string)).flatten().filter(|p| p.len() > 1);
            out.rows.push(StyleRow { row: row["id"].as_str().unwrap_or("").to_string(), kind: kind.to_string(), style: index as i32, logo_path });
        }
        out
    }

    /// The table and every logo movie it names (read once: the host loads this with its HUD).
    pub fn load(pack: &mut Pack) -> FormStyles {
        let mut out = FormStyles::table(pack);
        out.load_logos(pack);
        out
    }

    /// Reads the logo movies of the table's rows (each once).
    pub fn load_logos(&mut self, pack: &mut Pack) {
        let names: Vec<String> = self.rows.iter().filter_map(|r| r.logo_path.as_deref().and_then(logo_movie_name)).map(str::to_string).collect();
        for name in names {
            if self.logos.contains_key(&name) || self.missing.contains(&name) {
                continue;
            }
            match pack.movie(&name) {
                Ok(def) => {
                    self.logos.insert(name, def);
                }
                Err(_) => self.missing.push(name),
            }
        }
    }

    /// The rows served (`CHANGE_*`).
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The style of a formchange command kind (`ETresCommandKind::game_name()`, e.g. `TRES_CMD_KIND_CHANGE_L1`) whose full
    /// time is `total` seconds. None: not a formchange, or the pack has no row for it.
    pub fn style_of(&self, kind: &str, total: f32) -> Option<FormStyle> {
        let row = self.rows.iter().find(|r| r.kind == kind)?;
        let logo = row.logo_path.as_deref().and_then(logo_movie_name).and_then(|name| self.logos.get(name).map(|def| LogoMovie { name: name.to_string(), def: def.clone() }));
        Some(FormStyle { kind: row.kind.clone(), row: row.row.clone(), style: row.style, icon_type: icon_type_of(&row.kind), logo_path: row.logo_path.clone(), logo, total })
    }
}

/// One-off: the style of a formchange kind straight from a pack (reads the table and that row's logo movie).
pub fn style_of(pack: &mut Pack, kind: &str, total: f32) -> Option<FormStyle> {
    let mut styles = FormStyles::table(pack);
    styles.rows.retain(|r| r.kind == kind);
    styles.load_logos(pack);
    styles.style_of(kind, total)
}
