//! (weapons F-SHIP) The attack states of Wheel of Fate's forms (pawn slot 1337 0x141432F00): COMBO_HL (id 87: Highwind
//! 0x8C) and COMBO_FL (88: Storm Flag 0x8D). Both are subclasses of `TresPlayerState_Attack` (attack.rs, which they wrap)
//! and KH3 shares everything but these between them (vtables ComboHL 0x14690F388, its anim interface 0x14690F808;
//! ComboFL 0x14690E778, the Attack class's interfaces 0x1469009F0 / 0x146900E38; audit/vfuncs.json and base/KH3.exe):
//!
//! | class | slot | address | what |
//! |---|---|---|---|
//! | HL | 95 Reset | 0x141352E40 | Attack's Reset, +0x280 = 0 (the rain clock), +0x284 = 0 (the rain on), +0x288 = None (its projectile) |
//! | FL | 95 Reset | 0x141352E20 | Attack's Reset, +0x250 = 2000.0 (no reader in the class: shared with ComboGM) |
//! | HL | 97 Tick | 0x141353A50 | Attack's Tick, then slot 143 |
//! | both | 122 Decide | HL 0x141361CC0, FL 0x141361530 | `combat::forms::ship::decide_hl` / `decide_fl` |
//! | FL | 125 | 0x141366630 | the flag throw: M_F412 -> M_F413 -> M_F414 (`throw_chain`) |
//! | HL | 136 | 0x1413524A0 | the warp of kind 0xA0: above the target (`warp_hl`); other kinds the Attack class's 0x141337320 |
//! | HL | 143 (new) | 0x141364D20 | the spear rain (`rain`) |
//! | HL | iface 69 (the shot handler) | 0x141358E20 | outside kind 0x37: a Shot notify starts the rain with its m_ProjectileName |
//!
//! PlayAttack (slot 120 0x1413378B0) is the Attack class's, cases 0x9C .. 0xA9: here from the rows `attack.<form>.<kind>` of
//! `sheets/port/sora-form-ship.json`. The attack request in the forms is [`try_attack`] (the form package's `attack` hook):
//! KH3's TryAttack 0x141437330 with the class the form asks for; the rows of `CanAttack` are attack.rs's with MaxCombo 5.
//!
//! Also here (the Attack state's fields are this module's): the body of FINISH_FL (`TresPlayerState_StyleFinishFL`, 39:
//! Begin 0x1413CC350 = StyleFinish's, Tick 0x1413D5060, End 0x1413CF130 with the cleanup 0x1413CFAF0; the framework
//! queues STYLEFINISH 33 for every finish command and runs the body through the package's finish hooks).

use glam::Vec3;

use super::attack::Attack;
use super::common::ground_probe;
use super::style_finish::StyleFinish;
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, NotifyClass, NotifyData, NotifyPhase, Notifies, PlayArgs};
use crate::combat::forms::ship::{self as sh, ShipDecideIn};
use crate::combat::{form, tables::P};
use crate::enums::ETresAbilityKind as Ab;
use crate::host::TargetId;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};

/// (AUDIT-B F-SHIP) +0x22C of the Attack state (its Reset 0x14133D230 writes 0xB; ComboHL writes none): the warp's aura group.
const WARP_AURA: i32 = 0xB;

/// The fields the two classes add to the Attack state's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShipCombo {
    /// The Attack state's fields.
    pub(crate) a: Attack,
    /// HL +0x284 / +0x280: the spear rain is on / its clock (`rain.HL` values[0] between waves).
    rain_on: bool,
    rain_timer: f32,
    /// +0x1E1 / +0x258: the warp is still to be made (PlayAttack's head) / its countdown (slot 117).
    warp: bool,
    warp_delay: f32,
    /// The warp's head made him invincible (pawn vt+0x2068(1)); End takes it off.
    invincible: bool,
}

