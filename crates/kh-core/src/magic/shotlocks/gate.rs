//! (SHOT-A gate) Warp Trick (kind 13 `MI_GATE`, Happy Gear's MAX kind; SHOT-B: kind 19 `BX_GATE` Zone Connector, Nano
//! Gear's MAX kind): update 0x1413B7B40 (TresPlayerState_ShootFlow vtable 0x14693CA58 +0x4E0) and its spawner 0x1413C02D0
//! (+0x4E8). Rows `gate.*` of sheet sora-shotlocks (tools/shotlocks/gate.js).
//!
//! What KH3 does (re/decomp/1413B/1413B7B40.c, re/decomp/1413C/1413C02D0.c, constants re-read in SHOT-B's listings
//! spec/scripts/gate/dis_1413B7B40.txt / dis_1413C02D0.txt; Begin 0x1413A5CD0 cases 0xD / 0x13: the delegate, the name,
//! group 0 / 0x645 (never read by this family), voice 0):
//! - sub 0: gravity off (pawn vt+0x1728(0)), Flying stopped (vt+0x17A0(5, 1)); the WEAPON CHANGE pawn vt+0x13C8(weapon
//!   type, 4, 0, ...) (kind 0x13: 8) to Twin Yo-Yos' w_so064 + w_so065 (Nano Arms' w_so172) and the form's anim set
//!   AnimSet4 (AnimSet8): no form is started; Play `M_4481` (AnimSet4: 4481, 0.8667 s; blend -1, mode 2, group 0 - a
//!   literal, not +0xC0), voice 0; face request (+0x84 bit 4); sub 4. No lift: he floats where he is.
//! - sub 3 / 4: at M_4481's end Play `M_4482` (0.8 s, mode 3: LOOPED), m_bEnableChange off, +0xCC bit 0 (the spawner
//!   on), sub 5, +0xA4 = 3.0 s, +0xA0 = 0.5 s.
//! - sub 5: +0xA4 -= dt; at 0: pawn vt+0x23C8() (0x1405A8610: EndEffect of the pawn's aura group 0xC62 / 0xC48; the core
//!   keeps no such effect: nothing to end), Play `M_4483` (0.6667 s, mode 2: its root motion drifts him ~150 cm back),
//!   m_bEnableChange off, sub 2, FUN_14140E500(ctrl, 0) = WorldSlow off (combat/form.rs `world_slow`).
//! - any other sub (2): m_bEnableChange and +0xB0 != 0: Begin's weapon back (vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1)):
//!   never here (M_4483 has no TresEnableChange), so End 0x1413ABA70 gives it back; at the clip's end Finish.
//! - the tail of every tick (after Finish too): while +0xCC bit 0, the spawner while +0x90 < N, else the bit off.
//! - THE SPAWNER 0x1413C02D0 ([`spawner`]): +0xA0 counts down first. The locked entry +0x110 (ctrl+0xB08) must exist and
//!   live (else +0x110 += 1, +0x114 = 0, nothing more this tick); weapon out (vt+0x1330: Begin drew it). Then up to 6 GATES
//!   per call, each: R = (pitch u24 x 60 / 2^24 - 60, yaw u24 x 360 / 2^24 - 180, roll 0) from the engine's xorshift
//!   (pitch first; the port: `PawnMagic::next_random`), off = R.RotateVector((300, 0, 0)), one `+0xB8` projectile
//!   (TresProjMI_Gate_C / TresProjBX_Gate_C) AT aim - off flying along off / |off| (aim = the entry's lock-on point,
//!   vt+0xB88), fired at once, no homing target, cmd 0xFD / the kind (vt+0x1100); +0x90 += 1 (N reached: stop); +0x114 +=
//!   1 (the entry's lock count reached: the next entry, stop). Then +0xA0 = 0.1 when the entry was finished (else 0) and
//!   +0xA4 = 0.5: sub 5 ends 0.5 s after the last gate. A failed spawn returns before those stores.
//! - THE GATE: a plain TresProjectileBase (magic/projectile.rs, rows `proj.mi_gate.*`): no particle of its own (its fire
//!   effect `vw_so064_gte_yoyo01` at the spawn point is the portal), 2000 cm/s straight (no homing), no root shape (it
//!   passes the level), a 100 cm SPHERE swept, no destroy on hit (each enemy once), its life ends after 600 cm (0.3 s,
//!   300 cm past the aim point). Its hits are the generic projectile path's: row SF_MIGATE (SF_BXGATE), power 0.9,
//!   VERTICALROLL, magic, guard break: the launch / juggle is the row's damage kind, on every ER enemy.
//!
//! "Warp Trick": Sora himself does not move (the update sets no velocity); the warping is the gates appearing round each
//! locked target. The weapon form is kept on the pawn (`PawnMagic::shot_weapon_form` 4 / 8): the hosts draw that form's
//! weapon parts (`gate.k<kind>.weapon_units`) instead of the Keyblade meanwhile.

