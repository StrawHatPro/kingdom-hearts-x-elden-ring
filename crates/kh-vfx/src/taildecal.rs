//! (VFX-RING) `ParticleModuleTypeDataSQEX_TailDecal`: ONE DEFERRED DECAL ON EVERY POINT OF A TAIL. Its only user in the four
//! roots is keyblades/vw_so032_fns1 `tail` (emitter 10): a trail of glitter decals (omi_dcl_glt_00) stamped on the ground
//! along the path of the bolt node it hangs under.
//!
//! What KH3 runs (traced):
//! - CreateInstance 0x1443141B0 makes a 0xD80-byte instance, ctor 0x144275950: the TailShapeTypeMesh instance (ctor
//!   0x144275B20, vtable 0x147711A90: a mesh emitter whose particles are the tail's points, with the SQEX_TailShape helper
//!   at +0x8D0, vtable 0x147710E90) plus the decal part: type data +0xD30, the decal components +0xD38 (count +0xD40), their
//!   material instances +0xD48, three latches +0xD58 / +0xD60 / +0xD68 (flag bit 0, value at +4); vtable 0x147712B40.
//! - THE PARTICLES ARE THE TAIL'S POINTS: Tick_SpawnParticles (slot 0x50 = 0x14429EE80 -> slot 0x260 = 0x14429F210) hands
//!   the spawn to the helper (vt+0x10 = 0x14429EEA0): one particle per TailInterval of the travelled path (0x144287CC0:
//!   TailInterval x |component scale|, 0x144286050: the path length), up to TailNumber. kh-vfx's tail walk (`spots` in
//!   sim.rs) is that same set: the head, then a point every interval back along the node's path, at most TailNumber.
//! - Per tick (slot 0x38 = 0x144282D00): slot 0x590 = 0x14427CF10 makes the missing decals, one UDecalComponent per active
//!   particle (DecalMaterial +0x1C0, DecalSize at the emitter time +0x1D8, SortOrder +0x1CC, FadeScreenSize +0x1C8,
//!   registered with the world, not attached), and LATCHES CustomParam0..2 at its first call when bUseCustomParamNSpawnTime
//!   (+0x300 / +0x340 / +0x380; values +0x308 / +0x348 / +0x388 at the emitter time); then slot 0x598 = 0x14429F5D0 updates
//!   every active one (slot 0x388 = 0x144297AA0) and gives the spare ones DecalSize 0.
//! - 0x144297AA0, particle k, decal k:
//!   * DecalSize = DecalSize(emitter time) x the node scale (+0x39C, the one TypeDataSQEX_Decal's 0x144281110 uses) x the
//!     particle's Size (+0x50) - the decal box's half extents;
//!   * world location = the particle's Location (+0x10); world rotation = FRotator(DecalRotation(emitter time)) then the
//!     particle's mesh rotation payload (+0x874: Rotation +0x18, degrees, X roll Y pitch Z yaw) - SetWorldLocationAndRotation
//!     (0x144041B30): the node's and the component's rotation do not turn it;
//!   * Decal_Color = the particle's Color.rgb (+0x60) x DecalColor(emitter time); Decal_Opacity = DecalOpacity(emitter time) x
//!     the particle's Color.a (+0x6C); Decal_CustomParam0..2 = the latch or the value at the emitter time (0x144292DF0);
//!     Decal_ZFadeSegment (+0x3C0), Decal_ZFadePow (+0x3C4).
//! - Render3DPreview 0x14432FCC0 (editor only) draws the same boxes: particle Size x +0x39C x DecalSize around Location.
//!
//! The particles' colour and size along the tail are kh-vfx's tail rules (TailScale / TailColor / TailAlpha over the points,
//! the carrier's colour from Color / ColorScaleOverLife), as the TailShape sprites take them. The payload rotation is 0: no
//! emitter of the pack's TailDecal has a mesh-rotation module, and nothing else writes it.

use glam::{Affine3A, Mat3, Vec3};

use crate::def::DecalDef;
use crate::draw::{Decal, DrawList};
use crate::math::euler;
use crate::rng::Rng;

/// What the decals of one tail-decal emitter read this frame.
pub struct Frame {
    /// The emitter time (every curve of the type data is sampled at it).
    pub rel: f32,
    pub time: f32,
    /// The particles' Size (the tail's carrier: Size / SizeScale).
    pub size: Vec3,
    /// The node's world scale (+0x39C).
    pub node_scale: Vec3,
    /// CustomParam0..2 latched at the first tick (`bUseCustomParamNSpawnTime`), else None.
    pub latch: [Option<f32>; 3],
}

