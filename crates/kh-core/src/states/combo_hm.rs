//! (weapons F-HAMMER) `TresPlayerState_ComboHM` (id 76) and `TresPlayerState_ComboDR` (id 79): the attack states of Hyper
//! Hammer (0x81) / Boom Hammer (0x92) and of Drill Punch (0x84) / Clock Drill (0x93) (pawn slot 1337 0x141432F00: 0x81 /
//! 0x92 -> class 0x4C, 0x84 / 0x93 -> 0x4F). Subclasses of `TresPlayerState_Attack` (attack.rs, which these wrap): their
//! vtables (0x14690FF70 / 0x14690D720) differ from the Attack class only by the combo base's thunks (slots 95 .. 98 Reset /
//! Begin / Tick / End and 120 PlayAttack: the Attack class's own functions) and Decide (slot 122: ComboHM 0x141362030,
//! ComboDR 0x141360F70; combat/forms/hammer.rs `decide_hm` / `decide_dr`).
//!
//! PlayAttack (slot 120 0x1413378B0) cases 0x58 .. 0x6A and 0x25 (Barrier Cracker): from the rows `attack.<form>.<kind>` of
//! `sheets/port/sora-form-hammer.json` (tools/sora_form_hammer.js). The attack request in the forms is `try_attack` below
//! (the form package's `attack` hook): KH3's TryAttack 0x141437330 with the class the form asks for.
//!
//! Also here: the bodies of FINISH_DR (0x9E): Drill Punch's in STYLEFINISH (StyleFinish Begin 0x1413CBF70 case 0x9E in form
//! 0x84 -> slot 148 0x1413DF1F0) and Clock Drill's, `TresPlayerState_StyleFinishDR` (id 34; TryStyleFinish 0x141447840 in form
//! 0x93): Reset 0x1413D0DC0, Begin 0x1413CC300, Tick 0x1413D4990, End 0x1413CF0C0. The framework queues STYLEFINISH (33) for
//! every finish command (combat/form.rs `start_style`): the body runs there through the form package's finish hooks; the
//! state 34 below is the same body for the day `start_style` takes TryStyleFinish's class.

use super::attack::Attack;
use super::common::{ground_probe, on_ground};
use super::style_finish::StyleFinish;
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, NotifyClass, NotifyPhase, Notifies, PlayArgs};
use crate::combat::forms::hammer::{self as hm, HammerDecideIn};
use crate::combat::tables::P;
use crate::enums::ETresAbilityKind as Ab;
use crate::host::{ScriptHold, TargetId};
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;
// (F-SHAPES) `glam::Vec3` no longer used: the Clock's shots are placed by their hull track.

/// The class of the attack (ComboHM's or ComboDR's Decide).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Hammer,
    Drill,
}

/// The fields of either class: the Attack state's, and what the class keeps of its PlayAttack.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Combo {
    pub(crate) a: Attack,
    pub(crate) class: Class,
    /// PlayAttack's `pUStackX_10` (pawn vt+0x2068(1)): the attack is invincible until its End.
    pub(crate) invincible: bool,
    /// The kind's TresAnimNotify_Shot spawns its projectile (Barrier Cracker).
    pub(crate) shot: bool,
}

/// `TresPlayerState_ComboHM` (76).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComboHm(pub(crate) Combo);

/// `TresPlayerState_ComboDR` (79).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComboDr(pub(crate) Combo);

impl Combo {
    fn new(a: Attack, class: Class) -> Combo {
        Combo { a, class, invincible: false, shot: false }
    }

