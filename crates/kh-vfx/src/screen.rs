//! (VFXR) THE SCREEN-WIDE TRICKS AND THE CHARACTER-MATERIAL DRIVES of KH3's effects: the Square Enix modules that do not
//! draw particles but ask the renderer for a post-process pass (`SQEX_RadialBlur`, `SQEX_BloomRadialBlur`,
//! `SQEX_GaussBlur`, `SQEX_ScreenBlur`, `SQEX_Filter`, `SQEX_PostprocessFringe`, `SQEX_PostprocessAura`) or write a
//! material parameter of the character the effect belongs to (`SQEX_Material_InLight`, `_Aura`, `_Scalar`, `_Vector`).
//! The evaluator samples them at the emitter's time and hands them to the host in the draw list (`DrawList::screen`,
//! `DrawList::char_params`); the hosts run the passes (kh-app vfx_post.rs, kh-er-render vfxpost.rs).
//!
//! WHERE THE MATHS COMES FROM (evidence levels as in docs/sora-vfx-spec.md):
//! - [S] the game's own post-process shaders, disassembled from Engine/GlobalShaderCache-PCD3D_SM5.bin
//!   (build/sora_tmp/vfxr/gsc/*.asm): `FPostProcessSqexRadialBlurSetupPS` (a 2x2 box down-sample),
//!   `TPostProcessSqexRadialBlurPS<0|1>` (ten taps, weights 0.19 .. 0.01, step = (centre - uv) x power for <0> RADIAL,
//!   direction x power for <1> LINEAR), `FPostProcessSqexRadialBlurCompositePS` (the ring weight below, lerp to the blur),
//!   `FPostProcessSqexBloomRadialBlurSetupPS` / `CompositePS` (threshold / exposure subtracted, the blur ADDED x weight),
//!   `TPostProcessSqexScreenBlurPS<0|1|2>` (a displaced copy mixed by the ring weight; <2> mixes toward white),
//!   `TPostProcessSqexEffectFilterPS` (negative, grey and edge amounts).
//! - [A] the mapping of the module's properties onto those constants (the C++ that fills them was not traced):
//!   ring = (HoleSize, AttenuationDistanceInside, Range, AttenuationDistanceOutside); centre = (PosX, PosY) with
//!   bUseScreenPos, else the emitter projected; Alpha = the composite's weight; Power = the tap step; Angle (degrees)
//!   the LINEAR direction. Every value of the pack reads sensibly under it (a full-screen blur for Range 2, a hole that
//!   fades in for HoleSize / Inside, Zettaflare's faint streaks for Power 0.0002).
//! - [A] SQEX_Filter: FildterType ADD adds FilterColor x FilterAlpha, ALPHA mixes toward FilterColor by FilterAlpha
//!   (both on the displayed picture: UseAlphaFilterAtHDR is false on every user).
//! - (AUDIT-B W-VFX) SQEX_Filter GRAY / NPR, traced: the module's tick (0x1442EEC40) samples (FilterColor.xyz, FilterAlpha)
//!   at the emitter time (x FilterColorScaleAtHDR / FilterAlphaScaleAtHDR in HDR) and hands that one vector to the filter
//!   manager (vtable 0x147703A20) by its type (ALPHA 0, ADD 1, SUB 2, NPR 3, GRAY 4, HDR_ALPHA 5: slots +0x10 + 8 x type;
//!   GRAY 0x1443BE840 keeps one entry per component in the list of its type); 0x1443A9170 SUMS each list's vectors
//!   (NPR into manager+0x98, GRAY into +0xA8). The pass `PassEffectFilter` (0x1434409D0, TPostProcessSqexEffectFilterPS)
//!   sets the view's three filter rows (+0x860 / +0x870 / +0x880) into the shader, whose rows read: cb0[21] =
//!   (NPR: x != 0 spares the pixels of object class 7, -, -, w = the NEGATIVE amount), cb0[22] = (GRAY: x != 0 spares the
//!   pixels of object class 5, y = the edge colour k -> (k, 1.01 k, k), z = the EDGE amount, w = the GREY amount): grey =
//!   c + w (mean - c), negative = c + w (1 - 2 c) (each only when w > 0), the edge where the mean depth step to 8 neighbours
//!   2 px away is >= 1e-4 on pixels of class 0. So GRAY's FilterColor is (spare flag, edge colour, edge amount) and its
//!   alpha the grey amount (before: x was read as a negative amount, which turned Clock Drill's finish into a white
//!   negative of the scene).
//! - [A] SQEX_GaussBlur: Size is the blur radius in percent of the screen width (UE4's gaussian kernel convention),
//!   Intensity the mix toward it.

use glam::Vec3;
use serde_json::Value;

use crate::dist::Dist;
use crate::rng::Rng;

/// Which post-process pass a screen module asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ScreenKind {
    /// `SQEX_RadialBlur`: the picture blurred toward / along a direction, mixed in by the ring weight.
    RadialBlur,
    /// `SQEX_BloomRadialBlur`: what is brighter than a threshold, radially blurred and ADDED (light streaks).
    BloomRadialBlur,
    /// `SQEX_GaussBlur`: the whole picture softened.
    GaussBlur,
    /// `SQEX_ScreenBlur`: a displaced copy of the picture mixed in by the ring weight (`mode` 2: toward white).
    ScreenBlur,
    /// `SQEX_Filter`: a full-screen colour filter (`mode` 0 ADD, 1 ALPHA, 2 GRAY).
    Filter,
    /// `SQEX_PostprocessFringe`: colour fringes (chromatic aberration) toward the screen's edges.
    Fringe,
    /// `SQEX_PostprocessAura`: a glow around the characters' silhouettes.
    Aura,
}

