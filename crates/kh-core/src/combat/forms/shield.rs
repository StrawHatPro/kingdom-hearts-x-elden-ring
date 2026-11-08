//! (weapons F-SHIELD) Counter Shield (`form.SH`, CHANGE_SH 0x89, Hero's Origin) and Frying Pan (`form.SH2`, CHANGE_SH2
//! 0x91, Grand Chef): their form package (documentation/KH3_WEAPONS_PLAN.md 1.3 / 4; build/sora_tmp/weapons/F-SHIELD/
//! STATE.md). Both are first-stage forms of 30 s with FINISH_SH (0xA3), three grow levels of the weapon and one attack
//! class, COMBO_SH (id 84, `states/combo_sh.rs`). The data is `sheets/port/sora-form-shield.json`
//! (tools/sora_form_shield.js): every number below that is not an address comes from it.
//!
//! WHAT KH3 DOES (decompiled C in re/decomp; addresses in the sheet and the files):
//! - THE GROW LEVEL (P3's open question): the shield guard (TresPlayerState_Guard slot 118 0x14139D2D0, the tick of weapon
//!   forms 0x0B / 0x13) plays `GuardChange` on a SUCCESSFUL guard hit at LV1 and `GuardChange2` at LV2, each with GrowUp
//!   (pawn slot 638 0x1405BD810: ChangeWeapon(grow + 1), never past LV3). ComboSH's Decide 0x1413632A0 then gives the
//!   grown attack (LV2: 0x55 `M_B418`, LV3: 0x56 `M_B426`: invincible, the world slowed, the combo's end) instead of the
//!   combo, and at its `TresEnableChange` the weapon goes back to LV1 (pawn slot 637 0x1405BD770 (0, 0, 0)). `Form::grow`
//!   is the level (1 ..= 3).
//! - THE START SECTION (P3's other open question): StyleChange Begin 0x1413CBA70 case 0x89 sets +0xA4 = 2, the argument of
//!   anim interface vt+0x1F0 after the transformation clip starts = SetVoiceGroupNo(2): `on_start` below.
//! - THE COMBO: `decide`, the kinds 0x50 .. 0x57 (PlayAttack 0x1413378B0, the rows `attack.<form>.<kind>`), MaxCombo 5.
//!   Frying Pan plays the same clips with the effect groups 0x867 .. 0x86B (its fire effects instead of the lightning) and
//!   its attack rows SH2_* for SCSH_* (0x141357590: `row_rename`).
//! - FINISH_SH: TresPlayerState_StyleFinishSH (id 38): Pegasus and the Chariot (Ultimate Finish) / the Frying Pan;
//!   `states/combo_sh.rs`.
//!
//! Not in the core (hosts / INT; STATE.md): the replacement anim set for the states that do not play through the form
//! (locomotion, guard: `replaced_clip`), the form weapon's hit shapes (`group_shape`), the row rename at the hit
//! (`row_rename`), the guard's grow (`guard_grow`), the style pawns' drawing (`finisher_pawns`), the Attack command label
//! (`attack_command`).

use super::FormMoveset;
use crate::anim::AnimId;
use crate::combat::form::{Form, KeybladeFormRow};
use crate::host::TargetId;
use crate::pawn::Ctx;
use crate::states::FinishView;
use crate::states::StateData;
use glam::Vec3;

// The rows (sheets/port/sora-form-shield.json): `FormShieldRow`, `FORM_SHIELD`, `FSH_*`.
include!(concat!(env!("KH_GEN_DIR"), "/sora_form_shield.rs"));

/// CHANGE_SH: Counter Shield.
pub const CHANGE_SH: i32 = 0x89;
/// CHANGE_SH2: Frying Pan.
pub const CHANGE_SH2: i32 = 0x91;
/// FINISH_SH (both forms: SetStyleFinishCommand 0x1413EE030).
pub const FINISH_SH: i32 = 0xA3;
/// The finish extra of the ride: FINISH extra: shield thunder (TresCmdData 0xAA `shex_thunder`).
pub const FINISH_EXTRA_THUNDER: i32 = 0xAA;

/// The package: both forms, their finish command (STYLEFINISH 0xA3 body: `states/combo_sh.rs`).
pub(crate) const MOVESET: FormMoveset = FormMoveset {
    name: "shield",
    forms: &["form.SH", "form.SH2"],
    finishes: &[FINISH_SH],
    on_start,
    attack,
    finish_begin: crate::states::combo_sh::finish_begin,
    finish_tick: crate::states::combo_sh::finish_tick,
    replace_clip,
    hit_shape,
    hit_volumes, // (F-SHAPES)
    row_rename: rename,
    // (F-LOCO) `replace_all_plays: true` gone: every play of every form resolves through its anim sets (forms `resolve`).
    ..FormMoveset::NONE
};

/// The sheet's form key of a command: "SH" / "SH2".
fn form_key(cmd: i32) -> Option<&'static str> {
    match cmd {
        CHANGE_SH => Some("SH"),
        CHANGE_SH2 => Some("SH2"),
        _ => None,
    }
}

/// A shield form (Counter Shield or Frying Pan).
pub fn is_shield(cmd: i32) -> bool {
    form_key(cmd).is_some()
}

fn rows(what: &'static str, form: &'static str) -> impl Iterator<Item = &'static FormShieldRow> {
    FORM_SHIELD.iter().filter(move |r| r.what == what && r.form == form)
}

/// The row of an attack kind in a shield form (`attack.<form>.<kind>`).
pub fn attack_row(cmd: i32, kind: u8) -> Option<&'static FormShieldRow> {
    let form = form_key(cmd)?;
    rows("attack", form).find(|r| r.code == i32::from(kind))
}

/// m_ReplaceAnimSets: the clip (animation-table row) a logical name plays as in the form (Counter Shield: AnimSet11;
/// Frying Pan: AnimSet11 + AnimSet19); None = the base set's row. For the pose hosts too (locomotion, guard: STATE.md).
pub fn replaced_clip(cmd: i32, logical: &str) -> Option<&'static str> {
    form_key(cmd)?;
    super::replaced_clip(cmd, logical) // (F-LOCO) the one contract: the form's anim set chain (its blend spaces too)
}

/// The clip a logical name plays as in the held form, else the name itself.
pub(crate) fn clip(cmd: i32, logical: &str) -> &str {
    replaced_clip(cmd, logical).unwrap_or(logical)
}

/// The form package's `replace_clip` (the framework's own plays: the finish bodies). One moveset holds both forms and the
/// hook gets no command: Frying Pan's map is Counter Shield's plus AnimSet19 (`M_J423`, never played in Counter Shield),
/// the one name they share with different clips (`StyleChangeEasy`) is not played by the framework for them.
fn replace_clip(logical: &str) -> Option<&'static str> {
    replaced_clip(CHANGE_SH2, logical)
}

/// The kinds that spend a grow level (0x55 LV2, 0x56 LV3).
pub(crate) fn is_grown_kind(kind: u8) -> bool {
    matches!(kind, 0x55 | 0x56)
}

