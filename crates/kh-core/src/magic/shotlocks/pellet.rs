//! (SHOT-A pellet) Atomic Deluge (kind 4 "HE散弾", kh3_system.command.sf_pellet_0; Hero's Origin's MAX kind; SHOT-B:
//! kind 24 Fruit Crusher, Grand Chef's MAX kind): update 0x1413B9910 (TresPlayerState_ShootFlow vtable 0x14693CA58
//! +0x488). Rows `pellet.*` of sheet sora-shotlocks (tools/shotlocks/pellet.js).
//!
//! What KH3 does (re/decomp/1413B/1413B9910.c and its disassembly; the spawner 0x1413C1E40):
//! - sub 0: gravity off, Flying (stopped); the WEAPON CHANGE pawn vt+0x13C8 (ChangeWeapon 0x14144A9F0) to weapon form
//!   0x13 for kind 0x18 (Frying Pan) else 0x0B (Counter Shield), grow LVL01; then pawn vt+0x1400 (equipment vt+0x518
//!   0x14178EE30) equips that set's TRANSFORM units (m_TransformSetData: the Deform weapon w_so031_wep / w_so141_Wep)
//!   instead of its m_Equips; Play `M_B481` (AnimSet11, mode 2, group +0xC0); m_bFaceAt; sub 4. No lift (M_B481's own
//!   root motion raises him, up to 271 cm; End then selects Falling when he is still in the air).
//! - sub 4 / 5: at M_B481's end pawn vt+0x13E8(2, 0, 0.0) (0x1405BD770: ChangeWeapon of the form on with grow 2 =
//!   LVL03: its m_Equips, the transform set off), Play `ShootFlowEnd` (group 0), m_bEnableChange = 0, sub 2.
//! - sub 2: once m_bEnableChange is back (ShootFlowEnd's TresEnableChange) and +0xB0 != 0: vt+0x1408 (transform set
//!   off), ChangeWeapon(type, +0xB0, +0xB1) = Begin's weapon back, +0xB0 = 0; at the clip's end Finish. (Unlike Infinity
//!   Circle no release-pending call here: the aim mode closes at End.)
//! - Every tick, while flagsCC bit 0 is set (M_B481's TresAtk begin, slot 64: also +0xA0 = 0.3 for kind 4): the spawner
//!   while spawnIndex < N, else the bit off; then SetFace(false, 0).
//! - The spawner 0x1413C1E40 (`spawner`): one `SF_SHIELDSHOT` (the weapon's projectile set: TresProjHE_ShieldShot_C /
//!   TresProjRE_ShieldShot_C: speed 0, 1 s, a 30 cm sphere) AT the lock-on point of the locked target, facing away from
//!   Sora, fired at once; each locked target gets its own lock count of shots in list order; the wait before the next
//!   one is (6 - spawnIndex) / 60 s (5, 4, 3, 2, 1 ticks, then one per tick).
//! - M_B481's attack window 2.1 .. 4.37 s (group SphereL, row SF_SHIELDSHOT_BLOW, no interval) is the generic hit path's
//!   (combat/hit.rs); its shape is the transform unit's SphereL ([`hit_shape`]): w_so031_wep's 10 m sphere on Sora's
//!   kh_root 5 m down. Every hit runs through the generic paths, so it works on every target the lock-on accepts.
//!
//! The weapon is kept on the pawn: `PawnMagic::shot_weapon_form` (the form) and `shot_weapon_units` (the transform set,
//! then the LV3 parts); the hosts draw those units meanwhile (INT).

use glam::Vec3;

use crate::anim::{AnimId, NotifyClass, NotifyData, NotifyEvent, NotifyPhase, PlayArgs};
use crate::host::{EventKind, TargetFlags, TargetId};
use crate::magic::projectile::{self, SpawnArgs};
use crate::magic::spells::cmd;
use crate::magic::tables::{Sheet, PV};
use crate::pawn::Ctx;
use crate::states::shoot_flow::ShootFlow;

const S: Sheet = Sheet::Shotlocks;

/// The family runs (`magic/shotlocks::ready`).
pub(crate) const PORTED: bool = true;