/// `EffectPriority` (ScreenEffectPriority_Normal / Hi / Highest): a host that runs one pass of a kind takes the
/// highest priority, then the strongest.
fn priority_of(p: &Value) -> u8 {
    match p.get("EffectPriority").and_then(Value::as_str).unwrap_or("") {
        s if s.ends_with("_Highest") => 3,
        s if s.ends_with("_Hi") || s.ends_with("_High") => 2,
        s if s.ends_with("_Low") => 0,
        _ => 1,
    }
}

/// One screen module of an emitter, as authored.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenDef {
    pub kind: ScreenKind,
    pub priority: u8,
    /// `StartTime` / `EndTime` (emitter seconds) of modules that carry them; the pass runs inside.
    pub start: f32,
    pub end: f32,
    /// `bUseScreenPos`: the centre is (PosX, PosY) of the screen; else the emitter's place, projected.
    pub screen_pos: bool,
    pub pos: [Option<Dist>; 2],
    /// Radial / bloom radial: `Power`, `Angle` (degrees), `Alpha`; LINEAR mode.
    pub power: Option<Dist>,
    pub angle: Option<Dist>,
    pub alpha: Option<Dist>,
    pub linear: bool,
    /// The ring: `HoleSize`, `AttenuationDistanceInside`, `Range`, `AttenuationDistanceOutside`.
    pub ring: [Option<Dist>; 4],
    /// Bloom radial: `LuminanceThreshold`.
    pub threshold: Option<Dist>,
    /// Gauss: `Size`; aura: `Size`.
    pub size: Option<Dist>,
    /// Gauss / screen blur / fringe: `Intensity` (`ScreenBlurIntensity`).
    pub intensity: Option<Dist>,
    /// Screen blur: `ScreenBlurOffsetX` / `Y`.
    pub offset: [Option<Dist>; 2],
    /// Filter / aura: the colour.
    pub color: Option<Dist>,
    /// Filter: 0 ADD, 1 ALPHA, 2 GRAY; screen blur: 0 displaced copy, 2 toward white.
    pub mode: u8,
    /// `bIsEnableDistanceFade` with `DistanceFadeRate` (cm): faded out with the camera's distance [A].
    pub distance_fade: Option<f32>,
}

/// One pass the host is asked to run this frame (already sampled).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenFx {
    pub kind: ScreenKind,
    pub priority: u8,
    /// The centre in the world (cm, KH3 space) when the module follows the emitter; `None`: `screen` is the centre.
    pub world: Option<Vec3>,
    /// The centre in screen UV (0..1, origin top left) when `world` is None.
    pub screen: [f32; 2],
    /// The tap step (radial: a fraction of the way to the centre per tap; linear: of the screen per tap).
    pub power: f32,
    /// LINEAR direction, degrees (0 = screen right, counter-clockwise on screen) [A].
    pub angle: f32,
    /// The composite weight (Alpha; Intensity of gauss / screen blur / fringe; FilterAlpha of a filter).
    pub alpha: f32,
    pub linear: bool,
    /// (hole, inside fade, range, outside fade) in screen widths from the centre.
    pub ring: [f32; 4],
    /// Bloom radial: luminance threshold (KH3 scene-linear, before exposure).
    pub threshold: f32,
    /// Gauss: radius in percent of the screen width; aura: size.
    pub size: f32,
    /// Screen blur: displacement (screen widths) at full weight.
    pub offset: [f32; 2],
    /// Filter / aura colour.
    pub color: [f32; 3],
    pub mode: u8,
    /// The camera distance at which a distance-faded pass is gone (0: no fade).
    pub fade_distance: f32,
}

impl ScreenFx {
    /// A pass with every value neutral (tests and hosts).
    pub fn neutral(kind: ScreenKind) -> ScreenFx {
        ScreenFx { kind, priority: 1, world: None, screen: [0.5, 0.5], power: 0.0, angle: 0.0, alpha: 0.0, linear: false, ring: [0.0, 0.0, 2.0, 0.0], threshold: 0.0, size: 0.0, offset: [0.0; 2], color: [0.0; 3], mode: 0, fade_distance: 0.0 }
    }
}

fn dist(p: &Value, name: &str) -> Option<Dist> {
    p.get(name).and_then(Dist::parse)
}

fn num(p: &Value, name: &str, default: f32) -> f32 {
    p.get(name).and_then(Value::as_f64).map_or(default, |v| v as f32)
}

