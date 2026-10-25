//! (IMPL_ER86) SORA'S LIGHT, WORLD-ANCHORED AND STEADY (build/sora_tmp/er/IMPL_ER86_STATE.md).
//!
//! The user, on build 239039F07FD4: "the way we are handling lighting on Sora from the Elden Ring world is VERY unstable,
//! his lighting changes when the camera moves. We need to fix that and make it stable."
//!
//! What moved with the camera (all measured offline in examples/light_orbit.rs; numbers in the state note):
//! 1. THE PROBES AT HIS FEET (light.rs, er_char.wgsl `probe_ps`): a 3 x 3 grid of taps in SCREEN space around the pixel of
//!    his feet, spread 35 cm in the picture plane. On the ground that grid lies up to a metre toward / away from the camera
//!    (35 cm / sin(pitch)), and its top row falls on his own legs (in the game's targets since his early depth, IMPL_ER53,
//!    shaded with whatever G-buffer lay behind them: a wall, far ground, the sky). So the ground the rig measured - its key
//!    shadow (`mask.w`), its ambient and the hero light's darkness gate - turned with the camera.
//!    FIX: probe3.ps.dxil (`ER_PROBE_WORLD`): 16 taps on the ground in fixed WORLD directions around his feet (radius
//!    `r.probe_radius_cm`), each kept only where the scene depth without him shows that ground and his body capsules do not
//!    hide it from the camera (`world_taps`, `hidden_by`: the shader's code mirrored here and unit-tested).
//! 2. THE REFLECTION ENVIRONMENT'S LEVEL (light.rs `env_level`): the brightest of 18 taps along the TOP OF THE PICTURE.
//!    Looking down at sunlit ground made his metal four times brighter than looking at the sky. FIX: the game's sky of the
//!    hour (its fog colours, world constants) with a floor of the clear grey those taps read where the sky is
//!    (`r.env_floor`, 0.014), and the world probes for the ground (`env_view`); `r.env_screen = 1` = the taps again.
//! 3. THE GAME'S EXPOSURE (SC_SceneExposure: its eye adaptation, which follows what the camera looks at): the hero light and
//!    the effects' lights are given in display units over it, and the darkness gate compares display levels. FIX: the
//!    exposure is eased in the log domain (`r.exposure_tau_s`, 2 s) before any of that uses it.
//! 4. THE DARKNESS GATE (pointlight.rs `DarkGate`): hysteresis only; a reading that crossed a threshold for one frame
//!    switched the hero light (and drew the Keyblade). FIX: a crossing must hold `hero.gate_hold_s` (1.5 s) (`Gate`).
//! 5. THE HERO LIGHT ON HIM (pointlight.rs `sora_lights`, IMPL_ER64 `hero.sora_placed`): placed 120 cm TOWARD THE CAMERA,
//!    so it orbited with the camera and lit whatever side was seen. FIX: `hero.anchor` 1 = in front of HIS FACING (eased,
//!    `hero.anchor_tau_s`), `hero.anchor_cm` forward, as a lantern he carries; 2 = straight above him; 0 = the camera (old).
//! 6. FRAMES WITHOUT THE GAME'S LIGHT CONSTANTS (no directional light pass recognised, or the scene row not believed): the
//!    key light vanished for that frame. FIX: the last sun / scene constants are kept `r.light_hold_s` (2 s).
//! 7. THE CUT AT SIX LIGHTS ON HIM (`sora_lights`) and at twelve effect lights in the pass (`rank`): hard, so two lights
//!    swapping places popped. FIX: the last lights kept fade out as the first one dropped catches up (`soft_cut`), and the
//!    pass ranks the effects' lights by what they give HIM (`rank_near`).
//! Everything the probes give is eased with a time constant that depends on HIS speed: `r.light_tau_still_s` (1 s) while he
//! stands (then only the camera moves), `r.light_tau_s` (0.3 s) from `r.light_move_cms` (120 cm/s) on: walking into the
//! shade still shows within a few tenths of a second.
//!
//! `r.light_stable = 0`: every one of these back to the behaviour before (the probes' old easing, the camera-placed hero
//! light, the screen's environment taps, the raw exposure, the gate without its hold, the hard cuts).

use std::sync::Mutex;
use std::time::Instant;

use glam::{Vec3, Vec4};

use crate::light::GameLight;

