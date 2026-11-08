//! (CAMP-OPTIONS, THE USER 2026-11-05: "make button remapping possible on both controller and keyboard, using the settings
//! menu"; "don't overcomplicate remapping, each button can be remapped, not each action"; "so A is confirm, but you can make
//! confirm B instead for example"; "two buttons cannot be mapped to different things, they will be swapped in the options
//! menu after the warning box saying what it will be swapped with is confirmed by the player"; "keyboard and mouse defaults
//! should be KH3's keyboard and mouse defaults") THE BUTTON TABLES, one per device, applied at the input layer before any KH
//! logic (kh-er pad.rs / inputs.rs): every gameplay system and every menu sees the remapped buttons.
//!
//! A ROW IS A BUTTON ROLE as KH3 names it (the default button's function: KH3's key-config texts, kh3_settings.keyboard
//! .keyconfig_NNNN), its value the input that now plays that role. Assigning a role an input another role holds SWAPS the
//! two, after the player's Yes to a warning naming both (the controller's table is a permutation of its 16 buttons: every
//! role always has exactly one button); a keyboard key no role holds is simply taken.
//!
//! DEFAULTS: the controller = identity (KH3's Xbox layout); the keyboard and mouse = KH3 PC's own defaults (its executable's
//! default key table FUN_140edf630, the first key of each slot and the second, as KH3's Key Configuration shows them: build/
//! sora_tmp/weapons/CAMP-OPTIONS/re-d/kh3_kbm_defaults.json + NOTES.md). The shortcut combinations (LB + a face button) are
//! KH3's own logic over these roles, so they follow without rows of their own. KH3's "Move camera" rows hold no key (mouse
//! look only) and are not rows here.
//!
//! PERSISTENCE: the mod's settings file, one key per device (`remap.pad`, `remap.kbm`): the roles that differ from their
//! default, `role=input` separated by spaces; unset = the defaults.

use glam::Vec2;
use kh_core::{PadButton, PadButtons};

/// Which table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Device {
    Pad,
    Kbm,
}

impl Device {
    /// The settings file's key of the table.
    pub const fn ini_key(self) -> &'static str {
        match self {
            Device::Pad => "remap.pad",
            Device::Kbm => "remap.kbm",
        }
    }
}

/// What a keyboard / mouse role does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    Button(PadButton),
    /// The left stick: 0 up, 1 down, 2 left, 3 right.
    Stick(u8),
    /// KH3's walk: both movement axes x 0.5 while held.
    Walk,
}

/// One role: its settings-file name, KH3's text key (its label, read from the player's pack: [`label`]), what it does, its
/// default input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Role {
    pub name: &'static str,
    pub text_key: &'static str,
    pub out: Out,
    pub default: u16,
}

/// (KH3TEXT 2026-11-08) KH3's key-config texts (the pack's `kh3_settings` table), set when the UI pack loads (hud.rs). KH3's
/// text is never compiled in (the release checker refuses a DLL with KH3's game text).
static LABELS: std::sync::Mutex<Option<std::collections::HashMap<String, String>>> = std::sync::Mutex::new(None);

/// Keeps the pack's key-config texts for [`label`] (the whole `kh3_settings` table may be passed: only the roles' keys are
/// read).
pub fn set_labels(table: std::collections::HashMap<String, String>) {
    *LABELS.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(table);
}

/// A role's row name: KH3's text of its key (kh3_settings.keyboard.keyconfig_NNNN) from the player's pack; the key itself
/// when the pack has none (never a compiled KH3 line).
pub fn label(role: &Role) -> String {
    label_in(LABELS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref(), role)
}

/// (tests) The player's pack's `kh3_settings` table (build/sora_pack, made by the setup), or `None` without a pack.
#[cfg(test)]
pub fn test_pack_labels() -> Option<std::collections::HashMap<String, String>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui/strings/en/kh3_settings.json");
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let mut table = std::collections::HashMap::new();
    for (ns, rows) in json["namespaces"].as_object()? {
        for (k, v) in rows.as_object()? {
            table.insert(format!("{ns}.{k}"), v.as_str().unwrap_or("").to_string());
        }
    }
    Some(table)
}

/// [`label`] from a given table.
pub fn label_in(table: Option<&std::collections::HashMap<String, String>>, role: &Role) -> String {
    table.and_then(|t| t.get(role.text_key)).filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| role.text_key.to_string())
}

