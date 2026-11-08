//! (weapons F-RAGE) `TresPlayerState_ComboAnti` (id 86, TSID_STATE_COMBO_AN): the attack state of Rage Form (0x8B; pawn
//! slot 1337 0x141432F00: form 0x8B -> class 0x56). A subclass of `TresPlayerState_Attack` (attack.rs, which this wraps);
//! it overrides (UTresPlayerState_ComboAnti, the class's own functions):
//!
//! | what | address | |
//! |---|---|---|
//! | Decide (slot 122) | 0x141360000 | the combo: [`decide`] |
//! | slot 136 | 0x141352140 | Attack's warp point 0x141337320 with 200.0 for the drop below the target (the base: 100) |
//! | the M_D414 chain | 0x141366300 | at M_D414's end: back to the start point (+0x240, swept), m_bFaceAt, M_D421 |
//!
//! PlayAttack (slot 120 0x1413378B0) is the Attack class's, cases 0xBE .. 0xC6: here from the rows `attack.<kind>` of
//! `sheets/port/sora-form-rage.json` (tools/sora_form_rage.js). MaxCombo is 5, Finishers 1 (0x141433570 / 0x141433A60
//! case 0x8B). The attack request in the form is `try_attack` below (the form package's `attack` hook, combat/forms): KH3's
//! TryAttack 0x141437330 with the class the form asks for; the rows of `CanAttack` are attack.rs's.
//!
//! (weapons F-RAGE) Also here: the two STYLEFINISH bodies of Rage Form (TresPlayerState_StyleFinish, id 33, by command):
//! FINISH_AN 0x1413DEBC0 (M_D418) and RISKCHARGE 0x1413E2A10 (M_D419).

use super::attack::Attack;
use super::common::{ground_probe, on_ground};
use super::style_finish::StyleFinish;
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, Notifies, PlayArgs};
use crate::combat::forms::rage::{self as rg, FINISH_AN, RISKCHARGE};
use crate::combat::{form, tables::P};
use crate::enums::ETresAbilityKind;
use crate::host::TargetId;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};
use glam::Vec3;

/// The kinds of COMBO_ANTI (PlayAttack 0x1413378B0).
pub mod kind {
    pub const CRIMSON_UPPER: u8 = 0xBF;
    pub const VERTICAL_DROP: u8 = 0xC0;
    pub const BLADE_TURN: u8 = 0xC1;
    pub const WARP_THRUST: u8 = 0xC2;
    pub const SPEED_SLASH: u8 = 0xC3;
    pub const VERTICAL_SLASH: u8 = 0xC4;
    pub const CRIMSON_BLAST: u8 = 0xC5;
}

/// What Decide (0x141360000) reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AntiDecideIn {
    /// state+0x128.
    pub combo: i32,
    /// state+0x154.
    pub reach: u8,
    /// pawn+0xE10 (vt+0x1DC8).
    pub last_kind: u8,
    /// state+0x1CC / +0x1D0 / +0x1E0 / +0x142.
    pub dist: f32,
    pub dz: f32,
    pub target_high: bool,
    pub air: bool,
    /// state+0x1C4: lock-on targets within 400 cm.
    pub num_enemies: i32,
    /// state+0x1E8 != 0: a forced kind (no approach warp).
    pub forced: bool,
    /// pawn+0xE14 (vt+0x1D08).
    pub finish_count: i32,
}

/// What it leaves behind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AntiDecision {
    pub kind: u8,
    pub reach: u8,
    pub finish: bool,
    pub finish_end: bool,
    pub finish_count: i32,
    /// combo 0: pawn+0xE10 = 0 (vt+0x1D40(0)).
    pub last_reset: bool,
    /// +0x1E1 = 1 with +0x25C = 0.5: the approach warp (a far / high / below target) and its invincibility.
    pub approach: bool,
}

