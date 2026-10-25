//! The visual-effect seam: the core says WHICH effect, WHERE and how it is attached; the host
//! owns the particles (or, in the test level, a labelled marker).

use super::Attach;
use glam::Vec3;

/// Which effect: the row index of the generated effect table (the effects sheet of
/// docs/sora-vfx-spec.md through `tools/gen_port.js`, once it exists).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectId(pub u32);

/// One spawned instance. Allocated by the core, counting up from 1; 0 is never used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectHandle(pub u32);

/// A request to spawn an effect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectSpawn {
    pub id: EffectId,
    /// The effect's name in its sheet; may be empty.
    pub name: &'static str,
    /// Where: a world point, or attached to Sora / a bone / a target (the host then follows it).
    pub at: Attach,
    /// Orientation, degrees (x = pitch up, y = yaw about Z from +X toward +Y, z = roll): in the
    /// world for `Attach::World`, relative to what it is attached to otherwise.
    pub rotation: Vec3,
    /// Uniform scale, 1 = as authored.
    pub scale: f32,
    /// True: it stays until stopped through its handle (trails, auras). False: it ends by itself.
    pub persistent: bool,
}

impl EffectSpawn {
    /// A one-shot at Sora's origin, unrotated, unscaled.
    pub const fn at_sora(id: EffectId, name: &'static str) -> EffectSpawn {
        EffectSpawn { id, name, at: Attach::SORA, rotation: Vec3::ZERO, scale: 1.0, persistent: false }
    }
}

/// The host's effect output. Nothing is returned.
/// (DON-MAGIC tint) A colour for one spawned effect (the plan's [R] recolours): what it draws is recoloured at draw time,
/// rgb = max(r, g, b) x `color` x `intensity` (its brightness kept, its hue replaced), alpha unchanged (kh-vfx
/// `World::set_tint`). A host without it draws the effect in its own colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tint {
    /// Linear RGB, 0..1.
    pub color: [f32; 3],
    pub intensity: f32,
}

impl Tint {
    pub const fn rgb(r: f32, g: f32, b: f32) -> Tint {
        Tint { color: [r, g, b], intensity: 1.0 }
    }
}

pub trait Effects {
    fn spawn(&mut self, handle: EffectHandle, spawn: &EffectSpawn);
    /// (DON-MAGIC tint) Recolour the effect just spawned with `handle` (see [`Tint`]); the default: nothing.
    fn tint(&mut self, _handle: EffectHandle, _tint: Tint) {}
    /// (DON-MAGIC orbit) Move the live effect of `handle` (Ball Lightning's orbiting balls): spawned at a world point, it
    /// moves to `at` (cm, core axes); spawned on Sora / a target / a member, `at` becomes its offset. The default: nothing.
    fn place(&mut self, _handle: EffectHandle, _at: Vec3) {}
    /// (ZETTA) Move, turn and scale the live effect of `handle` placed at a world point (Zettaflare's beam laid along the
    /// staff -> target line and kept on it): `at` its world place (cm, core axes), `rotation` (pitch, yaw, roll) degrees,
    /// `scale` per local axis (replaces the spawn's uniform scale: the beam stretched to the target's distance). An effect
    /// attached to something keeps its anchor and takes only `rotation` / `scale`. The default: nothing.
    fn transform(&mut self, _handle: EffectHandle, _at: Vec3, _rotation: Vec3, _scale: Vec3) {}
    /// Stop an instance: `immediate` removes it at once, otherwise it stops emitting and fades
    /// as authored. Unknown and finished handles are ignored.
    fn stop(&mut self, handle: EffectHandle, immediate: bool);
}

/// Nothing is drawn.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullEffects;

impl Effects for NullEffects {
    fn spawn(&mut self, _handle: EffectHandle, _spawn: &EffectSpawn) {}
    fn stop(&mut self, _handle: EffectHandle, _immediate: bool) {}
}
