//! (BUTTON-ICONS, THE USER 2026-11-05: remapped buttons and keys must show in the UI and HUD; "make sure playstation and
//! keyboard + mouse buttons are supported too. Let the user choose what buttons to display in the settings menu"; "xbox and
//! playstation, since they are both controllers, use the same mappings as each other") THE BUTTON-ICON STATE OF THE UI,
//! built from the two remap tables (remap.rs), the "Button Icons" setting and the device used last; kh-ui glyphs.rs
//! resolves every `<icon(...)>` through it.
//!
//! THE SETTING (`hud.button_icons` in kh-er.ini, the Display tab's row): 0 Automatic (default), 1 Xbox, 2 PlayStation,
//! 3 Keyboard and Mouse. Automatic follows the device used last, as KH3 PC does (its auto setting asks the connected device,
//! 0x141595EA0 / 0x141595160): a controller gives that controller's family (a Sony pad: PlayStation; any other: Xbox, KH3's
//! default "xboxone"), the keyboard or mouse gives KH3 PC's key glyphs.
//!
//! The controller has ONE table for both families: Xbox and PlayStation art only draw the same physical position
//! differently (A / Cross, B / Circle, X / Square, Y / Triangle, LB / L1, RB / R1, LT / L2, RT / R2, L3, R3, View / touch
//! pad, Menu / OPTIONS: KH3's PS4 set `IconText/ps4`).

use crate::remap::{roles, Device, Out, Table, MOUSE_LEFT, MOUSE_MIDDLE, MOUSE_RIGHT, MOUSE_X1, MOUSE_X2, WHEEL_DOWN, WHEEL_UP};
use kh_core::PadButton;
use kh_ui::glyphs::{Family, GlyphState};

/// kh-er.ini's key of the setting.
pub const SETTING_KEY: &str = "hud.button_icons";

/// The setting's values (the row's stored values).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Automatic = 0,
    Xbox = 1,
    PlayStation = 2,
    Kbm = 3,
}

impl Setting {
    /// The settings file's value (unset / unknown: Automatic).
    pub fn parse(value: Option<&str>) -> Setting {
        match value.and_then(|v| v.trim().parse::<f32>().ok()).map(|v| v.round() as i32) {
            Some(1) => Setting::Xbox,
            Some(2) => Setting::PlayStation,
            Some(3) => Setting::Kbm,
            _ => Setting::Automatic,
        }
    }
}

/// AUTOMATIC: the device used last (the start: a controller of the family found connected, KH3's default Xbox).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Last {
    /// The device used last: a controller family or the keyboard / mouse.
    pub device: Family,
    /// The controller family seen last (the controller art of the keyboard set's sticks and of the controller's table page).
    pub pad: Family,
}

impl Default for Last {
    fn default() -> Last {
        Last { device: Family::Xbox, pad: Family::Xbox }
    }
}

impl Last {
    /// One frame's input: whether the controller / the keyboard or mouse was used, and the connected controller's family
    /// (its art). Both used in one frame: the device stays (a controller device takes the connected family's art).
    pub fn input(&mut self, pad_used: bool, pad_family: Family, kbm_used: bool) {
        self.pad = if pad_family == Family::PlayStation { Family::PlayStation } else { Family::Xbox };
        self.device = match (pad_used, kbm_used) {
            (false, true) => Family::Kbm,
            (true, false) => self.pad,
            _ if self.device == Family::Kbm => Family::Kbm,
            _ => self.pad,
        };
    }
}

/// The set the icons show, and the controller art: (shown, controller family).
pub fn shown(setting: Setting, last: &Last) -> (Family, Family) {
    match setting {
        Setting::Automatic => (last.device, last.pad),
        Setting::Xbox => (Family::Xbox, Family::Xbox),
        Setting::PlayStation => (Family::PlayStation, Family::PlayStation),
        Setting::Kbm => (Family::Kbm, last.pad),
    }
}

