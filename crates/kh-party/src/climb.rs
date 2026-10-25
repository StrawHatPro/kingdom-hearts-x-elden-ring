//! (B5) The friends' ledge states, transcribed from the decompiled C:
//! - `UTresNpcState_Climbing` (`TSID_STATE_CLIMBING` 7, vtable 0x146837448: Init 0x141247B10, Begin
//!   0x141241270, Tick 0x141249650, End 0x141244460, CanChange slot 0x388 0x1412405A0, busy slot 0x298
//!   0x141252F30). Requested through iface vt+0x538 (0x141276490) with the NPC climb probe, which is the
//!   non-player branch of FUN_140634E60: FUN_140636BF0 (a `TresPlayerPawnBase` takes the vault probe of the
//!   actions spec II.6, gated by `m_bEnableClimbing`; every other pawn takes this one, ungated). Callers:
//!   the Run tick 0x140442430 (Run +0xBC bit 0x40, set by its Init 0x14043E670), the SlopeSlide tick
//!   0x14126C950 sub 0, the SuperSlide tick 0x14126D3B0 sub 0 (then +0xB5: the SonicSlideClimbing clips).
//! - `UTresNpcState_Dangle` : `UTresCharState_Dangle` (`TSID_STATE_DANGLE` 6, vtable 0x1468381D0: thunks to
//!   the base Init 0x14150DEA0, Begin 0x141508DE0, Tick 0x14150F220, End 0x14150BB80). Requested through
//!   iface vt+0x208 (0x141276580) by the jump's air probe 0x140446070 when the launch aimed the jump at a
//!   ledge (`ledge_arc`, the +0x132 branch of 0x140433FD0): the Run tick does NOT request it.
//!
//! Positions are the actor location (`cmc.location`, foot level), as kh-core's Sora ports.

use crate::host::PartyHost;
use crate::ids::Chara;
use crate::pawn::{forward, horizontal, wrap180, yaw_of, NpcPawn, StepCtx};
use crate::states::NpcStateId;
use crate::tables;
use glam::{Vec2, Vec3};
use kh_core::actions::ACTION_PARAMS as A;
use kh_core::cmc::MovementMode;
use kh_core::{surface, World};

fn p(id: &str) -> f32 {
    tables::param(id)
}

fn rot_dir(pitch: f32, yaw: f32) -> Vec3 {
    let (sp, cp) = pitch.to_radians().sin_cos();
    let (sy, cy) = yaw.to_radians().sin_cos();
    Vec3::new(cp * cy, cp * sy, sp)
}

// ============================================================================ the climb probe

/// What FUN_140636BF0 hands back (the request 0x141276490 copies it into the state +0x88..+0xAD).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClimbHit {
    /// +0x88 / +0x90: the obstacle (the port has components only).
    pub component: u32,
    /// +0x98 / +0xA0: the top point; +0xA0 (its Z) is the lift target.
    pub point: Vec3,
    /// +0xA4: the yaw toward it (XY).
    pub yaw: f32,
    /// +0xA8: the height above the feet.
    pub height: f32,
    /// +0xAC: higher than 150; +0xAD: higher than 100 (and not high).
    pub high: bool,
    pub middle: bool,
}

