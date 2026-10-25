//! `TresPlayerState_Attack` (id 12) and its requests `CanAttack` / `TryAttack`
//! (docs/sora-combat-spec.md sections 3, 4.1, 4.3, 5, 6; the decision itself is
//! `combat/decide.rs`; hit windows are `combat/hit.rs`; targeting `combat/targeting.rs`).
//! Addresses are the exe's. One instance plays one attack; the combo is a chain of queued
//! instances.
//!
//! Literal readings and assumptions (each reported; marked where they are):
//! - `End` 0x141335120 was not read (spec 5, open question 1): everything on its call list is
//!   restored unconditionally;
//! - the per-kind bits of state+0x234 that the sheet does not carry (bit 3 "no vertical homing",
//!   bits 15 / 16 of the root-scale channel) are never set;
//! - `TargetMetrics`' velocity lead of the target (`slot453 x 8 x [0x14871E340]`) and the "adjusted
//!   actor location" (pawn slot 220) are not read: the lock-on point and the actor origin are used;
//! - `PreDecide`'s floor-slope test (FUN_14063C870, pitch < -35) is not made;
//! - the Z channel's unspecified branch (DirZ and dz of different signs) commands 0;
//! - the root scale (spec 6.3, open question 4) is applied to the root motion of each tick.
//!
//! Not here: launch requests (spec 3.4), forced kinds (guard / risk-dodge counters, slide and
//! dive-fall attacks), the weapon recoil (slot 129), the chase warp (6.4), the swim exit, the
//! attack camera. A kind without a row plays nothing (spec 4.2, last consequence).
//! (All but the swim exit and the dive-fall attack have since been added by the later passes;
//! the fourth pass, 2026-10-12: state+0x234 bit 3 / +0x23C bit 4, the HOPPING / SLOPESLIDE
//! requests with kind 0x2F, the sneeze latch controller+0x920, the attack camera's calls.)

use glam::{Vec2, Vec3};

use super::common::{ground_probe, on_ground, turn};
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, NotifyClass, NotifyPhase, Notifies, PlayArgs};
use crate::combat::decide::{decide, ComboAbilities, DecideIn};
use crate::combat::tables::{attack_kind, P};
use crate::combat::AttackCtx;
use crate::host::{TargetFlags, TargetId, TargetInfo};
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Attack {
    pub(super) sub: u8,
    /// +0x118 `m_HomingTargetComponent`.
    pub target: TargetId,
    /// +0x128.
    pub combo: i32,
    /// +0x12C, +0x130, +0x134, +0x138: hit counters (their meaning is open: strand C question 4).
    pub(super) counters: [i32; 4],
    /// +0x140, +0x141, +0x142, +0x143.
    pub(super) finish: bool,
    pub(super) finish_end: bool,
    pub air: bool,
    pub(super) next_queued: bool,
    /// +0x14C, +0x150, +0x154, +0x164.
    pub(super) z_gain: f32,
    pub(super) speed_override: f32,
    pub(super) reach: u8,
    pub(super) swing_started: bool,
    /// +0x1B0, +0x1C4, +0x1CC, +0x1D0, +0x1D4, +0x1D8.
    pub kind: u8,
    pub(super) num_enemies: i32,
    pub(super) dist: f32,
    pub(super) dz: f32,
    pub(super) dir_z: f32,
    pub(super) root_len: f32,
    /// +0x1DC, +0x1DD, +0x1DE, +0x1DF, +0x1E0.
    pub(super) target_on_ground: bool,
    pub(super) fixed_dir_z: bool,
    pub(super) dive: bool,
    pub(super) air_mode: bool,
    pub(super) target_high: bool,
    /// +0x1EC bit 2.
    pub(super) launch: bool,
    /// +0x84 bits 2, 4, 5: `m_bEnableTraction`, `m_bFaceAt`, `m_bEnableInput`.
    pub(super) enable_traction: bool,
    pub(super) face_at: bool,
    pub(super) enable_input: bool,
    /// +0x1F4 bits 3 / 4, +0x1F8, +0x1FC.
    pub(super) facing_on: bool,
    pub(super) keep_face: bool,
    pub(super) face_timer: f32,
    pub(super) face_yaw: f32,
    /// +0x220, +0x224, +0x228: the channel phases; +0x234 bits 0 / 1: the channels in use.
    pub(super) phase_xy: u8,
    pub(super) phase_root: u8,
    pub(super) phase_z: u8,
    pub(super) chan_xy: bool,
    pub(super) chan_root: bool,
    /// +0x24C.
    pub(super) stop_dist: f32,
    /// +0x18C: 0.6 for an air-mode attack, zeroed by a landed hit (strand A 6.3).
    pub(super) air_hold: f32,
    /// +0x254 and +0x234 bit 7: the cancel window (what the guard / dodge / air-slide gates test).
    pub(super) cancel_timer: f32,
    pub cancel: bool,
    /// +0x234 bit 4 (set with the air mode): the hover that ends at `TresEnableChange`.
    pub(super) hover: bool,
    /// +0x23C bit 3: the Z channel re-arms at each traction window (kinds 0x14, 0x15).
    pub(super) rearm_z: bool,
    /// +0x1E8: the forced kind (guard counters 0x21 / 0x23 / 0x24, blow counter 0x27, risk-dodge
    /// counters 0x28 - 0x2B: combat spec 5.6.2); 0 = the decision's kind.
    pub forced_kind: u8,
    /// +0x23C bit 0: a just-guard attack (the damage rule of spec 8.3; `info.justGuard`).
    pub just_guard: bool,
    /// The instance is a `TresPlayerState_ComboLimit` (id 90, Second Form): its `Decide`
    /// 0x141362AB0 (states/combo_limit.rs wraps this struct).
    pub limit: bool,
    /// state+0x1EC bit 3 with +0x1E4: the attack was requested out of a wall run of this class
    /// (0 up, 1 / 2 along): `Decide` rule 4 picks a wall-run attack, kinds 0x31 - 0x34.
    pub wall_run: Option<u8>,
    /// state+0x234 bit 21: the endless combo of CHARGE_BERSERK (set by Begin; `CanAttack` then does
    /// not stop at `MaxCombo`).
    pub(super) endless: bool,
    /// +0x258: the warp countdown of the chase attack (kind 0x22; > 0: `PlayAttack` waits), and
    /// +0x1E1: the teleport is still to be made.
    pub(super) warp_delay: f32,
    pub(super) warp_start: bool,
    /// state+0x234 bit 5: the dive has landed; bit 6: the shot handler was called once.
    pub(super) dive_landed: bool,
    pub(super) shot_called: bool,
    /// state+0x170: shots this attack has spawned.
    pub shots: i32,
    // ---- fourth pass (2026-10-12)
    /// state+0x234 bit 3: while the traction window is open the Z channel writes 0 (kinds 0x1C
    /// Rising Spiral and 0x37 Sonic Cleave of the Kingdom Key's set; [I] 0x14134A74D).
    pub(super) no_vz: bool,
    /// state+0x23C bit 4: `TargetMetrics` leaves DirZ alone ([I] 0x141331132; the same kinds).
    pub(super) keep_dir_z: bool,
    /// state+0x1EC bit 4 / bit 5: the attack was requested out of HOPPING / out of SLOPESLIDE
    /// (`TryAttack` cases 9 / 10): `PlayAttack` then plays kind 0x10, or 0x2F below the target.
    pub hop: bool,
    pub slope: bool,
    /// state+0x234 bit 15, rush suppression: written by the body-contact handler (notify
    /// sub-object slot 8 0x141343160) while his push body touches the target's:
    /// `= target.m_bRushSuppression`. The XY channel then homes at 0 and the root-scale channel
    /// writes 0.1 (spec 6.3).
    pub(super) rush: bool,
}

impl Attack {
    /// Reset (0x14133D230).
    pub const RESET: Attack = Attack {
        sub: 0,
        target: TargetId::NONE,
        combo: 0,
        counters: [0; 4],
        finish: false,
        finish_end: false,
        air: false,
        next_queued: false,
        z_gain: 0.0,
        speed_override: 0.0,
        reach: 0,
        swing_started: false,
        kind: 0,
        num_enemies: 0,
        dist: 0.0,
        dz: 0.0,
        dir_z: 0.0,
        root_len: 0.0,
        target_on_ground: true,
        fixed_dir_z: false,
        dive: false,
        air_mode: false,
        target_high: false,
        launch: false,
        enable_traction: false,
        face_at: false,
        enable_input: false,
        facing_on: false,
        keep_face: false,
        face_timer: 0.0,
        face_yaw: 0.0,
        phase_xy: 0,
        phase_root: 0,
        phase_z: 0,
        chan_xy: false,
        chan_root: false,
        stop_dist: 160.0,
        air_hold: 0.0,
        cancel_timer: 0.0,
        cancel: false,
        hover: false,
        rearm_z: false,
        forced_kind: 0,
        just_guard: false,
        limit: false,
        wall_run: None,
        endless: false,
        warp_delay: 0.0,
        warp_start: false,
        dive_landed: false,
        shot_called: false,
        shots: 0,
        no_vz: false,
        keep_dir_z: false,
        hop: false,
        slope: false,
        rush: false,
    };

