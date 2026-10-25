//! (IMPL_ER39) LIGHT FROM SORA'S EFFECTS AND A SLIGHT HERO LIGHT IN THE DARK
//! (build/sora_tmp/er/IMPL_ER39_STATE.md).
//!
//! The user: "make fire actually emit light, and add a slight hero light around Sora when he's in
//! pitch black so things are still slightly visible around him".
//!
//! WHAT LIGHTS. (1) KH3's own particle lights: `ParticleModuleLight` of the effects (Fire / Fira /
//! Firaga, their bursts, fire finishers, any KH3 effect that carries one) arrive in the effects'
//! draw list (`kh_vfx::DrawList::lights`: game space cm, colour x brightness, radius cm); the
//! data says `bUseInverseSquaredFalloff` false with `LightExponent` 2 - 3, so the falloff is the
//! engine's exponent form `pow(1 - saturate(d^2 / r^2), exponent)` (as kh-look's rig evaluates
//! it) and the light on a surface is `colour x N.L x falloff x albedo / pi` (UE 4.17's simple
//! deferred light). (2) THE HERO LIGHT: a soft neutral-cool point a little above Sora, switched
//! by a DARKNESS GATE on the game's own light at his feet (the probes: scene colour / surface
//! colour x the game's exposure = what a white surface there SHOWS), with hysteresis and a fade.
//!
//! HOW. One additive full-screen pass of ours (one triangle, scissored to the lights' screen
//! boxes) right after the game's deferred stage, BEFORE Sora is drawn and AFTER the light probes
//! read the scene (so the probes, his own light rig and the darkness gate never see our light: no
//! feedback): for each pixel the position is rebuilt from the game's depth, the normal from the
//! depth's neighbours (no G-buffer normal is decoded), the surface colour is the game's G-buffer
//! colour target (the one the probes read), the lights are summed and occluded by a short
//! screen-space march through the depth buffer. The game's forward stage (fog, water, its
//! particles), TAA, bloom, exposure and tone mapping follow as for everything in the scene.
//!
//! UNITS. Effect lights: KH3 scene-linear x the effects' own scale (`vfx::effect_scale`: KH3's
//! reference white shows as 1 at the game's exposure), so a lit surface keeps its KH3 relation
//! to the flame that lights it. Hero light: given as what a WHITE surface facing it SHOWS (display
//! linear) at full strength, divided by the game's exposure.
//!
//! SORA HIMSELF: his materials get none of this pass (he is drawn after it).
//! (IMPL_ER44, the user: "also sora should be lit by his own hero light") WITH THE PACK'S `*_pl`
//! PIXEL SHADERS his materials take the lights as REAL POINT LIGHTS, in addition to the game's
//! light he gets anyway: up to `MAX_ON_SORA` of them in the frame constants (`sora_block`), each
//! through the material's own light shader (kh-look `kh_light`: Lambert + GGX + the subsurface
//! transmission) into the light buffer the KHS post-process material composes, with the pass's
//! falloff and units (what a white surface facing the light shows = what the ground beside him
//! gets x `hero_sora` / `fx_sora`). The hero light is PLACED for him above and toward the camera
//! (`hero_sora_height`, `hero_sora_toward`; a light 1.5 m above his feet sits inside his head),
//! with its colour scaled so that his chest gets what the real light gives there. A small share
//! of ambient remains (`hero_sora_ambient`, `fx_sora_ambient`) for the side turned away.
//! WITHOUT those shaders (an older pack) his rig's ambient (non-directional) is lifted by a share
//! of what the lights give a white surface at his chest (`fx_on_sora`, `hero_on_sora`), as before.
#![allow(clippy::too_many_arguments)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use glam::{Mat4, Vec3, Vec4};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

/// Lights the pass takes at most (the shader's arrays).
pub const MAX_LIGHTS: usize = 16;

/// The host's switches and numbers (kh-er: control keys `fx.light`, `r.hero_light` and the rows of
/// sheets/port/sora-er-fx.json).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightSettings {
    /// `fx.light`: the effects' particle lights light the world.
    pub fx: bool,
    /// `fx.light_gain`: a factor on them.
    pub fx_gain: f32,
    /// `fx.light_max`: the brightest this many are taken (the rest are dropped).
    pub fx_max: usize,
    /// `fx.light_on_sora`: the share of their light at his chest that lifts his ambient.
    pub fx_on_sora: f32,
    /// `r.hero_light`: 0 off, 1 automatic (the darkness gate), 2 always on (testing aid).
    pub hero: u8,
    /// The gate: dark when a white surface at his feet shows less than `hero_dark_on`, light again
    /// above `hero_dark_off` (display linear, the game's exposure applied).
    pub hero_dark_on: f32,
    pub hero_dark_off: f32,
    /// Seconds the hero light takes to fade fully in or out.
    pub hero_fade: f32,
    /// Radius (cm) and height of its centre above his feet (cm).
    pub hero_radius: f32,
    pub hero_height: f32,
    /// What a white surface facing it at its centre shows at full strength (display linear).
    pub hero_intensity: f32,
    pub hero_color: Vec3,
    /// The falloff's exponent and the wrap of its N.L (0 = plain Lambert; 0.5 = softer).
    pub hero_exponent: f32,
    pub hero_wrap: f32,
    /// The share of its light at his chest that lifts his ambient (packs without the `*_pl` shaders).
    pub hero_on_sora: f32,
    /// Screen-space occlusion steps per light (0 = none).
    pub occlusion_steps: u32,
    /// (IMPL_ER44) `hero.sora_light`: the hero light on his materials as a point light, relative to what the
    /// ground beside him gets (0 = none); `hero.sora_ambient`: the ambient share that remains with it.
    pub hero_sora: f32,
    pub hero_sora_ambient: f32,
    /// `hero.sora_height_cm`, `hero.sora_toward_cm`: where the hero light sits FOR HIM: this high above his feet
    /// and this far from him toward the camera (level).
    pub hero_sora_height: f32,
    pub hero_sora_toward: f32,
    /// `fx.sora_light`, `fx.sora_ambient`: the same for the effects' lights (from where they are).
    pub fx_sora: f32,
    pub fx_sora_ambient: f32,
    /// (IMPL_ER44 part 2, the user: "make Sora pull out the keyblade and have the keyblade sparkle. It is this effect that
    /// creates the light for the hero light and it casts shadow and illuminates Sora") `hero.keyblade`: the hero light
    /// comes from the Keyblade (Sora draws it while the gate is closed; the light sits on the blade, `hero_blade` cm from
    /// the grip along it, and fades with the weapon's opacity); false = the plain light above him (IMPL_ER39).
    pub hero_keyblade: bool,
    pub hero_blade: f32,
    /// `hero.shadow`: how dark his body's shadow from the Keyblade light is in the pass (0 = none, 1 = black).
    pub hero_shadow: f32,
    /// `hero.sparkle_s`: the Keyblade's sparkle effect is started again every this many seconds.
    pub hero_sparkle: f32,
    /// (IMPL_ER53, the user: "the glow on the keyblade pulses instead of being constant while in the dark, it must be
    /// constant") `hero.glow`: the Keyblade's glow is STEADY: KH3's appear effect's blade-glow emitters held at
    /// `hero.glow_hold_s` for as long as the light is on (`GLOW_EFFECT`), only its small sparkles restarted at
    /// `hero.sparkle_s` (`SPARK_EFFECT`; neither carries the effect's own particle light: the hero light is the light).
    /// false = IMPL_ER44's whole appear effect restarted at the interval.
    pub hero_glow: bool,
    pub hero_glow_hold: f32,
    /// (IMPL_ER56, the user: "I want ambient effects to light Sora") `fx.ambient_sora`: the share of their light on Sora for
    /// the AMBIENT effects (the save point, the prize orbs: `kh_vfx::Light::characters` false), with their own exponent; the
    /// save point's light is ~7x the hero light and green: at 1 it turned him green, this keeps it a soft tint.
    pub fx_ambient_sora: f32,
    /// (IMPL_ER64, the user: "Sora needs to be lit directionally by items in Elden Ring's world too such as torches")
    /// `r.world_lights` / `world.light`: THE WORLD'S LIGHTS ON SORA. The game's own direct light on the ground around his
    /// feet (the ring of probe2.ps.dxil: 24 taps of its light accumulation) is fitted with ONE point light where the
    /// ground is brightest (`world_light`); it lights Sora (his *_pl shaders) and the Keyblade, not the world (the game
    /// lit that). Off where a white surface at his feet shows more than `world.max_level` (daylight: the sun is his
    /// key light already); `world.gain` its strength; `world.min_contrast` the least (brightest - darkest) / brightest
    /// of the ring for a direction to count (a ring lit evenly has none).
    /// (IMPL_ER64) `hero.sora_placed`: on Sora the Keyblade light comes from above him toward the camera (the placed
    /// light of IMPL_ER44 part 1), not from the blade at his hip (which lit him from below: a horror light).
    pub hero_sora_placed: bool,
    pub world: bool,
    pub world_max_level: f32,
    pub world_gain: f32,
    pub world_min_contrast: f32,
}