/// The screen module `short` (class name without `ParticleModule`) with its params, or None for any other class.
pub fn parse_screen(short: &str, p: &Value) -> Option<ScreenDef> {
    let kind = match short {
        "SQEX_RadialBlur" => ScreenKind::RadialBlur,
        "SQEX_BloomRadialBlur" => ScreenKind::BloomRadialBlur,
        "SQEX_GaussBlur" => ScreenKind::GaussBlur,
        "SQEX_ScreenBlur" => ScreenKind::ScreenBlur,
        "SQEX_Filter" => ScreenKind::Filter,
        "SQEX_PostprocessFringe" => ScreenKind::Fringe,
        "SQEX_PostprocessAura" => ScreenKind::Aura,
        _ => return None,
    };
    let blur = kind == ScreenKind::ScreenBlur;
    let pre = |n: &str| if blur { format!("ScreenBlur{n}") } else { n.to_string() };
    let filter_mode = match p.get("FildterType").and_then(Value::as_str).unwrap_or("") {
        s if s.ends_with("_ALPHA") => 1,
        s if s.ends_with("_GRAY") => 2,
        s if s.ends_with("_NPR") => 3, // (AUDIT-B W-VFX) the negative (TPostProcessSqexEffectFilterPS cb0[21])
        _ => 0,
    };
    Some(ScreenDef {
        kind,
        priority: priority_of(p),
        start: num(p, "StartTime", 0.0),
        // (modules without an EndTime run for as long as their emitter does)
        end: p.get("EndTime").and_then(Value::as_f64).map_or(f32::INFINITY, |v| v as f32),
        screen_pos: p.get("bUseScreenPos").and_then(Value::as_bool).unwrap_or(false),
        pos: [dist(p, &pre("PosX")), dist(p, &pre("PosY"))],
        power: dist(p, "Power"),
        angle: dist(p, &pre("Angle")),
        alpha: match kind {
            ScreenKind::Filter => dist(p, "FilterAlpha"),
            ScreenKind::GaussBlur | ScreenKind::Fringe => dist(p, "Intensity"),
            ScreenKind::ScreenBlur => dist(p, "ScreenBlurIntensity"),
            _ => dist(p, "Alpha"),
        },
        linear: p.get("Mode").and_then(Value::as_str).is_some_and(|m| m.ends_with("_LINEAR")),
        ring: if blur {
            [dist(p, "ScreenBlurHoleSize"), dist(p, "ScreenBlurAttenuationDistanceInside"), dist(p, "ScreenBlurRange"), dist(p, "ScreenBlurAttenuationDistanceOutside")]
        } else {
            [dist(p, "HoleSize"), dist(p, "AttenuationDistanceInside"), dist(p, "Range"), dist(p, "AttenuationDistanceOutside")]
        },
        threshold: dist(p, "LuminanceThreshold"),
        size: dist(p, "Size"),
        intensity: dist(p, "Intensity"),
        offset: [dist(p, "ScreenBlurOffsetX"), dist(p, "ScreenBlurOffsetY")],
        color: match kind {
            ScreenKind::Filter => dist(p, "FilterColor"),
            _ => dist(p, "Color"),
        },
        mode: if kind == ScreenKind::Filter { filter_mode } else { 0 },
        distance_fade: p.get("bIsEnableDistanceFade").and_then(Value::as_bool).unwrap_or(false).then(|| num(p, "DistanceFadeRate", 500.0)).filter(|r| *r > 0.0),
    })
}

impl ScreenDef {
    /// The pass at emitter time `t` (seconds since its start), centred on `node` (world); None outside its window or
    /// when it would do nothing.
    pub fn sample(&self, t: f32, node: Vec3, fade: f32) -> Option<ScreenFx> {
        if t < self.start || t > self.end {
            return None;
        }
        let mut rng = Rng::new(1);
        let f = |d: &Option<Dist>, default: f32, rng: &mut Rng| d.as_ref().map_or(default, |d| d.float(t, rng));
        let alpha = f(&self.alpha, if self.kind == ScreenKind::Filter { 1.0 } else { 0.0 }, &mut rng) * fade;
        let color = self.color.as_ref().map_or(Vec3::ONE, |c| c.vec3(t, &mut rng));
        let fx = ScreenFx {
            kind: self.kind,
            priority: self.priority,
            world: (!self.screen_pos).then_some(node),
            screen: [f(&self.pos[0], 0.5, &mut rng), f(&self.pos[1], 0.5, &mut rng)],
            power: f(&self.power, 0.0, &mut rng),
            angle: f(&self.angle, 0.0, &mut rng),
            alpha,
            linear: self.linear,
            ring: [f(&self.ring[0], 0.0, &mut rng), f(&self.ring[1], 0.0, &mut rng), f(&self.ring[2], 2.0, &mut rng), f(&self.ring[3], 0.0, &mut rng)],
            threshold: f(&self.threshold, 0.0, &mut rng),
            size: f(&self.size, 0.0, &mut rng),
            offset: [f(&self.offset[0], 0.0, &mut rng), f(&self.offset[1], 0.0, &mut rng)],
            color: color.to_array(),
            mode: self.mode,
            fade_distance: self.distance_fade.unwrap_or(0.0),
        };
        // (a pass that would change nothing is not asked for)
        let idle = match self.kind {
            ScreenKind::Filter if self.mode == 0 => fx.alpha.abs() < 1.0e-5 || color.abs().max_element() < 1.0e-6,
            _ => fx.alpha.abs() < 1.0e-5,
        };
        (!idle && fx.alpha.is_finite()).then_some(fx)
    }
}

// ------------------------------------------------------------------------------------------- the passes' maths

/// [S] `FPostProcessSqexRadialBlurCompositePS`: the weight of the blurred picture at distance `d` (screen widths, the
/// vertical part scaled by height / width) from the centre: sat((d - hole) / inside) - sat((d - range) / outside),
/// not below 0 (each fade width + 1e-5), times alpha by the caller.
pub fn ring_weight(d: f32, ring: [f32; 4]) -> f32 {
    let [hole, inside, range, outside] = ring;
    let a = ((d - hole) / (inside + 1.0e-5)).clamp(0.0, 1.0);
    let b = ((d - range) / (outside + 1.0e-5)).clamp(0.0, 1.0);
    (a - b).max(0.0)
}