/// The family's fields of the state object: +0x110 / +0x114 (the locked entry the spawner serves and its shots so far),
/// +0xB0 != 0 (Begin's weapon form is to come back: `restore`) and the form the update put on.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct State {
    entry: i32,
    shots: i32,
    restore: bool,
    form: u8,
}

/// The weapon form sub 0 puts on for `kind` (0x1413B9AD7: 0x13 for kind 0x18, else 0x0B).
pub fn weapon_form(kind: u8) -> u8 {
    if i32::from(kind) == S.int("pellet.weapon_form.kind") {
        S.int("pellet.weapon_form.that_kind") as u8
    } else {
        S.int("pellet.weapon_form.other") as u8
    }
}

/// The units of an equip set of weapon form `form` (`which`: "transform" = m_TransformSetData of LVL01, "g2" = the
/// m_Equips of grow 2 = LVL03); empty when the sheet has none.
pub fn weapon_units(form: u8, which: &str) -> &'static [&'static str] {
    match S.find(&format!("pellet.units.f{form}.{which}")) {
        Some(PV::L(units)) => units,
        _ => &[],
    }
}

/// After the common Begin and the per-kind set-up: nothing of its own (Begin 0x1413A5CD0 cases 4 / 24 store the
/// delegate, no projectile name, the group and the voice).
pub(crate) fn begin(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

/// Begin's weapon back (sub 2 / End: vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1), +0xB0 = 0).
fn give_weapon_back(st: &mut State, cx: &mut Ctx) {
    if std::mem::take(&mut st.restore) {
        cx.pawn.magic.shot_weapon_form = 0;
        cx.pawn.magic.shot_weapon_units = &[];
    }
}

/// The update delegate 0x1413B9910; `false` = the state finished this tick (no PostTick).
pub(crate) fn update(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) -> bool {
    let mut finished = false;
    match sf.sub {
        0 => {
            let pawn = &mut *cx.pawn;
            pawn.cmc.gravity_off = true;
            pawn.cmc.stop_movement();
            pawn.cmc.set_flying();
            // ChangeWeapon(type, form, LVL01), then vt+0x1400: the form's transform set.
            st.form = weapon_form(sf.kind);
            pawn.magic.shot_weapon_form = st.form;
            pawn.magic.shot_weapon_units = weapon_units(st.form, "transform");
            st.restore = true;
            let clip = AnimId::from_name(S.text("pellet.clip_sequence")).unwrap_or(AnimId::B481);
            super::play(sf, cx, clip);
            sf.face_request = true;
            sf.sub = 4;
        }
        4 | 5 => {
            if cx.pawn.anim.ended {
                // vt+0x13E8(2, 0, 0.0): the form's LV3 parts (grow `pellet.end_grow`), the transform set off.
                let grow = S.int("pellet.end_grow");
                cx.pawn.magic.shot_weapon_units = weapon_units(st.form, &format!("g{grow}"));
                cx.pawn.anim.play_with(AnimId::ShootFlowEnd, PlayArgs::mode(2, -1.0));
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

/// The spawner vt+0x490 = 0x1413C1E40(dt): at most one SF_SHIELDSHOT per call.
fn spawner(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) {
    if sf.ring_timer > 0.0 {
        sf.ring_timer -= dt;
        if sf.ring_timer > 0.0 {
            return;
        }
        sf.ring_timer = 0.0;
    }
    // The locked entry +0x110 (ctrl+0xB08): its component must still be there (the port: the host still reports the
    // target alive; the host keeps dead bodies, KH3 destroys them), else the next entry.
    let entry = usize::try_from(st.entry).ok().and_then(|i| cx.pawn.magic.aim.locked.get(i).copied());
    let Some((e, t)) = entry.and_then(|e| cx.target(e.target).filter(|t| t.flags.has(TargetFlags::ALIVE)).map(|t| (e, t))) else {
        st.entry += 1;
        st.shots = 0;
        return;
    };
    // pawn vt+0x1330 (weapon out): Begin drew the weapon; the port has no other state that sheathes it here.
    // pawn vt+0x1450("SF_SHIELDSHOT", T, T, T - eye, team, fire): T = the target component's location (vt+0xB88).
    let Some(def) = super::projectile_named(cx, S.text("pellet.proj_name")) else { return };
    let direction = t.center - crate::magic::shotlock::lock_point(cx);
    projectile::spawn(cx, &SpawnArgs { def, position: t.center, direction, cmd: cmd::SHOOTFLOW, shoot_flow: sf.kind, finish: false, situation: false, full_mp: false, combo: [0; 4], target: TargetId::NONE, attached: None, fire: S.flag("pellet.spawn.fire") });
    st.shots += 1;
    sf.spawn_index += 1;
    if st.shots >= e.lock_count {
        st.shots = 0;
        st.entry += 1;
    }
    sf.ring_timer = ((S.int("pellet.spawn.base") - sf.spawn_index) as f32 * (1.0 / S.num("pellet.spawn.rate"))).max(0.0);
}

/// Every notify event of a pose tick: M_B481's `TresAnimNotifyState_BpEvent` (0 .. 5.13 s: the state-event camera
/// blueprint BP_TresPlayerStateEventCam_ShootFlow's start / end events) goes to the hosts as `AnimBpEvent` (as
/// Infinity Circle's). The window and its hits are the generic path's.
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

/// The Shot handler's spawn point (none: M_B481 has no TresShot; the spawner places its shots itself).
pub(crate) fn muzzle(_sf: &ShootFlow, _st: &State, _cx: &Ctx, _index: usize) -> Option<Vec3> {
    None
}

/// The shape of a hit window's group on the units a pellet shotlock put on (combat/hit.rs `shape`): a SPHERE of
/// `pellet.shape.<unit>.<group>.*` on Sora's mesh socket kh_root (OWNER_ATTACHED_MESH; B481 is a root-motion clip: its
/// root track (up to 271 cm up) moves the actor and kh_sk keeps its reference pose, so kh_root = the actor location, as
/// SHOT-C's ultima) plus the offset turned by his yaw. `None` for any other group, without such a unit, or for
/// a shape on the unit's own mesh (w_so141_Wep's ATK_01: no socket track of that mesh in the port).
pub(crate) fn hit_shape(cx: &Ctx, group: &str) -> Option<(Vec3, Vec3, f32)> {
    for unit in cx.pawn.magic.shot_weapon_units {
        let blueprint = unit.rsplit('/').next().unwrap_or("").to_ascii_lowercase();
        let key = format!("pellet.shape.{blueprint}.{}", group.to_ascii_lowercase());
        if S.text_or(&format!("{key}.type"), "-") != "SPHERE" || S.text_or(&format!("{key}.attach"), "-") != "OWNER_ATTACHED_MESH" || S.text_or(&format!("{key}.socket"), "-") != "kh_root" {
            continue;
        }
        let o = S.vec3(&format!("{key}.offset"));
        let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
        let c = cx.pawn.cmc.location + Vec3::new(o.x * cos - o.y * sin, o.x * sin + o.y * cos, o.z);
        return Some((c, c, S.num(&format!("{key}.radius"))));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_kh3_s() {
        assert_eq!((weapon_form(4), weapon_form(24)), (11, 19));
        assert_eq!((super::super::family(4), super::super::family(24)), ("pellet", "pellet"));
        assert!(super::super::ready(4) && super::super::ready(24));
        assert_eq!(super::super::max_kind(2, false), 4);
        assert!(weapon_units(11, "transform").iter().any(|u| u.ends_with("w_so031_wep")));
        assert!(weapon_units(11, "g2").iter().any(|u| u.ends_with("w_so034_wep")));
        assert!(weapon_units(19, "transform").iter().any(|u| u.ends_with("w_so141_Wep")));
        assert_eq!((S.text("pellet.window.group"), S.text("pellet.window.row")), ("SphereL", "SF_SHIELDSHOT_BLOW"));
        assert_eq!(S.num("pellet.shape.w_so031_wep.spherel.radius"), 1000.0);
        assert_eq!(1.0 / S.num("pellet.spawn.rate"), 0.016_666_668, "f32 [145D314C4h]");
        assert!(AnimId::from_name(S.text("pellet.clip_sequence")).is_some());
    }
}
