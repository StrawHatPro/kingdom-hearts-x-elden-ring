//! (AUDIT-C) THE STATE-EVENT CAMERA BLUEPRINTS' NOTIFY EVENTS OF THE FORM CLIPS. KH3 sends every
//! `TresAnimNotifyState_BpEvent` window of Sora's clips to every state-event camera blueprint the pawn holds
//! (`ReceiveAnimNotifyStartBpEvent` / `ReceiveAnimNotifyEndBpEvent(SendActor, AnimSeqName, EventID, Param)`); each
//! blueprint compares `AnimSeqName` itself. The formchange / finisher blueprints read 2026-11-03
//! (build/sora_tmp/weapons/AUDIT-C/CAM/bp/*.txt, kh-camera `form_bp.rs`):
//! - `BP_TresPlayerStateEvent_form_change`: 1401 / 1403 (the StyleChange clips of Second Form, Light / Dark Form),
//!   1413 1416 1421 1422 1426 1433 (Second Form's finishers), D401 (StyleChange13, Rage Form), D417 (Crimson Blast),
//!   D418 (FINISH_AN), end of 1403 / 1073.
//! - `BP_TresPlayerStateEventCamera_AltemaWep`: 9401 (StyleChange9), 9418 (COMBO_SW's last finisher), 9419 (FINISH_SW).
//! - `BP_TresPlayerStateEventCamera_Oath_keeper`: N424 / N425 (FINISH_TF / FINISH_TF2), end of N425.
//!
//! The FR states (friend.rs) and the shotlocks (magic/shotlocks) send their own clips' events; this sends the others
//! for every other clip, with the end of every window it opened. Since 2026-11-05 (the coordinator's decision) this
//! includes the Second Form family (0x94 ..= 0x98) and the formless clips (0002 / 1050 / 0107 / 1176 of
//! BP_TresPlayerStateEvent_motion_notify), as KH3 sends them; tests/golden/second_form_sessions.trace was re-recorded on
//! purpose for these events (P3/STATE.md, AUDIT-C/AUDIT.md).
//!
//! The Ultimate Finish's Blue Hour (Oath_keeper @2377 .. @1750, N425's start) spawns `vw_so220_fin_fsg0` at Sora's actor
//! location, rotation 0 (`SqExSpawnEmitterAtLocationKeywordSpawn(pawn, "vw_so220_fin_fsg0", 0, location, R0, true)`); the
//! end of N425 (@2439 .. @2522) `FadeOut(0)`s it: both are done here, at the same notify events.

use crate::anim::{Notifies, NotifyClass, NotifyData, NotifyPhase};
use crate::host::{Attach, EffectHandle, EffectId, EffectSpawn, EventKind};
use crate::pawn::Ctx;
use crate::states::{StateData, StateId};
use glam::Vec3;

/// Oath_keeper @676: the Blue Hour particle of N425's start.
pub const BLUE_HOUR_PARTICLE: &str = "vw_so220_fin_fsg0";

/// What the sender keeps between frames (`PawnCombat::bp_events`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct BpEvents {
    /// The windows whose start was sent and whose end was not yet.
    pub open: Vec<&'static str>,
    /// The Blue Hour particle while N425's window is open.
    pub blue_hour: Option<EffectHandle>,
    /// (AUDIT-B W-FIN) BP_TresPlayerStateEventCamera_fz_key's FZ_FINISH (@8092, 7419's start; false @7232 / @18641) and
    /// ATARI (@3311: its 7419 line trace hit a ceiling; false @7221), and the ceiling look's particle `vw_so073_fin_fsg0`
    /// (@7411) while it is on.
    pub fz_finish: bool,
    pub atari: bool,
    pub ceiling: Option<EffectHandle>,
}

/// (AUDIT-B W-FIN) fz_key @7411: FINISH_SK's ceiling-look particle (an opaque, unlit, two-sided sphere omd_sph_32x32_00 x 300
/// m, smt_so070_fin_fsg_00o, with its sparkles): what is left of the world once the level is hidden.
pub const CEILING_PARTICLE: &str = "vw_so073_fin_fsg0";

/// (AUDIT-B W-FIN) fz_key @3089 / @3139 (= kh-camera forms_claws `TRACE_7419`): from his actor location + (0, 0, 100) to
/// `TransformLocation(his transform, this)`; a hit = ATARI (a ceiling over him).
pub const CEILING_TRACE: Vec3 = Vec3::new(-137.773_83, -72.918_39, 956.998_05);

/// States whose own code sends their clips' BpEvents (friend.rs `on_notifies`, magic/shotlocks `on_notify`).
fn sends_its_own(id: StateId) -> bool {
    matches!(id, StateId::FrShieldGuardian | StateId::FrGoofyshoot | StateId::FrCallmeteor | StateId::FrFlareforce | StateId::Shootflow)
}

