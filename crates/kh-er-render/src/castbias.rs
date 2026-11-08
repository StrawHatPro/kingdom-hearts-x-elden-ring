//! (LIGHT-RT, 2026-11-08) THE DEPTH BIAS OF THE KH CHARACTERS' CASTERS IN ELDEN RING'S SUN ATLAS.
//!
//! The user (on DEPLOY4, Elden Ring Style): "look at the ring artifacts going up his body!". Light and dark contour bands,
//! about 5 cm apart, went up Sora's trousers, thighs and torso.
//!
//! Cause. With `r.shadow_real` (on by default) gfx.rs `draw_cast` and party.rs `cast` draw every KH character depth-only into
//! the game's cascaded sun atlas, so they cast on the world and on each other. That atlas is D16 over the light camera's
//! 4096 m (IMPL_ER17: 6.25 cm a step).
//!
//! The Elden Ring Style takes the game's own light at his texel raw (hycap_ps native branch). That includes the game's
//! screen-space sun mask (GXScreenSpaceCascadedShadow_PCF4 / PCF9 / Soft), which compares each pixel's light depth against
//! the atlas. The game's own `_Dpt` caster vertex shaders write no offset, and those mask shaders add none (disassembled:
//! er78/dis/rend, GXFlver_*_Dpt_Skin.vpo). So the margin a surface needs against its own entry in the atlas comes from the
//! casters' RASTERIZER depth bias. Ours had none (DepthBias 0, SlopeScaledDepthBias 0).
//!
//! His surface was therefore compared against its own depth, rounded to the nearest D16 step. Half of every 6.25 cm step
//! rounds towards the sun and shades itself, so each step of light depth up his body became one dark band (shadow acne).
//!
//! The KH3 Style never showed it: its capture ignores the owner's own body in the atlas (the KH caster maps, er_char.wgsl
//! hc_tap_kh). Whether ray tracing is on only decides whether this cascade mask or the game's separately traced shadow
//! (GXRend_CompositeSeparatedShadowMask*) is what lands on him in a given spot. Nothing is stored: kh-config.txt and kh-er.ini
//! carry nothing of it.
//!
//! Fix. The casters drawn INTO THE GAME'S ATLAS get a depth bias away from the sun, a constant part for the D16 rounding plus
//! a slope part for the filter's reach. The atlas is reversed (the nearer the light, the larger), so both values are
//! negative. The KH caster maps (our own, read only by our capture's owner test) stay unbiased: an atlas entry pushed away
//! from the sun still passes `atlas <= map + tolerance`, so the KH3 Style's owner test is unchanged.
//!
//! The values are proven by the offline model below (`tests`). It rasterises a body's light depth into a D16 atlas the way
//! Direct3D does (bias, then round), and filters it the way the game's mask does (3 x 3 bilinear compares).

use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

/// The constant part, in D16 steps (r = 1 / 65536 for a 16-bit UNORM target): -2 = 12.5 cm towards the far end of the light.
/// The D16 rounding is at most half a step, so 2 steps leave 1.5 steps of margin for the reference's own precision.
pub const CAST_DEPTH_BIAS: i32 = -2;
/// The slope part, x the triangle's largest depth change per atlas texel. The game's PCF9 / Soft filters compare
/// neighbouring texels up to two texels from the pixel's own (bilinear compares on a 3 x 3 grid), so the margin must exceed
/// two texels' change of depth. 3 covers that with one texel to spare.
pub const CAST_SLOPE_BIAS: f32 = -3.0;
/// No clamp: a grazing triangle (its side to the sun) may go as far as its slope takes it; it shades nothing of itself then.
pub const CAST_BIAS_CLAMP: f32 = 0.0;

/// The D16 steps the light camera's 4096 m are cut into (IMPL_ER17), metres a step.
pub const D16_STEP_M: f32 = 4096.0 / 65535.0;

static KEY_BIAS: AtomicI32 = AtomicI32::new(CAST_DEPTH_BIAS);
static KEY_SLOPE: AtomicU32 = AtomicU32::new(0);
static KEY_SLOPE_SET: AtomicU32 = AtomicU32::new(0);

