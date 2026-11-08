//! (SHOT-A drain) Drain Shock (kind 3 `HE_DRAIN`, Hero's Origin) / Shimmering Drops (kind 10 `RA_DRAIN`, Ever After): the
//! update 0x1413B73D0 (ShootFlow vtable 0x14693CA58 +0x468). Rows `drain.*` of sheet sora-shotlocks
//! (tools/shotlocks/drain.js).
//!
//! What KH3 does (decompiled C, the constants read in the disassembly):
//! - sub 0: gravity off, Flying (stopped), vt+0x1388(1, 0, 0) (weapon presentation: not kept, as King of Hearts); Play
//!   `StyleChange11` (blend -1, mode 2, group 0xC78), voice group 1; m_bFaceAt; sub 4, or sub 3 when LiftCheck(100) lifts.
//! - sub 3: LiftStep; then the sub 4 code in the same tick.
//! - sub 4: the clip's TresTiming (0.2 s; OnTiming 0x1413B57D0 sets flagsCC bit 0 for kinds 3 / 10) or its end:
//!   StopMovement, bit 0 off, vt+0x470 (0x1413BFBF0): ONE `HE_DRAIN_BLOW` (kind 3; any other kind `RA_DRAIN_BLOW`) at his
//!   lock point along his facing (TresProjHE_Drain_C: a static 800 cm SPHERE, row SF_DRAIN_BLOW, 0.5 s); sub 5.
//! - sub 5: the TresAtk begin (1.2333 s; slot 64 sets bit 0): vt+0x470 again (sub 5): the hit counter +0x114 = 0, and per
//!   lock entry, per lock, one `+0xB8` projectile (`HE_DRAIN` / `RA_DRAIN` = TresProj*_DrainEx_C, a 30 cm SPHERE, row
//!   SF_DRAIN / SF_RADRAIN, 1 s) at the target's lock-on point; +0xA0 = +0xA4 = 0.8 s; sub 6.
//! - THE HIT CALL-BACK 0x1413B3780 (state+0x48 slot 18, from the pawn's attack-hit dispatch 0x1405B3A80): a hit of an
//!   attacker with `m_bAttachToHitActor` (only the DrainEx classes) on an "Enemy" character, not guarded: one drain orb
//!   per victim (the held weapon's `m_pDrainEffect`, vp_dra_blt02, spawned at the victim) or the victim's count + 1;
//!   +0x114 += 1.
//! - sub 6: vt+0x478 (0x1413BDE70) moves the orbs: orb `i` of `n` is pulled toward a point of a ring around his lock point
//!   (direction: 45 degrees up from his facing, rolled `i x 360 / n + elapsed x 360` about it; radius `0.8 x 500`, after
//!   half the time shrinking by 1000 cm/s to 0) by the elapsed fraction, and rises `(left - half) x dt x 20000` while more
//!   than half the time is left; +0xA4 -= dt; at 0: when the drains hit (+0x114 > 0), Recover(6 x the locks of every lock
//!   entry, 0, 0, 0xD) - HP for Sora; FUN_14140E500(ctrl, 0) (the attack slow motion off: not kept, as King of Hearts);
//!   vt+0x480 (0x1413BD870): the orbs off; sub 7. Every sub 6 tick then runs the sub 7 code too.
//! - sub 7: at StyleChange11's end: Play `M_1472` (group 0), voice group 0, m_bEnableChange = 0; sub 2.
//! - sub 2: at its end vt+0x1388(0, 0, 0) and Finish.
//! - End 0x1413ABA70 removes orbs still on (vt+0x368).
//!
//! No MP or focus: the only recovery is the HP of sub 6. The hits run the generic projectile path (magic/projectile.rs);
//! the call-back reads them from this step's `Hit` events (the projectiles tick before the state in the same step).

use glam::Vec3;

use super::projectile_named;
use crate::anim::{AnimId, NotifyEvent, PlayArgs};
use crate::host::{Attach, EffectHandle, EffectId, EffectSpawn, EventKind, TargetFlags, TargetId};
use crate::magic::projectile::{self, SpawnArgs};
use crate::magic::shotlock::lock_point;
use crate::magic::spells::cmd;
use crate::magic::tables::Sheet;
use crate::pawn::Ctx;
use crate::states::shoot_flow::ShootFlow;

