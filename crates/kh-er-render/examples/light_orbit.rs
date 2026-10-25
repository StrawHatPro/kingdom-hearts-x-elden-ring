//! (IMPL_ER86) THE ORBIT TEST OF SORA'S LIGHT, offline: the camera orbits a still Sora under a fixed world light, and his lit
//! colour per body region is recorded per frame, through the renderer's own light code (light.rs `rig`, pointlight.rs
//! `white_level` / `DarkGate` / `plan` / `sora_lights`, stable.rs) - once as before IMPL_ER86, once with the steady light.
//!
//! What the game would hand that code is MODELLED (it cannot be read offline): a world of ground, a cliff and a tree's shadow,
//! lit by a sun and a sky; the game's targets at a pixel (scene colour, direct light x surface colour, shadow mask, surface
//! colour) from a ray through that pixel, with his body in the depth (his early depth, IMPL_ER53: a pixel on his body is shaded
//! with the G-buffer of what lies behind it) and the game's screen-space occlusion around him; the picture's top row (the
//! environment taps); the game's eye adaptation (its exposure follows the share of sky in view). The probes are taken the old
//! way (the 3 x 3 screen grid of probe.ps.dxil) and the new way (stable.rs `world_taps` / `hidden_by`, the mirror of
//! probe3.ps.dxil's ER_PROBE_WORLD).
//!
//! ```text
//! cargo run --release -p kh-er-render --no-default-features --example light_orbit -- --out build/sora_tmp/er/er86/orbit
//! ```
//! Writes `<out>_<scenario>_<mode>.csv`, `<out>_summary.txt`, `<out>_<scenario>.svg` and `<out>_light_<scenario>_<mode>_<deg>.txt`
//! (log-format `light:` lines for the effects harness's `--game-light`).

use glam::{Vec3, Vec4};
use kh_er_render::light::{self, GameLight};
use kh_er_render::pointlight::{self, DarkGate, LightSettings};
use kh_er_render::stable::{self, Settings};

const W: f32 = 1280.0;
const H: f32 = 720.0;
const FOV_Y: f32 = 50.0;
const FPS: f32 = 60.0;
const GRASS: Vec3 = Vec3::new(0.32, 0.26, 0.15);
const ROCK: Vec3 = Vec3::new(0.25, 0.24, 0.22);
const CLEAR: f32 = 0.014;
const WALL_X: f32 = 320.0;
const SHADE_X: f32 = -40.0;

/// The world's light of a scenario.
#[derive(Clone, Copy)]
struct World {
    /// unit vector to the sun (game space) and its colour (cbLight Col_0), the sky light's colour (Col_1)
    sun_dir: Vec3,
    sun: Vec3,
    sky: Vec3,
    /// the environment / GI light per unit of surface colour
    gi: f32,
    /// a torch on the ground's light (game space, colour x intensity, radius cm)
    torch: Option<(Vec3, Vec3, f32)>,
    /// what the sky shows the eye adaptation (scene units)
    sky_lum: f32,
    fog: [Vec3; 3],
}

#[derive(Clone, Copy, Debug)]
enum Hit {
    Sky,
    Ground(Vec3),
    Wall(Vec3),
}

struct Cam {
    pos: Vec3,
    right: Vec3,
    up: Vec3,
    fwd: Vec3,
    tan_y: f32,
}

impl Cam {
    fn look(pos: Vec3, at: Vec3) -> Cam {
        let fwd = (at - pos).normalize();
        // game space is left-handed (X forward, Y right, Z up): right = up x forward
        let right = Vec3::Z.cross(fwd).normalize();
        let up = fwd.cross(right).normalize();
        Cam { pos, right, up, fwd, tan_y: (FOV_Y.to_radians() * 0.5).tan() }
    }
    fn project(&self, p: Vec3) -> Option<(f32, f32, f32)> {
        let d = p - self.pos;
        let z = d.dot(self.fwd);
        if z <= 1.0 {
            return None;
        }
        let tan_x = self.tan_y * W / H;
        let (x, y) = (d.dot(self.right) / (z * tan_x), d.dot(self.up) / (z * self.tan_y));
        Some(((x * 0.5 + 0.5) * W, (0.5 - y * 0.5) * H, z))
    }
    fn ray(&self, px: f32, py: f32) -> Vec3 {
        let tan_x = self.tan_y * W / H;
        let (x, y) = ((px / W) * 2.0 - 1.0, 1.0 - (py / H) * 2.0);
        (self.fwd + self.right * x * tan_x + self.up * y * self.tan_y).normalize()
    }
}

/// His capsules standing, facing +X (game space cm): (a, b, radius); the frame-constant layout of gfx.rs (`er.caps`).
fn capsules(feet: Vec3) -> Vec<Vec4> {
    let segs: [(Vec3, Vec3, f32); 13] = [
        (Vec3::new(0.0, 0.0, 95.0), Vec3::new(0.0, 0.0, 125.0), 15.0),
        (Vec3::new(0.0, 0.0, 125.0), Vec3::new(0.0, 0.0, 140.0), 14.0),
        (Vec3::new(0.0, 0.0, 150.0), Vec3::new(0.0, 0.0, 162.0), 12.0),
        (Vec3::new(0.0, -18.0, 135.0), Vec3::new(0.0, -25.0, 110.0), 6.5),
        (Vec3::new(0.0, -25.0, 110.0), Vec3::new(2.0, -27.0, 85.0), 6.0),
        (Vec3::new(0.0, 18.0, 135.0), Vec3::new(0.0, 25.0, 110.0), 6.5),
        (Vec3::new(0.0, 25.0, 110.0), Vec3::new(2.0, 27.0, 85.0), 6.0),
        (Vec3::new(0.0, -9.0, 90.0), Vec3::new(2.0, -10.0, 48.0), 10.0),
        (Vec3::new(2.0, -10.0, 48.0), Vec3::new(0.0, -10.0, 8.0), 9.0),
        (Vec3::new(0.0, 9.0, 90.0), Vec3::new(2.0, 10.0, 48.0), 10.0),
        (Vec3::new(2.0, 10.0, 48.0), Vec3::new(0.0, 10.0, 8.0), 9.0),
        (Vec3::new(-2.0, -10.0, 8.0), Vec3::new(14.0, -10.0, 4.0), 6.0),
        (Vec3::new(-2.0, 10.0, 8.0), Vec3::new(14.0, 10.0, 4.0), 6.0),
    ];
    let mut out = Vec::new();
    for (a, b, r) in segs {
        out.push((feet + a).extend(r));
        out.push((feet + b).extend(1.0));
    }
    out
}

