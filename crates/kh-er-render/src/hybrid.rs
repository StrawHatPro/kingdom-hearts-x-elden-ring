//! (IMPL_ER78) THE HYBRID: KH3's toon shading lit by Elden Ring's own world light (the user, 2026-10-15: "do the hybrid").
//!
//! Evidence and route: build/sora_tmp/er/IMPL_ER78_STATE.md. In short (all read from the game's shader binaries):
//! - the game's G-buffer is five targets: 0 = world normal * 0.5 + 0.5 (R10G10B10A2), 1 = (metalness, gloss,
//!   translucency, occlusion) (R8G8B8A8), 2 = (albedo, light set / 255) (R8G8B8A8 typeless, sRGB), 3 = SSS (R8G8),
//!   4 = emissive (R16G16B16A16 float);
//! - its light passes accumulate DIFFUSE light without albedo into one R11G11B10 target, specular into another;
//!   GXLightCmbAlbedo multiplies the diffuse one by albedo x (1 - metalness) before the composite.
//!
//! So: before the lighting (at the early split, where his early depth goes) every character our renderer draws writes its
//! normal and a WHITE, non-metallic surface into the game's G-buffer; after the stage the diffuse target at its pixels is
//! the light the game gave it (all local lights, shadows, AO, GI). Its colour shader (`*_hy` variants) reads that light per
//! pixel, puts it through a soft band staircase and hands it to KH3's own light buffer, which KH3's post-process material
//! turns into the toon look (its colour table, rim, outline) as before.
//!
//! PER DRAW, NOT PER CHARACTER: the G-buffer write is one draw per opaque / masked slot of any model in `Assets.models`,
//! and the light is read at each pixel's own position, so any number of characters work in one frame with no ids or
//! stencil values of their own (the G-buffer pass marks exactly their pixels). The only frame-wide value is the light
//! level the bands are measured against (the rig's white, from the ground at Sora's feet), shared like today's rig.
//! THE SEAM for the party (Donald n_ex001, Goofy n_ex002): load their models into the same `Assets.models` (or a second
//! assets set drawn by the same loop) with pixel shaders built by kh-shaderc's `_hy` / `_gb` entries; nothing else.
//!
//! (HY, 2026-10-16, build/sora_tmp/er/IMPL_HY_STATE.md) THE THIRD VERSION, ON BY DEFAULT: `*_hy3` / `*_gb3` (+ `_gb3_hair` for a
//! hair core) for Sora, every outfit and own set, the form weapons and the party. His shaders never read the game's targets:
//! a capture pass of ours (gfx.rs `HyCapture`, hycap.ps.dxil) copies the game's light (its albedo taken out), occlusion, normal
//! and sun visibility over the box of the G-buffer pass right after the deferred stage; each pixel takes the capture only where
//! its normal is his own (else a neighbour's, else today's light); KH3's undirected light stays in its indirect term (the rig's
//! share is taken out of the game's light), his own sun shadow from his capsules, the soft bands, KH3's rim / outline / colour
//! table as before. `r.hybrid 0` reloads today's files; any failed check = today's light for the frame.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

use glam::Vec3;

/// The keys (`r.hybrid*` in the control file; kh-er frame.rs hands them over every second).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// `r.hybrid` (IMPL_ER78b: default 0 until a dump from the game confirms it; 1 = on). 0 = exactly the path before IMPL_ER78.
    pub on: bool,
    /// `r.hybrid_cast` (IMPL_ER78b: default 0): while the hybrid is on his skinned mesh also goes into the sun's cascades (IMPL_ER17's
    /// real shadow, as `r.shadow_real = 1`): his shadow on the world and on himself through the game's own shadow mask.
    pub cast: bool,
    /// `r.hybrid_game` (1): the share of the game's light; `r.hybrid_rig` (0): the share of the stand-in rig's light
    /// (today's key + ambient) added to it; `r.hybrid_spec` (1): KH3's specular from the sun, shadowed per pixel.
    pub game: f32,
    pub rig: f32,
    pub spec: f32,
    /// (IMPL_ER78b) `r.hybrid_sh` (1): the share of KH3's own indirect term (the rig's SH: his eyes, face and fill) kept; 0 =
    /// the game's light alone (the ER78 build: dark eyes and face).
    pub sh: f32,
    /// THE BANDS on the received light L (1 = what lit level ground at his feet gets): `r.hybrid_band_lo` (0.25) the
    /// shade / light step, `r.hybrid_band_hi` (1.30) a highlight step, `r.hybrid_band_soft` (0.06) their half width,
    /// `r.hybrid_band_mix` (0.75: 0 = the light as received, 1 = full staircase), `r.hybrid_shade` (0.50) the level of the
    /// shade band, `r.hybrid_high` (0) what the highlight band adds.
    pub band_lo: f32,
    pub band_hi: f32,
    pub band_soft: f32,
    pub band_mix: f32,
    pub shade: f32,
    pub high: f32,
    /// `r.hybrid_ao` (1): the game's ambient occlusion on him as it gave it; 0 = taken out again (approximately), 2 = doubled.
    pub ao: f32,
    /// `r.hybrid_fog` (0): extra haze on him toward the game's fog colour, per 100 m (0 = the game's own fog only, which
    /// already reaches him: he is in the scene depth before its fog passes).
    pub fog: f32,
    /// The surface he shows the game: `r.hybrid_wrap` (0) G-buffer 1 .z, the game's translucency (a softer terminator of
    /// its lights); `r.hybrid_gloss` (0.3) G-buffer 1 .y; `r.hybrid_lightset` (0) G-buffer 2 .a (which of the two
    /// directional light sets of cbLight lights him).
    pub wrap: f32,
    pub gloss: f32,
    pub lightset: u8,
    /// `r.hybrid_edge` (1): a masked slot's dither coverage is at least this opaque (his pixels behind a partly covered
    /// hair pixel are the game-lit white surface now: blending with it gives a pale fringe). 0 = as before.
    pub edge: f32,
    /// (HY) THE THIRD VERSION's keys (`*_hy3` shaders; frame.rs reads them through `with_keys`):
    /// `r.hybrid_amb_sub` (1): the share of the rig's undirected light (what KH3's indirect term already gives him) taken
    /// out of the game's light before it goes into the light buffer (KH3 keeps the two apart; 0 = the game's whole light in
    /// the light buffer, as ER78);
    pub amb_sub: f32,
    /// `r.hybrid_self` (0 since RSF2 round 17; was 1): the strength of his own shadow on himself from the sun (his capsules). In
    /// game (rsf17, the user: "strange shadow artifacts on his face and pants") the capsules printed soft capsule-shaped greys on his
    /// torso, legs and face; the other characters' real shadows on him come from the KH caster maps (r.shadow_real). 1 = as before;
    pub self_shadow: f32,
    /// `r.hybrid_lift` (1): the game's light on him x this (the own sets' look file may ask for more: cgfield.rs);
    pub lift: f32,
    /// `r.hybrid_match` (0.94): the cos of the angle within which the captured G-buffer normal must agree with his own normal
    /// at a pixel (else a neighbour's, else today's light for that pixel); `r.hybrid_search` (2): the neighbours' reach (px);
    pub match_cos: f32,
    pub search: f32,
    /// `r.hybrid_debug` (0): 1 the source of each pixel's light (green own / yellow neighbour / red none), 2 the game's light
    /// in his light buffer, 3 its occlusion, 4 the sun's visibility x his own shadow, 5 (offline orbit test) the pixel's place on
    /// him; shown in place of his colour.
    pub debug: u8,
    /// (RSF2 round 5) `r.hybrid_mask` (1): at KH pixels the capture takes the game's screen-space mask (its occlusion and its
    /// sun visibility, which may be the world's behind him) out of the game's light and puts back the mean of that mask over
    /// the KH pixels on screen (er_char.wgsl hycap `hc_mean`): nothing of the background's pattern lands on a KH character.
    /// 0 = the capture as before (the per-pixel mask).
    pub mask: f32,
    /// (RSF2 round 12, the user: "make the shadow edges work") `r.hybrid_sun` (1): the sun's visibility at a KH texel is looked up
    /// in Elden Ring's OWN CASCADED SHADOW MAPS at that texel's own place (the scene depth with the KH characters in it, his
    /// normal), not taken from the screen-space mask: a building's, a wall's or a branch's shadow edge falls on him sharp, where it
    /// really is, and nothing of the background's screen pattern can (the maps are world space). Where no cascade of this frame
    /// holds the point: the round-5 mean. 0 = the round-5 mean everywhere (one visibility for all the KH characters on screen).
    pub sun: f32,
    /// (RSF2 round 12) `r.hybrid_sun_soft` (1): the width of the lookup's filter in atlas texels (1 = a 3 x 3 bilinear PCF, the
    /// size of the game's own; 0.5 sharper, 2 softer).
    pub sun_soft: f32,
    /// (RSF2 round 12) `r.hybrid_sun_bias`: the lookup's depth bias in steps of the 16-bit atlas (see `BIAS_STEPS`).
    pub sun_bias: f32,
    /// (RSF2 round 12) `r.hybrid_sun_fit` (1): the sun's strength at the KH texels is FITTED from the game's own light there (its sun
    /// x its own mask against the rest, least squares over the capture's grid, hycap `hc_wide`), not taken from our model of the sun
    /// alone (er.hy[7]: in game about a third of the game's real sun on them, so a shadow edge came out faint). 0 = the model.
    pub sun_fit: f32,
    /// (RSF2 round 13, the user: "I don't see any shadow lines") `r.hybrid_shadow` (1): the strength of THE WORLD'S SHADOW FACTOR the
    /// colour shaders multiply the whole colour of a KH character by (hycap fit 2: the light with the visibility at his own place
    /// over the light in the sun, the ratio the ground beside him shows). 0 = the shadow only inside the staircase (round 12).
    pub shadow: f32,
    /// (RSF2 round 13) `r.hybrid_shade` (0.35): fit 3's shade ratio (the game's light in the shade over in the sun) where no lit /
    /// shaded pair of samples is near a texel; measured pairs replace it.
    pub shade_ratio: f32,
    /// (RSF2 round 10) `r.hybrid_fit` = 1 (round 10's default; round 11: 2, below): the dark shapes the game's light passes print on the KH characters from what lies behind
    /// them (the user's r.hybrid_debug=2 frame: the castle wall's crenellations, branch lines) are CLOSED: each KH texel takes the
    /// brightest captured light among the KH texels around it that face the same way (er_char.wgsl hycap `hc_close`). 0 = the
    /// per-pixel capture (round 7).
    /// (RSF2 round 11) 2 (the default): THE WIDE FILL (er_char.wgsl hycap `hc_wide`): per pixel only the sun's part (the mean
    /// visibility), the rest of the game's light the normal-weighted mean over the KH texels of a wide grid around it. In game
    /// the close (1) left blotches all over them and the per-pixel light (0) the tree's branch lines.
    pub fit: f32,
    /// (RSF2 round 11) `r.hybrid_gbclear` (1): after the G-buffer pass, the game's G-buffer targets 3 (subsurface) and 4 (emissive)
    /// are set to "none" under every KH character (gfx.rs, hygb.ps.dxil). Without it they keep the background's values at his
    /// pixels and the game's subsurface blur runs over his light with the background's profile (foliage behind him: the
    /// branch, crenellation and arch shapes in the user's r.hybrid_debug=2 frames). 0 = as before (targets 0..2 only).
    pub gbclear: f32,
    /// `r.hybrid_rig_own` (0.35): the share of the rig's light (cgfield's studio key) kept for an outfit drawn with its world's
    /// own set (Quadratum, San Fransokyo: KH3 lights its CG characters with a studio key; the game's sun alone leaves a CG face
    /// flat when it is behind him). The KH outfits take `rig`.
    pub rig_own: f32,
}

