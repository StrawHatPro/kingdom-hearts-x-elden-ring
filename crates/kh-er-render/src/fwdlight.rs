//! (VFX-2 lit) THE FORWARD LIGHTING OF KH3's LIT TRANSLUCENT EFFECTS, FED FROM ELDEN RING'S LIGHT.
//!
//! KH3's forward-lit smoke (jmb_smk_implit_ac_00sa, jmb_sublit_msa, smt_so050_smk_sublit_ac_msa, smt_so053_mdl_smk_lit_00k:
//! MSM_DefaultLit translucents of the fork's surface forward shading) light themselves in their base-pass pixel shader with UE
//! 4.17's forward lighting [traced: build/cache/vfx_shaders/154_jmb_smk_implit_ac_00sa.sprite.asm, the same in all seven]:
//! - `ForwardGlobalLightData` (7 rows): the DIRECTIONAL light (row 4 direction to it, row 5 colour; `(diffuse / pi + GGX) x N.L x
//!   colour`), unshadowed but by the primitive's precomputed shadowing (`PrecomputedLightingBuffer[5].x`, picked by
//!   `Primitive[11].y` and the light's channel mask row 6), and the light grid's shape (rows 0..3);
//! - the LIGHT GRID's integer buffer walk: `NumCulledLightsGrid` (count, start per cell), `CulledLightDataGrid` (light indices),
//!   `ForwardLocalLightBuffer` (4 rows a light: position + 1 / radius, colour + falloff exponent, direction + channel mask,
//!   spot angles + source radius / packed length), radial `(1 - (d / r)^2)^exponent` or inverse-squared falloff;
//! - the REFLECTION CAPTURES of the same grid's second half (`ReflectionCapture[341]` arrays indexed by the capture: position +
//!   radius [0], brightness / array index / shape [341], offset [682], box transforms [1023 + 4i], box scales [2387]) sampled
//!   from the capture cube array at a roughness mip, times the environment BRDF;
//! - no indirect diffuse term at all (the smoke's "ambient" is its emissive from the level's scene colours:
//!   SceneBrightColor / SceneDarkColor, the sprite factory's rows 12 / 13).
//!
//! Elden Ring gives none of those objects. What stands for each, THE SAME LIGHT OUR HYBRID GIVES THE KH CHARACTERS:
//! - the directional light = the game's sun as the hybrid takes it (gfx.rs `hy_sun`: its direction, Col_0 x the calibration /
//!   pi per N.L on a white surface), as UE's colour (x pi); the precomputed shadowing = the game's own sun visibility at his
//!   feet (the probes' shadow mask, as the rig's key: 1 - shadow colour + shadow colour x mask.w);
//! - the light grid = ONE cell holding the lights our renderer puts on the KH characters this frame (pointlight.rs: the effects'
//!   particle lights, which KH3's own grid holds, the hero light, and the world light fitted to the game's direct light on the
//!   ring of probes around his feet), each as UE's light (colour x pi, its radius and falloff exponent, a point light);
//! - the reflection captures = ONE infinite sphere capture whose cube is the reflection environment our characters reflect
//!   (kh-look's sky cube x light.rs `env_level` x `env_tint`: the game's sky of the hour and the ground at his feet);
//! - the scene colours (forward-lit owners only) = the rig's light on a white surface facing up, with and without its key.
//!
//! All of it is held here in SCENE units (what the game's scene target holds) and handed to the effects' frame constants by
//! [`extend_frame`], divided by the effects' scale (vfx.rs: KH3 units, as `er.lit`). Layout = er_vfx.wgsl `ErFrame.fwd_*`.

use std::sync::Mutex;

use glam::Vec3;

