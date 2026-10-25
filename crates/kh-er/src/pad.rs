//! The Xbox pad, read through XInput directly and converted exactly as `kh-app/src/xinput.rs`
//! does (UE 4.17's `XInputInterface`: `short <= 0 ? short / 32768 : short / 32767`, no dead zone,
//! triggers are buttons above 30). The same logic in a second place because that file is tied to
//! Bevy types; unifying the two in a small shared crate is noted as follow-up work.

use glam::Vec2;
use kh_core::{PadButton, PadButtons};
use windows_sys::Win32::UI::Input::XboxController::XINPUT_STATE;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pad {
    /// Right and up positive.
    pub left: Vec2,
    pub right: Vec2,
    pub buttons: PadButtons,
}

const BUTTON_BITS: [(u16, PadButton); 14] = [
    (0x0001, PadButton::DPadUp),
    (0x0002, PadButton::DPadDown),
    (0x0004, PadButton::DPadLeft),
    (0x0008, PadButton::DPadRight),
    (0x0010, PadButton::SpecialRight),
    (0x0020, PadButton::SpecialLeft),
    (0x0040, PadButton::LeftThumb),
    (0x0080, PadButton::RightThumb),
    (0x0100, PadButton::LeftShoulder),
    (0x0200, PadButton::RightShoulder),
    (0x1000, PadButton::FaceBottom),
    (0x2000, PadButton::FaceRight),
    (0x4000, PadButton::FaceLeft),
    (0x8000, PadButton::FaceTop),
];

/// UE 4.17 `ShortToNormalizedFloat`.
pub fn short_to_float(value: i16) -> f32 {
    let norm = if value <= 0 { 32768.0 } else { 32767.0 };
    value as f32 / norm
}

#[derive(Default)]
pub struct Reader {
    index: Option<u32>,
    wait: u32,
}

impl Reader {
    fn state(index: u32) -> Option<XINPUT_STATE> {
        // (around our own hook of the function: inputs.rs)
        crate::inputs::xinput_state(index)
    }

    /// The first connected pad now; `None` when there is none (looked for about once a second).
    pub fn read(&mut self) -> Option<Pad> {
        let mut found = self.index.and_then(Self::state);
        if found.is_none() {
            self.index = None;
            if self.wait > 0 {
                self.wait -= 1;
                return None;
            }
            self.wait = 60;
            for candidate in 0..4 {
                if let Some(s) = Self::state(candidate) {
                    self.index = Some(candidate);
                    found = Some(s);
                    break;
                }
            }
        }
        let pad = found?.Gamepad;
        let mut buttons = PadButtons::NONE;
        for (bit, button) in BUTTON_BITS {
            buttons.set(button, pad.wButtons & bit != 0);
        }
        buttons.set(PadButton::LeftTrigger, pad.bLeftTrigger > 30);
        buttons.set(PadButton::RightTrigger, pad.bRightTrigger > 30);
        Some(Pad {
            left: Vec2::new(short_to_float(pad.sThumbLX), short_to_float(pad.sThumbLY)),
            right: Vec2::new(short_to_float(pad.sThumbRX), short_to_float(pad.sThumbRY)),
            buttons,
        })
    }
}