impl Default for LightSettings {
    fn default() -> LightSettings {
        LightSettings {
            fx: true,
            fx_gain: 1.0,
            fx_max: 12,
            fx_on_sora: 0.5,
            hero: 1,
            hero_dark_on: 0.06,
            hero_dark_off: 0.10,
            hero_fade: 1.0,
            hero_radius: 750.0,
            hero_height: 150.0,
            hero_intensity: 0.8,
            hero_color: Vec3::new(1.0, 0.93, 0.82),
            hero_exponent: 2.0,
            hero_wrap: 0.5,
            hero_on_sora: 0.35,
            occlusion_steps: 6,
            hero_sora: 1.0,
            hero_sora_ambient: 0.3,
            hero_sora_height: 200.0,
            hero_sora_toward: 120.0,
            fx_sora: 1.0,
            fx_sora_ambient: 0.1,
            hero_keyblade: true,
            hero_blade: 55.0,
            hero_shadow: 0.85,
            hero_sparkle: 2.5,
            hero_glow: true,
            hero_glow_hold: 0.25,
            fx_ambient_sora: 0.15,
            hero_sora_placed: true,
            world: false,
            world_max_level: 0.5,
            world_gain: 1.0,
            world_min_contrast: 0.35,
        }
    }
}

static SETTINGS: Mutex<Option<LightSettings>> = Mutex::new(None);
/// The effects' particle lights of the latest draw list (game space) and when they came.
static FX_LIGHTS: Mutex<Option<(Vec<kh_vfx::Light>, Instant)>> = Mutex::new(None);
/// The darkness gate and when it was stepped last.
static GATE: Mutex<Option<(DarkGate, Instant)>> = Mutex::new(None);

pub fn set_settings(settings: LightSettings) {
    if let Ok(mut guard) = SETTINGS.lock() {
        if guard.as_ref() != Some(&settings) {
            crate::rlog!("render: light settings {settings:?}");
        }
        *guard = Some(settings);
    }
}

pub fn settings() -> LightSettings {
    SETTINGS.lock().ok().and_then(|g| *g).unwrap_or_default()
}

/// (vfx.rs `submit`) The particle lights of the frame's draw list.
pub(crate) fn set_fx_lights(lights: &[kh_vfx::Light]) {
    if let Ok(mut guard) = FX_LIGHTS.lock() {
        let held = guard.get_or_insert_with(|| (Vec::new(), Instant::now()));
        held.0.clear();
        held.0.extend_from_slice(lights);
        held.1 = Instant::now();
    }
}

/// The lights of a draw list handed over in the last 150 ms (the effects stop handing lists over
/// when they are switched off).
fn fx_lights_now() -> Vec<kh_vfx::Light> {
    FX_LIGHTS.lock().ok().and_then(|g| g.as_ref().filter(|(_, at)| at.elapsed().as_millis() < 150).map(|(l, _)| l.clone())).unwrap_or_default()
}

// ---------------------------------------------------------------------------- pure parts

/// The engine's exponent falloff (a light without `bUseInverseSquaredFalloff`): 1 at the centre,
/// 0 at and beyond the radius.
pub fn radial(distance_sq: f32, radius: f32, exponent: f32) -> f32 {
    if radius <= 0.0 || !distance_sq.is_finite() {
        return 0.0;
    }
    let x = (1.0 - distance_sq / (radius * radius)).clamp(0.0, 1.0);
    if x <= 0.0 {
        0.0
    } else {
        x.powf(exponent.max(0.0))
    }
}

/// N.L with a wrap (0 = Lambert): what the shader does.
pub fn wrapped(n_dot_l: f32, wrap: f32) -> f32 {
    ((n_dot_l + wrap) / (1.0 + wrap)).clamp(0.0, 1.0)
}

fn luma(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// What a WHITE surface at his feet shows (display linear): the game's composed scene colour over
/// its surface colour there, times its exposure. `None` when the frame gave no usable probe.
pub fn white_level(l: &crate::light::GameLight) -> Option<f32> {
    if !l.scene || !l.probed() || !(l.exposure.is_finite() && l.exposure > 1.0e-3) {
        return None;
    }
    let albedo = luma(l.albedo_px);
    let level = luma(l.scene_px.max(Vec3::ZERO)) / albedo.max(crate::light::DARK) * l.exposure;
    level.is_finite().then_some(level)
}

/// THE DARKNESS GATE with hysteresis and a fade (a weight 0..1 that moves at most 1 / `fade`
/// a second toward its target).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DarkGate {
    pub dark: bool,
    pub weight: f32,
}

impl DarkGate {
    /// `level`: `white_level` of the frame (`None` = no reading: the gate holds its state).
    /// `mode`: 0 off, 1 automatic, 2 always on. Returns the weight.
    pub fn step(&mut self, level: Option<f32>, dt: f32, on_below: f32, off_above: f32, fade: f32, mode: u8) -> f32 {
        match mode {
            0 => self.dark = false,
            2 => self.dark = true,
            _ => {
                if let Some(level) = level {
                    if self.dark && level > off_above.max(on_below) {
                        self.dark = false;
                    } else if !self.dark && level < on_below {
                        self.dark = true;
                    }
                }
            }
        }
        let target = if self.dark { 1.0 } else { 0.0 };
        let rate = if fade > 1.0e-3 { dt.clamp(0.0, 0.25) / fade } else { 1.0 };
        self.weight += (target - self.weight).clamp(-rate, rate);
        self.weight = self.weight.clamp(0.0, 1.0);
        self.weight
    }

    /// The weight as the light uses it (eased at both ends).
    pub fn strength(&self) -> f32 {
        let w = self.weight;
        w * w * (3.0 - 2.0 * w)
    }
}

/// One light of the pass: game space (cm); `color` in the target's units (what a white surface
/// facing it at its centre gets); `on_sora`: the share his ambient takes (packs without the `*_pl`
/// shaders). (IMPL_ER44) `sora`: its strength on his materials as a point light (relative to the
/// ground), `sora_ambient`: the ambient share that remains with it; `placed`: the plain hero light (placed
/// for him above and toward the camera); `body_shadow`: how dark his body's shadow from it is in the pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassLight {
    pub position: Vec3,
    pub radius: f32,
    pub color: Vec3,
    pub exponent: f32,
    pub wrap: f32,
    pub on_sora: f32,
    pub sora: f32,
    pub sora_ambient: f32,
    pub placed: bool,
    pub body_shadow: f32,
}

/// The effects' lights worth drawing, brightest (colour x radius) first, at most `max`.
pub fn rank(lights: &[kh_vfx::Light], max: usize) -> Vec<kh_vfx::Light> {
    let mut out: Vec<kh_vfx::Light> = lights.iter().filter(|l| l.radius > 1.0 && l.position.is_finite() && l.color.is_finite() && l.color.max_element() > 1.0e-4).copied().collect();
    out.sort_by(|a, b| (b.color.max_element() * b.radius).total_cmp(&(a.color.max_element() * a.radius)));
    out.truncate(max);
    out
}

/// (IMPL_ER56: each light now carries its own `LightExponent`; this is the fallback for one without) The exponent KH3's fire lights carry (`LightExponent` of vm_fir_blt0 2, vm_blc_fir0 / vm_blz_fir0 3);
/// the draw list does not carry it per light.
pub const FX_EXPONENT: f32 = 2.0;

/// (IMPL_ER56, offline comparisons) one exponent for every effect light (0 = each light's own, the default); the
/// renderer before IMPL_ER56 used FX_EXPONENT for all.
static FX_EXPONENT_FORCED: AtomicU32 = AtomicU32::new(0);

pub fn force_fx_exponent(exponent: f32) {
    FX_EXPONENT_FORCED.store(exponent.max(0.0).to_bits(), Ordering::Relaxed);
}

/// The frame's lights. `fx_scale`: KH3 scene-linear -> the target (`vfx::effect_scale` x the
/// effects' `hdr`); `display_to_target`: display linear -> the target (1 / the game's exposure);
/// `feet`: his feet (game space), `hero`: the gate's strength; `blade`: the Keyblade light's place on the blade and
/// the weapon's opacity (`None`: the Keyblade is not drawn).
pub fn plan(s: &LightSettings, fx: &[kh_vfx::Light], fx_scale: f32, display_to_target: f32, feet: Option<Vec3>, hero: f32, blade: Option<(Vec3, f32)>) -> Vec<PassLight> {
    let mut out = Vec::new();
    if s.hero > 0 && hero > 1.0e-3 && s.hero_intensity > 0.0 && s.hero_radius > 1.0 {
        // (IMPL_ER44) from the Keyblade (fading with it), or the plain light above his feet
        let at = if s.hero_keyblade { blade.filter(|(p, o)| *o > 0.0 && p.is_finite()).map(|(p, o)| (p, o.min(1.0), false)) } else { feet.map(|f| (f + Vec3::Z * s.hero_height, 1.0, true)) };
        if let Some((position, opacity, placed)) = at {
            // (IMPL_ER64, the user: "the hero light on sora still didn't look great") ON SORA the Keyblade light is placed
            // as a KH3 character light (above him, toward the camera: `hero.sora_placed`), its strength matched to what the
            // blade gives his chest; the world is still lit from the blade (and his shadow thrown from it)
            out.push(PassLight { position, radius: s.hero_radius, color: s.hero_color * (s.hero_intensity * hero * opacity * display_to_target), exponent: s.hero_exponent, wrap: s.hero_wrap, on_sora: s.hero_on_sora, sora: s.hero_sora, sora_ambient: s.hero_sora_ambient, placed: placed || s.hero_sora_placed, body_shadow: if placed { 0.0 } else { s.hero_shadow } });
        }
    }
    if s.fx && s.fx_gain > 0.0 {
        let room = MAX_LIGHTS - out.len();
        for l in rank(fx, s.fx_max.min(room)) {
            out.push(PassLight { position: l.position, radius: l.radius, color: l.color * (s.fx_gain * fx_scale * std::f32::consts::FRAC_1_PI), exponent: match f32::from_bits(FX_EXPONENT_FORCED.load(Ordering::Relaxed)) { forced if forced > 0.0 => forced, _ if l.exponent > 0.0 => l.exponent, _ => FX_EXPONENT }, wrap: 0.0, on_sora: s.fx_on_sora * if l.characters { 1.0 } else { s.fx_ambient_sora }, sora: s.fx_sora * if l.characters { 1.0 } else { s.fx_ambient_sora }, sora_ambient: s.fx_sora_ambient * if l.characters { 1.0 } else { s.fx_ambient_sora }, placed: false, body_shadow: 0.0 });
        }
    }
    out.retain(|l| l.color.is_finite() && l.color.max_element() > 0.0);
    out
}

