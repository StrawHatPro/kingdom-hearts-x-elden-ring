//! (vfx implementer 3) Camera shakes as an ADDITIVE OFFSET REQUEST on top of whatever camera is in
//! use: the rig, the lock-on camera and the aim camera do not know about it; the host adds
//! [`ShakeSet::offset`] to the view it draws.
//!
//! What plays them in the game: the particle module `SQEX_CameraShake` (Update 0x1442FC5E0 [C]):
//! once the emitter time reaches `ShakePlayTime` it calls the camera manager's play function
//! (vt+0x7F0) with the `Shake` class, `CameraShakeScale(emitter time)` scaled by the camera's
//! distance, play space 0 (camera local) and the optional oscillation duration.
//!
//! The shake itself is the stock `UCameraShake` of UE 4.17 [U] (`UpdateAndApplyCameraShake`; the
//! function was not located in the executable): sine oscillators for location (cm, camera
//! local), rotation (degrees) and field of view, a linear blend in and out, a duration. The
//! class defaults are read from the blueprint assets (`PACK/vfx/host.json` `camera_shakes`).
//! Square Enix's additions to the oscillator (`m_AmplitudeRandScaleMax`: 1 in every shake of the
//! set) are not evaluated.

use glam::Vec3;

/// `FFOscillator`: amplitude, frequency (radians per second, as stock multiplies the time by it)
/// and whether the phase starts at zero (`EOO_OffsetZero`) instead of at random.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Oscillator {
    pub amplitude: f32,
    pub frequency: f32,
    pub start_at_zero: bool,
}

/// The class defaults of one `UCameraShake` blueprint.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShakeDef {
    pub name: String,
    /// Seconds; negative = until stopped.
    pub duration: f32,
    pub blend_in: f32,
    pub blend_out: f32,
    /// Pitch, yaw, roll (degrees).
    pub rot: [Oscillator; 3],
    /// X forward, Y right, Z up of the camera (cm).
    pub loc: [Oscillator; 3],
    /// Degrees of field of view.
    pub fov: Oscillator,
    /// `bSingleInstance`: playing it again restarts the running instance.
    pub single_instance: bool,
}

/// What is added to the view of this step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShakeOffset {
    /// Camera-local centimetres: X forward, Y right, Z up.
    pub location: Vec3,
    /// Degrees: x pitch (up positive), y yaw (right positive), z roll.
    pub rotation: Vec3,
    pub fov: f32,
}

#[derive(Clone, Debug)]
struct Active {
    def: ShakeDef,
    scale: f32,
    /// Seconds left; negative = infinite.
    remaining: f32,
    blend_in_time: f32,
    blend_out_time: f32,
    blending_in: bool,
    blending_out: bool,
    /// Phases: location x, y, z, rotation pitch, yaw, roll, fov.
    phase: [f32; 7],
    offset: ShakeOffset,
}

/// The shakes that are playing.
#[derive(Clone, Debug, Default)]
pub struct ShakeSet {
    active: Vec<Active>,
    /// A small deterministic stream for the start phases (the game uses the engine's global one).
    seed: u32,
}

impl ShakeSet {
    pub fn new() -> ShakeSet {
        ShakeSet { active: Vec::new(), seed: 0x2545_F491 }
    }

    fn random(&mut self) -> f32 {
        let mut x = self.seed.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / 16_777_216.0
    }