    /// PlayAttack (slot 120 0x1413378B0) of the form kinds (attack.rs `play_attack` for the others: the forced kinds of the
    /// guard / risk-dodge counters).
    fn play(&mut self, cx: &mut Ctx) {
        let cmd = cx.pawn.combat.form.cmd;
        let kind = self.a.kind;
        let Some(row) = hm::attack_row_for(cmd, kind, self.a.air) else {
            self.a.play_attack(cx);
            return;
        };
        let pawn = &mut *cx.pawn;
        pawn.combat.last_kind = kind;
        if pawn.combat.history.len() >= 16 {
            pawn.combat.history.remove(0);
        }
        pawn.combat.history.push(kind);
        let flag = |f: &str| row.flags.contains(&f);
        let value = |k: usize| row.values.get(k).copied().unwrap_or(0.0);
        let a = &mut self.a;
        // The defaults before the switch: m_bFaceAt; the XY channel on, the root channel off; stop 160; z gain 3.
        a.face_at = true;
        (a.chan_xy, a.chan_root) = (row.chan_xy, row.chan_root);
        (a.stop_dist, a.z_gain, a.keep_face) = (row.stop_dist, row.z_gain, false);
        a.root_len = row.root_len;
        // +0x1DF: the case's own, or bAir (0x5B), or bAir for 0x59 (with +0x234 bit 16).
        a.air_mode = row.air_mode || ((flag("air_if_air") || flag("air_is_air")) && a.air);
        if flag("no_vz") {
            a.no_vz = true;
        }
        if flag("keep_dir_z") {
            a.keep_dir_z = true;
        }
        if flag("dive") {
            a.dive = true;
        }
        if flag("fixed_dir_z") || flag("dive") || flag("keep_dir_z") {
            a.dir_z = value(2);
            a.fixed_dir_z |= flag("fixed_dir_z");
        }
        // `if dist < D: fVar28 = divisor, fVar32 = max, the XY channel off (and the root channel on)`.
        let (near, divisor, max) = (value(3), value(4), value(5));
        let mut scale_div = 0.0;
        if near > 0.0 && a.dist < near {
            scale_div = divisor;
            a.chan_xy = false;
            if flag("near_root") {
                a.chan_root = true;
            }
        }
        // The tail (LAB_14133A87A): ground: gravity on, DirZ 0; air mode: +0x234 |= 4 | 0x10, GravityScale 0.1, Flying,
        // +0x18C = 0.6.
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
        pawn.combat.swing_hit = false;
        // The play's root scale: clamp(dist / fVar28, 0.1, fVar32) when fVar28 and dist are not 0, else 1.
        let scale = if scale_div != 0.0 && a.dist != 0.0 { (a.dist / scale_div).clamp(0.1, max.max(0.1)) } else { 1.0 };
        pawn.root_scale = (scale, 1.0);
        // Play(name, 0, 0, mode 2, +0x198 (0x76C in 0x92 / 0x93; 0xD48 Barrier Cracker), 1.0, scale).
        if row.group != 0 {
            pawn.next_effect_group = Some(row.group);
        }
        match AnimId::from_name(row.clip) {
            Some(anim) => pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
            None => pawn.note("a hammer-form attack clip is not in the generated animation table (tools/gen_port.js FORM_ANIM_SETS)"),
        }
        // fVar33 > 0: anim vt+0xF0 = SetAnimTime (0x5F starts M_3412 at 0.2 s).
        let start = value(6);
        if start > 0.0 {
            pawn.anim.set_time(start);
        }
        // +0x164 = 1 (Barrier Cracker: the swing counts as started at once).
        if flag("swing") {
            a.swing_started = true;
        }
        self.invincible = flag("invincible");
        if self.invincible {
            pawn.status.damage_invalid = true;
        }
        // (AUDIT-B W-CORE) 0x1405B8B30(pawn+0x460, 8): Barrier Cracker 0x25 (+0x234 |= 0x2800) and Drill Punch's giga upper 0x6A
        // (+0x234 |= 0x802): his push power level PPL_90 until End (attack.rs `counter_body` / `raise_push`). 0x59 in the air
        // (+0x142): +0x234 |= 0x10000 (the traction's bit-16 root path: attack.rs `root_keep`).
        match kind {
            hm::KIND_BARRIER_CRACKER => a.counter_body(pawn),
            0x6A => a.raise_push(pawn),
            0x59 if a.air => a.root_keep = true,
            _ => {}
        }
        self.shot = flag("shot");
        a.face_yaw = pawn.yaw;
        a.sync_ctx(pawn);
        // anim vt+0x1F0(+0x19C): SetVoiceGroupNo (1: 0x5D / 0x67; 0x34: Barrier Cracker; else 0).
        cx.set_voice_group_no(value(7) as i32);
    }