fn ray_capsule(o: Vec3, d: Vec3, caps: &[Vec4]) -> Option<f32> {
    // marched: fine enough for pixels of a body a few metres away
    let mut best: Option<f32> = None;
    for c in caps.chunks_exact(2) {
        let (a, b, r) = (c[0].truncate(), c[1].truncate(), c[0].w);
        let mid = (a + b) * 0.5;
        let t0 = (mid - o).dot(d);
        if t0 < 0.0 {
            continue;
        }
        let mut t = (t0 - 60.0).max(0.0);
        while t < t0 + 60.0 {
            let p = o + d * t;
            if stable::seg_dist(p, p, a, b) < r {
                best = Some(best.map_or(t, |x: f32| x.min(t)));
                break;
            }
            t += 1.0;
        }
    }
    best
}

fn ray_world(o: Vec3, d: Vec3) -> (Hit, f32) {
    let mut best = (Hit::Sky, f32::MAX);
    if d.z < -1.0e-4 {
        let t = -o.z / d.z;
        if t > 0.0 && t < 6000.0 {
            best = (Hit::Ground(o + d * t), t);
        }
    }
    if d.x > 1.0e-4 {
        let t = (WALL_X - o.x) / d.x;
        let p = o + d * t;
        if t > 0.0 && t < best.1 && p.z >= 0.0 && p.z < 1500.0 {
            best = (Hit::Wall(p), t);
        }
    }
    best
}

/// The game's targets at a pixel: (scene colour, direct light x surface colour, shadow mask, surface colour).
struct Px {
    scene: Vec3,
    direct: Vec3,
    mask: Vec4,
    albedo: Vec3,
}

fn sun_lit(p: Vec3) -> f32 {
    if p.x < SHADE_X {
        0.0
    } else {
        1.0
    }
}

/// The light per unit of surface colour at `p` with normal `n` (the game's diffuse accumulation over the colour).
fn light_at(w: &World, p: Vec3, n: Vec3, lit: f32) -> Vec3 {
    let cal = light::CALIBRATION_DEFAULT * std::f32::consts::FRAC_1_PI;
    let sky_part = (n.z * 0.5 + 0.5).clamp(0.0, 1.0);
    let mut l = (w.sun * n.dot(w.sun_dir).max(0.0) * lit + w.sky * sky_part) * cal + Vec3::splat(w.gi);
    if let Some((at, c, r)) = w.torch {
        let to = at - p;
        let f = pointlight::radial(to.length_squared(), r, 2.0);
        l += c * f * n.dot(to.normalize_or_zero()).max(0.0) * std::f32::consts::FRAC_1_PI;
    }
    l
}

fn pixel(w: &World, cam: &Cam, px: f32, py: f32, feet: Vec3, caps: &[Vec4]) -> Px {
    let d = cam.ray(px, py);
    let (hit, t_world) = ray_world(cam.pos, d);
    let body = ray_capsule(cam.pos, d, caps).filter(|t| *t < t_world);
    let (n_bg, albedo, bg_point) = match hit {
        Hit::Sky => (Vec3::Z, Vec3::ZERO, None),
        Hit::Ground(p) => (Vec3::Z, GRASS, Some(p)),
        Hit::Wall(p) => (-Vec3::X, ROCK, Some(p)),
    };
    // where the game shades this pixel: his body when it is in front (early depth), else what the ray hits
    let (shade_at, ao) = match (body, bg_point) {
        (Some(t), _) => (Some(cam.pos + d * t), 0.7),
        (None, Some(p)) => {
            // the game's screen-space occlusion of the ground around his feet (his body is in its depth)
            let r = Vec3::new(p.x - feet.x, p.y - feet.y, 0.0).length();
            (Some(p), if matches!(hit, Hit::Ground(_)) { 1.0 - 0.5 * (-(r - 12.0).max(0.0) / 25.0).exp() } else { 1.0 })
        }
        (None, None) => (None, 1.0),
    };
    match shade_at {
        Some(q) if albedo.max_element() > 0.0 => {
            let lit = sun_lit(q);
            let l = light_at(w, q, n_bg, lit);
            let direct = l * albedo * ao;
            Px { scene: direct * 1.08, direct, mask: Vec4::new(ao, ao, 1.0, lit), albedo }
        }
        _ => Px { scene: Vec3::splat(CLEAR), direct: Vec3::ZERO, mask: Vec4::ONE, albedo: Vec3::ZERO },
    }
}

/// The probes of probe.ps.dxil: the 3 x 3 grid around the pixel of his feet (texels 0..4 and 6 of the probe row).
fn probe_grid(w: &World, cam: &Cam, feet: Vec3, caps: &[Vec4], out: &mut GameLight) {
    let Some((fx, fy, z)) = cam.project(feet) else { return };
    let (cx, cy) = (fx.clamp(0.04 * W, 0.96 * W), fy.clamp(0.04 * H, 0.96 * H));
    let spread = (0.35 / ((z * 0.01) * cam.tan_y) * H * 0.5).clamp(2.0, 200.0);
    let (mut scene, mut direct, mut mask, mut albedo) = (Vec3::ZERO, Vec3::ZERO, Vec4::ZERO, Vec3::ZERO);
    for k in 0..9 {
        let (ox, oy) = ((k % 3) as f32 - 1.0, (k / 3) as f32 - 1.0);
        let (x, y) = ((cx + ox * spread).clamp(0.0, W - 1.0), (cy + oy * spread).clamp(0.0, H - 1.0));
        let p = pixel(w, cam, x.floor() + 0.5, y.floor() + 0.5, feet, caps);
        scene += p.scene;
        direct += p.direct;
        mask += p.mask / 9.0;
        albedo += p.albedo;
    }
    (out.scene_px, out.direct_px, out.mask, out.albedo_px, out.taps) = (scene / 9.0, direct / 9.0, mask, albedo / 9.0, 9.0);
}