impl Default for Settings {
    fn default() -> Self {
        // (HY, 2026-10-16) `r.hybrid` was DEFAULT 1 (the user asked for Sora and his friends in Elden Ring's own render pass).
        // (RSF2, 2026-10-18, FINAL7: "DOES THIS LOOK FIXED TO YOU") it was DEFAULT 0 for FINAL8-FINAL10 (the world behind a KH
        // character printed on him; `r.hybrid 0` stopped it).
        // (RSF2 round 8, 2026-10-19, the user: "In the next deploy, also make the new rendering that r.hybrid=1 enables the
        // default") DEFAULT 1 AGAIN, by the user's decision (after round 5's mask rule, round 7's KH-only capture and round 8's
        // per-outfit binding of the game's view-projection). `r.hybrid = 0` = today's light (the rig).
        Settings {
            on: true,
            cast: false,
            game: 1.0,
            rig: 0.0,
            spec: 1.0,
            sh: 1.0,
            band_lo: 0.22,
            band_hi: 1.30,
            band_soft: 0.10,
            band_mix: 0.55,
            shade: 0.62,
            high: 0.0,
            ao: 1.0,
            fog: 0.0,
            wrap: 0.0,
            gloss: 0.3,
            lightset: 0,
            edge: 1.0,
            amb_sub: 1.0,
            self_shadow: 0.0,
            lift: 1.0,
            match_cos: 0.94,
            search: 2.0,
            debug: 0,
            rig_own: 0.35,
            mask: 1.0,
            fit: 6.0, // (RSF round 19) the probes (5: hc_blur, round 16)
            gbclear: 1.0,
            sun: 1.0,
            sun_soft: 1.0,
            sun_bias: BIAS_STEPS,
            sun_fit: 1.0,
            shadow: 1.0,
            shade_ratio: 0.35,
        }
    }
}

/// (HY) The third version's keys from the control file (`get`: the key's number, absent = the default), over `d` (frame.rs
/// builds the older keys and takes these with `..hybrid::with_keys(d, ..)`).
pub fn with_keys(d: Settings, get: &dyn Fn(&str) -> Option<f32>) -> Settings {
    let f = |key: &str, default: f32| get(key).filter(|v| v.is_finite()).unwrap_or(default);
    Settings {
        amb_sub: f("r.hybrid_amb_sub", d.amb_sub).clamp(0.0, 1.5),
        self_shadow: f("r.hybrid_self", d.self_shadow).clamp(0.0, 1.0),
        lift: f("r.hybrid_lift", d.lift).clamp(0.0, 8.0),
        match_cos: f("r.hybrid_match", d.match_cos).clamp(-1.0, 1.0),
        search: f("r.hybrid_search", d.search).clamp(0.0, 2.0),
        debug: f("r.hybrid_debug", d.debug as f32).clamp(0.0, 6.0) as u8,
        rig_own: f("r.hybrid_rig_own", d.rig_own).clamp(0.0, 1.0),
        mask: f("r.hybrid_mask", d.mask).clamp(0.0, 1.0),
        fit: f("r.hybrid_fit", d.fit).clamp(0.0, 6.0).round(), // (RSF round 19) 6: the probes
        gbclear: f("r.hybrid_gbclear", d.gbclear).clamp(0.0, 1.0),
        sun: f("r.hybrid_sun", d.sun).clamp(0.0, 1.0),
        sun_soft: f("r.hybrid_sun_soft", d.sun_soft).clamp(0.0, 4.0),
        sun_bias: f("r.hybrid_sun_bias", d.sun_bias).clamp(0.0, 1000.0),
        sun_fit: f("r.hybrid_sun_fit", d.sun_fit).clamp(0.0, 1.0),
        shadow: f("r.hybrid_shadow", d.shadow).clamp(0.0, 1.0),
        shade_ratio: f("r.hybrid_shade", d.shade_ratio).clamp(0.03, 1.0),
        ..d
    }
}

static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);
/// `r.hybrid_dump`: a change asks for one dump (the renderer takes it at its next hybrid frame).
pub(crate) static DUMP_WANTED: AtomicBool = AtomicBool::new(false);
/// (for the log) frames lit by the game since the last status line, frames that fell back, dumps written.
pub(crate) static FRAMES_ON: AtomicU32 = AtomicU32::new(0);
pub(crate) static FRAMES_BACK: AtomicU32 = AtomicU32::new(0);
pub(crate) static DUMPS: AtomicU32 = AtomicU32::new(0);
/// Why the latest frame fell back to the rig (for the status line).
pub(crate) static WHY: Mutex<&'static str> = Mutex::new("not tried yet");
/// The `*_hy` / `*_gb` shader sets were loaded with the pack (gfx.rs read_shaders).
pub(crate) static SHADERS: AtomicBool = AtomicBool::new(false);
/// (offline harness only, lib.rs `test_hybrid_light`) the game's light constants are not required (the harness has none).
pub(crate) static TEST: AtomicBool = AtomicBool::new(false);
/// (HY, offline harness only: lib.rs `test_capsules`) everybody's capsules of the last frame drawn with `TEST` on.
pub(crate) static TEST_CAPS: Mutex<Vec<(Vec3, Vec3, f32)>> = Mutex::new(Vec::new());
/// (HY) the capture's state for the status line (its size, the game targets it read and their states at the end of the
/// deferred stage, as last seen)
pub(crate) static CAPTURE: Mutex<String> = Mutex::new(String::new());

pub fn set(s: Settings) {
    if let Ok(mut g) = SETTINGS.lock() {
        *g = Some(s);
    }
}

pub fn settings() -> Settings {
    SETTINGS.lock().ok().and_then(|g| *g).unwrap_or_default()
}

pub fn request_dump() {
    DUMP_WANTED.store(true, Ordering::Relaxed);
}

pub(crate) fn fell_back(why: &'static str) {
    FRAMES_BACK.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut w) = WHY.lock() {
        *w = why;
    }
}

/// The status line's part.
pub fn status() -> String {
    let s = settings();
    format!(
        "(HY) hybrid {}: {} frame(s) lit by the game's own render pass, {} on today's light (latest reason: {}); shaders {}; capture {}; dumps {}{}; (RSF2 round 11) G-buffer targets 3 / 4 cleared under the KH characters in {} frame(s) (r.hybrid_gbclear {}); (RSF2 round 12) the sun from the game's cascades at each KH texel: {} frame(s), {} cascade read(s) (r.hybrid_sun {}, r.hybrid_sun_soft {}); cascade sets this second: {}; (RSF2 round 14) the KH characters in the game's half-size effect depth: {} pass(es) of {} split(s); (RSF2 round 15) KH caster maps drawn in {} frame(s), read by the capture in {}; (CAPPERF) capture at its level of detail in {} frame(s), full in {} (r.hybrid_lod_px {})",
        if s.on { "on" } else { "OFF (r.hybrid = 0)" },
        FRAMES_ON.swap(0, Ordering::Relaxed),
        FRAMES_BACK.swap(0, Ordering::Relaxed),
        WHY.lock().map(|mut w| std::mem::replace(&mut *w, "none this second")).unwrap_or("?"),
        match (SHADERS.load(Ordering::Relaxed), READ.load(Ordering::Relaxed)) {
            (true, _) => "the *_hy3 / *_gb3 sets",
            (false, 2) => "today's sets (r.hybrid was 0 when the outfit was read)",
            _ => "today's sets (the pack has no complete *_hy3 + *_gb3 set, or no hycap.ps.dxil)",
        },
        CAPTURE.lock().map(|c| c.clone()).unwrap_or_default(),
        DUMPS.load(Ordering::Relaxed),
        if s.debug > 0 { format!("; DEBUG VIEW {} (r.hybrid_debug)", s.debug) } else { String::new() },
        GB_CLEARED.swap(0, Ordering::Relaxed),
        s.gbclear,
        crate::gfx::SUN_FRAMES.swap(0, Ordering::Relaxed),
        crate::gfx::SUN_READS.swap(0, Ordering::Relaxed),
        s.sun,
        s.sun_soft,
        crate::gfx::SUN_SETS.lock().map(|mut v| std::mem::take(&mut *v).iter().map(|(k, n)| format!("[{k}] x{n}")).collect::<Vec<_>>().join(" ")).unwrap_or_default(),
        crate::gfx::FX_DRAWN.swap(0, Ordering::Relaxed),
        crate::gfx::FX_SPLITS.swap(0, Ordering::Relaxed),
        crate::gfx::KH_MAPS_DRAWN.swap(0, Ordering::Relaxed),
        crate::gfx::KH_MAPS_USED.swap(0, Ordering::Relaxed),
        LOD_FRAMES.swap(0, Ordering::Relaxed),
        FULL_FRAMES.swap(0, Ordering::Relaxed),
        f32::from_bits(LOD_PX.load(Ordering::Relaxed))
    )
}