// ---- input codes: a pad button = its PadButton index; keyboard = Windows virtual-key codes; mouse buttons = their VK codes
// (VK_LBUTTON 1, VK_RBUTTON 2, VK_MBUTTON 4, VK_XBUTTON1 5, VK_XBUTTON2 6); the wheel = two codes of its own
pub const MOUSE_LEFT: u16 = 0x01;
pub const MOUSE_RIGHT: u16 = 0x02;
pub const MOUSE_MIDDLE: u16 = 0x04;
pub const MOUSE_X1: u16 = 0x05;
pub const MOUSE_X2: u16 = 0x06;
pub const WHEEL_UP: u16 = 0x101;
pub const WHEEL_DOWN: u16 = 0x102;

const fn role(name: &'static str, text_key: &'static str, out: Out, default: u16) -> Role {
    Role { name, text_key, out, default }
}

/// KH3's names of the controller's button roles (its key-config texts for the default button's function).
pub const PAD_ROLES: [Role; 16] = [
    role("A", "kh3_settings.keyboard.keyconfig_0010", Out::Button(PadButton::FaceBottom), PadButton::FaceBottom as u16),
    role("B", "kh3_settings.keyboard.keyconfig_0020", Out::Button(PadButton::FaceRight), PadButton::FaceRight as u16),
    role("X", "kh3_settings.keyboard.keyconfig_0030", Out::Button(PadButton::FaceLeft), PadButton::FaceLeft as u16),
    role("Y", "kh3_settings.keyboard.keyconfig_0040", Out::Button(PadButton::FaceTop), PadButton::FaceTop as u16),
    role("LB", "kh3_settings.keyboard.keyconfig_0050", Out::Button(PadButton::LeftShoulder), PadButton::LeftShoulder as u16),
    role("RB", "kh3_settings.keyboard.keyconfig_0060", Out::Button(PadButton::RightShoulder), PadButton::RightShoulder as u16),
    role("LT", "kh3_settings.keyboard.keyconfig_0070", Out::Button(PadButton::LeftTrigger), PadButton::LeftTrigger as u16),
    role("RT", "kh3_settings.keyboard.keyconfig_0080", Out::Button(PadButton::RightTrigger), PadButton::RightTrigger as u16),
    role("Up", "kh3_settings.keyboard.keyconfig_0200", Out::Button(PadButton::DPadUp), PadButton::DPadUp as u16),
    role("Down", "kh3_settings.keyboard.keyconfig_0210", Out::Button(PadButton::DPadDown), PadButton::DPadDown as u16),
    role("Left", "kh3_settings.keyboard.keyconfig_0220", Out::Button(PadButton::DPadLeft), PadButton::DPadLeft as u16),
    role("Right", "kh3_settings.keyboard.keyconfig_0230", Out::Button(PadButton::DPadRight), PadButton::DPadRight as u16),
    role("L3", "kh3_settings.keyboard.keyconfig_0140", Out::Button(PadButton::LeftThumb), PadButton::LeftThumb as u16),
    role("R3", "kh3_settings.keyboard.keyconfig_0190", Out::Button(PadButton::RightThumb), PadButton::RightThumb as u16),
    role("View", "kh3_settings.keyboard.keyconfig_0250", Out::Button(PadButton::SpecialLeft), PadButton::SpecialLeft as u16),
    role("Menu", "kh3_settings.keyboard.keyconfig_0240", Out::Button(PadButton::SpecialRight), PadButton::SpecialRight as u16),
];

// Windows virtual-key codes of KH3's defaults
const VK_BACK: u16 = 0x08;
const VK_RETURN: u16 = 0x0D;
const VK_SPACE: u16 = 0x20;
const VK_LEFT: u16 = 0x25;
const VK_UP: u16 = 0x26;
const VK_RIGHT: u16 = 0x27;
const VK_DOWN: u16 = 0x28;
const VK_LSHIFT: u16 = 0xA0;
const VK_RSHIFT: u16 = 0xA1;
const VK_RCONTROL: u16 = 0xA3;
const VK_LMENU: u16 = 0xA4;
const VK_RMENU: u16 = 0xA5;