    /// The target, while it is targetable (component vt+0xA80).
    pub(super) fn target_info(&self, cx: &Ctx) -> Option<TargetInfo> {
        cx.target(self.target).filter(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE))
    }

    /// `TargetMetrics` (slot 123 0x141330D30), spec 4.1.
    pub(super) fn target_metrics(&mut self, cx: &Ctx) {
        let own = cx.pawn.cmc.location;
        let Some(t) = self.target_info(cx) else { return };
        // The lead (0x141330E25; strand B 2): where the target will be in 8 / 30 s.
        let p = t.center + t.velocity * (8.0 / 30.0);
        self.target_on_ground = !t.flags.has(TargetFlags::AIRBORNE);
        // (er-host, IMPL_ER14) less the terrain's own rise between the two (0 unless the host answers: `Targets::terrain_dz`)
        self.dz = p.z - own.z - cx.host.targets.terrain_dz(t.id);
        self.target_high = self.dz > P.decide_rise_dz;
        self.dist = ((p - own).truncate().length() - P.decide_dist_offset).max(0.0);
        // [I] 0x141331132: `test byte [state+23Ch],10h; jne` skips the DirZ write.
        if self.keep_dir_z {
            return;
        }
        self.dir_z = 0.0;
        if self.dz.abs() > 30.0 {
            self.dir_z = (self.dz * P.traction_z_initial_gain).clamp(-P.traction_z_initial_max, P.traction_z_initial_max);
        }
    }

    pub(super) fn sync_ctx(&self, pawn: &mut Pawn) {
        pawn.combat.attack_ctx = AttackCtx { kind: self.kind, counters: [self.combo, self.counters[0], self.counters[1], self.counters[2]], finish_end: self.finish_end, aerial: self.air_mode, just_guard: self.just_guard, has_target: self.target != TargetId::NONE, target: self.target, state: true };
    }

    /// (magic implementer) Slot 124 (0x1413325F0, decompiled C; called once with 0 from the head
    /// of `PlayAttack`): the FLOWMOTION ATTACK CHOICE. A forced kind 0x36 / 0x37 becomes Buzz Saw
    /// (0x38) with two or more enemies around and TURN_CUTTER (93); else Sonic Slash (0x36) when
    /// the target is farther than 800 cm and SONIC_SLASH (91) is equipped; else Sonic Cleave (0x37)
    /// when it is more below than away and SONIC_DOWN (92) is equipped. A flow kind whose own
    /// ability (0x39: SUMMERSALT 94) is missing is dropped for an ordinary air attack by the
    /// reach (state+0x154: 1 -> 0x11, 2 -> 0x12, 3 -> 0x0F, else 0x10).
    fn flow_refine(&mut self, pawn: &Pawn) {
        use crate::enums::ETresAbilityKind as Ab;
        let has = |a| pawn.status.abilities.has(a);
        if matches!(self.forced_kind, 0x36 | 0x37) {
            if self.num_enemies >= 2 && has(Ab::TurnCutter) {
                self.forced_kind = 0x38;
            } else if self.dist > 800.0 && has(Ab::SonicSlash) {
                self.forced_kind = 0x36;
            } else if self.dist < self.dz.abs() && has(Ab::SonicDown) {
                self.forced_kind = 0x37;
            }
        }
        let ability = match self.forced_kind {
            0x36 => Ab::SonicSlash,
            0x37 => Ab::SonicDown,
            0x38 => Ab::TurnCutter,
            0x39 => Ab::Summersalt,
            _ => return,
        };
        if !has(ability) {
            self.forced_kind = match self.reach {
                1 => 0x11,
                2 => 0x12,
                3 => 0x0F,
                _ => 0x10,
            };
        }
    }

    /// `PlayAttack` (slot 120 0x1413378B0), spec 4.3.
    pub(super) fn play_attack(&mut self, cx: &mut Ctx) {
        let p = *cx.pawn.params();
        // The head of PlayAttack (0x1413378B0, decompiled C; the kinds' own cases [I]): the
        // requests out of HOPPING (state+0x1EC bit 4; bit 6 is CLIMBING's) and SLOPESLIDE (bit 5):
        //   if +0x1EC & 0x50: kind = 0x10;                 forcedKind = 0; bAir = 1
        //   elif +0x1EC & 0x20: kind = dz > 30 ? 0x10 : 0x2F; forcedKind = 0; bAir = 1
        if self.hop || self.slope {
            self.kind = if self.slope && !(self.dz > 30.0) { 0x2F } else { 0x10 };
            (self.forced_kind, self.air) = (0, true);
        }
        let kind = self.kind;
        // The attack camera of the kinds that ask for one (0x1413378B0: 0x14140DF20(ctrl, 0, 0, 0, 0),
        // then on): 0x38 Buzz Saw mode 5 (the camera pulls back: distance x1.6), 0x28 Hook Upper
        // mode 3 (the lock-on camera on the target). Off again at End.
        match kind {
            0x38 => {
                crate::combat::camera::off(cx);
                crate::combat::camera::request(cx, true, 5, TargetId::NONE);
            }
            0x28 => {
                crate::combat::camera::off(cx);
                crate::combat::camera::request(cx, true, 3, self.target);
            }
            _ => {}
        }
        let pawn = &mut *cx.pawn;
        pawn.combat.last_kind = kind;
        if pawn.combat.history.len() >= 16 {
            pawn.combat.history.remove(0);
        }
        pawn.combat.history.push(kind);
        // Defaults.
        self.face_at = true;
        (self.chan_xy, self.chan_root) = (true, false);
        (self.stop_dist, self.z_gain) = (P.traction_stop_dist, P.traction_z_gain);
        self.air_mode = false;
        // Kind 0x1E, the sneeze (0x1413378B0 case 0x1E, decompiled C): `BadStatSneeze` on the ground,
        // `BadStatAirSneeze` with the air-mode flag off it; the traction channel off (+0x234 bit 0).
        if kind == 0x1E {
            (self.chan_xy, self.chan_root) = (false, false);
            self.air_mode = self.air;
            if self.air_mode {
                pawn.cmc.gravity_scale = P.air_gravity_scale;
                pawn.cmc.gravity_off = true;
                pawn.cmc.set_flying();
                self.hover = true;
                self.air_hold = P.air_hold_time;
            } else {
                pawn.cmc.gravity_off = false;
                self.dir_z = 0.0;
            }
            pawn.combat.swing_hit = false;
            pawn.root_scale = (1.0, 1.0);
            match AnimId::from_name(if self.air { "BadStatAirSneeze" } else { "BadStatSneeze" }) {
                Some(anim) => pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
                None => pawn.note("the sneeze clip is not in the generated animation table"),
            }
            Self::sneeze_latch(pawn, self.endless, self.combo);
            // (No hit window opens in the clip: `swing_started` is what lets the state chain.)
            self.swing_started = true;
            self.face_yaw = pawn.yaw;
            self.sync_ctx(pawn);
            return;
        }
        // (party stream D) Kinds 0x3F..0x43, the unison / fusion finishers (0x1413378B0 cases 0x3F ..
        // 0x43, decompiled C): clip DoUFire / DoUBlizzard / DoUThunder / GooFSpin / GooFRocket,
        // +0x234 |= 0x808 (Rocket also +0x1D8 = 1300), +0x1DF = 1; the friend joins on Sora's
        // "vfx_root" (TresNpcState_n_ex001_FRDonaldSora 0x1411CACA0 / n_ex002_FRGoofySora
        // 0x1411CB5D0); the clip's Shot notify spawns the team shot (slots 137 / 138).
        if (0x3F..=0x43).contains(&kind) {
            (self.chan_xy, self.chan_root, self.no_vz) = (false, false, true);
            pawn.combat.swing_hit = false;
            pawn.root_scale = (1.0, 1.0);
            pawn.cmc.gravity_off = false;
            self.dir_z = 0.0;
            self.swing_started = true;
            self.face_yaw = pawn.yaw;
            let target = self.target;
            crate::combat::friend::play_unison(cx, kind, target);
            self.sync_ctx(cx.pawn);
            return;
        }
        let Some(row) = attack_kind(kind) else {
            pawn.note("an attack kind without a row was decided (spec 4.2: an empty finisher slot): nothing is played");
            return;
        };
        (self.stop_dist, self.z_gain, self.air_mode, self.keep_face) = (row.stop_dist, row.z_gain, row.air_mode, row.keep_face);
        self.root_len = row.root_len.unwrap_or(0.0);
        self.chan_root = row.root_len.is_some();
        match kind {
            // "root length and root-scale mode (traction channel off) only when dist < 300".
            3 | 0x12 => {
                let near = self.dist < 300.0;
                (self.chan_root, self.chan_xy) = (near, !near);
                if !near {
                    self.root_len = 0.0;
                }
            }
            // "traction speed state+0x150 = 800".
            0x17 => self.speed_override = P.traction_speed,
            0x0D | 0x0E => self.dive = true,
            // Wall-run attacks 0x32 / 0x33: the root-scale channel is ADDED (+0x234 |= 2), root length 595.
            0x32 | 0x33 => (self.chan_xy, self.chan_root) = (true, true),
            // 0x34: m_bFaceAt off (z gain 4.5 and stop distance 60 are the row's).
            0x34 => self.face_at = false,
            // 0x39, the Somersault: no target, no facing, every traction channel off (+0x234 &= ~7),
            // cancel only at TresEnableChange (bit 8), callback mode 3.
            0x39 => {
                (self.face_at, self.facing_on, self.chan_xy, self.chan_root) = (false, false, false, false);
                self.target = TargetId::NONE;
            }
            0x14 | 0x15 => self.rearm_z = true,
            // 0x1C Rising Spiral (clip 1319) [I 0x14133817C]: +0x234 |= 8, +0x23C |= 0x10.
            0x1C => (self.no_vz, self.keep_dir_z) = (true, true),
            _ => {}
        }
        // Kind 0x2F, the slope-slide attack (clip M_1143, row RUN_SLOPE) [I 0x14133A160]: the Play
        // call's root scale XY = clamp(dist / 815, 0.1, 2.5), state+0x18C = 0.3 (the air-mode tail
        // writes 0.6 over it), air mode.
        // (magic implementer) Case 0x36, the airstep's flow attack (clip 1181): the Play call's
        // root scale XY = clamp(dist / 850, 0.1, 0.7058824) (0x1413378B0: fVar28 = 850, fVar32 =
        // 0.7058824; the tail's "dist / fVar28" clamp), state+0x18C = 0.3. (State+0x190 bit 0: not read.)
        let mut flow_scale = None;
        if kind == 0x36 {
            flow_scale = Some(if self.dist != 0.0 { (self.dist / 850.0).clamp(0.1, 0.705_882_4) } else { 1.0 });
        }
        // Case 0x38 (Buzz Saw, clip 1183): fVar28 = 1000 with the default upper bound 2.5.
        if kind == 0x38 {
            flow_scale = Some(if self.dist != 0.0 { (self.dist / 1000.0).clamp(0.1, 2.5) } else { 1.0 });
        }
        if kind == 0x2F {
            flow_scale = Some(if self.dist != 0.0 { (self.dist / 815.0).clamp(0.1, 2.5) } else { 1.0 });
        }
        // Case 0x37 (Sonic Cleave, clip 1182): a dive (state+0x1DE) with DirZ (state+0x1D4) = -1000.
        // FOURTH PASS, the case's other writes [I 0x14133A2EA - 0x14133A31C]: +0x23C |= 0x10 (DirZ is
        // kept by TargetMetrics), +0x234 |= 8 (the Z channel writes 0 while the traction window is
        // open: he comes down by the clip's root motion, 500 cm; the dive's ground probe and landing
        // are never reached inside the window), +0x190 |= 1 and +0x148 = 5.0 (no reader in the Attack
        // class: byte scan of 0x141330000 - 0x141350000), stop distance 250.
        let cleave = kind == 0x37;
        if cleave {
            self.dive = true;
            (self.no_vz, self.keep_dir_z) = (true, true);
        }
        // Tail.
        if !self.air_mode {
            pawn.cmc.gravity_off = false;
            self.dir_z = 0.0;
        } else {
            pawn.cmc.gravity_scale = P.air_gravity_scale;
            pawn.cmc.gravity_off = true;
            pawn.cmc.set_flying();
            self.hover = true;
            self.air_hold = P.air_hold_time;
        }
        pawn.combat.swing_hit = false;
        pawn.root_scale = (flow_scale.unwrap_or(1.0), 1.0);
        // State+0x18C = 0.3 for 0x36 (the air-mode tail writes 0.6 over it for 0x38).
        if kind == 0x36 {
            self.air_hold = 0.3;
        }
        if cleave {
            self.dir_z = -1000.0;
        }
        match AnimId::from_name(row.anim) {
            // Mode 2: one-shot with root motion; blend-in the asset's.
            Some(anim) => pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
            None => pawn.note("an attack animation is not in the generated animation table"),
        }
        Self::sneeze_latch(pawn, self.endless, self.combo);
        let _ = p;
        self.face_yaw = pawn.yaw;
        // (XSI) The XS abilities' attacks (abilities/xs): Sonic Dash moves by its own root motion (no traction)
        if matches!(kind, crate::abilities::xs::kind::SONIC_DASH | crate::abilities::xs::kind::SONIC_LAUNCH) {
            // ... and does not turn back to the target it flashed through (the facing channel is off).
            (self.chan_xy, self.chan_root, self.face_at, self.facing_on) = (false, false, false, false);
        }
        self.sync_ctx(pawn);
        crate::abilities::xs::on_play_attack(cx, kind, self.target);
        self.face_yaw = cx.pawn.yaw;
    }

    /// The tail of PlayAttack [I 0x14133AC8A - 0x14133ACB7]: with the SNEEZE status on him
    /// (pawn+0x85D == 13), outside the endless combo (state+0x234 bit 21) and from the step
    /// `combo >= MaxCombo() - 2` on: `0x14140DF30(ctrl, 1)` = controller+0x920 = 1 and the HUD's
    /// Attack command becomes "Sneeze" (0x141583FF0): the NEXT attack of the chain is the sneeze
    /// (Decide rule 6). So the sneeze takes the place of the combo's finisher.
    fn sneeze_latch(pawn: &mut Pawn, endless: bool, combo: i32) {
        if pawn.combat.bad.is(crate::combat::badstat::kind::SNEEZE) && !endless && combo >= max_combo(pawn) - 2 {
            pawn.combat.sneeze_next = true;
        }
    }

    /// Facing (slot 126 0x14134D0E0), spec 6.2.
    pub(super) fn facing(&mut self, cx: &mut Ctx, dt: f32) {
        let mut alpha = dt * P.face_rate;
        if std::mem::take(&mut self.face_at) {
            self.facing_on = true;
            self.face_timer = P.face_timer;
            alpha = 1.0;
            self.target_metrics(cx);
        }
        if !self.facing_on {
            return;
        }
        if let Some(t) = self.target_info(cx) {
            let d = t.center - cx.pawn.cmc.location;
            if d.truncate().length_squared() > 1.0e-8 {
                self.face_yaw = d.y.atan2(d.x).to_degrees();
            }
        }
        turn(cx.pawn, self.face_yaw, alpha);
        if !self.keep_face {
            self.face_timer -= dt;
            if self.face_timer <= 0.0 {
                self.facing_on = false;
            }
        }
    }

    /// Traction (slot 116 0x141349CC0), spec 6.3: the XY homing channel, the root-scale channel
    /// and the Z channel.
    pub(super) fn traction(&mut self, cx: &mut Ctx, dt: f32) {
        // The body-contact notification of this frame (0x140633EE0 -> slot 8 0x141343160): only
        // for the pawn that owns the attack's target; no contact leaves the bit as it is.
        if self.target != TargetId::NONE && crate::combat::push::touching(cx, self.target) {
            self.rush = cx.host.targets.rush_suppression(self.target);
        }
        let target = self.target_info(cx);
        let w = cx.world;
        let pawn = &mut *cx.pawn;
        let velocity = pawn.cmc.velocity;
        let speed_xy = velocity.truncate().length();
        let (dzc, dxy) = match target {
            Some(t) => {
                let d = t.center - pawn.cmc.location;
                (d.z.clamp(-P.traction_dz_clamp, P.traction_dz_clamp), d.truncate().length().clamp(0.0, P.traction_dist_clamp))
            }
            None => (0.0, 0.0),
        };
        let first_air = self.combo == 0 && self.air;
        let k = P.traction_first_air_scale;
        let open = self.enable_traction;
        // ---- XY channel
        if self.chan_xy {
            let (mut wanted, mut rate) = (0.0f32, P.traction_rate_on);
            // The phase follows the window (a second window re-opens the channel: strand B 2).
            self.phase_xy = open as u8;
            let mut inside = false;
            if self.phase_xy == 1 {
                if self.rush {
                    // "elif bit 15: inside = true".
                    inside = true;
                } else {
                    wanted = self.speed_override;
                    if wanted == 0.0 {
                        if dxy > self.stop_dist {
                            wanted = P.traction_speed;
                        } else {
                            inside = true;
                        }
                    }
                }
                if first_air {
                    wanted *= k;
                    rate = P.traction_rate_on * k;
                }
            }
            if !(wanted > 0.0) {
                wanted = 0.0;
                rate = if inside {
                    P.traction_rate_off_ground
                } else if self.air_mode && !first_air {
                    P.traction_rate_off_air
                } else {
                    P.traction_rate_off_ground
                };
            }
            // Approach (slot 135 0x1413364D0).
            let v = if speed_xy == wanted {
                wanted
            } else {
                let mut d = wanted - speed_xy;
                let f = rate * dt * 60.0;
                if f < 1.0 {
                    d *= f;
                }
                if d.abs() < 0.0003 { wanted } else { speed_xy + d }
            };
            let a = self.face_yaw.to_radians();
            pawn.cmc.set_velocity_xy(Vec2::new(a.cos(), a.sin()) * v);
        }
        // ---- root-scale channel
        if self.chan_root {
            if self.phase_root == 0 && open {
                self.phase_root = 1;
            }
            if self.phase_root == 1 {
                if open {
                    let mut scale = match target {
                        Some(_) if self.root_len > 0.0 => dxy / self.root_len,
                        Some(_) => 0.0,
                        None => 1.0,
                    };
                    if first_air && target.is_some() {
                        scale *= k;
                    }
                    // "bit 15: SetRootScale(0.1)".
                    pawn.root_scale.0 = if self.rush { 0.1 } else { scale };
                } else {
                    // "unless bit 15 or 16: SetRootScale(1.0)".
                    if !self.rush {
                        pawn.root_scale.0 = 1.0;
                    }
                    self.phase_root = 2;
                }
            }
        }
        // ---- Z channel (air mode)
        if self.air_mode {
            if self.phase_z == 0 && open {
                self.phase_z = 1;
            }
            if self.phase_z == 1 && open {
                let vz;
                if self.no_vz {
                    // [I] 0x14134A74D: `test dl,8` (state+0x234 bit 3) before the dive test: vz = 0.
                    vz = 0.0;
                } else if self.dive {
                    let (hit, distance, _) = ground_probe(pawn, w, P.decide_ground_probe_range);
                    let g = if hit { distance } else { P.decide_ground_probe_range };
                    if on_ground(pawn) || g < 1.0 {
                        // Landed: +0x234 |= 0x20, the shot handler (iface40 vt+0x228 0x141344580: the
                        // shockwave, below), traction off, StopMovement, the air-mode channel off.
                        if !self.dive_landed {
                            self.dive_landed = true;
                            self.shot_handler(pawn);
                        }
                        self.enable_traction = false;
                        pawn.cmc.stop_movement();
                        self.air_mode = false;
                        self.phase_z = 2;
                        pawn.cmc.gravity_off = false;
                        if pawn.cmc.is_flying() {
                            pawn.cmc.set_falling();
                        }
                        return;
                    }
                    vz = velocity.z.min(self.dir_z) + self.dir_z * 0.25;
                } else if self.fixed_dir_z {
                    vz = self.dir_z;
                } else if self.dir_z != 0.0 && dzc != 0.0 && (self.dir_z > 0.0) == (dzc > 0.0) {
                    self.dir_z = dzc * self.z_gain;
                    vz = self.dir_z;
                } else {
                    // The signs differ (or DirZ is 0): the current Z speed is written again.
                    vz = velocity.z;
                }
                pawn.cmc.set_velocity_z(vz);
            } else if self.phase_z == 1 {
                // The window closed: gravity on, Falling.
                pawn.cmc.gravity_off = false;
                if pawn.cmc.is_flying() {
                    pawn.cmc.set_falling();
                }
                self.phase_z = 2;
            }
            if self.phase_z == 2 {
                // No write at all once DirZ is 0: he then falls under the (scaled) gravity.
                if self.dir_z != 0.0 {
                    let next = self.dir_z - dt * self.dir_z * P.traction_z_decay;
                    self.dir_z = if (next > 0.0) != (self.dir_z > 0.0) { 0.0 } else { next };
                    pawn.cmc.set_velocity_z(self.dir_z);
                }
                if self.rearm_z && open {
                    self.phase_z = 1;
                    self.dir_z = if dzc.abs() > 30.0 { (dzc * P.traction_z_initial_gain).clamp(-P.traction_z_initial_max, P.traction_z_initial_max) } else { 0.0 };
                }
            }
        }
    }
}