/// [S] `TPostProcessSqexRadialBlurPS`: the ten taps' weights (tap k at uv + k x step); they sum to 1.
pub const RADIAL_TAPS: [f32; 10] = [0.19, 0.17, 0.15, 0.13, 0.11, 0.09, 0.07, 0.05, 0.03, 0.01];

/// [S] The tap step of a radial blur pass at `uv` (screen UV): RADIAL (centre - uv) x power, LINEAR the direction
/// (angle degrees, y down on screen) x power.
pub fn radial_step(uv: [f32; 2], center: [f32; 2], power: f32, linear: bool, angle_deg: f32) -> [f32; 2] {
    if linear {
        let a = angle_deg.to_radians();
        [a.cos() * power, -a.sin() * power]
    } else {
        [(center[0] - uv[0]) * power, (center[1] - uv[1]) * power]
    }
}

/// UE 4.17's default bloom (FPostProcessSettings, the values KH3's gameplay levels keep: no level of the game overrides
/// them outside cutscene cameras; Config/DefaultEngine.ini r.DefaultFeature.Bloom=True, BloomQuality 5): intensity,
/// threshold (-1: no threshold, every pixel blooms by saturate((L + 1) / 2)), and the six gaussian levels as
/// (size in percent of the screen width x BloomSizeScale 4, tint).
pub const UE4_BLOOM_INTENSITY: f32 = 0.675;
pub const UE4_BLOOM_THRESHOLD: f32 = -1.0;
pub const UE4_BLOOM_SIZE_SCALE: f32 = 4.0;
pub const UE4_BLOOM_LEVELS: [(f32, f32); 6] = [(0.3, 0.3465), (1.0, 0.138), (2.0, 0.1176), (10.0, 0.066), (30.0, 0.066), (64.0, 0.061)];
/// (VFXR bloom fix) UE 4's gaussian bloom divides every stage's tint by the number of stages ("TintScale = 1 /
/// NumBloomStages": the six stages together stay a fair share of the source) [A: UE source as remembered, not traced in
/// KH3's binary]; without it the effects' bloom read far too strong against the user's KH3 footage
/// (build/sora_tmp/vfxr/shots/bloom_fix).
pub const UE4_BLOOM_TINT_SCALE: f32 = 1.0 / 6.0;

/// (VFXR bloom fix) A stage's size is a DIAMETER in percent of the view width (UE 4 PostProcessWeightedSampleSum: "/2 as
/// we use the diameter, 100 as we use percent"): the kernel radius in pixels = size x BloomSizeScale x width / 200.
pub fn ue4_bloom_radius_px(level: usize, width: f32) -> f32 {
    UE4_BLOOM_LEVELS[level].0 * UE4_BLOOM_SIZE_SCALE * width / 200.0
}

/// UE4's bloom set-up (PostProcessBloom.usf BloomSetup): the share of a pixel of luminance `l` (exposed) that blooms.
pub fn ue4_bloom_amount(l: f32, threshold: f32) -> f32 {
    ((l - threshold) * 0.5).clamp(0.0, 1.0)
}

/// The gaussian weights of a kernel of `radius` pixels (UE4's: sigma = radius / 3 [A]), taps -n..=n, normalised.
pub fn gauss_weights(radius_px: f32, max_taps: usize) -> Vec<f32> {
    let sigma = (radius_px / 3.0).max(1.0e-3);
    let n = (radius_px.ceil() as usize).clamp(0, max_taps);
    let mut w: Vec<f32> = (0..=2 * n).map(|k| (-((k as f32 - n as f32).powi(2)) / (2.0 * sigma * sigma)).exp()).collect();
    let sum: f32 = w.iter().sum();
    w.iter_mut().for_each(|x| *x /= sum);
    w
}

/// [S] `TPostProcessSqexEffectFilterPS` and [A] the module mapping: what a filter does to a DISPLAYED colour (0..1).
pub fn apply_filter(c: [f32; 3], mode: u8, color: [f32; 3], alpha: f32) -> [f32; 3] {
    match mode {
        // ADD
        0 => [c[0] + color[0] * alpha, c[1] + color[1] * alpha, c[2] + color[2] * alpha],
        // ALPHA
        1 => [c[0] + (color[0] - c[0]) * alpha, c[1] + (color[1] - c[1]) * alpha, c[2] + (color[2] - c[2]) * alpha],
        // (AUDIT-B W-VFX) NPR: the negative by alpha (cb0[21].w)
        3 => {
            let neg = alpha.max(0.0);
            [c[0] + (1.0 - 2.0 * c[0]) * neg, c[1] + (1.0 - 2.0 * c[1]) * neg, c[2] + (1.0 - 2.0 * c[2]) * neg]
        }
        // (AUDIT-B W-VFX) GRAY: the grey by alpha (cb0[22].w); FilterColor is (spare flag, edge colour, edge amount): the
        // object classes and the depth edge are the hosts' shaders' (no classes, no depth here)
        _ => {
            let grey = alpha.max(0.0);
            let m = (c[0] + c[1] + c[2]) / 3.0;
            [c[0] + (m - c[0]) * grey, c[1] + (m - c[1]) * grey, c[2] + (m - c[2]) * grey]
        }
    }
}

