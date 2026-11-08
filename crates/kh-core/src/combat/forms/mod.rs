//! (weapons P3) THE FORM MOVESETS: what a form package (F1 .. F13, F-RAGE: documentation/KH3_WEAPONS_PLAN.md 1.3, 4,
//! 6e) plugs into the form framework for its forms. The framework is `combat/form.rs` (StartStyle, finish commands,
//! EndStyle, the wear form), `combat/style.rs` (the gauge, AwardAtMax's offer, the form's clock),
//! `states/style_change.rs` (the transformation) and `states/style_finish.rs` (the finish commands); the forms are the
//! rows `form.<code>` of `sheets/port/sora-keyblade-forms.json` (`combat::form::KEYBLADE_FORMS`, written by
//! `tools/sora_keyblade_forms.js`).
//!
//! WHAT THE FRAMEWORK DOES FOR EVERY READY FORM FROM ITS ROW ALONE (no hook):
//! - the offer: a full gauge offers the held Keyblade's first stage (form level 0) and, inside a first stage, its
//!   `next` (form level 1) - AwardAtMax 0x1413F0ED0; Second Form and Rage Form offer no further form;
//! - the command (`command_time` x (1 + 0.2 FORM_TIME)), StartStyle: the form, its level (`stage`), its clock
//!   (`time`), the Keyblade it needs (taken from its slot when another is held; refused when it is in none), bad
//!   statuses blocked;
//! - STYLECHANGE: the world slow, `transform_clip`, the wear form (`wear_form` at `wear_at`) with its abilities
//!   (`abilities`), the charge effect on the weapon's socket (`units`), then the finish command;
//! - the finish command (`finish`; the Second Form family: `finish_levels`), +3 s per further full gauge;
//! - STYLEFINISH for its finish command (with no body the form simply ends), EndStyle (`end_delete`).
//!
//! WHAT A FORM PACKAGE ADDS: its own file `combat/forms/<name>.rs` with one `pub(crate) const MOVESET: FormMoveset`
//! (start from [`FormMoveset::NONE`]), one `mod <name>;` line and one entry in [`MOVESETS`] below, and its forms'
//! `moveset` cell in tools/sora_keyblade_forms.js (`MOVESET`) set to its name (the test below checks the two agree).
//! From then on its forms are READY. Its own states (the form's attack and move states, a STYLEFINISH body that needs
//! more than the shared fields) are new `StateData` variants: one registry line each in `states/mod.rs` (INT). The data
//! the renderer and the sound / effect hosts read for the form is on the row and on `Form` (`Form::row`,
//! `weapon_units`, `projectile`, `transform_clip`), and the wear form goes `Form.wear_form` -> `kh_er_render::set_wear_form`
//! -> kh-look `step_wear` -> kh-er `boc.rs effective` as Second Form's does.
//!
//! Until a form's package lands the form is PENDING (`form::ready`): never offered, its command refused, never a crash.

// (weapons F-SHIELD) Counter Shield / Frying Pan
pub mod shield;
// (F-RAGE) Rage Form
pub mod rage;
// (F-CLAWS) Agile Claws / Twin Yo-Yos / Blizzard Claws / Blizzard Blades
pub mod claws;
// (F-STAFF) Mirage Staff
pub mod staff;
// (F-SHIP) Highwind / Storm Flag
pub mod ship;
// (F-HAMMER) Hyper Hammer / Drill Punch / Boom Hammer / Clock Drill
pub mod hammer;
// (F-ARROWGUN) Double Arrowguns / Magic Launcher / Hunny Blasters / Hunny Launcher
pub mod arrowgun;
// (F-NANO) Nano Arms
pub mod nano;
// (F-LIGHTDARK) Light Form / Dark Form / Double Form
pub mod lightdark;
// (F-ULTIMATE) Ultimate Form
pub mod ultimate;
// (F-SECOND) Second Form S / D: Ancient Light's data, the Starlight weapon's shapes (the moveset is SECOND_FORM below)
pub mod second;
// (AUDIT-A PLACEMENT) every form weapon's ChangeAttachParent window and every ThrowWeapon of the forms (the hosts' placement)
pub mod placement;

