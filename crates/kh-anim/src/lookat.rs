//! (party I2) The friends' head / spine LOOK-AT: `TresAnimNode_ExLookAt` (node #12 of n_ex00x_Anim) and the pawn /
//! anim-instance side that feeds it, read from the decompiled C. Data: `data/lookat.json` of a friend's pack
//! (tools/party_lookat_pack.js, from the AnimBlueprint's node and anim-instance CDO).
//!
//! WHO FEEDS IT (the game's chain, every frame):
//! - `ATresAIController_BT` 0x140527460 (NPC controller vtable 0x14684C4E8 slot 0xA48, the UpdateControlRotation slot):
//!   `GetFocalPoint` (0x14051BF40, stock UE: the highest focus priority whose actor is not the own pawn, else its
//!   point) valid -> pawn `SetLookAt(1, 0.5)` (vt+0x1108 0x1405BA850) + `SetLookAtLocation(point)` (vt+0x1118
//!   0x1405BA890), else `SetLookAt(0, 0.5)`. The focal point of an actor is `GetFocalPointOnActor` 0x141263BF0 =
//!   the actor's look-at target point (vt+0x6F0 0x1405BAAB0: its lock-on component's look-at bone, Sora
//!   `MyLookAtTargetBoneName` = "t_camera" [A p_ex001_PawnBase]). The trees set the focus with
//!   `TresBTService_GameplayFocus` (FollowPlayer: PlayerActor while standing, SelfActor = none while moving).
//! - The pawn's two `FUN_143f0aa50` interpolators (ticked by 0x1405B2190): +0x780 (value +0x788) = the focus one
//!   above, +0x768 (value +0x770) = the notify one: AI BeginPlay 0x14054B1C0 sets it to 1 / 1, every
//!   `TresAnimNotifyState_LookAt` begin 0x140563C90 / end 0x140565C90 calls the anim interface (+0x440) slot
//!   0x288 (on: 0x1405B4300, target 1) or 0x290 (off: 0x1405B30A0, target 0) with m_StartInterpolationTime /
//!   m_EndInterpolationTime; m_ReverseTrg swaps on / off (most of the friends' clips: off for the clip).
//! - `LookAtRate` = pawn vt+0x10C8 0x1405BA830 = +0x788 x +0x770; `LookAtLocation` = vt+0x1110 (+0x798) through the
//!   anim instance's delay ring of `m_LookAtLocationBufNum` frames (NativeUpdateAnimation 0x14058B450; refilled with
//!   the current point while the previous LookAtRate was 0).
//!
//! THE NODE (`EvaluateBoneTransforms` 0x1405384E0, init 0x140544FB0, update 0x140554A10, reset in 0x140536060):
//! the look direction from the base bone (+ offset) to the point in component space: yaw about +Z against the
//! forward +Y, pitch about +X after the yaw is removed; the back margin when the point is behind; clamped to the
//! limits (the rest = the eye rotation, node +0x124); turned toward at m_RotationSpeedDeg (no speed or a direct look:
//! at once); each rot bone turns by its coefficients (pitch about the component X axis, yaw about Z, applied in the
//! bone's frame: `local * (qYaw * qPitch)`), then follows its rot-bone parent's change; the results go out as
//! component-space transforms blended by the node alpha (`LocalBlendCSBoneTransforms`). Alpha < 1e-5 resets the
//! current / target / eye rotation to zero (the next look starts from the front).

use glam::{Quat, Vec3};

use crate::json::Json;
use crate::pack::Skeleton;
use crate::Trs;

const DEG: f32 = std::f32::consts::PI / 180.0;
/// The mesh component's axes the node measures against [C: DAT_1486d8670 / 8680 / 8690 = (0,1,0), (0,0,1), (1,0,0)].
const FORWARD: Vec3 = Vec3::Y;
const UP: Vec3 = Vec3::Z;
const SIDE: Vec3 = Vec3::X;