/// The one pass of each kind a host runs: the highest priority, then the strongest.
pub fn pick(list: &[ScreenFx]) -> Vec<ScreenFx> {
    let mut out: Vec<ScreenFx> = Vec::new();
    for fx in list {
        match out.iter_mut().find(|o| o.kind == fx.kind) {
            Some(o) => {
                if (fx.priority, fx.alpha.abs()) > (o.priority, o.alpha.abs()) {
                    *o = *fx;
                }
            }
            None => out.push(*fx),
        }
    }
    // Filters of different modes may stack: the strongest ADD and the strongest ALPHA both run.
    // (AUDIT-B W-VFX) GRAY / NPR: KH3 SUMS every live component's (FilterColor, FilterAlpha) of the type (0x1443A9170)
    for mode in [2u8, 3] {
        let mut same = list.iter().filter(|f| f.kind == ScreenKind::Filter && f.mode == mode);
        let Some(first) = same.next() else { continue };
        let mut sum = *first;
        for f in same {
            sum.color = [sum.color[0] + f.color[0], sum.color[1] + f.color[1], sum.color[2] + f.color[2]];
            sum.alpha += f.alpha;
            sum.priority = sum.priority.max(f.priority);
        }
        out.retain(|o| !(o.kind == ScreenKind::Filter && o.mode == mode));
        out.push(sum);
    }
    for fx in list.iter().filter(|f| f.kind == ScreenKind::Filter && f.mode < 2) {
        if !out.iter().any(|o| o.kind == ScreenKind::Filter && o.mode == fx.mode) {
            out.push(*fx);
        } else if let Some(o) = out.iter_mut().find(|o| o.kind == ScreenKind::Filter && o.mode == fx.mode) {
            if (fx.priority, fx.alpha.abs()) > (o.priority, o.alpha.abs()) {
                *o = *fx;
            }
        }
    }
    out.sort_by_key(|f| (f.kind, f.mode));
    out
}

// ------------------------------------------------------------------------------------------- character drives

/// `SQEX_Material_*`: material parameters of the character the effect belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct CharParamDef {
    /// (parameter name, its value over the emitter time, a vector (else a scalar))
    pub values: Vec<(String, Dist, bool)>,
    /// `InLightCenterPosition`: written from the emitter's place (the in-light module).
    pub center: bool,
    /// `InLightCameraOffset`: the centre moved this far toward the camera (cm) [A].
    pub camera_offset: Option<Dist>,
    /// `NoSetUseLastParameter` as the module has it. (FREEZE-FX) False = UseLastParameter: the material drive leaves its last
    /// value on the mesh when the emitter ends (`CharParam::keep`; the frozen look's "Ice" 0 -> 1, held, then 1 -> 0 by the break).
    pub keep_last: bool,
    /// `ParamType`: 0 the main body (SQEX_IN_LIGHT_TYPE_MAIN), 1 a sub mesh.
    pub target: u8,
    /// (VFX-2) `SQEX_Material_Scalar` / `_Vector`: their update (0x144378F10 / 0x144379F10) sets the parameter on the
    /// emitter's own dynamic material, else on the mesh of `vt+0x2F8` (0x1442871A0: the emitter's own SQEX_SkeletalMesh
    /// component, else its parent emitter's, else the owner's mesh `AttachOwnerMeshComp`): the character only when no
    /// skeletal mesh emitter is in between (`EmitterDef::mat_params`).
    pub material: bool,
    /// (VFX-2) `SQEX_Material_Vector.MaterialAlphaParam`: the vector parameter's alpha (FLinearColor A) for a dynamic material.
    pub alpha: Option<Dist>,
}

/// One drive this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct CharParam {
    /// The effect it comes from (the host knows whose it is).
    pub effect: u64,
    pub name: String,
    pub value: [f32; 4],
    pub vector: bool,
    pub target: u8,
    /// (FREEZE-FX) `SQEX_Material_Scalar` / `_Vector` with UseLastParameter (`NoSetUseLastParameter` false): KH3 leaves the
    /// last value on the mesh when the emitter ends (KH3's frozen look: `vs_abn_frz_in` puts "Ice" at 1 and leaves it there
    /// until the break's `vs_abn_frz_out0` takes it back). The host keeps it while the effect's spawn is not stopped.
    pub keep: bool,
}