use glam::Vec3;

use crate::anim::{AnimId, NotifyClass, NotifyData, NotifyEvent, NotifyPhase, PlayArgs};
use crate::host::{EventKind, TargetFlags, TargetId};
use crate::magic::projectile::{self, SpawnArgs};
use crate::magic::spells::cmd;
use crate::magic::tables::Sheet;
use crate::pawn::Ctx;
use crate::states::shoot_flow::ShootFlow;

const S: Sheet = Sheet::Shotlocks;

/// The family runs (`magic/shotlocks::ready`).
pub(crate) const PORTED: bool = true;

/// The family's fields of the state object: +0x110 / +0x114 (the locked entry the spawner serves and its gates so far),
/// +0xB0 != 0 (Begin's weapon form is to come back: `restore`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct State {
    entry: i32,
    per: i32,
    restore: bool,
}

/// The kind whose rows hold this kind's values: the code branches on 0x13 (Zone Connector) only; every other kind of the
/// update is Warp Trick's (13).
fn row_kind(kind: u8) -> u8 {
    if i32::from(kind) == S.int("gate.weapon_form.kind") {
        19
    } else {
        13
    }
}

/// The weapon form sub 0 puts on for `kind` (0x1413B7DD7: 8 for kind 0x13, else 4).
pub fn weapon_form(kind: u8) -> u8 {
    if i32::from(kind) == S.int("gate.weapon_form.kind") {
        S.int("gate.weapon_form.that_kind") as u8
    } else {
        S.int("gate.weapon_form.other") as u8
    }
}

/// The clip of a step (`charge` M_4481, `loop` M_4482, `end` M_4483) through the kind's weapon form's anim set.
fn clip(kind: u8, key: &str) -> AnimId {
    let fallback = match key {
        "charge" => AnimId::A4481,
        "loop" => AnimId::A4482,
        _ => AnimId::A4483,
    };
    AnimId::from_name(S.text(&format!("gate.k{}.clip.{key}.sequence", row_kind(kind)))).unwrap_or(fallback)
}

/// `Play(clip, slot 0, blend -1, mode, group 0, rate 1)`, then the voice index 0 (anim interface vt+0x1F0(0)) when the clip
/// has a length.
fn play(sf: &ShootFlow, cx: &mut Ctx, key: &str) {
    let clip = clip(sf.kind, key);
    cx.pawn.anim.play_with(clip, PlayArgs::mode(S.int(&format!("gate.clip.{key}.mode")) as u8, -1.0));
    if clip.data().length > 0.0 {
        cx.set_voice_group_no(0);
    }
}