/// The keys (kh-er frame.rs hands them over once a second; absent = these defaults).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// `r.light_stable`: the whole of this module (false = the behaviour before IMPL_ER86).
    pub on: bool,
    /// `r.probe_world`: the probes on the ground in world directions (probe3.ps.dxil); false = the screen grid.
    pub probe_world: bool,
    /// `r.probe_radius_cm`: their distance from his feet.
    pub probe_radius: f32,
    /// `r.light_tau_s`: the time constant (s) of the probes while he moves; `r.light_tau_still_s` while he stands;
    /// `r.light_move_cms`: the speed (cm/s) from which the moving one holds.
    pub tau_move: f32,
    pub tau_still: f32,
    pub move_speed: f32,
    /// `r.exposure_tau_s`: the game's exposure, eased (log domain).
    pub tau_exposure: f32,
    /// `r.env_tau_s`: the sky / environment values, eased.
    pub tau_env: f32,
    /// `r.env_screen`: the top-of-the-picture taps feed the reflection environment again (as before).
    pub env_screen: bool,
    /// `r.env_floor`: the sky level the environment never goes below (the clear grey the picture's top shows where the sky is).
    pub env_floor: f32,
    /// `r.light_hold_s`: how long the last directional light / scene constants are kept through frames without them.
    pub hold: f32,
    /// `hero.gate_hold_s`: how long a reading must stay across a threshold before the darkness gate follows it.
    pub gate_hold: f32,
    /// `hero.anchor`: where the hero light sits for him: 0 toward the camera (before), 1 in front of his facing, 2 above.
    pub hero_anchor: u8,
    /// `hero.anchor_cm`: how far in front of him (anchor 1).
    pub hero_forward: f32,
    /// `hero.anchor_tau_s`: his facing, eased for it.
    pub tau_facing: f32,
    /// `fx.soft_cut`: the lights at the cut fade instead of popping.
    pub soft_cut: bool,
    /// `fx.rank_near`: the pass keeps the effects' lights that give HIM the most.
    pub rank_near: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            on: true,
            probe_world: true,
            probe_radius: 65.0,
            tau_move: 0.3,
            tau_still: 1.0,
            move_speed: 120.0,
            tau_exposure: 2.0,
            tau_env: 1.0,
            env_screen: false,
            env_floor: 0.014,
            hold: 2.0,
            gate_hold: 1.5,
            hero_anchor: 1,
            hero_forward: 60.0,
            tau_facing: 0.4,
            soft_cut: true,
            rank_near: true,
        }
    }
}

static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);

pub fn set_settings(s: Settings) {
    if let Ok(mut guard) = SETTINGS.lock() {
        if guard.as_ref() != Some(&s) {
            crate::rlog!("render: (IMPL_ER86) steady light settings {s:?}");
        }
        *guard = Some(s);
    }
}

pub fn settings() -> Settings {
    SETTINGS.lock().ok().and_then(|g| *g).unwrap_or_default()
}

/// The time-constant blend of one step: `1 - exp(-dt / tau)` (frame-rate independent; 1 for tau 0).
pub fn blend(dt: f32, tau: f32) -> f32 {
    if !(tau > 1.0e-4) {
        return 1.0;
    }
    (1.0 - (-dt.max(0.0) / tau).exp()).clamp(0.0, 1.0)
}