/// The NPC climb probe FUN_140636BF0: a sphere of 35 (PxSphereGeometry {0, 35}, FUN_144526380 multi
/// sweep, channels 0 / 1 / 0x12 blocking) dropped from 260 to 80 above the feet on the axis 70 cm ahead
/// (the actor rotation's vector, XY); the hits in order, a start-penetrating one ends the search; the first
/// on another component flagged `SQEX_bEnableAutoRun` (component vt+0x520) with a walkable impact (pawn
/// vt+0xD20) whose height above the feet is 45..225, whose surface is no steeper than 35 deg (the impact
/// normal's rotator pitched by -90: its vector's elevation >= -35) and that lies within 45 deg of the facing
/// (dot >= 0.707107, XY direction to the impact point).
pub fn climb_probe(pw: &NpcPawn, w: &dyn World) -> Option<ClimbHit> {
    let l = pw.position();
    let f = rot_dir(pw.trav.pitch, pw.yaw);
    let a = Vec3::new(l.x + f.x * p("climb.probe_ahead"), l.y + f.y * p("climb.probe_ahead"), l.z + p("climb.probe_top"));
    let b = Vec3::new(a.x, a.y, a.z - p("climb.probe_drop"));
    let r = p("climb.probe_radius");
    let walkable = pw.params.floor_walkable_z;
    for hit in w.sweep_capsule_all(a, b, r, r) {
        if hit.start_penetrating {
            break;
        }
        if w.surface_flags(hit.component) & surface::AUTO_RUN == 0 || hit.impact_normal.z < walkable {
            continue;
        }
        let h = hit.position.z - l.z;
        if !(p("climb.height_min")..=p("climb.height_max")).contains(&h) {
            continue;
        }
        let n = hit.impact_normal;
        let pitch = n.z.atan2(n.truncate().length()).to_degrees();
        let v = rot_dir(pitch - 90.0, n.y.atan2(n.x).to_degrees());
        if -(v.z.atan2(v.truncate().length()).to_degrees()) > p("climb.slope_max") {
            continue;
        }
        let flat = Vec2::new(hit.position.x - l.x, hit.position.y - l.y);
        let dir = if flat.length_squared() >= 1.0e-8 { flat.normalize() } else { Vec2::ZERO };
        if dir.x * f.x + dir.y * f.y < p("climb.facing_dot") {
            continue;
        }
        return Some(ClimbHit {
            component: hit.component,
            point: hit.position,
            yaw: dir.y.atan2(dir.x).to_degrees(),
            height: h,
            high: h > p("climb.high"),
            middle: h <= p("climb.high") && h > p("climb.middle"),
        });
    }
    None
}

/// `UTresNpcState_Climbing` fields (Init 0x141247B10).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ClimbRun {
    pub sub: u8,
    pub hit: Option<ClimbHit>,
    /// +0x84 bit 1 m_bReverseLeg (the request's).
    pub reverse_leg: bool,
    /// +0xAE: the AI move fed this tick (iface vt+0x118, magnitude > 0.5), cleared every tick.
    pub input: bool,
    /// +0xB0: the lift factor per 60 Hz tick (1.0 at Init; 0.1 / 0.25 / 0.3 at the first tick).
    pub lift: f32,
    /// +0xB4: a blend-in of 0.1 for the clip (0 from the Run); +0xB5: from the SuperSlide (the
    /// SonicSlideClimbing clips, the SuperSlide again at the end).
    pub blend: bool,
    pub sonic: bool,
    /// TresStateBase m_bEnableTraction (the TresAnimNotifyState_Traction window; the tick clears it) and
    /// m_bEnableChange (TresAnimNotify_EnableChange).
    pub traction: bool,
    pub enable_change: bool,
}

/// The climb request (iface vt+0x538 0x141276490): state 7 with the probe's facts and the leg; Init; the
/// running state ends (vt+0x270 of the caller) and Begin 0x141241270 runs: vt+0x1A78(0) (a no-op for the
/// friends), StopMovement + the input cleared (vt+0x16E8), Flying (vt+0x17A0(5, 1)), gravity off
/// (vt+0x1728(0): movement +0xA3C bit 1), m_bEnableTraction = 0.
pub fn request_climb(pw: &mut NpcPawn, hit: ClimbHit, reverse_leg: bool, sonic: bool) {
    pw.end_action();
    pw.enter(NpcStateId::Climbing);
    pw.trav.climb = ClimbRun { sub: 0, hit: Some(hit), reverse_leg, input: false, lift: 1.0, blend: false, sonic, traction: false, enable_change: false };
    pw.cmc.stop_movement();
    pw.cmc.control_input = Vec3::ZERO;
    pw.cmc.set_flying();
    pw.cmc.gravity_off = true;
}

/// vt+0x388 0x1412405A0: the landing clip (sub 4) with m_bEnableChange.
pub fn climb_can_change(pw: &NpcPawn) -> bool {
    pw.trav.climb.sub == 4 && pw.trav.climb.enable_change
}

