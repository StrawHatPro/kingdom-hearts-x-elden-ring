//! THE MENUS' PAD INPUT, shared by every host (moved from kh-app/src/gfx_hud/mod.rs `step_hook`): the buttons
//! pressed since the HUD's last frame and the direction events of the D-pad and the left stick with the UI
//! pad's key repeat. A host calls [`MenuInput::step`] once per input sample (the test app: per fixed step;
//! Elden Ring: per game frame) and hands [`MenuInput::take`] to `hud::Frame`.

use kh_core::{PadButton as B, PadButtons};

/// LOCAL: the stick deflection that counts as a direction key.
pub const NAV_STICK: f32 = 0.5;
/// THE UI PAD'S KEY REPEAT (cleanup pass; sheet sora-ui-params rows `menu.repeat_first` /
/// `menu.repeat_next`): the per-key update 0x14158EED0 (called for every key of the UI pad by
/// 0x14158E390, the UI manager's update 0x1415B0ED0) loads the key's timer with 0.6 on the press
/// (`mov dword [rbx+4], 3F19999Ah` at 0x14158EF35), subtracts the frame time while the key is
/// held and, when it falls below zero, sets the repeat byte and reloads 0.06 (`3D75C28Fh` at
/// 0x14158EFD6). LOCAL: the D-pad and the stick share one timer per direction here (the game
/// has separate keys 10 - 13 and 14 - 17), and the frame time is the host's sample time.
pub const NAV_REPEAT_FIRST: f32 = 0.6;
pub const NAV_REPEAT_NEXT: f32 = 0.06;

/// One held frame of a UI pad key: true = the repeat byte is set in this frame.
pub fn nav_repeat(timer: &mut f32, dt: f32) -> bool {
    *timer -= dt;
    if *timer < 0.0 {
        *timer = NAV_REPEAT_NEXT;
        return true;
    }
    false
}

pub struct MenuInput {
    /// The direction was down in the last sample, and its repeat timer.
    down: [bool; 4],
    timer: [f32; 4],
    events: Vec<(u8, bool)>,
    pressed: u32,
}

impl MenuInput {
    pub const fn new() -> MenuInput {
        MenuInput { down: [false; 4], timer: [0.0; 4], events: Vec::new(), pressed: 0 }
    }

    /// One input sample: notes the pad buttons pressed and turns the D-pad and the left stick into
    /// direction events (0 up, 1 down, 2 left, 3 right; true = auto-repeat): one on the press, then
    /// the repeat while held.
    pub fn step(&mut self, input: &kh_core::Input, dt: f32) {
        let pad = &input.pad;
        self.pressed |= pad.pressed.0;
        let stick = input.move_stick;
        let (ax, ay) = (stick.x.abs(), stick.y.abs());
        let down = [
            pad.held.has(B::DPadUp) || (stick.y > NAV_STICK && ay >= ax),
            pad.held.has(B::DPadDown) || (stick.y < -NAV_STICK && ay >= ax),
            pad.held.has(B::DPadLeft) || (stick.x < -NAV_STICK && ax > ay),
            pad.held.has(B::DPadRight) || (stick.x > NAV_STICK && ax > ay),
        ];
        for dir in 0..4 {
            if !down[dir] {
                self.down[dir] = false;
                continue;
            }
            if !self.down[dir] {
                // Pressed this frame (key state 1): the timer is loaded, no decrement in this frame.
                self.down[dir] = true;
                self.timer[dir] = NAV_REPEAT_FIRST;
                self.events.push((dir as u8, false));
                continue;
            }
            // Held (key state 3): the timer runs down; below zero the repeat byte is set for this
            // frame and the timer reloaded (no carry of the overshoot, at most one repeat a frame).
            if nav_repeat(&mut self.timer[dir], dt) {
                self.events.push((dir as u8, true));
            }
        }
    }