/// The material module `short` with its params, or None for any other class.
pub fn parse_char(short: &str, p: &Value) -> Option<CharParamDef> {
    let keep_last = p.get("NoSetUseLastParameter").and_then(Value::as_bool).unwrap_or(false);
    let name = p.get("ParamName").and_then(Value::as_str).unwrap_or("").to_string();
    let mut values = Vec::new();
    let mut push = |n: &str, key: &str, vector: bool| {
        if let Some(d) = dist(p, key) {
            values.push((n.to_string(), d, vector));
        }
    };
    let def = match short {
        "SQEX_Material_InLight" => {
            // (HAIR-PULSE) KH3's parameter names come from ParamType, not ParamName: 0x14436FCA0 takes them from the table of
            // 0x140387190 at +0 (MAIN: "InLight", "InLightColor" ...) or +0x40 (SUB: "Sub_InLight", "Sub_InLightColor" ...),
            // the scalar included (`r9 = ParamType != 0 ? 0x40 : 0`). Every body / hair / face material of Sora (all outfits)
            // reads only the MAIN names; the weapons read the SUB ones. So the form charge glow vs_com_lv2 (SUB) never lights
            // Sora's model (it lit his hair and clothes every 2 s in every form when it was written as "InLight").
            let target = u8::from(!p.get("ParamType").and_then(Value::as_str).unwrap_or("_MAIN").ends_with("_MAIN"));
            let names = in_light_names(target); // (HAIR-PULSE)
            push(names[0], "InLightParam", false); // (HAIR-PULSE) was ParamName (every module of the pack: "InLight")
            push(names[1], "InLightColor", true); // (HAIR-PULSE) names[..] were the MAIN literals
            push(names[2], "InLightOutColor", true);
            push(names[4], "InLightCenterBounds", false);
            push(names[5], "InLightCenterBoundsBlur", false);
            push(names[6], "InLightOutBounds", false);
            push(names[7], "InLightOutBoundsBlur", false);
            CharParamDef { values, center: true, camera_offset: dist(p, "InLightCameraOffset"), keep_last, target, material: false, alpha: None }
        }
        "SQEX_Material_Aura" => {
            let main = if name.is_empty() { "Aura".to_string() } else { name };
            push(&main, "Aura", false);
            push("AuraColor", "AuraColor", true);
            push("AuraOutColor", "AuraOutColor", true);
            CharParamDef { values, center: false, camera_offset: None, keep_last, target: 0, material: false, alpha: None }
        }
        "SQEX_Material_Scalar" if !name.is_empty() => {
            push(&name, "MaterialScalarParam", false);
            CharParamDef { values, center: false, camera_offset: None, keep_last, target: 0, material: true, alpha: None }
        }
        "SQEX_Material_Vector" if !name.is_empty() => {
            push(&name, "MaterialVectorParam", true);
            CharParamDef { values, center: false, camera_offset: None, keep_last, target: u8::from(p.get("BoneOrSocketName").and_then(Value::as_str).is_some_and(|b| b != "None")), material: true, alpha: dist(p, "MaterialAlphaParam") }
        }
        _ => return None,
    };
    Some(def)
}

impl CharParamDef {
    /// (VFX-2) A `SQEX_Material_*` module's values at emitter time `t`, for the dynamic material of a skeletal mesh
    /// emitter (name, value, vector), in the module order: a later module of the same name wins, as its later call does.
    pub fn values_at(&self, t: f32, out: &mut Vec<(String, [f32; 4], bool)>) {
        let mut rng = Rng::new(1);
        for (name, d, vector) in &self.values {
            let value = if *vector {
                let v = d.vec3(t, &mut rng);
                [v.x, v.y, v.z, self.alpha.as_ref().map_or(0.0, |a| a.float(t, &mut rng))]
            } else {
                [d.float(t, &mut rng), 0.0, 0.0, 0.0]
            };
            match out.iter_mut().find(|o| o.0 == *name) {
                Some(o) => *o = (name.clone(), value, *vector),
                None => out.push((name.clone(), value, *vector)),
            }
        }
    }

    /// The drives at emitter time `t`; `node` the emitter's place, `camera` the camera's (for the in-light centre).
    pub fn sample(&self, effect: u64, t: f32, node: Vec3, camera: Vec3, out: &mut Vec<CharParam>) {
        let mut rng = Rng::new(1);
        for (name, d, vector) in &self.values {
            let value = if *vector {
                let v = d.vec3(t, &mut rng);
                [v.x, v.y, v.z, 0.0]
            } else {
                [d.float(t, &mut rng), 0.0, 0.0, 0.0]
            };
            out.push(CharParam { effect, name: name.clone(), value, vector: *vector, target: self.target, keep: self.material && !self.keep_last /* (FREEZE-FX) */ });
        }
        if self.center {
            let offset = self.camera_offset.as_ref().map_or(0.0, |d| d.float(t, &mut rng));
            let c = node + (camera - node).normalize_or_zero() * offset;
            // (world cm; the host makes it relative to the character: `in_light_center`)
            // (HAIR-PULSE) the SUB module writes Sub_InLightCenterPosition (0x14436FCA0)
            out.push(CharParam { effect, name: in_light_names(self.target)[3].to_string(), value: [c.x, c.y, c.z, 1.0], vector: true, target: self.target, keep: false });
        }
    }
}

/// (HAIR-PULSE) KH3's in-light parameter names (FName table 0x140387190) of a `ParamType`: 0 SQEX_IN_LIGHT_TYPE_MAIN, else
/// SQEX_IN_LIGHT_TYPE_SUB. Order: the scalar, colour, out colour, centre position, centre bounds, its blur, out bounds, its blur.
pub fn in_light_names(target: u8) -> [&'static str; 8] {
    if target == 0 {
        ["InLight", "InLightColor", "InLightOutColor", "InLightCenterPosition", "InLightCenterBounds", "InLightCenterBoundsBlur", "InLightOutBounds", "InLightOutBoundsBlur"]
    } else {
        ["Sub_InLight", "Sub_InLightColor", "Sub_InLightOutColor", "Sub_InLightCenterPosition", "Sub_InLightCenterBounds", "Sub_InLightCenterBoundsBlur", "Sub_InLightOutBounds", "Sub_InLightOutBoundsBlur"]
    }
}

/// (HAIR-PULSE) The drive is an in-light centre (MAIN or SUB), which the hosts make relative to the character.
pub fn is_in_light_center(name: &str) -> bool {
    name == "InLightCenterPosition" || name == "Sub_InLightCenterPosition"
}