    /// Begin (Attack's 0x141333210 with the class's Decide).
    fn begin(&mut self, cx: &mut Ctx) {
        // ---- Attack's Begin (attack.rs `begin`, the parts that apply to the class)
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
        // ---- Decide (slot 122)
        let pawn = &mut *cx.pawn;
        let has = |k| pawn.status.abilities.has(k);
        let input = HammerDecideIn {
            combo: self.a.combo,
            reach: self.a.reach,
            last_kind: pawn.combat.last_kind,
            last_at_begin: pawn.combat.last_kind,
            dist: self.a.dist,
            dz: self.a.dz,
            target_high: self.a.target_high,
            target_on_ground: self.a.target_on_ground,
            air: self.a.air,
            num_enemies: self.a.num_enemies,
            forced_kind: self.a.forced_kind,
            flow_abilities: [has(Ab::TurnCutter), has(Ab::SonicSlash), has(Ab::SonicDown), has(Ab::Summersalt)],
        };
        let d = match self.class {
            Class::Hammer => hm::decide_hm(&input),
            Class::Drill => hm::decide_dr(&input),
        };
        if d.last_reset {
            pawn.combat.last_kind = 0;
            pawn.combat.stocks = (0, 0);
        }
        (self.a.kind, self.a.reach, self.a.air) = (d.kind, d.reach, d.air);
        (self.a.finish, self.a.finish_end) = (d.finish, d.finish_end);
        if let Some(z) = d.fixed_dir_z {
            (self.a.fixed_dir_z, self.a.dir_z) = (true, z);
        }
        pawn.combat.finish_end |= d.finish_end;
        self.a.forced_kind = d.forced_kind;
        // The head of PlayAttack: "if forcedKind(+0x1E8) != 0: kind = forcedKind" (slot 124 has run: +0x234 bit 20).
        if self.a.forced_kind != 0 {
            self.a.kind = self.a.forced_kind;
        }
        // ---- PlayAttack
        // (AUDIT-B W-CORE) the Attack's Begin 0x141333210 (this class's slot 96 thunk): Final Blow's chase over Decide's kind.
        if self.a.chase_begin(cx) {
            return;
        }
        self.play(cx);
        self.a.sub = 1;
    }

    /// End (Attack's 0x141335120). Its chain test knows the classes Attack / ComboLimit / FireMagic as "the chain goes on":
    /// with COMBO_HM / COMBO_DR queued (KH3: any attack class) the chain memory it would reset is kept and the counters pass on.
    fn end(&mut self, cx: &mut Ctx) {
        let queued_combo = matches!(cx.pawn.queue.queued_id(), Some(StateId::ComboHm | StateId::ComboDr));
        let before = cx.pawn.combat.clone();
        let landed = cx.pawn.combat.swing_hit;
        self.a.end(cx);
        if queued_combo {
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
        // pawn vt+0x2068(0).
        if self.invincible {
            cx.pawn.status.damage_invalid = false;
            self.invincible = false;
        }
    }

    /// The notifies: the Attack state's; a TresAnimNotify_Shot of Barrier Cracker spawns BARRIERCRACKER (the shot handler
    /// 0x141344580 case 0x25) at the form weapon's ATK_01 (vt+0x1410).
    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        self.a.on_notifies(cx, notifies);
        if !self.shot || self.a.kind != hm::KIND_BARRIER_CRACKER {
            return;
        }
        for event in &notifies.events {
            if (event.class(), event.phase) != (NotifyClass::TresAnimNotifyShot, NotifyPhase::Instant) {
                continue;
            }
            let Some((row, radius, life, effect)) = hm::barrier_cracker() else { continue };
            let at = hm::muzzle(cx, event.sequence_name(), cx.pawn.anim.time).unwrap_or_else(|| crate::combat::hit::shot_origin(cx));
            crate::combat::hit::spawn_xs_shot(cx, "BARRIERCRACKER", row, at, radius, life, effect);
            self.a.shots += 1;
        }
    }
}

