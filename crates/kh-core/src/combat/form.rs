//! Second Form's bookkeeping on the controller: combat strand F 10.3 - 10.7
//! (`build/sora_tmp/combat/P2_F_section.md`): `StartStyle` (controller slot 831 0x14141A9D0) with
//! its gates `CanStyleChange` / `CanStyleFinish` (pawn slots 1343 / 1344), `SetStyleFinishCommand`
//! (slot 837 0x1413EE030), `EndStyle` (slot 832 0x14141CA20), `CanEndStyle` (slot 857
//! 0x141401D10), the wear form with its abilities (`p_ex001_FormAbility`), and the world slow.
//!
//! The states are `states/style_change.rs` (31), `states/combo_limit.rs` (90) and
//! `states/style_finish.rs` (33); the gauge and the form's clock are `style.rs`.
//!
//! Read in decompiled C only and implemented as read (marked [C] where they are): the chain of
//! `CanStyleChange`'s pawn tests, `EndStyle`'s pose part (the `BattleStart` replay in Idle is
//! not done), what `ctrl.slot840()` clears of the lock-on (nothing is cleared here).
//!
//! (weapons P3) THE FORM FRAMEWORK (documentation/KH3_WEAPONS_PLAN.md 1.3): every formchange of KH3 is a row
//! `form.<code>` of `sheets/port/sora-keyblade-forms.json` ([`KEYBLADE_FORMS`], `tools/sora_keyblade_forms.js`), and the
//! functions here and in `style.rs` / `states/style_change.rs` / `states/style_finish.rs` read the held Keyblade's forms
//! from it, as KH3's own switches do (AwardAtMax 0x1413F0ED0 by weapon type and form level, StartStyle 0x14141A9D0,
//! SetStyleFinishCommand 0x1413EE030, EndStyle 0x14141CA20 by form command [C]). Second Form (`form.L1`) takes exactly
//! the path it took before (the golden trace, tests/form_framework.rs); the form packages' hooks are `combat/forms`.
//! THE PENDING RULE is [`ready`]. The held Keyblade comes from [`equipped_keyblade`] (the interface to P2's Keyblade
//! slots, build/sora_tmp/weapons/P3/STATE.md).

use super::situation;
use super::style::kind_of;
use super::tables::{command_row, FORM_ABILITIES, P};
use crate::enums::ETresAbilityKind;
use crate::host::time::MASK_WORLD;
use crate::host::{EffectHandle, EventKind}; // (HAIR-PULSE) Attach / EffectId / EffectSpawn / NOTIFY_ID_NONE went with the charge glow
use crate::pawn::Ctx;
use crate::states::{StateData, StateId};
use crate::status::Status;

// (weapons P3) the rows (sheets/port/sora-keyblade-forms.json): `KeybladeFormRow`, `KEYBLADE_FORMS`, `KBFORM_*`,
// `keyblade_forms_by_id`.
include!(concat!(env!("KH_GEN_DIR"), "/sora_keyblade_forms.rs"));

/// `CHANGE_AN`: Rage Form (wear form 7 "Anti"; offered by an HP rule, not by the gauge: its row's `trigger`).
pub const CHANGE_AN: i32 = 0x8B;
/// `CHANGE_L2`: Second Form S (Starlight), whose finish level 3 is `LM_UNIONDISCODE` (SetFinishLevel 0x141414C70).
pub const CHANGE_L2: i32 = 0x95;
/// `LM_UNIONDISCODE` (Ancient Light) as Second Form S's finish level 3.
const LM_UNIONDISCODE: i32 = 0xAE;
/// `RISKCHARGE`: Rage Form's finish command while fewer than three were used (its row's note).
const RISKCHARGE: i32 = 0xB3;
/// The Kingdom Key: the Keyblade of an empty or unknown `Status::weapon` (weapon type 1, as every pawn was before the
/// Keyblades: the port's only weapon).
pub const KINGDOM_KEY: &str = "WEP_KEYBLADE_SO_00";

/// controller +0xB34 ... +0xB78 and what hangs on them.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Form {
    /// +0xB38: the form's command (0x94 in Second Form), 0 = no form.
    pub cmd: i32,
    /// +0xB34: the form level (`TresCmdData[cmd].Param1`).
    pub level: i32,
    /// +0xB54 / +0xB58: form time left / full.
    pub time_left: f32,
    pub time_full: f32,
    /// +0xB64 / +0xB68: the finish level 0..3 and its command (171 / 172 / 173).
    pub finish_level: i32,
    pub finish_cmd: i32,
    /// +0xB5C: a formchange command is pending in the queue; +0xB5D: a finish command was given.
    pub award_pending: bool,
    pub finish_pending: bool,
    /// +0xB60: "style fix" (`DebugSetStyleFix`): no command is enqueued, the form does not end.
    pub style_fix: bool,
    /// +0xB78 bit 0 (`BP_NotifyPauseSituationTimer`) and bit 3 ("time up", raised every frame
    /// the form's time is out but it cannot end yet).
    pub paused: bool,
    pub time_up: bool,
    /// controller+0x8F0: the time the latest finish command was given with.
    pub last_finish_time: f32,
    /// The wear form applied (`ETresCharWearForm`: 0 NORMAL, 5 `TRES_WEAR_FORM_05`).
    pub wear_form: u8,
    /// pawn+0x634 bit 0: bad statuses are cured and blocked.
    pub bad_status_block: bool,
    /// pawn+0xE2C: the "command in action" (set by the executor, cleared by the state's end event).
    pub command_in_action: i32,
    /// controller+0x878 bit 3: the world is slowed (`WorldSlow`).
    pub world_slow: bool,
    /// (AUDIT-A ARROWGUN) controller+0x878 bit 2: the DARK SCREEN of a command is on ([`dark_screen`], 0x14140DF80).
    pub dark_screen: bool,
    /// The charge effect on the weapon while the form lasts (vfx spec 2.8).
    charge_effect: Option<EffectHandle>,
    /// Forms begun / ended so far (for tests and the debug text).
    pub begun: u32,
    pub ended: u32,
    /// (CHESTFIX) PORT ADDITION: times the net gave a form's look and abilities back that no form held (style.rs `tick`).
    pub nets: u32,
    // ---- (weapons P3) the form framework
    /// The grow level of the form's weapon (`ETresWeaponGrowLvl` 1 ..= the row's `grow_levels`; 0 = no form): which
    /// weapon parts are drawn ([`Form::weapon_units`]). Set to 1 when a form starts; what raises it in KH3 (Counter
    /// Shield, Frying Pan) was not traced: the form's package does.
    pub grow: u8,
    /// StartStyle's weapon changes so far (`pawn.vt+0x13C8(want, ...)`: the form's Keyblade was in another slot and
    /// became the current one; for tests and the debug text).
    pub weapon_changes: u32,
    /// DEBUG / TESTS: every form counts as READY (the framework's generic behaviour with no moveset): see [`ready`].
    pub debug_ready: bool,
    /// (P3) DEBUG / TESTS: every form but the Second Form family counts as PENDING (the pending rule tested however many
    /// form packages have landed); `debug_ready` wins over it.
    pub debug_pending: bool,
    /// (F-RAGE) Rage Form's counts, cooldown and aura (pawn+0xE40 / +0xE58 / +0xE5C: combat/forms/rage.rs).
    pub rage: super::forms::rage::RageForm,
    /// (AUDIT-B W-CORE) What MyEquipment holds of the held form now ([`FormEquip`]): hosts draw exactly that.
    pub equip: FormEquip,
}