/// [A] The character materials read `InLightCenterPosition` relative to the character (its default (0, 0, 70) is chest
/// height): the drive's world centre minus the character's root, in world axes.
pub fn in_light_center(world: Vec3, character_root: Vec3) -> [f32; 4] {
    let r = world - character_root;
    [r.x, r.y, r.z, 0.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_weight_is_the_composite_shaders() {
        // Range 2, no hole: the whole screen at full weight (vm_blz_han080)
        for d in [0.0, 0.3, 0.7, 1.2] {
            assert!((ring_weight(d, [0.0, 0.0, 2.0, 0.0]) - 1.0).abs() < 1.0e-4 || d == 0.0);
        }
        // a hole of 0.2 fading in over 0.3 (vs_div_blr00): 0 inside, half at 0.35, full past 0.5
        let ring = [0.2, 0.3, 1.0, 0.0];
        assert_eq!(ring_weight(0.1, ring), 0.0);
        assert!((ring_weight(0.35, ring) - 0.5).abs() < 1.0e-3);
        assert!((ring_weight(0.6, ring) - 1.0).abs() < 1.0e-3);
        assert_eq!(ring_weight(1.1, ring), 0.0);
        // full within 0.1, gone 0.2 later (vt_grv_bom01)
        let ring = [0.0, 0.0, 0.1, 0.2];
        assert!((ring_weight(0.05, ring) - 1.0).abs() < 1.0e-3);
        assert!((ring_weight(0.2, ring) - 0.5).abs() < 1.0e-3);
        assert_eq!(ring_weight(0.31, ring), 0.0);
        // hole = range: nothing (VN_slg_hvy)
        assert_eq!(ring_weight(1.0, [2.0, 0.0, 2.0, 0.0]), 0.0);
    }

    #[test]
    fn the_radial_taps_sum_to_one_and_step_toward_the_centre() {
        assert!((RADIAL_TAPS.iter().sum::<f32>() - 1.0).abs() < 1.0e-5);
        let s = radial_step([0.9, 0.5], [0.5, 0.5], 0.01, false, 0.0);
        assert!((s[0] + 0.004).abs() < 1.0e-6 && s[1].abs() < 1.0e-6);
        // linear at 90 degrees: straight up the screen
        let s = radial_step([0.2, 0.2], [0.5, 0.5], 0.001, true, 90.0);
        assert!(s[0].abs() < 1.0e-6 && (s[1] + 0.001).abs() < 1.0e-6);
    }

    #[test]
    fn a_radial_blur_module_samples_its_curve_and_window() {
        let json: Value = serde_json::from_str(r#"{"HoleSize":0,"AttenuationDistanceInside":0,"Range":2,"AttenuationDistanceOutside":0,"PosX":0.5,"PosY":0.5,"bUseScreenPos":false,
            "Alpha":{"distribution":{"type":"constant","constant":0.5,"lookup_table":{"op":1,"entry_count":1,"entry_stride":1,"sub_entry_stride":0,"time_scale":0,"time_bias":0,"lock_flag":0,"values":[0.5]}}},
            "Angle":134,"Power":0.0005,"Mode":"SQEX_RADIAL_BLUR_MODE_LINEAR","EffectPriority":"ScreenEffectPriority_Normal","EndTime":1,"StartTime":0}"#).unwrap();
        let def = parse_screen("SQEX_RadialBlur", &json).expect("parsed");
        assert!(def.linear && def.kind == ScreenKind::RadialBlur);
        let fx = def.sample(0.5, Vec3::new(1.0, 2.0, 3.0), 1.0).expect("active");
        assert_eq!(fx.world, Some(Vec3::new(1.0, 2.0, 3.0)));
        assert!((fx.alpha - 0.5).abs() < 1.0e-6 && (fx.power - 0.0005).abs() < 1.0e-9 && (fx.angle - 134.0).abs() < 1.0e-4);
        assert_eq!(fx.ring, [0.0, 0.0, 2.0, 0.0]);
        assert!(def.sample(1.5, Vec3::ZERO, 1.0).is_none());
        // the component's fade takes it out
        assert!(def.sample(0.5, Vec3::ZERO, 0.0).is_none());
    }

    #[test]
    fn filters_add_mix_and_grey() {
        let a = apply_filter([0.2, 0.2, 0.2], 0, [0.3, 0.7, 4.0], 0.5);
        assert!((a[0] - 0.35).abs() < 1.0e-6 && (a[1] - 0.55).abs() < 1.0e-6 && (a[2] - 2.2).abs() < 1.0e-6);
        let a = apply_filter([0.2, 0.4, 0.6], 1, [1.0, 1.0, 1.0], 0.5);
        assert!((a[0] - 0.6).abs() < 1.0e-6 && (a[2] - 0.8).abs() < 1.0e-6);
        // (AUDIT-B W-VFX) GRAY greys by alpha whatever its FilterColor (spare flag, edge colour, edge amount): Clock Drill's
        // vw_so114_fns_nis00 (1, 1, 0.6) is grey, not a negative
        let g = apply_filter([0.0, 0.3, 0.9], 2, [1.0, 1.0, 0.6], 1.0);
        assert!((g[0] - 0.4).abs() < 1.0e-6 && (g[1] - 0.4).abs() < 1.0e-6 && (g[2] - 0.4).abs() < 1.0e-6);
        let h = apply_filter([0.0, 0.3, 0.9], 2, [0.0, 1.0, 0.0], 0.5);
        assert!((h[0] - 0.2).abs() < 1.0e-6 && (h[2] - 0.65).abs() < 1.0e-6);
        // NPR: the negative by alpha
        let n = apply_filter([0.25, 0.5, 1.0], 3, [0.0, 0.0, 0.0], 1.0);
        assert!((n[0] - 0.75).abs() < 1.0e-6 && n[2].abs() < 1.0e-6);
    }

    #[test]
    fn ue4_bloom_constants_and_gauss() {
        assert!((ue4_bloom_amount(0.0, UE4_BLOOM_THRESHOLD) - 0.5).abs() < 1.0e-6);
        assert_eq!(ue4_bloom_amount(5.0, UE4_BLOOM_THRESHOLD), 1.0);
        assert_eq!(ue4_bloom_amount(0.5, 8.0), 0.0);
        let total: f32 = UE4_BLOOM_LEVELS.iter().map(|(_, t)| t).sum();
        assert!((total - 0.7951).abs() < 1.0e-3);
        let w = gauss_weights(6.0, 32);
        assert_eq!(w.len(), 13);
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1.0e-5 && w[6] > w[5] && (w[0] - w[12]).abs() < 1.0e-7);
    }

    #[test]
    fn one_pass_per_kind_by_priority_then_strength() {
        let mut a = ScreenFx::neutral(ScreenKind::RadialBlur);
        a.alpha = 0.9;
        let mut b = a;
        b.alpha = 0.2;
        b.priority = 3;
        let mut c = ScreenFx::neutral(ScreenKind::Filter);
        c.alpha = 0.3;
        let mut d = c;
        d.mode = 1;
        let picked = pick(&[a, b, c, d]);
        assert_eq!(picked.len(), 3);
        assert_eq!(picked.iter().find(|f| f.kind == ScreenKind::RadialBlur).unwrap().priority, 3);
    }

    #[test]
    fn the_in_light_module_drives_the_characters_parameters() {
        let json: Value = serde_json::from_str(r#"{"InLightCameraOffset":0,"InLightOutBoundsBlur":5,"InLightOutBounds":20,"InLightCenterBoundsBlur":2,"InLightCenterBounds":3,
            "InLightOutColor":{"x":1,"y":0.2,"z":0},"InLightColor":{"x":1,"y":0.7,"z":0.5},"InLightParam":10,"ParamType":"SQEX_IN_LIGHT_TYPE_MAIN","NoSetUseLastParameter":false,"ParamName":"InLight"}"#).unwrap();
        let def = parse_char("SQEX_Material_InLight", &json).expect("parsed");
        let mut out = Vec::new();
        def.sample(7, 0.3, Vec3::new(100.0, 0.0, 50.0), Vec3::new(-500.0, 0.0, 100.0), &mut out);
        let get = |n: &str| out.iter().find(|p| p.name == n).unwrap().value;
        assert_eq!(get("InLight")[0], 10.0);
        assert_eq!(&get("InLightColor")[..3], &[1.0, 0.7, 0.5]);
        assert_eq!(get("InLightOutBounds")[0], 20.0);
        assert_eq!(&get("InLightCenterPosition")[..3], &[100.0, 0.0, 50.0]);
        assert_eq!(in_light_center(Vec3::new(100.0, 0.0, 50.0), Vec3::new(90.0, 0.0, 0.0)), [10.0, 0.0, 50.0, 0.0]);
        assert!(out.iter().all(|p| p.effect == 7));
    }

    /// (HAIR-PULSE) A SUB in-light module (the form charge glow vs_com_lv2) writes only KH3's `Sub_InLight*` names
    /// (0x14436FCA0 +0x40), never the MAIN ones Sora's body / hair materials read.
    #[test]
    fn a_sub_in_light_module_writes_the_sub_names_only() {
        let json: Value = serde_json::from_str(r#"{"InLightCameraOffset":0,"InLightOutBoundsBlur":1,"InLightOutBounds":3,"InLightCenterBoundsBlur":1,"InLightCenterBounds":2,
            "InLightOutColor":{"x":5,"y":4,"z":1},"InLightColor":{"x":10,"y":10,"z":10},"InLightParam":0.5,"ParamType":"SQEX_IN_LIGHT_TYPE_SUB","NoSetUseLastParameter":false,"ParamName":"InLight"}"#).unwrap();
        let def = parse_char("SQEX_Material_InLight", &json).expect("parsed");
        assert_eq!(def.target, 1);
        let mut out = Vec::new();
        def.sample(3, 0.5, Vec3::new(10.0, 0.0, 90.0), Vec3::new(-500.0, 0.0, 100.0), &mut out);
        let names: Vec<&str> = out.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Sub_InLight", "Sub_InLightColor", "Sub_InLightOutColor", "Sub_InLightCenterBounds", "Sub_InLightCenterBoundsBlur", "Sub_InLightOutBounds", "Sub_InLightOutBoundsBlur", "Sub_InLightCenterPosition"]);
        assert_eq!(out[0].value[0], 0.5);
        assert!(names.iter().all(|n| !in_light_names(0).contains(n)));
        assert!(is_in_light_center("Sub_InLightCenterPosition") && is_in_light_center("InLightCenterPosition") && !is_in_light_center("InLight"));
    }
}
