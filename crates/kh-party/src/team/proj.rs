//! OWNER: party stream D. The link pawns' projectiles in flight (Donald Meteor's meteors,
//! Flare Force's missiles): real actors that move, home, touch the map or a target and die, with
//! their body effect following them (the host draws it through `party_fx::publish_projectiles`,
//! class -> effect rows in `build/sora_pack/party/team/team_host.json`, tools/party_team_pack.js).
//!
//! Evidence (tags as the plan's):
//! - `CallMeteor_Proj` CDO [A]: `TresProjectileMovementComponent` InitialSpeed 1200, m_Accel 2400,
//!   MaxSpeed 4000; m_MainLife 3, m_bExplodeOnTimeOut (no explosion class on it: nothing spawns);
//!   attack sphere 150 cm, row FR_CALLMETEOR_FINISH, destroyed on a pawn hit (no
//!   m_bNoDestroyOnHit); hit effect vn_ex001_fcm_hit00 on pawns AND on the map (m_bChangeMapHit,
//!   m_MapHitEffect, decal 256 / 10 s); body ParticleComp vn_ex001_fcm_emi00 (activated at the
//!   fire, link pawn Finish tick 0x1412DA0B0 [C]); mesh o_ex210_0 (a static mesh: not drawn by the
//!   port, see the state note). The meteor stops at the map [D: its root's collision profile was
//!   not read; the map hit effect says it touches it].
//! - `FlareForce_Proj` CDO [A]: attack sphere 25 cm (group "Missile", row FR_FLAREFORCE_FINISH01),
//!   MaxSpeed 1000 on its movement; FlareForce_Pawn m_fMaxSpeed 1200, m_fHomingAngle 1000,
//!   m_fHomingUpdateRange 100, m_fLifeTime 10 (ctor 0x1412C6960 [C]); body vn_ex001_fff_dou00,
//!   MyFireEffects vn_ex001_fff_spn00; mesh o_ex208_0 (skeletal; drawn by kh-app's team props, D3).
//!   (D3) The missiles do not use `fly`: their machine is the link pawn's tick override 0x1412E72B0
//!   (link.rs `flare_missile`) with the homing velocity of FUN_1412E82B0 (`flare_homing` below)
//!   and `sweep_move`.

use glam::Vec3;
use kh_core::host::party::SeamHandles;
use kh_core::host::{Attach, EffectHandle, EffectId, EffectSpawn, Effects, TargetId, TargetInfo};
use kh_core::world::World;

/// Which projectile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjKind {
    Meteor,
    Missile,
}

impl ProjKind {
    /// The blueprint class (the host's class -> effects table).
    pub const fn class(self) -> &'static str {
        match self {
            ProjKind::Meteor => "CallMeteor_Proj_C",
            ProjKind::Missile => "FlareForce_Proj_C",
        }
    }
}

/// What ended a flight this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Contact {
    Pawn { target: TargetId, at: Vec3 },
    Map { at: Vec3 },
    Timeout { at: Vec3 },
}

