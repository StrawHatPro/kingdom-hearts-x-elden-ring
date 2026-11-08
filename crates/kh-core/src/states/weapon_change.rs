//! (weapons P2, 2026-11-03) `TresPlayerState_WeaponChange` (id 32): KH3's Keyblade switch, with its gate and request
//! (documentation/KH3_WEAPONS_PLAN.md 1.3; the Keyblade data: status/keyblade.rs). Read in decompiled C (re/decomp),
//! the vtables from base/KH3.exe:
//!
//! | what | address |
//! |---|---|
//! | class vtable (ctor 0x1413C9D80) | 0x14694E320: slot 95 Reset 0x1413D10E0, 96 Begin 0x14041F120 (the empty base), 97 Tick 0x1413D9E50, 98 End 0x1413CF7D0 |
//! | `CanWeaponChange` (pawn vt+0x1368) | 0x1414300E0 (request-interface slot 231 0x141430080 calls it) |
//! | the request (request-interface slot 232, "TryWeaponChange") | 0x1414426E0 |
//! | next / previous slot request (slots 234 / 235) | 0x141446590 / 0x141446640, slot search 0x141782070 / 0x1417820E0 |
//! | `ChangeWeapon(item, form, grow, ..)` (pawn vt+0x13C8) | 0x14144A9F0 |
//! | the form's suspend / resume around it (controller vt+0x1AE0 -> 0x14141D9D0, vt+0x1AE8 -> 0x14141E2B0) | |
//! | the pawn tick part (carry timer, glide-resume bit) | 0x14144A530 |
//!
//! THE STATE: Reset clears the weapon (+0x88) and the combo-carry flag (+0x8C). Begin is empty. Tick sub 0 changes the
//! weapon (`ChangeWeapon`; retried every tick until it succeeds), then: the carry timer pawn+0x10E4 = 0.5 s when +0x8C
//! is set and nothing is queued (pawn vt+0x1768), the glide-resume bit pawn+0x1460 bit 0 is set, `Draw(1, 1, 1.0)`,
//! sub = 1. Tick sub 1: the state's clip has ended or a state is queued -> finish (the state plays no clip of its own:
//! its anim proxy reports "ended" at once, so it lasts two ticks unless a request comes first). End: the same carry
//! timer rule. Body animation, movement and gravity are left as they were: the switch has no clip in KH3.
//!
//! THE COMBO ACROSS A SWITCH: requested out of an attack the switch is queued behind it (PushState) and the combo goes
//! on when a hit landed (pawn+0xE28, Combo Master sets it), else the combo count pawn+0xE18 is set to 0. The attack's
//! End hands its counters to the pawn instead of ending the chain (attack.rs, queued 32); an attack out of the switch
//! (TryAttack case 32) or out of Idle / Run / Jump / Glide while the 0.5 s carry timer runs continues the combo.
//!
//! THE FORM ACROSS A SWITCH (`ChangeWeapon` + 0x14141D9D0 / 0x14141E2B0): a formchange belongs to its Keyblade SLOT.
//! The held form's time left, finish level and weapon form are stored in the old slot, the form is taken off (finish
//! commands deleted, wear form NORMAL, HUD gauge gone, the bad-status block off; the formchange gauge itself is the
//! controller's and stays), and the NEW slot's stored form, if any, comes back with its stored time and finish level,
//! its wear form, and its finish command offered again with at least `MyStyleFinishOffsetTimeWeaponChange` (1.0 s:
//! `SetStyleFinishCommand(0, bWeaponChange = 1)`, controller vt+0x1A28). Switching is refused during Rage Form
//! (`CHANGE_AN` 0x8B, controller+0xB38) and while the form's time is up (controller+0xB78 bit 3). The store and the
//! restore are done here with the form framework's own calls (combat/form.rs `end_style`, `apply_wear_form`,
//! `set_style_finish_command`); the per-form extras of 0x14141E2B0 (Hyper Hammer / Drill Punch gauges, Wheel of Fate's
//! effects ...) belong to the form packages (build/sora_tmp/weapons/P2/STATE.md).
//!
//! NOT PORTED (no counterpart in the port, said in STATE.md): the code-menu option A7 that drops the slot's form on a
//! switch (0x1405AD320(pawn, 7)), the weapon restriction bit pawn+0xE60 bit 15, the swim test (vt+0x1C30), the pawn
//! tests vt+0x29C0 other than the states 61 / 62, controller+0xA08 bit 0, pawn+0xE60 bit 19 (stored per slot, meaning
//! unknown), controller vt+0x1A40 / 0x1A68 / 0x1710 / 0x18B8 of the suspend.

use super::{PlayerState, StateData, StateId};
use crate::combat::form;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;
use crate::status::keyblade::SlotForm;

