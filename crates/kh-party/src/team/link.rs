//! OWNER: party stream D. The friends' side of the team attacks, stepped once per fixed step after
//! `Party::step` (contract D.3): the NPC link points and offers, the link pawns of the four
//! situation team attacks and the unison / fusion partner states.
//!
//! Evidence and departures (tags as the plan's: [C] decompiled C, [A] asset, [D] derived / designed):
//!
//! - Link points [C] `TresNpcPawnBase` tick 0x14125EB40 with slots 1265 0x1412585B0 (the player's
//!   gauge full), 1267 0x141257690 (ready: 1, offer pending: 2, blocked: -1, reset: 0), 1275
//!   0x141261B30 (points by Sora's distance: d > 1000 cm -> 4..12, > 500 -> 8..16, else 10..20, a
//!   uniform integer; while the friend is > 1000 cm from its target (min(vt+0x27C8, vt+0x27D0) [C]): 2..4 / 4..6 / 8..12;
//!   the target taken as the nearest enemy [D]), 1278 0x141277180 (reset), 1279 0x1412793F0 (threshold =
//!   round((80..120 % - 0.1 x COMBI_UP) x `m_FriendLinkPointMax` 100), at least 1),
//!   `m_FriendLinkPointInterval` 5 s [A]. The offer: 1286 0x141279280 picks a kind of
//!   `m_HaveFriendLinkCommand` at random [C] (Donald 207, 208; Goofy 193, 195 and the world kinds
//!   194 / 198 / 201 / 206, which the candidate filter vt+0x2828 [?] leaves out here: no such worlds),
//!   1219 0x141279010 resets every friend's points and calls the player's `NotifySetActionCommand`
//!   (here: the `Friend::offer` row of one step). Goofy 0x1411D1020: 193 needs Donald.
//! - Link pawns: the participants are ATTACHED to the link pawn and play their clips there
//!   (`vt+0x23A8(link pawn, motion)` [C 0x1412D5680]); the clips of Sora and the friends are authored
//!   in that common space, so each one's actor is held at the link pawn's transform [D]. The link
//!   pawns' state machines are read for CallMeteor (Appear / Finish / Disappear), FlareForce (Appear
//!   / Finish / Disappear + the missile machine), GoofyShoot (Appear / Move + Goofy's NPC state) and
//!   ShieldGuardian (its state table and input slots; the movement is kh-core's CMC); where a time is
//!   not read it is the participants' clip length or the asset numbers named at each constant [D].
//! - Hits: the team rows are rows of Sora's `p_ex001_AttackData` [A] (FR_CALLMETEOR_FINISH ...):
//!   they are delivered as Sora's shots through `FriendLinkView::shots`.

use super::proj::{rich_eval, rot_vector, Contact, ProjKind, TeamProj, METEOR_SHAKE_MAX, METEOR_SHAKE_MIN, METEOR_SHAKE_Z, FLARE_APPEAR_END, FLARE_APPEAR_X, FLARE_APPEAR_Z, FLARE_FIRE_END, FLARE_FIRE_X, FLARE_FIRE_Z};
use super::{fr_id, FrDrive, TeamEvent, TeamMember, TeamOutput, TeamParty};
use kh_core::cmc::Cmc;
use kh_core::host::party::SeamHandles;
use kh_core::host::{Attach, EffectId, EffectSpawn, Effects};
use crate::command::MemberCommand;
use crate::host::PartyHost;
use crate::ids::{Chara, MemberId};
use crate::states::NpcStateId;
use glam::{Quat, Vec3};
use kh_core::host::{Friend, FriendLinkRequest, FriendLinkView, TargetFlags, TargetId, TargetInfo, TeamShot};

/// `m_FriendLinkPointMax` (`ATresNpcPawnBase` ctor 0x141254FF0: +0xB38 = 100.0) [C].
pub const LINK_POINT_MAX: f32 = 100.0;
/// `m_FriendLinkPointInterval` (`n_ex00x_PawnBase` CDO: 5) [A].
pub const LINK_POINT_INTERVAL: f32 = 5.0;
/// `m_HaveFriendLinkCommand` (ctors 0x1411BA820 Donald: 0xCF, 0xD0; 0x1411BAB80 Goofy: 0xC1, 0xC3, ...) [C].
pub const DONALD_COMMANDS: [i32; 2] = [207, 208];
pub const GOOFY_COMMANDS: [i32; 2] = [193, 195];

/// (D4) The link pawns' actor origin above Sora's actor origin: 0 for all four [C 0x14127A780:
/// FUN_1405D9130 spawns the link pawn at the player pawn's root-component world location (+0x2B0)
/// with his yaw only]. KH3's actor origin is the foot level, the `TresRootComponent` shapes sit
/// above it (Sora's capsule at +91, ShieldGuardian's at +190, CallMeteor's at +300, FlareForce's
/// box at +200, GoofyShoot's sphere at 0 [A *_Pawn TresRoot CollisionShapesSrc]), so the offsets
/// the C adds to the pawn's location (meteor base +650, the missiles' appear offset, GoofyShoot's
/// T +1000, its FR_GOOFYSHOOT_SORA sphere at relative 0) start at the feet. The cameras' base
/// is the same root location (0x140621E00: the target actor's root +0x2B0).
pub const LINK_PAWN_ORIGIN_Z: f32 = 0.0;
/// (D4) Goofy's capsule centre above his origin (n_ex002_PawnBase TresRoot CAPSULE1 r 40, half
/// height 89, at +91) [A]: his rush's body sweep in Goofy Bombardier.
pub const GOOFY_CAPSULE_Z: f32 = 91.0;
/// (D4) The player pawn's point FUN_1412AB570 measures from: his mesh's "center" bone (mesh
/// +0x117C = bone index of FName "center", 0x1405E1120 / 0x1400A1200), else his root. The bone's
/// height: (party I1) the bind pose of p_ex001_0_Skeleton [A]: "center" (bone 1) is a child of the root
/// "kh_sk" at translation (0, -102.338, 0) under kh_sk's rotation (-0.7071, 0, 0, 0.7071) = -90 deg about X,
/// i.e. 102.338 cm above the root; the mesh component sits at the actor origin (RelativeLocation 0, yaw 270,
/// scale 1: build/sora_pack/manifest.json skeleton / placement). The game reads the POSED bone (FUN_1412AB570);
/// kh-party has no access to Sora's pose, so the bind height stands in for it.
pub const SORA_CENTER_BONE_Z: f32 = 102.338;
/// (D4) CallMeteor's `m_fFinishMaxRange` (ctor 0x1412C6650: 10000; the CDO keeps it) [C]: the
/// range of its FUN_1412AC290 target search (0x1412CD0E0, with the sight test).
pub const METEOR_FINISH_MAX_RANGE: f32 = 10_000.0;
/// CallMeteor_Pawn CDO [A] (ctor 0x1412C6650 [C] for what the CDO keeps): `m_fAppearHeight` 650,
/// `m_fAppearRange` 900, `m_fAppearScale` 5.3 (/s), `m_fBaseSpeed` 1700, `m_fWhiteOutTimer` 4.2,
/// `m_fWhiteInTimer` 4.8, `m_fFinishRange` 1500 (ctor), `m_fFinishHopeRange` 1200,
/// `m_fFinishCameraOffset` 900; the trace sphere 150 (FUN_1412D2B30(150) -> +0xD60).
pub const METEOR_HEIGHT: f32 = 650.0;
pub const METEOR_RANGE: f32 = 900.0;
pub const METEOR_APPEAR_SCALE: f32 = 5.3;
pub const METEOR_SPEED: f32 = 1700.0;
pub const METEOR_WHITE_OUT: f32 = 4.2;
pub const METEOR_WHITE_IN: f32 = 4.8;
pub const METEOR_FINISH_RANGE: f32 = 1500.0;
pub const METEOR_CAMERA_OFFSET: f32 = 900.0;
pub const METEOR_TRACE: f32 = 150.0;
/// The Disappear countdown (Disappear begin 0x1412CCB90 sets the timer to 2.0; its tick
/// 0x1412D9910 counts down and destroys the pawn at 0, or earlier when the fade effect ends) [C].
pub const METEOR_DISAPPEAR: f32 = 2.0;
/// CMeteoFinish's length (the Appear ends when a slave pawn's motion ends, vt+0x2698 0x141401C70) [A].
pub const CMETEO_LENGTH: f32 = 2.0;
pub const FINISH_HOPE_RANGE: f32 = 1200.0;
/// `m_MeteorInfos` (CDO, 40 rows) [A]: base yaw, start length, target length (fractions of the
/// range), start timer (x `m_fWhiteOutTimer`), initial speed (x `m_fBaseSpeed`).
pub const METEORS: [(f32, f32, f32, f32, f32); 40] = [
    (0.0, 0.0, 0.0, 0.2, 1.2),
    (0.0, 0.05, 0.31, 0.3, 1.0),
    (51.42857, 0.12, 0.34, 0.4, 1.1),
    (102.85714, 0.08, 0.25, 0.5, 1.2),
    (154.28572, 0.14, 0.41, 0.2, 1.1),
    (205.71428, 0.09, 0.35, 0.3, 1.0),
    (257.14285, 0.12, 0.8, 0.42, 1.2),
    (308.57144, 0.08, 0.4, 0.5, 1.0),
    (9.0, 0.15, 0.57, 0.48, 1.1),
    (36.692307, 0.27, 0.63, 0.38, 1.2),
    (64.38461, 0.15, 0.59, 0.34, 1.1),
    (92.07692, 0.2, 0.55, 0.26, 1.3),
    (119.76923, 0.14, 0.6, 0.44, 1.2),
    (147.46153, 0.23, 0.56, 0.38, 1.1),
    (175.15384, 0.18, 0.61, 0.46, 1.3),
    (202.84616, 0.22, 0.59, 0.5, 1.1),
    (230.53847, 0.16, 0.63, 0.42, 1.0),
    (258.23077, 0.21, 0.55, 0.34, 1.3),
    (285.92307, 0.15, 0.62, 0.48, 1.2),
    (313.6154, 0.23, 0.54, 0.36, 1.3),
    (341.30768, 0.24, 0.6, 0.26, 1.0),
    (18.0, 0.2, 0.9, 0.3, 1.1),
    (36.947369, 0.31, 0.75, 0.4, 1.4),
    (55.894737, 0.25, 0.83, 0.46, 1.2),
    (74.8421, 0.31, 0.95, 0.3, 1.0),
    (93.789474, 0.33, 0.88, 0.22, 1.1),
    (112.73684, 0.28, 0.78, 0.3, 1.3),
    (131.6842, 0.24, 0.91, 0.42, 1.2),
    (150.63158, 0.3, 0.85, 0.46, 1.1),
    (169.57895, 0.26, 0.89, 0.43, 1.2),
    (188.52632, 0.33, 0.93, 0.2, 1.0),
    (207.47368, 0.24, 0.88, 0.5, 1.2),
    (226.42105, 0.29, 0.79, 0.45, 1.1),
    (245.36842, 0.2, 0.9, 0.26, 1.3),
    (264.3158, 0.31, 0.78, 0.29, 1.1),
    (283.26315, 0.29, 0.86, 0.2, 1.0),
    (302.21054, 0.35, 0.92, 0.32, 1.2),
    (321.1579, 0.29, 0.81, 0.46, 1.1),
    (340.10526, 0.26, 0.93, 0.4, 1.2),
    (359.05264, 0.3, 0.86, 0.35, 1.3),
];
/// FlareForce_Pawn CDO [A] (ctor 0x1412C6960 [C]): `m_nLockOnMaxNum` 3, `m_fFinishHopeRange`
/// 1200, `m_fMaxSpeed` 1200, `m_fHomingAngle` 1000, `m_fHomingUpdateRange` 100, `m_fAppearPitch`
/// 40, `m_fAppearScale` 5 (ctor), `m_vAppearOffset` (180, 0, 100), `m_fTargetHeight_2` 1000 /
/// `_3` 2000 (ctor: the row by the target's height, FUN_1412DF900 [C]).
pub const FLARE_LOCK_MAX: usize = 3;
pub const FLARE_SPEED: f32 = 1200.0;
pub const FLARE_HOMING_ANGLE: f32 = 1000.0;
pub const FLARE_HOMING_UPDATE: f32 = 100.0;
pub const FLARE_APPEAR_PITCH: f32 = 40.0;
pub const FLARE_APPEAR_SCALE: f32 = 5.0;
pub const FLARE_APPEAR_OFFSET: Vec3 = Vec3::new(180.0, 0.0, 100.0);
pub const FLARE_TARGET_HEIGHT_2: f32 = 1000.0;
pub const FLARE_TARGET_HEIGHT_3: f32 = 2000.0;
/// FForceFinish's length [A].
pub const FFORCE_LENGTH: f32 = 3.9333334;
/// `m_MissileInfos` (CDO, 16 rows) [A]: appear yaw, appear timer, fire timer.
pub const MISSILES: [(f32, f32, f32); 16] = [
    (0.0, 0.0, 3.115),
    (22.5, 0.15, 2.488),
    (45.0, 0.15, 1.865),
    (67.5, 0.25, 1.365),
    (90.0, 0.05, 2.615),
    (112.5, 0.15, 1.99),
    (135.0, 0.2, 1.398),
    (157.5, 0.25, 2.738),
    (180.0, 0.05, 2.108),
    (202.5, 0.15, 1.482),
    (225.0, 0.25, 2.865),
    (247.5, 0.1, 2.238),
    (270.0, 0.0, 1.615),
    (292.5, 0.1, 2.99),
    (315.0, 0.2, 2.365),
    (337.5, 0.05, 1.738),
];
/// ShieldGuardian_Pawn CDO [A]: `m_Param_MaxTime` 20 s.
pub const SHIELD_MAX_TIME: f32 = 20.0;
/// ShieldGuardian_Pawn `TresCharMovement0.MaxWalkSpeed` 180 [A].
pub const SHIELD_WALK_SPEED: f32 = 180.0;

/// Sora's FForceFinish (p_ex001 1723): its TresAnimNotify_Timing (m_Param 0) at 1.2064 s starts
/// the Finish (link pawn +0xC6E via FUN_1412E1950) [A + C].
pub const FFORCE_FINISH_TIMING: f32 = 1.206_405_9;
/// The Disappear state's time in the state table (0x4041F120 = 3.0303 s, 0x1412E2430) [C].
pub const FLARE_DISAPPEAR_TIME: f32 = 3.030_303;
/// FlareForce_Pawn CDO [A]: `m_vUpOffset` (200, 0, 250), `m_fHomingRange` 25; ctor `m_fLifeTime` 10 [C].
pub const FLARE_UP_OFFSET: Vec3 = Vec3::new(200.0, 0.0, 250.0);
pub const FLARE_HOMING_RANGE: f32 = 25.0;
pub const FLARE_LIFE_TIME: f32 = 10.0;

/// What a missile's tick produced.
#[derive(Clone, Copy, Debug, PartialEq)]
enum MissileOut {
    None,
    Hit(Vec3),
    Explode(Vec3),
}

/// `v` rotated by `yaw` degrees about Z.
fn rot_yaw(v: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}


/// The world gravity (`GetGravityZ` with the CDOs' default scale 1: -980) [A].
pub const GRAVITY: f32 = 980.0;
/// (D4) The GoofyShoot link pawn's own gravity: vt+0x1660 0x1405AAD90 -> its movement's GetGravityZ
/// 0x140582270 = world -980 x `GravityScale` 3 (GoofyShoot_Pawn TresCharMovement0 [A]) x +0xBF8 (1):
/// what FUN_1412C93A0 launches with (g = -vt+0x1660) and what the rise falls under (mode 3) [C].
pub const GSHOOT_GRAVITY: f32 = 980.0 * 3.0;
/// Goofy Bombardier [C 0x1412CC160 / 0x1412DCE50, A]: T = target + 1000 z (ctor +0xBE0 = 1000),
/// `m_Param_ThrowPitch` -45 (ctor 0x1412C6FB0), the throw speed 2300, the SORA sphere 350 cm
/// (GoofyShoot_Pawn TresAtkColl0), the RUSH sphere 150 cm (w_gu_WepBase, socket ATK_00).
pub const GSHOOT_TARGET_HEIGHT: f32 = 1000.0;
pub const GSHOOT_THROW_PITCH: f32 = -45.0;
pub const GSHOOT_THROW_SPEED: f32 = 2300.0;
pub const GSHOOT_SORA_RADIUS: f32 = 350.0;
pub const GSHOOT_RUSH_RADIUS: f32 = 150.0;
/// (D4) The link pawn's OWN motions: its mesh o_at001_0 (GoofyShoot_Pawn TresCharMesh0) with the
/// anim set o_at001_AnimSet rows GShootStart = rt/1636 (57 frames, SequenceLength 1.8666667),
/// GShootRise = 1637 (0.4), GShootHit = 1638 (1.2333333), GShootFinish = 1639 (2.0333333) [A]; the
/// Move tick 0x1412DCE50 waits on ITS motion (+0x440 FUN_1405ACE60) after GShootStart (case 1)
/// and GShootHit (case 3), on Goofy's after GShootFinish (case 4: n_ex002 1639, 1.4333333) [C];
/// Goofy's M_1501 (n_ex002, 0.6333333) [A].
pub const GSHOOT_START_LENGTH: f32 = 1.866_666_7;
pub const GSHOOT_RISE_LENGTH: f32 = 0.4;
pub const GSHOOT_HIT_LENGTH: f32 = 1.233_333_3;
pub const GSHOOT_FINISH_LENGTH_PAWN: f32 = 2.033_333_3;
pub const GSHOOT_FINISH_LENGTH_GOOFY: f32 = 1.433_333_3;
pub const GOOFY_M1501_LENGTH: f32 = 0.633_333_3;