/// One projectile of a link pawn.
#[derive(Clone, Debug, PartialEq)]
pub struct TeamProj {
    pub kind: ProjKind,
    /// Index in the link pawn's info table (`m_MeteorInfos` / `m_MissileInfos`).
    pub index: usize,
    pub position: Vec3,
    /// Facing (unit): the actor's X axis.
    pub forward: Vec3,
    pub speed: f32,
    pub accel: f32,
    pub max_speed: f32,
    /// Seconds left before the timeout.
    pub life: f32,
    /// Attack sphere radius (cm) and row.
    pub radius: f32,
    pub row: &'static str,
    pub target: TargetId,
    pub target_point: Option<Vec3>,
    pub homing_turn: f32,
    pub homing_stop: f32,
    /// Actor scale (the meteors grow from 0.01 to 1 while they appear).
    pub scale: f32,
    pub visible: bool,
    pub flying: bool,
    pub alive: bool,
    /// The appear / fire times on the link pawn's timer (s).
    pub appear_at: f32,
    pub fire_at: f32,
    /// The body effect (0 = none) and the appear effect (the meteor's m_FireEffect, attached).
    pub effect: u32,
    pub appear_effect: u32,
    /// Flare Force missile machine (`m_MissileInfos[i]` +0x0C.. [C 0x1412E72B0]): velocity (the
    /// projectile movement's), the speed-rate / turn-rate ramps (+0x18 / +0x1C, set to 10 / 2 pi
    /// at the show), the launch direction (+0x40), the state (+0x14: 1 hidden, 2 appear, 3 fire
    /// recoil, 4 to the up point, 5 homing, 10 explode), its countdown (+0x0C), the curve time
    /// (+0x10) and the flags (+0x20: 1 no target, 2 target lost).
    pub velocity: Vec3,
    pub rate: f32,
    pub turn: f32,
    pub launch_dir: Vec3,
    pub mstate: u8,
    pub countdown: f32,
    pub curve_t: f32,
    /// (D4) The info flag bit 0 (+0x20): set by FUN_1412E43E0 when the missile is the FIRST to take
    /// its target, or by state 4 0x1412E6530 when it has none: state 5 homes with the fast law
    /// (0.7 .. 1.0 x max speed, no ramp) [C].
    pub no_target: bool,
    pub target_lost: bool,
    /// (party D5) Call Meteor's hover (`m_MeteorInfos[i]` +0x14 spawn location, +0x24 the shake curve
    /// time in `curve_t`, flag 8 = `shake`): while set, the pawn's Tick override 0x1412E6E70 puts the
    /// meteor at `base` + m_ShakeCurveData(t) each tick [C]. `spin`: the actor rotation of the spawn spin
    /// (FUN_1412CAF60 SetActorRotation R(i x 30, 0, j x 30)) as (pitch, roll), degrees, drawn instead of
    /// `forward` while set; the Finish's placement
    /// (0x1412E3900: Rotation(direction), roll 0) clears it [C].
    pub base: Vec3,
    pub shake: bool,
    pub spin: Option<(f32, f32)>,
}

impl TeamProj {
    pub fn new(kind: ProjKind, index: usize, position: Vec3, forward: Vec3) -> TeamProj {
        let (radius, row, life) = match kind {
            ProjKind::Meteor => (150.0, "FR_CALLMETEOR_FINISH", 3.0),
            ProjKind::Missile => (25.0, "FR_FLAREFORCE_FINISH01", 10.0),
        };
        TeamProj { kind, index, position, forward, speed: 0.0, accel: 0.0, max_speed: 0.0, life, radius, row, target: TargetId::NONE, target_point: None, homing_turn: 0.0, homing_stop: 0.0, scale: 0.01, visible: false, flying: false, alive: true, appear_at: 0.0, fire_at: f32::MAX, effect: 0, appear_effect: 0, velocity: Vec3::ZERO, rate: 0.0, turn: 0.0, launch_dir: forward, mstate: 1, countdown: 0.0, curve_t: 0.0, no_target: false, target_lost: false, base: position, shake: false, spin: None }
    }

