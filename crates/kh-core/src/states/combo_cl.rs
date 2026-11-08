//! (weapons F-CLAWS) The attack states of the claw / yo-yo / blade forms (pawn slot 1337 0x141432F00): COMBO_CL (id 85:
//! Agile Claws 0x8A and Blizzard Claws 0x90 share the class), COMBO_YO (77: Twin Yo-Yos 0x82), COMBO_SK (80: Blizzard
//! Blades 0x85). All three are subclasses of `TresPlayerState_Attack` (attack.rs, which they wrap) and KH3 shares
//! everything but these between them (vtables ComboCL 0x14690CB10, ComboYO 0x146914228, ComboSK 0x146911BD8 against
//! ComboLimit's 0x146910B80, which overrides only Decide):
//!
//! | class | slot | address | what |
//! |---|---|---|---|
//! | all | 122 Decide | CL 0x141360440, YO 0x1413646B0, SK 0x141363570 | `combat::forms::claws::decide_*` |
//! | CL | iface 13 (attack-window begin) | 0x141356F10 | the base handler, then in form 0x90 the rows SCCL_* -> CL2_* (combat/hit.rs) |
//! | YO | 98 End | 0x141350900 | with +0x234 bit 11 (the pull): pawn vt+0x1010(1, 0) (body collision on), then Attack's End |
//! | YO | 136 | 0x141337890 | the warp of slot 136 with 100 (the drop under the point) |
//! | YO | iface 15 (on contact) | 0x1413561B0 | the base handler; kind 0x97: m_bEnableChange; kind 0x96 (Anchor Shot) on its target: the pull (below) |
//! | SK | iface 15 | 0x141355DD0 | the base handler; kind 0xAF on a victim whose bounds are at least 220 cm tall: M_7422 |
//!
//! PlayAttack (slot 120 0x1413378B0) is the Attack class's, cases 0x88 .. 0x91, 0x92 .. 0x9B, 0xAA .. 0xB4: here from the
//! rows `attack.<form>.<kind>` of `sheets/port/sora-form-claws.json`. The attack request in the forms is [`try_attack`]
//! (the form package's `attack` hook): KH3's TryAttack 0x141437330 with the class the form asks for.
//!
//! Also here (the Attack state's fields are this module's): the bodies of FINISH_YO (STYLEFINISH 33, slot 149 0x1413E0CD0)
//! and FINISH_SK (`TresPlayerState_StyleFinishSK` 35: Begin 0x1413CC360, Tick 0x1413D6F30, End 0x1413CF2D0; the framework
//! queues STYLEFINISH 33 for every finish command and runs the body through the package's finish hooks).

use glam::Vec3;

use super::attack::{swept_warp, warp_point, Attack};
use super::common::{ground_probe, on_ground};
use super::style_finish::StyleFinish;
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, Notifies, PlayArgs};
use crate::combat::forms::claws::{self as cl, ClawsDecideIn};
use crate::combat::{form, tables::P};
use crate::enums::ETresAbilityKind as Ab;
use crate::host::{TargetFlags, TargetId};
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};

/// The fields one of the three classes adds to the Attack state's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FormCombo {
    /// The Attack state's fields.
    pub(crate) a: Attack,
    /// pawn+0xE24 when this tick's contacts were last looked at (the on-contact handlers of iface slot 15 run on the hits
    /// combat/hit.rs landed since).
    hits_seen: i32,
    /// +0x234 bit 11: Anchor Shot's pull was made (YO End: the body collision back on; the invincibility ends).
    pulled: bool,
    /// (F-POLISH) pawn+0x1560 / +0x155C: Anchor Shot's warp point, set by the on-contact handler, applied by the pawn tick of
    /// the next frame (0x141448AA0's head: FUN_143EAC3D0).
    warp: Option<Vec3>,
}

impl FormCombo {
    pub(crate) fn new(a: Attack) -> FormCombo {
        FormCombo { a, hits_seen: 0, pulled: false, warp: None }
    }
}

macro_rules! classes {
    ($($ty:ident => $id:ident,)+) => {
        $(
            #[derive(Clone, Copy, Debug, PartialEq)]
            pub(crate) struct $ty(pub(crate) FormCombo);

            impl PlayerState for $ty {
                const ID: StateId = StateId::$id;

                fn sub_step(&self) -> u8 {
                    self.0.a.sub
                }

                fn begin(&mut self, cx: &mut Ctx) {
                    begin(&mut self.0, cx);
                }

                fn tick(&mut self, cx: &mut Ctx, dt: f32) {
                    tick(&mut self.0, cx, dt);
                }

                fn end(&mut self, cx: &mut Ctx) {
                    end(&mut self.0, cx);
                }

                fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
                    self.0.a.on_notifies(cx, notifies);
                }

                fn movement_state(&self, _pawn: &Pawn) -> MovementState {
                    MovementState::Other
                }
            }
        )+
    };
}

