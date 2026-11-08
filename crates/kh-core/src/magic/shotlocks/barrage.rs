//! (SHOT-A) Diving Barrage (kind 6 `DW_BARRAGE`, Shooting Star's alternative MAX kind; SHOT-B: 21 `PO_BARRAGE` Hunny
//! Drizzle, Hunny Spout's): update 0x1413B6320. Rows `barrage.*` of sheet sora-shotlocks (tools/shotlocks/barrage.js).
//!
//! What KH3 does (decompiled C, constants re-read in the disassembly; the delegate is ShootFlow vtable 0x14693CA58 +
//! 0x3D8, Begin 0x1413A5CD0 case 6: name DW_BARRAGE, group 0, voice 0; case 0x15: PO_BARRAGE, group 0x71E):
//! - sub 0: gravity off, Flying (stopped); the WEAPON CHANGE pawn vt+0x13C8(weapon type, 2, ...) (kind 0x15: 0x10) to
//!   Double Arrowguns' (Hunny Blasters') two guns and anim set AnimSet2 (no form is started); Play `M_2481` (blend -1,
//!   mode 2, group +0xC0); face request; sub 4. +0xE4 = (his location x, y, his centre z); +0xD8 = the first target's
//!   point (its component vt+0xB88; none: his centre) raised by `barrage.up` (1000) + his half height (vt+0x548);
//!   +0xF0 = max(0, horizontal distance - 50) (stored; no reader found).
//! - sub 4 (the rise, M_2481 1.2 s): every tick the velocity (vt+0x16B0) toward that point: xy = 5 x the offset (within
//!   50 cm: half his velocity), z = 5 x the offset when above him or more than 100 cm below, else 0. At the clip's end:
//!   the body collision on (vt+0x1010(1, 0)), StopMovement, Play `M_2482`, sub 5, the point 800 cm lower.
//! - sub 5 (the dive, M_2482 2.1667 s): his velocity, z = 0.5 x (point z - centre z) while he is above the point, else
//!   0; M_2482's 16 TresShot notifies fire the shared Shot handler (mod.rs `spawn_shot`: kind 6 / 0x15 a PAIR, weapon
//!   [LocationIndex] then [index ^ 1], each along muzzle - centre, homing on locked[spawn mod locked], while spawn < N:
//!   18 locks = 9 pairs). At the clip's end: StopMovement, Play `M_2483`, m_bEnableChange = 0, sub 2.
//! - sub 2 (M_2483 0.4333 s): once m_bEnableChange is back (M_2483's TresEnableChange 0.2995 s) and +0xB0 != 0: the
//!   weapon form of Begin back (vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1)); at the clip's end Finish. No release of the
//!   controller's pending flag here (the common End's). End (0x1413ABA70) gives the weapon back when that has not happened.
//! - The projectile: w_so020 DW_BARRAGE = TresProjSS_Barrage_C (row SF_BARRAGE; w_so040 PO_BARRAGE = TresProjPO_Barrage_C,
//!   SF_POBARRAGE), a plain TresProjectileBase (magic/projectile.rs: homing, the generic hits on every target).
//!
//! The muzzles (pawn vt+0x1410(i): weapon[i]'s ATK_01) are the FK tracks of the two guns over M_2481..3 (rows
//! `barrage.k<kind>.muzzle.<sequence>`). The weapon form is kept on the pawn (`PawnMagic::shot_weapon_form`): the hosts
//! draw that form's weapon parts (`barrage.k<kind>.weapon_units`) instead of the Keyblade meanwhile.

use glam::{Vec2, Vec3};

use crate::anim::{AnimId, NotifyClass, NotifyData, NotifyEvent, NotifyPhase};
use crate::host::EventKind;
use crate::magic::tables::Sheet;
use crate::pawn::Ctx;
use crate::states::shoot_flow::ShootFlow;

const S: Sheet = Sheet::Shotlocks;

/// The family runs (`magic/shotlocks::ready`).
pub(crate) const PORTED: bool = true;

/// The family's fields of the state object: +0xD8 (the point he flies to), +0xF0 (the horizontal reach, stored only),
/// +0xB0 != 0 (Begin's weapon form still to come back; the port keeps the override on the pawn).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct State {
    point: Vec3,
    reach: f32,
    restore: bool,
}

/// The kind whose rows hold this kind's values: the code branches on 0x15 (Hunny Drizzle) only, every other kind of the
/// update is Diving Barrage's (6).
fn row_kind(kind: u8) -> u8 {
    if kind == 0x15 {
        0x15
    } else {
        6
    }
}