/// (AUDIT-B W-CORE) THE FORM'S EQUIPMENT AS KH3 SWAPS IT (UTresEquipmentComponent 0x141791120, the equip of a weapon form:
/// `if bTransform && set.m_TransformSetData.Num >= 1: +0x2C8 |= 4, units = set + 0x18 (m_TransformSetData) else units = set +
/// 0x08` (the form's own units), each equip REPLACING the units before it (vt+0x410 -> vt+0x610)):
/// - StartStyle 0x14141A9D0 only switches the Keyblade (pawn vt+0x13C8(weapon type, form 0, ...): the form's Keyblade in its
///   base form);
/// - StyleChange Tick 0x1413D3AC0 sub 0 (the forms of their own clip, `StyleChange2` .. `StyleChange21`): pawn vt+0x13E0(weapon
///   form, +0x99, bTransform = 1) -> [`FormEquip::Transform`];
/// - Light / Dark / Double Form (+0xA0 bit 6) at sub 3 (and End when left at sub 2 / 3): vt+0x13E0(.., bTransform = 0) ->
///   [`FormEquip::Weapons`]; the Second Form family (+0xA0 bit 1) equips nothing in StyleChange;
/// - StyleChange End 0x1413CE760 (every case but 0x94 .. 0x98, 0x99 / 0x9A, 0xDD .. 0xE0): pawn vt+0x1408 = equipment slot 164
///   0x14177ED40: "if +0x2C8 & 4: clear it, units = set + 0x08" -> [`FormEquip::Weapons`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FormEquip {
    /// The form's own units (m_Equips by grow level) in place of the Keyblade: the state outside StyleChange.
    #[default]
    Weapons,
    /// The transform units (m_TransformSetData) ALONE, when the form's set has any (else its own units, as KH3's Num test).
    Transform,
    /// Nothing of the form is equipped yet: the Keyblade in his hand (StyleChange before its equip).
    Keyblade,
}

impl Form {
    /// The row of the held form (None = no form).
    pub fn row(&self) -> Option<&'static KeybladeFormRow> {
        row_of(self.cmd)
    }

    /// The weapon parts of the held form at its grow level (`unit@part:socket=blueprint`, the row's `units` /
    /// `grow_units`): what the renderer attaches instead of the Keyblade. Empty = no form.
    pub fn weapon_units(&self) -> Vec<&'static str> {
        let Some(row) = self.row() else { return Vec::new() };
        let prefix = format!("G{}:", self.grow);
        let grown: Vec<&'static str> = row.grow_units.iter().filter_map(|u| u.strip_prefix(prefix.as_str())).collect();
        if self.grow > 1 && !grown.is_empty() { grown } else { row.units.to_vec() }
    }

    /// The blueprint of one of the held form's projectile kinds by its name (`DW_NORMAL`, `BZ_BOMBSHOT1` ...: the row's
    /// `projectiles`, `name=blueprint`).
    pub fn projectile(&self, name: &str) -> Option<&'static str> {
        self.row()?.projectiles.iter().find_map(|p| p.split_once('=').filter(|(n, _)| *n == name).map(|(_, bp)| bp))
    }

    /// The held form's transformation clip (`StyleChange`, `StyleChange2` ...).
    pub fn transform_clip(&self) -> Option<&'static str> {
        self.row().map(|r| r.transform_clip)
    }
}

/// The Second Form family: `CHANGE_L1` ... `CHANGE_L5` (0x94 ..= 0x98).
pub fn is_second_form(cmd: i32) -> bool {
    (0x94..=0x98).contains(&cmd)
}

/// (weapons P3) The row of a formchange command (`CHANGE_*`).
pub fn row_of(cmd: i32) -> Option<&'static KeybladeFormRow> {
    if cmd == 0 {
        return None;
    }
    KEYBLADE_FORMS.iter().find(|r| r.cmd == cmd)
}

/// (weapons P3) THE HELD KEYBLADE (the Keyblade side, P2: `Status::current_keyblade`, the active slot): its item key
/// (`WEP_KEYBLADE_SO_nn`). None held, or a key that is not a Keyblade = the Kingdom Key.
pub fn equipped_keyblade(status: &Status) -> &'static str {
    status.current_keyblade().map_or(KINGDOM_KEY, |id| id.key())
}

/// (weapons P3) The form's Keyblade is held, or in one of Sora's slots (P2, `status/keyblade.rs`).
fn keyblade_at_hand(status: &Status, key: &str) -> bool {
    equipped_keyblade(status) == key || crate::status::KeybladeId::from_key(key).is_some_and(|id| status.keyblades().slot_of(id).is_some())
}