/// `TresPlayerState_ComboAnti::Decide` (0x141360000), the numbers from the sheet's `rule.decide_near` / `rule.combo`.
pub fn decide(i: &AntiDecideIn) -> AntiDecision {
    let near = rg::rule("decide_near");
    let (near, far, below) = (near.first().copied().unwrap_or(400.0), near.get(1).copied().unwrap_or(1000.0), near.get(2).copied().unwrap_or(-100.0));
    let (max, fin) = rg::combo_limits();
    let mut d = AntiDecision { kind: 0, reach: i.reach, finish: false, finish_end: false, finish_count: i.finish_count, last_reset: false, approach: false };
    let mut last = i.last_kind;
    if i.combo == 0 {
        if i.dist > near {
            d.reach = 1;
        } else if i.num_enemies > 1 {
            d.reach = 2;
        }
        last = 0;
        d.last_reset = true;
    }
    d.approach = !i.forced && i.dist <= far && (i.dist > near || i.target_high || (i.air && i.dz < below));
    if i.combo >= max - fin {
        // the finisher: Crimson Blast; the combo's end with the last finisher or the last step.
        d.finish = true;
        d.finish_end = fin - 1 <= i.finish_count || max - 1 <= i.combo;
        d.finish_count = i.finish_count + 1;
        d.kind = kind::CRIMSON_BLAST;
        return d;
    }
    let first = if i.air { kind::SPEED_SLASH } else { kind::CRIMSON_UPPER };
    d.kind = if i.combo == 0 {
        if d.reach != 2 { first } else { kind::BLADE_TURN }
    } else if i.combo > 2 {
        // +0x240 = the start point, +0x1E1 = 1, +0x25C = 0 (no invincibility), +0x22C = 0x11: the warp thrust.
        d.approach = false;
        kind::WARP_THRUST
    } else {
        match last {
            kind::CRIMSON_UPPER => kind::VERTICAL_DROP,
            kind::VERTICAL_DROP | kind::VERTICAL_SLASH => kind::BLADE_TURN,
            kind::BLADE_TURN => first,
            kind::SPEED_SLASH => kind::VERTICAL_SLASH,
            _ if d.reach != 2 => if i.air { kind::VERTICAL_SLASH } else { kind::VERTICAL_DROP },
            _ => first,
        }
    };
    d
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComboAnti {
    /// The Attack state's fields.
    pub(crate) a: Attack,
    /// +0x240: where he stood before the warp (M_D414 comes back to it).
    start: Vec3,
    /// +0x25C: the invincibility of the approach warp (> 0 counting; -1 ran out; 0 none).
    invincible: f32,
    /// M_D421 was played (the return of M_D414).
    returned: bool,
    /// (AUDIT-C) +0x1E1: the approach warp is pending (the vanish runs: JumpNormalStart, +0x258 = 0.2 s).
    pending: bool,
}

impl ComboAnti {
    /// Reset (the Attack class's).
    pub(crate) fn new(a: Attack) -> ComboAnti {
        ComboAnti { a, start: Vec3::ZERO, invincible: 0.0, returned: false, pending: false }
    }

    /// The warp (Attack 0x141337320 through ComboAnti slot 136): 150 cm short of the target's lock point on the line from
    /// him, 200 cm below it, each leg swept against the level (as the chase warp of attack.rs); then he faces it.
    fn warp(&mut self, cx: &mut Ctx) {
        let Some(t) = self.a.target_info(cx) else { return };
        let w = rg::rule("warp");
        let (dist, drop) = (w.first().copied().unwrap_or(150.0), w.get(1).copied().unwrap_or(200.0));
        let own = cx.pawn.cmc.location;
        self.start = own;
        let dir = ((t.center - own) * Vec3::new(1.0, 1.0, 0.0)).normalize_or_zero();
        // (F-CLAWS for F-RAGE) ComboAnti slot 136 0x141352140 = the Attack's 0x141337320 with 200, straight mode (state 0x56):
        // the shared slot-136 line tests from HIM (states/attack.rs `warp_point`), placed swept (`swept_warp`)
        let end = super::attack::warp_point(cx, &t, dist, drop);
        cx.pawn.cmc.location = super::attack::swept_warp(cx, end);
        cx.pawn.cmc.stop_movement();
        if dir != Vec3::ZERO {
            cx.pawn.yaw = wrap_degrees(dir.y.atan2(dir.x).to_degrees());
        }
    }

    /// PlayAttack (slot 120 0x1413378B0) of the Rage kinds 0xBE .. 0xC6 (attack.rs `play_attack` for the others).
    fn play(&mut self, cx: &mut Ctx) {
        let kind = self.a.kind;
        let Some(row) = rg::attack_row(kind) else {
            // A forced kind (flow attacks, counters): the Attack class's own case.
            self.a.play_attack(cx);
            return;
        };
        if row.flags.contains(&"warp") {
            // case 0xC2: the homing target targetable -> +0x240 = his root, the warp (slot 136), +0x21C = 3.
            self.warp(cx);
        }
        let pawn = &mut *cx.pawn;
        pawn.combat.last_kind = kind;
        if pawn.combat.history.len() >= 16 {
            pawn.combat.history.remove(0);
        }
        pawn.combat.history.push(kind);
        let a = &mut self.a;
        a.face_at = true;
        (a.chan_xy, a.chan_root) = (row.chan_xy, row.chan_root);
        (a.stop_dist, a.z_gain, a.air_mode, a.keep_face) = (row.stop_dist, row.z_gain, row.air_mode, row.keep_face);
        a.root_len = row.root_len;
        // The tail (LAB_14133A87A): ground: gravity on, DirZ 0; air mode (every Rage kind): +0x234 |= 4 | 0x10,
        // GravityScale 0.1, Flying, +0x18C = 0.6.
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
        pawn.root_scale = (1.0, 1.0);
        match AnimId::from_name(row.clip) {
            Some(anim) => pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
            None => pawn.note("a Rage Form attack clip is not in the generated animation table (tools/gen_port.js FORM_ANIM_SETS)"),
        }
        a.face_yaw = pawn.yaw;
        a.sync_ctx(pawn);
    }

    /// 0x141366300: at M_D414's end (anim vt+200) back to the start point (swept), m_bFaceAt, M_D421 (blend -1), voice 0.
    fn warp_return(&mut self, cx: &mut Ctx) {
        if self.a.kind != kind::WARP_THRUST || self.returned || !cx.pawn.anim.ended {
            return;
        }
        let Some(row) = rg::chain_row() else { return };
        self.returned = true;
        let p = *cx.pawn.params();
        let centre = Vec3::Z * p.capsule_center_z;
        let from = cx.pawn.cmc.location;
        let back = match cx.world.sweep_capsule(from + centre, self.start + centre, p.capsule_radius, p.capsule_half_height) {
            Some(hit) if !hit.start_penetrating => from + (self.start - from) * hit.time.clamp(0.0, 1.0),
            Some(_) => from,
            None => self.start,
        };
        cx.pawn.cmc.location = back;
        self.a.face_at = true;
        if let Some(id) = AnimId::from_name(row.clip) {
            cx.pawn.anim.play_with(id, PlayArgs::mode(2, -1.0));
            cx.set_voice_group_no(0);
        }
    }
}

impl PlayerState for ComboAnti {
    const ID: StateId = StateId::ComboAn;

    fn sub_step(&self) -> u8 {
        self.a.sub
    }

    /// Attack's Begin 0x141333210 with ComboAnti's Decide, then PlayAttack (the approach warp first: Attack's Tick
    /// 0x14134AA80 makes it before PlayAttack, +0x258 being 0).
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
        // ---- Decide (0x141360000)
        let pawn = &mut *cx.pawn;
        let input = AntiDecideIn {
            combo: self.a.combo,
            reach: self.a.reach,
            last_kind: pawn.combat.last_kind,
            dist: self.a.dist,
            dz: self.a.dz,
            target_high: self.a.target_high,
            air: self.a.air,
            num_enemies: self.a.num_enemies,
            forced: self.a.forced_kind != 0,
            finish_count: pawn.combat.finish_count,
        };
        let d = decide(&input);
        if d.last_reset {
            pawn.combat.last_kind = 0;
            pawn.combat.stocks = (0, 0);
        }
        (self.a.kind, self.a.reach) = (d.kind, d.reach);
        (self.a.finish, self.a.finish_end) = (d.finish, d.finish_end);
        pawn.combat.finish_count = d.finish_count;
        pawn.combat.finish_end |= d.finish_end;
        if self.a.forced_kind != 0 {
            self.a.kind = self.a.forced_kind;
        }
        // (AUDIT-B W-CORE) the Attack's Begin 0x141333210 after Decide: Final Blow's chase (+0x22C = 0xD over Decide's 0x11).
        if self.a.chase_begin(cx) {
            // (AUDIT-B W-DATA) W-CORE's open item: Decide's +0x25C = 0.5 (the approach warp's invincibility, 0x141360000) is KEPT
            // when the chase takes the warp over (Begin 0x141333210 rewrites only the kind, +0x1E1 and +0x22C), so the Attack's
            // Tick counts it down after the chase's teleport: the invincibility ends 0.5 s after it (or at TresEnableChange)
            if d.approach {
                self.invincible = rg::rule("warp").get(2).copied().unwrap_or(0.5);
            }
            return;
        }
        // ---- the approach warp (+0x1E1, +0x25C = 0.5): the teleport, then invincible until 0.5 s or TresEnableChange.
        if d.approach && self.a.target_info(cx).is_some() {
            self.invincible = rg::rule("warp").get(2).copied().unwrap_or(0.5);
            cx.pawn.status.damage_invalid = true;
            // (AUDIT-C) PlayAttack's head with +0x1E1 (0x1413378B0 lines 95 .. 112): JumpNormalStart, Flying, invincible, the
            // vanish's aura (anim interface vt+0x1A0(m_CmnAuraEffect, +0x22C = 0x11, 0): ead_pc_aura_com0 group 17) and
            // +0x258 = 0.2 s; slot 117 0x14134AA80 then warps (slot 136) and starts group 0x12 (F-RAGE had the warp at once).
            let pawn = &mut *cx.pawn;
            pawn.cmc.gravity_off = true;
            pawn.cmc.set_flying();
            pawn.anim.play_with(AnimId::JumpNormalStart, PlayArgs::mode(2, -1.0));
            self.a.warp_delay = crate::combat::tables::P.chase_warp_delay;
            self.pending = true;
            rg::warp_aura(cx, rg::WARP_AURA_GROUP);
            cx.set_voice_group_no(crate::host::audio::attack_voice_group_no(self.a.kind as i32));
            self.a.sub = 1;
            return;
        }
        // ---- PlayAttack
        self.play(cx);
        cx.set_voice_group_no(crate::host::audio::attack_voice_group_no(self.a.kind as i32));
        self.a.sub = 1;
    }

    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        // (AUDIT-C) the vanish (+0x1E1): +0x258 counts down; at 0 slot 117: the warp, group 0x12, then PlayAttack proper.
        if self.pending {
            self.a.warp_delay -= dt;
            if self.a.warp_delay > 0.0 {
                return;
            }
            self.a.warp_delay = 0.0;
            self.pending = false;
            self.warp(cx);
            rg::warp_aura(cx, rg::WARP_AURA_GROUP + 1);
            self.play(cx);
            return;
        }
        self.warp_return(cx);
        // Attack's Tick 0x141341340: +0x25C counts down unless a warp is pending; at -1 or TresEnableChange the
        // invincibility (pawn vt+0x2060 / vt+0x2068) ends.
        // (AUDIT-B W-DATA) "unless a warp is pending": the chase's countdown (attack.rs `warp_delay`) holds it too
        if self.invincible > 0.0 && self.a.warp_delay <= 0.0 {
            self.invincible -= dt;
            if self.invincible <= 0.0 {
                self.invincible = -1.0;
            }
        }
        if (cx.pawn.queue.flags.enable_change || self.invincible == -1.0) && cx.pawn.status.damage_invalid {
            cx.pawn.status.damage_invalid = false;
            self.invincible = 0.0;
        }
        self.a.tick(cx, dt);
    }

    fn end(&mut self, cx: &mut Ctx) {
        // (AUDIT-C) End 0x141335120 with the warp still pending: the reappearance's aura (vt+0x1A0(.., +0x22C + 1, 0)).
        if std::mem::take(&mut self.pending) {
            rg::warp_aura(cx, rg::WARP_AURA_GROUP + 1);
        }
        // Attack's End. Its chain test knows the classes Attack / ComboLimit / FireMagic as "the chain goes on": with a
        // COMBO_ANTI queued (KH3: any attack class) the chain memory it would reset is kept, and the counters pass on.
        let queued = cx.pawn.queue.queued_id() == Some(StateId::ComboAn);
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
        if self.invincible != 0.0 {
            cx.pawn.status.damage_invalid = false;
        }
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        // (AUDIT-C) COMBO_ANTI's own shot handler (anim interface slot 69 0x141357AF0: combat/forms/rage.rs `shot`) for each
        // TresAnimNotify_Shot (its LocationIndex); kind 0x37 stays the Attack's.
        for event in &notifies.events {
            if (event.class(), event.phase) != (crate::anim::NotifyClass::TresAnimNotifyShot, crate::anim::NotifyPhase::Instant) {
                continue;
            }
            let index = match event.def().data {
                crate::anim::NotifyData::Shot { location_index, .. } => location_index.unwrap_or(0),
                _ => 0,
            };
            self.a.sync_ctx(cx.pawn);
            if rg::shot(cx, self.a.kind, index) {
                self.a.shots += 1;
            }
        }
        self.a.on_notifies(cx, notifies);
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// `CanAttack` (slot 44 0x141426330) in Rage Form: attack.rs's rows with COMBO_ANTI as the attack class and MaxCombo 5.
/// None = a state whose row is attack.rs's own business (the traversal states: their forced flow kinds).
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
    let max = rg::combo_limits().0;
    let chain = |a: &Attack| s.queue.nothing_queued() && !a.next_queued && a.swing_started && a.enable_input && a.combo < max && s.combat.hit_flag && !s.combat.finish_end;
    Some(match state {
        StateData::ComboAnti(c) => chain(&c.a),
        StateData::Attack(a) => chain(a),
        StateData::ComboLimit(c) => chain(&c.0),
        StateData::FireMagic(m) => m.can_attack(s, max) && s.combat.hit_flag && !s.combat.finish_end,
        StateData::UseItem(_) => s.queue.flags.enable_change,
        StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_) | StateData::Glide(_) | StateData::Floating(_) | StateData::Swaying(_) => true,
        StateData::DodgeRoll(_) | StateData::AirSlide(_) => s.queue.flags.enable_change,
        StateData::Damage(d) => s.queue.nothing_queued() && !d.bad && matches!(d.kind.value(), 1 | 2 | 7 | 8 | 9 | 10) && s.queue.flags.enable_change,
        StateData::BadstatStun(_) => s.queue.nothing_queued() && s.queue.flags.enable_change,
        StateData::WallRun(_) | StateData::Hopping(_) | StateData::SlopeSlide(_) | StateData::SuperSlide(_) | StateData::WallKick(_) | StateData::PoleTurn(_) | StateData::RailSlide(_) => return None,
        _ => false,
    })
}