/// (party D5) The GoofyShoot link pawn's TresRoot: one SPHERE of Size.x 220 at the root (GoofyShoot_Pawn
/// TresRoot CollisionShapesSrc [A]); the pawn's MaxStepHeight is 250 (TresCharMovement0 [A]). The pawn is
/// spawned at the player's root (feet) with collision handling 1 = AlwaysSpawn (0x14127A780 -> 0x1405D9130
/// -> 0x1440F9A30(.., 1, ..)) [C]; its own clips hold the root kh_sk 220 above the sockets (1636 / 1637:
/// kh_sk (0, 0, 220), X_Link0n (0, 220, 0) under its -90 deg X = 220 below [A]), so the riders stand on the
/// floor only with the actor resting on that sphere, 220 above the feet. [D: the lift out of the floor
/// (the walking movement's floor step, MaxStepHeight 250 > 220) is inferred, not read.]
pub const GSHOOT_ROOT_REST_Z: f32 = 220.0;

/// (party D5) One frame of the link pawn's own clip (tools/party_team_pack.js `writeGShootSockets`): in the
/// pawn ACTOR frame (UE cm, the mesh's default relative rotation yaw 270 applied, no extra mesh pitch):
/// `root` = the root bone kh_sk's track (the root motion source), `s1` / `q1` = the socket X_Link01_skt
/// (Sora) location / rotation (x, y, z, w), `s2` / `q2` = X_Link02_skt (Goofy), the drawn pose's kh_sk at
/// its reference pose (RootMotionRootLock RefPose) [A].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GShootFrame {
    pub root: [f32; 3],
    pub s1: [f32; 3],
    pub q1: [f32; 4],
    pub s2: [f32; 3],
    pub q2: [f32; 4],
}

/// (party D5) One of the link pawn's clips (o_at001 rt/1636..1639) sampled on its own frame grid.
#[derive(Clone, Copy, Debug)]
pub struct GShootClip {
    pub name: &'static str,
    pub length: f32,
    pub frames: &'static [GShootFrame],
}

#[cfg(kh_party_team_gshoot)]
mod gshoot_gen {
    use super::{GShootClip, GShootFrame};
    include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_team_gshoot.rs"));
}
/// (party D5) GShootStart / GSHootRise / GShootHit / GShootFinish (empty without the generated table:
/// the sockets then stay at the GShootStart pose, 220 below the actor).
#[cfg(kh_party_team_gshoot)]
pub use gshoot_gen::GSHOOT_LINK_CLIPS;
#[cfg(not(kh_party_team_gshoot))]
pub static GSHOOT_LINK_CLIPS: &[GShootClip] = &[];

/// (party D5) The link pawn's clip `clip` (0..=3) at time `t` (looped for GSHootRise: play mode 3 at
/// 0x1412DCE50 case 1 [C]; the others clamp at their end), linear between frames [A].
pub fn gshoot_frame(clip: usize, t: f32) -> GShootFrame {
    const REST: GShootFrame = GShootFrame { root: [0.0, 0.0, GSHOOT_ROOT_REST_Z], s1: [0.0, 0.0, -GSHOOT_ROOT_REST_Z], q1: [0.0, 0.0, 0.0, 1.0], s2: [0.0, 0.0, -GSHOOT_ROOT_REST_Z], q2: [0.0, 0.0, 0.0, 1.0] };
    let Some(c) = GSHOOT_LINK_CLIPS.get(clip).filter(|c| !c.frames.is_empty()) else { return REST };
    let n = c.frames.len();
    if n == 1 || c.length <= 0.0 {
        return c.frames[0];
    }
    let t = if clip == 1 { t.rem_euclid(c.length) } else { t.clamp(0.0, c.length) };
    let x = t / c.length * (n - 1) as f32;
    let k = (x.floor() as usize).min(n - 2);
    let f = (x - k as f32).clamp(0.0, 1.0);
    let (a, b) = (c.frames[k], c.frames[k + 1]);
    let l3 = |p: [f32; 3], q: [f32; 3]| [p[0] + (q[0] - p[0]) * f, p[1] + (q[1] - p[1]) * f, p[2] + (q[2] - p[2]) * f];
    let l4 = |p: [f32; 4], q: [f32; 4]| {
        let s = if p[0] * q[0] + p[1] * q[1] + p[2] * q[2] + p[3] * q[3] < 0.0 { -1.0 } else { 1.0 };
        let v = Quat::from_xyzw(p[0] + (s * q[0] - p[0]) * f, p[1] + (s * q[1] - p[1]) * f, p[2] + (s * q[2] - p[2]) * f, p[3] + (s * q[3] - p[3]) * f).normalize();
        [v.x, v.y, v.z, v.w]
    };
    GShootFrame { root: l3(a.root, b.root), s1: l3(a.s1, b.s1), q1: l4(a.q1, b.q1), s2: l3(a.s2, b.s2), q2: l4(a.q2, b.q2) }
}

/// (party D5) UE `FRotator(pitch, yaw, 0).Quaternion()` in the port's (UE) axes: yaw about +Z, pitch
/// nose-up about the lateral axis (= glam's rotation about Y by -pitch).
fn ue_quat(pitch: f32, yaw: f32) -> Quat {
    Quat::from_rotation_z(yaw.to_radians()) * Quat::from_rotation_y(-pitch.to_radians())
}

/// (party D5) Yaw and pitch (degrees) of a rotation's forward axis (UE `FQuat::Rotator` without the roll).
fn yaw_pitch(q: Quat) -> (f32, f32) {
    let f = q * Vec3::X;
    (f.y.atan2(f.x).to_degrees(), f.z.atan2(f.truncate().length()).to_degrees())
}

/// FUN_1412C93A0 (extra height 0, the normal mode) [C]: the link pawn's rise to T. With T above:
/// vz = sqrt(2 g dz), the flight time = the apex time vz / g, vxy = d / t. Else the 45 deg arc of
/// FUN_1412C90E0. Returns (velocity, flight time).
pub fn goofy_rise(start: Vec3, target: Vec3, g: f32) -> (Vec3, f32) {
    let d = (target - start).truncate();
    let dz = target.z - start.z;
    let g = g.max(0.001);
    if dz > 0.0 {
        let vz = (2.0 * g * dz).sqrt();
        let t = vz / g;
        let vh = d.length() / t;
        let dir = d.normalize_or_zero();
        (Vec3::new(dir.x * vh, dir.y * vh, vz), d.length() / vh.max(1e-6))
    } else {
        let (v, t, _) = goofy_arc(start, target, g, 45.0);
        (v, t)
    }
}

/// FUN_1412C90E0 [C]: the launch velocity reaching `target` from `start` at `pitch` degrees under
/// gravity `g`: k = g / (2 (d tan(pitch) - dz)) (k < 0: g / (2 d tan + 2000), then g / 2, then 1),
/// v = sqrt(k) d / cos; returns (velocity, the flight time d / (v cos), valid = k >= 0).
pub fn goofy_arc(start: Vec3, target: Vec3, g: f32, pitch: f32) -> (Vec3, f32, bool) {
    let g = if g <= 0.0 { 0.001 } else { g };
    let p = pitch.to_radians();
    let dxy = (target - start).truncate();
    let d = dxy.length();
    let h = p.tan() * d;
    let mut denom = (h - (target.z - start.z)) * 2.0;
    if denom == 0.0 {
        denom = 0.001;
    }
    let mut k = g / denom;
    let valid = k >= 0.0;
    if !valid {
        k = g / (h + h + 2000.0);
        if k < 0.0 {
            k = g * 0.5;
            if k < 0.0 {
                k = 1.0;
            }
        }
    }
    let c = p.cos();
    let v = k.sqrt() * (d / c);
    let dir = dxy.normalize_or_zero();
    let vc = c * v;
    (Vec3::new(dir.x * vc, dir.y * vc, p.sin() * v), d / vc.max(1e-6), valid)
}

/// The special mode's throw (0x1412DCE50 case 5, +0xBE4 set): pitches -28..-5 step 5 with
/// gravity x 3, the first valid speed in 1500..3200 cm/s, else the one nearest 2000 (the first
/// result is the fallback) [C]. Returns (velocity, flight time).
pub fn goofy_arc_search(start: Vec3, target: Vec3, g: f32) -> (Vec3, f32) {
    let mut best: Option<(Vec3, f32)> = None;
    let mut pitch = -28.0;
    while pitch <= -5.0 {
        let (v, t, valid) = goofy_arc(start, target, g * 3.0, pitch);
        if best.is_none() {
            best = Some((v, t));
        }
        if valid {
            let s = v.length();
            if (1500.0..=3200.0).contains(&s) {
                return (v, t);
            }
            if best.is_some_and(|(b, _)| (s - 2000.0).abs() < (b.length() - 2000.0).abs()) {
                best = Some((v, t));
            }
        }
        pitch += 5.0;
    }
    best.unwrap_or((Vec3::ZERO, 0.0))
}

/// Distance from `p` to the segment a..b.
fn seg_dist(a: Vec3, b: Vec3, p: Vec3) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 1e-9 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t - p).length()
}


/// Trinity Guard's link pawn body numbers (see the 193 arm) and clip lengths (p_ex001 / o_at001
/// 1630, 1635, 1631, 1627) [A, C].
pub const SGUARD_JUMP_Z: f32 = 420.0;
pub const SGUARD_TURN_RATE: f32 = 360.0;
pub const SGUARD_BASH_LENGTH: f32 = 1.133_333_3;
pub const SGUARD_BASH2_LENGTH: f32 = 1.633_333_3;
pub const SGUARD_AIR_BASH_LENGTH: f32 = 0.833_333_3;
pub const SGUARD_LAND_LENGTH: f32 = 0.333_333_3;

/// The ShieldGuardian_Pawn movement component as kh-core's CMC parameters: its root capsule
/// (TresRoot CAPSULE1 r 170, half height 190, centre 190) [A], MaxWalkSpeed 180, MaxStepHeight 47
/// [A], the Tres CMC constructor's AirControl 0.05, MaxAcceleration 2048, GroundFriction 8, the
/// jump-phase gravity scales 1 / 0 / 1 [C 0x140570EB0].
pub fn shield_params() -> kh_core::params::SoraParams {
    kh_core::params::SoraParams {
        capsule_radius: 170.0,
        capsule_half_height: 190.0,
        capsule_center_z: 190.0,
        speed_run: SHIELD_WALK_SPEED,
        floor_max_step_height: 47.0,
        air_control: 0.05,
        accel_default: 2048.0,
        ground_friction: 8.0,
        gravity_scale: 1.0,
        gravity_jump_up_scale: 1.0,
        gravity_jump_duration_time: 0.0,
        gravity_jump_duration_scale: 1.0,
        move_spawn_frame_skip: false,
        ..kh_core::params::SORA_PARAMS
    }
}


fn fwd(yaw: f32) -> Vec3 {
    let a = yaw.to_radians();
    Vec3::new(a.cos(), a.sin(), 0.0)
}

/// One member's link points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct LinkPoints {
    points: f32,
    timer: f32,
    threshold: f32,
}

/// A running team attack's link pawn.
#[derive(Clone, Debug, PartialEq)]
struct Run {
    kind: i32,
    t: f32,
    /// The link pawn's transform (feet, yaw).
    at: Vec3,
    yaw: f32,
    /// The point the attack aims at (lock-on target within range, else in front) and the targets.
    aim: Vec3,
    target: TargetId,
    members: Vec<MemberId>,
    sora_motion: Option<&'static str>,
    sora_action: Option<u8>,
    shots: Vec<TeamShot>,
    finished: bool,
    sent: u32,
    /// Per-attack counters: shots fired, meteors landed, the clip index of a sequence.
    n: u32,
    clip_t: f32,
    phase: u8,
    vz: f32,
    cancel: bool,
    /// Where the friends are when not at the link pawn (Goofy in flight).
    friend_at: Option<Vec3>,
    /// The participants were released (Disappear / Goofy's throw) while the link pawn lives on
    /// for its projectiles or Goofy's rush; the time of it; the link pawn's own end.
    released: bool,
    t_release: f32,
    done: bool,
    /// Goofy Bombardier: the link pawn's / Goofy's velocity, the flight time, the targets hit by
    /// the running attack sphere.
    vel: Vec3,
    flight: f32,
    hit_set: Vec<TargetId>,
    /// Trinity Guard: the link pawn's own movement component; the landing clip time left.
    body: Option<kh_core::cmc::Cmc>,
    landed: f32,
    /// (party D5) Goofy Bombardier: the link pawn's extra mesh pitch (vt+0xDC0 / vt+0xDC8, degrees), the
    /// root-motion sample last applied (phase, kh_sk translation in the actor frame), where X_Link01_skt
    /// holds Sora (feet, yaw), Goofy's yaw on X_Link02_skt.
    link_pitch: f32,
    rm_prev: Option<(u8, Vec3)>,
    sora_hold: Option<(Vec3, f32)>,
    friend_yaw: Option<f32>,
}