macro_rules! combo_state {
    ($type:ident, $id:expr) => {
        impl PlayerState for $type {
            const ID: StateId = $id;

            fn sub_step(&self) -> u8 {
                self.0.a.sub
            }

            fn begin(&mut self, cx: &mut Ctx) {
                self.0.begin(cx);
            }

            /// Tick: the Attack class's (0x141341340).
            fn tick(&mut self, cx: &mut Ctx, dt: f32) {
                self.0.a.tick(cx, dt);
            }

            fn end(&mut self, cx: &mut Ctx) {
                self.0.end(cx);
            }

            fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
                self.0.on_notifies(cx, notifies);
            }

            fn movement_state(&self, _pawn: &Pawn) -> MovementState {
                MovementState::Other
            }
        }
    };
}
combo_state!(ComboHm, StateId::ComboHm);
combo_state!(ComboDr, StateId::ComboDr);

/// The attack class's state for the held form.
fn class_state(cmd: i32, a: Attack) -> StateData {
    if hm::is_drill(cmd) {
        ComboDr(Combo::new(a, Class::Drill)).into()
    } else {
        ComboHm(Combo::new(a, Class::Hammer)).into()
    }
}

/// The running attack's fields of any attack class the four forms meet.
fn chained(state: &mut StateData) -> Option<&mut Attack> {
    match state {
        StateData::ComboHm(c) => Some(&mut c.0.a),
        StateData::ComboDr(c) => Some(&mut c.0.a),
        StateData::Attack(a) => Some(a),
        StateData::ComboLimit(c) => Some(&mut c.0),
        _ => None,
    }
}

/// `CanAttack` (slot 44 0x141426330) in the four forms: attack.rs's rows with COMBO_HM / COMBO_DR as the attack class and
/// MaxCombo 5. None = a state whose row is attack.rs's own business (the traversal states: their forced flow kinds).
fn can_attack(state: &mut StateData, s: &mut Pawn) -> Option<bool> {
    if s.combat.command_cooldown > 0.0 {
        return Some(false);
    }
    if s.status.abilities.has(Ab::ComboMaster) {
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
    let nothing_queued = s.queue.nothing_queued();
    let (hit_flag, finish_end) = (s.combat.hit_flag, s.combat.finish_end);
    let chain = |a: &Attack| nothing_queued && !a.next_queued && a.swing_started && a.enable_input && a.combo < hm::MAX_COMBO && hit_flag && !finish_end;
    Some(match state {
        StateData::FireMagic(m) => m.can_attack(s, hm::MAX_COMBO) && hit_flag && !finish_end,
        StateData::UseItem(_) => s.queue.flags.enable_change,
        StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_) | StateData::Glide(_) | StateData::Floating(_) | StateData::Swaying(_) => true,
        StateData::DodgeRoll(_) | StateData::AirSlide(_) => s.queue.flags.enable_change,
        StateData::Damage(d) => nothing_queued && !d.bad && matches!(d.kind.value(), 1 | 2 | 7 | 8 | 9 | 10) && s.queue.flags.enable_change,
        StateData::BadstatStun(_) => nothing_queued && s.queue.flags.enable_change,
        StateData::WeaponChange(w) => w.changed && hit_flag,
        StateData::WallRun(_) | StateData::Hopping(_) | StateData::SlopeSlide(_) | StateData::SuperSlide(_) | StateData::WallKick(_) | StateData::PoleTurn(_) | StateData::RailSlide(_) => return None,
        other => chained(other).is_some_and(|a| chain(a)),
    })
}