/// The world probe of probe3.ps.dxil (ER_PROBE_WORLD), mirrored: stable.rs `world_taps` / `hidden_by`.
fn probe_world(w: &World, cam: &Cam, feet: Vec3, caps: &[Vec4], radius: f32, out: &mut GameLight) -> [Option<stable::Tap>; stable::WORLD_TAPS] {
    let (mut scene, mut direct, mut mask, mut albedo, mut n) = (Vec3::ZERO, Vec3::ZERO, Vec4::ZERO, Vec3::ZERO, 0.0);
    let mut taps = [None; stable::WORLD_TAPS];
    for (k, p) in stable::world_taps(feet, radius).into_iter().enumerate() {
        let Some((x, y, _)) = cam.project(p) else { continue };
        if !(0.01 * W..0.99 * W).contains(&x) || !(0.01 * H..0.99 * H).contains(&y) {
            continue;
        }
        // the scene depth WITHOUT him at that pixel: the ground near the tap?
        let d = cam.ray(x.floor() + 0.5, y.floor() + 0.5);
        let (hit, _) = ray_world(cam.pos, d);
        let q = match hit {
            Hit::Ground(q) | Hit::Wall(q) => q,
            Hit::Sky => continue,
        };
        if q.distance(p) > stable::WORLD_TOLERANCE || stable::hidden_by(caps, cam.pos, p) {
            continue;
        }
        let px = pixel(w, cam, x.floor() + 0.5, y.floor() + 0.5, feet, caps);
        if px.albedo.min_element() <= 0.02 {
            continue;
        }
        scene += px.scene;
        direct += px.direct;
        mask += px.mask;
        albedo += px.albedo;
        n += 1.0;
        taps[k] = Some(stable::Tap { direct: px.direct, albedo: px.albedo, scene: px.scene, mask: px.mask });
    }
    if n > 0.0 {
        (out.scene_px, out.direct_px, out.mask, out.albedo_px, out.taps) = (scene / n, direct / n, mask / n, albedo / n, n);
    }
    taps
}

/// The picture's top (texel 6): the brightest of 18 taps.
fn env_taps(w: &World, cam: &Cam, feet: Vec3, caps: &[Vec4], out: &mut GameLight) {
    let mut best = Vec3::ZERO;
    for k in 0..9 {
        for row in 0..2 {
            let c = pixel(w, cam, (0.1 + 0.1 * k as f32) * W, (0.04 + 0.09 * row as f32) * H, feet, caps).scene;
            if luma(c) > luma(best) {
                best = c;
            }
        }
    }
    (out.env_px, out.env_taps) = (best, 1.0);
}

fn luma(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// The share of sky in the picture (for the eye adaptation): the horizon's place, and the cliff that hides the sky.
fn sky_share(cam: &Cam) -> f32 {
    let mut sky = 0.0;
    for k in 0..16 {
        for j in 0..9 {
            let d = cam.ray((k as f32 + 0.5) / 16.0 * W, (j as f32 + 0.5) / 9.0 * H);
            if matches!(ray_world(cam.pos, d).0, Hit::Sky) {
                sky += 1.0;
            }
        }
    }
    sky / 144.0
}

const REGIONS: [(&str, Vec3, Vec3); 7] = [
    ("head top", Vec3::new(0.0, 0.0, 165.0), Vec3::new(0.0, 0.0, 1.0)),
    ("face", Vec3::new(12.0, 0.0, 155.0), Vec3::new(1.0, 0.0, 0.0)),
    ("chest", Vec3::new(15.0, 0.0, 115.0), Vec3::new(1.0, 0.0, 0.0)),
    ("back", Vec3::new(-15.0, 0.0, 115.0), Vec3::new(-1.0, 0.0, 0.0)),
    ("left side", Vec3::new(0.0, -20.0, 115.0), Vec3::new(0.0, -1.0, 0.0)),
    ("right side", Vec3::new(0.0, 20.0, 115.0), Vec3::new(0.0, 1.0, 0.0)),
    ("legs front", Vec3::new(10.0, 0.0, 50.0), Vec3::new(1.0, 0.0, 0.0)),
];

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Before,
    Steady,
}

struct Sample {
    t: f32,
    regions: [Vec3; 7],
    mask_w: f32,
    ambient: f32,
    env: f32,
    exposure: f32,
    hero: f32,
    level: f32,
    gate_dark: bool,
    cam_deg: f32,
}

/// A path of the camera / of him: (time) -> (camera position, his feet, his speed).
type Path = dyn Fn(f32) -> (Vec3, Vec3);