/// `MaxCombo()` and the abilities of the decision, for the pawn as it stands.
fn combo_abilities(s: &Pawn) -> ComboAbilities {
    ComboAbilities::of(&s.status.abilities)
}

/// The attack state runs in a form of the Second Form family: its class is `ComboLimit` (pawn
/// slot 1337 0x141432F00, table 0x14143316C; spec 10.5).
pub(crate) fn in_form(s: &Pawn) -> bool {
    crate::combat::form::is_second_form(s.combat.form.cmd)
}

/// `MaxCombo()` (pawn slot 928 0x141433570): 6 in Second Form (Combo Plus is only in the
/// no-form branch), else 4 + Combo Plus / Air Combo Plus.
pub(crate) fn max_combo(s: &Pawn) -> i32 {
    if in_form(s) { P.form_max_combo } else { combo_abilities(s).max_combo(on_ground(s)) }
}

/// `Finishers()` (pawn slot 1253 0x141433A60): 3 in Second Form.
pub(crate) fn finishers(s: &Pawn) -> i32 {
    if in_form(s) { P.form_finishers } else { combo_abilities(s).finishers() }
}

impl Attack {
    /// The shot handler (anim interface vt+0x228 0x141344580, verdict same-partial; decompiled C,
    /// the switch and the bit tests read there; spec 8.8). It is called by `TresAnimNotify_Shot`
    /// and by the dive's landing (traction, 0x14134A7E0 / 0x14134A8D5). By attack kind
    /// (pawn vt+0x1DC8) it spawns a shot of the pawn's projectile set; of the Kingdom Key's base
    /// move set only kind 0x0E (the finisher air-down, clip 1318) has a case:
    ///
    /// ```text
    /// case 0x0E: if (state+0x234 & 0x60) != 0x60: state+0x234 |= 0x40; return
    ///            name = "DOWNIMPACT"
    /// ```
    ///
    /// Bit 5 is set by the landing just before its call, bit 6 by the first call: the shot needs
    /// BOTH the landing and one earlier call. Clip 1318 has its Shot notify at 0.6667 s, so the
    /// shockwave appears at the later of the two: on landing when he lands after 0.6667 s, at
    /// 0.6667 s when he landed before. Kind 0x0D (the non-finisher air-down, the same clip) has
    /// no case: no shockwave. Magic Flash (7) and Final Heaven (0x0C) have none either.
    pub(super) fn shot_handler(&mut self, pawn: &mut Pawn) {
        // Case 0x37 (Sonic Cleave, the flowmotion attack; clip 1182 with its Shot notify at
        // 0.8667 s): the same two bits, and only while no shot was spawned yet (state+0x170 == 0):
        // "SHOCKWAVE_S", 100 cm in front of the shot location. Its first landed hit with combo 0
        // also sets both bits and calls the handler (on-contact 0x14134208A): see `tick`.
        let name = match self.kind {
            0x0E => "DOWNIMPACT",
            0x37 => "SHOCKWAVE_S",
            // (party stream D) slot 137 0x14134CCE0 / slot 138 0x14134C710: the team shots, at once.
            0x3F..=0x42 => {
                pawn.combat.pending_shot = Some(crate::combat::friend::unison_shot(self.kind));
                self.shots += 1;
                return;
            }
            _ => return,
        };
        if !(self.dive_landed && self.shot_called) {
            self.shot_called = true;
            return;
        }
        if self.kind == 0x37 && self.shots > 0 {
            return;
        }
        pawn.combat.pending_shot = Some(name);
        self.shots += 1;
    }
}

