//! (VFXR cinematic) A sequence's LOOK, driven over time: what KH3's event matinees change while a spell plays (the
//! post-process volume's `Settings.AutoExposureBias` and `Settings.BloomThreshold` tracks, and the TresGame
//! `TresInterpTrackLightExposure` track on the key light `lt_directional_01`). A sequence sets a target and a blend time;
//! the host blends toward it in real time (frame-rate independent: `step(dt)` once a frame) and applies it.
//!
//! KH3's numbers for Zettaflare (event kg702, matinee `kg702b`, the user's GIF = matinee 58.767 .. 79.633 s; read from
//! `build/sora_tmp/party/ZETTA/data/kg702_dump.txt`, every key CIM_Constant, i.e. a cut, blend 0):
//! - `Settings.AutoExposureBias`: the event's volume holds -6; 52.03 s -6.3, 58.77 s -6.6, 72.83 s -6.0, 74.33 s -6.6
//!   -> `exposure_ev` = bias - (-6): -0.6 from the channel, 0 at the release (72.83), -0.6 from 74.33;
//! - `TresInterpTrackLightExposure` (lt_directional_01): -0.85 is its normal (every cut outside the spell returns to it);
//!   58.77 s -2, 63.50 s -1, 65.87 s -1.2 -> `key_ev` = value - (-0.85): -1.15, -0.15, -0.35;
//! - `Settings.BloomThreshold`: 72.83 s 10, 74.33 s 5 (KH3's default -1 before): `bloom_threshold`;
//! - `Settings.BloomIntensity` -1 at 72.83 s: not applied (a negative intensity has no meaning in UE 4.17's composite;
//!   untraced, taken as "left as it is").
//! KH3's effect materials divide their emissive by the eye adaptation (SQEXEnableEmissiveRevEyeAdaptation): an exposure
//! change leaves them as they are. So `exposure_ev` darkens the WORLD and the characters, not the effects; the hosts
//! follow that (Elden Ring: our characters only, its world is left alone).

use std::sync::Mutex;

/// What a sequence asks for. `NEUTRAL` = the level's own look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// The scene's exposure offset, EV (2^ev on the world's light; the characters' light too).
    pub exposure_ev: f32,
    /// The key light's (sun's) own offset, EV, on top of `exposure_ev`.
    pub key_ev: f32,
    /// UE 4.17's bloom threshold in KH3's exposed units (-1 = KH3's default: everything blooms, half at luminance 0).
    pub bloom_threshold: f32,
}

impl Look {
    pub const NEUTRAL: Look = Look { exposure_ev: 0.0, key_ev: 0.0, bloom_threshold: -1.0 };

    /// The factor on the world's ambient / sky light.
    pub fn exposure_scale(&self) -> f32 {
        self.exposure_ev.exp2()
    }

    /// The factor on the key light (sun).
    pub fn key_scale(&self) -> f32 {
        (self.exposure_ev + self.key_ev).exp2()
    }

    fn toward(self, target: Look, rate: Look, dt: f32) -> Look {
        let go = |now: f32, to: f32, r: f32| if r <= 0.0 { to } else { now + (to - now).clamp(-r * dt, r * dt) };
        Look { exposure_ev: go(self.exposure_ev, target.exposure_ev, rate.exposure_ev), key_ev: go(self.key_ev, target.key_ev, rate.key_ev), bloom_threshold: go(self.bloom_threshold, target.bloom_threshold, rate.bloom_threshold) }
    }
}

impl Default for Look {
    fn default() -> Look {
        Look::NEUTRAL
    }
}

struct State {
    now: Look,
    target: Look,
    /// Per field, units per second (0 = at once).
    rate: Look,
}

static STATE: Mutex<State> = Mutex::new(State { now: Look::NEUTRAL, target: Look::NEUTRAL, rate: Look { exposure_ev: 0.0, key_ev: 0.0, bloom_threshold: 0.0 } });

/// A sequence: blend to `look` over `blend_secs` (0 = at once, a cut). Lasts until the next `set` or [`reset`].
pub fn set(look: Look, blend_secs: f32) {
    if let Ok(mut s) = STATE.lock() {
        let rate = |a: f32, b: f32| if blend_secs > 0.0 { (b - a).abs() / blend_secs } else { 0.0 };
        s.rate = Look { exposure_ev: rate(s.now.exposure_ev, look.exposure_ev), key_ev: rate(s.now.key_ev, look.key_ev), bloom_threshold: rate(s.now.bloom_threshold, look.bloom_threshold) };
        s.target = look;
        if blend_secs <= 0.0 {
            s.now = look;
        }
    }
}

/// Back to the level's own look over `blend_secs`. EVERY way out of a sequence calls it (its end, an interrupt, a KO,
/// a load); [`Look::NEUTRAL`] is also where a host starts.
pub fn reset(blend_secs: f32) {
    set(Look::NEUTRAL, blend_secs);
}

/// The host, once per drawn frame: advances the blend by `dt` real seconds and returns the look to apply.
pub fn step(dt: f32) -> Look {
    match STATE.lock() {
        Ok(mut s) => {
            let (target, rate) = (s.target, s.rate);
            s.now = s.now.toward(target, rate, dt.max(0.0));
            s.now
        }
        Err(_) => Look::NEUTRAL,
    }
}

/// The look as it stands (for a second reader in the same frame).
pub fn current() -> Look {
    STATE.lock().map(|s| s.now).unwrap_or(Look::NEUTRAL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blends_in_real_time_and_resets() {
        // (one test: the state is the process's)
        reset(0.0);
        assert_eq!(current(), Look::NEUTRAL);
        set(Look { exposure_ev: -0.6, key_ev: -1.15, bloom_threshold: 10.0 }, 0.5);
        // the same 0.5 s in 2 frames or in 50: the same place (frame-rate independent)
        let a = { step(0.25); step(0.25) };
        set(Look::NEUTRAL, 0.0);
        set(Look { exposure_ev: -0.6, key_ev: -1.15, bloom_threshold: 10.0 }, 0.5);
        let mut b = Look::NEUTRAL;
        for _ in 0..50 {
            b = step(0.01);
        }
        assert!((a.exposure_ev - b.exposure_ev).abs() < 1e-4 && (a.bloom_threshold - 10.0).abs() < 1e-4 && (b.key_ev + 1.15).abs() < 1e-4);
        // half way
        reset(0.0);
        set(Look { exposure_ev: -1.0, key_ev: 0.0, bloom_threshold: -1.0 }, 1.0);
        assert!((step(0.5).exposure_ev + 0.5).abs() < 1e-4);
        assert!((Look { exposure_ev: -1.0, key_ev: -1.0, bloom_threshold: -1.0 }.key_scale() - 0.25).abs() < 1e-6);
        reset(0.0);
        assert_eq!(step(0.016), Look::NEUTRAL);
    }
}