/// `TryAttack(target, 0)` (slot 45 0x141437330) in the four forms: the request queues COMBO_HM / COMBO_DR (the form
/// package's `attack` hook). None = attack.rs's own path (the traversal states).
pub(crate) fn try_attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    let cmd = cx.pawn.combat.form.cmd;
    let s = &mut *cx.pawn;
    if !can_attack(state, s)? {
        return Some(false);
    }
    let forced = crate::combat::guard::forced_attack(state.id(), s);
    let (target, forced_kind, just_guard, push) = match forced {
        Some(o) => (if o.target != TargetId::NONE { o.target } else { target }, o.kind, o.just_guard, o.push),
        None => (target, 0, false, false),
    };
    s.combat.lockon.attack_target = target;
    if let Some(cur) = chained(state) {
        // The combo step: queued behind the running attack.
        cur.next_queued = true;
        let next = Attack { combo: cur.combo + 1, air: cur.air, target, reach: cur.reach, ..Attack::RESET };
        s.combat.combo = next.combo;
        s.queue.push_state(class_state(cmd, next));
    } else if let StateData::FireMagic(m) = state {
        let (combo, air) = m.attack_follows();
        s.combat.combo = combo;
        s.queue.push_state(class_state(cmd, Attack { combo, air, target, ..Attack::RESET }));
    } else if let StateData::WeaponChange(_) = state {
        let next = Attack { combo: s.combat.combo + 1, air: true, target, ..Attack::RESET };
        s.combat.pass_on = s.combat.carry;
        s.combat.combo = next.combo;
        s.queue.set_state(class_state(cmd, next));
    } else {
        // IDLE, RUN, JUMP, GUARD ...: SetState (RISKDODGE / AIR_SLIDE: PushState); the carry timer of a Keyblade switch.
        let carried = s.magic.combo_keep > 0.0 && matches!(state, StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_) | StateData::Glide(_) | StateData::Floating(_));
        let combo = if carried { s.combat.combo + 1 } else { 0 };
        if carried {
            s.combat.pass_on = s.combat.carry;
        }
        let next = class_state(cmd, Attack { combo, air: !on_ground(s), target, forced_kind, just_guard, ..Attack::RESET });
        s.combat.combo = combo;
        if push {
            s.queue.push_state(next);
        } else {
            s.queue.set_state(next);
        }
    }
    Some(true)
}

// ================================================================================================ FINISH_DR

/// Drill Punch's sub-steps (StyleFinish slot 148 0x1413DF1F0) and the Clock's (StyleFinishDR Tick 0x1413D4990), as
/// `StyleFinish.a.sub`. The fields of the classes: `root_len` = +0x194 (the drill's give-up timer) / +0x2E8 (the Clock's
/// clock), `catch` = m_pCatchPawn, `accompany` = m_pClock (its root and yaw), `a.shots` = the Clock's windows already
/// spawned (a bit each), `a.combo` = the time of its interval window's last shot (ms).
pub(crate) fn finish_begin(f: &mut StyleFinish, _cx: &mut Ctx) {
    // Reset 0x1413D0DC0: +0x2E0 = 0, +0x2E8 = 0, +0x2EC = 0, +0x164 = 0.
    f.a.sub = 0;
    f.root_len = 0.0;
    f.a.shots = 0;
    f.a.combo = 0;
    f.catch = TargetId::NONE;
    f.accompany = None;
}

/// The per-command tick of FINISH_DR: true = done (chained on itself).
pub(crate) fn finish_tick(f: &mut StyleFinish, cx: &mut Ctx, dt: f32) -> bool {
    if cx.pawn.combat.form.cmd == hm::CHANGE_DR2 {
        return clock_drill(f, cx, dt);
    }
    drill(f, cx, dt)
}