classes! {
    ComboCl => ComboCl,
    ComboYo => ComboYo,
    ComboSk => ComboSk,
}

/// The class instance the held form asks for (pawn slot 1337 0x141432F00).
fn instance(cmd: i32, a: Attack) -> Option<StateData> {
    let f = FormCombo::new(a);
    match cmd {
        cl::CHANGE_CL | cl::CHANGE_CL2 => Some(ComboCl(f).into()),
        cl::CHANGE_YO => Some(ComboYo(f).into()),
        cl::CHANGE_SK => Some(ComboSk(f).into()),
        _ => None,
    }
}

impl StateData {
    /// (F-CLAWS) The fields of a running COMBO_CL / COMBO_YO / COMBO_SK.
    pub(crate) fn form_combo(&self) -> Option<&FormCombo> {
        match self {
            StateData::ComboCl(c) => Some(&c.0),
            StateData::ComboYo(c) => Some(&c.0),
            StateData::ComboSk(c) => Some(&c.0),
            _ => None,
        }
    }
}

/// (F-POLISH) The on-contact handlers at KH3's moment: right after the attack-collision manager's pass that made the contacts
/// (combat/hit.rs `tick`, called by sora.rs after it), as KH3 calls iface slot 15 from the hit itself; the pull's warp then
/// lands at the next frame's pawn tick (pawn+0x1560 / +0x155C, 0x141448AA0's head: the next `tick` here).
pub(crate) fn after_hits(state: &mut StateData, cx: &mut Ctx) {
    let f = match state {
        StateData::ComboYo(c) => &mut c.0,
        StateData::ComboSk(c) => &mut c.0,
        StateData::ComboCl(c) => &mut c.0,
        _ => return,
    };
    contacts(f, cx);
}

/// Slot 124 (0x1413325F0) as attack.rs's `flow_refine`: a forced flow kind by the abilities.
fn flow_refine(a: &mut Attack, pawn: &Pawn) {
    let has = |k| pawn.status.abilities.has(k);
    if matches!(a.forced_kind, 0x36 | 0x37) {
        if a.num_enemies >= 2 && has(Ab::TurnCutter) {
            a.forced_kind = 0x38;
        } else if a.dist > 800.0 && has(Ab::SonicSlash) {
            a.forced_kind = 0x36;
        } else if a.dist < a.dz.abs() && has(Ab::SonicDown) {
            a.forced_kind = 0x37;
        }
    }
    let ability = match a.forced_kind {
        0x36 => Ab::SonicSlash,
        0x37 => Ab::SonicDown,
        0x38 => Ab::TurnCutter,
        0x39 => Ab::Summersalt,
        _ => return,
    };
    if !has(ability) {
        a.forced_kind = match a.reach {
            1 => 0x11,
            2 => 0x12,
            3 => 0x0F,
            _ => 0x10,
        };
    }
}

