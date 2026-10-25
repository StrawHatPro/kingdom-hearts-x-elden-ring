//! (NF) KH3'S NEAR-CAMERA / SEE-THROUGH FADE OF THE CHARACTERS (TresCameraManager's "shut-out" fade), engine-free: which
//! character fades, to what, how fast, and the screen-door both renderers draw it with. Read from the game
//! (build/sora_tmp/party/IMPL_NF_STATE.md section 1; the rows: sheets/port/sora-camera-fade.json and
//! sora-camera-fade-volumes.json):
//!
//! - the camera's collision pass (FUN_1405D87F0) overlaps a 1 cm ball at the camera and sweeps it from the look point to the
//!   camera on the Camera channel; every camera volume it touches (a TresBodyCollPrimitive of the character's blueprint with
//!   m_bEnableCamera, m_OverlapCameraFunction FADE) registers its owner with the manager (FUN_1405C9660 -> FUN_1405EFBE0),
//!   the player's pawn excepted;
//! - the owner's CameraFade runs linearly to his m_CameraFadeAlpha (0.5) at 0.3 s per unit, and back to 1 at the same pace
//!   as soon as no volume of his is touched (FUN_1405FBFF0); his children / attached actors (the weapon) take the same value;
//! - the value is the CameraFade scalar of every material of his meshes; the masked materials' mask shader turns it into a
//!   DitherTemporalAA screen door: a pixel stays where frac((x + 2 y + phase) / 6) x 6/7 + CameraFade - 5/6 >= 0.
//!
//! The port adds (rows camfade.lead_overlap / camfade.limb_guard): the LEADER fades while the camera is inside him (any
//! character leads here; KH3's player, Sora, never has the camera inside him), and every character also has an
//! overlap-only volume along each limb and his trunk (the camera inside any part of him fades him). No character is named
//! in this code: the volumes are rows per character id, and a character without rows gets the limb volumes.

use glam::{Quat, Vec3};

include!(env!("KH_SORA_CAMERA_FADE_RS"));
include!(env!("KH_SORA_CAMERA_FADE_VOLUMES_RS"));

/// The rows of sheets/port/sora-camera-fade.json.
pub const P: SoraCameraFadeParams = SORA_CAMERA_FADE_PARAMS;

/// A capsule in game space (cm): the segment `a`..`b` and the radius (a ball: `a == b`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub a: Vec3,
    pub b: Vec3,
    pub r: f32,
}

/// Where a volume comes from: KH3's camera volumes (the overlap at the camera and the look-line sweep) or the port's limb
/// guard (the overlap at the camera only).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Camera,
    Limb,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    pub shape: Shape,
    pub kind: Kind,
}

/// A bone in game space: its place, its rotation and its scale (UE's socket transform).
pub type BoneAt = (Vec3, Quat, f32);

/// UE's FRotator (pitch, yaw, roll; degrees) as a rotation in the game's axes (X forward, Y right, Z up): yaw about Z,
/// then pitch nose up, then roll (kh-partyhost poser::ue_rotator, the same composition as FRotator::Quaternion).
pub fn ue_rotator(pitch: f32, yaw: f32, roll: f32) -> Quat {
    Quat::from_rotation_z(yaw.to_radians()) * Quat::from_rotation_y(-pitch.to_radians()) * Quat::from_rotation_x(-roll.to_radians())
}

/// The camera volume rows of a character (UE names: compared without case).
pub fn rows_of(chara: &str) -> impl Iterator<Item = &'static CamFadeVolumeRow> + '_ {
    CAMERA_FADE_VOLUMES.iter().filter(move |r| r.chara.eq_ignore_ascii_case(chara) && r.function.eq_ignore_ascii_case("FADE"))
}

/// One row placed on its bone: the shape's centre at the bone's transform of RelativeLocation, the capsule's axis = the
/// bone's rotation x RelativeRotation x local Z, its segment half = half height - radius (FKSphylElem Length = 2 (Z - X),
/// 0x140649590), everything x the bone's scale. `None` for a shape that is neither a sphere nor a capsule.
pub fn place_row(row: &CamFadeVolumeRow, bone: BoneAt) -> Option<Shape> {
    let (at, rotation, scale) = bone;
    let v = |s: &[f32]| Vec3::new(s.first().copied().unwrap_or(0.0), s.get(1).copied().unwrap_or(0.0), s.get(2).copied().unwrap_or(0.0));
    let centre = at + rotation * (v(row.offset) * scale);
    let r = row.radius.max(1.0) * scale;
    if row.shape.eq_ignore_ascii_case("SPHERE") {
        return Some(Shape { a: centre, b: centre, r });
    }
    if !row.shape.eq_ignore_ascii_case("CAPSULE") {
        return None;
    }
    let rot = v(row.rotation);
    let axis = rotation * ue_rotator(rot.x, rot.y, rot.z) * Vec3::Z;
    let half = (row.half_height.max(1.0) - row.radius.max(1.0)).max(0.0) * scale;
    Some(Shape { a: centre - axis * half, b: centre + axis * half, r })
}

