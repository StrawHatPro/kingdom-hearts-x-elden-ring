//! (party X2) DECISION 9 (the user, 2026-10-15; XD_PLAYABLE_KITS.md 4.8): A FRIEND LEADER SPRINTS AS SORA DOES. KH3's
//! sprint is a player-pawn field (`MySprintSpeed`; the NPC pawns have none), so giving it to Donald / Goofy is the
//! port's design; the RULE is Sora's Run state (kh-core states/run.rs sub-step 0, the run tick 0x1413AFA34): while he
//! runs (not walking, no turn-around) a timer counts; past `MySprintSpeedDelay` (1.0 s) the speed limit
//! `MaxWalkSpeed` becomes `MySprintSpeed` (980 cm/s); a walk tick, a turn-around, a stop or leaving the run state
//! resets it to the run speed. The speed ramp (dec015) and the speed-matched loop (dec016) of the pad's run read
//! `cmc.max_walk_speed`, so the walk gait scales and the run cycle plays faster with the speed, as Sora's do.

use crate::leader_magic::PadMagic;
use kh_core::params::SORA_PARAMS;

/// One run tick (after the run's sub-step dispatch): `running` = sub-step 0 and not walking; `state_time` = seconds
/// in the run state (a re-entered state resets the timer). `run_speed` = the member's own run speed (680).
pub fn tick(x2: &mut PadMagic, running: bool, state_time: f32, dt: f32, run_speed: f32, max_walk_speed: &mut f32) {
    if !running || state_time < x2.sprint_timer - dt * 2.0 {
        x2.sprint_timer = 0.0;
    }
    if running {
        x2.sprint_timer += dt;
    }
    x2.sprinting = running && x2.sprint_timer > SORA_PARAMS.speed_sprint_delay;
    *max_walk_speed = if x2.sprinting { SORA_PARAMS.speed_sprint } else { run_speed };
}

/// Out of the run state: no sprint.
pub fn reset(x2: &mut PadMagic) {
    x2.sprint_timer = 0.0;
    x2.sprinting = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprint_after_the_delay_and_reset_by_a_walk() {
        let mut x2 = PadMagic::default();
        let mut speed = 680.0;
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        for _ in 0..55 {
            t += dt;
            tick(&mut x2, true, t, dt, 680.0, &mut speed);
        }
        assert_eq!(speed, 680.0, "not before {} s", SORA_PARAMS.speed_sprint_delay);
        for _ in 0..10 {
            t += dt;
            tick(&mut x2, true, t, dt, 680.0, &mut speed);
        }
        assert!(x2.sprinting && speed == SORA_PARAMS.speed_sprint && speed == 980.0);
        tick(&mut x2, false, t, dt, 680.0, &mut speed);
        assert!(!x2.sprinting && speed == 680.0 && x2.sprint_timer == 0.0);
        // a re-entered run state starts over
        for _ in 0..70 {
            t += dt;
            tick(&mut x2, true, t, dt, 680.0, &mut speed);
        }
        tick(&mut x2, true, 0.0, dt, 680.0, &mut speed);
        assert!(!x2.sprinting);
    }
}
