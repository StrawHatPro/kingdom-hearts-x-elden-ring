//! (AUDIT-B W-HANDOVER) THE EFFECTS' LINE TRACE IN ELDEN RING: kh-vfx's `RayWorld` (the world seam of KH3's
//! `ParticleModuleSQEX_Raycast`, kh-vfx def.rs `RaycastDef`) answered through Elden Ring's own Havok world.
//!
//! KH3 (0x14435FCB0): one trace of the effect's component on ECC_WorldStatic (the source actor and trigger volumes ignored)
//! from the particle's / child's place to `RayEndOffset`; the nearest blocking hit, its normal and its physical material's
//! SurfaceType (KH3's DefaultEngine.ini: 32 puddle, 33 knee-deep, 34 waist-deep, 35 .. 37 deeper water). The hammers' and
//! Nano Arms' ground cracks (`zwr_gen`) play unless that is water; their water pillars (`ps_miz_hashira`) only over water.
//!
//! Here: the nearest contact of the KH characters' own collision word (havok.rs `FILTER_PLAYER`, the Tarnished's: what he
//! stands on), and the water surface by kh-er's measured rule (guards.rs `water_surfaces`: a contact both water-seeing
//! layers 0x58 / 0x5a have and no solid layer has). The nearer of the two answers; water is SurfaceType 32. Generic: every
//! map, no tables. Game thread only (kh-vfx steps the casting systems on the calling thread, `World::set_rays`).

use glam::Vec3;
use kh_vfx::{RayHit, RayWorld};

use crate::coords::{self, Frame};

/// KH3's SurfaceType of the water surfaces (`水たまり`, the first of the water list every crack generator checks).
pub const WATER_SURFACE: u8 = 32;

/// The cast's sphere (m): a line trace as near as the game's shape cast allows.
const RADIUS: f32 = 0.01;

/// The ray world of one frame: the frame between KH3's space and Elden Ring's.
pub struct ErRays {
    pub frame: Frame,
}

/// The nearest answer of a ray `length` m long from the casts' contacts (distances along the ray, m, with their normals in
/// Elden Ring's axes): the solid layer's nearest contact, unless a water surface (guards.rs rule) lies nearer.
pub fn pick(solid: &[(f32, Vec3)], water_a: &[(f32, Vec3)], water_b: &[f32], length: f32) -> Option<(f32, Vec3, u8)> {
    let solid_d: Vec<f32> = solid.iter().map(|c| c.0).collect();
    let water_a_d: Vec<f32> = water_a.iter().map(|c| c.0).collect();
    let water = crate::guards::water_surfaces(&water_a_d, water_b, &solid_d).into_iter().find(|d| *d >= 0.0 && *d <= length);
    let ground = solid.iter().filter(|c| c.0.is_finite() && c.0 >= 0.0 && c.0 <= length).min_by(|a, b| a.0.total_cmp(&b.0)).copied();
    match (ground, water) {
        (Some(g), Some(w)) if w < g.0 => Some((w, Vec3::Y, WATER_SURFACE)),
        (None, Some(w)) => Some((w, Vec3::Y, WATER_SURFACE)),
        (Some(g), _) => Some((g.0, g.1, 0)),
        (None, None) => None,
    }
}

fn contacts(origin: Vec3, delta: Vec3, filter: u32, hits: &mut Vec<crate::havok::CastHit>) -> Vec<(f32, Vec3)> {
    crate::havok::cast_sphere(origin, delta, RADIUS, filter, false, hits);
    let length = delta.length();
    hits.iter().filter(|h| h.fraction.is_finite()).map(|h| (h.fraction * length, h.normal)).collect()
}

impl RayWorld for ErRays {
    fn trace(&self, start: Vec3, end: Vec3) -> Option<RayHit> {
        let origin = self.frame.pos_kh_to_er(start);
        let delta = coords::dir_kh_to_er(end - start);
        let length = delta.length();
        if length < 1.0e-6 {
            return None;
        }
        let mut hits = Vec::new();
        let solid = contacts(origin, delta, crate::havok::FILTER_PLAYER, &mut hits);
        let water_a = contacts(origin, delta, crate::havok::FILTER_CAMERA, &mut hits);
        let water_b: Vec<f32> = if water_a.is_empty() { Vec::new() } else { contacts(origin, delta, crate::havok::filter_of_layer(0x5a), &mut hits).into_iter().map(|c| c.0).collect() };
        let (d, normal, surface) = pick(&solid, &water_a, &water_b, length)?;
        Some(RayHit { point: start + (end - start) * (d / length), normal: coords::unit_er_to_kh(normal).normalize_or(Vec3::Z), surface })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the ground under a crack; water nearer than the bed; water alone; nothing within the ray
    #[test]
    fn the_nearer_of_ground_and_water_answers() {
        let up = Vec3::Y;
        assert_eq!(pick(&[(0.4, up)], &[(0.4, up)], &[], 0.6), Some((0.4, up, 0)), "dry ground: what every layer sees is not water");
        assert_eq!(pick(&[(0.55, up)], &[(0.2, up), (0.55, up)], &[0.2], 0.6), Some((0.2, Vec3::Y, WATER_SURFACE)), "a puddle over the bed");
        assert_eq!(pick(&[], &[(0.3, up)], &[0.3], 0.6), Some((0.3, Vec3::Y, WATER_SURFACE)), "deep water: no bed within the ray");
        assert_eq!(pick(&[(0.3, up)], &[(0.3, up)], &[0.3], 0.6), Some((0.3, up, 0)), "a solid contact there is ground, not water");
        assert_eq!(pick(&[(0.9, up)], &[], &[], 0.6), None, "past the ray's end");
        let slope = Vec3::new(0.0, 0.8, 0.6).normalize();
        assert_eq!(pick(&[(0.1, slope)], &[(0.1, slope)], &[], 0.6).map(|h| h.1), Some(slope), "a slope's normal is given back");
    }
}