/// pawn+0x10E4: the carry timer a switch out of a hit combo starts (pawn vt+0x1768(0.5f)).
pub(crate) const COMBO_CARRY_TIME: f32 = 0.5;
/// `CHANGE_AN` (Rage Form): no switch while it is held (`CanWeaponChange`: controller+0xB38 == 0x8B).
const RAGE_FORM: i32 = 0x8B;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WeaponChange {
    sub: u8,
    /// +0x88: the weapon to change to: here the Keyblade slot it is in (KH3 keeps the item id and finds its slot).
    pub slot: u8,
    /// +0x8C: the switch was asked for out of an attack chain with a hit landed: the combo carries over.
    pub carry: bool,
    /// The switch is done (sub 0 succeeded).
    pub changed: bool,
}

impl WeaponChange {
    /// Reset 0x1413D10E0.
    pub const RESET: WeaponChange = WeaponChange { sub: 0, slot: 0, carry: false, changed: false };
}

/// The attack-class state ids of `CanWeaponChange` and the request (12 .. 14, 75 .. 90, 156, 157, 159 .. 161).
fn attack_class(id: StateId) -> bool {
    matches!(id.value(), 12..=14 | 75..=90 | 156 | 157 | 159..=161)
}

/// `CanWeaponChange` (pawn vt+0x1368, 0x1414300E0) with request-interface slot 233 (0x1414300A0): two Keyblades or
/// more, nothing queued, not dying, the state rows, the form rows, the command in action.
pub(crate) fn can_weapon_change(state: &StateData, cx: &Ctx) -> bool {
    let s = &*cx.pawn;
    // slot 233 (0x14177BFE0): at least two filled weapon slots.
    if s.status.keyblades().filled() < 2 {
        return false;
    }
    // vt+0x2540: DIE (59) current or next, or dead.
    if s.status.hp < 1 || state.id() == StateId::Die || s.queue.queued_id() == Some(StateId::Die) {
        return false;
    }
    // FUN_1405AB6C0: nothing queued.
    if !s.queue.nothing_queued() {
        return false;
    }
    // vt+0x29C0: not CATCH / BIND (61 / 62), current or next.
    if matches!(state.id().value(), 61 | 62) || s.queue.queued_id().is_some_and(|id| matches!(id.value(), 61 | 62)) {
        return false;
    }
    // controller+0xB38 != CHANGE_AN, controller+0xB78 bit 3 (the form's time is up) clear.
    let f = &s.combat.form;
    if f.cmd == RAGE_FORM || f.time_up {
        return false;
    }
    let id = state.id();
    let by_state = match id.value() {
        // 3 JUMP: not the flow jump (+0xF8), not sub-steps 7, 8, 9.
        3 => match state {
            StateData::Jump(j) => !j.flow && !matches!(j.sub, 7..=9),
            _ => false,
        },
        6..=11 | 15..=17 | 19..=28 | 30..=42 | 61 | 62 | 66 | 73 | 74 | 92 | 95..=97 | 101 | 103 | 110 | 112 | 114 | 115 | 117 | 119..=124 | 126..=138 | 142 | 144 | 145 | 150 | 153 | 162..=167 => false,
        // The attacks: no combo step queued (+0x143), not a finisher's end (pawn+0xE48 bit 5), no unison bit (+0x1EC
        // bit 7: the port keeps the pending unison on the pawn).
        _ if attack_class(id) => state.attack().is_some_and(|a| !a.next_queued) && !s.combat.finish_end && s.combat.friend.unison == 0,
        // 18 FIREMAGIC: no follow-up queued (+0xD8 bit 4), not a finisher's end; with a magic command in action (the
        // usual case) also a shot has left (+0xD0 > 0): `shot_left_unchained` covers both.
        18 => matches!(state, StateData::FireMagic(m) if m.shot_left_unchained()) && !s.combat.finish_end,
        _ => true,
    };
    // The command in action (pawn+0xE2C): 0x4B and 0xC0 refuse; the default row: pawn+0xE60 bit 17 (the command
    // latch) clear.
    by_state && !matches!(s.combat.form.command_in_action, 0x4B | 0xC0) && !s.magic.command_latch
}