/// (IMPL_ER64) THE WORLD'S LIGHT ON SORA from the ring of the game's direct light around his feet: `ring` (light on a
/// white surface, w = valid) and `ring_at` (tap relative to his feet, cm). The baseline is the darkest valid tap (light
/// that is everywhere: it reaches him through his ambient already); the rest, weighted by how much each tap exceeds
/// it, points at the light: a point light 150 cm toward that side and 120 cm up, radius 700 cm, its colour such that it
/// gives the brightest tap what that tap shows above the baseline. `None` when fewer than 6 taps are valid or the ring
/// is lit evenly (contrast below `min_contrast`). Game space, target units (as the pass lights).
pub fn world_light(ring: &[Vec4], ring_at: &[Vec4], feet: Vec3, gain: f32, min_contrast: f32) -> Option<PassLight> {
    let taps: Vec<(Vec3, Vec3)> = ring.iter().zip(ring_at).filter(|(l, a)| l.w > 0.5 && a.w > 0.5).map(|(l, a)| (l.truncate().max(Vec3::ZERO), a.truncate())).collect();
    if taps.len() < 6 || gain <= 0.0 {
        return None;
    }
    let base = taps.iter().map(|(l, _)| luma(*l)).fold(f32::MAX, f32::min);
    let (top_light, top_at) = taps.iter().copied().max_by(|a, b| luma(a.0).total_cmp(&luma(b.0)))?;
    let top = luma(top_light);
    if !(top > 1.0e-5) || (top - base) / top < min_contrast {
        return None;
    }
    let mut toward = Vec3::ZERO;
    for (l, at) in &taps {
        let excess = (luma(*l) - base).max(0.0);
        toward += Vec3::new(at.x, at.y, 0.0).normalize_or_zero() * excess;
    }
    let dir = toward.normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }
    const RADIUS: f32 = 700.0;
    let position = feet + dir * 150.0 + Vec3::Z * 120.0;
    let p = feet + top_at;
    let to = position - p;
    let falloff = radial(to.length_squared(), RADIUS, 2.0) * (to.z / to.length().max(1.0)).max(0.05);
    // (the brightest tap's light above the baseline, in its own colour)
    let base_colour = taps.iter().map(|(l, _)| *l).min_by(|a, b| luma(*a).total_cmp(&luma(*b))).unwrap_or(Vec3::ZERO);
    let colour = (top_light - base_colour).max(Vec3::ZERO) / falloff.max(0.05) * gain;
    (colour.is_finite() && colour.max_element() > 0.0).then_some(PassLight { position, radius: RADIUS, color: colour, exponent: 2.0, wrap: 0.3, on_sora: 0.0, sora: 1.0, sora_ambient: 0.05, placed: false, body_shadow: 0.0 })
}

/// (IMPL_ER64) The world's light of this frame for Sora (eased over ~0.3 s so that it does not jump with the taps), from
/// the game's light as read back; `level`: what a white surface at his feet shows (display).
pub(crate) fn world_light_now(game: Option<&crate::light::GameLight>, feet: Option<Vec3>, level: Option<f32>) -> Option<PassLight> {
    let s = settings();
    let raw = match (s.world, game, feet) {
        (true, Some(g), Some(f)) if level.is_none_or(|l| l <= s.world_max_level) => world_light(&g.ring, &g.ring_at, f, s.world_gain, s.world_min_contrast),
        _ => None,
    };
    let mut guard = WORLD.lock().unwrap_or_else(|e| e.into_inner());
    let eased = match (*guard, raw) {
        (Some(before), Some(now)) => Some(PassLight { position: before.position.lerp(now.position, 0.1), color: before.color.lerp(now.color, 0.1), ..now }),
        (Some(before), None) => Some(PassLight { color: before.color * 0.9, ..before }).filter(|l| l.color.max_element() > 1.0e-4),
        (None, now) => now,
    };
    *guard = eased;
    WORLD_SEEN.store(raw.is_some() as u32, Ordering::Relaxed);
    eased
}

static WORLD: Mutex<Option<PassLight>> = Mutex::new(None);
/// (offline harness) lights standing for the game's own (a torch): drawn by the pass on the world only.
static TEST_PASS: Mutex<Vec<PassLight>> = Mutex::new(Vec::new());

pub fn set_test_pass_lights(lights: Vec<PassLight>) {
    if let Ok(mut g) = TEST_PASS.lock() {
        *g = lights;
    }
}
static WORLD_SEEN: AtomicU32 = AtomicU32::new(0);
/// (offline harness) a ring to use instead of the game's (light, at), for the harness that has no game light.
static TEST_RING: Mutex<Option<(Vec<Vec4>, Vec<Vec4>)>> = Mutex::new(None);

pub fn set_test_ring(ring: Option<(Vec<Vec4>, Vec<Vec4>)>) {
    if let Ok(mut g) = TEST_RING.lock() {
        *g = ring;
    }
}

pub(crate) fn test_ring() -> Option<(Vec<Vec4>, Vec<Vec4>)> {
    TEST_RING.lock().ok().and_then(|g| g.clone())
}

/// What the lights give his ambient (in the TARGET's units; the caller divides by his out-scale):
/// their light on a white surface at `chest`, half of it (an ambient has no direction), times
/// each light's share: `sora_ambient` when his materials take the lights themselves
/// (`own_shaders`: the pack's `*_pl` shaders are in use), `on_sora` otherwise.
pub fn on_sora(lights: &[PassLight], chest: Vec3, own_shaders: bool) -> Vec3 {
    lights.iter().map(|l| l.color * (radial(l.position.distance_squared(chest), l.radius, l.exponent) * 0.5 * if own_shaders { l.sora_ambient } else { l.on_sora })).sum()
}

// ---------------------------------------------------------------------------- (IMPL_ER44) the lights on Sora

/// Point lights his materials take at most (er_char.wgsl `xl_pos` / `xl_col`).
pub const MAX_ON_SORA: usize = 6;
/// His chest and head above his feet (cm): where the lights are weighed and the placed hero light is matched.
const CHEST: f32 = 100.0;
const HEAD: f32 = 150.0;

/// One point light on his materials: game space (cm), radius (cm), `color` in the target's units as the
/// pass's (what a white surface facing it at its centre gets), the falloff exponent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoraLight {
    pub position: Vec3,
    pub radius: f32,
    pub color: Vec3,
    pub exponent: f32,
}

/// The lights on Sora: `feet` his feet (game space), `camera` the camera's position (game space).
/// Each pass light x its `sora` share; the plain hero light (`placed`; the Keyblade light stays on the blade) moved to `hero_height` above his feet and
/// `hero_toward` toward the camera (level; straight above him when the camera is above him), its colour
/// scaled so that his CHEST gets what the real light gives there; effect lights from where they are,
/// when their sphere reaches him. The strongest on him first, at most `MAX_ON_SORA`.
pub fn sora_lights(lights: &[PassLight], feet: Vec3, camera: Vec3, hero_height: f32, hero_toward: f32) -> Vec<SoraLight> {
    let chest = feet + Vec3::Z * CHEST;
    let toward = Vec3::new(camera.x - feet.x, camera.y - feet.y, 0.0).normalize_or_zero();
    let mut out: Vec<(f32, SoraLight)> = Vec::new();
    for l in lights.iter().filter(|l| l.sora > 0.0 && l.radius > 1.0 && l.position.is_finite() && l.color.is_finite()) {
        let (position, scale) = if l.placed {
            let placed = feet + Vec3::Z * hero_height + toward * hero_toward;
            let there = radial(placed.distance_squared(chest), l.radius, l.exponent);
            if there < 1.0e-3 {
                continue;
            }
            (placed, radial(l.position.distance_squared(chest), l.radius, l.exponent) / there)
        } else {
            (l.position, 1.0)
        };
        let light = SoraLight { position, radius: l.radius, color: l.color * (l.sora * scale), exponent: l.exponent };
        // (how much of it reaches him: the most at his feet, chest or head)
        let reach = [feet + Vec3::Z * 10.0, chest, feet + Vec3::Z * HEAD].iter().map(|p| radial(position.distance_squared(*p), l.radius, l.exponent)).fold(0.0f32, f32::max);
        let weight = luma(light.color) * reach;
        if weight > 0.0 && weight.is_finite() {
            out.push((weight, light));
        }
    }
    // (IMPL_ER86) with the steady light (stable.rs, `fx.soft_cut`) the lights at the cut fade instead of popping
    let st = crate::stable::settings();
    crate::stable::soft_cut(out, MAX_ON_SORA, st.on && st.soft_cut).into_iter().map(|(f, l)| SoraLight { color: l.color * f, ..l }).collect()
}