/// The values latched at the emitter's first tick (0x14427CF10): CustomParamN at that emitter time where its
/// `bUseCustomParamNSpawnTime` is set.
pub fn latch(decal: &DecalDef, rel: f32) -> [Option<f32>; 3] {
    let mut rng = Rng::new(1);
    [0, 1, 2].map(|k| if decal.custom_spawn_time[k] { Some(decal.custom[k].as_ref().map_or(0.0, |d| d.float(rel, &mut rng))) } else { None })
}

/// The decals of the tail's points: `points` = (place in the world, place along the tail 0..1, TailScale there, the
/// particle's colour there).
pub fn push(out: &mut DrawList, decal: &DecalDef, material: u32, points: &[(Vec3, f32, f32, [f32; 4])], f: &Frame) {
    let mut rng = Rng::new(1);
    let size = decal.size.vec3(f.rel, &mut rng) * f.node_scale;
    let tint = decal.color.as_ref().map_or(Vec3::ONE, |x| x.vec3(f.rel, &mut rng));
    let opacity = decal.opacity.as_ref().map_or(1.0, |x| x.float(f.rel, &mut rng));
    // world rotation: DecalRotation then the (zero) payload rotation
    let turn = decal.rotation.as_ref().map_or(Mat3::IDENTITY, |x| euler(x.vec3(f.rel, &mut rng)));
    let custom = [0, 1, 2].map(|k| f.latch[k].unwrap_or_else(|| decal.custom[k].as_ref().map_or(0.0, |x| x.float(f.rel, &mut rng))));
    for &(at, _along, scale, c) in points {
        let half = size * f.size * scale;
        let color = [c[0] * tint.x, c[1] * tint.y, c[2] * tint.z, c[3] * opacity];
        if color[3] <= 0.0 || half.min_element() <= 0.0 {
            continue;
        }
        out.decals.push(Decal { material, transform: Affine3A::from_mat3_translation(turn * Mat3::from_diagonal(half), at), color, custom, z_fade: decal.z_fade, time: f.time });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dist::Dist;

    fn def() -> DecalDef {
        DecalDef {
            material: "m".to_string(),
            size: Dist::constant_vec(Vec3::new(400.0, 70.0, 70.0)),
            rotation: Some(Dist::constant_vec(Vec3::new(0.0, -90.0, 0.0))),
            color: Some(Dist::constant_vec(Vec3::new(1.0, 0.5, 1.0))),
            opacity: Some(Dist::constant(0.5)),
            custom: [Some(Dist::constant(3.0)), None, None],
            z_fade: [0.0, 1.0],
            scale_rot: false,
            custom_spawn_time: [true, false, false],
        }
    }

    #[test]
    fn one_box_per_point_sized_turned_and_coloured_as_0x144297aa0() {
        let d = def();
        let mut out = DrawList::default();
        let pts = [(Vec3::new(0.0, 0.0, 10.0), 0.0, 1.0, [2.0, 2.0, 2.0, 1.0]), (Vec3::new(-12.0, 0.0, 10.0), 0.5, 0.5, [2.0, 2.0, 2.0, 0.4]), (Vec3::new(-24.0, 0.0, 10.0), 1.0, 1.0, [1.0, 1.0, 1.0, 0.0])];
        let f = Frame { rel: 0.1, time: 0.4, size: Vec3::ONE, node_scale: Vec3::splat(2.0), latch: latch(&d, 0.0) };
        push(&mut out, &d, 7, &pts, &f);
        assert_eq!(out.decals.len(), 2, "a point of alpha 0 draws nothing");
        let a = &out.decals[0];
        assert_eq!(Vec3::from(a.transform.translation), pts[0].0);
        // DecalRotation (0, -90, 0): the projection axis X turned straight down, half extent 400 x node scale 2
        let x = Vec3::from(a.transform.matrix3.x_axis);
        assert!((x - Vec3::new(0.0, 0.0, -800.0)).length() < 0.05, "{x:?}");
        assert!((Vec3::from(a.transform.matrix3.y_axis).length() - 140.0).abs() < 0.01);
        assert_eq!(a.color, [2.0, 1.0, 2.0, 0.5]);
        assert_eq!(a.custom, [3.0, 0.0, 0.0], "the latch");
        // the tail's scale on the second point
        assert!((Vec3::from(out.decals[1].transform.matrix3.x_axis).length() - 400.0).abs() < 0.05);
        assert!((out.decals[1].color[3] - 0.2).abs() < 1.0e-6);
    }

    #[test]
    fn the_latch_only_where_spawn_time_is_set() {
        let d = def();
        assert_eq!(latch(&d, 0.3), [Some(3.0), None, None]);
    }
}