/// THE REQUEST (request-interface slot 232, 0x1414426E0), for the Keyblade in `slot` (the slot search is the caller's:
/// status/keyblade.rs `request_switch`). Out of an attack: Combo Master marks a hit, the carry flag = a hit landed (else
/// the combo count goes to 0), `PushState(32)`; out of anything else: `SetState(32)`. The unison request is cleared
/// (pawn vt+0x28B8 0x141431A80). Returns whether the switch was queued.
pub(crate) fn try_weapon_change(state: &StateData, cx: &mut Ctx, slot: usize) -> bool {
    if !can_weapon_change(state, cx) {
        return false;
    }
    let s = &mut *cx.pawn;
    let keyblades = s.status.keyblades();
    // The weapon is the current one, or no weapon: refused.
    if keyblades.slot(slot).is_none() || slot == keyblades.current_slot() {
        return false;
    }
    let id = state.id();
    if matches!(id.value(), 22 | 40 | 41 | 42 | 96 | 97 | 103 | 119 | 138) {
        return false;
    }
    let next = WeaponChange { slot: slot as u8, ..WeaponChange::RESET };
    if attack_class(id) || id == StateId::Firemagic {
        // HasAbility(COMBO_MASTER 30): pawn vt+0x1D80(1) = the hit flag pawn+0xE28.
        if s.status.has(crate::enums::ETresAbilityKind::ComboMaster) {
            s.combat.hit_flag = true;
        }
        let carry = s.combat.hit_flag;
        if !carry {
            // pawn vt+0x1D50(0): pawn+0xE18 = 0.
            s.combat.combo = 0;
        }
        s.queue.push_state(WeaponChange { carry, ..next });
    } else {
        s.queue.set_state(next);
    }
    s.combat.friend.unison = 0;
    true
}

/// The part of the pawn tick 0x14144A530: outside IDLE / RUN / JUMP (1 .. 3) and WEAPONCHANGE the glide-resume bit and
/// the carry timer are cleared; else the timer counts down, and when it runs out `0x14140DF30(ctrl, 0)`: the sneeze latch
/// is dropped. (StartStyle's own weapon change, `pawn.vt+0x13C8(want, ..)` inside 0x14141A9D0, is the form framework's:
/// combat/form.rs `take_form_keyblade`.)
pub(crate) fn pawn_tick(state: &StateData, cx: &mut Ctx, dt: f32) {
    // (P2 hitfx) the weapon model the hits come from (its sound sets): the held form's hand unit, else the held Keyblade.
    let units = cx.pawn.combat.form.weapon_units();
    let weapons = crate::status::keyblade::attack_weapons(&units, cx.pawn.status.current_keyblade());
    cx.pawn.status.attack_model = weapons.first().map_or("w_so010", |bp| crate::status::keyblade::model_of(bp));
    // (AUDIT-C TF) the left hand's weapon (pawn vt+0x1428(2), LH_WEAPON): the held form's LEFT_HAND unit, else none
    cx.pawn.status.left_attack_model = units.iter().copied().find(|u| u.contains("@LEFT_HAND:")).map_or("", |u| crate::status::keyblade::model_of(crate::status::keyblade::blueprint_of(u)));
    let s = &mut *cx.pawn;
    let id = state.id().value();
    if id > 3 && id != StateId::Weaponchange.value() {
        s.status.glide_resume = false;
        s.magic.combo_keep = 0.0;
        return;
    }
    if s.magic.combo_keep > 0.0 {
        s.magic.combo_keep -= dt;
        if s.magic.combo_keep < 0.0 {
            s.magic.combo_keep = 0.0;
            s.combat.sneeze_next = false;
        }
    }
}

/// The glide-resume bit pawn+0x1460 bit 0 (set by this state's tick), taken: Idle / Glide Begin read and clear it.
pub(crate) fn take_glide_resume(s: &mut Pawn) -> bool {
    std::mem::take(&mut s.status.glide_resume)
}

/// The store of `ChangeWeapon` and the suspend 0x14141D9D0: the held form (if any) as the old slot keeps it, and the
/// form taken off. The formchange gauge is the controller's: it stays as it was.
fn suspend_form(cx: &mut Ctx) -> Option<SlotForm> {
    let f = cx.pawn.combat.form;
    if f.cmd == 0 {
        return None;
    }
    let record = SlotForm { cmd: f.cmd, level: f.level, time_left: f.time_left, time_full: f.time_full, finish_level: f.finish_level, finish_cmd: f.finish_cmd, grow: f.grow, wear_form: f.wear_form };
    let gauge = cx.pawn.combat.style;
    form::end_style(cx, false, false);
    cx.pawn.combat.style = gauge;
    Some(record)
}

/// The restore of `ChangeWeapon` (controller vt+0x1AA0 / 0x1AA8: time left and finish level of the new slot) and the
/// resume 0x14141E2B0: the form command and level of the slot's weapon form, the bad-status block (0x1405B9230(pawn,
/// 1, 1)), the wear form, the HUD gauge (0x14157AFE0), "finish given" cleared (+0xB5D), then `SetStyleFinishCommand(0,
/// 1)`: the finish command comes back with at least 1.0 s.
fn resume_form(cx: &mut Ctx, record: SlotForm) {
    {
        let f = &mut cx.pawn.combat.form;
        (f.cmd, f.level, f.time_left, f.time_full) = (record.cmd, record.level, record.time_left, record.time_full);
        (f.finish_level, f.finish_cmd, f.grow) = (record.finish_level, record.finish_cmd, record.grow);
        (f.finish_pending, f.award_pending, f.time_up) = (false, false, false);
        f.bad_status_block = true;
        f.begun += 1;
    }
    form::apply_wear_form(cx, record.wear_form);
    // (F-ULTIMATE, F-POLISH) the resume's per-form switch: the soft-target range of every form that changes it.
    resume_switch(cx, record.cmd);
    cx.pawn.event(crate::host::EventKind::FormBegin { kind: crate::combat::style::kind_of(record.cmd), duration: record.time_full });
    form::set_style_finish_command(cx, false, true);
}

