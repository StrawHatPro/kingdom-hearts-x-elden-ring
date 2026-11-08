//! (AUDIT-A CAMS) KH3'S DARK SCREEN in the Elden Ring renderer: every finish command dims the scene's light to
//! x `DARK_SCREEN_RATE` (0.5) over 1.0 s and gives it back over 1.0 s after.
//!
//! KH3 (decompiled, re/decomp; read 2026-11-05):
//! - The command begin event 0x14150A1F0 calls `DarkScreen(ctrl, 1, 1.0, 1.0)` = 0x14140DF80, the end event 0x14150C440
//!   `DarkScreen(ctrl, 0, 1.0, 1.0)` (kh-core `combat::form::dark_screen` / `Form::dark_screen`, AUDIT-A ARROWGUN).
//! - 0x14140DF80: rate 1.0 means pawn+0x1558 (ATresPlayerPawnBase::MyDarkScreenRate, 0.5); coefficient = (1 - rate) x
//!   (ctrl+0x830)->+0x4A4 (0 without it) + rate, clamped 0 .. 1; `ATresSceneManager::SetMasterLightCoefficient` body
//!   0x1414F7B60 (time, coefficient, priority 1, channel 2, eject 0) on; off (only while ctrl+0x878 bit 2):
//!   `SetMasterLightCoefficientDefault` body 0x1414F7C50 (time, priority 1, channel 2, eject 1) = target 1.0.
//! - 0x1414F7B60 / 0x1414F7C50: the entry (priority, channel) of the scene manager's list at +0x500 gets its transition
//!   0x1411946E0 (time, target, elapsed 0, active) with FROM = the value now (the camera manager's +0x4A0, so a reversal
//!   mid-fade starts where the light is).
//! - The scene manager's tick (ATresSceneManager 0x141500B80): every entry steps 0x14119C4C0, which is LINEAR
//!   (value = from + (target - from) x clamp(elapsed / time, 0, 1)); the head entry's value goes to the player camera
//!   manager's +0x4A0 (override bit 0x200 of +0x2808: the view's master light coefficient) and to the KH shader
//!   controller (this+0x3E0) through 0x1426B73B0 = its `SetKHSSceneEmissiveCoefficient` event (FName DAT_149afb7e8, the
//!   class's last event name; KHS_Blueprint: `SetSceneEmissivIntencity` -> MPC KHS_Control `SceneEmissivIntencity`).
//!   An ejected entry that has reached its target is removed; with the list empty, 0x1414F0E80 puts 1.0 back on both.
//!   So the world's light AND the scene's emissive scale together: the whole lit scene, the characters in it included.
//!
//! HERE: Elden Ring's lit scene (its deferred composite with the KH characters in its G-buffer (the hybrid: ER's own
//! light), its sky and forward stage) is multiplied by the coefficient in the renderer's late pass (gfx.rs `draw_late`,
//! the pass Thunder's scene darkening already uses: after the game's forward stage, before its TAA / exposure / tone
//! mapping), and our late effects are drawn after it, undimmed (KH3's particles are not lights). The coefficient multiplies
//! the effects' scene light (vfx.rs `scene_dim`). [`frame`] is the host's call, once a frame.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

/// ATresPlayerPawnBase::MyDarkScreenRate (pawn+0x1558, 0.5 in the ctor 0x141423210; = kh-core `combat::form::DARK_SCREEN_RATE`,
/// the test below holds them equal: kh-core is not a dependency of the renderer).
pub const RATE: f32 = 0.5;
/// The begin / end events' transition time, s (= kh-core `combat::form::DARK_SCREEN_TIME`).
pub const TIME: f32 = 1.0;

/// One master-light entry's transition (0x1411946E0 / 0x14119C4C0), as data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DarkScreen {
    from: f32,
    target: f32,
    time: f32,
    elapsed: f32,
    value: f32,
    on: bool,
}

impl Default for DarkScreen {
    fn default() -> DarkScreen {
        DarkScreen { from: 1.0, target: 1.0, time: 0.0, elapsed: 0.0, value: 1.0, on: false }
    }
}