/// UE's name of a key or mouse button (the BindKey column of KH3 PC's keyboard glyph table, IconTextData_km), by the remap
/// tables' code (a Windows virtual-key code, the mouse buttons' VK codes, the wheel's two codes). A key UE names but KH3 has
/// no glyph for gets its UE name and draws `Blank` (as KH3 PC); "" for a code with no name.
pub fn ue_key_name(code: u16) -> String {
    let fixed = match code {
        MOUSE_LEFT => "LeftMouseButton",
        MOUSE_RIGHT => "RightMouseButton",
        MOUSE_MIDDLE => "MiddleMouseButton",
        MOUSE_X1 => "ThumbMouseButton",
        MOUSE_X2 => "ThumbMouseButton2",
        WHEEL_UP => "MouseScrollUp",
        WHEEL_DOWN => "MouseScrollDown",
        0x08 => "BackSpace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x10 | 0xA0 => "LeftShift",
        0xA1 => "RightShift",
        0x11 | 0xA2 => "LeftControl",
        0xA3 => "RightControl",
        0x12 | 0xA4 => "LeftAlt",
        0xA5 => "RightAlt",
        0x13 => "Pause",
        0x14 => "CapsLock",
        0x1B => "Escape",
        0x20 => "SpaceBar",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x2D => "Insert",
        0x2E => "Delete",
        0x6A => "Multiply",
        0x6B => "Add",
        0x6D => "Subtract",
        0x6E => "Decimal",
        0x6F => "Divide",
        0x90 => "NumLock",
        0x91 => "ScrollLock",
        0xBA => "Semicolon",
        0xBB => "Equals",
        0xBC => "Comma",
        0xBD => "Hyphen",
        0xBE => "Period",
        0xBF => "Slash",
        0xC0 => "Tilde",
        0xDB => "LeftBracket",
        0xDC => "Backslash",
        0xDD => "RightBracket",
        0xDE => "Apostrophe",
        _ => "",
    };
    if !fixed.is_empty() {
        return fixed.to_string();
    }
    const DIGITS: [&str; 10] = ["Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine"];
    match code {
        0x30..=0x39 => DIGITS[(code - 0x30) as usize].to_string(),
        0x41..=0x5A => char::from(code as u8).to_string(),
        0x60..=0x69 => format!("NumPad{}", DIGITS[(code - 0x60) as usize]),
        0x70..=0x87 => format!("F{}", code - 0x6F),
        _ => String::new(),
    }
}

/// THE STATE THE UI DRAWS WITH: the shown set, the controller art, each position's role holder in the controller's table,
/// and each position's first key in the keyboard / mouse table (KH3 PC shows the first key of an action, FUN_1415a8fc0).
pub fn glyph_state(shown: (Family, Family), pad: &Table, kbm: &Table) -> GlyphState {
    let mut state = GlyphState { family: shown.0, pad_family: shown.1, ..GlyphState::default() };
    if pad.device == Device::Pad {
        for (p, code) in pad.assign.iter().enumerate().take(16) {
            state.pad[p] = (*code).min(15) as u8;
        }
    }
    if kbm.device == Device::Kbm {
        for (p, b) in PadButton::ALL.iter().enumerate() {
            state.keys[p] = roles(Device::Kbm).iter().zip(&kbm.assign).find(|(r, _)| r.out == Out::Button(*b)).map(|(_, c)| ue_key_name(*c)).unwrap_or_default();
        }
        for d in 0..4u8 {
            state.stick[d as usize] = roles(Device::Kbm).iter().zip(&kbm.assign).find(|(r, _)| r.out == Out::Stick(d)).map(|(_, c)| ue_key_name(*c)).unwrap_or_default();
        }
    }
    state
}