/// `TryAttack(target, 0)` (slot 45 0x141437330) in Rage Form: the request queues COMBO_ANTI (the form package's `attack`
/// hook). None = attack.rs's own path (the traversal states). No Keyblade switch in the form (P2), so no WEAPONCHANGE row.
pub(crate) fn try_attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    // (XSI, kept in the form) Rage Raid offered: Y / Attack plays it. (AUDIT-C) KH3's own in-form counter, kind 0xC6 (the
    // same clip M_D420 with AN_COUNTER), is FOUND: the guard's back slide (OnGuardedHit 0x14143CAF0 case 0x8B -> AIR_SLIDE flag
    // 0x100: reaction command 0xD, +0xB0 = 0xC6) forces it through combat/guard.rs `forced_attack` below.
    let raid = crate::abilities::xs::rage::raid_due(cx);
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
    let chained = match state {
        StateData::ComboAnti(c) => Some(&mut c.a),
        StateData::Attack(a) => Some(a),
        StateData::ComboLimit(c) => Some(&mut c.0),
        _ => None,
    };
    if let Some(cur) = chained {
        cur.next_queued = true;
        let next = Attack { combo: cur.combo + 1, air: cur.air, target, reach: cur.reach, ..Attack::RESET };
        s.combat.combo = next.combo;
        s.queue.push_state(ComboAnti::new(next));
    } else if let StateData::FireMagic(m) = state {
        let (combo, air) = m.attack_follows();
        s.combat.combo = combo;
        s.queue.push_state(ComboAnti::new(Attack { combo, air, target, ..Attack::RESET }));
    } else {
        // IDLE, RUN, JUMP, GUARD ...: SetState (RISKDODGE / AIR_SLIDE: PushState). A new combo starts at 0.
        let (target, forced_kind) = match raid {
            Some(t) => (t, crate::abilities::xs::rage::RAGE_RAID),
            None => (target, forced_kind),
        };
        let next = ComboAnti::new(Attack { combo: 0, air: !on_ground(s), target, forced_kind, just_guard, ..Attack::RESET });
        s.combat.combo = 0;
        if push {
            s.queue.push_state(next);
        } else {
            s.queue.set_state(next);
        }
    }
    Some(true)
}