/// The limb guard's bones (row camfade.limb_bones: FROM:TO:RADIUS, ...).
pub fn limb_bones() -> Vec<(&'static str, &'static str, f32)> {
    P.camfade_limb_bones
        .split(',')
        .filter_map(|part| {
            let mut it = part.trim().split(':');
            let (from, to, r) = (it.next()?, it.next()?, it.next()?.trim().parse::<f32>().ok()?);
            Some((from.trim(), to.trim(), r))
        })
        .collect()
}

/// EVERY VOLUME OF A CHARACTER this frame: his KH3 camera volumes (rows of `chara`, on his bones), then the port's limb
/// guard along his bones (radius + camfade.limb_margin); `bone` looks a bone up by name (game space), `root` is his mesh
/// component's place (a character with neither: one upright capsule there).
pub fn volumes(chara: &str, bone: &dyn Fn(&str) -> Option<BoneAt>, root: Vec3) -> Vec<Volume> {
    let mut out: Vec<Volume> = rows_of(chara).filter_map(|row| place_row(row, bone(row.bone)?)).map(|shape| Volume { shape, kind: Kind::Camera }).collect();
    if P.camfade_limb_guard {
        for (from, to, r) in limb_bones() {
            if let (Some(a), Some(b)) = (bone(from), bone(to)) {
                out.push(Volume { shape: Shape { a: a.0, b: b.0, r: r * a.2 + P.camfade_limb_margin }, kind: Kind::Limb });
            }
        }
        if out.is_empty() {
            out.push(Volume { shape: Shape { a: root + Vec3::Z * 40.0, b: root + Vec3::Z * 120.0, r: 40.0 + P.camfade_limb_margin }, kind: Kind::Limb });
        }
    }
    out
}

/// The squared distance of a point from a segment.
fn point_segment2(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 1.0e-12 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t - p).length_squared()
}

/// The squared distance between two segments (the closest points, clamped to both).
pub fn segment_segment2(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> f32 {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.length_squared(), d2.length_squared(), d2.dot(r));
    if a <= 1.0e-12 && e <= 1.0e-12 {
        return r.length_squared();
    }
    if a <= 1.0e-12 {
        return point_segment2(p1, p2, q2);
    }
    if e <= 1.0e-12 {
        return point_segment2(p2, p1, q1);
    }
    let c = d1.dot(r);
    let b = d1.dot(d2);
    let denom = a * e - b * b;
    let mut s = if denom > 1.0e-9 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
    let mut t = (b * s + f) / e;
    if t < 0.0 {
        t = 0.0;
        s = (-c / a).clamp(0.0, 1.0);
    } else if t > 1.0 {
        t = 1.0;
        s = ((b - c) / a).clamp(0.0, 1.0);
    }
    ((p1 + d1 * s) - (p2 + d2 * t)).length_squared()
}

/// Why a character fades this frame (for the log and the tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Touch {
    /// The camera ball is inside one of his KH3 camera volumes.
    Inside,
    /// The look line (look point -> camera) passes through one of his KH3 camera volumes.
    Between,
    /// The camera is inside one of his limb volumes (the port's guard).
    Limb,
}

/// DOES THE CAMERA TOUCH HIM (FUN_1405D87F0's two queries on his volumes): the ball of camfade.probe_radius at the camera
/// inside a volume, or swept from the look point to the camera through a KH3 camera volume. `lead`: the character the
/// player controls (KH3 leaves him out, FUN_1405EFBE0; the port keeps the camera-inside test for him, camfade.lead_overlap).
pub fn touches(camera: Vec3, look: Option<Vec3>, lead: bool, volumes: &[Volume]) -> Option<Touch> {
    let r0 = P.camfade_probe_radius;
    let inside_ok = !lead || !P.camfade_player_excluded || P.camfade_lead_overlap;
    let sweep_ok = P.camfade_sweep && (!lead || !P.camfade_player_excluded);
    let mut found: Option<Touch> = None;
    for v in volumes {
        let reach = v.shape.r + r0;
        if inside_ok && point_segment2(camera, v.shape.a, v.shape.b) <= reach * reach {
            match v.kind {
                Kind::Camera => return Some(Touch::Inside),
                Kind::Limb => found = found.or(Some(Touch::Limb)),
            }
        }
        if v.kind == Kind::Camera && sweep_ok {
            if let Some(look) = look {
                if segment_segment2(look, camera, v.shape.a, v.shape.b) <= reach * reach {
                    found = Some(Touch::Between);
                }
            }
        }
    }
    found
}