fn luma(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

// ---------------------------------------------------------------------------- the old easing (r.light_stable = 0)

/// The probes' easing of the renderer before IMPL_ER86 (gfx.rs, kept as it was): 8 % a frame toward the new probes,
/// the last good ones while none, the environment taps 4 % a frame. `before` = the light of the last frame.
pub fn ease_old(before: Option<&GameLight>, mut now: GameLight) -> GameLight {
    if let Some(before) = before.filter(|b| b.probed()) {
        if !now.probed() {
            (now.scene_px, now.direct_px, now.albedo_px, now.mask, now.taps) = (before.scene_px, before.direct_px, before.albedo_px, before.mask, before.taps);
        } else {
            let ease = 0.08;
            now.scene_px = before.scene_px.lerp(now.scene_px, ease);
            now.direct_px = before.direct_px.lerp(now.direct_px, ease);
            now.albedo_px = before.albedo_px.lerp(now.albedo_px, ease);
            now.mask = before.mask.lerp(now.mask, ease);
        }
        if before.env_taps >= 1.0 {
            now.env_px = if now.env_taps >= 1.0 { before.env_px.lerp(now.env_px, 0.04) } else { before.env_px };
            now.env_taps = now.env_taps.max(before.env_taps);
        }
    }
    now
}

// ---------------------------------------------------------------------------- the filter

/// THE STEADY GAME LIGHT: the light of each frame as read back (raw) in, the light his rig, the gate, the lights and the
/// environment use out. Explicit `dt` (seconds since the last step): pure and testable; `global_step` keeps one for the
/// renderer.
#[derive(Clone, Copy, Debug, Default)]
pub struct Filter {
    pub light: Option<GameLight>,
    sun_missing: f32,
    scene_missing: f32,
    feet: Option<Vec3>,
    /// His speed (cm/s), eased over 0.15 s.
    pub speed: f32,
}

impl Filter {
    /// The time constant of the probes for his current speed.
    pub fn tau(&self, s: &Settings) -> f32 {
        let k = if s.move_speed > 0.0 { (self.speed / s.move_speed).clamp(0.0, 1.0) } else { 1.0 };
        s.tau_still + (s.tau_move - s.tau_still) * k
    }

    pub fn step(&mut self, raw: Option<&GameLight>, feet: Option<Vec3>, dt: f32, s: &Settings) -> Option<GameLight> {
        let dt = if dt.is_finite() { dt.clamp(0.0, 0.25) } else { 0.0 };
        // his speed (a teleport or a rebase of more than 3 m in a step is not a speed)
        let v = match (self.feet, feet) {
            (Some(a), Some(b)) if dt > 1.0e-4 && a.distance(b) < 300.0 => a.distance(b) / dt,
            _ => 0.0,
        };
        self.speed += (v - self.speed) * blend(dt, 0.15);
        self.feet = feet;
        let Some(raw) = raw else {
            self.light = None;
            return None;
        };
        let Some(prev) = self.light else {
            self.light = Some(*raw);
            return self.light;
        };
        let mut out = *raw;
        let a = blend(dt, self.tau(s));
        let a_move = blend(dt, s.tau_move);
        let a_env = blend(dt, s.tau_env);
        // the probes (the ground around his feet): eased; while none (his feet hidden), the last ones
        if raw.probed() && prev.probed() {
            out.scene_px = prev.scene_px.lerp(raw.scene_px, a);
            out.direct_px = prev.direct_px.lerp(raw.direct_px, a);
            out.albedo_px = prev.albedo_px.lerp(raw.albedo_px, a);
            out.mask = prev.mask.lerp(raw.mask, a);
        } else if !raw.probed() && prev.probed() {
            (out.scene_px, out.direct_px, out.albedo_px, out.mask, out.taps) = (prev.scene_px, prev.direct_px, prev.albedo_px, prev.mask, prev.taps);
        }
        // the picture's top (only the fallback of the sky without constants, or with r.env_screen)
        if prev.env_taps >= 1.0 {
            out.env_px = if raw.env_taps >= 1.0 { prev.env_px.lerp(raw.env_px, a_env) } else { prev.env_px };
            out.env_taps = raw.env_taps.max(prev.env_taps);
        }
        // the directional light: a world value, eased as a world change; kept through frames without it
        match (raw.sun, prev.sun) {
            (Some((d, c)), Some((pd, pc))) => {
                self.sun_missing = 0.0;
                out.sun = Some((pd.lerp(d, a_move).normalize_or(d), pc.lerp(c, a_move)));
                out.sky = (prev.sky.0.lerp(raw.sky.0, a_move).normalize_or(raw.sky.0), prev.sky.1.lerp(raw.sky.1, a_move));
                out.shadow_color = prev.shadow_color.lerp(raw.shadow_color, a_move);
            }
            (None, Some(_)) => {
                self.sun_missing += dt;
                if self.sun_missing <= s.hold {
                    (out.sun, out.sky, out.shadow_color) = (prev.sun, prev.sky, prev.shadow_color);
                }
            }
            _ => self.sun_missing = 0.0,
        }
        // the scene constants: the exposure eased in the log domain, the fog colours as the sky; kept through frames without
        if raw.scene && prev.scene {
            self.scene_missing = 0.0;
            if raw.exposure > 1.0e-3 && prev.exposure > 1.0e-3 && raw.exposure.is_finite() {
                out.exposure = (prev.exposure.ln() + (raw.exposure.ln() - prev.exposure.ln()) * blend(dt, s.tau_exposure)).exp();
            }
            for k in 0..3 {
                out.fog[k] = prev.fog[k].lerp(raw.fog[k], a_env);
            }
        } else if !raw.scene && prev.scene {
            self.scene_missing += dt;
            if self.scene_missing <= s.hold {
                (out.scene, out.exposure, out.pre_exposure, out.fog) = (true, prev.exposure, prev.pre_exposure, prev.fog);
            }
        } else {
            self.scene_missing = 0.0;
        }
        self.light = Some(out);
        self.light
    }
}

/// THE DARKNESS GATE'S HOLD: the reading must stay across a threshold for `hold` seconds before the gate follows it. Out:
/// the reading the gate is stepped with (clamped below `on_below` while dark, above `off_above` while light; `None` = no
/// reading: the gate holds).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gate {
    pub dark: bool,
    primed: bool,
    pending: f32,
}

