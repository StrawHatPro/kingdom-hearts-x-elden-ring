//! (weapons F-NANO) `TresPlayerState_ComboGM` (id 81): the attack state of Nano Arms (0x86) (pawn slot 1337 0x141432F00:
//! form 0x86 -> ComboGM). A subclass of `TresPlayerState_Attack` (attack.rs, which this wraps) with the vtable 0x14690EBF0;
//! it overrides (build/sora_tmp/weapons/F-HAMMER/tools/vt.js against the Attack's):
//!
//! | slot | address | what |
//! |---|---|---|
//! | 95 Reset | 0x141352E20 | Attack's Reset, then +0x250 = 2000.0 (no reader in the class) |
//! | 96 Begin / 97 Tick / 98 End | 0x141333480 / 0x1413417E0 / 0x141335DF0 | thunks to the Attack's (0x141333210 / 0x141341340 / 0x141335120) |
//! | 122 Decide | 0x141361870 | the combo: `combat::forms::nano::decide` |
//! | 136 | 0x141337890 | the warp point: the Attack's 0x141337320 with 100 (the hookshot: `nano::hook_warp`) |
//!
//! PlayAttack (slot 120 0x1413378B0) is the Attack class's: its cases 0xB5 .. 0xBD and the GM branch of 0x25 are the rows
//! `attack.*` of `sheets/port/sora-form-nano.json`; the base kinds (1, 2, 3, 8, 0x10 .. 0x12, 0x17) play the base rows
//! with AnimSet8's clips (m_ReplaceAnimSets: 1301 -> 8301 ...). The hookshot's pull is the on-contact handler 0x141355780
//! of the class's notify object; the shots (0xB9 BX_ROCKET, 0x25 BARRIERCRACKER) are the shot handler 0x141344580's cases.

use super::attack::Attack;
use super::style_finish::StyleFinish;
use super::common::{ground_probe, on_ground};
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, NotifyClass, NotifyPhase, Notifies, PlayArgs};
use crate::combat::forms::nano::{self as gm, GmDecideIn};
use crate::combat::tables::P;
use crate::enums::ETresAbilityKind;
use crate::host::TargetId;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComboGm {
    /// The Attack state's fields.
    pub(crate) a: Attack,
}

impl ComboGm {
    /// Reset (0x141352E20).
    pub(crate) fn new(a: Attack) -> ComboGm {
        ComboGm { a }
    }