const S: Sheet = Sheet::Shotlocks;

/// The family runs (`magic/shotlocks::ready`).
pub(crate) const PORTED: bool = true;

/// One entry of the orb list +0x148 (0x24 bytes): the orb's particle, its victim (+0x1C) and the victim's drain hits
/// (+0x18). The list is kept on the pawn (`PawnMagic::drain_orbs`, as `volley`: the state struct is `Copy`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Orb {
    target: TargetId,
    effect: EffectHandle,
    /// The particle's world location (the component's, which the motion reads back every tick).
    position: Vec3,
    count: i32,
}

/// The family's fields of the state object: the hit counter +0x114 and the step whose hit events were read last.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct State {
    hits: i32,
    read_step: u64,
}

/// After the common Begin and the per-kind set-up: nothing of its own (Begin 0x1413A5CD0 cases 3 / 10: name, group 0,
/// voice 0).
pub(crate) fn begin(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

/// Play `clip` (slot 0, blend -1, mode 2) with effect group `group` (0: none); then the voice group (anim interface
/// +0x1F0) when the clip has a length.
fn play(cx: &mut Ctx, clip: AnimId, group: i32, voice_group: i32) {
    if group != 0 {
        cx.pawn.next_effect_group = Some(group);
    }
    cx.pawn.anim.play_with(clip, PlayArgs::mode(2, -1.0));
    if clip.data().length > 0.0 {
        cx.set_voice_group_no(voice_group);
    }
}

/// The update delegate 0x1413B73D0; `false` = the state finished this tick (no PostTick).
pub(crate) fn update(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) -> bool {
    on_hits(sf, st, cx);
    match sf.sub {
        0 => {
            let pawn = &mut *cx.pawn;
            pawn.cmc.gravity_off = true;
            pawn.cmc.stop_movement();
            pawn.cmc.set_flying();
            play(cx, AnimId::StyleChange11, S.int("drain.clip_group"), S.int("drain.voice_group.start"));
            sf.face_request = true;
            sf.sub = 4;
            if sf.lift_check(cx, S.num("drain.lift.height")) {
                sf.sub = 3;
            }
        }
        3 | 4 => {
            if sf.sub == 3 && sf.lift_step(cx, dt) {
                sf.sub = 4;
            }
            // (sub 3 falls into the sub 4 code)
            if sf.ring_active || cx.pawn.anim.ended {
                cx.pawn.cmc.stop_movement();
                sf.ring_active = false;
                spawn(sf, st, cx);
                sf.sub = 5;
            }
        }
        5 => {
            if sf.ring_active {
                spawn(sf, st, cx);
                sf.ring_timer = S.num("drain.timer");
                sf.syphon_timer = sf.ring_timer;
                sf.sub = 6;
            }
        }
        6 | 7 => {
            if sf.sub == 6 {
                move_orbs(sf, cx, dt);
                sf.syphon_timer -= dt;
                if sf.syphon_timer <= 0.0 {
                    if st.hits > 0 {
                        // Recover(6 x the locks of every lock entry, 0, 0, 0xD) (pawn vt+0x1878).
                        let total: i32 = cx.pawn.magic.aim.locked.iter().map(|e| e.lock_count).sum();
                        cx.pawn.status.recover(total * S.int("drain.heal.per_lock"), 0, 0, S.int("drain.heal.kind") as u8);
                    }
                    // FUN_14140E500(ctrl, 0): the attack slow motion off (the port keeps none, as King of Hearts).
                    orbs_off(cx, false);
                    sf.sub = 7;
                }
            }
            // The sub 7 code (sub 6 runs it every tick too).
            if cx.pawn.anim.ended {
                play(cx, AnimId::M1472, 0, S.int("drain.voice_group.end"));
                cx.pawn.queue.flags.enable_change = false;
                sf.sub = 2;
            }
        }
        _ => {
            if cx.pawn.anim.ended {
                cx.pawn.queue.do_chain();
                return false;
            }
        }
    }
    true
}

/// vt+0x470 0x1413BFBF0: in sub 5 the drains (+0xB8 per lock at every lock entry's target, the hit counter reset), else
/// the one blow at his lock point. Both along his facing (the root rotation's vector), fired at once, no homing target;
/// vt+0x1100(cmd, kind) = the flags of their hits.
fn spawn(sf: &ShootFlow, st: &mut State, cx: &mut Ctx) {
    let yaw = cx.pawn.yaw.to_radians();
    let direction = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
    let kind = sf.kind;
    let args = |def, position| SpawnArgs { def, position, direction, cmd: cmd::SHOOTFLOW, shoot_flow: kind, finish: false, situation: false, full_mp: false, combo: [0; 4], target: TargetId::NONE, attached: None, fire: true };
    if sf.sub == 5 {
        st.hits = 0;
        let Some(def) = projectile_named(cx, sf.proj_name) else { return };
        let locked: Vec<(TargetId, i32)> = cx.pawn.magic.aim.locked.iter().map(|e| (e.target, e.lock_count)).collect();
        for (target, count) in locked {
            // The entry's lock-on component (vt+0xB88: the target's lock-on point) and its owner must still exist.
            let Some(info) = cx.target(target) else { continue };
            for _ in 0..count {
                projectile::spawn(cx, &args(def, info.center));
            }
        }
    } else {
        let name = if kind == 3 { S.text("drain.k3.blow") } else { S.text("drain.k10.blow") };
        if let Some(def) = projectile_named(cx, name) {
            let at = lock_point(cx);
            projectile::spawn(cx, &args(def, at));
        }
    }
}

/// The held weapon's drain orb (weapon 0's `m_pDrainEffect`, `drain.orb.effect.<w_soNNN>`); `None` without one.
fn orb_effect(cx: &Ctx) -> Option<&'static str> {
    let id = cx.pawn.status.current_keyblade()?;
    let weapon = id.row().blueprint.rsplit('/').next()?.trim_end_matches("_Wep").to_ascii_lowercase();
    let name = S.text_or(&format!("drain.orb.effect.{weapon}"), "");
    (!name.is_empty()).then_some(name)
}

