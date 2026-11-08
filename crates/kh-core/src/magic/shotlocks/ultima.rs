//! (SHOT-C) Infinity Circle (kind 29 `UW_ULTIMA`, Ultima Weapon = weapon type 15; no MAX kind): update 0x1413BB3C0.
//! Rows `ultima.*` of sheet sora-shotlocks (tools/shotlocks/ultima.js).
//!
//! What KH3 does (decompiled C; the delegate is ShootFlow vtable 0x14693CA58 + 0x500, Begin 0x1413A5CD0 case 0x1D: name
//! UW_ULTIMA, group 0, voice 0):
//! - sub 0: gravity off, Flying (stopped); the WEAPON CHANGE pawn vt+0x13C8(weapon type, 9, ...) (ChangeWeapon
//!   0x14144A9F0) to weapon form 9 = Ultimate Form's weapon w_so192 with its anim set AnimSet9 (no form is started: no
//!   form command, no wear form); Play `M_9481` (blend -1, mode 2, group 0), voice 0; m_bFaceAt; sub 4. No lift.
//! - sub 3..5: at M_9481's end ShootFlowEnd, m_bEnableChange = 0, sub 2.
//! - sub 2: once m_bEnableChange is back (ShootFlowEnd's TresEnableChange) and +0xB0 != 0: the weapon form of Begin back
//!   (vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1)), WorldSlow off (FUN_14140E500(ctrl, 0): ctrl+0x878 bit 3, object types
//!   0x419C; nothing when it is not on); at the clip's end Finish. End (0x1413ABA70) gives the weapon back when that has
//!   not happened, and its slot 809 (vt+0x1948) is the aim mode's release. (SHOT-FIX) 0x14140E500 is WorldSlow, not the
//!   release: re/decomp/14140/14140E500.c.
//! - The attack is M_9481's own window (3.7 .. 5.0333 s): group `SF_Range` of w_so192's TresAtkColl0 (a SPHERE of 3000 cm
//!   on Sora's `kh_root`, OWNER_ATTACHED_MESH), row SF_ULTIMA (power 5, SMALL, magic), AttackInterval 0.2667 s: every
//!   target within 30 m is hit once per interval. It runs through the generic hit path (combat/hit.rs `on_notifies` /
//!   `tick`; this module answers the shape: [`hit_shape`]), so it works on every target the host's lock-on accepts.
//! - The Shot handler only flags kind 0x1D (no TresShot in M_9481); the clip's effects, sounds and voice are its notifies'.
//!
//! The weapon form is kept on the pawn (`PawnMagic::shot_weapon_form`): the hosts draw w_so192 instead of the Keyblade
//! while it is 9 (kh-er-render keyblade.rs `draw_form_weapon_parts`, form "09"; SHOT-C ultima.md INT line).

use glam::Vec3;

use crate::anim::{AnimId, NotifyClass, NotifyData, NotifyEvent, NotifyPhase};
use crate::host::EventKind;
use crate::magic::tables::Sheet;
use crate::pawn::Ctx;
use crate::states::shoot_flow::ShootFlow;

const S: Sheet = Sheet::Shotlocks;

/// The family runs (`magic/shotlocks::ready`).
pub(crate) const PORTED: bool = true;

/// The family's fields of the state object: +0xB0 / +0xB1 (the weapon form and its sub value at Begin, pawn vt+0x12E0 /
/// vt+0x12E8; the port keeps the override on the pawn: `restore` = +0xB0 != 0).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct State {
    restore: bool,
}

/// The weapon form the shotlock puts on (`ultima.weapon_form`: 9, Ultimate Form's weapon).
pub fn weapon_form() -> u8 {
    S.int("ultima.weapon_form") as u8
}

/// The weapon parts of that form (`unit@part:socket=blueprint`, form.SW's units): what the hosts draw instead of the
/// Keyblade while `PawnMagic::shot_weapon_form` is [`weapon_form`].
pub fn weapon_units() -> &'static [&'static str] {
    match S.find("ultima.weapon_units") {
        Some(crate::magic::tables::PV::L(units)) => units,
        _ => &[],
    }
}