    /// Shows the body effect (persistent, follows the actor).
    pub fn show_body(&mut self, ids: &mut SeamHandles, effects: &mut dyn Effects) {
        if self.effect == 0 {
            let h = ids.effect();
            let name = self.kind.class();
            effects.spawn(h, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(self.position), rotation: Vec3::ZERO, scale: 1.0, persistent: true });
            self.effect = h.0;
        }
    }

    /// Kills the projectile and its effects (fading as authored).
    pub fn kill(&mut self, effects: &mut dyn Effects) {
        self.alive = false;
        self.flying = false;
        for h in [self.effect, self.appear_effect] {
            if h != 0 {
                effects.stop(EffectHandle(h), false);
            }
        }
        self.effect = 0;
        self.appear_effect = 0;
    }

    /// One step of flight: homing, acceleration, the sweep against the map and the targets.
    pub fn fly(&mut self, dt: f32, world: &dyn World, targets: &[TargetInfo]) -> Option<Contact> {
        if !self.alive || !self.flying {
            return None;
        }
        if self.homing_turn > 0.0 {
            if let Some(goal) = self.target_point {
                let to = goal - self.position;
                if to.length() > self.homing_stop {
                    self.forward = turn_toward(self.forward, to.normalize_or_zero(), self.homing_turn.to_radians() * dt);
                }
            }
        }
        self.speed = (self.speed + self.accel * dt).min(self.max_speed.max(self.speed));
        let p0 = self.position;
        let p1 = p0 + self.forward * self.speed * dt;
        // Targets first (a pawn hit destroys it before the map is reached in the same step).
        let mut best: Option<(f32, Contact)> = None;
        for t in targets {
            let (a, b) = (t.center - Vec3::Z * (t.half_height - t.radius).max(0.0), t.center + Vec3::Z * (t.half_height - t.radius).max(0.0));
            let (s, d) = segment_segment(p0, p1, a, b);
            if d <= self.radius + t.radius && best.as_ref().is_none_or(|(bs, _)| s < *bs) {
                best = Some((s, Contact::Pawn { target: t.id, at: p0 + (p1 - p0) * s }));
            }
        }
        if let Some(hit) = world.line_trace(p0, p1) {
            if best.as_ref().is_none_or(|(s, _)| hit.time < *s) {
                best = Some((hit.time, Contact::Map { at: hit.position }));
            }
        }
        self.position = best.as_ref().map_or(p1, |(s, _)| p0 + (p1 - p0) * *s);
        self.life -= dt;
        match best {
            Some((_, c)) => Some(c),
            None if self.life <= 0.0 => Some(Contact::Timeout { at: self.position }),
            None => None,
        }
    }
}

impl TeamProj {
    /// `FUN_1412E82B0(info, missile, dt, rate, turn, min, max, cos_min, slow)` [C]: the Flare Force
    /// missile's homing velocity toward `target_point` (the info's aim, +0x2C). The wanted direction
    /// is the one to the aim (with `slow` > 0 blended toward the launch direction near the aim:
    /// Slerp(look, +0x40, 1 - min(d / slow, 1))); the rate / turn ramps step by dt / 6 dt when
    /// farther than a step from `rate` / `turn` (the step is ADDED without its sign, as the code
    /// does, 0x1412E8714..0x1412E875A); the speed goes `rate x dt` of the way to its goal (`max`
    /// when not turning, `min` when the wanted direction is more than acos(cos_min) away, else
    /// (cos - cos_min)(max - min) / (1 - cos_min) + 2 min as decompiled), clamped to min..max; the
    /// direction turns at most `turn x dt` toward the wanted one. Returns the distance of the
    /// explosion test: d x ((1 - cos) / 2 + 1), or 0 within 100 cm (and one step) of a lost target.
    #[allow(clippy::too_many_arguments)]
    pub fn flare_homing(&mut self, dt: f32, rate: f32, turn: f32, min: f32, max: f32, cos_min: f32, slow: f32) -> f32 {
        let aim = self.target_point.unwrap_or(self.position);
        let to = aim - self.position;
        let dist = to.length();
        let wanted = if to.abs().max_element() > 1e-4 {
            let look = to / dist.max(1e-6);
            if slow <= 0.0 {
                look
            } else {
                let k = (dist / slow).min(1.0);
                slerp_dir(look, self.launch_dir, 1.0 - k)
            }
        } else {
            self.forward
        };
        if (rate - self.rate).abs() > dt {
            self.rate += dt;
        } else {
            self.rate = rate;
        }
        if (turn - self.turn).abs() > dt * 6.0 {
            self.turn += dt * 6.0;
        } else {
            self.turn = turn;
        }
        let angle = self.turn * dt;
        let current = if self.velocity.abs().max_element() > 1e-4 { self.velocity.normalize() } else { wanted };
        let cos = wanted.dot(current);
        let speed = self.velocity.length();
        let goal = if angle > 0.0 {
            if cos >= cos_min {
                (cos - cos_min) * (max - min) / (1.0 - cos_min) + min + min
            } else {
                min
            }
        } else {
            max
        };
        let speed = (self.rate * dt * (goal - speed) + speed).max(min).min(max);
        let dir = if angle > 0.0 && cos < angle.cos() { turn_toward(current, wanted, angle) } else { wanted };
        self.velocity = dir * speed;
        self.forward = dir;
        self.speed = speed;
        if !self.target_lost {
            dist * ((1.0 - cos) * 0.5 + 1.0)
        } else if dist <= 100.0 && dist <= speed {
            0.0
        } else {
            dist
        }
    }

