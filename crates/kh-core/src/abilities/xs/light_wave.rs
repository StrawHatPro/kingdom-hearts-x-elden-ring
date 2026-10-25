//! (XSI) LIGHT WAVE (XS 2.9; ability LIGHT_WAVE 264, level B 44 / Astel): Re Mind's light wave during KH3's MP Charge.
//! While the charge runs the situation slot offers "Light Wave" once per charge (command `light_wave.command`: KH3's
//! unused Reserve248, named by the HUD). Y: `SoKcShoot` 1813 (kind 0xE9) and one wave at `light_wave.shot_time`; Y still
//! held at `light_wave.tap_max`: `SoKcIrradiate` 1814 and `light_wave.volley` waves `light_wave.interval` apart aimed
//! with the stick. A wave (KH3's `SoKc_LightWave_Projectile`: `light_wave.speed`, sphere `light_wave.radius`, ours: its
//! range) pierces; each target once; row SF_LFSHOOT (0.8 TOSS magic) x (1 + `light_wave.ga_bonus` per -ga spell).
//! The price: the MP Charge gets `light_wave.charge_extra` seconds longer.

use glam::Vec3;

use super::{has, X};
use crate::combat::situation;
use crate::enums::ETresAbilityKind as A;
use crate::host::{Attach, EffectId, EffectSpawn, TargetFlags, TargetId};
use crate::pawn::Ctx;
use crate::sora::Input;

pub const TAP: u8 = 0xE9;
pub const HOLD: u8 = 0xEA;
pub const ROW: &str = "SF_LFSHOOT";
pub const EFFECT: &str = "vg_kg07_pow_bem2";