use super::form::KeybladeFormRow;
use super::hull::Volume; // (F-SHAPES)
use crate::host::TargetId;
use crate::pawn::Ctx;
use crate::states::style_finish::StyleFinish;
use crate::states::StateData;

/// (F-SHIELD) A hit window's shape: (end 1, end 2, radius) in the world.
pub(crate) type HitShape = Option<(glam::Vec3, glam::Vec3, f32)>;

/// One form package's hooks. Every hook has a no-op default in [`FormMoveset::NONE`].
#[derive(Clone, Copy)]
pub(crate) struct FormMoveset {
    /// The package's name (the `moveset` cell of its rows).
    pub name: &'static str,
    /// The form rows it implements (`form.DW`, `form.BZ` ...).
    pub forms: &'static [&'static str],
    /// The finish commands whose STYLEFINISH body it implements (`FINISH_BZ` 0x9D ...): only these are offered.
    pub finishes: &'static [i32],
    /// StartStyle's per-form tail (0x14141A9D0, the switch after the form's fields: the row's `start_extra`), run once
    /// the framework has set the form up, before the finish commands are deleted.
    pub on_start: fn(&mut Ctx, &'static KeybladeFormRow),
    /// EndStyle's per-form part (0x14141CA20), run before the framework deletes `end_delete` and clears the form.
    pub on_end: fn(&mut Ctx, &'static KeybladeFormRow),
    /// Every controller frame while the form is held (in the gauge tick, after AwardAtMax, before the form's clock; not
    /// during STYLECHANGE, where the gauge tick returns early): the form's own
    /// gauges, its projectiles' upkeep, the grow level (`Form::grow`; what raises it in KH3 is not traced) ...
    pub tick: fn(&StateData, &mut Ctx, &'static KeybladeFormRow, f32),
    /// The form's attack request (pawn slot 1337 0x141432F00: the attack state's class by form): `Some(accepted)` =
    /// handled (its own attack state was requested or refused), `None` = the base attack. Called by every attack
    /// request that goes through `forms::attack` (today the reaction commands' Triangle; the attack button's request in
    /// `states/attack.rs` is INT's one line, build/sora_tmp/weapons/P3/STATE.md).
    pub attack: fn(&mut StateData, &mut Ctx, TargetId) -> Option<bool>,
    /// The finish command a full gauge gives in this form, from the row's `finish` (Rage Form: RISKCHARGE while fewer
    /// than three were used, then FINISH_AN; the row's note).
    pub finish_cmd: fn(&Ctx, &'static KeybladeFormRow) -> i32,
    /// STYLEFINISH `Begin` for one of `finishes`, after the begin event common to every finish command.
    pub finish_begin: fn(&mut StyleFinish, &mut Ctx),
    /// STYLEFINISH's per-command tick (slots 143 .. 161): true = the command is done and has chained on itself (as the
    /// three Second Form bodies do: `StyleFinish::finish` / `DoChain`). The shared helpers are `StyleFinish`'s `play`,
    /// `extra`, `remove_extra`, `pre_decide`, `channels`, `hover`, `traction_tick`, `finish` and its `a: Attack` fields;
    /// `End` ends the form (`form::finish_ended`) for every finish command.
    pub finish_tick: fn(&mut StyleFinish, &mut Ctx, f32) -> bool,
    /// A package alias for a name that is in none of the form's anim sets (None = unchanged). The anim sets themselves
    /// need no hook (F-LOCO): every play resolves through the held form's chain ([`replaced`]) first, then this.
    pub replace_clip: fn(&str) -> Option<&'static str>,
    /// (F-SHIELD) A hit window's shape for a group the Keyblade's component lacks (the form weapon's TresAtkColl0): (end 1,
    /// end 2, radius) in the world, from the window's sequence (index into `SEQUENCES`) at clip time `time` (combat/hit.rs).
    pub hit_shape: fn(&Ctx, &str, usize, f32) -> HitShape,
    /// (F-SHIELD) The attack row a hit window of `row` hits with in the held form (KH3's per-form renames, e.g.
    /// 0x141357590 SCSH_* -> SH2_*); None = unchanged.
    pub row_rename: fn(i32, &str) -> Option<&'static str>,
    /// (F-SHIP) Every shot of Sora's, every tick, before it hits (combat/hit.rs `tick`), whether a form is held or not: a
    /// package moves its own projectiles here (by their `Window::key`); others leave the shot alone.
    pub shot_motion: fn(&mut Ctx, &mut crate::combat::hit::Shot),
    /// (F-LIGHTDARK) m_ReplaceAnimSets by the held form's command (one package, several sets: Light / Dark Form AnimSet22,
    /// Double Form AnimSet23), tried before `replace_clip`; None = the answer of `replace_clip`.
    pub replace_clip_in: fn(i32, &str) -> Option<&'static str>,
    /// (F-SHAPES) A hit window's shapes when its group is a CONVEX hull or several shapes (the form weapon's TresAtkColl0:
    /// each shape its own primitive, one record and hit list per group; combat/hull.rs): the world [`Volume`]s at clip time
    /// `time` of the window's sequence; None = not such a group (then `hit_shape`).
    pub hit_volumes: fn(&Ctx, &str, usize, f32) -> Option<Vec<Volume>>,
}

fn no_start(_: &mut Ctx, _: &'static KeybladeFormRow) {}
fn no_tick(_: &StateData, _: &mut Ctx, _: &'static KeybladeFormRow, _: f32) {}
fn no_attack(_: &mut StateData, _: &mut Ctx, _: TargetId) -> Option<bool> {
    None
}
fn row_finish(_: &Ctx, row: &'static KeybladeFormRow) -> i32 {
    row.finish
}
fn no_finish_begin(_: &mut StyleFinish, _: &mut Ctx) {}
fn no_finish_tick(_: &mut StyleFinish, _: &mut Ctx, _: f32) -> bool {
    true
}
fn no_clip(_: &str) -> Option<&'static str> {
    None
}
fn no_shape(_: &Ctx, _: &str, _: usize, _: f32) -> HitShape {
    None
}
fn no_rename(_: i32, _: &str) -> Option<&'static str> {
    None
}
fn no_motion(_: &mut Ctx, _: &mut crate::combat::hit::Shot) {}
fn no_clip_in(_: i32, _: &str) -> Option<&'static str> {
    None
}
// (F-SHAPES)
fn no_volumes(_: &Ctx, _: &str, _: usize, _: f32) -> Option<Vec<Volume>> {
    None
}

impl FormMoveset {
    /// Every hook a no-op: `FormMoveset { name: "hyper_hammer", forms: &["form.HM", "form.DR"], on_start: start, ..FormMoveset::NONE }`.
    pub const NONE: FormMoveset = FormMoveset {
        name: "-",
        forms: &[],
        finishes: &[],
        on_start: no_start,
        on_end: no_start,
        tick: no_tick,
        attack: no_attack,
        finish_cmd: row_finish,
        finish_begin: no_finish_begin,
        finish_tick: no_finish_tick,
        replace_clip: no_clip,
        hit_shape: no_shape,
        row_rename: no_rename,
        shot_motion: no_motion,
        replace_clip_in: no_clip_in,
        hit_volumes: no_volumes, // (F-SHAPES)
    };
}

/// The framework's own moveset: the Second Form family (`form.L1` the Kingdom Key, `form.L2` Starlight, `form.L5` Dead
/// of Night) with COMBO_LIMIT (`states/attack.rs` `in_form`) and the three STYLEFINISH bodies of `states/style_finish.rs`;
/// its paths in form.rs / style.rs are the hard-wired ones of before (no hook of its own runs anything). (F-SECOND)
/// Starlight's finish level 3, `LM_UNIONDISCODE` (0xAE, STYLEFINISH slot 155 0x1413E2810), is the fourth body there
/// (`union_discode`); its clip's Starlight weapon groups come from `second::hit_shape` (None outside Second Form S).
const SECOND_FORM: FormMoveset = FormMoveset { name: "second_form", forms: &["form.L1", "form.L2", "form.L5"], finishes: &[0xAB, 0xAC, 0xAD, second::LM_UNIONDISCODE /* (F-SECOND) */], hit_shape: second::hit_shape /* (F-SECOND) */, ..FormMoveset::NONE };

/// THE REGISTRY: one entry per form package (`&<name>::MOVESET`), after the framework's own Second Form.
pub(crate) static MOVESETS: &[&FormMoveset] = &[&SECOND_FORM, &shield::MOVESET, &rage::MOVESET /* (F-RAGE) */, &claws::MOVESET /* (F-CLAWS) */, &staff::MOVESET /* (F-STAFF) */, &ship::MOVESET /* (F-SHIP) */, &arrowgun::MOVESET /* (F-ARROWGUN) */, &hammer::MOVESET /* (F-HAMMER) */, &nano::MOVESET /* (F-NANO) */, &lightdark::MOVESET /* (F-LIGHTDARK) */, &ultimate::MOVESET /* (F-ULTIMATE) */];

/// The moveset a form row runs with, for hosts and the debug text: "second_form" (the framework's own), a package's
/// name, or "pending" (`form::ready`); "-" for a row that does not exist.
pub fn moveset_name(id: &str) -> &'static str {
    match (super::form::keyblade_forms_by_id(id), of_form(id)) {
        (_, Some(m)) => m.name,
        (Some(_), None) => "pending",
        (None, None) => "-",
    }
}

/// The package of a form row.
pub(crate) fn of_form(id: &str) -> Option<&'static FormMoveset> {
    MOVESETS.iter().copied().find(|m| m.forms.contains(&id))
}