// ================================================================================================ FINISH_AN / RISKCHARGE

/// STYLEFINISH Begin for FINISH_AN / RISKCHARGE (after the begin common to every finish command).
pub(crate) fn finish_begin(f: &mut StyleFinish, _cx: &mut Ctx) {
    f.a.sub = 0;
    f.a.shots = 0;
}

/// The per-command tick: true = done (chained on itself).
pub(crate) fn finish_tick(f: &mut StyleFinish, cx: &mut Ctx, _dt: f32) -> bool {
    match f.cmd {
        FINISH_AN => finish_an(f, cx),
        RISKCHARGE => riskcharge(f, cx),
        _ => true,
    }
}

/// Play `row`'s clip (mode 2, blend -1) with an effect group, m_bFaceAt.
fn play_finish(f: &mut StyleFinish, cx: &mut Ctx, cmd: i32, group: i32) {
    let Some(row) = rg::finish_row(cmd) else { return };
    if group != 0 {
        cx.pawn.next_effect_group = Some(group);
    }
    match AnimId::from_name(row.clip) {
        Some(clip) => cx.pawn.anim.play_with(clip, PlayArgs::mode(2, -1.0)),
        None => cx.pawn.note("a Rage Form finish clip is not in the generated animation table (tools/gen_port.js FORM_ANIM_SETS)"),
    }
    f.a.swing_started = false;
    f.a.face_at = true;
}