/// Begin: Attack's Begin 0x141333210 with the class's Decide, then PlayAttack.
fn begin(f: &mut FormCombo, cx: &mut Ctx) {
    let a = &mut f.a;
    crate::actions::draw_for_attack(cx, a.combo == 0);
    cx.pawn.actions.last_pose = 1;
    let passed = std::mem::take(&mut cx.pawn.combat.pass_on);
    if a.combo > 0 {
        (a.counters[0], a.counters[3]) = passed;
    }
    a.counters[1] = a.counters[0];
    a.counters[2] = a.counters[3];
    cx.pawn.combat.carry = (0, 0);
    if a.target_info(cx).is_none() {
        a.target = cx.pawn.combat.lockon.current;
    }
    cx.pawn.cmc.stop_movement();
    // PreDecide (slot 121 0x14134C420).
    (a.target_on_ground, a.dist, a.dz) = (true, 0.0, 0.0);
    a.target_metrics(cx);
    a.air = true;
    let (hit, distance, _) = ground_probe(cx.pawn, cx.world, P.decide_ground_probe_range);
    if hit && distance <= P.decide_ground_probe_max {
        a.air = false;
    }
    a.num_enemies = cx.pawn.combat.lockon.crowd;
    f.hits_seen = cx.pawn.combat.hit_count;
    // Decide (slot 122): the flow kinds are refined first (ComboCL's slot 124 call with 0x36; the PlayAttack head for the
    // others).
    if a.forced_kind != 0 {
        flow_refine(a, cx.pawn);
    }
    let pawn = &mut *cx.pawn;
    let cmd = pawn.combat.form.cmd;
    let input = ClawsDecideIn {
        combo: a.combo,
        reach: a.reach,
        last_kind: pawn.combat.last_kind,
        dist: a.dist,
        dz: a.dz,
        target_high: a.target_high,
        air: a.air,
        num_enemies: a.num_enemies,
        wall_run: a.wall_run.is_some(),
        forced_kind: if matches!(cmd, cl::CHANGE_CL | cl::CHANGE_CL2) { a.forced_kind } else { 0 },
        finish_count: pawn.combat.finish_count,
    };
    let Some(d) = cl::decide(cmd, &input) else {
        a.play_attack(cx);
        a.sub = 1;
        return;
    };
    if d.last_reset {
        pawn.combat.last_kind = 0;
    }
    if d.forced_taken {
        a.forced_kind = 0;
    }
    (a.kind, a.reach, a.air, a.finish, a.finish_end) = (d.kind, d.reach, d.air, d.finish, d.finish_end);
    if d.fixed_dir_z {
        a.fixed_dir_z = true;
        if let Some(z) = d.dir_z {
            a.dir_z = z;
        }
    }
    pawn.combat.finish_count = d.finish_count;
    if d.finish_end {
        // pawn vt+0x2740(1): the pawn's finish-end bit.
        pawn.combat.finish_end = true;
    }
    // "if forcedKind(+0x1E8) != 0: kind = forcedKind" (the guard / risk-dodge counters, the flow kinds), with the air kinds'
    // ability variants (the head of PlayAttack).
    if a.forced_kind != 0 {
        let has = |k| pawn.status.abilities.has(k);
        a.forced_kind = match a.forced_kind {
            0x0F if has(Ab::AerialDive) => 0x20,
            0x10 if has(Ab::QuickShaft) => 0x18,
            0x11 if has(Ab::FlashStep) => 0x19,
            0x12 if has(Ab::RadialCut) => 0x1A,
            k => k,
        };
        a.kind = a.forced_kind;
    }
    // Final Blow (Attack Begin: +0x234 & 0x80000 handed over by the chain): the chase warp, as attack.rs.
    // (AUDIT-B W-CORE) attack.rs `chase_begin` (PlayAttack's head: JumpNormalStart, ead_pc_aura_com0 groups 13 / 14).
    if a.chase_begin(cx) {
        return;
    }
    play(a, cx);
    cx.set_voice_group_no(crate::host::audio::attack_voice_group_no(i32::from(a.kind)));
    a.sub = 1;
}

/// PlayAttack (slot 120 0x1413378B0) of the package's kinds (attack.rs `play_attack` for every other kind).
pub(crate) fn play(a: &mut Attack, cx: &mut Ctx) {
    let cmd = cx.pawn.combat.form.cmd;
    let kind = a.kind;
    let Some(row) = cl::attack_row(cmd, kind) else {
        a.play_attack(cx);
        return;
    };
    let pawn = &mut *cx.pawn;
    pawn.combat.last_kind = kind;
    if pawn.combat.history.len() >= 16 {
        pawn.combat.history.remove(0);
    }
    pawn.combat.history.push(kind);
    // The defaults before the switch, then the case.
    a.face_at = true;
    (a.chan_xy, a.chan_root) = (row.chan_xy, row.chan_root);
    (a.stop_dist, a.z_gain, a.air_mode, a.keep_face) = (row.stop_dist, row.z_gain, row.air_mode, false);
    a.root_len = row.root_len;
    let flag = |f: &str| row.flags.contains(&f);
    if flag("dive") {
        a.dive = true;
    }
    if flag("no_vz") {
        a.no_vz = true;
    }
    if flag("keep_dir_z") {
        a.keep_dir_z = true;
    }
    if let Some(&z) = row.values.get(3).filter(|z| **z != 0.0) {
        a.dir_z = z;
    }
    // The tail (LAB_14133A87A): ground: gravity on, DirZ 0; air mode: +0x234 |= 4 | 0x10, GravityScale 0.1, gravity off,
    // Flying, +0x18C = 0.6 (over 0x91's 0.3).
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
    // Play(name, 0, 0, mode 2, +0x198 (Blizzard Claws: 0x7DA, the ice variants of the effects), 1.0, scale).
    if row.group != 0 {
        pawn.next_effect_group = Some(row.group);
    }
    match AnimId::from_name(row.clip) {
        Some(anim) => pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
        None => pawn.note("a claw / yo-yo / blade attack clip is not in the generated animation table (tools/gen_port.js FORM_ANIM_SETS)"),
    }
    a.face_yaw = pawn.yaw;
    a.sync_ctx(pawn);
}