/// The package that implements a finish command's body.
pub(crate) fn of_finish(cmd: i32) -> Option<&'static FormMoveset> {
    MOVESETS.iter().copied().find(|m| m.finishes.contains(&cmd))
}

/// The held form's package.
pub(crate) fn of_current(cx: &Ctx) -> Option<(&'static FormMoveset, &'static KeybladeFormRow)> {
    let row = super::form::row_of(cx.pawn.combat.form.cmd)?;
    of_form(row.id).map(|m| (m, row))
}

/// The attack request in a form (see [`FormMoveset::attack`]).
pub(crate) fn attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    let (m, _) = of_current(cx)?;
    (m.attack)(state, cx, target)
}

/// (F-SHIELD) The held form's hit shape of a group (see [`FormMoveset::hit_shape`]).
pub(crate) fn hit_shape(cx: &Ctx, group: &str, sequence: usize, time: f32) -> HitShape {
    let (m, _) = of_current(cx)?;
    (m.hit_shape)(cx, group, sequence, time)
}

/// (F-SHAPES) The held form's shapes of a group made of hulls / several shapes (see [`FormMoveset::hit_volumes`]).
pub(crate) fn hit_volumes(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<Vec<Volume>> {
    let (m, _) = of_current(cx)?;
    (m.hit_volumes)(cx, group, sequence, time)
}

/// (F-SHIELD) The held form's rename of an attack row (see [`FormMoveset::row_rename`]).
pub(crate) fn row_rename(cmd: i32, row: &str) -> Option<&'static str> {
    let form = super::form::row_of(cmd)?;
    of_form(form.id).and_then(|m| (m.row_rename)(cmd, row))
}