impl ShipCombo {
    pub(crate) fn new(a: Attack) -> ShipCombo {
        ShipCombo { a, rain_on: false, rain_timer: 0.0, warp: false, warp_delay: 0.0, invincible: false }
    }
}

macro_rules! classes {
    ($($ty:ident => $id:ident,)+) => {
        $(
            #[derive(Clone, Copy, Debug, PartialEq)]
            pub(crate) struct $ty(pub(crate) ShipCombo);

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
                    on_notifies(&mut self.0, cx, notifies);
                }

                fn movement_state(&self, _pawn: &Pawn) -> MovementState {
                    MovementState::Other
                }
            }
        )+
    };
}

classes! {
    ComboHl => ComboHl,
    ComboFl => ComboFl,
}

/// The class instance the held form asks for (pawn slot 1337 0x141432F00).
fn instance(cmd: i32, a: Attack) -> Option<StateData> {
    let f = ShipCombo::new(a);
    match cmd {
        sh::CHANGE_HL => Some(ComboHl(f).into()),
        sh::CHANGE_FL => Some(ComboFl(f).into()),
        _ => None,
    }
}

impl StateData {
    /// (F-SHIP) The fields of a running COMBO_HL / COMBO_FL.
    pub(crate) fn ship_combo(&self) -> Option<&ShipCombo> {
        match self {
            StateData::ComboHl(c) => Some(&c.0),
            StateData::ComboFl(c) => Some(&c.0),
            _ => None,
        }
    }
}

