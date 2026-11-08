//! (F-ARROWGUN) `TresPlayerState_ComboDW` (id 75): the attack state of Double Arrowguns (0x80) and Hunny Blasters (0x8E),
//! and the code it shares with `TresPlayerState_ComboBZ` (id 78, states/combo_bz.rs: Magic Launcher 0x83, Hunny Launcher
//! 0x8F). Pawn slot 1337 0x141432F00 gives the class by form. Both are `TresPlayerState_Attack` (attack.rs, which they wrap)
//! with (vtables read from the exe against Attack's 0x146900578 / anim interface 0x1469009F0):
//!
//! | | ComboDW | ComboBZ |
//! |---|---|---|
//! | vtable / anim interface | 0x14690DEB8 / 0x14690E330 | 0x14690BF30 / 0x14690C3A8 |
//! | slots 95 .. 98 | thunks of Attack's Reset / Begin / Tick / End | the same |
//! | slot 122 Decide | 0x141361270 (`arrowgun::decide_dw`) | 0x1413602A0 (`arrowgun::decide_bz`) |
//! | anim slot 69 (+0x228, TresShot) | 0x1413582D0 (`dw_shot`) | 0x141357FA0 (`bz_shot`) |
//!
//! PlayAttack (slot 120 0x1413378B0) cases 0x44 .. 0x48 / 0x4A .. 0x4E: the rows `attack.<DW|BZ>.<kind>` of
//! sheets/port/sora-form-arrowgun.json (tools/sora_form_arrowgun.js). MaxCombo (0x141433570) 5 / 3. The attack request in
//! the forms is `try_attack` (the package's `attack` hook): KH3's TryAttack 0x141437330 with the form's class; the rows of
//! `CanAttack` are attack.rs's.

use super::attack::Attack;
use super::common::{ground_probe, on_ground};
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, NotifyClass, NotifyData, NotifyPhase, Notifies, PlayArgs};
use crate::combat::forms::arrowgun::{self as ag, Family, GunDecideIn, Homing};
use crate::combat::tables::P;
use crate::enums::ETresAbilityKind;
use crate::host::{TargetFlags, TargetId};
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;
use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComboDw {
    /// The Attack state's fields.
    pub(crate) a: Attack,
}

/// Begin (Attack's 0x141333210 through the thunk 0x141333480) with the class's Decide, then PlayAttack.
pub(super) fn begin(a: &mut Attack, cx: &mut Ctx, f: Family) {
    // ---- Attack's Begin (attack.rs `begin`, the parts that apply to the class: its Decide is the class's)
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
    // ---- Decide (slot 122)
    let pawn = &mut *cx.pawn;
    let input = GunDecideIn { combo: a.combo, reach: a.reach, last_kind: pawn.combat.last_kind, dist: a.dist, dz: a.dz, num_enemies: a.num_enemies, wall_run: a.wall_run.is_some(), finish_count: pawn.combat.finish_count };
    let d = match f {
        Family::Dw => ag::decide_dw(&input),
        Family::Bz => ag::decide_bz(&input),
    };
    if d.last_reset {
        pawn.combat.last_kind = 0;
    }
    (a.kind, a.reach) = (d.kind, d.reach);
    a.air |= d.air;
    (a.finish, a.finish_end) = (d.finish, d.finish_end);
    pawn.combat.finish_count = d.finish_count;
    pawn.combat.finish_end |= d.finish_end;
    // "if forcedKind(+0x1E8) != 0: kind = forcedKind" (the guard / risk-dodge counters, the flow kinds).
    if a.forced_kind != 0 {
        a.kind = a.forced_kind;
    }
    play(a, cx);
    cx.set_voice_group_no(crate::host::audio::attack_voice_group_no(a.kind as i32));
    a.sub = 1;
}

