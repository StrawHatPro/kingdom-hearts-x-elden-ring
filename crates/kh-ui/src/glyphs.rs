//! (BUTTON-ICONS, THE USER 2026-11-05: remapped buttons and keys must show in the UI and HUD; "make sure playstation and
//! keyboard + mouse buttons are supported too. Let the user choose what buttons to display in the settings menu"; "xbox and
//! playstation, since they are both controllers, use the same mappings as each other") WHICH GLYPH A BUTTON ICON DRAWS.
//!
//! KH3's text names its button icons by the PlayStation names of the pad's POSITIONS (`<icon(Cross)>` = the bottom face
//! button, `<icon(Triangle)>` the top one, `Decide` / `Cancel` the GamePadDecideType aliases: western layout Cross / Circle,
//! the pack's icon_text.json note). Each such name stands for THE ROLE of that position (the KH3 action its default button
//! plays). KH3 PC builds its keyboard aliases the same way (FUN_1415b4b60: every pad icon name -> the keys bound to the
//! same action -> their keyboard glyph rows by BindKey, FUN_1415a8dd0 / FUN_1415a8fc0 / FUN_14157ee80; no row: `Blank`).
//!
//! So an icon is resolved in two steps, both here and pure ([`resolve`]):
//! 1. the role -> the input that holds it now: the controller's ONE table (shared by the Xbox and PlayStation art: the
//!    user's rule) gives the physical pad position; the keyboard / mouse table gives the key;
//! 2. that input -> its glyph in the set being shown: the Xbox set (`IconText/xboxone`), the PlayStation set
//!    (`IconText/ps4`, KH3's PS4 layout: View = the touch pad, Menu = OPTIONS) or KH3 PC's keyboard and mouse set
//!    (`IconText_km`, the key's row by its BindKey; a key KH3 has no glyph for draws `Blank`, as KH3 PC does).
//!
//! THE STATE ([`GlyphState`]) is the host's (kh-host button_icons.rs builds it from the remap tables, the setting and the
//! last device used); [`set_state`] publishes it and bumps [`generation`], which the renderer's change hashes include, so
//! every text with an icon is drawn again when it changes. Unset (a host that never calls it, the offline sheets of
//! other packages): the identity tables in the pad set the fonts were loaded with: exactly the drawing from before.
//!
//! Names that bypass the role step (the settings screen's button table rows show the button itself, not its role):
//! `Phys_<pad icon name>` (that position's glyph in the shown pad art) and `KmKey_<UE key name>` (that key's glyph).
//!
//! Position numbers are kh-core's `PadButton` order (this crate does not depend on kh-core): 0 bottom face, 1 right face,
//! 2 left face, 3 top face, 4 LB, 5 RB, 6 LT, 7 RT, 8 D-pad up, 9 down, 10 left, 11 right, 12 L3, 13 R3, 14 View, 15 Menu.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The art a button icon is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Xbox,
    PlayStation,
    Kbm,
}

/// Each position's small icon name (the pad sets' rows, PlayStation names of the positions).
pub const SMALL: [&str; 16] = ["Cross", "Circle", "Square", "Triangle", "L1", "R1", "L2", "R2", "KeyUp", "KeyDown", "KeyLeft", "KeyRight", "L3", "R3", "Pad", "Option"];
/// Each position's large (60 x 60) icon, where the pad sets have one.
pub const BIG: [Option<&str>; 16] = [Some("Cross2"), Some("Circle2"), Some("Square2"), Some("Triangle2"), None, None, Some("L22"), Some("R22"), Some("KeyUp2"), Some("KeyDown2"), Some("KeyLeft2"), Some("KeyRight2"), None, None, Some("Pad2"), None];
/// The position of the right trigger (`R2Hold`, the held-trigger art, belongs to it).
const RT: u8 = 7;

/// What the UI draws a button icon as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphState {
    /// The set button icons show.
    pub family: Family,
    /// The controller art (Xbox or PlayStation) of the `Phys_` names (the controller's table page) and of the sticks.
    pub pad_family: Family,
    /// `pad[p]`: the physical position whose button plays position `p`'s role (the controller's table).
    pub pad: [u8; 16],
    /// `keys[p]`: the UE key name (`SpaceBar`, `LeftMouseButton`, ...) of the first key that plays position `p`'s role.
    pub keys: [String; 16],
    /// The left stick's keys: up, down, left, right.
    pub stick: [String; 4],
}