/// The on-contact handlers of iface slot 15 (ComboYO 0x1413561B0, ComboSK 0x141355DD0) for the contacts made since the last
/// tick (combat/hit.rs sweeps after MainProc: the state sees them here), each with ITS victim (F-POLISH: combat/hit.rs
/// `Contact`, the actor touched and how it stood at the contact; NONE = the level), in their order.
fn contacts(f: &mut FormCombo, cx: &mut Ctx) {
    let hits = cx.pawn.combat.hit_count;
    if hits <= f.hits_seen {
        f.hits_seen = hits;
        return;
    }
    let seen = std::mem::replace(&mut f.hits_seen, hits);
    let new: Vec<crate::combat::hit::Contact> = cx.pawn.combat.contacts.iter().filter(|c| c.count > seen).copied().collect();
    for c in new {
        let a = &mut f.a;
        match (cx.pawn.combat.form.cmd, a.kind) {
            // 0x97 (the pull): state+0x84 |= 1, m_bEnableChange.
            (cl::CHANGE_YO, cl::KIND_YO_PULL) => cx.pawn.queue.flags.enable_change = true,
            // 0x96 on the state's own homing target (+0x118): the pull, from where the victim stood at the contact.
            (cl::CHANGE_YO, cl::KIND_YO_ANCHOR) => {
                if let Some(t) = c.info.filter(|t| c.victim == a.target && t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE)) {
                    anchor_pull(f, cx, t);
                }
            }
            // 0xAF on a victim whose bounds' height - 20 is at least 200 (half of it >= 100): M_7422.
            (cl::CHANGE_SK, cl::KIND_SK_AIR) if c.info.is_some_and(|t| (2.0 * t.half_height - 20.0) * 0.5 >= 100.0) => sk_tall_hit(a, cx),
            _ => {}
        }
    }
}

/// Anchor Shot's pull (0x1413561B0, kind 0x96 on its target): slot 136 (ComboYO's: 150 short of the target's lock point on
/// the line from him, 100 under it) gives the point; pawn+0x1560 = it (FUN_1414466F0; the pawn tick 0x141448AA0 sets the
/// actor there); kind 0x97 with M_4416 started (1000 - the distance) x 0.000333 s in, the air mode, invincible
/// (pawn vt+0x2068(1)), +0x234 bit 11.
fn anchor_pull(f: &mut FormCombo, cx: &mut Ctx, t: crate::host::TargetInfo) {
    let own = cx.pawn.cmc.location;
    let dir = ((t.center - own) * Vec3::new(1.0, 1.0, 0.0)).normalize_or_zero();
    // (F-CLAWS) slot 136 as KH3 computes it: line tests from HIM, so the point never lies beyond a wall (warp_point).
    let end = warp_point(cx, &t, P.chase_warp_distance, P.chase_warp_drop);
    let start_time = ((1000.0 - (end - own).truncate().length()) * 0.000_333).max(0.0);
    let a = &mut f.a;
    let pawn = &mut *cx.pawn;
    // (F-POLISH) FUN_1414466F0: the point is set now, the actor put there by the next frame's pawn tick (`tick`).
    f.warp = Some(end);
    pawn.cmc.stop_movement();
    if dir != Vec3::ZERO {
        pawn.yaw = wrap_degrees(dir.y.atan2(dir.x).to_degrees());
    }
    a.kind = cl::KIND_YO_PULL;
    pawn.combat.last_kind = cl::KIND_YO_PULL;
    (a.stop_dist, a.z_gain) = (P.traction_stop_dist, P.traction_z_gain);
    (a.chan_xy, a.chan_root) = (true, false);
    (a.phase_xy, a.phase_root, a.phase_z) = (0, 0, 0);
    a.air_mode = true;
    a.root_len = 0.0;
    pawn.cmc.gravity_scale = P.air_gravity_scale;
    a.hover = true;
    pawn.cmc.gravity_off = true;
    pawn.cmc.set_flying();
    a.air_hold = P.air_hold_time;
    let clip = cl::clip(pawn.combat.form.cmd, "M_4416");
    if let Some(id) = AnimId::from_name(clip) {
        pawn.anim.play_with(id, PlayArgs::mode(2, -1.0));
        if start_time > 0.0 {
            pawn.anim.set_time(start_time);
        }
    }
    a.enable_input = true;
    a.swing_started = true;
    pawn.combat.hit_flag = true;
    a.face_at = true;
    a.face_yaw = pawn.yaw;
    pawn.status.damage_invalid = true;
    f.pulled = true;
    a.sync_ctx(pawn);
    cx.set_voice_group_no(0);
}

/// 0x141355DD0's tail: pawn+0xE28; the previous kind 0xB0; the defaults; the air mode; Play("M_7422") with voice group 0.
/// (The state's kind stays 0xAF: the handler writes the pawn's previous kind only.)
fn sk_tall_hit(a: &mut Attack, cx: &mut Ctx) {
    let pawn = &mut *cx.pawn;
    pawn.combat.hit_flag = true;
    pawn.combat.last_kind = cl::KIND_SK_TALL;
    (a.stop_dist, a.z_gain) = (P.traction_stop_dist, P.traction_z_gain);
    (a.chan_xy, a.chan_root) = (true, false);
    (a.phase_xy, a.phase_root, a.phase_z) = (0, 0, 0);
    a.air_mode = true;
    a.root_len = 0.0;
    pawn.cmc.gravity_scale = P.air_gravity_scale;
    a.hover = true;
    pawn.cmc.gravity_off = true;
    pawn.cmc.set_flying();
    a.air_hold = P.air_hold_time;
    if let Some(id) = AnimId::from_name(cl::clip(pawn.combat.form.cmd, "M_7422")) {
        pawn.anim.play_with(id, PlayArgs::mode(2, -1.0));
    }
    a.sync_ctx(pawn);
    cx.set_voice_group_no(0);
}

