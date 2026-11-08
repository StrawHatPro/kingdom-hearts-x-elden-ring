//! Elden Ring's own light, as read from its constant buffers and targets each frame, turned into
//! the rig of kh-look. Pure; unit-tested. (How the numbers are reached: gfx.rs, the game-light
//! pass; what was measured: build/sora_tmp/er/IMPL_ER3_STATE.md.)
//!
//! THE GAME-DATA TEXTURE (128 x 8 float texels, written on the GPU, read back a frame or two later):
//! rows 0..5 = `cbLight` (28 texels) of the candidate full-screen draws of the light passes, row 6
//! = `cbSceneParam` (126 texels), row 7 = the probes around his feet.
//!
//! Layouts are the game's own, by name, from its shaders' reflection data (exe 2.7.1.0), and the
//! formula is its directional light shader's (`GXLightAcc_DDirLightShadowed1`, read from its
//! disassembly): for a plain surface of colour c and normal n
//!
//! ```text
//! diffuse = c / pi * ( Col_0 * shadow * saturate(n . Vec_0) + Col_1 * saturate(n . Vec_1) )
//! shadow  = 1 - FC_ShadowColor + FC_ShadowColor * mask.a      (mask = the screen-space shadow mask)
//! ```
//!
//! (each term also times an occlusion weight from the mask's red / green and `FC_SsaoWeight`,
//! which is the ground's, not his: left out). `cbLight.FC_DirLight[0]`: texel 0 `Vec_0`, 1 `Vec_1`
//! (unit vectors TO the lights; Vec_1 is straight up: a sky light without a shadow), 4 `Col_0`, 5
//! `Col_1`. `cbSceneParam`: texel 1 `FC_ScreenSize`, 3 `SC_CameraPos`, 16..19 `FC_MatrixProj`,
//! 24..27 `VC_MatrixViewProj` (camera-relative), 28 `SC_PrevCameraPos`, 29..32
//! `VC_PrevMatrixViewProj`, 34 `FC_ShadowColor`, 72 (`SC_SceneExposure`, inverse,
//! `SC_PreExposure`, inverse).

use glam::{Vec3, Vec4};

pub const WIDTH: usize = 128;
pub const HEIGHT: usize = 8;
pub const CANDIDATES: usize = 6;
pub const ROW_SCENE: usize = 6;
pub const ROW_PROBE: usize = 7;
/// A surface colour darker than this is not divided by.
pub const DARK: f32 = 0.02;

/// One frame of the game's light.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GameLight {
    /// The directional light (sun / moon): unit vector TO it (host space) and `Col_0`; `None`:
    /// the frame had no directional light pass (or it was not recognised).
    pub sun: Option<(Vec3, Vec3)>,
    /// The second light of the set (`Vec_1`, `Col_1`): the sky light.
    pub sky: (Vec3, Vec3),
    pub shadow_color: Vec3,
    /// `cbSceneParam` was read (its screen size is the target's).
    pub scene: bool,
    pub exposure: f32,
    pub pre_exposure: f32,
    /// The projection's sub-pixel jitter in pixels (x right, y up).
    pub jitter: (f32, f32),
    /// Around his feet (means of nine taps of the game's targets, which hold the scene without
    /// him): the scene colour as composed, the direct diffuse light x surface colour (the direct
    /// light accumulation after the game's own multiplication by the surface colour), the surface
    /// colour, the screen-space shadow mask (alpha: 1 = lit by the directional light).
    pub scene_px: Vec3,
    pub direct_px: Vec3,
    pub albedo_px: Vec3,
    pub mask: Vec4,
    pub taps: f32,
    /// The scene colour along the top of the picture (nine taps: what is above and ahead of the
    /// camera, the sky outdoors) and the taps taken: the level of his reflection environment.
    pub env_px: Vec3,
    pub env_taps: f32,
    /// `cbSceneParam` rows 81..83: SC_VolumeFog ambient / toward-the-light / opposite colours
    /// (the game's own sky-side colours of the hour; IMPL_ER7_STATE.md, the Keyblade).
    pub fog: [Vec3; 3],
    /// (IMPL_ER64, probe2.ps.dxil) THE RING around his feet: per tap the game's direct light on a white surface there
    /// (w = 1 valid) and where the tap lies relative to his feet (cm; w = 1 valid). All zero without probe2.
    pub ring: [Vec4; RING],
    pub ring_at: [Vec4; RING],
    /// (VIEWLIGHT, the user: "lighting is flickery on the KH models") THE LIGHT ON HIS OWN BODY (probe4 texels 8..13: the game's
    /// light on the KH white at six points of his body) and how many points read it; `body_ratio`: the ground's received light over
    /// the body's, learnt while both read (stable.rs, eased; 0 = not learnt yet). `received` blends the two by the ground taps.
    pub body_px: Vec3,
    pub body_taps: f32,
    pub body_ratio: f32,
}

/// (IMPL_ER64) Taps of the ring (probe2.ps.dxil: texels 16.. and 40.. of the probe row).
pub const RING: usize = 24;

impl GameLight {
    /// The probes are usable: taps were taken on a surface with a colour.
    pub fn probed(&self) -> bool {
        // (VIEWLIGHT) by luminance: a deeply coloured surface (a red carpet, blue near 0) is readable (ground_light)
        self.taps >= 1.0 && luma(self.albedo_px) > DARK
    }
}

fn texel(words: &[f32], row: usize, x: usize) -> Vec4 {
    let at = (row * WIDTH + x) * 4;
    words.get(at..at + 4).map_or(Vec4::ZERO, |w| Vec4::new(w[0], w[1], w[2], w[3]))
}

fn colour(v: Vec4) -> Option<Vec3> {
    let c = v.truncate();
    (c.is_finite() && c.min_element() >= 0.0 && c.max_element() < 1.0e4).then_some(c)
}

