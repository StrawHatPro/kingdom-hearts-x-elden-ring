//! The low-HP alarm (docs/sora-audio-spec.md 5.6, route `code.alarm`): while HP <= MaxHP x 25 /
//! 100 (integer) [I 0x1405AD600] the HUD status tick requests the looping static SE 9
//! (`se00000_010`); it starts once 0.8 s AND 15 ticks have passed, stops when no request is
//! left, and pausing stops and re-arms it [I 0x14158A340, 0x14158D800].
//!
//! Ticks are the HUD manager updates, one per frame; the port ticks it once per simulation
//! step (the 60 Hz reference frame).

use crate::rules::RULES;

/// The `ETresStaticSE` id of the alarm loop.
pub const ALARM_STATIC_SE: u32 = 9;

/// HP <= MaxHP x 25 / 100 in integer math.
pub fn low_hp(hp: i32, hp_max: i32) -> bool {
    hp <= hp_max * RULES.alarm_hp_threshold_percent as i32 / 100
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlarmAction {
    None,
    Start,
    Stop,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LowHpAlarm {
    /// The request bits (H+0x258): bit 1 is the player status slot.
    requests: u8,
    timer: f32,
    ticks: i32,
    playing: bool,
}

impl LowHpAlarm {
    pub fn playing(&self) -> bool {
        self.playing
    }

    pub fn requested(&self) -> bool {
        self.requests != 0
    }

    fn arm(&mut self) {
        self.timer = RULES.alarm_start_delay;
        self.ticks = RULES.alarm_start_delay_ticks;
    }

    /// Sets or clears a request bit (0x14158A340 / 0x14158D800).
    pub fn request(&mut self, bit: u8, on: bool) {
        let before = self.requests;
        if on {
            self.requests |= bit;
        } else {
            self.requests &= !bit;
        }
        if before == 0 && self.requests != 0 {
            self.arm();
        }
    }

    /// One tick (0x14158A800).
    pub fn tick(&mut self, dt: f32, paused: bool) -> AlarmAction {
        if self.requests == 0 || paused {
            if self.requests != 0 {
                self.arm();
            }
            if self.playing {
                self.playing = false;
                return AlarmAction::Stop;
            }
            return AlarmAction::None;
        }
        if self.playing {
            return AlarmAction::None;
        }
        self.timer -= dt;
        self.ticks -= 1;
        if self.timer <= 0.0 && self.ticks <= 0 {
            self.playing = true;
            return AlarmAction::Start;
        }
        AlarmAction::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_threshold_is_integer_math() {
        assert!(low_hp(25, 100) && !low_hp(26, 100));
        // 135 * 25 / 100 = 33.
        assert!(low_hp(33, 135) && !low_hp(34, 135));
    }

    #[test]
    fn starts_after_0_8_s_and_15_ticks_and_stops_with_the_last_request() {
        let mut a = LowHpAlarm::default();
        a.request(1, true);
        let mut started = None;
        for tick in 1..=100 {
            if a.tick(1.0 / 60.0, false) == AlarmAction::Start {
                started = Some(tick);
                break;
            }
        }
        // 0.8 s at 60 Hz is 48 ticks: the time is what holds it back, not the 15 ticks.
        assert!(matches!(started, Some(48 | 49)), "{started:?}");
        // At 5 fps the 15 ticks are.
        let mut b = LowHpAlarm::default();
        b.request(1, true);
        assert_eq!((1..=100).find(|_| b.tick(0.2, false) == AlarmAction::Start), Some(15));
        assert_eq!(a.tick(1.0 / 60.0, false), AlarmAction::None);
        a.request(1, false);
        assert_eq!(a.tick(1.0 / 60.0, false), AlarmAction::Stop);
        assert!(!a.playing());
    }

    #[test]
    fn pausing_stops_and_rearms_it() {
        let mut a = LowHpAlarm::default();
        a.request(1, true);
        while a.tick(0.1, false) != AlarmAction::Start {}
        assert_eq!(a.tick(0.1, true), AlarmAction::Stop);
        assert_eq!(a.tick(0.1, true), AlarmAction::None);
        // The full delay again after the pause.
        assert_eq!((1..=100).find(|_| a.tick(0.1, false) == AlarmAction::Start), Some(15));
    }
}
