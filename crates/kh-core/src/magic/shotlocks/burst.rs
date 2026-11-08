//! (SHOT-B) Diamond Dust (kind 14 `FZ_BURST`, Crystal Snow = weapon type 7's base kind) and Blade Storm (kind 16
//! `CA_BURST`, Wheel of Fate = weapon type 10's base kind); KH3 also runs 0x1B `SL_SHOT` / 0x29 `DF_BURST` with this
//! update. Update 0x1413BADB0 and its spawner 0x1413C2A70. Rows `burst.*` of sheet sora-shotlocks (tools/shotlocks/burst.js).
//!
//! What KH3 does (instruction level: build/sora_tmp/weapons/SHOT-B/spec/burst.md; the delegate is ShootFlow vtable
//! 0x14693CA58 + 0x3A8, Begin 0x1413A5CD0 case 0xE / 0x10: names FZ_BURST / CA_BURST, groups 0x7D5 / 0):
//! - sub 0: Flying, stopped (pawn vt+0x17A0(5, 1)), gravity off (vt+0x1728(0)), the weapon presentation vt+0x1388(1, 0, ..)
//!   (not in the port, as states/shoot_flow.rs); Play `M_1466` (group +0xC0: its charge effects are the GroupID's rows);
//!   face request; sub 4, LiftCheck(500) -> sub 3.
//! - sub 3: LiftStep -> sub 4, then sub 4's code in the same tick.
//! - sub 4: at M_1466's end Flying, the volley set-up 0x1413C3330 (N, the first target, +0xA0 = 0.5, SetFace off), Play
//!   `M_1467`, flagsCC |= 1, sub 5.
//! - sub 5 / 6: at the clip's end `M_1468` / `M_1469` while fewer than N are out, else `ShootFlowEnd` (group 0) with
//!   PlayerSpecificAction(TRES_PSA_SHOOTFLOW_ACTION_END = 2, 0) (pawn vt+0x1038), sub 2; sub 7: ShootFlowEnd + the action.
//! - sub 2 (default): at the clip's end vt+0x1388(0, 0, 0), Finish; the tail still runs on that tick.
//! - the tail of every tick: while flagsCC bit 0, the spawner while +0x90 < N, else the bit off; SetFace(false, 0).
//! - the update never releases the aim mode (no 0x14140E500, unlike KC): End does.
//! - THE SPAWNER 0x1413C2A70 ([`spawner`]): +0xA0 counts down first (a shot on the tick it reaches 0); the target is the
//!   locked entry +0x110, LOCK BY LOCK (+0x114 against the entry's lock count; unlike KC's round robin); a gone target
//!   costs one call (+0x110 += 1, +0x114 = 0, no shot) and its remaining locks; the muzzle is weapon 0's ATK_01 (read for
//!   every shot), the direction normalize(muzzle - his lock-on point) in 3-D; one projectile of +0xB8 homing on the entry,
//!   SetMoveStartDelay(0.033333), StartFire; +0xA0 = 0.066666 in subs 5..7 only.
//! - The Shot handler sends these kinds to its default spawn branch, but no clip of the family has a TresShot (nor a TresAtk
//!   window): neither shared handler takes part.

use glam::Vec3;

use crate::anim::{AnimId, NotifyEvent, PlayArgs, SEQUENCES};
use crate::host::{EventKind, TargetFlags};
use crate::magic::projectile::{self, SpawnArgs};
use crate::magic::shotlock::lock_point;
use crate::magic::spells::cmd;
use crate::magic::tables::{Sheet, PV};
use crate::pawn::Ctx;
use crate::states::shoot_flow::ShootFlow;
use crate::states::StateId;

const S: Sheet = Sheet::Shotlocks;

/// The family runs (`magic/shotlocks::ready`).
pub(crate) const PORTED: bool = true;

/// The family's fields of the state object: +0x110 (the locked entry the spawner is on) and +0x114 (shots on it so far);
/// Reset 0x1413AD3E0 zeroes both.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct State {
    entry: usize,
    on_entry: i32,
}

