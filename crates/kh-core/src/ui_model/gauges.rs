//! The status HUD's gauges (docs/sora-ui-spec.md section 4, `UTresUIP_HudStatus`). The gauges
//! are timelines with one frame per gauge length and THE NATIVE SIDE COMPUTES THE FRAME: this
//! file is that computation. Constants are the rows of `sheets/port/sora-ui-params` named at
//! each (the sheet is still being written by the UI research side and is not compiled against).

use crate::status::Status;

/// `gauge.hp_frames_per_point` 2 and `gauge.hp_max_frames` 668 (the clip has 669 frames).
pub const HP_FRAMES_PER_POINT: f32 = 2.0;
pub const HP_MAX_FRAMES: u32 = 668;
/// `gauge.mp_frames_per_point` 1.8 and `gauge.mp_max_frames` 500 (clip 501).
pub const MP_FRAMES_PER_POINT: f32 = 1.8;
pub const MP_MAX_FRAMES: u32 = 500;
/// `gauge.focus_frames` 100 (clip 101).
pub const FOCUS_FRAMES: u32 = 100;
/// `gauge.red_delay` 0.8 s, `gauge.red_tick` 0.0333 s, `gauge.red_speed_base` 10.
pub const RED_DELAY: f32 = 0.8;
pub const RED_TICK: f32 = 0.0333;
pub const RED_STEPS: f32 = 10.0;
/// The follow moves by at least this many frames per tick (AS3 `FollowGauge`).
pub const RED_MIN_STEP: f32 = 0.1;

/// THE FRAME HELPER `F(v, max, frames)` 0x141598CA0 [I]: 0 if `v < 1` or `max < 1`; `frames` if
/// `v == max`; else `trunc(v * frames / max + 0.5)` (float) clamped to `[1, frames - 1]`: a
/// nearly empty gauge never looks empty and a nearly full one never looks full. THE CLIP
/// RECEIVES `F + 1` (frame 1 = empty).
pub fn gauge_frame(value: i32, max: i32, frames: u32) -> u32 {
    if value < 1 || max < 1 {
        return 0;
    }
    if value == max {
        return frames;
    }
    let f = (value as f32 * frames as f32 / max as f32 + 0.5) as i32;
    f.clamp(1, (frames as i32 - 1).max(1)) as u32
}

/// `hpMaxFrames = min(668, trunc(maxHP + maxHP))`: two frames per HP point, longest at 334.
pub fn hp_max_frames(max_hp: i32) -> u32 {
    ((max_hp.max(0) as f32 * HP_FRAMES_PER_POINT) as u32).min(HP_MAX_FRAMES)
}

/// `mpMaxFrames = min(500, trunc(maxMP * 1.8))`: longest at 277.
pub fn mp_max_frames(max_mp: i32) -> u32 {
    ((max_mp.max(0) as f32 * MP_FRAMES_PER_POINT) as u32).min(MP_MAX_FRAMES)
}

/// The numbers the status HUD shows. Frame fields are CLIP frames (`F + 1`; 1 = empty).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Gauges {
    pub level: u32,
    /// Total EXP.
    pub exp: i32,
    /// EXP still needed for the next level ("Next LV"); 0 at level 99.
    pub exp_to_next: i32,
    /// Fraction of the current level's EXP gauge, 0..1.
    pub exp_fraction: f32,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    /// The focus (shotlock) gauge.
    pub focus: i32,
    pub focus_max: i32,
    /// `pcHpDodai`: the base's frame = `hpMaxFrames + 1`.
    pub hp_base_frame: u32,
    /// `pcHpGg`: `F(hp, maxHP, hpMaxFrames) + 1`.
    pub hp_frame: u32,
    /// `pcHpGgR`: the red damage-lag gauge's frame (fractional while it follows).
    pub hp_red_frame: f32,
    /// `pcMpDodai`: `mpMaxFrames + 1`.
    pub mp_base_frame: u32,
    /// `pcMpGg`: `F(mp, maxMP, mpMaxFrames) + 1`; no lag gauge.
    pub mp_frame: u32,
    /// The pawn reports MP Charge (`vt+0x17E0`): the pack plays `charge_in` -> `charge_loop`.
    pub mp_charging: bool,
    /// The charge value (`vt+0x17F8`): `timer / T * maxMP`.
    pub mp_charge_value: f32,
    /// `pcMpGgC`, the pink gauge: `F(trunc(chargeValue), maxMP, mpMaxFrames) + 1`.
    pub mp_charge_frame: u32,
    /// `pcExGg`: `F(focus, maxFocus, 100) + 1`.
    pub focus_frame: u32,
    /// `pcExGgR`: the focus gauge's red lag frame.
    pub focus_red_frame: f32,
    /// `isMax`: focus == max (`max_in` -> `max_loop`).
    pub focus_is_max: bool,
    /// PINCH 0x1405AD600: `hp <= maxHP * 25 / 100` (integer): the dying portrait and the low-HP
    /// alarm. The gauge itself has no flashing frames.
    pub pinch: bool,
    /// AP in use / available, and the final stats (the camp menu's numbers; debug HUD).
    pub ap: i32,
    pub ap_max: i32,
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
}