    /// PlayAttack (slot 120 0x1413378B0) in Nano Arms.
    fn play(&mut self, cx: &mut Ctx) {
        let kind = self.a.kind;
        let row = if self.a.hop || self.a.slope { None } else { gm::attack_row(kind, self.a.air) };
        let Some(row) = row else {
            // The Attack class's own case (the base kinds, the forced flow / counter kinds, the hop / slope requests), its
            // clip replaced by AnimSet8 (the play goes through m_ReplaceAnimSets).
            self.a.play_attack(cx);
            if let Some(c) = crate::combat::tables::attack_kind(self.a.kind).and_then(|r| gm::replaced_clip(r.anim)).and_then(AnimId::from_name) {
                cx.pawn.anim.play_with(c, PlayArgs::mode(2, -1.0));
            }
            return;
        };
        let pawn = &mut *cx.pawn;
        pawn.combat.last_kind = kind;
        if pawn.combat.history.len() >= 16 {
            pawn.combat.history.remove(0);
        }
        pawn.combat.history.push(kind);
        let a = &mut self.a;
        let flag = |f: &str| row.flags.contains(&f);
        // The defaults (m_bFaceAt, the XY channel, stop 160, z gain 3), then the case's writes.
        a.face_at = true;
        (a.chan_xy, a.chan_root) = (row.chan_xy, row.chan_root);
        (a.stop_dist, a.z_gain, a.air_mode, a.keep_face) = (row.stop_dist, row.z_gain, row.air_mode, false);
        a.root_len = row.root_len;
        if flag("no_vz") {
            a.no_vz = true;
        }
        if flag("keep_dir_z") {
            a.keep_dir_z = true;
        }
        if flag("dive") {
            a.dive = true;
        }
        if flag("swing_started") {
            // 0x25: +0x164 = 1.
            a.swing_started = true;
        }
        // The tail (LAB_14133A87A): ground: gravity on, DirZ 0; air mode: +0x234 |= 4 | 0x10, GravityScale 0.1, Flying,
        // +0x18C = 0.6 (over the 0.3 of 0xB5).
        if !a.air_mode {
            pawn.cmc.gravity_off = false;
            a.dir_z = 0.0;
        } else {
            pawn.cmc.gravity_scale = P.air_gravity_scale;
            pawn.cmc.gravity_off = true;
            pawn.cmc.set_flying();
            a.hover = true;
            a.air_hold = P.air_hold_time;
        }
        // 0xBB: DirZ = -4000 with its dive (+0x1DE).
        let dir_z = row.values.first().copied().unwrap_or(0.0);
        if dir_z != 0.0 {
            a.dir_z = dir_z;
        }
        if flag("invincible") {
            // The tail's pawn vt+0x2068(1) (0x25): invincible until End (vt+0x2068(0)).
            pawn.status.damage_invalid = true;
        }
        // (AUDIT-B W-CORE) case 0x25 outside 0xDF / 0xE0: +0x234 |= 0x2800, 0x1405B8B30(pawn+0x460, 8) (attack.rs `counter_body`).
        if kind == gm::kind::COUNTER {
            a.counter_body(pawn);
        }
        // (AUDIT-B W-CORE) case 0xBA (M_8419): +0x234 |= 0x10000 (the traction's bit-16 root path: attack.rs `root_keep`).
        if kind == gm::kind::RISE {
            a.root_keep = true;
        }
        pawn.combat.swing_hit = false;
        pawn.root_scale = (1.0, 1.0);
        // Play(name, 0, 0, mode 2, +0x198 (the effect group: 0x25's 0xD48), 1.0, scale).
        let group = row.values.get(3).copied().unwrap_or(0.0) as i32;
        if group != 0 {
            pawn.next_effect_group = Some(group);
        }
        match AnimId::from_name(row.clip) {
            Some(anim) => pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
            None => pawn.note("a Nano Arms attack clip is not in the generated animation table (tools/gen_port.js FORM_ANIM_SETS_LATE)"),
        }
        a.face_yaw = pawn.yaw;
        a.sync_ctx(pawn);
    }

    /// The hookshot's pull (the on-contact handler 0x141355780, kind 0xBC): its yo-yo hit the attack's own target (the
    /// victim == the homing target's owner): warp beside it, kind 0xBD, M_8424 from the start time, the air mode.
    fn hook(&mut self, cx: &mut Ctx) {
        if self.a.kind != gm::kind::HOOK || self.a.target == TargetId::NONE {
            return;
        }
        let target = self.a.target;
        let hit = cx.pawn.combat.windows.iter().any(|w| w.row_name == "SCGM_HOOKSHOT" && w.hit_list().contains(&target));
        if !hit {
            return;
        }
        let Some((warp, start)) = gm::hook_warp(cx, target) else { return };
        let (stop, z_gain, gravity, hold) = gm::hook_values();
        // (F-CLAWS for F-NANO) placed swept: never into or through the level
        let warp = super::attack::swept_warp(cx, warp);
        let s = &mut *cx.pawn;
        s.cmc.location = warp;
        s.cmc.stop_movement();
        let a = &mut self.a;
        a.kind = gm::kind::HOOK_END;
        s.combat.last_kind = a.kind;
        // (the port's record of the kinds played: PawnCombat::history)
        if s.combat.history.len() >= 16 {
            s.combat.history.remove(0);
        }
        s.combat.history.push(a.kind);
        (a.stop_dist, a.z_gain) = (stop, z_gain);
        // +0x234 = (x & ~2) | 5 | 0x10 | 0x800: the XY channel on, the root-scale channel off, the hover.
        (a.chan_xy, a.chan_root, a.hover) = (true, false, true);
        (a.phase_xy, a.phase_root, a.phase_z) = (0, 0, 0);
        (a.air_mode, a.root_len) = (true, 0.0);
        if let Some(clip) = gm::attack_row(gm::kind::HOOK_END, true).and_then(|r| AnimId::from_name(r.clip)) {
            s.anim.play_with(clip, PlayArgs::mode(2, 0.0));
            if start > 0.0 {
                s.anim.set_time(start);
            }
        }
        s.cmc.gravity_scale = gravity;
        s.cmc.gravity_off = true;
        s.cmc.set_flying();
        a.air_hold = hold;
        (a.enable_input, a.face_at, a.swing_started) = (true, true, true);
        // pawn+0xE28 = 1 (the hit confirm); invincible (vt+0x2068(1)).
        s.combat.hit_flag = true;
        s.status.damage_invalid = true;
        a.sync_ctx(s);
    }
}