impl Gate {
    pub fn step(&mut self, level: Option<f32>, dt: f32, on_below: f32, off_above: f32, hold: f32) -> Option<f32> {
        let level = level.filter(|l| l.is_finite())?;
        let want = if self.dark { level <= off_above.max(on_below) } else { level < on_below };
        if !self.primed {
            (self.primed, self.dark, self.pending) = (true, want, 0.0);
        } else if want != self.dark {
            self.pending += dt.clamp(0.0, 0.25);
            if self.pending >= hold {
                (self.dark, self.pending) = (want, 0.0);
            }
        } else {
            self.pending = 0.0;
        }
        // (the reading itself, clamped to the side decided: the gate and its log line see a real level)
        Some(if self.dark { level.min(on_below * 0.999) } else { level.max(off_above.max(on_below) * 1.001 + 1.0e-6) })
    }
}

// ---------------------------------------------------------------------------- the world probes (probe3.ps.dxil, mirrored)

/// Taps of the world probe (er_char.wgsl `ER_PROBE_WORLD`).
pub const WORLD_TAPS: usize = 16;
/// How far (cm) the ground the depth shows may lie from the tap for the tap to count.
pub const WORLD_TOLERANCE: f32 = 30.0;
/// The margin (cm) around his capsules within which the line from the camera to a tap counts as hidden.
pub const BODY_MARGIN: f32 = 4.0;

/// The taps: `WORLD_TAPS` points on a circle of `radius` cm around his feet at their height, in fixed WORLD directions
/// (every 22.5 degrees, starting at 11.25), game space.
pub fn world_taps(feet: Vec3, radius: f32) -> [Vec3; WORLD_TAPS] {
    std::array::from_fn(|k| {
        let a = (k as f32 * 22.5 + 11.25).to_radians();
        feet + Vec3::new(a.cos(), a.sin(), 0.0) * radius
    })
}

/// The closest distance between the segments p1 q1 and p2 q2 (the shader's `er_seg_dist`, Ericson's method).
pub fn seg_dist(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> f32 {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let (c, b) = (d1.dot(r), d1.dot(d2));
    let denom = a * e - b * b;
    let mut s = if denom > 1.0e-6 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
    let mut t = (b * s + f) / e.max(1.0e-6);
    if t < 0.0 {
        t = 0.0;
        s = (-c / a.max(1.0e-6)).clamp(0.0, 1.0);
    } else if t > 1.0 {
        t = 1.0;
        s = ((b - c) / a.max(1.0e-6)).clamp(0.0, 1.0);
    }
    ((p1 + d1 * s) - (p2 + d2 * t)).length()
}

/// Whether his body (the frame's capsules: `caps[2k]` = end a + radius, `caps[2k + 1]` = end b, w > 0.5 when there) hides
/// the point `p` from the camera.
pub fn hidden_by(caps: &[Vec4], camera: Vec3, p: Vec3) -> bool {
    caps.chunks_exact(2).any(|c| c[1].w > 0.5 && seg_dist(camera, p, c[0].truncate(), c[1].truncate()) < c[0].w + BODY_MARGIN)
}

/// One tap of the world probe as the shader wrote it (texels 64 + k: direct light x surface colour, 80 + k: surface colour,
/// 96 + k: scene colour, 112 + k: shadow mask; w of the first = 1 when the tap counted this frame).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tap {
    pub direct: Vec3,
    pub albedo: Vec3,
    pub scene: Vec3,
    pub mask: Vec4,
}

/// The first texel of the world probe's taps in the probe row.
pub const TAP_TEXELS: usize = 64;

/// The taps of a frame from the game-data texture's floats (light.rs layout: rows of `light::WIDTH` texels).
pub fn parse_taps(words: &[f32]) -> [Option<Tap>; WORLD_TAPS] {
    let texel = |x: usize| {
        let at = (crate::light::ROW_PROBE * crate::light::WIDTH + x) * 4;
        words.get(at..at + 4).map_or(Vec4::ZERO, |w| Vec4::new(w[0], w[1], w[2], w[3]))
    };
    std::array::from_fn(|k| {
        let d = texel(TAP_TEXELS + k);
        let tap = Tap { direct: d.truncate(), albedo: texel(TAP_TEXELS + 16 + k).truncate(), scene: texel(TAP_TEXELS + 32 + k).truncate(), mask: texel(TAP_TEXELS + 48 + k) };
        let finite = tap.direct.is_finite() && tap.albedo.is_finite() && tap.scene.is_finite() && tap.mask.is_finite();
        (d.w > 0.5 && finite && tap.albedo.min_element() > crate::light::DARK).then_some(tap)
    })
}

/// THE TAPS' MEMORY: each world direction keeps the last value it showed while it is out of sight this frame (behind his
/// body, outside the picture, behind a wall), as long as he has not moved more than `MEMORY_MOVE` cm since and it is not
/// older than `MEMORY_AGE` s. The means are taken over every direction remembered, so the set does not change with the
/// camera: the ground is the same ground whichever of it the camera sees now.
#[derive(Clone, Copy, Debug, Default)]
pub struct TapMemory {
    taps: [Option<(Tap, Vec3, f32)>; WORLD_TAPS],
}