/// The weapon form the update puts on (vt+0x13C8's second argument): 2 Double Arrowguns, kind 0x15: 0x10 Hunny Blasters.
pub(crate) fn weapon_form(kind: u8) -> u8 {
    S.int(&format!("barrage.k{}.weapon_form", row_kind(kind))) as u8
}

/// The clip of a sub-step (`rise` M_2481, `dive` M_2482, `land` M_2483 of AnimSet2).
fn clip(key: &str) -> AnimId {
    let fallback = match key {
        "rise" => AnimId::A2481,
        "dive" => AnimId::A2482,
        _ => AnimId::A2483,
    };
    AnimId::from_name(S.text(&format!("barrage.clip.{key}.sequence"))).unwrap_or(fallback)
}

/// After the common Begin and the per-kind set-up: nothing of its own (Begin cases 6 / 0x15: the delegate, the name,
/// the group).
pub(crate) fn begin(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

/// Begin's weapon form back (sub 2 / End: vt+0x13C8(type, +0xB0, +0xB1), +0xB0 = 0).
fn give_weapon_back(st: &mut State, cx: &mut Ctx) {
    if std::mem::take(&mut st.restore) {
        cx.pawn.magic.shot_weapon_form = 0;
    }
}

/// Sub 0: the weapon change, M_2481, the origin and the point above the first target.
fn start(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx) {
    let pawn = &mut *cx.pawn;
    pawn.cmc.gravity_off = true;
    pawn.cmc.stop_movement();
    pawn.cmc.set_flying();
    pawn.magic.shot_weapon_form = weapon_form(sf.kind);
    st.restore = true;
    super::play(sf, cx, clip("rise"));
    sf.face_request = true;
    sf.sub = 4;
    // +0xE4: the root's location with his centre's height (vt+0x6E0).
    let centre = super::super::shotlock::lock_point(cx);
    sf.origin = Vec3::new(cx.pawn.cmc.location.x, cx.pawn.cmc.location.y, centre.z);
    // +0xD8: the first component's point (vt+0xB88), else his centre; + 1000 + his half height (vt+0x548).
    let at = cx.target(sf.first_target).map_or(centre, |t| t.center);
    st.point = at + Vec3::Z * (S.num("barrage.up") + cx.pawn.params().capsule_half_height);
    st.reach = ((st.point - sf.origin).truncate().length() - S.num("barrage.reach_margin")).max(0.0);
}

/// Sub 4's velocity toward the point (vt+0x16B0).
fn rise(st: &State, cx: &mut Ctx) {
    let d = st.point - super::super::shotlock::lock_point(cx);
    let gain = S.num("barrage.rise.gain");
    let xy = if d.truncate().length_squared() <= S.num("barrage.rise.near_sq") { cx.pawn.cmc.velocity.truncate() * S.num("barrage.rise.near_scale") } else { d.truncate() * gain };
    let z = if d.z > 0.0 || d.z < S.num("barrage.rise.z_low") { d.z * gain } else { 0.0 };
    set_velocity(cx, xy, z);
}

/// Sub 5's velocity: his own, the z toward the lowered point while above it.
fn dive(st: &State, cx: &mut Ctx) {
    let dz = st.point.z - super::super::shotlock::lock_point(cx).z;
    let z = if dz < 0.0 { dz * S.num("barrage.dive.gain") } else { 0.0 };
    let xy = cx.pawn.cmc.velocity.truncate();
    set_velocity(cx, xy, z);
}

/// pawn vt+0x16B0: the velocity, all axes.
fn set_velocity(cx: &mut Ctx, xy: Vec2, z: f32) {
    cx.pawn.cmc.velocity = xy.extend(z);
}

/// The update delegate 0x1413B6320; `false` = the state finished this tick (no PostTick).
pub(crate) fn update(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, _dt: f32) -> bool {
    match sf.sub {
        0 => start(sf, st, cx),
        4 => {
            if cx.pawn.anim.ended {
                // vt+0x1010(1, 0): the body collision on; StopMovement; M_2482; the point 800 cm lower.
                cx.pawn.actions.body_collision_off = false;
                cx.pawn.cmc.stop_movement();
                super::play(sf, cx, clip("dive"));
                sf.sub = 5;
                st.point.z -= S.num("barrage.dive.drop");
            } else {
                rise(st, cx);
            }
        }
        5 => {
            if cx.pawn.anim.ended {
                cx.pawn.cmc.stop_movement();
                super::play(sf, cx, clip("land"));
                cx.pawn.queue.flags.enable_change = false;
                sf.sub = 2;
            } else {
                dive(st, cx);
            }
        }
        _ => {
            if cx.pawn.queue.flags.enable_change && st.restore {
                give_weapon_back(st, cx);
            }
            if cx.pawn.anim.ended {
                cx.pawn.queue.do_chain();
                return false;
            }
        }
    }
    true
}

/// Every notify event of a pose tick: the clips' `TresAnimNotifyState_BpEvent` (the state-event camera blueprint's start
/// / end events, each clip from 0 to its end) goes to the hosts as `AnimBpEvent`, as Infinity Circle's. The shots are
/// the shared Shot handler's.
pub(crate) fn on_notify(_sf: &mut ShootFlow, _st: &mut State, cx: &mut Ctx, event: &NotifyEvent) {
    if event.class() != NotifyClass::TresAnimNotifyStateBpEvent {
        return;
    }
    let param = match event.def().data {
        NotifyData::Param { param } => param.unwrap_or(0),
        _ => 0,
    };
    let begin = match event.phase {
        NotifyPhase::Begin => true,
        NotifyPhase::End => false,
        _ => return,
    };
    cx.pawn.event(EventKind::AnimBpEvent { seq: event.sequence_name(), param, begin });
}

/// Before the common End's release of the aim mode: a state dropped before sub 2's restore gives the weapon back (End
/// 0x1413ABA70: +0xB0 != 0).
pub(crate) fn end(_sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx) {
    give_weapon_back(st, cx);
}

/// The Shot handler's spawn point for `LocationIndex` `index` (pawn vt+0x1410(i) 0x1405AA790: weapon[i]'s ATK_01, index
/// 0 when out of range): the FK track of the playing clip (M_2481..3) at the nearest frame, turned by his yaw onto his
/// location. `None` without the weapon form or outside those clips (the framework's fallback).
pub(crate) fn muzzle(sf: &ShootFlow, _st: &State, cx: &Ctx, index: usize) -> Option<Vec3> {
    let kind = row_kind(sf.kind);
    if cx.pawn.magic.shot_weapon_form != weapon_form(kind) {
        return None;
    }
    let key = ["rise", "dive", "land"].into_iter().find(|k| clip(k) == cx.pawn.anim.name)?;
    let track = S.list(&format!("barrage.k{kind}.muzzle.{}", S.text(&format!("barrage.clip.{key}.sequence"))));
    let frames = track.len() / 6;
    if frames == 0 {
        return None;
    }
    let length = S.num(&format!("barrage.clip.{key}.length"));
    let f = if length > 0.0 { (cx.pawn.anim.time / length).clamp(0.0, 1.0) * (frames - 1) as f32 } else { 0.0 };
    let at = (f.round() as usize).min(frames - 1) * 6 + if index < 2 { index * 3 } else { 0 };
    let m = track.get(at..at + 3)?;
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    Some(cx.pawn.cmc.location + Vec3::new(m[0] * cos - m[1] * sin, m[0] * sin + m[1] * cos, m[2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_kh3_s() {
        assert_eq!(S.text("barrage.update"), "0x1413B6320");
        assert_eq!(super::super::family(6), "barrage");
        assert!(super::super::ready(6));
        // Shooting Star: base 5, MAX 7, the alternative MAX 6.
        assert_eq!((super::super::base_kind(3), super::super::max_kind(3, false), super::super::max_kind(3, true)), (5, 7, 6));
        assert_eq!((weapon_form(6), weapon_form(0x15)), (2, 16));
        assert_eq!((clip("rise"), clip("dive"), clip("land")), (AnimId::A2481, AnimId::A2482, AnimId::A2483));
        assert_eq!((S.num("barrage.up"), S.num("barrage.dive.drop"), S.num("barrage.rise.gain"), S.num("barrage.dive.gain")), (1000.0, 800.0, 5.0, 0.5));
        assert_eq!((S.text("barrage.k6.proj_class"), S.text("barrage.k6.row")), ("TresProjSS_Barrage_C", "SF_BARRAGE"));
        assert_eq!(S.list("barrage.dive.shot_times").len(), 16);
        for kind in [6, 21] {
            for key in ["rise", "dive", "land"] {
                let seq = S.text(&format!("barrage.clip.{key}.sequence"));
                assert!(S.list(&format!("barrage.k{kind}.muzzle.{seq}")).len() >= 12, "{kind} {seq}");
            }
        }
    }
}