/// After the common Begin and the per-kind set-up: nothing of its own.
pub(crate) fn begin(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

/// Begin's weapon form back (sub 2 / End: vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1), +0xB0 = 0).
fn give_weapon_back(st: &mut State, cx: &mut Ctx) {
    if std::mem::take(&mut st.restore) {
        cx.pawn.magic.shot_weapon_form = 0;
    }
}

/// The update delegate 0x1413B7B40; `false` = the state finished this tick (no PostTick).
pub(crate) fn update(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) -> bool {
    let mut finished = false;
    match sf.sub {
        0 => {
            let pawn = &mut *cx.pawn;
            pawn.cmc.gravity_off = true;
            pawn.cmc.stop_movement();
            pawn.cmc.set_flying();
            // vt+0x13C8(weapon type, form, 0, ...): the form's weapons and anim set (M_448x live only there).
            pawn.magic.shot_weapon_form = weapon_form(sf.kind);
            st.restore = true;
            play(sf, cx, "charge");
            sf.face_request = true;
            sf.sub = 4;
        }
        3 | 4 => {
            if cx.pawn.anim.ended {
                play(sf, cx, "loop");
                cx.pawn.queue.flags.enable_change = false;
                sf.ring_active = true;
                sf.sub = 5;
                sf.syphon_timer = S.num("gate.loop.timer");
                sf.ring_timer = S.num("gate.loop.first_wait");
            }
        }
        5 => {
            sf.syphon_timer -= dt;
            if sf.syphon_timer <= 0.0 {
                // pawn vt+0x23C8 (EndEffect of the aura group 0xC62 / 0xC48: the core keeps none), M_4483.
                play(sf, cx, "end");
                cx.pawn.queue.flags.enable_change = false;
                sf.sub = 2;
                // FUN_14140E500(ctrl, 0): WorldSlow off.
                crate::combat::form::world_slow(cx, false);
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
    // The tail of every tick (after Finish too): the spawner while flagsCC bit 0 and +0x90 < N, else the bit off.
    if sf.ring_active {
        if sf.spawn_index < sf.n {
            spawner(sf, st, cx, dt);
        } else {
            sf.ring_active = false;
        }
    }
    !finished
}

/// One draw of the stream as the spawner scales it: `u24 x span / 2^bits + min`.
fn draw(cx: &mut Ctx, span: &str, min: &str) -> f32 {
    let scale = S.num(span) / (1u32 << S.int("gate.spawn.random_bits")) as f32;
    cx.pawn.magic.next_random() as f32 * scale + S.num(min)
}

/// The spawner vt+0x4E8 = 0x1413C02D0(dt): up to `gate.spawn.per_call` gates round the locked entry +0x110.
fn spawner(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) {
    if sf.ring_timer > 0.0 {
        sf.ring_timer -= dt;
        if sf.ring_timer > 0.0 {
            return;
        }
        sf.ring_timer = 0.0;
    }
    // The locked entry (ctrl+0xB08): its component must still be there (the port: the host still reports the target
    // alive), else the next entry, nothing more this tick.
    let entry = usize::try_from(st.entry).ok().and_then(|i| cx.pawn.magic.aim.locked.get(i).copied());
    let Some((e, t)) = entry.and_then(|e| cx.target(e.target).filter(|t| t.flags.has(TargetFlags::ALIVE)).map(|t| (e, t))) else {
        st.entry += 1;
        st.per = 0;
        return;
    };
    // pawn vt+0x1330 (weapon out): Begin drew the weapon; nothing sheathes it in this state.
    let Some(def) = super::projectile_named(cx, sf.proj_name) else { return };
    let distance = S.num("gate.spawn.distance");
    let min_len_sq = S.text("gate.spawn.dir_min_len_sq").parse::<f32>().unwrap_or(0.0);
    for _ in 0..S.int("gate.spawn.per_call") {
        // The aim point (vt+0xB88), then R (pitch first), off = R.RotateVector((300, 0, 0)).
        let aim = t.center;
        let pitch = draw(cx, "gate.spawn.pitch_span", "gate.spawn.pitch_min").to_radians();
        let yaw = draw(cx, "gate.spawn.yaw_span", "gate.spawn.yaw_min").to_radians();
        let off = Vec3::new(pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin()) * distance;
        let direction = if off.length_squared() > min_len_sq { off.normalize() } else { off };
        // pawn vt+0x1450(+0xB8, aim - off, aim - off, dir, team, fire 1); vt+0x1100(cmd, kind).
        projectile::spawn(cx, &SpawnArgs { def, position: aim - off, direction, cmd: cmd::SHOOTFLOW, shoot_flow: sf.kind, finish: false, situation: false, full_mp: false, combo: [0; 4], target: TargetId::NONE, attached: None, fire: S.flag("gate.spawn.fire") });
        sf.spawn_index += 1;
        if sf.spawn_index >= sf.n {
            break;
        }
        st.per += 1;
        if st.per >= e.lock_count {
            st.entry += 1;
            st.per = 0;
            break;
        }
    }
    sf.ring_timer = if st.per == 0 { S.num("gate.spawn.pause") } else { 0.0 };
    sf.syphon_timer = S.num("gate.spawn.keep");
}

/// Every notify event of a pose tick: the clips' `TresAnimNotifyState_BpEvent` (M_4481 / M_4483, each clip from 0 to its
/// end) goes to the hosts as `AnimBpEvent`, as Infinity Circle's. The clips have no TresShot / TresAtk / Timing (the
/// shared handlers do nothing for these kinds).
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

/// Before the common End's release of the aim mode: End 0x1413ABA70 gives Begin's weapon form back (+0xB0 != 0).
pub(crate) fn end(_sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx) {
    give_weapon_back(st, cx);
}

/// The Shot handler's spawn point (none: the clips have no TresShot; the spawner places its gates itself).
pub(crate) fn muzzle(_sf: &ShootFlow, _st: &State, _cx: &Ctx, _index: usize) -> Option<Vec3> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_kh3_s() {
        assert_eq!(S.text("gate.update"), "0x1413B7B40");
        for kind in [13, 19] {
            assert_eq!(super::super::family(kind), "gate");
            assert!(super::super::ready(kind));
        }
        // Happy Gear (weapon type 6): base Snakebite 12, MAX Warp Trick 13.
        assert_eq!((super::super::base_kind(6), super::super::max_kind(6, false)), (12, 13));
        assert_eq!((weapon_form(13), weapon_form(19)), (4, 8));
        // The parts the hosts draw meanwhile (sora-keyblade-forms form.YO / form.GM units).
        let units = |kind: u8| match S.find(&format!("gate.k{}.weapon_units", row_kind(kind))) {
            Some(crate::magic::tables::PV::L(units)) => units.to_vec(),
            _ => Vec::new(),
        };
        assert!(units(13).iter().any(|u| u.ends_with("w_so064_Wep")) && units(13).iter().any(|u| u.ends_with("w_so065_Wep")));
        assert!(units(19).iter().any(|u| u.ends_with("w_so172_Wep")));
        assert_eq!((clip(13, "charge"), clip(13, "loop"), clip(13, "end")), (AnimId::A4481, AnimId::A4482, AnimId::A4483));
        assert_eq!((clip(19, "charge"), clip(19, "loop"), clip(19, "end")), (AnimId::A8481, AnimId::A8482, AnimId::A8483));
        assert_eq!((S.int("gate.clip.charge.mode"), S.int("gate.clip.loop.mode"), S.int("gate.clip.end.mode")), (2, 3, 2));
        assert_eq!((S.num("gate.loop.timer"), S.num("gate.loop.first_wait"), S.num("gate.spawn.keep"), S.num("gate.spawn.pause")), (3.0, 0.5, 0.5, 0.1));
        assert_eq!((S.int("gate.spawn.per_call"), S.num("gate.spawn.distance")), (6, 300.0));
        // The f32 scales of the disassembly: 60 / 2^24 and 360 / 2^24.
        assert_eq!(S.num("gate.spawn.pitch_span") / (1u32 << S.int("gate.spawn.random_bits")) as f32, 3.576_278_7e-6);
        assert_eq!(S.num("gate.spawn.yaw_span") / (1u32 << S.int("gate.spawn.random_bits")) as f32, 2.145_767_2e-5);
        assert_eq!((S.text("gate.k13.proj_class"), S.text("gate.k13.row"), S.text("gate.k19.row")), ("TresProjMI_Gate_C", "SF_MIGATE", "SF_BXGATE"));
        assert_eq!(S.text("gate.spawn.dir_min_len_sq").parse::<f32>(), Ok(1.0e-8));
    }
}