/// (F-SHIP) Every package's `shot_motion` on one shot (see [`FormMoveset::shot_motion`]).
pub(crate) fn shot_motion(cx: &mut Ctx, shot: &mut crate::combat::hit::Shot) {
    for m in MOVESETS {
        (m.shot_motion)(cx, shot);
    }
}

// ================================================================================================================
// (F-LOCO) THE FORM MOVESET'S ANIMATIONS: ONE CONTRACT FOR EVERY FORM, DRIVEN BY KH3'S OWN DATA (no per-package hook).
//
// How KH3 does it (re/decomp):
// - THE INSTALL. StartStyle 0x14141A9D0 -> pawn vt+0x13C8 (ATresPlayerPawnBase 0x14144A9F0, ChangeWeapon) -> 0x1405BD620
//   -> MyEquipment vt+0x4D8 (UTresEquipmentComponent 0x14177C4A0) -> vt+0x5F0 (0x141791120): the equip data's form entry
//   (equip data vt+0x248(form)) holds m_ReplaceAnimSets at +0x30 (ArrayNum +0x38), m_ReplaceMagicSet +0x40,
//   m_ReplaceProjectileSet +0x48. With ArrayNum > 0 it calls the pawn's anim interface (pawn+0x440) slot +0x30 with the
//   array, else slot +0x38 (clear) [the two slots' targets inferred from the ArrayNum test; the anim instance's own
//   setter is TresAnimInstance vt+0x5D0 = 0x14056CCB0: copy the array, null = clear (vt+0x5F0 0x14056CC70 the same for
//   m_ReplaceAnimSets2), reached through UTresSkeletalMeshComponent vt+0xF80 0x1405D7940]. The arrays are the rows'
//   `anim_sets` (sheets/port/sora-keyblade-forms.json, from TresEquipKeybladeData m_EquipMap); EndStyle's weapon change
//   back installs the Keyblade's own (empty) list.
// - THE LOOKUP. Every play by name (UTresAnimInstance 0x14056ECE0) resolves the name with FindAnim 0x14056DE80: each list
//   from its LAST element to its first, first hit wins: m_ReplaceMatineeAnimSets, m_AppendAnimSetList, m_ReplaceAnimSets2,
//   m_ReplaceAnimSets, m_WorldAppendAnimSetList, MyAnimSets (the base set). So a form's set replaces the base set's row of
//   the same logical name for EVERY play of every state (Idle's "IdleB", Run's "RunLoopB" / "RunStartB" blend spaces,
//   the landings, the guard, the dodge, the damage reactions, the magic casts ...), and a later set (AnimSet16 of Hunny
//   Blasters) over an earlier (AnimSet2). The play then dispatches on the ASSET'S class: a blend space plays as one
//   (0x14056F260, the states' blend parameter drives it), so a replacement blend space blends as the base one did.
// - THE PORT. The generator (tools/gen_port.js) emits every form set as `anim::FORM_ANIM_SETS` (logical name -> row, the
//   blend spaces as rows with their samples, axes and grid); [`replaced`] walks the held form's chain last set first;
//   `AnimPlayer::start` (anim.rs) resolves every play through [`resolve`], the locomotion loop's walk / run pair too.
//   The package hooks `replace_clip_in` / `replace_clip` are only consulted for a name no set of the chain has.
// ================================================================================================================