impl PlayerState for ComboGm {
    const ID: StateId = StateId::ComboGm;

    fn sub_step(&self) -> u8 {
        self.a.sub
    }

    /// Begin (0x141333480 -> the Attack's 0x141333210) with ComboGM's Decide.
    fn begin(&mut self, cx: &mut Ctx) {
        crate::actions::draw_for_attack(cx, self.a.combo == 0);
        cx.pawn.actions.last_pose = 1;
        let passed = std::mem::take(&mut cx.pawn.combat.pass_on);
        if self.a.combo > 0 {
            (self.a.counters[0], self.a.counters[3]) = passed;
        }
        self.a.counters[1] = self.a.counters[0];
        self.a.counters[2] = self.a.counters[3];
        cx.pawn.combat.carry = (0, 0);
        if self.a.target_info(cx).is_none() {
            self.a.target = cx.pawn.combat.lockon.current;
        }
        cx.pawn.cmc.stop_movement();
        // PreDecide (slot 121 0x14134C420).
        (self.a.target_on_ground, self.a.dist, self.a.dz) = (true, 0.0, 0.0);
        self.a.target_metrics(cx);
        self.a.air = true;
        let (hit, distance, _) = ground_probe(cx.pawn, cx.world, P.decide_ground_probe_range);
        if hit && distance <= P.decide_ground_probe_max {
            self.a.air = false;
        }
        self.a.num_enemies = cx.pawn.combat.lockon.crowd;
        // ---- Decide (slot 122 0x141361870)
        let pawn = &mut *cx.pawn;
        let input = GmDecideIn { combo: self.a.combo, reach: self.a.reach, last_kind: pawn.combat.last_kind, dist: self.a.dist, dz: self.a.dz, target_high: self.a.target_high, air: self.a.air, num_enemies: self.a.num_enemies, finish_count: pawn.combat.finish_count };
        let d = gm::decide(&input);
        if d.last_reset {
            pawn.combat.last_kind = 0;
        }
        (self.a.kind, self.a.reach) = (d.kind, d.reach);
        (self.a.finish, self.a.finish_end) = (d.finish, d.finish_end);
        if let Some(z) = d.fixed_dir_z {
            (self.a.fixed_dir_z, self.a.dir_z) = (true, z);
        }
        pawn.combat.finish_count = d.finish_count;
        pawn.combat.finish_end |= d.finish_end;
        // The head of PlayAttack: "if +0x1E8 != 0: slot 124 (the flow refine), kind = forcedKind" (the guard counter 0x25,
        // the flow kinds) with the air kinds' ability variants.
        if self.a.forced_kind != 0 {
            self.a.flow_refine(pawn);
            let has = |a| pawn.status.abilities.has(a);
            use ETresAbilityKind as Ab;
            self.a.forced_kind = match self.a.forced_kind {
                0x0F if has(Ab::AerialDive) => 0x20,
                0x10 if has(Ab::QuickShaft) => 0x18,
                0x11 if has(Ab::FlashStep) => 0x19,
                0x12 if has(Ab::RadialCut) => 0x1A,
                k => k,
            };
            self.a.kind = self.a.forced_kind;
        }
        // (AUDIT-B W-CORE) the Attack's Begin 0x141333210 (this class's slot 96 thunk): Final Blow's chase over Decide's kind.
        if self.a.chase_begin(cx) {
            return;
        }
        self.play(cx);
        cx.set_voice_group_no(crate::host::audio::attack_voice_group_no(self.a.kind as i32));
        self.a.sub = 1;
    }