/// The friends' side of team attacks (one per party; contract D.3).
#[derive(Clone, Debug, Default)]
pub struct Team {
    sora: Option<(bool, bool, Vec3, f32)>,
    requests: Vec<FriendLinkRequest>,
    points: [LinkPoints; 2],
    offers: [Option<(i32, f32)>; 2],
    run: Option<Run>,
    /// The unison / fusion partner: member, kind, steps.
    unison: Option<(MemberId, u8, u32)>,
    /// The motion last given to each member (B plays a motion each time one is given).
    given: Vec<(MemberId, &'static str)>,
    rng: u32,
    /// Offers made so far (tests, traces).
    pub offers_made: u32,
    input: (Vec3, bool, bool),
    /// DEBUG: a command to offer at the next step whatever the points (`debug_offer`).
    forced: Option<i32>,
    /// The link pawns' projectiles (meteors, missiles), see proj.rs.
    pub projs: Vec<TeamProj>,
    /// Hits waiting for their time (delay s, shot): at most 4 leave per step (`FriendLinkView::shots`).
    pending: Vec<(f32, TeamShot)>,
    /// Seam handles of the team pawns (contract E.2: `SeamHandles::team()`).
    ids: Option<SeamHandles>,
    /// (D3, leader-agnostic) the player-controlled friend leading (None: Sora leads).
    leader: Option<MemberId>,
    /// (D4) The player controller's current target (vt+0x1690), from `Start` / `Input`.
    ctrl_target: TargetId,
    /// (D4) FlareForce's lock list (+0xCA0: target, missiles on it), emptied at each start.
    flare_locks: Vec<(TargetId, u32)>,
    /// (D4) Goofy's mesh pitch in the rush (FRGoofyShoot 0x1411CB010, see `mesh_pitch`): member,
    /// degrees (UE pitch of the extra mesh rotation in his actor frame).
    goofy_pitch: Option<(MemberId, f32)>,
    /// (party D5) Sora's pitch on the GoofyShoot link pawn's socket (see `sora_mesh_pitch`), degrees.
    sora_pitch: Option<f32>,
}

fn rand(rng: &mut u32) -> u32 {
    let mut x = (*rng).max(1);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *rng = x;
    x
}

/// Uniform integer in lo..=hi (the engine's `(r >> 8) x 2^-24 x n` pick; the meteors' `min(.., 5)`).
fn pick(rng: &mut u32, lo: i32, hi: i32) -> i32 {
    let n = (hi - lo + 1).max(1);
    lo + (((rand(rng) >> 8) as f32 / 16_777_216.0 * n as f32) as i32).min(n - 1)
}

/// A one-shot particle at a world point (the projectiles' hit / map-hit effects).
fn burst(ids: &mut SeamHandles, effects: &mut dyn Effects, name: &'static str, at: Vec3) -> u32 {
    let h = ids.effect();
    effects.spawn(h, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(at), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
    h.0
}

impl Team {
    pub fn new() -> Team {
        Team { rng: 0x9E37_79B9, ..Team::default() }
    }

    fn rand(&mut self) -> u32 {
        rand(&mut self.rng)
    }

    /// Uniform integer in lo..=hi (the engine's `(r >> 8) x 2^-24 x n` pick).
    fn pick(&mut self, lo: i32, hi: i32) -> i32 {
        pick(&mut self.rng, lo, hi)
    }

    /// From Sora's side (the host relays `Targets::friend_link`).
    pub fn request(&mut self, req: FriendLinkRequest) {
        if let FriendLinkRequest::Sora { gauge_full, in_battle, position, yaw } = req {
            self.sora = Some((gauge_full, in_battle, position, yaw));
        }
        self.requests.push(req);
    }

    /// (D3, leader-agnostic team attacks; contract D.6) The player-controlled leader when it is a
    /// friend (stream P calls it each step; None = Sora leads, the behaviour as before). The other
    /// friend's link points measure their distance to the LEADER (slot 1275 reads the player's
    /// pawn); the leader's own to Sora, his partner [D: the game has no friend leader]. The offers
    /// are unchanged: they go to the situation list of Sora's core, which the leader's Y executes.
    pub fn set_leader(&mut self, leader: Option<MemberId>) {
        self.leader = leader;
    }

    /// DEBUG (`team-gauge full`): both friends' link points to their threshold.
    pub fn debug_ready_all(&mut self) {
        for p in &mut self.points {
            if p.threshold <= 0.0 {
                p.threshold = LINK_POINT_MAX;
            }
            p.points = p.threshold;
            p.timer = 0.0;
        }
    }

    /// DEBUG (`team-gauge full KIND`): the friend that owns command `kind` (207 / 208 Donald, 193 / 195
    /// Goofy) offers it at the next step, in or out of battle.
    pub fn debug_offer(&mut self, kind: i32) {
        self.forced = Some(kind);
    }

    /// The live projectiles the host follows with their body effect (E.6 `publish_projectiles`):
    /// (effect handle, position, forward).
    pub fn projectile_taps(&self) -> impl Iterator<Item = (u32, Vec3, Vec3)> + '_ {
        self.projs.iter().filter(|p| p.alive && p.effect != 0).map(|p| (p.effect, p.position, p.forward))
    }

    /// The running team attack's command kind (0 = none).
    pub fn running(&self) -> i32 {
        self.run.as_ref().map_or(0, |r| r.kind)
    }

    /// The team-attack camera target (`m_pCameraTargetActor`): the link pawn / meteor target.
    pub fn camera_target(&self) -> Option<(Vec3, f32)> {
        self.run.as_ref().map(|r| (if r.kind == 207 { r.aim } else { r.at }, r.yaw))
    }

    /// `GetSpecificActionAccompanyPawn(0)`: the link pawn's ACTOR location and yaw: the base of the
    /// goofy_shoot / ShieldGuardian cameras (`BP_SettingTresCamera(Center)`). (D4) The actor
    /// location is the root's = Sora's feet at the spawn (see `LINK_PAWN_ORIGIN_Z`), which the
    /// cameras take as is (0x140621B90 -> 0x140624260 -> 0x140621E00: root +0x2B0) [C].
    pub fn accompany(&self) -> Option<(Vec3, f32)> {
        self.run.as_ref().map(|r| (r.at + Vec3::Z * LINK_PAWN_ORIGIN_Z, r.yaw))
    }

    /// (D4) The extra pitch of a member's MESH in its FR state, degrees: a UE pitch about the actor's
    /// lateral axis, applied to the mesh in the actor frame on top of its default relative rotation
    /// (0x1405AF2A0: relative = slerp(relative, Quat(R(pitch, 0, 0)) x default, alpha)). Goofy's in
    /// Goofy Bombardier's rush and M_1501; None = no extra rotation (the state's End 0x1411C09A0
    /// resets it at once: vt+0xDC8 alpha 1) [C].
    pub fn mesh_pitch(&self, id: MemberId) -> Option<f32> {
        self.goofy_pitch.filter(|g| g.0 == id).map(|g| g.1)
    }

    /// (party D5) Sora's pitch (UE pitch about his actor's lateral axis, degrees) while Goofy Bombardier's
    /// link pawn carries him on X_Link01_skt: the attachment snaps his root's rotation to the socket's
    /// (0x1405A3FA0, rules DAT_148750ED4 = SnapToTarget) and the socket turns with the pawn's mesh pitch
    /// (0x1412DCE50 cases 2..4) [C]; the seam's `FriendLinkView::sora` carries the yaw, this the pitch.
    /// After the release it eases back at 12 dt (his state tick's vt+0xDC8 / vt+0xDB8 idiom [C, rate D]).
    pub fn sora_mesh_pitch(&self) -> Option<f32> {
        self.sora_pitch
    }

    fn pitch_toward(&mut self, id: MemberId, target: f32, alpha: f32) {
        let cur = self.goofy_pitch.filter(|g| g.0 == id).map_or(0.0, |g| g.1);
        let d = crate::pawn::wrap180(target - cur);
        self.goofy_pitch = Some((id, cur + d * alpha.clamp(0.0, 1.0)));
    }

    /// The rows Sora's `Targets::friends` answers (Donald / Goofy in party-slot order).
    pub fn friend_rows(&self, party: &dyn TeamParty, out: &mut Vec<Friend>) {
        let mut members = Vec::new();
        party.team_members(&mut members);
        for m in members {
            let slot = m.chara.party_index().min(1);
            let mut link = FriendLinkView::default();
            if let Some(r) = self.run.as_ref().filter(|r| r.members.first() == Some(&m.id)) {
                let mut shots = [None; 4];
                for (s, shot) in shots.iter_mut().zip(r.shots.iter()) {
                    *s = Some(*shot);
                }
                link = FriendLinkView { kind: r.kind, phase: r.phase, finished: r.finished, sora: if r.released { None } else { r.sora_hold.or(Some((r.at, r.yaw))) }, sora_motion: r.sora_motion, camera_target: self.camera_target(), accompany: self.accompany(), sora_action: r.sora_action, shots };
            }
            out.push(Friend { uid: m.chara.uid(), position: m.position, yaw: m.yaw, ko: m.ko, combi_time: m.combi_time, combi_up: m.combi_up, offer: self.offers[slot], link });
        }
    }

    /// Gives `motion` to `id` only when it changed (B plays a motion each time one is given).
    fn drive(&mut self, party: &mut dyn TeamParty, id: MemberId, at: Vec3, yaw: f32, motion: Option<&'static str>) {
        let mut give = None;
        if let Some(m) = motion {
            match self.given.iter_mut().find(|g| g.0 == id) {
                Some(g) if g.1 == m => {}
                Some(g) => {
                    g.1 = m;
                    give = Some((m, -1.0, false));
                }
                None => {
                    self.given.push((id, m));
                    give = Some((m, -1.0, false));
                }
            }
        }
        party.fr_drive(id, FrDrive { transform: Some((at, yaw)), motion: give, visible: true, weapon_visible: true, invincible: true, collision: false });
    }

    fn enemies(host: &PartyHost) -> Vec<TargetInfo> {
        let mut all = Vec::new();
        host.targets.enumerate(&mut all);
        all.retain(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::HITTABLE) && t.flags.has(TargetFlags::ENEMY));
        all
    }

    /// (D4) The player pawn's point of FUN_1412AB570 (vt+0x25B0 0x1414012C0 = the player
    /// controller's pawn): his "center" bone, see `SORA_CENTER_BONE_Z`; `fallback` before Sora's
    /// first report.
    fn master_point(&self, fallback: Vec3) -> Vec3 {
        self.sora.map_or(fallback, |s| s.2) + Vec3::Z * SORA_CENTER_BONE_Z
    }

