//! The pad's buttons at device level: what the host reports each step in `Input::pad`.
//!
//! The set is Unreal's gamepad key set as this exe has it (`EKeys::Gamepad_FaceButton_Bottom`
//! ... `Gamepad_Special_Right`; movement spec 2, build/sora_tmp/input_notes.md). WHICH BUTTON
//! DOES WHAT is the game's key configuration, and only two rows of it are specified so far
//! (movement spec 2.4: config global `i.GamePadDecideType`, default 0, set up at 0x140EDD580):
//!
//! | action | button with `i.GamePadDecideType` = 0 (the default) |
//! |---|---|
//! | jump / cancel | `FaceRight` (Xbox B) |
//! | attack / decide | `FaceBottom` (Xbox A) |
//!
//! Every other action (magic shortcut, guard / dodge, lock-on, shotlock, command menu cursor,
//! situation command ...) gets its button from the specs still being written
//! (docs/sora-combat-spec.md, sora-actions-spec.md, sora-magic-spec.md, sora-ui-spec.md): their
//! implementers add the named constants here, next to `JUMP` and `ATTACK`. Until then states
//! read raw buttons and say which.
//!
//! The stage-1 controller code still reads the jump through `Input::jump_pressed` /
//! `Input::jump_held`; `Input::with_pad` fills those from `FaceRight`.

/// One button of the pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum PadButton {
    /// Xbox A, PlayStation cross.
    FaceBottom = 0,
    /// Xbox B, PlayStation circle.
    FaceRight = 1,
    /// Xbox X, PlayStation square.
    FaceLeft = 2,
    /// Xbox Y, PlayStation triangle.
    FaceTop = 3,
    /// LB / L1.
    LeftShoulder = 4,
    /// RB / R1.
    RightShoulder = 5,
    /// LT / L2 as a button.
    LeftTrigger = 6,
    /// RT / R2 as a button.
    RightTrigger = 7,
    DPadUp = 8,
    DPadDown = 9,
    DPadLeft = 10,
    DPadRight = 11,
    /// L3: the left stick pressed.
    LeftThumb = 12,
    /// R3: the right stick pressed (the camera reset reads it through `CameraInput::recentre`).
    RightThumb = 13,
    /// Back / View / Share (`Gamepad_Special_Left`).
    SpecialLeft = 14,
    /// Start / Menu / Options (`Gamepad_Special_Right`).
    SpecialRight = 15,
}

impl PadButton {
    pub const ALL: [PadButton; 16] = [
        PadButton::FaceBottom,
        PadButton::FaceRight,
        PadButton::FaceLeft,
        PadButton::FaceTop,
        PadButton::LeftShoulder,
        PadButton::RightShoulder,
        PadButton::LeftTrigger,
        PadButton::RightTrigger,
        PadButton::DPadUp,
        PadButton::DPadDown,
        PadButton::DPadLeft,
        PadButton::DPadRight,
        PadButton::LeftThumb,
        PadButton::RightThumb,
        PadButton::SpecialLeft,
        PadButton::SpecialRight,
    ];

    /// Jump / cancel with the game's default layout (`i.GamePadDecideType` 0).
    pub const JUMP: PadButton = PadButton::FaceRight;
    /// Attack / decide with the game's default layout.
    pub const ATTACK: PadButton = PadButton::FaceBottom;
    /// Guard / dodge roll / air slide with the game's default layout (sheet `sora-input-layout`:
    /// key global 0x149C70748 = FaceButton_Left; actions spec II.7).
    pub const GUARD: PadButton = PadButton::FaceLeft;
    /// Use the selected situation command (sheet `sora-input-layout` row `situation_command`:
    /// `Gamepad_FaceButton_Top`; combat strand F 11.5).
    pub const SITUATION: PadButton = PadButton::FaceTop;
    /// Step to the next situation command (row `left_trigger`: HUD 0x14162BAF0 on UI button 6).
    pub const SITUATION_NEXT: PadButton = PadButton::LeftTrigger;

