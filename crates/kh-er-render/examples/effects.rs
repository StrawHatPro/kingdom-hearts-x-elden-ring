//! THE OFFLINE PROOF OF SORA'S EFFECTS in the Elden Ring renderer: the renderer outside the game
//! (as `standalone`), in the SCENE arrangement, with Sora simulated and animated
//! (`kh_host::rig`: the core on a flat floor, a training dummy, scripted pad input), the effect
//! side every host shares (`kh_host::vfx::EffectHost`) and `kh_er_render::vfx` drawing its lists.
//!
//! ```text
//! cargo run --release -p kh-er-render --example effects -- --script "stop 1.5; cast firaga; stop 6"
//!     [--shots 112,122,132] [--out DIR/NAME] [--dummy 700] [--camera 520,230,110] [--no-vfx]
//!     [--distort 0] [--solid] [--vsync] [--warm SECONDS]
//! ```
//!
//! THE "GAME" here draws what the renderer looks for in Elden Ring's frame (survey.rs): one
//! submission that takes four full-size targets and the scene depth as a G-buffer pass, then an
//! R11G11B10 float scene target with the depth (a flat checkered floor at game z = 0 and a sky,
//! written with their depth); after that submission the renderer's own list runs (Sora, then his
//! effects); a second submission tone-maps the scene target into the back buffer (the fitted
//! ACES curve the test app's camera uses) and presents. So what runs is the scene-arrangement
//! path of the mod, with a real depth buffer, on a device of our own.
//!
//! (IMPL_ER53) `--fxtest`: the deferred submission is two lists (G-buffer + floor, then a COPY of the scene depth, as
//! the game's depth copies / downsamples made after its G-buffer), and a FORWARD STAGE follows it: a glow decal whose
//! pattern comes from that depth copy (the floor seal of the user's screenshot), a teal panel behind Sora and an orange
//! one in front of him, additive, tested against the scene depth. `--early 0|1`: his early depth (default 1).
//!
//! (IMPL_ER80) `--glowtest`: the game's GLOW SOURCE (the deferred composite's second target, the Erdtree's emissive over the
//! sky, Sora not in it) and its GLOW CHAIN (blurred, added over the picture in the post submission); `--glow-mask 0|1` (the
//! renderer's glow mask, default 1), `--glow-scale X`. `--taa`: a TAA stand-in in its own list right after the velocity list
//! (as the game's TempAA); `--jitter`: the camera jittered by sub-pixel Halton(2,3) offsets each frame; `--late-taa 0|1`: the
//! late re-composite before the TAA (default 1) or after it (as IMPL_ER65); `--flicker-box X0,Y0,X1,Y1`: frame-to-frame
//! differences of the shots inside that box are printed (the hair-edge measure).
//! (ZFIN round 8) `--game-jitter`: only the "game" (its floor, its characters) jittered by the Halton(2,3) offsets, as Elden
//! Ring's TAA jitters its own scene while our renderer's camera (and so our effects) stays unjittered.
//! (ER88) `--no-focus`: the window is never activated (tool window off-screen, shown with SW_SHOWNOACTIVATE): use it on
//! every run while a game is being tested on the same desktop.
//!
//! (ER79) Outfits: `--outfit ID --own-clips [--use-kh3 LIST]` (its own clips as in the game, KH3 Sora's for the slots of
//! LIST), `--chain [--chain-his] [--chain-angle]` (the keychain simulated, from the drawn hand or his), `--stretch` (the
//! outfit's skin on the CPU: worst edge stretch, bones far from the animated pose, the leg guard's proof: cloth vertices
//! inside the leg capsules before / after the vertex guard), `--no-guard`, `--lids`, `--track`, `--trace-bone NAME`,
//! `--dump-sim FILE` (poses for kh-anim tests/outfit_stand.rs replay_outfit_sim). IMPL_ER79_STATE.md.
//!
//! (IMPL_ER80b) `--shafts FILE`: the GAME'S OWN LightShaft_CompositePS (FILE: extracted from gxposteffect.shaderbnd) draws a
//! radial sky blur (shaftsrc.ps.dxil, from the scene depth with him) over the picture in the post submission, made AFTER the
//! renderer's hooks are in (the renderer recognises it by its hash and masks it: `--shaft-mask 0|1`); `--shaft-at X,Y,S`:
//! the light's screen position (uv) and strength; `--shaft-half`: the composite draws at HALF resolution into a target of its own
//! that is then added over the scene (upadd.ps.dxil), as the game may (the renderer makes his mask at that size). `--front-glow`: a glowing panel IN FRONT of his legs written into the
//! glow source by the forward stage (it must still glow over him).
//!
//! One simulation step per presented frame. `--shots` writes the back buffer of those frames
//! (frame = step number, as the test app's `--shots`) to `<out>_<frame>.png`. Before the script
//! starts, frames are presented until the effect materials of Sora's own effects are made
//! (their pipeline states and textures are made on the renderer's worker thread).

use std::ffi::c_void;
use std::mem::ManuallyDrop;

// (ER party S4) Donald and Goofy DRAWN (posed rows), the camp pictures, the void and the team props (`--friends*`: see that file)
#[cfg(feature = "party")]
#[path = "party_draw.rs"]
mod party_draw;
// (ZDRAW) `--zdraw CHR`: Zettaflare's target drawn from its own Elden Ring model (examples/zdraw_harness.rs)
// (PORT30 housekeeping, RSF) moved into examples/effects_zdraw/: cargo built it as an example of its own (no main)
#[path = "effects_zdraw/zdraw_harness.rs"]
mod zdraw_harness;
use std::path::PathBuf;

use glam::{Mat4, Quat, Vec3};
use kh_anim::Trs;
use kh_er_render::vfx::{Geo, VfxOptions};
use kh_er_render::{math, Body, Camera, FrameInput, Options, Weapon};
use kh_host::audio::PoseTap;
use kh_host::rig::{component_to_world, Dummy, Rig, Script, STEP};
use kh_host::seam::{AudioQueue, EffectQueue};
use kh_host::vfx::{AssetGeo, EffectHost, Scene};
use windows::core::{w, Interface, PCSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct3D::{ID3DBlob, D3D_FEATURE_LEVEL_11_0, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST};
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

// (ER party S5) `--party SCRIPT` / `--party-root`: Donald's and Goofy's effects (examples/effects_party/mod.rs)
mod effects_party;

// (ER87) `--size WxH` (default 1280x720): the harness's resolution (the hair close-ups at 2560x1440)
static SIZE: std::sync::OnceLock<(u32, u32)> = std::sync::OnceLock::new();
fn width() -> u32 {
    SIZE.get().map_or(1280, |s| s.0)
}
fn height() -> u32 {
    SIZE.get().map_or(720, |s| s.1)
}
const SCENE_FORMAT: DXGI_FORMAT = DXGI_FORMAT_R11G11B10_FLOAT;
const SRV: D3D12_RESOURCE_STATES = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;

unsafe extern "system" fn window_proc(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    DefWindowProcW(window, message, w, l)
}

fn transition(resource: &ID3D12Resource, before: D3D12_RESOURCE_STATES, after: D3D12_RESOURCE_STATES) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 { Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER { pResource: unsafe { std::mem::transmute_copy(resource) }, Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, StateBefore: before, StateAfter: after }) },
    }
}

unsafe fn texture(device: &ID3D12Device, format: DXGI_FORMAT, flags: D3D12_RESOURCE_FLAGS, state: D3D12_RESOURCE_STATES, clear: Option<&D3D12_CLEAR_VALUE>) -> ID3D12Resource {
    let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: width() as u64, Height: height(), DepthOrArraySize: 1, MipLevels: 1, Format: format, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: flags, ..Default::default() };
    let mut out: Option<ID3D12Resource> = None;
    device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &desc, state, clear.map(|c| c as *const _), &mut out).unwrap();
    out.unwrap()
}

/// (HY) Where the scene of Elden Ring's kind keeps its constants in the harness's constant buffer (hylight2 / hyfog).
const HY_CONSTANTS: usize = 4096;

/// (HY) A noon light line of the user's log (sun high, clear: er\logs\kh-er.log): the scene's light when `--game-light` is not given.
const HY_NOON: &str = "  light: sun dir (0.352, 0.884, 0.308) colour (0.5000, 0.4806, 0.4616) sky [0.0, 0.0, 0.0] | at his feet: scene [0.0079, 0.0076, 0.0069] direct [0.0071, 0.0069, 0.0062] surface [0.1585, 0.1505, 0.1429] mask [0.95, 1.0, 1.0, 0.99] probed true | scene true exposure 21.287 pre 1.00 jitter (0.375, -0.056) px | he gets (white surface): key [0.0922, 0.0886, 0.0851] ambient [0.0078, 0.0073, 0.0072] + up [0.0, 0.0, 0.0] (rig x33.9; ground / formula this frame 1.000) | top of the picture [0.0203, 0.0183, 0.0166] (1 taps): reflection environment x1.57 | fog ambient [0.0073, 0.0104, 0.0099] toward the light [0.2088, 0.2016, 0.1148] opposite [0.0667, 0.1096, 0.1096]";

/// (HY) THE HARNESS'S SCENE OF ELDEN RING'S KIND for the hybrid's proof (`--hy3`; er_vfx.wgsl HY_SCENE): the `--game-light` line's
/// sun (Col_0 x the calibration / pi per N.L, as the renderer's rig takes it) and its undirected light (the rig's measured ambient
/// and sky, scene units), so that open, unshadowed ground gets what the rig gives; world casters (`--hy3-occ "ax,ay,az,bx,by,bz,
/// r;.."`, cm around his feet), local point lights (`--hy3-lights "x,y,z,radius,r,g,b;.."`, colour in units of the rig's white),
/// the occlusion's strength (`--hy3-ao`, 1), fog (`--hy3-fog "density per m,r,g,b"`, colour in units of the rig's white), and
/// `--hy3-cast` (his capsules in the sun's "cascades": `r.hybrid_cast` 1's world; default off, as the mod).
struct HyScene {
    sun_dir: Vec3,
    sun: Vec3,
    amb: Vec3,
    amb_up: Vec3,
    ao: f32,
    fog: Vec3,
    fog_density: f32,
    occ: Vec<(Vec3, Vec3, f32)>,
    lights: Vec<(Vec3, f32, Vec3)>,
    cast: bool,
    /// the rig's white in scene units (the sky's level)
    white: f32,
    /// (RSF2 round 5) `--hy3-print`: the stand-in's screen-space mask on his pixels is a wall's with a barred window (made
    /// without him: the mechanism of fix6/ref)
    print: u8,
}

impl HyScene {
    fn from_args(value: &dyn Fn(&str) -> Option<String>, cast: bool) -> HyScene {
        let text = value("--game-light").and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_else(|| HY_NOON.to_string());
        let l = text.lines().find_map(kh_er_render::cgfield::parse_log_line).unwrap_or_default();
        let cal = kh_er_render::light::CALIBRATION_DEFAULT;
        let r = kh_er_render::light::rig(&l, Vec3::Y, cal, 1.0);
        let white = kh_er_render::light::reference_white() * r.out_scale;
        let (sun_dir, sun) = match l.sun {
            Some((d, c)) => (math::game_dir_of_host(d), c * cal * std::f32::consts::FRAC_1_PI),
            None => (Vec3::Z, Vec3::ZERO),
        };
        let numbers = |s: &str| s.split(',').filter_map(|x| x.trim().parse::<f32>().ok()).collect::<Vec<f32>>();
        let occ = value("--hy3-occ").map(|v| v.split(';').map(numbers).filter(|n| n.len() == 7).map(|n| (Vec3::new(n[0], n[1], n[2]), Vec3::new(n[3], n[4], n[5]), n[6])).collect()).unwrap_or_default();
        let lights = value("--hy3-lights").map(|v| v.split(';').map(numbers).filter(|n| n.len() == 7).map(|n| (Vec3::new(n[0], n[1], n[2]), n[3], Vec3::new(n[4], n[5], n[6]) * white)).collect()).unwrap_or_default();
        let fog = value("--hy3-fog").map(|v| numbers(&v)).filter(|n| n.len() == 4).unwrap_or(vec![0.0, 0.0, 0.0, 0.0]);
        let hs = HyScene { sun_dir, sun, amb: r.ambient * r.out_scale, amb_up: r.ambient_up * r.out_scale, ao: value("--hy3-ao").and_then(|v| v.parse().ok()).unwrap_or(1.0), fog: Vec3::new(fog[1], fog[2], fog[3]) * white, fog_density: fog[0], occ, lights, cast, white, print: value("--hy3-print").and_then(|v| v.parse().ok()).unwrap_or(0) };
        println!("effects: (HY) the scene of Elden Ring's kind: sun {:?} x {:?}, undirected {:?} + up {:?}, white {white:.5}, {} caster(s), {} light(s), fog {} / m, his capsules cast {}", hs.sun_dir, hs.sun, hs.amb, hs.amb_up, hs.occ.len(), hs.lights.len(), hs.fog_density, hs.cast);
        hs
    }

    /// The constants of hylight2 / hyfog: the harness's frame block + this scene (er_vfx.wgsl ErFrame HY_SCENE).
    fn block(&self, frame: &[f32], feet: Vec3, caps: &[(Vec3, Vec3, f32)]) -> Vec<f32> {
        let mut b = frame[..48].to_vec();
        let on = if self.sun.max_element() > 0.0 { 1.0 } else { 0.0 };
        for v in [self.sun_dir.extend(on), self.sun.extend(if self.cast { 1.0 } else { 0.0 } + 2.0 * self.print.min(3) as f32), self.amb.extend(self.ao), self.amb_up.extend(self.white), feet.extend(self.lights.len().min(4) as f32), self.fog.extend(self.fog_density)] {
            b.extend_from_slice(&v.to_array());
        }
        for k in 0..4 {
            match self.occ.get(k) {
                Some((a, e, r)) => {
                    b.extend_from_slice(&a.extend(*r).to_array());
                    b.extend_from_slice(&e.extend(1.0).to_array());
                }
                None => b.extend_from_slice(&[0.0; 8]),
            }
        }
        for k in 0..4 {
            b.extend_from_slice(&self.lights.get(k).map_or([0.0; 4], |(p, r, _)| p.extend(*r).to_array()));
        }
        for k in 0..4 {
            b.extend_from_slice(&self.lights.get(k).map_or([0.0; 4], |(_, _, c)| c.extend(0.0).to_array()));
        }
        for k in 0..32 {
            match caps.get(k) {
                Some((a, e, r)) => {
                    b.extend_from_slice(&a.extend(*r).to_array());
                    b.extend_from_slice(&e.extend(1.0).to_array());
                }
                None => b.extend_from_slice(&[0.0; 8]),
            }
        }
        b
    }
}

/// A full-screen pipeline state of the harness.
unsafe fn screen_pipeline(device: &ID3D12Device, root: &ID3D12RootSignature, vertex: &[u8], pixel: &[u8], target: DXGI_FORMAT, depth: Option<DXGI_FORMAT>) -> ID3D12PipelineState {
    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
    formats[0] = target;
    let mut blend = D3D12_BLEND_DESC::default();
    blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
    let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::transmute_copy(root),
        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
        PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
        BlendState: blend,
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: depth.is_some().into(), DepthWriteMask: if depth.is_some() { D3D12_DEPTH_WRITE_MASK_ALL } else { D3D12_DEPTH_WRITE_MASK_ZERO }, DepthFunc: D3D12_COMPARISON_FUNC_ALWAYS, StencilEnable: false.into(), ..Default::default() },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 1,
        RTVFormats: formats,
        DSVFormat: depth.unwrap_or(DXGI_FORMAT_UNKNOWN),
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        ..Default::default()
    };
    device.CreateGraphicsPipelineState(&desc).expect("a harness pipeline state")
}

/// (ZFIN) The harness's Elden Ring characters this step: their capsules for the "game" (a, b, radius, tint) and the finale
/// state of the first (ZETTA's timeline: `--finale STEP`), as kh-er hands them to the renderer. A killed target falls over
/// (its death) and is hidden once it has crumbled away (kh-er zfinale.rs hides the body then; its last white shape fades: the
/// state goes on with `hidden_for` for 2 s). The capsules carry their character's index (each is drawn with its own stencil).
#[allow(clippy::type_complexity)]
/// (ZFIN round 7) `--finale-ghosts`: characters on the target's very place handed to the renderer as its neighbours.
static GHOSTS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// (ZFIN round 8) `--fx-steps`: the effect steps per presented frame, in turn.
static FX_STEPS: std::sync::OnceLock<Vec<usize>> = std::sync::OnceLock::new();
/// (ZFIN round 8) the Halton sequence of base `b` (the TAA jitter's offsets)
fn halton(mut i: u32, b: u32) -> f32 {
    let (mut f, mut r) = (1.0f32, 0.0f32);
    while i > 0 {
        f /= b as f32;
        r += f * (i % b) as f32;
        i /= b;
    }
    r
}