/// (F-POLISH) The resume 0x14141E2B0's second switch (by the resumed command, after the bad-status block and the time):
/// the soft-target range controller+0x970 = MyAutoLockonLimit (pawn vt+0x2980) x 2.0 for 0x80 / 0x8E (after the multi-lock
/// start, slot 790 vt+0x18B0), 0x87, 0x88 (Mirage Staff falls into 0x87's), 0x8B and 0x8C; x 3.0 for 0x83 / 0x8F (after
/// slot 791 vt+0x18B8 and slot 840 vt+0x1A40). Not here: FUN_14140BE40 (the arrowguns' HUD call), the asset preloads
/// pawn vt+0xFB0 (0x88, 0x8D, 0x93), the pawn flag 0x40000 (0x81 0x82 0x84 0x8A 0x8C 0x8D: no reader traced); Mirage Staff's
/// camera +0x804 = 1.35 is read from the held form (`staff::camera_distance_scale`).
fn resume_switch(cx: &mut Ctx, cmd: i32) {
    use crate::combat::forms::arrowgun::{aim, CHANGE_BZ, CHANGE_BZ2, CHANGE_DW, CHANGE_DW2};
    use crate::combat::forms::{ship::CHANGE_HL, staff::CHANGE_WA, ultimate::CHANGE_SW};
    let scale = match cmd {
        CHANGE_DW | CHANGE_DW2 => {
            aim::multilock_start(cx);
            2.0
        }
        CHANGE_BZ | CHANGE_BZ2 => {
            aim::multilock_end(cx);
            aim::shooter_end(cx);
            3.0
        }
        CHANGE_SW | CHANGE_WA | form::CHANGE_AN | CHANGE_HL => 2.0,
        _ => return,
    };
    cx.pawn.combat.lockon.soft_range = crate::combat::targeting::AUTO_LIMIT * scale;
}

/// `ChangeWeapon` (pawn vt+0x13C8 0x14144A9F0) as this state calls it: the form of the old slot stored and taken off,
/// the Keyblade of `slot` made current (status: stats, abilities; the disappear / appear effects while the weapon is
/// out), the new slot's form back. False when the slot holds no Keyblade (the state retries).
fn change_weapon(cx: &mut Ctx, slot: usize) -> bool {
    if cx.pawn.status.keyblades().slot(slot).is_none() {
        return false;
    }
    let old_slot = cx.pawn.status.keyblades().current_slot();
    let stored = suspend_form(cx);
    cx.pawn.status.store_slot_form(old_slot, stored);
    let Some(switch) = cx.pawn.status.change_keyblade_slot(slot) else { return false };
    if cx.pawn.actions.weapon_out {
        crate::status::keyblade::spawn_switch_effects(cx, &switch);
    }
    if let Some(record) = cx.pawn.status.take_slot_form(slot) {
        resume_form(cx, record);
    }
    true
}

impl PlayerState for WeaponChange {
    const ID: StateId = StateId::Weaponchange;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin: the base's, empty (vtable slot 96 0x14041F120).
    fn begin(&mut self, _cx: &mut Ctx) {}

    /// Tick 0x1413D9E50.
    fn tick(&mut self, cx: &mut Ctx, _dt: f32) {
        if self.sub == 0 {
            // controller vt+0x1AE0 / ChangeWeapon / controller vt+0x1AE8: `change_weapon`; a failed change is retried.
            if !change_weapon(cx, self.slot as usize) {
                return;
            }
            self.changed = true;
            if self.carry && cx.pawn.queue.nothing_queued() {
                cx.pawn.magic.combo_keep = COMBO_CARRY_TIME;
            }
            cx.pawn.status.glide_resume = true;
            // pawn vt+0x1340(1, 1, 1.0): Draw.
            crate::actions::draw(cx, true);
            self.sub = 1;
        } else {
            // "IsAnimEnd(0) || a state is queued": the state's anim proxy has played nothing, so its clip counts as
            // ended on the first sub-1 tick.
            cx.pawn.queue.do_finish();
        }
    }

    /// End 0x1413CF7D0.
    fn end(&mut self, cx: &mut Ctx) {
        if self.carry && cx.pawn.queue.nothing_queued() {
            cx.pawn.magic.combo_keep = COMBO_CARRY_TIME;
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