/// `words`: the texture's floats, row after row. `size`: the scene target's size in pixels.
pub fn parse(words: &[f32], size: (f32, f32)) -> GameLight {
    let mut out = GameLight { shadow_color: Vec3::ONE, ..GameLight::default() };
    for row in 0..CANDIDATES {
        let d = texel(words, row, 0).truncate();
        // A `cbLight`: a unit vector and a finite, non-negative colour (the other full-screen
        // draws of the light passes have nothing at b5: zeros).
        if let (true, Some(c)) = ((d.length() - 1.0).abs() < 0.01, colour(texel(words, row, 4))) {
            out.sun = Some((d.normalize(), c));
            let up = texel(words, row, 1).truncate();
            if let (true, Some(c)) = ((up.length() - 1.0).abs() < 0.01, colour(texel(words, row, 5))) {
                out.sky = (up.normalize(), c);
            }
            break;
        }
    }
    let screen = texel(words, ROW_SCENE, 1);
    if (screen.x - size.0).abs() < 0.5 && (screen.y - size.1).abs() < 0.5 {
        out.scene = true;
        let e = texel(words, ROW_SCENE, 72);
        (out.exposure, out.pre_exposure) = (e.x, e.z);
        // FC_MatrixProj rows 0 and 1: clip.x = sx * x + jx * z, clip.y = sy * y + jy * z (w = z).
        out.jitter = (texel(words, ROW_SCENE, 16).z * 0.5 * size.0, texel(words, ROW_SCENE, 17).z * 0.5 * size.1);
        if let Some(c) = colour(texel(words, ROW_SCENE, 34)) {
            out.shadow_color = c.min(Vec3::ONE);
        }
        for k in 0..3 {
            out.fog[k] = colour(texel(words, ROW_SCENE, 81 + k)).unwrap_or(Vec3::ZERO);
        }
    }
    let scene = texel(words, ROW_PROBE, 3);
    (out.scene_px, out.taps) = (scene.truncate(), scene.w);
    out.direct_px = texel(words, ROW_PROBE, 1).truncate();
    out.mask = texel(words, ROW_PROBE, 2);
    out.albedo_px = texel(words, ROW_PROBE, 4).truncate();
    let env = texel(words, ROW_PROBE, 6);
    if let Some(c) = colour(env) {
        (out.env_px, out.env_taps) = (c, env.w);
    }
    // (VIEWLIGHT) the light on his own body (probe4 texel 9: the direct light on the KH white, w = the points read)
    let body = texel(words, ROW_PROBE, 9);
    if let Some(c) = colour(body).filter(|_| body.w >= 1.0) {
        (out.body_px, out.body_taps) = (c, body.w);
    }
    for k in 0..RING {
        let (light, at) = (texel(words, ROW_PROBE, 16 + k), texel(words, ROW_PROBE, 40 + k));
        if light.w > 0.5 && at.w > 0.5 && colour(light).is_some() && at.is_finite() {
            (out.ring[k], out.ring_at[k]) = (light, at);
        }
    }
    out
}

/// What the renderer gives kh-look for one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RigLight {
    /// Unit vector to the key light, HOST space.
    pub key_dir: Vec3,
    /// kh-look's `Rig::key` and the indirect term: its constant part and the part that grows
    /// with the normal's upward component (both as what a white surface shows).
    pub key: Vec3,
    pub ambient: Vec3,
    pub ambient_up: Vec3,
    /// What the shaded colour is multiplied with on its way into the game's scene target.
    pub out_scale: f32,
}

/// The white level kh-look's materials are graded for: what a white diffuse surface facing the
/// key light shows under the test level's rig (key / pi + ambient).
pub fn reference_white() -> f32 {
    kh_look::stand_in::KEY / std::f32::consts::PI + kh_look::stand_in::AMBIENT
}

pub fn luma_of(c: Vec3) -> f32 {
    luma(c)
}