/// (weapons P3) StartStyle's weapon change (0x14141A9D0: `if pawn weapon type != want: pawn.vt+0x13C8(want, ...)`): the
/// form's Keyblade becomes the current one when it is in another slot (the status is rebuilt: its stats and abilities).
fn take_form_keyblade(cx: &mut Ctx, key: &str) {
    if equipped_keyblade(&cx.pawn.status) == key {
        return;
    }
    let Some(id) = crate::status::KeybladeId::from_key(key) else { return };
    let mut slots = *cx.pawn.status.keyblades();
    if slots.slot_of(id).and_then(|slot| slots.set_current(slot).ok()).is_some() {
        cx.pawn.status.set_keyblades(slots);
        cx.pawn.combat.form.weapon_changes += 1;
    }
}

/// (weapons P3) KH3's weapon type of the held Keyblade (pawn slot 601 -> equipment vt+0x428 = its `ETresItemDefWeapon`
/// value, the switch AwardAtMax and StartStyle make): the rows' `weapon_type`. A key no row names (not a Keyblade) is the
/// Kingdom Key's 1.
pub fn weapon_type(status: &Status) -> u32 {
    let key = equipped_keyblade(status);
    KEYBLADE_FORMS.iter().find(|r| r.keyblade == key).map_or(1, |r| r.weapon_type)
}

/// (weapons P3) AwardAtMax's offer table (0x1413F0ED0, the switch on the weapon type): the form a full gauge offers to
/// weapon type `weapon_type` at form level `level` (0 = no form: the first stage; 1 = in a first stage: its second
/// stage), ready or not.
pub fn offered(weapon_type: u32, level: i32) -> Option<&'static KeybladeFormRow> {
    KEYBLADE_FORMS.iter().find(|r| r.weapon_type != 0 && r.weapon_type == weapon_type && r.stage == level + 1)
}

/// (weapons P3) THE PENDING RULE. A form is READY when it has a moveset in `combat::forms::MOVESETS` (the framework's
/// own Second Form: `form.L1`, `form.L2`, `form.L5`; a form package's), or any form while [`Form::debug_ready`] is set.
/// A PENDING form:
/// - is never offered: its full gauge is consumed exactly as KH3 consumes it (the grand-magic roll included), but no
///   formchange command enters the queue, so the Y slot shows nothing new and the HUD gauge simply empties;
/// - is refused by the executor when its command is used anyway (a debug enqueue): the entry stays and runs out;
/// - as a SECOND stage it is not offered inside its (ready) first stage, whose gauge then gives nothing;
/// - its finish command (a finish body nobody implements: [`finisher_ready`]) is never offered: the form then has no
///   finish command, and Second Form S (`form.L2`) would hold its finish level at 2 without a `LM_UNIONDISCODE` body
///   (F-SECOND: it has one, STYLEFINISH slot 155, so its level 3 is Ancient Light).
///
/// Rage Form (`form.AN`) is pending too: its HP rule (the row's `trigger`) is F-RAGE's to run.
pub fn ready(form: &Form, row: &KeybladeFormRow) -> bool {
    // (P3) the debug aid `debug_pending` (tests of the pending rule).
    form.debug_ready || (!(form.debug_pending && !is_second_form(row.cmd)) && super::forms::of_form(row.id).is_some())
}

/// (weapons P3) A finish command has a body: Second Form's three (STYLEFINISH 0xAB .. 0xAD), a form package's, or any
/// while [`Form::debug_ready`] is set (then STYLEFINISH ends at once and the form with it).
pub fn finisher_ready(form: &Form, cmd: i32) -> bool {
    form.debug_ready || super::forms::of_finish(cmd).is_some()
}

/// (weapons P3) A finish command of some form (a row's `finish` or `finish_levels`).
pub fn is_finish_kind(kind: i32) -> bool {
    kind != 0 && KEYBLADE_FORMS.iter().any(|r| r.finish == kind || r.finish_levels.contains(&kind))
}

/// (weapons P3) The finish command a full gauge gives in a form (SetStyleFinishCommand's switch: the row's `finish`, or
/// the form package's choice).
fn finish_of(cx: &Ctx, row: &'static KeybladeFormRow) -> i32 {
    match super::forms::of_form(row.id) {
        Some(m) => (m.finish_cmd)(cx, row),
        None => row.finish,
    }
}

/// (weapons P3) The finish command of the held form, 0 = none (the Second Form family: its finish level's).
pub(crate) fn current_finish(cx: &Ctx) -> i32 {
    let f = &cx.pawn.combat.form;
    if is_second_form(f.cmd) {
        return f.finish_cmd;
    }
    row_of(f.cmd).map_or(0, |row| finish_of(cx, row))
}

/// (weapons P3) AwardAtMax step 3's offer (0x1413F0ED0): nothing inside the Second Form family or Rage Form; else the
/// held Keyblade's form for the current form level, when it is READY.
pub(crate) fn offer_now(cx: &Ctx) -> Option<i32> {
    let f = &cx.pawn.combat.form;
    if is_second_form(f.cmd) || f.cmd == CHANGE_AN {
        return None;
    }
    offered(weapon_type(&cx.pawn.status), f.level).filter(|row| ready(f, row)).map(|row| row.cmd)
}

/// (weapons P3) The abilities a wear form adds (p_ex001_FormAbility, the rows' `abilities`; a repeat is another copy).
fn wear_form_abilities(form: u8) -> Vec<(ETresAbilityKind, u8)> {
    let mut list: Vec<(ETresAbilityKind, u8)> = Vec::new();
    let Some(row) = KEYBLADE_FORMS.iter().find(|r| r.wear_form == u32::from(form)) else { return list };
    for kind in row.abilities.iter().filter_map(|name| ETresAbilityKind::from_game_name(name)) {
        match list.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, copies)) => *copies += 1,
            None => list.push((kind, 1)),
        }
    }
    list
}

// (HAIR-PULSE) `weapon_socket` (the RWeapon unit's socket, where the core put the charge glow) went with the glow: KH3 never
// raises the form charge level (build/sora_tmp/weapons/HAIR-PULSE/STATE.md section 6).