    /// Tick (0x1413417E0 -> the Attack's 0x141341340), with the hookshot's on-contact handler.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        self.hook(cx);
        // (AUDIT-B F-NANO) the on-contact handler 0x141355780 for kind 0xBD: any contact of its window sets the notify
        // object's +0x3C bit 0 = state+0x84 bit 0 = m_bEnableChange (slot 128 0x14134D510 then ends the hover and the combo
        // goes on from the hit, not from the clip's TresEnableChange at 0.7333 s)
        if self.a.kind == gm::kind::HOOK_END && cx.pawn.combat.windows.iter().any(|w| w.row_name == "SCGM_HOOKSHOT_END" && !w.hit_list().is_empty()) {
            cx.pawn.queue.flags.enable_change = true;
        }
        self.a.tick(cx, dt);
    }

    /// End (0x141335DF0 -> the Attack's 0x141335120).
    fn end(&mut self, cx: &mut Ctx) {
        // Attack's End. Its chain test knows the classes Attack / ComboLimit / FireMagic as "the chain goes on": with a
        // ComboGM queued (KH3: any attack class) the chain memory it would reset is kept, and the counters pass on.
        let queued = cx.pawn.queue.queued_id() == Some(StateId::ComboGm);
        let before = cx.pawn.combat.clone();
        let landed = cx.pawn.combat.swing_hit;
        self.a.end(cx);
        if queued {
            let c = &mut cx.pawn.combat;
            (c.finish_count, c.combo, c.hit_count, c.hit_flag, c.finish_end, c.last_kind) = (before.finish_count, before.combo, before.hit_count, before.hit_flag, before.finish_end, before.last_kind);
            (c.sneeze_next, c.stocks, c.command_cooldown) = (before.sneeze_next, before.stocks, before.command_cooldown);
            c.friend.unison = before.friend.unison;
            c.lockon.attack_target = before.lockon.attack_target;
            (c.revenge, c.chase_armed) = (before.revenge, before.chase_armed);
            c.carry = before.carry;
            c.pass_on = if landed { (self.a.counters[0], self.a.counters[3]) } else { (0, 0) };
            c.chase_next = c.chase_armed;
        }
        // The Attack's End 0x141335120: pawn vt+0x2068(0) (the invincibility of 0x25 / the hookshot's pull).
        cx.pawn.status.damage_invalid = false;
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        self.a.on_notifies(cx, notifies);
        // `TresAnimNotify_Shot` -> the shot handler 0x141344580: Nano Arms' cases 0xB9 / 0x25.
        if notifies.events.iter().any(|e| (e.class(), e.phase) == (NotifyClass::TresAnimNotifyShot, NotifyPhase::Instant)) && matches!(self.a.kind, gm::kind::ROCKET | gm::kind::COUNTER) {
            self.a.sync_ctx(cx.pawn);
            gm::shot(cx, self.a.kind, self.a.target);
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// `CanAttack` (slot 44 0x141426330) in Nano Arms: attack.rs's rows with COMBO_GM as the attack class and MaxCombo 6. None =
/// a state whose row is attack.rs's own business (the traversal states: their forced flow kinds).
fn can_attack(state: &StateData, s: &mut Pawn) -> Option<bool> {
    if s.combat.command_cooldown > 0.0 {
        return Some(false);
    }
    if s.status.abilities.has(ETresAbilityKind::ComboMaster) {
        s.combat.hit_flag = true;
    }
    let s = &*s;
    match state.id() {
        StateId::Guard if s.actions.counter_window > 0.0 => return Some(true),
        StateId::Guard if super::guard::attack_out_of_hit(state, s) => return Some(true),
        StateId::AirSlide if s.actions.reaction_command.is_some() => return Some(true),
        id if s.combat.guard.offer.is_some_and(|o| o.source == id) => return Some(true),
        _ => {}
    }
    let max = gm::max_combo();
    let chain = |a: &Attack| s.queue.nothing_queued() && !a.next_queued && a.swing_started && a.enable_input && a.combo < max && s.combat.hit_flag && !s.combat.finish_end;
    Some(match state {
        StateData::ComboGm(c) => chain(&c.a),
        StateData::Attack(a) => chain(a),
        StateData::ComboLimit(c) => chain(&c.0),
        StateData::FireMagic(m) => m.can_attack(s, max) && s.combat.hit_flag && !s.combat.finish_end,
        StateData::UseItem(_) => s.queue.flags.enable_change,
        StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_) | StateData::Glide(_) | StateData::Floating(_) | StateData::Swaying(_) => true,
        StateData::DodgeRoll(_) | StateData::AirSlide(_) => s.queue.flags.enable_change,
        StateData::Damage(d) => s.queue.nothing_queued() && !d.bad && matches!(d.kind.value(), 1 | 2 | 7 | 8 | 9 | 10) && s.queue.flags.enable_change,
        StateData::BadstatStun(_) => s.queue.nothing_queued() && s.queue.flags.enable_change,
        StateData::WeaponChange(w) => w.changed && s.combat.hit_flag,
        StateData::WallRun(_) | StateData::Hopping(_) | StateData::SlopeSlide(_) | StateData::SuperSlide(_) | StateData::WallKick(_) | StateData::PoleTurn(_) | StateData::RailSlide(_) => return None,
        _ => false,
    })
}

/// `TryAttack(target, 0)` (slot 45 0x141437330) in Nano Arms: the request queues COMBO_GM (the form package's `attack`
/// hook); the guard counter is the Nano Counter (0x141437E28: forced kind 0x25 in 0x86). None = attack.rs's own path (the
/// traversal states).
pub(crate) fn try_attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    let s = &mut *cx.pawn;
    if !can_attack(state, s)? {
        return Some(false);
    }
    let guard_counter = state.id() == StateId::Guard && s.actions.counter_window > 0.0;
    let forced = crate::combat::guard::forced_attack(state.id(), s);
    let (target, forced_kind, just_guard, push) = match forced {
        Some(o) => (if o.target != TargetId::NONE { o.target } else { target }, if guard_counter { gm::kind::COUNTER } else { o.kind }, o.just_guard, o.push),
        None => (target, 0, false, false),
    };
    s.combat.lockon.attack_target = target;
    let chained = match state {
        StateData::ComboGm(c) => Some(&mut c.a),
        StateData::Attack(a) => Some(a),
        StateData::ComboLimit(c) => Some(&mut c.0),
        _ => None,
    };
    if let Some(cur) = chained {
        cur.next_queued = true;
        let next = Attack { combo: cur.combo + 1, air: cur.air, target, reach: cur.reach, ..Attack::RESET };
        s.combat.combo = next.combo;
        s.queue.push_state(ComboGm::new(next));
    } else if let StateData::FireMagic(m) = state {
        let (combo, air) = m.attack_follows();
        s.combat.combo = combo;
        s.queue.push_state(ComboGm::new(Attack { combo, air, target, ..Attack::RESET }));
    } else if let StateData::WeaponChange(_) = state {
        let next = Attack { combo: s.combat.combo + 1, air: true, target, ..Attack::RESET };
        s.combat.pass_on = s.combat.carry;
        s.combat.combo = next.combo;
        s.queue.set_state(ComboGm::new(next));
    } else {
        let carried = s.magic.combo_keep > 0.0 && matches!(state, StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_) | StateData::Glide(_) | StateData::Floating(_));
        let combo = if carried { s.combat.combo + 1 } else { 0 };
        if carried {
            s.combat.pass_on = s.combat.carry;
        }
        let next = ComboGm::new(Attack { combo, air: !on_ground(s), target, forced_kind, just_guard, ..Attack::RESET });
        s.combat.combo = combo;
        if push {
            s.queue.push_state(next);
        } else {
            s.queue.set_state(next);
        }
    }
    Some(true)
}