/// One `FTresLookAtRotBoneParam` (runtime entry of 0x80 bytes: +0 bone, +4 parent entry, +8 yaw, +0xC pitch, +0x10
/// ParamA coefficient).
#[derive(Clone, Debug, PartialEq)]
pub struct RotBone {
    pub bone: usize,
    pub yaw: f32,
    pub pitch: f32,
    pub param_a: f32,
    /// The nearest earlier rot bone that is an ancestor (InitializeBoneReferences 0x14054F080), whose change this one
    /// follows.
    pub parent: Option<usize>,
}

/// An eye-look blend space of the face anim set (NO_0_Eye / BA_00_Eye = n_ex00x_BS_id / _Bid): its 2D grid.
#[derive(Clone, Debug, PartialEq)]
pub struct EyeSpace {
    pub row: String,
    /// (min, max, grid_num) of X (eye yaw) and Y (eye pitch).
    pub axis: (f32, f32, usize),
    pub axis_y: (f32, f32, usize),
    pub samples: Vec<String>,
    /// Per grid point (X outer, Y inner): up to 3 (sample index, weight).
    pub grid: Vec<Vec<(usize, f32)>>,
}

impl EyeSpace {
    /// Stock UBlendSpace 2D: the four grid points around the input mixed bilinearly, their entries merged, normalised.
    pub fn weights(&self, x: f32, y: f32) -> Vec<(String, f32)> {
        let unit = |v: f32, (lo, hi, n): (f32, f32, usize)| if hi > lo { (v.clamp(lo, hi) - lo) / (hi - lo) * n as f32 } else { 0.0 };
        let (gx, gy) = (unit(x, self.axis), unit(y, self.axis_y));
        let (nx, ny) = (self.axis.2, self.axis_y.2);
        let (ix, iy) = ((gx.floor() as usize).min(nx.saturating_sub(1)), (gy.floor() as usize).min(ny.saturating_sub(1)));
        let (fx, fy) = (gx - ix as f32, gy - iy as f32);
        let mut out: Vec<(String, f32)> = Vec::new();
        for (dx, dy, w) in [(0, 0, (1.0 - fx) * (1.0 - fy)), (1, 0, fx * (1.0 - fy)), (0, 1, (1.0 - fx) * fy), (1, 1, fx * fy)] {
            let point = (ix + dx).min(nx) * (ny + 1) + (iy + dy).min(ny);
            for &(i, ew) in self.grid.get(point).into_iter().flatten() {
                let Some(name) = self.samples.get(i) else { continue };
                match out.iter_mut().find(|(n, _)| n == name) {
                    Some(slot) => slot.1 += w * ew,
                    None => out.push((name.clone(), w * ew)),
                }
            }
        }
        out.retain(|(_, w)| *w > crate::blend::ZERO_WEIGHT);
        let total: f32 = out.iter().map(|(_, w)| w).sum();
        if total > 0.0 {
            out.iter_mut().for_each(|(_, w)| *w /= total);
        }
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out
    }
}

/// The pawn's `FUN_143f0aa50` interpolator (value, target, time left).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Interp {
    pub value: f32,
    pub target: f32,
    pub remaining: f32,
}

impl Interp {
    pub fn set(&mut self, target: f32, time: f32) {
        self.target = target;
        self.remaining = time;
    }

    /// 0x143F0AA50.
    pub fn update(&mut self, dt: f32) {
        if self.target == self.value {
            return;
        }
        if self.remaining == 0.0 {
            self.value = self.target;
            return;
        }
        let rate = (self.target - self.value) / self.remaining;
        let next = rate * dt + self.value;
        self.value = if rate <= 0.0 {
            if next < self.target {
                self.target
            } else {
                next.min(1.0)
            }
        } else if next < 0.0 {
            0.0
        } else {
            next.min(self.target)
        };
        self.remaining -= dt;
    }
}