/// (AUDIT-A ARROWGUN) ATresPlayerPawnBase::MyDarkScreenRate (pawn+0x1558; the ctor 0x141423210 sets 0.5, no blueprint of
/// Sora's overrides it): the master light coefficient of a command's dark screen.
pub const DARK_SCREEN_RATE: f32 = 0.5;
/// (AUDIT-A ARROWGUN) The dark screen's transition time, s (the begin / end events pass 1.0 as its last argument).
pub const DARK_SCREEN_TIME: f32 = 1.0;

/// (AUDIT-A ARROWGUN) `DarkScreen(ctrl, on, rate, time)` (0x14140DF80), called by the command begin event (0x14150A1F0:
/// (1, 1.0, 1.0)) and the end event (0x14150C440: (0, 1.0, 1.0)) of every command state, so every finish: pawn vt+0x8F0
/// gives the ATresSceneManager and the call is its SetMasterLightCoefficient(InTransitionTime = time, InCoefficient =
/// (1 - rate) x ctrl+0x830's +0x4A4 (0 without that object) + rate with rate 1.0 meaning MyDarkScreenRate, clamped 0 .. 1,
/// InPriority 1, InChannel 2, IsEject 0) = 0x1414F7B60 (the native of execSetMasterLightCoefficient 0x142A25C10); off (only
/// while bit 2 is set): SetMasterLightCoefficientDefault(time, priority 1, channel 2, eject 1) = 0x1414F7C50; then bit 2 =
/// on. The scene's lights go to x 0.5 over 1 s while the finish runs and back over 1 s after it. The hosts read
/// `Form::dark_screen` with [`DARK_SCREEN_RATE`] / [`DARK_SCREEN_TIME`] (INT line: kh-er's scene light).
pub(crate) fn dark_screen(cx: &mut Ctx, on: bool) {
    cx.pawn.combat.form.dark_screen = on;
}

/// `WorldSlow(ctrl, on)` (0x14140E500): object types mask 0x419C at rate 0.1.
pub(crate) fn world_slow(cx: &mut Ctx, on: bool) {
    if cx.pawn.combat.form.world_slow == on {
        return;
    }
    cx.pawn.combat.form.world_slow = on;
    cx.set_time_dilation(MASK_WORLD, if on { P.form_world_slow_rate } else { 1.0 });
}

/// The wear form is applied (pawn vt+0x1498 -> equipment vt+0x4F8): its ability list replaces
/// the previous form's, the host swaps the textures (the event).
/// (HAIR-PULSE) NO CHARGE GLOW: KH3 spawns the form's weapons with charge level -1 (StartStyle -> pawn vt+0x13C8 ->
/// ATresCharPawnBase 0x1405BD620: MyEquipment vt+0x4D8(.., 0xffffffff, ..) -> 0x14177C4A0 -> 0x141791120 -> the weapon
/// spawn 0x14178E5D0 stores it at weapon+0x66C; the Appear end 0x14177E720 plays FUN_141792220(weapon, -1) = nothing), and
/// the only setter of a real level (pawn slots 655 / 656 -> the equipment interface +0x1E0, TRES_CMN_CEC_STYLECHANGE ->
/// 0x14178A480) has no caller; its debug UFUNCTION's controller slot is an empty stub. So no form shows `vs_com_lv2`
/// (group 3001), `vs_com_lv1` (3000) or the burst `vs_com_kyp01` (3002): they were spawned here for every form
/// (build/sora_tmp/weapons/HAIR-PULSE/STATE.md section 6). `Form::charge_effect` stays None.
pub(crate) fn apply_wear_form(cx: &mut Ctx, form: u8) {
    if cx.pawn.combat.form.wear_form == form {
        return;
    }
    cx.pawn.combat.form.wear_form = form;
    let list: Vec<(ETresAbilityKind, u8)> = if form == P.form_wear_form as u8 {
        FORM_ABILITIES.iter().filter_map(|row| ETresAbilityKind::from_value(row.ability_id).map(|kind| (kind, row.copies as u8))).collect()
    } else if form != 0 {
        // (weapons P3) the other wear forms' lists (p_ex001_FormAbility, the rows' `abilities`).
        wear_form_abilities(form)
    } else {
        Vec::new()
    };
    cx.pawn.status.abilities.set_form(&list);
    cx.pawn.event(EventKind::WearForm { form });
    if let Some(handle) = cx.pawn.combat.form.charge_effect.take() {
        cx.stop_effect(handle, false);
    }
    // (HAIR-PULSE) the spawn of `vs_com_lv2` (on the weapon socket) and `vs_com_kyp01` (at vfx_top) that stood here is gone:
    // KH3 never raises the form charge level (see above)
}

/// The state gate of `CanStyleChange` (0x141425510, jump table 0x1414256E0): every state passes
/// - ATTACK, DAMAGE, GUARD and DODGEROLL included, without waiting for `TresEnableChange` -
///   except the listed ones.
pub(crate) fn state_gate(state: &StateData, cx: &Ctx) -> bool {
    match state.id() {
        StateId::Dangle | StateId::Shootermode | StateId::Shootflow | StateId::Athleticflow | StateId::Gigas | StateId::SpecialShip | StateId::Smallairplane => false,
        // FIREMAGIC 18 only when `!(+0xD8 & 0x10) && +0xD0 > 0` (no follow-up queued and the
        // shot has left: `FireMagic::shot_left_unchained`, exposed by the magic implementer),
        // USE_ITEM 111 only with `m_bEnableChange`.
        StateId::Firemagic => matches!(state, StateData::FireMagic(m) if m.shot_left_unchained()),
        StateId::UseItem => cx.pawn.queue.flags.enable_change,
        _ => true,
    }
}

/// `CanStyleChange(cmd)` (pawn slot 1343 0x14142DEF0) [C for the chain]: no DIE state current or
/// queued, no state queued, the state gate. (The restriction bits and the swim test have no
/// counterpart here.)
fn can_style_change(state: &StateData, cx: &Ctx) -> bool {
    if cx.pawn.status.hp < 1 || state.id() == StateId::Die || cx.pawn.queue.queued_id() == Some(StateId::Die) {
        return false;
    }
    if !cx.pawn.queue.nothing_queued() {
        return false;
    }
    // pawn+0xE2C not in {0x4B, 0xC0}.
    if matches!(cx.pawn.combat.form.command_in_action, 0x4B | 0xC0) {
        return false;
    }
    state_gate(state, cx)
}