fn run(name: &str, world: &World, mode: Mode, seconds: f32, path: &Path, drop_sun: bool, hero_mode: u8) -> Vec<Sample> {
    let st = Settings { on: mode == Mode::Steady, ..Settings::default() };
    stable::set_settings(st);
    let ls = LightSettings { hero: hero_mode, ..LightSettings::default() };
    pointlight::set_settings(ls);
    let mut filter = stable::Filter::default();
    let mut memory = stable::TapMemory::default();
    let mut gate_hold = stable::Gate::default();
    let mut lift_ease = kh_er_render::cgfield::LiftEase::default();
    // (a forced hero light starts at full strength: the orbit measures its place, not its fade-in)
    let mut gate = if hero_mode == 2 { DarkGate { dark: true, weight: 1.0 } } else { DarkGate::default() };
    let mut prev: Option<GameLight> = None;
    let mut exposure = 0.0f32;
    let mut out = Vec::new();
    let dt = 1.0 / FPS;
    let frames = (seconds * FPS) as usize;
    let fallback = Vec3::new(kh_look::stand_in::KEY_DIR.y, kh_look::stand_in::KEY_DIR.z, kh_look::stand_in::KEY_DIR.x).normalize();
    for f in 0..frames {
        let t = f as f32 * dt;
        let (cam_pos, feet) = path(t);
        let cam = Cam::look(cam_pos, feet + Vec3::Z * 110.0);
        let caps = capsules(feet);
        // the game's eye adaptation (its own easing, 0.8 s)
        let sky = sky_share(&cam);
        let ground = luma(light_at(world, feet, Vec3::Z, sun_lit(feet)) * GRASS);
        // (its range is clamped, as an eye adaptation is: night stays dark)
        let target = (0.18 / (ground * (1.0 - sky) + world.sky_lum * sky).max(1.0e-5)).clamp(2.0, 30.0);
        exposure = if f == 0 { target } else { (exposure.ln() + (target.ln() - exposure.ln()) * stable::blend(dt, 0.8)).exp() };
        // the frame as the game would hand it (host axes for the light vectors: (y, z, x) of game space)
        let host = |g: Vec3| Vec3::new(g.y, g.z, g.x);
        let mut raw = GameLight {
            sun: Some((host(world.sun_dir), world.sun)),
            sky: (Vec3::Y, world.sky),
            shadow_color: Vec3::ONE,
            scene: true,
            exposure,
            pre_exposure: 1.0,
            fog: world.fog,
            ..GameLight::default()
        };
        if drop_sun && f % 20 == 7 {
            raw.sun = None;
        }
        if mode == Mode::Steady && st.probe_world {
            let taps = probe_world(world, &cam, feet, &caps, st.probe_radius, &mut raw);
            memory.update(&taps, feet, dt);
            raw = memory.apply(&raw);
        } else {
            probe_grid(world, &cam, feet, &caps, &mut raw);
        }
        env_taps(world, &cam, feet, &caps, &mut raw);
        let l = match mode {
            Mode::Before => stable::ease_old(prev.as_ref(), raw),
            Mode::Steady => filter.step(Some(&raw), Some(feet), dt, &st).unwrap(),
        };
        prev = Some(l);
        let rig = light::rig(&l, fallback, light::CALIBRATION_DEFAULT, 1.0);
        // (ER85) `ORBIT_OWN=facing`: an outfit with its own set (cgfield.rs, Quadratum / San Fransokyo) with the key anchored to
        // HIS facing (he faces game +X: host +Z); `ORBIT_OWN=camera`: the first ER85 version (the key turned toward the camera)
        let own = kh_er_render::cgfield::FieldShape { lift: 1.8, lift_share: 0.6, ..Default::default() };
        let rig = match std::env::var("ORBIT_OWN").as_deref() {
            Ok("facing") => kh_er_render::cgfield::shape_lifted(rig, Vec3::new(0.0, 0.0, 1.0), lift_ease.step(kh_er_render::cgfield::lift_for(&rig, l.mask.w, &own), dt), &own),
            // (San Fransokyo: no lift of its own)
            Ok("facing_plain") => kh_er_render::cgfield::shape(rig, Vec3::new(0.0, 0.0, 1.0), l.mask.w, &Default::default()),
            Ok("camera") => {
                let to_cam = cam_pos - feet;
                kh_er_render::cgfield::shape(rig, Vec3::new(to_cam.y, 0.0, to_cam.x), l.mask.w, &own)
            }
            _ => rig,
        };
        let key_dir = Vec3::new(rig.key_dir.z, rig.key_dir.x, rig.key_dir.y);
        let env_l = if mode == Mode::Steady { stable::env_view(&l, &st) } else { l };
        let env = light::env_level(&env_l, rig.out_scale, 1.0);
        // the hero light (Keyblade at his right hip), through the gate
        let level = pointlight::white_level(&l);
        let reading = if mode == Mode::Steady { gate_hold.step(level, dt, ls.hero_dark_on, ls.hero_dark_off, st.gate_hold) } else { level };
        gate.step(reading, dt, ls.hero_dark_on, ls.hero_dark_off, ls.hero_fade, ls.hero);
        let hero = gate.strength();
        let to_target = kh_er_render::vfx::effect_scale(Some(l.exposure), rig.out_scale);
        let blade = Some((feet + Vec3::new(35.0, 27.0, 55.0), 1.0));
        let pass = pointlight::plan(&ls, &[], to_target, to_target * light::reference_white(), Some(feet), hero, blade);
        let (toward, toward_cm) = if mode == Mode::Steady && st.hero_anchor == 1 { (feet + Vec3::X * 1000.0, st.hero_forward) } else if mode == Mode::Steady && st.hero_anchor == 2 { (feet, 0.0) } else { (cam.pos, ls.hero_sora_toward) };
        let on_him = pointlight::sora_lights(&pass, feet, toward, ls.hero_sora_height, toward_cm);
        let lift = pointlight::on_sora(&pass, feet + Vec3::Z * 100.0, true);
        let mut regions = [Vec3::ZERO; 7];
        for (k, (_, at, n)) in REGIONS.iter().enumerate() {
            // the r.flat white surface of gfx.rs (scene units): ambient + up x n.z + key x N.L, + his point lights
            let p = feet + *at;
            let mut c = (rig.ambient + rig.ambient_up * n.z.max(0.0)) * rig.out_scale + rig.key / std::f32::consts::PI * rig.out_scale * n.dot(key_dir).max(0.0) + lift;
            for s in &on_him {
                let to = s.position - p;
                c += s.color * pointlight::radial(to.length_squared(), s.radius, s.exponent) * n.dot(to.normalize_or_zero()).max(0.0);
            }
            regions[k] = c;
        }
        let cam_deg = (cam_pos.y - feet.y).atan2(cam_pos.x - feet.x).to_degrees();
        out.push(Sample { t, regions, mask_w: l.mask.w, ambient: luma(rig.ambient * rig.out_scale), env, exposure: l.exposure, hero, level: level.unwrap_or(0.0), gate_dark: gate.dark, cam_deg });
    }
    let _ = name;
    out
}

fn stats(v: &[f32]) -> (f32, f32, f32) {
    let n = v.len().max(1) as f32;
    let mean = v.iter().sum::<f32>() / n;
    let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n).sqrt();
    let (lo, hi) = v.iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
    (mean, if mean.abs() > 1.0e-9 { sd / mean * 100.0 } else { 0.0 }, if mean.abs() > 1.0e-9 { (hi - lo) / mean * 100.0 } else { 0.0 })
}