/// Slot 124 (0x1413325F0, decompiled C) with its argument `keep`: a forced flow kind 0x36 / 0x37 by the target and the
/// abilities (Buzz Saw / Sonic Slash / Sonic Cleave), then a flow kind other than `keep` whose ability is missing falls
/// back to the air kind of the reach. ComboHL's Decide calls it with 0x37 (Sonic Cleave stays without SONIC_DOWN), the head
/// of PlayAttack with 0 (state+0x234 bit 20 makes a second call a no-op).
fn flow_refine(a: &mut Attack, pawn: &Pawn, keep: u8) {
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
    if a.forced_kind == keep {
        return;
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
fn begin(f: &mut ShipCombo, cx: &mut Ctx) {
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
    let cmd = cx.pawn.combat.form.cmd;
    // ComboHL's Decide first: slot 124(0x37); a Sonic Cleave (forced 0x37) is Highwind's dive 0xA2, in the air.
    if cmd == sh::CHANGE_HL && a.forced_kind != 0 {
        flow_refine(a, cx.pawn, 0x37);
        if a.forced_kind == 0x37 {
            (a.air, a.forced_kind) = (true, 0xA2);
        }
    }
    let pawn = &mut *cx.pawn;
    let input = ShipDecideIn {
        combo: a.combo,
        reach: a.reach,
        last_kind: pawn.combat.last_kind,
        dist: a.dist,
        dz: a.dz,
        target_high: a.target_high,
        crowd: a.num_enemies,
        finish_count: pawn.combat.finish_count,
        finish_now: false,
    };
    let d = if cmd == sh::CHANGE_HL { sh::decide_hl(&input) } else { sh::decide_fl(&input) };
    if d.last_reset {
        pawn.combat.last_kind = 0;
    }
    (a.kind, a.reach, a.finish, a.finish_end) = (d.kind, d.reach, d.finish, d.finish_end);
    if d.rise {
        // +0x1DD = 1; +0x1D4 = pawn+0xBAC x 200.
        (a.fixed_dir_z, a.dir_z) = (true, P.decide_rise_speed);
    }
    pawn.combat.finish_count = d.finish_count;
    if d.finish_end {
        // pawn vt+0x2740(1): the pawn's finish-end bit.
        pawn.combat.finish_end = true;
    }
    if d.homing && a.target == TargetId::NONE {
        a.target = pawn.combat.lockon.current;
    }
    f.warp = d.warp;
    // "if forcedKind(+0x1E8) != 0: kind = forcedKind" (the head of PlayAttack: slot 124(0), then the air kinds' ability
    // variants).
    if a.forced_kind != 0 {
        if cmd == sh::CHANGE_FL {
            flow_refine(a, pawn, 0);
        }
        let has = |k| pawn.status.abilities.has(k);
        a.forced_kind = match a.forced_kind {
            0x0F if has(Ab::AerialDive) => 0x20,
            0x10 if has(Ab::QuickShaft) => 0x18,
            0x11 if has(Ab::FlashStep) => 0x19,
            0x12 if has(Ab::RadialCut) => 0x1A,
            k => k,
        };
        a.kind = a.forced_kind;
        f.warp = false;
    }
    // Final Blow (Attack Begin: +0x234 & 0x80000 handed over by the chain): the chase warp, as attack.rs.
    // (AUDIT-B W-CORE) attack.rs `chase_begin`: +0x22C = 0xD over Decide's warp, PlayAttack's head (JumpNormalStart, groups 13 / 14).
    if a.chase_begin(cx) {
        f.warp = false;
        return;
    }
    play(f, cx);
    cx.set_voice_group_no(crate::host::audio::attack_voice_group_no(i32::from(f.a.kind)));
    f.a.sub = 1;
}

/// PlayAttack (slot 120 0x1413378B0) of the package's kinds (attack.rs `play_attack` for every other kind).
fn play(f: &mut ShipCombo, cx: &mut Ctx) {
    let cmd = cx.pawn.combat.form.cmd;
    let a = &mut f.a;
    let kind = a.kind;
    let Some(row) = sh::attack_row(cmd, kind) else {
        a.play_attack(cx);
        return;
    };
    let pawn = &mut *cx.pawn;
    if f.warp {
        // The head of PlayAttack with +0x1E1 (decompiled C): +0x21C = 1, +0x234 = (bits & ~2) | 4, +0x224 = 0, air mode,
        // gravity off (vt+0x1728(0)), Flying (vt+0x17A0(5, 1)), Play("JumpNormalStart", 0, blend -1, mode 2), the countdown
        // +0x258 (`warp.HL.0xA0` values[0]), m_bFaceAt, invincible (vt+0x2068(1)); nothing else until the countdown ends.
        a.chan_root = false;
        a.air_mode = true;
        pawn.cmc.gravity_off = true;
        pawn.cmc.set_flying();
        pawn.cmc.stop_movement();
        match AnimId::from_name("JumpNormalStart") {
            Some(anim) => pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
            None => pawn.note("JumpNormalStart is not in the generated animation table"),
        }
        f.warp_delay = sh::warp()[0];
        a.face_at = true;
        pawn.status.damage_invalid = true;
        f.invincible = true;
        a.face_yaw = pawn.yaw;
        a.sync_ctx(pawn);
        // (AUDIT-B F-SHIP) the anim interface's vt+0x1A0(pawn vt+0x2458() = m_CmnAuraEffect, +0x22C, 0): the vanish, group 11
        // (+0x22C from the Attack Reset 0x14133D230) of ead_pc_aura_com0
        sh::warp_aura(cx, WARP_AURA);
        return;
    }
    pawn.combat.last_kind = kind;
    if pawn.combat.history.len() >= 16 {
        pawn.combat.history.remove(0);
    }
    pawn.combat.history.push(kind);
    // The defaults before the switch (m_bFaceAt, the XY channel on, the root scale off, stop 160, z gain 3, no air mode),
    // then the case's writes (the row).
    a.face_at = true;
    (a.chan_xy, a.chan_root) = (row.chan_xy, row.chan_root);
    (a.stop_dist, a.z_gain, a.air_mode, a.keep_face) = (row.stop_dist, row.z_gain, row.air_mode, false);
    a.root_len = row.root_len;
    if sh::has_flag(row, "root_keep") {
        a.root_keep = true;
    }
    if sh::has_flag(row, "dive") {
        a.dive = true;
    }
    if sh::has_flag(row, "no_vz") {
        a.no_vz = true;
    }
    if sh::has_flag(row, "keep_dir_z") {
        a.keep_dir_z = true;
    }
    if row.dir_z != 0.0 {
        a.dir_z = row.dir_z;
    }
    // The tail (LAB_14133A87A): ground: gravity on, DirZ 0; air mode: +0x234 |= 4 | 0x10, GravityScale 0.1, gravity off,
    // Flying, +0x18C = 0.6.
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
        None => pawn.note("a Highwind / Storm Flag attack clip is not in the generated animation table (tools/gen_port.js FORM_ANIM_SETS)"),
    }
    a.face_yaw = pawn.yaw;
    a.sync_ctx(pawn);
}

/// ComboHL slot 136 (0x1413524A0) for kind 0xA0, called by slot 117 (0x14134AA80) when the countdown ends: from his point
/// (slot 134 0x141336940: his root) toward the target's (its lock-on point raised by its collision half height, + 500),
/// swept with radius 60 (pawn vt+0x2750), then 100 down (the slot-117 call's second number 0x42C80000); he is put there
/// (0x143EAC3D0) and PlayAttack plays the kind. Without a target he stays.
fn warp_hl(f: &mut ShipCombo, cx: &mut Ctx) {
    let [_, height, drop, radius] = sh::warp();
    if let Some(t) = f.a.target_info(cx) {
        let from = cx.pawn.cmc.location;
        let to = Vec3::new(t.center.x, t.center.y, t.center.z + t.half_height + height);
        let swept = |a: Vec3, b: Vec3| match cx.world.sweep_capsule(a, b, radius, radius) {
            Some(hit) if !hit.start_penetrating => a + (b - a) * hit.time.clamp(0.0, 1.0),
            Some(_) => a,
            None => b,
        };
        let over = swept(from, to);
        let end = swept(over, over - Vec3::Z * drop);
        cx.pawn.cmc.location = end;
        cx.pawn.cmc.stop_movement();
        let d = t.center - end;
        if d.truncate().length_squared() > 1.0e-6 {
            cx.pawn.yaw = wrap_degrees(d.y.atan2(d.x).to_degrees());
        }
    }
    f.warp = false;
}

/// ComboHL slot 143 (0x141364D20, decompiled C), every tick after the Attack's: while +0x284 is on, every `rain.HL`
/// values[0] s a wave of values[2] HL_SPEAR shots at the angles yaw0 + k x values[3] (yaw0 = the engine's xorshift
/// `(r >> 8) x 8.940697e-6 - 180`, world degrees, pitch 0), each at the shot origin (pawn vt+0x1410: the weapon's
/// ATK_01) + direction x (values[4] + values[5] x the spears so far, taken at the wave's start) + (0, 0, values[6]),
/// pointing down; off once more than values[1] were spawned.
fn rain(f: &mut ShipCombo, cx: &mut Ctx, dt: f32) {
    if !f.rain_on {
        return;
    }
    let [interval, last, per_wave, step, base, per_spear, height] = sh::rain();
    if f.rain_timer > 0.0 {
        f.rain_timer -= dt;
        if f.rain_timer > 0.0 {
            return;
        }
        f.rain_timer = 0.0;
    }
    f.a.sync_ctx(cx.pawn);
    let origin = crate::combat::hit::shot_origin(cx);
    let radius = f.a.shots as f32 * per_spear + base;
    let rng = &mut cx.pawn.combat.rng;
    if *rng == 0 {
        *rng = 0x9E37_79B9;
    }
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    let mut yaw = (*rng >> 8) as f32 * 8.940_697e-6 - 180.0;
    for _ in 0..per_wave as usize {
        let (sin, cos) = yaw.to_radians().sin_cos();
        let at = origin + Vec3::new(cos, sin, 0.0) * radius + Vec3::Z * height;
        sh::spawn_spear(cx, f.a.shots, at);
        f.a.shots += 1;
        yaw += step;
    }
    f.rain_timer = interval;
    if f.a.shots as f32 > last {
        f.rain_on = false;
    }
}

/// ComboFL slot 125 (0x141366630, decompiled C), every tick before the Attack's: the flag throw. In M_F412 a landed hit
/// (pawn+0xE24 > 0) enables input; at its end M_F413 (blend 0, mode 2); in M_F413 the same, and at its end or with an
/// attack queued (+0x143) the flag comes back (pawn vt+0x13B8(7)) and M_F414 plays (blend -1); then a queued attack
/// sets m_bEnableChange.
fn throw_chain(f: &mut ShipCombo, cx: &mut Ctx) {
    let cmd = cx.pawn.combat.form.cmd;
    let playing = cx.pawn.anim.name.name();
    let (f412, f413) = (sh::clip(cmd, "M_F412"), sh::clip(cmd, "M_F413"));
    let hit = cx.pawn.combat.hit_count > 0;
    if playing == f412 {
        if hit {
            f.a.enable_input = true;
        }
        if !cx.pawn.anim.ended {
            return;
        }
        if let Some(id) = AnimId::from_name(f413) {
            cx.pawn.anim.play_with(id, PlayArgs::mode(2, 0.0));
            cx.set_voice_group_no(0);
        }
    } else if playing == f413 {
        if hit {
            f.a.enable_input = true;
        }
        if !cx.pawn.anim.ended && !f.a.next_queued {
            return;
        }
        // pawn vt+0x13B8(7) (equipment vt+0x560(7): the thrown flag back to his hand): the weapon's own clip, the host's.
        if let Some(id) = AnimId::from_name(sh::clip(cmd, "M_F414")) {
            cx.pawn.anim.play_with(id, PlayArgs::mode(2, -1.0));
            cx.set_voice_group_no(0);
        }
    } else {
        return;
    }
    if f.a.next_queued {
        cx.pawn.queue.flags.enable_change = true;
    }
}

/// Tick: ComboFL slot 125 first; slot 117's countdown of the warp (nothing else of the Tick while it runs); Attack's Tick;
/// ComboHL slot 143.
fn tick(f: &mut ShipCombo, cx: &mut Ctx, dt: f32) {
    let cmd = cx.pawn.combat.form.cmd;
    if cmd == sh::CHANGE_FL {
        throw_chain(f, cx);
    }
    if f.warp_delay > 0.0 {
        f.warp_delay -= dt;
        if f.warp_delay > 0.0 {
            return;
        }
        f.warp_delay = 0.0;
        if f.warp {
            warp_hl(f, cx);
            // (AUDIT-B F-SHIP) slot 117 0x14134AA80 after the move: vt+0x1A0(m_CmnAuraEffect, +0x22C + 1, 0): the reappearance
            sh::warp_aura(cx, WARP_AURA + 1);
        }
        play(f, cx);
        cx.set_voice_group_no(crate::host::audio::attack_voice_group_no(i32::from(f.a.kind)));
        return;
    }
    f.a.tick(cx, dt);
    if cmd == sh::CHANGE_HL {
        rain(f, cx, dt);
    }
}

/// End: Attack's End 0x141335120 (with the warp's invincibility taken off: pawn vt+0x2068(0)).
fn end(f: &mut ShipCombo, cx: &mut Ctx) {
    if std::mem::take(&mut f.invincible) {
        cx.pawn.status.damage_invalid = false;
    }
    // (AUDIT-B F-SHIP) End 0x141335120: the warp still pending (+0x1E1): the reappearance group (+0x22C + 1)
    if std::mem::take(&mut f.warp) {
        sh::warp_aura(cx, WARP_AURA + 1);
    }
    f.a.end(cx);
}

/// The notifies: the Attack state's; ComboHL's shot handler (anim interface slot 69 0x141358E20): outside kind 0x37 a Shot
/// notify turns the rain on (+0x284 = 1, +0x280 = 0, +0x288 = its m_ProjectileName); and the CONVEX groups of the forms
/// (HL_Finish): their prism as a shot of the window's length where the shape stands when the window opens (the shape is
/// `m_bDisableLocation1Attach`: frozen there).
fn on_notifies(f: &mut ShipCombo, cx: &mut Ctx, notifies: &Notifies) {
    f.a.on_notifies(cx, notifies);
    let cmd = cx.pawn.combat.form.cmd;
    for event in &notifies.events {
        match (event.class(), event.phase) {
            (NotifyClass::TresAnimNotifyShot, NotifyPhase::Instant) if cmd == sh::CHANGE_HL && cx.pawn.combat.last_kind != 0x37 => {
                (f.rain_on, f.rain_timer) = (true, 0.0);
            }
            (NotifyClass::TresAnimNotifyStateAttack, NotifyPhase::Begin) => {
                let def = event.def();
                let NotifyData::Attack { group, row, interval, hit_se_asset, .. } = def.data else { continue };
                if sh::convex_of(group).is_none() {
                    continue;
                }
                let clip = cx.pawn.anim.name.name();
                let Some(at) = sh::convex_point(cx, clip, group, cx.pawn.anim.time) else { continue };
                f.a.sync_ctx(cx.pawn);
                let yaw = cx.pawn.yaw;
                sh::spawn_convex(cx, sh::ConvexShot { key: sh::FINISH_SHAPE_KEY, group, row, at, yaw, life: (def.end - def.time).max(0.0), interval: interval.unwrap_or(0.0), hit_se: hit_se_asset });
            }
            _ => {}
        }
    }
}

// ================================================================================================ the request

/// `CanAttack` (slot 44 0x141426330) in the package's forms: attack.rs's rows with the form's class and MaxCombo 5. None =
/// a state whose row is attack.rs's own business (the traversal states: their forced flow kinds).
fn can_attack(state: &StateData, s: &mut Pawn) -> Option<bool> {
    let max = sh::MAX_COMBO;
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
    if let Some(f) = state.ship_combo() {
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
/// attack.rs's own path (the traversal states).
pub(crate) fn try_attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    let cmd = cx.pawn.combat.form.cmd;
    if !sh::is_ship(cmd) {
        return None;
    }
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
    // A running attack state of any class (Attack, ComboLimit, the form classes: StateData::attack_mut).
    let chained = state.attack_mut();
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
        (Attack { combo, air: !super::common::on_ground(s), target, forced_kind, just_guard, ..Attack::RESET }, push)
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

// ================================================================================================ FINISH_FL

/// The finish body's Begin (StyleFinishFL Begin 0x1413CC350 is StyleFinish's, the caller's; Reset 0x1413D0DC0: +0x2E0 ..
/// +0x2EC = 0). `root_len` = +0x2E8 (the clock, counted every tick), `count` = the Kraken's windows already given their
/// shot (a bit each), `a.combo` = the Kraken has started (+0xB48).
pub(crate) fn finish_begin(f: &mut StyleFinish, _cx: &mut Ctx) {
    f.a.sub = 0;
    f.root_len = 0.0;
    f.count = 0;
    f.a.combo = 0;
    f.a.swing_started = false;
}

/// The per-command tick: true = done (chained on itself).
pub(crate) fn finish_tick(f: &mut StyleFinish, cx: &mut Ctx, dt: f32) -> bool {
    if f.cmd != sh::FINISH_FL {
        return true;
    }
    kraken(f, cx, dt)
}

/// StyleFinishFL Tick (0x1413D5060, decompiled C): the Kraken. Its own clip M_F420 runs from sub-step 0 on the clock
/// `root_len`: its hit windows (`pawn.FL.w_so094`: FinishFL / FinishFL2 prisms at its root + (0, 0, 400), SCFL_KRAKEN*,
/// FinishFL2's interval 0.6 s) and its effects are given here at their times; the host draws the pawn
/// (`combat::forms::ship::finisher_pawns`).
fn kraken(f: &mut StyleFinish, cx: &mut Ctx, dt: f32) -> bool {
    let before = f.root_len;
    f.root_len += dt;
    match f.a.sub {
        0 => {
            // The Kraken spawned at his root with his yaw (0x1405D9130), set on the floor (0x1413F5600); he is put where it
            // stands (0x143EAC3D0); it is shown and plays M_F420; he flies (vt+0x17A0(5, 1)) and plays M_F420; +0x164 = 0;
            // +0x2E8 = 0; sub-step 2.
            let (hit, _, floor) = ground_probe(cx.pawn, cx.world, 1200.0);
            let at = cx.pawn.cmc.location;
            cx.pawn.cmc.location = Vec3::new(at.x, at.y, if hit { floor } else { at.z });
            cx.pawn.cmc.stop_movement();
            cx.pawn.cmc.set_flying();
            f.play(cx.pawn, "M_F420", -1.0, 2);
            // (AUDIT-B F-SHIP) the Kraken's own clip notifies (its AttachSound se02009_030, its 62 effects on its bones) fire
            // on its mesh from the generic weapon-clip pass (combat/forms/ship.rs `on_notifies`, rows `wnotify.w_so094.F420.*`).
            f.a.swing_started = false;
            f.root_len = 0.0;
            f.a.sub = 2;
            return false;
        }
        2 => {
            // The Kraken starts (+0xB48 = 1) and the tick returns; the next tick 0x14140E500(ctrl, 0): the world slow off.
            if f.a.combo == 0 {
                f.a.combo = 1;
            } else {
                form::world_slow(cx, false);
                f.a.sub = 5;
            }
        }
        5 => {
            // Until the Kraken's m_Param_MaxTime is reached, m_bEnableChange, or his M_F420 ends.
            if f.root_len >= sh::kraken_time() || cx.pawn.queue.flags.enable_change || cx.pawn.anim.ended {
                f.a.sub = 6;
            }
        }
        6 => {
            // 0x1413CFAF0: the Kraken removed (its shapes with it); Play("M_1501").
            kraken_clean(cx);
            f.play(cx.pawn, "M_1501", -1.0, 2);
            f.a.sub = 7;
            return false;
        }
        7 => {
            f.a.sub = 8;
            return false;
        }
        _ => {
            if cx.pawn.anim.ended {
                cx.pawn.queue.do_chain();
                return true;
            }
            return false;
        }
    }
    kraken_clip(f, cx, before);
    false
}

/// The Kraken's own clip between `before` and the clock: its hit windows (a prism shot each, for the window's length, at its
/// root: Sora stands there). (AUDIT-B F-SHIP) Its effects and sound are its clip's notifies on its own mesh (the generic
/// weapon-clip pass, combat/forms/ship.rs `on_notifies`), no longer all at its root.
fn kraken_clip(f: &mut StyleFinish, cx: &mut Ctx, _before: f32) {
    let root = cx.pawn.cmc.location;
    let yaw = cx.pawn.yaw;
    let hit_se = sh::kraken_hit_se();
    for (k, (begin, end, interval, row, group)) in sh::kraken_windows().into_iter().enumerate() {
        let bit = 1 << k;
        if f.count & bit != 0 || f.root_len < begin {
            continue;
        }
        f.count |= bit;
        f.a.sync_ctx(cx.pawn);
        let key = sh::KRAKEN_KEY + k as u16;
        sh::spawn_convex(cx, sh::ConvexShot { key, group, row, at: root, yaw, life: (end - f.root_len).max(0.0), interval, hit_se });
    }
}

/// 0x1413CFAF0: the Kraken gone, with its shapes.
pub(crate) fn kraken_clean(cx: &mut Ctx) {
    use crate::combat::hit::FORM_SHOT_SEQUENCE;
    cx.pawn.combat.shots.retain(|s| !(s.window.key.0 == FORM_SHOT_SEQUENCE && (sh::KRAKEN_KEY..sh::KRAKEN_KEY + 0x10).contains(&s.window.key.1)));
}