/// `StartStyle()` (0x14141A9D0), both branches: the formchange and the finish command.
pub(crate) fn start_style(state: &mut StateData, cx: &mut Ctx, cmd: i32) -> bool {
    // "swimming || !(ctrl+0x878 & 2) || ctrl+0x8C8 > 0": the command cool-down.
    if cx.pawn.combat.command_cooldown > 0.0 {
        return false;
    }
    // (weapons P3) the finish kinds of StartStyle's first switch (cmd - 0x6D: 0, 0x2F .. 0x3C, 0x3E .. 0x41, 0x74, 0x75,
    // 0x77; 0x46 = RISKCHARGE has a branch of its own, with Rage Form: here with the others, F-RAGE's body).
    if matches!(cmd, 0x6D | 0x9C..=0xA9 | 0xAB..=0xAE | RISKCHARGE | 0xE1 | 0xE2 | 0xE4) {
        // ---- the finish branch (0x14141AAAF; F 10.6)
        // CanStyleFinish (pawn slot 1344): the chain of CanStyleChange, form time left > 0, the
        // command's form family.
        let f = cx.pawn.combat.form;
        // (weapons P3) the family: 0xAB .. 0xAD the Second Form family's, 0xAE Second Form S's; every other one the held
        // form's own finish command; and a body for it (the pending rule).
        let family = match cmd {
            0xAB..=0xAD => is_second_form(f.cmd),
            LM_UNIONDISCODE => f.cmd == CHANGE_L2 && finisher_ready(&f, cmd),
            _ => !is_second_form(f.cmd) && f.cmd != 0 && current_finish(cx) == cmd && finisher_ready(&f, cmd),
        };
        if !can_style_change(state, cx) || f.time_left.partial_cmp(&0.0).is_none_or(std::cmp::Ordering::is_le) || !family {
            return false;
        }
        // TryStyleFinish (iface slot 192 0x141447840): refused when the state is already 33.
        if state.id() == StateId::Stylefinish {
            return false;
        }
        // (F-SHIELD) TryStyleFinish 0x141447840: the class by command (0xA3 FINISH_SH -> STYLEFINISH_SH 38).
        if cmd == super::forms::shield::FINISH_SH {
            cx.pawn.queue.set_state(crate::states::combo_sh::StyleFinishSh::new(crate::states::style_finish::StyleFinish::new(cmd)));
        } else {
            cx.pawn.queue.set_state(crate::states::style_finish::StyleFinish::new(cmd));
        }
        // (F-RAGE) StartStyle case 0x46: RISKCHARGE halves HP and keeps the form's time and the gauge (combat/forms/rage.rs).
        if cmd == RISKCHARGE { super::forms::rage::riskcharge_taken(cx); return true; }
        // "decInterval = 0 and form_time_left = 0": the form ends when the finish state ends.
        cx.pawn.combat.style.clear();
        cx.pawn.combat.form.time_left = 0.0;
        return true;
    }
    // ---- the formchange branch (F 10.3)
    if cx.pawn.combat.form.style_fix || !can_style_change(state, cx) {
        return false;
    }
    // (weapons P3) THE PENDING RULE (`ready`): a command without a form row (CHANGE_L3 / L4: no Keyblade item on PC;
    // the friends' 0x99 / 0x9A) or of a form without a moveset is refused: the entry stays and runs out.
    let Some(form_row) = row_of(cmd) else { return false };
    if !ready(&cx.pawn.combat.form, form_row) {
        return false;
    }
    // (weapons P3) a form of a Keyblade in none of Sora's slots is refused (its command came from a Keyblade since
    // taken off: KH3 cannot offer that).
    if cmd != CHANGE_AN && !keyblade_at_hand(&cx.pawn.status, form_row.keyblade) {
        return false;
    }
    // The reaction prompt and the just-guard timer are dropped (0x1414145C0(ctrl, 0, ...)).
    crate::actions::close_counter_window(cx.pawn);
    cx.pawn.queue.clear();
    // (weapons P3) Rage Form: `slot859(0, 0)` first, the held form ends (no weapon change); every other form: the
    // weapon type it needs.
    if cmd == CHANGE_AN {
        if cx.pawn.combat.form.cmd != 0 {
            finish_ended(cx);
        }
    } else {
        take_form_keyblade(cx, form_row.keyblade);
    }
    // TryStyleChange (iface slot 190 0x141445940) = PushState(31).
    cx.pawn.queue.push_state(crate::states::style_change::StyleChange::RESET);
    let row = command_row(cmd);
    // (weapons P3) a command sora-combat-form-commands does not carry: Param3 / Param1 from its form row.
    let duration = row.map_or(form_row.time, |r| r.param3).max(0.0);
    let duration = if duration > 0.0 { duration } else { P.form_duration_default };
    {
        let f = &mut cx.pawn.combat.form;
        f.cmd = cmd;
        f.level = row.map_or(form_row.stage, |r| r.param1 as i32);
        // The clock starts here, at the press.
        (f.time_full, f.time_left) = (duration, duration);
        (f.award_pending, f.finish_pending) = (false, false);
        // 0x1405B9230(pawn, 1, 1): bad statuses cured and blocked.
        f.bad_status_block = true;
        f.begun += 1;
        // (weapons P3) the weapon's first grow level.
        f.grow = 1;
    }
    cx.pawn.anim.form_cmd = cmd; // (F-SHIELD) the anim set chain
    cx.pawn.combat.style.clear();
    // (F-POLISH) StartStyle 0x14141A9D0's common part (before its form switch): controller slot 738 (vt+0x1710 0x141410FD0) resets the soft-target
    // range +0x970 to MyAutoLockonLimit for every form before the per-form switch (a second stage of a form that doubled it).
    cx.pawn.combat.lockon.soft_range = crate::combat::targeting::AUTO_LIMIT;
    // (weapons P3) StartStyle's per-form tail (the row's `start_extra`): the form package's.
    if let Some(m) = super::forms::of_form(form_row.id) {
        (m.on_start)(cx, form_row);
    }
    // The finish commands of every form are deleted.
    for kind in [0x9D, 0xA1, 0xA2, 0xA3, 0x9E, 0x9C, 0xA6, 0x9F, 0xA0, 0xA7, 0xA8, 0xA9, 0xE4, 0xAB, 0xAC, 0xAD, 0xAE, 0xE1, 0xE2] {
        situation::delete(cx, kind);
    }
    cx.pawn.event(EventKind::FormBegin { kind: kind_of(cmd), duration });
    true
}