impl super::StateData {
    /// The attack fields of a running attack state: `Attack` (12) or `ComboLimit` (90).
    pub(crate) fn attack(&self) -> Option<&Attack> {
        match self {
            StateData::Attack(a) => Some(a),
            StateData::ComboLimit(c) => Some(&c.0),
            _ => None,
        }
    }

    pub(crate) fn attack_mut(&mut self) -> Option<&mut Attack> {
        match self {
            StateData::Attack(a) => Some(a),
            StateData::ComboLimit(c) => Some(&mut c.0),
            _ => None,
        }
    }
}

/// Queues an attack instance of the class the pawn's form asks for.
fn queue_attack(s: &mut Pawn, next: Attack, push: bool) {
    let limit = in_form(s);
    let next = Attack { limit, ..next };
    match (limit, push) {
        (true, true) => s.queue.push_state(super::combo_limit::ComboLimit(next)),
        (true, false) => s.queue.set_state(super::combo_limit::ComboLimit(next)),
        (false, true) => s.queue.push_state(next),
        (false, false) => s.queue.set_state(next),
    }
}

/// (magic implementer) The airstep's enemy stop, sub-step 7 (0x141347455): `a = SetState(ATTACK
/// 12); a+0x118 = the target component; a+0x1E8 = kind` (0x36 after a plain arrival, 0x38 after
/// an enemy turn). The caller chains.
pub(crate) fn flow_attack(s: &mut Pawn, target: TargetId, kind: u8) {
    s.combat.lockon.attack_target = target;
    s.combat.combo = 0;
    queue_attack(s, Attack { combo: 0, air: true, target, forced_kind: kind, ..Attack::RESET }, false);
}

/// (XSI) An attack the XS abilities start themselves (abilities/xs: Sonic Dash, Ancient Light ...): the queue is
/// cleared and the attack of `kind` replaces the running state (`SetState`), with `target`.
pub(crate) fn xs_forced_attack(s: &mut Pawn, target: TargetId, kind: u8, _air: bool) {
    s.queue.clear();
    s.combat.lockon.attack_target = target;
    s.combat.combo = 0;
    let air = !on_ground(s);
    queue_attack(s, Attack { combo: 0, air, target, forced_kind: kind, ..Attack::RESET }, false);
}