/// (F-LOCO) The held form's replacement anim sets, the last one first (FindAnim 0x14056DE80's order).
fn chain(cmd: i32) -> impl Iterator<Item = &'static crate::anim::FormAnimSet> {
    super::form::row_of(cmd).into_iter().flat_map(|row| row.anim_sets.iter().rev()).filter_map(|path| {
        let set = path.rsplit(['.', '/']).next()?;
        crate::anim::FORM_ANIM_SETS.iter().find(|s| s.name == set)
    })
}

/// (F-LOCO) THE CONTRACT: the animation-table row a logical name (`IdleB`, `RunLoopB`, `GuardWalk`, `1301` ...) plays as
/// while the form `cmd` is held (m_ReplaceAnimSets through FindAnim); None = the base set's row (no form, Second Form, or
/// a name no set of the form has). A row may be a blend space (`RunLoopB7`): it plays as one.
pub fn replaced(cmd: i32, logical: &str) -> Option<crate::anim::AnimId> {
    if cmd == 0 {
        return None;
    }
    chain(cmd).find_map(|set| set.rows.iter().find(|r| r.0 == logical).map(|r| r.1))
}

/// (F-LOCO) [`replaced`] as the row's name (what the packages' `replaced_clip` answered).
pub fn replaced_clip(cmd: i32, logical: &str) -> Option<&'static str> {
    replaced(cmd, logical).map(crate::anim::AnimId::name)
}

