//! (LADDER2 2026-10-25) THE LADDER JUMP'S ARC (PORT DESIGN: KH3 has no ladders). Pure, a function of time only (frame-rate
//! independent). Shared by the hosts with ladders: kh-party's ladder jump of a party member (kh-party ladder.rs) and the
//! Elden Ring host's leap of Sora (kh-er ladder.rs). The user (2026-10-25): "make them jump up the entire height of the
//! ladder and safely land on top of it".
//!
//! 1. WAIT `delay` s; 2. START: on the floor for `START_S` s (KH3's JumpNormalStart), turning toward the heading; 3. RISE /
//! FALL: a parabola under `GRAVITY` whose top is `CLEAR` cm above the higher end; the horizontal move to the landing starts
//! only once he is `FOOT` cm above the landing floor (the arc never cuts through the ledge the ladder leans on); 4. LAND on
//! the landing facing the heading for `LAND_S` s; then Done.

use glam::Vec3;

/// [D] port tunables (cm, s).
pub const GRAVITY: f32 = 2400.0;
pub const CLEAR: f32 = 80.0;
pub const START_S: f32 = 0.12;
pub const LAND_S: f32 = 0.3;
pub const FOOT: f32 = 40.0;

/// Where the leap is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Wait,
    Start,
    Rise,
    Fall,
    Land,
    Done,
}

/// One moment of a leap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub position: Vec3,
    pub yaw: f32,
    pub velocity: Vec3,
    pub phase: Phase,
}

/// THE ARC (pure): from `from` (his origin as his movement holds it) to `to` (the same origin over the landing floor).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Leap {
    pub from: Vec3,
    pub to: Vec3,
    pub from_yaw: f32,
    pub facing: f32,
    pub delay: f32,
    /// Seconds since the leap was given.
    pub t: f32,
    /// He left the floor during the last `advance` (the host plays the jump's sound then; cleared by the host).
    pub took_off: bool,
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

pub fn wrap180(a: f32) -> f32 {
    (a + 180.0).rem_euclid(360.0) - 180.0
}

fn lerp_yaw(a: f32, b: f32, f: f32) -> f32 {
    wrap180(a + wrap180(b - a) * f)
}

impl Leap {
    pub fn new(from: Vec3, from_yaw: f32, to: Vec3, facing: f32, delay: f32) -> Leap {
        Leap { from, to, from_yaw, facing: wrap180(facing), delay: delay.max(0.0), t: 0.0, took_off: false }
    }

    /// The top of the arc (cm).
    pub fn apex(&self) -> f32 {
        self.from.z.max(self.to.z) + CLEAR
    }

    /// The take-off speed (cm/s up).
    pub fn v0(&self) -> f32 {
        (2.0 * GRAVITY * (self.apex() - self.from.z)).sqrt()
    }

    pub fn t_up(&self) -> f32 {
        self.v0() / GRAVITY
    }

    pub fn t_down(&self) -> f32 {
        (2.0 * (self.apex() - self.to.z) / GRAVITY).sqrt()
    }

    /// Seconds in the air.
    pub fn flight(&self) -> f32 {
        self.t_up() + self.t_down()
    }

    /// From the leap's start to the end of the landing.
    pub fn total(&self) -> f32 {
        self.delay + START_S + self.flight() + LAND_S
    }

    /// The flight time at which the horizontal move begins: he is `FOOT` cm above the landing floor (at once when he
    /// starts that high).
    fn t_across(&self) -> f32 {
        let h = self.to.z + FOOT - self.from.z;
        if h <= 0.0 {
            return 0.0;
        }
        let v0 = self.v0();
        let d = (v0 * v0 - 2.0 * GRAVITY * h).max(0.0);
        ((v0 - d.sqrt()) / GRAVITY).min(self.t_up())
    }

    /// The leap at `t` s after it was given.
    pub fn sample_at(&self, t: f32) -> Sample {
        let s = t - self.delay;
        if s < 0.0 {
            return Sample { position: self.from, yaw: self.from_yaw, velocity: Vec3::ZERO, phase: Phase::Wait };
        }
        if s < START_S {
            return Sample { position: self.from, yaw: lerp_yaw(self.from_yaw, self.facing, 0.5 * s / START_S), velocity: Vec3::ZERO, phase: Phase::Start };
        }
        let f = s - START_S;
        let (up, flight) = (self.t_up(), self.flight());
        if f < flight {
            let v0 = self.v0();
            let z = self.from.z + v0 * f - 0.5 * GRAVITY * f * f;
            let across = self.t_across();
            let span = (flight - across).max(1e-3);
            let h = smooth((f - across) / span);
            let dh = if f > across { let x = ((f - across) / span).clamp(0.0, 1.0); 6.0 * x * (1.0 - x) / span } else { 0.0 };
            let flat = self.to - self.from;
            let xy = Vec3::new(self.from.x + flat.x * h, self.from.y + flat.y * h, z);
            let velocity = Vec3::new(flat.x * dh, flat.y * dh, v0 - GRAVITY * f);
            let yaw = lerp_yaw(self.from_yaw, self.facing, 0.5 + 0.5 * smooth(f / up.max(1e-3)));
            return Sample { position: xy, yaw, velocity, phase: if f < up { Phase::Rise } else { Phase::Fall } };
        }
        let phase = if f < flight + LAND_S { Phase::Land } else { Phase::Done };
        Sample { position: self.to, yaw: self.facing, velocity: Vec3::ZERO, phase }
    }