/// `CanAttack` (slot 44 0x141426330), spec 3.2: the rows of the states this port has.
fn can_attack(state: &StateData, s: &mut Pawn) -> bool {
    // pawn+0xE34 > 0: the command cool-down (strand A 1.3).
    if s.combat.command_cooldown > 0.0 {
        return false;
    }
    // 0x141426401: with Combo Master the hit confirm is always given.
    if s.status.abilities.has(crate::enums::ETresAbilityKind::ComboMaster) {
        s.combat.hit_flag = true;
    }
    let s = &*s;
    let queued_attack = matches!(s.queue.queued_id(), Some(StateId::Attack) | Some(StateId::ComboLimit));
    // The counter windows of the actions implementer's states (spec 5.6.2; combat/guard.rs):
    // GUARD accepts an attack whenever the counter window is open; RISKDODGE / AIR_SLIDE while
    // their counter is offered.
    match state.id() {
        StateId::Guard if s.actions.counter_window > 0.0 => return true,
        // (ACTIONS IMPLEMENTER) ... and, without a window, a plain attack out of the guard-hit clip
        // after its TresEnableInput (0x141426832).
        StateId::Guard if super::guard::attack_out_of_hit(state, s) => return true,
        StateId::AirSlide if s.actions.reaction_command.is_some() => return true,
        id if s.combat.guard.offer.is_some_and(|o| o.source == id) => return true,
        _ => {}
    }
    if let Some(a) = state.attack() {
        // T (0x141426729): the hit-confirm byte pawn+0xE28 and "combo not over" (strand A 2).
        return s.queue.nothing_queued() && !a.next_queued && a.swing_started && a.enable_input && (a.combo < max_combo(s) || a.endless) && s.combat.hit_flag && !s.combat.finish_end;
    }
    match state {
        // (magic implementer) FIREMAGIC (strand A 2): its own row, then T; USE_ITEM: m_bEnableChange.
        StateData::FireMagic(m) => m.can_attack(s, max_combo(s)) && s.combat.hit_flag && !s.combat.finish_end,
        StateData::UseItem(_) => s.queue.flags.enable_change,
        // Idle.timer(+0x9C) <= 0: always (the field is never written: actions spec).
        StateData::Idle(_) | StateData::Run(_) => true,
        // 3 JUMP: !(Jump+0x98 == 2 && Velocity.z > 0) (pawn vt+0x1568 = Velocity.z; the meaning of
        // Jump+0x98 == 2 is not established: taken as never).
        StateData::Jump(_) => true,
        // (combat implementer, 2026-10-12: the rows of the action states, spec 3.2)
        // 4 GLIDE, 5 FLOATING, 11 SWAYING: always.
        StateData::Glide(_) | StateData::Floating(_) | StateData::Swaying(_) => true,
        // 16 DODGEROLL: m_bEnableChange. 20 AIR_SLIDE: flag bit 7 -> no, bit 6 -> yes, else
        // m_bEnableChange (the two bits are the recovery's: handled above; here the plain slide).
        StateData::DodgeRoll(_) | StateData::AirSlide(_) => s.queue.flags.enable_change,
        // 8 WALLRUN: the next state is not an Attack && sub-step 1 (the loop).
        StateData::WallRun(w) => !queued_attack && w.sub == 1,
        // 9 HOPPING, 10 SLOPESLIDE, 21 SUPER_SLIDE, 23 WALL_KICK, 24 POLE_TURN: the next state is
        // not an Attack ([I] jump table 0x141426EFC: cases 9 and 10 share the target 0x141426C38).
        StateData::Hopping(_) | StateData::SlopeSlide(_) | StateData::SuperSlide(_) | StateData::WallKick(_) | StateData::PoleTurn(_) => !queued_attack,
        // 22 RAILSLIDE: the state's own predicate (vt+0x398: NOT READ, taken as true) && the next
        // state is not an Attack.
        StateData::RailSlide(_) => !queued_attack,
        // DAMAGE (0x14142674F; strand E 9.5): nothing queued, the damage kind (state+0x88) in
        // {1, 2, 7, 8, 9, 10}, m_bEnableChange, and no bad status on the pawn (state+0x90).
        StateData::Damage(d) => s.queue.nothing_queued() && !d.bad && matches!(d.kind.value(), 1 | 2 | 7 | 8 | 9 | 10) && s.queue.flags.enable_change,
        // BADSTAT_STUN (122; strand A 2): nothing queued and m_bEnableChange (the end clip).
        StateData::BadstatStun(_) => s.queue.nothing_queued() && s.queue.flags.enable_change,
        #[allow(unreachable_patterns)]
        _ => false,
    }
}