/// pawn slot 637 0x1405BD770 with (0, 0, 0): ChangeWeapon(the held type and form, grow LV1).
pub(crate) fn weapon_reset(cx: &mut Ctx) {
    if is_shield(cx.pawn.combat.form.cmd) {
        cx.pawn.combat.form.grow = 1;
    }
}

/// The style pawn's m_Param_MaxTime (the ride / the pan), s (sheet `pawn.<form>.*` values[0]: a GUESS, STATE.md).
pub(crate) fn pawn_values(form: &'static str) -> &'static [f32] {
    rows("pawn", form).next().map_or(&[][..], |r| r.values)
}

/// (F-SHIELD) The shield guard's numbers (the row `guard.params`): [MyGuardMoveSpeed, the walk stop delay, the turn rate toward
/// the locked target, toward the stick, the descent's start height, its landing height, the ground probe range].
pub(crate) fn guard_params() -> [f32; 7] {
    let v = FORM_SHIELD.iter().find(|r| r.what == "guard").map_or(&[][..], |r| r.values);
    std::array::from_fn(|k| v.get(k).copied().unwrap_or(0.0))
}

/// The hit window of the Frying Pan pawn's own clip (`pawn.SH2.*` windows: "M_J423:a-b"), s.
pub(crate) fn pan_window() -> Option<(f32, f32)> {
    let w = rows("pawn", "SH2").next()?.windows.first()?;
    let (_, span) = w.split_once(':')?;
    let (a, b) = span.split_once('-')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

// ------------------------------------------------------------------------------------------------ the hooks

/// StartStyle's per-form tail: nothing for 0x89 / 0x91 in StartStyle itself; the transformation's voice group 2 of
/// Counter Shield (StyleChange Begin 0x1413CBA70: +0xA4 = 2 -> SetVoiceGroupNo after `StyleChange11` starts; the
/// transformation follows at once, so the number is set here).
fn on_start(_cx: &mut Ctx, _row: &'static KeybladeFormRow) {
    // (AUDIT-A SHIELD) nothing: StartStyle has no 0x89 / 0x91 case. The voice group 2 was set here before, and the
    // StyleChange state's begin (states/host.rs: SetVoiceGroupNo(0) per state begin) overwrote it before StyleChange11's
    // voice notify fired (silent: sora_atk4 has no unit 0). It is set where KH3 sets it: `style_change_voice_group`.
}

/// (AUDIT-A SHIELD) StyleChange's +0xA4 (Begin 0x1413CBA70: `case 0x89: +0xA4 = 2`, the sheet's `voice.SH.StyleChange11`),
/// handed to SetVoiceGroupNo by Tick sub 0 (0x1413D3AC0) right after the transformation clip starts; None = 0 (every other
/// form: Reset leaves +0xA4 at 0).
pub fn style_change_voice_group(cmd: i32) -> Option<i32> {
    if cmd != CHANGE_SH {
        return None;
    }
    FORM_SHIELD.iter().find(|r| r.what == "voice" && r.form == "SH").and_then(|r| r.values.first()).map(|v| *v as i32)
}

/// The attack request in the forms: COMBO_SH (pawn slot 1337 0x141432F00: 0x89 / 0x91 -> 0x54).
fn attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    crate::states::combo_sh::try_attack(state, cx, target)
}

// ------------------------------------------------------------------------------------------------ the decision

/// What ComboSH's Decide reads (state+0x128 the combo, +0x154 the reach, +0x1AC the previous kind = pawn+0xE10, +0x1CC /
/// +0x1D0 the distance / height to the target, +0x1E0 target high, +0x142 the air (+0x1EC bit 3 makes it 1), the grow
/// (equipment vt+0x448), pawn+0xE14 the finishers played).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShieldDecideIn {
    pub combo: i32,
    pub reach: u8,
    pub last_kind: u8,
    pub dist: f32,
    pub dz: f32,
    pub target_high: bool,
    pub air: bool,
    /// `Form::grow` (1 ..= 3).
    pub grow: u8,
    pub finish_count: i32,
}

/// What it leaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShieldDecision {
    pub kind: u8,
    pub reach: u8,
    pub finish: bool,
    pub finish_end: bool,
    pub finish_count: i32,
    /// combo 0: +0x1AC = 0 and pawn vt+0x1D40(0) (pawn+0xE10 = 0).
    pub last_reset: bool,
}

/// `TresPlayerState_ComboSH::Decide` (0x1413632A0, decompiled C):
///
/// ```text
/// PreDecide; if combo == 0: reach = dist > 400 ? 1 : (dz > 400 ? 2 : reach); last = 0; pawn+0xE10 = 0
/// max = 3, finishers = 5; if +0x1EC & 8: air = 1
/// grow LV2: 0x55; grow LV3: 0x56                                     (the grown attacks: PlayAttack sets the finish flags)
/// combo < 3:  last 0x50 / 0x57 -> 0x51 + (reach != 0);  0x51 -> reach ? 0x50 : 0x52;  0x52 -> 0x50 + (reach != 0)
///             else: combo != 0 -> 0x51 + (reach != 0); combo 0: reach 1 -> 0x52, reach 2 -> 0x51 (both kept as they are), else 0x50
///             0x50 with air or the target high -> 0x57
/// else:       finish; n = pawn+0xE14; n < 1 && combo < 4 -> 0x53; else pawn finish end, finish end, 0x54; pawn+0xE14 = n + 1
/// ```
pub fn decide(i: &ShieldDecideIn) -> ShieldDecision {
    let mut d = ShieldDecision { kind: 0, reach: i.reach, finish: false, finish_end: false, finish_count: i.finish_count, last_reset: false };
    let mut last = i.last_kind;
    if i.combo == 0 {
        if i.dist > 400.0 {
            d.reach = 1;
        } else if i.dz > 400.0 {
            d.reach = 2;
        }
        last = 0;
        d.last_reset = true;
    }
    match i.grow {
        2 => {
            d.kind = 0x55;
            return d;
        }
        3 => {
            d.kind = 0x56;
            return d;
        }
        _ => {}
    }
    let reach = d.reach;
    let step = |base: u8| base + u8::from(reach != 0);
    if i.combo < 3 {
        let kind = match last {
            0x50 | 0x57 => step(0x51),
            0x51 => if reach == 0 { 0x52 } else { 0x50 },
            0x52 => step(0x50),
            _ if i.combo != 0 => step(0x51),
            _ => match reach {
                1 => {
                    d.kind = 0x52;
                    return d;
                }
                2 => {
                    d.kind = 0x51;
                    return d;
                }
                _ => 0x50,
            },
        };
        d.kind = if kind == 0x50 && (i.air || i.target_high) { 0x57 } else { kind };
        return d;
    }
    d.finish = true;
    let n = i.finish_count;
    d.kind = if n < 1 && i.combo < crate::states::combo_sh::MAX_COMBO - 1 {
        0x53
    } else {
        d.finish_end = true;
        0x54
    };
    d.finish_count = n + 1;
    d
}

// ------------------------------------------------------------------------------------------------ for hosts and INT