    /// The buttons pressed and the direction events since the last call.
    pub fn take(&mut self) -> (PadButtons, Vec<(u8, bool)>) {
        (PadButtons(std::mem::take(&mut self.pressed)), std::mem::take(&mut self.events))
    }

    /// Forgets everything (the menu closed, the input owner changed).
    pub fn clear(&mut self) {
        *self = MenuInput::new();
    }
}

impl Default for MenuInput {
    fn default() -> MenuInput {
        MenuInput::new()
    }
}

/// (cutscene) SORA'S INPUT IN A CUTSCENE (`Hud::set_cutscene(true)`): nothing of the pad reaches him (no
/// move, no jump, no button: his command menu and shortcuts read `Input::pad`); the camera's yaw and the view
/// stay. A host that goes on stepping him in a cutscene hands him this instead of the pad's input; the RAW
/// input still goes to [`MenuInput::step`] (Start and the cursor of the cutscene pause menu).
pub fn cutscene_input(input: &kh_core::Input) -> kh_core::Input {
    kh_core::Input { camera_yaw: input.camera_yaw, view: input.view, ..kh_core::Input::default() }
}

#[cfg(test)]
mod nav_tests {
    use super::*;

    #[test]
    fn a_held_key_repeats_after_0_6_s_and_then_every_0_06_s() {
        // 60 Hz frames after the press frame; the frame numbers of the repeats.
        let (mut timer, dt) = (NAV_REPEAT_FIRST, 1.0 / 60.0);
        let repeats: Vec<i32> = (1..=50).filter(|_| nav_repeat(&mut timer, dt)).collect();
        // 0.6 s = 36 frames: the timer is below zero on the 37th held frame (float: 36 x 1/60
        // leaves a hair above or below zero, either is the game's own arithmetic); then every
        // 4th frame (0.06 s = 3.6 frames, no carry).
        assert!(repeats[0] == 36 || repeats[0] == 37, "{repeats:?}");
        assert_eq!(repeats[1] - repeats[0], 4, "{repeats:?}");
        assert_eq!(repeats[2] - repeats[1], 4, "{repeats:?}");
    }
}

/// THE CAMP MENU'S HOST HOOK FOR A PAD-ONLY HOST (`Hud::camp_hook`; the test app has its own in
/// menu_model.rs, which also follows the device used last): the settings guide shows the pad's Menu
/// glyph, and the character picture's clip of the Abilities and Stats screens plays in when the screen
/// is there, as the top screen's does. [D] by the clips' names and labels.
pub fn pad_camp_hook(movie: &mut kh_ui::Movie, fonts: &kh_ui::Fonts) {
    const SETTING_GUIDE: &str = "topMenu.cmdBtnTopMenu.coSettingGuide";
    if movie.exists(SETTING_GUIDE) {
        movie.set_visible(&format!("{SETTING_GUIDE}.settingGuideIcon"), false);
        movie.set_visible(&format!("{SETTING_GUIDE}.settingGuideIconSD"), true);
        let field = format!("{SETTING_GUIDE}.settingGuideIconSD.settingGuideBtnInfoText.textField");
        // (IconText row `Sd_Option`: the pad's Menu button.)
        if movie.text(&field).as_deref() != Some("<icon(Sd_Option)>") {
            movie.set_text(&field, "<icon(Sd_Option)>", fonts);
        }
    }
    for (screen, anm) in [("ability", "ability.abilityAnm.selectCharaImage1.charaAnm"), ("status", "status.statusAnm.selectCharaImage2.charaAnm")] {
        let open = matches!(movie.current_label(screen), Some("in" | "loop" | "left" | "right"));
        let now = movie.current_label(anm).map(str::to_string);
        if open && now.as_deref() == Some("off") {
            movie.goto_and_play(anm, "in");
        } else if !open && now.as_deref().is_some_and(|l| l != "off") {
            movie.goto_and_stop(anm, "off");
        }
    }
}
