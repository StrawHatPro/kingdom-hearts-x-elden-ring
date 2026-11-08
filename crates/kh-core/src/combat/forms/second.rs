//! (weapons F-SECOND) SECOND FORM S / D: Second Form's variants on Starlight (`form.L2`, CHANGE_L2 0x95) and Dead of Night
//! (`form.L5`, CHANGE_L5 0x98). KH3 runs both with Second Form's own code: AwardAtMax 0x1413F0ED0, StartStyle 0x14141A9D0,
//! StyleChange Begin 0x1413CBA70 / Tick 0x1413D3AC0, SetStyleFinishCommand 0x1413EE030 and EndStyle 0x14141CA20 each have ONE
//! case 0x94 .. 0x98 (decompiled C). They are the framework's own moveset (`forms::SECOND_FORM`, form.rs / style.rs); what
//! differs is data (sheets/port/sora-form-second.json `diff.*`, tools/sora_form_second.js):
//! - the weapon type (StartStyle's weapon change 0x12 / 0x13, AwardAtMax's offer by type 18 / 19): `form::take_form_keyblade`;
//! - the weapon (w_so150_Wep / w_so121_Wep: the Keyblade's own, `Form::weapon_units`) and its shotlock set (w_so150_ProjSet /
//!   w_so120_ProjSet: the Keyblade's own, the SHOT packages by weapon type);
//! - the HUD's form data (ui asset L2, act logo K22: the logo movies are not in the pack, kh-ui hud_command.rs);
//! - Second Form S only: finish level 3 is `LM_UNIONDISCODE` (Ancient Light, 0xAE: SetFinishLevel 0x141414C70 "0xAE in form
//!   0x95"), its STYLEFINISH body slot 155 0x1413E2810 (states/style_finish.rs `union_discode`, the sheet's `finish.L2.*`), and
//!   the hit shapes its clip M_1435 opens on the Starlight weapon's TresAtkColl0 (`shape.L2.*`, [`hit_shape`]).

use glam::Vec3;

use crate::anim::{NotifyData, SEQUENCES};
use crate::pawn::Ctx;

// The rows (sheets/port/sora-form-second.json): `FormSecondRow`, `FORM_SECOND`, `FSE_*`.
include!(concat!(env!("KH_GEN_DIR"), "/sora_form_second.rs"));

/// `LM_UNIONDISCODE` (ETresCommandKind 174): Second Form S's finish level 3, "Ancient Light".
pub const LM_UNIONDISCODE: i32 = 0xAE;

fn row(id: &str) -> Option<&'static FormSecondRow> {
    FORM_SECOND.iter().find(|r| r.id == id)
}

fn value(id: &str, k: usize) -> f32 {
    row(id).and_then(|r| r.values.get(k).copied()).unwrap_or(0.0)
}

/// Slot 155's numbers (the sheet's `finish.L2.*`): slot 164's height h (500), the probe's extra range (300: the probe
/// reaches h + 300), slot 165's rate (500), the hit windows the world slow waits for (+0x12C > 4).
pub(crate) fn rise_height() -> f32 {
    value("finish.L2.0", 0)
}
pub(crate) fn probe_extra() -> f32 {
    value("finish.L2.0", 1)
}
pub(crate) fn rise_rate() -> f32 {
    value("finish.L2.3", 0)
}
pub(crate) fn hits_before_slow_end() -> i32 {
    value("finish.L2.1", 0) as i32
}

/// The shape row of a group of the Starlight weapon that the Kingdom Key's component lacks (L2_Pillar1 .. 5, L2_Dark).
fn shape_row(group: &str) -> Option<&'static FormSecondRow> {
    FORM_SECOND.iter().find(|r| r.what == "shape" && r.logical == group)
}

/// (combat/hit.rs) `m_bDisableLocation1Attach` of the held form's weapon group: L2_Dark stays where it began.
pub(crate) fn fixed_group(cmd: i32, group: &str) -> bool {
    cmd == super::super::form::CHANGE_L2 && shape_row(group).is_some_and(|r| r.flags.get(5) == Some(&"frozen"))
}

