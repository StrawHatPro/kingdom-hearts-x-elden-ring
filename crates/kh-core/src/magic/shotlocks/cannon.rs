//! (SHOT-A) Cluster Cannonade (kind 7 `BZ_CANNON`, Shooting Star's MAX kind when 0x1413ECC80 is false; SHOT-B: 22
//! `PO_CANNON` Sweet Surprise, Hunny Spout's): update 0x1413B6960 and its cluster call-back 0x1413B3BF0. Rows `cannon.*`
//! of sheet sora-shotlocks (tools/shotlocks/cannon.js).
//!
//! What KH3 does (decompiled C, constants re-read in the disassembly; the delegate is ShootFlow vtable 0x14693CA58 +
//! 0x3E0, Begin 0x1413A5CD0 cases 7 / 0x16: names BZ_CANNON / PO_CANNON, group 0, voice 0):
//! - sub 0: gravity off, Flying (stopped); the WEAPON CHANGE pawn vt+0x13C8(weapon type, 5, ...) (kind 0x16: 0x11) to
//!   the Magic Launcher (Hunny Launcher) and its anim set AnimSet5 (no form is started); Play `ShootFlowStart` (blend
//!   0.2, mode 2, group 0: the cannon never passes +0xC0), voice group 0 (+0x1F0) when the clip has a length; face
//!   request; sub 4; LiftCheck(200) -> sub 3.
//! - sub 3: LiftStep -> sub 4, then the sub 4 code in the same tick.
//! - sub 4 (ShootFlowStart 0.6667 s): at the clip's end Flying (stopped), Play `M_5482` (blend -1), face request again,
//!   sub 5. M_5482's TresShot (0.552 s) is the shared Shot handler's default branch (mod.rs `spawn_shot`): ONE shell
//!   (+0xB8) from weapon 0's ATK_01 along his facing, homing target locked[0] (its class has homing off: it flies
//!   straight, 1000 cm/s), kept at +0x118 (`ShootFlow::kept`).
//! - sub 5 (M_5482 1.7 s): at the clip's end Play `ShootFlowEnd`, m_bEnableChange = 0, sub 2.
//! - sub 2 (ShootFlowEnd 0.8 s): once m_bEnableChange is back (its TresEnableChange 0.3 s) and +0xB0 != 0: Begin's
//!   weapon form back (vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1)), +0xB0 = 0; at the clip's end Finish. End
//!   (0x1413ABA70) gives the weapon back when that has not happened.
//! - THE CLUSTER 0x1413B3BF0 (OnProjectileShutdown of the state's notify interface +0x48 vt+0x98): the shell's Shutdown
//!   (its 0.5 s life with m_bExplodeOnTimeOut, or a hit) calls it while this state is the current one; when the actor is
//!   the kept one, the kind 7 / 0x16 and +0x120 is 0: N (+0x94) `BZ_CANNON2` at the shell's location, the i-th along
//!   `Rotator(Quat(yaw of its horizontal forward, roll 360 / N x i) * Quat(pitch 45)).Vector()` (a 45 degree cone around
//!   the flight line, i = 0 at the top), homing on locked[+0x90 mod locked] (+0x90 counts on from the shell), then +0x120
//!   = 1. Explode's respawn then puts the shell's `CannonEx` blast there (magic/projectile.rs: the shell's hit list taken
//!   over). The port's Shutdown records the actor (`PawnMagic::shut_down`); [`update`] runs the call-back first thing in
//!   the same frame (the state's MainProc follows the projectile tick).
//! - The projectiles (plain TresProjectileBase: magic/projectile.rs's homing and the generic hits on every target):
//!   w_so020 BZ_CANNON = TresProjSS_Cannon_C -> TresProjSS_CannonEx_C, BZ_CANNON2 = TresProjSS_Cannon2_C, row SF_CANNON
//!   (BLOW); w_so040 PO_CANNON = TresProjPO_Cannon_C -> TresProjPO_CannonEx_C, BZ_CANNON2 = TresProjPO_Cannon2_C, row
//!   SF_POCANNON.
//!
//! The muzzle (pawn vt+0x1410(0): the launcher's ATK_01) is its FK track over M_5482 (rows `cannon.k<kind>.muzzle.5482`).
//! The weapon form is kept on the pawn (`PawnMagic::shot_weapon_form`): the hosts draw that form's weapon parts
//! (`cannon.k<kind>.weapon_units`) instead of the Keyblade meanwhile.

use glam::Vec3;

use crate::anim::{AnimId, NotifyClass, NotifyData, NotifyEvent, NotifyPhase, PlayArgs};
use crate::host::EventKind;
use crate::magic::projectile::{self, SpawnArgs};
use crate::magic::spells::cmd;
use crate::magic::tables::Sheet;
use crate::pawn::Ctx;
use crate::states::shoot_flow::ShootFlow;