/// `CanLaunch` / `TryLaunch` (slots 47 / 48, 0x14142B3F0 / 0x14143EAE0; strand A 4): the jump
/// button during a combo, base form: an unused stock (Rising Spiral / Groundbreaker), a landed
/// hit, a non-finisher step, the same windows as an attack request. Each stock once per chain.
pub(crate) fn try_launch(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> bool {
    let s = &mut *cx.pawn;
    let Some(cur) = state.attack_mut() else { return false };
    if s.combat.command_cooldown > 0.0 || !s.queue.nothing_queued() {
        return false;
    }
    if s.status.abilities.has(crate::enums::ETresAbilityKind::ComboMaster) {
        s.combat.hit_flag = true;
    }
    if s.combat.stocks.0 != 1 && s.combat.stocks.1 != 1 {
        return false;
    }
    if cur.next_queued || !cur.swing_started || !cur.enable_input || cur.finish || !s.combat.hit_flag {
        return false;
    }
    s.combat.lockon.attack_target = target;
    cur.next_queued = true;
    let next = Attack { combo: 1, air: cur.air, target, reach: cur.reach, launch: true, ..Attack::RESET };
    s.combat.combo = 1;
    queue_attack(s, next, true);
    true
}

/// The attack a traversal state forces (TryAttack cases 7, 0x14 - 0x17): (forced kind, PushState
/// instead of SetState, the wall-run class, a yaw to turn to).
fn traversal_attack(state: &StateData) -> Option<(u8, bool, Option<u8>, Option<f32>)> {
    match state {
        StateData::WallRun(w) => Some((0, false, Some(w.class()), None)),
        // 9 HOPPING / 10 SLOPESLIDE (decompiled C of 0x141437330): SetState(attack); the request bit.
        StateData::Hopping(_) | StateData::SlopeSlide(_) => Some((0, false, None, None)),
        StateData::SuperSlide(_) | StateData::RailSlide(_) => Some((0x36, false, None, None)),
        StateData::WallKick(_) => Some((0x36, true, None, None)),
        StateData::PoleTurn(p) => match p.swing() {
            Some(yaw) => Some((0x39, false, None, Some(yaw))),
            None => Some((0x38, false, None, None)),
        },
        _ => None,
    }
}

/// `TryAttack(target, 0)` (slot 45 0x141437330), spec 3.3. Returns whether the request was taken.
pub(crate) fn try_attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> bool {
    // (XSI) The XS abilities' forced attacks (abilities/xs): Diving Strike out of a fall, Sonic Dash's launch.
    let xs = crate::abilities::xs::forced_attack(state, cx);
    let s = &mut *cx.pawn;
    if !can_attack(state, s) {
        return false;
    }
    // The forced kinds and the just-guard bit of the counter windows (spec 5.6.2).
    let forced = crate::combat::guard::forced_attack(state.id(), s);
    let (target, forced_kind, just_guard, push) = match forced {
        Some(o) => (if o.target != TargetId::NONE { o.target } else { target }, o.kind, o.just_guard, o.push),
        None => (target, 0, false, false),
    };
    // controller.slot731(target): the attack target, which pins the current target (strand D 7.6).
    s.combat.lockon.attack_target = target;
    if let Some(cur) = state.attack_mut() {
        // The combo step: queued behind the running attack.
        cur.next_queued = true;
        // (XSI) Sonic Dash's launch out of the dash (kind 0xE0 -> 0xE1).
        let (target, forced_kind) = match xs {
            Some((kind, t)) if cur.kind == crate::abilities::xs::kind::SONIC_DASH && kind == crate::abilities::xs::kind::SONIC_LAUNCH => (t, kind),
            _ => (target, 0),
        };
        let next = Attack { combo: cur.combo + 1, air: cur.air, target, reach: cur.reach, forced_kind, ..Attack::RESET };
        s.combat.combo = next.combo;
        queue_attack(s, next, true);
    } else if let StateData::FireMagic(m) = state {
        // (magic implementer) 0x141437BEA: flag 0x10 on the cast; PushState; combo = Magic+0xB4 + 1;
        // bAir = flag 1. The attack starts when the cast chains (its TresEnableChange).
        let (combo, air) = m.attack_follows();
        let next = Attack { combo, air, target, ..Attack::RESET };
        s.combat.combo = combo;
        queue_attack(s, next, true);
    } else if let Some((forced, push, wall_run, yaw)) = traversal_attack(state) {
        // (combat implementer, 2026-10-12) The traversal states of TryAttack 0x141437330 (decompiled
        // C, cases 7, 0x14 - 0x17): SetState(attack) with the target and
        //   8 WALLRUN:     next+0x1EC |= 8, next+0x1E4 = WallRun+0xA8 (the wall-run attack: Decide rule 4);
        //   21 SUPER_SLIDE: forcedKind 0x36;
        //   22 RAIL_SLIDE:  forcedKind 0x36 for Sora (m_ChrUniqueID 1; the pawn vt+0x21C8 branch - kinds
        //                   0x11 / 0x37 with bAir - is not his);
        //   23 WALL_KICK:   only with nothing queued: PushState, forcedKind 0x36, WallKick+0xA7 = 1;
        //   24 POLE_TURN:   a bar swing (PoleTurn+0xB0 bit 0): forcedKind 0x39 and Turn(release
        //                   rotation, 1.0); a pole spin: forcedKind 0x38.
        // The flow kinds are refined by slot 124 at the head of PlayAttack (`flow_refine`: the
        // abilities SONIC_SLASH / SONIC_DOWN / TURN_CUTTER / SUMMERSALT).
        if push && !s.queue.nothing_queued() {
            return false;
        }
        if let StateData::WallKick(k) = state {
            k.chain_latch = true;
        }
        if let Some(yaw) = yaw {
            s.yaw = wrap_degrees(yaw);
        }
        //   9 HOPPING:     next+0x1EC |= 0x10 (PlayAttack: kind 0x10, bAir);
        //   10 SLOPESLIDE: next+0x1EC |= 0x20 (PlayAttack: kind 0x2F `M_1143`, or 0x10 when the target
        //                  is more than 30 cm above).
        let (hop, slope) = (matches!(state, StateData::Hopping(_)), matches!(state, StateData::SlopeSlide(_)));
        let next = Attack { combo: 0, air: !on_ground(s), target, forced_kind: forced, wall_run, hop, slope, ..Attack::RESET };
        s.combat.combo = 0;
        queue_attack(s, next, push);
    } else {
        // IDLE, RUN, JUMP, GUARD: SetState; RISKDODGE / AIR_SLIDE: PushState behind the state.
        // A new combo always starts at 0 (spec 3.3; the combo is not carried into or out of
        // Second Form: 10.7).
        // (XSI) Diving Strike / Sonic Dash's launch (abilities/xs), unless a counter is forced; Rage Raid (an answer
        // to a revenge, XS 2.11) before the guard's own counter (our rule: the equipped XS move wins).
        let (target, forced_kind) = match xs {
            Some((kind, t)) if forced_kind == 0 || kind == crate::abilities::xs::rage::RAGE_RAID => (t, kind),
            _ => (target, forced_kind),
        };
        let next = Attack { combo: 0, air: !on_ground(s), target, forced_kind, just_guard, ..Attack::RESET };
        s.combat.combo = 0;
        queue_attack(s, next, push);
    }
    true
}

impl PlayerState for Attack {
    const ID: StateId = StateId::Attack;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// `Begin` (0x141333210), spec 5; `PreDecide` (0x14134C420) and `Decide`, spec 4.
    fn begin(&mut self, cx: &mut Ctx) {
        // (fixes pass 1) Begin 0x141333210, first calls: pawn slot 616 (1, 1, 1.0) = Draw(weapon)
        // and pawn+0x6E4 = 1 (the stance value: last pose BATTLE). They were missing: the first
        // swing was made with an empty hand until a hit raised the battle flag.
        crate::actions::draw_for_attack(cx, self.combo == 0);
        cx.pawn.actions.last_pose = 1;
        // The counters the previous attack of the chain passed on (its End: strand A 6.3), then
        // +0x130 = +0x12C, +0x134 = +0x138: the values BEFORE this attack's own windows (A 6.4).
        let passed = std::mem::take(&mut cx.pawn.combat.pass_on);
        if self.combo > 0 {
            (self.counters[0], self.counters[3]) = passed;
        }
        self.counters[1] = self.counters[0];
        self.counters[2] = self.counters[3];
        cx.pawn.combat.carry = (0, 0);
        // A target that is gone or not targetable: the controller's current target.
        if self.target_info(cx).is_none() {
            self.target = cx.pawn.combat.lockon.current;
        }
        cx.pawn.cmc.stop_movement();
        // PreDecide.
        (self.target_on_ground, self.dist, self.dz) = (true, 0.0, 0.0);
        self.target_metrics(cx);
        self.air = true;
        let (hit, distance, _) = ground_probe(cx.pawn, cx.world, P.decide_ground_probe_range);
        if hit && distance <= P.decide_ground_probe_max {
            self.air = false;
        }
        let pawn = &mut *cx.pawn;
        self.num_enemies = pawn.combat.lockon.crowd;
        let abilities = combo_abilities(pawn);
        let (max, fin) = (max_combo(pawn), finishers(pawn));
        let input = DecideIn {
            combo: self.combo,
            reach: self.reach,
            last_kind: pawn.combat.last_kind,
            dist: self.dist,
            dz: self.dz,
            target_high: self.target_high,
            target_on_ground: self.target_on_ground,
            air: self.air,
            num_enemies: self.num_enemies,
            launch: self.launch,
            // Begin 0x1413333BC (decompiled C): has(CHARGE_BERSERK 178) && no form
            // (TresCmdData[form_cmd].Param1 == 0) && the MP charge runs (pawn vt+0x17E0):
            // state+0x234 bit 21, the endless combo: every step is a normal step (spec 4.2).
            endless: pawn.status.abilities.has(crate::enums::ETresAbilityKind::ChargeBerserk) && pawn.combat.form.cmd == 0 && pawn.status.mp_charging,
            max_combo: max,
            finishers: fin,
            finish_count: pawn.combat.finish_count,
            stocks: pawn.combat.stocks,
        };
        self.endless = input.endless;
        let mut d = if self.limit { crate::combat::decide::decide_limit(&input, &abilities) } else { decide(&input, &abilities) };
        // Rule 4 (0x14134AF5D, decompiled C; spec 4.2): the wall-run request (state+0x1EC bit 3) with
        // a keyblade (pawn vt+0x28B0: taken as true for the Kingdom Key): the kind by where the
        // target is. side (+0x1C8, TargetMetrics 0x141330D30) from the wall-run class (+0x1E4), his
        // forward f and the direction t to the target: class 0: |f . t| >= 0.643 -> 0, else 1 or 2
        // by the sign of f x t; class 1: f . t < 0 -> 2 else 1; class 2: f . t >= 0 -> 2 else 1.
        if let (Some(class), false) = (self.wall_run, self.limit) {
            let side = match self.target_info(cx) {
                Some(t) => {
                    let a = cx.pawn.yaw.to_radians();
                    let (f, to) = (glam::Vec2::new(a.cos(), a.sin()), (t.center - cx.pawn.cmc.location).truncate().normalize_or_zero());
                    let dot = f.dot(to);
                    match class {
                        0 if dot.abs() >= 0.643 => 0,
                        0 => (f.perp_dot(to) >= 0.0) as u8 + 1,
                        1 => (dot < 0.0) as u8 + 1,
                        _ => (dot >= 0.0) as u8 + 1,
                    }
                }
                None => 0,
            };
            let (kind, dir_z) = if self.dz > 400.0 && self.dist <= 400.0 {
                (0x31, None)
            } else if self.dz < -250.0 && self.dist <= 400.0 {
                (0x34, None)
            } else {
                match side {
                    1 => (0x32, None),
                    2 => (0x33, None),
                    3 => (0x34, Some(-1000.0)),
                    _ => (0x31, Some(1000.0)),
                }
            };
            d = crate::combat::decide::Decision { kind, air: true, finish: false, finish_end: false, fixed_dir_z: dir_z, finish_count: cx.pawn.combat.finish_count, stocks: cx.pawn.combat.stocks, ..d };
        }
        // (party stream D) Rule 7 (0x14134ACA0: `if state+0x1EC & 0x80: finish flags; kind = 3F..43 by
        // pawn+0x1464`): the unison / fusion request of the roll one step before (the bit is copied
        // into the pushed attack while pawn+0x1464 is set: TryAttack 0x141437ADA); then the roll of
        // this step (`if combo + 1 == max - 1: pawn.slot1304()`). combat/friend.rs.
        if !self.launch && !self.limit && !cx.pawn.combat.sneeze_next && (self.combo > 0 || cx.pawn.combat.friend.leader_unison > 0.0) && cx.pawn.combat.friend.unison != 0 {
            // (D3) or on a friend leader's combo (the latch of FriendState::roll_leader_unison).
            cx.pawn.combat.friend.leader_unison = 0.0;
            let kind = crate::combat::friend::unison_attack_kind(cx.pawn.combat.friend.unison);
            d = crate::combat::decide::Decision { kind, finish: true, finish_end: true, finish_count: cx.pawn.combat.finish_count + 1, fixed_dir_z: None, ..d };
        } else if !self.launch && !self.limit && self.combo + 1 == max - 1 {
            crate::combat::friend::unison_roll(cx);
        }
        let pawn = &mut *cx.pawn;
        // Rule 6 (0x14134B0E5, decompiled C; spec 4.2): the controller's sneeze flag (+0x920: the
        // bad status SNEEZE is on him, spec 9.9.5) turns the attack into a sneeze: kind 0x1E with the
        // finish flags (state+0x140 / +0x141, the pawn's finish-end bit) and one more finisher
        // counted. After the launch rule, never in a ComboLimit instance (spec 10.5).
        // FOURTH PASS: controller+0x920 is not "the status is on him": it is latched by PlayAttack's
        // tail (`sneeze_latch`) and by the cast's (0x1413794B0), cleared when the chain ends
        // (pawn slot 981 -> 0x14140DF30(ctrl, 0)).
        if !self.launch && !self.limit && pawn.combat.sneeze_next {
            d = crate::combat::decide::Decision { kind: 0x1E, finish: true, finish_end: true, finish_count: pawn.combat.finish_count + 1, fixed_dir_z: None, ..d };
        }
        // (XSI) The XS ground finishers (abilities/xs/finishers.rs): our choice after KH3's decision.
        d.kind = crate::abilities::xs::finishers::refine(cx, d.kind, d.finish, d.air, self.limit, self.target);
        let pawn = &mut *cx.pawn;
        // "if forcedKind(+0x1E8) != 0: ... kind = forcedKind" (spec 4.2, rule 29).
        if self.forced_kind != 0 {
            // (magic implementer) The head of PlayAttack (0x1413378B0 line "if +0x1E8 != 0"): slot
            // 124 refines a flow kind, then the ordinary air kinds it may fall back to take their
            // ability variants.
            self.flow_refine(pawn);
            let has = |a| pawn.status.abilities.has(a);
            use crate::enums::ETresAbilityKind as Ab;
            self.forced_kind = match self.forced_kind {
                0x0F if has(Ab::AerialDive) => 0x20,
                0x10 if has(Ab::QuickShaft) => 0x18,
                0x11 if has(Ab::FlashStep) => 0x19,
                0x12 if has(Ab::RadialCut) => 0x1A,
                k => k,
            };
            d.kind = self.forced_kind;
        }
        (self.kind, self.reach, self.air, self.combo, self.finish, self.finish_end) = (d.kind, d.reach, d.air, d.combo, d.finish, d.finish_end);
        if let Some(z) = d.fixed_dir_z {
            (self.fixed_dir_z, self.dir_z) = (true, z);
        }
        pawn.combat.finish_count = d.finish_count;
        pawn.combat.stocks = d.stocks;
        pawn.combat.finish_end |= d.finish_end;
        // Final Blow (Begin 0x141333210: "if state+0x234 & 0x80000: kind = 0x22, warp start"): the
        // previous attack of the chain handed the armed chase over (its End, slot 113). PlayAttack
        // then only starts the warp countdown (slot 117 0x14134AA80, 0.2 s): the clip is played
        // when it runs out.
        if std::mem::take(&mut pawn.combat.chase_next) {
            self.kind = 0x22;
            (self.warp_start, self.warp_delay) = (true, P.chase_warp_delay);
            (pawn.combat.chase_armed, pawn.combat.revenge) = (false, TargetId::NONE);
            pawn.combat.chases += 1;
            crate::combat::situation::clear_reaction(cx, P.chase_command as i32);
            self.sync_ctx(cx.pawn);
            self.sub = 1;
            return;
        }
        self.play_attack(cx);
        // (audio implementer) SetVoiceGroupNo after the animation started: the kind's number.
        cx.set_voice_group_no(crate::host::audio::attack_voice_group_no(self.kind as i32));
        // (combat implementer) The sneeze (kind 0x1E): voice group `sora_sneeze` unit 1 (audio spec
        // 5.2 "direct plays from code"): the clip's voice notify finds it under number 1.
        if self.kind == 0x1E {
            cx.set_voice_group_no(1);
        }
        self.sub = 1;
    }

    /// `Tick` (0x141341340), spec 5.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        // Slot 129 (0x141331AF0; spec 5.6.3): the recoil hand-off of a repelled swing.
        if recoil(cx) {
            return;
        }
        // Slot 130: a target that stopped being targetable is dropped.
        if self.target != TargetId::NONE && self.target_info(cx).is_none() {
            self.target = TargetId::NONE;
        }
        // The warp countdown of the chase attack (slot 117 0x14134AA80, decompiled C): while it
        // runs nothing else of the Tick does; at 0 the teleport (slot 136 0x141337320 with 150 /
        // 100: 150 cm short of the target's lock point on the line from him, 100 cm below it,
        // each leg swept against the level), then PlayAttack (`ChaseAttack`, clip 1290).
        if self.warp_delay > 0.0 {
            self.warp_delay -= dt;
            if self.warp_delay > 0.0 {
                return;
            }
            self.warp_delay = 0.0;
            if std::mem::take(&mut self.warp_start) {
                if let Some(t) = self.target_info(cx) {
                    let own = cx.pawn.cmc.location;
                    let flat = (t.center - own) * Vec3::new(1.0, 1.0, 0.0);
                    let dir = flat.normalize_or_zero();
                    let beside = t.center - dir * P.chase_warp_distance;
                    let to = beside - Vec3::Z * P.chase_warp_drop;
                    let p = *cx.pawn.params();
                    let (r, hh) = (p.capsule_radius, p.capsule_half_height);
                    let centre = Vec3::Z * p.capsule_center_z;
                    // Leg 1: from the target's point out to the side point; leg 2: down.
                    let side = match cx.world.sweep_capsule(t.center, beside, r, r) {
                        Some(hit) if !hit.start_penetrating => t.center + (beside - t.center) * hit.time.clamp(0.0, 1.0),
                        _ => beside,
                    };
                    let down = Vec3::new(side.x, side.y, to.z);
                    let end = match cx.world.sweep_capsule(side + centre, down + centre, r, hh) {
                        Some(hit) if !hit.start_penetrating => side + (down - side) * hit.time.clamp(0.0, 1.0),
                        Some(_) => side,
                        None => down,
                    };
                    cx.pawn.cmc.location = end;
                    cx.pawn.cmc.stop_movement();
                    if dir != Vec3::ZERO {
                        cx.pawn.yaw = wrap_degrees(dir.y.atan2(dir.x).to_degrees());
                    }
                }
            }
            self.play_attack(cx);
            return;
        }
        self.facing(cx, dt);
        self.traction(cx, dt);
        // (XSI) The XS abilities' attacks: the dive's homing, the dash's timers (abilities/xs).
        crate::abilities::xs::attack_tick(cx, self.kind, dt);
        if let Some(name) = cx.pawn.combat.pending_shot.take() {
            // pawn vt+0x1450(name, location, location, forward, team, 0), then the shot gets the
            // attack's counters, finish-end and just-guard flags (shot vt+0x1130 / +0x1140 /
            // +0x1120 / +0x1180).
            self.sync_ctx(cx.pawn);
            let (mut origin, info) = (crate::combat::hit::shot_origin(cx), cx.pawn.combat.attack_ctx);
            if name == "SHOCKWAVE_S" {
                // 0x141344580: location += forward x 100.
                let a = cx.pawn.yaw.to_radians();
                origin += Vec3::new(a.cos(), a.sin(), 0.0) * 100.0;
            }
            // (party stream D6b) The team finishers' actors are placed by their spawn rule (sheet
            // sora-combat-shots: slot 137 0x14134CCE0 / 138 0x14134C710); `origin` (vt+0x1410) is
            // where their SpawnFireEffects plays.
            crate::combat::hit::spawn_shot(cx, name, origin, info);
        }
        // Sonic Cleave's hit (on-contact 0x14134208A, decompiled C): kind 0x37 with combo 0: the
        // first landed hit sets both shot bits and calls the shot handler.
        if self.kind == 0x37 && self.combo == 0 && cx.pawn.combat.swing_hit && self.shots == 0 {
            (self.dive_landed, self.shot_called) = (true, true);
            self.shot_handler(cx.pawn);
        }
        let s = &mut *cx.pawn;
        // Slot 128's tail (0x14134D510): the hover of an air attack ends at TresEnableChange.
        if s.queue.flags.enable_change && self.hover {
            self.hover = false;
            let p = *s.params();
            s.cmc.gravity_scale = p.gravity_scale;
            s.cmc.gravity_off = false;
            if s.cmc.is_flying() {
                s.cmc.set_falling();
            }
        }
        if s.anim.ended || (s.queue.flags.enable_change && self.next_queued) {
            s.queue.do_chain();
            return;
        }
        if self.swing_started {
            // Strand A 6.1: TresEnableChange also enables input; the cancel bit (+0x234 bit 7)
            // rises 0.1333 s after the first hit window, or at TresEnableChange (the combo's last
            // finisher: only at TresEnableChange).
            if s.queue.flags.enable_change {
                self.enable_input = true;
            }
            if !self.cancel {
                if !self.finish_end {
                    self.cancel_timer += dt;
                    if self.cancel_timer > P.swim_exit_delay {
                        self.cancel = true;
                    }
                }
                if s.queue.flags.enable_change {
                    self.cancel = true;
                }
            }
        }
        self.sync_ctx(s);
    }

    /// `End` (0x141335120): its call list (spec 5), unconditionally.
    fn end(&mut self, cx: &mut Ctx) {
        // (XSI) The XS abilities' attacks (abilities/xs).
        crate::abilities::xs::attack_end(cx, self.kind);
        // `if ctrl && kind in {0x28..0x2B, 0x36..0x39, 0x3F..0x43, 0xD0}: 0x14140DF20(ctrl, 0, 0, 0, 0)`.
        if matches!(self.kind, 0x28..=0x2B | 0x36..=0x39) {
            crate::combat::camera::off(cx);
        }
        // Slot 113 at End: the chase arms (or stays armed) while the revenge pawn is in a blow reaction;
        // slot 141(0): the reaction command goes when the attack ends.
        if crate::combat::hit::chase_ready(cx, cx.pawn.combat.revenge) {
            cx.pawn.combat.chase_armed = true;
        }
        if cx.pawn.combat.revenge != TargetId::NONE {
            crate::combat::situation::clear_reaction(cx, P.chase_command as i32);
        }
        let s = &mut *cx.pawn;
        let p = *s.params();
        let landed_hit = s.combat.swing_hit;
        s.cmc.max_walk_speed = p.speed_run;
        s.cmc.gravity_off = false;
        s.cmc.direct_xy = None;
        s.cmc.direct_z = None;
        if on_ground(s) {
            // SetMovementMode(Walking, bStop): Velocity = 0 (an air-mode attack that ended on the
            // floor: Falling with the same stop; the movement lands him again next tick).
            s.cmc.stop_movement();
        } else {
            // Falling, the velocity kept.
            if s.cmc.is_flying() {
                s.cmc.set_falling();
            }
            if landed_hit {
                self.air_hold = 0.0;
            } else {
                s.combat.air_slide_cooldown = 0.6;
                s.combat.air_guard_cooldown = 0.3333;
            }
        }
        s.cmc.gravity_scale = p.gravity_scale;
        s.root_scale = (1.0, 1.0);
        // The counters pass on only if this attack landed a hit.
        let counters = if landed_hit { (self.counters[0], self.counters[3]) } else { (0, 0) };
        match s.queue.queued_id() {
            // (magic implementer) FIREMAGIC (18) takes the same two counters (Magic+0xB8 / +0xC4).
            Some(StateId::Attack) | Some(StateId::ComboLimit) | Some(StateId::Firemagic) => {
                s.combat.pass_on = counters;
                // "if slot113(): next+0x234 |= 0x80000" (End 0x141335120; spec 5.4, 5.6.4): the armed
                // chase goes to the next attack of the chain (the caller re-tests the revenge
                // pawn's reaction: `chase_handover` below).
                s.combat.chase_next = s.combat.chase_armed;
            }
            _ => {
                // Pawn slot 981 (0x141432780): the chain is over, the combo memory is reset.
                let abilities = combo_abilities(s);
                let combat = &mut s.combat;
                (combat.finish_count, combat.combo, combat.hit_count, combat.hit_flag, combat.finish_end, combat.last_kind) = (0, 0, 0, false, false, 0);
                // ... and 0x14140DF30(ctrl, 0): the sneeze latch (controller+0x920) and the HUD's "Sneeze".
                combat.sneeze_next = false;
                // (party stream D) End 0x14134...: `if queued id != 12: pawn.slot1303()` (combat spec
                // line 816): the unison request is cleared when the chain ends.
                combat.friend.unison = 0;
                combat.stocks = (abilities.slash_upper, abilities.air_roll_beat);
                if self.air_hold > 0.0 {
                    // A whiffed air attack without a follow-up: the command cool-down, and the
                    // attack target record is cleared.
                    combat.command_cooldown = 0.6;
                    combat.lockon.attack_target = TargetId::NONE;
                }
                combat.carry = if landed_hit && !(self.air_hold > 0.0) { (self.counters[0], self.counters[3]) } else { (0, 0) };
                (combat.revenge, combat.chase_armed, combat.chase_next) = (TargetId::NONE, false, false);
            }
        }
        s.combat.attack_ctx = AttackCtx::default();
        s.combat.recoil = None;
        s.combat.map_reflect = false;
        s.yaw = wrap_degrees(s.yaw);
    }

    /// The notifies of spec 2: the hit window's begin (handler 0x1413459E0), `TresEnableInput`,
    /// the traction window, `TresFaceAt`. (`TresEnableChange` is the host's `m_bEnableChange`.)
    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        for event in &notifies.events {
            match (event.class(), event.phase) {
                (NotifyClass::TresAnimNotifyStateAttack, NotifyPhase::Begin) => {
                    self.swing_started = true;
                    self.keep_face = false;
                    // Slot 115 (0x141332790; strand A 6.4): +0x12C and +0x138 += max(m_ComboCountUpParam,
                    // 1). +0x12C only when pawn vt+0x1D20(row is magic) says so (0x1405AD380, verdict
                    // same): a physical row not while the MP charge runs with CHARGE_BERSERK (178), a
                    // magic row not with ENDLESS_MAGIC (82): the damage step of the combo boosts
                    // then stands still.
                    let magic_row = matches!(event.def().data, crate::anim::NotifyData::Attack { row, .. } if crate::combat::tables::attack_row(row).is_some_and(|(_, r)| r.magic));
                    let status = &cx.pawn.status;
                    use crate::enums::ETresAbilityKind as Ab;
                    let frozen = if magic_row { status.abilities.has(Ab::EndlessMagic) } else { status.abilities.has(Ab::ChargeBerserk) && status.mp_charging };
                    if !frozen {
                        self.counters[0] += 1;
                    }
                    self.counters[3] += 1;
                }
                (NotifyClass::TresAnimNotifyEnableInput, NotifyPhase::Instant) => self.enable_input = true,
                (NotifyClass::TresAnimNotifyStateTraction, NotifyPhase::Begin) => self.enable_traction = true,
                (NotifyClass::TresAnimNotifyStateTraction, NotifyPhase::End) => self.enable_traction = false,
                (NotifyClass::TresAnimNotifyFaceAt, NotifyPhase::Instant) => self.face_at = true,
                // `TresAnimNotify_Shot` (0x140561B70 -> anim interface vt+0x228): the shot handler.
                (NotifyClass::TresAnimNotifyShot, NotifyPhase::Instant) => self.shot_handler(cx.pawn),
                _ => {}
            }
        }
        self.sync_ctx(cx.pawn);
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// For hosts and tests: what the running attack is (`Sora::attack`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackView {
    pub kind: u8,
    pub combo: i32,
    pub air: bool,
    pub target: TargetId,
    pub finish: bool,
    pub finish_end: bool,
    pub swing_started: bool,
    pub enable_input: bool,
    pub enable_traction: bool,
    pub next_queued: bool,
    pub dist: f32,
    pub dz: f32,
    pub reach: u8,
    /// state+0x234 bit 15 (rush suppression: the bodies touch and the target asks for it).
    pub rush: bool,
}