fn erchr_frame(chars: &[(Vec3, f32, f32, u8)], step: u32, finale_at: Option<u32>, survive: bool, push: Vec3, fall_on: bool) -> (Vec<(usize, Vec3, Vec3, f32, f32)>, Option<kh_er_render::finale::FinaleTarget>) {
    use kh_er_render::finale::{Capsule, FinaleTarget};
    let t = finale_at.map(|s| (step as f32 - s as f32) * STEP);
    let mut caps = Vec::new();
    let mut finale = None;
    let mut hidden = false;
    let mut fall = 0.0f32;
    if let Some(t) = t.filter(|t| *t >= 0.0) {
        let (body, neighbours) = (chars[0], &chars[1..]);
        let capsule = |c: &(Vec3, f32, f32, u8)| Capsule { feet: c.0, radius: 0.22 * c.1, half_height: 0.5 * c.1 };
        let (immerse, silhouette, crumble, end) = if survive {
            ((t / 0.5).clamp(0.0, 1.0), false, 0.0, 2.5)
        } else {
            ((t / 0.5).clamp(0.0, 1.0), true, ((t - 0.833) / 2.2).clamp(0.0, 1.0), 0.833 + 2.2 + 0.6)
        };
        // (round 3) as kh-er zfinale.rs: a silhouette the renderer breaks apart is hidden from its first crumble, else at 1;
        // `--finale-fall`: the death fall (ER holds the target in the beam: none by default)
        let breaks = kh_er_render::finale::breaks_apart(7);
        let hide_at = if breaks { 0.833 } else { 0.833 + 2.2 };
        if !survive {
            if fall_on {
                let s = ((t - 0.3) / 1.0).clamp(0.0, 1.0);
                fall = s * s * (3.0 - 2.0 * s) * 85f32.to_radians();
            }
            hidden = crumble >= 1.0 || (crumble > 0.0 && breaks);
        }
        let hidden_for = (hidden).then_some(t - hide_at);
        if t < end || hidden_for.is_some_and(|h| h < 2.0) {
            finale = Some(FinaleTarget { id: 7, body: capsule(&body), immerse, silhouette, crumble, neighbours: neighbours.iter().map(capsule).chain((0..GHOSTS.load(std::sync::atomic::Ordering::Relaxed)).map(|k| { let mut c = capsule(&body); c.feet += Vec3::new(4.0 * k as f32 + 3.0, 2.0, 0.0); c.radius *= 1.1; c })).collect(), hidden_for, push, dark: { let x = (t / 0.8).clamp(0.0, 1.0); x * x * (3.0 - 2.0 * x) } });
        }
    }
    for (k, (feet, h, yaw, _)) in chars.iter().enumerate() {
        if k == 0 && hidden {
            continue;
        }
        let f = Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0);
        let s = Vec3::new(-f.y, f.x, 0.0);
        let z = Vec3::Z;
        let tint = if k == 0 { 1.0 } else { 2.0 };
        let mut parts: Vec<(Vec3, Vec3, f32, f32)> = Vec::new();
        for side in [-1.0f32, 1.0] {
            parts.push((s * 0.09 * side * h + z * 0.06 * h, s * 0.08 * side * h + z * 0.47 * h, 0.065 * h, tint));
            parts.push((s * 0.21 * side * h + z * 0.78 * h, s * 0.24 * side * h + f * 0.06 * h + z * 0.47 * h, 0.048 * h, tint));
        }
        parts.push((z * 0.52 * h, z * 0.76 * h, 0.13 * h, tint));
        parts.push((z * 0.86 * h, z * 0.90 * h, 0.075 * h, tint));
        if k == 0 {
            parts.push((s * 0.24 * h + f * 0.08 * h + z * 0.46 * h, s * 0.26 * h + f * 0.55 * h + z * 0.80 * h, 0.018 * h, 3.0));
        }
        let angle = if k == 0 { fall } else { 0.0 };
        let mut q = Quat::from_axis_angle(s, angle);
        if (q * z).dot(f) > 0.0 {
            q = Quat::from_axis_angle(s, -angle);
        }
        let lift = z * (0.12 * h * angle.sin());
        for (a, b, r, tint) in parts {
            caps.push((k, *feet + q * a + lift, *feet + q * b + lift, r, tint));
        }
    }
    (caps, finale)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|k| args.get(k + 1)).cloned();
    let flag = |name: &str| args.iter().any(|a| a == name);
    // (ER87) `--size WxH`; `--hair 0|1` (the hair path, hair.rs: default 1); `--orbit-step D`: the camera turns D degrees
    // round him per presented frame once the script runs (the turn test of the hair's shimmer)
    if let Some((w, h)) = value("--size").and_then(|v| v.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))) {
        let _ = SIZE.set((w, h));
    }
    kh_er_render::hair::set_on(value("--hair").as_deref() != Some("0"));
    // (ER91) `--hair91 0` (r.hair91 = 0: ER87's hair files), `--hair-msaa 0` (r.hair_msaa = 0), `--hair-msaa-far M`
    kh_er_render::hair::set_v91(value("--hair91").as_deref() != Some("0"));
    kh_er_render::hair91::set_on(value("--hair-msaa").as_deref() != Some("0"));
    if let Some(m) = value("--hair-msaa-far").and_then(|v| v.parse::<f32>().ok()) {
        kh_er_render::hair91::set_far(m);
    }
    let orbit_step: f32 = value("--orbit-step").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    // (ER89, harness only; ported to the live tree by ER90) the kh-er off keys the harness had no flag for: `--own-off ID[,ID]` /
    // `--own-on ID[,ID]` (boc.<id>.own_shaders = 0 / 1), `--own-look 0` (boc.own_look = 0), `--blood-face 0` (blood.face = 0),
    // `--taa-char 0` (r.taa_char = 0), `--velocity-own 0` (r.velocity_own = 0)
    if let Some(ids) = value("--own-off") {
        kh_er_render::ownset::set_off(ids.split(',').map(|s| s.trim().to_string()).collect());
    }
    if let Some(ids) = value("--own-on") {
        kh_er_render::ownset::set_on(ids.split(',').map(|s| s.trim().to_string()).collect());
    }
    if value("--own-look").as_deref() == Some("0") {
        kh_er_render::cgfield::set_look_off(vec!["*".to_string()]);
    }
    if value("--blood-face").as_deref() == Some("0") {
        kh_er_render::blood::set_face(false);
    }
    if value("--taa-char").as_deref() == Some("0") {
        kh_er_render::set_taa_char(false);
    }
    if value("--velocity-own").as_deref() == Some("0") {
        kh_er_render::taa::set_velocity_own(false);
    }
    let project = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let pack = value("--pack").map(PathBuf::from).unwrap_or_else(|| project.join("build").join("sora_pack"));
    // (ER90) `--loadout FILE`: another loadout sheet (build/sora_tmp/xsi/loadout_xsi.json: level 99 + every XS ability)
    let loadout = value("--loadout").map(PathBuf::from).unwrap_or_else(|| project.join("sheets").join("port").join("sora-loadout-99.json"));
    let script_text = value("--script").unwrap_or_else(|| "stop 1.5; cast firaga; stop 6".to_string());
    let shots: Vec<u32> = value("--shots").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
    let out_prefix = value("--out").unwrap_or_else(|| "effects".to_string());
    let dummy_cm: f32 = value("--dummy").and_then(|v| v.parse().ok()).unwrap_or(700.0);
    // The camera: this far behind him, this high, looking at this height above his feet (cm), turned by `--orbit` degrees.
    let camera_numbers: Vec<f32> = value("--camera").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
    let (cam_back, cam_up, cam_look) = if camera_numbers.len() == 3 { (camera_numbers[0], camera_numbers[1], camera_numbers[2]) } else { (520.0, 235.0, 115.0) };
    let orbit: f32 = value("--orbit").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    // (CV1) `--conv-shot KIND[:SUBJECT]` (over | two | close | push, subject sora | donald; `--conv-push F` 0..1, `--conv-side 1|-1`):
    // THE CONVERSATION CAMERA'S FRAMING (kh_camera::conv::frame) of Sora and Donald (Donald where `--friends-at` puts him), its
    // lens; the faces' stage places are printed (`conv faces:`) for the bubble pictures over the frames
    let conv_shot: Option<(kh_host::conv::ShotKind, bool)> = value("--conv-shot").map(|v| {
        let (k, who) = v.split_once(':').unwrap_or((v.as_str(), "donald"));
        let kind = match k { "two" => kh_host::conv::ShotKind::TwoShot, "close" => kh_host::conv::ShotKind::CloseUp, "push" => kh_host::conv::ShotKind::PushIn, _ => kh_host::conv::ShotKind::OverShoulder };
        (kind, who != "sora")
    });
    let conv_push: f32 = value("--conv-push").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let conv_side: f32 = value("--conv-side").and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let conv_friends: Vec<f32> = value("--friends-at").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect()).filter(|v: &Vec<f32>| v.len() == 4).unwrap_or(vec![-40.0, -150.0, -40.0, 160.0]);
    let fov_v: f32 = conv_shot.map_or(0.8378, |(k, _)| 2.0 * ((k.fov().to_radians() * 0.5).tan() / (16.0 / 9.0)).atan());
    let vsync = flag("--vsync");
    // (seconds the warm-up may take)
    let warm_limit: f32 = value("--warm").and_then(|v| v.parse().ok()).unwrap_or(90.0);
    let exposure: f32 = value("--exposure").and_then(|v| v.parse().ok()).unwrap_or(1.0);
    // (IMPL_ER39, the light pass) `--dark F`: the floor and sky at F of their light; `--wall CM`: a wall across X at that
    // distance; `--albedo R,G,B`: the first G-buffer target is the game's kind of surface-colour target (typeless,
    // read as sRGB) cleared to that linear colour; `--hero 0|1|2` (2 = forced on); `--no-light`: the pass off.
    let dark: f32 = value("--dark").and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let wall: f32 = value("--wall").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let albedo: Option<[f32; 4]> = value("--albedo").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect::<Vec<f32>>()).filter(|v| v.len() == 3).map(|v| [v[0], v[1], v[2], 1.0]);
    let hero_mode: u8 = value("--hero").and_then(|v| v.parse().ok()).unwrap_or(0);
    // (IMPL_ER44) `--sora-dark F`: Sora's stand-in rig (key light and ambient) at F of its light, as the floor with
    // `--dark F` (the harness has no game light to read); `--no-pl`: his older pixel shaders (no point lights of his own).
    let sora_dark: f32 = value("--sora-dark").and_then(|v| v.parse().ok()).unwrap_or(1.0);
    kh_er_render::pointlight::want_own_shaders(!flag("--no-pl"));
    // (IMPL_ER60) `--blood BODY[,BLADE[,FRESH[,SEED]]]`: the blood on him (0..1); `--no-bl`: the *_pl set (no blood layer).
    kh_er_render::blood::set_shaders_wanted(!flag("--no-bl"));
    // (IMPL_ER59) `--outfit kh1`: one of Sora's outfits (PACK/outfits/<id>)
    // (ER73) `--no-kd`: without the outfit's own KineDriver
    kh_er_render::outfit::set_kinedriver(!flag("--no-kd"));
    // (ER79) `--no-guard`: without the outfit's leg guard (legguard.json); `--no-twosided`: its cloth one-sided (twosided.json)
    kh_er_render::outfit::set_leg_guard(!flag("--no-guard"));
    kh_er_render::outfit::set_cloth_twosided(!flag("--no-twosided"));
    // (ER83) `--no-grip-ik`: without the outfit's hand contact IK (handik.rs)
    kh_er_render::handik::set_enabled(!flag("--no-grip-ik"));
    // (FP) `--no-fx-cull`: one-sided mesh particles draw their back faces too (kh-er `fx.cull_one_sided = 0`, before FP);
    // `--upload-kb N`: the character pipeline's per-frame upload buffer capped at N KB (the proof that a routed chandelier
    // fragment that does not fit skips itself instead of failing the frame; no kh-er key)
    kh_er_render::vfx::CULL_ONE_SIDED.store(!flag("--no-fx-cull"), std::sync::atomic::Ordering::Relaxed);
    kh_er_render::set_upload_test_cap(value("--upload-kb").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0) * 1024);
    // (ER83) `--grip-main 0`: the main hand's fingers as retargeted (no copy of his fist)
    kh_er_render::handik::set_main_fingers(value("--grip-main").as_deref() != Some("0"));
    // (ER81) `--own-shaders 0`: an outfit drawn with the KH World set even when it has its own (own_shaders.json: Verum Rex)
    if value("--own-shaders").as_deref() == Some("0") {
        kh_er_render::ownset::set_off(vec!["*".to_string()]);
    }
    // (ER81) `--own-preset NAME`: the outfit's own set with another of its presets (verumrex: quadratum_game)
    if let (Some(p), Some(id)) = (value("--own-preset"), value("--outfit")) {
        kh_er_render::ownset::set_presets(vec![(id, p)]);
    }
    if let Some(id) = value("--outfit") {
        kh_er_render::set_outfit(&id);
    }
    // (ER85) `--game-light FILE`: Elden Ring's light of a field frame, the `light:` line of the live log (kh-er.log), in place
    // of the stand-in rig: the field's code path (light.rs rig, out_scale; pass `--exposure` = the line's exposure)
    // (IMPL_ER86) `--stable 0`: Sora's light as before IMPL_ER86 (stable.rs off); `--hero-anchor N`: where the hero light sits for
    // him (0 the camera, 1 ahead of his facing, 2 above)
    {
        let d = kh_er_render::stable::Settings::default();
        kh_er_render::stable::set_settings(kh_er_render::stable::Settings { on: value("--stable").as_deref() != Some("0"), hero_anchor: value("--hero-anchor").and_then(|v| v.parse().ok()).unwrap_or(d.hero_anchor), ..d });
    }
    if let Some(file) = value("--game-light") {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        match text.lines().find_map(kh_er_render::cgfield::parse_log_line) {
            Some(l) => {
                println!("effects: (ER85) the game's light of {file}: {l:?}");
                kh_er_render::cgfield::set_test_light(Some(l));
            }
            None => {
                eprintln!("effects: --game-light {file}: no `light:` line");
                std::process::exit(2);
            }
        }
    }
    // (ER85) `--own-field 0|1` (default 1): the field light of an outfit with its own set (cgfield.rs); `--own-field-keys
    // k=v,..`: ambient (the undirected share), smin / smax (the camera-relative key), nkey / namb (neutral), side / up (degrees)
    {
        let mut s = kh_er_render::cgfield::FieldShape { on: value("--own-field").as_deref() != Some("0"), ..Default::default() };
        for kv in value("--own-field-keys").unwrap_or_default().split(',') {
            let Some((k, v)) = kv.split_once('=') else { continue };
            let Ok(v) = v.trim().parse::<f32>() else { continue };
            match k.trim() {
                "ambient" => s.ambient_max = v,
                "smin" => s.studio_min = v,
                "smax" => s.studio_max = v,
                "nkey" => s.neutral_key = v,
                "namb" => s.neutral_ambient = v,
                "side" => s.side_deg = v,
                "up" => s.up_deg = v,
                "lift" => s.lift = v,
                "lift_share" => s.lift_share = v,
                "lift_min" => s.lift_min = v,
                _ => {}
            }
        }
        kh_er_render::cgfield::set_shape(s);
    }
    // (ER70) `--poses F1,F2,..` / `--pose-frames N`: fixed poses of Sora from files (see where the body is submitted)
    let poses: Vec<Vec<Trs>> = value("--poses")
        .map(|v| {
            v.split(',')
                .filter_map(|f| std::fs::read_to_string(f.trim()).ok())
                .map(|text| {
                    text.lines()
                        .filter_map(|line| {
                            let n: Vec<f32> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                            (n.len() == 10).then(|| Trs { translation: Vec3::new(n[0], n[1], n[2]), rotation: glam::Quat::from_xyzw(n[3], n[4], n[5], n[6]), scale: Vec3::new(n[7], n[8], n[9]) })
                        })
                        .collect()
                })
                .collect()
        })
        .unwrap_or_default();
    let pose_frames: usize = value("--pose-frames").and_then(|v| v.parse().ok()).unwrap_or(10);
    if let Some(v) = value("--blood").map(|v| v.split(',').filter_map(|x| x.trim().parse::<f32>().ok()).collect::<Vec<f32>>()).filter(|v| !v.is_empty()) {
        kh_er_render::blood::set(kh_er_render::blood::BloodLook { body: v[0], blade: v.get(1).copied().unwrap_or(v[0]), fresh: v.get(2).copied().unwrap_or(1.0), seed: v.get(3).copied().unwrap_or(0.0) });
    }

    let script = Script::parse(&script_text).unwrap_or_else(|e| {
        eprintln!("effects: script: {e}");
        std::process::exit(2);
    });
    let frames: usize = value("--frames").and_then(|v| v.parse().ok()).unwrap_or(script.len());
    let mut rig = Rig::new(&pack, &loadout, (dummy_cm > 0.0).then(|| Vec3::new(dummy_cm, 0.0, 0.0))).unwrap_or_else(|e| {
        eprintln!("effects: {e}");
        std::process::exit(2);
    });
    // (ER party S4) the party's figures (`--friends*`)
    #[cfg(feature = "party")]
    let mut party_harness = party_draw::PartyHarness::from_args(&args, &pack, rig.out.position, rig.out.yaw);
    // (ER79) `--own-clips` with `--outfit ID`: the outfit's own clips as in the game (kh-er boc.rs), without those of
    // `--use-kh3 LIST` (default: the outfit's default of `boc.<id>.use_kh3`; `--use-kh3 0` = all its own)
    if let (true, Some(id)) = (flag("--own-clips"), value("--outfit")) {
        let list = value("--use-kh3").unwrap_or_else(|| kh_anim::outfit_clips::default_for(&id).to_string());
        let names = rig.animator.set_own_clips_except(Some(&pack.join("outfits").join(&id)), &kh_anim::outfit_clips::kh3_clips(&list));
        println!("effects: {id}'s own clips ({}), KH3 Sora's for \"{list}\": {}", names.len(), names.join(" "));
    }
    let mut effects = EffectHost::load(&pack).unwrap_or_else(|e| {
        eprintln!("effects: {e}");
        std::process::exit(2);
    });
    // (IMPL_ER52) `--savepoint X,Y,ROOT_Z[,old]` (cm): KH3's save point with its root there; its effects on the floor (z = 0)
    // as KH3's BeginPlay trace puts them (kh_host::savepoint::ground_offset), or at the root with `old` (before IMPL_ER52)
    // (ORBS) `--orbs X,Y,Z[,SPACING[,GLOW]]` (cm): KH3's prize orbs as kh-er draws them (free effects, the prize's component
    // scale, the `prize.glow` gain kh-er gives them: sheet 3.27 by default), in a row across game Y centred there:
    // HP, MP, munny, focus (small sizes), then the large HP, MP, munny. Their names / scales: kh_prize::host::effect.
    let mut orbs: Option<(Vec<(&str, Vec3, f32)>, f32, Vec<u64>)> = value("--orbs").map(|v| {
        let n: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        let at = Vec3::new(n.first().copied().unwrap_or(-150.0), n.get(1).copied().unwrap_or(0.0), n.get(2).copied().unwrap_or(60.0));
        let step = n.get(3).copied().unwrap_or(45.0);
        let glow = n.get(4).copied().unwrap_or(3.27);
        let list = [("vs_plz_hpb0", 0.5), ("vs_plz_mpb0", 0.5), ("VS_PLZ_MNY0", 0.75), ("vs_plz_fop0", 0.5), ("vs_plz_hpb0", 1.0), ("vs_plz_mpb0", 1.0), ("VS_PLZ_MNY0", 1.0)];
        let count = list.len() as f32;
        let row = list.iter().enumerate().map(|(i, (name, scale))| (*name, at + Vec3::new(0.0, (i as f32 - (count - 1.0) * 0.5) * step, 0.0), *scale)).collect();
        println!("effects: (ORBS) prize orbs at {at:?}, {step} cm apart, prize.glow {glow}");
        (row, glow, Vec::new())
    });
    let mut savepoint = value("--savepoint").map(|v| {
        let n: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        let root = Vec3::new(n.first().copied().unwrap_or(300.0), n.get(1).copied().unwrap_or(150.0), n.get(2).copied().unwrap_or(92.0));
        let mut sp = kh_host::savepoint::SavePoint::new(kh_host::savepoint::SavePointData::load(&pack), root);
        if !v.ends_with("old") {
            let floor = (root.z >= 0.0 && root.z <= kh_host::savepoint::GROUND_TRACE).then(|| Vec3::new(root.x, root.y, 0.0));
            sp.effect_offset = kh_host::savepoint::ground_offset(root, floor, kh_host::savepoint::GROUND_MISS);
        }
        println!("effects: save point root {root:?}, its effects at {:?}", sp.effects_at());
        sp
    });
    // (IMPL_ER75) `--slash-world`: the swing arcs where KH3's rows put them (left in the world)
    effects.slash_follow = !flag("--slash-world");
    let started = std::time::Instant::now();
    let registered = effects.preload();
    println!("effects: {}; {registered} effects parsed in {:.1} s", effects.describe(), started.elapsed().as_secs_f32());
    // (ER party S5) `--party-root` (implied by `--party`): the party's second effect root, as kh-er's fx.rs reads it (its own
    // shading table and shaders; its materials / meshes as "party:<name>"); `--party "T WHO ACTION; .."`: Donald and Goofy
    let mut party_rig = value("--party").map(|s| {
        effects_party::PartyRig::new(&pack, &s).unwrap_or_else(|e| {
            eprintln!("effects: {e}");
            std::process::exit(2);
        })
    });
    let mut party_audio = party_rig.as_ref().map(|_| {
        effects_party::PartyAudio::new(&pack, value("--party-audio-log").as_deref()).unwrap_or_else(|e| {
            eprintln!("effects: (party) audio: {e}");
            std::process::exit(2);
        })
    });
    if flag("--party-root") || party_rig.is_some() {
        match effects.load_party_root(&pack) {
            Ok(line) => println!("effects: (party) {line}; {} parsed", effects.preload_party()),
            Err(e) => println!("effects: (party) NO PARTY ROOT: {e}"),
        }
    }
    // (ER90, XSI request 1) `--xsi`: the XS abilities' effect root (PACK/xsi/vfx), as kh-er's fx.rs reads it (`fx.xsi`, default
    // on there); the form weapons are drawn whenever the pack has PACK/xsi/weapons (`--no-forms`: never submitted).
    // XS presets for the moves (applied before the script's first step): `--xs-finisher spiral|zantetsuken|horizon|stamp|blast`
    // (the core's DEBUG finisher), `--xs-style P` (form gauge points), `--xs-shotlock K` (the chosen shotlock, 11 = Spectral
    // Rays), `--xs-mp-charge` (MP charge running: Light Wave), `--xs-mp` (MP full every step), `--xs-lock` (locked on the dummy)
    let (mut kinds_seen, mut state_seen) = (0usize, None);
    if flag("--xsi") {
        match effects.load_xsi_root(&pack) {
            Ok(line) => println!("effects: (XSI) {line}; {} parsed", effects.preload_xsi()),
            Err(e) => println!("effects: (XSI) NO XS ROOT: {e}"),
        }
    }

    // SAFETY: plain Win32 / Direct3D 12 object creation and use on one thread.
    unsafe {
        let instance = GetModuleHandleW(None).unwrap();
        let class = WNDCLASSW { lpfnWndProc: Some(window_proc), hInstance: instance.into(), lpszClassName: w!("kh-er-render-effects"), ..Default::default() };
        RegisterClassW(&class);
        // (ER88) `--no-focus`: the window never takes the focus (no activation, a tool window off-screen), so a harness run
        // cannot steal it from a game being tested on the same desktop
        let no_focus = flag("--no-focus");
        let window = if no_focus {
            let w = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW, w!("kh-er-render-effects"), w!("kh-er-render effects"), WS_POPUP, -32000, -32000, width() as i32, height() as i32, None, None, Some(instance.into()), None).unwrap();
            let _ = ShowWindow(w, SW_SHOWNOACTIVATE);
            w
        } else {
            CreateWindowExW(WINDOW_EX_STYLE(0), w!("kh-er-render-effects"), w!("kh-er-render effects"), WS_OVERLAPPEDWINDOW | WS_VISIBLE, 60, 60, width() as i32 + 16, height() as i32 + 39, None, None, Some(instance.into()), None).unwrap()
        };
        let factory: IDXGIFactory2 = CreateDXGIFactory1().unwrap();
        let mut device: Option<ID3D12Device> = None;
        D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device).unwrap();
        let device = device.unwrap();
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC { Type: D3D12_COMMAND_LIST_TYPE_DIRECT, ..Default::default() }).unwrap();
        let desc = DXGI_SWAP_CHAIN_DESC1 { Width: width(), Height: height(), Format: DXGI_FORMAT_R8G8B8A8_UNORM, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT, BufferCount: 3, SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, ..Default::default() };
        let swap: IDXGISwapChain3 = factory.CreateSwapChainForHwnd(&queue, window, &desc, None, None).unwrap().cast().unwrap();
        let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).unwrap();
        let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None).unwrap();
        list.Close().unwrap();
        let allocator2: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).unwrap();
        let list2: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator2, None).unwrap();
        list2.Close().unwrap();
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap();
        let mut fence_value = 0u64;

        // ---- the "game's" targets: four G-buffer targets, the scene depth, the HDR scene target
        // (IMPL_ER78) `--hybrid`: the game's own five G-buffer targets by format (normal, parameters, surface colour, SSS,
        // emissive), a light pass after the G-buffer list (hylight.ps.dxil: a warm sun and a blue sky light on the G-buffer's
        // normals into an R11G11B10 pair) whose diffuse target the renderer reads as the game's (lib.rs test_hybrid_light);
        // `--hybrid-off`: the same with r.hybrid = 0 (today's light, the *_bl shaders)
        // (HY) `--hy3`: the hybrid against the harness's scene of Elden Ring's kind (hylight2.ps.dxil, see HyScene)
        let hy3 = flag("--hy3");
        let hybrid = flag("--hybrid") || flag("--hybrid-off") || hy3;
        // `--hy-sun X,Y,Z` (host axes, Y up; default 0.6,0.6,-0.5), `--hy-light SUN,SKY` (shares of the rig's white; default
        // 0.95,0.12), `--hy-dump STEP` (r.hybrid_dump at that step, into the --out folder), `--hy-keys k=v,..` (hybrid keys)
        let numbers = |name: &str| value(name).map(|v| v.split(',').filter_map(|x| x.trim().parse::<f32>().ok()).collect::<Vec<f32>>()).unwrap_or_default();
        let hy_sun = Some(numbers("--hy-sun")).filter(|v| v.len() == 3).unwrap_or(vec![0.6, 0.6, -0.5]);
        let hy_levels = Some(numbers("--hy-light")).filter(|v| !v.is_empty()).unwrap_or(vec![0.95, 0.12]);
        let hy_dump: Option<u32> = value("--hy-dump").and_then(|v| v.parse().ok());
        if let Some(keys) = value("--hy-keys") {
            let mut s = kh_er_render::hybrid::Settings { on: true, ..Default::default() };
            for kv in keys.split(',') {
                let Some((k, v)) = kv.split_once('=') else { continue };
                let Ok(v) = v.trim().parse::<f32>() else { continue };
                match k.trim() {
                    "game" => s.game = v,
                    "rig" => s.rig = v,
                    "spec" => s.spec = v,
                    "sh" => s.sh = v,
                    "on" => s.on = v != 0.0,
                    "lo" => s.band_lo = v,
                    "hi" => s.band_hi = v,
                    "soft" => s.band_soft = v,
                    "mix" => s.band_mix = v,
                    "shade" => s.shade = v,
                    "high" => s.high = v,
                    "ao" => s.ao = v,
                    "fog" => s.fog = v,
                    "wrap" => s.wrap = v,
                    "edge" => s.edge = v,
                    // (HY) the third version's keys
                    "amb_sub" => s.amb_sub = v,
                    "self" => s.self_shadow = v,
                    "lift" => s.lift = v,
                    "match" => s.match_cos = v,
                    "search" => s.search = v,
                    "debug" => s.debug = v as u8,
                    "rig_own" => s.rig_own = v,
                    "mask" => s.mask = v, // (RSF2 round 5) r.hybrid_mask
                    "fit" => s.fit = v, // (RSF2 round 10) r.hybrid_fit
                    "gbclear" => s.gbclear = v, // (RSF2 round 11) r.hybrid_gbclear
                    _ => {}
                }
            }
            kh_er_render::set_hybrid(s);
        }
        let rt = D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
        let g_formats: Vec<DXGI_FORMAT> = if hybrid {
            vec![DXGI_FORMAT_R10G10B10A2_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R8G8B8A8_TYPELESS, DXGI_FORMAT_R8G8_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT]
        } else {
            (0..4).map(|k| if k == 0 && albedo.is_some() { DXGI_FORMAT_R8G8B8A8_TYPELESS } else { DXGI_FORMAT_R8G8B8A8_UNORM }).collect()
        };
        let g_buffer: Vec<ID3D12Resource> = g_formats.iter().map(|f| texture(&device, *f, rt, SRV, None)).collect();
        let scene = texture(&device, SCENE_FORMAT, rt, SRV, None);
        let light_pair: Vec<ID3D12Resource> = (0..2).map(|_| texture(&device, SCENE_FORMAT, rt, SRV, None)).collect();
        let clear = D3D12_CLEAR_VALUE { Format: DXGI_FORMAT_D32_FLOAT_S8X24_UINT, Anonymous: D3D12_CLEAR_VALUE_0 { DepthStencil: D3D12_DEPTH_STENCIL_VALUE { Depth: 0.0, Stencil: 0 } } };
        let depth = texture(&device, DXGI_FORMAT_R32G8X24_TYPELESS, D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL, SRV, Some(&clear));
        let rtv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 16, ..Default::default() }).unwrap();
        let rtv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        let rtv = |k: usize| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr + k * rtv_size };
        for (k, target) in g_buffer.iter().enumerate() {
            if g_formats[k] == DXGI_FORMAT_R8G8B8A8_TYPELESS {
                let desc = D3D12_RENDER_TARGET_VIEW_DESC { Format: DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D, ..Default::default() };
                device.CreateRenderTargetView(target, Some(&desc), rtv(k));
            } else {
                device.CreateRenderTargetView(target, None, rtv(k));
            }
        }
        // (the scene target's view: 5; the light pair: 6, 7)
        const SCENE_RTV: usize = 5;
        device.CreateRenderTargetView(&scene, None, rtv(SCENE_RTV));
        device.CreateRenderTargetView(&light_pair[0], None, rtv(6));
        device.CreateRenderTargetView(&light_pair[1], None, rtv(7));
        if hybrid {
            kh_er_render::test_hybrid_light(light_pair[0].as_raw() as usize);
            // (IMPL_ER78b: r.hybrid is off by default; --hybrid switches it on)
            if value("--hy-keys").is_none() || flag("--hybrid-off") {
                kh_er_render::set_hybrid(kh_er_render::hybrid::Settings { on: !flag("--hybrid-off"), ..Default::default() });
            }
        }
        // (HY) `--hy3`: the game's screen-space shadow mask stand-in (RGBA8, RTV 12: written by hylight2) and the scene of Elden
        // Ring's kind (HyScene: the sun of the `--game-light` line with world casters, its sky, local lights, fog)
        let hy_mask = texture(&device, DXGI_FORMAT_R8G8B8A8_UNORM, rt, SRV, None);
        device.CreateRenderTargetView(&hy_mask, None, rtv(12));
        if hy3 {
            kh_er_render::test_hybrid_mask(hy_mask.as_raw() as usize);
        }
        let hy_scene = hy3.then(|| HyScene::from_args(&value, flag("--hy3-cast")));
        // (HY) `--r-hybrid 0|1`: the key r.hybrid as the control file gives it (0 = today's path with today's files; default on)
        if let Some(v) = value("--r-hybrid") {
            kh_er_render::set_hybrid(kh_er_render::hybrid::Settings { on: v != "0", ..kh_er_render::hybrid::settings() });
        }
        let dsv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV, NumDescriptors: 1, ..Default::default() }).unwrap();
        let dsv = dsv_heap.GetCPUDescriptorHandleForHeapStart();
        device.CreateDepthStencilView(&depth, Some(&D3D12_DEPTH_STENCIL_VIEW_DESC { Format: DXGI_FORMAT_D32_FLOAT_S8X24_UINT, ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D, ..Default::default() }), dsv);
        let srv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, NumDescriptors: 16, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 }).unwrap();
        device.CreateShaderResourceView(&scene, None, srv_heap.GetCPUDescriptorHandleForHeapStart());
        // (IMPL_ER80) the glow source (RTV 8, SRV entry 3), the TAA's history (SRV entry 4) and its output (RTV 9)
        let glow_target = texture(&device, SCENE_FORMAT, rt, SRV, None);
        let history = texture(&device, SCENE_FORMAT, rt, SRV, None);
        let taa_out = texture(&device, SCENE_FORMAT, rt, SRV, None);
        device.CreateRenderTargetView(&glow_target, None, rtv(8));
        device.CreateRenderTargetView(&taa_out, None, rtv(9));
        {
            let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
            device.CreateShaderResourceView(&glow_target, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetCPUDescriptorHandleForHeapStart().ptr + 3 * srv_size });
            device.CreateShaderResourceView(&history, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetCPUDescriptorHandleForHeapStart().ptr + 4 * srv_size });
        }
        kh_er_render::set_glow_mask(value("--glow-mask").as_deref() != Some("0"));
        // (NF) KH3's camera fade (camfade.rs): `--camfade 0|1` (default: the sheet's, on), `--camfade-temporal 0|1` (the screen
        // door's phase stepping each frame; default: the sheet's camfade.temporal, off), `--camfade-lead N` (the member
        // the player controls, 0 = Sora)
        if let Some(v) = value("--camfade") {
            kh_er_render::camfade::set_enabled(Some(v != "0"));
        }
        kh_er_render::camfade::set_temporal(value("--camfade-temporal").map(|v| v != "0"));
        kh_er_render::camfade::set_friends_look_fade(value("--camfade-friends").as_deref() == Some("1")); // (RSF round 18) 1 = KH3's friend fade (r.camfade_friends)
        kh_er_render::finale::set_display(value("--finale-display").as_deref() != Some("0")); // (ZFIN round 9) r.finale_display
        // (RSF2) `--camfade-inside 0`: KH3's 0.5 with the camera inside a character (default: out entirely, r.camfade_inside)
        kh_er_render::camfade::set_inside_out(value("--camfade-inside").as_deref() != Some("0"));
        // (IMPL_ER80b) the light-shaft source (RTV 10; SRV entries 6..9 = the game composite's t0..t3), the scene depth (SRV 5),
        // a sampler heap (the game composite's s0..s3: linear clamp)
        kh_er_render::set_shaft_mask(value("--shaft-mask").as_deref() != Some("0"));
        let shafts_file = value("--shafts");
        let shaft_at: Vec<f32> = value("--shaft-at").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect()).filter(|v: &Vec<f32>| v.len() == 3).unwrap_or(vec![0.5, 0.12, 1.2]);
        let shaft_target = texture(&device, SCENE_FORMAT, rt, SRV, None);
        device.CreateRenderTargetView(&shaft_target, None, rtv(10));
        // (`--shaft-half`: the composite's own half-resolution target: RTV 11, SRV entry 10)
        let shaft_half = flag("--shaft-half");
        let half_target = {
            let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: (width() / 2) as u64, Height: height() / 2, DepthOrArraySize: 1, MipLevels: 1, Format: SCENE_FORMAT, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: rt, ..Default::default() };
            let mut out: Option<ID3D12Resource> = None;
            device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &desc, SRV, None, &mut out).unwrap();
            out.unwrap()
        };
        device.CreateRenderTargetView(&half_target, None, rtv(11));
        {
            let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
            device.CreateShaderResourceView(&half_target, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetCPUDescriptorHandleForHeapStart().ptr + 10 * srv_size });
        }
        {
            let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
            let at = |k: usize| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetCPUDescriptorHandleForHeapStart().ptr + k * srv_size };
            let dview = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
            device.CreateShaderResourceView(&depth, Some(&dview), at(5));
            for k in 6..10 {
                device.CreateShaderResourceView(&shaft_target, None, at(k));
            }
        }
        let sampler_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, NumDescriptors: 4, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 }).unwrap();
        {
            let size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER) as usize;
            for k in 0..4 {
                device.CreateSampler(&D3D12_SAMPLER_DESC { Filter: D3D12_FILTER_MIN_MAG_MIP_LINEAR, AddressU: D3D12_TEXTURE_ADDRESS_MODE_CLAMP, AddressV: D3D12_TEXTURE_ADDRESS_MODE_CLAMP, AddressW: D3D12_TEXTURE_ADDRESS_MODE_CLAMP, MaxLOD: f32::MAX, ..Default::default() }, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: sampler_heap.GetCPUDescriptorHandleForHeapStart().ptr + k * size });
            }
        }
        // (made after the warm-up, when the renderer's hooks are in: the game's composite pipeline and its root signature)
        let mut game_shaft: Option<(ID3D12RootSignature, ID3D12PipelineState)> = None;
        // (IMPL_ER80) `--wear N`: the wear form (5 = Second Form's costume, lib.rs set_wear_form)
        if let Some(form) = value("--wear").and_then(|v| v.parse().ok()) {
            kh_er_render::set_wear_form(form);
        }
        kh_er_render::set_late_sora_taa(value("--late-taa").as_deref() != Some("0"));
        let glow_scale: f32 = value("--glow-scale").and_then(|v| v.parse().ok()).unwrap_or(1.6);
        let flicker_box: Option<[u32; 4]> = value("--flicker-box").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect::<Vec<u32>>()).filter(|v| v.len() == 4).map(|v| [v[0], v[1], v[2], v[3]]);
        let mut flicker_prev: Option<Vec<u8>> = None;
        let mut flicker_prev2: Option<Vec<u8>> = None;
        let mut flicker_sum = (0.0f64, 0u64, 0u32);
        // (the second temporal difference |f(t) - 2 f(t-1) + f(t-2)|: smooth motion cancels, a frame-to-frame flicker stays)
        let mut flicker_sum2 = (0.0f64, 0u64, 0u32);
        let mut taa_frames = 0u32;
        // (IMPL_ER53) `--fxtest`: the "game's" depth copy (made after its G-buffer) and its view (entry 1)
        // (IMPL_ER80) `--glowtest` (implies the forward stage), `--taa`, `--jitter`
        let glowtest = flag("--glowtest");
        let taa = flag("--taa");
        // (ER91) `--taa-blend X`: the TAA stand-in's weight of the new frame (default 0.12); 1 = no history: the game's split and
        // passes as with the TAA, the picture as without it (the hair's shimmer measured independently of the TAA)
        let taa_blend: f32 = value("--taa-blend").and_then(|v| v.parse().ok()).unwrap_or(0.12);
        let fxtest = flag("--fxtest") || hybrid || glowtest || taa;
        let depth_copy = texture(&device, DXGI_FORMAT_R32G8X24_TYPELESS, D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL, SRV, None);
        {
            let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
            let desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
            device.CreateShaderResourceView(&depth_copy, Some(&desc), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetCPUDescriptorHandleForHeapStart().ptr + srv_size });
            // (IMPL_ER78) entry 2: the first G-buffer target (the normals the hybrid's light pass stand-in reads)
            device.CreateShaderResourceView(&g_buffer[0], None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetCPUDescriptorHandleForHeapStart().ptr + 2 * srv_size });
            // (HY) entry 11: the scene depth itself (with his early depth: the "game's" light passes read it)
            device.CreateShaderResourceView(&depth, Some(&desc), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetCPUDescriptorHandleForHeapStart().ptr + 11 * srv_size });
        }
        if let Some(v) = value("--early") {
            kh_er_render::set_early_depth(v != "0");
        } else {
            kh_er_render::set_early_depth(true);
        }
        // (IMPL_ER65) `--late 0|1` (the late re-composite, default 1), `--late-far M`; `--erdtree`: the 38.jpg situation: a sky
        // glow masked by a depth copy made BEFORE his early depth (like a pass of the game reading the prepass), a fire panel
        // in front of his legs, and a velocity pass (so that the late pass runs)
        kh_er_render::set_late_sora(value("--late").as_deref() != Some("0"), value("--late-far").and_then(|v| v.parse().ok()).unwrap_or(100.0));
        let erdtree = flag("--erdtree");
        // (IMPL_ER74) `--decals-late`: our decals after him (as before IMPL_ER74)
        kh_er_render::vfx::DECALS_EARLY.store(!flag("--decals-late"), std::sync::atomic::Ordering::Relaxed);
        // (IMPL_ER56) `--stencil 0|1` (his stencil, default 1), `--stencil-ref N`
        kh_er_render::set_early_stencil(value("--stencil").as_deref() != Some("0"), value("--stencil-ref").and_then(|v| v.parse().ok()).unwrap_or(-1));

        // ---- the harness's own two passes (the floor + sky, the tone mapping)
        let scene_table = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV, NumDescriptors: 1, BaseShaderRegister: 13, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
        // (IMPL_ER80) t12: the TAA stand-in's history
        let history_table = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV, NumDescriptors: 1, BaseShaderRegister: 12, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
        let parameters = [
            D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV, Anonymous: D3D12_ROOT_PARAMETER_0 { Descriptor: D3D12_ROOT_DESCRIPTOR { ShaderRegister: 0, RegisterSpace: 0 } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL },
            D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE, Anonymous: D3D12_ROOT_PARAMETER_0 { DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE { NumDescriptorRanges: 1, pDescriptorRanges: scene_table.as_ptr() } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL },
            D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE, Anonymous: D3D12_ROOT_PARAMETER_0 { DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE { NumDescriptorRanges: 1, pDescriptorRanges: history_table.as_ptr() } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL },
        ];
        let root_desc = D3D12_ROOT_SIGNATURE_DESC { NumParameters: 3, pParameters: parameters.as_ptr(), NumStaticSamplers: 0, pStaticSamplers: std::ptr::null(), Flags: D3D12_ROOT_SIGNATURE_FLAG_NONE };
        let mut blob: Option<ID3DBlob> = None;
        D3D12SerializeRootSignature(&root_desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, None).unwrap();
        let blob = blob.unwrap();
        let root: ID3D12RootSignature = device.CreateRootSignature(0, std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())).unwrap();
        // (ZFIN round 8b) env KH_HARNESS_VFX=DIR: the harness's own screen shaders (hylight2, hyfog ...) from DIR when the pack has none
        let shader = |name: &str| std::fs::read(pack.join("vfx").join("er").join(name)).or_else(|e| std::env::var_os("KH_HARNESS_VFX").map(|d| std::fs::read(std::path::Path::new(&d).join(name))).unwrap_or(Err(e))).unwrap_or_else(|e| panic!("{name}: {e} (cargo run --release -p kh-shaderc -- --vfx)"));
        let (screen_vs, floor_ps, tone_ps) = (shader("screen.vs.dxil"), shader("floor.ps.dxil"), shader("tone.ps.dxil"));
        let floor_pipeline = screen_pipeline(&device, &root, &screen_vs, &floor_ps, SCENE_FORMAT, Some(DXGI_FORMAT_D32_FLOAT_S8X24_UINT));
        let tone_pipeline = screen_pipeline(&device, &root, &screen_vs, &tone_ps, DXGI_FORMAT_R8G8B8A8_UNORM, None);
        // (IMPL_ER53) the forward stage's stand-in: additive, depth tested (GREATER_EQUAL, reversed Z), not written
        let fx_pipeline = fxtest.then(|| {
            let pixel = shader("fxtest3.ps.dxil");
            let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
            formats[0] = SCENE_FORMAT;
            let mut blend = D3D12_BLEND_DESC::default();
            blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC { BlendEnable: true.into(), LogicOpEnable: false.into(), SrcBlend: D3D12_BLEND_ONE, DestBlend: D3D12_BLEND_ONE, BlendOp: D3D12_BLEND_OP_ADD, SrcBlendAlpha: D3D12_BLEND_ZERO, DestBlendAlpha: D3D12_BLEND_ONE, BlendOpAlpha: D3D12_BLEND_OP_ADD, LogicOp: D3D12_LOGIC_OP_NOOP, RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8 };
            let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(&root),
                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: screen_vs.as_ptr() as *const c_void, BytecodeLength: screen_vs.len() },
                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
                BlendState: blend,
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO, DepthFunc: D3D12_COMPARISON_FUNC_GREATER_EQUAL, StencilEnable: false.into(), ..Default::default() },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 1,
                RTVFormats: formats,
                DSVFormat: DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                ..Default::default()
            };
            let state: ID3D12PipelineState = device.CreateGraphicsPipelineState(&desc).expect("the fxtest pipeline state");
            // (IMPL_ER56) the sky glow: no depth test, drawn where the stencil is 0 ("nothing drawn": the sky)
            let mut sky = desc.clone();
            let keep = D3D12_DEPTH_STENCILOP_DESC { StencilFailOp: D3D12_STENCIL_OP_KEEP, StencilDepthFailOp: D3D12_STENCIL_OP_KEEP, StencilPassOp: D3D12_STENCIL_OP_KEEP, StencilFunc: D3D12_COMPARISON_FUNC_EQUAL };
            sky.DepthStencilState = D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO, DepthFunc: D3D12_COMPARISON_FUNC_ALWAYS, StencilEnable: true.into(), StencilReadMask: 0xFF, StencilWriteMask: 0, FrontFace: keep, BackFace: keep };
            let sky_state: ID3D12PipelineState = device.CreateGraphicsPipelineState(&sky).expect("the sky glow pipeline state");
            // the "G-buffer"'s stencil: the floor pass writes 3 everywhere, then a depth-only pass at depth 0 (EQUAL) puts 0
            // back where nothing was drawn (the sky)
            let replace = D3D12_DEPTH_STENCILOP_DESC { StencilFailOp: D3D12_STENCIL_OP_KEEP, StencilDepthFailOp: D3D12_STENCIL_OP_KEEP, StencilPassOp: D3D12_STENCIL_OP_REPLACE, StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS };
            let mut floor = desc.clone();
            floor.PS = D3D12_SHADER_BYTECODE { pShaderBytecode: floor_ps.as_ptr() as *const c_void, BytecodeLength: floor_ps.len() };
            floor.BlendState = D3D12_BLEND_DESC::default();
            floor.BlendState.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            floor.DepthStencilState = D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL, DepthFunc: D3D12_COMPARISON_FUNC_ALWAYS, StencilEnable: true.into(), StencilReadMask: 0xFF, StencilWriteMask: 0xFF, FrontFace: replace, BackFace: replace };
            let floor_state: ID3D12PipelineState = device.CreateGraphicsPipelineState(&floor).expect("the floor + stencil pipeline state");
            let mut mark = floor.clone();
            mark.PS = D3D12_SHADER_BYTECODE::default();
            mark.NumRenderTargets = 0;
            mark.RTVFormats = [DXGI_FORMAT_UNKNOWN; 8];
            mark.DepthStencilState.DepthWriteMask = D3D12_DEPTH_WRITE_MASK_ZERO;
            mark.DepthStencilState.DepthFunc = D3D12_COMPARISON_FUNC_EQUAL;
            let mark_state: ID3D12PipelineState = device.CreateGraphicsPipelineState(&mark).expect("the sky marker pipeline state");
            // (IMPL_ER65) the glow with no depth and no stencil test; a "velocity pass" (R16G16 float, full screen)
            let mut free = sky.clone();
            free.DepthStencilState.StencilEnable = false.into();
            let sky_free: ID3D12PipelineState = device.CreateGraphicsPipelineState(&free).expect("the free glow pipeline state");
            let mut velo = free.clone();
            velo.RTVFormats[0] = DXGI_FORMAT_R16G16_FLOAT;
            velo.DSVFormat = DXGI_FORMAT_UNKNOWN;
            velo.BlendState = D3D12_BLEND_DESC::default();
            velo.BlendState.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            let velocity_state: ID3D12PipelineState = device.CreateGraphicsPipelineState(&velo).expect("the velocity pipeline state");
            (state, sky_state, floor_state, mark_state, sky_free, velocity_state)
        });
        // (ZFIN) `--erchars "X,Y,H[,YAW];.."`: ELDEN RING'S CHARACTERS (capsule figures, game cm: feet x, y, height, facing yaw
        // in degrees) drawn by the "game" into the scene with their depth and the characters' stencil 0x50, after the floor; the
        // first is the finale's target. `--finale STEP`: ZETTA's finale on it from that step (killed: immerse over 0.5 s, the
        // silhouette, its death fall, crumble from +0.83 s over 2.2 s, then hidden as kh-er hides it; `--finale-survive`: white
        // only, 2.5 s), the others its neighbours. `--finale-mode N` (r.finale), `--erchr-shader FILE` (erchr.ps.dxil, built by
        // kh-shaderc --vfx --only erchr; default the pack's vfx/er).
        // (a fifth number: that character's stencil value, default 0x50 = 80, the measured class of Elden Ring's characters)
        let erchars: Vec<(Vec3, f32, f32, u8)> = value("--erchars").map(|v| v.split(';').filter_map(|c| {
            let n: Vec<f32> = c.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            (n.len() >= 3).then(|| (Vec3::new(n[0], n[1], 0.0), n[2], n.get(3).copied().unwrap_or(180.0), n.get(4).map_or(0x50, |s| *s as u8)))
        }).collect()).unwrap_or_default();
        let erchr_pipeline: Option<ID3D12PipelineState> = (!erchars.is_empty()).then(|| {
            let pixel = match value("--erchr-shader") {
                Some(file) => std::fs::read(&file).unwrap_or_else(|e| panic!("{file}: {e}")),
                None => shader("erchr.ps.dxil"),
            };
            let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
            formats[0] = SCENE_FORMAT;
            let mut blend = D3D12_BLEND_DESC::default();
            blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            let replace = D3D12_DEPTH_STENCILOP_DESC { StencilFailOp: D3D12_STENCIL_OP_KEEP, StencilDepthFailOp: D3D12_STENCIL_OP_KEEP, StencilPassOp: D3D12_STENCIL_OP_REPLACE, StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS };
            let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(&root),
                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: screen_vs.as_ptr() as *const c_void, BytecodeLength: screen_vs.len() },
                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
                BlendState: blend,
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL, DepthFunc: D3D12_COMPARISON_FUNC_GREATER_EQUAL, StencilEnable: true.into(), StencilReadMask: 0xFF, StencilWriteMask: 0xFF, FrontFace: replace, BackFace: replace },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 1,
                RTVFormats: formats,
                DSVFormat: DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                ..Default::default()
            };
            device.CreateGraphicsPipelineState(&desc).expect("the Elden Ring characters' pipeline state")
        });
        let finale_at: Option<u32> = value("--finale").and_then(|v| v.parse().ok());
        let finale_survive = flag("--finale-survive");
        // (round 3) `--finale-push X,Y,Z`: the beam's direction at it (default from Sora to it), `--finale-fall`: its death fall
        let finale_push: Vec3 = value("--finale-push").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect::<Vec<f32>>()).filter(|v| v.len() == 3).map(|v| Vec3::new(v[0], v[1], v[2])).unwrap_or_else(|| erchars.first().map_or(Vec3::X, |c| c.0.normalize_or_zero()));
        let finale_fall = flag("--finale-fall");
        // (ZDRAW) `--zdraw CHR [--zdraw-pose bind|hurt] [--zdraw-groups u,0,..] [--zdraw-skip WORDS] [--zdraw-parts bd_m_1100,..]`: the first `--erchars` character is
        // that real Elden Ring model (zdraw_harness.rs)
        let mut zh = value("--zdraw").map(|c| zdraw_harness::ZHarness::new(&c, value("--zdraw-pose").as_deref() != Some("bind"), value("--zdraw-groups").as_deref(), value("--zdraw-skip").as_deref(), value("--zdraw-parts").as_deref(), erchars.first().map_or(Vec3::ZERO, |e| e.0), erchars.first().map_or(180.0, |e| e.2)));
        // (ZFIN round 7) `--finale-ghosts N`: N more characters on the target's very place (Malenia's extra c2120 rows), as its neighbours
        GHOSTS.store(value("--finale-ghosts").and_then(|v| v.parse().ok()).unwrap_or(0), std::sync::atomic::Ordering::Relaxed);
        let _ = FX_STEPS.set(value("--fx-steps").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default()); // (ZFIN round 8)
        if let Some(m) = value("--finale-mode").and_then(|v| v.parse::<u32>().ok()) {
            kh_er_render::finale::set_mode(m, 0.0, value("--finale-fade").and_then(|v| v.parse().ok()).unwrap_or(-1.0));
        } else if let Some(f) = value("--finale-fade").and_then(|v| v.parse().ok()) {
            kh_er_render::finale::set_mode(1, 0.0, f);
        }
        // (ZFIN) `--finale-over WORDS`: the effect materials drawn over the silhouette (r.finale_over)
        if let Some(words) = value("--finale-over") {
            kh_er_render::finale::set_over(words.split(',').map(str::to_string).collect());
        }
        // (IMPL_ER78) the light pass stand-in: two R11G11B10 targets, no depth
        let hy_light_pipeline = hybrid.then(|| {
            let pixel = shader("hylight.ps.dxil");
            let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
            formats[0] = SCENE_FORMAT;
            formats[1] = SCENE_FORMAT;
            let mut blend = D3D12_BLEND_DESC::default();
            blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            blend.RenderTarget[1].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(&root),
                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: screen_vs.as_ptr() as *const c_void, BytecodeLength: screen_vs.len() },
                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
                BlendState: blend,
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), StencilEnable: false.into(), ..Default::default() },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 2,
                RTVFormats: formats,
                DSVFormat: DXGI_FORMAT_UNKNOWN,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                ..Default::default()
            };
            device.CreateGraphicsPipelineState::<ID3D12PipelineState>(&desc).expect("the hybrid light pass pipeline state")
        });
        // (HY) the scene of Elden Ring's kind: its light passes (diffuse, specular, mask, the composed scene) and its fog
        let hy3_pipelines = hy3.then(|| {
            let pixel = shader("hylight2.ps.dxil");
            let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
            (formats[0], formats[1], formats[2], formats[3]) = (SCENE_FORMAT, SCENE_FORMAT, DXGI_FORMAT_R8G8B8A8_UNORM, SCENE_FORMAT);
            let mut blend = D3D12_BLEND_DESC { IndependentBlendEnable: false.into(), ..Default::default() };
            for t in blend.RenderTarget.iter_mut() {
                t.RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            }
            let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(&root),
                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: screen_vs.as_ptr() as *const c_void, BytecodeLength: screen_vs.len() },
                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
                BlendState: blend,
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), StencilEnable: false.into(), ..Default::default() },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 4,
                RTVFormats: formats,
                DSVFormat: DXGI_FORMAT_UNKNOWN,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                ..Default::default()
            };
            let light: ID3D12PipelineState = device.CreateGraphicsPipelineState(&desc).expect("the hy3 light pass pipeline state");
            let fog_ps = shader("hyfog.ps.dxil");
            let mut fog = desc.clone();
            fog.PS = D3D12_SHADER_BYTECODE { pShaderBytecode: fog_ps.as_ptr() as *const c_void, BytecodeLength: fog_ps.len() };
            fog.NumRenderTargets = 1;
            fog.RTVFormats = [DXGI_FORMAT_UNKNOWN; 8];
            fog.RTVFormats[0] = SCENE_FORMAT;
            fog.BlendState = D3D12_BLEND_DESC::default();
            fog.BlendState.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC { BlendEnable: true.into(), LogicOpEnable: false.into(), SrcBlend: D3D12_BLEND_SRC_ALPHA, DestBlend: D3D12_BLEND_INV_SRC_ALPHA, BlendOp: D3D12_BLEND_OP_ADD, SrcBlendAlpha: D3D12_BLEND_ZERO, DestBlendAlpha: D3D12_BLEND_ONE, BlendOpAlpha: D3D12_BLEND_OP_ADD, LogicOp: D3D12_LOGIC_OP_NOOP, RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8 };
            let fog: ID3D12PipelineState = device.CreateGraphicsPipelineState(&fog).expect("the hy3 fog pipeline state");
            (light, fog)
        });
        let velocity_target = texture(&device, DXGI_FORMAT_R16G16_FLOAT, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, SRV, None);
        // (IMPL_ER80) the glow source pass ([scene | glow], the scene left alone), the glow chain (added over the scene), the TAA
        let glow_states = glowtest.then(|| {
            let src = shader("glowsrc.ps.dxil");
            let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
            formats[0] = SCENE_FORMAT;
            formats[1] = SCENE_FORMAT;
            let mut blend = D3D12_BLEND_DESC { IndependentBlendEnable: true.into(), ..Default::default() };
            blend.RenderTarget[1].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(&root),
                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: screen_vs.as_ptr() as *const c_void, BytecodeLength: screen_vs.len() },
                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: src.as_ptr() as *const c_void, BytecodeLength: src.len() },
                BlendState: blend,
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), StencilEnable: false.into(), ..Default::default() },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 2,
                RTVFormats: formats,
                DSVFormat: DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                ..Default::default()
            };
            let source: ID3D12PipelineState = device.CreateGraphicsPipelineState(&desc).expect("the glow source pipeline state");
            let comp = shader("glowcomp.ps.dxil");
            let mut add = desc.clone();
            add.PS = D3D12_SHADER_BYTECODE { pShaderBytecode: comp.as_ptr() as *const c_void, BytecodeLength: comp.len() };
            add.NumRenderTargets = 1;
            add.RTVFormats = [DXGI_FORMAT_UNKNOWN; 8];
            add.RTVFormats[0] = SCENE_FORMAT;
            add.DSVFormat = DXGI_FORMAT_UNKNOWN;
            add.BlendState = D3D12_BLEND_DESC::default();
            add.BlendState.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC { BlendEnable: true.into(), LogicOpEnable: false.into(), SrcBlend: D3D12_BLEND_ONE, DestBlend: D3D12_BLEND_ONE, BlendOp: D3D12_BLEND_OP_ADD, SrcBlendAlpha: D3D12_BLEND_ZERO, DestBlendAlpha: D3D12_BLEND_ONE, BlendOpAlpha: D3D12_BLEND_OP_ADD, LogicOp: D3D12_LOGIC_OP_NOOP, RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8 };
            let chain: ID3D12PipelineState = device.CreateGraphicsPipelineState(&add).expect("the glow chain pipeline state");
            (source, chain)
        });
        let taa_state = taa.then(|| screen_pipeline(&device, &root, &screen_vs, &shader("taa.ps.dxil"), SCENE_FORMAT, None));
        // (IMPL_ER80b) the light-shaft source pass (harness root signature, t13 = the scene depth)
        let shaft_src_state = shafts_file.is_some().then(|| screen_pipeline(&device, &root, &screen_vs, &shader("shaftsrc.ps.dxil"), SCENE_FORMAT, None));
        let upadd_state = (shafts_file.is_some() && shaft_half).then(|| {
            let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
            formats[0] = SCENE_FORMAT;
            let ps = shader("upadd.ps.dxil");
            let mut blend = D3D12_BLEND_DESC::default();
            blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC { BlendEnable: true.into(), LogicOpEnable: false.into(), SrcBlend: D3D12_BLEND_ONE, DestBlend: D3D12_BLEND_ONE, BlendOp: D3D12_BLEND_OP_ADD, SrcBlendAlpha: D3D12_BLEND_ZERO, DestBlendAlpha: D3D12_BLEND_ONE, BlendOpAlpha: D3D12_BLEND_OP_ADD, LogicOp: D3D12_LOGIC_OP_NOOP, RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8 };
            let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(&root),
                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: screen_vs.as_ptr() as *const c_void, BytecodeLength: screen_vs.len() },
                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: ps.as_ptr() as *const c_void, BytecodeLength: ps.len() },
                BlendState: blend,
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), StencilEnable: false.into(), ..Default::default() },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 1,
                RTVFormats: formats,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                ..Default::default()
            };
            device.CreateGraphicsPipelineState::<ID3D12PipelineState>(&desc).expect("the up-add pipeline state")
        });
        let allocator5: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).unwrap();
        let list5: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator5, None).unwrap();
        list5.Close().unwrap();
        let velocity_rtv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, ..Default::default() }).unwrap();
        device.CreateRenderTargetView(&velocity_target, None, velocity_rtv.GetCPUDescriptorHandleForHeapStart());
        if erdtree || taa {
            kh_er_render::test_velocity_target(velocity_target.as_raw() as usize);
        }
        let allocator3: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).unwrap();
        let list3: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator3, None).unwrap();
        list3.Close().unwrap();
        let allocator4: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).unwrap();
        let list4: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator4, None).unwrap();
        list4.Close().unwrap();
        let _ = PCSTR::null();
        // The frame constants of the harness's passes (`ErFrame`), one 256-byte block per frame slot.
        let constants = {
            // ((ZFIN) 16 KB: the Elden Ring characters' blocks from 8 KB on)
            let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_BUFFER, Width: 16384, Height: 1, DepthOrArraySize: 1, MipLevels: 1, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR, ..Default::default() };
            let mut out: Option<ID3D12Resource> = None;
            device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_UPLOAD, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_GENERIC_READ, None, &mut out).unwrap();
            out.unwrap()
        };
        let mut constants_at: *mut c_void = std::ptr::null_mut();
        constants.Map(0, None, Some(&mut constants_at)).unwrap();

        // A read-back buffer for the screenshots.
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total = 0u64;
        let back_desc = swap.GetBuffer::<ID3D12Resource>(0).unwrap().GetDesc();
        device.GetCopyableFootprints(&back_desc, 0, 1, 0, Some(&mut footprint), None, None, Some(&mut total));
        let mut readback: Option<ID3D12Resource> = None;
        let buffer_desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_BUFFER, Width: total, Height: 1, DepthOrArraySize: 1, MipLevels: 1, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR, ..Default::default() };
        device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_READBACK, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &buffer_desc, D3D12_RESOURCE_STATE_COPY_DEST, None, &mut readback).unwrap();
        let readback = readback.unwrap();
        let back_rtv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, ..Default::default() }).unwrap();

        // ---- the renderer, in the scene arrangement, with the stand-in rig (there is no game light here)
        kh_er_render::install(&pack, |text| println!("{text}"));
        kh_er_render::set_options(Options { at: kh_er_render::At::Scene, light: false, jitter: false, velocity: 0, shadow_sun: 0.0, key: sora_dark, ambient: sora_dark, debug: value("--char-debug").and_then(|v| v.parse().ok()).unwrap_or(0.0), ..Options::default() });
        // (IMPL_ER44) `--sora-ambient X`: the ambient share that remains with his own lights (hero and effects)
        let sora_ambient: Option<f32> = value("--sora-ambient").and_then(|v| v.parse().ok());
        // (IMPL_ER44 part 2) `--plain-hero`: hero.keyblade = 0 (the light above him, no Keyblade drawn for it);
        // `--hero-shadow X`, `--blade CM`, `--sparkle S`: the rows hero.shadow, hero.blade_cm, hero.sparkle_s
        let mut defaults = kh_er_render::pointlight::LightSettings::default();
        defaults.hero_keyblade = !flag("--plain-hero");
        if let Some(v) = value("--sparkle-fx").and_then(|v| v.parse().ok()) {
            kh_er_render::pointlight::choose_sparkle(v);
        }
        if let Some(v) = value("--hero-shadow").and_then(|v| v.parse().ok()) {
            defaults.hero_shadow = v;
        }
        if let Some(v) = value("--blade").and_then(|v| v.parse().ok()) {
            defaults.hero_blade = v;
        }
        if let Some(v) = value("--sparkle").and_then(|v| v.parse().ok()) {
            defaults.hero_sparkle = v;
        }
        // (IMPL_ER53) `--no-glow`: IMPL_ER44's whole appear effect at the interval; `--glow-hold S`; `--glow-keep a,b` /
        // `--spark-keep a,b`: other emitters for the two copies (`-` = none)
        defaults.hero_glow = !flag("--no-glow");
        // (IMPL_ER56) `--fx-exp N`: every effect light with exponent N (2 = as before IMPL_ER56)
        if let Some(v) = value("--fx-exp").and_then(|v| v.parse().ok()) {
            kh_er_render::pointlight::force_fx_exponent(v);
        }
        if let Some(v) = value("--glow-hold").and_then(|v| v.parse().ok()) {
            defaults.hero_glow_hold = v;
        }
        if value("--glow-keep").is_some() || value("--spark-keep").is_some() {
            let list = |name: &str, fallback: &[&str]| -> Vec<String> { value(name).map(|v| v.split(',').filter(|x| !x.is_empty() && *x != "-").map(str::to_string).collect()).unwrap_or_else(|| fallback.iter().map(|x| x.to_string()).collect()) };
            kh_er_render::pointlight::choose_glow_emitters(list("--glow-keep", &kh_er_render::pointlight::GLOW_EMITTERS), list("--spark-keep", &kh_er_render::pointlight::SPARK_EMITTERS));
        }
        // (IMPL_ER64) `--from-blade`: the Keyblade light on him from the blade (as before); `--no-world`: no world light on him;
        // `--torch X,Y,Z,R,G,B` (cm, target units): a light of "the game's" lighting the floor (the pass, world only), and the
        // ring of taps the game's light accumulation would give around his feet (fitted into his world light)
        defaults.hero_sora_placed = !flag("--from-blade");
        defaults.world = !flag("--no-world");
        let torch: Option<(Vec3, Vec3)> = value("--torch").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect::<Vec<f32>>()).filter(|v| v.len() == 6).map(|v| (Vec3::new(v[0], v[1], v[2]), Vec3::new(v[3], v[4], v[5])));
        if let Some((at, colour)) = torch {
            kh_er_render::pointlight::set_test_pass_lights(vec![kh_er_render::pointlight::PassLight { position: at, radius: 900.0, color: colour, exponent: 2.0, wrap: 0.0, on_sora: 0.0, sora: 0.0, sora_ambient: 0.0, placed: false, body_shadow: 0.0 }]);
        }
        // (IMPL_ER53) the hero light's numbers for a look: `--hero-int X`, `--hero-radius CM`, `--hero-color R,G,B`
        if let Some(v) = value("--hero-int").and_then(|v| v.parse().ok()) {
            defaults.hero_intensity = v;
        }
        if let Some(v) = value("--hero-radius").and_then(|v| v.parse().ok()) {
            defaults.hero_radius = v;
        }
        if let Some(c) = value("--hero-color").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect::<Vec<f32>>()).filter(|c| c.len() == 3) {
            defaults.hero_color = Vec3::new(c[0], c[1], c[2]);
        }
        kh_er_render::pointlight::set_settings(kh_er_render::pointlight::LightSettings { fx: !flag("--no-light"), hero: if flag("--no-light") { 0 } else { hero_mode }, hero_sora_ambient: sora_ambient.unwrap_or(defaults.hero_sora_ambient), fx_sora_ambient: sora_ambient.unwrap_or(defaults.fx_sora_ambient), ..defaults });
        kh_er_render::vfx::set_options(VfxOptions { on: !flag("--no-vfx"), distort: value("--distort").as_deref() != Some("0"), hdr: value("--hdr").and_then(|v| v.parse().ok()).unwrap_or(1.0), solid: flag("--solid"), decals: value("--decals").as_deref() != Some("0"), late: false });
        // (VFXR) `--bloom X` (KH3's bloom on the effects, default 1; 0 = the look before VFXR), `--screen-fx 0` (no screen passes)
        kh_er_render::vfx::set_post(value("--bloom").and_then(|v| v.parse().ok()).unwrap_or(1.0), value("--screen-fx").as_deref() != Some("0"));
        // (VFXR) `--eye 0`: the eye-adaptation materials by the lit-surface rule (the look before VFXR)
        kh_er_render::vfx::set_eye_adapt(value("--eye").as_deref() == Some("1")); // (VFXR eye evidence) default off
        // (VFXR occlusion) `--decal-chars 0`: the decals paint the characters too (as before)
        kh_er_render::vfx::set_decal_skip_chars(value("--decal-chars").as_deref() != Some("0"));
        // (VFXR) `--eye-decals 1`: the decals take the eye boost too; `--look E,K,T`: a cinematic look (exposure EV, key EV,
        // bloom threshold) from the start (kh_core::host::cinematic)
        kh_er_render::vfx::EYE_DECALS.store(value("--eye-decals").as_deref() == Some("1"), std::sync::atomic::Ordering::Relaxed);
        kh_er_render::vfx::SORT_GROUPS.store(value("--sort").as_deref() != Some("0"), std::sync::atomic::Ordering::Relaxed); // (VFXR sort) `--sort 0`: as before
        kh_er_render::vfx::DECAL_BLOOM.store(value("--decal-bloom").as_deref() == Some("1"), std::sync::atomic::Ordering::Relaxed); // (VFXR decal bloom)
        if let Some(v) = value("--look").map(|s| s.split(',').filter_map(|x| x.trim().parse::<f32>().ok()).collect::<Vec<_>>()).filter(|v| v.len() == 3) {
            kh_er_render::vfx::set_cine_look(v[0], v[1], v[2]);
        }
        let (materials, meshes) = effects.assets();
        let materials: Vec<(String, Geo, kh_vfx::Blend)> = materials
            .into_iter()
            .map(|(name, geo, blend)| {
                let geo = match geo {
                    AssetGeo::Quad => Geo::Quad,
                    AssetGeo::Ribbon => Geo::Ribbon,
                    AssetGeo::Mesh => Geo::Mesh,
                    AssetGeo::Decal => Geo::Decal,
                };
                (name, geo, blend)
            })
            .collect();
        if !flag("--no-preload") {
            kh_er_render::vfx::preload(&materials, &meshes);
            println!("effects: {} materials and {} meshes named for preloading", materials.len(), meshes.len());
        }

        let wait = |fence_value: &mut u64| {
            *fence_value += 1;
            queue.Signal(&fence, *fence_value).unwrap();
            while fence.GetCompletedValue() < *fence_value {
                std::thread::sleep(std::time::Duration::from_micros(100));
            }
        };
        let mut audio = AudioQueue::default();
        let mut seam = EffectQueue::default();
        let mut tap = PoseTap::default();
        let mut weapon_visible = false;
        let mut weapon_since = 10.0f32;
        // (IMPL_ER44) the Keyblade light, as kh-er's host does it: held out while the gate is closed, sparkling
        let mut sparkle = kh_er_render::pointlight::KeybladeGlow::default();
        let mut glow_handle: Option<kh_core::EffectHandle> = None;
        let hand = rig.animator.skeleton().index_of("R_buki");
        let socket = Trs { translation: Vec3::ZERO, rotation: Quat::from_xyzw(std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2), scale: Vec3::ONE };
        // (ER79) `--chain`: the Keyblade's keychain simulated (as kh-er sora.rs step_chain); with `--outfit` it hangs from the
        // outfit's hand, where the Keyblade is drawn; `--chain-his`: from his hand (before ER79)
        let mut keychain = if flag("--chain") { kh_anim::Keychain::load(&pack.join("weapon")).ok() } else { None };
        let chain_rig = value("--outfit").and_then(|id| kh_er_render::outfit::retarget_for(&pack, &id, rig.animator.skeleton()).ok().flatten());
        let mut chain_shown = false;
        // (ER79) `--stretch`: the outfit's skin on the CPU each step (as gfx.rs: joint x inverse bind, from the outfit's frame
        // pose), every edge against its bind length; the worst stretches and their joints are printed
        let stretch_rig = value("--outfit").filter(|_| flag("--stretch")).and_then(|id| {
            let r = kh_er_render::outfit::retarget_for(&pack, &id, rig.animator.skeleton()).ok().flatten()?;
            let m = kh_er_render::model::Model::load(&kh_er_render::outfit::model_file(&pack, &id)).ok()?;
            Some(r.with_guard_joints(&m.joints))
        });
        let mut stretch = stretch_rig.as_ref().and_then(|r| Stretch::new(&kh_er_render::outfit::model_file(&pack, &value("--outfit").unwrap_or_default()), r));
        let mut step = 0usize;
        let mut warm = 0u32;
        let mut presented = 0u32;
        let mut sim_micros = 0u128;
        let mut vfx_micros = 0u128;
        let mut vfx_peak = 0u128;
        let run_started = std::time::Instant::now();
        let mut warm_done_at = None;
        loop {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            // ---- warm-up: until the effect materials are made (or the limit)
            let warming = run_started.elapsed().as_secs_f32() < warm_limit && !flag("--no-preload") && kh_er_render::vfx::pending().is_none_or(|n| n > 0) && warm_done_at.is_none();
            if !warming && warm_done_at.is_none() {
                warm_done_at = Some(run_started.elapsed());
                println!("effects: warm-up over after {warm} frames, {:.1} s: {} still waiting; {}", run_started.elapsed().as_secs_f32(), kh_er_render::vfx::pending().map_or("-".to_string(), |n| n.to_string()), kh_er_render::vfx::status());
            }
            if warming {
                warm += 1;
            } else if step >= frames {
                break;
            }
            // ---- the host's frame: one simulation step (not while warming up), the pose, the effects
            if !warming {
                let before = std::time::Instant::now();
                // (the view of the camera below, for the shotlock's reticle: behind him, looking along his facing)
                rig.view = Some(kh_core::magic::View::from_camera(Vec3::new(-cam_back, 0.0, cam_up), orbit, -((cam_up - cam_look) / cam_back).atan().to_degrees(), 2.0 * ((0.8378f32 * 0.5).tan() * width() as f32 / height() as f32).atan().to_degrees(), width() as i32, height() as i32));
                {
                    let wanted = kh_er_render::pointlight::keyblade_wanted();
                    rig.sora.hold_weapon(wanted);
                    let drawn = weapon_visible && weapon_since >= 0.28;
                    // (IMPL_ER53) as kh-er sora.rs: the steady glow + the sparkles
                    let s = kh_er_render::pointlight::settings();
                    let glow = sparkle.step(STEP, wanted && drawn, s.hero_glow, s.hero_sparkle);
                    let at_blade = |name: &'static str, socket: &'static str, persistent: bool| kh_core::EffectSpawn { at: kh_core::Attach::Sora { bone: Some(socket), offset: Vec3::ZERO }, persistent, ..kh_core::EffectSpawn::at_sora(kh_core::EffectId(kh_core::notify::notify_asset_id(name)), name) };
                    if glow.start_glow {
                        rig.sora.debug_emit(kh_core::DebugEmit::Effect(at_blade(kh_er_render::pointlight::GLOW_EFFECT, kh_er_render::pointlight::GLOW_SOCKET, true)));
                    }
                    if glow.stop_glow {
                        if let Some(handle) = glow_handle.take() {
                            rig.sora.debug_emit(kh_core::DebugEmit::StopEffect(handle, false));
                        }
                    }
                    if glow.sparkle {
                        let (sparkle_fx, sparkle_socket) = if s.hero_glow { (kh_er_render::pointlight::SPARK_EFFECT, kh_er_render::pointlight::GLOW_SOCKET) } else { kh_er_render::pointlight::sparkle_effect() };
                        rig.sora.debug_emit(kh_core::DebugEmit::Effect(at_blade(sparkle_fx, sparkle_socket, false)));
                    }
                }
                // (ER90) the XS presets
                if step == 0 {
                    use kh_core::abilities::xs::finishers as f;
                    if let Some(k) = value("--xs-finisher").and_then(|n| match n.as_str() { "spiral" => Some(f::SPIRAL_RUSH), "zantetsuken" => Some(f::ZANTETSUKEN), "horizon" => Some(f::EVENT_HORIZON), "stamp" => Some(f::GIANT_STAMP), "blast" => Some(f::CRIMSON_BLAST), _ => None }) {
                        rig.sora.debug_xs_finisher(k);
                    }
                    if let Some(p) = value("--xs-style").and_then(|v| v.parse::<f32>().ok()) {
                        rig.sora.debug_add_style(p);
                    }
                    if let Some(k) = value("--xs-shotlock").and_then(|v| v.parse::<u8>().ok()) {
                        rig.sora.command_mut().shotlock = k;
                    }
                    if flag("--xs-mp-charge") {
                        rig.sora.status_mut().set_mp_charge(true, 0.0);
                    }
                    // `--xs-hp F`: HP at that share of the maximum (Riskcharge's offer)
                    if let Some(f) = value("--xs-hp").and_then(|v| v.parse::<f32>().ok()) {
                        let max = rig.sora.status().max_hp();
                        rig.sora.status_mut().set_hp(((max as f32 * f).round() as i32).max(1));
                    }
                }
                if flag("--xs-mp") {
                    rig.sora.status_mut().set_mp(999);
                }
                if flag("--xs-lock") {
                    let lock = &mut rig.sora.combat_mut().lockon;
                    (lock.locked, lock.current) = (true, kh_core::TargetId(1));
                }
                rig.step(&script, step, 0.0, &mut audio, &mut seam);
                // (ER90) `--list-kinds`: his attack kinds as they begin, and his state changes
                if flag("--list-kinds") {
                    let h = &rig.sora.combat().history;
                    if h.len() != kinds_seen {
                        println!("    step {step}: attack kinds {:02X?}", h);
                        kinds_seen = h.len();
                    }
                    if Some(rig.out.state) != state_seen {
                        println!("    step {step}: state {:?}", rig.out.state);
                        state_seen = Some(rig.out.state);
                    }
                }
                // (ER party S5) the party's step after Sora's, on the same seams (kh-er's order)
                if let Some(p) = party_rig.as_mut() {
                    let out = rig.out.clone();
                    let mut none = kh_core::host::NullTargets::default();
                    let targets: &mut dyn kh_core::Targets = match rig.dummy.as_mut() {
                        Some(d) => d,
                        None => &mut none,
                    };
                    p.step(step, STEP, &out, &rig.world, targets, &mut audio, &mut seam);
                }
                kh_er_render::handik::set_kh3_share(1.0 - rig.animator.own_share());
                // (ER83) `--clip NAME[@SECONDS]`: his pose is that clip of the pack (looped by the step clock, or held at
                // SECONDS) whatever the script does; `--weapon-on`: the Keyblade drawn
                if let Some(spec) = value("--clip") {
                    let (name, at) = spec.split_once('@').map_or((spec.as_str(), None), |(n, t)| (n, t.parse::<f32>().ok()));
                    if let Some(len) = rig.animator.clip_info(name).map(|i| i.length) {
                        let t = at.unwrap_or_else(|| (step as f32 * STEP) % len.max(1.0e-3));
                        let local = rig.animator.sample(name, t);
                        rig.pose.component = kh_anim::pose::component_space(rig.animator.skeleton(), &local);
                        rig.pose.local = local;
                        kh_er_render::handik::set_kh3_share(1.0);
                    }
                }
                if flag("--weapon-on") {
                    weapon_visible = true;
                }
                // (ER party S5) the step's sounds (Sora's and the party's) for the party harness's audio side
                if let Some(a) = party_audio.as_mut() {
                    a.take(&mut audio);
                }
                audio.0.clear();
                step += 1;
                sim_micros += before.elapsed().as_micros();
                if flag("--list") && step % 30 == 0 {
                    let lock = rig.sora.combat().lockon;
                    println!("    step {step}: state {:?} position ({:.0}, {:.0}, {:.0}) yaw {:.0}; lock current {} locked {}; dummy hits {}", rig.out.state, rig.out.position.x, rig.out.position.y, rig.out.position.z, rig.out.yaw, lock.current.0, lock.locked, rig.dummy.map_or(0, |d| d.hits));
                }
            }
            let out = rig.out.clone();
            let to_world = component_to_world(&out);
            // The camera: behind him (he faces game +X at the start), looking over him at the dummy.
            // (ER87) `--orbit-step`: turned further each frame of the script
            let (sin, cos) = (orbit + if warming { 0.0 } else { orbit_step * step as f32 }).to_radians().sin_cos();
            let back_dir = Vec3::new(-cos, -sin, 0.0);
            // (ER73) `--follow`: the camera keeps him in view where he runs
            let at = if flag("--follow") { Vec3::new(out.position.x, out.position.y, 0.0) } else { Vec3::ZERO };
            let focus = at + Vec3::new(0.0, 0.0, cam_look);
            let eye_game = at + back_dir * cam_back + Vec3::Z * cam_up;
            // (CV1) the conversation camera's framing instead
            let conv_heads = conv_shot.map(|_| {
                let turn = glam::Quat::from_rotation_z(rig.out.yaw.to_radians());
                let donald = turn * Vec3::new(conv_friends[0], conv_friends[1], 0.0);
                (Vec3::new(out.position.x, out.position.y, 0.0) + Vec3::Z * 140.0, donald + Vec3::Z * 76.0)
            });
            let (eye_game, focus) = match (conv_shot, conv_heads) {
                (Some((kind, on_donald)), Some((sora_head, donald_head))) => {
                    use kh_host::conv::camera::{frame, Subject};
                    let sora_s = Subject { id: 0, feet: sora_head - Vec3::Z * 140.0, head: sora_head, yaw: out.yaw };
                    let donald_s = Subject { id: 1, feet: donald_head - Vec3::Z * 76.0, head: donald_head, yaw: out.yaw + 180.0 };
                    let (s, o) = if on_donald { (donald_s, sora_s) } else { (sora_s, donald_s) };
                    frame(kind, &s, Some(&o), conv_side, conv_push)
                }
                _ => (eye_game, focus),
            };
            // (NF) `--camfade-path AX,AY,AZ,BX,BY,BZ,LX,LY,LZ`: the camera moves from A to B over the script's frames, looking at
            // L (game space, cm, from where the camera stands back from: `at`); the camera fade's look point is L
            let nf_path: Option<Vec<f32>> = value("--camfade-path").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect::<Vec<f32>>()).filter(|v| v.len() == 9);
            let (eye_game, focus) = match &nf_path {
                Some(p) => {
                    let u = if warming { 0.0 } else { (step as f32 / (frames.max(2) - 1) as f32).clamp(0.0, 1.0) };
                    (at + Vec3::new(p[0], p[1], p[2]).lerp(Vec3::new(p[3], p[4], p[5]), u), at + Vec3::new(p[6], p[7], p[8]))
                }
                None => (eye_game, focus),
            };
            kh_er_render::camfade::set_look(Some(focus));
            kh_er_render::camfade::set_lead(value("--camfade-lead").and_then(|v| v.parse::<u8>().ok()).filter(|v| *v > 0));
            // GAME (cm, X forward, Y right, Z up) -> HOST (m, X right, Y up, Z forward)
            let host = |g: Vec3| Vec3::new(g.y, g.z, g.x) * 0.01;
            let eye = host(eye_game);
            let forward = (host(focus) - eye).normalize();
            let right = Vec3::Y.cross(forward).normalize();
            let up = forward.cross(right);
            // (IMPL_ER80) `--jitter`: the whole view turned by a sub-pixel Halton(2,3) offset each frame (the game and Sora alike)
            let (forward, right, up) = if flag("--jitter") {
                let halton = |mut i: u32, b: u32| {
                    let (mut f, mut r) = (1.0f32, 0.0f32);
                    while i > 0 {
                        f /= b as f32;
                        r += f * (i % b) as f32;
                        i /= b;
                    }
                    r
                };
                let n = (presented % 8) + 1;
                let (jx, jy) = (halton(n, 2) - 0.5, halton(n, 3) - 0.5);
                let tan_y = (fov_v * 0.5).tan();
                let tan_x = tan_y * width() as f32 / height() as f32;
                let f = (forward + right * (2.0 * jx / width() as f32 * tan_x) + up * (2.0 * jy / height() as f32 * tan_y)).normalize();
                let r = Vec3::Y.cross(f).normalize();
                (f, r, f.cross(r))
            } else {
                (forward, right, up)
            };
            let camera = Camera { right, up, forward, position: eye, fov: fov_v, aspect: width() as f32 / height() as f32, near: 0.05, far: 10000.0 };
            // (ZFIN round 8) `--game-jitter`: the "game's" view turned by this frame's sub-pixel offset (its floor and characters
            // land where a jittered projection puts them), ours not; the renderer is told the jitter (pixels, x right, y up:
            // the view turned right moves the picture left; ER's TempAA does not un-jitter its input: TempAA.vpo)
            let game_camera = if flag("--game-jitter") {
                let n = (presented % 8) + 1;
                let (jx, jy) = (halton(n, 2) - 0.5, halton(n, 3) - 0.5);
                let tan_y = (fov_v * 0.5).tan();
                let tan_x = tan_y * width() as f32 / height() as f32;
                let f = (forward + right * (2.0 * jx / width() as f32 * tan_x) + up * (2.0 * jy / height() as f32 * tan_y)).normalize();
                let r = Vec3::Y.cross(f).normalize();
                Camera { right: r, up: f.cross(r), forward: f, ..camera }
            } else {
                camera
            };
            // (CV1) the faces on the 1920 x 1080 stage (for the bubble over this frame)
            if let (Some((sora_head, donald_head)), true) = (conv_heads, shots.contains(&(step as u32))) {
                let stage = |g: Vec3| -> (f32, f32, f32) {
                    let d = host(g) - eye;
                    let z = d.dot(forward);
                    let t = (fov_v * 0.5).tan();
                    let aspect = width() as f32 / height() as f32;
                    let (nx, ny) = (d.dot(right) / (z * t * aspect), d.dot(up) / (z * t));
                    (960.0 + nx * 540.0 * aspect, 540.0 - ny * 540.0, 0.16 / (z * t) * 540.0)
                };
                let (s, d) = (stage(sora_head), stage(donald_head));
                println!("conv faces: frame {step}: sora {:.0},{:.0},{:.0} donald {:.0},{:.0},{:.0}", s.0, s.1, s.2, d.0, d.1, d.2);
            }
            if !warming {
                let before = std::time::Instant::now();
                // (ER72) with `--outfit` of other proportions: the effects hang on the outfit's bones
                // (ER73) every outfit: the Keyblade's effects and trail on the worn outfit's hand (kh-er boc.rs effect_pose)
                static OUTFIT_RIG: std::sync::OnceLock<Option<kh_er_render::outfit::Retarget>> = std::sync::OnceLock::new();
                let outfit_rig = OUTFIT_RIG.get_or_init(|| value("--outfit").and_then(|id| kh_er_render::outfit::retarget_for(&pack, &id, rig.animator.skeleton()).ok().flatten()));
                let outfit_pose = outfit_rig.as_ref().map(|r| kh_anim::Pose { local: rig.pose.local.clone(), component: r.effect_component(rig.animator.skeleton(), &rig.pose.local, &rig.pose.component) });
                tap.record(step as u64, &out, rig.animator.skeleton(), outfit_pose.as_ref().unwrap_or(&rig.pose), &to_world);
                tap.samples.clear();
                let commands = std::mem::take(&mut seam.0);
                let dummy = rig.dummy;
                let target = |_: kh_core::TargetId| dummy.map(|d: Dummy| d.info.center);
                if let Some(sp) = savepoint.as_mut() {
                    // (SP2) `--savepoint-entry`: the save point acts on him (kh-host `SavePoint::step`, the entry burst on, as the
                    // Elden Ring mod): walking in plays KH3's burst vg_com_svp_in00 (the column, the rings, the sparkles)
                    if flag("--savepoint-entry") {
                        sp.entry.on = true;
                        sp.first_in_reactor = true;
                        let p = *rig.sora.params();
                        let s = sp.step(STEP, &mut rig.sora, (out.capsule_center, p.capsule_radius, p.capsule_half_height));
                        if s.heal_started {
                            println!("effects: (SP2) step {step}: he STEPS INTO the save point: KH3's burst {} starts (entry {})", sp.data.heal, sp.entry.entries);
                        }
                    }
                    sp.show(&mut effects); // (IMPL_ER52)
                }
                // (ORBS) the prize orbs: spawned once (endless idle effects), with kh-er's gain (prize.rs `configure`)
                if let Some((row, glow, ids)) = orbs.as_mut() {
                    if ids.is_empty() {
                        for (name, at, scale) in row.iter() {
                            match effects.free_spawn(name, *at, 0.0, *scale) {
                                Some(id) => {
                                    effects.free_gain(id, kh_er_render::vfx::prize_gain(*glow));
                                    ids.push(id);
                                }
                                None => println!("effects: (ORBS) no effect {name} in the pack"),
                            }
                        }
                        println!("effects: (ORBS) {} orb effect(s) spawned at step {step}, gain {}", ids.len(), kh_er_render::vfx::prize_gain(*glow));
                    }
                }
                for (name, base, keep, hold) in kh_er_render::pointlight::glow_variants() {
                    let keep: Vec<&str> = keep.iter().map(String::as_str).collect();
                    effects.define_variant(name, base, &keep, hold);
                }
                // (ER party S5) the members' poses and the spell actors for the effect side (kh-er fx.rs effect_step)
                if let Some(p) = party_rig.as_ref() {
                    p.feed(&mut effects);
                }
                // (ZFIN round 8) `--fx-steps N,N,..`: the effect steps per presented frame, in turn (Elden Ring's frames below 60 fps
                // carry 0, 1 or 2 of Sora's fixed steps: kh-er fx.rs steps the effects once per step)
                let fx_steps: usize = FX_STEPS.get().map_or(1, |v: &Vec<usize>| if v.is_empty() { 1 } else { v[step % v.len()] });
                for k in 0..fx_steps {
                    effects.step(STEP, if k == 0 { &commands } else { &[] }, &Scene { out: &out, sora: &rig.sora, tap: &tap, target: &target, ground: Some(0.0), eye: eye_game });
                }
                // (ER party S5) the audio side of the step (kh-er fx.rs after_step), then the party's loops: a line a second, and
                // when an action ends
                if let (Some(a), Some(p)) = (party_audio.as_mut(), party_rig.as_mut()) {
                    let listener = kh_audio::Listener { position: eye_game, forward: (focus - eye_game).normalize_or(Vec3::X), right: math::game_dir_of_host(right) };
                    a.step(&out, rig.animator.skeleton(), &rig.pose, &to_world, &commands, listener, &|at| rig.place(at), p);
                }
                if let Some(p) = party_rig.as_mut() {
                    let ended = std::mem::take(&mut p.said);
                    for line in &ended {
                        println!("{line}");
                    }
                    if !ended.is_empty() || step % 60 == 0 {
                        println!("party loops: step {step}: {}", p.loops(&effects));
                    }
                }
                for event in &out.events {
                    if let kh_core::EventKind::EffectSpawn { handle, spawn } = &event.kind {
                        if spawn.name == kh_er_render::pointlight::GLOW_EFFECT {
                            glow_handle = Some(*handle);
                        }
                    }
                    if let kh_core::EventKind::WeaponVisible { visible, .. } = event.kind {
                        if visible != weapon_visible {
                            (weapon_visible, weapon_since) = (visible, 0.0);
                        }
                    }
                }
                weapon_since += STEP;
                let view = kh_vfx::Camera { position: eye_game, forward: math::game_dir_of_host(forward), right: math::game_dir_of_host(right), up: math::game_dir_of_host(up) };
                let (list, material_names, mesh_names) = effects.draw(1.0, &view);
                kh_er_render::vfx::submit(list, material_names, mesh_names, &view, step as f32 * STEP);
                // (VFXR) the character drives, as kh-er fx.rs hands them over (`--char-fx 0`: none)
                kh_er_render::vfx::CHAR_FX.store(value("--char-fx").as_deref() != Some("0"), std::sync::atomic::Ordering::Relaxed);
                kh_er_render::vfx::set_char_drives(effects.char_drives().into_iter().map(|d| kh_er_render::vfx::CharDrive { owner: d.owner, name: d.name, value: d.value, vector: d.vector }).collect());
                let micros = before.elapsed().as_micros();
                vfx_micros += micros;
                vfx_peak = vfx_peak.max(micros);
                if flag("--list-fx") && step % 6 == 0 {
                    let names: Vec<String> = effects.world.live().iter().map(|l| format!("{} {:.2}s", l.name, l.seconds)).collect();
                    println!("    step {step}: {names:?}");
                }
                for line in effects.log.drain(..) {
                    println!("{line}");
                }
            }
            if let Some((at, colour)) = torch {
                // the ring "the game" would show: the torch on a flat floor (+ a little light everywhere), around his feet
                let feet = rig.out.position;
                let mut ring = Vec::new();
                let mut ring_at = Vec::new();
                for k in 0..24 {
                    let a = (k % 12) as f32 * 30f32.to_radians();
                    let r = if k >= 12 { 250.0 } else { 120.0 };
                    let p = Vec3::new(feet.x + a.cos() * r, feet.y + a.sin() * r, 0.0);
                    let to = at - p;
                    let lit = colour * kh_er_render::pointlight::radial(to.length_squared(), 900.0, 2.0) * (to.z / to.length().max(1.0)).max(0.0);
                    ring.push((lit + Vec3::splat(0.002)).extend(1.0));
                    ring_at.push((p - feet).extend(1.0));
                }
                kh_er_render::pointlight::set_test_ring(Some((ring, ring_at)));
            }
            let opacity = ((weapon_since - 0.22) / 0.06).clamp(0.0, 1.0);
            let opacity = if weapon_visible { opacity } else { 1.0 - opacity };
            let body = (!rig.pose.component.is_empty()).then(|| Body { component_to_world: to_world, local: rig.pose.local.clone(), component: rig.pose.component.clone() });
            // (ER70) `--poses F1,F2,..`: Sora's local pose read from files (one line a
            // bone: translation, rotation xyzw, scale), pose k from frame k x `--pose-frames` (default 10)
            let body = match poses.get((step as usize / pose_frames.max(1)).min(poses.len().saturating_sub(1))) {
                Some(local) if local.len() == rig.pose.local.len() => {
                    let skeleton = rig.animator.skeleton();
                    let mut component: Vec<Trs> = Vec::with_capacity(local.len());
                    for (i, l) in local.iter().enumerate() {
                        let c = match skeleton.bones[i].parent.and_then(|p| component.get(p)) {
                            Some(parent) => l.then(parent),
                            None => *l,
                        };
                        component.push(c);
                    }
                    Some(Body { component_to_world: to_world, local: local.clone(), component })
                }
                _ => body,
            };
            // (ER91) `--freeze STEP`: from that step on his pose is held (the hair's shimmer measured without his breathing)
            static FROZEN: std::sync::Mutex<Option<Body>> = std::sync::Mutex::new(None);
            let body = match value("--freeze").and_then(|v| v.parse::<u64>().ok()) {
                Some(at) if step as u64 >= at => {
                    let mut held = FROZEN.lock().unwrap();
                    if held.is_none() {
                        *held = body.clone();
                    }
                    held.clone()
                }
                _ => body,
            };
            let chain_pose: Vec<Trs> = match (keychain.as_mut(), hand) {
                (Some(chain), Some(h)) if opacity > 0.0 => {
                    if !chain_shown {
                        chain.reset();
                    }
                    chain_shown = true;
                    let held = match (&chain_rig, flag("--chain-his")) {
                        (Some(r), false) => r.effect_component(rig.animator.skeleton(), &rig.pose.local, &rig.pose.component).get(h).copied(),
                        _ => rig.pose.component.get(h).copied(),
                    };
                    let out = held.map(|hp| chain.step(STEP, &socket.then(&hp).then(&to_world), rig.animator.bonamik()).to_vec()).unwrap_or_default();
                    // (ER79) `--chain-angle`: how far the keychain (its first to last driven bone) is from hanging straight down
                    // where the Keyblade is DRAWN (the outfit's hand), degrees
                    if flag("--chain-angle") {
                        let drawn = chain_rig.as_ref().and_then(|r| r.effect_component(rig.animator.skeleton(), &rig.pose.local, &rig.pose.component).get(h).copied()).or_else(|| rig.pose.component.get(h).copied());
                        let driven = chain.driven_bones();
                        if let (Some(d), Some(&first), Some(&last)) = (drawn, driven.first(), driven.last()) {
                            let w = socket.then(&d).then(&to_world);
                            let dir = w.rotation * (out[last].translation - out[first].translation);
                            let angle = dir.normalize_or_zero().dot(-Vec3::Z).clamp(-1.0, 1.0).acos().to_degrees();
                            println!("chain: step {step} ({:?}): {angle:.1} deg off straight down", rig.out.state);
                        }
                    }
                    out
                }
                _ => {
                    chain_shown = false;
                    Vec::new()
                }
            };
            let weapon = hand.and_then(|hand| rig.pose.component.get(hand)).filter(|_| opacity > 0.0).map(|hand| Weapon { to_world: socket.then(hand).then(&to_world), component: chain_pose, opacity });
            if let (Some(st), Some(r)) = (stretch.as_mut(), stretch_rig.as_ref()) {
                let posed = r.frame(step as f32 * STEP, &to_world, &rig.pose.local, &rig.pose.component);
                st.step(step, &posed.1, &format!("{:?}", rig.out.state), r);
                // the joints the outfit's simulation / anti-clip moved far from the retargeted pose (its KineDriver included)
                let pre = r.pose(&rig.pose.local, &rig.pose.component);
                let far: Vec<String> = st.names.iter().zip(&st.joint_bone).filter_map(|(n, b)| {
                    let b = (*b)?;
                    let d = (posed.1.get(b)?.translation - pre.get(b)?.translation).length();
                    (d > 25.0).then(|| format!("{n} {d:.0} (sim {:.0?} retargeted {:.0?}, world origin in component {:.0?})", posed.1.get(b).unwrap().translation.to_array(), pre.get(b).unwrap().translation.to_array(), to_world.translation.to_array()))
                }).collect();
                if !far.is_empty() {
                    println!("stretch: step {step}: off the retargeted pose by > 25 cm: {}", far.join(", "));
                }
                // (ER79) `--track`: the outfit's hands and feet each step (component space, cm), for the blends' smoothness
                if flag("--track") {
                    let at = |n: &str| r.index_of(n).and_then(|k| posed.1.get(k)).map(|t| t.translation).unwrap_or_default();
                    let v: Vec<String> = ["R_te", "L_te", "R_ashi1", "L_ashi1", "atama"].iter().map(|n| { let p = at(n); format!("{:.4} {:.4} {:.4}", p.x, p.y, p.z) }).collect();
                    println!("track: {step} {:?} {} | {:?}", rig.out.state, v.join(" "), rig.animator.face_playing()[0].map(|f| f.0));
                }
                // (ER79) `--lids`: the eyelid gaps (upper lid a to lower lid a, both eyes, cm): his and the outfit's, each step
                if flag("--lids") {
                    let gap = |pose: &[Trs], idx: &dyn Fn(&str) -> Option<usize>| -> Option<f32> {
                        let r = (pose.get(idx("RU_eyelid_a")?)?.translation - pose.get(idx("RD_eyelid_a")?)?.translation).length();
                        let l = (pose.get(idx("LU_eyelid_a")?)?.translation - pose.get(idx("LD_eyelid_a")?)?.translation).length();
                        Some((r + l) * 0.5)
                    };
                    let sk = rig.animator.skeleton();
                    let his = gap(&rig.pose.component, &|n| sk.index_of(n));
                    let its = gap(&posed.1, &|n| r.index_of(n));
                    println!("lids: step {step} ({:?}): his {:.2} outfit {:.2} | face {:?}", rig.out.state, his.unwrap_or(-1.0), its.unwrap_or(-1.0), rig.animator.face_playing());
                }
                // `--dump-sim FILE`: per step the component's world place and the outfit's pose before its simulation (f32: 10 a
                // transform), for replaying its Bonamik offline
                if let Some(file) = value("--dump-sim") {
                    use std::io::Write;
                    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&file).unwrap();
                    let mut put = |t: &Trs| {
                        for v in t.translation.to_array().iter().chain(&t.rotation.to_array()).chain(&t.scale.to_array()) {
                            f.write_all(&v.to_le_bytes()).unwrap();
                        }
                    };
                    put(&to_world);
                    for t in &pre {
                        put(t);
                    }
                }
                // `--trace-bone NAME`: that bone of the outfit each step (retargeted + KineDriver, then after its simulation)
                if let Some(b) = value("--trace-bone").and_then(|n| r.index_of(&n)) {
                    if let (Some(a), Some(z)) = (pre.get(b), posed.1.get(b)) {
                        println!("trace: step {step} ({:?}): pose {:.1?} sim {:.1?}", rig.out.state, a.translation.to_array(), z.translation.to_array());
                    }
                }
            }
            // (ER90, XSI request 1) the XS moves' form weapons, as kh-er sora.rs submits them (the Kingdom Key hidden meanwhile)
            let forms = match rig.sora.attack().map(|a| a.kind) {
                Some(kind) if !flag("--no-forms") => {
                    use kh_core::abilities::xs;
                    let name = match kind {
                        xs::finishers::GIANT_STAMP => "giant_stamp",
                        xs::chandelier::CHANDELIER => "chandelier",
                        xs::myriad::MYRIAD => "myriad",
                        _ => "",
                    };
                    if name.is_empty() {
                        Vec::new()
                    } else {
                        // (XW) each weapon posed by its own clip at the time of his clip of the move
                        let sequences = rig.out.anim.name.sequences();
                        let time = kh_er_render::xsiweapon::clip_time(name, sequences.first().copied(), rig.out.anim.time);
                        kh_er_render::xsiweapon::place(name, &|b| rig.animator.skeleton().index_of(b), &rig.pose.component, &to_world, &socket, time)
                    }
                }
                _ => Vec::new(),
            };
            if !forms.is_empty() && flag("--list-fx") && step % 6 == 0 {
                println!("    step {step}: form weapons {:?}", forms.iter().map(|f| f.model.as_str()).collect::<Vec<_>>());
            }
            // (XW-b) Chandelier's chandelier pawn, as kh-er sora.rs submits it
            let mut forms = forms;
            if !flag("--no-forms") {
                if let Some(p) = rig.sora.xs_chandelier_pawn() {
                    forms.extend(kh_er_render::xsiweapon::pawn_form("chandelier", p.ground, p.yaw, p.time));
                }
            }
            let weapon = if forms.is_empty() { weapon } else { None };
            // (HY) his feet: the scene of Elden Ring's kind places its casters and lights around them
            // (RSF2) `--level-open 0`: the light level from the shadow at his feet (r.level_open 0)
            kh_er_render::light::set_open_level(value("--level-open").as_deref() != Some("0"));
            // (RSF) `--no-sora`: Sora is not drawn (a friend leads, Sora is out of the party): the friends are the only figures
            let (body, weapon, forms) = if flag("--no-sora") { (None, None, Vec::new()) } else { (body, weapon, forms) };
            let hy_feet = body.as_ref().map_or(Vec3::ZERO, |b| b.component_to_world.translation);
            kh_er_render::submit(FrameInput { camera, origin: Vec3::ZERO, clock: step as f32 * STEP, body, weapon, forms });
            // (ER party S4) the party of this frame
            #[cfg(feature = "party")]
            if let Some(p) = party_harness.as_mut() {
                p.frame(step, STEP, camera, rig.out.position);
            }

            // ---- the "game": its deferred submission (G-buffer, then floor and sky into the scene target with depth)
            let clip: Mat4 = math::clip_from_game(&game_camera, Vec3::ZERO); // (ZFIN round 8) --game-jitter
            let tan_y = (camera.fov * 0.5).tan();
            let (depth_a, depth_b) = math::depth_to_view(camera.near, camera.far);
            let mut frame: Vec<f32> = Vec::with_capacity(48);
            frame.extend_from_slice(&clip.to_cols_array());
            for v in [eye_game.extend(0.0), math::game_dir_of_host(game_camera.right).extend(0.0), math::game_dir_of_host(game_camera.up).extend(0.0), math::game_dir_of_host(game_camera.forward).extend(0.0)] {
                frame.extend_from_slice(&v.to_array());
            }
            frame.extend_from_slice(&[width() as f32, height() as f32, 1.0 / width() as f32, 1.0 / height() as f32]);
            frame.extend_from_slice(&[depth_a, depth_b, 1.0, 1.0 / (tan_y * camera.aspect)]);
            // (out_scale.x = 1: the scene target holds KH3's scene-linear values; z: the tone pass's exposure)
            frame.extend_from_slice(&[1.0, 0.0, exposure, wall]);
            // (lit.xyz: the floor's light; w: the sky's)
            frame.extend_from_slice(&[dark, dark, dark, dark]);
            std::ptr::copy_nonoverlapping(frame.as_ptr() as *const u8, constants_at as *mut u8, frame.len() * 4);
            let viewport = D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: width() as f32, Height: height() as f32, MinDepth: 0.0, MaxDepth: 1.0 };
            let scissor = RECT { left: 0, top: 0, right: width() as i32, bottom: height() as i32 };
            allocator.Reset().unwrap();
            list.Reset(&allocator, None).unwrap();
            // The G-buffer pass: four targets to RENDER_TARGET, then the depth to DEPTH_WRITE.
            let to_targets: Vec<_> = g_buffer.iter().map(|t| transition(t, SRV, D3D12_RESOURCE_STATE_RENDER_TARGET)).collect();
            list.ResourceBarrier(&to_targets);
            list.ResourceBarrier(&[transition(&depth, SRV, D3D12_RESOURCE_STATE_DEPTH_WRITE)]);
            let views: Vec<D3D12_CPU_DESCRIPTOR_HANDLE> = (0..g_buffer.len()).map(rtv).collect();
            list.OMSetRenderTargets(views.len() as u32, Some(views.as_ptr()), false, Some(&dsv));
            list.ClearDepthStencilView(dsv, D3D12_CLEAR_FLAG_DEPTH | D3D12_CLEAR_FLAG_STENCIL, 0.0, 0, None);
            if hybrid {
                // (IMPL_ER78) the floor's G-buffer: normal up (host Y), the floor's grey; Sora's pass writes his own
                list.ClearRenderTargetView(rtv(0), &[0.5, 1.0, 0.5, 0.0], None);
                list.ClearRenderTargetView(rtv(2), &[0.3, 0.3, 0.3, 0.0], None);
                // (RSF2 round 11) `--hy3-sss`: the world behind carries a subsurface profile and an emissive (foliage, the Erdtree):
                // what targets 3 / 4 hold under him unless the renderer clears them (r.hybrid_gbclear; the dump shows them)
                if flag("--hy3-sss") {
                    list.ClearRenderTargetView(rtv(3), &[1.0, 5.0 / 255.0, 0.0, 0.0], None);
                    list.ClearRenderTargetView(rtv(4), &[0.5, 0.4, 0.1, 1.0], None);
                }
            } else if let Some(colour) = albedo {
                list.ClearRenderTargetView(rtv(0), &colour, None);
            }
            let from_targets: Vec<_> = g_buffer.iter().map(|t| transition(t, D3D12_RESOURCE_STATE_RENDER_TARGET, SRV)).collect();
            list.ResourceBarrier(&from_targets);
            list.ResourceBarrier(&[transition(&depth, D3D12_RESOURCE_STATE_DEPTH_WRITE, SRV)]);
            // The "deferred composite": the scene target with the depth.
            list.ResourceBarrier(&[transition(&scene, SRV, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
            list.ResourceBarrier(&[transition(&depth, SRV, D3D12_RESOURCE_STATE_DEPTH_WRITE)]);
            list.OMSetRenderTargets(1, Some(&rtv(SCENE_RTV)), false, Some(&dsv));
            list.RSSetViewports(&[viewport]);
            list.RSSetScissorRects(&[scissor]);
            list.SetGraphicsRootSignature(&root);
            list.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress());
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            match fx_pipeline.as_ref() {
                // (IMPL_ER56) with --fxtest: the floor writes stencil 3, the sky gets 0 back
                Some((_, _, floor_state, mark_state, _, _)) => {
                    list.OMSetStencilRef(3);
                    list.SetPipelineState(floor_state);
                    list.DrawInstanced(3, 1, 0, 0);
                    list.OMSetStencilRef(0);
                    list.SetPipelineState(mark_state);
                    list.DrawInstanced(3, 1, 0, 0);
                }
                None => {
                    list.SetPipelineState(&floor_pipeline);
                    list.DrawInstanced(3, 1, 0, 0);
                }
            }
            // (ZFIN) Elden Ring's characters, with their depth and the characters' stencil class (0x50), and the finale's state
            if let Some(state) = erchr_pipeline.as_ref() {
                let (caps, finale) = erchr_frame(&erchars, step as u32, finale_at, finale_survive, finale_push, finale_fall);
                let zfin = finale.clone(); // (ZDRAW)
                kh_er_render::finale::set(finale.into_iter().collect());
                list.SetPipelineState(state);
                // (one draw per character, with its own stencil value; blocks of 1 KB from 8 KB on)
                for (c, character) in erchars.iter().enumerate().take(7) {
                    let mut b = frame.clone();
                    let mine: Vec<_> = caps.iter().filter(|x| x.0 == c).collect();
                    if mine.is_empty() || (c == 0 && zh.is_some()) /* (ZDRAW) its own model instead */ {
                        continue;
                    }
                    for k in 0..24 {
                        let (a, z, r, tint) = mine.get(k).map_or((Vec3::ZERO, Vec3::ZERO, 0.0, 0.0), |x| (x.1, x.2, x.3, x.4));
                        b.extend_from_slice(&[a.x, a.y, a.z, r, z.x, z.y, z.z, tint]);
                    }
                    let at = 8192 + 1024 * c;
                    std::ptr::copy_nonoverlapping(b.as_ptr() as *const u8, (constants_at as *mut u8).add(at), b.len() * 4);
                    list.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress() + at as u64);
                    list.OMSetStencilRef(u32::from(character.3));
                    list.DrawInstanced(3, 1, 0, 0);
                }
                list.OMSetStencilRef(0);
                list.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress());
                // (ZDRAW) the target as its own model: the finale's mask fed, the "game" drawing its body until it is hidden
                if let Some(z) = zh.as_mut() {
                    z.step(zfin.as_ref());
                    let dir = std::env::var_os("KH_FINALE_SHADERS").map(PathBuf::from).unwrap_or_else(|| project.join("build").join("sora_tmp").join("er").join("zdraw").join("pack").join("materials").join("er"));
                    z.draw_body(&device, &list, &clip, SCENE_FORMAT, &dir);
                    list.SetGraphicsRootSignature(&root);
                    list.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress());
                }
            }
            list.ResourceBarrier(&[transition(&scene, D3D12_RESOURCE_STATE_RENDER_TARGET, SRV)]);
            list.ResourceBarrier(&[transition(&depth, D3D12_RESOURCE_STATE_DEPTH_WRITE, SRV)]);
            if erdtree || glowtest {
                // (IMPL_ER65) the copy is made in the G-buffer list itself: BEFORE his early depth (a prepass copy)
                list.ResourceBarrier(&[transition(&depth, SRV, D3D12_RESOURCE_STATE_COPY_SOURCE), transition(&depth_copy, SRV, D3D12_RESOURCE_STATE_COPY_DEST)]);
                list.CopyResource(&depth_copy, &depth);
                list.ResourceBarrier(&[transition(&depth, D3D12_RESOURCE_STATE_COPY_SOURCE, SRV), transition(&depth_copy, D3D12_RESOURCE_STATE_COPY_DEST, SRV)]);
            }
            list.Close().unwrap();
            if fxtest {
                // (IMPL_ER53) the second list of the deferred stage: the depth copy the "forward stage" reads
                allocator3.Reset().unwrap();
                list3.Reset(&allocator3, None).unwrap();
                // (IMPL_ER78) --hybrid: the light passes (after the G-buffer list, so after his G-buffer pass): the pair of
                // light targets from the G-buffer's normals (sun toward host (0.6, 0.6, -0.5), at the stand-in rig's white)
                if let (Some((light_state, _)), Some(hs)) = (hy3_pipelines.as_ref(), hy_scene.as_ref()) {
                    // (HY) the scene of Elden Ring's kind: its light passes write the light pair, the mask and the composed scene
                    let b = hs.block(&frame, hy_feet, &kh_er_render::test_capsules());
                    std::ptr::copy_nonoverlapping(b.as_ptr() as *const u8, (constants_at as *mut u8).add(HY_CONSTANTS), b.len() * 4);
                    let targets = [&light_pair[0], &light_pair[1], &hy_mask, &scene];
                    list3.ResourceBarrier(&targets.iter().map(|t| transition(t, SRV, D3D12_RESOURCE_STATE_RENDER_TARGET)).collect::<Vec<_>>());
                    let views = [rtv(6), rtv(7), rtv(12), rtv(SCENE_RTV)];
                    list3.OMSetRenderTargets(4, Some(views.as_ptr()), false, None);
                    list3.RSSetViewports(&[viewport]);
                    list3.RSSetScissorRects(&[scissor]);
                    list3.SetGraphicsRootSignature(&root);
                    list3.SetDescriptorHeaps(&[Some(srv_heap.clone())]);
                    let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as u64;
                    list3.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + 2 * srv_size });
                    list3.SetGraphicsRootDescriptorTable(2, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + 11 * srv_size });
                    list3.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress() + HY_CONSTANTS as u64);
                    list3.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    list3.SetPipelineState(light_state);
                    list3.DrawInstanced(3, 1, 0, 0);
                    list3.ResourceBarrier(&targets.iter().map(|t| transition(t, D3D12_RESOURCE_STATE_RENDER_TARGET, SRV)).collect::<Vec<_>>());
                } else if let (true, Some(state)) = (hybrid, hy_light_pipeline.as_ref()) {
                    let mut b = frame.clone();
                    let n = b.len();
                    let white = kh_er_render::light::reference_white();
                    b[n - 8..n - 4].copy_from_slice(&[white * hy_levels[0], white * hy_levels.get(1).copied().unwrap_or(0.12), exposure, 0.0]);
                    b[n - 4..].copy_from_slice(&[hy_sun[0], hy_sun[1], hy_sun[2], 0.0]);
                    std::ptr::copy_nonoverlapping(b.as_ptr() as *const u8, (constants_at as *mut u8).add(3072), b.len() * 4);
                    list3.ResourceBarrier(&[transition(&light_pair[0], SRV, D3D12_RESOURCE_STATE_RENDER_TARGET), transition(&light_pair[1], SRV, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                    list3.OMSetRenderTargets(2, Some(&rtv(6)), true, None);
                    list3.RSSetViewports(&[viewport]);
                    list3.RSSetScissorRects(&[scissor]);
                    list3.SetGraphicsRootSignature(&root);
                    list3.SetDescriptorHeaps(&[Some(srv_heap.clone())]);
                    let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as u64;
                    list3.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + 2 * srv_size });
                    list3.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress() + 3072);
                    list3.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    list3.SetPipelineState(state);
                    list3.DrawInstanced(3, 1, 0, 0);
                    list3.ResourceBarrier(&[transition(&light_pair[0], D3D12_RESOURCE_STATE_RENDER_TARGET, SRV), transition(&light_pair[1], D3D12_RESOURCE_STATE_RENDER_TARGET, SRV)]);
                }
                // (IMPL_ER80) THE DEFERRED COMPOSITE's pair: [scene | glow source] with the depth written (as the game's
                // GXLightComposite): the glow source gets the Erdtree's emissive behind him (from the copy made before his depth)
                if let Some((source, _)) = glow_states.as_ref() {
                    let mut b = frame.clone();
                    let n = b.len();
                    b[n - 8..n - 4].copy_from_slice(&[glow_scale, 0.0, exposure, 0.0]);
                    std::ptr::copy_nonoverlapping(b.as_ptr() as *const u8, (constants_at as *mut u8).add(2048), b.len() * 4);
                    list3.ResourceBarrier(&[transition(&scene, SRV, D3D12_RESOURCE_STATE_RENDER_TARGET), transition(&glow_target, SRV, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                    list3.ResourceBarrier(&[transition(&depth, SRV, D3D12_RESOURCE_STATE_DEPTH_WRITE)]);
                    let pair = [rtv(SCENE_RTV), rtv(8)];
                    list3.OMSetRenderTargets(2, Some(pair.as_ptr()), false, Some(&dsv));
                    list3.RSSetViewports(&[viewport]);
                    list3.RSSetScissorRects(&[scissor]);
                    list3.SetGraphicsRootSignature(&root);
                    list3.SetDescriptorHeaps(&[Some(srv_heap.clone())]);
                    let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as u64;
                    list3.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + srv_size });
                    list3.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress() + 2048);
                    list3.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    list3.SetPipelineState(source);
                    list3.DrawInstanced(3, 1, 0, 0);
                    list3.ResourceBarrier(&[transition(&scene, D3D12_RESOURCE_STATE_RENDER_TARGET, SRV), transition(&glow_target, D3D12_RESOURCE_STATE_RENDER_TARGET, SRV)]);
                    list3.ResourceBarrier(&[transition(&depth, D3D12_RESOURCE_STATE_DEPTH_WRITE, SRV)]);
                }
                if !(erdtree || glowtest) {
                    list3.ResourceBarrier(&[transition(&depth, SRV, D3D12_RESOURCE_STATE_COPY_SOURCE), transition(&depth_copy, SRV, D3D12_RESOURCE_STATE_COPY_DEST)]);
                    list3.CopyResource(&depth_copy, &depth);
                    list3.ResourceBarrier(&[transition(&depth, D3D12_RESOURCE_STATE_COPY_SOURCE, SRV), transition(&depth_copy, D3D12_RESOURCE_STATE_COPY_DEST, SRV)]);
                } else {
                    list3.ResourceBarrier(&[transition(&depth, SRV, D3D12_RESOURCE_STATE_DEPTH_WRITE)]);
                    list3.ResourceBarrier(&[transition(&depth, D3D12_RESOURCE_STATE_DEPTH_WRITE, SRV)]);
                }
                list3.Close().unwrap();
                // (the renderer's hook splits this call after the G-buffer list for his early depth, and runs its own
                // colour list after both)
                queue.ExecuteCommandLists(&[Some(list.cast().unwrap()), Some(list3.cast().unwrap())]);
                // ---- the "game's" forward stage: decal + panels, tested against the scene depth
                if let Some((fx, sky_state, _, _, sky_free, velocity_state)) = fx_pipeline.as_ref() {
                    let block = |lit: [f32; 4], out_scale: [f32; 4]| -> Vec<f32> {
                        let mut b = frame.clone();
                        let n = b.len();
                        b[n - 8..n - 4].copy_from_slice(&out_scale);
                        b[n - 4..].copy_from_slice(&lit);
                        b
                    };
                    let layers: Vec<Vec<f32>> = if erdtree {
                        // the glow masked by the early copy (no depth test, no stencil), a fire panel in front of his legs only
                        vec![block([4.0, 0.0, 0.0, 0.0], [2.5, 0.0, exposure, 0.0]), block([2.0, -140.0, 22.0, 90.0], [1.6, 1.0, exposure, 0.0])]
                    } else {
                        vec![block([1.0, 100.0, 100.0, 420.0], [0.9, 0.0, exposure, 0.0]), block([2.0, 220.0, 70.0, 190.0], [1.2, 0.0, exposure, 0.0]), block([2.0, -140.0, 22.0, 210.0], [1.2, 1.0, exposure, 0.0]), block([3.0, 0.0, 0.0, 0.0], [1.5, 0.0, exposure, 0.0])]
                    };
                    for (k, b) in layers.iter().enumerate() {
                        std::ptr::copy_nonoverlapping(b.as_ptr() as *const u8, (constants_at as *mut u8).add(1024 + 256 * k), b.len() * 4);
                    }
                    allocator4.Reset().unwrap();
                    list4.Reset(&allocator4, None).unwrap();
                    list4.ResourceBarrier(&[transition(&scene, SRV, D3D12_RESOURCE_STATE_RENDER_TARGET), transition(&depth, SRV, D3D12_RESOURCE_STATE_DEPTH_WRITE)]);
                    list4.OMSetRenderTargets(1, Some(&rtv(SCENE_RTV)), false, Some(&dsv));
                    list4.RSSetViewports(&[viewport]);
                    list4.RSSetScissorRects(&[scissor]);
                    list4.SetGraphicsRootSignature(&root);
                    list4.SetDescriptorHeaps(&[Some(srv_heap.clone())]);
                    let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as u64;
                    list4.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + srv_size });
                    list4.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    list4.SetPipelineState(fx);
                    // (IMPL_ER80) `--no-layers`: the forward stage without its decal / panels / sky glow layers (the velocity pass stays)
                    for k in 0..if flag("--no-layers") { 0 } else { layers.len() } {
                        if erdtree && k == 0 {
                            list4.SetPipelineState(sky_free);
                        } else if erdtree {
                            list4.SetPipelineState(fx);
                        } else if k == 3 {
                            // the sky glow, masked by the stencil (0 = sky)
                            list4.SetPipelineState(sky_state);
                            list4.OMSetStencilRef(0);
                        }
                        list4.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress() + 1024 + 256 * k as u64);
                        list4.DrawInstanced(3, 1, 0, 0);
                    }
                    // (IMPL_ER80b) `--front-glow`: a glowing panel in front of his legs written into the GLOW SOURCE (additive,
                    // depth tested against the scene depth with him), after his glow mask: it must still glow over him
                    if flag("--front-glow") && glowtest {
                        let b = block([2.0, -140.0, 22.0, 90.0], [1.6, 1.0, exposure, 0.0]);
                        std::ptr::copy_nonoverlapping(b.as_ptr() as *const u8, (constants_at as *mut u8).add(3328), b.len() * 4);
                        list4.ResourceBarrier(&[transition(&glow_target, SRV, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                        list4.OMSetRenderTargets(1, Some(&rtv(8)), false, Some(&dsv));
                        list4.SetPipelineState(fx);
                        list4.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress() + 3328);
                        list4.DrawInstanced(3, 1, 0, 0);
                        list4.ResourceBarrier(&[transition(&glow_target, D3D12_RESOURCE_STATE_RENDER_TARGET, SRV)]);
                    }
                    // (HY) the scene of Elden Ring's kind: its fog over the composed picture with him in it (from the depth copy
                    // made after his early depth), as the game's volumetric fog composite
                    if let (Some((_, fog_state)), Some(hs)) = (hy3_pipelines.as_ref(), hy_scene.as_ref()) {
                        if hs.fog_density > 0.0 {
                            let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as u64;
                            list4.OMSetRenderTargets(1, Some(&rtv(SCENE_RTV)), false, None);
                            list4.SetGraphicsRootDescriptorTable(2, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + srv_size });
                            list4.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress() + HY_CONSTANTS as u64);
                            list4.SetPipelineState(fog_state);
                            list4.DrawInstanced(3, 1, 0, 0);
                        }
                    }
                    list4.ResourceBarrier(&[transition(&scene, D3D12_RESOURCE_STATE_RENDER_TARGET, SRV), transition(&depth, D3D12_RESOURCE_STATE_DEPTH_WRITE, SRV)]);
                    if erdtree || taa {
                        // the "velocity pass" (the late pass runs after the submission that holds it)
                        list4.ResourceBarrier(&[transition(&velocity_target, SRV, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                        list4.OMSetRenderTargets(1, Some(&velocity_rtv.GetCPUDescriptorHandleForHeapStart()), false, None);
                        list4.SetPipelineState(velocity_state);
                        list4.DrawInstanced(3, 1, 0, 0);
                        list4.ResourceBarrier(&[transition(&velocity_target, D3D12_RESOURCE_STATE_RENDER_TARGET, SRV)]);
                    }
                    list4.Close().unwrap();
                    // (IMPL_ER80) `--taa`: the TAA stand-in in the next list of the same submission (as the game's TempAA after its
                    // velocity list): history clamped to the picture's neighbourhood, blended; the result copied to the scene and
                    // kept as the next history
                    if let Some(state) = taa_state.as_ref() {
                        let mut b = frame.clone();
                        let n = b.len();
                        b[n - 8..n - 4].copy_from_slice(&[if taa_frames == 0 { 1.0 } else { taa_blend }, 0.0, exposure, 0.0]);
                        std::ptr::copy_nonoverlapping(b.as_ptr() as *const u8, (constants_at as *mut u8).add(2560), b.len() * 4);
                        taa_frames += 1;
                        allocator5.Reset().unwrap();
                        list5.Reset(&allocator5, None).unwrap();
                        list5.ResourceBarrier(&[transition(&taa_out, SRV, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                        list5.OMSetRenderTargets(1, Some(&rtv(9)), false, None);
                        list5.RSSetViewports(&[viewport]);
                        list5.RSSetScissorRects(&[scissor]);
                        list5.SetGraphicsRootSignature(&root);
                        list5.SetDescriptorHeaps(&[Some(srv_heap.clone())]);
                        let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as u64;
                        list5.SetGraphicsRootDescriptorTable(1, srv_heap.GetGPUDescriptorHandleForHeapStart());
                        list5.SetGraphicsRootDescriptorTable(2, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + 4 * srv_size });
                        list5.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress() + 2560);
                        list5.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                        list5.SetPipelineState(state);
                        list5.DrawInstanced(3, 1, 0, 0);
                        list5.ResourceBarrier(&[transition(&taa_out, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE), transition(&scene, SRV, D3D12_RESOURCE_STATE_COPY_DEST), transition(&history, SRV, D3D12_RESOURCE_STATE_COPY_DEST)]);
                        list5.CopyResource(&scene, &taa_out);
                        list5.CopyResource(&history, &taa_out);
                        list5.ResourceBarrier(&[transition(&taa_out, D3D12_RESOURCE_STATE_COPY_SOURCE, SRV), transition(&scene, D3D12_RESOURCE_STATE_COPY_DEST, SRV), transition(&history, D3D12_RESOURCE_STATE_COPY_DEST, SRV)]);
                        list5.Close().unwrap();
                        queue.ExecuteCommandLists(&[Some(list4.cast().unwrap()), Some(list5.cast().unwrap())]);
                    } else {
                        queue.ExecuteCommandLists(&[Some(list4.cast().unwrap())]);
                    }
                }
            } else {
                // (the renderer's hook on this call recognises the deferred stage and runs its own list right after it)
                queue.ExecuteCommandLists(&[Some(list.cast().unwrap())]);
            }

            // ---- the "game's" tone mapping and final composite into the back buffer
            let index = swap.GetCurrentBackBufferIndex();
            let back: ID3D12Resource = swap.GetBuffer(index).unwrap();
            device.CreateRenderTargetView(&back, None, back_rtv_heap.GetCPUDescriptorHandleForHeapStart());
            allocator2.Reset().unwrap();
            list2.Reset(&allocator2, None).unwrap();
            list2.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
            list2.OMSetRenderTargets(1, Some(&back_rtv_heap.GetCPUDescriptorHandleForHeapStart()), false, None);
            list2.RSSetViewports(&[viewport]);
            list2.RSSetScissorRects(&[scissor]);
            list2.SetGraphicsRootSignature(&root);
            list2.SetDescriptorHeaps(&[Some(srv_heap.clone())]);
            list2.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress());
            list2.SetGraphicsRootDescriptorTable(1, srv_heap.GetGPUDescriptorHandleForHeapStart());
            list2.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            // (IMPL_ER80b) the game's light shafts: the source pass, then THE GAME'S OWN LightShaft_CompositePS (its own root
            // signature from the shader, t0 / s1 through its tables 30 / 33), additive over the scene
            if let (Some(file), Some(src), false) = (shafts_file.as_ref(), shaft_src_state.as_ref(), warming) {
                if game_shaft.is_none() {
                    let ps = std::fs::read(file).unwrap_or_else(|e| panic!("{file}: {e}"));
                    let vs = shader("gamequad.vs.dxil");
                    let rs: ID3D12RootSignature = device.CreateRootSignature(0, &ps).expect("the game shader's root signature");
                    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
                    formats[0] = SCENE_FORMAT;
                    let mut blend = D3D12_BLEND_DESC::default();
                    blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC { BlendEnable: true.into(), LogicOpEnable: false.into(), SrcBlend: D3D12_BLEND_ONE, DestBlend: D3D12_BLEND_ONE, BlendOp: D3D12_BLEND_OP_ADD, SrcBlendAlpha: D3D12_BLEND_ZERO, DestBlendAlpha: D3D12_BLEND_ONE, BlendOpAlpha: D3D12_BLEND_OP_ADD, LogicOp: D3D12_LOGIC_OP_NOOP, RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8 };
                    let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                        pRootSignature: std::mem::transmute_copy(&rs),
                        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vs.as_ptr() as *const c_void, BytecodeLength: vs.len() },
                        PS: D3D12_SHADER_BYTECODE { pShaderBytecode: ps.as_ptr() as *const c_void, BytecodeLength: ps.len() },
                        BlendState: blend,
                        SampleMask: u32::MAX,
                        RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: true.into(), ..Default::default() },
                        DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), StencilEnable: false.into(), ..Default::default() },
                        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                        NumRenderTargets: 1,
                        RTVFormats: formats,
                        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                        ..Default::default()
                    };
                    let pso: ID3D12PipelineState = device.CreateGraphicsPipelineState(&desc).expect("the game's light-shaft composite pipeline state");
                    println!("effects: (IMPL_ER80b) the game's LightShaft_CompositePS pipeline made from {file}");
                    game_shaft = Some((rs, pso));
                }
                let mut b = frame.clone();
                let n = b.len();
                b[n - 4..].copy_from_slice(&[shaft_at[0], shaft_at[1], shaft_at[2], 0.0]);
                std::ptr::copy_nonoverlapping(b.as_ptr() as *const u8, (constants_at as *mut u8).add(3584), b.len() * 4);
                let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as u64;
                list2.ResourceBarrier(&[transition(&shaft_target, SRV, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                list2.OMSetRenderTargets(1, Some(&rtv(10)), false, None);
                list2.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + 5 * srv_size });
                list2.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress() + 3584);
                list2.SetPipelineState(src);
                list2.DrawInstanced(3, 1, 0, 0);
                list2.ResourceBarrier(&[transition(&shaft_target, D3D12_RESOURCE_STATE_RENDER_TARGET, SRV)]);
                // (the composite's target: the scene, or with --shaft-half a half-resolution target of its own, cleared)
                let (into, into_rtv, into_view) = if shaft_half { (&half_target, rtv(11), D3D12_VIEWPORT { Width: (width() / 2) as f32, Height: (height() / 2) as f32, ..viewport }) } else { (&scene, rtv(SCENE_RTV), viewport) };
                list2.ResourceBarrier(&[transition(into, SRV, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                if shaft_half {
                    list2.ClearRenderTargetView(into_rtv, &[0.0, 0.0, 0.0, 0.0], None);
                }
                if let Some((rs, pso)) = game_shaft.as_ref() {
                    list2.SetGraphicsRootSignature(rs);
                    list2.SetDescriptorHeaps(&[Some(srv_heap.clone()), Some(sampler_heap.clone())]);
                    list2.SetGraphicsRootDescriptorTable(30, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + 6 * srv_size });
                    list2.SetGraphicsRootDescriptorTable(33, sampler_heap.GetGPUDescriptorHandleForHeapStart());
                    list2.RSSetViewports(&[into_view]);
                    list2.OMSetRenderTargets(1, Some(&into_rtv), false, None);
                    list2.SetPipelineState(pso);
                    list2.DrawInstanced(3, 1, 0, 0);
                }
                list2.ResourceBarrier(&[transition(into, D3D12_RESOURCE_STATE_RENDER_TARGET, SRV)]);
                list2.RSSetViewports(&[viewport]);
                if let (true, Some(up)) = (shaft_half, upadd_state.as_ref()) {
                    let mut b = frame.clone();
                    let n = b.len();
                    b[n - 4..].copy_from_slice(&[0.5, 0.0, 0.0, 0.0]);
                    std::ptr::copy_nonoverlapping(b.as_ptr() as *const u8, (constants_at as *mut u8).add(3840), b.len() * 4);
                    list2.SetGraphicsRootSignature(&root);
                    list2.SetDescriptorHeaps(&[Some(srv_heap.clone())]);
                    list2.ResourceBarrier(&[transition(&scene, SRV, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                    list2.OMSetRenderTargets(1, Some(&rtv(SCENE_RTV)), false, None);
                    list2.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + 10 * srv_size });
                    list2.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress() + 3840);
                    list2.SetPipelineState(up);
                    list2.DrawInstanced(3, 1, 0, 0);
                    list2.ResourceBarrier(&[transition(&scene, D3D12_RESOURCE_STATE_RENDER_TARGET, SRV)]);
                }
                list2.SetGraphicsRootSignature(&root);
                list2.SetDescriptorHeaps(&[Some(srv_heap.clone())]);
                list2.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress());
                list2.SetGraphicsRootDescriptorTable(1, srv_heap.GetGPUDescriptorHandleForHeapStart());
                list2.OMSetRenderTargets(1, Some(&back_rtv_heap.GetCPUDescriptorHandleForHeapStart()), false, None);
            }
            // (IMPL_ER80) the game's glow chain: the glow source blurred and added over the scene, before the tone mapping
            if let Some((_, chain)) = glow_states.as_ref() {
                let mut b = frame.clone();
                let n = b.len();
                b[n - 8..n - 4].copy_from_slice(&[1.0, 0.0, exposure, 0.0]);
                std::ptr::copy_nonoverlapping(b.as_ptr() as *const u8, (constants_at as *mut u8).add(2304), b.len() * 4);
                let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as u64;
                list2.ResourceBarrier(&[transition(&scene, SRV, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                list2.OMSetRenderTargets(1, Some(&rtv(SCENE_RTV)), false, None);
                list2.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + 3 * srv_size });
                list2.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress() + 2304);
                list2.SetPipelineState(chain);
                list2.DrawInstanced(3, 1, 0, 0);
                list2.ResourceBarrier(&[transition(&scene, D3D12_RESOURCE_STATE_RENDER_TARGET, SRV)]);
                list2.OMSetRenderTargets(1, Some(&back_rtv_heap.GetCPUDescriptorHandleForHeapStart()), false, None);
                list2.SetGraphicsRootDescriptorTable(1, srv_heap.GetGPUDescriptorHandleForHeapStart());
                list2.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress());
            }
            list2.SetPipelineState(&tone_pipeline);
            list2.DrawInstanced(3, 1, 0, 0);
            list2.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PRESENT)]);
            list2.Close().unwrap();
            queue.ExecuteCommandLists(&[Some(list2.cast().unwrap())]);
            if let Err(e) = swap.Present(u32::from(vsync), DXGI_PRESENT(0)).ok() {
                println!("effects: Present failed: {e}");
                break;
            }
            wait(&mut fence_value);
            presented += 1;
            if presented % 120 == 0 {
                println!("effects: frame {presented} (step {step}, {:.1} s): {}", run_started.elapsed().as_secs_f32(), kh_er_render::status());
            }
            // (IMPL_ER78) `--hy-dump STEP`: the r.hybrid_dump of the mod at that step (written into the --out folder)
            if !warming && hy_dump == Some(step as u32) {
                let dir = std::path::Path::new(&out_prefix).parent().map(|p| p.to_path_buf()).unwrap_or_default();
                kh_er_render::set_dump_dir(&dir);
                kh_er_render::hybrid_dump();
            }
            if !warming && shots.contains(&(step as u32)) {
                // The buffer just presented keeps its picture (sequential flip): read it back.
                allocator.Reset().unwrap();
                list.Reset(&allocator, None).unwrap();
                list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
                let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&back), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
                let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&readback), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: footprint } };
                list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
                list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_PRESENT)]);
                list.Close().unwrap();
                // (a list of the harness that the renderer must not take for a submission of the game: it finishes no back buffer)
                queue.ExecuteCommandLists(&[Some(list.cast().unwrap())]);
                wait(&mut fence_value);
                let mut at: *mut c_void = std::ptr::null_mut();
                readback.Map(0, None, Some(&mut at)).unwrap();
                let mut pixels = Vec::with_capacity((width() * height() * 4) as usize);
                for y in 0..height() as usize {
                    pixels.extend_from_slice(std::slice::from_raw_parts((at as *const u8).add(footprint.Offset as usize + y * footprint.Footprint.RowPitch as usize), width() as usize * 4));
                }
                readback.Unmap(0, None);
                // (IMPL_ER80) `--flicker-box`: the mean absolute frame-to-frame difference inside the box (0..255 per channel) and
                // the share of its pixels that changed by more than 24 in any channel, against the previous shot
                if let Some([x0, y0, x1, y1]) = flicker_box {
                    let mut cut = Vec::new();
                    for y in y0.min(height())..y1.min(height()) {
                        let row = (y * width()) as usize * 4;
                        cut.extend_from_slice(&pixels[row + x0.min(width()) as usize * 4..row + x1.min(width()) as usize * 4]);
                    }
                    // (ZFIN round 8) the box's mean luma this shot (the circle's per-frame trace)
                    {
                        let n = (cut.len() / 4).max(1) as f64;
                        let luma: f64 = cut.chunks(4).map(|c| 0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64).sum::<f64>() / n;
                        println!("luma: step {step}: {luma:.3}");
                    }
                    if let Some(prev) = flicker_prev.as_ref().filter(|p| p.len() == cut.len()) {
                        let (mut sum, mut big, mut n) = (0u64, 0u64, 0u64);
                        for (a, b) in cut.chunks(4).zip(prev.chunks(4)) {
                            let d: Vec<u64> = (0..3).map(|k| (a[k] as i64 - b[k] as i64).unsigned_abs()).collect();
                            sum += d.iter().sum::<u64>();
                            big += u64::from(d.iter().any(|v| *v > 24));
                            n += 1;
                        }
                        let mean = sum as f64 / (3 * n.max(1)) as f64;
                        flicker_sum = (flicker_sum.0 + mean, flicker_sum.1 + big, flicker_sum.2 + 1);
                        let mut line = format!("flicker: step {step}: mean |d| {mean:.3}, {big} of {n} pixels changed by > 24");
                        if let Some(prev2) = flicker_prev2.as_ref().filter(|p| p.len() == cut.len()) {
                            let (mut sum2, mut big2) = (0u64, 0u64);
                            for ((a, b), c) in cut.chunks(4).zip(prev.chunks(4)).zip(prev2.chunks(4)) {
                                let d: Vec<u64> = (0..3).map(|k| (a[k] as i64 - 2 * b[k] as i64 + c[k] as i64).unsigned_abs()).collect();
                                sum2 += d.iter().sum::<u64>();
                                big2 += u64::from(d.iter().any(|v| *v > 24));
                            }
                            let mean2 = sum2 as f64 / (3 * n.max(1)) as f64;
                            flicker_sum2 = (flicker_sum2.0 + mean2, flicker_sum2.1 + big2, flicker_sum2.2 + 1);
                            line += &format!("; flicker (2nd difference) {mean2:.3}, {big2} pixels > 24");
                        }
                        println!("{line}");
                    }
                    flicker_prev2 = flicker_prev.take();
                    flicker_prev = Some(cut);
                }
                let path = format!("{out_prefix}_{step:03}.png");
                let file = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
                let mut encoder = png::Encoder::new(file, width(), height());
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
                println!("effects: wrote {path}");
                for line in kh_er_render::pointlight::describe() {
                    println!("    (IMPL_ER39) {line}");
                }
                if flag("--list") {
                    for line in kh_er_render::vfx::describe_latest() {
                        println!("    {line}");
                    }
                }
            }
        }
        let (systems, emitters, particles, open, ending) = effects.stats();
        let (shakes, spawned) = (effects.take_shakes().len(), effects.spawned);
        println!("effects: {step} steps; simulation {:.3} ms/step; effect host (step + draw list + prepare) {:.3} ms/step, peak {:.3} ms; alive at the end: {systems} systems, {emitters} emitters, {particles} particles ({open} persistent open, {ending} ending); {} spawned; shakes asked {}", sim_micros as f64 / 1000.0 / step.max(1) as f64, vfx_micros as f64 / 1000.0 / step.max(1) as f64, vfx_peak as f64 / 1000.0, spawned, shakes);
        // (IMPL_ER75) the trails over the whole run
        let t = effects.take_trail_stats();
        println!("effects: trail: {} step(s) with a trail; newest sample to the blade tip at most {:.1} cm; history up to {} sample(s); clock step {:.4} s; cuts {}", t.steps, t.max_gap, t.max_samples, t.dt, t.cuts);
        println!("effects: {}", kh_er_render::status());
        if flicker_sum.2 > 0 {
            println!("flicker: OVER {} frame pairs: mean |d| {:.3} per channel, {:.1} pixels a frame changed by > 24", flicker_sum.2, flicker_sum.0 / flicker_sum.2 as f64, flicker_sum.1 as f64 / flicker_sum.2 as f64);
            if flicker_sum2.2 > 0 {
                println!("flicker: OVER {} frame triples: 2nd difference {:.3} per channel, {:.1} pixels a frame > 24", flicker_sum2.2, flicker_sum2.0 / flicker_sum2.2 as f64, flicker_sum2.1 as f64 / flicker_sum2.2 as f64);
            }
        }
        // (ER73) the worn outfit's Bonamik and KineDriver
        println!("effects: {}", kh_er_render::outfit::sim_report());
        constants.Unmap(0, None);
        // (HY) a hybrid dump is written on a thread of its own: time to finish before the process ends
        if hy_dump.is_some() {
            std::thread::sleep(std::time::Duration::from_secs(4));
        }
        kh_er_render::shutdown();
        println!("effects: done");
    }
}