/// The shot of a finish clip (`shot <time>@<name>` of its row): FINISH_AN's AN_FINISH (row AN_FINISH_BOMB).
fn finish_shot(cmd: i32) -> Option<f32> {
    rg::finish_row(cmd)?.effects.iter().find_map(|e| e.strip_prefix("shot ")?.split_once('@').and_then(|(t, _)| t.parse().ok()))
}

/// The AN_FINISH projectile's particle (TresProjAN_Finish ParticleComp; the actor itself: rage.rs `spawn_shot`, AUDIT-C).
pub(crate) const FINISH_BLAST_PARTICLE: &str = "vp_ant_fin_tog00"; // (F-RAGE fix) pub(crate): rage::effect_names

/// FINISH_AN (StyleFinish 0x1413DEBC0): M_D418.
fn finish_an(f: &mut StyleFinish, cx: &mut Ctx) -> bool {
    match f.a.sub {
        0 => {
            play_finish(f, cx, FINISH_AN, 0);
            f.a.sub = 1;
            return false;
        }
        1 if f.a.swing_started => {
            // the first hit window: ClientPlayForceFeedback(1).
            cx.pawn.event(crate::host::EventKind::ForceFeedback { effect: 1, looping: false });
            f.a.sub = 5;
        }
        _ => {}
    }
    if matches!(f.a.sub, 1 | 5) {
        // the Shot notify (m_ProjectileName AN_FINISH): the projectile at the shot location; "more than 7 shots (+0x170)
        // -> WorldSlow off" (the clip spawns one: the end event ends the slow, as in KH3).
        if f.a.shots == 0 && finish_shot(FINISH_AN).is_some_and(|t| cx.pawn.anim.time >= t) {
            f.a.shots = 1;
            // (AUDIT-C) the projectile set's actor from its blueprint (sheets/port/sora-combat-shots.json AN_FINISH#0: SPHERE 150 at
            // (0, 0, 150), life 2, row AN_FINISH_BOMB, particle vp_ant_fin_tog00 at its root, HIT EFFECT vp_ex002_mag_hit00),
            // instead of the hand-made sphere without its hit effect.
            let at = crate::combat::hit::shot_origin(cx);
            rg::spawn_shot(cx, "AN_FINISH", at);
        }
        if f.a.shots > 7 {
            form::world_slow(cx, false);
        }
    }
    finish_tail(f, cx)
}