    /// `PlayCameraShake(class, scale, CameraLocal, _, duration)`: `duration` overrides the class's
    /// oscillation duration when given (`OverrideOscillationDuration` of the particle module).
    pub fn play(&mut self, def: &ShakeDef, scale: f32, duration: Option<f32>) {
        if scale <= 0.0 {
            return;
        }
        let mut phase = [0.0f32; 7];
        let oscillators = [def.loc[0], def.loc[1], def.loc[2], def.rot[0], def.rot[1], def.rot[2], def.fov];
        for (k, o) in oscillators.iter().enumerate() {
            // Stock: EOO_OffsetRandom starts anywhere in a full turn.
            phase[k] = if o.start_at_zero || o.amplitude == 0.0 { 0.0 } else { self.random() * std::f32::consts::TAU };
        }
        let total = duration.unwrap_or(def.duration);
        let fresh = Active { def: def.clone(), scale, remaining: total, blend_in_time: 0.0, blend_out_time: 0.0, blending_in: def.blend_in > 0.0, blending_out: false, phase, offset: ShakeOffset::default() };
        if def.single_instance {
            if let Some(running) = self.active.iter_mut().find(|a| a.def.name == def.name) {
                *running = fresh;
                return;
            }
        }
        // More than a handful at once is a runaway effect, not a design.
        if self.active.len() < 16 {
            self.active.push(fresh);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }

    pub fn clear(&mut self) {
        self.active.clear();
    }

    /// One step of every shake (stock `UpdateAndApplyCameraShake` [U]).
    pub fn step(&mut self, dt: f32) {
        for a in &mut self.active {
            let infinite = a.remaining < 0.0 && a.def.duration < 0.0;
            if !infinite {
                a.remaining -= dt;
                if a.remaining <= 0.0 {
                    a.remaining = 0.0;
                    a.offset = ShakeOffset::default();
                    continue;
                }
                if a.remaining < a.def.blend_out {
                    // Start the blend out where it leaves the remaining time.
                    if !a.blending_out {
                        a.blending_out = true;
                        a.blend_out_time = a.def.blend_out - a.remaining;
                    }
                }
            }
            if a.blending_in {
                a.blend_in_time += dt;
                if a.blend_in_time >= a.def.blend_in {
                    a.blending_in = false;
                }
            }
            if a.blending_out {
                a.blend_out_time += dt;
            }
            let blend_in = if a.blending_in { (a.blend_in_time / a.def.blend_in).clamp(0.0, 1.0) } else { 1.0 };
            let blend_out = if a.blending_out && a.def.blend_out > 0.0 { (1.0 - a.blend_out_time / a.def.blend_out).clamp(0.0, 1.0) } else { 1.0 };
            let scale = a.scale * blend_in * blend_out;
            let oscillators = [a.def.loc[0], a.def.loc[1], a.def.loc[2], a.def.rot[0], a.def.rot[1], a.def.rot[2], a.def.fov];
            let mut value = [0.0f32; 7];
            for (k, o) in oscillators.iter().enumerate() {
                // FFOscillator::UpdateOffset: phase += dt * frequency; amplitude * sin(phase).
                a.phase[k] += dt * o.frequency;
                value[k] = o.amplitude * a.phase[k].sin() * scale;
            }
            a.offset = ShakeOffset { location: Vec3::new(value[0], value[1], value[2]), rotation: Vec3::new(value[3], value[4], value[5]), fov: value[6] };
        }
        self.active.retain(|a| a.remaining != 0.0);
    }

    /// The sum of the running shakes, to add to the drawn view.
    pub fn offset(&self) -> ShakeOffset {
        let mut sum = ShakeOffset::default();
        for a in &self.active {
            sum.location += a.offset.location;
            sum.rotation += a.offset.rotation;
            sum.fov += a.offset.fov;
        }
        sum
    }
}

/// The distance term of `SQEX_CameraShake` (0x1442FC5E0 [C]): with `min < 1` the scale falls
/// linearly from 1 at `offset` cm between the camera and the emitter node to `min` at `distance`.
pub fn distance_scale(camera_to_node: f32, distance: f32, offset: f32, min: f32) -> f32 {
    if min >= 1.0 {
        return 1.0;
    }
    let d = (camera_to_node - offset).max(0.0) / (distance - offset).max(1.0e-6);
    (min - 1.0) * d.min(1.0) + 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> ShakeDef {
        ShakeDef { name: "t".into(), duration: 1.0, blend_in: 0.0, blend_out: 0.5, rot: [Oscillator { amplitude: 2.0, frequency: 30.0, start_at_zero: true }, Oscillator::default(), Oscillator::default()], loc: [Oscillator::default(), Oscillator::default(), Oscillator { amplitude: 20.0, frequency: 100.0, start_at_zero: true }], fov: Oscillator::default(), single_instance: true }
    }

    #[test]
    fn a_shake_oscillates_blends_out_and_ends() {
        let mut set = ShakeSet::new();
        set.play(&def(), 0.5, None);
        set.step(1.0 / 60.0);
        let o = set.offset();
        // Phase = dt * frequency; amplitude x scale x sin.
        assert!((o.location.z - 20.0 * 0.5 * (100.0f32 / 60.0).sin()).abs() < 1.0e-4, "{o:?}");
        assert!((o.rotation.x - 2.0 * 0.5 * (30.0f32 / 60.0).sin()).abs() < 1.0e-4, "{o:?}");
        // Inside the blend out the amplitude is bounded by the falling weight.
        for _ in 0..44 {
            set.step(1.0 / 60.0);
        }
        // 0.75 s in: 0.25 s left of a 0.5 s blend out = weight 0.5.
        assert!(set.offset().location.z.abs() <= 20.0 * 0.5 * 0.5 + 1.0e-3);
        for _ in 0..20 {
            set.step(1.0 / 60.0);
        }
        assert!(set.is_empty());
        assert_eq!(set.offset(), ShakeOffset::default());
    }

    #[test]
    fn a_single_instance_shake_restarts_and_the_distance_scales_it() {
        let mut set = ShakeSet::new();
        set.play(&def(), 1.0, None);
        set.play(&def(), 1.0, Some(0.2));
        assert_eq!(set.active.len(), 1);
        assert!((set.active[0].remaining - 0.2).abs() < 1.0e-6);
        // 2000 / 1000 / 0.2: full inside 10 m, a fifth beyond 20 m.
        assert_eq!(distance_scale(500.0, 2000.0, 1000.0, 0.2), 1.0);
        assert!((distance_scale(1500.0, 2000.0, 1000.0, 0.2) - 0.6).abs() < 1.0e-6);
        assert!((distance_scale(5000.0, 2000.0, 1000.0, 0.2) - 0.2).abs() < 1.0e-6);
        assert_eq!(distance_scale(5000.0, 2000.0, 1000.0, 1.0), 1.0);
    }
}