pub const MEMORY_MOVE: f32 = 25.0;
pub const MEMORY_AGE: f32 = 8.0;

impl TapMemory {
    pub fn update(&mut self, now: &[Option<Tap>; WORLD_TAPS], feet: Vec3, dt: f32) {
        for (slot, tap) in self.taps.iter_mut().zip(now) {
            *slot = match (tap, *slot) {
                (Some(t), _) => Some((*t, feet, 0.0)),
                (None, Some((t, at, age))) if at.distance(feet) <= MEMORY_MOVE && age + dt <= MEMORY_AGE => Some((t, at, age + dt.max(0.0))),
                _ => None,
            };
        }
    }

    /// Means over the directions remembered: (scene, direct, surface colour, mask, how many); `None` with fewer than 4.
    pub fn means(&self) -> Option<(Vec3, Vec3, Vec3, Vec4, f32)> {
        let kept: Vec<Tap> = self.taps.iter().flatten().map(|(t, ..)| *t).collect();
        if kept.len() < 4 {
            return None;
        }
        let n = kept.len() as f32;
        let sum = kept.iter().fold((Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec4::ZERO), |a, t| (a.0 + t.scene, a.1 + t.direct, a.2 + t.albedo, a.3 + t.mask));
        Some((sum.0 / n, sum.1 / n, sum.2 / n, sum.3 / n, n))
    }

    /// The light of a frame with its probes from the memory (unchanged without enough of it).
    pub fn apply(&self, raw: &GameLight) -> GameLight {
        let mut out = *raw;
        if let Some((scene, direct, albedo, mask, n)) = self.means() {
            (out.scene_px, out.direct_px, out.albedo_px, out.mask, out.taps) = (scene, direct, albedo, mask, n);
        }
        out
    }
}

// ---------------------------------------------------------------------------- the renderer's state

struct State {
    filter: Filter,
    gate: Gate,
    at: Option<Instant>,
    gate_at: Option<Instant>,
    facing: Option<Vec3>,
    facing_at: Option<Instant>,
    memory: TapMemory,
    memory_at: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State { filter: Filter { light: None, sun_missing: 0.0, scene_missing: 0.0, feet: None, speed: 0.0 }, gate: Gate { dark: false, primed: false, pending: 0.0 }, at: None, gate_at: None, facing: None, facing_at: None, memory: TapMemory { taps: [None; WORLD_TAPS] }, memory_at: None });

/// (gfx.rs, at the read-back of the game-data texture) The world probe's taps of that frame into the memory.
pub(crate) fn read_taps(words: &[f32], feet: Option<Vec3>) {
    let Some(feet) = feet else { return };
    let taps = parse_taps(words);
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let dt = st.memory_at.map_or(0.0, |t| t.elapsed().as_secs_f32());
    st.memory_at = Some(Instant::now());
    st.memory.update(&taps, feet, dt);
}

/// (gfx.rs, once a frame) The steady light of this frame from the raw one (`None`: no game light this frame).
pub(crate) fn global_step(raw: Option<&GameLight>, feet: Option<Vec3>) -> Option<GameLight> {
    let s = settings();
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let dt = st.at.map_or(0.0, |t| t.elapsed().as_secs_f32());
    st.at = Some(Instant::now());
    // (the world probe's memory, when it was read in the last half second: the probes over every direction remembered)
    let fresh = s.probe_world && st.memory_at.is_some_and(|t| t.elapsed().as_secs_f32() < 0.5);
    let raw = raw.map(|r| if fresh { st.memory.apply(r) } else { *r });
    st.filter.step(raw.as_ref(), feet, dt, &s)
}

/// (gfx.rs) The reading the darkness gate is stepped with this frame (the hold applied).
pub(crate) fn gate_level(level: Option<f32>, on_below: f32, off_above: f32) -> Option<f32> {
    let s = settings();
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let dt = st.gate_at.map_or(0.0, |t| t.elapsed().as_secs_f32());
    st.gate_at = Some(Instant::now());
    st.gate.step(level, dt, on_below, off_above, s.gate_hold)
}