/// PlayAttack (slot 120 0x1413378B0) of the gun kinds (attack.rs `play_attack` for the forced ones).
fn play(a: &mut Attack, cx: &mut Ctx) {
    let cmd = cx.pawn.combat.form.cmd;
    let kind = a.kind;
    if kind == ag::KIND_BARRIER_CRACKER && ag::is_hunny(cmd) {
        barrier_cracker(a, cx);
        return;
    }
    let Some(row) = ag::attack_row(cmd, kind) else {
        a.play_attack(cx);
        return;
    };
    let pawn = &mut *cx.pawn;
    pawn.combat.last_kind = kind;
    if pawn.combat.history.len() >= 16 {
        pawn.combat.history.remove(0);
    }
    pawn.combat.history.push(kind);
    let flag = |f: &str| ag::flag(row, f);
    let value = |i: usize| row.values.get(i).copied().unwrap_or(0.0);
    // Defaults (the Attack class's), then the case's writes.
    a.face_at = true;
    (a.chan_xy, a.chan_root) = (true, flag("chan_root"));
    a.stop_dist = P.traction_stop_dist;
    // +0x14C: the DW cases write 1.0; the BZ cases keep the class default (0x4A doubles it for |dz| >= 150).
    a.z_gain = if row.form == "DW" { value(0) } else { P.traction_z_gain };
    if ag::flag_value(row, "zgain2_dz").and_then(|v| v.parse::<f32>().ok()).is_some_and(|d| a.dz.abs() >= d) {
        a.z_gain *= 2.0;
    }
    a.air_mode = flag("air_mode");
    a.keep_face = flag("keep_face");
    a.root_len = value(1);
    if flag("no_xy") {
        a.chan_xy = false;
    }
    // The root-scale divisor (fVar28: XY scale = clamp(dist / divisor, 0.1, 2.5) at the play): 0x4A only inside 450 cm
    // (where it also drops the XY channel), 0x4B / 0x4C always (0x4C drops the XY channel inside 560).
    let mut divisor = value(3);
    if let Some(near) = ag::flag_value(row, "no_xy_near").and_then(|v| v.parse::<f32>().ok()) {
        let inside = a.dist < near;
        if inside {
            a.chan_xy = false;
        } else if kind == 0x4A {
            divisor = 0.0;
        }
    }
    if flag("keep_dir_z") {
        a.keep_dir_z = true;
    }
    let mut clip = row.clip;
    // 0x4D / 0x4E: bAir (+0x142) plays the air clip and sets the air mode (+0x1DF).
    if let (true, Some(air)) = (a.air, ag::flag_value(row, "air_clip")) {
        clip = ag::clip(cmd, air);
        a.air_mode = true;
    }
    if a.finish_end {
        pawn.combat.finish_end = true;
    }
    // The tail (LAB_14133A87A): ground: gravity on, DirZ 0; air mode: GravityScale 0.1, Flying, the hover, +0x18C = 0.6.
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
    // 0x47: +0x1D4 = 250 with +0x1DD (DirZ fixed).
    if flag("fixed_dir_z") {
        (a.fixed_dir_z, a.dir_z) = (true, value(2));
    }
    // (AUDIT-A ARROWGUN) +0x16C (0x44 .. 0x46): the stick steers the attack (slot 131 0x141341870, attack.rs `steering`);
    // the play's tail (after LAB_14133A87A): MaxWalkSpeed = vt+0x1588() x 0.9, MaxAcceleration = 3672 (0x45658001), +0x17C = his
    // yaw. +0x16D / +0x188 (0x48): the traction aims 500 cm above the target, both reset when its window closes.
    a.steer = flag("f16c");
    if a.steer {
        a.steer_accel = pawn.cmc.max_acceleration;
        pawn.cmc.max_walk_speed = pawn.params().speed_run * 0.9;
        pawn.cmc.max_acceleration = f32::from_bits(0x4565_8001);
        (a.steer_yaw, a.steer_scale) = (pawn.yaw, 0.0);
    }
    a.reset_dz = flag("f16d");
    a.target_dz = value(4);
    pawn.combat.swing_hit = false;
    let scale = if divisor > 0.0 && a.dist != 0.0 { (a.dist / divisor).clamp(0.1, 2.5) } else { 1.0 };
    pawn.root_scale = (scale, 1.0);
    // Play(name, 0, 0, mode 2, +0x198 (the Hunny forms' effect group 0x71E / 0x721 ..), 1.0, scale).
    if ag::is_hunny(cmd) && row.group != 0 {
        pawn.next_effect_group = Some(row.group);
    }
    match AnimId::from_name(clip) {
        Some(anim) => pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
        None => pawn.note("a gun-form attack clip is not in the generated animation table (tools/gen_port.js FORM_ANIM_SETS)"),
    }
    a.face_yaw = pawn.yaw;
    a.sync_ctx(pawn);
}

