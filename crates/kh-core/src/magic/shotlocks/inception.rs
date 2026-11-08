//! (SHOT-C) Stellar Inception (kind 42 = 0x2A `SF_LIGHTINCEPTION`, the MAX shotlock of Oathkeeper = weapon type 13 and of
//! Oblivion = 14): update 0x1413B7F10. Rows `inception.*` of sheet sora-shotlocks (tools/shotlocks/inception.js).
//!
//! What KH3 does (decompiled C, constants re-read in the disassembly; the delegate is ShootFlow vtable 0x14693CA58 +
//! 0x508, Begin 0x1413A5CD0 case 0x2A: name SF_LIGHTINCEPTION, group 0, voice 0):
//! - sub 0: +0xE4 = his location; gravity off, Flying (stopped); the WEAPON CHANGE pawn vt+0x13C8(weapon type, 0x18, ...)
//!   (ChangeWeapon 0x14144A9F0) to weapon form 0x18 = TRES_WEAPON_FORM_18, Double Form's two Keyblades (Oathkeeper w_so200
//!   in the right hand, Oblivion w_so210 in the left; AnimSet23 holds M_N481): no form is started (no form command, no wear
//!   form); Play `M_N481` (blend -1, mode 2, group 0), voice 0; face request; sub 4. No lift.
//! - sub 3 / 4: flagsCC bit 2 (OnTiming 0x1413B57D0 for 0x2A, M_N481's TresAnimNotify_Timing at 2.333 s) -> sub 5.
//!   Sub 3..5: at the clip's end ShootFlowEnd, m_bEnableChange = 0, sub 2 and WorldSlow off (FUN_14140E500(ctrl, 0)).
//!   (SHOT-FIX) 0x14140E500 is WorldSlow (ctrl+0x878 bit 3), not the release: the aim mode is released by End's slot 809,
//!   so the locked list stays for the spawner's last bolts (re/decomp/14140/14140E500.c).
//! - sub 1 / 2: once m_bEnableChange is back (ShootFlowEnd's TresEnableChange) and +0xB0 != 0: Begin's weapon form back
//!   (vt+0x13C8(type, +0xB0, +0xB1)); at the clip's end pawn vt+0x1E48(1.0) (the pawn+0xE44 invincibility timer: NOT
//!   ported, the port has none) and Finish. End (0x1413ABA70) gives the weapon back when that has not happened.
//! - every tick while flagsCC bit 2: the spawner 0x1413C0850 while the spawn counter +0x90 < N (total locks), else the bit
//!   off; then SetFace off (vt+0x390(0, 0)).
//! - THE BOLTS (spawner 0x1413C0850, [`spawner`]): the first waits `inception.delay` (0.3 s, +0xA0 set by the TresAtk begin
//!   handler 0x1413B5430 at the window's begin, counted down only while the spawner runs), then ONE bolt a tick: for the
//!   locked entry +0x110 (dead: skipped, one tick lost) bolt +0x90 heads at his yaw + 360 / N x +0x90, pitched 30 up, and
//!   starts 300 cm to the LEFT of that heading from his lock-on point; it is fired at once and homes on that entry; each
//!   entry gets as many bolts as its lock count (+0x114 against the entry's +0xC). The class is TresProjTF_LightInception
//!   (both Keyblades' sets: magic/projectile.rs, row SF_LIGHTINCEPTION_BOMB).
//! - THE FINISH: M_N481's own window (2.0 .. 4.267 s): group `R_Finish` of w_so200's TresAtkColl0 (a SPHERE on Sora's
//!   `kh_root` growing 600 -> 3000 cm at 3000 cm/s of window time), row SF_LIGHTINCEPTION (power 0.4, TOSS, magic), no
//!   interval: every target in it is hit once. Through the generic hit path (combat/hit.rs; this module answers the shape:
//!   [`hit_shape`]), so it works on every target the host's lock-on accepts and launches as KH3's TOSS rows do.
//!
//! The weapon form is kept on the pawn (`PawnMagic::shot_weapon_form`): the hosts draw [`weapon_units`] instead of the
//! Keyblade while it is 0x18 (SHOT-C inception.md INT line).

use glam::Vec3;

use crate::anim::{AnimId, NotifyClass, NotifyData, NotifyEvent, NotifyPhase};
use crate::host::{EventKind, TargetFlags, TargetId};
use crate::magic::projectile::{self, SpawnArgs};
use crate::magic::spells::cmd;
use crate::magic::tables::Sheet;
use crate::pawn::Ctx;
use crate::states::shoot_flow::ShootFlow;