    /// Moves by `velocity x dt` with the sweep against the targets and the map (the missiles'
    /// projectile movement and their 25 cm attack sphere).
    pub fn sweep_move(&mut self, dt: f32, world: &dyn World, targets: &[TargetInfo]) -> Option<Contact> {
        let p0 = self.position;
        let p1 = p0 + self.velocity * dt;
        let mut best: Option<(f32, Contact)> = None;
        for t in targets {
            let (a, b) = (t.center - Vec3::Z * (t.half_height - t.radius).max(0.0), t.center + Vec3::Z * (t.half_height - t.radius).max(0.0));
            let (s, d) = segment_segment(p0, p1, a, b);
            if d <= self.radius + t.radius && best.as_ref().is_none_or(|(bs, _)| s < *bs) {
                best = Some((s, Contact::Pawn { target: t.id, at: p0 + (p1 - p0) * s }));
            }
        }
        if let Some(hit) = world.line_trace(p0, p1) {
            if best.as_ref().is_none_or(|(s, _)| hit.time < *s) {
                best = Some((hit.time, Contact::Map { at: hit.position }));
            }
        }
        self.position = best.as_ref().map_or(p1, |(s, _)| p0 + (p1 - p0) * *s);
        best.map(|b| b.1)
    }
}

/// Direction slerp (unit vectors): `a` at 0, `b` at 1.
fn slerp_dir(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    if t <= 0.0 || b == Vec3::ZERO {
        return a;
    }
    let angle = a.dot(b).clamp(-1.0, 1.0).acos();
    turn_toward(a, b, angle * t.min(1.0))
}

/// One key of an `FRichCurve` (all keys of the two Flare Force curves are cubic) [A].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RichKey {
    pub time: f32,
    pub value: f32,
    pub arrive: f32,
    pub leave: f32,
}

const fn rk(time: f32, value: f32, arrive: f32, leave: f32) -> RichKey {
    RichKey { time, value, arrive, leave }
}

/// `FRichCurve::Eval` for cubic keys with constant extrapolation (the engine default):
/// Bezier(P0, P0 + leave x dt / 3, P3 - arrive x dt / 3, P3).
pub fn rich_eval(keys: &[RichKey], t: f32) -> f32 {
    let Some(first) = keys.first() else { return 0.0 };
    if t <= first.time {
        return first.value;
    }
    let last = keys[keys.len() - 1];
    if t >= last.time {
        return last.value;
    }
    let i = keys.windows(2).position(|w| t < w[1].time).unwrap_or(0);
    let (k1, k2) = (keys[i], keys[i + 1]);
    let diff = k2.time - k1.time;
    let a = (t - k1.time) / diff;
    let (p0, p3) = (k1.value, k2.value);
    let (p1, p2) = (p0 + k1.leave * diff / 3.0, p3 - k2.arrive * diff / 3.0);
    let u = 1.0 - a;
    u * u * u * p0 + 3.0 * u * u * a * p1 + 3.0 * u * a * a * p2 + a * a * a * p3
}

