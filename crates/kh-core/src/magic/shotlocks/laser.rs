//! (SHOT-A laser) Spectral Rays (kind 11 "ラプ反射", kh3_system.command.sf_laser_0; Ever After's MAX kind): update
//! 0x1413B9600 (TresPlayerState_ShootFlow vtable 0x14693CA58 +0x4D0), spawner 0x1413C1A90 (+0x4D8). Rows `laser.*` of
//! sheet sora-shotlocks (tools/shotlocks/laser.js).
//!
//! What KH3 does (re/decomp/1413B/1413B9600.c, 1413C/1413C1A90.c and their disassembly; Begin 0x1413A5CD0 case 0xB: no
//! projectile name, group 0, voice 0):
//! - sub 0: gravity off, Flying (stopped); the WEAPON CHANGE pawn vt+0x13C8(weapon type, 10, ...) (ChangeWeapon
//!   0x14144A9F0) to weapon form 10 = Mirage Staff (form.WA: w_so072_Wep, anim set AnimSet10; no form is started); Play
//!   `M_A481` (blend -1, mode 2, group 0), voice 0; m_bFaceAt; sub 4; LiftCheck(`laser.lift.height` 200) -> sub 3.
//! - sub 3: LiftStep -> sub 4 (that tick ends there).
//! - sub 4 / 5: at M_A481's end (4.37 s) Play `ShootFlowEnd` (group 0), m_bEnableChange = 0, sub 2.
//! - sub 2: once m_bEnableChange is back (ShootFlowEnd's TresEnableChange 0.3 s) and +0xB0 != 0: Begin's weapon back
//!   (vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1)), WorldSlow off (FUN_14140E500(ctrl, 0): ctrl+0x878 bit 3, combat/form.rs
//!   `world_slow`; the aim mode's release is the common End's slot 809), +0xB0 = 0; at the clip's end Finish (the tail below still runs that tick). End (0x1413ABA70) gives the weapon back when that
//!   has not happened.
//! - Every tick, while flagsCC bit 0 is set (M_A481's TresAtk begin at 1.6 s, slot 64: also +0xA0 = 0.5 for kind 0xB):
//!   the spawner while spawnIndex < N, else the bit off; then SetFace(false, 0).
//! - The spawner 0x1413C1A90 ([`spawner`]): +0xA0 counts down; then one `SF_REFLECTIVELASER` (TresProjRA_ReflectiveLaser_C:
//!   speed 0, 1 s, a 30 cm sphere of row SF_REFLECTIVELASER, body particle vw_so072_stf_hit0) AT the locked target's
//!   lock-on point, facing away from Sora, fired at once; each locked target gets its own lock count of rays in list
//!   order, `laser.spawn.interval` (0.1 s; at 60 Hz 7 ticks: 0.1 - 6 dt is still > 0 in f32) apart, the last ones
//!   during ShootFlowEnd; the spawner stops with the state (at 60 Hz 27 of 28 locks fire before 5.1667 s).
//! - M_A481's attack window 1.6 .. 3.6 s (group `SF_Range`, row SF_REFLECTIVELASER_RANGE (TOSS: a launch), interval 1 s,
//!   hit effects of w_so072_Wep's `ReflecLaser` group) is the generic hit path's (combat/hit.rs); its shape is
//!   w_so072_Wep's SF_Range ([`hit_shape`]): a 10 m sphere on Sora's kh_root 5 m down. Every hit runs through the
//!   generic paths, so it works on every target the lock-on accepts.
//!
//! Both routes to kind 11 run here: Ever After's MAX kind (SelectKind) and XSI's Customize-screen choice (a port
//! addition, abilities/xs/spectral.rs). The weapon form is kept on the pawn (`PawnMagic::shot_weapon_form` 10): the
//! hosts draw form 10's own parts (w_so072_Wep on R_wep7_skt) meanwhile.

use glam::Vec3;

use crate::anim::{AnimId, NotifyClass, NotifyData, NotifyEvent, NotifyPhase, PlayArgs};
use crate::host::{EventKind, TargetFlags, TargetId};
use crate::magic::projectile::{self, SpawnArgs, StageDef};
use crate::magic::spells::cmd;
use crate::magic::tables::Sheet;
use crate::pawn::Ctx;
use crate::states::shoot_flow::ShootFlow;

const S: Sheet = Sheet::Shotlocks;

/// The family runs (`magic/shotlocks::ready`).
pub(crate) const PORTED: bool = true;

/// The family's fields of the state object: +0x110 / +0x114 (the locked entry the spawner serves and its rays so far),
/// +0xB0 != 0 (Begin's weapon form is to come back: `restore`; the port keeps the override on the pawn).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct State {
    entry: i32,
    shots: i32,
    restore: bool,
}