/// (F-LOCO) The row a play of `id` plays while the form `cmd` is held ([`replaced`] by the row's name, else `id`). Every
/// play goes through it (anim.rs `AnimPlayer::start`, the locomotion loop's walk / run pair too); idempotent (a set's
/// rows are never logical names of the same chain: tested below).
pub fn resolve(cmd: i32, id: crate::anim::AnimId) -> crate::anim::AnimId {
    replaced(cmd, id.name()).unwrap_or(id)
}

/// (AUDIT-B W-DATA) The logical name a FORM WEAPON plays while Sora plays the held form's BLEND-SPACE row `id` (KH3: each
/// weapon actor's anim instance plays its own row of the logical name Sora's state asked for, so Twin Yo-Yos' yo-yos play
/// their single clip 4321 "MagicFire" whatever samples of his MagicFire4 blend he shows). A sequence row needs no answer: the
/// weapon finds its clip by that sequence (or `M_<seq>`). None: no form, not a blend space, or no set of the chain has it.
pub fn weapon_logical(cmd: i32, id: crate::anim::AnimId) -> Option<&'static str> {
    if cmd == 0 || id.data().samples.len() < 2 {
        return None;
    }
    chain(cmd).find_map(|set| set.rows.iter().find(|r| r.1 == id).map(|r| r.0))
}

/// The package hooks' alias of a name (see [`FormMoveset::replace_clip_in`] / [`FormMoveset::replace_clip`]).
fn hook_clip(cmd: i32, name: &str) -> Option<&'static str> {
    let row = super::form::row_of(cmd)?;
    of_form(row.id).and_then(|m| (m.replace_clip_in)(cmd, name).or_else(|| (m.replace_clip)(name)))
}

/// The clip `name` is played as by the framework's own plays (the transformation, the finish bodies: P3) and the
/// packages' explicit lookups: the form's anim sets ([`replaced`]), else the package's alias hooks, else the name.
pub(crate) fn clip(cmd: i32, name: &str) -> &str {
    replaced_clip(cmd, name).or_else(|| hook_clip(cmd, name)).unwrap_or(name)
}

/// (F-NANO) An animation-table row through the held form's replacement anim sets ([`clip`] by the row's name): the plays
/// of the states that are not the form's own (the dodge, the air dodge, the landings: AnimSet8's 8051 / 8054 / 8034 ...).
pub(crate) fn anim(cmd: i32, id: crate::anim::AnimId) -> crate::anim::AnimId {
    crate::anim::AnimId::from_name(clip(cmd, id.name())).unwrap_or(id)
}

/// (F-LOCO) The Idle and Run states keep the battle stance (their battle flag, pawn vt+0x1740, forced on) in forms 0x85
/// Blizzard Blades, 0x87 Ultimate Form, 0x88 Mirage Staff and 0x8B Rage Form: Idle Tick 0x1413944D0 (with a player
/// controller: its form vt+0x1A70) and Run Tick 0x1413AED70, both `(form - 0x85) < 7 && (0x4D >> (form - 0x85)) & 1`.
pub fn keeps_battle_stance(cmd: i32) -> bool {
    let d = cmd.wrapping_sub(0x85);
    (0..7).contains(&d) && (0x4D >> d) & 1 != 0
}