    pub fn sample(&self) -> Sample {
        self.sample_at(self.t)
    }

    /// One step: the new moment; `took_off` is set when he left the floor in it.
    pub fn advance(&mut self, dt: f32) -> Sample {
        let before = self.sample().phase;
        self.t += dt.max(0.0);
        let now = self.sample();
        if matches!(before, Phase::Wait | Phase::Start) && !matches!(now.phase, Phase::Wait | Phase::Start) {
            self.took_off = true;
        }
        now
    }

    pub fn done(&self) -> bool {
        self.sample().phase == Phase::Done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    /// An 11 m ladder as the user's log has it (the top 1 m ahead of the bottom): up first, over the lip, down onto the top
    /// floor facing the ladder's heading; the arc clears the lip.
    #[test]
    fn the_leap_goes_up_then_over_onto_the_landing() {
        let from = Vec3::new(0.0, 0.0, 90.0);
        let to = Vec3::new(100.0, 0.0, 1190.0);
        let mut l = Leap::new(from, 170.0, to, 0.0, 0.0);
        let mut phases = Vec::new();
        let mut highest = f32::MIN;
        let mut took = 0;
        while !l.done() {
            let s = l.advance(DT);
            took += u32::from(std::mem::take(&mut l.took_off));
            if phases.last() != Some(&s.phase) {
                phases.push(s.phase);
            }
            highest = highest.max(s.position.z);
            // nothing moves forward while he is below the landing floor + FOOT
            if s.position.z < to.z + FOOT - 1.0 && s.phase == Phase::Rise {
                assert!(s.position.x.abs() < 1e-3, "{s:?}");
            }
            assert!(l.t < 10.0);
        }
        assert_eq!(phases, vec![Phase::Start, Phase::Rise, Phase::Fall, Phase::Land, Phase::Done]);
        assert_eq!(took, 1, "one take-off");
        assert!((highest - (to.z + CLEAR)).abs() < 2.0, "{highest}");
        let end = l.sample();
        assert_eq!(end.position, to);
        assert_eq!(end.yaw, 0.0);
        assert!(l.total() < 2.2, "an 11 m leap takes {:.2} s", l.total());
    }

    /// The same moment at 30 and 144 Hz (a function of time only).
    #[test]
    fn the_leap_is_frame_rate_independent() {
        let mk = || Leap::new(Vec3::ZERO, -90.0, Vec3::new(-50.0, 80.0, 400.0), 120.0, 0.25);
        let (mut a, mut b) = (mk(), mk());
        for _ in 0..30 {
            a.advance(1.0 / 30.0);
        }
        for _ in 0..144 {
            b.advance(1.0 / 144.0);
        }
        let (sa, sb) = (a.sample(), b.sample());
        assert!(sa.position.distance(sb.position) < 0.01 && (sa.yaw - sb.yaw).abs() < 0.01, "{sa:?} {sb:?}");
    }

    /// The wait: he stands until his turn; every heading lands exactly on the facing asked.
    #[test]
    fn the_wait_and_every_heading() {
        let l = Leap::new(Vec3::new(5.0, 5.0, 0.0), 30.0, Vec3::new(0.0, 0.0, 300.0), 0.0, 0.4);
        assert_eq!(l.sample_at(0.3).phase, Phase::Wait);
        assert_eq!(l.sample_at(0.3).position, Vec3::new(5.0, 5.0, 0.0));
        assert_eq!(l.sample_at(0.45).phase, Phase::Start);
        for facing in [-179.0, -90.0, -45.0, 0.0, 45.0, 90.0, 135.0, 180.0] {
            let l = Leap::new(Vec3::ZERO, facing + 150.0, Vec3::new(30.0, -70.0, 500.0), facing, 0.0);
            let end = l.sample_at(l.total() + 0.01);
            assert_eq!(end.phase, Phase::Done);
            assert!(wrap180(end.yaw - facing).abs() < 1e-3, "{facing} -> {}", end.yaw);
        }
    }
}