/// `TresAnimNode_ExLookAt` with its runtime state.
#[derive(Clone, Debug, PartialEq)]
pub struct ExLookAt {
    pub base: Option<usize>,
    pub offset: Vec3,
    /// (pitch, yaw, roll) radians, each pair sorted (init 0x140544FB0): +0xF4.. / +0x100..
    pub min: Vec3,
    pub max: Vec3,
    /// +0x134 bits 4 / 8 / 0x10: pitch / roll / yaw limits equal (that axis is not clamped; roll / yaw then wrap).
    pub free_pitch: bool,
    pub free_roll: bool,
    pub free_yaw: bool,
    /// +0x130, radians.
    pub margin: f32,
    /// +0xF0, radians per second.
    pub speed: f32,
    pub direct: bool,
    pub bones: Vec<RotBone>,
    /// +0x10C target, +0x118 current, +0x124 eye: (pitch, yaw, roll) degrees.
    pub target: Vec3,
    pub current: Vec3,
    pub eye: Vec3,
    /// +0x134 bit 0x20: reset done (set when the node does not run, cleared after a run).
    reset: bool,
    /// Anim instance: `m_LookAtLocationBufNum` and the delay ring (+0x5A8, write +0x5B8, read +0x5BC).
    pub buf_num: usize,
    ring: Vec<Vec3>,
    write: usize,
    read: usize,
    /// The pawn side: +0x780 focus interpolator, +0x768 notify interpolator, +0x798 point.
    pub focus: Interp,
    pub notify: Interp,
    pub point: Vec3,
    /// The anim instance's LookAtRate / LookAtLocation of the latest update (what the node read).
    pub rate: f32,
    pub location: Vec3,
    last_rate: f32,
    /// The face anim set's eye-look blend spaces.
    pub eye_spaces: Vec<EyeSpace>,
    /// (FACE) THE HEAD'S LOOK-UP CAP (radians of the head's own pitch; None = KH3's limits only). KH3 turns every rot
    /// bone by its coefficient x the look pitch, so the head ends at (sum of the chain's pitch coefficients) x the pitch:
    /// Donald 2.1 x 40 deg = 84 deg, Goofy 2.3 x 40 = 92 deg up. The user (2026-10-22): "Limit how high Donald's head
    /// can look up to a reasonable amount, then he uses his eyes to look the rest of the way up to the normal limits for
    /// how far his eyes look". With a cap the look pitch's upper limit is cap / gain; the rest goes to the eyes (the eye
    /// input `eye` = the pitch beyond the limit, KH3's eye-look grids NO_0_Eye / BA_00_Eye, up to their own 50 deg).
    pub head_up_max: Option<f32>,
    /// (FACE, the user 2026-10-22: "Goofy should use his eyes to look around too") THE EYES' SHARE of every look within
    /// the head's limits (0..1, OUR DESIGN; 0 = KH3's node: the eyes only take what the head's limits leave). The head turns
    /// to (1 - share) x the clamped look; the eyes aim at the target from where the head points NOW (`eye` = look -
    /// `current`), so they lead and the head follows at its own turn speed. Key `eye_share` of lookat.json, setter
    /// [`ExLookAt::set_eye_share`].
    pub eye_share: f32,
    /// (FACE) The eyes' turn speed (degrees per second, OUR DESIGN 600: a quick but not instant move; no pop when a target
    /// changes); `eye` follows its aim at this rate.
    pub eye_speed: f32,
    /// (FACE) THE CAST LOOK'S HOLD (OUR DESIGN, the user 2026-10-23: "Donald should look at his target when he's casting
    /// magic not anything else"): while held, the head's look-at stays on whatever the clips' TresAnimNotifyState_LookAt
    /// states say (KH3's cast clips turn it off); `want` keeps what those states asked, restored when the hold ends.
    /// Both moves blend over [`HOLD_BLEND`]. Setter [`ExLookAt::set_hold`].
    pub hold: bool,
    want: f32,
}

/// (FACE) The blend (s) of the cast look's hold on and off.
pub const HOLD_BLEND: f32 = 0.25;