    pub const fn name(self) -> &'static str {
        match self {
            PadButton::FaceBottom => "FaceBottom",
            PadButton::FaceRight => "FaceRight",
            PadButton::FaceLeft => "FaceLeft",
            PadButton::FaceTop => "FaceTop",
            PadButton::LeftShoulder => "LeftShoulder",
            PadButton::RightShoulder => "RightShoulder",
            PadButton::LeftTrigger => "LeftTrigger",
            PadButton::RightTrigger => "RightTrigger",
            PadButton::DPadUp => "DPadUp",
            PadButton::DPadDown => "DPadDown",
            PadButton::DPadLeft => "DPadLeft",
            PadButton::DPadRight => "DPadRight",
            PadButton::LeftThumb => "LeftThumb",
            PadButton::RightThumb => "RightThumb",
            PadButton::SpecialLeft => "SpecialLeft",
            PadButton::SpecialRight => "SpecialRight",
        }
    }

    /// A button by name, case and `_` / `-` ignored: its own name (`FaceBottom`), the Xbox
    /// letters and labels (`a b x y lb rb lt rt up down left right l3 r3 back start`), or one of
    /// the two actions whose button is specified (`jump` = `cancel`, `attack` = `decide`).
    pub fn from_name(name: &str) -> Option<PadButton> {
        let key: String = name.chars().filter(|c| *c != '_' && *c != '-').flat_map(char::to_lowercase).collect();
        Some(match key.as_str() {
            "facebottom" | "a" | "cross" | "attack" | "decide" => PadButton::FaceBottom,
            "faceright" | "b" | "circle" | "jump" | "cancel" => PadButton::FaceRight,
            "faceleft" | "x" | "square" | "guard" | "dodge" => PadButton::FaceLeft,
            "facetop" | "y" | "triangle" => PadButton::FaceTop,
            "leftshoulder" | "lb" | "l1" => PadButton::LeftShoulder,
            "rightshoulder" | "rb" | "r1" => PadButton::RightShoulder,
            "lefttrigger" | "lt" | "l2" => PadButton::LeftTrigger,
            "righttrigger" | "rt" | "r2" => PadButton::RightTrigger,
            "dpadup" | "up" => PadButton::DPadUp,
            "dpaddown" | "down" => PadButton::DPadDown,
            "dpadleft" | "left" => PadButton::DPadLeft,
            "dpadright" | "right" => PadButton::DPadRight,
            "leftthumb" | "l3" => PadButton::LeftThumb,
            "rightthumb" | "r3" => PadButton::RightThumb,
            "specialleft" | "back" | "select" | "view" | "share" => PadButton::SpecialLeft,
            "specialright" | "start" | "menu" | "options" => PadButton::SpecialRight,
            _ => return None,
        })
    }
}

/// A set of buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PadButtons(pub u32);

impl PadButtons {
    pub const NONE: PadButtons = PadButtons(0);

    pub const fn has(self, button: PadButton) -> bool {
        self.0 & (1 << button as u32) != 0
    }

    pub fn set(&mut self, button: PadButton, on: bool) {
        if on {
            self.0 |= 1 << button as u32;
        } else {
            self.0 &= !(1 << button as u32);
        }
    }

    pub const fn with(self, button: PadButton) -> PadButtons {
        PadButtons(self.0 | 1 << button as u32)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The buttons in the set, in `PadButton::ALL` order.
    pub fn iter(self) -> impl Iterator<Item = PadButton> {
        PadButton::ALL.into_iter().filter(move |b| self.has(*b))
    }
}

/// The buttons for one step, as the game's controller polls them: `held` = `IsInputKeyDown`,
/// `pressed` = `WasInputKeyJustPressed` (true on the one step the button went down: the host
/// latches a press until a step consumes it), `released` = the one step it came up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PadState {
    pub held: PadButtons,
    pub pressed: PadButtons,
    pub released: PadButtons,
}

impl PadState {
    /// The state of this step from the buttons down now and the buttons that were down in the
    /// previous step.
    pub const fn from_held(now: PadButtons, before: PadButtons) -> PadState {
        PadState { held: now, pressed: PadButtons(now.0 & !before.0), released: PadButtons(before.0 & !now.0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_edges_and_the_two_specified_actions() {
        for button in PadButton::ALL {
            assert_eq!(PadButton::from_name(button.name()), Some(button));
        }
        assert_eq!((PadButton::from_name("jump"), PadButton::from_name("Attack"), PadButton::from_name("face_button_x")), (Some(PadButton::FaceRight), Some(PadButton::FaceBottom), None));
        assert_eq!((PadButton::from_name("RB"), PadButton::from_name("dpad-up"), PadButton::from_name("r3")), (Some(PadButton::RightShoulder), Some(PadButton::DPadUp), Some(PadButton::RightThumb)));
        let before = PadButtons::NONE.with(PadButton::FaceLeft).with(PadButton::FaceTop);
        let now = PadButtons::NONE.with(PadButton::FaceTop).with(PadButton::FaceRight);
        let state = PadState::from_held(now, before);
        assert_eq!(state.pressed.iter().collect::<Vec<_>>(), vec![PadButton::FaceRight]);
        assert_eq!(state.released.iter().collect::<Vec<_>>(), vec![PadButton::FaceLeft]);
        assert!(state.held.has(PadButton::FaceTop) && !state.held.has(PadButton::FaceLeft));
    }
}