fn play(pw: &mut NpcPawn, name: &'static str, blend: f32) -> bool {
    if tables::anim_set(pw.chara).get(name).is_none() {
        return false;
    }
    let c = tables::clip(pw.chara, name);
    pw.anim.play(c, blend);
    pw.hit_list.clear();
    true
}

fn has(pw: &NpcPawn, name: &str) -> bool {
    tables::anim_set(pw.chara).get(name).is_some()
}

/// Climbing Tick 0x141249650.
pub fn climb_tick(pw: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let c = pw.trav.climb;
    let mut done = false;
    match c.sub {
        0 => {
            // the clip by the obstacle's class (blend +0xB4 ? 0.1 : 0, mode 2); +0xB0 the lift factor
            let blend = if c.blend { 0.1 } else { 0.0 };
            let hit = c.hit.unwrap_or(ClimbHit { component: 0, point: pw.position(), yaw: pw.yaw, height: 0.0, high: false, middle: false });
            if !hit.high {
                let name: &'static str = if !hit.middle {
                    // ClimbingR either leg (the C picks ClimbingR for both)
                    if c.sonic {
                        "SonicSlideClimbing"
                    } else {
                        "ClimbingR"
                    }
                } else if c.sonic {
                    "SonicSlideClimbingM"
                } else if has(pw, "ClimbingMR") {
                    "ClimbingMR"
                } else {
                    "ClimbingR"
                };
                play(pw, name, blend);
                pw.trav.climb.lift = if hit.middle { p("climb.lift_middle") } else { p("climb.lift_low") };
                pw.trav.climb.sub = 2;
            } else {
                let name: &'static str = if c.sonic {
                    "SonicSlideClimbingH"
                } else if has(pw, "ClimbingH") {
                    "ClimbingH"
                } else if has(pw, "ClimbingMR") {
                    "ClimbingMR"
                } else {
                    "ClimbingR"
                };
                play(pw, name, blend);
                pw.trav.climb.reverse_leg = false;
                pw.trav.climb.sub = 1;
                pw.trav.climb.lift = p("climb.lift_high");
            }
        }
        1 | 2 => {
            // traction off: wait for the clip's end; on: lift Z toward +0xA0 by +0xB0 of the gap per tick until
            // the gap closes (traction off), then the drop: SetBase(none), Falling, gravity on, sub 3.
            let ended = pw.anim.finished;
            let mut drop = ended;
            if !ended && c.traction {
                let target = c.hit.map_or(pw.position().z, |h| h.point.z);
                let gap = target - pw.position().z;
                if gap > 0.0 {
                    // PORT: the game's per-tick fraction (60 Hz) made frame-rate independent.
                    let k = 1.0 - (1.0 - c.lift.clamp(0.0, 1.0)).powf(cx.dt * 60.0);
                    pw.cmc.location.z += gap * k;
                } else {
                    pw.trav.climb.traction = false;
                    drop = true;
                }
            } else if !ended {
                drop = false;
            }
            if drop {
                pw.cmc.set_falling();
                pw.cmc.gravity_off = false;
                pw.trav.climb.sub = 3;
            }
        }
        3 => {
            if pw.anim.finished {
                // StopMovement + input cleared; Walking (vt+0x17A0(1, 1)); LightLandL / R by the leg (blend -1,
                // mode 2), sub 4; from the SuperSlide: done.
                pw.cmc.stop_movement();
                pw.cmc.control_input = Vec3::ZERO;
                if pw.cmc.mode != MovementMode::Walking {
                    pw.cmc.mode = MovementMode::Walking;
                    pw.cmc.velocity.z = 0.0;
                }
                if !c.sonic {
                    let name = if c.reverse_leg { "LightLandL" } else { "LightLandR" };
                    play(pw, name, -1.0);
                    pw.trav.climb.sub = 4;
                } else {
                    done = true;
                }
            }
        }
        _ => {
            if pw.anim.finished {
                done = true;
            }
        }
    }
    pw.trav.climb.input = false;
    if done {
        if c.sonic {
            // iface vt+0x248(1) (the SuperSlide request 0x141276B70) and its m_nSubStep (+0x78) = 4
            climb_end(pw);
            crate::traverse::begin_super_slide_at(pw, 4);
            return;
        }
        let _ = host;
        crate::traverse::back_pub(pw);
    }
}