/// The length of the block below (floats).
pub const SORA_BLOCK_FLOATS: usize = 4 * (1 + 2 * MAX_ON_SORA);

/// Floats of the frame constants' last block (er_char.wgsl `xl_head`, `xl_pos`, `xl_col`): head (count, -, -, -),
/// per light position + radius, colour + exponent. The colour goes to the material's light shader in KH3 light
/// units: a white Lambert surface shows colour / pi there and the shaded colour is multiplied by `out_scale`
/// on its way into the target, so colour = the pass's colour x pi / out_scale (what the ground gets).
pub fn sora_block(lights: &[SoraLight], out_scale: f32) -> Vec<f32> {
    let mut out = vec![0.0f32; SORA_BLOCK_FLOATS];
    if !(out_scale.is_finite() && out_scale > 0.0) {
        return out;
    }
    let n = lights.len().min(MAX_ON_SORA);
    out[0] = n as f32;
    for (k, l) in lights.iter().take(n).enumerate() {
        let colour = l.color * (std::f32::consts::PI / out_scale);
        out[4 * (1 + k)..4 * (2 + k)].copy_from_slice(&[l.position.x, l.position.y, l.position.z, l.radius]);
        out[4 * (1 + MAX_ON_SORA + k)..4 * (2 + MAX_ON_SORA + k)].copy_from_slice(&[colour.x, colour.y, colour.z, l.exponent]);
    }
    out
}

/// Whether the pipelines of his materials were made from the pack's `*_pl` shaders (gfx.rs `read_shaders`).
static OWN_SHADERS: AtomicBool = AtomicBool::new(false);
/// Lights on him in the latest frame (for the log).
static ON_SORA: AtomicU32 = AtomicU32::new(0);

/// (offline comparisons) false = the older pixel shaders even when the pack has the *_pl set; read when the shaders load.
static OWN_WANTED: AtomicBool = AtomicBool::new(true);

pub fn want_own_shaders(on: bool) {
    OWN_WANTED.store(on, Ordering::Relaxed);
}

pub(crate) fn own_shaders_wanted() -> bool {
    OWN_WANTED.load(Ordering::Relaxed)
}

pub(crate) fn set_own_shaders(on: bool) {
    OWN_SHADERS.store(on, Ordering::Relaxed);
}

/// True when his materials take the lights themselves.
pub fn own_shaders() -> bool {
    OWN_SHADERS.load(Ordering::Relaxed)
}

/// (gfx.rs) The lights on him of this frame: the block of the frame constants, and his ambient lift (target units).
pub(crate) fn for_sora(lights: &[PassLight], feet: Option<Vec3>, camera: Vec3, out_scale: f32) -> (Vec<f32>, Vec3) {
    for_sora_toward(lights, feet, camera, settings().hero_sora_toward, out_scale)
}

/// (IMPL_ER86) `for_sora` with the placed hero light moved toward `toward` by `toward_cm` (stable.rs `hero_toward`: the
/// camera as before, or a point ahead of his facing, or his feet = straight above him).
pub(crate) fn for_sora_toward(lights: &[PassLight], feet: Option<Vec3>, camera: Vec3, toward_cm: f32, out_scale: f32) -> (Vec<f32>, Vec3) {
    let Some(feet) = feet else {
        ON_SORA.store(0, Ordering::Relaxed);
        return (vec![0.0; SORA_BLOCK_FLOATS], Vec3::ZERO);
    };
    let own = own_shaders();
    let lift = on_sora(lights, feet + Vec3::Z * CHEST, own);
    let s = settings();
    let on_him = if own { sora_lights(lights, feet, camera, s.hero_sora_height, toward_cm) } else { Vec::new() };
    ON_SORA.store(on_him.len() as u32, Ordering::Relaxed);
    if let Ok(mut last) = LAST_SORA.lock() {
        last.clone_from(&on_him);
    }
    (sora_block(&on_him, out_scale), lift)
}

/// The constant block of the pass (b1): head (count, the surface colour is the game's, the
/// colour used without it, occlusion steps), head2 (occlusion thickness m, bias m, -, -), then
/// per light position + radius, colour + exponent, (wrap, his body's shadow (plight2 only), -, -).
pub fn block(lights: &[PassLight], albedo: bool, fallback_albedo: f32, steps: u32) -> Vec<f32> {
    let n = lights.len().min(MAX_LIGHTS);
    let mut out = vec![0.0f32; 4 * (2 + 3 * MAX_LIGHTS)];
    out[..4].copy_from_slice(&[n as f32, if albedo { 1.0 } else { 0.0 }, fallback_albedo, steps as f32]);
    out[4..8].copy_from_slice(&[1.5, 0.12, 0.0, 0.0]);
    for (k, l) in lights.iter().take(n).enumerate() {
        let at = |array: usize| 4 * (2 + array * MAX_LIGHTS + k);
        out[at(0)..at(0) + 4].copy_from_slice(&[l.position.x, l.position.y, l.position.z, l.radius]);
        out[at(1)..at(1) + 4].copy_from_slice(&[l.color.x, l.color.y, l.color.z, l.exponent]);
        out[at(2)..at(2) + 4].copy_from_slice(&[l.wrap, l.body_shadow, 0.0, 0.0]);
    }
    out
}

/// The pixels the lights can reach: the union of their spheres' screen boxes (left, top, right,
/// bottom); the whole target when one sphere holds the camera or lies behind it; `None` when
/// none is in view.
pub fn screen_box(lights: &[PassLight], clip: &Mat4, width: u32, height: u32, tan_y: f32) -> Option<(i32, i32, i32, i32)> {
    let (w, h) = (width as f32, height as f32);
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for l in lights {
        let c = *clip * l.position.extend(1.0);
        let radius_m = l.radius * 0.01;
        if c.w <= radius_m + 0.05 {
            return Some((0, 0, width as i32, height as i32));
        }
        let centre = glam::Vec2::new((c.x / c.w * 0.5 + 0.5) * w, (0.5 - c.y / c.w * 0.5) * h);
        // (a sphere's projection is a little larger than r / w: x 1.25 covers it at the angles that matter)
        let reach = radius_m / (c.w * tan_y.max(1.0e-3)) * h * 0.5 * 1.25 + 2.0;
        lo = lo.min(centre - glam::Vec2::splat(reach));
        hi = hi.max(centre + glam::Vec2::splat(reach));
    }
    let b = (lo.x.max(0.0) as i32, lo.y.max(0.0) as i32, hi.x.min(w).max(0.0) as i32, hi.y.min(h).max(0.0) as i32);
    (b.2 > b.0 && b.3 > b.1).then_some(b)
}

// ---------------------------------------------------------------------------- the frame's numbers

/// What the colour draw asks once a frame (gfx.rs): the lights, the hero's strength, the reading.
pub struct Frame {
    pub lights: Vec<PassLight>,
    pub hero: f32,
    pub level: Option<f32>,
}

/// Steps the gate on the frame's reading and plans the lights.
pub(crate) fn frame(level: Option<f32>, fx_scale: f32, display_to_target: f32, feet: Option<Vec3>, blade: Option<(Vec3, f32)>) -> Frame {
    let s = settings();
    let hero = {
        let mut guard = GATE.lock().unwrap_or_else(|e| e.into_inner());
        let (gate, at) = guard.get_or_insert_with(|| (DarkGate::default(), Instant::now()));
        let dt = at.elapsed().as_secs_f32();
        *at = Instant::now();
        gate.step(level, dt, s.hero_dark_on, s.hero_dark_off, s.hero_fade, s.hero);
        gate.strength()
    };
    let fx = if s.fx { fx_lights_now() } else { Vec::new() };
    // (IMPL_ER86) with more effect lights than the pass takes: those that give HIM the most, softly cut (stable.rs)
    let st = crate::stable::settings();
    let fx = match feet {
        Some(f) if st.on && st.rank_near && fx.len() > s.fx_max => crate::stable::rank_near(&fx, s.fx_max, f).into_iter().map(|(k, l)| kh_vfx::Light { color: l.color * k, ..l }).collect(),
        _ => fx,
    };
    let mut lights = plan(&s, &fx, fx_scale, display_to_target, feet, hero, blade);
    if let Ok(test) = TEST_PASS.lock() {
        lights.extend(test.iter().copied());
    }
    if let Ok(mut last) = LAST.lock() {
        last.clone_from(&lights);
    }
    // (for the log)
    FX_SEEN_MAX.fetch_max(fx.len() as u32, Ordering::Relaxed);
    HERO_BITS.store(hero.to_bits(), Ordering::Relaxed);
    if let Some(level) = level {
        LEVEL_BITS.store(level.to_bits(), Ordering::Relaxed);
    }
    Frame { lights, hero, level }
}

// ---------------------------------------------------------------------------- (IMPL_ER44) the Keyblade light's host side

/// True while Sora should hold the Keyblade out for the hero light: the light is on (`r.hero_light` 1 / 2), it comes from
/// the Keyblade (`hero.keyblade`) and the darkness gate is closed (or forced).
pub fn keyblade_wanted() -> bool {
    let s = settings();
    if s.hero == 0 || !s.hero_keyblade {
        return false;
    }
    s.hero == 2 || GATE.lock().map(|g| g.as_ref().is_some_and(|(gate, _)| gate.dark)).unwrap_or(false)
}

