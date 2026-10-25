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

use super::situation;
use super::style::kind_of;
use super::tables::{command_row, FORM_ABILITIES, P};
use crate::enums::ETresAbilityKind;
use crate::host::time::MASK_WORLD;
use crate::host::{Attach, EffectHandle, EffectId, EffectSpawn, EventKind};
use crate::notify::NOTIFY_ID_NONE;
use crate::pawn::Ctx;
use crate::states::{StateData, StateId};

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
    /// The charge effect on the weapon while the form lasts (vfx spec 2.8).
    charge_effect: Option<EffectHandle>,
    /// Forms begun / ended so far (for tests and the debug text).
    pub begun: u32,
    pub ended: u32,
}

/// The Second Form family: `CHANGE_L1` ... `CHANGE_L5` (0x94 ..= 0x98).
pub fn is_second_form(cmd: i32) -> bool {
    (0x94..=0x98).contains(&cmd)
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
/// the previous form's, the host swaps the textures (the event), the weapon's charge effect
/// follows (vfx spec 2.8: level > 0 = group 3001 `vs_com_lv2` on the weapon and the burst group
/// 3002 `vs_com_kyp01` at `vfx_top`; who raises the level in the game was not traced: here it
/// is raised with the form and dropped at its end).
pub(crate) fn apply_wear_form(cx: &mut Ctx, form: u8) {
    if cx.pawn.combat.form.wear_form == form {
        return;
    }
    cx.pawn.combat.form.wear_form = form;
    let list: Vec<(ETresAbilityKind, u8)> = if form == P.form_wear_form as u8 {
        FORM_ABILITIES.iter().filter_map(|row| ETresAbilityKind::from_value(row.ability_id).map(|kind| (kind, row.copies as u8))).collect()
    } else {
        Vec::new()
    };
    cx.pawn.status.abilities.set_form(&list);
    cx.pawn.event(EventKind::WearForm { form });
    if let Some(handle) = cx.pawn.combat.form.charge_effect.take() {
        cx.stop_effect(handle, false);
    }
    if form != 0 {
        let weapon = Attach::Sora { bone: Some("R_wep_skt"), offset: glam::Vec3::ZERO };
        let handle = cx.spawn_effect(EffectSpawn { id: EffectId(NOTIFY_ID_NONE), name: "vs_com_lv2", at: weapon, rotation: glam::Vec3::ZERO, scale: 1.0, persistent: true });
        cx.pawn.combat.form.charge_effect = Some(handle);
        cx.spawn_effect(EffectSpawn { id: EffectId(NOTIFY_ID_NONE), name: "vs_com_kyp01", at: Attach::Sora { bone: Some("vfx_top"), offset: glam::Vec3::ZERO }, rotation: glam::Vec3::ZERO, scale: 1.0, persistent: false });
    }
}

/// The state gate of `CanStyleChange` (0x141425510, jump table 0x1414256E0): every state passes
/// - ATTACK, DAMAGE, GUARD and DODGEROLL included, without waiting for `TresEnableChange` -
/// except the listed ones.
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
    if matches!(cmd, 0xAB..=0xAE) {
        // ---- the finish branch (0x14141AAAF; F 10.6)
        // CanStyleFinish (pawn slot 1344): the chain of CanStyleChange, form time left > 0, the
        // command's form family.
        let f = cx.pawn.combat.form;
        if !can_style_change(state, cx) || !(f.time_left > 0.0) || !is_second_form(f.cmd) || cmd == 0xAE {
            return false;
        }
        // TryStyleFinish (iface slot 192 0x141447840): refused when the state is already 33.
        if state.id() == StateId::Stylefinish {
            return false;
        }
        cx.pawn.queue.set_state(crate::states::style_finish::StyleFinish::new(cmd));
        // "decInterval = 0 and form_time_left = 0": the form ends when the finish state ends.
        cx.pawn.combat.style.clear();
        cx.pawn.combat.form.time_left = 0.0;
        return true;
    }
    // ---- the formchange branch (F 10.3)
    if cx.pawn.combat.form.style_fix || !can_style_change(state, cx) {
        return false;
    }
    // The reaction prompt and the just-guard timer are dropped (0x1414145C0(ctrl, 0, ...)).
    crate::actions::close_counter_window(cx.pawn);
    cx.pawn.queue.clear();
    // TryStyleChange (iface slot 190 0x141445940) = PushState(31).
    cx.pawn.queue.push_state(crate::states::style_change::StyleChange::RESET);
    let row = command_row(cmd);
    let duration = row.map(|r| r.param3).filter(|d| *d > 0.0).unwrap_or(P.form_duration_default);
    {
        let f = &mut cx.pawn.combat.form;
        f.cmd = cmd;
        f.level = row.map_or(P.form_level as i32, |r| r.param1 as i32);
        // The clock starts here, at the press.
        (f.time_full, f.time_left) = (duration, duration);
        (f.award_pending, f.finish_pending) = (false, false);
        // 0x1405B9230(pawn, 1, 1): bad statuses cured and blocked.
        f.bad_status_block = true;
        f.begun += 1;
    }
    cx.pawn.combat.style.clear();
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
        3 => P.form_finish_cmd_lv3 as i32,
        _ => 0,
    };
}

/// `SetStyleFinishCommand(bLevelUp, bWeaponChange)` (0x1413EE030), forms 0x94 .. 0x98: gives the
/// finish command of the next level and stretches the form's time. Only acts while a form is
/// active.
pub(crate) fn set_style_finish_command(cx: &mut Ctx, level_up: bool, weapon_change: bool) -> bool {
    let f = cx.pawn.combat.form;
    if !is_second_form(f.cmd) {
        return false;
    }
    let mut level = f.finish_level;
    if (level_up || level == 0) && level < P.form_finish_level_max as i32 {
        level += 1;
    }
    set_finish_level(cx, level);
    let cmd = cx.pawn.combat.form.finish_cmd;
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
    }
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