// ================================================================================================ FINISH_GM

/// StyleFinishGM Begin 0x1413CC360: StyleFinish's Begin (done by the caller), FUN_1405D4D90(pawn, 1, 0); Reset 0x1413D0DC0:
/// +0x2E0 .. +0x2EC = 0, +0x164 = 0.
pub(crate) fn finish_begin(f: &mut StyleFinish, cx: &mut Ctx) {
    f.a.sub = 0;
    f.a.swing_started = false;
    f.root_len = 0.0;
    cx.pawn.combat.nano.gate = None;
}

/// StyleFinishGM Tick 0x1413D5660 (the sheet's `finish.*`): the gate, Sora hidden while it fires, the end.
pub(crate) fn finish_tick(f: &mut StyleFinish, cx: &mut Ctx, dt: f32) -> bool {
    // +0x2E8 += dt (a clock no reader of the class uses).
    f.root_len += dt;
    match f.a.sub {
        0 => {
            // The gate spawned at Sora's location and rotation, linked, shown, M_8416; Sora Flying, M_8416; the effect group
            // 0x66D (+0x198, FUN_1405B93B0).
            let position = cx.pawn.cmc.location;
            let yaw = cx.pawn.yaw;
            cx.pawn.cmc.set_flying();
            cx.pawn.next_effect_group = Some(gm::code("code.finish", 0) as i32);
            f.play(cx.pawn, "M_8416", -1.0, 2);
            cx.pawn.combat.nano.gate = Some(gm::Gate { model: "w_so174", position, yaw, clip: "M_8416", time: 0.0, next: [0.0; 2] });
            f.a.sub = 2;
            f.root_len = 0.0;
            // (AUDIT-B F-NANO) pawn vt+0x1038(0, 0) = 0x141432AF0: OnPlayerSpecificAction(0, 0) + the state-event component
            // (FUN_1413E3190) -> the state-event blueprints' ReceivePlayerSpecificAction (none of Nano Gear's has one)
            cx.pawn.event(crate::host::EventKind::PlayerSpecificAction { state: StateId::Stylefinish, action: 0, sub: 0 });
        }
        2 => {
            f.root_len = 0.0;
            if cx.pawn.anim.ended {
                // Weapon away (vt+0x1340(0, 0, 1.0)), Sora hidden (vt+0xD98(0)); both M_8417; gate +0xB48 = 0; the world slow
                // off (0x14140E500(ctrl, 0)).
                crate::actions::draw(cx, false);
                f.play(cx.pawn, "M_8417", -1.0, 2);
                crate::combat::form::world_slow(cx, false);
                let n = &mut cx.pawn.combat.nano;
                n.hide_sora = true;
                if let Some(g) = n.gate.as_mut() {
                    (g.clip, g.time, g.next) = ("M_8417", 0.0, [0.0; 2]);
                }
                f.a.sub = 5;
            }
        }
        5 => {
            gm::portal(cx);
            if cx.pawn.anim.ended {
                cx.pawn.combat.nano.hide_sora = false;
                f.play(cx.pawn, "M_8418", -1.0, 2);
                if let Some(g) = cx.pawn.combat.nano.gate.as_mut() {
                    (g.clip, g.time) = ("M_8418", 0.0);
                }
                f.a.sub = 6;
            }
        }
        _ => {
            if cx.pawn.anim.ended {
                // slot 0x270 (DoChain); End 0x1413CF170: the gate's end (vt+0x2E8, gate vt+0x2590), the effect group ended.
                let n = &mut cx.pawn.combat.nano;
                (n.gate, n.hide_sora) = (None, false);
                // (AUDIT-B F-NANO) End 0x1413CF170: once (+0x2EC) pawn vt+0x1038(0x42) = OnPlayerSpecificAction(66)
                cx.pawn.event(crate::host::EventKind::PlayerSpecificAction { state: StateId::Stylefinish, action: gm::code("code.finish", 1) as u8, sub: 0 });
                return f.finish(cx.pawn);
            }
        }
    }
    // The gate plays its clip with Sora's.
    let t = cx.pawn.anim.time;
    if let Some(g) = cx.pawn.combat.nano.gate.as_mut() {
        g.time = t;
    }
    false
}

