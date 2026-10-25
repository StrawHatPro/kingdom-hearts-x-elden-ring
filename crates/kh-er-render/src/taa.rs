//! (ER88) SORA AND ELDEN RING'S TEMPORAL ANTI-ALIASING: what the game's TAA needs from him, and a check of it in the log.
//!
//! The game's TAA (er80/dis/post: StencilAndMotionVectorPreprocess -> TempAA[StencilMask], IMPL_ER88_STATE.md) reprojects each
//! pixel by the motion vector at the nearest depth of its 2x2 block and weighs the history by a per-pixel CLASS taken from
//! stencil bits 6-7. His class is set in gfx.rs (`r.taa_char`: the characters' 0x50). His motion vectors (er_char.wgsl
//! `velo_ps`) take the camera's part from the game's own `VC_PrevMatrixViewProj` (cbSceneParam texels 29..32, relative to
//! `SC_PrevCameraPos`, texel 28), which carries THIS frame's jitter so that the jitter cancels.
//!
//! This module checks that assumption on the game-data read-back of consecutive frames: the game's previous view-projection
//! of frame N against the view-projection of frame N - 1 (`VC_MatrixViewProj`, texels 24..27, relative to `SC_CameraPos`,
//! texel 3) with its jitter swapped for frame N's (`FC_MatrixProj` texels 16 / 17 .z: ndc = ... + jitter). A few points in
//! front of the camera are projected both ways; the difference (pixels) is what his motion vectors would be off by if the
//! game's previous matrix were not the one he was drawn with. Logged once a second (`status`).
//!
//! THE OWN-RECORD ROUTE (`r.velocity_own`, default 1; needs the pack's skin_v88.vs.dxil / skin_cg_v88.vs.dxil): his position a
//! frame ago is moved by the camera's motion from the camera he was drawn with a frame ago to this frame's (`R`, our own record
//! of the KH3 camera the game draws with) and seen through THIS frame's game view-projection, so the camera's part of his
//! motion vectors no longer depends on the game's VC_PrevMatrixViewProj (the jitter cancels as before). The frame constant
//! `prev_host_from_game` carries R x (game -> host) with its w row set to 2 (the flag); the velocity vertex shader
//! (er_char.wgsl ER_VELO_OWN) uses it where our own view of this frame lands within 1.5 px of the game's (jitter taken off),
//! else the game's previous matrix. That velocity state tests the depth GREATER_EQUAL with a small bias (its vertex shader is
//! another compilation than the colour draw's: no bit-exact EQUAL).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

use glam::{Mat4, Vec3, Vec4};
use windows::Win32::Graphics::Direct3D12::*;

use crate::math::{self, Camera};

use crate::light::{ROW_SCENE, WIDTH};

fn texel(words: &[f32], row: usize, x: usize) -> Vec4 {
    let at = (row * WIDTH + x) * 4;
    words.get(at..at + 4).map_or(Vec4::ZERO, |w| Vec4::new(w[0], w[1], w[2], w[3]))
}

/// One frame's matrices from the game-data read-back.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    pub vp: [Vec4; 4],
    pub cam: Vec3,
    pub prev_vp: [Vec4; 4],
    pub prev_cam: Vec3,
    /// FC_MatrixProj rows 0 and 1, .z: the jitter in NDC.
    pub jitter: (f32, f32),
    /// FC_MatrixView rows (right, up, forward).
    pub axes: [Vec3; 3],
}

impl Frame {
    pub fn parse(words: &[f32]) -> Option<Frame> {
        let row = |k: usize| texel(words, ROW_SCENE, k);
        let f = Frame {
            vp: [row(24), row(25), row(26), row(27)],
            cam: row(3).truncate(),
            prev_vp: [row(29), row(30), row(31), row(32)],
            prev_cam: row(28).truncate(),
            jitter: (row(16).z, row(17).z),
            axes: [row(5).truncate(), row(6).truncate(), row(7).truncate()],
        };
        let finite = f.vp.iter().chain(f.prev_vp.iter()).all(|r| r.is_finite()) && f.cam.is_finite() && f.prev_cam.is_finite();
        (finite && f.vp[3].length_squared() > 0.0 && f.prev_vp[3].length_squared() > 0.0).then_some(f)
    }
}