/// The caught pawn is held (the generic `Targets::script_hold`: every ER enemy) and (F-POLISH) CARRIED on the drill's
/// X_link_skt (state 62's link, vt+0x16E8 / FUN_143EAC880): its actor location (UE: the capsule's centre, our target's
/// `center`) on the socket of the playing clip's frame (`hm::catch_link`); held in place when the clip has no such track.
fn hold(cx: &mut Ctx, target: TargetId, dt: f32) {
    if target == TargetId::NONE {
        return;
    }
    let face = cx.pawn.cmc.location;
    let sequence = cx.pawn.anim.name.data().samples.first().map(|&(_, i)| crate::anim::SEQUENCES[i].name);
    let link = sequence.and_then(|s| hm::catch_link(cx, s, cx.pawn.anim.time));
    let carry = link.zip(cx.target(target)).map(|(at, t)| at - (t.center - t.position));
    cx.host.targets.script_hold(target, Some(ScriptHold { face, turn_rate: 0.0, dt, hurt: true, carry }));
}

/// The caught pawn is given back.
pub(crate) fn release(f: &mut StyleFinish, cx: &mut Ctx) {
    let target = std::mem::replace(&mut f.catch, TargetId::NONE);
    if target != TargetId::NONE {
        cx.host.targets.script_hold(target, None);
    }
}

/// StyleFinish slot 148 0x1413DF1F0: Drill Punch's Ultimate Finish (FINISH_DR in form 0x84).
fn drill(f: &mut StyleFinish, cx: &mut Ctx, dt: f32) -> bool {
    match f.a.sub {
        0 => {
            // vt+0x17A0(5, 1): Flying; Play("M_6421", blend -1, mode 2) + voice 0; +0x188 = -150, +0x260 = 1.0, +0x1F4 |= 0x20,
            // +0x194 = 0.
            cx.pawn.cmc.set_flying();
            f.play(cx.pawn, "M_6421", -1.0, 2);
            cx.set_voice_group_no(0);
            f.root_len = 0.0;
            // (AUDIT-B F-HAMMER) +0x1F4 bit 5: the facing (slot 126 0x14134D0E0, F-LOCO's) pitches him toward the target while
            // it is on; +0x188 = -150: the target point's height offset (attack.rs `target_dz`). (+0x260 = 1.0 is the chase
            // warp's countdown start, spec 6.7: slot 148 never sets +0x1E1, so no warp reads it.)
            (f.a.pitched, f.a.target_dz) = (true, -150.0);
            f.a.sub = 2;
        }
        2 => {
            // slot 127(1, 0.2): facing on for 0.2 s.
            (f.a.facing_on, f.a.face_timer) = (true, 0.2);
            // m_pCatchPawn: the target the warhead's SCDR_FINISH (DRILL_BIND) window hit first.
            if f.catch == TargetId::NONE {
                if let Some(t) = cx.pawn.combat.windows.iter().find(|w| w.group == "Warhead").and_then(|w| w.hit_list().first().copied()) {
                    f.catch = t;
                }
            }
            let caught = f.catch != TargetId::NONE && cx.target(f.catch).is_some();
            if !caught {
                if !cx.pawn.anim.ended {
                    f.root_len += dt;
                    if f.root_len > 10.0 {
                        // (AUDIT-B F-HAMMER) the state's End with +0x1F4 bit 5 (0x141335120): Turn((0, yaw, 0), 1.0), bit cleared
                        f.a.pitched = false;
                        let yaw = cx.pawn.yaw;
                        super::common::turn_to(cx.pawn, 0.0, yaw, 1.0);
                        release(f, cx);
                        cx.pawn.queue.do_chain();
                        return true;
                    }
                    return false;
                }
            } else {
                hold(cx, f.catch, dt);
            }
            // slot 127(0, 0), +0x1F4 &= ~0x20, the pitch levelled (vt+0xDB0), Play("M_6423") + voice 0, +0x164 = 0.
            (f.a.facing_on, f.a.face_timer) = (false, 0.0);
            // (AUDIT-B F-HAMMER) +0x218 = 0, Turn((0, the root's yaw +0x1FC, 0), 1.0) (0x1413DF1F0 lines 245-280)
            (f.a.pitched, f.a.pitch_aim) = (false, 0.0);
            let yaw = cx.pawn.yaw;
            super::common::turn_to(cx.pawn, 0.0, yaw, 1.0);
            f.play(cx.pawn, "M_6423", -1.0, 2);
            cx.set_voice_group_no(0);
            f.a.swing_started = false;
            f.a.sub = 5;
        }
        _ => {
            // The caught pawn kept on the drill (vt+0x16E8, FUN_143EAC880); at M_6423's end DoChain (vt+0x270).
            hold(cx, f.catch, dt);
            if cx.pawn.anim.ended {
                release(f, cx);
                cx.pawn.queue.do_chain();
                return true;
            }
        }
    }
    false
}