/// KH3 PC's keyboard and mouse defaults, each key of each slot a role of its own (re-d/kh3_kbm_defaults.json, FUN_140edf630).
pub const KBM_ROLES: [Role; 32] = [
    role("Confirm", "kh3_settings.keyboard.keyconfig_0010", Out::Button(PadButton::FaceBottom), MOUSE_LEFT),
    role("Confirm2", "kh3_settings.keyboard.keyconfig_0010", Out::Button(PadButton::FaceBottom), VK_RETURN),
    role("Cancel", "kh3_settings.keyboard.keyconfig_0020", Out::Button(PadButton::FaceRight), VK_SPACE),
    role("Cancel2", "kh3_settings.keyboard.keyconfig_0020", Out::Button(PadButton::FaceRight), VK_BACK),
    role("Block", "kh3_settings.keyboard.keyconfig_0030", Out::Button(PadButton::FaceLeft), MOUSE_RIGHT),
    role("Block2", "kh3_settings.keyboard.keyconfig_0030", Out::Button(PadButton::FaceLeft), VK_RSHIFT),
    role("Situation", "kh3_settings.keyboard.keyconfig_0040", Out::Button(PadButton::FaceTop), b'F' as u16),
    role("Situation2", "kh3_settings.keyboard.keyconfig_0040", Out::Button(PadButton::FaceTop), VK_RCONTROL),
    role("Shortcuts", "kh3_settings.keyboard.keyconfig_0050", Out::Button(PadButton::LeftShoulder), VK_LSHIFT),
    role("LockOn", "kh3_settings.keyboard.keyconfig_0060", Out::Button(PadButton::RightShoulder), b'R' as u16),
    role("CycleSituation", "kh3_settings.keyboard.keyconfig_0070", Out::Button(PadButton::LeftTrigger), b'C' as u16),
    role("ChangeTarget", "kh3_settings.keyboard.keyconfig_0080", Out::Button(PadButton::RightTrigger), b'E' as u16),
    role("Forward", "kh3_settings.keyboard.keyconfig_0090", Out::Stick(0), b'W' as u16),
    role("Backward", "kh3_settings.keyboard.keyconfig_0100", Out::Stick(1), b'S' as u16),
    role("MoveLeft", "kh3_settings.keyboard.keyconfig_0110", Out::Stick(2), b'A' as u16),
    role("MoveRight", "kh3_settings.keyboard.keyconfig_0120", Out::Stick(3), b'D' as u16),
    role("Walk", "kh3_settings.keyboard.keyconfig_0130", Out::Walk, VK_LMENU),
    role("ResetCamera", "kh3_settings.keyboard.keyconfig_0190", Out::Button(PadButton::RightThumb), VK_RMENU),
    role("ResetCamera2", "kh3_settings.keyboard.keyconfig_0190", Out::Button(PadButton::RightThumb), MOUSE_MIDDLE),
    role("CursorUp", "kh3_settings.keyboard.keyconfig_0200", Out::Button(PadButton::DPadUp), VK_UP),
    role("CursorUp2", "kh3_settings.keyboard.keyconfig_0200", Out::Button(PadButton::DPadUp), WHEEL_UP),
    role("CursorDown", "kh3_settings.keyboard.keyconfig_0210", Out::Button(PadButton::DPadDown), VK_DOWN),
    role("CursorDown2", "kh3_settings.keyboard.keyconfig_0210", Out::Button(PadButton::DPadDown), WHEEL_DOWN),
    role("CursorLeft", "kh3_settings.keyboard.keyconfig_0220", Out::Button(PadButton::DPadLeft), VK_LEFT),
    role("CursorLeft2", "kh3_settings.keyboard.keyconfig_0220", Out::Button(PadButton::DPadLeft), MOUSE_X1),
    role("CursorRight", "kh3_settings.keyboard.keyconfig_0230", Out::Button(PadButton::DPadRight), VK_RIGHT),
    role("CursorRight2", "kh3_settings.keyboard.keyconfig_0230", Out::Button(PadButton::DPadRight), MOUSE_X2),
    role("Menu", "kh3_settings.keyboard.keyconfig_0240", Out::Button(PadButton::SpecialRight), b'M' as u16),
    role("Menu2", "kh3_settings.keyboard.keyconfig_0240", Out::Button(PadButton::SpecialRight), b'4' as u16),
    role("CameraMode", "kh3_settings.keyboard.keyconfig_0250", Out::Button(PadButton::SpecialLeft), b'N' as u16),
    role("CameraMode2", "kh3_settings.keyboard.keyconfig_0250", Out::Button(PadButton::SpecialLeft), b'3' as u16),
    role("QuickTurn", "kh3_settings.keyboard.keyconfig_0140", Out::Button(PadButton::LeftThumb), b'X' as u16),
];

pub fn roles(device: Device) -> &'static [Role] {
    match device {
        Device::Pad => &PAD_ROLES,
        Device::Kbm => &KBM_ROLES,
    }
}