impl DarkScreen {
    /// The core's `Form::dark_screen` this frame: a change starts the transition from the value now (0x1414F7B60 / C50).
    pub fn set(&mut self, on: bool) {
        if on == self.on {
            return;
        }
        self.on = on;
        (self.from, self.target, self.time, self.elapsed) = (self.value, if on { RATE.clamp(0.0, 1.0) } else { 1.0 }, TIME, 0.0);
    }

    /// One tick of `dt` seconds (0x14119C4C0: linear, clamped); the master light coefficient after it.
    pub fn step(&mut self, dt: f32) -> f32 {
        self.elapsed += dt.max(0.0);
        let a = if self.time <= 0.0 { 1.0 } else { (self.elapsed / self.time).clamp(0.0, 1.0) };
        self.value = self.from + (self.target - self.from) * a;
        self.value
    }

    /// The coefficient now.
    pub fn value(&self) -> f32 {
        self.value
    }
}

static STATE: Mutex<Option<DarkScreen>> = Mutex::new(None);
static COEFFICIENT: AtomicU32 = AtomicU32::new(0x3F80_0000);

/// The host, once a frame: the core's `Form::dark_screen` (false: the transition back) and the frame's game time (0
/// while paused). Returns the coefficient the scene is multiplied by now.
pub fn frame(on: bool, dt: f32) -> f32 {
    let c = match STATE.lock() {
        Ok(mut g) => {
            let s = g.get_or_insert_with(DarkScreen::default);
            s.set(on);
            s.step(dt)
        }
        Err(_) => 1.0,
    };
    COEFFICIENT.store(if c.is_finite() { c.clamp(0.0, 1.0) } else { 1.0 }.to_bits(), Ordering::Relaxed);
    c
}

/// The master light coefficient the late pass multiplies the scene by (1 = untouched).
pub fn coefficient() -> f32 {
    f32::from_bits(COEFFICIENT.load(Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    /// KH3's timing: x 0.5 reached linearly in 1.0 s (halfway at 0.5 s), held while the finish runs, back to 1 in 1.0 s.
    #[test]
    fn the_finish_dims_linearly_to_half_in_one_second_and_back() {
        let mut d = DarkScreen::default();
        assert_eq!(d.step(DT), 1.0);
        d.set(true);
        let mut v = Vec::new();
        for _ in 0..120 {
            v.push(d.step(DT));
        }
        assert!((v[29] - 0.75).abs() < 1e-4, "0.5 s: {}", v[29]);
        assert!((v[59] - 0.5).abs() < 1e-5, "1.0 s: {}", v[59]);
        assert!(v.windows(2).take(59).all(|w| w[1] < w[0]), "falling for 1 s");
        assert!(v[60..].iter().all(|x| (x - 0.5).abs() < 1e-6), "held");
        d.set(false);
        let back: Vec<f32> = (0..60).map(|_| d.step(DT)).collect();
        assert!((back[29] - 0.75).abs() < 1e-4 && (back[59] - 1.0).abs() < 1e-5, "{back:?}");
    }

    /// A finish that ends inside the fade turns back from where the light is (FROM = the value now, 0x1414F7C50).
    #[test]
    fn a_reversal_mid_fade_starts_from_the_light_now() {
        let mut d = DarkScreen::default();
        d.set(true);
        for _ in 0..30 {
            d.step(DT);
        }
        let at = d.value();
        assert!((at - 0.75).abs() < 1e-4);
        d.set(false);
        let first = d.step(DT);
        assert!(first > at && first < at + 0.01, "{first}");
        for _ in 0..59 {
            d.step(DT);
        }
        assert!((d.value() - 1.0).abs() < 1e-5);
        // the same state again changes nothing
        d.set(false);
        assert_eq!(d.step(DT), 1.0);
    }

    #[test]
    fn the_rate_and_time_are_kh3s() {
        assert_eq!((RATE, TIME), (kh_core::combat::form::DARK_SCREEN_RATE, kh_core::combat::form::DARK_SCREEN_TIME));
    }
}