fn write_csv(path: &str, s: &[Sample]) {
    let mut text = String::from("t,cam_deg,exposure,mask_w,ambient,env_level,hero,white_level,gate_dark");
    for (name, ..) in REGIONS {
        text += &format!(",{}", name.replace(' ', "_"));
    }
    text += "\n";
    for x in s {
        text += &format!("{:.4},{:.1},{:.4},{:.4},{:.6},{:.4},{:.4},{:.4},{}", x.t, x.cam_deg, x.exposure, x.mask_w, x.ambient, x.env, x.hero, x.level, x.gate_dark as u8);
        for r in x.regions {
            text += &format!(",{:.6}", luma(r));
        }
        text += "\n";
    }
    std::fs::write(path, text).unwrap();
}

/// A small line chart (SVG) of one quantity over time, before (orange) and with the steady light (blue).
fn svg(title: &str, unit: &str, series: &[(&str, &str, Vec<(f32, f32)>)], marks: &[(f32, &str)]) -> String {
    let (w, h, l, r, t, b) = (760.0, 260.0, 64.0, 150.0, 34.0, 34.0);
    let xs = series.iter().flat_map(|s| s.2.iter().map(|p| p.0));
    let (x0, x1) = xs.clone().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
    let ys: Vec<f32> = series.iter().flat_map(|s| s.2.iter().map(|p| p.1)).collect();
    let (mut y0, mut y1) = ys.iter().fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(*y), b.max(*y)));
    y0 = y0.min(0.0);
    if y1 <= y0 {
        y1 = y0 + 1.0;
    }
    y1 += (y1 - y0) * 0.08;
    let px = |x: f32| l + (x - x0) / (x1 - x0).max(1.0e-6) * (w - l - r);
    let py = |y: f32| h - b - (y - y0) / (y1 - y0) * (h - t - b);
    let mut o = format!("<svg xmlns='http://www.w3.org/2000/svg' width='{w}' height='{h}' viewBox='0 0 {w} {h}' font-family='Segoe UI, Arial, sans-serif'>\n<rect width='100%' height='100%' fill='#fcfcfb'/>\n<text x='{l}' y='20' font-size='14' font-weight='600' fill='#0b0b0b'>{title}</text>\n");
    for k in 0..=4 {
        let y = y0 + (y1 - y0) * k as f32 / 4.0;
        o += &format!("<line x1='{l}' x2='{}' y1='{:.1}' y2='{:.1}' stroke='#e4e3df' stroke-width='1'/><text x='{}' y='{:.1}' font-size='11' fill='#52514e' text-anchor='end'>{}</text>\n", w - r, py(y), py(y), l - 6.0, py(y) + 4.0, fmt(y));
    }
    o += &format!("<text x='{}' y='{}' font-size='11' fill='#52514e' text-anchor='middle'>time (s)</text><text x='14' y='{:.1}' font-size='11' fill='#52514e' transform='rotate(-90 14 {:.1})' text-anchor='middle'>{unit}</text>\n", (l + w - r) * 0.5, h - 6.0, (t + h - b) * 0.5, (t + h - b) * 0.5);
    let mut tx = x0.ceil();
    while tx <= x1 {
        o += &format!("<text x='{:.1}' y='{}' font-size='11' fill='#52514e' text-anchor='middle'>{tx:.0}</text>\n", px(tx), h - b + 14.0);
        tx += if x1 - x0 > 12.0 { 2.0 } else { 1.0 };
    }
    for (at, label) in marks {
        o += &format!("<line x1='{:.1}' x2='{:.1}' y1='{t}' y2='{}' stroke='#c3c2b7' stroke-dasharray='3 3'/><text x='{:.1}' y='{}' font-size='10' fill='#52514e'>{label}</text>\n", px(*at), px(*at), h - b, px(*at) + 3.0, t + 10.0);
    }
    for (k, (name, colour, pts)) in series.iter().enumerate() {
        let d: String = pts.iter().enumerate().map(|(i, (x, y))| format!("{}{:.1},{:.1}", if i == 0 { "M" } else { "L" }, px(*x), py(*y))).collect();
        o += &format!("<path d='{d}' fill='none' stroke='{colour}' stroke-width='2' stroke-linejoin='round'/>\n");
        let ly = t + 14.0 + k as f32 * 18.0;
        o += &format!("<line x1='{}' x2='{}' y1='{ly:.1}' y2='{ly:.1}' stroke='{colour}' stroke-width='2'/><text x='{}' y='{:.1}' font-size='12' fill='#0b0b0b'>{name}</text>\n", w - r + 10.0, w - r + 30.0, w - r + 36.0, ly + 4.0);
    }
    o + "</svg>\n"
}

fn fmt(y: f32) -> String {
    if y.abs() >= 10.0 {
        format!("{y:.0}")
    } else if y.abs() >= 0.1 {
        format!("{y:.2}")
    } else {
        format!("{y:.4}")
    }
}