/// (testing aid) `r.cast_bias` (D16 steps) / `r.cast_slope`: read when the casters' pipelines are made (the next payload
/// start). `None` = the default.
pub fn set_keys(bias: Option<i32>, slope: Option<f32>) {
    KEY_BIAS.store(bias.map_or(CAST_DEPTH_BIAS, |b| b.clamp(-1000, 1000)), Ordering::Relaxed);
    match slope.filter(|s| s.is_finite()) {
        Some(s) => {
            KEY_SLOPE.store(s.clamp(-64.0, 64.0).to_bits(), Ordering::Relaxed);
            KEY_SLOPE_SET.store(1, Ordering::Relaxed);
        }
        None => KEY_SLOPE_SET.store(0, Ordering::Relaxed),
    }
}

/// The rasterizer depth bias the casters into the game's atlas are made with: (DepthBias, SlopeScaledDepthBias,
/// DepthBiasClamp).
pub fn atlas_bias() -> (i32, f32, f32) {
    let slope = if KEY_SLOPE_SET.load(Ordering::Relaxed) != 0 { f32::from_bits(KEY_SLOPE.load(Ordering::Relaxed)) } else { CAST_SLOPE_BIAS };
    (KEY_BIAS.load(Ordering::Relaxed), slope, CAST_BIAS_CLAMP)
}

/// What Direct3D 12 stores for a caster texel in a D16 target: the depth (in steps) plus DepthBias x 1 + Slope x the
/// triangle's largest change per texel (steps), clamped by `clamp` when it is not 0, rounded to the nearest step.
pub fn d16_stored(depth_steps: f32, max_slope_steps: f32, bias: (i32, f32, f32)) -> f32 {
    let mut b = bias.0 as f32 + bias.1 * max_slope_steps;
    if bias.2 > 0.0 {
        b = b.min(bias.2 * 65536.0);
    } else if bias.2 < 0.0 {
        b = b.max(bias.2 * 65536.0);
    }
    (depth_steps + b).clamp(0.0, 65535.0).round()
}