fn luma(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// (VIEWLIGHT) The light that reached the ground: its light over its colour, channel by channel where the channel is readable
/// (above DARK); a channel the surface hardly reflects (the blue of a red carpet) takes the grey level of the readable ones
/// (their luminance ratio) instead of a division by almost nothing.
pub fn ground_light(lit: Vec3, albedo: Vec3) -> Vec3 {
    let grey = luma(lit) / luma(albedo).max(DARK);
    Vec3::new(
        if albedo.x > DARK { lit.x / albedo.x } else { grey },
        if albedo.y > DARK { lit.y / albedo.y } else { grey },
        if albedo.z > DARK { lit.z / albedo.z } else { grey },
    )
}

/// (VIEWLIGHT) How strongly the ground's taps are trusted against his body's light: 0 with none or one, 1 from six (smooth), so the
/// light the rig is made from never steps when taps move in and out of view or behind a body.
pub fn ground_weight(taps: f32) -> f32 {
    let t = ((taps - 1.0) / 5.0).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// (VIEWLIGHT) THE LIGHT THAT REACHES HIM, white surface, scene units: the ground's (its light over its colour) blended with the
/// light on his own body scaled to the ground's (`body_ratio`), by ground_weight. None when neither reads.
pub fn received(l: &GameLight) -> Option<Vec3> {
    let ground = l.probed().then(|| ground_light(if l.direct_px.max_element() > 0.0 { l.direct_px } else { l.scene_px }, l.albedo_px));
    let body = (l.body_taps >= 1.0 && l.body_px.is_finite()).then(|| l.body_px * if l.body_ratio > 0.0 { l.body_ratio } else { 1.0 });
    match (ground, body) {
        (Some(g), Some(b)) => Some(b.lerp(g, ground_weight(l.taps))),
        (Some(g), None) => Some(g),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// (VIEWLIGHT, the user: "characters get lighter when they pass by bright spots behind them, like it's bleeding through them")
/// THE BLEED FIX: the game's light read at his own pixels (the capture, the body points) carries what its screen-space terms gather
/// from the pixels around (a bright window or brazier behind him); the ground taps are checked in world space (their depth on the
/// ground at his feet) and do not. While both read, the body's light is held to the ground's by the ratio learnt over
/// stable::BODY_RATIO_TAU (slow): a quick rise of the body's light alone (the background) is taken back, a change of the place's
/// light (both move) is kept. The factor multiplies the game's light on him (the hybrid's lift row); 1 without both readings,
/// eased in by the ground taps, bounded to 0.67 .. 1.5.
pub fn bleed_fix(l: &GameLight) -> f32 {
    if !(l.probed() && l.body_taps >= 1.0 && l.body_ratio > 0.0) {
        return 1.0;
    }
    let g = luma(ground_light(if l.direct_px.max_element() > 0.0 { l.direct_px } else { l.scene_px }, l.albedo_px));
    let b = luma(l.body_px);
    if !(g > 1.0e-7 && b > 1.0e-7) {
        return 1.0;
    }
    let fix = (g / l.body_ratio / b).clamp(0.67, 1.5);
    1.0 + (fix - 1.0) * ground_weight(l.taps)
}

/// (VIEWLIGHT, the user: "lighting is flickery on the KH models") THE LEVEL ADAPTS AS AN EYE (as the game's own exposure): the rig's
/// out scale of each character (key: 0 Sora, 1.. the friends) eased in the log domain over LEVEL_TAU s, so no single frame (a
/// tap entering view, a probe source changing) steps it; a jump of more than 8 x (a load, a warp) is taken at once.
pub const LEVEL_TAU: f32 = 0.35;

pub fn ease_rig(key: usize, r: RigLight) -> RigLight {
    static LAST: std::sync::Mutex<[(f32, Option<std::time::Instant>); 8]> = std::sync::Mutex::new([(0.0, None); 8]);
    let Ok(mut last) = LAST.lock() else { return r };
    let Some(slot) = last.get_mut(key) else { return r };
    // (ER-NATIVE) the native level is the game's own exposure, not an estimate: never eased (and the easing starts afresh when
    // the earlier modes come back)
    if crate::hybrid::native_now() && native_k().is_some() {
        *slot = (0.0, None);
        return r;
    }
    let now = std::time::Instant::now();
    let dt = slot.1.map_or(0.0, |t| now.duration_since(t).as_secs_f32()).min(1.0);
    let eased = ease_level(slot.0, r.out_scale, dt);
    *slot = (eased, Some(now));
    with_out_scale(r, eased)
}

/// One step of the level's easing (pure, tested): `before` 0 = none yet.
pub fn ease_level(before: f32, now: f32, dt: f32) -> f32 {
    if !(before > 0.0 && before.is_finite()) || !(now > 0.0 && now.is_finite()) || (now / before).max(before / now) > 8.0 {
        return now;
    }
    let a = (1.0 - (-dt.max(0.0) / LEVEL_TAU).exp()).clamp(0.0, 1.0);
    (before.ln() + (now.ln() - before.ln()) * a).exp()
}

/// The rig handed over at another level: the same light in the scene (key, ambient and up scale with 1 / out scale).
pub fn with_out_scale(r: RigLight, out_scale: f32) -> RigLight {
    if !(out_scale > 0.0 && r.out_scale > 0.0) {
        return r;
    }
    let f = r.out_scale / out_scale;
    RigLight { key: r.key * f, ambient: r.ambient * f, ambient_up: r.ambient_up * f, out_scale, ..r }
}

fn ratio(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x / b.x.max(DARK), a.y / b.y.max(DARK), a.z / b.z.max(DARK))
}

/// What the ground at his feet shows of the directional light against what the light shader's
/// plain formula gives it: MEASURED (noon, sand and grass, 2.7.1.0) at 0.5 .. 0.6, not 1 (the
/// game's surfaces show less than colour x Col_0 x N.L / pi: a factor between its G-buffer colour
/// and what its composite uses that was not traced). His lights are scaled by it, so that a
/// surface of his of the ground's colour shows what the ground shows. Only a frame in which the
/// ground is in the open, lit by the directional light alone (its direct light has the light's
/// colour: no camp fire) gives a value.
pub const CALIBRATION_DEFAULT: f32 = 0.58;

pub fn calibration(l: &GameLight) -> Option<f32> {
    let (dir, colour) = l.sun?;
    if !l.probed() || l.mask.w < 0.9 || dir.y < 0.25 {
        return None;
    }
    let explained = (colour * dir.y + l.sky.1 * l.sky.0.y.max(0.0)) * std::f32::consts::FRAC_1_PI;
    let measured = ratio(l.direct_px, l.albedo_px);
    if luma(explained) < 1.0e-4 || measured.length() < 1.0e-5 || measured.normalize().dot(explained.normalize()) < 0.995 {
        return None;
    }
    let c = luma(measured) / luma(explained);
    (0.25..=1.5).contains(&c).then_some(c)
}

/// MEASURED (noon, the beach of the first camp, 2.7.1.0): the diffuse light of the ground in the
/// shade of a cliff against the ground in the sun beside it (0.0085 against 0.064 per unit of
/// surface colour).
pub const SHADE_SHARE: f32 = 0.13;

/// What the mean of the game's two fog in-scatter colours is scaled by to stand for its sky in
/// scene units (MEASURED at noon over Agheel Lake, IMPL_ER7_STATE.md: fog toward the light
/// (0.170, 0.282, 0.308), opposite (0.0065, 0.0086, 0.0098), where the last pass's level had been
/// judged against a sky of 0.0236: 0.0236 / 0.137).
pub const FOG_SKY: f32 = 0.17;

/// THE GAME'S SKY OF THE HOUR, from its own light constants (`cbSceneParam` SC_VolumeFog colours:
/// the colour its air has toward and away from the sun / moon). They follow the clock (noon blue
/// white 0.28, dusk orange 0.09, night blue 0.022), which the scene target's top row at our draw
/// point does not: the sky is drawn later, so that tap read the same clear grey (0.0138 - 0.0143)
/// at dusk and at night. Without the constants (the frame gave none): that tap, as before.
pub fn sky_estimate(l: &GameLight) -> Option<Vec3> {
    let fog = (l.fog[1] + l.fog[2]) * 0.5 * FOG_SKY;
    if l.scene && luma(fog) > 1.0e-5 {
        Some(fog)
    } else if l.env_taps >= 1.0 {
        Some(l.env_px)
    } else {
        None
    }
}

/// THE HUE OF THE REFLECTION ENVIRONMENT: gains on red and blue (green = 1) that turn the stand-in
/// cube's own blue-grey into the colour of what surrounds him now: 60 % the game's sky of the hour,
/// 40 % the light that reaches the ground at his feet (`ambient`: the rig's measured undirected
/// light, which is orange by a fire), each at its brightness in the scene.
pub fn env_tint(l: &GameLight, ambient: Vec3) -> Vec3 {
    use kh_look::stand_in::{ENV_GROUND, ENV_HORIZON, ENV_ZENITH};
    let Some(sky) = sky_estimate(l) else { return Vec3::ONE };
    let near = if luma(ambient) > 1.0e-6 && l.taps >= 1.0 { ambient / luma(ambient) * luma(l.scene_px) } else { Vec3::ZERO };
    let theirs = sky * 0.6 + near * 0.4;
    let ours = (ENV_HORIZON + ENV_ZENITH) * 0.5 * 0.6 + ENV_GROUND * 0.4;
    if theirs.y <= 1.0e-7 || ours.min_element() <= 1.0e-6 {
        return Vec3::ONE;
    }
    let gain = (theirs / theirs.y) / (ours / ours.y);
    Vec3::new(gain.x.clamp(0.4, 2.5), 1.0, gain.z.clamp(0.4, 2.5))
}

/// THE LEVEL OF THE REFLECTION ENVIRONMENT (IMPL_ER6_STATE.md, the Keyblade): the generated sky
/// cube of the stand-in rig is scaled so that it shows what the game's own picture shows: the
/// brightest of 18 taps along its top (the sky outdoors) and the ground at his feet (MEASURED every
/// frame, eased): metal then reflects
/// surroundings as bright as Elden Ring's, where before it reflected the test level's sky at the
/// level of his own light. `scale`: key `r.env` (1 = as measured). 1.0 when nothing was measured.
/// (VIEWLIGHT) the sky term of `env_level` against the white the rig hands him over at (his whole light, the sun as if
/// unshadowed included). MEASURED outdoors (the round logs of 2026-11-03, noon / dusk / RT OFF): 0.25 - 0.48, so in the open
/// nothing changes; indoors (Roundtable Hold, Stormveil's halls) the fog sky of the hour stood 2 - 20 x above it.
pub const SKY_PER_WHITE: f32 = 0.5;

pub fn env_level(l: &GameLight, out_scale: f32, scale: f32) -> f32 {
    use kh_look::stand_in::{ENV_GROUND, ENV_HORIZON, ENV_ZENITH};
    let Some(sky) = sky_estimate(l) else { return 1.0 };
    if out_scale <= 0.0 || scale <= 0.0 {
        return 1.0;
    }
    // (the cube has one level: it is set so that a 60 : 40 mix of its sky and its ground shows
    // the same mix of the game's sky (the brightest tap of the top of the picture) and the
    // game's ground at his feet)
    let ours = 0.6 * luma((ENV_HORIZON + ENV_ZENITH) * 0.5) + 0.4 * luma(ENV_GROUND);
    // (THE LEVEL is never below what the picture's top row gave the last pass: the user found the
    // Keyblade too dark, and at night the game's own sky colours are darker than that grey)
    let sky = luma(sky).max(if l.env_taps >= 1.0 { luma(l.env_px) } else { 0.0 });
    // (VIEWLIGHT, Roundtable Hold's east corridor: the level pinned at its x12 cap) INDOORS THE SKY IS NOT HIS SURROUNDINGS. The fog
    // constants are the outdoors' sky of the hour also under a roof; there it stood 8 x above the light the ground at his feet got,
    // and the cube (a blue sky, a green ground) shone off every glossy rim. The sky term is capped (`SKY_PER_WHITE`).
    // (VIEWLIGHT, ROUND-2: x9.8 - x10.2 in Roundtable Hold's corridor, RT MAX and RT OFF, with this cap at 2 x the light the ground
    // received, and x10.24 / x12.00 where nothing read) the cap at the ground's light was too loose: indoors the rig's white IS
    // that light (no sun), outdoors it is ~8 x (the open sun), so a sky at 2 x the ground was 1.2 x his white inside and ~0.2 x
    // outside. The cap is now against the rig's own white (out_scale x the reference white): always defined, so it ALWAYS holds,
    // measured or not, and it never toggles with a tap count.
    let got = received(l);
    let white = out_scale * reference_white();
    let sky = sky.min(SKY_PER_WHITE * white);
    // (VIEWLIGHT) when the probes read his own body (probe4's body fallback: the KH white), the ground's colour is not known: the
    // light at an Elden Ring ground's colour (0.12 - 0.17, measured at his feet) stands for it
    let ground = match got {
        // (the ground's scene colour where its taps read, the received light at an Elden Ring ground's colour (0.12 - 0.17,
        // measured at his feet) where only his body does, blended as `received` is)
        Some(g) => { let w = if l.probed() { ground_weight(l.taps) } else { 0.0 }; luma(g) * 0.15 * (1.0 - w) + luma(l.scene_px) * w }
        None => white * 0.15,
    };
    let theirs = 0.6 * sky + 0.4 * ground;
    (theirs / out_scale / ours * scale).clamp(0.25, 12.0)
}

/// The rig for the game's light `l`. `fallback_dir`: the key direction when the game has no
/// directional light (host space).
///
/// - KEY = the game's directional light (EXACT: its direction and colour), shadowed as the ground
///   at his feet is (the shadow mask there: he is in the shade when the ground under him is;
///   STAND-IN for a shadow term of his own pixels).
/// - The game's sky light (EXACT colour; a light from straight above without a shadow) as the
///   two-band form of a clamped cosine: a quarter everywhere and a half times the upward part
///   of the normal.
/// - AMBIENT (MEASURED at his feet, an estimate): the diffuse light of the ground per unit of
///   surface colour, less what the two lights above put there: the game's environment / GI light
///   and its local lights (a camp fire), as an undirected term; in the open at least
///   `SHADE_SHARE` of the ground's light.
/// - The KH3 materials (their post-process grading above all) are made for the light level of
///   KH3's own scenes, so the rig is handed over AT THAT LEVEL (scaled so that white is the
///   reference white) and the result is scaled back by `out_scale`: his brightness in the scene
///   target is linear in the game's light, and a white diffuse surface of his shows what a white
///   diffuse surface of the game shows beside him.
pub fn rig(l: &GameLight, fallback_dir: Vec3, key_gain: f32, ambient_gain: f32) -> RigLight {
    rig_for(l, fallback_dir, key_gain, ambient_gain, false)
}

/// (RSF2 round 4, FAULT 5: Goofy lime / neon in sunlit grass) `r.level_open` (default 1): while the hybrid draws (the game's light
/// per pixel, its shadows on him included), the light LEVEL a character is handed to KH3's look at is the level of the OPEN
/// scene (the sun as if unshadowed + the undirected light), not of the shadow at his feet. In grass the screen-space shadow
/// mask at his feet reads the sun hidden (the user's and ERT4's logs: mask.w 0, key 0 by day), the level jumped up to 20 x
/// (x60 -> x1225) and his SUNLIT body was handed over 20 x too bright: KH3's bands and colour table clipped it to neon lime.
/// The key itself still carries that shadow (the rig's light when the hybrid falls back).
static OPEN_LEVEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_open_level(on: bool) {
    OPEN_LEVEL.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The open-scene level is used this frame: `r.level_open` and the hybrid's shaders in use (`r.hybrid`).
pub fn open_level_now() -> bool {
    OPEN_LEVEL.load(std::sync::atomic::Ordering::Relaxed) && crate::hybrid::settings().on && crate::hybrid::SHADERS.load(std::sync::atomic::Ordering::Relaxed)
}

/// (VIEWLIGHT, 2026-11-03, Roundtable Hold's east corridor at RT MAX: the Moogle, Donald, Goofy and Sora's shoes NEON green-cyan,
/// with r.hybrid 0 too) THE LEVEL NEVER RUNS AWAY WHEN NOTHING IS MEASURED. In an interior at night the frame has no sun colour and
/// no sky (both 0), and when the probes at his feet find no surface (`probed false`: the corridor's floor under the party, the
/// feet hidden) nothing lights the rig: its white fell to the 1e-5 floor and the level climbed to x73 000 (the round's log, over
/// ~40 s of easing), so every KH colour was handed to KH3's look tens of thousands of times too bright and its colour table
/// clipped it to neon. The white is now at least `WHITE_FLOOR_EXPOSED / exposure`: what the game's own exposure says a white
/// surface there shows at the least. Measured white x exposure: noon 0.48, the corridor with the probes on its lit floor 0.11 - 0.18
/// (looks right), on its dark carpet 0.046 (Sora's yellow shoes and Donald's feet already lime at RT OFF), unmeasured 0.0001
/// (neon). The floor 0.15 keeps the hand-over level in the range that looks right; a sunlit or lit scene never reaches it.
pub const WHITE_FLOOR_EXPOSED: f32 = 0.15;

/// The least white (scene units) the rig is handed over at: [`WHITE_FLOOR_EXPOSED`] over the game's exposure (0: none known).
pub fn white_floor(l: &GameLight) -> f32 {
    if l.scene && l.exposure.is_finite() && l.exposure > 1.0e-3 {
        WHITE_FLOOR_EXPOSED / l.exposure
    } else {
        0.0
    }
}

/// (ER-NATIVE) THE UNITS, one fixed mapping for every KH object (the user, 2026-11-03: "no estimating"). Elden Ring's light
/// passes write linear HDR light (x SC_PreExposure) and its tone mapper takes the scene x SC_SceneExposure / SC_PreExposure; KH3's
/// materials are graded at their reference white ([`reference_white`]: what a white surface shows under the test level's rig, at
/// KH3's camera exposure 1). The two cameras meet where the KH effects already meet the game (vfx.rs `effect_scale`, IMPL_ER_FX):
/// KH3's reference white is the value ER's tone mapper gets as 1.0. So the game's light L (its buffers' units) is
/// `L x reference white x SC_SceneExposure / SC_PreExposure` in KH3's units, and whatever KH3's shading returns goes back by the
/// inverse: no per-frame level of ours, nothing measured at his feet, no easing; the only input is the game's own exposure
/// constant of the frame (the number its tone mapper multiplies with), as read back, held through frames without a reading.
static RAW_EXPOSURE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// (ER-NATIVE) the game's exposure of a frame as READ (never eased): SC_SceneExposure / SC_PreExposure; a frame without the
/// scene constants keeps the last one
pub fn note_exposure(l: Option<&GameLight>) {
    if let Some(e) = l.and_then(exposure_of) {
        RAW_EXPOSURE.store(e.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }
}

/// The exposure a frame's constants give ER's tone mapper (SC_SceneExposure over SC_PreExposure; pre-exposure 1 when absent).
pub fn exposure_of(l: &GameLight) -> Option<f32> {
    let pre = if l.pre_exposure.is_finite() && l.pre_exposure > 1.0e-6 { l.pre_exposure } else { 1.0 };
    let e = l.exposure / pre;
    (l.scene && e.is_finite() && e > 1.0e-3 && e < 1.0e6).then_some(e)
}

/// (ER-NATIVE) the game's exposure as last read (None before the first reading)
pub fn exposure_now() -> Option<f32> {
    let e = f32::from_bits(RAW_EXPOSURE.load(std::sync::atomic::Ordering::Relaxed));
    (e.is_finite() && e > 1.0e-3).then_some(e)
}

/// (ER-NATIVE) The factor from the game's light (its buffers' units) to KH3's: reference white x the exposure. Pure.
pub fn native_k_of(exposure: f32) -> f32 {
    reference_white() * exposure
}

/// (ER-NATIVE) this frame's factor (None: no exposure read yet), with the fixed anchor `r.hybrid_native_white` (1 = the
/// effects' rule)
pub fn native_k() -> Option<f32> {
    let white = crate::hybrid::settings().native_white;
    exposure_now().map(|e| native_k_of(e) / if white.is_finite() && white > 0.0 { white } else { 1.0 })
}

/// (ER-NATIVE, the status line) the units in use
pub fn native_units_text() -> String {
    match exposure_now() {
        Some(e) => {
            let k = native_k().unwrap_or_else(|| native_k_of(e));
            format!("KH3 = game x {k:.3} (reference white {:.3} x the game's exposure {e:.3} / r.hybrid_native_white {:.2}), out x {:.6}", reference_white(), crate::hybrid::settings().native_white, 1.0 / k)
        }
        None => "no exposure read yet (the rig's level stands in)".to_string(),
    }
}

/// (ER-NATIVE) THE RIG IN THE NATIVE MODE: only the out scale matters for a KH texel the capture lit (its light is the game's own
/// per texel, the rig's SH share 0); the rig itself stands for the few pixels without a captured texel (a translucent slot's
/// indirect term, a texel with no KH G-buffer near it) and is made from the game's own constants alone, exactly: the directional
/// light (Col_0, in the units its diffuse target shows: x CALIBRATION_DEFAULT, nothing shadowed at his feet) and the sky light as
/// the two-band clamped cosine; no probe, no ground, no floor. All in KH3's units at `k`.
pub fn rig_native(l: &GameLight, fallback_dir: Vec3, k: f32) -> RigLight {
    let (key_dir, sun) = match l.sun {
        Some((d, c)) => (d, c * CALIBRATION_DEFAULT),
        None => (fallback_dir, Vec3::ZERO),
    };
    let sky = l.sky.1 * CALIBRATION_DEFAULT * std::f32::consts::FRAC_1_PI;
    RigLight { key_dir, key: sun * k, ambient: sky * 0.25 * k, ambient_up: sky * 0.5 * k, out_scale: 1.0 / k }
}

/// The rig of a frame for any KH character (gfx.rs for Sora, gfx/party.rs for each member): the native rig while the native mode
/// draws and an exposure was read, else `rig_for` with the open level as before.
pub fn rig_now(l: &GameLight, fallback_dir: Vec3, key_gain: f32, ambient_gain: f32) -> RigLight {
    match native_k().filter(|_| crate::hybrid::native_now()) {
        Some(k) => rig_native(l, fallback_dir, k),
        None => rig_for(l, fallback_dir, key_gain, ambient_gain, open_level_now()),
    }
}

/// [`rig`] with the level chosen: `open_level` = the level of the sun unshadowed (see [`set_open_level`]).
pub fn rig_for(l: &GameLight, fallback_dir: Vec3, key_gain: f32, ambient_gain: f32, open_level: bool) -> RigLight {
    // (`key_gain` carries the calibration: see `calibration`)
    let inv_pi = std::f32::consts::FRAC_1_PI * key_gain;
    let key_gain = 1.0;
    let lit = Vec3::ONE - l.shadow_color + l.shadow_color * l.mask.w.clamp(0.0, 1.0);
    let (key_dir, sun) = match l.sun {
        Some((d, c)) => (d, c * lit * inv_pi),
        None => (fallback_dir, Vec3::ZERO),
    };
    // (RSF2) the sun as if nothing shadowed his feet: the open scene's level
    let sun_open = match l.sun {
        Some((_, c)) => c * inv_pi,
        None => Vec3::ZERO,
    };
    let sky = l.sky.1 * inv_pi;
    let (mut ambient, ambient_up) = (sky * 0.25, sky * 0.5);
    if let Some(total) = received(l) {
        // What the two lights put on level ground, and what the ground got from all direct lights.
        // (MEASURED: the target read as `direct_px` holds ALL the diffuse light of the ground times
        // its colour, the environment / GI light included: in the shade it is nearly the whole
        // scene colour. What the scene shows beyond it is the ground's reflection: not his light.)
        let explained = sun * key_dir.y.max(0.0) + sky * l.sky.0.y.max(0.0);
        // (where that target was not found the scene colour stands in: it adds the ground's reflection)
        // (VIEWLIGHT) `received`: the ground's light blended with his body's, smoothly by the ground taps
        // In the open the two lights explain (nearly) all of it, and how much exactly depends on
        // the ground's material, so the rest is not measurable there: it is at least the share
        // the ground in the shade keeps of the ground in the sun.
        ambient += (total - explained).max(total * SHADE_SHARE);
    }
    let (sun, ambient, ambient_up) = (sun * key_gain, ambient * ambient_gain, ambient_up * ambient_gain);
    let white = luma(if open_level { sun_open * key_gain } else { sun } + ambient + ambient_up * 0.5).max(1.0e-5).max(white_floor(l));
    let k = (reference_white() / white).min(1.0e5);
    RigLight { key_dir, key: sun * std::f32::consts::PI * k, ambient: ambient * k, ambient_up: ambient_up * k, out_scale: 1.0 / k }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (RSF2 round 4, FAULT 5) In sunlit grass the shadow mask at his feet reads the sun hidden and the ground dark: the old
    /// level jumps up (x490 in the harness line, x1225 in ERT4's game), the open level stays the open scene's; the key keeps
    /// the shadow either way.
    #[test]
    fn rsf2_the_open_level_does_not_follow_the_shadow_at_his_feet() {
        let open = GameLight { sun: Some((Vec3::new(0.49, 0.77, 0.41), Vec3::new(0.5, 0.48, 0.46))), scene: true, taps: 9.0, scene_px: Vec3::new(0.0226, 0.0205, 0.0188), direct_px: Vec3::new(0.0187, 0.0166, 0.0153), albedo_px: Vec3::new(0.133, 0.123, 0.118), mask: Vec4::new(0.93, 1.0, 1.0, 1.0), shadow_color: Vec3::ONE, ..GameLight::default() };
        let grass = GameLight { scene_px: Vec3::new(0.0012, 0.0014, 0.0006), direct_px: Vec3::new(0.0009, 0.0011, 0.0004), albedo_px: Vec3::new(0.14, 0.162, 0.064), mask: Vec4::new(0.84, 1.0, 1.0, 0.0), ..open };
        let k = CALIBRATION_DEFAULT;
        let (lit_old, grass_old) = (rig_for(&open, Vec3::Y, k, 1.0, false), rig_for(&grass, Vec3::Y, k, 1.0, false));
        let (lit_new, grass_new) = (rig_for(&open, Vec3::Y, k, 1.0, true), rig_for(&grass, Vec3::Y, k, 1.0, true));
        let jump = |a: &RigLight, b: &RigLight| a.out_scale / b.out_scale;
        assert!(jump(&lit_old, &grass_old) > 5.0, "the old level jumps in the grass: {}", jump(&lit_old, &grass_old));
        assert!(jump(&lit_new, &grass_new) < 2.0, "the open level stays (its undirected part still measured): {}", jump(&lit_new, &grass_new));
        assert!(grass_new.key.length() < 1.0e-6, "the key keeps the shadow at his feet (the hybrid lights him per pixel)");
    }

    fn data(sun_row: usize) -> Vec<f32> {
        let mut words = vec![0.0f32; WIDTH * HEIGHT * 4];
        let mut put = |row: usize, x: usize, v: [f32; 4]| words[(row * WIDTH + x) * 4..][..4].copy_from_slice(&v);
        put(sun_row, 0, [0.596287, 0.747020, 0.293944, 0.25]);
        put(sun_row, 1, [0.0, 1.0, 0.0, 0.2]);
        put(sun_row, 4, [0.133333, 0.184545, 0.3, 0.09]);
        put(sun_row, 5, [0.03, 0.04, 0.06, 0.0]);
        put(ROW_SCENE, 1, [2560.0, 1440.0, 1.0 / 2560.0, 1.0 / 1440.0]);
        put(ROW_SCENE, 16, [1.263396, 0.0, 0.0, 0.0]);
        put(ROW_SCENE, 17, [0.0, 2.246037, 0.000231, 0.0]);
        put(ROW_SCENE, 34, [1.0, 1.0, 1.0, 0.005]);
        put(ROW_SCENE, 72, [18.894775, 0.052925, 1.0, 1.0]);
        // The ground: colour (0.3, 0.25, 0.15); it shows 0.05 of direct light and 0.01 of other light per unit of colour.
        put(ROW_PROBE, 1, [0.3 * 0.05, 0.25 * 0.05, 0.15 * 0.05, 9.0]);
        put(ROW_PROBE, 2, [0.9, 1.0, 1.0, 1.0]);
        put(ROW_PROBE, 3, [0.3 * 0.06, 0.25 * 0.06, 0.15 * 0.06, 9.0]);
        put(ROW_PROBE, 4, [0.3, 0.25, 0.15, 9.0]);
        words
    }

    #[test]
    fn the_reflection_environment_takes_the_games_sky_and_the_fires_hue() {
        let mut l = GameLight { scene: true, taps: 9.0, scene_px: Vec3::splat(0.004), env_px: Vec3::splat(0.0138), env_taps: 1.0, ..GameLight::default() };
        // Night: the fog colours, not the grey tap.
        l.fog = [Vec3::ZERO, Vec3::new(0.0111, 0.0233, 0.0271), Vec3::new(0.0111, 0.0233, 0.0271)];
        let night = sky_estimate(&l).unwrap();
        assert!(luma(night) < 0.005 && night.z > night.x * 2.0, "{night:?}");
        // By a fire the cube turns warm (more red, less blue than under a white light); without constants nothing changes.
        let (cold, warm) = (env_tint(&l, Vec3::ONE), env_tint(&l, Vec3::new(1.0, 0.45, 0.2)));
        assert!(warm.x > cold.x * 1.2 && warm.z < cold.z, "{cold:?} {warm:?}");
        assert_eq!(env_tint(&GameLight::default(), Vec3::ONE), Vec3::ONE);
    }

    #[test]
    fn the_reflection_environment_follows_the_picture() {
        let mut l = GameLight::default();
        // Nothing measured: the old level.
        assert_eq!(env_level(&l, 0.02, 1.0), 1.0);
        // A sky of 0.02 over ground of 0.006 with a rig 60 times the scene: about two and a half times the old level.
        (l.env_px, l.env_taps, l.scene_px, l.taps) = (Vec3::splat(0.02), 1.0, Vec3::splat(0.006), 9.0);
        let level = env_level(&l, 1.0 / 60.0, 1.0);
        assert!((2.0..3.0).contains(&level), "{level}");
        // r.env = 0: the old behaviour; a tap into the sun is held to SKY_PER_WHITE x the rig's white (VIEWLIGHT), far below the 12 x clamp it used to reach
        assert_eq!(env_level(&l, 1.0 / 60.0, 0.0), 1.0);
        l.env_px = Vec3::splat(50.0);
        let sun = env_level(&l, 1.0 / 60.0, 1.0);
        assert!(sun > 0.25 && sun < 12.0, "{sun}");
    }

    #[test]
    fn viewlight_a_red_carpet_is_measured_and_the_sky_stays_outside() {
        // (the corridor's carpet: blue near 0; the outdoors' fog sky under the roof)
        let l = GameLight { scene: true, exposure: 56.0, fog: [Vec3::ZERO, Vec3::new(1.0, 0.84, 0.31), Vec3::new(0.06, 0.08, 0.09)], scene_px: Vec3::new(0.003, 0.0006, 0.0001), direct_px: Vec3::new(0.003, 0.0006, 0.0001), albedo_px: Vec3::new(0.3, 0.06, 0.01), mask: glam::Vec4::ONE, taps: 9.0, ..Default::default() };
        assert!(l.probed());
        let g = ground_light(l.direct_px, l.albedo_px);
        assert!(g.is_finite() && (g.x - 0.01).abs() < 1.0e-6 && g.z > 0.0 && g.z < 0.02, "{g:?}");
        let r = rig_for(&l, Vec3::Y, CALIBRATION_DEFAULT, 1.0, true);
        let open = GameLight { albedo_px: Vec3::ZERO, ..l };
        assert!(env_level(&l, r.out_scale, 1.0) < env_level(&open, r.out_scale, 1.0));
        assert!(env_level(&l, r.out_scale, 1.0) < 12.0);
    }

    #[test]
    fn viewlight_the_level_never_runs_away_unmeasured() {
        // (the Roundtable corridor at night: a sun of colour 0, no sky, the probes at his feet found no surface, exposure 56)
        let l = GameLight { sun: Some((Vec3::Y, Vec3::ZERO)), scene: true, exposure: 56.0, ..Default::default() };
        let r = rig_for(&l, Vec3::Y, CALIBRATION_DEFAULT, 1.0, true);
        let level = 1.0 / r.out_scale;
        assert!(level < 0.1 * 1.0e5, "{level}");
        assert!((r.out_scale * reference_white() - WHITE_FLOOR_EXPOSED / 56.0).abs() < 1.0e-6);
        // a measured scene (the same corridor with the probes: white x exposure 0.18) is untouched
        let probed = GameLight { scene_px: Vec3::splat(0.0034), direct_px: Vec3::splat(0.0034), albedo_px: Vec3::splat(0.3), mask: glam::Vec4::ONE, taps: 9.0, ..l };
        let measured = rig_for(&probed, Vec3::Y, CALIBRATION_DEFAULT, 1.0, true);
        let unfloored = rig_for(&GameLight { exposure: 0.0, ..probed }, Vec3::Y, CALIBRATION_DEFAULT, 1.0, true);
        assert_eq!(measured.out_scale, unfloored.out_scale);
    }

    #[test]
    fn viewlight_the_sky_cap_holds_with_nothing_measured() {
        // (ROUND-2's corridor log: probed false, no body light, the outdoors' fog sky under the roof: x10.24 .. x12.00 before)
        let l = GameLight { sun: Some((Vec3::Y, Vec3::ZERO)), scene: true, exposure: 56.0, fog: [Vec3::ZERO, Vec3::new(1.0, 0.84, 0.31), Vec3::new(0.06, 0.08, 0.09)], ..Default::default() };
        assert!(received(&l).is_none());
        let r = rig_for(&l, Vec3::Y, CALIBRATION_DEFAULT, 1.0, true);
        let level = env_level(&l, r.out_scale, 1.0);
        // the sky at most SKY_PER_WHITE x the rig's white, the ground 0.15 x: (0.6 x 0.5 + 0.4 x 0.15) x the white over the cube's mix
        use kh_look::stand_in::{ENV_GROUND, ENV_HORIZON, ENV_ZENITH};
        let ours = 0.6 * luma((ENV_HORIZON + ENV_ZENITH) * 0.5) + 0.4 * luma(ENV_GROUND);
        let most = (0.6 * SKY_PER_WHITE + 0.4 * 0.15) * reference_white() / ours;
        assert!(level <= most + 1.0e-4 && level < 12.0, "{level} against {most}");
        // (this round's log, Roundtable Hold by the fireplace, the probes reading: x10.12 with the cap at 2 x the ground's light)
        let fire = GameLight { scene_px: Vec3::new(0.0011, 0.0005, 0.0002), direct_px: Vec3::new(0.0008, 0.0004, 0.0001), albedo_px: Vec3::new(0.1001, 0.0794, 0.051), mask: glam::Vec4::new(0.9, 1.0, 1.0, 0.0), taps: 13.0, ..l };
        let r = rig_for(&fire, Vec3::Y, CALIBRATION_DEFAULT, 1.0, true);
        let inside = env_level(&fire, r.out_scale, 1.0);
        assert!(inside < 4.0, "indoors x{inside}");
        // (outdoors, r4_b's line: sunlit, the sky 0.25 x his white) untouched: the same level with the cap off
        let open = GameLight { sun: Some((Vec3::new(-0.537, 0.748, -0.39), Vec3::new(0.5031, 0.4729, 0.451))), shadow_color: Vec3::ONE, scene: true, exposure: 25.0, fog: [Vec3::ZERO, Vec3::new(0.2052, 0.169, 0.1102), Vec3::new(0.1102, 0.1597, 0.1597)], scene_px: Vec3::new(0.0021, 0.0022, 0.0013), direct_px: Vec3::new(0.0017, 0.0017, 0.0008), albedo_px: Vec3::new(0.124, 0.1338, 0.06), mask: glam::Vec4::new(0.82, 1.0, 1.0, 0.0), taps: 14.0, ..Default::default() };
        let r = rig_for(&open, Vec3::Y, CALIBRATION_DEFAULT, 1.0, true);
        let sky = luma(sky_estimate(&open).unwrap());
        assert!(sky < SKY_PER_WHITE * r.out_scale * reference_white(), "outdoors the cap does not bite: sky {sky}");
    }
    #[test]
    fn the_game_data_is_read_by_the_games_layout() {
        // (the numbers of the capture at night: IMPL_ER3_STATE.md)
        let l = parse(&data(3), (2560.0, 1440.0));
        let (dir, colour) = l.sun.unwrap();
        assert!((dir - Vec3::new(0.596287, 0.747020, 0.293944)).length() < 1.0e-4 && (colour.z - 0.3).abs() < 1.0e-6);
        assert!(l.sky.0 == Vec3::Y && (l.sky.1.y - 0.04).abs() < 1.0e-6 && l.shadow_color == Vec3::ONE);
        assert!(l.scene && (l.exposure - 18.894775).abs() < 1.0e-4 && l.pre_exposure == 1.0);
        assert!((l.jitter.1 - 0.000231 * 720.0).abs() < 1.0e-4 && l.jitter.0 == 0.0);
        assert!(l.probed() && l.mask.w == 1.0 && (l.albedo_px.x - 0.3).abs() < 1.0e-6);
        // Another target size: the scene row is not believed; no light row: no sun.
        assert!(!parse(&data(3), (1920.0, 1080.0)).scene);
        let mut none = data(0);
        none[..4].copy_from_slice(&[0.0; 4]);
        assert!(parse(&none, (2560.0, 1440.0)).sun.is_none());
    }

    #[test]
    fn the_rig_keeps_the_games_brightness() {
        let pi = std::f32::consts::PI;
        let l = parse(&data(0), (2560.0, 1440.0));
        let r = rig(&l, Vec3::Y, 1.0, 1.0);
        let (dir, colour) = l.sun.unwrap();
        // The key is the game's light by its own formula: a white surface facing it shows Col_0 / pi.
        assert!((r.key / pi * r.out_scale - colour / pi).length() < 1.0e-6);
        // Level ground: the diffuse light the two lights do not explain, and the sky light's
        // two-band cosine (a quarter + a half).
        let explained = colour / pi * dir.y + l.sky.1 / pi;
        let expected = (Vec3::splat(0.05) - explained).max(Vec3::splat(0.05 * SHADE_SHARE)) + l.sky.1 / pi * 0.75;
        assert!(((r.ambient + r.ambient_up) * r.out_scale - expected).length() < 1.0e-5);
        // The rig itself sits at the reference white (the level the materials are graded for).
        assert!((luma(r.key / pi + r.ambient + r.ambient_up * 0.5) - reference_white()).abs() < 1.0e-3);
        // The ground in the shade (mask alpha 0): no key.
        let mut shade = l;
        shade.mask.w = 0.0;
        assert!(rig(&shade, Vec3::Y, 1.0, 1.0).key == Vec3::ZERO);
        // No probe (his feet on a black surface): the two lights alone; no light at all: nothing infinite.
        let mut blind = l;
        blind.albedo_px = Vec3::ZERO;
        let r = rig(&blind, Vec3::Y, 1.0, 1.0);
        assert!((r.ambient * r.out_scale - l.sky.1 / pi * 0.25).length() < 1.0e-6);
        // The calibration: the ground lit by the sun alone, showing 60 % of the plain formula.
        let mut open = l;
        open.sky.1 = Vec3::ZERO;
        open.direct_px = open.albedo_px * (colour / pi * dir.y) * 0.6;
        assert!((calibration(&open).unwrap() - 0.6).abs() < 1.0e-4);
        // Fire light on the ground (another colour), or the ground in the shade: no value.
        open.direct_px = open.albedo_px * Vec3::new(0.2, 0.08, 0.02);
        assert!(calibration(&open).is_none() && calibration(&shade).is_none());
        let dark = rig(&GameLight::default(), Vec3::Y, 1.0, 1.0);
        assert!(dark.key.is_finite() && dark.ambient.is_finite() && dark.out_scale < 1.0e-3);
    }

    /// (ER-NATIVE) THE UNITS: KH3's reference white is where the game's tone mapper gets 1.0 (the effects' rule, vfx.rs
    /// effect_scale): k = reference white x SC_SceneExposure / SC_PreExposure; the out scale is its inverse, exactly the effects'
    /// scale; nothing measured at his feet changes it (no probe, no ground, no body, no floor, no easing)
    #[test]
    fn native_units_are_the_games_exposure_alone() {
        let e = 18.894_775;
        let k = native_k_of(e);
        assert!((k - reference_white() * e).abs() < 1.0e-4);
        assert!((1.0 / k - crate::vfx::effect_scale(Some(e), 1.0)).abs() < 1.0e-9, "one mapping for the characters and the effects");
        let l = GameLight { scene: true, exposure: e, pre_exposure: 1.0, sun: Some((Vec3::new(0.3, 0.9, 0.1).normalize(), Vec3::new(0.2, 0.18, 0.15))), sky: (Vec3::Y, Vec3::new(0.02, 0.03, 0.05)), ..GameLight::default() };
        assert_eq!(exposure_of(&l), Some(e));
        assert_eq!(exposure_of(&GameLight { pre_exposure: 2.0, ..l }), Some(e / 2.0), "the light passes write x SC_PreExposure");
        assert_eq!(exposure_of(&GameLight { scene: false, ..l }), None);
        let a = rig_native(&l, Vec3::Y, k);
        assert!((a.out_scale * k - 1.0).abs() < 1.0e-6);
        // the probes, the ground, the body, the shadow at his feet: none of them moves the native rig
        let probed = GameLight { taps: 9.0, albedo_px: Vec3::splat(0.3), direct_px: Vec3::splat(0.5), scene_px: Vec3::splat(0.7), mask: glam::Vec4::new(1.0, 1.0, 1.0, 0.0), body_px: Vec3::splat(4.0), body_taps: 6.0, body_ratio: 0.3, ..l };
        assert_eq!(rig_native(&probed, Vec3::Y, k), a);
        // the game's directional light, exactly: a white surface facing it shows its diffuse target's value x k
        assert!((a.key / std::f32::consts::PI - l.sun.unwrap().1 * CALIBRATION_DEFAULT / std::f32::consts::PI * k).length() < 1.0e-5);
        assert_eq!(native_k_of(e * 2.0) / native_k_of(e), 2.0);
    }
}