/// One wave in flight.
#[derive(Clone, Debug, PartialEq)]
pub struct Wave {
    pub position: Vec3,
    pub velocity: Vec3,
    pub travelled: f32,
    pub hit: Vec<TargetId>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaveState {
    /// Light Wave was offered during the charge that runs.
    pub offered: bool,
    /// The volley (held Y) runs; waves fired by this attack; the clip time the next one is due.
    pub volley: bool,
    pub fired: u32,
    pub waves: Vec<Wave>,
    /// Waves fired so far (for tests).
    pub total: u32,
}

fn command() -> i32 {
    X.light_wave_command as i32
}

/// Every step: the offer (once per charge), the waves in flight.
pub(crate) fn tick(cx: &mut Ctx, input: &Input, dt: f32) {
    let charging = cx.pawn.status.mp_charging;
    if !charging {
        cx.pawn.combat.xs.wave.offered = false;
        if cx.pawn.combat.situation.has(command()) {
            situation::delete(cx, command());
        }
    } else if has(cx.pawn, A::LightWave) && !cx.pawn.combat.xs.wave.offered {
        cx.pawn.combat.xs.wave.offered = true;
        let left = cx.pawn.status.mp_charge_timer.max(0.1);
        situation::enqueue(cx, command(), left);
    }
    let _ = input;
    fly(cx, dt);
}

/// The command (situation.rs `execute`): the tap's attack; the charge gets longer.
pub(crate) fn execute(state: &mut crate::states::StateData, cx: &mut Ctx) -> bool {
    if !has(cx.pawn, A::LightWave) || !cx.pawn.status.mp_charging || !crate::combat::form::state_gate(state, cx) {
        return false;
    }
    cx.pawn.status.mp_charge_timer += X.light_wave_charge_extra;
    let target = cx.pawn.combat.lockon.current;
    crate::states::attack::xs_forced_attack(cx.pawn, target, TAP, false);
    true
}

pub(crate) fn begin(cx: &mut Ctx) {
    let w = &mut cx.pawn.combat.xs.wave;
    (w.volley, w.fired) = (false, 0);
}

/// The attack's tick: the volley's switch, the waves due.
pub(crate) fn attack_tick(cx: &mut Ctx) {
    let time = cx.pawn.anim.time;
    let y = cx.pawn.input.pad.held.has(crate::input::PadButton::SITUATION);
    let w = &cx.pawn.combat.xs.wave;
    if !w.volley && w.fired == 0 && time >= X.light_wave_tap_max && y && cx.pawn.anim.name == crate::anim::AnimId::SoKcShoot {
        cx.pawn.combat.xs.wave.volley = true;
        cx.pawn.anim.play_with(crate::anim::AnimId::SoKcIrradiate, crate::anim::PlayArgs::mode(2, -1.0));
        return;
    }
    let w = &cx.pawn.combat.xs.wave;
    let due = if w.volley { w.fired < X.light_wave_volley as u32 && cx.pawn.anim.time >= w.fired as f32 * X.light_wave_interval } else { w.fired == 0 && time >= X.light_wave_shot_time };
    if due {
        fire(cx);
    }
}

/// The direction of a wave: the stick's when it is pushed, else the lock-on target, else the facing.
fn aim(cx: &Ctx) -> Vec3 {
    let own = cx.pawn.cmc.location + Vec3::Z * X.light_wave_height;
    if cx.pawn.stick_magnitude >= X.light_wave_stick_min {
        let a = cx.pawn.stick_yaw.to_radians();
        return Vec3::new(a.cos(), a.sin(), 0.0);
    }
    let target = cx.pawn.combat.lockon.current;
    if let Some(t) = (target != TargetId::NONE).then(|| cx.target(target)).flatten() {
        return (t.center - own).normalize_or_zero();
    }
    let a = cx.pawn.yaw.to_radians();
    Vec3::new(a.cos(), a.sin(), 0.0)
}

fn fire(cx: &mut Ctx) {
    let dir = aim(cx);
    if dir.truncate().length_squared() > 1.0e-4 {
        cx.pawn.yaw = dir.y.atan2(dir.x).to_degrees();
    }
    let position = cx.pawn.cmc.location + Vec3::Z * X.light_wave_height + dir * X.light_wave_offset;
    cx.spawn_effect(EffectSpawn { id: EffectId(crate::notify::notify_asset_id(EFFECT)), name: EFFECT, at: Attach::World(position), rotation: Vec3::new(0.0, dir.y.atan2(dir.x).to_degrees(), 0.0), scale: 1.0, persistent: false });
    let w = &mut cx.pawn.combat.xs.wave;
    w.fired += 1;
    w.total += 1;
    w.waves.push(Wave { position, velocity: dir * X.light_wave_speed, travelled: 0.0, hit: Vec::new() });
}

/// The waves in flight: move, hit each target once (a one-step XS shot of SF_LFSHOOT on it), die at their range.
fn fly(cx: &mut Ctx, dt: f32) {
    if cx.pawn.combat.xs.wave.waves.is_empty() {
        return;
    }
    let mut waves = std::mem::take(&mut cx.pawn.combat.xs.wave.waves);
    let mut targets = Vec::new();
    cx.targets(&mut targets);
    for w in waves.iter_mut() {
        let from = w.position;
        let step = w.velocity * dt;
        w.position += step;
        w.travelled += step.length();
        for t in targets.iter().filter(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::HITTABLE)) {
            if w.hit.contains(&t.id) {
                continue;
            }
            // The segment of this step against the target's centre.
            let ab = w.position - from;
            let k = if ab.length_squared() > 1.0e-6 { ((t.center - from).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            let near = from + ab * k;
            if (t.center - near).length() <= X.light_wave_radius + t.radius {
                w.hit.push(t.id);
                crate::combat::hit::spawn_xs_shot(cx, "XS_WAVE", ROW, t.center, t.radius.max(X.light_wave_hit_radius_min), X.light_wave_hit_life, None);
            }
        }
    }
    waves.retain(|w| w.travelled < X.light_wave_range);
    cx.pawn.combat.xs.wave.waves = waves;
}

/// The power factor of a wave: 1 + `light_wave.ga_bonus` per -ga spell learned.
pub(crate) fn power(cx: &Ctx) -> f32 {
    let ga = cx.pawn.status.magic.iter().filter(|m| m.tier >= 3).count();
    1.0 + X.light_wave_ga_bonus * ga as f32
}