/// Lights of the one grid cell (er_vfx.wgsl `fwd_l`: four rows each).
pub const MAX_LIGHTS: usize = 16;
/// Floats [`extend_frame`] appends: fwd_g 8 rows, fwd_env 5 rows, fwd_l 4 rows per light.
pub const FRAME_FLOATS: usize = (8 + 5 + 4 * MAX_LIGHTS) * 4;
/// UE's `ShadowMapChannelMask` of the directional light: static channel 0 (the primitive's precomputed shadowing), dynamic
/// channel 0 (bit 4: without one the fork's code gives the light no strength), lighting channel 0 (bit 8).
pub const SUN_MASK: f32 = 273.0;
/// A local light's mask: lighting channel 0 only (no shadow channel: unshadowed, as KH3's dynamic lights in the grid).
pub const LIGHT_MASK: f32 = 256.0;
/// The capture cube's last mip (View[160].z, ReflectionCubemapMaxMip; er_vfx.wgsl `kh_fwd_env` turns the mip back into the
/// roughness with it).
pub const CUBE_MAX_MIP: f32 = 7.0;

/// One light of the cell, scene units: what a white surface facing it at its centre gets (pointlight.rs `PassLight`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub position: Vec3,
    pub radius: f32,
    pub color: Vec3,
    pub exponent: f32,
}

/// The frame's light, scene units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    /// To the sun (game space, unit) and its light per unit N.L on a white surface.
    pub sun: Option<(Vec3, Vec3)>,
    /// The sun's visibility (1 = lit).
    pub visibility: f32,
    /// The reflection environment: zenith, horizon, ground.
    pub env: [Vec3; 3],
    /// A white surface facing up: lit, in the shade (the scene colours); None = the harness's own (1, 0.45).
    pub scene: Option<(Vec3, Vec3)>,
}

static FRAME: Mutex<Option<Frame>> = Mutex::new(None);
static LIGHTS: Mutex<Vec<Light>> = Mutex::new(Vec::new());

/// The lights on the KH characters this frame (gfx.rs, after pointlight.rs planned them; empty: none).
pub fn set_lights(lights: &[crate::pointlight::PassLight]) {
    if let Ok(mut l) = LIGHTS.lock() {
        l.clear();
        l.extend(lights.iter().filter(|p| p.radius > 1.0 && p.position.is_finite() && p.color.is_finite() && p.color.max_element() > 0.0).take(MAX_LIGHTS).map(|p| Light { position: p.position, radius: p.radius, color: p.color, exponent: p.exponent }));
    }
}