/// The hit call-back 0x1413B3780 for this step's hits: kinds 3 / 10, an attacker with `m_bAttachToHitActor` (the
/// kind's drain projectile: its flags and row), an ENEMY victim, not guarded (result 5).
fn on_hits(sf: &ShootFlow, st: &mut State, cx: &mut Ctx) {
    if st.read_step == cx.pawn.step {
        return;
    }
    st.read_step = cx.pawn.step;
    let kind = sf.kind;
    if !matches!(kind, 3 | 10) || !S.flag_or(&format!("drain.k{kind}.drain_attach"), false) {
        return;
    }
    let row = S.text_or(&format!("drain.k{kind}.drain_row"), "-");
    let flags = cmd::SHOOTFLOW as u32 | (kind as u32) << 8;
    let victims: Vec<TargetId> = cx
        .pawn
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::Hit { record, result } if record.flags == flags && record.attack_name == row && result.accepted && !result.guarded => Some(record.target),
            _ => None,
        })
        .collect();
    for target in victims {
        let Some(info) = cx.target(target).filter(|t| t.flags.has(TargetFlags::ENEMY)) else { continue };
        if let Some(orb) = cx.pawn.magic.drain_orbs.iter_mut().find(|o| o.target == target) {
            orb.count += 1;
            st.hits += 1;
            continue;
        }
        // SpawnEmitterAtLocation(weapon 0's m_pDrainEffect, the victim's location, no rotation); none: no entry, no count.
        let Some(name) = orb_effect(cx) else { continue };
        let effect = cx.spawn_effect(EffectSpawn { id: EffectId(u32::MAX), name, at: Attach::World(info.center), rotation: Vec3::ZERO, scale: 1.0, persistent: true });
        cx.pawn.magic.drain_orbs.push(Orb { target, effect, position: info.center, count: 1 });
        st.hits += 1;
    }
}