impl Default for GlyphState {
    /// KH3's defaults: the Xbox set, the identity table, KH3 PC's default keys (kh-host remap.rs KBM_ROLES' first key of each).
    fn default() -> GlyphState {
        let k = |s: &str| s.to_string();
        GlyphState {
            family: Family::Xbox,
            pad_family: Family::Xbox,
            pad: std::array::from_fn(|p| p as u8),
            keys: [k("LeftMouseButton"), k("SpaceBar"), k("RightMouseButton"), k("F"), k("LeftShift"), k("R"), k("C"), k("E"), k("Up"), k("Down"), k("Left"), k("Right"), k("X"), k("RightAlt"), k("N"), k("M")],
            stick: [k("W"), k("S"), k("A"), k("D")],
        }
    }
}

static STATE: Mutex<Option<Arc<GlyphState>>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Publishes the state (a no-op when it is the same); every text with an icon is drawn again after a change.
pub fn set_state(state: GlyphState) -> bool {
    let Ok(mut guard) = STATE.lock() else { return false };
    if guard.as_deref() == Some(&state) {
        return false;
    }
    *guard = Some(Arc::new(state));
    GENERATION.fetch_add(1, Ordering::Relaxed);
    true
}

/// The published state (None: no host published one).
pub fn state() -> Option<Arc<GlyphState>> {
    STATE.lock().ok().and_then(|g| g.clone())
}

/// Bumped on every change of the state (the renderer's change hashes include it).
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// One glyph to draw: a row of a set, and the name whose native size it is drawn at (a large icon drawn with a small row:
/// the large icon's size).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    /// A row of a controller set.
    Pad { family: Family, row: String, size_of: Option<String> },
    /// A row of the keyboard and mouse set (`Blank` for a key with no row).
    Km { row: String, size_of: Option<String> },
    /// The key whose row is found by its BindKey (`Blank` when none); `big`: its large row (`<row>_2`) where there is one.
    KmBind { key: String, big: bool, size_of: Option<String> },
    /// Not a button icon (category icons, KH3's own keyboard rows `Kb_Ins` ...): the name itself, as before.
    Plain(String),
}

/// The left / right stick's directions: up, down, left, right.
fn stick_dir(rest: &str) -> Option<Vec<u8>> {
    Some(match rest {
        "" => vec![0, 2, 1, 3],
        "up" | "upgre" => vec![0],
        "down" | "downgre" => vec![1],
        "left" | "leftgre" => vec![2],
        "right" | "rightgre" => vec![3],
        "updown" => vec![0, 1],
        "leftright" => vec![2, 3],
        _ => return None,
    })
}

/// A requested name's meaning.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Req {
    /// A role (position), large or not, the held-trigger art.
    Role { p: u8, big: bool, hold: bool },
    /// The D-pad's combined icons: their parts and the pad sets' own combined row.
    DPad { parts: &'static [u8], row: &'static str },
    /// A stick icon: left (true) or right, its directions, the pad sets' own row.
    Stick { left: bool, dirs: Vec<u8>, row: String },
    /// A physical position (no role step), large or not.
    Phys { p: u8, big: bool },
    /// A keyboard / mouse key by its UE name.
    Key(String),
}

fn req(name: &str) -> Option<Req> {
    let n = name.to_ascii_lowercase();
    if let Some(rest) = n.strip_prefix("phys_") {
        return SMALL.iter().position(|s| s.eq_ignore_ascii_case(rest)).map(|p| Req::Phys { p: p as u8, big: false }).or_else(|| BIG.iter().position(|b| b.is_some_and(|b| b.eq_ignore_ascii_case(rest))).map(|p| Req::Phys { p: p as u8, big: true }));
    }
    if n.starts_with("kmkey_") {
        return Some(Req::Key(name[6..].to_string()));
    }
    let role = |p: u8, big: bool| Some(Req::Role { p, big, hold: false });
    match n.as_str() {
        "decide" => return role(0, false),
        "cancel" => return role(1, false),
        "decide2" => return role(0, true),
        "cancel2" => return role(1, true),
        "sd_option" => return role(15, false),
        "sd_pad" => return role(14, false),
        "r2hold" => return Some(Req::Role { p: RT, big: false, hold: true }),
        "key" => return Some(Req::DPad { parts: &[8, 10, 9, 11], row: "Key" }),
        "keyupdown" => return Some(Req::DPad { parts: &[8, 9], row: "KeyUpDown" }),
        // (forms_hud_operation's guide row spells it so; the pad sets' row is KeyLeftRight)
        "keyleftright" | "keyleftyright" => return Some(Req::DPad { parts: &[10, 11], row: "KeyLeftRight" }),
        _ => {}
    }
    if let Some(p) = SMALL.iter().position(|s| s.eq_ignore_ascii_case(&n)) {
        return role(p as u8, false);
    }
    if let Some(p) = BIG.iter().position(|b| b.is_some_and(|b| b.eq_ignore_ascii_case(&n))) {
        return role(p as u8, true);
    }
    for (prefix, left) in [("lstick", true), ("rstick", false)] {
        if let Some(rest) = n.strip_prefix(prefix) {
            let dirs = stick_dir(rest)?;
            return Some(Req::Stick { left, dirs, row: name.to_string() });
        }
    }
    None
}