fn luma(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// The frame's light from what gfx.rs has (`hy_sun`, the game's light read back, the rig, the reflection environment's
/// level and hue, the characters' out scale); None everywhere = no game light: the sun off, visibility 1, the stand-in sky.
pub fn frame_from(sun: Option<(Vec3, Vec3)>, game: Option<&crate::light::GameLight>, rig: Option<&crate::light::RigLight>, env_level: f32, env_tint: Vec3, out_scale: f32) -> Frame {
    use kh_look::stand_in::{ENV_GROUND, ENV_HORIZON, ENV_ZENITH};
    let visibility = game.map_or(1.0, |l| {
        let s = luma(l.shadow_color).clamp(0.0, 1.0);
        1.0 - s + s * l.mask.w.clamp(0.0, 1.0)
    });
    let k = env_level * out_scale.max(0.0);
    let env = [ENV_ZENITH * env_tint * k, ENV_HORIZON * env_tint * k, ENV_GROUND * env_tint * k];
    let scene = rig.map(|r| {
        let shade = (r.ambient + r.ambient_up) * r.out_scale;
        (shade + r.key * r.key_dir.y.max(0.0) * r.out_scale, shade)
    });
    let sun = sun.filter(|(d, c)| d.is_finite() && c.is_finite() && d.length() > 0.5 && c.max_element() > 0.0);
    Frame { sun, visibility, env, scene }
}

/// This frame's light (gfx.rs, once a frame before the effects are drawn).
pub fn set_frame(frame: Frame) {
    if let Ok(mut f) = FRAME.lock() {
        if f.is_none() {
            crate::rlog!("vfx: (VFX-2 lit) the forward-lit effects take Elden Ring's light: sun {:?}, visibility {:.2}", frame.sun, frame.visibility);
        }
        *f = Some(frame);
    }
}

/// The rows of `ErFrame.fwd_*` for `frame` and `lights`, KH3 units (`scale`: scene units per KH3 unit, vfx.rs `cx.scale x hdr`).
pub fn rows(frame: &Frame, lights: &[Light], scale: f32) -> Vec<f32> {
    let inv = 1.0 / scale.max(1.0e-9);
    let pi = std::f32::consts::PI;
    let n = lights.len().min(MAX_LIGHTS) as f32;
    let mut out = Vec::with_capacity(FRAME_FLOATS);
    let (dir, colour, has) = match frame.sun {
        Some((d, c)) => (d.normalize(), c * pi * inv, 1.0),
        None => (Vec3::Z, Vec3::ZERO, 0.0),
    };
    // ForwardGlobalLightData: [0] (NumLocalLights, NumReflectionCaptures, HasDirectionalLight, NumGridCells), [1] CulledGridSize,
    // MaxCulledLightsPerCell, [2] LightGridPixelSizeShift (every pixel in cell 0), [3] LightGridZParams (slice 0), [4] the direction
    // to the light, [5] its colour, [6] (ShadowMapChannelMask, -, DistanceFadeMAD = no fade); [7] (the harness's) the sun's
    // visibility (PrecomputedLightingBuffer[5].x) and the number of lights
    out.extend_from_slice(&[n, 1.0, has, 1.0, 1.0, 1.0, 1.0, MAX_LIGHTS as f32, 30.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    out.extend_from_slice(&[dir.x, dir.y, dir.z, 0.0, colour.x, colour.y, colour.z, 1.0, SUN_MASK, 0.0, 0.0, 1.0]);
    out.extend_from_slice(&[frame.visibility.clamp(0.0, 1.0), n, 0.0, 0.0]);
    for c in frame.env {
        let c = c * inv;
        out.extend_from_slice(&[c.x, c.y, c.z, 1.0]);
    }
    match frame.scene {
        Some((bright, dark)) => {
            let (b, d) = (bright * inv, dark * inv);
            out.extend_from_slice(&[b.x, b.y, b.z, 1.0, d.x, d.y, d.z, 1.0]);
        }
        None => out.extend_from_slice(&[1.0, 1.0, 1.0, 0.0, 0.45, 0.45, 0.45, 0.0]),
    }
    for k in 0..MAX_LIGHTS {
        match lights.get(k) {
            Some(l) => {
                let c = l.color * pi * inv;
                out.extend_from_slice(&[l.position.x, l.position.y, l.position.z, 1.0 / l.radius.max(1.0)]);
                out.extend_from_slice(&[c.x, c.y, c.z, l.exponent.max(1.0e-3)]);
                // a point light: direction unused, lighting channel 0; SpotAngles (-2, 1): no cone; no source radius / length
                out.extend_from_slice(&[0.0, 0.0, 1.0, LIGHT_MASK, -2.0, 1.0, 0.0, 0.0]);
            }
            None => out.extend_from_slice(&[0.0; 16]),
        }
    }
    out
}

/// Appends this frame's rows to the effects' frame constants (vfx.rs `draw`, right after `lit`).
pub fn extend_frame(frame: &mut Vec<f32>, scale: f32) {
    let f = FRAME.lock().ok().and_then(|g| *g).unwrap_or(Frame { visibility: 1.0, ..Frame::default() });
    let lights = LIGHTS.lock().map(|l| l.clone()).unwrap_or_default();
    frame.extend(rows(&f, &lights, scale));
}

/// The eye-adaptation frame's copy (vfx.rs `frame_eye`: its out scale x `k`, its lit / `k`): the light colours / `k` too, so a
/// lit surface shows what it shows in the other frame. `rows` = the appended part.
pub fn eye(rows: &mut [f32], k: f32) {
    let k = k.max(1.0e-6);
    let mut scale = |at: usize, n: usize| {
        for v in rows.iter_mut().skip(at).take(n) {
            *v /= k;
        }
    };
    // the sun's colour (fwd_g[5].xyz), the environment and the scene colours (fwd_env[0..4].xyz), each light's colour
    scale(5 * 4, 3);
    for r in 0..5 {
        scale((8 + r) * 4, 3);
    }
    for l in 0..MAX_LIGHTS {
        scale((13 + 4 * l + 1) * 4, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rows are what er_vfx.wgsl's `ErFrame.fwd_*` reads: the sun as UE's colour (x pi, KH3 units), its mask and visibility,
    /// a light's four rows, and nothing for the lights after the last.
    #[test]
    fn the_rows_are_the_harness_layout() {
        let f = Frame { sun: Some((Vec3::new(0.0, 0.0, 2.0), Vec3::new(0.1, 0.2, 0.3))), visibility: 0.5, env: [Vec3::ONE; 3], scene: None };
        let l = [Light { position: Vec3::new(10.0, 20.0, 30.0), radius: 400.0, color: Vec3::splat(2.0), exponent: 2.0 }];
        let r = rows(&f, &l, 2.0);
        assert_eq!(r.len(), FRAME_FLOATS);
        assert_eq!(&r[0..4], &[1.0, 1.0, 1.0, 1.0]);
        assert_eq!(&r[16..19], &[0.0, 0.0, 1.0]);
        let pi = std::f32::consts::PI;
        assert!((r[20] - 0.1 * pi / 2.0).abs() < 1.0e-6 && (r[22] - 0.3 * pi / 2.0).abs() < 1.0e-6);
        assert_eq!((r[24], r[27], r[28], r[29]), (SUN_MASK, 1.0, 0.5, 1.0));
        // the scene colours without a rig: the harness's own (1, 0.45), flagged w = 0
        assert_eq!(&r[44..52], &[1.0, 1.0, 1.0, 0.0, 0.45, 0.45, 0.45, 0.0]);
        let at = 13 * 4;
        assert_eq!(&r[at..at + 4], &[10.0, 20.0, 30.0, 1.0 / 400.0]);
        assert!((r[at + 4] - pi).abs() < 1.0e-6 && r[at + 7] == 2.0);
        assert_eq!(&r[at + 8..at + 16], &[0.0, 0.0, 1.0, LIGHT_MASK, -2.0, 1.0, 0.0, 0.0]);
        assert!(r[at + 16..].iter().all(|v| *v == 0.0));
        // no sun: HasDirectionalLight 0, no colour
        let r = rows(&Frame { visibility: 1.0, ..Frame::default() }, &[], 1.0);
        assert_eq!((r[0], r[2], r[20], r[29]), (0.0, 0.0, 0.0, 0.0));
    }

    /// The eye frame divides exactly the colours (the sun, the environment and scene colours, the lights) by k.
    #[test]
    fn the_eye_frame_scales_the_colours_only() {
        let f = Frame { sun: Some((Vec3::Z, Vec3::ONE)), visibility: 1.0, env: [Vec3::ONE; 3], scene: Some((Vec3::ONE, Vec3::ONE)) };
        let l = [Light { position: Vec3::ONE, radius: 100.0, color: Vec3::ONE, exponent: 2.0 }];
        let a = rows(&f, &l, 1.0);
        let mut b = a.clone();
        eye(&mut b, 2.0);
        for (k, (x, y)) in a.iter().zip(&b).enumerate() {
            let colour = (20..23).contains(&k) || ((32..52).contains(&k) && k % 4 != 3) || (56..59).contains(&k);
            assert_eq!(*y, if colour { x / 2.0 } else { *x }, "float {k}");
        }
    }

    /// The precomputed shadowing is the game's sun visibility at his feet, softened by its shadow colour as the rig's key is.
    #[test]
    fn the_visibility_follows_the_shadow_mask() {
        let mut g = crate::light::GameLight { shadow_color: Vec3::ONE, ..Default::default() };
        g.mask.w = 0.0;
        assert_eq!(frame_from(None, Some(&g), None, 1.0, Vec3::ONE, 1.0).visibility, 0.0);
        g.mask.w = 1.0;
        assert_eq!(frame_from(None, Some(&g), None, 1.0, Vec3::ONE, 1.0).visibility, 1.0);
        g.shadow_color = Vec3::splat(0.5);
        g.mask.w = 0.0;
        assert!((frame_from(None, Some(&g), None, 1.0, Vec3::ONE, 1.0).visibility - 0.5).abs() < 1.0e-6);
        assert_eq!(frame_from(None, None, None, 1.0, Vec3::ONE, 1.0).visibility, 1.0);
    }
}