/// (ER79) `--stretch`: the outfit's skinned mesh on the CPU against its bind pose (gfx.rs's skin rule).
struct Stretch {
    model: kh_er_render::model::Model,
    joint_bone: Vec<Option<usize>>,
    /// (vertex a, vertex b, bind length)
    edges: Vec<(usize, usize, f32)>,
    names: Vec<String>,
    worst: f32,
    last_step: Option<usize>,
    guard_before: usize,
    guard_after: usize,
    guard_deepest: f32,
}

impl Stretch {
    fn new(file: &std::path::Path, r: &kh_er_render::outfit::Retarget) -> Option<Stretch> {
        let model = kh_er_render::model::Model::load(file).map_err(|e| eprintln!("stretch: {e}")).ok()?;
        let joint_bone = model.joints.iter().map(|(n, _)| r.index_of(n)).collect();
        let names = model.joints.iter().map(|(n, _)| n.clone()).collect();
        let mut seen = std::collections::HashSet::new();
        let mut edges = Vec::new();
        for p in &model.primitives {
            let tri = &model.indices[p.first_index as usize..(p.first_index + p.index_count) as usize];
            for t in tri.chunks(3).filter(|t| t.len() == 3) {
                for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                    let (a, b) = ((t[i] as i64 + p.base_vertex as i64) as usize, (t[j] as i64 + p.base_vertex as i64) as usize);
                    let key = (a.min(b), a.max(b));
                    if seen.insert(key) {
                        let len = (Vec3::from(model.vertices[a].position) - Vec3::from(model.vertices[b].position)).length();
                        if len > 1.0e-5 {
                            edges.push((a, b, len));
                        }
                    }
                }
            }
        }
        println!("stretch: {} ({} vertices, {} edges, {} joints, {} not in the outfit's skeleton)", file.display(), model.vertices.len(), edges.len(), model.joints.len(), model.joints.len() - model.joints.iter().filter(|(n, _)| r.index_of(n).is_some()).count());
        Some(Stretch { model, joint_bone, edges, names, worst: 0.0, last_step: None, guard_before: 0, guard_after: 0, guard_deepest: 0.0 })
    }

    fn step(&mut self, step: usize, posed: &[Trs], state: &str, r: &kh_er_render::outfit::Retarget) {
        // (the warm-up presents step 0 many times)
        if self.last_step == Some(step) {
            return;
        }
        self.last_step = Some(step);
        let mats: Vec<Mat4> = self.model.joints.iter().zip(&self.joint_bone).map(|((_, inv), b)| match b.and_then(|b| posed.get(b)) {
            Some(t) => kh_er_render::math::joint_in_pack(t) * *inv,
            None => Mat4::IDENTITY,
        }).collect();
        let skinned: Vec<Vec3> = self.model.vertices.iter().map(|v| {
            let mut m = Mat4::ZERO;
            for k in 0..4 {
                if v.weights[k] > 0.0 {
                    m += mats.get(v.joints[k] as usize).copied().unwrap_or(Mat4::IDENTITY) * v.weights[k];
                }
            }
            m.transform_point3(Vec3::from(v.position))
        }).collect();
        // (ER79) THE LEG GUARD's proof: cloth vertices (any weight on a cloth joint) inside the leg capsules (flat ends), before
        // and after the per-vertex guard (the skin shader's rule, done here on the CPU); the guarded positions go on
        let mut skinned = skinned;
        if let Some(g) = &r.guard {
            let caps = r.guard_capsules_pack(posed);
            let inside = |p: Vec3| -> Option<(f32, Vec3)> {
                let mut deepest: Option<(f32, Vec3)> = None;
                for (a, ra, b, rb) in &caps {
                    let d = *b - *a;
                    let t = (p - *a).dot(d) / d.length_squared().max(1.0e-10);
                    if !(0.0..=1.0).contains(&t) {
                        continue;
                    }
                    let q = *a + d * t;
                    let rad = ra + (rb - ra) * t;
                    let depth = rad - (p - q).length();
                    if depth > 1.0e-5 && deepest.is_none_or(|(x, _)| depth > x) {
                        deepest = Some((depth, q + (p - q) * (rad / (p - q).length().max(1.0e-6))));
                    }
                }
                deepest
            };
            let (mut before, mut after, mut deepest) = (0usize, 0usize, 0.0f32);
            let mut worst_vertex = None;
            for (k, v) in self.model.vertices.iter().enumerate() {
                if !(0..4).any(|c| v.weights[c] > 0.0 && g.is_cloth(v.joints[c] as usize)) {
                    continue;
                }
                if let Some((depth, _)) = inside(skinned[k]) {
                    before += 1;
                    if depth > deepest {
                        deepest = depth;
                        worst_vertex = Some(k);
                    }
                }
                // the shader's rule: each capsule in turn
                let mut p = skinned[k];
                let rounds: usize = std::env::var("KH_GUARD_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
                for (a, ra, b, rb) in std::iter::repeat_n(caps.iter(), rounds).flatten() {
                    let d = *b - *a;
                    let t = (p - *a).dot(d) / d.length_squared().max(1.0e-10);
                    if (0.0..=1.0).contains(&t) {
                        let q = *a + d * t;
                        let rad = ra + (rb - ra) * t;
                        let l = (p - q).length();
                        if l < rad && l > 1.0e-6 {
                            p = q + (p - q) * (rad / l);
                        }
                    }
                }
                skinned[k] = p;
                if inside(p).is_some_and(|(depth, _)| depth > 1.0e-4) {
                    after += 1;
                }
            }
            self.guard_before += before;
            self.guard_after += after;
            self.guard_deepest = self.guard_deepest.max(deepest * 100.0);
            if before > 0 || step % 60 == 0 {
                let wv = worst_vertex.map(|k| {
                    let x = &self.model.vertices[k];
                    (0..4).filter(|c| x.weights[*c] > 0.01).map(|c| format!("{} {:.2}", self.names.get(x.joints[c] as usize).map_or("?", |s| s.as_str()), x.weights[c])).collect::<Vec<_>>().join(", ")
                });
                println!("guard: step {step} ({state}): cloth vertices inside the leg capsules: {before} before the vertex guard (deepest {:.2} cm{}), {after} after; totals {} / {} (deepest so far {:.2} cm)", deepest * 100.0, wv.map_or(String::new(), |w| format!(", [{w}]")), self.guard_before, self.guard_after, self.guard_deepest);
            }
        }
        // the worst LENGTHENING in centimetres (pack units are metres), with its ratio
        let mut worst = (0.0f32, 0usize, 0usize, 1.0f32);
        for &(a, b, len) in &self.edges {
            let now = (skinned[a] - skinned[b]).length();
            if now - len > worst.0 {
                worst = ((now - len), a, b, now / len);
            }
        }
        worst.0 *= 100.0;
        let joints = |v: usize| {
            let x = &self.model.vertices[v];
            (0..4).filter(|k| x.weights[*k] > 0.01).map(|k| format!("{} {:.2}", self.names.get(x.joints[k] as usize).map_or("?", |s| s.as_str()), x.weights[k])).collect::<Vec<_>>().join(", ")
        };
        if worst.0 > 2.0 || step % 60 == 0 || worst.0 > self.worst * 1.2 && worst.0 > 1.0 {
            println!("stretch: step {step} ({state}): worst edge +{:.1} cm (x{:.2}) between v{} [{}] and v{} [{}]", worst.0, worst.3, worst.1, joints(worst.1), worst.2, joints(worst.2));
        }
        self.worst = self.worst.max(worst.0);
    }
}