fn log_line(l: &GameLight) -> String {
    let v = |c: Vec3| format!("[{:.5}, {:.5}, {:.5}]", c.x, c.y, c.z);
    let (d, c) = l.sun.unwrap_or((Vec3::Y, Vec3::ZERO));
    format!(
        "  light: sun dir ({:.4}, {:.4}, {:.4}) colour ({:.4}, {:.4}, {:.4}) sky {} | at his feet: scene {} direct {} surface {} mask [{:.3}, {:.3}, {:.3}, {:.3}] probed true | scene true exposure {:.3} pre 1.00 jitter (0.0, 0.0) px | top of the picture {} (1 taps): x | fog ambient {} toward the light {} opposite {}",
        d.x, d.y, d.z, c.x, c.y, c.z, v(l.sky.1), v(l.scene_px), v(l.direct_px), v(l.albedo_px), l.mask.x, l.mask.y, l.mask.z, l.mask.w, l.exposure, v(l.env_px), v(l.fog[0]), v(l.fog[1]), v(l.fog[2])
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|k| args.get(k + 1)).cloned();
    let out = value("--out").unwrap_or_else(|| "light_orbit".to_string());
    let noon = World {
        sun_dir: Vec3::new(-0.45, 0.35, 0.82).normalize(),
        sun: Vec3::new(0.50, 0.48, 0.46),
        sky: Vec3::new(0.05, 0.06, 0.08),
        gi: 0.006,
        torch: None,
        sky_lum: 0.05,
        fog: [Vec3::new(0.02, 0.03, 0.035), Vec3::new(0.170, 0.282, 0.308), Vec3::new(0.0065, 0.0086, 0.0098)],
    };
    // night by a ruin: a weak moon; the ground in the open shows a white level near the gate's thresholds
    let night = World {
        sun_dir: Vec3::new(-0.45, 0.35, 0.82).normalize(),
        sun: Vec3::new(0.0105, 0.014, 0.0235),
        sky: Vec3::new(0.002, 0.003, 0.005),
        gi: 0.0004,
        torch: None,
        sky_lum: 0.0016,
        fog: [Vec3::new(0.001, 0.002, 0.003), Vec3::new(0.0111, 0.0233, 0.0271), Vec3::new(0.0111, 0.0233, 0.0271)],
    };
    // the orbit: 360 degrees at a 20 degree pitch (6 s), the pitch from 5 to 60 degrees (3 s), the distance 2.5 to 7 m (3 s)
    let orbit = |t: f32| -> (Vec3, Vec3) {
        let feet = Vec3::ZERO;
        let (az, pitch, dist) = if t < 6.0 {
            (180.0 + t / 6.0 * 360.0, 20.0f32, 350.0f32)
        } else if t < 9.0 {
            (180.0, 5.0 + (t - 6.0) / 3.0 * 55.0, 350.0)
        } else {
            (180.0, 20.0, 250.0 + (t - 9.0) / 3.0 * 450.0)
        };
        let (a, p) = (az.to_radians(), pitch.to_radians());
        (feet + Vec3::Z * 110.0 + Vec3::new(a.cos() * p.cos(), a.sin() * p.cos(), p.sin()) * dist, feet)
    };
    // the walk: standing in the sun (1 s), walking -X at 150 cm/s into the tree's shade (edge at x = -40), on to the torch
    let torch_at = Vec3::new(-420.0, 260.0, 90.0);
    let walk_world = World { torch: Some((torch_at, Vec3::new(1.0, 0.45, 0.15) * 0.25, 500.0)), ..noon };
    let walk = |t: f32| -> (Vec3, Vec3) {
        let feet = if t < 1.0 {
            Vec3::new(60.0, 0.0, 0.0)
        } else if t < 3.0 {
            Vec3::new(60.0 - (t - 1.0) * 150.0, 0.0, 0.0)
        } else if t < 5.0 {
            Vec3::new(-240.0, 0.0, 0.0)
        } else if t < 7.0 {
            Vec3::new(-240.0 - (t - 5.0) * 60.0, (t - 5.0) * 110.0, 0.0)
        } else {
            Vec3::new(-360.0, 220.0, 0.0)
        };
        // the camera behind him (third person), swaying 25 degrees either way
        let az = (180.0 + 25.0 * (t * 1.3).sin()).to_radians();
        (feet + Vec3::new(az.cos() * 330.0, az.sin() * 330.0, 120.0 + 110.0), feet)
    };
    let mut summary = String::from("IMPL_ER86 orbit test (examples/light_orbit.rs): his diffuse light per body region, scene-linear (before the game's exposure), luma.\nCV = standard deviation / mean; range = (max - min) / mean, over the whole segment.\n\n");
    let segs: [(&str, f32, f32); 3] = [("yaw orbit 360 deg", 0.5, 6.0), ("pitch 5..60 deg", 6.0, 9.0), ("distance 2.5..7 m", 9.0, 12.0)];
    for (scenario, world, hero_mode, drop_sun) in [("noon", &noon, 1u8, false), ("night", &night, 1u8, false), ("night_hero", &night, 2u8, false), ("noon_dropsun", &noon, 1u8, true)] {
        let mut runs = Vec::new();
        for mode in [Mode::Before, Mode::Steady] {
            let s = run(scenario, world, mode, 12.0, &orbit, drop_sun, hero_mode);
            write_csv(&format!("{out}_{scenario}_{}.csv", if mode == Mode::Before { "before" } else { "steady" }), &s);
            runs.push(s);
        }
        summary += &format!("== {scenario} ==\n");
        for (seg, a, b) in segs {
            summary += &format!("  -- {seg} ({a}..{b} s)\n  {:<12} {:>22} {:>22}\n", "region", "before CV / range %", "steady CV / range %");
            for (k, (name, ..)) in REGIONS.iter().enumerate() {
                let pick = |s: &Vec<Sample>| s.iter().filter(|x| x.t >= a && x.t < b).map(|x| luma(x.regions[k])).collect::<Vec<f32>>();
                let (b0, b1) = (stats(&pick(&runs[0])), stats(&pick(&runs[1])));
                summary += &format!("  {name:<12} {:>9.2} / {:>8.2}    {:>9.2} / {:>8.2}\n", b0.1, b0.2, b1.1, b1.2);
            }
            let input = |s: &Vec<Sample>, f: &dyn Fn(&Sample) -> f32| stats(&s.iter().filter(|x| x.t >= a && x.t < b).map(f).collect::<Vec<f32>>());
            for (label, f) in [("key shadow (mask.w)", &(|x: &Sample| x.mask_w) as &dyn Fn(&Sample) -> f32), ("ambient", &|x: &Sample| x.ambient), ("env level", &|x: &Sample| x.env), ("exposure used", &|x: &Sample| x.exposure), ("hero strength", &|x: &Sample| x.hero), ("white level (gate)", &|x: &Sample| x.level)] {
                let (p, q) = (input(&runs[0], f), input(&runs[1], f));
                summary += &format!("  input {label:<20} mean {:>8} -> {:>8}   CV {:>6.2} -> {:>6.2} %   range {:>7.2} -> {:>7.2} %\n", fmt(p.0), fmt(q.0), p.1, q.1, p.2, q.2);
            }
            let flips = |s: &Vec<Sample>| s.windows(2).filter(|w| w[0].t >= a && w[1].t < b && w[0].gate_dark != w[1].gate_dark).count();
            summary += &format!("  darkness gate switches: {} -> {}\n", flips(&runs[0]), flips(&runs[1]));
            // the worst frame-to-frame step of any region (pops)
            let pop = |s: &Vec<Sample>| s.windows(2).filter(|w| w[0].t >= a && w[1].t < b).flat_map(|w| (0..7).map(move |k| ((luma(w[1].regions[k]) - luma(w[0].regions[k])) / luma(w[0].regions[k]).max(1.0e-6)).abs() * 100.0)).fold(0.0f32, f32::max);
            summary += &format!("  worst frame-to-frame step of a region: {:.2} % -> {:.2} %\n", pop(&runs[0]), pop(&runs[1]));
        }
        summary += "\n";
        // plots: chest, back, head top; the inputs
        let line = |s: &Vec<Sample>, f: &dyn Fn(&Sample) -> f32| s.iter().map(|x| (x.t, f(x))).collect::<Vec<(f32, f32)>>();
        let marks = [(6.0, "pitch"), (9.0, "distance")];
        let mut doc = String::new();
        for (k, name) in [(2usize, "chest"), (3, "back"), (0, "head top")] {
            doc += &svg(&format!("{scenario}: his {name} (scene-linear luma) while the camera orbits"), "light", &[("before", "#eb6834", line(&runs[0], &|x| luma(x.regions[k]))), ("steady", "#2a78d6", line(&runs[1], &|x| luma(x.regions[k])))], &marks);
        }
        doc += &svg(&format!("{scenario}: key-light shadow from the probes (mask.w)"), "mask.w", &[("before", "#eb6834", line(&runs[0], &|x| x.mask_w)), ("steady", "#2a78d6", line(&runs[1], &|x| x.mask_w))], &marks);
        doc += &svg(&format!("{scenario}: reflection environment level"), "x", &[("before", "#eb6834", line(&runs[0], &|x| x.env)), ("steady", "#2a78d6", line(&runs[1], &|x| x.env))], &marks);
        doc += &svg(&format!("{scenario}: white level at his feet (display; gate on below 0.06, off above 0.10)"), "level", &[("before", "#eb6834", line(&runs[0], &|x| x.level)), ("steady", "#2a78d6", line(&runs[1], &|x| x.level))], &marks);
        doc += &svg(&format!("{scenario}: hero light strength (gate)"), "0..1", &[("before", "#eb6834", line(&runs[0], &|x| x.hero)), ("steady", "#2a78d6", line(&runs[1], &|x| x.hero))], &marks);
        std::fs::write(format!("{out}_{scenario}.html"), format!("<!doctype html><meta charset='utf-8'><title>ER86 {scenario}</title><body style='background:#fcfcfb;margin:16px'>{}</body>", doc.replace("\n<svg", "<br><svg"))).unwrap();
    }
    // the walk test (steady only, and before for comparison)
    let mut walks = Vec::new();
    for mode in [Mode::Before, Mode::Steady] {
        let s = run("walk", &walk_world, mode, 9.0, &walk, false, 1);
        write_csv(&format!("{out}_walk_{}.csv", if mode == Mode::Before { "before" } else { "steady" }), &s);
        walks.push(s);
    }
    summary += "== walk: sun (1 s) -> walking into the tree's shade (edge crossed at 1.67 s) -> standing in the shade -> walking to a torch (5..7 s) -> by the torch ==\n";
    for (m, s) in walks.iter().enumerate() {
        for (region, k) in [("chest (away from the sun)", 2usize), ("back (toward the sun)", 3)] {
            let at = |t: f32| s.iter().find(|x| x.t >= t).map_or(0.0, |x| luma(x.regions[k]));
            let (sun, shade) = (at(0.9), at(4.9));
            let cross = 1.0 + 100.0 / 150.0;
            let t_at = |frac: f32| s.iter().find(|x| x.t > 1.0 && (luma(x.regions[k]) - sun) / (shade - sun) >= frac).map_or(f32::NAN, |x| x.t - cross);
            let warm = |t: f32| s.iter().find(|x| x.t >= t).map_or(0.0, |x| x.regions[k].x / x.regions[k].z.max(1.0e-6));
            let still: Vec<f32> = s.iter().filter(|x| x.t >= 3.5 && x.t < 5.0).map(|x| luma(x.regions[k])).collect();
            summary += &format!("  {} {region}: sun {:.4} -> shade {:.4} ({:.0} %); 10 % of the change {:+.2} s, 90 % {:+.2} s from crossing the edge; standing in the shade, camera swaying: CV {:.2} %; by the torch {:.4}, warmth (red / blue) {:.2} -> {:.2}
", ["before", "steady"][m], sun, shade, shade / sun * 100.0, t_at(0.1), t_at(0.9), stats(&still).1, at(8.9), warm(4.9), warm(8.9));
        }
    }
    let line = |s: &Vec<Sample>, k: usize| s.iter().map(|x| (x.t, luma(x.regions[k]))).collect::<Vec<(f32, f32)>>();
    let marks = [(1.67, "shade edge"), (5.0, "to the torch"), (7.0, "at the torch")];
    let mut doc = svg("walk: his back, toward the sun (sun -> shade -> torch)", "light", &[("before", "#eb6834", line(&walks[0], 3)), ("steady", "#2a78d6", line(&walks[1], 3))], &marks);
    doc += &svg("walk: his chest, away from the sun", "light", &[("before", "#eb6834", line(&walks[0], 2)), ("steady", "#2a78d6", line(&walks[1], 2))], &marks);
    doc += &svg("walk: chest warmth (red / blue)", "ratio", &[("before", "#eb6834", walks[0].iter().map(|x| (x.t, x.regions[2].x / x.regions[2].z.max(1e-6))).collect()), ("steady", "#2a78d6", walks[1].iter().map(|x| (x.t, x.regions[2].x / x.regions[2].z.max(1e-6))).collect())], &marks);
    std::fs::write(format!("{out}_walk.html"), format!("<!doctype html><meta charset='utf-8'><title>ER86 walk</title><body style='background:#fcfcfb;margin:16px'>{}</body>", doc.replace("\n<svg", "<br><svg"))).unwrap();
    // light lines for the effects harness at orbit angles (noon and night; the frame as each path would light him, settled)
    for (scenario, world) in [("noon", &noon), ("night", &night)] {
        for mode in [Mode::Before, Mode::Steady] {
            for deg in [0, 45, 90, 135, 180, 225, 270, 315] {
                let fixed = move |_t: f32| -> (Vec3, Vec3) {
                    let (a, p) = ((deg as f32).to_radians(), 20f32.to_radians());
                    (Vec3::Z * 110.0 + Vec3::new(a.cos() * p.cos(), a.sin() * p.cos(), p.sin()) * 520.0, Vec3::ZERO)
                };
                let st = Settings { on: mode == Mode::Steady, ..Settings::default() };
                stable::set_settings(st);
                // one settled frame: probes as each path takes them
                let cam_pos = fixed(0.0).0;
                let cam = Cam::look(cam_pos, Vec3::Z * 110.0);
                let caps = capsules(Vec3::ZERO);
                let host = |g: Vec3| Vec3::new(g.y, g.z, g.x);
                let sky = sky_share(&cam);
                let ground = luma(light_at(world, Vec3::ZERO, Vec3::Z, 1.0) * GRASS);
                let mut l = GameLight { sun: Some((host(world.sun_dir), world.sun)), sky: (Vec3::Y, world.sky), shadow_color: Vec3::ONE, scene: true, exposure: (0.18 / (ground * (1.0 - sky) + world.sky_lum * sky).max(1.0e-5)).clamp(2.0, 30.0), pre_exposure: 1.0, fog: world.fog, ..GameLight::default() };
                if mode == Mode::Steady {
                    // (settled: every direction seen once, from a full turn of the camera)
                    let mut memory = stable::TapMemory::default();
                    for turn in 0..16 {
                        let a = (turn as f32 * 22.5).to_radians();
                        let c = Cam::look(Vec3::Z * 110.0 + Vec3::new(a.cos(), a.sin(), 0.36) * 500.0, Vec3::Z * 110.0);
                        let mut scratch = l;
                        memory.update(&probe_world(world, &c, Vec3::ZERO, &caps, st.probe_radius, &mut scratch), Vec3::ZERO, 0.1);
                    }
                    probe_world(world, &cam, Vec3::ZERO, &caps, st.probe_radius, &mut l);
                    l = memory.apply(&l);
                } else {
                    probe_grid(world, &cam, Vec3::ZERO, &caps, &mut l);
                }
                env_taps(world, &cam, Vec3::ZERO, &caps, &mut l);
                std::fs::write(format!("{out}_light_{scenario}_{}_{deg}.txt", if mode == Mode::Before { "before" } else { "steady" }), log_line(&l) + "\n").unwrap();
            }
        }
    }
    // the cut at six lights on him: eight effect lights circling him at different speeds (their ranks swap); his chest's light
    // per frame with the hard cut (before) and the soft one (steady)
    summary += "
== the cut at six lights on him: 8 effect lights circling at 1.5..2.6 m, ranks swapping, 6 s ==
";
    let mut cut_lines = Vec::new();
    for steady in [false, true] {
        stable::set_settings(Settings { on: steady, ..Settings::default() });
        let mut series = Vec::new();
        let mut reference: Vec<f32> = Vec::new();
        for f in 0..360 {
            let t = f as f32 / FPS;
            let lights: Vec<pointlight::PassLight> = (0..8)
                .map(|k| {
                    let r = 150.0 + 15.0 * k as f32 + 60.0 * (t * (0.3 + 0.05 * k as f32) + k as f32).sin();
                    let a = t * (0.25 + 0.06 * k as f32) + k as f32 * 0.8;
                    pointlight::PassLight { position: Vec3::new(a.cos() * r, a.sin() * r, 110.0), radius: 450.0, color: Vec3::new(0.02, 0.012, 0.004), exponent: 2.0, wrap: 0.0, on_sora: 0.5, sora: 1.0, sora_ambient: 0.1, placed: false, body_shadow: 0.0 }
                })
                .collect();
            let on = pointlight::sora_lights(&lights, Vec3::ZERO, Vec3::new(-400.0, 0.0, 200.0), 200.0, 120.0);
            let p = Vec3::new(15.0, 0.0, 115.0);
            // (the reference: every light, no cut)
            let all: f32 = lights.iter().map(|s| luma(s.color) * pointlight::radial((s.position - p).length_squared(), s.radius, s.exponent) * Vec3::X.dot((s.position - p).normalize_or_zero()).max(0.0)).sum();
            let c: f32 = on.iter().map(|s| luma(s.color) * pointlight::radial((s.position - p).length_squared(), s.radius, s.exponent) * Vec3::X.dot((s.position - p).normalize_or_zero()).max(0.0)).sum();
            series.push((t, c));
            reference.push(all);
        }
        let worst = series.windows(2).map(|w| ((w[1].1 - w[0].1) / w[0].1.max(1.0e-9)).abs() * 100.0).fold(0.0f32, f32::max);
        // the jump the cut adds: the step of the kept lights less the step of all of them, over all of them
        let jumps: Vec<f32> = (1..series.len()).map(|k| (((series[k].1 - series[k - 1].1) - (reference[k] - reference[k - 1])) / reference[k - 1].max(1.0e-9)).abs() * 100.0).collect();
        let big = jumps.iter().filter(|s| **s > 1.0).count();
        let worst_jump = jumps.iter().copied().fold(0.0f32, f32::max);
        summary += &format!("  {}: worst frame-to-frame step of his chest light {:.2} %; worst jump the cut itself adds {:.2} % of his light, jumps over 1 %: {}
", if steady { "steady (soft cut)" } else { "before (hard cut)" }, worst, worst_jump, big);
        cut_lines.push((if steady { "steady" } else { "before" }, if steady { "#2a78d6" } else { "#eb6834" }, series));
    }
    std::fs::write(format!("{out}_cut.html"), format!("<!doctype html><meta charset='utf-8'><title>ER86 cut</title><body style='background:#fcfcfb;margin:16px'>{}</body>", svg("the cut at six lights: his chest's light from 8 circling effect lights", "light", &cut_lines, &[]))).unwrap();
    std::fs::write(format!("{out}_summary.txt"), &summary).unwrap();
    print!("{summary}");
}