/// RISKCHARGE (StyleFinish 0x1413E2A10): M_D419 with the effect group Riskcharges + 9.
fn riskcharge(f: &mut StyleFinish, cx: &mut Ctx) -> bool {
    match f.a.sub {
        0 => {
            let n = cx.pawn.combat.form.rage.riskcharges as i32;
            let base = rg::rule("riskcharge_group").first().copied().unwrap_or(9.0) as i32;
            play_finish(f, cx, RISKCHARGE, if n > 0 { n + base } else { 0 });
            // (AUDIT-C) the charge is M_D419's own group-n+9 notifies; the eyes come at the End (rage.rs `riskcharge_ended`)
            f.a.sub = 1;
            return false;
        }
        1 if f.a.swing_started => {
            form::world_slow(cx, false);
            cx.pawn.event(crate::host::EventKind::ForceFeedback { effect: 1, looping: false });
            f.a.sub = 2;
        }
        _ => {}
    }
    finish_tail(f, cx)
}

/// Both bodies' tail: at TresEnableChange gravity on (vt+0x1728(1)), Falling (vt+0x1658), sub 12; at the clip's end DoChain.
fn finish_tail(f: &mut StyleFinish, cx: &mut Ctx) -> bool {
    if f.a.sub != 12 && cx.pawn.queue.flags.enable_change {
        cx.pawn.cmc.gravity_off = false;
        cx.pawn.cmc.set_falling();
        f.a.sub = 12;
    }
    if cx.pawn.anim.ended {
        return f.finish(cx.pawn);
    }
    false
}