/// (RSF2 round 11) frames whose G-buffer targets 3 / 4 were cleared under the KH characters (gfx.rs)
pub(crate) static GB_CLEARED: AtomicU32 = AtomicU32::new(0);

// ---------------------------------------------------------------------------- (CAPPERF) the capture's level of detail

/// (CAPPERF, the user: the game drops from ~60 to ~30 fps when the camera is close to Sora; MEASURED with passtime.rs: the
/// capture ~12 ms of ~14 ms of our GPU there, ~1 ms at play distance) `r.hybrid_lod_px = P` (default `LOD_PX_DEFAULT`; 0 = never):
/// while the KH characters' own boxes (each one's scissor box, summed: about the pixels the capture really works on) cover P
/// pixels or more, the capture runs at its LEVEL OF DETAIL (er_char.wgsl hycap, er.hy[17].z = 1): the shade ratio's lattice at
/// twice the step (the same reach) and skipped in full sun, and the KH maps' maximum searched once per filter in hc_sun_vis. Off
/// below 0.8 P (no switching back and forth at the edge).
pub const LOD_PX_DEFAULT: f32 = 1_500_000.0;
static LOD_PX: AtomicU32 = AtomicU32::new(LOD_PX_DEFAULT.to_bits());
static LOD_ON: AtomicBool = AtomicBool::new(false);
/// frames captured at the level of detail / at full detail since the last status line
pub(crate) static LOD_FRAMES: AtomicU32 = AtomicU32::new(0);
pub(crate) static FULL_FRAMES: AtomicU32 = AtomicU32::new(0);
/// (CAPPERF) Where the level of detail goes in the block: row 17, component z (the capture shader alone reads row 17).
pub const LOD_ROW: usize = 17;
pub const LOD_COMPONENT: usize = 2;

pub fn set_lod_px(px: f32) {
    LOD_PX.store(if px.is_finite() { px.max(0.0) } else { LOD_PX_DEFAULT }.to_bits(), Ordering::Relaxed);
}

/// (CAPPERF) The next state of the level of detail: on from `lod_px` pixels, off below 0.8 x; `lod_px` <= 0 = always off.
pub fn lod_next(was: bool, px: f64, lod_px: f32) -> bool {
    if !(lod_px > 0.0) || !px.is_finite() {
        return false;
    }
    let at = f64::from(lod_px);
    if was { px >= 0.8 * at } else { px >= at }
}

/// (CAPPERF) This frame's level of detail from the pixels of the KH characters' boxes (gfx.rs, with the capture's box row);
/// counted for the status line.
pub(crate) fn lod_frame(px: f64) -> bool {
    let on = lod_next(LOD_ON.load(Ordering::Relaxed), px, f32::from_bits(LOD_PX.load(Ordering::Relaxed)));
    LOD_ON.store(on, Ordering::Relaxed);
    if on { &LOD_FRAMES } else { &FULL_FRAMES }.fetch_add(1, Ordering::Relaxed);
    on
}

/// (CAPPERF) The pixels a set of screen boxes covers, each counted once (overlaps twice): the capture's work, roughly.
pub fn boxes_px(boxes: &[[i32; 4]]) -> f64 {
    boxes.iter().map(|b| f64::from((b[2] - b[0]).max(0)) * f64::from((b[3] - b[1]).max(0))).sum()
}

// ---------------------------------------------------------------------------- the G-buffer's targets

/// DXGI format numbers (kept as plain numbers: this module is tested without Direct3D).
pub mod fmt {
    pub const R16G16B16A16_TYPELESS: i32 = 9;
    pub const R16G16B16A16_FLOAT: i32 = 10;
    pub const R10G10B10A2_TYPELESS: i32 = 23;
    pub const R10G10B10A2_UNORM: i32 = 24;
    pub const R11G11B10_FLOAT: i32 = 26;
    pub const R8G8B8A8_TYPELESS: i32 = 27;
    pub const R8G8B8A8_UNORM: i32 = 28;
    pub const R8G8B8A8_UNORM_SRGB: i32 = 29;
    pub const R8G8_TYPELESS: i32 = 48;
    pub const R8G8_UNORM: i32 = 49;
}

pub const ROLES: usize = 5;
/// (IMPL_ER78b) The targets the G-buffer pass writes: normal, parameters, surface colour. The subsurface and emissive
/// targets are left as the game wrote them (no barrier, no write): fewer of the game's resources touched.
pub const WRITTEN: usize = 3;
pub const ROLE_NAMES: [&str; ROLES] = ["gbuffer0_normal", "gbuffer1_params", "gbuffer2_albedo", "gbuffer3_sss", "gbuffer4_emissive"];

/// The role of a G-buffer target by its resource format (IMPL_ER78_STATE.md table), and the typed format our render
/// target view of it takes. The surface colour target (typeless) is written through an sRGB view, as the game reads it.
pub fn role_of(format: i32) -> Option<(usize, i32)> {
    match format {
        fmt::R10G10B10A2_UNORM | fmt::R10G10B10A2_TYPELESS => Some((0, fmt::R10G10B10A2_UNORM)),
        fmt::R8G8B8A8_UNORM => Some((1, fmt::R8G8B8A8_UNORM)),
        fmt::R8G8B8A8_TYPELESS | fmt::R8G8B8A8_UNORM_SRGB => Some((2, fmt::R8G8B8A8_UNORM_SRGB)),
        fmt::R8G8_UNORM | fmt::R8G8_TYPELESS => Some((3, fmt::R8G8_UNORM)),
        fmt::R16G16B16A16_FLOAT | fmt::R16G16B16A16_TYPELESS => Some((4, fmt::R16G16B16A16_FLOAT)),
        _ => None,
    }
}

/// One target of the game's last G-buffer list: pointer, resource format, size, its state at the end of that list.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GbTarget {
    pub resource: usize,
    pub format: i32,
    pub width: u64,
    pub height: u32,
    pub state: i32,
}

const RENDER_TARGET: i32 = 0x4;
const SHADER_READ: i32 = 0x40 | 0x80;

/// VALIDATION of the G-buffer as the survey saw it this frame: exactly one target per role, all of the scene depth's size,
/// each resting either as a render target or in a shader-resource state (nothing else is put a barrier on). Returns the
/// targets in role order.
pub fn order(targets: &[GbTarget], width: u64, height: u32) -> Result<[GbTarget; ROLES], &'static str> {
    if targets.len() != ROLES {
        return Err("the G-buffer list did not have five targets");
    }
    let mut out: [Option<GbTarget>; ROLES] = [None; ROLES];
    for t in targets {
        let Some((role, _)) = role_of(t.format) else { return Err("a G-buffer target has an unknown format") };
        if out[role].is_some() {
            return Err("two G-buffer targets of the same role");
        }
        if t.width != width || t.height != height {
            return Err("a G-buffer target is not of the scene depth's size");
        }
        if t.resource == 0 {
            return Err("a G-buffer target is unknown");
        }
        let resting = t.state == RENDER_TARGET || (t.state != 0 && t.state & !SHADER_READ == 0);
        if !resting {
            return Err("a G-buffer target rests in a state we do not put barriers on");
        }
        out[role] = Some(*t);
    }
    let mut done = [GbTarget::default(); ROLES];
    for (slot, t) in done.iter_mut().zip(out) {
        *slot = t.ok_or("a G-buffer role is missing")?;
    }
    // (IMPL_ER78b) the targets we write rest in ONE state (the game moves its G-buffer together): anything else is a
    // pattern we do not know
    if done[..WRITTEN].iter().any(|t| t.state != done[0].state) {
        return Err("the G-buffer targets rest in different states");
    }
    Ok(done)
}

/// (HY) A state a pixel shader may read a resource in while nobody writes it: PIXEL_SHADER_RESOURCE set, and none of
/// RENDER_TARGET, UNORDERED_ACCESS, DEPTH_WRITE, COPY_DEST, RESOLVE_DEST.
pub fn readable(state: i32) -> bool {
    const PIXEL_SRV: i32 = 0x80;
    const WRITES: i32 = 0x4 | 0x8 | 0x10 | 0x400 | 0x1000;
    state != 0 && state & PIXEL_SRV != 0 && state & WRITES == 0
}

/// (RSF) THE REACH OF A DRAWN KH CHARACTER for his screen box (the hybrid's G-buffer pass and its light capture): a ball of
/// `BONE_REACH_CM` round EVERY bone of his pose, whoever he is (Sora, Donald, Goofy, any later one: no bone names), so that a
/// long snout, hanging ears, a hat, big shoes or hair spikes lie in his box. Before, the box came from his named capsules
/// only: a part outside it kept the world's G-buffer under him and took the light captured from what lies behind him.
pub const BONE_REACH_CM: f32 = 30.0;