/// One character the camera may fade this frame.
pub struct Subject<'a> {
    /// The host's id of him (stable from frame to frame).
    pub key: u32,
    /// The player controls him (the camera's leader).
    pub lead: bool,
    pub volumes: &'a [Volume],
}

/// THE FADE LIST (TresCameraManager+0x28A0): per character his CameraFade value (1 = whole).
#[derive(Clone, Debug, Default)]
pub struct CamFade {
    entries: Vec<(u32, f32, Option<Touch>)>,
}

impl CamFade {
    /// One step of `dt` seconds: every subject touched runs toward camfade.alpha, every other one back toward 1, linearly
    /// at camfade.seconds_per_unit (FUN_1411940B0 / FUN_1405FBFF0); a character no longer submitted leaves the list (KH3
    /// drops an entry whose actor is gone). With camfade.on false every value is 1.
    pub fn step(&mut self, dt: f32, camera: Vec3, look: Option<Vec3>, subjects: &[Subject]) {
        let rate = if P.camfade_seconds_per_unit > 0.0 { dt.max(0.0) / P.camfade_seconds_per_unit } else { f32::INFINITY };
        let mut next: Vec<(u32, f32, Option<Touch>)> = Vec::with_capacity(subjects.len());
        for s in subjects {
            let before = self.entries.iter().find(|e| e.0 == s.key).map_or(1.0, |e| e.1);
            let touch = if P.camfade_on { touches(camera, look, s.lead, s.volumes) } else { None };
            let target = if touch.is_some() { P.camfade_alpha.clamp(0.0, 1.0) } else { 1.0 };
            let value = if (target - before).abs() <= rate { target } else { before + (target - before).signum() * rate };
            next.push((s.key, value, touch));
        }
        self.entries = next;
    }

    /// His CameraFade value (1 = whole; a character not in the list: 1).
    pub fn value(&self, key: u32) -> f32 {
        self.entries.iter().find(|e| e.0 == key).map_or(1.0, |e| e.1)
    }

    /// 1 - value: what the renderers carry (0 = no fade, so a block that does not set it is whole).
    pub fn amount(&self, key: u32) -> f32 {
        1.0 - self.value(key)
    }

    /// Why he was touched in the latest step.
    pub fn touch(&self, key: u32) -> Option<Touch> {
        self.entries.iter().find(|e| e.0 == key).and_then(|e| e.2)
    }

    /// (key, value, touch) of every character in the list.
    pub fn entries(&self) -> &[(u32, f32, Option<Touch>)] {
        &self.entries
    }
}

/// KH3's mask test at one of its six phases (materials.wgsl kh_mask, first test): frac(d) x 6/7 + value - 5/6 >= 0.
pub fn kh3_phase_keeps(d: u32, value: f32) -> bool {
    let n = P.camfade_dither_phases.max(1) as u32;
    ((d % n) as f32 / n as f32) * P.camfade_dither_scale + value - P.camfade_dither_bias >= 0.0
}

/// KH3's SHARE of the pixels a CameraFade value keeps (the six phases' average, what KH3's TAA shows: 0.5 -> 3 / 6).
pub fn coverage(value: f32) -> f32 {
    let n = P.camfade_dither_phases.max(1) as u32;
    (0..n).filter(|k| kh3_phase_keeps(*k, value)).count() as f32 / n as f32
}

/// The 4 x 4 ordered (Bayer) matrix: 4 M(x, y) + M(x / 2, y / 2), M = ((x ^ y) & 1) x 2 + (y & 1) (camfade.screen_door).
pub fn bayer4(x: u32, y: u32) -> u32 {
    let m = |x: u32, y: u32| ((x ^ y) & 1) * 2 + (y & 1);
    4 * m(x & 1, y & 1) + m((x >> 1) & 1, (y >> 1) & 1)
}