    /// (D4) FUN_1412AC290(pawn, point, b3D 1, sight, max) [C]: the player controller's current
    /// target (vt+0x948 0x141401270 -> controller vt+0x1690 = ctrl+0x994, soft or locked) as it
    /// is, with no range test; only without one, the hostile `ATresCharPawnBase`s
    /// (FUN_140439810 relation 2 = the enemy team's list) that have a lock-on component
    /// (`m_FlowTarget` +0x888 -> LOCKABLE) and pass vt+0x698, by the 3-D distance from `point`
    /// to that component's location (vt+0xB88 -> `center`): the nearest (`<=`: the later of
    /// equal ones), skipping one whose line from `point` is blocked when `sight` (FUN_143FD41F0
    /// channel 3; here the map), none at or past `max`.
    fn pawn_target(ctrl: TargetId, host: &PartyHost, point: Vec3, sight: bool, max: f32) -> Option<TargetInfo> {
        let mut all = Vec::new();
        host.targets.enumerate(&mut all);
        if ctrl != TargetId::NONE {
            if let Some(t) = all.iter().find(|t| t.id == ctrl && t.flags.has(TargetFlags::ALIVE)) {
                return Some(*t);
            }
        }
        let mut best: Option<TargetInfo> = None;
        let mut d_best = 3.4e38_f32;
        for t in all.iter().filter(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::ENEMY | TargetFlags::LOCKABLE)) {
            let d = (t.center - point).length();
            if d <= d_best {
                if sight && host.world.line_trace(point, t.center).is_some() {
                    continue;
                }
                best = Some(*t);
                d_best = d;
            }
        }
        best.filter(|_| d_best < max)
    }

    /// (D4) FUN_1412E43E0 [C], a missile's own target, chosen when its fire curve ends (state 3
    /// 0x1412E59A0, after the up point): info +0x24 cleared and flag bit 0 off; over the hostile
    /// list (FUN_140439810 relation 2, vt+0x698), score = (2 + 2 x dot(the missile's velocity
    /// direction (vt+0x2B8 = GetVelocity, normalized), the flat unit direction from the missile
    /// to the candidate's lock point (vt+0x6E0)))  clamped to >= 0.01, times the weight
    /// 1 - n / `m_nLockOnMaxNum` (n = the missiles that already took it, list +0xCA0; >= 0.01),
    /// times 5 for the pawn's main target (+0xC50, its point then +0xC70); the strictly best
    /// above 1e-8 wins (the first of equals). Its count goes up; a target taken for the first
    /// time sets flag bit 0 (the fast homing law of state 5). No range limit. Returns (target,
    /// first on it).
    fn flare_pick(p: &TeamProj, enemies: &[TargetInfo], main: Option<&TargetInfo>, locks: &mut Vec<(TargetId, u32)>) -> Option<(TargetId, bool)> {
        // Before its Fire (vt+0x1270) the movement still has its spawn velocity: local (1, 0, 0)
        // along the facing (see state 3), so the direction is the spawn facing.
        let v = p.forward.try_normalize().unwrap_or(Vec3::ZERO);
        let mut best: Option<TargetId> = None;
        let mut score_best = 1e-8_f32;
        for t in enemies {
            let mut w = 1.0;
            if let Some((_, n)) = locks.iter().find(|l| l.0 == t.id) {
                w = (1.0 - *n as f32 / FLARE_LOCK_MAX as f32).max(0.01);
            }
            let mut at = t.center;
            if main.is_some_and(|m| m.id == t.id) {
                at = main.map_or(at, |m| m.center);
                w *= 5.0;
            }
            let d = (at - p.position).truncate().normalize_or_zero();
            let s = (2.0 * (v.x * d.x + v.y * d.y) + 2.0).max(0.01);
            if score_best < s * w {
                best = Some(t.id);
                score_best = s * w;
            }
        }
        let id = best?;
        match locks.iter_mut().find(|l| l.0 == id) {
            Some(l) => {
                l.1 += 1;
                Some((id, false))
            }
            None => {
                locks.push((id, 1));
                Some((id, true))
            }
        }
    }

    /// One fixed step, after `Party::step`.
    pub fn step(&mut self, dt: f32, party: &mut dyn TeamParty, host: &mut PartyHost) -> TeamOutput {
        let mut out = TeamOutput::default();
        let mut members = Vec::new();
        party.team_members(&mut members);
        self.offers = [None; 2];
        let requests = std::mem::take(&mut self.requests);
        for req in requests {
            match req {
                FriendLinkRequest::Start { kind, target } => {
                    self.ctrl_target = target;
                    self.start(kind, target, &members, party, host, &mut out)
                }
                FriendLinkRequest::End { kind, .. } => {
                    // Sora's FR state ending does not destroy a link pawn that already let him go
                    // (it lives on for its missiles / Goofy's rush, see Run::released) [C].
                    if self.running() == kind && !self.run.as_ref().is_some_and(|r| r.released && !r.done) {
                        self.stop(party, host, &mut out);
                    }
                }
                FriendLinkRequest::Input { stick, attack, jump, target, .. } => {
                    self.input = (stick, attack, jump);
                    self.ctrl_target = target;
                }
                FriendLinkRequest::Action { action: 2, .. } => {
                    if let Some(r) = &mut self.run {
                        r.cancel = true;
                    }
                }
                FriendLinkRequest::Unison { uid, kind, .. } => {
                    if let Some(m) = members.iter().find(|m| m.chara.uid() == uid && !m.ko) {
                        let state = if uid == 11 { fr_id::DONALD_SORA } else { fr_id::GOOFY_SORA };
                        party.team_command(m.id, MemberCommand::EnterFrState { state });
                        self.given.retain(|g| g.0 != m.id);
                        self.unison = Some((m.id, kind, 0));
                    }
                }
                _ => {}
            }
        }
        self.unison_step(&members, party);
        if self.run.is_some() {
            self.run_step(dt, &members, party, host, &mut out);
        } else {
            let enemies = Self::enemies(host);
            self.link_points(dt, &members, &enemies, &mut out);
        }
        out
    }

    /// `TresNpcState_n_ex001_FRDonaldSora` 0x1411CACA0 / `n_ex002_FRGoofySora` 0x1411CB5D0 [C]: on
    /// Sora's "vfx_root" (AttachToActor vt+0x23A8 [C]; the socket is on kh_sk at location 0 with rotation
    /// (90, 0, -90) and Sora's mesh sits at his origin with yaw 270 [A]: his actor transform; kh_sk's own
    /// animated offset in the DoU / GooF clips is not followed [D]), invincible, the clip of the kind; at its end
    /// detach and finish.
    fn unison_step(&mut self, members: &[TeamMember], party: &mut dyn TeamParty) {
        let Some((id, kind, n)) = self.unison else { return };
        let Some(m) = members.iter().find(|m| m.id == id) else {
            self.unison = None;
            return;
        };
        let clip = match kind {
            0x3F => "DoUFire",
            0x40 => "DoUBlizzard",
            0x41 => "DoUThunder",
            0x42 => "GooFSpin",
            _ => "GooFRocket",
        };
        let (at, yaw) = self.sora.map_or((m.position, m.yaw), |s| (s.2, s.3));
        if n > 2 && matches!(m.state, NpcStateId::Fr(_)) && m.clip == clip && m.clip_ended {
            party.team_command(id, MemberCommand::Stop);
            self.given.retain(|g| g.0 != id);
            self.unison = None;
            return;
        }
        if n > 120 && !matches!(m.state, NpcStateId::Fr(_)) {
            self.unison = None;
            return;
        }
        if !matches!(m.state, NpcStateId::Fr(_)) {
            // Not in yet (an AI command of the same frame may have taken the member's command slot):
            // ask again.
            let state = if m.chara == Chara::Donald { fr_id::DONALD_SORA } else { fr_id::GOOFY_SORA };
            party.team_command(id, MemberCommand::EnterFrState { state });
            self.given.retain(|g| g.0 != id);
        }
        self.drive(party, id, at, yaw, Some(clip));
        self.unison = Some((id, kind, n + 1));
    }

    /// One missile of Flare Force for one tick: the machine of `ATresFriendLinkPawnFlareForce`'s
    /// tick override 0x1412E72B0 over `m_MissileInfos[i]` (the countdown +0x0C drops by dt in every
    /// state) [C]:
    /// - 1 (0x1412E5110): hidden at base + R(yaw + m_fAppearYaw) x m_vAppearOffset; at 0: shown,
    ///   rate 10, turn 2 pi, countdown = m_fFireTimer - the pawn timer -> 2.
    /// - 2 (0x1412E6740): scale + m_fAppearScale x dt up to 1; offset + AppearCurve(t) (t up to
    ///   the curve end); at 0: scale 1, countdown = the fire curve's length, t = 0 -> 3.
    /// - 3 (0x1412E59A0): offset + FireCurve(t); at 0: the up point = base + R x m_vUpOffset +
    ///   (rand 0..7 - 3) x 8 z, the launch direction to the target + 300 z, fired -> 4.
    /// - 4 (0x1412E6530): homing (10, 100 deg/s, 100..1000, 0.5, 100) to the up point; < 100: aim
    ///   at the pawn's target (or the missile's own) + 300 z, no target = flag 1 -> 5.
    /// - 5 (0x1412E6290): homing (10, m_fHomingAngle, 0.2 / 0.7 x max with a target, 0.7 / 1.0
    ///   without, cos 0.3 / 0) to the aim; <= m_fHomingRange 25: explode; the aim follows the
    ///   target when it moved more than m_fHomingUpdateRange; after m_fLifeTime 10 s: explode.
    #[allow(clippy::too_many_arguments)]
    fn flare_missile(p: &mut TeamProj, dt: f32, base: Vec3, yaw: f32, pawn_t: f32, main: Option<&TargetInfo>, own: Option<&TargetInfo>, rng: &mut u32, ids: &mut SeamHandles, host: &mut PartyHost) -> MissileOut {
        let info = MISSILES[p.index];
        let ryaw = yaw + info.0;
        p.countdown -= dt;
        match p.mstate {
            1 => {
                p.position = base + rot_yaw(FLARE_APPEAR_OFFSET, ryaw);
                if p.countdown <= 0.0 {
                    p.visible = true;
                    p.show_body(ids, host.effects);
                    (p.rate, p.turn) = (10.0, std::f32::consts::TAU);
                    (p.mstate, p.countdown) = (2, info.2 - pawn_t);
                }
            }
            2 => {
                p.scale = (p.scale + FLARE_APPEAR_SCALE * dt).min(1.0);
                p.curve_t = (p.curve_t + dt).min(FLARE_APPEAR_END);
                let wobble = Vec3::new(rich_eval(&FLARE_APPEAR_X, p.curve_t), 0.0, rich_eval(&FLARE_APPEAR_Z, p.curve_t));
                p.position = base + rot_yaw(FLARE_APPEAR_OFFSET + wobble, ryaw);
                if p.countdown <= 0.0 {
                    p.scale = 1.0;
                    (p.mstate, p.countdown, p.curve_t) = (3, FLARE_FIRE_END, 0.0);
                }
            }
            3 => {
                p.curve_t += dt;
                let recoil = Vec3::new(rich_eval(&FLARE_FIRE_X, p.curve_t), 0.0, rich_eval(&FLARE_FIRE_Z, p.curve_t));
                p.position = base + rot_yaw(FLARE_APPEAR_OFFSET + recoil, ryaw);
                if p.countdown <= 0.0 {
                    let k = pick(rng, 0, 7);
                    let up = base + rot_yaw(FLARE_UP_OFFSET, ryaw) + Vec3::Z * ((k - 3) as f32 * 8.0);
                    let aim = match (own, main) {
                        (Some(o), Some(m)) if o.id != m.id => o.center,
                        (_, Some(m)) => m.center,
                        (Some(o), None) => o.center,
                        (None, None) => base + rot_yaw(Vec3::X * FINISH_HOPE_RANGE, yaw),
                    };
                    p.launch_dir = (aim + Vec3::Z * 300.0 - up).try_normalize().unwrap_or(p.forward);
                    p.target_point = Some(up);
                    // Fire (vt+0x1270): the movement starts; UE's projectile movement keeps its
                    // default local velocity (1, 0, 0) with no InitialSpeed: 1 cm/s along the
                    // missile's facing [A: FlareForce_Proj ProjectileComp has no InitialSpeed].
                    p.velocity = p.forward;
                    (p.mstate, p.countdown, p.flying) = (4, 0.0, false);
                    let (fy, fp) = (p.forward.y.atan2(p.forward.x).to_degrees(), p.forward.z.clamp(-1.0, 1.0).asin().to_degrees());
                    host.effects.spawn(ids.effect(), &EffectSpawn { id: EffectId(kh_core::magic::projectile::FIRE_EFFECTS_ID), name: ProjKind::Missile.class(), at: Attach::World(p.position), rotation: Vec3::new(fp, fy, 0.0), scale: 1.0, persistent: false });
                }
            }
            4 => {
                let d = p.flare_homing(dt, 10.0, 100f32.to_radians(), 100.0, 1000.0, 0.5, 100.0);
                if d < 100.0 {
                    let mut aim = main.map_or(p.target_point.unwrap_or(p.position), |m| m.center);
                    (p.mstate, p.countdown) = (5, 0.0);
                    match own {
                        Some(o) => {
                            if main.is_none_or(|m| m.id != o.id) {
                                aim = o.center;
                            }
                            aim.z += 300.0;
                        }
                        None => p.no_target = true,
                    }
                    p.target_point = Some(aim);
                }
            }
            5 => {
                let (lo, hi, c) = if p.no_target { (FLARE_SPEED * 0.7, FLARE_SPEED, 0.0) } else { (FLARE_SPEED * 0.2, FLARE_SPEED * 0.7, 0.3) };
                let d = p.flare_homing(dt, 10.0, FLARE_HOMING_ANGLE.to_radians(), lo, hi, c, 0.0);
                if d <= FLARE_HOMING_RANGE {
                    p.mstate = 10;
                } else {
                    match own {
                        None => p.target_lost = true,
                        Some(o) => {
                            let new = if main.is_some_and(|m| m.id == o.id) { main.map_or(o.center, |m| m.center) } else { o.center };
                            if (new - p.target_point.unwrap_or(new)).length() > FLARE_HOMING_UPDATE {
                                p.target_point = Some(new);
                            }
                        }
                    }
                    if p.countdown <= -FLARE_LIFE_TIME {
                        p.mstate = 10;
                    }
                }
            }
            _ => {}
        }
        if p.mstate == 10 {
            return MissileOut::Explode(p.position);
        }
        if p.mstate >= 4 {
            let enemies = Self::enemies(host);
            match p.sweep_move(dt, host.world, &enemies) {
                Some(Contact::Pawn { at, .. }) => return MissileOut::Hit(at),
                // [D] a map contact explodes it (the projectile's world collision was not read).
                Some(Contact::Map { at }) | Some(Contact::Timeout { at }) => return MissileOut::Explode(at),
                None => {}
            }
        }
        MissileOut::None
    }

    /// The NPC link points (see the module text) and the offers.
    fn link_points(&mut self, dt: f32, members: &[TeamMember], enemies: &[TargetInfo], out: &mut TeamOutput) {
        if let Some(kind) = self.forced.take() {
            let owner = if DONALD_COMMANDS.contains(&kind) { Chara::Donald } else { Chara::Goofy };
            if let Some(m) = members.iter().find(|m| m.chara == owner && !m.ko) {
                self.offers[m.chara.party_index().min(1)] = Some((kind, -1.0));
                self.offers_made += 1;
                out.events.push(TeamEvent::Offered { member: m.id, kind, time: -1.0 });
                return;
            }
        }
        let Some((gauge_full, in_battle, sora_at, _)) = self.sora else { return };
        let donald_ok = members.iter().any(|m| m.chara == Chara::Donald && !m.ko);
        for m in members {
            let slot = m.chara.party_index().min(1);
            // Slot 1267: 0 (reset) unless in battle with the player's gauge full; -1 while blocked.
            let ready = gauge_full && in_battle && !m.ko && !matches!(m.state, NpcStateId::Fr(_) | NpcStateId::Die | NpcStateId::Damage);
            if !(gauge_full && in_battle) {
                // Slot 1278: reset.
                self.points[slot] = LinkPoints { points: 0.0, timer: LINK_POINT_INTERVAL, threshold: self.points[slot].threshold };
                continue;
            }
            if !ready {
                continue;
            }
            if self.points[slot].threshold <= 0.0 {
                // Slot 1279: (80..120 % - 0.1 x COMBI_UP) x max, rounded, at least 1.
                let pct = self.pick(0, 40) + 80;
                let th = ((pct as f32 * 0.01 - m.combi_up as f32 * 0.1) * LINK_POINT_MAX).round().max(1.0);
                self.points[slot].threshold = th;
            }
            let p = self.points[slot];
            if p.points < p.threshold {
                let timer = (p.timer - dt).clamp(0.0, LINK_POINT_INTERVAL);
                self.points[slot].timer = timer;
                if timer > 0.0 {
                    continue;
                }
                // Slot 1275 0x141261B30: by Sora's distance, and fewer while the friend is more
                // than 1000 cm from its target (min of vt+0x27C8 0x1412619A0 = the actor distance
                // and vt+0x27D0 0x141261780 = the distance to its target point) [C]; its target
                // (vt+0x28E8 0x141264840) is taken as the nearest enemy [D: not followed].
                let player_at = match self.leader {
                    Some(l) if l != m.id => members.iter().find(|x| x.id == l).map_or(sora_at, |x| x.position),
                    _ => sora_at,
                };
                let d2 = (m.position - player_at).length_squared();
                let far = enemies.iter().map(|t| (t.position - m.position).length().min((t.center - m.position).length())).fold(f32::MAX, f32::min);
                let (lo, hi) = if far > 1000.0 && far < f32::MAX {
                    if d2 > 1.0e6 {
                        (2, 4)
                    } else if d2 > 250_000.0 {
                        (4, 6)
                    } else {
                        (8, 12)
                    }
                } else if d2 > 1.0e6 {
                    (4, 12)
                } else if d2 > 250_000.0 {
                    (8, 16)
                } else {
                    (10, 20)
                };
                let add = self.pick(lo, hi) as f32;
                let q = &mut self.points[slot];
                q.points = (q.points + add).clamp(0.0, LINK_POINT_MAX.max(q.threshold));
                q.timer = LINK_POINT_INTERVAL;
                continue;
            }
            // Slot 1286 -> 1219: a kind at random; the player's NotifySetActionCommand; all reset.
            let list: Vec<i32> = match m.chara {
                Chara::Donald => DONALD_COMMANDS.to_vec(),
                Chara::Goofy => GOOFY_COMMANDS.iter().copied().filter(|k| *k != 193 || donald_ok).collect(),
            };
            if list.is_empty() {
                continue;
            }
            let kind = list[(self.rand() as usize >> 8) % list.len()];
            self.offers[slot] = Some((kind, -1.0));
            self.offers_made += 1;
            out.events.push(TeamEvent::Offered { member: m.id, kind, time: -1.0 });
            for q in &mut self.points {
                *q = LinkPoints { points: 0.0, timer: LINK_POINT_INTERVAL, threshold: 0.0 };
            }
            break;
        }
    }

    fn start(&mut self, kind: i32, target: TargetId, members: &[TeamMember], party: &mut dyn TeamParty, host: &PartyHost, out: &mut TeamOutput) {
        let Some((_, _, at, yaw)) = self.sora else { return };
        let (chars, state): (&[Chara], u16) = match kind {
            207 => (&[Chara::Donald], fr_id::CALL_METEOR),
            208 => (&[Chara::Donald], fr_id::FLARE_FORCE),
            195 => (&[Chara::Goofy], fr_id::GOOFY_SHOOT),
            193 => (&[Chara::Donald, Chara::Goofy], fr_id::SHIELD_GUARDIAN),
            _ => return,
        };
        let ids: Vec<MemberId> = chars.iter().filter_map(|c| members.iter().find(|m| m.chara == *c && !m.ko).map(|m| m.id)).collect();
        if ids.len() != chars.len() {
            return;
        }
        for id in &ids {
            party.team_command(*id, MemberCommand::EnterFrState { state });
            self.given.retain(|g| g.0 != *id);
        }
        // (D4) The aim, per link pawn [C]:
        // - 207 CallMeteor Finish 0x1412CD0E0: FUN_1412AC290(Sora's point, 3-D, sight,
        //   m_fFinishMaxRange 10000): its location (vt+0xB88) and +0xC6A = 1; else the pawn's
        //   location + m_fFinishHopeRange along its facing (the +0xC6A = 0 branch).
        // - 208 FlareForce: the pawn's main target +0xC50 / +0xC70 = FUN_1412AC290(.., no sight,
        //   1e6), re-read every tick (0x1412E72B0); the Appear faces it.
        // - 195 GoofyShoot Appear begin 0x1412CC160: the controller's target (vt+0x1690) only, T =
        //   its location + 1000 z; else pawn + fwd x 400 + 1000 z (no search).
        // - 193 ShieldGuardian: the lock-on target within m_fFinishHopeRange, else the nearest
        //   one within it, else ahead (D3, unchanged) [D].
        let enemies = Self::enemies(host);
        let point = self.master_point(at);
        let aimed: Option<TargetInfo> = match kind {
            207 => Self::pawn_target(target, host, point, true, METEOR_FINISH_MAX_RANGE),
            208 => Self::pawn_target(target, host, point, false, 1.0e6),
            195 => enemies.iter().find(|t| t.id == target).copied(),
            _ => {
                let in_range = |t: &&TargetInfo| (t.position - at).truncate().length() <= FINISH_HOPE_RANGE;
                enemies.iter().find(|t| t.id == target).filter(in_range).or_else(|| enemies.iter().filter(in_range).min_by(|a, b| (a.position - at).length().total_cmp(&(b.position - at).length()))).copied()
            }
        };
        let (aim, target) = match aimed {
            Some(t) => (t.center, t.id),
            None => (at + fwd(yaw) * FINISH_HOPE_RANGE, TargetId::NONE),
        };
        self.flare_locks.clear();
        let face = if aimed.is_some() { let d = aim - at; d.y.atan2(d.x).to_degrees() } else { yaw };
        self.run = Some(Run { kind, t: 0.0, at, yaw: face, aim, target, members: ids, sora_motion: None, sora_action: None, shots: Vec::new(), finished: false, sent: 0, n: 0, clip_t: 0.0, phase: 0, vz: 0.0, cancel: false, friend_at: None, released: false, t_release: 0.0, done: false, vel: Vec3::ZERO, flight: 0.0, hit_set: Vec::new(), body: None, landed: 0.0, link_pitch: 0.0, rm_prev: None, sora_hold: None, friend_yaw: None });
        if kind == 195 {
            // goofy_shoot camera: the generic (Hanyo) mode (ActorSpecificAction 5) [D: sender not read].
            out.events.push(TeamEvent::ActorSpecificAction { action: 5 });
        }
        out.events.push(TeamEvent::Started { kind });
    }

    fn stop(&mut self, party: &mut dyn TeamParty, host: &mut PartyHost, out: &mut TeamOutput) {
        for p in &mut self.projs {
            p.kill(host.effects);
        }
        self.projs.clear();
        self.pending.clear();
        // (D4) Goofy's FRGoofyShoot End 0x1411C09A0: vt+0xDC8(1) = the mesh's default at once [C].
        self.goofy_pitch = None;
        self.sora_pitch = None;
        if let Some(r) = self.run.take() {
            for id in &r.members {
                party.team_command(*id, MemberCommand::Stop);
                self.given.retain(|g| g.0 != *id);
            }
            out.events.push(TeamEvent::Ended { kind: r.kind });
        }
    }

    fn run_step(&mut self, dt: f32, members: &[TeamMember], party: &mut dyn TeamParty, host: &mut PartyHost, out: &mut TeamOutput) {
        let Some(mut r) = self.run.take() else { return };
        r.shots.clear();
        r.sora_action = None;
        let t0 = r.t;
        r.t += dt;
        let once = |r: &mut Run, bit: u32| {
            let first = r.sent & (1 << bit) == 0;
            r.sent |= 1 << bit;
            first
        };
        let friend_motion: Option<&'static str>;
        match r.kind {
            // Donald Meteor (ATresFriendLinkPawnCallMeteor) on one timer (+0xC64, reset only by
            // the Appear's step 0 and set to 2.0 by the Disappear) [C]:
            // - Appear 0x1412D5680: step 1 = the participants play CMeteoFinish on the link pawn,
            //   78, the 40 meteors spawn hidden at 0.01 scale (FUN_1412CAF60 with 0.4); step 2 =
            //   each meteor shows at start timer x `m_fWhiteOutTimer` with its m_FireEffect
            //   attached and grows by `m_fAppearScale` / s; ends when a slave pawn's motion ends.
            // - Finish 0x1412CD0E0 + FUN_1412E3900: the target point, 79, every meteor hidden and
            //   placed above it; tick 0x1412DA0B0: each meteor shows and fires at its fire time,
            //   `m_MeteorExpProj` at the target at `m_fWhiteOutTimer`, Disappear at `m_fWhiteInTimer`.
            // - Disappear 0x1412CCB90 (meteors destroyed, 80) + tick 0x1412D9910 (2 s).
            207 => {
                let ids = self.ids.get_or_insert(SeamHandles::team());
                if once(&mut r, 0) {
                    r.sora_action = Some(78);
                    r.sora_motion = Some("CMeteoFinish");
                    let base = r.at + Vec3::Z * (LINK_PAWN_ORIGIN_Z + METEOR_HEIGHT);
                    for (i, m) in METEORS.iter().enumerate() {
                        let offset = if i == 0 {
                            rot_vector(80.0, r.yaw) * 150.0
                        } else {
                            let k = pick(&mut self.rng, 0, 5);
                            rot_vector((k - 1) as f32 * 7.0, m.0) * METEOR_RANGE * m.2
                        };
                        let mut p = TeamProj::new(ProjKind::Meteor, i, base + offset, -Vec3::Z);
                        // (party D5) FUN_1412CAF60 after the spawn, in its draw order: the spin
                        // SetActorRotation(R(pick 0..12 x 30, 0, pick 0..12 x 30)) (world yaw 0), the
                        // info's spawn location (+0x14), shake off (flag 8 cleared), and the shake curve
                        // time (+0x24) uniform in m_ShakeCurveData's time range [C].
                        let spin_pitch = pick(&mut self.rng, 0, 12) as f32 * 30.0;
                        p.spin = Some((spin_pitch, pick(&mut self.rng, 0, 12) as f32 * 30.0));
                        p.forward = rot_vector(spin_pitch, 0.0);
                        p.base = p.position;
                        p.shake = false;
                        p.curve_t = if (METEOR_SHAKE_MAX - METEOR_SHAKE_MIN).abs() <= 1e-8 { 0.0 } else { (rand(&mut self.rng) >> 8) as f32 / 16_777_216.0 * (METEOR_SHAKE_MAX - METEOR_SHAKE_MIN) + METEOR_SHAKE_MIN };
                        p.appear_at = m.3 * METEOR_WHITE_OUT;
                        p.fire_at = ((m.3 - 0.2) * (1.0 - 0.4) * 3.333_333_3 + 0.4) * METEOR_WHITE_OUT;
                        self.projs.push(p);
                    }
                }
                friend_motion = Some("CMeteoFinish");
                match r.phase {
                    0 => {
                        for p in &mut self.projs {
                            if !p.visible && p.appear_at <= r.t {
                                p.visible = true;
                                p.appear_effect = burst(ids, host.effects, "vn_ex001_fcm_spn00", p.position);
                                // (party D5) the Appear tick 0x1412D5680 at the show: flags |= 2 (fade in)
                                // | 8 (shake) [C].
                                p.shake = true;
                            }
                            if p.visible {
                                p.scale = (p.scale + METEOR_APPEAR_SCALE * dt).min(1.0);
                            }
                        }
                        let ended = r.t > 0.2 && members.iter().any(|m| r.members.contains(&m.id) && m.clip == "CMeteoFinish" && m.clip_ended);
                        if ended || r.t >= CMETEO_LENGTH {
                            r.phase = 1;
                            r.sora_action = Some(79);
                            // The target point T, the source S above it (+ trace sphere + 5 +
                            // `m_fFinishCameraOffset`, traced up), the yaw from the link pawn to T.
                            let t_point = r.aim;
                            let up = t_point + Vec3::Z * (METEOR_TRACE + 5.0 + METEOR_CAMERA_OFFSET);
                            let s_point = host.world.line_trace(t_point, up).map_or(up, |h| h.position);
                            let d = t_point - (r.at + Vec3::Z * LINK_PAWN_ORIGIN_Z);
                            let yaw = if d.truncate().length() > 1e-3 { d.y.atan2(d.x).to_degrees() } else { r.yaw };
                            let dz = s_point.z - t_point.z;
                            let ratio = if dz < METEOR_CAMERA_OFFSET { 1.0 - dz / METEOR_CAMERA_OFFSET } else { 1.0 };
                            let from = s_point - Vec3::Z * 100.0;
                            for p in &mut self.projs {
                                let k = pick(&mut self.rng, 0, 5);
                                let m = METEORS[p.index];
                                let dir = rot_vector(-2.0 * k as f32, yaw + m.0);
                                let length = (m.2 - m.1) * ratio + m.1;
                                let start = from + dir * METEOR_FINISH_RANGE * length;
                                let start = host.world.line_trace(from, start).map_or(start, |h| h.position);
                                let end = t_point + dir * METEOR_FINISH_RANGE * m.2;
                                p.position = start;
                                p.forward = (end - start).try_normalize().unwrap_or(-Vec3::Z);
                                // (party D5) 0x1412E3900: SetActorLocationAndRotation(start, Rotation(dir)),
                                // flag 8 cleared: no shake, the spin's roll gone [C].
                                (p.shake, p.spin) = (false, None);
                                (p.speed, p.accel, p.max_speed) = (m.4 * METEOR_SPEED, 2400.0, 4000.0);
                                p.visible = false;
                                if p.appear_effect != 0 {
                                    host.effects.stop(kh_core::host::EffectHandle(p.appear_effect), true);
                                    p.appear_effect = 0;
                                }
                            }
                        }
                    }
                    1 => {
                        for p in &mut self.projs {
                            if p.alive && !p.flying && p.fire_at <= r.t {
                                (p.visible, p.flying) = (true, true);
                                p.show_body(ids, host.effects);
                            }
                        }
                        if t0 < METEOR_WHITE_OUT && r.t >= METEOR_WHITE_OUT {
                            // m_MeteorExpProj: sphere 100 + 1500 / s up to 1500, 1.2 s, no particle [A].
                            self.pending.push((0.0, TeamShot { row: "FR_CALLMETEOR_FINISH_EXP", position: r.aim, radius: 100.0, grow: 1500.0, max: 1500.0, life: 1.2, effect: "" }));
                        }
                        if r.t >= METEOR_WHITE_IN {
                            for p in &mut self.projs {
                                p.kill(host.effects);
                            }
                            (r.phase, r.clip_t) = (2, METEOR_DISAPPEAR);
                            r.sora_action = Some(80);
                        }
                    }
                    _ => {
                        r.clip_t -= dt;
                        if r.clip_t <= 0.0 {
                            r.finished = true;
                        }
                    }
                }
                // (party D5) The pawn's Tick override 0x1412E6E70, after the state tick (its base
                // Tick 0x14141ED60 first): for each live meteor with flag 8 and m_ShakeCurveData set,
                // t = info +0x24 + dt, wrapped by (max - min) once past the range's end, stored; the
                // meteor at info +0x14 + GetVectorValue(t) (0x144084BC0; X / Y without keys = 0) by
                // SetActorLocation [C]. Its fade (+0x20 by m_fAppearFade / m_fDisappearFade x dt into a
                // material scalar parameter, name at DAT_1487E0DD0 not read) is not ported [D].
                for p in self.projs.iter_mut().filter(|p| p.alive && p.shake) {
                    let mut t = p.curve_t + dt;
                    if METEOR_SHAKE_MAX <= t {
                        t = t + METEOR_SHAKE_MIN - METEOR_SHAKE_MAX;
                    }
                    p.curve_t = t;
                    p.position = p.base + Vec3::Z * rich_eval(&METEOR_SHAKE_Z, t);
                }
            }
            // Flare Force (ATresFriendLinkPawnFlareForce, states 0x1412E2430) [C]:
            // - Appear begin 0x1412CC040 / tick 0x1412D5F00: sub 0 (Sora in his state 136, <= 1 s)
            //   the pawn faces the aim, FForceFinish on the link pawn (all participants), 87,
            //   timer +0xC64 = 0; sub 2 waits for +0xC6E, set by FUN_1412E1950 (the +0x440
            //   interface's slot 0x348 = TresAnimNotify_Timing 0x140561CC0 with m_Param 0 while
            //   +0xC6F): Sora's FForceFinish (1723) has that notify at 1.2064 s [A].
            // - Finish begin 0x1412CDB40: the 16 missiles spawn hidden at 0.01 scale at pawn +
            //   R(yaw + m_fAppearYaw) x m_vAppearOffset, pitched m_fAppearPitch; info state 1,
            //   countdown = m_fAppearTimer. Finish tick 0x1412DA3D0: Disappear when the
            //   participants' motion ends (vt+0x2698). The missiles' own machine runs in the
            //   pawn's tick override 0x1412E72B0 in every state (see `flare_missile`).
            // - Disappear 0x1412CCE40 releases the participants; the pawn lingers for its
            //   missiles (state time 3.03, 0x4041F120 in the state table) [C].
            208 => {
                let ids = self.ids.get_or_insert(SeamHandles::team());
                if once(&mut r, 0) {
                    r.sora_action = Some(87);
                    r.sora_motion = Some("FForceFinish");
                    r.clip_t = 0.0;
                }
                friend_motion = Some("FForceFinish");
                let base = r.at + Vec3::Z * LINK_PAWN_ORIGIN_Z;
                r.clip_t += dt;
                if r.phase == 0 && r.clip_t >= FFORCE_FINISH_TIMING {
                    r.phase = 1;
                    for (i, m) in MISSILES.iter().enumerate() {
                        let yaw = r.yaw + m.0;
                        let mut p = TeamProj::new(ProjKind::Missile, i, base + rot_yaw(FLARE_APPEAR_OFFSET, yaw), rot_vector(FLARE_APPEAR_PITCH, yaw));
                        (p.appear_at, p.fire_at, p.mstate, p.countdown) = (m.1, m.2, 1, m.1);
                        self.projs.push(p);
                    }
                }
                if r.phase == 1 && r.clip_t >= FFORCE_LENGTH {
                    // Disappear: the participants are released (Sora's state goes on by itself).
                    (r.phase, r.n) = (2, 0);
                    r.finished = true;
                    r.released = true;
                    r.t_release = r.t;
                }
                // (D4) The pawn's main target each tick: FUN_1412AC290(Sora's point, 3-D, no
                // sight, 1e6) -> +0xC50 / +0xC70 (0x1412E72B0) [C].
                let enemies = Self::enemies(host);
                let point = self.sora.map_or(r.at, |s| s.2) + Vec3::Z * SORA_CENTER_BONE_Z;
                let main = Self::pawn_target(self.ctrl_target, host, point, false, 1.0e6);
                let pawn_z = base.z;
                for p in &mut self.projs {
                    if !p.alive {
                        continue;
                    }
                    // (D4) The own target (info +0x24) is chosen by FUN_1412E43E0 when the fire
                    // curve ends (the 3 -> 4 step of `flare_missile`, here just before it), in
                    // the missiles' order within a tick [C].
                    if p.mstate == 3 && p.countdown - dt <= 0.0 {
                        p.target = TargetId::NONE;
                        p.no_target = false;
                        if let Some((id, first)) = Self::flare_pick(p, &enemies, main.as_ref(), &mut self.flare_locks) {
                            p.target = id;
                            p.no_target = first;
                        }
                    }
                    let own_t = enemies.iter().find(|t| t.id == p.target).copied();
                    let out_of = Self::flare_missile(p, dt, base, r.yaw, r.clip_t, main.as_ref(), own_t.as_ref(), &mut self.rng, ids, host);
                    match out_of {
                        MissileOut::None => {}
                        MissileOut::Hit(at) => {
                            // The missile's own 25 cm sphere (row FR_FLAREFORCE_FINISH01, its CDO
                            // default) on a pawn; destroyed on the hit (no m_bNoDestroyOnHit) [A].
                            self.pending.push((0.0, TeamShot { row: "FR_FLAREFORCE_FINISH01", position: at, radius: p.radius, grow: 0.0, max: p.radius, life: 0.1, effect: "" }));
                            p.kill(host.effects);
                        }
                        MissileOut::Explode(at) => {
                            // State 10 (0x1412E56D0): m_MissileExpProj at the missile, its row by
                            // the height over the pawn: <= m_fTargetHeight_2 (1000) FINISH01, <=
                            // m_fTargetHeight_3 (2000) FINISH02, else FINISH03 [C]. FlareForce_ExpProj
                            // [A]: sphere 30 + 500 / s up to 65, life 0.9, collision from 0.1 s.
                            let dz = at.z - pawn_z;
                            let row = if dz <= FLARE_TARGET_HEIGHT_2 { "FR_FLAREFORCE_FINISH01" } else if dz <= FLARE_TARGET_HEIGHT_3 { "FR_FLAREFORCE_FINISH02" } else { "FR_FLAREFORCE_FINISH03" };
                            burst(ids, host.effects, "vn_ex001_fff_hit00", at);
                            self.pending.push((0.1, TeamShot { row, position: at, radius: 65.0, grow: 0.0, max: 65.0, life: 0.8, effect: "" }));
                            p.kill(host.effects);
                        }
                    }
                }
                self.projs.retain(|p| p.alive);
                if r.released && ((self.projs.is_empty() && self.pending.is_empty()) || r.t - r.t_release >= FLARE_DISAPPEAR_TIME) {
                    r.done = true;
                }
            }
            // Goofy Bombardier (ATresFriendLinkPawnGoofyShoot, states 0x1412E2800) [C]:
            // - Appear begin 0x1412CC160: T = pawn + fwd x 400 + 1000 z, or the controller's target
            //   (vt+0x1690) + 1000 z; the dummy target note at T (the camera's, action 5). Appear
            //   tick 0x1412D6F90: GShootStart on the link pawn, the participants attached.
            // - Move tick 0x1412DCE50: 1 at GShootStart's end GSHootRise, gravity on, the launch
            //   of FUN_1412C93A0 (apex at T) and FR_GOOFYSHOOT_SORA (the pawn's 350 cm sphere [A]);
            //   2 until the flight time or < 220 cm of T: SORA off, flying mode, GShootHit; 3 at its
            //   end GShootFinish; 4 at Goofy's end: Goofy GShootEnd + FR_GOOFYSHOOT_RUSH; 5 Sora and
            //   Goofy detached, Goofy thrown at R(m_Param_ThrowPitch -45, yaw to the target) x 2300.
            // - Goofy's FRGoofyShoot 0x1411CB010 sub 1: horizontal velocity x 0.9^(60 dt), gravity;
            //   at the ground / a wall / 10 s: m_GoofyShootProj (GoofyShoot_Proj = FR_GOOFYSHOOT_EXP,
            //   spawned as Sora's) at him, RUSH off, M_1501, then his state ends.
            // The special mode (+0xBE4: e_ex041 / e_ex702 targets, arc search FUN_1412C90E0) has no
            // such enemy here: `goofy_arc` keeps it, unit-tested.
            195 => {
                let enemies = Self::enemies(host);
                // (D4) The controller's current target (vt+0x1690), no search: the Appear's T
                // (0x1412CC160) and the throw's yaw (0x1412DCE50 case 5) [C].
                let target = enemies.iter().find(|t| t.id == self.ctrl_target).copied();
                if once(&mut r, 1) {
                    // (party D5) the actor rests on its 220 cm TresRoot sphere (GSHOOT_ROOT_REST_Z [A, D]).
                    r.at.z += GSHOOT_ROOT_REST_Z;
                }
                let centre = r.at + Vec3::Z * LINK_PAWN_ORIGIN_Z;
                if once(&mut r, 0) {
                    r.aim = match target {
                        Some(t) => t.center + Vec3::Z * GSHOOT_TARGET_HEIGHT,
                        None => centre + fwd(r.yaw) * 400.0 + Vec3::Z * GSHOOT_TARGET_HEIGHT,
                    };
                }
                let gid = r.members.first().copied();
                let goofy = members.iter().find(|m| Some(m.id) == gid).copied();
                let goofy_ended = |clip: &str| goofy.is_some_and(|g| g.clip == clip && g.clip_ended);
                r.clip_t += dt;
                match r.phase {
                    0 => {
                        r.sora_motion = Some("GShootStart");
                        friend_motion = Some("GShootStart");
                        // (party D5) cases 0..1 of 0x1412DCE50 face the actor to T flat: vt+0xDB0
                        // (0x1405DDD90: slerp to Rotation(T - loc), alpha = clamp(dt x 360, 0, 1) = 1
                        // at any frame rate) [C].
                        let to_t = (r.aim - r.at).truncate();
                        if to_t.length_squared() > 1e-6 {
                            r.yaw = to_t.y.atan2(to_t.x).to_degrees();
                        }
                        // (D4) the link pawn's own GShootStart (o_at001 1636) ends (case 1) [C, A].
                        if r.clip_t >= GSHOOT_START_LENGTH {
                            let (v, flight) = goofy_rise(centre, r.aim, GSHOOT_GRAVITY);
                            (r.phase, r.clip_t, r.vel, r.flight) = (1, 0.0, v, flight);
                            r.hit_set.clear();
                        }
                    }
                    1 => {
                        r.sora_motion = Some("GShootRise");
                        friend_motion = Some("GShootRise");
                        r.vel.z -= GSHOOT_GRAVITY * dt;
                        let p0 = r.at;
                        r.at += r.vel * dt;
                        // FR_GOOFYSHOOT_SORA: the link pawn's 350 cm sphere, one hit per target.
                        for t in &enemies {
                            if !r.hit_set.contains(&t.id) && seg_dist(p0 + Vec3::Z * LINK_PAWN_ORIGIN_Z, r.at + Vec3::Z * LINK_PAWN_ORIGIN_Z, t.center) <= GSHOOT_SORA_RADIUS + t.radius {
                                r.hit_set.push(t.id);
                                self.pending.push((0.0, TeamShot { row: "FR_GOOFYSHOOT_SORA", position: t.center, radius: t.radius.max(25.0), grow: 0.0, max: t.radius.max(25.0), life: 0.1, effect: "" }));
                            }
                        }
                        let near = (r.aim - (r.at + Vec3::Z * LINK_PAWN_ORIGIN_Z)).length_squared() < 48_400.0;
                        if r.clip_t >= r.flight || near {
                            (r.phase, r.clip_t, r.vel) = (2, 0.0, Vec3::ZERO);
                        }
                    }
                    2 => {
                        r.sora_motion = Some("GShootHit");
                        friend_motion = Some("GShootHit");
                        if r.clip_t >= GSHOOT_HIT_LENGTH {
                            (r.phase, r.clip_t) = (3, 0.0);
                        }
                    }
                    3 => {
                        r.sora_motion = Some("GShootFinish");
                        friend_motion = Some("GShootFinish");
                        if (r.clip_t > 0.2 && goofy_ended("GShootFinish")) || r.clip_t >= GSHOOT_FINISH_LENGTH_GOOFY {
                            (r.phase, r.clip_t) = (4, 0.0);
                            r.hit_set.clear();
                        }
                    }
                    4 => {
                        // Sora and Goofy detached (Move 5): Sora keeps his clip and falls after it.
                        friend_motion = Some("GShootEnd");
                        r.finished = true;
                        (r.released, r.t_release) = (true, r.t);
                        // (party D5) Goofy leaves from where X_Link02_skt held him (last step's
                        // socket); the throw yaw below is his own from here on.
                        let g = r.friend_at.unwrap_or(r.at);
                        r.friend_yaw = None;
                        r.sora_hold = None;
                        let f = fwd(r.yaw);
                        let mut x = match target {
                            Some(t) => t.center + Vec3::Z * GSHOOT_TARGET_HEIGHT,
                            None => r.aim,
                        };
                        let d = (x - g).truncate();
                        if f.truncate().dot(d.normalize_or_zero()) < 0.5 || d.length_squared() < 90_000.0 {
                            x = Vec3::new(g.x + f.x * 300.0, g.y + f.y * 300.0, x.z);
                        }
                        let dd = (x - g).truncate();
                        let yaw = if dd.length_squared() > 1e-6 { dd.y.atan2(dd.x).to_degrees() } else { r.yaw };
                        r.yaw = yaw;
                        r.vel = rot_vector(GSHOOT_THROW_PITCH, yaw) * GSHOOT_THROW_SPEED;
                        r.friend_at = Some(g);
                        (r.phase, r.clip_t) = (5, 0.0);
                    }
                    5 => {
                        friend_motion = Some("GShootEnd");
                        let k = 0.9f32.powf(dt * 60.0);
                        (r.vel.x, r.vel.y) = (r.vel.x * k, r.vel.y * k);
                        // (D4) his mesh pitched toward R(Rotation(velocity).pitch + 90, 0, 0) by
                        // vt+0xDC0 (0x1405AF4B0 -> 0x1405AF2A0: mesh relative rotation slerped to
                        // that x its default, alpha = 6 dt) on the damped velocity (0x1411CB010 sub 1) [C].
                        if let Some(id) = gid {
                            let vp = r.vel.z.atan2(r.vel.truncate().length()).to_degrees();
                            self.pitch_toward(id, vp + 90.0, 6.0 * dt);
                        }
                        r.vel.z -= GRAVITY * dt;
                        let g0 = r.friend_at.unwrap_or(r.at);
                        let mut g1 = g0 + r.vel * dt;
                        let c0 = g0 + Vec3::Z * GOOFY_CAPSULE_Z;
                        let c1 = g1 + Vec3::Z * GOOFY_CAPSULE_Z;
                        let wall = host.world.line_trace(c0, c1).map(|h| h.position);
                        if let Some(w) = wall {
                            g1 = w - Vec3::Z * GOOFY_CAPSULE_Z;
                        }
                        let ground = host.world.line_trace(g1 + Vec3::Z * 20.0, g1 - Vec3::Z * 15.0).map(|h| h.position);
                        // FR_GOOFYSHOOT_RUSH: Goofy's own row, his shield's 150 cm sphere.
                        for t in &enemies {
                            if !r.hit_set.contains(&t.id) && seg_dist(c0, c1, t.center) <= GSHOOT_RUSH_RADIUS + t.radius {
                                r.hit_set.push(t.id);
                                if let Some(id) = gid {
                                    party.fr_hit(id, "FR_GOOFYSHOOT_RUSH", t.id, t.center, host);
                                }
                            }
                        }
                        r.friend_at = Some(ground.unwrap_or(g1));
                        if ground.is_some() || wall.is_some() || r.clip_t >= 10.0 {
                            // GoofyShoot_Proj CDO [A]: sphere 500 + 1000 / s up to 900, life 0.5, no
                            // destroy on hit, particle vn_ex002_gsh_sho0; at his location.
                            // (D4) at his root location (0x1411CB010: uStack_78 = RootComponent +0x2B0) [C].
                            let at = r.friend_at.unwrap_or(g1);
                            self.pending.push((0.0, TeamShot { row: "FR_GOOFYSHOOT_EXP", position: at, radius: 500.0, grow: 1000.0, max: 900.0, life: 0.5, effect: "vn_ex002_gsh_sho0" }));
                            (r.phase, r.clip_t, r.vel) = (6, 0.0, Vec3::ZERO);
                        }
                    }
                    _ => {
                        friend_motion = Some("M_1501");
                        // (D4) sub 2: the mesh back to its default by vt+0xDC8 (alpha 15 dt) [C 0x1411CB010].
                        if let Some(id) = gid {
                            self.pitch_toward(id, 0.0, 15.0 * dt);
                        }
                        if (r.clip_t > 0.1 && goofy_ended("M_1501")) || r.clip_t >= GOOFY_M1501_LENGTH {
                            r.done = true;
                        }
                    }
                }
                if r.phase <= 3 {
                    // (party D5) THE RIDERS ON THE ANIMATED SOCKETS. The link pawn plays its own
                    // clip of the phase (GShootStart 1636, GSHootRise 1637 looped, GShootHit 1638,
                    // GShootFinish 1639) with root motion [C: play modes 2 / 3 at 0x1412D6F90 /
                    // 0x1412DCE50, docs/sora-actions-spec.md]: the root's track moves the ACTOR (the
                    // drawn root held at its reference pose, RefPose lock [A]) as a displacement added
                    // to the movement (spec "RM", velocity untouched), through the mesh's world rotation;
                    // the participants are attached to X_Link01_skt (Sora) / X_Link02_skt (Goofy) with
                    // SnapToTarget location and rotation (0x1412D6F90 sub 1 -> 0x1405A3FA0) [C].
                    let clip = r.phase as usize;
                    // The mesh's extra pitch (vt+0xDC0 0x1405AF4B0 -> 0x1405AF2A0: relative = slerp(rel,
                    // Quat(R) x default, alpha)): case 2 (the rise) toward R(Rotation(velocity).pitch - 90,
                    // 0, 0) at 18 dt; case 3 (GShootHit) back to the default (vt+0xDC8 0x1405AEED0) at
                    // 12 dt, case 4 (GShootFinish) at 30 dt [C 0x1412DCE50]. One axis: slerp = the angle
                    // moved by alpha along the short way.
                    let (goal, rate) = match r.phase {
                        1 => (r.vel.z.atan2(r.vel.truncate().length()).to_degrees() - 90.0, 18.0),
                        2 => (0.0, 12.0),
                        3 => (0.0, 30.0),
                        _ => (r.link_pitch, 0.0),
                    };
                    r.link_pitch += crate::pawn::wrap180(goal - r.link_pitch) * (rate * dt).clamp(0.0, 1.0);
                    let mesh_rot = ue_quat(0.0, r.yaw) * ue_quat(r.link_pitch, 0.0);
                    let fr = gshoot_frame(clip, r.clip_t);
                    let root = Vec3::from_array(fr.root);
                    let delta = match r.rm_prev {
                        Some((p, prev)) if p as usize == clip => root - prev,
                        // the clip changed this step: the rest of the old clip's root track
                        Some((p, prev)) => Vec3::from_array(gshoot_frame(p as usize, f32::MAX).root) - prev,
                        None => Vec3::ZERO,
                    };
                    let delta = if clip == 1 { Vec3::ZERO } else { delta };
                    r.rm_prev = Some((r.phase, root));
                    r.at += mesh_rot * delta;
                    let place = |s: [f32; 3], q: [f32; 4]| {
                        let (yaw, pitch) = yaw_pitch(mesh_rot * Quat::from_xyzw(q[0], q[1], q[2], q[3]));
                        (r.at + mesh_rot * Vec3::from_array(s), yaw, pitch)
                    };
                    let (s1, y1, p1) = place(fr.s1, fr.q1);
                    let (s2, y2, p2) = place(fr.s2, fr.q2);
                    r.sora_hold = Some((s1, y1));
                    self.sora_pitch = Some(p1);
                    r.friend_at = Some(s2);
                    r.friend_yaw = Some(y2);
                    if let Some(id) = gid {
                        self.goofy_pitch = Some((id, p2));
                    }
                } else if let Some(p) = self.sora_pitch {
                    // Sora let go: his pitch back toward level (his state tick's vt+0xDC8 idiom, 12 dt) [C, D rate].
                    let p = p - p * (12.0 * dt).clamp(0.0, 1.0);
                    self.sora_pitch = (p.abs() > 0.01).then_some(p);
                }
            }
            // Trinity Guard (ATresFriendLinkPawnShieldGuardian: states 0x1412F5AF0 Appear / Idle /
            // Move / Attack / AirAttack / Finish / Damage / Disappear) [C]: SGuardStart (1.8), then
            // the link pawn is a character of its own: the stick is its move input (vt+0x27F0
            // 0x14130B0A0 keeps the direction at +0xBE4), Idle 0x1412F0110 / Move 0x1412F0AD0 play
            // SGuardIdle / SGuardWalk, A = Attack (vt+0x27D0 0x14130AEA0: Attack on the ground,
            // AirAttack in the air), B = the character jump (its CMC; the jump clips by phase from
            // vt+0x1C88 0x1412F4780: SGuardJumpStart / Rise / Fall / Land), Y = Finish (vt+0x27E0
            // 0x14130B060) or `m_Param_MaxTime` 20 s: 68, SGuardHeavyBash with
            // FR_SHIELDGUARDIAN_HEAVYBASH. The movement is kh-core's CMC with the pawn's numbers:
            // MaxWalkSpeed 180, MaxStepHeight 47 [A ShieldGuardian_Pawn]; JumpZVelocity 420,
            // MaxAcceleration 2048, GroundFriction 8, AirControl 0.05, RotationRate yaw 360, gravity
            // scale 1 [C UTresCharMovementComponent ctor 0x140570EB0, not overridden].
            _ => {
                let (stick, attack, jump) = self.input;
                self.input.1 = false;
                self.input.2 = false;
                if r.body.is_none() {
                    r.body = Some(Cmc::new(&shield_params(), r.at));
                }
                match r.phase {
                    0 => {
                        r.sora_motion = Some("SGuardStart");
                        friend_motion = Some("SGuardStart");
                        if r.t >= 1.8 {
                            r.phase = 1;
                        }
                    }
                    1 | 2 | 3 => {
                        let params = shield_params();
                        let mut body = r.body.unwrap_or_else(|| Cmc::new(&params, r.at));
                        r.clip_t += dt;
                        let grounded = body.on_ground();
                        let bash_len = match (r.phase, r.n % 2 == 1, r.vz > 0.5) {
                            (_, _, true) => SGUARD_AIR_BASH_LENGTH,
                            (_, true, _) => SGUARD_BASH_LENGTH,
                            _ => SGUARD_BASH2_LENGTH,
                        };
                        if r.phase == 2 && r.clip_t >= bash_len {
                            (r.phase, r.vz) = (if grounded { 1 } else { 3 }, 0.0);
                        }
                        if r.phase == 3 && grounded && r.clip_t > 0.1 {
                            (r.phase, r.clip_t) = (1, 0.0);
                            r.landed = SGUARD_LAND_LENGTH;
                        }
                        if matches!(r.phase, 1 | 3) && attack {
                            // Attack (ground: SHIELDBASH, then SHIELDBASH2) / AirAttack.
                            let air = !grounded;
                            r.n += 1;
                            (r.phase, r.clip_t, r.vz) = (2, 0.0, if air { 1.0 } else { 0.0 });
                            let row = if air || r.n % 2 == 1 { "FR_SHIELDGUARDIAN_SHIELDBASH" } else { "FR_SHIELDGUARDIAN_SHIELDBASH2" };
                            r.shots.push(TeamShot { row, position: r.at + fwd(r.yaw) * 150.0 + Vec3::Z * 80.0, radius: 200.0, grow: 0.0, max: 200.0, life: 0.3, effect: "VN_slg_bsh0" });
                        } else if r.phase == 1 && jump && grounded {
                            body.jump_z_velocity = SGUARD_JUMP_Z;
                            body.do_jump();
                            (r.phase, r.clip_t) = (3, 0.0);
                        }
                        // The move input (none while bashing on the ground).
                        let input = if r.phase == 2 && r.vz < 0.5 { Vec3::ZERO } else { Vec3::new(stick.x, stick.y, 0.0) * stick.length().min(1.0) / stick.length().max(1e-6) };
                        let input = if stick.length() > 0.1 { input } else { Vec3::ZERO };
                        if input != Vec3::ZERO && !(r.phase == 2 && r.vz < 0.5) {
                            let want = input.y.atan2(input.x).to_degrees();
                            let d = crate::pawn::wrap180(want - r.yaw);
                            r.yaw += d.clamp(-SGUARD_TURN_RATE * dt, SGUARD_TURN_RATE * dt);
                        }
                        body.control_input = input;
                        let mut yaw = r.yaw;
                        body.tick_head(host.world, &params, dt);
                        body.perform_movement(host.world, &params, dt, None, &mut yaw);
                        r.at = body.location;
                        r.body = Some(body);
                        r.landed = (r.landed - dt).max(0.0);
                        let m = match r.phase {
                            2 if r.vz > 0.5 => "SGuardAirShieldBash",
                            2 => if r.n % 2 == 1 { "SGuardShieldBash" } else { "SGuardShieldBash2" },
                            3 => {
                                if r.clip_t < 0.1 {
                                    "SGuardJumpStart"
                                } else if body.velocity.z > 0.0 {
                                    "SGuardJumpRise"
                                } else {
                                    "SGuardFall"
                                }
                            }
                            _ if r.landed > 0.0 && input == Vec3::ZERO => "SGuardLand",
                            _ => if input != Vec3::ZERO { "SGuardWalk" } else { "SGuardIdle" },
                        };
                        r.sora_motion = Some(m);
                        friend_motion = Some(m);
                        if (r.t >= SHIELD_MAX_TIME + 1.8 || r.cancel) && r.phase == 1 {
                            (r.phase, r.clip_t) = (4, 0.0);
                            r.sora_action = Some(68);
                        }
                    }
                    _ => {
                        r.sora_motion = Some("SGuardHeavyBash");
                        friend_motion = Some("SGuardHeavyBash");
                        let before = r.clip_t;
                        r.clip_t += dt;
                        if before < 0.8 && r.clip_t >= 0.8 {
                            r.shots.push(TeamShot { row: "FR_SHIELDGUARDIAN_HEAVYBASH", position: r.at + fwd(r.yaw) * 200.0, radius: 300.0, grow: 1000.0, max: 600.0, life: 0.3, effect: "VN_slg_hvy00" });
                        }
                        if r.clip_t >= 1.9667 {
                            r.finished = true;
                        }
                    }
                }
            }
        }
        // The projectiles in flight (proj.rs): a pawn hit is a one-step shot of the projectile's
        // sphere (Sora's shot system applies the row); the missiles' m_MissileExpProj
        // (FlareForce_ExpProj [A]: sphere 30 + 500 / s up to 65, life 0.9, collision from 0.1 s,
        // row FR_FLAREFORCE_FINISH01, particle vn_ex001_fff_hit00) follows each contact.
        if !self.projs.is_empty() {
            let ids = self.ids.get_or_insert(SeamHandles::team());
            let enemies = Self::enemies(host);
            for p in &mut self.projs {
                let Some(contact) = p.fly(dt, host.world, &enemies) else { continue };
                let at = match contact {
                    Contact::Pawn { at, .. } | Contact::Map { at } | Contact::Timeout { at } => at,
                };
                match (p.kind, contact) {
                    (ProjKind::Meteor, Contact::Pawn { .. }) => {
                        self.pending.push((0.0, TeamShot { row: p.row, position: at, radius: p.radius, grow: 0.0, max: p.radius, life: 0.1, effect: "vn_ex001_fcm_hit00" }));
                    }
                    (ProjKind::Meteor, Contact::Map { .. }) => {
                        burst(ids, host.effects, "vn_ex001_fcm_hit00", at);
                    }
                    (ProjKind::Missile, Contact::Pawn { .. } | Contact::Map { .. }) => {
                        if matches!(contact, Contact::Pawn { .. }) {
                            self.pending.push((0.0, TeamShot { row: p.row, position: at, radius: p.radius, grow: 0.0, max: p.radius, life: 0.1, effect: "" }));
                        }
                        burst(ids, host.effects, "vn_ex001_fff_hit00", at);
                        self.pending.push((0.1, TeamShot { row: "FR_FLAREFORCE_FINISH01", position: at, radius: 65.0, grow: 0.0, max: 65.0, life: 0.8, effect: "" }));
                    }
                    _ => {}
                }
                p.kill(host.effects);
            }
            self.projs.retain(|p| p.alive);
        }
        for q in &mut self.pending {
            q.0 -= dt;
        }
        while r.shots.len() < 4 {
            let Some(k) = self.pending.iter().position(|q| q.0 <= 0.0) else { break };
            r.shots.push(self.pending.remove(k).1);
        }
        for s in &r.shots {
            out.events.push(TeamEvent::ProjectileSpawned { kind: s.row, at: s.position });
        }
        let ids = r.members.clone();
        // (party D5) Goofy on X_Link02_skt faces the socket's yaw (see the 195 arm).
        let (at, yaw) = (r.friend_at.unwrap_or(r.at), r.friend_yaw.unwrap_or(r.yaw));
        self.run = Some(r);
        let fr_state = match self.run.as_ref().map_or(0, |r| r.kind) {
            207 => fr_id::CALL_METEOR,
            208 => fr_id::FLARE_FORCE,
            195 => fr_id::GOOFY_SHOOT,
            _ => fr_id::SHIELD_GUARDIAN,
        };
        let (released, done, kind) = self.run.as_ref().map_or((false, false, 0), |r| (r.released, r.done, r.kind));
        if done {
            self.stop(party, host, out);
            return;
        }
        if released && kind != 195 {
            // Disappear released the participants: they leave their FR states once.
            if let Some(r) = self.run.as_mut().filter(|r| r.sent & (1 << 31) == 0) {
                r.sent |= 1 << 31;
                for id in &ids {
                    party.team_command(*id, MemberCommand::Stop);
                    self.given.retain(|g| g.0 != *id);
                }
            }
            return;
        }
        for id in ids {
            // A member that has not entered its FR state yet (B takes the command at its next step,
            // and a running action may still hold it): ask again; its motion is given anew.
            if members.iter().any(|m| m.id == id && !matches!(m.state, NpcStateId::Fr(_))) {
                party.team_command(id, MemberCommand::EnterFrState { state: fr_state });
                self.given.retain(|g| g.0 != id);
            }
            if members.iter().any(|m| m.id == id) {
                self.drive(party, id, at, yaw, friend_motion);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandTicket;

    #[derive(Default)]
    struct FakeParty {
        members: Vec<TeamMember>,
        commands: Vec<(MemberId, MemberCommand)>,
        drives: Vec<(MemberId, FrDrive)>,
    }

    impl TeamParty for FakeParty {
        fn team_members(&self, out: &mut Vec<TeamMember>) {
            out.extend(self.members.iter().copied());
        }
        fn team_command(&mut self, id: MemberId, cmd: MemberCommand) -> CommandTicket {
            self.commands.push((id, cmd));
            if let Some(m) = self.members.iter_mut().find(|m| m.id == id) {
                m.state = match cmd {
                    MemberCommand::EnterFrState { state } => NpcStateId::Fr(state),
                    _ => NpcStateId::Idle,
                };
            }
            CommandTicket(1)
        }
        fn fr_drive(&mut self, id: MemberId, drive: FrDrive) {
            self.drives.push((id, drive));
        }
    }

    fn member(id: u8, chara: Chara) -> TeamMember {
        TeamMember { id: MemberId(id), chara, position: Vec3::new(0.0, 150.0 * (id as f32 * 2.0 - 1.0), 0.0), yaw: 0.0, on_ground: true, ko: false, state: NpcStateId::Idle, in_battle: true, combi_time: 0, combi_up: 0, clip: "", clip_time: 0.0, clip_ended: false }
    }

    fn party() -> FakeParty {
        FakeParty { members: vec![member(0, Chara::Donald), member(1, Chara::Goofy)], ..Default::default() }
    }

    #[test]
    fn link_points_offer_a_command_once_the_gauge_is_full_in_battle() {
        let world = kh_core::test_world::TestWorld::floor();
        let mut null = crate::host::NullPartyHost::default();
        let mut host = null.host(&world);
        let (mut team, mut p) = (Team::new(), party());
        let mut offered = None;
        for _ in 0..(60 * 120) {
            team.request(FriendLinkRequest::Sora { gauge_full: true, in_battle: true, position: Vec3::ZERO, yaw: 0.0 });
            let out = team.step(1.0 / 60.0, &mut p, &mut host);
            if let Some(TeamEvent::Offered { kind, .. }) = out.events.first() {
                offered = Some(*kind);
                break;
            }
        }
        let kind = offered.expect("an offer within two minutes");
        assert!([207, 208, 193, 195].contains(&kind));
        let mut rows = Vec::new();
        team.friend_rows(&p, &mut rows);
        assert!(rows.iter().any(|f| f.offer.is_some_and(|o| o.0 == kind)), "published in the rows");
        // Without the gauge: points reset, nothing offered.
        team.debug_ready_all();
        team.request(FriendLinkRequest::Sora { gauge_full: false, in_battle: true, position: Vec3::ZERO, yaw: 0.0 });
        assert!(team.step(1.0 / 60.0, &mut p, &mut host).events.is_empty());
    }

    #[test]
    fn donald_meteor_runs_its_link_pawn_and_ends() {
        let world = kh_core::test_world::TestWorld::floor();
        let mut null = crate::host::NullPartyHost::default();
        let mut host = null.host(&world);
        let (mut team, mut p) = (Team::new(), party());
        team.request(FriendLinkRequest::Sora { gauge_full: true, in_battle: true, position: Vec3::ZERO, yaw: 0.0 });
        team.request(FriendLinkRequest::Start { kind: 207, target: TargetId::NONE });
        team.step(1.0 / 60.0, &mut p, &mut host);
        assert!(p.commands.contains(&(MemberId(0), MemberCommand::EnterFrState { state: fr_id::CALL_METEOR })));
        let (mut actions, mut shots, mut done) = (Vec::new(), 0, false);
        for _ in 0..600 {
            let mut rows = Vec::new();
            team.friend_rows(&p, &mut rows);
            let link = rows[0].link;
            actions.extend(link.sora_action);
            shots += link.shots.iter().flatten().count();
            if link.finished {
                done = true;
                break;
            }
            team.step(1.0 / 60.0, &mut p, &mut host);
        }
        assert!(done);
        assert_eq!(actions, [78, 79, 80], "the meteor cameras' actions in order");
        // No enemy in the null host: the 40 meteors fall onto the floor (map hits, no shot), then
        // m_MeteorExpProj at the target.
        assert_eq!(shots, 1, "only the blast hits (nothing to hit for the meteors)");
        assert!(team.projs.is_empty(), "the Disappear destroyed every meteor");
        team.request(FriendLinkRequest::End { kind: 207, interrupted: false });
        team.step(1.0 / 60.0, &mut p, &mut host);
        assert_eq!(team.running(), 0);
        assert!(p.commands.contains(&(MemberId(0), MemberCommand::Stop)));
    }

    /// (party D5) Call Meteor's hover: each shown meteor bobs on m_ShakeCurveData (Z only, -9.8 .. 12.8 cm)
    /// about its spawn location from its own random curve time, spun R(i x 30, 0, j x 30) until the Finish.
    #[test]
    fn d5_meteors_hover_on_the_shake_curve_with_their_spawn_spin() {
        let world = kh_core::test_world::TestWorld::floor();
        let mut null = crate::host::NullPartyHost::default();
        let mut host = null.host(&world);
        let (mut team, mut p) = (Team::new(), party());
        team.request(FriendLinkRequest::Sora { gauge_full: true, in_battle: true, position: Vec3::ZERO, yaw: 0.0 });
        team.request(FriendLinkRequest::Start { kind: 207, target: TargetId::NONE });
        team.step(1.0 / 60.0, &mut p, &mut host);
        assert_eq!(team.projs.len(), 40);
        let times: Vec<f32> = team.projs.iter().map(|m| m.curve_t).collect();
        assert!(times.iter().all(|t| (METEOR_SHAKE_MIN..METEOR_SHAKE_MAX).contains(t)));
        assert!(times.windows(2).any(|w| w[0] != w[1]), "per-meteor random curve times");
        for m in &team.projs {
            let pitch = m.forward.z.clamp(-1.0, 1.0).asin().to_degrees();
            let (sp, roll) = m.spin.expect("spun at the spawn");
            assert!((roll / 30.0).fract().abs() < 1e-4 && (0.0..=360.0).contains(&roll) && (sp / 30.0).fract().abs() < 1e-4);
            assert!(((pitch + 90.0) % 30.0).abs() < 0.01 || ((pitch + 90.0) % 30.0 - 30.0).abs() < 0.01, "pitch {pitch}");
        }
        let (mut lo, mut hi, mut wrapped) = (f32::MAX, f32::MIN, false);
        for _ in 0..240 {
            let before: Vec<f32> = team.projs.iter().map(|m| m.curve_t).collect();
            team.step(1.0 / 60.0, &mut p, &mut host);
            if team.running() != 207 || team.run.as_ref().is_some_and(|r| r.phase != 0) {
                break;
            }
            for (m, b) in team.projs.iter().zip(before) {
                if m.shake {
                    let dz = m.position.z - m.base.z;
                    (lo, hi) = (lo.min(dz), hi.max(dz));
                    assert!((m.position.truncate() - m.base.truncate()).length() < 1e-3);
                    wrapped |= m.curve_t < b;
                }
            }
        }
        assert!(lo < -5.0 && hi > 8.0 && lo > -10.0 && hi < 13.0, "the bob {lo} .. {hi}");
        assert!(wrapped, "the curve time wraps by the time range");
        // After the Finish every meteor is placed at its start, no shake, no roll.
        for _ in 0..60 {
            team.step(1.0 / 60.0, &mut p, &mut host);
        }
        assert!(team.projs.iter().all(|m| !m.shake && m.spin.is_none()));
    }

    #[test]
    fn trinity_guard_needs_both_and_cancels_into_the_heavy_bash() {
        let world = kh_core::test_world::TestWorld::floor();
        let mut null = crate::host::NullPartyHost::default();
        let mut host = null.host(&world);
        let (mut team, mut p) = (Team::new(), party());
        team.request(FriendLinkRequest::Sora { gauge_full: true, in_battle: true, position: Vec3::ZERO, yaw: 0.0 });
        team.request(FriendLinkRequest::Start { kind: 193, target: TargetId::NONE });
        team.step(1.0 / 60.0, &mut p, &mut host);
        assert_eq!(team.running(), 193);
        for _ in 0..120 {
            team.step(1.0 / 60.0, &mut p, &mut host);
        }
        team.request(FriendLinkRequest::Input { kind: 193, stick: Vec3::ZERO, attack: true, jump: false, target: TargetId::NONE });
        team.step(1.0 / 60.0, &mut p, &mut host);
        let mut rows = Vec::new();
        team.friend_rows(&p, &mut rows);
        assert_eq!(rows[0].link.sora_motion, Some("SGuardShieldBash"));
        assert_eq!(rows[0].link.shots[0].map(|s| s.row), Some("FR_SHIELDGUARDIAN_SHIELDBASH"));
        team.request(FriendLinkRequest::Action { kind: 193, action: 2 });
        let mut saw68 = false;
        for _ in 0..300 {
            team.step(1.0 / 60.0, &mut p, &mut host);
            let mut rows = Vec::new();
            team.friend_rows(&p, &mut rows);
            saw68 |= rows[0].link.sora_action == Some(68);
            if rows[0].link.finished {
                break;
            }
        }
        assert!(saw68);
    }
    /// One enemy for the tests (D3).
    struct Foe(TargetInfo);

    impl kh_core::host::Targets for Foe {
        fn enumerate(&self, out: &mut Vec<TargetInfo>) {
            out.push(self.0);
        }
        fn info(&self, id: TargetId) -> Option<TargetInfo> {
            (id == self.0.id).then_some(self.0)
        }
        fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<kh_core::host::HurtShape>) {}
        fn apply_hit(&mut self, _hit: &kh_core::host::HitRecord) -> kh_core::host::HitResult {
            kh_core::host::HitResult::default()
        }
    }

    fn foe(at: Vec3) -> Foe {
        Foe(TargetInfo { id: TargetId(9), position: at, center: at + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::ENEMY | TargetFlags::LOCKABLE), hp: 1.0, hp_max: 1.0 })
    }

    #[test]
    fn flare_force_finishes_at_soras_timing_notify_and_its_missiles_run_their_machine() {
        let world = kh_core::test_world::TestWorld::floor();
        let mut null = crate::host::NullPartyHost::default();
        let mut targets = foe(Vec3::new(700.0, 0.0, 0.0));
        let mut host = PartyHost { world: &world, targets: &mut targets, audio: &mut null.audio, effects: &mut null.effects };
        let (mut team, mut p) = (Team::new(), party());
        team.request(FriendLinkRequest::Sora { gauge_full: true, in_battle: true, position: Vec3::ZERO, yaw: 0.0 });
        team.request(FriendLinkRequest::Start { kind: 208, target: TargetId(9) });
        let dt = 1.0 / 60.0;
        let (mut t, mut spawned_at, mut first_shown, mut first_fired, mut rows, mut released_at) = (0.0f32, None, None, None, Vec::new(), None);
        let mut wobble_seen = false;
        for _ in 0..(60 * 20) {
            team.step(dt, &mut p, &mut host);
            t += dt;
            if spawned_at.is_none() && !team.projs.is_empty() {
                spawned_at = Some(t);
                assert_eq!(team.projs.len(), 16);
                assert!(team.projs.iter().all(|m| m.mstate <= 2 && m.scale <= 0.1), "spawned hidden at 0.01 (those with m_fAppearTimer 0 show at once)");
            }
            if first_shown.is_none() && team.projs.iter().any(|m| m.visible) {
                first_shown = Some(t);
            }
            if team.projs.iter().any(|m| m.mstate == 2 && m.curve_t > 0.4) {
                // The hover wobble: the appear curve moves the missile off m_vAppearOffset.
                let m = team.projs.iter().find(|m| m.mstate == 2 && m.curve_t > 0.4).unwrap();
                let rest = Vec3::new(0.0, 0.0, LINK_PAWN_ORIGIN_Z) + rot_yaw(FLARE_APPEAR_OFFSET, MISSILES[m.index].0);
                wobble_seen |= (m.position - rest).length() > 2.0;
            }
            if first_fired.is_none() && team.projs.iter().any(|m| m.mstate >= 4) {
                first_fired = Some(t);
            }
            let mut fr = Vec::new();
            team.friend_rows(&p, &mut fr);
            rows.extend(fr.iter().flat_map(|f| f.link.shots.iter().flatten().map(|s| s.row)));
            if released_at.is_none() && fr.iter().any(|f| f.link.finished) {
                released_at = Some(t);
            }
            if team.running() == 0 {
                break;
            }
        }
        let spawned = spawned_at.expect("the Finish began");
        assert!((spawned - FFORCE_FINISH_TIMING).abs() < 2.0 * dt, "Finish at Sora's Timing notify: {spawned}");
        // Missiles 0 and 12 appear at once (m_fAppearTimer 0) after the Finish began.
        assert!(first_shown.unwrap() - spawned < 2.0 * dt);
        assert!(wobble_seen, "the appear curve moves the hovering missiles");
        // Each fires at max(its show, m_fFireTimer on the pawn's timer), plus the 0.183 s recoil.
        let expect = MISSILES.iter().map(|m| (spawned + m.1).max(m.2) + FLARE_FIRE_END).fold(f32::MAX, f32::min);
        let fired = first_fired.unwrap();
        assert!((fired - expect).abs() < 3.0 * dt, "first launch at {fired}, expected {expect}");
        assert!(released_at.unwrap() >= FFORCE_LENGTH - dt, "Disappear releases at the clip's end");
        assert!(rows.iter().any(|r| r.starts_with("FR_FLAREFORCE_FINISH0")), "{rows:?}");
        assert_eq!(team.running(), 0, "the link pawn ended with its missiles");
        assert!(p.commands.contains(&(MemberId(0), MemberCommand::Stop)));
    }

    /// (party I2) D2's open note "some missiles meet the floating dummy 5 at 600..970 cm but no hit event appears":
    /// the test-level dummy 5 (radius 40, height 120, floating 250 cm up) as the only foe 700 cm ahead.
    #[test]
    fn i2_flare_missiles_reach_a_floating_target() {
        let world = kh_core::test_world::TestWorld::floor();
        let mut null = crate::host::NullPartyHost::default();
        let at = Vec3::new(700.0, -250.0, 250.0);
        let mut targets = Foe(TargetInfo { id: TargetId(5), position: at, center: at + Vec3::Z * 60.0, velocity: Vec3::ZERO, yaw: 0.0, radius: 40.0, half_height: 60.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::ENEMY | TargetFlags::LOCKABLE | TargetFlags::AIRBORNE), hp: 1.0, hp_max: 1.0 });
        let mut host = PartyHost { world: &world, targets: &mut targets, audio: &mut null.audio, effects: &mut null.effects };
        let (mut team, mut p) = (Team::new(), party());
        team.request(FriendLinkRequest::Sora { gauge_full: true, in_battle: true, position: Vec3::ZERO, yaw: 0.0 });
        team.request(FriendLinkRequest::Start { kind: 208, target: TargetId(5) });
        let (mut near, mut all, mut closest) = (0, 0, f32::MAX);
        for _ in 0..(60 * 20) {
            team.step(1.0 / 60.0, &mut p, &mut host);
            for m in team.projs.iter().filter(|m| m.mstate >= 4) {
                closest = closest.min((m.position - (at + Vec3::Z * 60.0)).length());
            }
            let mut fr = Vec::new();
            team.friend_rows(&p, &mut fr);
            for s in fr.iter().flat_map(|f| f.link.shots.iter().flatten()) {
                all += 1;
                // a shot that overlaps the target's capsule (segment 290..330, radius 40) is a hit the host will see
                let c = at + Vec3::Z * 60.0;
                let z = s.position.z.clamp(c.z - 20.0, c.z + 20.0);
                if (s.position - Vec3::new(c.x, c.y, z)).length() <= s.radius.max(s.max) + 40.0 {
                    near += 1;
                }
            }
            if team.running() == 0 {
                break;
            }
        }
        println!("flare vs floating target: {near} of {all} shots overlap it, closest missile {closest:.0} cm");
        assert!(near > 0, "missiles reach the floating target: {near} / {all}, closest {closest}");
    }

    #[test]
    fn flare_missiles_home_with_the_games_law() {
        // FUN_1412E82B0 state 5 with a target: speed 240..840, 1000 deg/s; from the facing (1 cm/s).
        let mut m = TeamProj::new(ProjKind::Missile, 0, Vec3::new(0.0, 0.0, 300.0), rot_vector(40.0, 180.0));
        (m.velocity, m.rate, m.turn, m.target_point) = (m.forward, 10.0, std::f32::consts::TAU, Some(Vec3::new(800.0, 0.0, 90.0)));
        let mut d = f32::MAX;
        for _ in 0..600 {
            d = m.flare_homing(1.0 / 60.0, 10.0, FLARE_HOMING_ANGLE.to_radians(), 240.0, 840.0, 0.3, 0.0);
            let v = m.velocity;
            m.position += v / 60.0;
            if d <= FLARE_HOMING_RANGE {
                break;
            }
            assert!(m.speed >= 240.0 - 1e-3 && m.speed <= 840.0 + 1e-3);
        }
        assert!(d <= FLARE_HOMING_RANGE, "reached the aim: {d}");
        // The turn ramp only grows (the step is added without its sign, as decompiled).
        assert!(m.turn > std::f32::consts::TAU);
        // The two curves: the hover wobble peaks near +8 / +20 cm, the recoil ends at -57 / -43 cm.
        assert!((rich_eval(&FLARE_APPEAR_X, 0.418437) - 8.00897).abs() < 1e-3);
        assert!((rich_eval(&FLARE_APPEAR_Z, 0.49763393) - 19.862804).abs() < 1e-3);
        assert!((rich_eval(&FLARE_FIRE_X, 1.0) + 57.221436).abs() < 1e-3);
        let mid = rich_eval(&FLARE_FIRE_Z, 0.09);
        assert!(mid < 0.0 && mid > -43.36, "{mid}");
    }

    #[test]
    fn goofy_bombardier_rises_to_the_point_above_the_target_throws_goofy_and_explodes() {
        let world = kh_core::test_world::TestWorld::floor();
        let mut null = crate::host::NullPartyHost::default();
        let mut targets = foe(Vec3::new(300.0, 0.0, 0.0));
        let mut host = PartyHost { world: &world, targets: &mut targets, audio: &mut null.audio, effects: &mut null.effects };
        let (mut team, mut p) = (Team::new(), party());
        team.request(FriendLinkRequest::Sora { gauge_full: true, in_battle: true, position: Vec3::ZERO, yaw: 0.0 });
        team.request(FriendLinkRequest::Start { kind: 195, target: TargetId(9) });
        let dt = 1.0 / 60.0;
        let (mut top, mut rows, mut released, mut goofy_low) = (0.0f32, Vec::new(), false, f32::MAX);
        for _ in 0..(60 * 15) {
            team.step(dt, &mut p, &mut host);
            let mut fr = Vec::new();
            team.friend_rows(&p, &mut fr);
            if let Some(r) = team.run.as_ref() {
                if r.phase <= 1 {
                    // (party D5) the rise only: GShootHit's root motion lifts the actor on afterwards.
                    top = top.max(r.at.z);
                }
                if r.released {
                    released = true;
                    goofy_low = goofy_low.min(r.friend_at.map_or(f32::MAX, |g| g.z));
                    assert!(fr.iter().all(|f| f.link.sora.is_none()), "Sora is let go at the throw");
                }
            }
            rows.extend(fr.iter().flat_map(|f| f.link.shots.iter().flatten().map(|s| s.row)));
            if team.running() == 0 {
                break;
            }
        }
        // The rise aims its apex at T = target centre + 1000 and stops within 220 cm of it.
        assert!((90.0 + GSHOOT_TARGET_HEIGHT) - (top + LINK_PAWN_ORIGIN_Z) < 220.0 && top + LINK_PAWN_ORIGIN_Z < 90.0 + GSHOOT_TARGET_HEIGHT + 1.0, "stops within 220 cm of T: {top}");
        assert!(released);
        assert!(goofy_low.abs() < 1.0, "Goofy lands on the floor: {goofy_low}");
        assert!(rows.contains(&"FR_GOOFYSHOOT_SORA"), "the rise hits (350 cm sphere): {rows:?}");
        assert!(rows.contains(&"FR_GOOFYSHOOT_EXP"), "GoofyShoot_Proj at his landing");
        assert_eq!(team.running(), 0);
        // Sora's End while the pawn lives on does not cut Goofy's rush (tested by the run ending by
        // itself above); the arc search of the special mode keeps its window.
        let (v, t) = goofy_arc_search(Vec3::new(0.0, 0.0, 1200.0), Vec3::new(900.0, 0.0, 100.0), GRAVITY);
        assert!(t > 0.0 && v.z < 0.0);
        let (v45, _, ok) = goofy_arc(Vec3::ZERO, Vec3::new(1000.0, 0.0, 0.0), GRAVITY, 45.0);
        assert!(ok && (v45.length() - (GRAVITY * 1000.0).sqrt()).abs() < 1.0, "the 45 deg range law: {}", v45.length());
    }

    /// (party D5) Goofy Bombardier's riders ride the link pawn's animated sockets: on the floor while the
    /// actor rests 220 cm up (GShootStart), carried by GShootHit's root motion (~9.4 m in the clip) and its
    /// socket track, continuous into GShootFinish, Goofy pitched by X_Link02's own rotation there.
    #[test]
    fn d5_the_riders_follow_the_link_pawns_animated_sockets() {
        if GSHOOT_LINK_CLIPS.is_empty() {
            eprintln!("no build/gen/rust/party_team_gshoot.rs (node tools/party_team_pack.js --gshoot): skipped");
            return;
        }
        // The data itself: 1638's last socket frame = 1639's first (RefPose root lock), 1638 root ~9.4 m.
        let (hit_end, fin_0) = (gshoot_frame(2, f32::MAX), gshoot_frame(3, 0.0));
        assert!((Vec3::from_array(hit_end.s1) - Vec3::from_array(fin_0.s1)).length() < 0.5);
        let rm = Vec3::from_array(hit_end.root) - Vec3::from_array(gshoot_frame(2, 0.0).root);
        assert!((rm.length() - 944.0).abs() < 10.0, "GShootHit root motion {rm}");
        let world = kh_core::test_world::TestWorld::floor();
        let mut null = crate::host::NullPartyHost::default();
        let mut targets = foe(Vec3::new(300.0, 0.0, 0.0));
        let mut host = PartyHost { world: &world, targets: &mut targets, audio: &mut null.audio, effects: &mut null.effects };
        let (mut team, mut p) = (Team::new(), party());
        team.request(FriendLinkRequest::Sora { gauge_full: true, in_battle: true, position: Vec3::ZERO, yaw: 0.0 });
        team.request(FriendLinkRequest::Start { kind: 195, target: TargetId(9) });
        let dt = 1.0 / 60.0;
        let mut log: Vec<(u8, Vec3, Vec3, Vec3, f32)> = Vec::new();
        for _ in 0..(60 * 15) {
            team.step(dt, &mut p, &mut host);
            let mut fr = Vec::new();
            team.friend_rows(&p, &mut fr);
            if let Some(r) = team.run.as_ref().filter(|r| !r.released) {
                let sora = fr.iter().find_map(|f| f.link.sora).expect("Sora held");
                log.push((r.phase, r.at, sora.0, r.friend_at.expect("Goofy on his socket"), team.goofy_pitch.map_or(0.0, |g| g.1)));
            }
            if team.running() == 0 {
                break;
            }
        }
        assert!(log.iter().all(|l| l.2.z > -1.0 && l.3.z > -1.0), "the riders never under the floor");
        let start = log.iter().find(|l| l.0 == 0).copied().unwrap();
        assert!((start.1.z - GSHOOT_ROOT_REST_Z).abs() < 1.0 && start.2.z.abs() < 1.0 && start.3.z.abs() < 1.0, "on the floor under the resting actor: {start:?}");
        let hit: Vec<_> = log.iter().filter(|l| l.0 == 2).copied().collect();
        let (h0, h1) = (hit[0], *hit.last().unwrap());
        assert!((h1.1 - h0.1).length() > 700.0, "the actor carried by the root motion: {} -> {}", h0.1, h1.1);
        assert!((h1.2 - h0.2).length() > 500.0, "Sora carried: {} -> {}", h0.2, h1.2);
        let k = log.iter().position(|l| l.0 == 3).unwrap();
        assert!((log[k].2 - log[k - 1].2).length() < 40.0, "1638 -> 1639 continuous: {} -> {}", log[k - 1].2, log[k].2);
        let last = *log.iter().rev().find(|l| l.0 == 3).unwrap();
        assert!((last.3 - last.2).length() > 50.0, "Goofy's socket leaves Sora's in GShootFinish");
        assert!(last.4 > 5.0, "Goofy pitched by X_Link02's rotation: {}", last.4);
    }

    #[test]
    fn trinity_guard_walks_at_180_and_jumps_with_its_own_movement_component() {
        let world = kh_core::test_world::TestWorld::floor();
        let mut null = crate::host::NullPartyHost::default();
        let mut host = null.host(&world);
        let (mut team, mut p) = (Team::new(), party());
        team.request(FriendLinkRequest::Sora { gauge_full: true, in_battle: true, position: Vec3::ZERO, yaw: 0.0 });
        team.request(FriendLinkRequest::Start { kind: 193, target: TargetId::NONE });
        let dt = 1.0 / 60.0;
        for _ in 0..115 {
            team.step(dt, &mut p, &mut host);
        }
        // Walk +Y for 2 s: the yaw turns at 360 deg/s, the speed reaches MaxWalkSpeed 180.
        let x0 = team.run.as_ref().unwrap().at;
        for _ in 0..120 {
            team.request(FriendLinkRequest::Input { kind: 193, stick: Vec3::Y, attack: false, jump: false, target: TargetId::NONE });
            team.step(dt, &mut p, &mut host);
        }
        let r = team.run.as_ref().unwrap();
        let v = r.body.unwrap().velocity.truncate().length();
        assert!((v - SHIELD_WALK_SPEED).abs() < 1.0, "walk speed {v}");
        assert!((r.yaw - 90.0).abs() < 1.0 && r.at.y - x0.y > 250.0, "{} {:?}", r.yaw, r.at);
        assert_eq!(r.sora_motion, Some("SGuardWalk"));
        // Jump: JumpZVelocity 420 under 980: apex ~ 90 cm, then the land clip.
        team.request(FriendLinkRequest::Input { kind: 193, stick: Vec3::ZERO, attack: false, jump: true, target: TargetId::NONE });
        let (mut top, mut motions) = (0.0f32, Vec::new());
        for _ in 0..90 {
            team.step(dt, &mut p, &mut host);
            let r = team.run.as_ref().unwrap();
            top = top.max(r.at.z);
            if motions.last() != Some(&r.sora_motion) {
                motions.push(r.sora_motion);
            }
        }
        assert!((top - 420.0 * 420.0 / (2.0 * GRAVITY)).abs() < 6.0, "apex {top}");
        assert!(motions.contains(&Some("SGuardJumpRise")) && motions.contains(&Some("SGuardFall")) && motions.contains(&Some("SGuardLand")), "{motions:?}");
        let z = team.run.as_ref().unwrap().at.z;
        assert!((z - x0.z).abs() < 3.0, "landed back at {z} (stood at {})", x0.z);
    }

    // ---- (D4)

    struct Foes(Vec<TargetInfo>);

    impl kh_core::host::Targets for Foes {
        fn enumerate(&self, out: &mut Vec<TargetInfo>) {
            out.extend(self.0.iter().copied());
        }
        fn info(&self, id: TargetId) -> Option<TargetInfo> {
            self.0.iter().find(|t| t.id == id).copied()
        }
        fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<kh_core::host::HurtShape>) {}
        fn apply_hit(&mut self, _hit: &kh_core::host::HitRecord) -> kh_core::host::HitResult {
            kh_core::host::HitResult::default()
        }
    }

    fn foe_at(id: u32, at: Vec3) -> TargetInfo {
        TargetInfo { id: TargetId(id), position: at, center: at + Vec3::Z * 90.0, ..foe(at).0 }
    }

    #[test]
    fn d4_missiles_take_their_targets_by_facing_lock_count_and_the_main_target() {
        // FUN_1412E43E0: A ahead (score 4), B to the side (score 2, x 5 as the main target).
        let (a, b) = (foe_at(1, Vec3::new(800.0, 0.0, 0.0)), foe_at(2, Vec3::new(0.0, 800.0, 0.0)));
        let enemies = vec![a, b];
        let mut locks = Vec::new();
        let m = TeamProj::new(ProjKind::Missile, 0, Vec3::ZERO, Vec3::X);
        let picks: Vec<(TargetId, bool)> = (0..4).map(|_| Team::flare_pick(&m, &enemies, Some(&b), &mut locks).unwrap()).collect();
        // B: 10, then 2 x 5 x 2/3 = 6.67 > 4; then 3.33 < A's 4: A (first on it); then A 4 x 2/3 = 2.67 < B 3.33.
        assert_eq!(picks, vec![(TargetId(2), true), (TargetId(2), false), (TargetId(1), true), (TargetId(2), false)]);
        // Without a main target the one ahead wins until its weight drops under the side one's.
        let mut locks = Vec::new();
        let picks: Vec<TargetId> = (0..3).map(|_| Team::flare_pick(&m, &enemies, None, &mut locks).unwrap().0).collect();
        assert_eq!(picks, vec![TargetId(1), TargetId(1), TargetId(2)], "4, 2.67 > 2, then 1.33 < 2");
    }

    #[test]
    fn d4_the_pawn_target_is_the_controllers_else_the_nearest_lockable_one() {
        let world = kh_core::test_world::TestWorld::floor();
        let mut null = crate::host::NullPartyHost::default();
        let far = foe_at(1, Vec3::new(5000.0, 0.0, 0.0));
        let near = foe_at(2, Vec3::new(300.0, 0.0, 0.0));
        let mut unlockable = foe_at(3, Vec3::new(100.0, 0.0, 0.0));
        unlockable.flags = TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::ENEMY);
        let mut targets = Foes(vec![far, near, unlockable]);
        let host = PartyHost { world: &world, targets: &mut targets, audio: &mut null.audio, effects: &mut null.effects };
        let p = Vec3::Z * SORA_CENTER_BONE_Z;
        // The controller's target as it is, with no range test.
        assert_eq!(Team::pawn_target(TargetId(1), &host, p, false, 1000.0).map(|t| t.id), Some(TargetId(1)));
        // Else the nearest with a lock-on component; none at or past `max`.
        assert_eq!(Team::pawn_target(TargetId::NONE, &host, p, false, 1.0e6).map(|t| t.id), Some(TargetId(2)));
        assert_eq!(Team::pawn_target(TargetId::NONE, &host, p, false, 200.0).map(|t| t.id), None);
    }

    #[test]
    fn d4_goofys_mesh_pitch_follows_the_rush_and_returns() {
        let mut team = Team::new();
        let g = MemberId(1);
        assert_eq!(team.mesh_pitch(g), None);
        // Thrown at -45 deg: the target is R(45 + ...): pitch + 90 = 45, alpha 6 dt per step.
        for _ in 0..120 {
            team.pitch_toward(g, -45.0 + 90.0, 6.0 / 60.0);
        }
        assert!((team.mesh_pitch(g).unwrap() - 45.0).abs() < 0.01);
        for _ in 0..120 {
            team.pitch_toward(g, 0.0, 15.0 / 60.0);
        }
        assert!(team.mesh_pitch(g).unwrap().abs() < 0.01);
        assert_eq!(team.mesh_pitch(MemberId(0)), None);
    }
}