/// End 0x141244460: SetBase(none) (vt+0xA60(0, 0, 1)), Walking (vt+0x17A0(1, 1)), gravity on (vt+0x1728(1)).
pub fn climb_end(pw: &mut NpcPawn) {
    if pw.cmc.mode != MovementMode::Walking {
        pw.cmc.mode = MovementMode::Walking;
        pw.cmc.velocity.z = 0.0;
    }
    pw.cmc.gravity_off = false;
}

// ============================================================================ the ledge probe and Dangle

/// A ledge found by the probe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ledge {
    pub component: u32,
    /// The hang position (actor location) and yaw.
    pub position: Vec3,
    pub yaw: f32,
    /// The contact on the ledge's edge.
    pub contact: Vec3,
}

/// The friend's `m_DangleOffsetPos` (n_ex001_PawnBase (-30, 0, -70), n_ex002_PawnBase (-40, 0, -150)).
pub fn dangle_offset(chara: Chara) -> Vec3 {
    Vec3::new(tables::cparam(chara, "dangle.offset_x"), 0.0, tables::cparam(chara, "dangle.offset_z"))
}

/// The jump's ledge probe height (0x140446070 / 0x140433FD0): 275 - m_DangleOffsetPos.Z (pawn vt+0xE88
/// 0x140541BA0) - 150.
pub fn dangle_probe_up(chara: Chara) -> f32 {
    p("dangle.probe_top") - dangle_offset(chara).z - p("dangle.probe_below")
}

/// (party I1) The ledge probe FUN_140638900 for a pawn that is NOT a TresPlayerPawnBase (the friends): the IsA test
/// at 0x140638A70 leaves the pawn pointer null, so the loop at 0x140638B87 goes straight to attempt 3
/// (0x140639360, uStack_404 = 3): the player's attempts 0 / 1 / 2 (the radius-50 sphere 50 cm ahead, yawed 0 / +45
/// / -45) never run for an NPC. Attempt 3, with the callers' arguments (the air probes 0x140446070 through
/// FUN_140638740 = the actor's location and rotation; the launch 0x140433FD0 = the arc point's XY, its Z - half
/// height, the yaw toward the target): A = L + F x fwd (0) + up Z (275 - m_DangleOffsetPos.Z - 150), B = A - 50 Z,
/// a sphere of 1.5 x the root's larger horizontal extent (root +0x288 / +0x28C: the capsule radius; Donald 45,
/// Goofy 60) swept A -> B on the Pawn channel; the first blocking hit that does not start penetrating, is not the
/// pawn, has a component flagged SQEX_bEnableDangle (vt+0x4F0) and is walkable (pawn vt+0xD20); dir =
/// normalize2D(ImpactPoint - B) (zero when degenerate: yaw 0), the hang point = ImpactPoint + RotYaw(yaw) x
/// m_DangleOffsetPos (vt+0xE88); SPACE (0x1406395A2): a sphere of |offset.x| - 10 from (hang.xy, hang.z - offset.z)
/// to (hang.xy, hang.z - 10), any blocking hit of another actor (or a start-penetrating one) fails; GRIP
/// (0x140639B06): a sphere of 5 from (hang.xy, ImpactPoint.z + 5) toward the ImpactPoint over dist2D - 2.5, any
/// blocking hit fails. Attempt 3 is the last: a failure is final.
pub fn ledge_probe(pw: &NpcPawn, w: &dyn World, location: Vec3, yaw: f32, up: f32, down: f32) -> Option<Ledge> {
    let offset = dangle_offset(pw.chara);
    let radius = ledge_probe_radius(pw);
    let walkable_z = pw.params.floor_walkable_z;
    let sphere_all = |a: Vec3, b: Vec3, r: f32| w.sweep_capsule_all(a, b, r, r);
    let a = location + forward(yaw) * p("dangle.probe_forward_npc") + Vec3::Z * up;
    let b = a - Vec3::Z * down;
    let hit = sphere_all(a, b, radius).into_iter().find(|h| !h.start_penetrating && w.surface_flags(h.component) & surface::DANGLE != 0 && h.impact_normal.z >= walkable_z)?;
    let flat = (hit.position - b).truncate();
    let toward = if flat.length_squared() >= 1.0e-8 { flat.normalize().extend(0.0) } else { Vec3::ZERO };
    let lyaw = if toward == Vec3::ZERO { 0.0 } else { yaw_of(toward) };
    let (s, c) = lyaw.to_radians().sin_cos();
    let position = hit.position + Vec3::new(offset.x * c, offset.x * s, offset.z);
    let top = Vec3::new(position.x, position.y, position.z - offset.z);
    let space_r = (offset.x.abs() - 10.0).max(1.0);
    if !sphere_all(top, Vec3::new(position.x, position.y, position.z - 10.0), space_r).is_empty() {
        return None;
    }
    let grip = Vec3::new(position.x, position.y, hit.position.z + 5.0);
    let reach = (hit.position - position).truncate().length() - 2.5;
    let along = Vec3::new(hit.position.x - position.x, hit.position.y - position.y, 0.0).normalize_or_zero();
    if reach > 0.0 && !sphere_all(grip, grip + along * reach, A.dangle_grip_radius).is_empty() {
        return None;
    }
    Some(Ledge { component: hit.component, position, yaw: lyaw, contact: hit.position })
}