/// `FlareForceAppear_CurveData` (UCurveVector: X and Z channels, Y without keys) [A]: the hover
/// wobble added to `m_vAppearOffset` in missile state 2 (0x1412E6740); time range -0.0244..4.4020.
pub const FLARE_APPEAR_X: [RichKey; 10] = [
    rk(-0.024380207, -0.0904541, 0.0, 0.0),
    rk(0.418437, 8.00897, -0.8213913, -0.82140625),
    rk(0.956976, -7.9481125, 0.12094028, 0.12094028),
    rk(1.36088, 8.12295, -0.058501665, -0.058501665),
    rk(1.931098, -8.0051, 0.0, 0.0),
    rk(2.4458778, 8.12295, -0.11606368, -0.11606368),
    rk(2.9131393, -8.11908, -0.25931334, -0.25931334),
    rk(3.3249636, 7.894991, -0.10582275, -0.10582275),
    rk(3.9902172, -8.233059, 1.2086165, 1.2086285),
    rk(4.4020414, 0.03042984, 0.0, 0.0),
];
pub const FLARE_APPEAR_Z: [RichKey; 10] = [
    rk(-0.024380207, -0.0904541, 0.0, 0.0),
    rk(0.49763393, 19.862804, -4.1097183, -4.109697),
    rk(0.98073506, -15.29977, -0.13706565, -0.13706565),
    rk(1.3292015, 19.748825, -0.49093902, -0.49093902),
    rk(1.978616, -15.78967, 0.1042891, 0.1042891),
    rk(2.4221187, 19.862804, -0.1260181, -0.1260181),
    rk(2.9593973, -15.913265, 0.2665165, 0.2665165),
    rk(3.2774453, 20.090763, 0.055453423, 0.055453423),
    rk(3.9870663, -15.856277, 3.6330335, 3.6330228),
    rk(4.384058, -0.33539963, 0.0, 0.29952106),
];
/// `FlareForceFire_CurveData` [A]: the recoil added in missile state 3 (0x1412E59A0); its time
/// range (0.18268 s) is state 3's countdown.
pub const FLARE_FIRE_X: [RichKey; 2] = [rk(0.0, 0.0, -375329.38, -474.47748), rk(0.15996403, -57.221436, 0.0, -26.958025)];
pub const FLARE_FIRE_Z: [RichKey; 2] = [rk(0.0, 0.0, -4351.4946, -395.41205), rk(0.18267995, -43.357758, 0.0, 22.978441)];
/// (party D5) `CallMeteorShake_CurveData` (UCurveVector, `m_ShakeCurveData` +0xBA8 of CallMeteor_Pawn): only
/// FloatCurves[2] (Z) has keys, all RCIM_Cubic, no weights; X and Y have none = 0 [A]. Its time range
/// (FUN_144084110 GetTimeRange) is the first / last key time.
pub const METEOR_SHAKE_Z: [RichKey; 4] = [
    rk(-0.042_037_964, -0.031_101_227, -0.003_752_449_3, 0.0),
    rk(1.082_466_1, 12.796_192, 0.151_958_14, 0.151_954_9),
    rk(2.636_962, -9.788_109, 0.099_140_84, 0.099_140_96),
    rk(3.648_320_2, -0.014_230_728, 0.0, -0.048_687_696),
];
pub const METEOR_SHAKE_MIN: f32 = -0.042_037_964;
pub const METEOR_SHAKE_MAX: f32 = 3.648_320_2;

/// `GetTimeRange` maxima of the two curves (FUN_144084110).
pub const FLARE_APPEAR_END: f32 = 4.4020414;
pub const FLARE_FIRE_END: f32 = 0.18267995;