/// The guard's grow (Guard slot 118 0x14139D2D0, sub 0 of a hit entry; INT line in states/guard.rs, STATE.md): a
/// SUCCESSFUL guard hit in a shield form at LV1 / LV2 grows the weapon (GrowUp 0x1405BD810) and returns the clip that
/// replaces `GuardHit` / `AirGuardHit` (`GuardChange` / `GuardChange2` of the form's set, air variants); None = the
/// usual clip (not a shield form, or LV3).
pub fn guard_grow(held: &mut Form, air: bool) -> Option<AnimId> {
    let form = form_key(held.cmd)?;
    let level = held.grow;
    let row = rows("grow", form).find(|r| r.code == i32::from(level))?;
    if level >= 3 {
        return None;
    }
    held.grow = level + 1;
    let name = if air { row.flags.get(1).copied().unwrap_or(row.clip) } else { row.clip };
    AnimId::from_name(name)
}

/// The Attack command's label. (AUDIT-A SHIELD, F-HUD's trace; was INFERRED from the grow level, which KH3 does not use):
/// KH3 renames the Attack slot through the GUARD-COUNTER WINDOW: FUN_1414145C0 (actions.rs `form_counter`) -> 0x14158C180(hud,
/// 0, id, 0) -> 0x14162A440 -> 0x141618040 `setMainCommandText(0, name(id), flash)` while the window (2 s) is open: Counter
/// Shield 0x10 "Wrathful Fist" / 0x11 "Wrathful Flurry" (the shield grown: `(bSuccess ^ 1) < grow`), Frying Pan 0x12 "Flame
/// Torrent" / 0x13 "Flame Barrage". `counter_window` / `counter_command` = `Sora::actions()`'s. None = "Attack". The HUD's
/// own reader is kh-host hud_forms.rs `attack_text` (the same rule for every form's counter); this is that rule for the
/// shields alone.
pub fn attack_command(form: &Form, counter_window: f32, counter_command: i32) -> Option<i32> {
    form_key(form.cmd)?;
    (counter_window > 0.0 && matches!(counter_command, 0x10..=0x13)).then_some(counter_command)
}

/// The attack row a window of `row` hits with in the held form (0x141357590: SCSH_* -> SH2_* in Frying Pan).
pub fn row_rename(cmd: i32, row: &str) -> Option<&'static str> {
    if cmd != CHANGE_SH2 {
        return None;
    }
    rows("rename", "SH2").find(|r| r.logical.eq_ignore_ascii_case(row)).map(|r| r.clip)
}