pub fn bone_balls(component: &[kh_anim::Trs], to_world: &kh_anim::Trs) -> Vec<(Vec3, Vec3, f32)> {
    component.iter().map(|t| t.then(to_world).translation).filter(|p| p.is_finite()).map(|p| (p, p, BONE_REACH_CM)).collect()
}

/// (RSF) A model drawn without a pose of its own (a weapon in its reference pose, a team prop): its cooked bounds (component
/// space origin, extent, radius; kh_look `Library::bounds_of`) as one ball where it stands.
pub fn bounds_ball(to_world: &kh_anim::Trs, bounds: (Vec3, Vec3, f32)) -> (Vec3, Vec3, f32) {
    let centre = kh_anim::Trs { translation: bounds.0, ..kh_anim::Trs::IDENTITY }.then(to_world).translation;
    (centre, centre, bounds.2 * to_world.scale.max_element().max(1.0e-3))
}

/// (HY) Two screen boxes (x0, y0, x1, y1) as one.
pub fn union_box(a: Option<[i32; 4]>, b: Option<[i32; 4]>) -> Option<[i32; 4]> {
    match (a, b) {
        (Some(a), Some(b)) => Some([a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]),
        (a, b) => a.or(b),
    }
}

/// The light target the colour shaders read: R11G11B10 float, the size of the scene.
pub fn light_ok(format: i32, width: u64, height: u32, scene: (u64, u32)) -> bool {
    format == fmt::R11G11B10_FLOAT && (width, height) == scene
}

// ---------------------------------------------------------------------------- the light

fn luma(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    if b <= a {
        return if x < a { 0.0 } else { 1.0 };
    }
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// THE STAIRCASE (er_char.wgsl `hy_band` is the same formula): L = received light / the white level. Below the step the
/// shade band (its own variation kept under a square root: black stays black), a flat lit band above it, a highlight band
/// above `band_hi`, and beyond 1 the light as it is (a torch right on him still brightens him).
pub fn band(l: f32, s: &Settings) -> f32 {
    let lo = s.band_lo.max(1.0e-3);
    let soft = s.band_soft.max(1.0e-4);
    let dark = s.shade * (l / lo).clamp(0.0, 1.0).sqrt();
    let s1 = smoothstep(lo - soft, lo + soft, l);
    let s2 = smoothstep(s.band_hi - soft, s.band_hi + soft, l);
    dark + (1.0 - dark) * s1 + s.high * s2 + (l - 1.0).max(0.0)
}

/// What the colour shader does to the light (KH units: scene light x `k`, the rig's factor): ambient occlusion
/// (`mask_ao` = min(mask.r, mask.g) at the pixel) to its strength, then the staircase on its brightness, its hue kept.
pub fn tone(light_kh: Vec3, mask_ao: f32, white: f32, s: &Settings) -> Vec3 {
    let ao = mask_ao.clamp(0.05, 1.0);
    let e = light_kh.max(Vec3::ZERO) * ao.powf(s.ao - 1.0);
    let l = luma(e) / white.max(1.0e-6);
    if l <= 1.0e-5 {
        return Vec3::ZERO;
    }
    let t = l + (band(l, s) - l) * s.band_mix;
    e * (t / l)
}

/// THE FRAME BLOCK of the `*_hy` / `*_gb` shaders (er_char.wgsl `hy`, after the blood block). `on` = this frame is lit by
/// the game (all checks passed); `white` = the rig's white level (kh-look's reference white); `k` = 1 / the rig's out
/// scale (scene light -> KH units); `fog_colour` the game's fog ambient colour (scene units). Off: zeros (the shaders do
/// what the `*_bl` set does).
/// (HY) 12 rows: the *_hy3 / *_gb3 shaders' block (er_char.wgsl KH_HY3). The older *_hy2 shaders read its first 6 rows, as
/// before (they are no longer picked by this renderer).
/// (RSF2 round 12) 16 rows: rows 10..15 are the cascades' (gfx.rs writes them with the capture, `sun_rows`); only the capture
/// shader (hycap, ER_HYCAP) declares them. The block is the frame block's LAST part, so the rows added at its end change nothing
/// for the colour shaders, which declare 12.
pub const BLOCK_FLOATS: usize = 92;

/// (HY) What the frame gives the block besides the keys.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameIn {
    /// the rig's white level (kh-look's reference white), scene -> KH units (1 / out scale), the game's fog colour (scene)
    pub white: f32,
    pub k: f32,
    pub fog: Vec3,
    /// the sun: unit vector TO it in GAME space and its light per unit N.L on a white surface (SCENE units: Col_0 x the
    /// calibration / pi); none = no sun term (no own shadow, no sun specular from the hybrid)
    pub sun: Option<(Vec3, Vec3)>,
    /// the drawn outfit has its world's own set (cgfield.rs active): `rig_own` instead of `rig`; its look's lift
    pub own_set: bool,
    pub lift: f32,
}

pub fn block(s: &Settings, on: bool, f: &FrameIn) -> [f32; BLOCK_FLOATS] {
    let mut out = [0.0f32; BLOCK_FLOATS];
    if !on {
        return out;
    }
    let (sun_dir, sun_light, self_shadow) = match f.sun.filter(|(d, c)| d.is_finite() && c.is_finite() && d.length() > 0.5) {
        Some((d, c)) => (d.normalize(), c.max(Vec3::ZERO), s.self_shadow.clamp(0.0, 1.0)),
        None => (Vec3::ZERO, Vec3::ZERO, 0.0),
    };
    let rig = if f.own_set { s.rig_own } else { s.rig };
    let lift = (s.lift * if f.lift.is_finite() && f.lift > 0.0 { f.lift } else { 1.0 }).clamp(0.0, 8.0);
    let rows: [[f32; 4]; 23] = [
        [1.0, s.game, rig, s.spec],
        [s.band_lo, s.band_hi, s.band_soft, s.band_mix.clamp(0.0, 1.0)],
        [s.shade, s.high, s.ao, s.sh.max(0.0)],
        [f.white, f.k, s.wrap.clamp(0.0, 1.0), s.lightset.min(1) as f32 / 255.0],
        [f.fog.x * f.k, f.fog.y * f.k, f.fog.z * f.k, (s.fog / 100.0).max(0.0)],
        [s.edge.clamp(0.0, 1.0), s.amb_sub, s.gloss.clamp(0.0, 1.0), s.debug.min(6) as f32],
        [sun_dir.x, sun_dir.y, sun_dir.z, self_shadow],
        [sun_light.x, sun_light.y, sun_light.z, lift],
        [s.match_cos, s.search.clamp(0.0, 2.0).round(), if s.mask > 0.5 { 1.0 } else { 0.0 }, s.fit.clamp(0.0, 7.0).round()], // (RSF round 19) 6 the probes (7 their weight: testing aid)
        // (RSF2 round 5) the capture's box (x0, y0, x1, y1 px): gfx.rs writes it with the capture (`box_row`)
        [0.0; 4],
        // (RSF2 round 12) the cascades: [10] count, the filter width (texels), the depth bias (atlas steps), the normal offset (m);
        // [11..15] each cascade's viewport in the atlas (x, y, width, height px): gfx.rs writes them with the capture (`sun_rows`)
        [0.0; 4],
        [0.0; 4],
        [0.0; 4],
        [0.0; 4],
        [0.0; 4],
        [0.0; 4],
        // (RSF2 round 12) [16] the sun's strength fitted on the KH texels (hycap `hc_wide`): on, its least and most (x the model)
        [if s.sun_fit > 0.5 { 1.0 } else { 0.0 }, 1.0, 8.0, s.shadow.clamp(0.0, 1.0)],
        // (RSF2 round 13) [17] fit 3: the shade ratio used where none can be measured around a texel (r.hybrid_shade)
        [s.shade_ratio.clamp(0.03, 1.0), 0.65, 0.0, 0.0],
        // (RSF2 round 15) [18] the KH caster maps' cascade viewport, [19] (on, map size, self tolerance, its row), [20..22] the
        // party's feet: gfx.rs writes them with the capture (`kh_rows`)
        [0.0; 4],
        [0.0; 4],
        [0.0; 4],
        [0.0; 4],
        [0.0; 4],
    ];
    for (k, row) in rows.iter().enumerate() {
        out[k * 4..k * 4 + 4].copy_from_slice(row);
    }
    out
}

/// (RSF2 round 5) Where the capture's box goes in the block (row 9: x0, y0, x1, y1 in pixels; er_char.wgsl hycap `hc_mean`).
pub const BOX_ROW: usize = 9;

/// (RSF2 round 12) Where the cascades go in the block (rows 10..15; er_char.wgsl hycap `hc_sun_vis`), how many fit, how far
/// along his normal (m) the point is moved before it is looked up, and the depth bias in steps of the 16-bit atlas (no acne).
pub const SUN_ROW: usize = 10;
pub const CASC_MAX: usize = 5;
pub const NORMAL_OFFSET_M: f32 = 0.06;
pub const BIAS_STEPS: f32 = 3.0;

/// (RSF2 round 12) The six rows of the cascades: [count, filter width (texels), bias (atlas steps), normal offset (m)], then each
/// viewport (x, y, width, height in atlas pixels), at most `CASC_MAX`; no cascade = all zero (the capture keeps the round-5 mean).
pub fn sun_rows(soft: f32, bias: f32, viewports: &[[f32; 4]]) -> [f32; 24] {
    let mut out = [0.0f32; 24];
    let n = viewports.len().min(CASC_MAX);
    if n == 0 {
        return out;
    }
    out[..4].copy_from_slice(&[n as f32, soft.clamp(0.0, 4.0), bias, NORMAL_OFFSET_M]);
    for (k, v) in viewports.iter().take(n).enumerate() {
        out[4 + k * 4..8 + k * 4].copy_from_slice(v);
    }
    out
}