/// (FORM-LOCO2) THE USER'S RULE (2026-11-06: "ALL forms, when transformed into them but not in combat, need to keep using
/// the combat animations of the form until they end"): the Idle and Run states' battle flag is forced on while ANY form is
/// held (`Form::cmd` != 0, from StartStyle until EndStyle clears it), not only in KH3's four ([`keeps_battle_stance`], which
/// this contains). So the form's battle set (IdleB / IdleLockon, WalkLoopB, RunStartB / RunLoopB / RunEnd*B, LandB, the
/// form's replacement anim sets over them: [`resolve`]) plays in and out of combat, the form weapon stays drawn, and only the
/// form's end lets Idle's `BattleEnd` / Run's `RunBattleEnd` and the battle linger run again. KH3's own rule differs for the
/// other forms only in Idle after a battle: there the clip end plays Sheathe + `BattleEnd` (0x1413944D0); in Run KH3 already
/// keeps a form's set (the switch is gated to weapon kinds 1 / 0x16 / 0x17, vt+0x28B0, Run Tick 0x1413AED70), and a form
/// weapon out keeps `battleAnim` (0x141435A10: `actions::battle_anim`).
///
/// The Second Form family (0x94 ..= 0x98) is included ([`SECOND_FORM_HOLDS_BATTLE_STANCE`], approved by the coordinator
/// 2026-11-06: "ALL forms"); its golden trace was re-blessed for it (build/sora_tmp/weapons/FORM-LOCO2/STATE.md).
pub fn holds_battle_stance(cmd: i32) -> bool {
    cmd != 0 && (SECOND_FORM_HOLDS_BATTLE_STANCE || !super::form::is_second_form(cmd))
}

/// (FORM-LOCO2) Second Form under [`holds_battle_stance`]: on (the user's "ALL forms", approved 2026-11-06).
pub const SECOND_FORM_HOLDS_BATTLE_STANCE: bool = true;

