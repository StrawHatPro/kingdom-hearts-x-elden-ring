//! (SHOT-A) Meteor Shower (kind 5 `SF_SHOOTINGSTAR`, Shooting Star = weapon type 3's base kind; SHOT-B's 18 `BX_SHOOT` /
//! 23 `RE_SHOOT`, SHOT-C's 40 `LF_SHOOT`): update 0x1413BA090 and its stream spawner 0x1413C24D0. Rows `meteor.*` of
//! sheet sora-shotlocks (tools/shotlocks/meteor.js).
//!
//! What KH3 does (instructions: SHOT-B's listings dis_1413BA090.txt / dis_1413C24D0.txt, decompiled C
//! re/decomp/1413B/1413BA090.c; the delegate is ShootFlow vtable 0x14693CA58 + 0x3D0, Begin 0x1413A5CD0: kind 5 is its
//! default branch, name SF_SHOOTINGSTAR, group 0; the others name their projectile and group):
//! - sub 0: gravity off (pawn vt+0x1728(0)), Flying stopped (vt+0x17A0(5, 1)), the weapon presentation vt+0x1388(1, 0, 0)
//!   (not in the port, as states/shoot_flow.rs); Play `M_1475` (2.9 s; blend -1, mode 2, group +0xC0), voice group 0;
//!   face request (+0x84 bit 4); sub 1; LiftCheck(100 cm) -> sub 3.
//! - sub 3: LiftStep -> sub 4, then sub 1's code in the same tick.
//! - sub 1 (and 3): at M_1475's end `ShootFlowEnd` (group 0, a literal), voice group 0; sub 2.
//! - any other sub (2, and 4: the switch sends sub 4 here): at the clip's end vt+0x1388(0, 0, 0), kind 0x28: pawn
//!   vt+0x1E48(1.0) (pawn+0xE44 = 1.0; no reader found: not ported), Finish. So a LIFTED Meteor Shower (Sora within 100 cm
//!   of the floor) finishes at M_1475's end without ShootFlowEnd; from higher up ShootFlowEnd plays first.
//! - every tick, after Finish too: while flagsCC bit 0 (the shared Shot handler 0x1413B4A70 on M_1475's TresShot at 1.2 s:
//!   `sf.ring_active`) the spawner while +0x90 < N, else the bit off; then SetFace(false, 0) (vt+0x390 0x1413BDB60).
//! - THE STREAM (spawner 0x1413C24D0, [`spawner`]): +0xA0 counts down first (the projectile comes on the tick it reaches
//!   0); the target is locked[+0x90 mod lockedTargets] (no locked target: +0xD0); a dead one: NO spawn and no count (the
//!   same index is tried again next tick); the origin is the right-hand weapon actor's root (ATK_00 of the clip's socket
//!   track, the weapon's origin; ATK_01 the fallback), the direction normalize(origin - his lock-on point); one projectile
//!   of +0xB8 homing on the target, fired at once (`TresProjSS_Shoot` etc.: `TresProj_Ragnarok` subclasses, the helix of
//!   magic/projectile.rs with their own `rolling_*` rows); +0x90 += 1; +0xA0 = 0.066666 s (N < 8) / 0.033333 s (N < 15) /
//!   stays 0 (one a tick). The muzzle particle pawn+0x14E0 is not set for Sora (none). The projectiles are not added to
//!   the state's list +0x128 (no formation).

use glam::Vec3;

use crate::anim::{AnimId, NotifyEvent, PlayArgs, SEQUENCES};
use crate::host::TargetFlags;
use crate::magic::projectile::{self, SpawnArgs};
use crate::magic::shotlock::lock_point;
use crate::magic::spells::cmd;
use crate::magic::tables::Sheet;
use crate::pawn::{Ctx, Pawn};
use crate::states::shoot_flow::ShootFlow;

const S: Sheet = Sheet::Shotlocks;

/// The family runs (`magic/shotlocks::ready`).
pub(crate) const PORTED: bool = true;

/// The family's fields of the state object: none of its own (the update uses the common +0x90 / +0x94 / +0xA0 / +0xCC).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct State {}