/// `SetFinishLevel(lv)` (controller slot 853 0x141414C70).
fn set_finish_level(cx: &mut Ctx, level: i32) {
    let f = &mut cx.pawn.combat.form;
    f.finish_level = level.min(P.form_finish_level_max as i32);
    f.finish_cmd = match f.finish_level {
        1 => P.form_finish_cmd_lv1 as i32,
        2 => P.form_finish_cmd_lv2 as i32,
        // (weapons P3) "0xAE in form 0x95" (Second Form S: Ancient Light).
        3 if f.cmd == CHANGE_L2 => LM_UNIONDISCODE,
        3 => P.form_finish_cmd_lv3 as i32,
        _ => 0,
    };
}

/// `SetStyleFinishCommand(bLevelUp, bWeaponChange)` (0x1413EE030): gives the held form's finish
/// command (forms 0x94 .. 0x98: the one of the next level) and stretches the form's time. Only
/// acts while a form is active.
pub(crate) fn set_style_finish_command(cx: &mut Ctx, level_up: bool, weapon_change: bool) -> bool {
    let f = cx.pawn.combat.form;
    let cmd = if is_second_form(f.cmd) {
        let mut level = f.finish_level;
        if (level_up || level == 0) && level < P.form_finish_level_max as i32 {
            level += 1;
        }
        // (weapons P3) the pending rule: Second Form S's level 3 (LM_UNIONDISCODE) without a body stays at level 2.
        if level == 3 && f.cmd == CHANGE_L2 && !finisher_ready(&f, LM_UNIONDISCODE) {
            level = 2;
        }
        set_finish_level(cx, level);
        cx.pawn.combat.form.finish_cmd
    } else {
        // (weapons P3) every other form: its one finish command (the switch: the row's `finish`), when it has a body.
        // The first stages have none: their full gauge offers the second stage instead (style.rs).
        let Some(row) = row_of(f.cmd) else { return false };
        let kind = finish_of(cx, row);
        if !finisher_ready(&f, kind) {
            return false;
        }
        kind
    };
    if cmd == 0 {
        return false;
    }
    let f = &mut cx.pawn.combat.form;
    let t = if !f.finish_pending {
        // The first time in this form.
        let offset = if weapon_change { P.form_finish_offset_time_weapon_change } else { P.form_finish_offset_time };
        if f.time_left < offset {
            f.time_left = offset;
        }
        f.time_left
    } else {
        // A further level: +3 s, never above the full time.
        f.time_left = (f.time_left + P.form_finish_offset_time).min(f.time_full);
        f.time_left
    };
    f.last_finish_time = t;
    f.finish_pending = true;
    situation::enqueue(cx, cmd, t);
    true
}

/// `CanEndStyle()` (0x141401D10): false while the queued next or the current state is an attack
/// state, GUARD, DODGEROLL, FIREMAGIC, AIR_SLIDE, STYLECHANGE, STYLEFINISH_*, COMBO_*.
pub(crate) fn can_end_style(state: &StateData, cx: &Ctx) -> bool {
    let blocked = |id: StateId| matches!(id.value(), 12 | 13 | 14 | 15 | 16 | 18 | 20 | 31 | 33..=39 | 75..=90 | 156 | 157 | 159 | 160 | 161);
    !blocked(state.id()) && !cx.pawn.queue.queued_id().is_some_and(blocked)
}

/// `EndStyle(bKeepPose, bDeleteGrand)` (0x14141CA20).
pub(crate) fn end_style(cx: &mut Ctx, keep_pose: bool, delete_grand: bool) {
    let cmd = cx.pawn.combat.form.cmd;
    crate::actions::close_counter_window(cx.pawn);
    if is_second_form(cmd) {
        for kind in [0xAB, 0xAC, 0xAD, 0xAE] {
            situation::delete(cx, kind);
        }
        set_finish_level(cx, 0);
    } else if let Some(row) = row_of(cmd) {
        // (weapons P3) every other form: its package's part, then the switch's deletes (the row's `end_delete`: its
        // second stage's offer, or its finish commands).
        if let Some(m) = super::forms::of_form(row.id) {
            (m.on_end)(cx, row);
        }
        for &kind in row.end_delete {
            situation::delete(cx, kind);
        }
    }
    if delete_grand {
        situation::delete(cx, 0x7E);
        situation::delete(cx, 0xE7);
    }
    {
        let f = &mut cx.pawn.combat.form;
        (f.time_left, f.level, f.cmd, f.style_fix) = (0.0, 0, 0, false);
        (f.finish_pending, f.award_pending, f.time_up) = (false, false, false);
        // 0x1405B9230(pawn, 0, 1): the bad-status block is off.
        f.bad_status_block = false;
        f.ended += 1;
        f.grow = 0;
    }
    cx.pawn.anim.form_cmd = 0; // (F-SHIELD) the anim set chain
    cx.pawn.combat.style.clear();
    if cmd != 0 {
        cx.pawn.event(EventKind::FormEnd { kind: kind_of(cmd) });
    }
    if !keep_pose {
        // pawn.vt+0x13E0(1, 0, 0, 0.2): equipment back to the base variant: wear form NORMAL,
        // the form's abilities removed [C]. (In IDLE the game replays "BattleStart": the stance
        // code is the actions implementer's.)
        apply_wear_form(cx, 0);
    }
}