/// vt+0x478 0x1413BDE70 (`dt` the update's): every orb pulled toward its ring point by the elapsed fraction.
fn move_orbs(sf: &ShootFlow, cx: &mut Ctx, dt: f32) {
    let n = cx.pawn.magic.drain_orbs.len();
    if n == 0 {
        return;
    }
    let (time, left) = (sf.ring_timer, sf.syphon_timer);
    let elapsed = time - left;
    let half = time * S.num("drain.orb.half");
    let centre = lock_point(cx);
    let yaw = cx.pawn.yaw.to_radians();
    let forward = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
    let right = Vec3::new(-forward.y, forward.x, 0.0);
    let (spin, pitch) = (S.num("drain.orb.spin"), S.num("drain.orb.pitch").to_radians());
    let mut radius = time * S.num("drain.orb.radius_rate");
    if elapsed > half {
        radius += (elapsed - half) * S.num("drain.orb.shrink_rate");
    }
    let rise = S.num("drain.orb.rise_rate");
    for (i, orb) in cx.pawn.magic.drain_orbs.iter_mut().enumerate() {
        // FRotator(his pitch, yaw, roll + i x 360 / n + elapsed x 360) x FRotator(45, 0, 0), its vector.
        let roll = (i as f32 * (spin / n as f32) + elapsed * spin).to_radians();
        let direction = forward * pitch.cos() + (right * roll.sin() + Vec3::Z * roll.cos()) * pitch.sin();
        let goal = centre + direction * radius;
        let c = orb.position;
        let mut z = c.z;
        if left > half {
            z += (left - half) * (dt * rise);
        }
        let d = goal - c;
        orb.position = Vec3::new(c.x + elapsed * d.x, c.y + elapsed * d.y, z + elapsed * d.z);
        // SetWorldLocation of the particle component.
        cx.host.effects.place(orb.effect, orb.position);
    }
}

/// vt+0x480 0x1413BD870 (`immediate` false: the component's vt+0xAE8) / End (vt+0x368, `immediate`): the orbs off.
fn orbs_off(cx: &mut Ctx, immediate: bool) {
    for orb in std::mem::take(&mut cx.pawn.magic.drain_orbs) {
        cx.stop_effect(orb.effect, immediate);
    }
}

/// Every notify event of a pose tick, after the shared handlers (they set flagsCC bit 0 on TresTiming / TresAtk begin):
/// nothing of its own.
pub(crate) fn on_notify(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx, _event: &NotifyEvent) {}

/// Before the common End's release of the aim mode: End 0x1413ABA70 removes the orbs still on.
pub(crate) fn end(_sf: &mut ShootFlow, _st: &mut State, cx: &mut Ctx) {
    orbs_off(cx, true);
}

/// The Shot handler's spawn point (none: the clips have no TresShot; the Shot handler only flags kinds 3 / 10).
pub(crate) fn muzzle(_sf: &ShootFlow, _st: &State, _cx: &Ctx, _index: usize) -> Option<Vec3> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_kh3_s() {
        for kind in [3, 10] {
            assert_eq!(super::super::family(kind), "drain");
            assert!(super::super::ready(kind));
        }
        assert_eq!((super::super::base_kind(2), super::super::base_kind(5)), (3, 10));
        assert_eq!((S.text("drain.clip"), S.num("drain.clip_length"), S.int("drain.clip_group")), ("StyleChange11", 2.5, 0xC78));
        assert_eq!((S.text("drain.end_clip"), S.num("drain.lift.height"), S.num("drain.timer")), ("M_1472", 100.0, 0.8));
        assert_eq!((S.text("drain.k3.blow"), S.text("drain.k10.blow")), ("HE_DRAIN_BLOW", "RA_DRAIN_BLOW"));
        assert_eq!((S.text("drain.k3.drain_row"), S.text("drain.k10.drain_row")), ("SF_DRAIN", "SF_RADRAIN"));
        assert!(S.flag("drain.k3.drain_attach") && !S.flag("drain.k3.blow_attach"));
        assert_eq!((S.int("drain.heal.per_lock"), S.int("drain.heal.kind")), (6, 0xD));
        assert_eq!(S.text("drain.orb.effect.w_so030"), "vp_dra_blt02");
        assert!((AnimId::StyleChange11.data().length - 2.5).abs() < 1.0e-4);
    }
}