fn project(m: &[Vec4; 4], rel: Vec3) -> Option<(f32, f32)> {
    let p = rel.extend(1.0);
    let w = m[3].dot(p);
    (w > 1.0e-4).then(|| (m[0].dot(p) / w, m[1].dot(p) / w))
}

/// Points in front of the camera of `now` (host space): along its forward axis (either sign, whichever projects in front)
/// 3 m and 12 m away, and 3 m away offset right / up.
fn probes(now: &Frame) -> Vec<Vec3> {
    let [right, up, fwd] = now.axes;
    let fwd = if project(&now.vp, fwd * 3.0).is_some() { fwd } else { -fwd };
    [fwd * 3.0, fwd * 12.0, fwd * 3.0 + right * 1.0, fwd * 3.0 - right * 1.0 + up * 0.6, fwd * 3.0 - up * 0.8].iter().map(|d| now.cam + *d).collect()
}

/// (pixels) how far the game's previous view-projection of `now` puts the probe points from where the frame `before` drew them
/// with `now`'s jitter (`rejit`) / with its own (`plain`), and how far the camera itself moved them (`camera`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pair {
    pub rejit: f32,
    pub plain: f32,
    pub camera: f32,
}

pub fn compare(before: &Frame, now: &Frame, size: (f32, f32)) -> Option<Pair> {
    let mut out = Pair::default();
    let mut n = 0;
    for p in probes(now) {
        let (Some(game), Some(old), Some(cur)) = (project(&now.prev_vp, p - now.prev_cam), project(&before.vp, p - before.cam), project(&now.vp, p - now.cam)) else { continue };
        let px = |a: (f32, f32), b: (f32, f32)| (((a.0 - b.0) * 0.5 * size.0).powi(2) + ((a.1 - b.1) * 0.5 * size.1).powi(2)).sqrt();
        let rejit = (old.0 - before.jitter.0 + now.jitter.0, old.1 - before.jitter.1 + now.jitter.1);
        out.rejit = out.rejit.max(px(game, rejit));
        out.plain = out.plain.max(px(game, old));
        out.camera = out.camera.max(px((cur.0 - now.jitter.0, cur.1 - now.jitter.1), (old.0 - before.jitter.0, old.1 - before.jitter.1)));
        n += 1;
    }
    (n > 0).then_some(out)
}