/// After the common Begin and the per-kind set-up: nothing of its own (Begin 0x1413A5CD0 case 0x1D stores the delegate,
/// the name and group 0 only).
pub(crate) fn begin(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

/// The weapon form of Begin back (sub 2 / End: vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1)).
fn give_weapon_back(st: &mut State, cx: &mut Ctx) {
    if std::mem::take(&mut st.restore) {
        cx.pawn.magic.shot_weapon_form = 0;
    }
}

/// The update delegate 0x1413BB3C0; `false` = the state finished this tick (no PostTick).
pub(crate) fn update(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, _dt: f32) -> bool {
    match sf.sub {
        0 => {
            let pawn = &mut *cx.pawn;
            pawn.cmc.gravity_off = true;
            pawn.cmc.stop_movement();
            pawn.cmc.set_flying();
            // vt+0x13C8(weapon type, 9, ...): weapon form 9 (the form's anim set AnimSet9 holds M_9481).
            pawn.magic.shot_weapon_form = weapon_form();
            st.restore = true;
            // `ultima.clip` M_9481 of AnimSet9 = the animation table's row A9481 (sequence 9481; F-ULTIMATE's FORM_ANIM_SETS_LATE).
            super::play(sf, cx, AnimId::A9481);
            sf.face_request = true;
            sf.sub = 4;
        }
        3..=5 => {
            if cx.pawn.anim.ended {
                super::play(sf, cx, AnimId::ShootFlowEnd);
                cx.pawn.queue.flags.enable_change = false;
                sf.sub = 2;
            }
        }
        _ => {
            if cx.pawn.queue.flags.enable_change && st.restore {
                give_weapon_back(st, cx);
                // (SHOT-FIX) FUN_14140E500(ctrl, 0) = WorldSlow off (1413BB3C0.c:64), not the release: the aim mode is
                // released by End's slot 809 (states/shoot_flow.rs `end`).
                crate::combat::form::world_slow(cx, false);
            }
            if cx.pawn.anim.ended {
                cx.pawn.queue.do_chain();
                return false;
            }
        }
    }
    true
}

/// Every notify event of a pose tick: M_9481's `TresAnimNotifyState_BpEvent` (0 .. 4.3 s: the state-event camera
/// blueprint BP_TresPlayerStateEventCam_ShootFlow's start / end events) goes to the hosts as `AnimBpEvent`, as the FR
/// states send theirs. The hit window is the generic path's.
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

/// Before the common End's release of the aim mode: a state dropped before sub 2 gives the weapon back (End 0x1413ABA70:
/// +0xB0 != 0).
pub(crate) fn end(_sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx) {
    give_weapon_back(st, cx);
}

/// The Shot handler's spawn point (none: kind 0x1D spawns nothing).
pub(crate) fn muzzle(_sf: &ShootFlow, _st: &State, _cx: &Ctx, _index: usize) -> Option<Vec3> {
    None
}

/// The shape of a hit window of weapon form 9's own groups (combat/hit.rs `shape`): `SF_Range`, a sphere of
/// `ultima.range.radius` on Sora's kh_root (his root = the actor location: M_9481 has root motion, kh_sk keeps its
/// reference pose 0) plus `ultima.range.offset` turned by his yaw. `None` for any other group or without the form.
pub(crate) fn hit_shape(cx: &Ctx, group: &str) -> Option<(Vec3, Vec3, f32)> {
    if cx.pawn.magic.shot_weapon_form != weapon_form() || group != S.text("ultima.window.group") {
        return None;
    }
    let o = S.vec3("ultima.range.offset");
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    let c = cx.pawn.cmc.location + Vec3::new(o.x * cos - o.y * sin, o.x * sin + o.y * cos, o.z);
    Some((c, c, S.num("ultima.range.radius")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_kh3_s() {
        assert_eq!((S.int("ultima.kind"), weapon_form()), (29, 9));
        assert_eq!(super::super::family(29), "ultima");
        assert!(super::super::ready(29));
        assert_eq!(super::super::base_kind(15), 29);
        assert_eq!(super::super::max_kind(15, false), 29);
        assert_eq!((S.text("ultima.window.group"), S.text("ultima.window.row"), S.num("ultima.range.radius")), ("SF_Range", "SF_ULTIMA", 3000.0));
        assert!(weapon_units().iter().any(|u| u.ends_with("w_so192_Wep")));
        assert_eq!(S.text("ultima.clip"), "M_9481");
        assert_eq!(AnimId::A9481.data().samples.len(), 1);
    }
}