/// An input's shown name (the controller's buttons by KH3's Xbox names; keys and mouse buttons by their keycap / KH3 text).
pub fn input_name(device: Device, code: u16) -> String {
    if device == Device::Pad {
        return match PadButton::ALL.get(code as usize) {
            Some(PadButton::FaceBottom) => "A",
            Some(PadButton::FaceRight) => "B",
            Some(PadButton::FaceLeft) => "X",
            Some(PadButton::FaceTop) => "Y",
            Some(PadButton::LeftShoulder) => "LB",
            Some(PadButton::RightShoulder) => "RB",
            Some(PadButton::LeftTrigger) => "LT",
            Some(PadButton::RightTrigger) => "RT",
            Some(PadButton::DPadUp) => "D-pad Up",
            Some(PadButton::DPadDown) => "D-pad Down",
            Some(PadButton::DPadLeft) => "D-pad Left",
            Some(PadButton::DPadRight) => "D-pad Right",
            Some(PadButton::LeftThumb) => "L3",
            Some(PadButton::RightThumb) => "R3",
            Some(PadButton::SpecialLeft) => "View",
            Some(PadButton::SpecialRight) => "Menu",
            None => "-",
        }
        .to_string();
    }
    let named = match code {
        MOUSE_LEFT => "Left Click",
        MOUSE_RIGHT => "Right Click",
        MOUSE_MIDDLE => "Middle Click",
        MOUSE_X1 => "Mouse Button 4",
        MOUSE_X2 => "Mouse Button 5",
        WHEEL_UP => "Wheel Up",
        WHEEL_DOWN => "Wheel Down",
        VK_BACK => "Backspace",
        0x09 => "Tab",
        VK_RETURN => "Enter",
        0x13 => "Pause",
        0x14 => "Caps Lock",
        0x1B => "Esc",
        VK_SPACE => "Space",
        0x21 => "Page Up",
        0x22 => "Page Down",
        0x23 => "End",
        0x24 => "Home",
        VK_LEFT => "Left",
        VK_UP => "Up",
        VK_RIGHT => "Right",
        VK_DOWN => "Down",
        0x2D => "Insert",
        0x2E => "Delete",
        VK_LSHIFT => "Left Shift",
        VK_RSHIFT => "Right Shift",
        0xA2 => "Left Ctrl",
        VK_RCONTROL => "Right Ctrl",
        VK_LMENU => "Left Alt",
        VK_RMENU => "Right Alt",
        0xBA => ";",
        0xBB => "=",
        0xBC => ",",
        0xBD => "-",
        0xBE => ".",
        0xBF => "/",
        0xC0 => "`",
        0xDB => "[",
        0xDC => "\\",
        0xDD => "]",
        0xDE => "'",
        _ => "",
    };
    if !named.is_empty() {
        return named.to_string();
    }
    match code {
        0x30..=0x39 | 0x41..=0x5A => char::from(code as u8).to_string(),
        0x60..=0x69 => format!("Num {}", code - 0x60),
        0x70..=0x87 => format!("F{}", code - 0x6F),
        _ => format!("Key {code:#04x}"),
    }
}

/// A device's table: the input of each role (in the role order).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub device: Device,
    pub assign: Vec<u16>,
}

/// What assigning an input to a role means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assign {
    /// The role has it already.
    Same,
    /// No role holds it: taken at once.
    Free,
    /// Another role (its index) holds it: the two swap after the player's Yes.
    Swap(usize),
}

impl Table {
    pub fn defaults(device: Device) -> Table {
        Table { device, assign: roles(device).iter().map(|r| r.default).collect() }
    }

    /// The table of the settings file's value (`role=input` pairs; unknown roles or inputs are skipped; a pad table that
    /// would not be a permutation of the 16 buttons gives the defaults).
    pub fn parse(device: Device, text: &str) -> Table {
        let mut t = Table::defaults(device);
        let rs = roles(device);
        for pair in text.split_whitespace() {
            let Some((name, input)) = pair.split_once('=') else { continue };
            let (Some(k), Ok(code)) = (rs.iter().position(|r| r.name == name), parse_code(input)) else { continue };
            if device == Device::Pad && code as usize >= PadButton::ALL.len() {
                continue;
            }
            t.assign[k] = code;
        }
        if device == Device::Pad {
            let mut seen = [false; 16];
            for c in &t.assign {
                if std::mem::replace(&mut seen[*c as usize], true) {
                    return Table::defaults(device);
                }
            }
        }
        t
    }