const S: Sheet = Sheet::Shotlocks;

/// The family runs (`magic/shotlocks::ready`).
pub(crate) const PORTED: bool = true;

/// The family's fields of the state object: +0x120 (the cluster was spawned), +0xB0 != 0 (Begin's weapon form still to
/// come back; the port keeps the override on the pawn).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct State {
    cluster: bool,
    restore: bool,
}

/// The kind whose rows hold this kind's values: the code branches on 0x16 (Sweet Surprise) only, every other kind of the
/// update is Cluster Cannonade's (7).
fn row_kind(kind: u8) -> u8 {
    if kind == 0x16 {
        0x16
    } else {
        7
    }
}

/// The weapon form the update puts on (vt+0x13C8's second argument): 5 Magic Launcher, kind 0x16: 0x11 Hunny Launcher.
pub(crate) fn weapon_form(kind: u8) -> u8 {
    S.int(&format!("cannon.k{}.weapon_form", row_kind(kind))) as u8
}

/// The clip of a sub-step (`start` ShootFlowStart, `fire` M_5482 of AnimSet5, `end` ShootFlowEnd).
fn clip(key: &str) -> AnimId {
    let fallback = match key {
        "start" => AnimId::ShootFlowStart,
        "fire" => AnimId::A5482,
        _ => AnimId::ShootFlowEnd,
    };
    AnimId::from_name(S.text(&format!("cannon.clip.{key}"))).or_else(|| AnimId::from_name(S.text(&format!("cannon.clip.{key}.sequence")))).unwrap_or(fallback)
}

/// `Play(clip, slot 0, blend, mode 2, group 0, rate 1)`, then the voice group (anim interface +0x1F0(0)) when it has a
/// length.
fn play(cx: &mut Ctx, clip: AnimId, blend: f32) {
    cx.pawn.anim.play_with(clip, PlayArgs::mode(2, blend));
    if clip.data().length > 0.0 {
        cx.set_voice_group_no(S.int("cannon.voice_group"));
    }
}