/// A hit shape of the form weapon's TresAtkColl0 (the rows `track.<form>.<clip>.<group>`, tools/sora_form_shield.js):
/// the group's shapes on the weapon parts of the grow level the clip is played at, at clip time `time`, as (end 1, end 2,
/// radius) in the world for an actor at `location` turned `yaw`. Per animation frame from the weapon mesh's sockets
/// (ATK_01 ...) on Sora's socket under the clip, interpolated as the Keyblade's own socket track (combat/hit.rs). None =
/// not a tracked group of that clip in that form.
pub fn track_shape(cmd: i32, group: &str, clip: &str, time: f32, location: Vec3, yaw: f32) -> Option<(Vec3, Vec3, f32)> {
    let form = form_key(cmd)?;
    let row = rows("track", form).find(|r| r.clip == clip && r.logical == group)?;
    let frames = usize::try_from(row.code).ok().filter(|f| *f > 0 && row.values.len() >= f * 7)?;
    let last = frames - 1;
    let f = if row.length > 0.0 { (time / row.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
    let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
    let j = (i + 1).min(last);
    let at = |k: usize| row.values[i * 7 + k] + (row.values[j * 7 + k] - row.values[i * 7 + k]) * t;
    let (sin, cos) = yaw.to_radians().sin_cos();
    let world = |v: Vec3| location + Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z);
    Some((world(Vec3::new(at(0), at(1), at(2))), world(Vec3::new(at(3), at(4), at(5))), at(6)))
}

/// The package's `hit_shape` hook (combat/hit.rs `shape`): the held form's tracked groups.
fn hit_shape(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<(Vec3, Vec3, f32)> {
    let clip = crate::anim::SEQUENCES.get(sequence)?.name;
    track_shape(cx.pawn.combat.form.cmd, group, clip, time, cx.pawn.cmc.location, cx.pawn.yaw)
}

/// (F-SHAPES) The world shapes of a group with a CONVEX (`vol.<form>.<clip>.<group>.<k>` rows: SH_Finish / SH_Finish2 of the
/// shields, tres_colltest01 x (6, 6, 8) and (0.5, 0.5, 8) + 30/s up to 6, and Counter Shield's PunchL x (2, 2, 3.5) along the
/// fist; before, spheres of their m_Size 75 / 200) at a clip time, for an actor at `location` turned `yaw`.
pub fn volume_shapes(cmd: i32, group: &str, clip: &str, time: f32, location: Vec3, yaw: f32) -> Option<Vec<crate::combat::hull::Volume>> {
    let form = form_key(cmd)?;
    let out: Vec<_> = rows("vol", form).filter(|r| r.clip == clip && r.logical == group).filter_map(|r| crate::combat::hull::VolTrack::of_row(r.flags, r.code, r.length, r.values)).map(|t| t.placed(time, location, yaw)).collect();
    (!out.is_empty()).then_some(out)
}

/// (F-SHAPES) The package's `hit_volumes` hook (combat/hit.rs `sweep_volumes`).
fn hit_volumes(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<Vec<crate::combat::hull::Volume>> {
    let clip = crate::anim::SEQUENCES.get(sequence)?.name;
    volume_shapes(cx.pawn.combat.form.cmd, group, clip, time, cx.pawn.cmc.location, cx.pawn.yaw)
}

/// The package's `row_rename` hook.
fn rename(cmd: i32, row: &str) -> Option<&'static str> {
    row_rename(cmd, row)
}

/// One style pawn the host draws during FINISH_SH (P5's `kh_er_render::keyblade::form_pawn`): its model, the logical clip
/// it plays (None = its row is not packed: its reference pose), drawn at Sora.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinisherPawn {
    pub model: &'static str,
    pub clip: Option<&'static str>,
    /// (AUDIT-A SHIELD) where it stands relative to Sora's actor: the offset in his actor frame (cm) and its yaw relative to
    /// his (degrees); zero = at his root (`finisher_pawns_at` fills it; KH3's links of the ride).
    pub offset: Vec3,
    pub yaw: f32,
}

/// (AUDIT-A SHIELD) The Chariot's model: TresStylePawnSHChariot, Pegasus's `m_SHChariotAssets` (StylePawn/w_so036 ->
/// StylePawn/w_so035): its OWN actor and mesh (100 bones, 3 slots), linked to Pegasus at X_Link_skt (StyleFinishSH Tick
/// 0x1413D5E50 sub 2: Chariot vt+0x23A8(Pegasus, "X_Link_skt", 1)), playing the same M_B424 / M_B425 as Pegasus and Sora.
pub const CHARIOT_MODEL: &str = "w_so035";

/// The style pawns of the running FINISH_SH (from `Sora::finish()` and the form command). (AUDIT-A SHIELD) KH3's Tick
/// 0x1413D5E50: sub 0 spawns Pegasus (w_so036) and the Chariot (w_so035) HIDDEN (vt+0xD98(0) on both); they play M_B424 with
/// Sora at M_B423's end (sub 1); sub 2 links them and shows both (vt+0xD98(1)); sub 3 plays M_B425 on the three at M_B424's
/// end; the cleanup (sub 5, 0x1413CFB60) removes them. So both are drawn from the port's sub 3 (after sub 2 ran) to 4, the
/// Chariot as its own pawn. The Frying Pan (w_so145) is shown at its spawn (slot 167 sub 0) until the cleanup.
pub fn finisher_pawns(finish: Option<FinishView>, cmd: i32) -> Vec<FinisherPawn> {
    let Some(f) = finish.filter(|f| f.cmd == FINISH_SH) else { return Vec::new() };
    let Some(key) = form_key(cmd) else { return Vec::new() };
    let Some(pawn) = rows("pawn", key).next() else { return Vec::new() };
    let at = |model: &'static str, clip: &'static str| FinisherPawn { model, clip: Some(clip), offset: Vec3::ZERO, yaw: 0.0 };
    match (key, f.sub) {
        ("SH", 3) => vec![at(pawn.clip, "M_B424"), at(CHARIOT_MODEL, "M_B424")],
        ("SH", 4) => vec![at(pawn.clip, "M_B425"), at(CHARIOT_MODEL, "M_B425")],
        ("SH2", 0..=4) => vec![at(pawn.clip, "M_J423")],
        _ => Vec::new(),
    }
}

/// (AUDIT-A SHIELD) [`finisher_pawns`] placed as KH3 links them, for a host that draws them while Sora plays `anim`
/// (`Output::anim` / `anim_ticked`): Pegasus and the Chariot stand where the ride's links put them relative to Sora
/// (`ride_link`: he rides the Chariot behind Pegasus, StyleFinishSH Tick sub 2's attachments; before, they were drawn with
/// their roots at his feet, so he hung under Pegasus), and M_B425 is drawn as its heaviest SAMPLE (B425, B425DL .. UR: the
/// pawns' samples are packed, keyblades/w_so036|w_so035/anims/B425*.glb; the logical blend space has no clip file) at his
/// time. KH3 plays M_B425 on all three with mode 2 (no loop bit: asm 0x1413D7xxx `mov dword ptr [rsp+0x20],2` before each
/// play), so its clock holds at the samples' end (0.6 s) and the pose changes with the blend axes (Pegasus's delta yaw and
/// pitch: banking, climbing). Host: pawn at `feet + R(his yaw) x offset`, yaw `his yaw + yaw`, clip `clip`, his clip time.
pub fn finisher_pawns_at(finish: Option<FinishView>, cmd: i32, anim: &crate::anim::AnimRequest) -> Vec<FinisherPawn> {
    let [(seq, _, time), _] = anim.body_samples();
    finisher_pawns(finish, cmd)
        .into_iter()
        .map(|mut p| {
            if p.clip == Some("M_B425") {
                p.clip = ["B425", "B425DL", "B425DR", "B425UL", "B425UR"].into_iter().find(|s| *s == seq).or(p.clip);
            }
            (p.offset, p.yaw) = pawn_place(p.model, seq, time);
            p
        })
        .collect()
}

/// (AUDIT-A SHIELD) Where the drawing host puts a pawn of [`finisher_pawns_at`]: Sora's feet / yaw -> the pawn's (world point,
/// yaw).
pub fn place_on(feet: Vec3, yaw: f32, p: &FinisherPawn) -> (Vec3, f32) {
    let (s, c) = yaw.to_radians().sin_cos();
    (feet + Vec3::new(p.offset.x * c - p.offset.y * s, p.offset.x * s + p.offset.y * c, p.offset.z), yaw + p.yaw)
}

/// (AUDIT-A SHIELD) THE RIDE'S LINKS (StyleFinishSH Tick 0x1413D5E50 sub 2: the Chariot attached to Pegasus's X_Link_skt,
/// Sora to the Chariot's X_link_skt; the rows `plink.SH.<clip>`, tools/sora_form_shield.js): at clip `seq` (M_B424, a
/// sample of M_B425) time `time`, where the Chariot's actor and Sora's actor stand in PEGASUS's actor frame: (chariot
/// point, chariot yaw, Sora point, Sora yaw), cm / degrees. E.g. B425: the Chariot 250 cm behind Pegasus, 30 up, Sora on
/// its floor (68 up); the turning samples swing them 172 cm aside, 46 degrees. None = not such a clip.
pub fn ride_link(seq: &str, time: f32) -> Option<(Vec3, f32, Vec3, f32)> {
    let r = FORM_SHIELD.iter().find(|r| r.what == "plink" && r.clip == seq)?;
    let frames = usize::try_from(r.code).ok().filter(|f| *f > 0 && r.values.len() >= f * 8)?;
    let last = frames - 1;
    let f = if r.length > 0.0 { (time / r.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
    let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
    let j = (i + 1).min(last);
    let at = |k: usize| r.values[i * 8 + k] + (r.values[j * 8 + k] - r.values[i * 8 + k]) * t;
    Some((Vec3::new(at(0), at(1), at(2)), at(3), Vec3::new(at(4), at(5), at(6)), at(7)))
}

/// (AUDIT-A SHIELD) StylePawn/w_so036 TresCharMesh0 RelativeLocation x (its mesh ahead of its actor), cm.
const PEGASUS_MESH_X: f32 = 100.0;

/// (AUDIT-A SHIELD) A point / yaw of a pawn's own MESH frame (Pegasus `w_so036`, the Chariot `w_so035`) in SORA's actor
/// frame while the three are linked (`ride_link`; anything else: unchanged, the pawn stands at his root).
fn in_sora_frame(model: &str, seq: &str, time: f32, p: Vec3, yaw: f32) -> (Vec3, f32) {
    let Some((c, c_yaw, s, s_yaw)) = ride_link(seq, time) else { return (p, yaw) };
    let turn = |v: Vec3, deg: f32| {
        let (sn, cs) = deg.to_radians().sin_cos();
        Vec3::new(v.x * cs - v.y * sn, v.x * sn + v.y * cs, v.z)
    };
    // the point in Pegasus's frame
    let (pp, py) = match model {
        CHARIOT_MODEL => (c + turn(p, c_yaw), yaw + c_yaw),
        // Pegasus's mesh stands 100 cm ahead of its actor (StylePawn/w_so036 TresCharMesh0 RelativeLocation (100, 0, 0))
        "w_so036" => (p + Vec3::new(PEGASUS_MESH_X, 0.0, 0.0), yaw),
        _ => return (p, yaw),
    };
    (turn(pp - s, -s_yaw), py - s_yaw)
}

/// (AUDIT-A SHIELD) Where each finisher pawn stands relative to Sora's actor while Sora plays `seq` at `time` (his heaviest
/// sample: `Output::anim_ticked.body_samples()[0]`): (offset in his actor frame, yaw relative to his). Pegasus and the
/// Chariot through the ride's links (`ride_link`: Sora rides the Chariot behind Pegasus, KH3's attachments), the Frying Pan
/// at his root. THE DRAWING HOSTS place each pawn of [`finisher_pawns`] with this (kh-er sora.rs INT line, the harness).
pub fn pawn_place(model: &str, seq: &str, time: f32) -> (Vec3, f32) {
    in_sora_frame(model, seq, time, Vec3::ZERO, 0.0)
}

/// (AUDIT-A SHIELD) The anchor of a pawn's clip notify (`pawn:<row id>`, the `pnotify` rows of the sheet: tools/
/// sora_form_shield.js) at the pawn's clip time `time` (its play runs in step with Sora's: the hosts pass the time of his
/// heaviest sample, `Output::anim_ticked.body_samples()[0]`): the point in Sora's ACTOR frame (x forward, y right, z up, cm;
/// the pawn stands at his root with his yaw) and the rotator (pitch, yaw, roll, degrees) in that frame, or in the WORLD when
/// the notify's bAbsoluteRotation is set (the bool). None = not such an anchor. kh-host vfx.rs resolves `Attach::Sora`
/// bones named `pawn:` with it, so an effect of Pegasus's kh_sk / R_hane4, the Chariot's L_naka or the pan's pan_handle3
/// follows that socket under the pawn's own clip, as KH3's attach component does on the pawn's mesh.
pub fn pawn_socket(name: &str, time: f32) -> Option<(Vec3, Vec3, bool)> {
    let id = name.strip_prefix("pawn:")?;
    let r = FORM_SHIELD.iter().find(|r| r.what == "pnotify" && r.id == id)?;
    let frames = usize::try_from(r.code).ok().filter(|f| *f > 0 && r.values.len() >= 3 + f * 6)?;
    let last = frames - 1;
    let f = if r.length > 0.0 { (time / r.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
    let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
    let j = (i + 1).min(last);
    let v = |frame: usize, k: usize| r.values[3 + frame * 6 + k];
    let at = |k: usize| v(i, k) + (v(j, k) - v(i, k)) * t;
    let near = if t < 0.5 { i } else { j };
    // (the pawns' own frames -> Sora's through the ride's links; a weapon's / the pan's track is in his frame already)
    let absolute = r.flags.get(2) == Some(&"abs");
    let (p, yaw) = in_sora_frame(r.logical, r.clip, time, Vec3::new(at(0), at(1), at(2)), v(near, 4));
    Some((p, Vec3::new(v(near, 3), if absolute { v(near, 4) } else { yaw }, v(near, 5)), absolute))
}

/// (AUDIT-A PLACEMENT) A THROWN weapon's own-clip anchor (`pawn:<row>` of a `.thrown` row: the raid's M_B414 notifies on the
/// shield / pan KH3 threw): (the model, its bone / socket the notify names, "None" = its root). kh-host vfx.rs then places
/// the effect on the weapon where it flies (kh-core `placement::ThrownWeapon`) under its own throw clip, not in Sora's frame.
pub fn thrown_row(name: &str) -> Option<(&'static str, &'static str)> {
    let id = name.strip_prefix("pawn:")?;
    let r = FORM_SHIELD.iter().find(|r| r.what == "pnotify" && r.id == id && r.flags.get(4) == Some(&"thrown"))?;
    Some((r.logical, r.flags.get(1).copied().unwrap_or("None")))
}

/// (AUDIT-A SHIELD) The key range of the pawns' notify states in `Pawn::notify_handles`: (0xE000 | the play's id mod 0x1000,
/// the row's index); never a sequence index (SEQUENCES < 0xE000 rows) nor AUDIT-B's weapon range 0xF000.
const PNOTIFY_KEY: u16 = 0xE000;

/// (AUDIT-A SHIELD) THE GROWN WEAPON'S AURA: each grown part's actor plays its `ResetPose` when GrowUp (pawn slot 638
/// 0x1405BD810 -> ChangeWeapon) spawns it, and that clip's one TresAnimNotify_AttachEffect (time 0, WhenChangeAnimationType
/// NONE: it lives as long as the weapon actor) is the grown shield's / pan's glow: (form, grow level, the anchor on the
/// part (AUDIT-B's `kb:<model>:<mount>:<bone>`, kh-host vfx/kbweapon.rs), the effect). The part goes when the weapon changes
/// again (the grown attack's weapon reset to LV1, the next grow, the form's end), and its aura with it.
pub const GROWN_AURA: [(&str, u8, &str, &str); 4] = [
    ("SH", 2, "kb:w_so033:R_Wep3_skt:center", "/Game/Effects/wep/w_so030/par/vw_lv1_inl0.vw_lv1_inl0"),
    ("SH", 3, "kb:w_so034:R_Wep3_skt:center", "/Game/Effects/wep/w_so030/par/vw_lv2_inl0.vw_lv2_inl0"),
    ("SH2", 2, "kb:w_so143:R_Wep3_skt:fire_sclA", "/Game/Effects/wep/w_so140/par/vw_so143_idg_fre01.vw_so143_idg_fre01"),
    ("SH2", 3, "kb:w_so144:R_Wep3_skt:fire_sclA", "/Game/Effects/wep/w_so140/par/vw_so144_idg_fre02.vw_so144_idg_fre02"),
];

/// The aura's row index in `Pawn::notify_handles` (with [`AURA_KEY`]).
const AURA_INDEX: u16 = 0xFFFF;

/// (AUDIT-A SHIELD) The grown aura's slot in `Pawn::notify_handles`: (0x5000 | grow level, 0xFFFF), outside the rows' range
/// (the shared pass ends every handle of its range that is not one of its rows).
const AURA_KEY: u16 = 0x5000;

/// (AUDIT-A SHIELD) THE WEAPONS' AND PAWNS' OWN CLIP NOTIFIES, every pose tick (sora.rs), through AUDIT-B's ONE shared
/// weapon-clip pass (forms/ship.rs `wnotify_pass_dt`; this package's `wnotify` rows, one per `pnotify` row, under the key
/// range 0xE000). Each weapon actor of the form and each finisher pawn plays its row of Sora's logical name in step with
/// him (StyleFinishSH Tick 0x1413D5E50 sub 1 / 3 for Pegasus and the Chariot, slot 167 0x1413E1210 sub 0 for the pan), and
/// its notifies fire on ITS mesh: effects and sounds on the `pawn:<pnotify row>` anchor ([`pawn_socket`]: the notify's socket
/// tracked under its own clip, through the ride's links for the pawns), the weapons' effects with his play's group, the
/// pawns' with 0. Pegasus's and the Chariot's M_B425 is a 2D blend space with NO NotifyTriggerMode (their assets set none;
/// Sora's sets SQEX_AllAnimation), so the engine's default AllAnimations: every sample in the blend above 1e-5 fires its own
/// notifies (UBlendSpaceBase::TickAssetPlayer 0x143F79E00; the rows' flag `all`); before, only the heaviest sample's did.
/// The THROWN shield's / pan's M_B414 notifies run on that weapon's own clip clock from the throw (flag `thrown`,
/// placement::ThrownWeapon), until the recall. A state notify ends at its end time or when Sora's play changes.
pub(crate) fn pawn_notifies(cx: &mut Ctx, dt: f32) {
    use crate::host::{Attach, EffectId, EffectSpawn};
    use crate::notify::{notify_asset_id, NotifyHandle};
    let form = form_key(cx.pawn.combat.form.cmd);
    // the grown part's aura (its actor's ResetPose at the grow): it lives while that grow level holds
    let grow = cx.pawn.combat.form.grow;
    let mut k = 0;
    while k < cx.pawn.notify_handles.len() {
        let (key, index, handle) = cx.pawn.notify_handles[k];
        if key & 0xF000 != AURA_KEY || index != AURA_INDEX || (form.is_some() && key & 0x000F == u16::from(grow)) {
            k += 1;
            continue;
        }
        cx.pawn.notify_handles.remove(k);
        if let NotifyHandle::Effect(h) = handle {
            cx.stop_effect(h, false);
        }
    }
    if let Some(&(_, _, anchor, name)) = form.and_then(|f| GROWN_AURA.iter().find(|a| a.0 == f && a.1 == grow)) {
        let aura_key = AURA_KEY | u16::from(grow);
        if !cx.pawn.notify_handles.iter().any(|h| h.0 == aura_key && h.1 == AURA_INDEX) {
            let handle = cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(name)), name, at: Attach::Sora { bone: Some(anchor), offset: Vec3::ZERO }, rotation: Vec3::ZERO, scale: 1.0, persistent: true });
            cx.pawn.notify_handles.push((aura_key, AURA_INDEX, NotifyHandle::Effect(handle)));
        }
    }
    // the rows (a form not held: none, so every state of the range ends)
    let row = |i: usize| FORM_SHIELD.get(i).filter(|r| r.what == "wnotify" && Some(r.form) == form).map(|r| super::ship::WNotify { clip: r.clip, code: r.code, flags: r.flags, effects: r.effects, sounds: r.sounds, values: r.values });
    super::ship::wnotify_pass_dt(cx, PNOTIFY_KEY, FORM_SHIELD.len(), &row, dt);
}

/// (AUDIT-A SHIELD) The finish's End (0x1413CF260 -> the cleanup 0x1413CFB60 destroys the pawns): every pawn notify state
/// still running stops with its pawn.
pub(crate) fn pawn_notifies_end(cx: &mut Ctx) {
    use crate::notify::NotifyHandle;
    let (mine, rest): (Vec<_>, Vec<_>) = cx.pawn.notify_handles.drain(..).partition(|h| h.0 & 0xF000 == PNOTIFY_KEY);
    cx.pawn.notify_handles = rest;
    for (_, _, handle) in mine {
        match handle {
            NotifyHandle::Sound(h, fade) => cx.stop_sound(h, fade),
            NotifyHandle::Effect(h) => cx.stop_effect(h, false),
        }
    }
}

/// (AUDIT-A SHIELD) The effects of the finisher pawns and their actors that are named after other models (the hosts'
/// other-keyblade name rule: kh-core forms::shared_clip_effects): the pawns' clip notifies (`pnotify` rows), Pegasus's
/// FINISH_SH_RUSH hit effect (StylePawn/w_so036 TresAtkColl0 m_HitEffects: Shooting Star's vw_so020_hit0), the thunder's
/// body (StylePawn/ProjSHThunder ParticleComp VW_SO035_THD0) and the pawns' disappear effects (ReceiveAccompanyPawnDisappear:
/// w_so036 vw_so035_dis_top, w_so035 vw_so035_dis_end).
pub fn pawn_effects(cmd: i32) -> Vec<&'static str> {
    let Some(form) = form_key(cmd) else { return Vec::new() };
    let mut out: Vec<&'static str> = rows("pnotify", form).filter_map(|r| r.effects.first().copied()).filter_map(|p| p.rsplit(['.', '/']).next()).collect();
    if cmd == CHANGE_SH {
        out.extend([RUSH_HIT_EFFECT, THUNDER_BODY, DISAPPEAR_PEGASUS, DISAPPEAR_CHARIOT]);
    }
    out
}

/// (AUDIT-A SHIELD) StylePawn/w_so036 TresAtkColl0 `m_HitEffects` [None]: `m_PawnHitEffect` = vw_so020_hit0 (import -51).
pub const RUSH_HIT_EFFECT: &str = "vw_so020_hit0";
/// (AUDIT-A SHIELD) StylePawn/ProjSHThunder (TresStyleProjSHThunder) `ParticleComp` template VW_SO035_THD0 (its body for its
/// m_MainLife; its hit effect is vs_thd_hit0, the row's own).
pub const THUNDER_BODY: &str = "vw_so035_thd0";
/// (AUDIT-A SHIELD) The pawns' `ReceiveAccompanyPawnDisappear` (their event graphs: SqExSpawnEmitterAttached on the pawn).
pub const DISAPPEAR_PEGASUS: &str = "vw_so035_dis_top";
pub const DISAPPEAR_CHARIOT: &str = "vw_so035_dis_end";

/// The ride's HUD gauge (StyleFinishSH Tick sub 4: controller vt+0x1B38(timer, -1)): the time left over Pegasus's
/// m_Param_MaxTime, 0 ..= 1, while the ride runs (INT: kh-host hud.rs draws it).
pub fn ride_gauge(finish: Option<FinishView>, cmd: i32) -> Option<f32> {
    let f = finish.filter(|f| f.cmd == FINISH_SH && f.sub == 4 && cmd == CHANGE_SH)?;
    let max = pawn_values("SH").first().copied().filter(|m| *m > 0.0)?;
    Some((f.root_len / max).clamp(0.0, 1.0))
}

/// The finish's cameras (StyleFinishSH Tick 0x1413D5E50; the hosts' camera is kh-camera forms.rs `FormCameras`, which takes
/// the phase from here). (AUDIT-A SHIELD, F-HUD's corrections, re-read in the decompiled Tick): sub 0 spawns an
/// `ATresCameraFix` on Sora (vt+0x788(pawn, ctrl)) with FUN_1405D6F40(100, 0) = SetFieldOfView(100) and FUN_1405D7840(0x78) =
/// priority 120, pushed with {0.0 s, ease-out (3), exp 2.0} = a CUT; it holds the view it starts on (no distance of its
/// own: `distance` 0). Sub 2 (once both pawns have meshes) spawns an `ATresCameraNormal` on PEGASUS: FUN_140627E30 =
/// m_TargetOffset (0, 0, 100), FUN_14060A640 = m_Rotation (pitch -25, 0, 0), FUN_1405EFB60 = distance 700, priority 120, a cut
/// in; no FOV: the component's 90; the Fix leaves with {0.5 s, ease-out, 2.0}. The cleanup 0x1413CFB60 (sub 5) removes the
/// ride camera with {0.5 s, ease-out, 2.0}. Port sub-steps: the Fix while sub 0 ..= 2 (sub 2's code has not run), the ride
/// camera from 3 to 5 (the cleanup runs in sub 5's tick).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinishCamera {
    pub on_pegasus: bool,
    pub offset: Vec3,
    pub pitch: f32,
    /// cm; 0 = the Fix holds the view it starts on.
    pub distance: f32,
    pub fov: f32,
    /// The camera stack priority (0x78).
    pub priority: i32,
    /// (blend in, blend out) s with ease-out exp 2.0: 0 = a cut.
    pub blend: (f32, f32),
}