    /// The settings file's value: the roles that differ from their default (empty = the defaults).
    pub fn format(&self) -> String {
        roles(self.device).iter().zip(&self.assign).filter(|(r, c)| r.default != **c).map(|(r, c)| format!("{}={c:#x}", r.name)).collect::<Vec<_>>().join(" ")
    }

    /// The role that holds an input.
    pub fn holder(&self, code: u16) -> Option<usize> {
        self.assign.iter().position(|c| *c == code)
    }

    pub fn check(&self, role: usize, code: u16) -> Assign {
        match self.holder(code) {
            Some(k) if k == role => Assign::Same,
            Some(k) => Assign::Swap(k),
            None => Assign::Free,
        }
    }

    /// Assigns (after the player's Yes when it swaps): the role takes the input, the other role (if any) the role's old one.
    pub fn assign(&mut self, role: usize, code: u16) {
        if role >= self.assign.len() {
            return;
        }
        let old = self.assign[role];
        if let Some(other) = self.holder(code).filter(|k| *k != role) {
            self.assign[other] = old;
        }
        self.assign[role] = code;
    }

    /// The question of a swap: "<input> is assigned to <other role>. Swap <role> and <other role>?"
    pub fn swap_question(&self, role: usize, code: u16, other: usize, label: &dyn Fn(&Role) -> String) -> String {
        self.swap_question_with(role, code, other, label, &input_name)
    }

    /// (BUTTON-ICONS) The same question with the input shown by `input` (the settings screen: the button's glyph).
    pub fn swap_question_with(&self, role: usize, code: u16, other: usize, label: &dyn Fn(&Role) -> String, input: &dyn Fn(Device, u16) -> String) -> String {
        let rs = roles(self.device);
        format!("{} is assigned to {}.\nSwap {} and {}?", input(self.device, code), label(&rs[other]), label(&rs[role]), label(&rs[other]))
    }

    /// THE CONTROLLER'S TABLE APPLIED (pure): a role is held while the button assigned to it is.
    pub fn apply_pad(&self, physical: PadButtons) -> PadButtons {
        let mut out = PadButtons::NONE;
        for (r, code) in PAD_ROLES.iter().zip(&self.assign) {
            if let (Out::Button(b), Some(p)) = (r.out, PadButton::ALL.get(*code as usize)) {
                if physical.has(*p) {
                    out.set(b, true);
                }
            }
        }
        out
    }

    /// THE KEYBOARD AND MOUSE TABLE APPLIED (pure): `down(code)` = that key / mouse input is held (the wheel's codes for one
    /// notch's short press). The left stick from the movement roles (walk halves it), the buttons from the others.
    pub fn apply_kbm(&self, down: &dyn Fn(u16) -> bool) -> (Vec2, PadButtons) {
        let mut buttons = PadButtons::NONE;
        let (mut x, mut y, mut walk) = (0.0f32, 0.0f32, false);
        for (r, code) in KBM_ROLES.iter().zip(&self.assign) {
            if !down(*code) {
                continue;
            }
            match r.out {
                Out::Button(b) => buttons.set(b, true),
                Out::Stick(0) => y += 1.0,
                Out::Stick(1) => y -= 1.0,
                Out::Stick(2) => x -= 1.0,
                Out::Stick(_) => x += 1.0,
                Out::Walk => walk = true,
            }
        }
        let mut stick = Vec2::new(x.clamp(-1.0, 1.0), y.clamp(-1.0, 1.0));
        if walk {
            stick *= 0.5;
        }
        (stick, buttons)
    }

    /// (CAMP-OPTIONS) THE KEYBOARD ON A KH SCREEN (load, save, camp, shops, dialogs, the settings): the table, plus KH3's
    /// default key of every role whose default key the table gives to no role (a key moved away still works where nothing
    /// else took it; a key the table uses is never read twice, so a swap never presses two roles at once). The lock-out
    /// protection: whatever the table says, the menus stay usable with KH3's keys.
    pub fn apply_kbm_protected(&self, down: &dyn Fn(u16) -> bool) -> (Vec2, PadButtons) {
        let (mut stick, mut buttons) = self.apply_kbm(down);
        let defaults = Table::defaults(Device::Kbm);
        let free = |code: u16| !self.assign.contains(&code);
        let (s, b) = defaults.apply_kbm(&|code| free(code) && down(code));
        if stick == Vec2::ZERO {
            stick = s;
        }
        buttons = PadButtons(buttons.0 | b.0);
        (stick, buttons)
    }
}