/// (party I1) The probe's sphere: 1.5 x the root's larger horizontal extent (0x140446070: max(root +0x288, +0x28C)
/// x 1.5; 0x140433FD0: fStack_228 x 1.5, the same max).
pub fn ledge_probe_radius(pw: &NpcPawn) -> f32 {
    pw.params.capsule_radius * p("dangle.probe_radius_scale_npc")
}

/// The launch's ledge branch (0x140433FD0, with a target more than 500 above him and the Dangle state
/// present): the arc of the jump to (target - half height) with h0 = h_min = 0 (FUN_140578330) stepped at
/// 0.05 s from the location + half height; in the upper half of the rise (prev Z >= (Z + target Z) / 2) the
/// capsule (the root's larger extent, the half height; channel 3) is swept point to point until it hits; once
/// it has, every falling step probes for a ledge (FUN_140638900 at the hit's XY, the arc Z - half height, the
/// yaw toward the target; up 275 - offset.Z - 150, down 50); the search ends when the arc falls to 3 half
/// heights below the target. A ledge whose top is within a half height of the target's Z: the jump to (target
/// - half height) with h0 = h_min = 0, +0x132 = 1, MaxWalkSpeed |v.xy|, JumpZVelocity v.z, DoJump (+0xA6 =
/// its answer). `None`: the plain launch.
pub fn ledge_arc(pw: &mut NpcPawn, target: Vec3, w: &dyn World) -> Option<bool> {
    let l = pw.position();
    let half = pw.params.capsule_half_height;
    let ext = pw.params.capsule_radius;
    let g = pw.cmc.gravity_z(&pw.params);
    let (s_up, s_dur, t_dur) = (pw.params.gravity_jump_up_scale, pw.params.gravity_jump_duration_scale, pw.params.gravity_jump_duration_time);
    let aim = target - Vec3::Z * half;
    let v = crate::actions::jump_velocity(l, aim, 0.0, 0.0, g, s_up, s_dur, t_dur);
    let d = horizontal(target - l);
    let yaw = if d.length_squared() >= 1.0e-8 { yaw_of(d) } else { 0.0 };
    let up = dangle_probe_up(pw.chara);
    let mut prev = Vec3::new(l.x, l.y, l.z + half);
    let mut hit: Option<Vec3> = None;
    let mut t = p("jump.arc_step");
    let mut found: Option<Ledge> = None;
    while t < 30.0 {
        let pz = v.z * t - (-g) * 0.5 * t * t + (l.z + half);
        let pt = Vec3::new(l.x + v.x * t, l.y + v.y * t, pz);
        if pt.z <= prev.z && pt.z <= target.z - half * 3.0 {
            break;
        }
        match hit {
            Some(h) => {
                if pt.z <= prev.z {
                    if let Some(ledge) = ledge_probe(pw, w, Vec3::new(h.x, h.y, pt.z - half), yaw, up, p("dangle.probe_down")) {
                        found = Some(ledge);
                        break;
                    }
                }
            }
            None => {
                if prev.z < pt.z && (l.z + target.z) * 0.5 <= prev.z {
                    if let Some(sw) = w.sweep_capsule(prev, pt, ext, half) {
                        hit = Some(prev + (pt - prev) * sw.time.clamp(0.0, 1.0));
                    }
                }
            }
        }
        prev = pt;
        t += p("jump.arc_step");
    }
    let ledge = found?;
    if (target.z - (ledge.position.z - dangle_offset(pw.chara).z)).abs() >= half {
        return None;
    }
    let v2 = crate::actions::jump_velocity(l, aim, 0.0, 0.0, g, s_up, s_dur, t_dur);
    pw.trav.jump.dangle_ok = true;
    pw.cmc.max_walk_speed = Vec2::new(v2.x, v2.y).length();
    pw.cmc.jump_z_velocity = v2.z;
    pw.cmc.do_jump();
    Some(true)
}