/// (vfx implementer 3) The two `EndEffect` calls at the end of a finish command (StyleFinish
/// `End` 0x1413CEBD0 and the Last Arcanum tail 0x1413E1E9D; combat spec 10.9 item 4 [I]):
/// - `(pawn+0x598)+0x200 .vt+0x1A8(-1, 1)` = `EndEffect(every group)` on the weapons' attach
///   components (vfx spec 4.2): everything attached to the Keyblade ends by its end type. In the
///   core that is the Second Form charge effect (`vs_com_lv2`, group 3001), which otherwise
///   lasted until the wear form went back (and for ever with a fixed style);
/// - `pawn.vt+0x23C8()` 0x1405A8610 = `EndEffect(group, 0)` of the pawn's common aura slot
///   (group pawn+0x900, chosen per state by 0x141442930 [C]): the core spawns no aura effect,
///   so there is nothing of the pawn's to end here.
pub(crate) fn end_finish_effects(cx: &mut Ctx) {
    if let Some(handle) = cx.pawn.combat.form.charge_effect.take() {
        cx.stop_effect(handle, false);
    }
}

/// `ctrl.slot859(0, 0)` (the StyleFinish end event): ends the form unless the style is fixed.
pub(crate) fn finish_ended(cx: &mut Ctx) {
    if cx.pawn.combat.form.style_fix {
        cx.pawn.combat.form.finish_pending = false;
    } else {
        end_style(cx, false, false);
    }
}