/// (F-SHAPES) The window key base of the Clock's shots (combat/hit.rs `FormShot::key`: + the window's index).
const CLOCK_SHOT_KEY: u16 = 0x9390;

/// The Clock's hit windows (its clip L421, the row `pawn.DR2.w_so114`): shots of its shapes at the Clock (FinishDR /
/// FinishDR2: tres_colltest01 under m_Scale). (F-SHAPES) One shot per window, as KH3's one primitive per window: its hull
/// follows the `wep` bone's track of L421 (`hm::clock_track`, combat/hull.rs) from the window's start to its end, with the
/// window's AttackInterval (TimerTick 0x14062CFC0: the hit list empties an interval after the first hit). Before, a
/// sphere of the hull's horizontal radius was spawned per interval where the shape stood at that moment.
fn clock_hits(f: &mut StyleFinish, cx: &mut Ctx, time: f32) {
    let (Some(clock), Some((root, yaw))) = (hm::clock(), f.accompany) else { return };
    for (k, w) in clock.windows.iter().enumerate().take(16) {
        let mut p = w.split('@');
        let (Some(span), Some(group), Some(row), Some(interval)) = (p.next(), p.next(), p.next(), p.next()) else { continue };
        let Some((a, b)) = span.split_once('-') else { continue };
        let (Ok(a), Ok(b), Ok(interval)) = (a.parse::<f32>(), b.parse::<f32>(), interval.parse::<f32>()) else { continue };
        if time < a || time > b {
            continue;
        }
        let bit = 1 << k;
        if f.a.shots & bit != 0 {
            continue;
        }
        f.a.shots |= bit;
        let Some(track) = hm::clock_track(group) else { continue };
        let group: &'static str = if group == "FinishDR2" { "FinishDR2" } else { "FinishDR" };
        let row: &'static str = match row {
            "DR2_FINISH_NEEDLE" => "DR2_FINISH_NEEDLE",
            "DR2_FINISH_END" => "DR2_FINISH_END",
            _ => "DR2_FINISH",
        };
        let info = cx.pawn.combat.attack_ctx;
        let key = CLOCK_SHOT_KEY + k as u16;
        crate::combat::hit::spawn_form_shot(cx, crate::combat::hit::FormShot { key, group, row, position: root, yaw, size: (0.0, 0.0, 0.0), life: (b - time).max(1.0 / 60.0), hit_effect: None, hit_se: None, info, hit_list: Vec::new() });
        if let Some(s) = cx.pawn.combat.shots.last_mut().filter(|s| s.window.key == (crate::combat::hit::FORM_SHOT_SEQUENCE, key)) {
            s.hull = Some(crate::combat::hull::HullShot { track, t0: time, last: None });
            s.window.interval = interval;
            // (AUDIT-B F-HAMMER) the Clock's OWN hit entry: its shapes' m_EffectGrpName "None" -> StylePawn/w_so114
            // TresAtkColl0 m_HitEffects["None"] (vw_so114_fns_hit00, direct vw_so110_str01; chooser 0x140628960), not the
            // Kingdom Key's table.
            // (AUDIT-B W-DATA) and NO hit SE: L421's three Attack notifies carry only m_WeaponHitSEAliasID HIT4 (no m_HitSEAsset),
            // and the activation 0x140625500 (decompiled lines 125-149) resolves an alias ONLY when the component's owner IsA
            // ATresWeaponBase (weapon vt+0xE90 = m_SoundAssets_Hit[alias]); the Clock is ATresStylePawnDRClock (StylePawnBase ->
            // AccompanyPawnBase -> TresCharPawnBase, sheets/reflect/classes.json) with an empty TresEquipment0, so the prim keeps
            // its default (slot 333 0x1406457C0: m_DefaultHitSE, from shapes whose m_HitSEAsset is null / m_HitSEID SEAL_NOTHING)
            // and the entry's m_PawnHitSEAsset(PerAttr) are null: the chooser plays nothing. (F-HAMMER's HIT4 from Sora's set was
            // an inference.) The hit effect's own SeAttach (vw_so114_fns_hit00 -> se02011_039) still sounds.
            s.window.source = crate::status::keyblade::hit_source(&["w_so114"], false, None);
            s.window.hit_se = None;
        }
    }
}