/// `UTresCharState_Dangle` fields (Init 0x14150DEA0, as kh-core's port names them).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct DangleRun {
    /// 0 GripStart, 1 GripLoop, 2 the climb-up (GripRise), 3 failed (ends at once).
    pub sub: u8,
    pub component: u32,
    pub position: Vec3,
    pub yaw: f32,
    /// +0xB4 / +0xB8: the AI move fed this tick (magnitude > 0.5) and its yaw.
    pub stick: bool,
    pub stick_yaw: f32,
    /// +0xBC: pushed from a wall run (straight to the climb-up; never for the friends' requests).
    pub from_wall_run: bool,
    /// +0xBD bit 2: the Begin moved him down off a lip (the climb-up levels the rotation).
    pub level: bool,
    /// TresStateBase m_bEnableChange (GripStart's TresAnimNotify_EnableChange).
    pub enable_change: bool,
    /// (party I1) +0xA4 with +0xBD bit 0: the hang point in the component's space (push 0x141444810: the inverse of
    /// ComponentToWorld when the component is alive), and the based movement of a movable ledge: the pawn's location
    /// in the base's space and the base's yaw when it was taken (vt+0xA60(component, 0, 1) in Begin).
    pub local: Option<Vec3>,
    pub based: Option<(Vec3, f32)>,
}

/// `RequestDangle` (iface vt+0x208 0x141276580): state 6, +0x88 / +0x90 the ledge, +0x98 the hang point, +0xB0
/// the yaw, +0xBC the wall-run variant; the running state ends; Begin 0x141508DE0.
pub fn request_dangle(pw: &mut NpcPawn, ledge: Ledge, from_wall_run: bool, w: &dyn World) {
    pw.end_action();
    pw.enter(NpcStateId::Dangle);
    // (party I1) +0xA4 = the hang point in the component's space while the component lives (+0xBD bit 0)
    let local = w.component_motion(ledge.component).map(|m| m.to_local(ledge.position));
    pw.trav.dangle = DangleRun { sub: 0, component: ledge.component, position: ledge.position, yaw: ledge.yaw, from_wall_run, local, ..DangleRun::default() };
    dangle_begin(pw, w);
}