// (weapons P3) the form framework's data against the constants Second Form ran on, and the pending rule at the executor.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::{AnimId, AnimPlayer};
    use crate::cmc::Cmc;
    use crate::host::NullHost;
    use crate::params::SORA_PARAMS;
    use crate::pawn::Pawn;
    use crate::test_world::TestWorld;
    use glam::Vec3;

    fn pawn(weapon: &str) -> Pawn {
        let mut p = Pawn::new(SORA_PARAMS, Cmc::new(&SORA_PARAMS, Vec3::ZERO), 0.0, AnimPlayer::new(AnimId::IdleN));
        p.status.weapon = weapon.to_string();
        p.status.hp = 100;
        p
    }

    // The Second Form row is the data of the hard-wired path: command, level, time, lifetime, wear form, its abilities,
    // the finish levels, the weapon socket.
    #[test]
    fn the_second_form_row_is_what_second_form_ran_on() {
        let row = keyblade_forms_by_id("form.L1").unwrap();
        let cmd = command_row(row.cmd).unwrap();
        assert_eq!((row.cmd, row.stage, row.weapon_type, row.keyblade), (super::super::style::CHANGE_L1, P.form_level, 1, KINGDOM_KEY));
        assert_eq!((row.time, row.command_time, row.wear_form), (cmd.param3, super::super::style::CHANGE_L1_LIFETIME, P.form_wear_form as u32));
        assert_eq!(row.finish_levels, [P.form_finish_cmd_lv1 as i32, P.form_finish_cmd_lv2 as i32, P.form_finish_cmd_lv3 as i32]);
        let sheet: Vec<_> = FORM_ABILITIES.iter().map(|r| r.ability).collect();
        assert_eq!(row.abilities, sheet.as_slice(), "p_ex001_FormAbility[TRES_WEAR_FORM_05]");
        // (HAIR-PULSE) was `weapon_socket(row.cmd)`: the RWeapon unit's socket read from the row itself
        assert!(row.units.iter().any(|u| u.starts_with("RWeapon@RIGHT_HAND:R_wep_skt=")), "{:?}", row.units);
        assert_eq!((row.wear_at, row.moveset), ("style_change", "second_form"));
        assert_eq!(keyblade_forms_by_id("form.L2").unwrap().finish_levels[2], LM_UNIONDISCODE);
    }

    // AwardAtMax's chains (0x1413F0ED0): the first stage by weapon type, the second inside it, none after it.
    #[test]
    fn the_offer_table_by_weapon_type_and_form_level() {
        let chain = |wt: u32| (offered(wt, 0).map(|r| r.command), offered(wt, 1).map(|r| r.command), offered(wt, 2).map(|r| r.command));
        assert_eq!(chain(1), (Some("CHANGE_L1"), None, None));
        assert_eq!(chain(3), (Some("CHANGE_DW"), Some("CHANGE_BZ"), None), "Shooting Star");
        assert_eq!(chain(8), (Some("CHANGE_DW2"), Some("CHANGE_BZ2"), None), "Hunny Spout");
        assert_eq!(chain(13), (Some("CHANGE_LF"), Some("CHANGE_TF"), None), "Oathkeeper");
        assert_eq!(chain(18), (Some("CHANGE_L2"), None, None), "Starlight");
        assert_eq!(chain(19), (Some("CHANGE_L5"), None, None), "Dead of Night");
        assert_eq!((weapon_type(&pawn("").status), weapon_type(&pawn("WEP_KEYBLADE_SO_02").status), weapon_type(&pawn("NO_SUCH").status)), (1, 3, 1));
        for row in KEYBLADE_FORMS {
            assert_eq!(row_of(row.cmd).map(|r| r.id), Some(row.id), "one row per command");
            if row.stage == 2 {
                assert!(row.finish != 0 && row.first != "-", "{}: a second stage has a finish command and a first stage", row.id);
            }
        }
        // the code, not the names: Agile Claws has no finish command (it leads to Twin Yo-Yos), Frying Pan has FINISH_SH.
        assert_eq!((keyblade_forms_by_id("form.CL").unwrap().finish, keyblade_forms_by_id("form.SH2").unwrap().finish), (0, 0xA3));
        // Rage Form: no Keyblade, wear form 7, offered by HP, not by the gauge.
        let an = keyblade_forms_by_id("form.AN").unwrap();
        assert_eq!((an.cmd, an.weapon_type, an.wear_form, an.transform_clip, an.moveset), (CHANGE_AN, 0, 7, "StyleChange13", "rage")); // (F-RAGE) its package
        assert_eq!(wear_form_abilities(7), [(ETresAbilityKind::ComboMaster, 1)]);
        assert!(wear_form_abilities(4).contains(&(ETresAbilityKind::Airslide, 3)), "a repeat is another copy");
    }

    // THE PENDING RULE at the executor: a pending form's command is refused (the entry stays); with debug_ready it runs.
    #[test]
    fn a_pending_form_is_refused_by_the_executor_and_never_offered() {
        let mut p = pawn("WEP_KEYBLADE_SO_02");
        let world = TestWorld::floor();
        let mut null = NullHost::default();
        let mut host = null.host(&world);
        let mut state = StateData::DEFAULT;
        {
            let mut cx = Ctx::new(&mut p, &mut host);
            // (P3) Double Arrowguns made PENDING by the debug aid (its package has landed: combat/forms/arrowgun.rs), so the
            // rule is tested however many form packages exist.
            cx.pawn.combat.form.debug_pending = true;
            assert_eq!(offer_now(&cx), None, "a pending form is not offered");
            assert!(situation::enqueue(&mut cx, 0x80, -1.0));
            assert!(!situation::execute(&mut state, &mut cx, 0x80), "refused");
            assert!(cx.pawn.combat.situation.has(0x80) && cx.pawn.combat.form.cmd == 0, "the entry stays, no form");
            cx.pawn.combat.form.debug_pending = false;
            assert!(!situation::execute(&mut state, &mut cx, 0x96), "CHANGE_L3: no row, refused");
            cx.pawn.combat.form.debug_ready = true;
            assert_eq!(offer_now(&cx), Some(0x80));
            assert!(situation::execute(&mut state, &mut cx, 0x80));
            let f = cx.pawn.combat.form;
            assert_eq!((f.cmd, f.level, f.time_full, f.grow, f.weapon_changes), (0x80, 1, 20.0, 1, 0));
        }
        // Another Keyblade held: the form's own Keyblade is taken from its slot (StartStyle's weapon change); in no slot,
        // the command is refused.
        let mut state = StateData::DEFAULT;
        let mut p = pawn(KINGDOM_KEY);
        p.combat.form.debug_ready = true;
        {
            let mut cx = Ctx::new(&mut p, &mut host);
            assert!(!situation::execute(&mut state, &mut cx, 0x80), "Shooting Star is in no slot");
        }
        let (slots, notes) = crate::status::Keyblades::from_keys(&[Some(KINGDOM_KEY), Some("WEP_KEYBLADE_SO_02")], 0, &[]);
        assert!(notes.is_empty(), "{notes:?}");
        p.status.set_keyblades(slots);
        p.status.hp = 100;
        let mut cx = Ctx::new(&mut p, &mut host);
        assert!(situation::execute(&mut state, &mut cx, 0x80));
        assert_eq!((equipped_keyblade(&cx.pawn.status), cx.pawn.combat.form.weapon_changes, cx.pawn.status.keyblades().current_slot()), ("WEP_KEYBLADE_SO_02", 1, 1));
    }

    // (P3) A reaction prompt of slot B (ex kinds 1 - 3, 5, 9, 13: here REVENGESLASH 3) over a form's finish command: the
    // finish stays "given" (+0xB5D) because AwardAtMax's HUD query still finds it (`Queue::hud_has`: KH3 keeps it in
    // hud+0x7D0 under the prompt's overlay), comes back to slot B with the prompt's end and is used.
    #[test]
    fn a_reaction_prompt_over_slot_b_keeps_the_finish() {
        let mut p = pawn("WEP_KEYBLADE_SO_08");
        p.combat.form.debug_ready = true;
        let world = TestWorld::floor();
        let mut null = NullHost::default();
        let mut host = null.host(&world);
        let mut state = StateData::DEFAULT;
        let mut cx = Ctx::new(&mut p, &mut host);
        // (no state machine runs here: the queue as it stands in IDLE is put back after each command)
        let idle = cx.pawn.queue.clone();
        assert!(situation::execute(&mut state, &mut cx, 0x86), "Nano Arms");
        cx.pawn.queue = idle.clone();
        assert!(set_style_finish_command(&mut cx, true, false), "the transformation's end gives FINISH_GM");
        assert_eq!(cx.pawn.combat.situation.slot_b.map(|e| e.kind), Some(0xA0));
        situation::set_reaction(&mut cx, 3, 2.0);
        let q = &cx.pawn.combat.situation;
        assert!(q.slot_b.is_some_and(|e| e.ex && e.kind == 3) && !q.has(0xA0) && q.hud_has(0xA0), "the prompt over the finish command");
        for _ in 0..10 {
            super::super::style::tick(&state, &mut cx, 1.0 / 60.0);
        }
        assert!(cx.pawn.combat.form.finish_pending, "AwardAtMax keeps the finish given under the prompt");
        situation::clear_reaction(&mut cx, 3);
        assert_eq!(cx.pawn.combat.situation.slot_b.map(|e| e.kind), Some(0xA0), "back in slot B");
        cx.pawn.queue = idle;
        assert!(situation::execute(&mut state, &mut cx, 0xA0), "the finish command is used");
    }

    // Starlight's Second Form S: finish level 3 is Ancient Light (0xAE). (F-SECOND) Its body (STYLEFINISH slot 155) has
    // landed, so the level-2 hold of the pending rule is lifted: level 3 is reached and stays (no level 4).
    #[test]
    fn second_form_s_reaches_ancient_light_at_finish_level_three() {
        let mut p = pawn("WEP_KEYBLADE_SO_17");
        let world = TestWorld::floor();
        let mut null = NullHost::default();
        let mut host = null.host(&world);
        let mut state = StateData::DEFAULT;
        let mut cx = Ctx::new(&mut p, &mut host);
        assert_eq!(offer_now(&cx), Some(CHANGE_L2), "Second Form S is Second Form's moveset: ready");
        assert!(finisher_ready(&cx.pawn.combat.form, LM_UNIONDISCODE), "slot 155 has a body");
        assert!(situation::execute(&mut state, &mut cx, CHANGE_L2));
        let mut levels = Vec::new();
        for _ in 0..4 {
            set_style_finish_command(&mut cx, true, false);
            levels.push((cx.pawn.combat.form.finish_level, cx.pawn.combat.form.finish_cmd));
        }
        assert_eq!(levels, [(1, 0xAB), (2, 0xAC), (3, LM_UNIONDISCODE), (3, LM_UNIONDISCODE)], "KH3: 0xAE in form 0x95");
        assert!(cx.pawn.combat.situation.hud_has(LM_UNIONDISCODE));
    }
}