/// PlayAttack case 0x25 (0x1413378B0) outside 0xDF / 0xE0: `BarrierCracker` (bAir +0x142: `AirBarrierCracker` with the air
/// mode +0x1DF), +0x198 = 0xD48 (the effect group), +0x19C = 0x34, +0x164 = 1 (the swing has started), +0x234 |= 0x2800 (no
/// reader in the class), then the tail. The Hunny sets play it as AnimSet16 / 17's rows (2341 / 2351, 5341 / 5351); its
/// TresShot spawns BARRIERCRACKER (both shot handlers' case 0x25).
fn barrier_cracker(a: &mut Attack, cx: &mut Ctx) {
    let cmd = cx.pawn.combat.form.cmd;
    let pawn = &mut *cx.pawn;
    pawn.combat.last_kind = a.kind;
    if pawn.combat.history.len() >= 16 {
        pawn.combat.history.remove(0);
    }
    pawn.combat.history.push(a.kind);
    a.face_at = true;
    (a.chan_xy, a.chan_root) = (true, false);
    (a.stop_dist, a.z_gain) = (P.traction_stop_dist, P.traction_z_gain);
    a.air_mode = a.air;
    a.swing_started = true;
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
    // (AUDIT-A ARROWGUN) +0x234 |= 0x2800: bit 13 is "the attack landed a hit" (`swing_hit`: End 0x141335120 in the air
    // then zeroes +0x18C instead of the air-slide / air-guard cooldowns, and the counters pass on); bit 11 makes End call
    // 0x1405B6100(pawn+0x460): component +0x130's slot 0x720 with the flag (+0x70)+0x98, a setting this attack never
    // changed (nothing in the core to put back).
    // (AUDIT-B W-CORE) Corrected: bit 13 is no hit flag (its only reader is End's air branch; the counters pass on by
    // +0x13C & 5, which case 0x25 does not write), and 0x1405B8B30(pawn+0x460, 8) DID change a setting: MyBodyColl +0x4A4,
    // his push power level (PPL_90), which End's 0x1405B6100 puts back (attack.rs `counter_body`).
    a.counter_body(pawn);
    pawn.combat.swing_hit = false;
    pawn.root_scale = (1.0, 1.0);
    pawn.next_effect_group = Some(0xD48);
    let clip = ag::clip(cmd, if a.air { "AirBarrierCracker" } else { "BarrierCracker" });
    match AnimId::from_name(clip) {
        Some(anim) => pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
        None => pawn.note("Barrier Cracker's clip is not in the generated animation table"),
    }
    a.face_yaw = pawn.yaw;
    a.sync_ctx(pawn);
}

/// A homing target while it is targetable (component vt+0xA80).
fn targetable(cx: &Ctx, t: TargetId) -> Option<TargetId> {
    cx.target(t).filter(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE)).map(|t| t.id)
}

/// The homing target of a ComboBZ shot: the state's homing target component (+0x118) while it is targetable.
fn shot_target(cx: &Ctx, a: &Attack) -> Option<TargetId> {
    targetable(cx, a.target)
}

fn yaw_dir(yaw_deg: f32, pitch_deg: f32) -> Vec3 {
    let (y, p) = (yaw_deg.to_radians(), pitch_deg.to_radians());
    Vec3::new(p.cos() * y.cos(), p.cos() * y.sin(), p.sin())
}

/// ComboDW's shot handler (anim slot 69 0x1413582D0), one TresShot notify: the kind's projectile from one or both
/// guns (the notify's LocationIndex, 0 when > 1, then the next), a fan of +-45 degrees (0x44 / 0x45), outward from Sora's
/// centre through the muzzle (0x46 / 0x48), or a ring of 8 pitched 45 degrees up (0x47); homing on the target, else on
/// the point 1000 cm ahead of Sora's centre (vt+0x1210).
fn dw_shot(a: &mut Attack, cx: &mut Ctx, location: i32) {
    let row = ag::attack_row(cx.pawn.combat.form.cmd, a.kind);
    let name = match row.and_then(|r| ag::flag_value(r, "shot")) {
        Some(name) => name,
        None if a.kind == ag::KIND_BARRIER_CRACKER => "BARRIERCRACKER",
        None => return,
    };
    let get = |key: &str| row.and_then(|r| ag::flag_value(r, key));
    let hands: i32 = get("hands").and_then(|v| v.parse().ok()).unwrap_or(1);
    let ring: i32 = get("ring").and_then(|v| v.parse().ok()).unwrap_or(1);
    let ring_pitch: f32 = get("ring_pitch").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let fan: Option<f32> = get("fan").and_then(|v| v.parse().ok());
    let outward = row.is_some_and(|r| ag::flag(r, "outward"));
    let first = if location > 1 { 0 } else { location };
    let centre = ag::centre(cx);
    let yaw = cx.pawn.yaw;
    a.sync_ctx(cx.pawn);
    let info = cx.pawn.combat.attack_ctx;
    for h in 0..hands {
        let index = first + h;
        let at = ag::muzzle(cx, index);
        let mut dir = yaw_dir(yaw, 0.0);
        if let Some(fan) = fan {
            dir = yaw_dir(yaw + if index == 0 { fan } else { -fan }, 0.0);
        } else if outward {
            dir = (at - centre).normalize_or_zero();
        }
        // Per hand: the controller's switch list entry `shots % count` (or the shooter pick), else the state's target.
        let target = targetable(cx, cx.pawn.combat.arrowgun.aim.gun_target(a.shots, a.target));
        let homing = match target {
            Some(t) => Homing::Target(t),
            None => {
                let (sin, cos) = yaw.to_radians().sin_cos();
                Homing::Point(centre + Vec3::new(cos, sin, 0.0) * 1000.0)
            }
        };
        for i in 0..ring {
            if ring > 1 {
                let flat = (at - centre).truncate();
                let base = if flat.length_squared() > 1.0e-6 { flat.y.atan2(flat.x).to_degrees() } else { yaw };
                dir = yaw_dir(base + 360.0 / ring as f32 * i as f32, ring_pitch);
            }
            if ag::fire(cx, name, at, dir, homing, info) {
                a.shots += 1;
            }
        }
    }
}