/// After the running state's notifies: the BpEvent windows that began or ended this frame, as `AnimBpEvent`.
pub(crate) fn emit(state: &StateData, cx: &mut Ctx, notifies: &Notifies) {
    // (AUDIT-B W-FIN) fz_key ReceivePlayerDtorState @11613 (STYLEFINISH 33 / STYLEFINISH_SK 35 ...) -> @18641: FZ_FINISH false,
    // @9622 .. @9584 the particle FadeOut(0) (ATARI is left as it is, as KH3 does)
    if state.id() != StateId::Stylefinish && (cx.pawn.combat.bp_events.fz_finish || cx.pawn.combat.bp_events.ceiling.is_some()) {
        cx.pawn.combat.bp_events.fz_finish = false;
        if let Some(h) = cx.pawn.combat.bp_events.ceiling.take() {
            cx.stop_effect(h, true);
        }
    }
    if sends_its_own(state.id()) {
        return;
    }
    // (AUDIT-C, 2026-11-05, the coordinator's decision) every clip's windows, as KH3 sends them: also the Second Form
    // family's and the formless clips' (the golden trace was re-recorded for these events; see the module note).
    for e in notifies.events.iter().filter(|e| e.class() == NotifyClass::TresAnimNotifyStateBpEvent) {
        let param = match e.def().data {
            NotifyData::Param { param } => param.unwrap_or(0),
            _ => 0,
        };
        let seq = e.sequence_name();
        let begin = match e.phase {
            NotifyPhase::Begin => true,
            NotifyPhase::End => match cx.pawn.combat.bp_events.open.iter().position(|s| *s == seq) {
                Some(i) => {
                    cx.pawn.combat.bp_events.open.remove(i);
                    false
                }
                None => continue,
            },
            _ => continue,
        };
        if begin {
            cx.pawn.combat.bp_events.open.push(seq);
        }
        cx.pawn.event(EventKind::AnimBpEvent { seq, param, begin });
        if seq == "N425" {
            blue_hour_particle(cx, begin);
        }
        if matches!(seq, "7419" | "7423") {
            ceiling_look(cx, seq, param, begin); // (AUDIT-B W-FIN)
        }
    }
}

/// (AUDIT-B W-FIN) FINISH_SK's CEILING LOOK, the particle half (BP_TresPlayerStateEventCamera_fz_key; the camera, level and
/// light halves are kh-camera forms_claws `ceiling` / FormBpOutput `tower` and kh-er-render bluehour.rs `set_tower`):
/// - 7419's start (fz_Finish01 @8946 -> @2399 .. @3311): FZ_FINISH; the line trace from his actor location + 100 up to
///   `TransformLocation(his transform, CEILING_TRACE)` (957 cm over his head): a hit = ATARI.
/// - 7423's start with Param != 0 (Fz_FInish01_End @10761), FZ_FINISH and ATARI: @9451 the level hidden, @7411
///   `SpawnEmitterAtLocation(vw_so073_fin_fsg0, his actor location, R(0, 0, 0), bAutoDestroy)`.
/// - 7423's end (ReceiveAnimNotifyEndBpEvent @10603 -> Fz_FInish02_End @10776 -> @10151), ATARI: @9688 `FadeOut(0)` (gone
///   at once: SQEX FadeOut with t <= 0), ... @7221 ATARI / @7232 FZ_FINISH false.
fn ceiling_look(cx: &mut Ctx, seq: &str, param: i32, begin: bool) {
    match (seq, begin) {
        ("7419", true) => {
            let (at, yaw) = (cx.pawn.cmc.location, cx.pawn.yaw);
            let (s, c) = yaw.to_radians().sin_cos();
            let l = CEILING_TRACE;
            let to = at + Vec3::new(l.x * c - l.y * s, l.x * s + l.y * c, l.z);
            let hit = cx.world.line_trace(at + Vec3::Z * 100.0, to).is_some();
            let ev = &mut cx.pawn.combat.bp_events;
            ev.fz_finish = true;
            ev.atari = hit;
        }
        ("7423", true) => {
            let ev = &cx.pawn.combat.bp_events;
            if param != 0 && ev.fz_finish && ev.atari && ev.ceiling.is_none() {
                let at = cx.pawn.cmc.location;
                let handle = cx.spawn_effect(EffectSpawn { id: EffectId(crate::notify::notify_asset_id(CEILING_PARTICLE)), name: CEILING_PARTICLE, at: Attach::World(at), rotation: Vec3::ZERO, scale: 1.0, persistent: true });
                cx.pawn.combat.bp_events.ceiling = Some(handle);
            }
        }
        ("7423", false) if cx.pawn.combat.bp_events.atari => {
            if let Some(h) = cx.pawn.combat.bp_events.ceiling.take() {
                cx.stop_effect(h, true);
            }
            let ev = &mut cx.pawn.combat.bp_events;
            (ev.atari, ev.fz_finish) = (false, false);
        }
        _ => {}
    }
}

/// Oath_keeper @583 .. @676 (start) / @2522 (end): `vw_so220_fin_fsg0` at the actor location, `FadeOut(0)` at the end.
fn blue_hour_particle(cx: &mut Ctx, begin: bool) {
    if let Some(old) = cx.pawn.combat.bp_events.blue_hour.take() {
        cx.stop_effect(old, true);
    }
    if begin {
        let at = cx.pawn.cmc.location;
        let handle = cx.spawn_effect(EffectSpawn { id: EffectId(crate::notify::notify_asset_id(BLUE_HOUR_PARTICLE)), name: BLUE_HOUR_PARTICLE, at: Attach::World(at), rotation: Vec3::ZERO, scale: 1.0, persistent: true });
        cx.pawn.combat.bp_events.blue_hour = Some(handle);
    }
}