/// The camera of the running FINISH_SH (Counter Shield), None = the normal camera.
pub fn finish_camera(finish: Option<FinishView>, cmd: i32) -> Option<FinishCamera> {
    let f = finish.filter(|f| f.cmd == FINISH_SH && cmd == CHANGE_SH)?;
    match f.sub {
        0..=2 => Some(FinishCamera { on_pegasus: false, offset: Vec3::ZERO, pitch: 0.0, distance: 0.0, fov: 100.0, priority: 0x78, blend: (0.0, 0.5) }),
        3..=5 => Some(FinishCamera { on_pegasus: true, offset: Vec3::Z * 100.0, pitch: -25.0, distance: 700.0, fov: 90.0, priority: 0x78, blend: (0.0, 0.5) }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn din(combo: i32, last: u8, reach: u8, grow: u8, n: i32) -> ShieldDecideIn {
        ShieldDecideIn { combo, reach, last_kind: last, dist: 150.0, dz: 0.0, target_high: false, air: false, grow, finish_count: n }
    }

    /// The decision chain of a ground combo near one target: 0x50, 0x51 (reach 0: 0x52 after 0x51), then the finishers.
    #[test]
    fn the_combo_of_combo_sh() {
        let (mut last, mut reach, mut n, mut kinds) = (0u8, 0u8, 0, Vec::new());
        for combo in 0..5 {
            let d = decide(&din(combo, last, reach, 1, n));
            kinds.push(d.kind);
            (last, reach, n) = (d.kind, d.reach, d.finish_count);
            if d.finish_end {
                break;
            }
        }
        assert_eq!(kinds, [0x50, 0x51, 0x52, 0x53, 0x54]);
        // Far (reach 1): 0x52 first, then 0x50 / 0x51 alternate with reach != 0.
        let d = decide(&ShieldDecideIn { dist: 550.0, ..din(0, 0, 0, 1, 0) });
        assert_eq!((d.kind, d.reach), (0x52, 1));
        assert_eq!(decide(&din(1, 0x52, 1, 1, 0)).kind, 0x51);
        assert_eq!(decide(&din(2, 0x51, 1, 1, 0)).kind, 0x50);
        // In the air 0x50 is 0x57; a grown shield spends its level at once, whatever the combo.
        assert_eq!(decide(&ShieldDecideIn { air: true, ..din(0, 0, 0, 1, 0) }).kind, 0x57);
        assert_eq!((decide(&din(0, 0, 0, 2, 0)).kind, decide(&din(2, 0x51, 0, 3, 0)).kind), (0x55, 0x56));
        // Combo 4 (or a second finisher) is the end.
        let d = decide(&din(4, 0x53, 0, 1, 1));
        assert!(d.finish && d.finish_end && d.kind == 0x54);
    }

    /// (AUDIT-A SHIELD) The ride's links (Tick sub 2's attachments): Sora on the Chariot's floor 150 cm behind Pegasus's actor,
    /// so Pegasus's mesh (100 cm ahead of its actor) stands 250 cm ahead of him and 68 cm lower; the turning samples swing the
    /// Chariot aside; M_B425 is drawn as its heaviest sample; the Frying Pan stays at his root.
    #[test]
    fn the_pawns_stand_where_kh3_links_them() {
        let (c, c_yaw, s, s_yaw) = ride_link("B425", 0.0).unwrap();
        assert!((c - Vec3::new(-150.0, 0.0, 30.0)).length() < 0.5 && c_yaw.abs() < 0.1, "{c} {c_yaw}");
        assert!((s - Vec3::new(-150.0, 0.0, 67.8)).length() < 0.5 && s_yaw.abs() < 0.1, "{s} {s_yaw}");
        let (peg, _) = pawn_place("w_so036", "B425", 0.0);
        assert!((peg - Vec3::new(250.0, 0.0, -67.8)).length() < 0.5, "{peg}");
        let (car, _) = pawn_place(CHARIOT_MODEL, "B425", 0.0);
        assert!((car - Vec3::new(0.0, 0.0, -37.8)).length() < 0.5, "{car}");
        let (_, _, s_ur, yaw_ur) = ride_link("B425UR", 0.0).unwrap();
        assert!(s_ur.y > 150.0 && yaw_ur < -40.0, "{s_ur} {yaw_ur}");
        assert_eq!(pawn_place("w_so145", "J423", 1.0), (Vec3::ZERO, 0.0));
    }

    /// (AUDIT-A SHIELD) The pawns' M_B425 has no NotifyTriggerMode: the engine's AllAnimations (0x143F79E00), so every sample
    /// the axes blend above 1e-5 fires its notifies. Their rows run through AUDIT-B's shared pass: one `wnotify` row per
    /// `pnotify` row, the M_B425 samples flagged `all`, the thrown weapon's rows `thrown` on its own clip clock.
    #[test]
    fn the_pawns_rows_run_through_the_shared_pass() {
        let mut req = crate::anim::AnimPlayer::new(AnimId::from_name("M_B425").unwrap()).request();
        let names = |r: &crate::anim::AnimRequest| {
            let mut v: Vec<&str> = r.active_samples().iter().map(|s| s.0).collect();
            v.sort_unstable();
            v
        };
        assert_eq!(names(&req), ["B425"], "level flight: the centre sample alone");
        (req.axis, req.axis_y) = (1.15, 12.5);
        let blended = req.active_samples();
        let total: f32 = blended.iter().map(|s| s.1).sum();
        assert!(blended.len() >= 2 && (total - 1.0).abs() < 1.0e-4, "a banking climb blends several samples: {blended:?}");
        let rows = |what: &'static str| FORM_SHIELD.iter().filter(move |r| r.what == what);
        assert_eq!((rows("pnotify").count(), rows("wnotify").count()), (133, 133));
        for w in rows("wnotify") {
            let p = rows("pnotify").find(|p| p.id.strip_prefix("pnotify.") == w.id.strip_prefix("wnotify.")).unwrap_or_else(|| panic!("{}", w.id));
            // (AUDIT-B W-VFX2) a form weapon's thunder (beam) row spawns on its kb: anchor with the end socket (the host reads
            // both ends on the weapon's mesh, KH3 0x144250690); every other row on its pnotify row's track
            let beam = w.flags[0].starts_with(&format!("kb:{}:", p.logical)) && w.flags[0].contains(":to") && !p.flags.contains(&"thrown");
            assert!(beam || w.flags[0].strip_prefix("pawn:") == Some(p.id), "{}: the anchor is its pnotify row's track ({})", w.id, w.flags[0]);
            let all = ["w_so036", "w_so035"].contains(&p.logical) && p.clip.starts_with("B425");
            assert_eq!((w.flags.contains(&"all"), w.flags.contains(&"thrown")), (all, p.flags.contains(&"thrown")), "{}", w.id);
        }
        assert_eq!(rows("wnotify").filter(|w| w.flags.contains(&"thrown")).count(), 5, "w_so032's 3, w_so142's 2");
        // (AUDIT-B W-VFX2) w_so031's vw_so030_und / top / naka rows (B401, B285 / B286, B433 / B434)
        assert_eq!(rows("wnotify").filter(|w| w.flags[0].starts_with("kb:")).count(), 32, "the transform unit's thunder rows");
    }

    #[test]
    fn the_sheet_carries_both_forms() {
        for cmd in [CHANGE_SH, CHANGE_SH2] {
            for kind in 0x50..=0x57u8 {
                let r = attack_row(cmd, kind).unwrap_or_else(|| panic!("{cmd:#x} {kind:#x}"));
                assert!(AnimId::from_name(r.clip).is_some(), "{} {}: not in the animation table", r.id, r.clip);
            }
        }
        assert_eq!(replaced_clip(CHANGE_SH, "M_B411"), Some("B419"));
        assert_eq!(replaced_clip(CHANGE_SH2, "M_J423"), Some("J423"));
        assert_eq!(replaced_clip(CHANGE_SH, "M_J423"), None);
        assert_eq!(replaced_clip(CHANGE_SH, "GuardChange"), Some("B285"));
        assert_eq!((attack_row(CHANGE_SH, 0x50).unwrap().group, attack_row(CHANGE_SH2, 0x50).unwrap().group), (0, 0x867));
        assert_eq!(row_rename(CHANGE_SH2, "SCSH_TURN"), Some("SH2_TURN"));
        assert_eq!(row_rename(CHANGE_SH, "SCSH_TURN"), None);
        assert_eq!(pan_window(), Some((2.5333, 3.3333)));
    }

    #[test]
    fn the_guard_grows_the_shield_to_lv3() {
        let mut p = crate::pawn::Pawn::new(crate::params::SORA_PARAMS, crate::cmc::Cmc::new(&crate::params::SORA_PARAMS, Vec3::ZERO), 0.0, crate::anim::AnimPlayer::new(AnimId::IdleN));
        assert_eq!(guard_grow(&mut p.combat.form, false), None, "no form");
        p.combat.form.cmd = CHANGE_SH;
        p.combat.form.grow = 1;
        assert_eq!(guard_grow(&mut p.combat.form, false).map(AnimId::name), Some("B285"));
        // (AUDIT-A SHIELD) the label follows the counter window (F-HUD's trace), not the grow level
        assert_eq!(attack_command(&p.combat.form, 0.0, 0x10), None, "no window: Attack");
        assert_eq!(attack_command(&p.combat.form, 2.0, 0x10), Some(0x10));
        assert_eq!(guard_grow(&mut p.combat.form, true).map(AnimId::name), Some("B434"));
        assert_eq!((p.combat.form.grow, attack_command(&p.combat.form, 1.0, 0x11)), (3, Some(0x11)));
        assert_eq!(guard_grow(&mut p.combat.form, false), None, "LV3 is the top");
        assert!(p.combat.form.weapon_units()[0].ends_with("w_so034_wep"), "{:?}", p.combat.form.weapon_units());
        p.combat.form.cmd = CHANGE_SH2;
        assert_eq!(attack_command(&p.combat.form, 1.0, 0x13), Some(0x13));
        assert_eq!(attack_command(&p.combat.form, 1.0, 0x03), None, "another form's counter is not the shields'");
    }

    #[test]
    fn the_shield_hit_shapes_follow_the_weapon_sockets() {
        // Shield Spin (B419): the shield's ATK_01 sphere (r 120) on R_Wep3_skt, frame 0 as the sheet has it.
        let (a, b, r) = track_shape(CHANGE_SH, "None", "B419", 0.0, Vec3::ZERO, 0.0).unwrap();
        assert_eq!((a, b, r), (Vec3::new(-15.9, 46.6, 135.2), Vec3::new(-15.9, 46.6, 135.2), 120.0));
        // Turned 90 degrees and moved: the same point in the world.
        let (a, _, _) = track_shape(CHANGE_SH, "None", "B419", 0.0, Vec3::new(100.0, 0.0, 0.0), 90.0).unwrap();
        assert!((a - Vec3::new(100.0 - 46.6, -15.9, 135.2)).length() < 1.0e-3, "{a}");
        // It moves along the clip; LV3's punch (w_so034 PunchL r 200) only in Wrathful Flurry; Frying Pan's own pan.
        assert_ne!(track_shape(CHANGE_SH, "None", "B419", 0.45, Vec3::ZERO, 0.0).unwrap().0, Vec3::new(-15.9, 46.6, 135.2));
        // (F-SHAPES) Counter Shield's PunchL is w_so034's CONVEX (a hull row now), Frying Pan's w_so144 PunchL a CAPSULE2.
        assert_eq!(track_shape(CHANGE_SH, "PunchL", "B426", 1.0, Vec3::ZERO, 0.0), None);
        assert_eq!(track_shape(CHANGE_SH2, "PunchL", "B426", 1.0, Vec3::ZERO, 0.0).map(|s| s.2), Some(375.0));
        assert_eq!(track_shape(CHANGE_SH, "PunchL", "B419", 0.0, Vec3::ZERO, 0.0), None);
        assert_eq!(track_shape(0x94, "None", "B419", 0.0, Vec3::ZERO, 0.0), None, "not in Second Form");
    }

    /// (F-SHAPES) The shields' CONVEX shapes are tres_colltest01 hulls: SH_Finish (6, 6, 8, LOCAL: upright, radius 600, half
    /// height 400 at the shield's ATK_01 of the window's start), SH_Finish2 growing from (0.5, 0.5) to 6 (its z capped at 6 from
    /// the start: FUN_140649BF0), PunchL (2, 2, 3.5) turned with the fist (NORMAL, pitch 90). The old stand-ins were spheres of
    /// m_Size 75 / 200.
    #[test]
    fn the_shields_convex_shapes_are_hulls() {
        use crate::combat::hull::Volume;
        let hull = |cmd: i32, g: &str, clip: &str, t: f32| match volume_shapes(cmd, g, clip, t, Vec3::ZERO, 0.0).as_deref() {
            Some([Volume::Hull(h)]) => *h,
            other => panic!("{g}: {other:?}"),
        };
        let finish = hull(CHANGE_SH, "SH_Finish", "B417", 0.6);
        assert!((finish.scale - Vec3::new(6.0, 6.0, 8.0)).length() < 1.0e-3 && finish.rotation.angle_between(glam::Quat::IDENTITY) < 1.0e-4, "{finish:?}");
        assert!(finish.distance(finish.centre + Vec3::new(590.0, 0.0, 390.0)) == 0.0);
        let early = hull(CHANGE_SH2, "SH_Finish2", "B417", 0.9);
        let late = hull(CHANGE_SH2, "SH_Finish2", "B417", 1.5);
        assert!((early.scale - Vec3::new(0.5, 0.5, 6.0)).length() < 1.0e-3 && (late.scale - Vec3::new(6.0, 6.0, 6.0)).length() < 1.0e-3, "{early:?} {late:?}");
        let punch = hull(CHANGE_SH, "PunchL", "B426", 1.0);
        assert!((punch.scale - Vec3::new(2.0, 2.0, 3.5)).length() < 1.0e-3);
        assert!(punch.rotation.angle_between(glam::Quat::IDENTITY) > 0.1, "turned with the fist (pitch 90 under ATK_01)");
        assert!(volume_shapes(CHANGE_SH2, "PunchL", "B426", 1.0, Vec3::ZERO, 0.0).is_none(), "Frying Pan's PunchL is a capsule");
    }
}