/// ComboBZ's shot handler (anim slot 69 0x141357FA0): 0x4D BZ_BOMBSHOT1 / 0x4E BZ_BOMBSHOT2 at the notify's muzzle, along
/// his forward, homing on the state's target (vt+0x1220) when it has one; 0x37 is the Attack class's (Sonic Cleave).
fn bz_shot(a: &mut Attack, cx: &mut Ctx, location: i32) {
    let name = match ag::attack_row(cx.pawn.combat.form.cmd, a.kind).and_then(|r| ag::flag_value(r, "shot")) {
        Some(name) => name,
        None if a.kind == ag::KIND_BARRIER_CRACKER => "BARRIERCRACKER",
        None => return,
    };
    let at = ag::muzzle(cx, location);
    let dir = yaw_dir(cx.pawn.yaw, 0.0);
    let homing = shot_target(cx, a).map_or(Homing::None, Homing::Target);
    a.sync_ctx(cx.pawn);
    let info = cx.pawn.combat.attack_ctx;
    if ag::fire(cx, name, at, dir, homing, info) {
        a.shots += 1;
    }
}

/// The notifies: the Attack class's, then the class's TresShot handler.
pub(super) fn on_notifies(a: &mut Attack, cx: &mut Ctx, notifies: &Notifies, f: Family) {
    a.on_notifies(cx, notifies);
    for event in &notifies.events {
        if (event.class(), event.phase) != (NotifyClass::TresAnimNotifyShot, NotifyPhase::Instant) {
            continue;
        }
        let location = match event.def().data {
            NotifyData::Shot { location_index, .. } => location_index.unwrap_or(0),
            _ => 0,
        };
        // The head of both handlers (0x1413582D0 / 0x141357FA0): state+0x1F4 &= ~0x10 (the facing hold ends), +0x234 &= ~8,
        // +0x164 = 1: the swing has started (a gun clip has no attack window: this is what lets the combo chain).
        a.keep_face = false;
        a.no_vz = false;
        a.swing_started = true;
        match f {
            Family::Dw => dw_shot(a, cx, location),
            Family::Bz => bz_shot(a, cx, location),
        }
    }
}

/// End: Attack's End (its chain test knows both gun classes: attack.rs).
pub(super) fn end(a: &mut Attack, cx: &mut Ctx) {
    a.end(cx);
}

impl PlayerState for ComboDw {
    const ID: StateId = StateId::ComboDw;

    fn sub_step(&self) -> u8 {
        self.a.sub
    }