const S: Sheet = Sheet::Shotlocks;

/// The family runs (`magic/shotlocks::ready`).
pub(crate) const PORTED: bool = true;

/// The family's fields of the state object: +0x110 (the locked entry the spawner is on), +0x114 (bolts given to it), and
/// +0xB0 != 0 (Begin's weapon form still to come back; the port keeps the override on the pawn).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct State {
    entry: usize,
    given: i32,
    restore: bool,
}

/// The weapon form the shotlock puts on (`inception.weapon_form`: 0x18, Double Form's).
pub fn weapon_form() -> u8 {
    S.int("inception.weapon_form") as u8
}

/// The weapon parts of that form (`unit@part:socket=blueprint`, form.TF's units): what the hosts draw instead of the
/// Keyblade while `PawnMagic::shot_weapon_form` is [`weapon_form`].
pub fn weapon_units() -> &'static [&'static str] {
    match S.find("inception.weapon_units") {
        Some(crate::magic::tables::PV::L(units)) => units,
        _ => &[],
    }
}

/// After the common Begin and the per-kind set-up: nothing of its own (Begin case 0x2A: the delegate and the name only).
pub(crate) fn begin(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

/// Begin's weapon form back (sub 2 / End: vt+0x13C8(type, +0xB0, +0xB1), +0xB0 = 0).
fn give_weapon_back(st: &mut State, cx: &mut Ctx) {
    if std::mem::take(&mut st.restore) {
        cx.pawn.magic.shot_weapon_form = 0;
    }
}

/// The update delegate 0x1413B7F10; `false` = the state finished this tick (no PostTick).
pub(crate) fn update(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) -> bool {
    let mut running = true;
    match sf.sub {
        0 => {
            sf.origin = cx.pawn.cmc.location;
            let pawn = &mut *cx.pawn;
            pawn.cmc.gravity_off = true;
            pawn.cmc.stop_movement();
            pawn.cmc.set_flying();
            // vt+0x13C8(weapon type, 0x18, ...): Double Form's weapons (its anim set AnimSet23 holds M_N481).
            pawn.magic.shot_weapon_form = weapon_form();
            st.restore = true;
            // M_N481 of AnimSet23 = the animation-table row `inception.clip_sequence` (N481).
            let clip = AnimId::from_name(S.text("inception.clip_sequence")).unwrap_or(AnimId::N481);
            super::play(sf, cx, clip);
            sf.face_request = true;
            sf.sub = 4;
        }
        3..=5 => {
            if sf.sub < 5 && sf.timing2 {
                sf.sub = 5;
            }
            if cx.pawn.anim.ended {
                super::play(sf, cx, AnimId::ShootFlowEnd);
                cx.pawn.queue.flags.enable_change = false;
                sf.sub = 2;
                // (SHOT-FIX) FUN_14140E500(ctrl, 0) = WorldSlow off (1413B7F10.c:93), not the release (End's slot 809).
                crate::combat::form::world_slow(cx, false);
            }
        }
        _ => {
            if cx.pawn.queue.flags.enable_change && st.restore {
                give_weapon_back(st, cx);
            }
            if cx.pawn.anim.ended {
                // pawn vt+0x1E48(1.0): the 1 s invincibility timer pawn+0xE44.
                crate::states::damage::set_invincible(cx, 1.0);
                cx.pawn.queue.do_chain();
                running = false;
            }
        }
    }
    // The tail of every tick (also after Finish): the bolts while flagsCC bit 2, then SetFace off.
    if sf.timing2 {
        if sf.spawn_index < sf.n {
            spawner(sf, st, cx, dt);
        } else {
            sf.timing2 = false;
        }
        sf.facing = false;
        sf.face_timer = 0.0;
    }
    running
}

/// The spawner 0x1413C0850(dt): at most one bolt per call (see the module doc).
fn spawner(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) {
    if sf.ring_timer > 0.0 {
        sf.ring_timer -= dt;
        if sf.ring_timer > 0.0 {
            return;
        }
        sf.ring_timer = 0.0;
    }
    let locked: Vec<(TargetId, i32)> = cx.pawn.magic.aim.locked.iter().map(|e| (e.target, e.lock_count)).collect();
    let Some(&(target, count)) = locked.get(st.entry) else {
        // +0x110 past the locked targets: no bolt, the next entry.
        st.entry += 1;
        st.given = 0;
        return;
    };
    if !cx.target(target).is_some_and(|t| t.flags.has(TargetFlags::ALIVE)) {
        st.entry += 1;
        st.given = 0;
        return;
    }
    let n = sf.n.max(1) as f32;
    let yaw = cx.pawn.yaw + S.num("inception.spawn.yaw_turn") / n * sf.spawn_index as f32;
    let (pitch, side_yaw) = (S.num("inception.spawn.pitch").to_radians(), (yaw + S.num("inception.spawn.side_yaw")).to_radians());
    let (sy, cy) = yaw.to_radians().sin_cos();
    let direction = Vec3::new(pitch.cos() * cy, pitch.cos() * sy, pitch.sin());
    let position = crate::magic::shotlock::lock_point(cx) + Vec3::new(side_yaw.cos(), side_yaw.sin(), 0.0) * S.num("inception.spawn.side");
    if let Some(def) = super::projectile_named(cx, sf.proj_name) {
        let id = projectile::spawn(cx, &SpawnArgs { def, position, direction, cmd: cmd::SHOOTFLOW, shoot_flow: sf.kind, finish: false, situation: false, full_mp: false, combo: [0; 4], target, attached: None, fire: true });
        cx.pawn.magic.volley.push(id);
    }
    st.given += 1;
    sf.spawn_index += 1;
    if st.given >= count {
        st.entry += 1;
        st.given = 0;
    }
}

/// Every notify event of a pose tick: M_N481's `TresAnimNotifyState_BpEvent` (the state-event camera blueprint's start /
/// end events) goes to the hosts as `AnimBpEvent`, as Infinity Circle's does. The Timing / TresAtk handlers are the
/// framework's (magic/shotlocks `on_notifies`); the hit window is the generic path's.
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

/// The Shot handler's spawn point (none: M_N481 has no TresShot).
pub(crate) fn muzzle(_sf: &ShootFlow, _st: &State, _cx: &Ctx, _index: usize) -> Option<Vec3> {
    None
}

/// The shape of a hit window of weapon form 0x18's `R_Finish` (combat/hit.rs `shape`): a sphere on Sora's kh_root (his
/// root = the actor location) plus `inception.finish.offset` turned by his yaw, its radius `inception.finish.radius`
/// growing `inception.finish.inc` a second of window time up to `inception.finish.inc_max` (0x140649E40). `None` for any
/// other group or without the form.
pub(crate) fn hit_shape(cx: &Ctx, group: &str, window_time: f32) -> Option<(Vec3, Vec3, f32)> {
    if cx.pawn.magic.shot_weapon_form != weapon_form() || group != S.text("inception.window.group") {
        return None;
    }
    let o = S.vec3("inception.finish.offset");
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    let c = cx.pawn.cmc.location + Vec3::new(o.x * cos - o.y * sin, o.x * sin + o.y * cos, o.z);
    let r0 = S.num("inception.finish.radius");
    let radius = (r0 + S.num("inception.finish.inc") * window_time).min(S.num("inception.finish.inc_max").max(r0));
    Some((c, c, radius))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_kh3_s() {
        assert_eq!((S.int("inception.kind"), weapon_form()), (42, 0x18));
        assert_eq!(super::super::family(42), "inception");
        assert!(super::super::ready(42));
        for weapon in [13, 14] {
            assert_eq!(super::super::max_kind(weapon, false), 42, "weapon type {weapon}");
        }
        assert_eq!((super::super::base_kind(13), super::super::base_kind(14)), (40, 41));
        assert_eq!((S.text("inception.window.group"), S.text("inception.window.row"), S.num("inception.finish.radius"), S.num("inception.finish.inc_max")), ("R_Finish", "SF_LIGHTINCEPTION", 600.0, 3000.0));
        assert!(weapon_units().iter().any(|u| u.ends_with("w_so200_Wep")) && weapon_units().iter().any(|u| u.ends_with("w_so210_Wep")));
        assert_eq!(AnimId::from_name(S.text("inception.clip_sequence")), Some(AnimId::N481));
        assert_eq!((S.num("inception.spawn.pitch"), S.num("inception.spawn.side"), S.num("inception.delay")), (30.0, 300.0, 0.3));
    }
}