/// After the common Begin and the per-kind set-up: nothing of its own (Begin stores the delegate, +0xB8 and +0xC0 only).
pub(crate) fn begin(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

/// The update delegate 0x1413BA090; `false` = the state finished this tick (no PostTick).
pub(crate) fn update(sf: &mut ShootFlow, _st: &mut State, cx: &mut Ctx, dt: f32) -> bool {
    let mut running = true;
    match sf.sub {
        0 => {
            let pawn = &mut *cx.pawn;
            pawn.cmc.gravity_off = true;
            pawn.cmc.stop_movement();
            pawn.cmc.set_flying();
            let clip = AnimId::from_name(S.text("meteor.clip")).unwrap_or(AnimId::M1475);
            super::play(sf, cx, clip);
            cx.set_voice_group_no(S.int("meteor.voice_group"));
            sf.face_request = true;
            sf.sub = 1;
            if sf.lift_check(cx, S.num("meteor.lift.height")) {
                sf.sub = 3;
            }
        }
        1 | 3 => {
            if sf.sub == 3 && sf.lift_step(cx, dt) {
                sf.sub = S.int("meteor.lift.sub_after") as u8;
            }
            if cx.pawn.anim.ended {
                // ShootFlowEnd with group 0 (not +0xC0).
                cx.pawn.anim.play_with(AnimId::ShootFlowEnd, PlayArgs::mode(2, -1.0));
                cx.set_voice_group_no(S.int("meteor.voice_group"));
                sf.sub = 2;
            }
        }
        _ => {
            if cx.pawn.anim.ended {
                // (vt+0x1388(0, 0, 0): not in the port.) Kind 0x28: pawn vt+0x1E48(1.0) -> pawn+0xE44 (SHOT-C: the timer).
                if sf.kind == 0x28 {
                    crate::states::damage::set_invincible(cx, 1.0); // (SHOT-C)
                }
                cx.pawn.queue.do_chain();
                running = false;
            }
        }
    }
    // The tail of every tick (after Finish too): the stream while flagsCC bit 0, then SetFace(false, 0).
    if sf.ring_active {
        if sf.spawn_index < sf.n {
            spawner(sf, cx, dt);
        } else {
            sf.ring_active = false;
        }
        sf.facing = false;
        sf.face_timer = 0.0;
    }
    running
}

/// The stream spawner 0x1413C24D0(dt): at most one projectile per call (see the module doc).
fn spawner(sf: &mut ShootFlow, cx: &mut Ctx, dt: f32) {
    if sf.ring_timer > 0.0 {
        sf.ring_timer -= dt;
        if sf.ring_timer > 0.0 {
            return;
        }
        sf.ring_timer = 0.0;
    }
    // locked[+0x90 mod lockedTargets] (the controller's list), else the state's first target; it must live.
    let locked = &cx.pawn.magic.aim.locked;
    let target = if locked.is_empty() { sf.first_target } else { locked[sf.spawn_index.max(0) as usize % locked.len()].target };
    if !cx.target(target).is_some_and(|t| t.flags.has(TargetFlags::ALIVE)) {
        return;
    }
    // (pawn vt+0x1330, the weapon drawn: Begin drew it.)
    let Some(def) = super::projectile_named(cx, sf.proj_name) else { return };
    let origin = weapon_root(cx.pawn).or_else(|| crate::states::fire_magic::muzzle(cx.pawn)).unwrap_or_else(|| lock_point(cx));
    let d = origin - lock_point(cx);
    let min = S.text("meteor.stream.min_len_sq").parse::<f32>().unwrap_or(0.0);
    let direction = if d.length_squared() > min { d.normalize() } else { d };
    projectile::spawn(cx, &SpawnArgs { def, position: origin, direction, cmd: cmd::SHOOTFLOW, shoot_flow: sf.kind, finish: false, situation: false, full_mp: false, combo: [0; 4], target, attached: None, fire: true });
    sf.spawn_index += 1;
    if sf.n < S.int("meteor.stream.small_below") {
        sf.ring_timer = S.num("meteor.stream.small_interval");
    } else if sf.n < S.int("meteor.stream.mid_below") {
        sf.ring_timer = S.num("meteor.stream.mid_interval");
    }
}

/// The right-hand weapon actor's root (pawn vt+0x1428(1) -> its RootComponent's location): the weapon's origin ATK_00 of
/// the playing clip's socket track (`meteor.stream.origin_socket`), turned into the world; `None` without a track.
fn weapon_root(pawn: &Pawn) -> Option<Vec3> {
    let request = pawn.anim.request();
    let mut sum = Vec3::ZERO;
    let mut total = 0.0;
    for (name, weight) in pawn.anim.name.sample_weights(request.axis) {
        if weight <= 0.0 {
            continue;
        }
        let Some(sequence) = SEQUENCES.iter().find(|s| s.name == name) else { continue };
        let track = sequence.sockets;
        let Some(last) = track.len().checked_sub(1) else { continue };
        let f = if sequence.length > 0.0 { (pawn.anim.time / sequence.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
        let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
        let (a, b) = (track[i], track[(i + 1).min(last)]);
        sum += Vec3::new(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t) * weight;
        total += weight;
    }
    if total <= 0.0 {
        return None;
    }
    let local = sum / total;
    let (sin, cos) = pawn.yaw.to_radians().sin_cos();
    Some(pawn.cmc.location + Vec3::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos, local.z))
}

/// Every notify event of a pose tick: nothing of its own (the shared Shot handler starts the stream).
pub(crate) fn on_notify(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx, _event: &NotifyEvent) {}

/// Before the common End's release of the aim mode: nothing of its own.
pub(crate) fn end(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

/// The family's spawn point (the stream's origin: the right-hand weapon actor's root); the Shot handler spawns nothing
/// for these kinds.
pub(crate) fn muzzle(_sf: &ShootFlow, _st: &State, cx: &Ctx, _index: usize) -> Option<Vec3> {
    weapon_root(cx.pawn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_kh3_s() {
        for k in [5, 18, 23, 40] {
            assert_eq!(super::super::family(k), "meteor", "kind {k}");
            assert!(super::super::ready(k));
        }
        assert_eq!((super::super::base_kind(3), S.text("meteor.k5.proj_class"), S.text("meteor.k5.attack_row")), (5, "TresProjSS_Shoot_C", "SF_SHOOTINGSTAR"));
        assert_eq!((S.num("meteor.lift.height"), S.num("meteor.shot_time"), S.int("meteor.lift.sub_after")), (100.0, 1.2, 4));
        assert_eq!((S.int("meteor.stream.small_below"), S.int("meteor.stream.mid_below")), (8, 15));
        assert!((S.num("meteor.stream.small_interval") - 0.066666).abs() < 1.0e-6 && (S.num("meteor.stream.mid_interval") - 0.033333).abs() < 1.0e-6);
        assert_eq!(S.text("meteor.stream.min_len_sq").parse::<f32>(), Ok(1.0e-8));
        assert!(AnimId::from_name(S.text("meteor.clip")).is_some());
        assert!(SEQUENCES.iter().any(|s| s.name == "1475" && !s.sockets.is_empty()), "M_1475's socket track");
    }
}