    fn begin(&mut self, cx: &mut Ctx) {
        begin(&mut self.a, cx, Family::Dw);
    }

    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        self.a.tick(cx, dt);
    }

    fn end(&mut self, cx: &mut Ctx) {
        end(&mut self.a, cx);
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        on_notifies(&mut self.a, cx, notifies, Family::Dw);
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

fn queue(s: &mut Pawn, f: Family, a: Attack, push: bool) {
    let state: StateData = match f {
        Family::Dw => ComboDw { a }.into(),
        Family::Bz => super::combo_bz::ComboBz { a }.into(),
    };
    if push {
        s.queue.push_state(state);
    } else {
        s.queue.set_state(state);
    }
}

/// `CanAttack` (slot 44 0x141426330) in the gun forms: attack.rs's rows with the form's class and MaxCombo. None = a state
/// whose row is attack.rs's own business (the traversal states: their forced flow kinds).
fn can_attack(state: &StateData, s: &mut Pawn, f: Family) -> Option<bool> {
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
    let max = ag::max_combo(f);
    if let Some(a) = state.attack() {
        return Some(s.queue.nothing_queued() && !a.next_queued && a.swing_started && a.enable_input && a.combo < max && s.combat.hit_flag && !s.combat.finish_end);
    }
    Some(match state {
        StateData::FireMagic(m) => m.can_attack(s, max) && s.combat.hit_flag && !s.combat.finish_end,
        StateData::UseItem(_) => s.queue.flags.enable_change,
        StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_) | StateData::Glide(_) | StateData::Floating(_) | StateData::Swaying(_) => true,
        StateData::DodgeRoll(_) | StateData::AirSlide(_) => s.queue.flags.enable_change,
        StateData::Damage(d) => s.queue.nothing_queued() && !d.bad && matches!(d.kind.value(), 1 | 2 | 7 | 8 | 9 | 10) && s.queue.flags.enable_change,
        StateData::BadstatStun(_) => s.queue.nothing_queued() && s.queue.flags.enable_change,
        StateData::WeaponChange(w) => w.changed && s.combat.hit_flag,
        // (F-ARROWGUN) 40 SHOOTERMODE: CanAttack 0x141426330 lets it attack.
        StateData::ShooterMode(_) => true,
        StateData::WallRun(_) | StateData::Hopping(_) | StateData::SlopeSlide(_) | StateData::SuperSlide(_) | StateData::WallKick(_) | StateData::PoleTurn(_) | StateData::RailSlide(_) => return None,
        _ => false,
    })
}

/// `TryAttack(target, 0)` (slot 45 0x141437330) in the gun forms: the request queues COMBO_DW / COMBO_BZ (the form
/// package's `attack` hook). None = attack.rs's own path (the traversal states).
pub(crate) fn try_attack(state: &mut StateData, cx: &mut Ctx, target: TargetId, f: Family) -> Option<bool> {
    let s = &mut *cx.pawn;
    if !can_attack(state, s, f)? {
        return Some(false);
    }
    let forced = crate::combat::guard::forced_attack(state.id(), s);
    let (target, forced_kind, just_guard, push) = match forced {
        Some(o) => (if o.target != TargetId::NONE { o.target } else { target }, o.kind, o.just_guard, o.push),
        None => (target, 0, false, false),
    };
    s.combat.lockon.attack_target = target;
    if let Some(cur) = state.attack_mut() {
        // The combo step: queued behind the running attack.
        cur.next_queued = true;
        let next = Attack { combo: cur.combo + 1, air: cur.air, target, reach: cur.reach, ..Attack::RESET };
        s.combat.combo = next.combo;
        queue(s, f, next, true);
    } else if let StateData::FireMagic(m) = state {
        let (combo, air) = m.attack_follows();
        s.combat.combo = combo;
        queue(s, f, Attack { combo, air, target, ..Attack::RESET }, true);
    } else if let StateData::WeaponChange(_) = state {
        let next = Attack { combo: s.combat.combo + 1, air: true, target, ..Attack::RESET };
        s.combat.pass_on = s.combat.carry;
        s.combat.combo = next.combo;
        queue(s, f, next, false);
    } else {
        // (F-ARROWGUN) TryAttack 0x14143E670 case 40: out of ShooterMode he turns to its aim yaw (+0xCC) first.
        if let StateData::ShooterMode(m) = state {
            s.yaw = crate::sora::wrap_degrees(m.yaw);
        }
        // IDLE, RUN, JUMP, GUARD ...: SetState (RISKDODGE / AIR_SLIDE: PushState); the carry timer of a Keyblade switch.
        let carried = s.magic.combo_keep > 0.0 && matches!(state, StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_) | StateData::Glide(_) | StateData::Floating(_));
        let combo = if carried { s.combat.combo + 1 } else { 0 };
        if carried {
            s.combat.pass_on = s.combat.carry;
        }
        s.combat.combo = combo;
        queue(s, f, Attack { combo, air: !on_ground(s), target, forced_kind, just_guard, ..Attack::RESET }, push);
    }
    Some(true)
}