fn parse_code(s: &str) -> Result<u16, std::num::ParseIntError> {
    match s.strip_prefix("0x") {
        Some(h) => u16::from_str_radix(h, 16),
        None => s.parse(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pb(list: &[PadButton]) -> PadButtons {
        let mut b = PadButtons::NONE;
        for p in list {
            b.set(*p, true);
        }
        b
    }

    #[test]
    fn co_confirm_to_b_swaps_with_cancel_and_reset() {
        let mut t = Table::defaults(Device::Pad);
        let b = PadButton::FaceRight as u16;
        // "Confirm" (role 0, A) on B: B holds "Cancel / Jump": a swap, after the question naming both
        assert_eq!(t.check(0, b), Assign::Swap(1));
        let q = t.swap_question(0, b, 1, &|r| label_in(None, r));
        assert_eq!(q, "B is assigned to kh3_settings.keyboard.keyconfig_0020.\nSwap kh3_settings.keyboard.keyconfig_0010 and kh3_settings.keyboard.keyconfig_0020?");
        let pack = std::collections::HashMap::from([("kh3_settings.keyboard.keyconfig_0010".to_string(), "<confirm>".to_string()), ("kh3_settings.keyboard.keyconfig_0020".to_string(), "<cancel>".to_string())]);
        let q = t.swap_question(0, b, 1, &|r| label_in(Some(&pack), r));
        assert_eq!(q, "B is assigned to <cancel>.\nSwap <confirm> and <cancel>?", "the pack's texts");
        t.assign(0, b);
        // B now confirms, A does what B did
        let pressed_b = pb(&[PadButton::FaceRight]);
        assert_eq!(t.apply_pad(pressed_b), pb(&[PadButton::FaceBottom]));
        assert_eq!(t.apply_pad(pb(&[PadButton::FaceBottom])), pb(&[PadButton::FaceRight]));
        // persisted and read back
        let text = t.format();
        assert_eq!(text, "A=0x1 B=0x0");
        assert_eq!(Table::parse(Device::Pad, &text), t);
        // every other button is itself
        for p in PadButton::ALL.iter().filter(|p| !matches!(p, PadButton::FaceBottom | PadButton::FaceRight)) {
            assert_eq!(t.apply_pad(pb(&[*p])), pb(&[*p]));
        }
        // reset: the defaults (an empty value)
        assert_eq!(Table::parse(Device::Pad, ""), Table::defaults(Device::Pad));
        assert_eq!(Table::defaults(Device::Pad).format(), "");
        // a pad table that is no permutation is refused (defaults)
        assert_eq!(Table::parse(Device::Pad, "A=0x1"), Table::defaults(Device::Pad));
        // the same button: nothing to do
        assert_eq!(t.check(0, b), Assign::Same);
    }

    #[test]
    fn co_every_pad_role_resolves_through_the_table() {
        let t = Table::defaults(Device::Pad);
        for p in PadButton::ALL {
            assert_eq!(t.apply_pad(pb(&[p])), pb(&[p]), "{p:?}");
        }
        assert_eq!(PAD_ROLES.len(), PadButton::ALL.len());
        // a full rotation of the face buttons, persisted
        let mut r = t.clone();
        r.assign(0, PadButton::FaceLeft as u16);
        r.assign(1, PadButton::FaceTop as u16);
        let back = Table::parse(Device::Pad, &r.format());
        assert_eq!(back, r);
        assert_eq!(back.apply_pad(pb(&[PadButton::FaceTop])), pb(&[PadButton::FaceRight]));
    }
    // (release) the test of KH3's keyboard and mouse defaults compiles in the developer's extraction (build/sora_tmp/...
    // /kh3_kbm_defaults.json), which no release has: left out.

    /// (KH3TEXT) Every role's label is KH3's text from the player's pack (build/sora_pack, made by the setup); none is
    /// compiled in: without the pack a row shows its key.
    #[test]
    fn labels_come_from_the_pack() {
        let role = PAD_ROLES[1];
        assert_eq!(label_in(None, &role), "kh3_settings.keyboard.keyconfig_0020");
        let Some(table) = test_pack_labels() else { return };
        for r in PAD_ROLES.iter().chain(KBM_ROLES.iter()) {
            let shown = label_in(Some(&table), r);
            assert!(!shown.is_empty() && shown != r.text_key, "{} ({}) is not in the pack", r.name, r.text_key);
        }
        assert!(label_in(Some(&table), &role).contains("Jump"), "B's row is KH3's Cancel / Jump text");
    }
}