// (F-RAGE) the form's path: the offer, the transformation's HP, RISKCHARGE x 3 (HP halved, the form goes on), FINISH_AN,
// the end (cooldown, counts), no Keyblade switch, the battle's reset; COMBO_ANTI's decision.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::{AnimId, AnimPlayer};
    use crate::cmc::Cmc;
    use crate::combat::{situation, style};
    use crate::host::NullHost;
    use crate::params::SORA_PARAMS;
    use crate::test_world::TestWorld;
    use rg::CHANGE_AN;

    fn sora() -> Pawn {
        let mut p = Pawn::new(SORA_PARAMS, Cmc::new(&SORA_PARAMS, Vec3::ZERO), 0.0, AnimPlayer::new(AnimId::IdleN));
        let (slots, notes) = crate::status::Keyblades::from_keys(&[Some(form::KINGDOM_KEY), Some("WEP_KEYBLADE_SO_02")], 0, &[]);
        assert!(notes.is_empty(), "{notes:?}");
        p.status.set_keyblades(slots);
        // MaxHP 160 (a bare status has none: the base is the difficulty row's).
        p.status.hp_base = 160;
        p.status.hp = p.status.max_hp();
        p
    }

    #[test]
    fn rage_form_offer_riskcharge_finish_and_end() {
        let mut p = sora();
        let max = p.status.max_hp();
        assert!(max >= 8, "MaxHP {max}");
        let world = TestWorld::floor();
        let mut null = NullHost::default();
        let mut host = null.host(&world);
        let mut state = StateData::DEFAULT;
        let mut cx = Ctx::new(&mut p, &mut host);
        // THE OFFER: a loss to 25 % or less is rolled; above it, or with the cooldown, it is not.
        style::tick(&state, &mut cx, 0.0);
        cx.pawn.status.hp = max / 2;
        style::tick(&state, &mut cx, 0.0);
        assert_eq!(cx.pawn.combat.form.rage.rolls, 0, "above 25 %: no roll");
        cx.pawn.combat.form.rage.cooldown = 10.0;
        cx.pawn.status.hp = 1;
        style::tick(&state, &mut cx, 0.0);
        assert_eq!(cx.pawn.combat.form.rage.rolls, 0, "the cooldown blocks it");
        cx.pawn.combat.form.rage.cooldown = 0.0;
        let mut tries = 0;
        while !cx.pawn.combat.situation.has(CHANGE_AN) && tries < 200 {
            cx.pawn.status.hp = max;
            style::tick(&state, &mut cx, 0.0);
            cx.pawn.status.hp = 1;
            style::tick(&state, &mut cx, 0.0);
            tries += 1;
        }
        assert!(cx.pawn.combat.situation.has(CHANGE_AN), "a 34 % roll comes up within 200 losses");
        assert_eq!((cx.pawn.combat.form.rage.rolls, cx.pawn.combat.form.rage.offers), (tries, 1));
        // THE FORM: Y on the situation command; the first form frame after the transformation: HP = MaxHP, count 1.
        assert!(situation::execute(&mut state, &mut cx, CHANGE_AN));
        assert_eq!(cx.pawn.combat.form.cmd, CHANGE_AN);
        cx.pawn.queue = super::super::host::Queue::new(); // a running state, nothing queued
        style::tick(&state, &mut cx, 0.016);
        assert_eq!((cx.pawn.status.hp, cx.pawn.combat.form.rage.uses, cx.pawn.combat.form.rage.riskcharges), (max, 1, 0));
        // NO KEYBLADE SWITCH in Rage Form (P2: CanWeaponChange, controller+0xB38 == CHANGE_AN).
        assert!(!crate::states::weapon_change::can_weapon_change(&state, &cx));
        // RISKCHARGE x 3: each halves HP (at least 1), keeps the form's time; the fourth full gauge gives FINISH_AN.
        let time = cx.pawn.combat.form.time_left;
        let mut hp = max;
        for n in 1..=3 {
            assert!(form::set_style_finish_command(&mut cx, true, false));
            assert!(cx.pawn.combat.situation.has(RISKCHARGE), "Riskcharge {n} offered");
            assert!(situation::execute(&mut state, &mut cx, RISKCHARGE), "Riskcharge {n} taken");
            hp = (hp / 2).max(1);
            let f = cx.pawn.combat.form;
            assert_eq!((cx.pawn.status.hp, f.rage.riskcharges, f.cmd, f.time_left), (hp, n, CHANGE_AN, time), "Riskcharge {n}");
            // its STYLEFINISH ends: the form goes on (the end event skips slot 859 for 0xB3).
            let mut body = StyleFinish::new(RISKCHARGE);
            body.end(&mut cx);
            assert_eq!(cx.pawn.combat.form.cmd, CHANGE_AN, "Riskcharge {n}: still in the form");
            cx.pawn.queue = super::super::host::Queue::new(); // a running state, nothing queued
            cx.pawn.combat.form.finish_pending = false;
        }
        assert!(form::set_style_finish_command(&mut cx, true, false));
        assert!(cx.pawn.combat.situation.has(FINISH_AN) && form::current_finish(&cx) == FINISH_AN, "after three: FINISH_AN");
        assert!(!situation::execute(&mut state, &mut cx, RISKCHARGE), "no fourth Riskcharge");
        // FINISH_AN: the finish branch zeroes the form's time; its end ends the form: cooldown 90 s, Riskcharges 0.
        assert!(situation::execute(&mut state, &mut cx, FINISH_AN));
        assert_eq!(cx.pawn.combat.form.time_left, 0.0);
        let mut body = StyleFinish::new(FINISH_AN);
        body.end(&mut cx);
        let f = cx.pawn.combat.form;
        assert_eq!((f.cmd, f.wear_form, f.rage.riskcharges, f.rage.cooldown, f.rage.uses), (0, 0, 0, 90.0, 1));
        // the second Rage of the battle is rolled with f = 30; the battle's end puts the count back to 0.
        cx.pawn.actions.battle = true;
        style::tick(&state, &mut cx, 0.0);
        cx.pawn.actions.battle = false;
        style::tick(&state, &mut cx, 0.0);
        assert_eq!(cx.pawn.combat.form.rage.uses, 0);
        // the cooldown runs down with the frames.
        style::tick(&state, &mut cx, 1.0);
        assert_eq!(cx.pawn.combat.form.rage.cooldown, 89.0);
        // HP 1 stays 1.
        cx.pawn.status.hp = 1;
        rg::riskcharge_taken(&mut cx);
        assert_eq!(cx.pawn.status.hp, 1);
    }

    fn chain(air: bool, reach_crowd: i32) -> Vec<u8> {
        let mut out = Vec::new();
        let (mut last, mut reach, mut fc) = (0, 0, 0);
        for combo in 0..5 {
            let d = decide(&AntiDecideIn { combo, reach, last_kind: last, dist: 100.0, dz: 0.0, target_high: false, air, num_enemies: reach_crowd, forced: false, finish_count: fc });
            (last, reach, fc) = (d.kind, d.reach, d.finish_count);
            out.push(d.kind);
            if combo == 4 {
                assert!(d.finish && d.finish_end, "the one finisher ends the combo");
            }
        }
        out
    }

    // Decide 0x141360000: ground BF C0 C1 C2 C5, air C3 C4 C1 C2 C5, a crowd (reach 2) opens with Blade Turn; the approach.
    #[test]
    fn combo_anti_decides_as_kh3() {
        assert_eq!(chain(false, 1), [0xBF, 0xC0, 0xC1, 0xC2, 0xC5]);
        assert_eq!(chain(true, 1), [0xC3, 0xC4, 0xC1, 0xC2, 0xC5]);
        assert_eq!(chain(false, 3), [0xC1, 0xBF, 0xC0, 0xC2, 0xC5]);
        let d = |dist: f32, high: bool, air: bool, dz: f32| decide(&AntiDecideIn { combo: 0, reach: 0, last_kind: 0, dist, dz, target_high: high, air, num_enemies: 1, forced: false, finish_count: 0 }).approach;
        assert_eq!([d(300.0, false, false, 0.0), d(500.0, false, false, 0.0), d(1200.0, false, false, 0.0), d(300.0, true, false, 0.0), d(300.0, false, true, -150.0)], [false, true, false, true, true]);
    }
}