/// (gfx.rs) The point the hero light on him is placed toward (pointlight.rs `sora_lights`' `camera`), and how far: the camera
/// (anchor 0), a point ahead of his eased facing (1) or his feet (2: straight above him). `facing`: his forward axis (game space).
pub(crate) fn hero_toward(camera: Vec3, feet: Vec3, facing: Option<Vec3>, toward_before: f32) -> (Vec3, f32) {
    let s = settings();
    if !s.on || s.hero_anchor == 0 {
        return (camera, toward_before);
    }
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let dt = st.facing_at.map_or(0.0, |t| t.elapsed().as_secs_f32()).clamp(0.0, 0.25);
    st.facing_at = Some(Instant::now());
    let level = facing.map(|f| Vec3::new(f.x, f.y, 0.0)).and_then(|f| f.try_normalize());
    st.facing = match (st.facing, level) {
        (Some(before), Some(now)) => Some(before.lerp(now, blend(dt, s.tau_facing)).try_normalize().unwrap_or(now)),
        (None, now) => now,
        (before, None) => before,
    };
    match (s.hero_anchor, st.facing) {
        (1, Some(f)) => (feet + f * 1000.0, s.hero_forward),
        _ => (feet, 0.0),
    }
}

/// (gfx.rs) The game light the reflection environment's level and hue are made from: the picture's top taps replaced by
/// the floor (`r.env_floor`) unless `r.env_screen`, so that the sky is the game's own of the hour (light.rs `sky_estimate`
/// takes its fog colours first) and never below the clear grey.
pub fn env_view(l: &GameLight, s: &Settings) -> GameLight {
    let mut out = *l;
    if !s.env_screen {
        (out.env_px, out.env_taps) = (Vec3::splat(s.env_floor.max(0.0)), 1.0);
    } else if l.env_taps >= 1.0 {
        out.env_px = l.env_px.max(Vec3::splat(s.env_floor.max(0.0)));
    }
    out
}

/// (pointlight.rs) The lights kept at a cut, each with the factor it is drawn with: sorted by `weight` (largest first), the
/// first `max`; with `soft`, the LAST one kept fades out as the weight of the first one left out comes near its own (it has
/// factor 0 at the moment the two would swap places: no pop; the others are whole, so many lights of one strength keep
/// `max - 1` of them at full).
pub fn soft_cut<T: Copy>(mut weighted: Vec<(f32, T)>, max: usize, soft: bool) -> Vec<(f32, T)> {
    weighted.sort_by(|a, b| b.0.total_cmp(&a.0));
    let next = weighted.get(max).map_or(0.0, |w| w.0);
    weighted.truncate(max);
    let last = weighted.len().saturating_sub(1);
    weighted
        .into_iter()
        .enumerate()
        .map(|(k, (w, t))| {
            let f = if soft && k == last && next > 0.0 && w > 0.0 { ((w - next) / (0.3 * w)).clamp(0.0, 1.0) } else { 1.0 };
            let f = f * f * (3.0 - 2.0 * f);
            (f, t)
        })
        .filter(|(f, _)| *f > 1.0e-4)
        .collect()
}

/// (pointlight.rs `plan`) The effects' lights the pass keeps when there are more than `max`: by what they give his chest
/// (his feet + 100 cm) plus a share of their own size (so far bright ones still light the world), softly cut.
pub fn rank_near(lights: &[kh_vfx::Light], max: usize, feet: Vec3) -> Vec<(f32, kh_vfx::Light)> {
    let chest = feet + Vec3::Z * 100.0;
    let weighted: Vec<(f32, kh_vfx::Light)> = lights
        .iter()
        .filter(|l| l.radius > 1.0 && l.position.is_finite() && l.color.is_finite() && l.color.max_element() > 1.0e-4)
        .map(|l| {
            let reach = crate::pointlight::radial(l.position.distance_squared(chest), l.radius, if l.exponent > 0.0 { l.exponent } else { crate::pointlight::FX_EXPONENT });
            (luma(l.color.max(Vec3::ZERO)).max(1.0e-6) * (reach + 0.05) * l.radius.min(2000.0), *l)
        })
        .collect();
    soft_cut(weighted, max, settings().soft_cut)
}