/// (RSF2 round 12) The cascades worth reading from the game's caster passes (their viewports in an atlas of `atlas` px): inside
/// the atlas, not the whole atlas (the game's cache copy pass, drawn with zero matrices), one per viewport (a cascade drawn twice
/// keeps its last pass), the smallest first, at most `CASC_MAX`. Returns the pass indices. (Which one a point takes is decided per
/// point by the finest texel among the cascades that hold it: `csm_visibility`, hycap `hc_sun_vis`.)
pub fn cascade_passes(atlas: (u32, u32), viewports: &[[f32; 4]]) -> Vec<usize> {
    let (w, h) = (atlas.0 as f32, atlas.1 as f32);
    let mut chosen: Vec<usize> = Vec::new();
    for (k, v) in viewports.iter().enumerate() {
        let inside = v[2] >= 1.0 && v[3] >= 1.0 && v[0] >= 0.0 && v[1] >= 0.0 && v[0] + v[2] <= w + 0.5 && v[1] + v[3] <= h + 0.5;
        let whole = v[2] >= w - 0.5 && v[3] >= h - 0.5;
        if !inside || whole {
            continue;
        }
        match chosen.iter().position(|c| viewports[*c] == *v) {
            Some(at) => chosen[at] = k,
            None => chosen.push(k),
        }
    }
    chosen.sort_by(|a, b| (viewports[*a][2] * viewports[*a][3]).total_cmp(&(viewports[*b][2] * viewports[*b][3])));
    chosen.truncate(CASC_MAX);
    chosen
}

/// (RSF2 round 12) THE LOOKUP, as hycap `hc_sun_vis` does it (mirrored here for the tests): a point (host space, m) moved along its
/// normal; of the cascades (`cam` SC_CameraPos, `vp` VC_MatrixViewProj rows, `viewport` its rectangle) whose picture holds it, the
/// one with the finest texel on the ground (the smallest world width per atlas texel); there a bilinear PCF over 3 x 3 texels
/// (`soft` scales its step): the atlas holds reversed depth (the nearer to the light the larger), lit where the stored depth is
/// not nearer than the point's by more than the bias. `None` = no cascade holds it.
pub fn csm_visibility(p: Vec3, n: Vec3, soft: f32, cascades: &[(Vec3, [glam::Vec4; 4], [f32; 4])], atlas: &dyn Fn(i32, i32) -> f32) -> Option<f32> {
    let q = p + n * NORMAL_OFFSET_M;
    let mut best: Option<(f32, glam::Vec3, [f32; 4])> = None;
    for (cam, vp, v) in cascades {
        let rel = (q - *cam).extend(1.0);
        let c = glam::Vec4::new(vp[0].dot(rel), vp[1].dot(rel), vp[2].dot(rel), vp[3].dot(rel));
        if c.w <= 1.0e-6 {
            continue;
        }
        let s = c.truncate() / c.w;
        if s.x.abs() > 0.97 || s.y.abs() > 0.97 || s.z <= 0.0 || s.z >= 1.0 {
            continue;
        }
        let texel = 2.0 / (vp[0].truncate().length() * v[2]).max(1.0e-9);
        if best.is_none_or(|b| texel < b.0) {
            best = Some((texel, s, *v));
        }
    }
    let (_, s, v) = best?;
    let px = glam::Vec2::new(v[0] + (s.x * 0.5 + 0.5) * v[2], v[1] + (0.5 - s.y * 0.5) * v[3]);
    let step = soft.max(0.0);
    let bias = BIAS_STEPS / 65535.0;
    // (the bilinear PCF: the tent over 3 x 3 texels sampled on a 4 x 4 grid; step 0 = the one texel)
    if step <= 0.0 {
        return Some(if atlas(px.x as i32, px.y as i32) <= s.z + bias { 1.0 } else { 0.0 });
    }
    let g = (px - glam::Vec2::splat(0.5 * step)) / step;
    let b = g.floor();
    let f = g - b;
    let wx = [1.0 - f.x, 1.0, 1.0, f.x];
    let wy = [1.0 - f.y, 1.0, 1.0, f.y];
    let mut lit = 0.0;
    for (j, wyj) in wy.iter().enumerate() {
        for (i, wxi) in wx.iter().enumerate() {
            let at = (b + glam::Vec2::new(i as f32 - 1.0, j as f32 - 1.0)) * step + glam::Vec2::splat(0.5 * step);
            if atlas(at.x.floor() as i32, at.y.floor() as i32) <= s.z + bias {
                lit += wxi * wyj;
            }
        }
    }
    Some(lit / 9.0)
}

/// (RSF2 round 15) Where the KH caster maps' rows go (rows 18..22), their size, and the self tolerance in atlas steps.
pub const KH_ROW: usize = 18;
pub const KH_MAP_SIZE: u32 = 2048;
pub const KH_SELF_STEPS: f32 = 4.0;

/// (RSF2 round 15) The five rows: the maps' cascade viewport, (on, size, self tolerance, the cascade's row index among the sun rows),
/// the party's feet (game space cm, w = 1 when there). None = off (all zero: the lookup as round 12).
/// (testing aid) `r.hybrid_kh_test = 1`: the owner rule off (every KH map shades every KH texel); see er_char.wgsl hc_sun_vis
pub static KH_TEST: AtomicU32 = AtomicU32::new(0);

pub fn kh_rows(maps: Option<([f32; 4], usize)>, party_feet: &[Vec3]) -> [f32; 20] {
    let mut out = [0.0f32; 20];
    let Some((vp, row)) = maps else { return out };
    out[..4].copy_from_slice(&vp);
    out[4..8].copy_from_slice(&[if KH_TEST.load(Ordering::Relaxed) == 1 { 2.0 } else { 1.0 }, KH_MAP_SIZE as f32, KH_SELF_STEPS, row as f32]);
    for (k, f) in party_feet.iter().take(3).enumerate() {
        out[8 + k * 4..12 + k * 4].copy_from_slice(&[f.x, f.y, f.z, 1.0]);
    }
    out
}

/// (RSF2 round 5) The box row's four floats.
pub fn box_row(bx: [i32; 4]) -> [f32; 4] {
    [bx[0] as f32, bx[1] as f32, bx[2] as f32, bx[3] as f32]
}

/// (HY) What `r.hybrid` reads the pack with: the *_hy3 / *_gb3 set was complete and the key on when the drawn outfit's
/// shaders were read (1), complete but the key off (2), not complete (0). A change of the key reloads the outfit (as `r.hair`),
/// so `r.hybrid 0` draws with exactly the shaders of today's path.
pub(crate) static READ: AtomicU32 = AtomicU32::new(0);

pub(crate) fn note_read(complete: bool, used: bool) {
    READ.store(if !complete { 0 } else if used { 1 } else { 2 }, Ordering::Relaxed);
}

pub fn reload_wanted() -> bool {
    match READ.load(Ordering::Relaxed) {
        1 => !settings().on,
        2 => settings().on,
        _ => false,
    }
}

/// (IMPL_ER78b) THE G-BUFFER PASS'S SCISSOR: the screen box around his body capsules and the Keyblade's (game space, cm;
/// `clip` = game space -> clip, w in metres), each end widened by its radius + 10 cm + `pad` pixels. A second limit beside
/// the depth test (EQUAL to his own early depth): nothing of the G-buffer outside this box can change. A capsule end behind
/// the camera: the whole picture (the depth test alone); no capsules: `None` (no pass).
pub fn scissor_box(caps: &[(Vec3, Vec3, f32)], clip: &glam::Mat4, tan_y: f32, width: u32, height: u32, pad: f32) -> Option<[i32; 4]> {
    if caps.is_empty() {
        return None;
    }
    let (mut low, mut high) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for (a, b, radius) in caps {
        for p in [*a, *b] {
            let c = *clip * p.extend(1.0);
            if !c.is_finite() || c.w < 0.05 {
                return Some([0, 0, width as i32, height as i32]);
            }
            let centre = glam::Vec2::new((c.x / c.w * 0.5 + 0.5) * width as f32, (0.5 - c.y / c.w * 0.5) * height as f32);
            let reach = ((radius * 0.01 + 0.1) / (c.w * tan_y.max(1.0e-3)) * height as f32 * 0.5).max(0.0) + pad;
            low = low.min(centre - glam::Vec2::splat(reach));
            high = high.max(centre + glam::Vec2::splat(reach));
        }
    }
    let r = [(low.x.floor() as i32).clamp(0, width as i32), (low.y.floor() as i32).clamp(0, height as i32), (high.x.ceil() as i32).clamp(0, width as i32), (high.y.ceil() as i32).clamp(0, height as i32)];
    (r[2] > r[0] && r[3] > r[1]).then_some(r)
}

// ---------------------------------------------------------------------------- the dump (`r.hybrid_dump`)

/// Bytes per texel of the formats the dump reads.
pub fn texel_bytes(format: i32) -> Option<usize> {
    match format {
        fmt::R16G16B16A16_FLOAT | fmt::R16G16B16A16_TYPELESS => Some(8),
        fmt::R10G10B10A2_UNORM | fmt::R10G10B10A2_TYPELESS | fmt::R11G11B10_FLOAT | fmt::R8G8B8A8_TYPELESS | fmt::R8G8B8A8_UNORM | fmt::R8G8B8A8_UNORM_SRGB => Some(4),
        fmt::R8G8_UNORM | fmt::R8G8_TYPELESS => Some(2),
        _ => None,
    }
}