/// After the common Begin and the per-kind set-up: nothing of its own (Begin cases 7 / 0x16: the delegate, the name).
pub(crate) fn begin(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

/// Begin's weapon form back (sub 2 / End: vt+0x13C8(type, +0xB0, +0xB1), +0xB0 = 0).
fn give_weapon_back(st: &mut State, cx: &mut Ctx) {
    if std::mem::take(&mut st.restore) {
        cx.pawn.magic.shot_weapon_form = 0;
    }
}

/// Sub 0: the weapon change, ShootFlowStart, the lift check.
fn start(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx) {
    let pawn = &mut *cx.pawn;
    pawn.cmc.gravity_off = true;
    pawn.cmc.stop_movement();
    pawn.cmc.set_flying();
    pawn.magic.shot_weapon_form = weapon_form(sf.kind);
    st.restore = true;
    play(cx, clip("start"), S.num("cannon.clip.start.blend"));
    sf.face_request = true;
    sf.sub = 4;
    if sf.lift_check(cx, S.num("cannon.lift.height")) {
        sf.sub = 3;
    }
}

/// OnProjectileShutdown 0x1413B3BF0 for the actors this frame's projectile tick shut down: the kept shell's cluster.
fn cluster(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx) {
    if st.cluster || sf.kept == 0 || !matches!(sf.kind, 7 | 0x16) {
        return;
    }
    let Some(&(_, at, forward)) = cx.pawn.magic.shut_down.iter().find(|s| s.0 == sf.kept) else { return };
    // f = the shell's forward with z = 0 (normalised when |f.xy|^2 > 1e-8): R = f.Rotation() (pitch 0, roll 0); the
    // yaw is atan2 of the raw xy (the same either way).
    let yaw = forward.y.atan2(forward.x);
    let a = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
    let right = Vec3::new(-a.y, a.x, 0.0);
    let (sp, cp) = S.num("cannon.cluster.pitch").to_radians().sin_cos();
    let step = S.num("cannon.cluster.spin") / sf.n as f32;
    // (a weapon without BZ_CANNON2 spawns nothing: pawn vt+0x1450 returns null N times)
    if let Some(def) = super::projectile_named(cx, S.text("cannon.cluster.name")) {
        for i in 0..sf.n {
            // R.roll += 360 / N x i; Quat(R) * Quat(pitch 45) -> Vector: cos 45 x a + sin 45 x (sin roll x right + cos
            // roll x up).
            let (sr, cr) = (step * i as f32).to_radians().sin_cos();
            let direction = a * cp + (right * sr + Vec3::Z * cr) * sp;
            let target = super::target_for(sf, cx, sf.spawn_index);
            projectile::spawn(cx, &SpawnArgs { def, position: at, direction, cmd: cmd::SHOOTFLOW, shoot_flow: sf.kind, finish: false, situation: false, full_mp: false, combo: [0; 4], target, attached: None, fire: true });
            sf.spawn_index += 1;
        }
    }
    st.cluster = true;
}

/// The update delegate 0x1413B6960; `false` = the state finished this tick (no PostTick).
pub(crate) fn update(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) -> bool {
    cluster(sf, st, cx);
    match sf.sub {
        0 => start(sf, st, cx),
        3 | 4 => {
            if sf.sub == 3 && sf.lift_step(cx, dt) {
                sf.sub = 4;
            }
            // (sub 3 falls into the sub 4 code)
            if cx.pawn.anim.ended {
                // vt+0x17A0(5, 1): Flying, stopped; M_5482; a face request again.
                cx.pawn.cmc.stop_movement();
                cx.pawn.cmc.set_flying();
                play(cx, clip("fire"), -1.0);
                sf.face_request = true;
                sf.sub = 5;
            }
        }
        5 => {
            if cx.pawn.anim.ended {
                play(cx, clip("end"), -1.0);
                cx.pawn.queue.flags.enable_change = false;
                sf.sub = 2;
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
/// / end events; M_5482 has a second one, param 1, 0.4945 - 1.3422 s) goes to the hosts as `AnimBpEvent`, as Infinity
/// Circle's. The shell is the shared Shot handler's.
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

/// The Shot handler's spawn point (pawn vt+0x1410(i) 0x1405AA790: weapon[i]'s ATK_01; the launcher is weapon 0, the only
/// one, so every index is 0's): its FK track over M_5482, linear between frames, turned by his yaw onto his location.
/// `None` without the weapon form or outside M_5482 (the framework's fallback).
pub(crate) fn muzzle(sf: &ShootFlow, _st: &State, cx: &Ctx, _index: usize) -> Option<Vec3> {
    let kind = row_kind(sf.kind);
    if cx.pawn.magic.shot_weapon_form != weapon_form(kind) || cx.pawn.anim.name != clip("fire") {
        return None;
    }
    let seq = S.text("cannon.clip.fire.sequence");
    let track = S.list(&format!("cannon.k{kind}.muzzle.{seq}"));
    let frames = track.len() / 3;
    if frames == 0 {
        return None;
    }
    let length = S.num("cannon.clip.fire.length");
    let f = if length > 0.0 { (cx.pawn.anim.time / length).clamp(0.0, 1.0) * (frames - 1) as f32 } else { 0.0 };
    let (i, w) = ((f.floor() as usize).min(frames - 1), f - f.floor());
    let j = (i + 1).min(frames - 1);
    let at = |k: usize| track[i * 3 + k] + (track[j * 3 + k] - track[i * 3 + k]) * w;
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    let (x, y, z) = (at(0), at(1), at(2));
    Some(cx.pawn.cmc.location + Vec3::new(x * cos - y * sin, x * sin + y * cos, z))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_kh3_s() {
        assert_eq!(S.text("cannon.update"), "0x1413B6960");
        assert_eq!((super::super::family(7), super::super::family(22)), ("cannon", "cannon"));
        assert!(super::super::ready(7) && super::super::ready(22));
        // Shooting Star: base 5, MAX 7 (the alternative MAX 6).
        assert_eq!((super::super::base_kind(3), super::super::max_kind(3, false)), (5, 7));
        assert_eq!((weapon_form(7), weapon_form(0x16)), (5, 17));
        assert_eq!((clip("start"), clip("fire"), clip("end")), (AnimId::ShootFlowStart, AnimId::A5482, AnimId::ShootFlowEnd));
        assert_eq!((S.num("cannon.lift.height"), S.num("cannon.clip.start.blend"), S.num("cannon.cluster.pitch"), S.num("cannon.cluster.spin")), (200.0, 0.2, 45.0, 360.0));
        assert_eq!((S.text("cannon.k7.proj_class"), S.text("cannon.k7.ex_class"), S.text("cannon.k7.cluster_class"), S.text("cannon.k7.row")), ("TresProjSS_Cannon_C", "TresProjSS_CannonEx_C", "TresProjSS_Cannon2_C", "SF_CANNON"));
        assert_eq!((S.text("cannon.k22.proj_class"), S.text("cannon.k22.cluster_class"), S.text("cannon.k22.row")), ("TresProjPO_Cannon_C", "TresProjPO_Cannon2_C", "SF_POCANNON"));
        assert!((S.num("cannon.fire.shot_time") - 0.552).abs() < 1e-3);
        for kind in [7, 22] {
            assert!(S.list(&format!("cannon.k{kind}.muzzle.5482")).len() >= 30, "{kind}");
        }
    }
}