/// The weapon form sub 0 puts on (`laser.weapon_form`: 10, Mirage Staff).
pub(crate) fn weapon_form() -> u8 {
    S.int("laser.weapon_form") as u8
}

/// After the common Begin and the per-kind set-up: nothing of its own (Begin 0x1413A5CD0 case 0xB stores the delegate,
/// no projectile name, group 0 and voice 0).
pub(crate) fn begin(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

/// Begin's weapon back (sub 2 / End: vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1), +0xB0 = 0).
fn give_weapon_back(st: &mut State, cx: &mut Ctx) {
    if std::mem::take(&mut st.restore) {
        cx.pawn.magic.shot_weapon_form = 0;
    }
}

/// `Play(name, slot 0, blend -1, mode 2, group 0, rate 1)` (both Plays of 0x1413B9600 pass group 0).
fn play(cx: &mut Ctx, clip: AnimId) {
    cx.pawn.anim.play_with(clip, PlayArgs::mode(2, -1.0));
}

/// The update delegate 0x1413B9600; `false` = the state finished this tick (no PostTick).
pub(crate) fn update(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) -> bool {
    let mut finished = false;
    match sf.sub {
        0 => {
            let pawn = &mut *cx.pawn;
            pawn.cmc.gravity_off = true;
            pawn.cmc.stop_movement();
            pawn.cmc.set_flying();
            // vt+0x13C8(weapon type, 10, ...): Mirage Staff (its anim set AnimSet10 holds M_A481).
            pawn.magic.shot_weapon_form = weapon_form();
            st.restore = true;
            play(cx, AnimId::from_name(S.text("laser.clip_sequence")).unwrap_or(AnimId::A481));
            sf.face_request = true;
            sf.sub = 4;
            if sf.lift_check(cx, S.num("laser.lift.height")) {
                sf.sub = 3;
            }
        }
        3 => {
            if sf.lift_step(cx, dt) {
                sf.sub = 4;
            }
        }
        4 | 5 => {
            if cx.pawn.anim.ended {
                play(cx, AnimId::ShootFlowEnd);
                cx.pawn.queue.flags.enable_change = false;
                sf.sub = 2;
            }
        }
        _ => {
            if cx.pawn.queue.flags.enable_change && st.restore {
                give_weapon_back(st, cx);
                // FUN_14140E500(ctrl, 0) = WorldSlow off (ctrl+0x878 bit 3; nothing when it is not on). Not the release
                // handshake (slot 809, the common End's): the locked list stays for the spawner's last rays.
                crate::combat::form::world_slow(cx, false);
            }
            if cx.pawn.anim.ended {
                cx.pawn.queue.do_chain();
                finished = true;
            }
        }
    }
    if sf.ring_active {
        if sf.spawn_index < sf.n {
            spawner(sf, st, cx, dt);
        } else {
            sf.ring_active = false;
        }
        // SetFace(false, 0).
        sf.facing = false;
        sf.face_timer = 0.0;
    }
    !finished
}

/// SF_REFLECTIVELASER's class: the held Keyblade's projectile set (pawn vt+0x1450 -> equipment: Ever After's w_so070),
/// else weapon form 10's own set (`laser.proj_set`, w_so070) for the Customize screen's Spectral Rays with another
/// Keyblade (XSI's port addition: KH3 reaches kind 11 with Ever After only).
fn ray_def(cx: &Ctx) -> Option<StageDef> {
    let name = S.text("laser.proj_name");
    super::projectile_named(cx, name).or_else(|| {
        let class = S.text_or(&format!("projset.{}.{}", S.text("laser.proj_set"), name.to_ascii_lowercase()), "");
        StageDef::shot_class(class)
    })
}

/// The spawner vt+0x4D8 = 0x1413C1A90(dt): at most one SF_REFLECTIVELASER per call.
fn spawner(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) {
    if sf.ring_timer > 0.0 {
        sf.ring_timer -= dt;
        if sf.ring_timer > 0.0 {
            return;
        }
        sf.ring_timer = 0.0;
    }
    // The locked entry +0x110 (ctrl+0xB08): its component must still be there and not pending kill (the port: the host
    // still reports the target alive), else the next entry.
    let entry = usize::try_from(st.entry).ok().and_then(|i| cx.pawn.magic.aim.locked.get(i).copied());
    let Some((e, t)) = entry.and_then(|e| cx.target(e.target).filter(|t| t.flags.has(TargetFlags::ALIVE)).map(|t| (e, t))) else {
        st.entry += 1;
        st.shots = 0;
        return;
    };
    // pawn vt+0x1330 (0x1405ADE80, weapon out): Begin drew the weapon; nothing sheathes it in this state.
    // pawn vt+0x1450("SF_REFLECTIVELASER", T, T, T - centre, team, fire): T = the target component's location (vt+0xB88).
    let Some(def) = ray_def(cx) else { return };
    let direction = t.center - crate::magic::shotlock::lock_point(cx);
    projectile::spawn(cx, &SpawnArgs { def, position: t.center, direction, cmd: cmd::SHOOTFLOW, shoot_flow: sf.kind, finish: false, situation: false, full_mp: false, combo: [0; 4], target: TargetId::NONE, attached: None, fire: S.flag("laser.spawn.fire") });
    st.shots += 1;
    sf.spawn_index += 1;
    if st.shots >= e.lock_count {
        st.shots = 0;
        st.entry += 1;
    }
    sf.ring_timer = S.num("laser.spawn.interval");
}

/// Every notify event of a pose tick: M_A481's `TresAnimNotifyState_BpEvent` (0 .. 4.37 s: the state-event camera
/// blueprint's start / end events) goes to the hosts as `AnimBpEvent` (as Infinity Circle's). The window and its hits
/// are the generic path's.
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

/// The Shot handler's spawn point (none: M_A481 has no TresShot; the spawner places its rays itself).
pub(crate) fn muzzle(_sf: &ShootFlow, _st: &State, _cx: &Ctx, _index: usize) -> Option<Vec3> {
    None
}

/// The shape of a hit window of weapon form 10's own groups (combat/hit.rs `shape`): `SF_Range` of w_so072_Wep, a
/// sphere of `laser.range.radius` on Sora's kh_root (A481 has no root motion and keeps kh_sk at (0, 0, 0): his root =
/// the actor location, as SHOT-C's ultima) plus `laser.range.offset` turned by his yaw. `None` for any other group or
/// without the form.
pub(crate) fn hit_shape(cx: &Ctx, group: &str) -> Option<(Vec3, Vec3, f32)> {
    if cx.pawn.magic.shot_weapon_form != weapon_form() || group != S.text("laser.window.group") {
        return None;
    }
    let o = S.vec3("laser.range.offset");
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    let c = cx.pawn.cmc.location + Vec3::new(o.x * cos - o.y * sin, o.x * sin + o.y * cos, o.z);
    Some((c, c, S.num("laser.range.radius")))
}

/// The pawn hit effect of a window's `m_HitEffectGrpName` while weapon form 10 is on (the chooser 0x140628960 takes
/// w_so072_Wep's m_HitEffects entry of that group): `ReflecLaser` -> `laser.range.hit_effect` (vw_so072_stf_hit0).
pub(crate) fn hit_effect(cx: &Ctx, group: &str) -> Option<&'static str> {
    (cx.pawn.magic.shot_weapon_form == weapon_form() && group == S.text("laser.window.hit_effect_group")).then(|| S.text("laser.range.hit_effect"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_kh3_s() {
        assert_eq!(S.text("laser.update"), "0x1413B9600");
        assert_eq!(super::super::family(11), "laser");
        assert!(super::super::ready(11));
        // Ever After (weapon type 5): base 10 Shimmering Drops, MAX 11 Spectral Rays.
        assert_eq!((super::super::base_kind(5), super::super::max_kind(5, false)), (10, 11));
        assert_eq!(weapon_form(), 10);
        assert_eq!(AnimId::from_name(S.text("laser.clip_sequence")), Some(AnimId::A481));
        assert_eq!((S.num("laser.lift.height"), S.num("laser.spawn.interval"), S.num("laser.spawn.delay")), (200.0, 0.1, 0.5));
        assert_eq!((S.text("laser.proj_name"), S.text("laser.proj_class"), S.text("laser.proj_set")), ("SF_REFLECTIVELASER", "TresProjRA_ReflectiveLaser_C", "w_so070"));
        assert_eq!((S.text("laser.window.group"), S.text("laser.window.row"), S.num("laser.range.radius")), ("SF_Range", "SF_REFLECTIVELASER_RANGE", 1000.0));
        assert_eq!(S.vec3("laser.range.offset"), Vec3::new(0.0, 0.0, -500.0));
        assert_eq!(S.text("laser.range.hit_effect"), "vw_so072_stf_hit0");
        assert_eq!(StageDef::shot_class(S.text("laser.proj_class")).map(|d| d.class_name), Some("TresProjRA_ReflectiveLaser_C"));
    }
}