#[derive(Default)]
struct State {
    last: Option<(u64, Frame)>,
    pairs: u32,
    gaps: u32,
    sum: Pair,
    worst: Pair,
    over_half: u32,
    own_sum: f32,
    own_worst: f32,
    own_n: u32,
    own_over: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// gfx.rs: the game-data read-back of the frame drawn as `frame` (the renderer's count; consecutive frames differ by 1).
pub fn check(words: &[f32], size: (f32, f32), frame: u64) {
    let Some(now) = Frame::parse(words) else { return };
    let own = own_clip(frame).and_then(|m| own_vs_game(&m, &now, size));
    let Ok(mut guard) = STATE.lock() else { return };
    let s = guard.get_or_insert_with(State::default);
    if let Some(d) = own {
        (s.own_sum, s.own_worst, s.own_n) = (s.own_sum + d, s.own_worst.max(d), s.own_n + 1);
        s.own_over += u32::from(d >= 1.5);
    }
    match s.last {
        Some((f, before)) if f + 1 == frame => {
            if let Some(p) = compare(&before, &now, size) {
                s.pairs += 1;
                s.sum = Pair { rejit: s.sum.rejit + p.rejit, plain: s.sum.plain + p.plain, camera: s.sum.camera + p.camera };
                s.worst = Pair { rejit: s.worst.rejit.max(p.rejit), plain: s.worst.plain.max(p.plain), camera: s.worst.camera.max(p.camera) };
                s.over_half += u32::from(p.rejit > 0.5);
            }
        }
        Some(_) => s.gaps += 1,
        None => {}
    }
    s.last = Some((frame, now));
}

/// One line for the log (resets the counters).
pub fn status() -> String {
    let stencil = crate::gfx::taa_status();
    let states = own(|o| o.states.len()).unwrap_or(0);
    let route = format!(
        "his motion vectors' camera part: {} frame(s) from our own record, {} from the game's previous matrix ({}, the pack's v88 skin {}, {} own-record velocity state(s))",
        OWN_FRAMES.swap(0, Ordering::Relaxed),
        GAME_FRAMES.swap(0, Ordering::Relaxed),
        if OWN_WANTED.load(Ordering::Relaxed) { "r.velocity_own 1" } else { "r.velocity_own 0" },
        if own(|o| o.vs.is_some()).unwrap_or(false) { "read" } else { "missing" },
        states
    );
    let Ok(mut guard) = STATE.lock() else { return format!("(ER88) TAA: {stencil}; {route}") };
    let Some(s) = guard.as_mut() else { return format!("(ER88) TAA: {stencil}; {route}; no game-data read-back yet") };
    let n = s.pairs.max(1) as f32;
    let line = format!(
        "(ER88) TAA: {stencil}; {route}; our view vs the game's: mean {:.3} px, worst {:.3} px, {} of {} frame(s) at 1.5 px or more (there the game's matrix is used); the game's previous view-projection vs the frame before's: mean {:.3} px, worst {:.3} px, {} of {} frame pair(s) over 0.5 px (with the old jitter kept: mean {:.3} px; the camera moved the points {:.2} px mean, {:.2} worst; {} gap(s))",
        s.own_sum / s.own_n.max(1) as f32,
        s.own_worst,
        s.own_over,
        s.own_n,
        s.sum.rejit / n,
        s.worst.rejit,
        s.over_half,
        s.pairs,
        s.sum.plain / n,
        s.sum.camera / n,
        s.worst.camera,
        s.gaps
    );
    (s.pairs, s.gaps, s.sum, s.worst, s.over_half) = (0, 0, Pair::default(), Pair::default(), 0);
    (s.own_sum, s.own_worst, s.own_n, s.own_over) = (0.0, 0.0, 0, 0);
    line
}

// ---------------------------------------------------------------------------- the own-record route

/// `r.velocity_own` (default on).
static OWN_WANTED: AtomicBool = AtomicBool::new(true);
static OWN_FRAMES: AtomicU32 = AtomicU32::new(0);
static GAME_FRAMES: AtomicU32 = AtomicU32::new(0);

pub fn set_velocity_own(on: bool) {
    OWN_WANTED.store(on, Ordering::Relaxed);
}

struct Own {
    /// The velocity vertex shader of the current set (cg = the outfit skin with its leg guard), from the pack.
    vs: Option<(bool, Vec<u8>)>,
    /// (pipeline key bits, cg) -> the velocity state with that vertex shader.
    states: HashMap<(u64, bool), ID3D12PipelineState>,
    told: bool,
    /// The camera he was drawn with (renderer frame number), and the origin of that frame.
    last: Option<(u64, Camera, Vec3)>,
    /// Per renderer frame: our own clip-from-HOST matrix (for the check against the game's of the same frame).
    clips: Vec<(u64, Mat4)>,
}

static OWN: Mutex<Option<Own>> = Mutex::new(None);

fn own<R>(f: impl FnOnce(&mut Own) -> R) -> Option<R> {
    let mut guard = OWN.lock().ok()?;
    Some(f(guard.get_or_insert_with(|| Own { vs: None, states: HashMap::new(), told: false, last: None, clips: Vec::new() })))
}

/// gfx.rs read_shaders: the pack folder of the set being read and whether its skin is the outfit's (skin_cg).
pub fn load_velocity_vs(dir: &std::path::Path, cg: bool) {
    let file = dir.join(if cg { "skin_cg_v88.vs.dxil" } else { "skin_v88.vs.dxil" });
    let bytes = std::fs::read(&file).ok();
    own(|o| {
        if bytes.is_none() && !o.told {
            crate::rlog!("render: (ER88) {} is not in the pack: his motion vectors take the camera's part from the game's previous view-projection (as before)", file.display());
            o.told = true;
        }
        o.vs = bytes.map(|b| (cg, b));
    });
}

/// gfx.rs pipelines(): right after the velocity state is made from `desc`, the same with the own-record vertex shader and
/// the depth tested GREATER_EQUAL with a small bias toward the camera (reversed Z). Not fatal.
/// # Safety
/// `desc` must be a valid description (its pointers alive for the call).
pub unsafe fn make_velocity(device: &ID3D12Device, desc: &D3D12_GRAPHICS_PIPELINE_STATE_DESC, key: u64) {
    let Some(Some((cg, vs))) = own(|o| o.vs.clone()) else { return };
    // (a bitwise copy: the root signature pointer in it stays the caller's, never released here)
    let mut d: std::mem::ManuallyDrop<D3D12_GRAPHICS_PIPELINE_STATE_DESC> = std::mem::ManuallyDrop::new(std::ptr::read(desc));
    d.VS = D3D12_SHADER_BYTECODE { pShaderBytecode: vs.as_ptr() as *const std::ffi::c_void, BytecodeLength: vs.len() };
    own_route_depth(&mut d);
    match device.CreateGraphicsPipelineState::<ID3D12PipelineState>(&*d) {
        Ok(state) => {
            own(|o| o.states.insert((key, cg), state));
        }
        Err(e) => crate::rlog!("render: (ER88) the own-record velocity state failed ({e}): the game's previous view-projection is used"),
    }
}

/// (RSF) The own-record velocity state's depth test: GREATER_EQUAL with a small bias toward the camera (reversed Z), its vertex
/// shader being another compilation than the colour draw's (no bit-exact EQUAL). Sora's states (make_velocity) and the party's
/// (gfx.rs `velocity_own_pipeline`) alike.
pub(crate) fn own_route_depth(d: &mut D3D12_GRAPHICS_PIPELINE_STATE_DESC) {
    d.DepthStencilState.DepthFunc = D3D12_COMPARISON_FUNC_GREATER_EQUAL;
    d.RasterizerState.DepthBias = 64;
    d.RasterizerState.SlopeScaledDepthBias = 2.0;
    d.RasterizerState.DepthBiasClamp = 0.0;
}

/// (RSF) `r.velocity_own`: the party's velocity draws take their own-record states only while it is on (as Sora's).
#[cfg_attr(not(feature = "party"), allow(dead_code))]
pub(crate) fn own_wanted() -> bool {
    OWN_WANTED.load(Ordering::Relaxed)
}

/// The own-record velocity state for this pipeline key and the current set, when the route is on and the state is made.
pub fn velocity_state(key: u64) -> Option<ID3D12PipelineState> {
    if !OWN_WANTED.load(Ordering::Relaxed) {
        return None;
    }
    own(|o| o.vs.as_ref().and_then(|(cg, _)| o.states.get(&(key, *cg)).cloned())).flatten()
}

/// gfx.rs, the frame constants: `prev_host_from_game` for this frame. `plain` = the game -> host of the frame before (as
/// before ER88); `own_route` = the own-record velocity state will draw this frame. Returns R x (game -> host) with the flag
/// (w row 2) when the camera of the frame before is known (consecutive frame, same origin), else `plain`. Also records this
/// frame's camera and our own clip-from-host (for the check).
pub fn prev_host(plain: Mat4, camera: &Camera, origin: Vec3, frame: u64, own_route: bool) -> Mat4 {
    let clip_from_host = math::clip_from_view(camera.fov, camera.aspect, camera.near, camera.far) * math::view_from_host(camera);
    own(|o| {
        let out = match o.last {
            Some((f, before, at)) if own_route && f + 1 == frame && at == origin && camera.position.is_finite() && before.position.is_finite() => {
                OWN_FRAMES.fetch_add(1, Ordering::Relaxed);
                own_matrix(camera, &before, origin)
            }
            _ => {
                GAME_FRAMES.fetch_add(1, Ordering::Relaxed);
                plain
            }
        };
        o.last = Some((frame, *camera, origin));
        o.clips.push((frame, clip_from_host));
        if o.clips.len() > 8 {
            o.clips.remove(0);
        }
        out
    })
    .unwrap_or(plain)
}

/// The own-record `prev_host_from_game`: R x (game -> host) with its w row 2 (the flag), R = view(now)^-1 x view(a frame ago).
pub(crate) fn own_matrix(now: &Camera, before: &Camera, origin: Vec3) -> Mat4 {
    let r = math::view_from_host(now).inverse() * math::view_from_host(before);
    let mut m = r * math::host_from_game(origin);
    m.w_axis.w = 2.0;
    m
}

/// (for the check) our own clip-from-host of a renderer frame.
fn own_clip(frame: u64) -> Option<Mat4> {
    own(|o| o.clips.iter().find(|(f, _)| *f == frame).map(|(_, m)| *m)).flatten()
}

/// (pixels) how far our own view of a frame puts the probe points from the game's view-projection of it, jitter taken off.
pub fn own_vs_game(own: &Mat4, now: &Frame, size: (f32, f32)) -> Option<f32> {
    let mut worst: Option<f32> = None;
    for p in probes(now) {
        let Some(game) = project(&now.vp, p - now.cam) else { continue };
        let c = *own * p.extend(1.0);
        if c.w <= 1.0e-4 {
            continue;
        }
        let d = (((c.x / c.w - (game.0 - now.jitter.0)) * 0.5 * size.0).powi(2) + ((c.y / c.w - (game.1 - now.jitter.1)) * 0.5 * size.1).powi(2)).sqrt();
        worst = Some(worst.map_or(d, |w: f32| w.max(d)));
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat4;

    /// A frame of a camera at `cam` looking along +x (host space), rows as the game stores them (clip = rows . rel), with
    /// `jitter` in NDC, and a previous matrix of `prev` (its camera, rotation about y, CURRENT jitter).
    fn frame(cam: Vec3, yaw: f32, jitter: (f32, f32), prev: Option<(Vec3, f32)>) -> Frame {
        let vp = |yaw: f32| {
            let view = Mat4::from_rotation_y(yaw);
            let mut proj = Mat4::perspective_lh(1.0, 16.0 / 9.0, 0.05, 1000.0);
            proj.z_axis.x += jitter.0;
            proj.z_axis.y += jitter.1;
            let m = proj * view;
            [m.row(0), m.row(1), m.row(2), m.row(3)]
        };
        let (pc, py) = prev.unwrap_or((cam, yaw));
        let view = Mat4::from_rotation_y(yaw);
        let axes = [view.row(0).truncate(), view.row(1).truncate(), view.row(2).truncate()];
        Frame { vp: vp(yaw), cam, prev_vp: vp(py), prev_cam: pc, jitter, axes }
    }

    #[test]
    fn a_previous_matrix_of_the_frame_before_reads_zero_and_a_wrong_one_does_not() {
        let size = (2560.0, 1440.0);
        let before = frame(Vec3::new(0.0, 1.5, 0.0), 0.10, (0.0002, -0.0003), None);
        // right: the game's previous = the frame before's, re-jittered with this frame's jitter
        let now = frame(Vec3::new(0.05, 1.5, 0.0), 0.12, (-0.0001, 0.0002), Some((Vec3::new(0.0, 1.5, 0.0), 0.10)));
        let p = compare(&before, &now, size).unwrap();
        assert!(p.rejit < 0.01, "{p:?}");
        assert!(p.plain > 0.1, "the jitter difference shows when it is not swapped: {p:?}");
        assert!(p.camera > 10.0, "the camera turned: {p:?}");
        // wrong: the game's previous is a view a quarter of a degree off
        let off = frame(Vec3::new(0.05, 1.5, 0.0), 0.12, (-0.0001, 0.0002), Some((Vec3::new(0.0, 1.5, 0.0), 0.10 + 0.25f32.to_radians())));
        let q = compare(&before, &off, size).unwrap();
        assert!(q.rejit > 3.0, "{q:?}");
    }

    #[test]
    fn the_own_record_moves_his_old_position_into_where_the_camera_now_sees_it() {
        let cam = |pos: Vec3, yaw: f32| {
            let (s, c) = yaw.sin_cos();
            Camera { right: Vec3::new(c, 0.0, -s), up: Vec3::Y, forward: Vec3::new(s, 0.0, c), position: pos, fov: 0.8, aspect: 16.0 / 9.0, near: 0.05, far: 1.0e4 }
        };
        let origin = Vec3::new(3.0, 0.0, -2.0);
        let (a, b) = (cam(Vec3::new(0.0, 1.6, -3.0), 0.2), cam(Vec3::new(0.3, 1.6, -2.8), 0.35));
        let plain = math::host_from_game(origin);
        set_velocity_own(true);
        // frame 100: no record yet -> plain; frame 101: the record -> R x plain, flagged
        assert_eq!(prev_host(plain, &a, origin, 100, true), plain);
        let m = prev_host(plain, &b, origin, 101, true);
        assert_eq!(m.w_axis.w, 2.0);
        // a game point (cm) a frame ago, through the moved matrix and THIS camera = through the OLD camera
        let game = Vec3::new(120.0, -40.0, 90.0);
        let clip = |c: &Camera, h: Vec3| {
            let v = math::clip_from_view(c.fov, c.aspect, c.near, c.far) * math::view_from_host(c) * h.extend(1.0);
            (v.x / v.w, v.y / v.w)
        };
        let h = m * game.extend(1.0);
        assert_eq!(h.w, 2.0, "the flag rides in w");
        let (now, then) = (clip(&b, h.truncate()), clip(&a, plain.transform_point3(game)));
        assert!((now.0 - then.0).abs() < 1.0e-5 && (now.1 - then.1).abs() < 1.0e-5, "{now:?} {then:?}");
        // the route off for a frame (its state not drawn) or a gap: plain
        assert_eq!(prev_host(plain, &a, origin, 102, false), plain);
        assert_eq!(prev_host(plain, &b, origin, 104, true), plain);
        // the check of our own view against the game's: the same camera reads ~0
        let vp = math::clip_from_view(b.fov, b.aspect, b.near, b.far) * math::view_from_host(&b) * Mat4::from_translation(b.position);
        let f = Frame { vp: [vp.row(0), vp.row(1), vp.row(2), vp.row(3)], cam: b.position, prev_vp: [Vec4::ZERO; 4], prev_cam: b.position, jitter: (0.0, 0.0), axes: [b.right, b.up, b.forward] };
        let own = math::clip_from_view(b.fov, b.aspect, b.near, b.far) * math::view_from_host(&b);
        assert!(own_vs_game(&own, &f, (2560.0, 1440.0)).unwrap() < 0.01);
    }

    #[test]
    fn the_read_back_rows_are_parsed() {
        let mut words = vec![0.0f32; WIDTH * crate::light::HEIGHT * 4];
        let mut put = |x: usize, v: [f32; 4]| words[(ROW_SCENE * WIDTH + x) * 4..(ROW_SCENE * WIDTH + x) * 4 + 4].copy_from_slice(&v);
        put(3, [1.0, 2.0, 3.0, 0.0]);
        put(16, [1.26, 0.0, 0.0002, 0.0]);
        put(17, [0.0, 2.24, -0.0003, 0.0]);
        put(27, [0.0, 0.0, 1.0, 0.0]);
        put(32, [0.0, 0.0, 1.0, 0.0]);
        let f = Frame::parse(&words).unwrap();
        assert_eq!((f.cam, f.jitter), (Vec3::new(1.0, 2.0, 3.0), (0.0002, -0.0003)));
    }
}