/// After the common Begin and the per-kind set-up: nothing of its own (Begin stores the delegate, +0xB8 and +0xC0 only).
pub(crate) fn begin(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

fn clip(key: &str) -> AnimId {
    AnimId::from_name(S.text(&format!("burst.clip.{key}"))).unwrap_or(AnimId::ShootFlowEnd)
}

/// `Play("ShootFlowEnd", group 0)` and PlayerSpecificAction(SHOOTFLOW_ACTION_END, 0) (pawn vt+0x1038 0x141432AF0).
fn shoot_flow_end(sf: &mut ShootFlow, cx: &mut Ctx) {
    cx.pawn.anim.play_with(clip("end"), PlayArgs::mode(2, -1.0));
    cx.pawn.event(EventKind::PlayerSpecificAction { state: StateId::Shootflow, action: S.int("burst.end_action") as u8, sub: 0 });
    sf.sub = 2;
}

/// The update delegate 0x1413BADB0; `false` = the state finished this tick (no PostTick).
pub(crate) fn update(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) -> bool {
    let mut running = true;
    match sf.sub {
        0 => {
            let pawn = &mut *cx.pawn;
            pawn.cmc.stop_movement();
            pawn.cmc.set_flying();
            pawn.cmc.gravity_off = true;
            super::play(sf, cx, clip("charge"));
            sf.face_request = true;
            sf.sub = 4;
            if sf.lift_check(cx, S.num("burst.lift.height")) {
                sf.sub = 3;
            }
        }
        3 | 4 => {
            if sf.sub == 3 && sf.lift_step(cx, dt) {
                sf.sub = 4;
            }
            // (sub 3 falls into sub 4's code whether or not the lift finished.)
            if cx.pawn.anim.ended {
                cx.pawn.cmc.stop_movement();
                cx.pawn.cmc.set_flying();
                // The volley set-up 0x1413C3330: N, the first target, flagsCC |= 1, +0xA0 = 0.5, SetFace(false, 0).
                sf.set_up(cx);
                super::play(sf, cx, clip("fire1"));
                sf.ring_active = true;
                sf.sub = 5;
            }
        }
        5 | 6 => {
            if cx.pawn.anim.ended {
                if sf.spawn_index < sf.n {
                    let next = if sf.sub == 5 { "fire2" } else { "fire3" };
                    super::play(sf, cx, clip(next));
                    sf.sub += 1;
                } else {
                    shoot_flow_end(sf, cx);
                }
            }
        }
        7 => {
            if cx.pawn.anim.ended {
                shoot_flow_end(sf, cx);
            }
        }
        _ => {
            if cx.pawn.anim.ended {
                // (vt+0x1388(0, 0, 0): not in the port.) Kind 0x29: pawn vt+0x1E48(1.0) (SHOT-C: the pawn+0xE44 timer).
                if sf.kind == 0x29 {
                    crate::states::damage::set_invincible(cx, 1.0); // (SHOT-C)
                }
                cx.pawn.queue.do_chain();
                running = false;
            }
        }
    }
    // The tail of every tick (after Finish too): the spawner while flagsCC bit 0, then SetFace(false, 0).
    if sf.ring_active {
        if sf.spawn_index < sf.n {
            spawner(sf, st, cx, dt);
        } else {
            sf.ring_active = false;
        }
        sf.facing = false;
        sf.face_timer = 0.0;
    }
    running
}

/// The spawner 0x1413C2A70(dt): at most one projectile per call (see the module doc).
fn spawner(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) {
    if sf.ring_timer > 0.0 {
        sf.ring_timer -= dt;
        if sf.ring_timer > 0.0 {
            return;
        }
        sf.ring_timer = 0.0;
    }
    let locked = &cx.pawn.magic.aim.locked;
    let entry = locked.get(st.entry).copied();
    let alive = entry.is_some_and(|e| cx.target(e.target).is_some_and(|t| t.flags.has(TargetFlags::ALIVE)));
    let Some(e) = entry.filter(|_| alive) else {
        // skip: the entry's remaining locks are dropped, no shot this call.
        st.entry += 1;
        st.on_entry = 0;
        return;
    };
    // (pawn vt+0x1330, the weapon usable: Begin drew it.)
    let Some(def) = super::projectile_named(cx, sf.proj_name) else { return };
    // Weapon 0's ATK_01 (pawn vt+0x1410(0)), sampled for every shot.
    let muzzle = blade_tip(sf, cx).or_else(|| crate::states::fire_magic::muzzle(cx.pawn)).unwrap_or_else(|| lock_point(cx));
    let d = muzzle - lock_point(cx);
    let min = S.text("burst.spawn.min_len_sq").parse::<f32>().unwrap_or(0.0);
    let direction = if d.length_squared() > min { d.normalize() } else { d };
    let id = projectile::spawn(cx, &SpawnArgs { def, position: muzzle, direction, cmd: cmd::SHOOTFLOW, shoot_flow: sf.kind, finish: false, situation: false, full_mp: false, combo: [0; 4], target: e.target, attached: None, fire: true });
    if let Some(p) = cx.pawn.magic.projectiles.iter_mut().find(|p| p.id == id) {
        p.set_move_start_delay(S.num("burst.spawn.move_start_delay"));
    }
    st.on_entry += 1;
    sf.spawn_index += 1;
    if st.on_entry >= e.lock_count {
        st.on_entry = 0;
        st.entry += 1;
    }
    if rearms(sf.sub) {
        sf.ring_timer = S.num("burst.spawn.interval");
    }
}

/// The held Keyblade's ATK_01 now (`burst.k<kind>.muzzle.<sequence>`: FK per frame of the playing clip, the Keyblade on
/// R_wep_skt; tools/shotlocks/burst.js), turned into the world; `None` for a clip or kind without a track.
fn blade_tip(sf: &ShootFlow, cx: &Ctx) -> Option<Vec3> {
    let pawn = &*cx.pawn;
    let &(_, index) = pawn.anim.name.data().samples.first()?;
    let sequence = SEQUENCES.get(index)?;
    let track = match S.find(&format!("burst.k{}.muzzle.{}", sf.kind, sequence.name.to_ascii_lowercase())) {
        Some(PV::A(track)) if track.len() >= 3 => track,
        _ => return None,
    };
    let last = track.len() / 3 - 1;
    let f = if sequence.length > 0.0 { (pawn.anim.time / sequence.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
    let (i, w) = ((f.floor() as usize).min(last), f - f.floor());
    let j = (i + 1).min(last);
    let at = |k: usize| track[i * 3 + k] + (track[j * 3 + k] - track[i * 3 + k]) * w;
    let (sin, cos) = pawn.yaw.to_radians().sin_cos();
    Some(pawn.cmc.location + Vec3::new(at(0) * cos - at(1) * sin, at(0) * sin + at(1) * cos, at(2)))
}

/// The subs whose shots re-arm the timer (`burst.spawn.rearm_subs`: 5, 6, 7).
fn rearms(sub: u8) -> bool {
    S.list("burst.spawn.rearm_subs").iter().any(|&s| s as u8 == sub)
}

/// Every notify event of a pose tick: nothing of its own.
pub(crate) fn on_notify(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx, _event: &NotifyEvent) {}

/// Before the common End's release of the aim mode: nothing of its own.
pub(crate) fn end(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

/// The Shot handler's spawn point: the blade tip (no TresShot in the family's clips).
pub(crate) fn muzzle(_sf: &ShootFlow, _st: &State, _cx: &Ctx, _index: usize) -> Option<Vec3> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_kh3_s() {
        for k in [14, 16] {
            assert_eq!(super::super::family(k), "burst", "kind {k}");
            assert!(super::super::ready(k));
        }
        assert_eq!((super::super::base_kind(7), super::super::base_kind(10)), (14, 16));
        assert_eq!((S.text("burst.k14.proj_class"), S.text("burst.k16.proj_class")), ("TresProjFZ_Burst_C", "TresProjCA_Burst_C"));
        assert_eq!((S.int("burst.k14.effect_group"), S.int("burst.k16.effect_group")), (0x7D5, 0));
        assert_eq!((S.num("burst.lift.height"), S.num("burst.spawn.first_delay"), S.int("burst.end_action")), (500.0, 0.5, 2));
        assert!((S.num("burst.spawn.interval") - 0.066666).abs() < 1.0e-6 && (S.num("burst.spawn.move_start_delay") - 0.033333).abs() < 1.0e-6);
        assert!(rearms(5) && rearms(7) && !rearms(2));
        for k in [14, 16, 27, 41] {
            for seq in ["1467", "1468", "1469", "1453"] {
                assert!(S.list(&format!("burst.k{k}.muzzle.{seq}")).len().is_multiple_of(3), "kind {k} {seq}");
            }
        }
        for key in ["charge", "fire1", "fire2", "fire3", "end"] {
            assert!(AnimId::from_name(S.text(&format!("burst.clip.{key}"))).is_some(), "{key}");
        }
    }
}