/// The icon of one input itself (no role step): the settings screen's button table rows and its swap question.
pub fn input_icon(device: Device, code: u16) -> String {
    match device {
        Device::Pad => kh_ui::glyphs::SMALL.get(code as usize).map_or_else(|| "-".to_string(), |n| format!("<icon(Phys_{n})>")),
        Device::Kbm => match ue_key_name(code) {
            k if k.is_empty() => crate::remap::input_name(device, code),
            k => format!("<icon(KmKey_{k})> {}", crate::remap::input_name(device, code)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_ui::glyphs::{resolve, Part};

    fn pad_row(state: &GlyphState, name: &str) -> String {
        match resolve(state, name).as_slice() {
            [Part::Pad { row, .. }] => row.clone(),
            other => panic!("{name}: {other:?}"),
        }
    }

    #[test]
    fn bi_a_swapped_table_moves_every_confirm_icon_in_both_pad_sets() {
        let mut pad = Table::defaults(Device::Pad);
        pad.assign(PadButton::FaceBottom as usize, PadButton::FaceRight as u16); // Confirm on B (Cancel on A)
        let kbm = Table::defaults(Device::Kbm);
        for family in [Family::Xbox, Family::PlayStation] {
            let s = glyph_state((family, family), &pad, &kbm);
            assert_eq!((pad_row(&s, "Decide"), pad_row(&s, "Cross"), pad_row(&s, "Cancel"), pad_row(&s, "Circle")), ("Circle".into(), "Circle".into(), "Cross".into(), "Cross".into()), "{family:?}");
            assert_eq!((pad_row(&s, "Cross2"), pad_row(&s, "Decide2")), ("Circle2".into(), "Circle2".into()));
            assert_eq!(pad_row(&s, "Triangle"), "Triangle", "an unswapped role keeps its button");
            assert_eq!(pad_row(&s, "Phys_Cross"), "Cross", "the table page shows the button itself");
            match &resolve(&s, "Cross")[0] {
                Part::Pad { family: f, .. } => assert_eq!(*f, family),
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn bi_one_pad_table_is_the_same_positions_in_xbox_and_playstation_art() {
        let mut pad = Table::defaults(Device::Pad);
        pad.assign(PadButton::FaceTop as usize, PadButton::LeftShoulder as u16);
        pad.assign(PadButton::DPadUp as usize, PadButton::SpecialRight as u16);
        let kbm = Table::defaults(Device::Kbm);
        let xb = glyph_state((Family::Xbox, Family::Xbox), &pad, &kbm);
        let ps = glyph_state((Family::PlayStation, Family::PlayStation), &pad, &kbm);
        assert_eq!(xb.pad, ps.pad, "one table");
        for name in ["Triangle", "L1", "KeyUp", "Option", "Cross", "Key", "KeyUpDown", "R2Hold"] {
            let (a, b) = (resolve(&xb, name), resolve(&ps, name));
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(&b) {
                match (a, b) {
                    (Part::Pad { family: fa, row: ra, .. }, Part::Pad { family: fb, row: rb, .. }) => {
                        assert_eq!(ra, rb, "{name}: the same position");
                        assert_eq!((*fa, *fb), (Family::Xbox, Family::PlayStation), "{name}: different art");
                    }
                    other => panic!("{other:?}"),
                }
            }
        }
        assert_eq!(pad_row(&xb, "Triangle"), "L1");
        // the D-pad's combined icon falls apart into its buttons once one of them moved
        assert_eq!(resolve(&xb, "KeyUpDown").len(), 2);
        assert_eq!(pad_row(&xb, "KeyLeftRight"), "KeyLeftRight");
    }

    #[test]
    fn bi_the_keyboard_set_shows_each_roles_first_key() {
        let pad = Table::defaults(Device::Pad);
        let mut kbm = Table::defaults(Device::Kbm);
        let s = glyph_state((Family::Kbm, Family::Xbox), &pad, &kbm);
        let key = |s: &GlyphState, n: &str| match resolve(s, n).as_slice() {
            [Part::KmBind { key, .. }] => key.clone(),
            other => panic!("{n}: {other:?}"),
        };
        assert_eq!((key(&s, "Decide"), key(&s, "Cancel"), key(&s, "Square"), key(&s, "Triangle"), key(&s, "L1"), key(&s, "R1")), ("LeftMouseButton".into(), "SpaceBar".into(), "RightMouseButton".into(), "F".into(), "LeftShift".into(), "R".into()));
        assert_eq!(resolve(&s, "Lstick").len(), 4, "WASD");
        assert_eq!(resolve(&s, "Rstick"), vec![Part::Km { row: "Mouse".into(), size_of: None }]);
        // Confirm swapped to Space: every Confirm icon shows Space
        kbm.assign(0, 0x20);
        let s = glyph_state((Family::Kbm, Family::Xbox), &pad, &kbm);
        assert_eq!((key(&s, "Decide"), key(&s, "Cancel")), ("SpaceBar".into(), "LeftMouseButton".into()));
        assert!(matches!(&resolve(&s, "Decide2")[0], Part::KmBind { big: true, .. }));
    }

    #[test]
    fn bi_automatic_follows_the_last_device() {
        let mut last = Last::default();
        assert_eq!(shown(Setting::Automatic, &last), (Family::Xbox, Family::Xbox), "KH3's default");
        last.input(false, Family::Xbox, true);
        assert_eq!(shown(Setting::Automatic, &last).0, Family::Kbm);
        last.input(true, Family::PlayStation, false);
        assert_eq!(shown(Setting::Automatic, &last), (Family::PlayStation, Family::PlayStation));
        last.input(false, Family::PlayStation, false);
        assert_eq!(shown(Setting::Automatic, &last).0, Family::PlayStation, "no input: no change");
        last.input(false, Family::PlayStation, true);
        assert_eq!(shown(Setting::Automatic, &last), (Family::Kbm, Family::PlayStation), "the controller art stays the pad's");
        last.input(true, Family::Xbox, false);
        assert_eq!(shown(Setting::Automatic, &last).0, Family::Xbox);
        // a fixed choice ignores the device
        assert_eq!(shown(Setting::PlayStation, &last).0, Family::PlayStation);
        assert_eq!(shown(Setting::Kbm, &last).0, Family::Kbm);
        assert_eq!(shown(Setting::Xbox, &Last { device: Family::Kbm, pad: Family::PlayStation }).0, Family::Xbox);
        assert_eq!((Setting::parse(None), Setting::parse(Some("2")), Setting::parse(Some(" 3 ")), Setting::parse(Some("x"))), (Setting::Automatic, Setting::PlayStation, Setting::Kbm, Setting::Automatic));
    }

    #[test]
    fn bi_ue_names_of_the_default_keys() {
        assert_eq!((ue_key_name(0x20), ue_key_name(0x0D), ue_key_name(b'W' as u16), ue_key_name(b'4' as u16), ue_key_name(0x70), ue_key_name(0x63)), ("SpaceBar".into(), "Enter".into(), "W".into(), "Four".into(), "F1".into(), "NumPadThree".into()));
        assert_eq!((ue_key_name(MOUSE_X1), ue_key_name(WHEEL_UP), ue_key_name(0x09), ue_key_name(0xFF)), ("ThumbMouseButton".into(), "MouseScrollUp".into(), "Tab".into(), String::new()));
        assert_eq!(input_icon(Device::Pad, PadButton::FaceRight as u16), "<icon(Phys_Circle)>");
        assert_eq!(input_icon(Device::Kbm, 0x20), "<icon(KmKey_SpaceBar)> Space");
    }
}