/// StyleFinishDR Tick 0x1413D4990: Clock Drill's Ultimate Finish, the Clock.
fn clock_drill(f: &mut StyleFinish, cx: &mut Ctx, dt: f32) -> bool {
    // +0x2E8 += dt every tick.
    f.root_len += dt;
    match f.a.sub {
        0 => {
            // The Clock spawned at Sora's location with his yaw, started, shown; Sora put where it stands; both play M_L421 (blend
            // -1) + voice 0; +0x164 = 0; +0x2E8 = 0; vt+0x1038(0, 0).
            f.accompany = Some((cx.pawn.cmc.location, cx.pawn.yaw));
            f.play(cx.pawn, "M_L421", -1.0, 2);
            cx.set_voice_group_no(0);
            f.a.swing_started = false;
            f.root_len = 0.0;
            f.a.sub = 2;
        }
        2 | 5 => {
            let time = cx.pawn.anim.time;
            // While M_L421 is before 3.8 s Sora stays at the Clock (0x143EAC3D0).
            if time < 3.8 {
                if let Some((at, _)) = f.accompany {
                    cx.pawn.cmc.location = at;
                }
            }
            clock_hits(f, cx, time);
            let max = hm::clock().and_then(|c| c.values.first().copied()).unwrap_or(0.0);
            if f.accompany.is_some() && f.root_len < max && !cx.pawn.queue.flags.enable_change && !cx.pawn.anim.ended {
                return false;
            }
            f.a.sub = 6;
        }
        6 => {
            // 0x1413CFA10: the Clock cleaned up; sub 8.
            f.accompany = None;
            f.a.sub = 8;
            if cx.pawn.anim.ended {
                cx.pawn.queue.do_chain();
                return true;
            }
        }
        _ => {
            f.a.sub = 8;
            if cx.pawn.anim.ended {
                cx.pawn.queue.do_chain();
                return true;
            }
        }
    }
    false
}

/// `TresPlayerState_StyleFinishDR` (id 34): STYLEFINISH with the Clock's body (see the file head).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StyleFinishDr(pub(crate) StyleFinish);

impl PlayerState for StyleFinishDr {
    const ID: StateId = StateId::StylefinishDr;

    fn sub_step(&self) -> u8 {
        self.0.a.sub
    }

    /// Begin 0x1413CC300: StyleFinish's Begin (while the pawn is valid).
    fn begin(&mut self, cx: &mut Ctx) {
        self.0.begin(cx);
    }

    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        self.0.tick(cx, dt);
    }

    /// End 0x1413CF0C0: 0x1413CFA10 (the Clock cleaned up), then the command end event (StyleFinish's End).
    fn end(&mut self, cx: &mut Ctx) {
        self.0.accompany = None;
        self.0.end(cx);
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        self.0.on_notifies(cx, notifies);
    }

    fn movement_state(&self, pawn: &Pawn) -> MovementState {
        self.0.movement_state(pawn)
    }
}