/// (IMPL_ER53) THE STEADY KEYBLADE GLOW: copies of KH3's own appear effect (the host's `EffectHost::define_variant`):
/// `GLOW_EFFECT` = the blade-glow emitters only, its clock held (`hero.glow_hold_s`), started once when the light comes on
/// and stopped softly (it plays out as authored) when it goes off; `SPARK_EFFECT` = the small sparkles only, restarted
/// at `hero.sparkle_s`. The effect's particle light (`light0`) is in neither: the light is the hero light alone, steady.
/// Emitters chosen by looking (build/sora_tmp/er/er53/shots).
pub const GLOW_BASE: &str = "VW_SO010_APP0";
pub const GLOW_SOCKET: &str = "R_wep_skt";
pub const GLOW_EFFECT: &str = "VW_SO010_APP0#glow";
pub const SPARK_EFFECT: &str = "VW_SO010_APP0#spark";
pub const GLOW_EMITTERS: [&str; 2] = ["Key", "glw_I"];
pub const SPARK_EMITTERS: [&str; 3] = ["ps_cntglwcrs_pos", "c_cntcrs0", "c_cnt_dot1"];

/// (offline comparisons) other emitter lists for the two copies (`None` = the constants above).
static GLOW_KEEP: Mutex<Option<(Vec<String>, Vec<String>)>> = Mutex::new(None);

pub fn choose_glow_emitters(glow: Vec<String>, spark: Vec<String>) {
    if let Ok(mut g) = GLOW_KEEP.lock() {
        *g = Some((glow, spark));
    }
}

/// The two copies as a host defines them: (name, base, kept emitters, held clock).
pub fn glow_variants() -> [(&'static str, &'static str, Vec<String>, Option<f32>); 2] {
    let s = settings();
    let (glow, spark) = GLOW_KEEP.lock().ok().and_then(|g| g.clone()).unwrap_or_else(|| (GLOW_EMITTERS.iter().map(|e| e.to_string()).collect(), SPARK_EMITTERS.iter().map(|e| e.to_string()).collect()));
    [(GLOW_EFFECT, GLOW_BASE, glow, Some(s.hero_glow_hold.max(0.0))), (SPARK_EFFECT, GLOW_BASE, spark, None)]
}

/// What a host does in one step for the Keyblade glow.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlowStep {
    /// Start `GLOW_EFFECT` (persistent, at `GLOW_SOCKET`).
    pub start_glow: bool,
    /// Stop it softly.
    pub stop_glow: bool,
    /// Start the sparkle effect (`SPARK_EFFECT`; with `hero.glow` 0: `sparkle_effect()`).
    pub sparkle: bool,
}

/// (IMPL_ER53) The Keyblade glow's host side: `lit` = the light is on and the blade fully drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KeybladeGlow {
    pub glowing: bool,
    pub sparkle: Sparkle,
}

impl KeybladeGlow {
    pub fn step(&mut self, dt: f32, lit: bool, steady: bool, interval: f32) -> GlowStep {
        let want = lit && steady;
        let out = GlowStep { start_glow: want && !self.glowing, stop_glow: !want && self.glowing, sparkle: self.sparkle.step(dt, lit, interval) };
        self.glowing = want;
        out
    }
}

/// The Keyblade's sparkle: KH3's effect started again at an interval while the light is on and the blade is drawn
/// (the first at once).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sparkle {
    pub since: Option<f32>,
}

impl Sparkle {
    /// One host step: true = start the effect now.
    pub fn step(&mut self, dt: f32, on: bool, interval: f32) -> bool {
        if !on {
            self.since = None;
            return false;
        }
        match self.since {
            Some(t) if t + dt < interval.max(0.1) => {
                self.since = Some(t + dt);
                false
            }
            _ => {
                self.since = Some(0.0);
                true
            }
        }
    }
}

/// KH3's own effects the Keyblade can sparkle with (the Kingdom Key's own, `Effects/wep/w_so010/par`; another Keyblade's
/// effects, e.g. `vw_so072_wep_lgt0` "weapon light", are refused by the effect-group rule) and the socket each is started at:
/// 0 `VW_SO010_APP0` (its appear effect, category keyblade_appear: the blade glows and blue sparkles run along it; at
/// the hand socket as the game's Draw starts it), 1 `vw_so010_str0` (its hit star: star sprites that fly off), 2
/// `VW_SO010_LOC0` (the small tip flash of its finisher clips, 0.19 s: hardly visible). Harness pictures: er44/shots
/// cmp_sparkle_*.png.
pub const SPARKLE_EFFECTS: [(&str, &str); 3] = [("VW_SO010_APP0", "R_wep_skt"), ("vw_so010_str0", "vfx_top"), ("VW_SO010_LOC0", "vfx_top")];
static SPARKLE_CHOICE: AtomicU32 = AtomicU32::new(0);

/// (offline comparisons) which of `SPARKLE_EFFECTS`.
pub fn choose_sparkle(index: usize) {
    SPARKLE_CHOICE.store(index.min(SPARKLE_EFFECTS.len() - 1) as u32, Ordering::Relaxed);
}

/// The sparkle effect and its socket.
pub fn sparkle_effect() -> (&'static str, &'static str) {
    SPARKLE_EFFECTS[(SPARKLE_CHOICE.load(Ordering::Relaxed) as usize).min(SPARKLE_EFFECTS.len() - 1)]
}

// ---------------------------------------------------------------------------- the GPU side

static FRAMES_LIT: AtomicU32 = AtomicU32::new(0);
static LIGHTS_DRAWN: AtomicU64 = AtomicU64::new(0);
static FX_SEEN_MAX: AtomicU32 = AtomicU32::new(0);
static PIXELS: AtomicU64 = AtomicU64::new(0);
static HERO_BITS: AtomicU32 = AtomicU32::new(0);
static LEVEL_BITS: AtomicU32 = AtomicU32::new(0x7FC0_0000);

/// The pass's pipeline: (device, target format) it was made for; `None` inside = it could not be made.
struct Kit {
    device: usize,
    format: i32,
    pipeline: Option<ID3D12PipelineState>,
}

// SAFETY: D3D12 objects are free-threaded.
unsafe impl Send for Kit {}

static KIT: Mutex<Option<Kit>> = Mutex::new(None);
/// The lights of the latest frame (for `describe`).
static LAST: Mutex<Vec<PassLight>> = Mutex::new(Vec::new());
/// (IMPL_ER44) The lights on him of the latest frame (for `describe`).
static LAST_SORA: Mutex<Vec<SoraLight>> = Mutex::new(Vec::new());

/// The latest frame's lights, one line each (offline harness, diagnosis).
pub fn describe() -> Vec<String> {
    let mut out: Vec<String> = LAST.lock().map(|l| l.iter().map(|l| format!("light at ({:.0}, {:.0}, {:.0}) cm radius {:.0} cm colour ({:.4}, {:.4}, {:.4}) exponent {} wrap {}", l.position.x, l.position.y, l.position.z, l.radius, l.color.x, l.color.y, l.color.z, l.exponent, l.wrap)).collect()).unwrap_or_default();
    if let Ok(on_him) = LAST_SORA.lock() {
        out.extend(on_him.iter().map(|l| format!("  on Sora: at ({:.0}, {:.0}, {:.0}) cm radius {:.0} cm colour ({:.4}, {:.4}, {:.4}) exponent {}", l.position.x, l.position.y, l.position.z, l.radius, l.color.x, l.color.y, l.color.z, l.exponent)));
    }
    out
}

/// The additive pipeline (one triangle, no depth, the target's colour + the pass's output).
unsafe fn make_pipeline(device: &ID3D12Device, root: &ID3D12RootSignature, format: DXGI_FORMAT) -> Result<ID3D12PipelineState, String> {
    let pack = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone())).ok_or("no pack")?;
    let dir = pack.join("materials").join("er");
    let read = |name: &str| std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e} (cargo run --release -p kh-shaderc -- --only plight)"));
    // (IMPL_ER44) plight2: the same pass + his body's shadow from the Keyblade light (capsules); the older file without it
    let (vertex, pixel) = (read("blob.vs.dxil")?, read("plight2.ps.dxil").or_else(|_| read("plight.ps.dxil"))?);
    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
    formats[0] = format;
    let mut blend = D3D12_BLEND_DESC::default();
    blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
        BlendEnable: true.into(),
        LogicOpEnable: false.into(),
        SrcBlend: D3D12_BLEND_ONE,
        DestBlend: D3D12_BLEND_ONE,
        BlendOp: D3D12_BLEND_OP_ADD,
        SrcBlendAlpha: D3D12_BLEND_ZERO,
        DestBlendAlpha: D3D12_BLEND_ONE,
        BlendOpAlpha: D3D12_BLEND_OP_ADD,
        LogicOp: D3D12_LOGIC_OP_NOOP,
        // (the target's alpha is the game's: left alone)
        RenderTargetWriteMask: (D3D12_COLOR_WRITE_ENABLE_RED.0 | D3D12_COLOR_WRITE_ENABLE_GREEN.0 | D3D12_COLOR_WRITE_ENABLE_BLUE.0) as u8,
    };
    let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::transmute_copy(root),
        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
        PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
        BlendState: blend,
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), StencilEnable: false.into(), ..Default::default() },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 1,
        RTVFormats: formats,
        DSVFormat: DXGI_FORMAT_UNKNOWN,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        ..Default::default()
    };
    device.CreateGraphicsPipelineState(&desc).map_err(|e| format!("CreateGraphicsPipelineState(plight): {e}"))
}