/// Begin 0x141508DE0 ((party I1) with the movable-base branches: +0xA4 re-derived, the support test FUN_1406385E0, the base): the body at the
/// hang point swept there from 10 cm behind (kh-core's reading of the pawn vt+0x9D8 tests): a start-penetrating
/// hit fails (sub 3) unless from a wall run, otherwise he stops short by what is left; SetActorLocationAndRotation
/// (FUN_143EAC700); StopMovement; vt+0xDC8(1); Custom (6) and the base = the ledge (vt+0xA60), `GripStart`, sub
/// 0; from a wall run: Flying, `GripRise`, sub 2. m_bEnableChange = 0.
fn dangle_begin(pw: &mut NpcPawn, w: &dyn World) {
    let mut d = pw.trav.dangle;
    d.enable_change = false;
    // (party I1) +0xBD bit 0: +0x98 = ComponentToWorld x (+0xA4 x scale) (a moving base carried the hang point)
    let motion = w.component_motion(d.component);
    if let (Some(m), Some(local)) = (motion, d.local) {
        d.position = m.to_world(local);
    }
    let back = forward(d.yaw) * -p("dangle.begin_backoff");
    let centre = d.position + Vec3::Z * pw.params.capsule_center_z;
    let mut ok = true;
    if let Some(hit) = w.sweep_capsule(centre + back, centre, pw.params.capsule_radius, pw.params.capsule_half_height) {
        if hit.start_penetrating {
            ok = d.from_wall_run;
        } else {
            d.position += back * (1.0 - hit.time).clamp(0.0, 1.0);
        }
    }
    // (party I1) variant 0 on a Movable component (+0x348 == 2): the support test FUN_1406385E0(pawn, T((0, yaw, 0),
    // +0x98, root scale), 10) (FUN_14064A090) must find the ledge top under the contact
    if ok && !d.from_wall_run && motion.is_some_and(|m| m.movable) {
        ok = ledge_supports(pw, w, d.position, d.yaw);
    }
    if !ok {
        d.sub = 3;
        pw.trav.dangle = d;
        return;
    }
    pw.cmc.location = d.position;
    pw.yaw = wrap180(d.yaw);
    // (party I1) the base = the ledge (vt+0xA60(component, 0, 1)): a movable one carries him (UE based movement)
    d.based = motion.filter(|m| m.movable).map(|m| (m.to_local(d.position), yaw_of(m.rotation * Vec3::X)));
    pw.cmc.stop_movement();
    pw.cmc.control_input = Vec3::ZERO;
    if d.from_wall_run {
        pw.cmc.set_flying();
        play(pw, "GripRise", -1.0);
        d.sub = 2;
    } else {
        pw.cmc.set_custom();
        play(pw, "GripStart", -1.0);
        d.sub = 0;
    }
    pw.trav.dangle = d;
}

/// The feed (iface vt+0x118 0x141270D90, state 6): magnitude > 0.5 -> +0xB8 = the yaw, +0xB4 = 1.
pub fn dangle_feed(pw: &mut NpcPawn, yaw: f32, magnitude: f32) {
    if magnitude > p("dangle.input_min") {
        pw.trav.dangle.stick = true;
        pw.trav.dangle.stick_yaw = yaw;
    }
}