/// Time since the hit window of `group` began (the clip's TresAnimNotifyState_Attack at or before `time`).
fn window_time(sequence: usize, group: &str, time: f32) -> f32 {
    let Some(seq) = SEQUENCES.get(sequence) else { return 0.0 };
    seq.notifies
        .iter()
        .filter(|n| matches!(n.data, NotifyData::Attack { group: g, .. } if g == group) && n.time <= time + 1.0e-4)
        .map(|n| time - n.time)
        .fold(f32::INFINITY, f32::min)
        .clamp(0.0, 10.0)
}

/// THE PACKAGE'S `hit_shape` (Second Form's moveset; combat/hit.rs asks the held form first): in Second Form S, a window of
/// a Starlight weapon group (its TresAtkColl0: OWNER_ATTACHED_MESH on the wielder's `vfx_cure`, the relative location in the
/// socket frame = actor space) as (end 1, end 2, radius) in the world. CAPSULE1 (rotation NORMAL, the socket upright): an
/// upright segment of half height `m_Size.Z`; SPHERE: its centre. The radius grows by `m_IncSize.X` a second of the window
/// up to `m_IncMaxSize.X` (0x140649E40). None = not such a group, or another form.
pub(crate) fn hit_shape(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<(Vec3, Vec3, f32)> {
    if cx.pawn.combat.form.cmd != super::super::form::CHANGE_L2 {
        return None;
    }
    let r = shape_row(group)?;
    let v = |k: usize| r.values.get(k).copied().unwrap_or(0.0);
    let since = window_time(sequence, group, time);
    let radius = if v(6) > 0.0 { (v(0) + v(6) * since).min(v(7).max(v(0))) } else { v(0) };
    let socket = Vec3::from_slice(row("socket.vfx_cure").map_or(&[0.0, 0.0, 0.0][..], |s| s.values));
    let local = socket + Vec3::new(v(3), v(4), v(5));
    let pawn = &*cx.pawn;
    let (sin, cos) = pawn.yaw.to_radians().sin_cos();
    let p = pawn.cmc.location + Vec3::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos, local.z);
    match r.flags.first().copied() {
        Some("CAPSULE1") => Some((p - Vec3::Z * v(2), p + Vec3::Z * v(2), radius)),
        Some("SPHERE") => Some((p, p, radius)),
        _ => None,
    }
}

/// (XS Ancient Light, abilities/xs/ancient.rs) THE GAUGE'S SECOND FORM OFFER: the formchange command of the Second Form
/// family (CHANGE_L1 .. CHANGE_L5, 0x94 ..= 0x98) standing in the situation queue. AwardAtMax 0x1413F0ED0 offers the held
/// Keyblade's own variant (weapon type 1 -> CHANGE_L1, 18 -> CHANGE_L2, 19 -> CHANGE_L5; `form::offered`): the one beside
/// which Ancient Light is offered and which it spends.
pub(crate) fn gauge_offer(cx: &Ctx) -> Option<i32> {
    (0x94..=0x98).find(|&cmd| cx.pawn.combat.situation.has(cmd))
}

#[cfg(test)]
mod tests {
    use super::*;

    // The sheet holds the six Starlight groups, the five pillars of M_1435, the slot-155 numbers, and the rows' differences.
    #[test]
    fn the_sheet_has_what_slot_155_reads() {
        assert_eq!(FORM_SECOND.iter().filter(|r| r.what == "shape").count(), 6);
        assert_eq!((rise_height(), probe_extra(), rise_rate(), hits_before_slow_end()), (500.0, 300.0, 500.0, 4));
        let windows = row("finish.L2.0").expect("sub 0").windows;
        assert_eq!(windows.iter().filter(|w| w.contains("@L2_Pillar")).count(), 5);
        assert!(windows.iter().any(|w| w.starts_with("4.6333-5.0333@ImpactLL@LM_UNIONDISCODE4")), "{windows:?}");
        assert!(fixed_group(0x95, "L2_Dark") && !fixed_group(0x95, "L2_Pillar1") && !fixed_group(0x94, "L2_Dark"));
        assert!(row("diff.L2").is_some_and(|r| r.flags.contains(&"finish_levels")));
        assert!(row("diff.L5").is_some_and(|r| !r.flags.contains(&"finish_levels") && r.flags.contains(&"units")));
    }
}