/// What the colour draw hands the pass (gfx.rs; the list has our root signature, our heaps and
/// the frame constants at b0 set, the target bound, the scene depth readable).
pub(crate) struct PassCx<'a> {
    pub device: &'a ID3D12Device,
    pub list: &'a ID3D12GraphicsCommandList,
    pub root: &'a ID3D12RootSignature,
    pub format: DXGI_FORMAT,
    /// The table of ours (17 views) the pass's views are written into, and its GPU handle.
    pub table_cpu: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub table_gpu: D3D12_GPU_DESCRIPTOR_HANDLE,
    pub view_size: usize,
    pub depth: &'a ID3D12Resource,
    pub depth_view_format: DXGI_FORMAT,
    pub albedo: Option<&'a ID3D12Resource>,
    pub white: &'a ID3D12Resource,
    /// The constant block's GPU address (b1).
    pub constants: u64,
    pub scissor: (i32, i32, i32, i32),
    pub lights: usize,
}

/// Records the pass. False when its pipeline is not there (logged once).
pub(crate) unsafe fn draw(cx: &PassCx) -> bool {
    let pipeline = {
        let mut guard = KIT.lock().unwrap_or_else(|e| e.into_inner());
        let device = cx.device.as_raw() as usize;
        if guard.as_ref().is_none_or(|k| k.device != device || k.format != cx.format.0) {
            let made = make_pipeline(cx.device, cx.root, cx.format);
            if let Err(e) = &made {
                crate::rlog!("render: the light pass cannot be made ({e}): effects and the hero light light nothing");
            }
            *guard = Some(Kit { device, format: cx.format.0, pipeline: made.ok() });
        }
        match guard.as_ref().and_then(|k| k.pipeline.clone()) {
            Some(p) => p,
            None => return false,
        }
    };
    // The views: t16 the scene depth (its depth plane), t3 the surface colour (sRGB: linear values).
    let at = |k: usize| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: cx.table_cpu.ptr + k * cx.view_size };
    let depth = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: cx.depth_view_format, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
    cx.device.CreateShaderResourceView(cx.depth, Some(&depth), at(16));
    match cx.albedo {
        Some(albedo) => {
            let desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
            cx.device.CreateShaderResourceView(albedo, Some(&desc), at(3));
        }
        None => cx.device.CreateShaderResourceView(cx.white, None, at(3)),
    }
    let (l, t, r, b) = cx.scissor;
    cx.list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: l, top: t, right: r, bottom: b }]);
    cx.list.SetPipelineState(&pipeline);
    cx.list.SetGraphicsRootConstantBufferView(1, cx.constants);
    cx.list.SetGraphicsRootDescriptorTable(3, cx.table_gpu);
    cx.list.DrawInstanced(3, 1, 0, 0);
    FRAMES_LIT.fetch_add(1, Ordering::Relaxed);
    LIGHTS_DRAWN.fetch_add(cx.lights as u64, Ordering::Relaxed);
    PIXELS.fetch_add(((r - l).max(0) as u64) * ((b - t).max(0) as u64), Ordering::Relaxed);
    true
}

/// Drops the pipeline (renderer shutdown / device change).
pub(crate) fn release() {
    if let Ok(mut guard) = KIT.lock() {
        *guard = None;
    }
}