/// (FACE) The default eyes' share of a look (see `ExLookAt::eye_share`).
pub const EYE_SHARE: f32 = 0.35;
/// (FACE) The default eye turn speed (deg/s).
pub const EYE_SPEED: f32 = 600.0;

/// UE `FRotator::Quaternion` (degrees).
fn rotator_quat(r: Vec3) -> Quat {
    let (sp, cp) = (r.x * DEG * 0.5).sin_cos();
    let (sy, cy) = (r.y * DEG * 0.5).sin_cos();
    let (sr, cr) = (r.z * DEG * 0.5).sin_cos();
    Quat::from_xyzw(cr * sp * sy - sr * cp * cy, -cr * sp * cy - sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy)
}

/// `FMath::UnwindRadians` (0x1405548D0).
fn unwind_rad(mut a: f32) -> f32 {
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

/// `FMath::UnwindDegrees` (0x140554890).
fn unwind_deg(mut a: f32) -> f32 {
    while a > 180.0 {
        a -= 360.0;
    }
    while a < -180.0 {
        a += 360.0;
    }
    a
}

/// `FRotator::NormalizeAxis` (the divps-by-360 block after the turn): into (-180, 180].
fn normalize_axis(a: f32) -> f32 {
    let mut x = a % 360.0;
    if x < 0.0 {
        x += 360.0;
    }
    if x > 180.0 {
        x -= 360.0;
    }
    x
}

fn safe_normal(v: Vec3) -> Vec3 {
    let l = v.length_squared();
    if l == 1.0 {
        v
    } else if l >= 1.0e-8 {
        v / l.sqrt()
    } else {
        Vec3::ZERO
    }
}

/// `FTransform::BlendWith` (translation / scale lerp, rotation fast lerp on the short arc, normalised).
fn blend(a: &Trs, b: &Trs, alpha: f32) -> Trs {
    let bq = if a.rotation.dot(b.rotation) < 0.0 { -b.rotation } else { b.rotation };
    let q = Quat::from_xyzw(a.rotation.x + (bq.x - a.rotation.x) * alpha, a.rotation.y + (bq.y - a.rotation.y) * alpha, a.rotation.z + (bq.z - a.rotation.z) * alpha, a.rotation.w + (bq.w - a.rotation.w) * alpha).normalize();
    Trs { translation: a.translation.lerp(b.translation, alpha), rotation: q, scale: a.scale.lerp(b.scale, alpha) }
}

fn component(skeleton: &Skeleton, local: &[Trs], bone: usize) -> Trs {
    let mut chain = Vec::new();
    let mut b = Some(bone);
    while let Some(i) = b {
        chain.push(i);
        b = skeleton.bones[i].parent;
    }
    let mut out = Trs::IDENTITY;
    for &i in chain.iter().rev() {
        out = local[i].then(&out);
    }
    out
}

fn is_ancestor(skeleton: &Skeleton, ancestor: usize, bone: usize) -> bool {
    let mut b = skeleton.bones[bone].parent;
    while let Some(i) = b {
        if i == ancestor {
            return true;
        }
        b = skeleton.bones[i].parent;
    }
    false
}

impl ExLookAt {
    /// From `data/lookat.json`.
    pub fn from_json(data: &Json, skeleton: &Skeleton) -> Result<ExLookAt, String> {
        let v3 = |j: &Json| {
            let f = j.floats();
            if f.len() == 3 {
                Ok(Vec3::new(f[0], f[1], f[2]))
            } else {
                Err("lookat.json: a vector of 3 expected".to_string())
            }
        };
        let base = data.get("base_bone").as_str().and_then(|n| skeleton.index_of(n));
        let (lo, hi) = (v3(data.get("rotation_limit_min_deg"))? * DEG, v3(data.get("rotation_limit_max_deg"))? * DEG);
        let (min, max) = (lo.min(hi), lo.max(hi));
        let mut bones: Vec<RotBone> = Vec::new();
        for b in data.get("rot_bones").items() {
            let Some(bone) = b.get("bone").as_str().and_then(|n| skeleton.index_of(n)) else { continue };
            let parent = bones.iter().rposition(|p| is_ancestor(skeleton, p.bone, bone));
            bones.push(RotBone { bone, yaw: b.get("yaw").as_f32().unwrap_or(0.0), pitch: b.get("pitch").as_f32().unwrap_or(0.0), param_a: b.get("param_a").as_f32().unwrap_or(0.0), parent });
        }
        let buf_num = data.get("anim_instance").get("location_buf_num").as_usize().unwrap_or(0);
        let notify_initial = data.get("pawn").get("notify_initial").as_f32().unwrap_or(1.0);
        Ok(ExLookAt {
            base,
            offset: v3(data.get("base_offset"))?,
            min,
            max,
            free_pitch: min.x == max.x,
            free_roll: min.z == max.z,
            free_yaw: min.y == max.y,
            margin: data.get("back_margin_deg").as_f32().unwrap_or(0.0) * DEG,
            speed: data.get("rotation_speed_deg").as_f32().unwrap_or(0.0) * DEG,
            direct: data.get("direct_look").as_bool().unwrap_or(false),
            bones,
            target: Vec3::ZERO,
            current: Vec3::ZERO,
            eye: Vec3::ZERO,
            reset: false,
            buf_num,
            ring: Vec::new(),
            write: 0,
            read: 0,
            focus: Interp::default(),
            notify: Interp { value: notify_initial, target: notify_initial, remaining: 0.0 },
            point: Vec3::ZERO,
            rate: 0.0,
            location: Vec3::ZERO,
            last_rate: 0.0,
            eye_spaces: data
                .get("eye_blend_spaces")
                .items()
                .iter()
                .filter_map(|e| {
                    let axis = |j: &Json| {
                        let f = j.floats();
                        (f.len() == 3).then(|| (f[0], f[1], f[2] as usize))
                    };
                    Some(EyeSpace {
                        row: e.get("row").as_str()?.to_string(),
                        axis: axis(e.get("axis"))?,
                        axis_y: axis(e.get("axis_y"))?,
                        samples: e.get("samples").items().iter().filter_map(|s| s.as_str().map(str::to_string)).collect(),
                        grid: e.get("grid").items().iter().map(|p| p.items().iter().filter_map(|x| Some((x.at(0).as_usize()?, x.at(1).as_f32()?))).collect()).collect(),
                    })
                })
                .collect(),
            head_up_max: data.get("head_up_max_deg").as_f32().filter(|d| *d > 0.0).map(|d| d * DEG), // (FACE)
            eye_share: data.get("eye_share").as_f32().unwrap_or(EYE_SHARE).clamp(0.0, 1.0), // (FACE)
            eye_speed: data.get("eye_speed_deg").as_f32().unwrap_or(EYE_SPEED).max(1.0),   // (FACE)
            hold: false,                                                                         // (FACE)
            want: notify_initial,                                                                // (FACE)
        })
    }

    /// (FACE) The head's own pitch per degree of look pitch: the pitch coefficients of the rot bones down the chain to the
    /// last one (the head), each turning everything after it.
    pub fn head_pitch_gain(&self) -> f32 {
        self.bones.iter().map(|b| b.pitch.max(0.0)).sum()
    }

    /// (FACE) Caps the head's look-up pitch at `deg` degrees (None or <= 0: KH3's own limit). See `head_up_max`.
    pub fn set_head_up_max(&mut self, deg: Option<f32>) {
        self.head_up_max = deg.filter(|d| *d > 0.0).map(|d| d * DEG);
    }

    /// (FACE) The eyes' share of a look (0..1; 0 = KH3's node). See `eye_share`.
    pub fn set_eye_share(&mut self, share: f32) {
        self.eye_share = share.clamp(0.0, 1.0);
    }

    /// (FACE) The upper limit of the look pitch (radians): KH3's, or the head's cap / gain when lower.
    pub fn pitch_up_limit(&self) -> f32 {
        let gain = self.head_pitch_gain();
        match self.head_up_max {
            Some(cap) if gain > 0.0 => self.max.x.min(cap / gain),
            _ => self.max.x,
        }
    }

    /// The controller's UpdateControlRotation (0x140527460): the focal point this tick, `None` = no focus.
    pub fn set_focus(&mut self, point: Option<Vec3>) {
        match point {
            Some(p) => {
                // (FACE) under the cast look's hold the focus comes in at the hold's blend (KH3's SetLookAt: 0.5 s)
                self.focus.set(1.0, if self.hold { HOLD_BLEND } else { 0.5 });
                self.point = p;
            }
            None => self.focus.set(0.0, 0.5),
        }
    }

    /// A `TresAnimNotifyState_LookAt` begin / end: `on` after m_ReverseTrg was applied.
    pub fn notify(&mut self, on: bool, time: f32) {
        // (FACE) under the cast look's hold the states' ask is kept for its end
        self.want = if on { 1.0 } else { 0.0 };
        if !self.hold {
            self.notify.set(self.want, time);
        }
    }

    /// (FACE) The cast look's hold on / off (see `hold`): on = the head's look-at on over the clips' LookAt states; off =
    /// back to what they ask now. Both blend over [`HOLD_BLEND`].
    pub fn set_hold(&mut self, on: bool) {
        if on == self.hold {
            return;
        }
        self.hold = on;
        self.notify.set(if on { 1.0 } else { self.want }, HOLD_BLEND);
    }

    /// The pawn tick (0x1405B2190) then the anim instance's NativeUpdateAnimation read (0x14058B450): rate and the
    /// delayed location for this frame's evaluation.
    pub fn update(&mut self, dt: f32) {
        self.notify.update(dt);
        self.focus.update(dt);
        self.rate = self.focus.value * self.notify.value;
        // (FACE) the location runs while there is a focus (the eyes follow it even where a LookAt notify state turns the
        // head's look-at off; KH3's ring ran on LookAtRate)
        let previous = self.last_rate;
        self.last_rate = self.focus.value;
        if self.focus.value <= 0.0 {
            return;
        }
        let n = self.buf_num;
        if n == 0 {
            self.location = self.point;
            return;
        }
        if previous <= 0.0 || self.ring.len() != n {
            self.ring = vec![self.point; n];
            (self.write, self.read) = (n - 1, 0);
        }
        self.ring[self.write] = self.point;
        self.write = if self.write + 1 >= n { 0 } else { self.write + 1 };
        self.location = self.ring[self.read];
        self.read = if self.read + 1 >= n { 0 } else { self.read + 1 };
    }

    /// The eye-look blend space input of a face row (anim vt+0x470 -> 0x14056BC10: (-LookAtEyeRot.yaw,
    /// LookAtEyeRot.pitch) with LookAtRate as the alpha, mixed from the default (0, 0) [inf]); `None` for other rows.
    pub fn eye_weights(&self, row: &str) -> Option<Vec<(String, f32)>> {
        let space = self.eye_spaces.iter().find(|e| e.row.eq_ignore_ascii_case(row))?;
        let a = self.eye_rate();
        Some(space.weights(-self.eye.y * a, self.eye.x * a))
    }

    /// (FACE) The eyes' look-at weight: the focus interpolator alone (KH3: LookAtRate = focus x notify). The eyes keep
    /// looking where a LookAt notify state turns the head's look-at off (the battle idle).
    pub fn eye_rate(&self) -> f32 {
        self.focus.value.clamp(0.0, 1.0)
    }

    fn reset_rotations(&mut self) {
        if !self.reset {
            (self.target, self.current, self.eye) = (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
            self.reset = true;
        }
    }

    /// The node in the graph (EvaluateComponentSpace 0x140536060 -> EvaluateBoneTransforms 0x1405384E0), on the local
    /// pose. `alpha` = the bound LookAtRate.
    pub fn evaluate(&mut self, skeleton: &Skeleton, local: &mut [Trs], component_to_world: &Trs, dt: f32) {
        let alpha = self.rate.clamp(0.0, 1.0);
        // (FACE) the eyes look while there is a focus, the head only with LookAtRate
        let head_on = alpha >= 1.0e-5;
        let Some(base) = self.base.filter(|_| head_on || self.eye_rate() >= 1.0e-5) else {
            self.reset_rotations();
            return;
        };
        if !head_on {
            (self.target, self.current) = (Vec3::ZERO, Vec3::ZERO);
            self.reset = true;
        }
        // ---- the look direction (component space)
        let origin = component(skeleton, local, base).transform_point(self.offset);
        let target = Trs::IDENTITY.relative_to(component_to_world).transform_point(self.location);
        let d = target - origin;
        let h = safe_normal(d - d.dot(UP) * UP);
        let facing = h.dot(FORWARD);
        let mut yaw = facing.clamp(-1.0, 1.0).acos();
        if FORWARD.x * h.y - FORWARD.y * h.x < 0.0 {
            yaw = -yaw;
        }
        let v = Quat::from_axis_angle(UP, -yaw) * d;
        let v = safe_normal(v - v.dot(SIDE) * SIDE);
        let mut pitch = v.dot(FORWARD).clamp(-1.0, 1.0).acos();
        if v.z * FORWARD.y - v.y * FORWARD.z < 0.0 {
            pitch = -pitch;
        }
        // The back margin: the point behind him keeps the head on the side it is turned to.
        if !self.free_yaw && self.margin > 0.0 && !self.reset && facing < 0.0 {
            let m = if self.current.y < 0.0 { -self.margin } else { self.margin };
            let rest = unwind_rad(yaw - m);
            if rest * m >= 0.0 {
                yaw = self.current.y * DEG + rest;
            }
        }
        // (FACE) the head's look-up cap lowers the upper limit; the eyes take what is beyond it (`self.eye` below)
        let up = self.pitch_up_limit();
        // (FACE) the head turns to (1 - eye_share) of the look, within its limits; the eyes take the rest (below, after the
        // turn): a far target still brings the head to its limit (the look-up cap), the eyes past it
        let head_share = 1.0 - self.eye_share.clamp(0.0, 1.0);
        let (hp, hy) = (pitch * head_share, yaw * head_share);
        let p = if self.free_pitch && self.head_up_max.is_none() { hp } else if self.free_pitch { hp.min(up) } else { hp.clamp(self.min.x, up.max(self.min.x)) };
        let y = if self.free_yaw { hy } else { hy.clamp(self.min.y, self.max.y) };
        self.target = if head_on { Vec3::new(p / DEG, y / DEG, 0.0) } else { Vec3::ZERO };
        // ---- the turn toward the target
        if dt > 0.0 && head_on {
            if self.speed <= 0.0 || self.direct {
                self.current = self.target;
            } else {
                let step = self.speed * dt;
                let q = rotator_quat(self.current).inverse() * rotator_quat(self.target);
                let angle = unwind_rad(2.0 * q.w.clamp(-1.0, 1.0).acos());
                if angle.abs() <= step {
                    self.current = self.target;
                } else {
                    let t = (step / angle.abs()).clamp(0.0, 1.0);
                    let c = self.current;
                    let dp = unwind_deg(self.target.x - c.x);
                    let dr = if self.free_roll { unwind_deg(self.target.z - c.z) } else { self.target.z - c.z };
                    let dy = if self.free_yaw { unwind_deg(self.target.y - c.y) } else { self.target.y - c.y };
                    self.current = Vec3::new(normalize_axis(c.x + dp * t), normalize_axis(c.y + dy * t), normalize_axis(c.z + dr * t));
                }
            }
        }
        // ---- (FACE) the eyes: at the target from where the head points now (they lead; the head follows), at their own
        // turn speed. KH3's node gave them only what the limits left: (pitch - clamped, yaw - clamped).
        let aim = Vec3::new(pitch / DEG - self.current.x, unwind_deg(yaw / DEG - self.current.y), 0.0);
        if dt > 0.0 {
            let d = aim - self.eye;
            let step = self.eye_speed * dt;
            self.eye = if d.length() <= step { aim } else { self.eye + d * (step / d.length()) };
        }
        if !head_on {
            return; // (FACE) the head's look-at is off (a LookAt notify state): the eyes only
        }
        // ---- the rot bones (ParamA coefficients: all 0 in both friends' graphs, so the ParamA terms vanish)
        let (cur_pitch, cur_yaw) = (self.current.x * DEG, self.current.y * DEG);
        let mut out: Vec<(usize, Trs)> = Vec::with_capacity(self.bones.len());
        let mut original: Vec<Trs> = Vec::with_capacity(self.bones.len());
        for rb in &self.bones {
            let cs = component(skeleton, local, rb.bone);
            original.push(cs);
            let mut new_cs = cs;
            if rb.yaw > 0.0 || rb.pitch > 0.0 {
                let (yaw_angle, pitch_angle) = (rb.yaw * cur_yaw, rb.pitch * cur_pitch);
                let inv = cs.rotation.conjugate();
                let one = |x: f32| if x.abs() <= 1.0e-8 { 0.0 } else { 1.0 / x };
                let inv_scale = Vec3::new(one(cs.scale.x), one(cs.scale.y), one(cs.scale.z));
                let (sy, cy) = (yaw_angle * 0.5).sin_cos();
                let (sp, cp) = (pitch_angle * 0.5).sin_cos();
                let ay = inv * (UP * inv_scale);
                let ap = inv * (SIDE * inv_scale);
                let delta = Quat::from_xyzw(ay.x * sy, ay.y * sy, ay.z * sy, cy) * Quat::from_xyzw(ap.x * sp, ap.y * sp, ap.z * sp, cp);
                let parent_cs = skeleton.bones[rb.bone].parent.map_or(Trs::IDENTITY, |pb| component(skeleton, local, pb));
                let mut in_parent = cs.relative_to(&parent_cs);
                in_parent.rotation = (in_parent.rotation * delta).normalize();
                new_cs = in_parent.then(&parent_cs);
            }
            if let Some(pe) = rb.parent {
                if let Some(parent_new) = out.get(pe).map(|o| o.1) {
                    new_cs = new_cs.relative_to(&original[pe]).then(&parent_new);
                }
            }
            out.push((rb.bone, new_cs));
        }
        // ---- LocalBlendCSBoneTransforms (bone order; each relative to its parent's CURRENT component transform)
        let mut order: Vec<usize> = (0..out.len()).collect();
        order.sort_by_key(|&i| out[i].0);
        for i in order {
            let (bone, cs) = out[i];
            let parent_cs = skeleton.bones[bone].parent.map_or(Trs::IDENTITY, |pb| component(skeleton, local, pb));
            let target_local = cs.relative_to(&parent_cs);
            local[bone] = if alpha >= 1.0 - 1.0e-8 { target_local } else { blend(&local[bone], &target_local, alpha) };
        }
        self.reset = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_interpolator_is_the_games() {
        let mut i = Interp::default();
        i.set(1.0, 0.5);
        i.update(0.25);
        assert!((i.value - 0.5).abs() < 1e-6, "{}", i.value);
        i.update(0.25);
        assert_eq!(i.value, 1.0);
        i.set(0.0, 0.0);
        i.update(0.016);
        assert_eq!(i.value, 0.0);
    }

    #[test]
    fn rotator_quaternion_matches_ue() {
        // Yaw 90 about +Z (UE's yaw sign: the formula above, FRotator::Quaternion).
        let q = rotator_quat(Vec3::new(0.0, 90.0, 0.0));
        assert!((q.z - (45f32 * DEG).sin()).abs() < 1e-6 && (q.w - (45f32 * DEG).cos()).abs() < 1e-6);
    }
}