impl Gauges {
    /// The gauges of a status; the red frames come from the lag model (`RedLag::frame`), 0 =
    /// take the fill's frame.
    pub fn of(status: &Status, hp_red: f32, focus_red: f32) -> Gauges {
        if status.placeholder {
            return Gauges::default();
        }
        let (hp_frames, mp_frames) = (hp_max_frames(status.hp_max), mp_max_frames(status.mp_max));
        let hp_frame = gauge_frame(status.hp, status.hp_max, hp_frames) + 1;
        let focus_frame = gauge_frame(status.focus, status.focus_max, FOCUS_FRAMES) + 1;
        let charge = status.mp_charge_gauge();
        Gauges {
            level: status.level,
            exp: status.exp,
            exp_to_next: status.exp_to_next(),
            exp_fraction: status.exp_fraction(),
            hp: status.hp,
            hp_max: status.hp_max,
            mp: status.mp,
            mp_max: status.mp_max,
            focus: status.focus,
            focus_max: status.focus_max,
            hp_base_frame: hp_frames + 1,
            hp_frame,
            hp_red_frame: if hp_red > 0.0 { hp_red } else { hp_frame as f32 },
            mp_base_frame: mp_frames + 1,
            mp_frame: gauge_frame(status.mp, status.mp_max, mp_frames) + 1,
            mp_charging: status.mp_charging,
            mp_charge_value: charge,
            mp_charge_frame: gauge_frame(charge as i32, status.mp_max, mp_frames) + 1,
            focus_frame,
            focus_red_frame: if focus_red > 0.0 { focus_red } else { focus_frame as f32 },
            focus_is_max: status.focus_max > 0 && status.focus == status.focus_max,
            pinch: status.is_pinch(),
            ap: status.stats.ap,
            ap_max: status.stats.ap_max,
            strength: status.stats.strength,
            magic: status.stats.magic,
            defense: status.stats.defense,
        }
    }
}

/// THE RED DAMAGE-LAG GAUGE (`pcHpGgR`; UI spec section 4, AS3 `FollowGauge`), as a
/// deterministic fixed-step model. When the fill's frame drops, the red gauge stays where it
/// is for 0.8 s, then every 33.3 ms moves by `(target - start) / 10` frames (at least 0.1)
/// until the target: about a third of a second. A heal, or the slot's first appearance, sets it
/// to the new frame at once. A new hit during the follow keeps the red position and retargets
/// (the step becomes a tenth of the new distance; a running wait is not restarted, a running
/// follow goes on). The game's timer is wall-clock; here it advances by the step's dt.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RedLag {
    /// Where the red gauge is, in clip frames; 0 = not shown yet.
    red: f32,
    /// The fill frame it is following to.
    target: f32,
    /// Frames per tick (negative: moving down).
    step: f32,
    /// Seconds of the 0.8 s wait left; meaningful while `phase == 1`.
    delay: f32,
    /// Seconds since the last tick; meaningful while `phase == 2`.
    tick: f32,
    /// 0 idle, 1 waiting, 2 following.
    phase: u8,
}

impl RedLag {
    /// The red gauge's clip frame (0 before the first update).
    pub fn frame(&self) -> f32 {
        self.red
    }

    /// True while it waits or moves.
    pub fn active(&self) -> bool {
        self.phase != 0
    }

    /// One step: `fill` = the fill gauge's clip frame now, `top` = the base's clip frame.
    pub fn update(&mut self, dt: f32, fill: u32, top: u32) {
        let fill = fill as f32;
        // start = min(frame sent last, hpMaxFrames): the red gauge never stands past the base.
        if self.red > top as f32 {
            self.red = top as f32;
        }
        if self.red <= 0.0 || fill >= self.red {
            // First appearance, or a heal (also a heal past the red gauge during a follow).
            *self = RedLag { red: fill, target: fill, ..RedLag::default() };
            return;
        }
        if fill != self.target {
            // A hit: retarget from where the red gauge is.
            self.target = fill;
            let step = (self.target - self.red) / RED_STEPS;
            self.step = if step.abs() < RED_MIN_STEP { -RED_MIN_STEP } else { step };
            if self.phase == 0 {
                self.phase = 1;
                self.delay = RED_DELAY;
                return; // the wait starts with this step's end
            }
        }
        match self.phase {
            1 => {
                self.delay -= dt;
                if self.delay <= 1.0e-4 {
                    self.phase = 2;
                    self.tick = 0.0;
                }
            }
            2 => {
                self.tick += dt;
                while self.phase == 2 && self.tick >= RED_TICK - 1.0e-5 {
                    self.tick -= RED_TICK;
                    self.red += self.step;
                    if self.red <= self.target + 1.0e-3 {
                        self.red = self.target;
                        self.phase = 0;
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_helper_never_shows_empty_or_full_in_between() {
        // UI spec section 4: F(v, max, frames).
        assert_eq!((gauge_frame(0, 250, 500), gauge_frame(250, 250, 500), gauge_frame(1, 250, 500), gauge_frame(249, 250, 500)), (0, 500, 2, 498));
        assert_eq!((gauge_frame(1, 1000, 100), gauge_frame(999, 1000, 100), gauge_frame(5, 0, 100)), (1, 99, 0));
        assert_eq!((hp_max_frames(250), hp_max_frames(334), hp_max_frames(400), mp_max_frames(179), mp_max_frames(277), mp_max_frames(300)), (500, 668, 668, 322, 498, 500));
    }
}