/// Tick 0x14150F220 for an AI-input pawn: pawn vt+0x1A20 is (vt+0x1A18() - 1) < 2, the controller's
/// m_CharInputType (ATresAIController +0x4E0, TRES_CHAR_INPUT_AI = 0 for the friends) being PAD / DEBUG_PAD;
/// for the friends it is false, so the input climbs within 90 deg of the facing and lets go beyond. Sub 0: the
/// GripStart clip's end -> `GripLoop` (mode 3), sub 1; the input is read from m_bEnableChange on (sub 0) or in
/// sub 1. CLIMB: the cool-down pawn +0x6F8 = 0 (vt+0x2380), Flying, `GripRise` (mode 2), sub 2. RELEASE: the
/// cool-down max(0.6) (vt+0x2378), iface vt+0x560 (a no-op 0x14041F1C0 for the friends), the state ends (he
/// falls: End). STAY: the cool-down max(0.6). Sub 2: GripRise's end ends the state; sub 3 ends at once.
pub fn dangle_tick(pw: &mut NpcPawn, cx: &StepCtx, w: &dyn World) {
    // (party I1) the based movement of a movable ledge (the movement tick runs before the state's): the pawn keeps
    // his place in the base's space and turns with its yaw
    if let Some((local, base_yaw)) = pw.trav.dangle.based {
        match w.component_motion(pw.trav.dangle.component) {
            Some(m) => {
                pw.cmc.location = m.to_world(local);
                let now = yaw_of(m.rotation * Vec3::X);
                pw.yaw = wrap180(pw.yaw + wrap180(now - base_yaw));
                pw.trav.dangle.based = Some((local, now));
            }
            None => pw.trav.dangle.based = None,
        }
    }
    let d = pw.trav.dangle;
    let mut done = false;
    match d.sub {
        0 | 1 => {
            let mut read = d.sub == 1;
            if d.sub == 0 {
                if pw.anim.finished {
                    let c = tables::clip(pw.chara, "GripLoop");
                    pw.anim.play_loop(c, -1.0);
                    pw.trav.dangle.sub = 1;
                }
                read = d.enable_change;
            }
            if read {
                // (party I1) a pad-driven member (stream P: a PlayerController answers input type 1 to vt+0x1A18,
                // so vt+0x1A20 is true) takes the tick's pad branch: climb under 45 (0x14150F4D8), let go from 135
                // (0x14150F4E1), stay between; the AI friends (TRES_CHAR_INPUT_AI) climb under 90, else let go.
                let a = wrap180(d.stick_yaw - pw.yaw).abs();
                let (climb_max, release_min) = if pw.pad.is_some() { (A.dangle_climb_angle_pad, A.dangle_release_angle_pad) } else { (p("dangle.climb_angle_ai"), p("dangle.climb_angle_ai")) };
                if !d.stick || (a >= climb_max && a < release_min) {
                    pw.trav.dangle_cooldown = pw.trav.dangle_cooldown.max(p("dangle.cooldown"));
                    // (party I1) STAY on a movable component (+0x90 alive, 0x14150F5BB): FUN_140638690(pawn, 10.0) =
                    // the support test at the actor's transform; no ledge top under the contact -> Fall, the state ends
                    if w.component_motion(d.component).is_some_and(|m| m.movable) && !ledge_supports(pw, w, pw.position(), pw.yaw) {
                        done = true;
                    }
                } else if a < climb_max {
                    pw.trav.dangle_cooldown = 0.0;
                    pw.cmc.set_flying();
                    play(pw, "GripRise", -1.0);
                    pw.trav.dangle.sub = 2;
                } else {
                    pw.trav.dangle_cooldown = pw.trav.dangle_cooldown.max(p("dangle.cooldown"));
                    done = true;
                }
            }
        }
        2 => {
            if d.level {
                // vt+0xDB0 toward (0, yaw, 0) alpha dt x 6: the pitch / roll levelled (yaw only here)
                let _ = cx;
            }
            if pw.anim.finished {
                done = true;
            }
        }
        _ => done = true,
    }
    pw.trav.dangle.stick = false;
    if done {
        crate::traverse::back_pub(pw);
    }
}

/// (party I1) FUN_14064A090 (through FUN_1406385E0 / FUN_140638690) with radius 10: r = max(10 - 5, 0) = 5; the point
/// T x (r / 2 - offset.x, -offset.y, -offset.z) (2.5 cm past the contact, on the ledge top); a sphere of 10 swept from
/// the point + (r + 10) Z down to the point on the pawn's channel: a blocking walkable hit (pawn vt+0xD20) = supported.
pub fn ledge_supports(pw: &NpcPawn, w: &dyn World, feet: Vec3, yaw: f32) -> bool {
    let off = dangle_offset(pw.chara);
    let r = (10.0f32 - 5.0).max(0.0);
    let (s, c) = yaw.to_radians().sin_cos();
    let lx = r * 0.5 - off.x;
    let point = feet + Vec3::new(lx * c, lx * s, -off.z);
    let start = point + Vec3::Z * (r + 10.0);
    w.sweep_capsule_all(start, point, 10.0, 10.0).into_iter().any(|h| h.impact_normal.z >= pw.params.floor_walkable_z)
}

/// End 0x14150BB80: Falling (vt+0x17A0(3)); the weapon re-drawn by vt+0x1340 for some next states (not modelled).
pub fn dangle_end(pw: &mut NpcPawn) {
    if pw.trav.dangle.sub != 3 {
        pw.cmc.set_falling();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_jump_ledge_probe_height_follows_the_dangle_offsets() {
        // 275 - (-70) - 150 (Donald), 275 - (-150) - 150 (Goofy)
        assert_eq!(dangle_probe_up(Chara::Donald), 195.0);
        assert_eq!(dangle_probe_up(Chara::Goofy), 275.0);
    }
}