fn tick(f: &mut FormCombo, cx: &mut Ctx, dt: f32) {
    // (F-POLISH) the pawn tick's head 0x141448AA0: a pending warp (pawn+0x155C) is made (FUN_143EAC3D0), then cleared.
    if let Some(at) = f.warp.take() {
        // (F-CLAWS) swept: never into or through the level (swept_warp)
        cx.pawn.cmc.location = swept_warp(cx, at);
        cx.pawn.cmc.stop_movement();
    }
    contacts(f, cx);
    f.a.tick(cx, dt);
}

/// End: (YO 0x141350900: with the pull, the body collision back on) Attack's End 0x141335120 (its pawn vt+0x2068(0)).
fn end(f: &mut FormCombo, cx: &mut Ctx) {
    if f.pulled {
        cx.pawn.actions.body_collision_off = false;
        cx.pawn.status.damage_invalid = false;
        f.pulled = false;
    }
    f.a.end(cx);
}

// ================================================================================================ the request

/// `CanAttack` (slot 44 0x141426330) in the package's forms: attack.rs's rows with the form's class and MaxCombo. None = a
/// state whose row is attack.rs's own business (the traversal states: their forced flow kinds).
fn can_attack(state: &StateData, s: &mut Pawn, max: i32) -> Option<bool> {
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
    let chain = |a: &Attack| s.queue.nothing_queued() && !a.next_queued && a.swing_started && a.enable_input && a.combo < max && s.combat.hit_flag && !s.combat.finish_end;
    if let Some(f) = state.form_combo() {
        return Some(chain(&f.a));
    }
    Some(match state {
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

/// `TryAttack(target, 0)` (slot 45 0x141437330) in the package's forms: the request queues the form's class. None =
/// attack.rs's own path (the traversal states; it queues the base class, whose forced flow kinds the class would refine).
pub(crate) fn try_attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    let cmd = cx.pawn.combat.form.cmd;
    let (max, _) = cl::max_combo_and_finishers(cmd)?;
    let s = &mut *cx.pawn;
    if !can_attack(state, s, max)? {
        return Some(false);
    }
    let forced = crate::combat::guard::forced_attack(state.id(), s);
    let (target, forced_kind, just_guard, push) = match forced {
        Some(o) => (if o.target != TargetId::NONE { o.target } else { target }, o.kind, o.just_guard, o.push),
        None => (target, 0, false, false),
    };
    s.combat.lockon.attack_target = target;
    let chained = match state {
        StateData::Attack(a) => Some(a),
        StateData::ComboLimit(c) => Some(&mut c.0),
        StateData::ComboCl(c) => Some(&mut c.0.a),
        StateData::ComboYo(c) => Some(&mut c.0.a),
        StateData::ComboSk(c) => Some(&mut c.0.a),
        _ => None,
    };
    let (next, push) = if let Some(cur) = chained {
        cur.next_queued = true;
        (Attack { combo: cur.combo + 1, air: cur.air, target, reach: cur.reach, ..Attack::RESET }, true)
    } else if let StateData::FireMagic(m) = state {
        let (combo, air) = m.attack_follows();
        (Attack { combo, air, target, ..Attack::RESET }, true)
    } else if let StateData::WeaponChange(_) = state {
        s.combat.pass_on = s.combat.carry;
        (Attack { combo: s.combat.combo + 1, air: true, target, ..Attack::RESET }, false)
    } else {
        let carried = s.magic.combo_keep > 0.0 && matches!(state, StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_) | StateData::Glide(_) | StateData::Floating(_));
        let combo = if carried { s.combat.combo + 1 } else { 0 };
        if carried {
            s.combat.pass_on = s.combat.carry;
        }
        (Attack { combo, air: !on_ground(s), target, forced_kind, just_guard, ..Attack::RESET }, push)
    };
    s.combat.combo = next.combo;
    let next = instance(cmd, next)?;
    if push {
        s.queue.push_state(next);
    } else {
        s.queue.set_state(next);
    }
    Some(true)
}

// ================================================================================================ FINISH_YO / FINISH_SK

/// The finish body's Begin (StyleFinish's Begin is the caller's). FINISH_SK: Reset 0x1413D0DC0 (+0x2E0 .. +0x2EC = 0,
/// +0x164 = 0), Begin 0x1413CC360 (StyleFinish's, then FUN_1405D4D90(pawn, 1, 0): not read).
pub(crate) fn finish_begin(f: &mut StyleFinish, _cx: &mut Ctx) {
    f.a.sub = 0;
    f.root_len = 0.0;
    f.count = 0;
    f.a.swing_started = false;
}

/// The per-command tick: true = done (chained on itself).
pub(crate) fn finish_tick(f: &mut StyleFinish, cx: &mut Ctx, dt: f32) -> bool {
    match f.cmd {
        cl::FINISH_YO => finish_yo(f, cx, dt),
        cl::FINISH_SK => finish_sk(f, cx, dt),
        _ => true,
    }
}

/// FINISH_YO, StyleFinish slot 149 (0x1413E0CD0): the giant yo-yo, M_4420. `root_len` = +0x194 (the 10 s timer), `catch` =
/// m_pCatchPawn (+0x280). (AUDIT-B F-CLAWS) The whole function as decompiled (re/decomp/1413E/1413E0CD0.c):
/// - sub 0: gravity off, Flying, Play("M_4420", -1, mode 2), +0x188 = -150 (the facing's aim offset z, read only by a pitching
///   facing, +0x1F4 bit 5, which FINISH_YO never sets), +0x260 = 1.0 (the warp delay's reload: no warp here), +0x194 = 0.
/// - sub 1: until a hit window opens (+0x164).
/// - sub 2: slot 127(1, 0.2) every tick (Attack 0x14134C6F0: facing on, its timer 0.2 s); with a caught pawn: while it is not
///   in state 62 the timer (10 s -> DoChain); in state 62: stopped, weapon off, collision off, gravity off, MOVE_None and
///   attached to the giant yo-yo's X_link01 (vt+0x1428(2) = the weapons, FUN_143E8FC60); without one: until M_4420 ends or a
///   window opens again (m_pCatchPawn = 0). Then WorldSlow(off) (0x14140E500(ctrl, 0)), slot 127(0, 0), +0x1F4 bit 5 off,
///   +0x1FC = his yaw, +0x218 = 0, Turn((0, yaw, 0), 1.0) (vt+0xDB0: the pitch levelled), +0x164 = 0, sub 5.
/// - sub 5: the caught pawn kept on X_link01 (vt+0x16E8; relative location 0, FUN_143EAC880); at M_4420's end Falling, gravity
///   on, the default gravity scale, DoChain. (StyleFinish End 0x1413CEBD0 gives the caught pawn back: style_finish.rs `end`.)
///
/// m_pCatchPawn has no native writer (asm of 0x1413C .. 0x1413F and 0x14133 .. 0x14135: the only stores to +0x280 are Reset
/// 0x1413D0CA0 and the clears of End 0x1413CEBD0 / slots 148 / 149; no blueprint names it): the reflected property the bind
/// reaction fills. Taken as F-HAMMER takes Drill Punch's DRILL_BIND: the first pawn the YO_Warhead window (SCYO_FINISH,
/// TRES_DMG_KIND_YO_BIND) hit, in its bind state (62) from that hit on.
fn finish_yo(f: &mut StyleFinish, cx: &mut Ctx, dt: f32) -> bool {
    match f.a.sub {
        0 => {
            // gravity off (pawn vt+0x1728(0)), Flying (vt+0x17A0(5, 1)); Play("M_4420", blend -1, mode 2) + voice group 0.
            cx.pawn.cmc.gravity_off = true;
            cx.pawn.cmc.set_flying();
            f.play(cx.pawn, "M_4420", -1.0, 2);
            cx.set_voice_group_no(0);
            f.root_len = 0.0;
            f.catch = TargetId::NONE;
            f.a.sub = 1;
        }
        1 => {
            if f.a.swing_started {
                f.a.swing_started = false;
                f.a.sub = 2;
            }
        }
        2 => {
            // (AUDIT-B F-CLAWS) slot 127(1, 0.2): facing on with its 0.2 s timer (not m_bFaceAt's snap)
            (f.a.facing_on, f.a.face_timer) = (true, 0.2);
            if f.catch == TargetId::NONE {
                if let Some(t) = cx.pawn.combat.windows.iter().find(|w| w.group == "YO_Warhead").and_then(|w| w.hit_list().first().copied()) {
                    f.catch = t;
                }
            }
            if f.catch != TargetId::NONE && cx.target(f.catch).is_some() {
                // in state 62: taken onto X_link01, then on to sub 5 in this tick (as KH3)
                yo_hold(cx, f.catch, dt);
            } else if cx.pawn.anim.ended || f.a.swing_started {
                f.catch = TargetId::NONE;
            } else {
                f.root_len += dt;
                if f.root_len > 10.0 {
                    cx.pawn.queue.do_chain();
                    return true;
                }
                return false;
            }
            form::world_slow(cx, false);
            (f.a.facing_on, f.a.face_timer, f.a.face_at, f.a.pitched, f.a.pitch_aim) = (false, 0.0, false, false, 0.0);
            let yaw = cx.pawn.yaw;
            f.a.face_yaw = yaw;
            super::common::turn_to(cx.pawn, 0.0, yaw, 1.0);
            f.a.swing_started = false;
            f.a.sub = 5;
        }
        _ => {
            // sub 5 / 0xC: the caught pawn kept on X_link01; at M_4420's end Falling (vt+0x17A0(3, 1)), gravity on, the
            // default gravity scale, DoChain.
            yo_hold(cx, f.catch, dt);
            if cx.pawn.anim.ended {
                return f.finish(cx.pawn);
            }
        }
    }
    false
}

/// (AUDIT-B F-CLAWS) The caught pawn held (the generic `Targets::script_hold`: every ER enemy) and CARRIED on the giant yo-yo's
/// X_link01: its actor location (UE: the capsule's centre, our target's `center`) on the bone at the playing clip's frame
/// (`claws::catch_link`), relative location 0 (FUN_143EAC880).
fn yo_hold(cx: &mut Ctx, target: TargetId, dt: f32) {
    if target == TargetId::NONE {
        return;
    }
    let face = cx.pawn.cmc.location;
    let sequence = cx.pawn.anim.name.data().samples.first().map(|&(_, i)| crate::anim::SEQUENCES[i].name);
    let link = sequence.and_then(|s| cl::catch_link(cx.pawn.cmc.location, cx.pawn.yaw, s, cx.pawn.anim.time));
    let carry = link.zip(cx.target(target)).map(|(at, t)| at - (t.center - t.position));
    cx.host.targets.script_hold(target, Some(crate::host::ScriptHold { face, turn_rate: 0.0, dt, hurt: true, carry }));
}

/// FINISH_SK, StyleFinishSK Tick (0x1413D6F30): the Chandelier. `root_len` = +0x2E8 (the clock: counted every tick, held at
/// 0 through sub-steps 0 and 1, so it is the chandelier's clip time from its M_7423); `count` = the crash hit given.
fn finish_sk(f: &mut StyleFinish, cx: &mut Ctx, dt: f32) -> bool {
    let before = f.root_len;
    f.root_len += dt;
    let row = cl::chandelier_row();
    let (length, max_time) = row.map_or((0.0, 10.0), |r| (r.length, r.values.first().copied().unwrap_or(10.0)));
    match f.a.sub {
        0 => {
            // The Chandelier spawned at his location with his yaw (FUN_1405D9130 with the accompany pawn class), hidden
            // (vt+0xD98(0)), standing on the floor below him (an accompany pawn lands: FUN_1413F5600 -> vt+0x2790 0x1413F5840,
            // the land spawn within m_Param_LandSpawnHeight 1200 cm).
            let (hit, _, floor) = ground_probe(cx.pawn, cx.world, 1200.0);
            let at = cx.pawn.cmc.location;
            let ground = Vec3::new(at.x, at.y, if hit { floor } else { at.z });
            // (AUDIT-B W-FIN) its centre = its OWN root shape's (StylePawn/w_so088 TresRoot: a SPHERE r 250 at z 250, the row's
            // values [5]; was Sora's capsule centre)
            let center = ground + Vec3::Z * row.and_then(|r| r.values.get(5).copied()).unwrap_or(cx.pawn.params().capsule_center_z);
            // (AUDIT-B F-CLAWS) FUN_143EAC3D0(Sora, the chandelier's root location, unswept): he is put where it stands (in
            // the air he is brought down onto the floor under him); then FUN_1405AD850 false (not falling) -> vt+0x17A0(1):
            // Walking. (AUDIT-B W-FIN) Its root IS the floor: the land spawn 0x1413F5840 adds ATresPawnBase vt+0x9F0 0x1405D4BA0 =
            // max(0, -(MyRoot vt+0xA58 0x1405CD120: the root shapes' lowest point above the root)) = 0 for its sphere (0 .. 500).
            if hit {
                let origin = crate::sora::Sora::origin_height_on_floor(cx.pawn.params());
                cx.pawn.cmc.location = ground + Vec3::Z * origin;
                cx.pawn.cmc.stop_movement();
                let p = *cx.pawn.params();
                cx.pawn.cmc.walk_on_floor(cx.world, &p);
            }
            cx.pawn.combat.xs.chandelier = Some(crate::abilities::xs::chandelier::Chandelier { scale: 1.0, crashed: false, ground, center, yaw: cx.pawn.yaw, fx_time: f32::NEG_INFINITY });
            f.play(cx.pawn, "M_7419", -1.0, 2);
            cx.set_voice_group_no(0);
            f.a.swing_started = false;
            f.a.sub = 1;
            f.root_len = 0.0;
        }
        1 => {
            f.root_len = 0.0;
            if cx.pawn.anim.ended {
                // weapon off (vt+0xFF0(0)), the body collision off (vt+0x1010(0, 0)), Flying; he and the chandelier play
                // M_7423; its done flag (+0xB48) cleared.
                crate::actions::draw(cx, false);
                cx.pawn.actions.body_collision_off = true;
                cx.pawn.cmc.set_flying();
                f.play(cx.pawn, "M_7423", -1.0, 2);
                f.a.sub = 2;
            }
        }
        2..=5 => {
            chandelier_tick(f, cx, before);
            if f.a.sub < 5 {
                // sub 2: shown; sub 3: until its done flag (its clip ran out), then WorldSlow(off).
                f.a.sub = 3;
                if f.root_len >= length {
                    form::world_slow(cx, false);
                    f.a.sub = 5;
                } else {
                    return false;
                }
            }
            // sub 5: while the clock is below the chandelier's m_Param_MaxTime, no TresEnableChange and his clip runs.
            if f.root_len < max_time && !cx.pawn.queue.flags.enable_change && !cx.pawn.anim.ended {
                return false;
            }
            f.a.sub = 6;
        }
        6 => {
            // 0x1413CFE10: the chandelier cleaned up; Play("M_1501").
            chandelier_clean(cx);
            f.play(cx.pawn, "M_1501", -1.0, 2);
            f.a.sub = 7;
        }
        7 => f.a.sub = 8,
        _ => {
            if cx.pawn.anim.ended {
                cx.pawn.queue.do_chain();
                return true;
            }
        }
    }
    false
}

/// The chandelier's own clip: its effects (abilities/xs/chandelier.rs `pawn_effects`, the clip's AttachEffect notifies)
/// and its hit window (SCSK_CHANDELIER_END on its group FinishSK2: one shot of the generic KH hit path at its spot, for
/// the window's length, of the shape's radius).
fn chandelier_tick(f: &mut StyleFinish, cx: &mut Ctx, before: f32) {
    let Some(c) = cx.pawn.combat.xs.chandelier else { return };
    // (AUDIT-B F-CLAWS) its clip's own effects AND sounds from the data (`wfx.SK.w_so088.7423.*`: sockets, offsets, scales,
    // states), followed on its bones; was XS's hand list `pawn_effects` (no sounds, no sockets)
    cl::chandelier_fx(cx, c.ground, c.yaw, before, f.root_len);
    if f.count == 0 {
        if let Some((start, end, row)) = cl::chandelier_window().filter(|(start, ..)| f.root_len >= *start) {
            f.count = 1;
            // (F-SHAPES) FinishSK2 is the pawn's CONVEX hull on its kh_sk (combat/hull.rs, `cl::chandelier_track`), placed at the
            // pawn's root and yaw and following its 7423 from this clip time; before, a sphere of m_Size.X (50) at the target's centre.
            if let Some(track) = cl::chandelier_track() {
                let info = cx.pawn.combat.attack_ctx;
                let key = CHANDELIER_SHOT_KEY;
                crate::combat::hit::spawn_form_shot(cx, crate::combat::hit::FormShot { key, group: "FinishSK2", row, position: c.ground, yaw: c.yaw, size: (0.0, 0.0, 0.0), life: (end - f.root_len).max(1.0 / 60.0), hit_effect: None, hit_se: None, info, hit_list: Vec::new() });
                if let Some(s) = cx.pawn.combat.shots.last_mut().filter(|s| s.window.key == (crate::combat::hit::FORM_SHOT_SEQUENCE, key)) {
                    s.hull = Some(crate::combat::hull::HullShot { track, t0: f.root_len, last: None });
                    // (AUDIT-B W-DATA) the hit effects of the component the window is on: StylePawn/w_so088 TresAtkColl0 has NO
                    // m_HitEffects entry (so the chooser 0x140628960 spawns none, as the Kraken's: F-SHIP), not the Kingdom
                    // Key's vw_so010_hit0; and no hit SE (its notify has only m_WeaponHitSEAliasID HIT4, which 0x140625500 resolves
                    // for an ATresWeaponBase owner only; the shape's m_HitSEAsset is null, m_HitSEID SEAL_NOTHING)
                    s.window.source = crate::status::keyblade::hit_source(&["w_so088"], false, None);
                }
            } else {
                let radius = cl::chandelier_row().and_then(|r| r.values.get(1).copied()).unwrap_or(0.0);
                crate::combat::hit::spawn_xs_shot(cx, "FinishSK2", row, c.center, radius, end - start, None);
            }
        }
    }
}

/// (F-SHAPES) The window key of the Chandelier's FinishSK2 shot (combat/hit.rs `FormShot::key`).
const CHANDELIER_SHOT_KEY: u16 = 0x93A0;

/// 0x1413CFE10 (and the form's end): the chandelier gone, his body collision back.
pub(crate) fn chandelier_clean(cx: &mut Ctx) {
    cx.pawn.combat.xs.chandelier = None;
    cx.pawn.actions.body_collision_off = false;
}