/// (log) One line for the status block.
pub fn status() -> String {
    let s = settings();
    if !s.on {
        return "(IMPL_ER86) steady light OFF (r.light_stable = 0): the light before IMPL_ER86".to_string();
    }
    let st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    format!(
        "(IMPL_ER86) steady light on: probes {} r {:.0} cm, tau {:.2} s (his speed {:.0} cm/s), exposure tau {:.1} s, env {}, gate hold {:.1} s (dark {}), hero anchor {}",
        if s.probe_world { "WORLD" } else { "screen" },
        s.probe_radius,
        st.filter.tau(&s),
        st.filter.speed,
        s.tau_exposure,
        if s.env_screen { "screen taps" } else { "sky constants" },
        s.gate_hold,
        st.gate.dark,
        ["camera", "his facing", "above"][s.hero_anchor.min(2) as usize]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(scene: f32, sun: Option<Vec3>) -> GameLight {
        GameLight {
            sun: sun.map(|d| (d, Vec3::splat(0.5))),
            sky: (Vec3::Y, Vec3::splat(0.05)),
            shadow_color: Vec3::ONE,
            scene: true,
            exposure: 10.0,
            pre_exposure: 1.0,
            scene_px: Vec3::splat(scene),
            direct_px: Vec3::splat(scene * 0.8),
            albedo_px: Vec3::splat(0.3),
            mask: Vec4::ONE,
            taps: 16.0,
            ..GameLight::default()
        }
    }

    #[test]
    fn the_blend_is_frame_rate_independent() {
        // two steps of 1/120 s give what one step of 1/60 s gives
        let one = blend(1.0 / 60.0, 0.3);
        let two = 1.0 - (1.0 - blend(1.0 / 120.0, 0.3)).powi(2);
        assert!((one - two).abs() < 1.0e-6);
        assert_eq!(blend(0.1, 0.0), 1.0);
    }

    #[test]
    fn standing_still_eases_slowly_and_moving_follows_quickly() {
        let s = Settings::default();
        let (dark, bright) = (lit(0.01, Some(Vec3::Y)), lit(0.05, Some(Vec3::Y)));
        let mut still = Filter::default();
        still.step(Some(&dark), Some(Vec3::ZERO), 0.0, &s);
        let mut moving = still;
        let mut at = Vec3::ZERO;
        let (mut a, mut b) = (dark, dark);
        for _ in 0..18 {
            a = still.step(Some(&bright), Some(Vec3::ZERO), 1.0 / 60.0, &s).unwrap();
            at += Vec3::X * 4.0; // 240 cm/s
            b = moving.step(Some(&bright), Some(at), 1.0 / 60.0, &s).unwrap();
        }
        // after 0.3 s: standing ~26 % of the way (tau 1 s), walking well over half (tau 0.3 s once his speed shows)
        let (fa, fb) = ((a.scene_px.x - 0.01) / 0.04, (b.scene_px.x - 0.01) / 0.04);
        assert!((0.2..0.32).contains(&fa), "{fa}");
        assert!(fb > 0.5 && fb < 0.7, "{fb}");
    }

    #[test]
    fn the_sun_and_the_scene_are_kept_through_frames_without_them() {
        let s = Settings::default();
        let mut f = Filter::default();
        f.step(Some(&lit(0.02, Some(Vec3::Y))), None, 0.0, &s);
        let mut gap = lit(0.02, None);
        gap.scene = false;
        let held = f.step(Some(&gap), None, 1.0 / 60.0, &s).unwrap();
        assert!(held.sun.is_some() && held.scene && held.exposure == 10.0);
        for _ in 0..200 {
            f.step(Some(&gap), None, 1.0 / 60.0, &s);
        }
        let gone = f.light.unwrap();
        assert!(gone.sun.is_none() && !gone.scene, "after the hold the frame's own values");
    }

    #[test]
    fn the_exposure_is_eased_in_the_log_domain() {
        let s = Settings::default();
        let mut f = Filter::default();
        f.step(Some(&lit(0.02, Some(Vec3::Y))), None, 0.0, &s);
        let mut l = lit(0.02, Some(Vec3::Y));
        l.exposure = 40.0;
        let one = f.step(Some(&l), None, 2.0 * 0.25, &s).unwrap();
        // two steps of 0.25 s against tau 2 s: 22 % of the way in log: 10 * 4^0.221
        assert!((one.exposure - 10.0 * 4f32.powf(1.0 - (-0.25f32 / 2.0).exp())).abs() < 0.05, "{}", one.exposure);
    }

    #[test]
    fn the_gate_needs_the_reading_to_hold() {
        let mut g = Gate::default();
        assert_eq!(g.step(Some(0.5), 0.0, 0.06, 0.10, 1.5), Some(0.5));
        // a reading between the thresholds while light is handed on above the upper one
        assert!(g.step(Some(0.08), 0.0, 0.06, 0.10, 1.5).unwrap() > 0.10);
        // a dip of one second does not switch it
        for _ in 0..60 {
            g.step(Some(0.01), 1.0 / 60.0, 0.06, 0.10, 1.5);
        }
        assert!(!g.dark);
        g.step(Some(0.5), 1.0 / 60.0, 0.06, 0.10, 1.5);
        for _ in 0..100 {
            g.step(Some(0.01), 1.0 / 60.0, 0.06, 0.10, 1.5);
        }
        assert!(g.dark);
        // between the thresholds it stays dark; no reading: no step
        for _ in 0..200 {
            g.step(Some(0.08), 1.0 / 60.0, 0.06, 0.10, 1.5);
        }
        assert!(g.dark && g.step(None, 0.1, 0.06, 0.10, 1.5).is_none());
    }

    #[test]
    fn the_world_taps_and_his_body() {
        let taps = world_taps(Vec3::new(100.0, 0.0, 5.0), 65.0);
        assert!(taps.iter().all(|t| (t.truncate().distance(glam::Vec2::new(100.0, 0.0)) - 65.0).abs() < 1.0e-3 && t.z == 5.0));
        // one leg capsule at his feet; the camera behind him (-X): the tap in front (+X) is hidden, the one behind is not
        let caps = [Vec4::new(0.0, 0.0, 10.0, 9.0), Vec4::new(0.0, 0.0, 90.0, 1.0)];
        let camera = Vec3::new(-400.0, 0.0, 150.0);
        assert!(hidden_by(&caps, camera, Vec3::new(65.0, 0.0, 0.0)));
        assert!(!hidden_by(&caps, camera, Vec3::new(-65.0, 0.0, 0.0)));
        // a capsule that is not there hides nothing
        assert!(!hidden_by(&[caps[0], Vec4::new(0.0, 0.0, 90.0, 0.0)], camera, Vec3::new(65.0, 0.0, 0.0)));
        assert!((seg_dist(Vec3::ZERO, Vec3::X * 10.0, Vec3::new(5.0, 3.0, -1.0), Vec3::new(5.0, 3.0, 1.0)) - 3.0).abs() < 1.0e-5);
    }

    #[test]
    fn a_tap_out_of_sight_keeps_its_last_value_until_he_moves() {
        let tap = |v: f32| Some(Tap { direct: Vec3::splat(v), albedo: Vec3::splat(0.3), scene: Vec3::splat(v), mask: Vec4::ONE });
        let mut m = TapMemory::default();
        let mut now: [Option<Tap>; WORLD_TAPS] = std::array::from_fn(|k| tap(if k < 8 { 0.01 } else { 0.05 }));
        m.update(&now, Vec3::ZERO, 0.0);
        let all = m.means().unwrap();
        // the camera now sees only half of them: the means stay
        for t in now.iter_mut().skip(4).take(6) {
            *t = None;
        }
        m.update(&now, Vec3::ZERO, 0.1);
        assert_eq!(m.means().unwrap(), all);
        // he walks half a metre: the hidden ones are forgotten
        m.update(&now, Vec3::X * 50.0, 0.1);
        assert_eq!(m.means().unwrap().4, 10.0);
        // the texture's layout: the w of the direct texel says the tap counted
        let mut words = vec![0.0f32; crate::light::WIDTH * crate::light::HEIGHT * 4];
        let at = |x: usize| (crate::light::ROW_PROBE * crate::light::WIDTH + x) * 4;
        words[at(TAP_TEXELS + 3)..at(TAP_TEXELS + 3) + 4].copy_from_slice(&[0.02, 0.02, 0.02, 1.0]);
        words[at(TAP_TEXELS + 19)..at(TAP_TEXELS + 19) + 4].copy_from_slice(&[0.3, 0.3, 0.3, 1.0]);
        let p = parse_taps(&words);
        assert!(p[3].is_some() && p.iter().filter(|t| t.is_some()).count() == 1);
    }

    #[test]
    fn the_cut_fades_instead_of_popping() {
        let lights = |w6: f32| vec![(1.0, 0), (0.9, 1), (w6, 2), (0.5, 3)];
        // the third light far above the one left out: full; as they meet, it fades to nothing (the first two stay whole)
        let far = soft_cut(lights(0.8), 3, true);
        assert_eq!(far.len(), 3);
        assert!((far[2].0 - 1.0).abs() < 1.0e-6);
        let near = soft_cut(lights(0.52), 3, true);
        assert!(near.iter().find(|(_, i)| *i == 2).is_none_or(|(f, _)| *f < 0.05));
        assert!(near.iter().filter(|(_, i)| *i < 2).all(|(f, _)| *f == 1.0));
        // ten of one strength: the first max - 1 whole
        let same: Vec<(f32, usize)> = (0..10).map(|k| (1.0 - k as f32 * 1.0e-4, k)).collect();
        assert_eq!(soft_cut(same, 6, true).iter().filter(|(f, _)| *f == 1.0).count(), 5);
        // without soft: hard, as before
        assert!(soft_cut(lights(0.52), 3, false).iter().all(|(f, _)| *f == 1.0));
    }

    #[test]
    fn the_environment_takes_the_sky_constants_not_the_picture() {
        let s = Settings::default();
        let mut l = lit(0.02, Some(Vec3::Y));
        (l.env_px, l.env_taps) = (Vec3::splat(0.12), 1.0);
        let v = env_view(&l, &s);
        assert_eq!(v.env_px, Vec3::splat(0.014));
        let old = Settings { env_screen: true, ..s };
        assert_eq!(env_view(&l, &old).env_px, Vec3::splat(0.12));
    }
}