/// (AUDIT-A SHIELD) The effects KH3 spawns in the forms of the Keyblade `key` (an item key, `WEP_KEYBLADE_SO_10`) that are
/// named after ANOTHER Keyblade's models: every AttachEffect row of group 0 / -1 in the clips of the forms' replacement anim
/// sets (their group-0 plays spawn them: kh-core notify.rs, KH3's group rule, attach component `Set` 0x141770D30). Frying
/// Pan plays Counter Shield's shared clips with group 0 for 0x51 (B412: `vw_so032_tuk0`, PlayAttack 0x1413378B0 sets
/// +0x198 only for 0x50 / 0x53 .. 0x57) and the guard's GuardChange / GuardChange2 (Guard slot 118 0x14139D2D0 plays group
/// 0: `vw_so033_start0`, `vw_so034_start1`): KH3 draws them. The hosts' other-keyblade name rule (kh-host vfx.rs) lets
/// these through (kh-er sora/kbdraw.rs, kh-host audit_a.rs). Lower case base names, sorted, no duplicates.
pub fn shared_clip_effects(key: &str) -> Vec<String> {
    let mut out = Vec::new();
    for row in super::form::KEYBLADE_FORMS.iter().filter(|r| r.keyblade == key) {
        // the finisher pawns' own effects (Pegasus's rush hit is Shooting Star's vw_so020_hit0)
        out.extend(shield::pawn_effects(row.cmd).into_iter().map(str::to_lowercase));
        // (AUDIT-B W-FIN) Blizzard Blades' FINISH_SK: its state-event camera's ceiling look spawns Ever After's
        // vw_so073_fin_fsg0 (fz_key @7411, kh-core states/bp_event.rs)
        if claws::form_key(row.cmd) == Some("SK") {
            out.push(crate::states::bp_event::CEILING_PARTICLE.to_string());
        }
        for set in chain(row.cmd) {
            for &(_, id) in set.rows {
                for &(_, index) in id.data().samples {
                    for n in crate::anim::SEQUENCES[index].notifies {
                        if let crate::anim::NotifyData::Effect { entries } = n.data {
                            for e in entries.iter().filter(|e| matches!(e.group_id, 0 | -1)) {
                                if let Some(name) = e.effect.and_then(|p| p.rsplit(['.', '/']).next()) {
                                    out.push(name.to_lowercase());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::form::KEYBLADE_FORMS;

    // The sheet's `moveset` column and the registry agree: "second_form" rows are the framework's own, "pending" rows
    // have no package, every other row names the package that lists it.
    #[test]
    fn the_sheet_and_the_registry_agree_on_which_forms_have_a_moveset() {
        for row in KEYBLADE_FORMS {
            match (row.moveset, of_form(row.id)) {
                ("pending", None) => {}
                (name, Some(m)) => assert_eq!(name, m.name, "{}: the sheet names {name}, the registry {}", row.id, m.name),
                (name, None) => panic!("{}: the sheet names the moveset {name}, which is not in forms::MOVESETS", row.id),
            }
            assert_eq!(moveset_name(row.id), row.moveset);
        }
        assert_eq!(moveset_name("form.NONE"), "-");
        for m in MOVESETS {
            for f in m.finishes {
                assert!(KEYBLADE_FORMS.iter().any(|r| m.forms.contains(&r.id) && (r.finish == *f || r.finish_levels.contains(f) || r.end_delete.contains(f) /* (F-RAGE) Rage Form's RISKCHARGE: a finish its EndStyle deletes */)), "{}: finish {f} is not one of its forms'", m.name);
            }
        }
    }

    #[test]
    fn the_none_moveset_changes_nothing() {
        let m = FormMoveset::NONE;
        assert_eq!(((m.replace_clip)("StyleChange"), m.forms.len(), m.finishes.len()), (None, 0, 0));
        assert_eq!(clip(0x94, "M_1421"), "M_1421", "Second Form replaces no clip");
    }

    // (F-LOCO) Every form's m_ReplaceAnimSets are in the generated table, the chain is searched last set first, and the
    // resolution is idempotent (a replacement row is never a logical name the same chain replaces again).
    #[test]
    fn every_form_resolves_through_its_anim_set_chain() {
        use crate::anim::{AnimId, FORM_ANIM_SETS};
        for row in KEYBLADE_FORMS {
            assert_eq!(chain(row.cmd).count(), row.anim_sets.len(), "{}: a set of {:?} is not in anim::FORM_ANIM_SETS", row.id, row.anim_sets);
            for set in chain(row.cmd) {
                for &(logical, _) in set.rows {
                    let once = replaced(row.cmd, logical).expect("a name of the chain is replaced");
                    assert_eq!(resolve(row.cmd, once), once, "{}: {logical} -> {} is replaced again", row.id, once.name());
                }
            }
        }
        // Hunny Blasters (0x8E): AnimSet16 over AnimSet2 (its BarrierCracker), the rest from AnimSet2.
        let set16 = FORM_ANIM_SETS.iter().find(|s| s.name == "p_ex001_AnimSet16").expect("AnimSet16");
        let cracker = set16.rows.iter().find(|r| r.0 == "BarrierCracker").expect("BarrierCracker in AnimSet16").1;
        assert_eq!(replaced(0x8E, "BarrierCracker"), Some(cracker));
        assert_eq!(replaced_clip(0x8E, "IdleB"), Some("2015"));
        assert_eq!(replaced(0x85, "RunLoopB"), AnimId::from_name("RunLoopB7"), "Blizzard Blades skate");
        assert!(replaced(0x85, "RunLoopB").is_some_and(|r| r.data().blend_space));
        assert_eq!((replaced(0, "IdleB"), replaced(0x94, "IdleB")), (None, None), "no form, Second Form: the base set");
        assert_eq!(resolve(0x8B, AnimId::IdleB).name(), "D015", "Rage Form's idle");
        assert!([0x85, 0x87, 0x88, 0x8B].iter().all(|&c| keeps_battle_stance(c)));
        assert!(![0x84, 0x86, 0x89, 0x8A, 0x8C, 0x94].iter().any(|&c| keeps_battle_stance(c)));
    }
}