fn half(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((h >> 10) & 0x1F) as i32;
    let m = (h & 0x3FF) as f32;
    match e {
        0 => sign * m * 2f32.powi(-24),
        31 => {
            if m == 0.0 {
                sign * f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => sign * (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

/// An unsigned small float of R11G11B10 (`mantissa` bits, 5 exponent bits).
fn small_float(v: u32, mantissa: u32) -> f32 {
    let e = (v >> mantissa) & 0x1F;
    let m = (v & ((1 << mantissa) - 1)) as f32 / (1 << mantissa) as f32;
    match e {
        0 => m * 2f32.powi(-14),
        31 => f32::INFINITY,
        _ => (1.0 + m) * 2f32.powi(e as i32 - 15),
    }
}

/// One texel as four floats (unorm formats as stored, i.e. the sRGB target's bytes / 255 without decoding).
pub fn decode(format: i32, b: &[u8]) -> [f32; 4] {
    let word = |at: usize| u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
    match format {
        fmt::R16G16B16A16_FLOAT | fmt::R16G16B16A16_TYPELESS => {
            let h = |k: usize| half(u16::from_le_bytes([b[2 * k], b[2 * k + 1]]));
            [h(0), h(1), h(2), h(3)]
        }
        fmt::R10G10B10A2_UNORM | fmt::R10G10B10A2_TYPELESS => {
            let w = word(0);
            [(w & 0x3FF) as f32 / 1023.0, ((w >> 10) & 0x3FF) as f32 / 1023.0, ((w >> 20) & 0x3FF) as f32 / 1023.0, (w >> 30) as f32 / 3.0]
        }
        fmt::R11G11B10_FLOAT => {
            let w = word(0);
            [small_float(w & 0x7FF, 6), small_float((w >> 11) & 0x7FF, 6), small_float(w >> 22, 5), 1.0]
        }
        fmt::R8G8_UNORM | fmt::R8G8_TYPELESS => [b[0] as f32 / 255.0, b[1] as f32 / 255.0, 0.0, 1.0],
        _ => [b[0] as f32 / 255.0, b[1] as f32 / 255.0, b[2] as f32 / 255.0, b[3] as f32 / 255.0],
    }
}

fn srgb(c: f32) -> f32 {
    let c = c.max(0.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// A picture of a region for the eye (RGBA8): unorm targets as stored (alpha forced opaque, the alpha written into a
/// second picture by the caller when wanted), float targets scaled so that their 98th percentile of brightness is 0.8
/// and sRGB-encoded. Returns the pixels and the scale used (1 for unorm).
pub fn picture(format: i32, texels: &[[f32; 4]]) -> (Vec<u8>, f32) {
    let float = matches!(format, fmt::R16G16B16A16_FLOAT | fmt::R16G16B16A16_TYPELESS | fmt::R11G11B10_FLOAT);
    let scale = if float {
        let mut l: Vec<f32> = texels.iter().map(|t| luma(Vec3::new(t[0], t[1], t[2]))).filter(|v| v.is_finite()).collect();
        l.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p = l.get((l.len() as f32 * 0.98) as usize).copied().unwrap_or(0.0);
        if p > 1.0e-9 {
            0.8 / p
        } else {
            1.0
        }
    } else {
        1.0
    };
    let mut out = Vec::with_capacity(texels.len() * 4);
    for t in texels {
        for c in 0..3 {
            let v = if float { srgb(t[c] * scale) } else { t[c] };
            out.push((v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
        }
        out.push(255);
    }
    (out, scale)
}

/// Writes one picture as PNG.
pub fn write_png(path: &std::path::Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())
}

/// The screen box of the dump: around his feet and head (pixels), padded, at most `max` square, inside the picture.
pub fn dump_box(feet: Option<(f32, f32)>, head: Option<(f32, f32)>, width: u32, height: u32, max: u32) -> Option<[u32; 4]> {
    let (f, h) = (feet?, head.or(feet)?);
    let (cx, cy) = ((f.0 + h.0) * 0.5, (f.1 + h.1) * 0.5);
    let reach = ((f.1 - h.1).abs().max((f.0 - h.0).abs()) * 0.75 + 48.0).clamp(64.0, max as f32 * 0.5);
    let x0 = (cx - reach).clamp(0.0, width as f32 - 1.0) as u32;
    let y0 = (cy - reach).clamp(0.0, height as f32 - 1.0) as u32;
    let x1 = ((cx + reach) as u32).clamp(x0 + 1, width);
    let y1 = ((cy + reach) as u32).clamp(y0 + 1, height);
    Some([x0, y0, x1, y1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(resource: usize, format: i32, state: i32) -> GbTarget {
        GbTarget { resource, format, width: 2560, height: 1440, state }
    }

    #[test]
    fn the_g_buffer_is_ordered_by_role_and_checked() {
        // (the survey's order: 24, 28, 27, 49, 10 — given here shuffled)
        let t = [target(5, 10, 0xC0), target(1, 24, 0xC0), target(3, 27, 0xC0), target(2, 28, 0xC0), target(4, 49, 0x80)];
        let o = order(&t, 2560, 1440).unwrap();
        assert_eq!(o.map(|t| t.resource), [1, 2, 3, 4, 5]);
        // four targets, a wrong size, a strange state, a doubled role, an unknown format: no hybrid
        assert!(order(&t[..4], 2560, 1440).is_err());
        assert!(order(&t, 1920, 1080).is_err());
        let mut odd = t;
        odd[0].state = 0x10;
        assert!(order(&odd, 2560, 1440).is_err());
        odd = t;
        odd[0].state = 0;
        assert!(order(&odd, 2560, 1440).is_err());
        // (IMPL_ER78b) written targets in different states; all render targets is fine
        odd = t;
        odd[2].state = 4;
        assert!(order(&odd, 2560, 1440).is_err());
        odd = t;
        for k in 0..5 {
            odd[k].state = 4;
        }
        assert!(order(&odd, 2560, 1440).is_ok());
        odd = t;
        odd[4].format = 28;
        assert!(order(&odd, 2560, 1440).is_err());
        odd = t;
        odd[4].format = 2;
        assert!(order(&odd, 2560, 1440).is_err());
        // the views: the typeless colour target through sRGB
        assert_eq!(role_of(27), Some((2, fmt::R8G8B8A8_UNORM_SRGB)));
        assert!(light_ok(26, 2560, 1440, (2560, 1440)) && !light_ok(10, 2560, 1440, (2560, 1440)) && !light_ok(26, 1280, 720, (2560, 1440)));
    }

    #[test]
    fn the_staircase_steps_keeps_black_and_bright_lights() {
        let s = Settings::default();
        // black stays black; the shade band below the step, the lit band flat above it
        assert_eq!(band(0.0, &s), 0.0);
        assert!((band(s.band_lo * 0.5, &s) - s.shade * 0.5f32.sqrt()).abs() < 1.0e-4);
        assert!((band(0.6, &s) - 1.0).abs() < 1.0e-4 && (band(0.9, &s) - 1.0).abs() < 1.0e-4);
        // monotonic through the step
        let mut last = 0.0;
        for k in 0..400 {
            let v = band(k as f32 * 0.01, &s);
            assert!(v + 1.0e-5 >= last, "at {}", k as f32 * 0.01);
            last = v;
        }
        // a light brighter than the ground's keeps brightening him
        assert!((band(3.0, &s) - 3.0).abs() < 1.0e-4);
        // a highlight band when asked for
        let h = Settings { high: 0.2, ..s };
        assert!((band(1.2, &h) - band(1.2, &s)).abs() < 0.2 && band(1.45, &h) > band(1.45, &s) + 0.19);
    }

    #[test]
    fn the_tone_keeps_the_hue_and_mixes_the_staircase() {
        let s = Settings::default();
        // a warm torch light at 0.6 of the white level: stepped up to the lit band, hue kept
        let warm = Vec3::new(1.0, 0.5, 0.2);
        let e = warm * (0.6 / luma(warm));
        let t = tone(e, 1.0, 1.0, &s);
        assert!((t.normalize() - e.normalize()).length() < 1.0e-5);
        let want = 0.6 + (1.0 - 0.6) * s.band_mix;
        assert!((luma(t) - want).abs() < 1.0e-3, "{}", luma(t));
        // band_mix 0 = the light as received; AO strength 1 = the game's AO untouched, 0 = divided out
        let raw = Settings { band_mix: 0.0, ..s };
        assert!((tone(e, 0.5, 1.0, &raw) - e).length() < 1.0e-5);
        let no_ao = Settings { ao: 0.0, ..raw };
        assert!((tone(e, 0.5, 1.0, &no_ao) - e * 2.0).length() < 1.0e-4);
        assert_eq!(tone(Vec3::ZERO, 1.0, 1.0, &s), Vec3::ZERO);
    }

    #[test]
    fn the_block_is_zero_when_off_and_carries_the_keys_when_on() {
        let s = Settings::default();
        let f = FrameIn { white: 3000.0, k: 50.0, fog: Vec3::ONE, ..Default::default() };
        assert_eq!(block(&s, false, &f), [0.0; BLOCK_FLOATS]);
        let f = FrameIn { fog: Vec3::new(0.01, 0.02, 0.03), ..f };
        let b = block(&s, true, &f);
        assert_eq!(b[0], 1.0);
        assert_eq!((b[4], b[5], b[6], b[7]), (s.band_lo, s.band_hi, s.band_soft, s.band_mix));
        assert_eq!((b[12], b[13]), (3000.0, 50.0));
        assert!((b[17] - 1.0).abs() < 1.0e-6 && b[19] == 0.0);
        assert_eq!(b[20], 1.0);
        assert!((b[22] - 0.3).abs() < 1.0e-6);
        let set1 = Settings { lightset: 7, ..s };
        assert!((block(&set1, true, &FrameIn { white: 1.0, k: 1.0, ..Default::default() })[15] - 1.0 / 255.0).abs() < 1.0e-7);
        // (HY) no sun: no own shadow, no sun term; with a sun: its direction (unit), its light and the own shadow's strength
        assert_eq!(&b[24..31], &[0.0; 7]);
        assert_eq!(b[31], 1.0);
        let sun = block(&s, true, &FrameIn { sun: Some((Vec3::new(0.0, 0.0, 2.0), Vec3::new(0.05, 0.04, 0.03))), lift: 1.5, ..f });
        assert_eq!(&sun[24..28], &[0.0, 0.0, 1.0, s.self_shadow]);
        assert_eq!(&sun[28..31], &[0.05, 0.04, 0.03]);
        assert!((sun[31] - 1.5).abs() < 1.0e-6);
        // the rig's share: `rig` for the KH outfits, `rig_own` for an own set; the debug view and the match in their rows
        assert_eq!(sun[2], s.rig);
        assert_eq!(block(&s, true, &FrameIn { own_set: true, ..f })[2], s.rig_own);
        let d = Settings { debug: 3, ..s };
        assert_eq!(block(&d, true, &f)[23], 3.0);
        assert_eq!((b[32], b[33]), (s.match_cos, 2.0));
        assert_eq!(&b[36..48], &[0.0; 12]);
    }

    /// (HY) the capture reads a game target only while it rests readable by pixel shaders with nobody writing it; the boxes join
    #[test]
    fn the_capture_reads_only_resting_targets_and_boxes_join() {
        assert!(readable(0x80) && readable(0xC0));
        assert!(!readable(0) && !readable(0x40) && !readable(0x4) && !readable(0x84) && !readable(0x90) && !readable(0x480));
        assert_eq!(union_box(Some([10, 20, 30, 40]), Some([0, 25, 50, 35])), Some([0, 20, 50, 40]));
        assert_eq!(union_box(None, Some([1, 2, 3, 4])), Some([1, 2, 3, 4]));
        assert_eq!(union_box(None, None), None);
    }

    /// (HY) a key change asks for the outfit again only when the complete set was read with the other value
    #[test]
    fn a_hybrid_key_change_reloads_only_a_complete_set() {
        note_read(false, false);
        assert!(!reload_wanted());
        note_read(true, settings().on);
        assert!(!reload_wanted());
        note_read(true, !settings().on);
        assert!(reload_wanted());
        note_read(false, false);
    }

    /// (RSF2 round 12) the cascades: chosen passes, the rows, and the lookup (a wall between the point and the sun: shadow; beside
    /// it: lit; the edge within the filter; the finest cascade that holds the point; outside every cascade: none)
    #[test]
    fn rsf12_the_sun_is_looked_up_in_the_games_own_cascades() {
        // the game's passes (IMPL_ER17, measured): the cache copy over the whole atlas, cascade 0 twice, two smaller ones twice
        let vps = [[0.0, 0.0, 8192.0, 4096.0], [0.0, 0.0, 4096.0, 4096.0], [0.0, 0.0, 4096.0, 4096.0], [4096.0, 0.0, 2048.0, 2048.0], [6144.0, 0.0, 2048.0, 2048.0], [4096.0, 0.0, 2048.0, 2048.0]];
        let chosen = cascade_passes((8192, 4096), &vps);
        assert_eq!(chosen, vec![5, 4, 2], "no whole-atlas pass, one per viewport (its last), the smallest first");
        let rows = sun_rows(1.0, BIAS_STEPS, &chosen.iter().map(|k| vps[*k]).collect::<Vec<_>>());
        assert_eq!(rows[..4], [3.0, 1.0, BIAS_STEPS, NORMAL_OFFSET_M]);
        assert_eq!(rows[4..8], [4096.0, 0.0, 2048.0, 2048.0]);
        assert_eq!(sun_rows(1.0, BIAS_STEPS, &[]), [0.0; 24]);
        // one cascade: the sun straight above (light camera 100 m up, looking down -y), +-20 m wide, depth 1 at the camera and
        // 0 at 200 m (reversed); a wall top at 3 m over x < 0, the ground (y = 0) elsewhere
        let cam = Vec3::new(0.0, 100.0, 0.0);
        let vp = [glam::Vec4::new(1.0 / 20.0, 0.0, 0.0, 0.0), glam::Vec4::new(0.0, 0.0, 1.0 / 20.0, 0.0), glam::Vec4::new(0.0, 1.0 / 200.0, 0.0, 0.5), glam::Vec4::new(0.0, 0.0, 0.0, 1.0)];
        let view = [0.0, 0.0, 1000.0, 1000.0];
        let depth_at = |height: f32| (height - 100.0) / 200.0 + 0.5;
        let atlas = |x: i32, _y: i32| if x < 500 { depth_at(3.0) } else { depth_at(0.0) };
        let cascades = [(cam, vp, view)];
        let up = Vec3::Y;
        assert_eq!(csm_visibility(Vec3::new(-2.0, 1.0, 0.0), up, 1.0, &cascades, &atlas), Some(0.0), "under the wall's top: shadow");
        assert_eq!(csm_visibility(Vec3::new(2.0, 1.0, 0.0), up, 1.0, &cascades, &atlas), Some(1.0), "beside it: lit");
        assert_eq!(csm_visibility(Vec3::new(2.0, 0.0, 0.0), up, 1.0, &cascades, &atlas), Some(1.0), "on the ground itself: no acne");
        let edge = csm_visibility(Vec3::new(0.0, 1.0, 0.0), up, 1.0, &cascades, &atlas).unwrap();
        assert!(edge > 0.0 && edge < 1.0, "the edge within the filter: {edge}");
        // a texel (4 cm) further in: the edge moves smoothly (bilinear), not in steps of a whole texel
        let a = csm_visibility(Vec3::new(0.01, 1.0, 0.0), up, 1.0, &cascades, &atlas).unwrap();
        let b = csm_visibility(Vec3::new(0.03, 1.0, 0.0), up, 1.0, &cascades, &atlas).unwrap();
        assert!(a < b && b < 1.0, "{a} {b}");
        assert_eq!(csm_visibility(Vec3::new(40.0, 1.0, 0.0), up, 1.0, &cascades, &atlas), None, "outside the cascade");
        // a second, coarser cascade over the same place (+-80 m into the same 1000 px): the finer one is taken
        let coarse = [glam::Vec4::new(1.0 / 80.0, 0.0, 0.0, 0.0), glam::Vec4::new(0.0, 0.0, 1.0 / 80.0, 0.0), vp[2], vp[3]];
        let all_lit = |_x: i32, _y: i32| 0.0;
        let both = [(cam, coarse, [1000.0, 0.0, 1000.0, 1000.0]), (cam, vp, view)];
        let pick = |x: i32, y: i32| if x >= 1000 { all_lit(x, y) } else { atlas(x, y) };
        assert_eq!(csm_visibility(Vec3::new(-2.0, 1.0, 0.0), up, 1.0, &both, &pick), Some(0.0), "the finer cascade decides");
        assert_eq!(csm_visibility(Vec3::new(-30.0, 1.0, 0.0), up, 1.0, &both, &pick), Some(1.0), "outside the fine one: the coarse one");
        // the shader's lookup is this one
        let text = include_str!("../shaders/er_char.wgsl");
        let cap = &text[text.find("#ifdef ER_HYCAP").unwrap()..];
        assert!(cap.contains("fn hc_sun_vis(") && cap.contains("vec2<i32>(24, r)") && cap.contains("vec2<i32>(3, r)") && cap.contains("er.hy[11 + best_r]"));
        assert!(cap.contains("abs(s.x) > 0.97") && cap.contains("hc_tap_kh(at, best_s.z, bias, owner, maps_on, pre)") && cap.contains("a <= z + bias"));
        let d = Settings::default();
        assert_eq!((d.sun, d.sun_soft), (1.0, 1.0));
        assert_eq!(with_keys(d, &|k: &str| (k == "r.hybrid_sun").then_some(0.0)).sun, 0.0);
        let b = block(&d, true, &FrameIn { white: 1.0, k: 1.0, ..Default::default() });
        assert_eq!(b.len(), 92);
        let r = kh_rows(Some(([0.0, 0.0, 4096.0, 4096.0], 2)), &[Vec3::new(1.0, 2.0, 3.0)]);
        assert_eq!(r[..8], [0.0, 0.0, 4096.0, 4096.0, 1.0, KH_MAP_SIZE as f32, KH_SELF_STEPS, 2.0]);
        assert_eq!(r[8..12], [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(r[12..20], [0.0; 8]);
        assert_eq!(kh_rows(None, &[]), [0.0; 20]);
        assert_eq!(b[16 * 4..16 * 4 + 4], [1.0, 1.0, 8.0, 1.0], "row 16: the sun's fitted strength, on, 1..8 x the model; (round 13) the world shadow's strength");
        assert_eq!(block(&with_keys(d, &|k: &str| (k == "r.hybrid_sun_fit").then_some(0.0)), true, &FrameIn { white: 1.0, k: 1.0, ..Default::default() })[16 * 4], 0.0);
        assert!(text.contains("hy: array<vec4<f32>, 23>") && cap.contains("er.hy[16].x > 0.5"));
    }

    /// (CAPPERF) the capture's level of detail: on from r.hybrid_lod_px pixels of the KH characters' boxes, off below 0.8 x, never
    /// with 0; its row (17.z) is zero in the block (gfx.rs writes it with the box) and free of every other use; the shader reads it
    /// in the shade ratio, its skip and the sun lookup only, and hc_wide skips the sun taps only where fits 3 / 5 / 6 read ao alone
    #[test]
    fn capperf_the_level_of_detail_switches_with_hysteresis() {
        assert!(!lod_next(false, 1_400_000.0, 1_500_000.0));
        assert!(lod_next(false, 1_500_000.0, 1_500_000.0));
        assert!(lod_next(true, 1_250_000.0, 1_500_000.0), "stays on above 0.8 x");
        assert!(!lod_next(true, 1_150_000.0, 1_500_000.0));
        assert!(!lod_next(true, 1.0e9, 0.0), "0 = never");
        assert!(!lod_next(false, f64::NAN, 1.0));
        assert_eq!(boxes_px(&[[0, 0, 10, 20], [5, 5, 4, 100]]), 200.0);
        let b = block(&Settings::default(), true, &FrameIn { white: 1.0, k: 1.0, ..Default::default() });
        assert_eq!(b[LOD_ROW * 4 + LOD_COMPONENT], 0.0);
        let text = include_str!("../shaders/er_char.wgsl");
        assert_eq!(text.matches("er.hy[17].z > 0.5").count(), 3, "the shade ratio, its skip, hc_sun_vis");
        let cap = &text[text.find("#ifdef ER_HYCAP").unwrap()..];
        assert!(cap.contains("let wide = hc_wide(t, n / len, er.hy[8].w > 3.5 && er.hy[8].w < 4.5);") && cap.contains("let wide = hc_wide(t, n / len, true);"));
        assert!(cap.contains("if (full && er.hy[16].x > 0.5"));
    }

    /// (RSF2 round 5) r.hybrid_mask: on by default, row 8.z of the block; 0 switches only it off; the capture's box row; the
    /// capture shader reads both rows (er_char.wgsl hycap), built with the hybrid block's layout (kh-shaderc --only hycap)
    #[test]
    fn rsf5_the_mask_rule_is_on_by_default_and_has_its_rows() {
        let d = Settings::default();
        assert_eq!(d.mask, 1.0);
        let f = FrameIn { white: 1.0, k: 1.0, ..Default::default() };
        let b = block(&d, true, &f);
        assert_eq!(b[8 * 4 + 2], 1.0, "row 8.z: the rule");
        assert_eq!(b[BOX_ROW * 4..BOX_ROW * 4 + 4], [0.0; 4], "row 9: the box, written by gfx.rs with the capture");
        let off = with_keys(d, &|k: &str| (k == "r.hybrid_mask").then_some(0.0));
        assert_eq!(block(&off, true, &f)[8 * 4 + 2], 0.0);
        assert_eq!(Settings { mask: d.mask, ..off }, d, "the key changes nothing else");
        assert_eq!(box_row([10, 20, 300, 400]), [10.0, 20.0, 300.0, 400.0]);
        let text = include_str!("../shaders/er_char.wgsl");
        let cap = &text[text.find("#ifdef ER_HYCAP").unwrap()..];
        assert!(cap.contains("er.hy[8].z > 0.5") && cap.contains("er.hy[9].xy") && cap.contains("fn hc_mean()"));
        // (round 11) the G-buffer's targets 3 / 4 cleared under the KH characters: on by default; its shader in the capture file
        assert_eq!(d.gbclear, 1.0);
        assert_eq!(with_keys(d, &|k: &str| (k == "r.hybrid_gbclear").then_some(0.0)).gbclear, 0.0);
        assert!(cap.contains("fn hygb_ps(") && cap.contains("@location(1) emissive"));
        // (round 10) the fit: on by default, row 8.w; the shader reads it
        assert_eq!(d.fit, 6.0); // (RSF round 19)
        assert_eq!(b[8 * 4 + 3], 6.0, "row 8.w: the fit (round 19: the probes; 5 = round 16 hc_blur)");
        assert_eq!(block(&with_keys(d, &|k: &str| (k == "r.hybrid_fit").then_some(1.0)), true, &f)[8 * 4 + 3], 1.0);
        assert_eq!(block(&with_keys(d, &|k: &str| (k == "r.hybrid_fit").then_some(0.0)), true, &f)[8 * 4 + 3], 0.0);
        assert!(cap.contains("er.hy[8].w > 0.5") && cap.contains("fn hc_close("));
        // (round 7) only KH texels are captured (with the rule): a pixel never takes the world's texel (Sora's hair fringe)
        assert!(cap.contains("er.hy[8].z > 0.5 && !hc_kh(textureLoad(hc_gb2, t, 0).xyz)"));
        let shaderc = include_str!("../../kh-shaderc/src/main.rs");
        assert!(shaderc.contains("[\"ER_HYCAP\", \"KH_EXTRA_LIGHTS\", \"KH_BLOOD\", \"KH_HYBRID\", \"KH_HY3\"]"));
    }

    /// (HY) r.hybrid is on by default; the keys of the third version read as given, clamped, absent = the default
    #[test]
    fn the_third_version_keys_read_and_clamp() {
        let d = Settings::default();
        assert!(d.on && !d.cast, "(RSF2 round 8) r.hybrid is on by default again, by the user's decision");
        let d = Settings { on: true, ..d };
        let none = with_keys(d, &|_| None);
        assert_eq!(none, d);
        let keys = |k: &str| match k {
            "r.hybrid_amb_sub" => Some(0.5),
            "r.hybrid_self" => Some(7.0),
            "r.hybrid_debug" => Some(2.0),
            "r.hybrid_search" => Some(9.0),
            "r.hybrid_lift" => Some(f32::NAN),
            _ => None,
        };
        let s = with_keys(d, &keys);
        assert_eq!((s.amb_sub, s.self_shadow, s.debug, s.search, s.lift), (0.5, 1.0, 2, 2.0, d.lift));
    }

    /// (IMPL_ER78b) the G-buffer pass's scissor: around his capsules only, the whole picture when one is behind the camera
    #[test]
    fn the_scissor_box_holds_his_body() {
        // a camera at the origin looking down +x (game space: x forward, z up): clip = (y, z, -, x) in metres
        let clip = glam::Mat4::from_cols(glam::Vec4::new(0.0, 0.0, 0.0, 0.01), glam::Vec4::new(0.01, 0.0, 0.0, 0.0), glam::Vec4::new(0.0, 0.01, 0.0, 0.0), glam::Vec4::new(0.0, 0.0, 0.1, 0.0));
        let body = [(Vec3::new(500.0, 0.0, 0.0), Vec3::new(500.0, 0.0, 150.0), 20.0)];
        let b = scissor_box(&body, &clip, 1.0, 2000, 1000, 0.0).unwrap();
        // 5 m ahead, 1.5 m tall: a narrow box in the middle reaching up from the horizon
        assert!(b[0] > 900 && b[2] < 1100 && b[1] < 400 && b[3] > 500 && b[3] < 600, "{b:?}");
        assert!(scissor_box(&[], &clip, 1.0, 2000, 1000, 0.0).is_none());
        let behind = [(Vec3::new(-100.0, 0.0, 0.0), Vec3::new(500.0, 0.0, 0.0), 20.0)];
        assert_eq!(scissor_box(&behind, &clip, 1.0, 2000, 1000, 0.0), Some([0, 0, 2000, 1000]));
    }

    #[test]
    fn the_dump_reads_the_game_formats() {
        // R11G11B10: 1.0 = exponent 15, mantissa 0 in each channel
        let one = (15u32 << 6) | ((15u32 << 6) << 11) | ((15u32 << 5) << 22);
        let t = decode(fmt::R11G11B10_FLOAT, &one.to_le_bytes());
        assert_eq!((t[0], t[1], t[2]), (1.0, 1.0, 1.0));
        // 0.5 in red (exponent 14), 0 elsewhere
        let t = decode(fmt::R11G11B10_FLOAT, &(14u32 << 6).to_le_bytes());
        assert_eq!((t[0], t[1], t[2]), (0.5, 0.0, 0.0));
        // R10G10B10A2: (1023, 0, 512, 3)
        let w: u32 = 1023 | (512 << 20) | (3 << 30);
        let t = decode(fmt::R10G10B10A2_UNORM, &w.to_le_bytes());
        assert!((t[0] - 1.0).abs() < 1.0e-6 && t[1] == 0.0 && (t[2] - 512.0 / 1023.0).abs() < 1.0e-6 && t[3] == 1.0);
        // half floats: 1.0 = 0x3C00, -2.0 = 0xC000, 0.5 = 0x3800
        let mut b = Vec::new();
        for h in [0x3C00u16, 0xC000, 0x3800, 0] {
            b.extend_from_slice(&h.to_le_bytes());
        }
        assert_eq!(decode(fmt::R16G16B16A16_FLOAT, &b), [1.0, -2.0, 0.5, 0.0]);
        assert_eq!(decode(fmt::R8G8_UNORM, &[255, 0]), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(texel_bytes(fmt::R8G8_UNORM), Some(2));
        assert_eq!(texel_bytes(61), None);
        // the picture of a float target is scaled to its bright end
        let (p, scale) = picture(fmt::R11G11B10_FLOAT, &[[0.01, 0.01, 0.01, 1.0]; 16]);
        assert!((scale - 80.0).abs() < 1.0e-3 && p[0] > 200);
        // the box: around feet and head, inside the picture
        let b = dump_box(Some((1280.0, 1000.0)), Some((1280.0, 600.0)), 2560, 1440, 768).unwrap();
        assert!(b[0] < 1280 && b[2] > 1280 && b[1] < 600 && b[3] > 1000 && b[2] - b[0] <= 768 && b[3] <= 1440);
        assert!(dump_box(None, None, 2560, 1440, 768).is_none());
        let edge = dump_box(Some((5.0, 1435.0)), None, 2560, 1440, 768).unwrap();
        assert!(edge[0] == 0 && edge[3] == 1440 && edge[2] > edge[0] && edge[3] > edge[1]);
    }
}