/// The status line part.
pub fn status() -> String {
    let (b, s, c) = atlas_bias();
    format!("(LIGHT-RT) the KH casters in the game's sun atlas with depth bias {b} step(s) + slope {s:.2} (clamp {c}) (r.cast_bias / r.cast_slope); the KH caster maps unbiased")
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    /// One light: towards the sun `l`, the atlas texel size (m) and the light-plane axes.
    struct Light {
        l: Vec3,
        lx: Vec3,
        ly: Vec3,
        texel: f32,
    }

    impl Light {
        fn new(elevation_deg: f32, azimuth_deg: f32, texel: f32) -> Light {
            let (e, a) = (elevation_deg.to_radians(), azimuth_deg.to_radians());
            let l = Vec3::new(e.cos() * a.sin(), -e.cos() * a.cos(), e.sin()).normalize();
            let lx = l.cross(Vec3::Z).normalize_or(Vec3::X);
            let ly = l.cross(lx).normalize();
            Light { l, lx, ly, texel }
        }
        /// light depth in D16 steps (the larger, the nearer the sun: the game's reversed atlas)
        fn steps(&self, p: Vec3) -> f32 {
            (p.dot(self.l) + 1000.0) / D16_STEP_M
        }
    }

    /// The body: vertical cylinders (centre x, y, radius, bottom, top), z up, metres: two legs and a torso, about Sora's size.
    const BODY: [(f32, f32, f32, f32, f32); 3] = [(-0.10, 0.0, 0.085, 0.0, 0.75), (0.10, 0.0, 0.085, 0.0, 0.75), (0.0, 0.0, 0.17, 0.75, 1.30)];

    /// The nearest-to-the-sun surface along the ray through the light-plane point (x, y): (its point, its normal).
    fn cast_hit(light: &Light, x: f32, y: f32) -> Option<(Vec3, Vec3)> {
        let o = light.lx * x + light.ly * y + light.l * 50.0;
        let d = -light.l;
        let mut best: Option<(f32, Vec3, Vec3)> = None;
        for &(cx, cy, r, z0, z1) in &BODY {
            let (ox, oy) = (o.x - cx, o.y - cy);
            let a = d.x * d.x + d.y * d.y;
            let b = 2.0 * (ox * d.x + oy * d.y);
            let c = ox * ox + oy * oy - r * r;
            let disc = b * b - 4.0 * a * c;
            if a > 1.0e-9 && disc >= 0.0 {
                let t = (-b - disc.sqrt()) / (2.0 * a);
                let p = o + d * t;
                if p.z >= z0 && p.z <= z1 && best.is_none_or(|(bt, _, _)| t < bt) {
                    best = Some((t, p, Vec3::new(p.x - cx, p.y - cy, 0.0).normalize()));
                }
            }
            // the top cap
            if d.z.abs() > 1.0e-6 {
                let t = (z1 - o.z) / d.z;
                let p = o + d * t;
                if t > 0.0 && (p.x - cx).powi(2) + (p.y - cy).powi(2) <= r * r && best.is_none_or(|(bt, _, _)| t < bt) {
                    best = Some((t, p, Vec3::Z));
                }
            }
        }
        best.map(|(_, p, n)| (p, n))
    }

    /// The atlas around the body: what the caster pass stores at each texel (D16 steps; 0 = cleared, nothing there).
    struct Atlas {
        size: i32,
        origin: (f32, f32),
        texels: Vec<f32>,
        texel: f32,
    }

    fn rasterise(light: &Light, bias: (i32, f32, f32)) -> Atlas {
        let span = 2.0;
        let size = (span / light.texel).ceil() as i32;
        let origin = (-span * 0.5, -span * 0.5 + 0.7 * light.ly.z);
        let mut texels = vec![0.0; (size * size) as usize];
        for j in 0..size {
            for i in 0..size {
                let x = origin.0 + (i as f32 + 0.5) * light.texel;
                let y = origin.1 + (j as f32 + 0.5) * light.texel;
                if let Some((p, n)) = cast_hit(light, x, y) {
                    // (the triangle's plane: the tangent plane here; its depth change per texel along the light-plane axes)
                    let nl = n.dot(light.l).max(1.0e-4);
                    let per_texel = light.texel / D16_STEP_M;
                    let slope = ((n.dot(light.lx) / nl).abs().max((n.dot(light.ly) / nl).abs()) * per_texel).min(1.0e4);
                    texels[(j * size + i) as usize] = d16_stored(light.steps(p), slope, bias);
                }
            }
        }
        Atlas { size, origin, texels, texel: light.texel }
    }

    impl Atlas {
        fn at(&self, i: i32, j: i32) -> f32 {
            if i < 0 || j < 0 || i >= self.size || j >= self.size {
                return 0.0;
            }
            self.texels[(j * self.size + i) as usize]
        }
        /// The game's mask at a light-plane point: 3 x 3 bilinear compares one texel apart, tent weights (PCF9's footprint);
        /// a texel shades when it is nearer the sun than the reference (stored > reference: reversed depth). 1 = lit.
        fn mask(&self, x: f32, y: f32, reference: f32) -> f32 {
            let gx = (x - self.origin.0) / self.texel - 0.5;
            let gy = (y - self.origin.1) / self.texel - 0.5;
            let mut sum = 0.0;
            let mut weights = 0.0;
            for dj in -1i32..=1 {
                for di in -1i32..=1 {
                    let w = (2 - di.abs()) as f32 * (2 - dj.abs()) as f32;
                    let (sx, sy) = (gx + di as f32, gy + dj as f32);
                    let (bx, by) = (sx.floor(), sy.floor());
                    let (fx, fy) = (sx - bx, sy - by);
                    let lit = |i: i32, j: i32| if self.at(i, j) > reference { 0.0 } else { 1.0 };
                    let (i, j) = (bx as i32, by as i32);
                    let top = lit(i, j) * (1.0 - fx) + lit(i + 1, j) * fx;
                    let bottom = lit(i, j + 1) * (1.0 - fx) + lit(i + 1, j + 1) * fx;
                    sum += w * (top * (1.0 - fy) + bottom * fy);
                    weights += w;
                }
            }
            sum / weights
        }
    }

    /// Whether ANOTHER surface lies between the point and the sun within the filter's reach (three texels around it): another
    /// part of the body (the torso over the legs) or its own part's flat top (the model's cylinders end in a flat cap, which a
    /// body's rounded shoulders do not). There the game's mask shades him for real or softly at the edge, so it is no acne.
    fn shaded_by_another_part(light: &Light, p: Vec3, own: usize) -> bool {
        let reach = 3.0 * light.texel;
        for (k, &(cx, cy, r, z0, z1)) in BODY.iter().enumerate() {
            if k == own {
                // its own top: where the ray towards the sun crosses the top's height, within the reach of the cap
                if light.l.z > 1.0e-4 && p.z < z1 {
                    let q = p + light.l * ((z1 - p.z) / light.l.z);
                    if (q.x - cx).hypot(q.y - cy) < r + reach {
                        return true;
                    }
                }
                continue;
            }
            for (ox, oy) in [(0.0, 0.0), (reach, 0.0), (-reach, 0.0), (0.0, reach), (0.0, -reach)] {
                let o = p + light.lx * ox + light.ly * oy;
                // the ray o + t l, t > 0, against the cylinder's wall and its top cap
                let (dx, dy) = (o.x - cx, o.y - cy);
                let a = light.l.x * light.l.x + light.l.y * light.l.y;
                let b = 2.0 * (dx * light.l.x + dy * light.l.y);
                let c = dx * dx + dy * dy - (r + reach) * (r + reach);
                let disc = b * b - 4.0 * a * c;
                if a > 1.0e-9 && disc >= 0.0 {
                    for t in [(-b - disc.sqrt()) / (2.0 * a), (-b + disc.sqrt()) / (2.0 * a)] {
                        let z = o.z + light.l.z * t;
                        if t > 0.0 && z >= z0 - reach && z <= z1 + reach {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// The body seen from the front (camera along +y, orthographic): every pixel on it with its point and normal.
    fn front_view(width: usize, height: usize) -> Vec<Option<(Vec3, Vec3, usize)>> {
        let mut out = vec![None; width * height];
        let m_per_px = 1.4 / height as f32;
        for py in 0..height {
            for px in 0..width {
                let x = (px as f32 + 0.5 - width as f32 * 0.5) * m_per_px;
                let z = 1.35 - (py as f32 + 0.5) * m_per_px;
                for (k, &(cx, cy, r, z0, z1)) in BODY.iter().enumerate() {
                    let dx = x - cx;
                    if dx.abs() < r && z >= z0 && z <= z1 {
                        let dy = -(r * r - dx * dx).sqrt();
                        out[py * width + px] = Some((Vec3::new(x, cy + dy, z), Vec3::new(dx, dy, 0.0).normalize(), k));
                        break;
                    }
                }
            }
        }
        out
    }

    /// What the game's mask does to the body's sunlit side.
    struct Shaded {
        /// the sun's visibility at each pixel (1 off the body)
        vis: Vec<f32>,
        /// the worst self-shading where the surface faces the sun (N.L > 0.15: a convex part cannot shade itself there, the
        /// truth is 1) and no other part shades it
        worst: f32,
        /// such pixels shaded below 0.9
        banded: usize,
        /// THE RINGS: up each column of such pixels, the steps of the visibility by more than 0.1 from one pixel to the next
        rings: usize,
        /// the mean self-shading over such pixels: how far the cascade mask is from a shadow source without his own body in it
        /// (the game's ray-traced shadow, RT on), which gives 1 there
        mean: f32,
    }

    fn shade(light: &Light, bias: (i32, f32, f32), view: &[Option<(Vec3, Vec3, usize)>], width: usize) -> Shaded {
        let atlas = rasterise(light, bias);
        let mut vis = vec![1.0; view.len()];
        let mut judged = vec![false; view.len()];
        let mut worst: f32 = 0.0;
        let mut banded = 0;
        let (mut sum, mut count) = (0.0, 0usize);
        for (k, px) in view.iter().enumerate() {
            let Some((p, n, own)) = px else { continue };
            let v = atlas.mask(p.dot(light.lx), p.dot(light.ly), light.steps(*p));
            vis[k] = v;
            if n.dot(light.l) > 0.15 && !shaded_by_another_part(light, *p, *own) {
                judged[k] = true;
                worst = worst.max(1.0 - v);
                sum += 1.0 - v;
                count += 1;
                if v < 0.9 {
                    banded += 1;
                }
            }
        }
        let rings = (width..view.len()).filter(|&k| judged[k] && judged[k - width] && (vis[k] - vis[k - width]).abs() > 0.1).count();
        Shaded { vis, worst, banded, rings, mean: sum / count.max(1) as f32 }
    }

    const UNBIASED: (i32, f32, f32) = (0, 0.0, 0.0);

    /// The suns the user's spot and the rest of the day give: elevation 15..85 degrees, from in front, the side and behind;
    /// the finest cascade (4096 texels over +-46 m: 2.25 cm, IMPL_ER17), which holds every point within tens of metres of the
    /// camera (the leader always), and twice as coarse (a 2048 viewport over the same span). The coarser cascades hold a
    /// character only far away, a few pixels tall, where one texel is wider than a leg.
    fn suns() -> Vec<Light> {
        let mut out = Vec::new();
        for texel in [0.0225, 0.045] {
            for elevation in [15.0, 35.0, 55.0, 70.7, 85.0] {
                for azimuth in [0.0, 50.0, 100.0, 160.0] {
                    out.push(Light::new(elevation, azimuth, texel));
                }
            }
        }
        out
    }

    #[test]
    fn the_unbiased_caster_bands_his_lit_side_like_the_screenshot() {
        // the user's spot: the sun's direction in the log (0.242, 0.944, 0.224) = 70.7 degrees up, the finest cascade
        let view = front_view(120, 240);
        let light = Light::new(70.7, 15.0, 0.0225);
        let s = shade(&light, UNBIASED, &view, 120);
        assert!(s.worst > 0.5, "the unbiased atlas shades his sunlit side by {}", s.worst);
        assert!(s.banded > 300, "only {} sunlit pixel(s) banded", s.banded);
        assert!(s.rings > 100, "only {} ring step(s)", s.rings);
        // RT OFF (the cascade mask) and RT ON (a traced shadow without his body) disagree on his sunlit side
        assert!(s.mean > 0.1, "the unbiased mask is off by {} on average", s.mean);
    }

    #[test]
    fn the_biased_caster_leaves_his_lit_side_clean_under_every_sun() {
        // no ring anywhere, nothing shaded below 0.9; the most left is the filter's soft reach at a grazing wall right below a
        // nearer surface (the torso's top within two texels of the light plane: 5 % at most, smooth), as on the game's own
        // characters
        let view = front_view(60, 120);
        for light in suns() {
            let s = shade(&light, atlas_bias_default(), &view, 60);
            assert!(s.rings == 0 && s.banded == 0 && s.worst < 0.08, "sun {:?} texel {}: self-shading {}, {} banded pixel(s), {} ring step(s)", light.l, light.texel, s.worst, s.banded, s.rings);
            // RT OFF and RT ON shade his sunlit side alike: the cascade mask agrees with a traced shadow without his own body
            assert!(s.mean < 0.005, "sun {:?} texel {}: the mask is off by {} on average", light.l, light.texel, s.mean);
        }
    }

    #[test]
    fn the_bias_keeps_the_body_shading_its_far_side_and_the_ground() {
        // his own side away from the sun stays shaded by his own body in the game's mask (Elden Ring's shadow on him), and
        // the bias moves his shadow on the ground by at most a few centimetres along the light
        let light = Light::new(45.0, 30.0, 0.0225);
        let atlas = rasterise(&light, atlas_bias_default());
        // a ground point well inside his shadow (behind the torso, away from the sun)
        let behind = Vec3::new(0.0, 0.0, 1.0) - Vec3::new(light.l.x, light.l.y, 0.0).normalize() * 0.9;
        let p = Vec3::new(behind.x, behind.y, 0.0);
        assert!(atlas.mask(p.dot(light.lx), p.dot(light.ly), light.steps(p)) < 0.05, "the ground behind him is lit");
        let (b, s, _) = atlas_bias_default();
        let push_m = (-(b as f32) + -s * (light.texel / D16_STEP_M)) * D16_STEP_M;
        assert!(push_m < 0.25, "the caster is pushed {push_m} m away from the sun");
    }

    #[test]
    fn the_bias_points_away_from_the_sun_in_the_reversed_atlas() {
        let (b, s, c) = atlas_bias_default();
        assert!(b < 0 && s < 0.0 && c == 0.0);
        // a stored depth only ever moves away from the sun (smaller in the reversed atlas)
        assert!(d16_stored(30000.0, 2.0, (b, s, c)) < 30000.0);
        assert_eq!(d16_stored(30000.4, 0.0, UNBIASED), 30000.0);
        assert_eq!(d16_stored(30000.6, 0.0, UNBIASED), 30001.0);
    }

    #[test]
    fn the_keys_change_the_bias_and_nothing_else_persists() {
        set_keys(Some(-5), Some(-1.5));
        assert_eq!(atlas_bias(), (-5, -1.5, 0.0));
        set_keys(None, None);
        assert_eq!(atlas_bias(), atlas_bias_default());
        assert!(status().contains("depth bias -2"));
    }

    #[test]
    fn the_route_and_the_bias_come_from_the_keys_alone() {
        // the Elden Ring Style (native) and the casters' bias are worked out from the control keys every time they are read:
        // nothing about ray tracing, a calibration or a cache is saved and read back, so an RT switch or a restart cannot
        // leave a different route behind
        let d = crate::hybrid::Settings::default();
        assert!(d.native, "the Elden Ring Style is the default");
        assert_eq!(crate::hybrid::with_keys(d, &|_| None).native, d.native);
        assert!(!crate::hybrid::with_keys(d, &|k| (k == "r.hybrid_native").then_some(0.0)).native);
        assert!(crate::hybrid::with_keys(d, &|k| (k == "r.hybrid_native").then_some(1.0)).native);
        // (the bias keys: the_keys_change_the_bias_and_nothing_else_persists, the one test that sets them)
    }

    fn atlas_bias_default() -> (i32, f32, f32) {
        (CAST_DEPTH_BIAS, CAST_SLOPE_BIAS, CAST_BIAS_CLAMP)
    }

    /// The before / after pictures (`KH_LIGHT_RT_PICS=<dir>`): the body from the front, lit by the sun through the game's mask
    /// (0.25 sky + 0.75 sun x N.L x the mask), unbiased on the left, biased on the right, at the user's sun and two more.
    #[test]
    fn pictures_before_and_after() {
        let Some(dir) = std::env::var_os("KH_LIGHT_RT_PICS") else { return };
        let (w, h) = (120usize, 240usize);
        let view = front_view(w, h);
        let lights = [Light::new(70.7, 15.0, 0.0225), Light::new(35.0, 60.0, 0.0225), Light::new(55.0, -30.0, 0.09)];
        let cols = lights.len() * 2;
        let mut rgba = vec![0u8; w * cols * h * 4];
        for (li, light) in lights.iter().enumerate() {
            for (bi, bias) in [UNBIASED, atlas_bias_default()].iter().enumerate() {
                let vis = shade(light, *bias, &view, w).vis;
                let col = li * 2 + bi;
                for y in 0..h {
                    for x in 0..w {
                        let k = y * w + x;
                        let o = (y * w * cols + col * w + x) * 4;
                        let c = match view[k] {
                            Some((_, n, _)) => {
                                let v = 0.25 + 0.75 * n.dot(light.l).max(0.0) * vis[k];
                                (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8
                            }
                            None => 40,
                        };
                        rgba[o..o + 4].copy_from_slice(&[c, c, if view[k].is_some() { c } else { 60 }, 255]);
                    }
                }
            }
        }
        let path = std::path::Path::new(&dir).join("offline_rings_before_after.png");
        crate::hybrid::write_png(&path, (w * cols) as u32, h as u32, &rgba).unwrap();
    }
}