pub(crate) fn view(state: &StateData) -> Option<AttackView> {
    match state.attack() {
        Some(a) => Some(AttackView { kind: a.kind, combo: a.combo, air: a.air, target: a.target, finish: a.finish, finish_end: a.finish_end, swing_started: a.swing_started, enable_input: a.enable_input, enable_traction: a.enable_traction, next_queued: a.next_queued, dist: a.dist, dz: a.dz, reach: a.reach, rush: a.rush }),
        None => None,
    }
}

/// Slot 129 (0x141331AF0; spec 5.6.3): a swing that was repelled (the on-contact handler left a
/// recoil record: attacker result REFLECT / GUARD_REFLECT / GUARD) is replaced by the weapon
/// recoil. GUARD_REFLECT first tries the guard reaction (iface vt+0x450 the counter-allowed gate,
/// then vt+0x460 0x141444C30: NOT READ, the actions side's guard-hit entry; it is taken as
/// refused here, and the record is dropped as the code's `if` without an `else` does).
pub(super) fn recoil(cx: &mut Ctx) -> bool {
    let Some((result, causer, position)) = cx.pawn.combat.recoil.take() else { return false };
    if result == crate::host::ReactionResult::GuardReflect {
        // (ACTIONS IMPLEMENTER) The guard reaction (iface vt+0x460 0x141444C30, read in the
        // decompiled C; the counter-allowed gate vt+0x450 passes for an attack state): the swing
        // is replaced by a successful guard-hit entry with the 1.0 s counter window; GUARD_REGENE
        // (74): pawn.vt+0x1878(0, 3, 0, 0, 0); DoChain.
        // (combat implementer) The gate itself (slot 138 0x141429CF0): `combat::guard::reaction_allowed`.
        if crate::combat::guard::reaction_allowed(StateId::Attack, cx.pawn) && super::guard::guard_reaction(cx, causer, position) {
            let mp = crate::abilities::passive::block_replenisher_mp(&cx.pawn.status);
            if mp > 0 {
                cx.pawn.status.recover(0, mp, 0, 0);
            }
            cx.pawn.queue.do_chain();
            return true;
        }
        return false;
    }
    super::reflect::request(cx, causer, position, Vec3::ZERO, false);
    cx.pawn.queue.do_chain();
    true
}