/// One line for the once-a-second log; resets its counters.
pub fn status() -> String {
    let s = settings();
    let frames = FRAMES_LIT.swap(0, Ordering::Relaxed);
    let lights = LIGHTS_DRAWN.swap(0, Ordering::Relaxed);
    let pixels = PIXELS.swap(0, Ordering::Relaxed);
    let level = f32::from_bits(LEVEL_BITS.load(Ordering::Relaxed));
    format!(
        "light pass (IMPL_ER39): {frames} frame(s) lit, {:.1} light(s) a frame, {:.2} Mpx a frame | effect lights {} (most this second {}) | hero light {} strength {:.2}, white at his feet shows {} (dark below {:.3}, light above {:.3}) | on Sora (IMPL_ER44): {}",
        if frames > 0 { lights as f64 / frames as f64 } else { 0.0 },
        if frames > 0 { pixels as f64 / frames as f64 / 1.0e6 } else { 0.0 },
        if s.fx { "on" } else { "OFF (fx.light = 0)" },
        FX_SEEN_MAX.swap(0, Ordering::Relaxed),
        match s.hero { 0 => "OFF (r.hero_light = 0)", 2 => "FORCED (r.hero_light = 2)", _ => "auto" },
        f32::from_bits(HERO_BITS.load(Ordering::Relaxed)),
        if level.is_finite() { format!("{level:.3}") } else { "-".to_string() },
        s.hero_dark_on,
        s.hero_dark_off,
        if own_shaders() { format!("{} point light(s) in his own shaders", ON_SORA.load(Ordering::Relaxed)) } else { "ambient lift only (his older pixel shaders: no *_pl set in use)".to_string() }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_falloff_is_one_at_the_centre_zero_at_the_radius_and_falls_monotonically() {
        assert_eq!(radial(0.0, 300.0, 2.0), 1.0);
        assert_eq!(radial(300.0 * 300.0, 300.0, 2.0), 0.0);
        assert_eq!(radial(400.0 * 400.0, 300.0, 3.0), 0.0);
        assert_eq!((radial(1.0, 0.0, 2.0), radial(f32::NAN, 100.0, 2.0)), (0.0, 0.0));
        // half the radius: (1 - 1/4)^2
        assert!((radial(150.0 * 150.0, 300.0, 2.0) - 0.5625).abs() < 1.0e-6);
        let mut before = 2.0;
        for k in 0..=30 {
            let d = k as f32 * 10.0;
            let v = radial(d * d, 300.0, 3.0);
            assert!(v <= before && (0.0..=1.0).contains(&v));
            before = v;
        }
        // a larger exponent falls faster
        assert!(radial(100.0 * 100.0, 300.0, 3.0) < radial(100.0 * 100.0, 300.0, 2.0));
        assert_eq!((wrapped(1.0, 0.5), wrapped(-1.0, 0.5), wrapped(0.0, 0.0)), (1.0, 0.0, 0.0));
        assert!((wrapped(0.0, 0.5) - 1.0 / 3.0).abs() < 1.0e-6);
    }

    #[test]
    fn the_gate_needs_both_thresholds_and_fades_over_its_time() {
        let (on, off, fade) = (0.06, 0.10, 1.0);
        let mut g = DarkGate::default();
        // light: nothing
        assert_eq!(g.step(Some(0.5), 0.016, on, off, fade, 1), 0.0);
        // between the thresholds from the light side: still light
        g.step(Some(0.08), 0.016, on, off, fade, 1);
        assert!(!g.dark);
        // dark: it comes in over one second (not at once)
        g.step(Some(0.02), 0.1, on, off, fade, 1);
        assert!(g.dark && (g.weight - 0.1).abs() < 1.0e-6);
        for _ in 0..20 {
            g.step(Some(0.02), 0.1, on, off, fade, 1);
        }
        assert_eq!(g.weight, 1.0);
        // flickering readings inside the band do not switch it off
        for level in [0.07, 0.09, 0.065, 0.095] {
            g.step(Some(level), 0.1, on, off, fade, 1);
            assert!(g.dark && g.weight == 1.0);
        }
        // no reading: held
        g.step(None, 0.1, on, off, fade, 1);
        assert!(g.dark);
        // above the upper threshold: it goes, over the same time
        g.step(Some(0.2), 0.25, on, off, fade, 1);
        assert!(!g.dark && (g.weight - 0.75).abs() < 1.0e-6);
        for _ in 0..3 {
            g.step(Some(0.2), 0.25, on, off, fade, 1);
        }
        assert_eq!((g.weight, g.strength()), (0.0, 0.0));
        // a long hitch moves it at most a quarter of a second's worth
        g.step(Some(0.0), 5.0, on, off, fade, 1);
        assert!((g.weight - 0.25).abs() < 1.0e-6);
        // modes: 0 off, 2 always
        let mut forced = DarkGate::default();
        forced.step(Some(10.0), 2.0, on, off, 0.0, 2);
        assert_eq!(forced.weight, 1.0);
        forced.step(Some(0.0), 0.016, on, off, 0.0, 0);
        assert_eq!(forced.weight, 0.0);
        assert_eq!((DarkGate { dark: true, weight: 0.5 }).strength(), 0.5);
    }

    #[test]
    fn the_white_level_is_scene_over_surface_times_exposure() {
        let mut l = crate::light::GameLight { scene: true, exposure: 20.0, taps: 9.0, albedo_px: Vec3::splat(0.1), scene_px: Vec3::splat(0.0005), ..Default::default() };
        assert!((white_level(&l).unwrap() - 0.1).abs() < 1.0e-5);
        l.exposure = 0.0;
        assert_eq!(white_level(&l), None);
        l.exposure = 20.0;
        l.taps = 0.0;
        assert_eq!(white_level(&l), None);
    }

    #[test]
    fn the_plan_puts_the_hero_first_ranks_the_effects_and_stays_out_of_the_light() {
        // (the plain light above him: hero.keyblade = 0)
        let s = LightSettings { fx: true, hero: 1, hero_keyblade: false, ..LightSettings::default() };
        let fx = vec![
            kh_vfx::Light { position: Vec3::new(100.0, 0.0, 50.0), color: Vec3::new(10.0, 5.0, 1.0), radius: 300.0, exponent: 2.0, characters: true },
            kh_vfx::Light { position: Vec3::new(0.0, 0.0, 50.0), color: Vec3::new(1.0, 0.5, 0.1), radius: 100.0, exponent: 2.0, characters: true },
            kh_vfx::Light { position: Vec3::ZERO, color: Vec3::ZERO, radius: 300.0, exponent: 2.0, characters: true },
            kh_vfx::Light { position: Vec3::ZERO, color: Vec3::ONE, radius: 0.5, exponent: 2.0, characters: true },
        ];
        let feet = Some(Vec3::new(10.0, 20.0, 0.0));
        let lit = plan(&s, &fx, 0.01, 1.0 / 20.0, feet, 0.0, None);
        // no hero light while the gate is closed; two usable effect lights, brightest first, x 1 / pi
        assert_eq!(lit.len(), 2);
        assert_eq!(lit[0].position, Vec3::new(100.0, 0.0, 50.0));
        assert!((lit[0].color.x - 10.0 * 0.01 / std::f32::consts::PI).abs() < 1.0e-6);
        let dark = plan(&s, &fx, 0.01, 1.0 / 20.0, feet, 1.0, None);
        assert_eq!(dark.len(), 3);
        assert_eq!(dark[0].position, Vec3::new(10.0, 20.0, s.hero_height));
        assert!((dark[0].color - s.hero_color * s.hero_intensity / 20.0).length() < 1.0e-6);
        // off switches
        assert!(plan(&LightSettings { fx: false, hero: 0, ..s }, &fx, 0.01, 0.05, feet, 1.0, None).is_empty());
        // never more than the shader holds
        let many: Vec<kh_vfx::Light> = (0..40).map(|k| kh_vfx::Light { position: Vec3::X * k as f32, color: Vec3::ONE, radius: 200.0, exponent: 2.0, characters: true }).collect();
        assert_eq!(plan(&LightSettings { fx_max: 99, ..s }, &many, 1.0, 1.0, feet, 1.0, None).len(), MAX_LIGHTS);
        // his ambient: half the white-surface light at his chest, times the share
        let one = [PassLight { position: Vec3::ZERO, radius: 100.0, color: Vec3::ONE, exponent: 2.0, wrap: 0.0, on_sora: 0.5, sora: 1.0, sora_ambient: 0.1, placed: false, body_shadow: 0.0 }];
        assert_eq!(on_sora(&one, Vec3::ZERO, false), Vec3::splat(0.25));
        assert_eq!(on_sora(&one, Vec3::X * 200.0, false), Vec3::ZERO);
        // (IMPL_ER44) with his own light shaders only the remaining ambient share
        assert!((on_sora(&one, Vec3::ZERO, true) - Vec3::splat(0.05)).length() < 1.0e-6);
        // the plan marks the hero light and carries the shares on him
        assert!(dark[0].placed && !dark[1].placed && dark[0].sora == s.hero_sora && dark[1].sora == s.fx_sora && dark[0].body_shadow == 0.0);
    }

    #[test]
    fn the_keyblade_light_sits_on_the_blade_fades_with_it_and_throws_his_shadow() {
        // (IMPL_ER64: by default the light is PLACED on him as a character light: below, the blade light on him as before)
        let placed = plan(&LightSettings { fx: false, hero: 1, ..LightSettings::default() }, &[], 1.0, 0.05, Some(Vec3::new(10.0, 20.0, 0.0)), 1.0, Some((Vec3::new(60.0, 30.0, 110.0), 1.0)));
        assert!(placed[0].placed && placed[0].position == Vec3::new(60.0, 30.0, 110.0) && placed[0].body_shadow > 0.0);
        let s = LightSettings { fx: false, hero: 1, hero_sora_placed: false, ..LightSettings::default() };
        assert!(s.hero_keyblade);
        let feet = Some(Vec3::new(10.0, 20.0, 0.0));
        let blade = Vec3::new(60.0, 30.0, 110.0);
        // no blade drawn yet: no light (it appears with the Keyblade)
        assert!(plan(&s, &[], 1.0, 0.05, feet, 1.0, None).is_empty());
        assert!(plan(&s, &[], 1.0, 0.05, feet, 1.0, Some((blade, 0.0))).is_empty());
        let full = plan(&s, &[], 1.0, 0.05, feet, 1.0, Some((blade, 1.0)));
        assert_eq!(full.len(), 1);
        assert_eq!(full[0].position, blade);
        assert!(!full[0].placed && full[0].body_shadow == s.hero_shadow);
        let half = plan(&s, &[], 1.0, 0.05, feet, 1.0, Some((blade, 0.5)));
        assert!((half[0].color - full[0].color * 0.5).length() < 1.0e-7);
        // with the gate half open it is weaker too
        let gate = plan(&s, &[], 1.0, 0.05, feet, 0.25, Some((blade, 1.0)));
        assert!((gate[0].color - full[0].color * 0.25).length() < 1.0e-7);
        // on him it stays where the blade is (direction from the blade)
        let on = sora_lights(&full, feet.unwrap(), Vec3::new(-400.0, 0.0, 200.0), s.hero_sora_height, s.hero_sora_toward);
        assert_eq!(on[0].position, blade);
        // the pass's block carries his shadow in opt.y
        let b = block(&full, true, 0.18, 6);
        assert_eq!(b[4 * (2 + 2 * MAX_LIGHTS) + 1], s.hero_shadow);
    }

    /// (IMPL_ER56) an effect light keeps its own exponent; one of an ambient effect (the save point) lights Sora softly
    #[test]
    fn effect_lights_keep_their_exponent_and_free_effects_do_not_light_sora() {
        let s = LightSettings { hero: 0, ..LightSettings::default() };
        let fx = [kh_vfx::Light { position: Vec3::new(60.0, 0.0, 0.0), color: Vec3::new(9.0, 18.0, 4.6), radius: 346.0, exponent: 16.0, characters: false }, kh_vfx::Light { position: Vec3::new(0.0, 50.0, 100.0), color: Vec3::new(10.0, 5.0, 1.0), radius: 300.0, exponent: 0.0, characters: true }];
        let lights = plan(&s, &fx, 1.0, 1.0, Some(Vec3::ZERO), 0.0, None);
        let save = lights.iter().find(|l| l.exponent == 16.0).expect("the save point light lights the world");
        assert_eq!((save.sora, save.sora_ambient, save.on_sora), (s.fx_sora * s.fx_ambient_sora, s.fx_sora_ambient * s.fx_ambient_sora, s.fx_on_sora * s.fx_ambient_sora));
        let fire = lights.iter().find(|l| l.exponent == FX_EXPONENT).expect("a light without an exponent takes the fallback");
        assert!(fire.sora > 0.0);
        let on_him = sora_lights(&lights, Vec3::ZERO, Vec3::new(-300.0, 0.0, 150.0), 200.0, 120.0);
        assert_eq!(on_him.len(), 2);
        let ambient = on_him.iter().find(|l| l.exponent == 16.0).unwrap();
        assert!((ambient.color - save.color * s.fx_sora * s.fx_ambient_sora).length() < 1e-4, "a soft share of it");
    }

    /// (IMPL_ER64) a torch beside him: the ring is brighter on its side; the fitted light stands on that side, above the
    /// ground, gives the brightest tap about what it shows; an evenly lit ring (the sun) or too few taps give none
    #[test]
    fn the_world_light_points_at_the_bright_side_of_the_ring() {
        let feet = Vec3::new(1000.0, -500.0, 30.0);
        let torch = feet + Vec3::new(0.0, 300.0, 150.0);
        let (mut ring, mut at) = (Vec::new(), Vec::new());
        for k in 0..24 {
            let a = (k % 12) as f32 * 30f32.to_radians();
            let r = if k >= 12 { 250.0 } else { 120.0 };
            let p = feet + Vec3::new(a.cos() * r, a.sin() * r, 0.0);
            let to = torch - p;
            let lit = Vec3::new(1.0, 0.6, 0.3) * radial(to.length_squared(), 900.0, 2.0) * (to.z / to.length()).max(0.0) + Vec3::splat(0.01);
            ring.push(lit.extend(1.0));
            at.push((p - feet).extend(1.0));
        }
        let l = world_light(&ring, &at, feet, 1.0, 0.35).expect("a light on the torch side");
        let dir = (l.position - feet).truncate().normalize();
        assert!(dir.y > 0.9, "toward the torch: {dir:?}");
        assert!(l.position.z > feet.z + 50.0 && l.color.x > l.color.z, "above the ground, in the torch colour");
        assert_eq!((l.sora, l.on_sora, l.placed), (1.0, 0.0, false));
        let even: Vec<Vec4> = ring.iter().map(|_| Vec4::new(0.5, 0.5, 0.5, 1.0)).collect();
        assert!(world_light(&even, &at, feet, 1.0, 0.35).is_none(), "evenly lit: no direction");
        let few: Vec<Vec4> = ring.iter().enumerate().map(|(k, l)| if k < 4 { *l } else { Vec4::ZERO }).collect();
        assert!(world_light(&few, &at, feet, 1.0, 0.35).is_none());
    }

    #[test]
    fn the_keyblade_glow_starts_once_stays_on_and_stops_with_the_light() {
        let dt = 1.0 / 60.0;
        let mut g = KeybladeGlow::default();
        let first = g.step(dt, true, true, 2.5);
        assert!(first.start_glow && first.sparkle && !first.stop_glow);
        let mut sparkles = 0;
        for _ in 0..299 {
            let s = g.step(dt, true, true, 2.5);
            assert!(!s.start_glow && !s.stop_glow, "the glow is started once and kept");
            sparkles += s.sparkle as u32;
        }
        assert_eq!(sparkles, 1, "only the sparkles come again (every 2.5 s)");
        let off = g.step(dt, false, true, 2.5);
        assert!(off.stop_glow && !off.sparkle && !off.start_glow);
        assert_eq!(g.step(dt, false, true, 2.5), GlowStep::default());
        // hero.glow 0: no held glow, the whole effect at the interval (IMPL_ER44)
        let mut old = KeybladeGlow::default();
        let s = old.step(dt, true, false, 2.5);
        assert!(s.sparkle && !s.start_glow);
        // the copies: the glow held, the sparkles not; no particle light in either
        let [glow, spark] = glow_variants();
        assert_eq!((glow.0, glow.1, spark.0, spark.3), (GLOW_EFFECT, GLOW_BASE, SPARK_EFFECT, None));
        assert!(glow.3.is_some() && !glow.2.iter().chain(spark.2.iter()).any(|e| e == "light0"));
    }

    #[test]
    fn the_sparkle_starts_at_once_then_at_its_interval_and_stops_with_the_light() {
        let mut k = Sparkle::default();
        assert!(!k.step(0.016, false, 1.5));
        assert!(k.step(0.016, true, 1.5));
        let mut started = 0;
        for _ in 0..300 {
            if k.step(1.0 / 60.0, true, 1.5) {
                started += 1;
            }
        }
        // 5 s at 1.5 s: 3 more
        assert_eq!(started, 3);
        assert!(!k.step(0.016, false, 1.5));
        assert_eq!(k.since, None);
        assert!(k.step(0.016, true, 1.5), "on again: at once");
    }

    #[test]
    fn the_block_and_the_screen_box() {
        let l = PassLight { position: Vec3::new(1.0, 2.0, 3.0), radius: 4.0, color: Vec3::new(5.0, 6.0, 7.0), exponent: 2.0, wrap: 0.5, on_sora: 0.0, sora: 0.0, sora_ambient: 0.0, placed: false, body_shadow: 0.0 };
        let b = block(&[l, l], true, 0.18, 6);
        assert_eq!(b.len(), 4 * (2 + 3 * MAX_LIGHTS));
        assert_eq!(&b[..4], &[2.0, 1.0, 0.18, 6.0]);
        assert_eq!(&b[8..12], &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(&b[4 * (2 + MAX_LIGHTS + 1)..4 * (2 + MAX_LIGHTS + 1) + 4], &[5.0, 6.0, 7.0, 2.0]);
        assert_eq!(b[4 * (2 + 2 * MAX_LIGHTS)], 0.5);
        // a camera at the origin looking along +X (game) through a plain perspective: a light 10 m ahead
        let camera = crate::Camera { right: Vec3::X, up: Vec3::Y, forward: Vec3::Z, position: Vec3::ZERO, fov: 1.0, aspect: 16.0 / 9.0, near: 0.05, far: 1000.0 };
        let clip = crate::math::clip_from_game(&camera, Vec3::ZERO);
        let ahead = PassLight { position: Vec3::new(1000.0, 0.0, 0.0), radius: 100.0, ..l };
        let (x0, y0, x1, y1) = screen_box(&[ahead], &clip, 1280, 720, 0.5f32.tan()).unwrap();
        assert!(x0 > 400 && x1 < 880 && y0 > 150 && y1 < 570 && x0 < 640 && x1 > 640, "{x0} {y0} {x1} {y1}");
        // around the camera: the whole target; behind it: the whole target (it may light what is in view)
        let around = PassLight { position: Vec3::new(50.0, 0.0, 0.0), radius: 300.0, ..l };
        assert_eq!(screen_box(&[around], &clip, 1280, 720, 0.5f32.tan()), Some((0, 0, 1280, 720)));
        // far off to the side: nothing
        let aside = PassLight { position: Vec3::new(1000.0, 5000.0, 0.0), radius: 100.0, ..l };
        assert_eq!(screen_box(&[aside], &clip, 1280, 720, 0.5f32.tan()), None);
    }

    // (IMPL_ER44) THE LIGHTS ON SORA
    fn light(position: Vec3, color: Vec3, hero: bool) -> PassLight {
        PassLight { position, radius: 550.0, color, exponent: 2.0, wrap: 0.0, on_sora: 0.35, sora: 1.0, sora_ambient: 0.1, placed: hero, body_shadow: 0.0 }
    }

    #[test]
    fn the_hero_light_on_sora_sits_above_him_toward_the_camera_and_gives_his_chest_what_the_real_one_gives() {
        let feet = Vec3::new(1000.0, -200.0, 50.0);
        let hero = light(feet + Vec3::Z * 150.0, Vec3::new(0.82, 0.9, 1.0) * 0.01, true);
        let camera = feet + Vec3::new(-400.0, 300.0, 250.0);
        let on = sora_lights(&[hero], feet, camera, 200.0, 120.0);
        assert_eq!(on.len(), 1);
        let toward = Vec3::new(-0.8, 0.6, 0.0);
        assert!((on[0].position - (feet + Vec3::Z * 200.0 + toward * 120.0)).length() < 1.0e-3, "{:?}", on[0].position);
        // at his chest: the same light as the real one gives there
        let chest = feet + Vec3::Z * 100.0;
        let real = hero.color * radial(hero.position.distance_squared(chest), 550.0, 2.0);
        let placed = on[0].color * radial(on[0].position.distance_squared(chest), 550.0, 2.0);
        assert!((real - placed).length() < 1.0e-7, "{real} {placed}");
        // its strength on him scales it; 0 = none
        assert!((sora_lights(&[PassLight { sora: 0.5, ..hero }], feet, camera, 200.0, 120.0)[0].color - on[0].color * 0.5).length() < 1.0e-7);
        assert!(sora_lights(&[PassLight { sora: 0.0, ..hero }], feet, camera, 200.0, 120.0).is_empty());
        // a camera straight above him: the light straight above him (no direction toward it)
        let above = sora_lights(&[hero], feet, feet + Vec3::Z * 600.0, 200.0, 120.0);
        assert!((above[0].position - (feet + Vec3::Z * 200.0)).length() < 1.0e-3);
    }

    #[test]
    fn effect_lights_on_sora_stay_where_they_are_reach_him_or_are_left_out_and_the_strongest_come_first() {
        let feet = Vec3::ZERO;
        let near = light(Vec3::new(150.0, 0.0, 120.0), Vec3::new(3.0, 1.5, 0.3), false);
        let faint = light(Vec3::new(-100.0, 0.0, 100.0), Vec3::new(0.3, 0.15, 0.03), false);
        let far = PassLight { radius: 300.0, ..light(Vec3::new(2000.0, 0.0, 100.0), Vec3::ONE * 50.0, false) };
        let on = sora_lights(&[faint, far, near], feet, Vec3::new(-500.0, 0.0, 200.0), 200.0, 120.0);
        assert_eq!(on.len(), 2);
        assert_eq!((on[0].position, on[1].position), (near.position, faint.position));
        assert_eq!(on[0].color, near.color);
        // never more than the shader holds ((IMPL_ER86) with the steady light the last one kept fades as the first one left
        // out comes near it: ten lights of nearly one strength keep five at full and the sixth faded or left out)
        let many: Vec<PassLight> = (0..10).map(|k| light(Vec3::new(50.0 + k as f32, 0.0, 100.0), Vec3::ONE, false)).collect();
        let kept = sora_lights(&many, feet, Vec3::X * 500.0, 200.0, 120.0);
        assert!(kept.len() >= MAX_ON_SORA - 1 && kept.len() <= MAX_ON_SORA);
    }

    #[test]
    fn the_block_on_sora_is_in_kh3_light_units_and_empty_means_his_old_look() {
        let l = SoraLight { position: Vec3::new(1.0, 2.0, 3.0), radius: 400.0, color: Vec3::new(0.5, 0.25, 0.125), exponent: 3.0 };
        let b = sora_block(&[l], 0.5);
        assert_eq!(b.len(), SORA_BLOCK_FLOATS);
        assert_eq!(&b[..4], &[1.0, 0.0, 0.0, 0.0]);
        assert_eq!(&b[4..8], &[1.0, 2.0, 3.0, 400.0]);
        let pi = std::f32::consts::PI;
        let c = &b[4 * (1 + MAX_ON_SORA)..4 * (2 + MAX_ON_SORA)];
        // a white Lambert surface facing it at its centre: colour / pi x out_scale = the pass's colour
        assert!((c[0] / pi * 0.5 - 0.5).abs() < 1.0e-6 && (c[1] / pi * 0.5 - 0.25).abs() < 1.0e-6 && c[3] == 3.0);
        // no lights, or no usable scale: all zero (the count 0 leaves his shading as without the block)
        assert!(sora_block(&[], 0.5).iter().all(|v| *v == 0.0));
        assert!(sora_block(&[l], 0.0).iter().all(|v| *v == 0.0));
        assert_eq!(sora_block(&[l; 9], 1.0)[0], MAX_ON_SORA as f32);
    }
}