/// The pad family of a state's shown set (the keyboard set shows its sticks / physical names in the pad art).
fn pad_family(state: &GlyphState) -> Family {
    match state.family {
        Family::Kbm => state.pad_family,
        f => f,
    }
}

/// THE RESOLVER (pure): what a requested icon name draws under a state, glyph by glyph.
pub fn resolve(state: &GlyphState, name: &str) -> Vec<Part> {
    let Some(r) = req(name) else { return vec![Part::Plain(name.to_string())] };
    let pf = pad_family(state);
    let pad_row = |row: &str, size_of: Option<&str>| Part::Pad { family: pf, row: row.to_string(), size_of: size_of.map(str::to_string) };
    let phys = |p: u8, big: bool, size_of: Option<&str>| -> Part {
        let p = (p as usize).min(15);
        match (big, BIG[p]) {
            (true, Some(b)) => pad_row(b, None),
            (true, None) => pad_row(SMALL[p], size_of),
            (false, _) => pad_row(SMALL[p], None),
        }
    };
    let key = |p: u8, big: bool| -> Part {
        let k = state.keys.get(p as usize).cloned().unwrap_or_default();
        Part::KmBind { key: k, big, size_of: big.then(|| BIG[(p as usize).min(15)].unwrap_or(SMALL[(p as usize).min(15)]).to_string()) }
    };
    match r {
        Req::Phys { p, big } => vec![phys(p, big, BIG[p as usize])],
        Req::Key(k) => vec![Part::KmBind { key: k, big: false, size_of: None }],
        Req::Role { p, big, hold } => match state.family {
            Family::Kbm => vec![key(p, big)],
            _ => {
                let at = state.pad.get(p as usize).copied().unwrap_or(p);
                if hold && at == RT {
                    vec![pad_row("R2Hold", None)]
                } else {
                    vec![phys(at, big, BIG[p as usize].or(Some(SMALL[p as usize])))]
                }
            }
        },
        Req::DPad { parts, row } => match state.family {
            Family::Kbm => parts.iter().map(|q| key(*q, false)).collect(),
            _ if parts.iter().all(|q| state.pad.get(*q as usize) == Some(q)) => vec![pad_row(row, None)],
            _ => parts.iter().map(|q| phys(state.pad.get(*q as usize).copied().unwrap_or(*q), false, None)).collect(),
        },
        Req::Stick { left, dirs, row } => match state.family {
            Family::Kbm if left => dirs.iter().map(|d| Part::KmBind { key: state.stick.get(*d as usize).cloned().unwrap_or_default(), big: false, size_of: None }).collect(),
            // (the right stick is the mouse on the keyboard: KH3 PC registers the mouse as the right stick; its rows
            // Mouse / MouseForward / MouseBack / MouseLeft / MouseRight carry no BindKey, so they are named here)
            Family::Kbm => match dirs.as_slice() {
                [0] => vec![Part::Km { row: "MouseForward".into(), size_of: None }],
                [1] => vec![Part::Km { row: "MouseBack".into(), size_of: None }],
                [2] => vec![Part::Km { row: "MouseLeft".into(), size_of: None }],
                [3] => vec![Part::Km { row: "MouseRight".into(), size_of: None }],
                _ => vec![Part::Km { row: "Mouse".into(), size_of: None }],
            },
            _ => vec![pad_row(&row, None)],
        },
    }
}

/// The published state's resolution (None: no host published one; the caller draws the name as before).
pub fn resolve_now(name: &str) -> Option<Vec<Part>> {
    state().map(|s| resolve(&s, name))
}


/// The language of the keyboard glyphs' picture (`IconText/Localization/<lang>/IconText_km`; KH3 picks it by its culture,
/// FUN_1415a5900). Set BEFORE the fonts are loaded (Elden Ring: `hud.button_icons_lang`; the test app: KH_UI_KM_LANG).
static KM_LANG: Mutex<Option<String>> = Mutex::new(None);

pub fn set_km_language(lang: &str) {
    if let Ok(mut guard) = KM_LANG.lock() {
        *guard = Some(lang.to_string());
    }
}

pub fn km_language() -> String {
    KM_LANG.lock().ok().and_then(|g| g.clone()).or_else(|| std::env::var("KH_UI_KM_LANG").ok()).filter(|s| !s.is_empty()).unwrap_or_else(|| "en".to_string())
}

/// A button icon name (one the resolver maps through the tables; the rest are drawn as named).
pub fn is_button(name: &str) -> bool {
    req(name).is_some()
}