// ------------------------------------------------------------------------------------------------ (DON-KIT)

/// (DON-KIT) DONALD'S TEAM ATTACKS AT RESONANCE 3 (documentation/DONALD_OVERHAUL.md 6.6, DON_PLAN R22): x `team.res3_mult`
/// (1.2) while any of a Donald's meters in the party is at Resonance 3, else 1.
pub fn don_res3_mult(party: &crate::party::Party) -> f32 {
    let d = crate::donald::data();
    let full = party.members().filter_map(|id| party.status(id)).filter_map(|s| s.donald.as_ref()).any(|st| st.core.full_power_any(&d));
    if full {
        crate::pad::donald::K.team_res3_mult
    } else {
        1.0
    }
}

/// (DON-KIT) A team attack's hit (its row of Sora's attack data) is one of Donald's: Flare Force, Donald Meteor, the Unisons
/// (with Sora) and Trinity Guard (with Goofy).
pub fn is_donald_team_row(row: &str) -> bool {
    row.starts_with("FR_FLAREFORCE") || row.starts_with("FR_CALLMETEOR") || row.starts_with("UNISON_") || row.starts_with("FR_SHIELDGUARDIAN")
}

/// (DON-KIT) THE MULTIPLIER OF ONE HIT (`HitRecord::attack_name`) of a team attack, for the hosts' damage route of Sora's
/// hits (the team shots are delivered by Sora's core: kh-core spawn_team_shot): `don_res3_mult` on Donald's team rows, 1 on
/// anything else.
pub fn don_team_hit_mult(party: &crate::party::Party, row: &str) -> f32 {
    if is_donald_team_row(row) && party.members().any(|id| party.member(id).is_some_and(|m| m.chara == Chara::Donald)) {
        don_res3_mult(party)
    } else {
        1.0
    }
}