/// THE SCREEN DOOR, as the shaders draw it (er_char.wgsl `er_camfade_keep`, char_common.wgsl `kh_camfade_keep`): the pixel
/// (x, y) of the character stays where (bayer4(x + phase, y + 3 phase) + 0.5) / 16 < KH3's share of the value.
pub fn keeps(x: u32, y: u32, phase: u32, value: f32) -> bool {
    if value >= 1.0 {
        return true;
    }
    (bayer4(x.wrapping_add(phase), y.wrapping_add(3 * phase)) as f32 + 0.5) / 16.0 < coverage(value)
}

/// The phase of the screen door in frame `frame`: 0 (a still door, camfade.temporal off); stepping once a frame through
/// camfade.phase_cycle with camfade.temporal on and a temporal anti-aliasing to resolve it.
pub fn phase(frame: u64, temporal: bool) -> u32 {
    if temporal && P.camfade_temporal {
        (frame % P.camfade_phase_cycle.max(1) as u64) as u32
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bones_upright(at: Vec3) -> impl Fn(&str) -> Option<BoneAt> {
        // a character standing at `at`: the KH3 spine bones point up their X axis (a bone frame turned so X = up)
        let up = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
        move |name: &str| {
            let h = match name.to_ascii_lowercase().as_str() {
                "center" => 95.0,
                "hara1" | "hara2" => 100.0,
                "koshi" => 90.0,
                "mune" => 120.0,
                "kubi" => 145.0,
                "atama" => 155.0,
                "r_te" => 100.0,
                _ => return None,
            };
            let side = if name.eq_ignore_ascii_case("R_te") { Vec3::Y * 45.0 } else { Vec3::ZERO };
            Some((at + Vec3::Z * h + side, up, 1.0))
        }
    }

    #[test]
    fn nf_kh3_numbers_are_the_games() {
        assert_eq!(P.camfade_alpha, 0.5);
        assert_eq!(P.camfade_seconds_per_unit, 0.3);
        assert_eq!(P.camfade_probe_radius, 1.0);
        assert!(P.camfade_on && P.camfade_sweep && P.camfade_player_excluded && P.camfade_lead_overlap && P.camfade_limb_guard);
        // every party character has KH3 camera volumes; Goofy's are a capsule r 50 / half height 100 on hara2 and a ball r 35 on R_te
        assert_eq!(rows_of("p_ex001").count(), 8);
        assert_eq!(rows_of("n_ex001").count(), 1);
        let g: Vec<_> = rows_of("N_EX002").collect();
        assert_eq!(g.len(), 2);
        assert!(g.iter().any(|r| r.bone == "hara2" && r.shape == "CAPSULE" && r.radius == 50.0 && r.half_height == 100.0));
        assert!(g.iter().any(|r| r.bone == "R_te" && r.shape == "SPHERE" && r.radius == 35.0));
    }

    #[test]
    fn nf_ue_rotator_matches_frotator_quaternion() {
        // FRotator::Quaternion (UE 4.17) on raw components
        let ue = |p: f32, y: f32, r: f32| {
            let (sp, cp) = (p.to_radians() * 0.5).sin_cos();
            let (sy, cy) = (y.to_radians() * 0.5).sin_cos();
            let (sr, cr) = (r.to_radians() * 0.5).sin_cos();
            Quat::from_xyzw(cr * sp * sy - sr * cp * cy, -cr * sp * cy - sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy)
        };
        for (p, y, r) in [(90.0, 0.0, -15.0), (0.0, 0.0, 90.0), (30.0, 45.0, 60.0), (-20.0, 170.0, 5.0)] {
            let (a, b) = (ue_rotator(p, y, r), ue(p, y, r));
            assert!(a.dot(b).abs() > 0.9999, "{p} {y} {r}: {a:?} vs {b:?}");
        }
        // pitch 90 turns the capsule's axis (local Z) to -X
        assert!((ue_rotator(90.0, 0.0, 0.0) * Vec3::Z - Vec3::NEG_X).length() < 1.0e-5);
    }

    #[test]
    fn nf_capsule_is_placed_with_the_radius_inside_the_half_height() {
        let row = rows_of("n_ex002").find(|r| r.bone == "hara2").unwrap();
        let s = place_row(row, (Vec3::new(0.0, 0.0, 100.0), Quat::IDENTITY, 1.0)).unwrap();
        assert_eq!(s.r, 50.0);
        assert!(((s.b - s.a).length() - 100.0).abs() < 1.0e-3, "segment 2 (100 - 50)");
        assert!(((s.a + s.b) * 0.5 - Vec3::new(0.0, -5.0, 100.0)).length() < 1.0e-3);
    }

    #[test]
    fn nf_inside_between_and_clear() {
        let at = Vec3::new(500.0, 0.0, 0.0);
        let vols = volumes("n_ex002", &bones_upright(at), at);
        assert!(vols.iter().any(|v| v.kind == Kind::Camera) && vols.iter().any(|v| v.kind == Kind::Limb));
        // the camera in his belly: inside (friend or leader)
        assert_eq!(touches(at + Vec3::new(10.0, 0.0, 100.0), None, false, &vols), Some(Touch::Inside));
        assert_eq!(touches(at + Vec3::new(10.0, 0.0, 100.0), None, true, &vols), Some(Touch::Inside));
        // he stands between the camera and the look point: the see-through fade (a friend only)
        let look = Vec3::new(1000.0, 0.0, 100.0);
        let cam = Vec3::new(0.0, 0.0, 100.0);
        assert_eq!(touches(cam, Some(look), false, &vols), Some(Touch::Between));
        assert_eq!(touches(cam, Some(look), true, &vols), None, "the leader is never faded by the look line");
        // well clear
        assert_eq!(touches(at + Vec3::new(-300.0, 0.0, 100.0), Some(at + Vec3::new(-300.0, 300.0, 100.0)), false, &vols), None);
    }

    #[test]
    fn nf_tween_is_linear_at_0_3_s_per_unit_and_returns() {
        let at = Vec3::ZERO;
        let vols = volumes("n_ex001", &bones_upright(at), at);
        let mut f = CamFade::default();
        let cam_in = Vec3::new(0.0, 0.0, 95.0);
        let s = [Subject { key: 1, lead: false, volumes: &vols }];
        // 1 -> 0.5 in 0.15 s: after 0.075 s at 0.75
        f.step(0.075, cam_in, None, &s);
        assert!((f.value(1) - 0.75).abs() < 1.0e-5, "{}", f.value(1));
        f.step(0.075, cam_in, None, &s);
        assert!((f.value(1) - 0.5).abs() < 1.0e-5);
        f.step(1.0, cam_in, None, &s);
        assert_eq!(f.value(1), 0.5);
        // away: back to 1 at the same pace, frame-rate independent (ten 1/120 s steps = one 1/12 s step)
        let cam_out = Vec3::new(-400.0, 0.0, 95.0);
        let mut g = f.clone();
        for _ in 0..10 {
            f.step(1.0 / 120.0, cam_out, None, &s);
        }
        g.step(10.0 / 120.0, cam_out, None, &s);
        assert!((f.value(1) - g.value(1)).abs() < 1.0e-5);
        assert!((f.value(1) - (0.5 + (10.0 / 120.0) / 0.3)).abs() < 1.0e-4);
        // a character no longer submitted leaves the list: whole
        f.step(0.016, cam_in, None, &[]);
        assert_eq!(f.value(1), 1.0);
    }

    #[test]
    fn nf_the_screen_door_keeps_half_the_pixels_at_0_5() {
        // KH3's mask: CameraFade 1 keeps everything, 0.5 three of six phases, 0 nothing
        assert_eq!(coverage(1.0), 1.0);
        assert!((coverage(0.5) - 0.5).abs() < 1.0e-6);
        assert_eq!(coverage(0.0), 0.0);
        let kept = (0..60u32).flat_map(|y| (0..60u32).map(move |x| (x, y))).filter(|(x, y)| keeps(*x, *y, 0, 0.5)).count();
        assert_eq!(kept, 1800);
        // at 0.5 a one-pixel checkerboard: no two kept pixels side by side
        for (x, y) in [(0u32, 0u32), (3, 7), (100, 51)] {
            assert_ne!(keeps(x, y, 0, 0.5), keeps(x + 1, y, 0, 0.5));
            assert_ne!(keeps(x, y, 0, 0.5), keeps(x, y + 1, 0, 0.5));
        }
        // the 16 cells are 0..15 once each; every share keeps exactly its count of cells
        let mut cells: Vec<u32> = (0..16).map(|k| bayer4(k % 4, k / 4)).collect();
        cells.sort();
        assert_eq!(cells, (0..16).collect::<Vec<_>>());
        for v in [0.9f32, 0.75, 0.6, 0.5] {
            let n = (0..16u32).filter(|k| keeps(k % 4, k / 4, 0, v)).count() as f32 / 16.0;
            assert!((n - coverage(v)).abs() <= 1.0 / 16.0, "{v}: {n} vs {}", coverage(v));
        }
        assert_eq!(phase(13, false), 0);
        assert_eq!(phase(13, true), if P.camfade_temporal { 1 } else { 0 });
    }
}