/// Rotates unit `from` toward unit `to` by at most `max` radians.
pub fn turn_toward(from: Vec3, to: Vec3, max: f32) -> Vec3 {
    let from = from.normalize_or_zero();
    if to == Vec3::ZERO || from == Vec3::ZERO {
        return if from == Vec3::ZERO { to } else { from };
    }
    let angle = from.dot(to).clamp(-1.0, 1.0).acos();
    if angle <= max || angle < 1e-6 {
        return to;
    }
    let axis = from.cross(to);
    let axis = if axis.length_squared() < 1e-12 { from.any_orthonormal_vector() } else { axis.normalize() };
    glam::Quat::from_axis_angle(axis, max) * from
}

/// Closest approach of segments p0..p1 and a..b: (fraction along p0..p1, distance).
fn segment_segment(p0: Vec3, p1: Vec3, a: Vec3, b: Vec3) -> (f32, f32) {
    let (d1, d2, r) = (p1 - p0, b - a, p0 - a);
    let (aa, ee, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let (s, t);
    if aa <= 1e-9 && ee <= 1e-9 {
        return (0.0, r.length());
    }
    if aa <= 1e-9 {
        s = 0.0;
        t = (f / ee).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if ee <= 1e-9 {
            t = 0.0;
            s = (-c / aa).clamp(0.0, 1.0);
        } else {
            let bb = d1.dot(d2);
            let denom = aa * ee - bb * bb;
            let s0 = if denom.abs() > 1e-9 { ((bb * f - c * ee) / denom).clamp(0.0, 1.0) } else { 0.0 };
            let t0 = (bb * s0 + f) / ee;
            if t0 < 0.0 {
                t = 0.0;
                s = (-c / aa).clamp(0.0, 1.0);
            } else if t0 > 1.0 {
                t = 1.0;
                s = ((bb - c) / aa).clamp(0.0, 1.0);
            } else {
                t = t0;
                s = s0;
            }
        }
    }
    (s, ((p0 + d1 * s) - (a + d2 * t)).length())
}

/// `FRotator(pitch, yaw, 0).Vector()` (degrees; yaw from +X toward +Y).
pub fn rot_vector(pitch: f32, yaw: f32) -> Vec3 {
    let (p, y) = (pitch.to_radians(), yaw.to_radians());
    Vec3::new(p.cos() * y.cos(), p.cos() * y.sin(), p.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_falling_meteor_stops_at_the_floor_and_a_missile_homes_onto_its_target() {
        let world = kh_core::test_world::TestWorld::floor();
        let mut m = TeamProj::new(ProjKind::Meteor, 0, Vec3::new(0.0, 0.0, 955.0), -Vec3::Z);
        (m.speed, m.accel, m.max_speed, m.flying) = (1700.0, 2400.0, 4000.0, true);
        let mut contact = None;
        for _ in 0..120 {
            if let Some(c) = m.fly(1.0 / 60.0, &world, &[]) {
                contact = Some(c);
                break;
            }
        }
        assert!(matches!(contact, Some(Contact::Map { at }) if at.z.abs() < 1.0), "{contact:?}");
        let target = TargetInfo { id: TargetId(7), position: Vec3::new(800.0, 300.0, 0.0), center: Vec3::new(800.0, 300.0, 90.0), velocity: Vec3::ZERO, yaw: 0.0, radius: 40.0, half_height: 90.0, flags: Default::default(), hp: 1.0, hp_max: 1.0 };
        let mut p = TeamProj::new(ProjKind::Missile, 0, Vec3::new(0.0, 0.0, 200.0), rot_vector(40.0, 180.0));
        (p.speed, p.max_speed, p.flying, p.homing_turn, p.homing_stop, p.target_point) = (1200.0, 1200.0, true, 1000.0, 100.0, Some(target.center));
        let mut hit = None;
        for _ in 0..600 {
            if let Some(c) = p.fly(1.0 / 60.0, &world, &[target]) {
                hit = Some(c);
                break;
            }
        }
        assert!(matches!(hit, Some(Contact::Pawn { target: TargetId(7), .. })), "{hit:?}");
    }
}
