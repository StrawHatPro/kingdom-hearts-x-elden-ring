//! `TresPlayerState_StyleFinish` (id 33): the finish commands of Second Form (combat spec 10.6,
//! 10.9: Reset 0x1413D0CA0, Begin 0x1413CBF70, Tick 0x1413D4550, End 0x1413CEBD0; the three
//! per-command ticks Stun Impact 0x1413E2630, Sonic Rave 0x1413E1FC0, Last Arcanum 0x1413E1800
//! were read whole at instruction level at the merge). A subclass of the attack state: it uses
//! its facing (slot 126), traction (slot 116 through the delegate of slot 128), `PreDecide` and
//! hit windows; here it holds an `Attack` for those.
//!
//! What 10.9 left open, as settled by the fourth pass (2026-10-12; spec 10.9 items 1 - 5):
//! - `count` (state+0x128) in Last Arcanum: the StyleFinish class overrides the `TresAtk` begin
//!   handler (anim-interface slot 64, 0x1413DD7D0 [I]): the Attack state's handler, then
//!   `if pawn.commandInAction() == 0xAD: count += 1`: one per hit window that opens, in every
//!   sub-step (`M_1427` has seven windows: the count is 6 at 1.4333 s);
//! - `finishEnd` (state+0x141) in Last Arcanum: NO writer besides the clear of sub-step 0 (byte
//!   scans of 0x141330000 - 0x141350000 and 0x1413C8000 - 0x1413E6000: the only writers are
//!   `Decide` and its form variants, which the finish commands never call): it stays 0, so the
//!   extra removed at the end is asked for as `LMEX_BREAK` and the hits never carry `finish`;
//! - Last Arcanum's root lengths: state+0x1D8 is read only by the root-scale channel of the
//!   traction update (0x14134A4B2 / 0x14134A56F), which is off here: carried, not used;
//! - `pawn.vt+0x1048(1, 0, 0)` = `ClientPlayForceFeedback(effect table[1], not looping)`
//!   (0x1405D33C0 -> controller interface slot 4 0x141444270 -> 0x144C73F00):
//!   `EventKind::ForceFeedback`; `pawn.vt+0x23C8()` (0x1405A8610) and the equipment call
//!   `(pawn+0x598)+0x200 .vt+0x1A8(-1, 1)` are `EndEffect` of the pawn's and the weapons' attached
//!   effects (vfx spec 4.2): the effect side's (no group seam in the core yet);
//! - the attack camera (0x14140DF20) of Sonic Rave: `combat/camera.rs`, mode 3;
//! - the stick-steering block of `Tick` (state+0x16C) has no writer in the three Kingdom Key
//!   commands (only slot 144 0x1413DED90 sets it): dead here.
//!   The post effect and the HUD hiding are the host's to derive from `EventKind::TimeDilation` /
//!   the form events.
//!
//! (weapons P3) The other forms' finish commands (FINISH_BZ ...) run here too: their body is the form package's
//! (`combat::forms` `finishes` / `finish_begin` / `finish_tick`), after the begin event common to all; a finish command
//! without a body ends at once (DoFinish), and `End` ends the form.
//!
//! (F-SECOND) The fourth per-command tick of the class, slot 155 0x1413E2810 (Ancient Light, `LM_UNIONDISCODE` 0xAE: Second
//! Form S's finish level 3) with its helpers slot 164 0x1413D12A0 / 165 0x1413E50D0 (the rise), read whole in decompiled C;
//! and the Attack state's slot 115 in the TresAtk begin handler (+0x12C, which slot 155 counts).


use super::attack::Attack;
use super::common::{ground_probe, on_ground};
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, NotifyClass, NotifyPhase, Notifies, PlayArgs};
use crate::combat::{form, situation};
use crate::host::TargetId;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

const LM_STUNIMPACT: i32 = 0xAB;
const LM_SONICRAVE: i32 = 0xAC;
const LM_LASTARCANUM: i32 = 0xAD;
const LMEX_RAVE: i32 = 0xAF;
const LMEX_FINISH: i32 = 0xB0;
const LMEX_BREAK: i32 = 0xB1;
const LMEX_THEEND: i32 = 0xB2;

/// Sonic Rave's start clip by the code as read (see `sonic_rave` sub-step 0): always `M_1424`.
const SONIC_RAVE_START_BY_CODE: bool = true;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StyleFinish {
    /// The command (pawn+0xE2C at Begin): 0xAB / 0xAC / 0xAD.
    pub cmd: i32,
    /// The attack state's fields (sub-step, target, channels ...).
    pub a: Attack,
    /// +0x288: the extra command the executor stored (pawn slot 1346 0x14143EE20).
    pub pending: i32,
    /// +0x28C bit 0: an extra command is on show in the finish slot.
    extra_shown: bool,
    /// +0x128: dashes (Sonic Rave) / hit windows (Last Arcanum).
    pub count: i32,
    /// +0x1D8 as written by the command (10.9 item 3).
    pub root_len: f32,
    /// (F-HAMMER) m_pCatchPawn: the pawn a finish caught (Drill Punch's warhead), held until the finish ends.
    pub catch: TargetId,
    /// (F-HAMMER) The finish's accompany pawn (m_pClock of STYLEFINISH_DR): its root (world) and yaw, while it stands.
    pub accompany: Option<(glam::Vec3, f32)>,
    /// (F-SECOND) +0x290: where slot 164 lifts Sora to (Ancient Light's rise; Reset clears it).
    pub rise_to: glam::Vec3,
}

impl StyleFinish {
    pub fn new(cmd: i32) -> StyleFinish {
        StyleFinish { cmd, a: Attack::RESET, pending: 0, extra_shown: false, count: 0, root_len: 0.0, catch: TargetId::NONE, accompany: None, rise_to: glam::Vec3::ZERO /* (F-SECOND) */ }
    }

    /// `Play(name, blendIn, mode)`. (weapons P3) Through the held form's replacement anim set (combat/forms
    /// `replace_clip`); for the form packages' finish bodies too.
    pub(crate) fn play(&mut self, s: &mut Pawn, name: &str, blend_in: f32, mode: u8) {
        match AnimId::from_name(crate::combat::forms::clip(s.combat.form.cmd, name)) {
            Some(clip) => s.anim.play_with(clip, PlayArgs::mode(mode, blend_in)),
            None => s.note("a finish-command clip (M_1421 .. M_1434) is not in the generated animation table"),
        }
        self.a.swing_started = false;
    }

    /// `extra(k)` (slot 162 0x1413E3690).
    pub(crate) fn extra(&mut self, cx: &mut Ctx, kind: i32) {
        situation::set_extra(cx, kind);
        self.extra_shown = true;
    }

    /// `remove(k, b)` (slot 163).
    pub(crate) fn remove_extra(&mut self, cx: &mut Ctx) {
        situation::clear_extra(cx);
        self.extra_shown = false;
    }

    /// `PreDecide` (slot 121): the target metrics and the air test.
    pub(crate) fn pre_decide(&mut self, cx: &mut Ctx) {
        (self.a.target_on_ground, self.a.dist, self.a.dz) = (true, 0.0, 0.0);
        self.a.target_metrics(cx);
    }

    /// DASH_SETUP of Sonic Rave, also the per-step setup of Last Arcanum: the XY homing channel
    /// on, the root-scale channel off, z gain 3.0, the channel phases 0, air mode.
    pub(crate) fn channels(&mut self, cx: &mut Ctx, root_len: f32) {
        self.pre_decide(cx);
        let a = &mut self.a;
        (a.chan_xy, a.chan_root, a.z_gain) = (true, false, 3.0);
        (a.phase_xy, a.phase_root, a.phase_z) = (0, 0, 0);
        a.air_mode = true;
        self.root_len = root_len;
    }

    /// Sub 0 of Sonic Rave and Last Arcanum: +0x234 |= 4 | 0x10, GravityScale 0.1, Flying.
    pub(crate) fn hover(&mut self, s: &mut Pawn) {
        self.a.air_mode = true;
        self.a.hover = true;
        s.cmc.gravity_scale = crate::combat::tables::P.air_gravity_scale;
        s.cmc.set_flying();
    }

    /// FINISH: Falling, gravity on, GravityScale back to the class default, DoChain.
    pub(crate) fn finish(&mut self, s: &mut Pawn) -> bool {
        let p = *s.params();
        s.cmc.set_falling();
        s.cmc.gravity_off = false;
        s.cmc.gravity_scale = p.gravity_scale;
        s.queue.do_chain();
        true
    }

    /// Stun Impact, slot 152 (0x1413E2630).
    fn stun_impact(&mut self, cx: &mut Ctx) -> bool {
        if self.a.sub == 0 {
            self.a.face_at = true;
            let name = if on_ground(cx.pawn) { "M_1421" } else { "M_1422" };
            self.play(cx.pawn, name, -1.0, 2);
            self.a.sub = 1;
            return false;
        }
        if self.a.sub == 1 && self.a.swing_started {
            // The first hit window has opened: the world slow ends, and pawn.vt+0x1048(1, 0, 0) =
            // the rumble effect 1 of the game's table.
            form::world_slow(cx, false);
            cx.pawn.event(crate::host::EventKind::ForceFeedback { effect: 1, looping: false });
            self.a.sub = 2;
        }
        let s = &mut *cx.pawn;
        if self.a.sub == 2 && s.queue.flags.enable_change {
            let p = *s.params();
            s.cmc.gravity_off = false;
            s.cmc.gravity_scale = p.gravity_scale;
            self.a.sub = 0xC;
        }
        if s.anim.ended {
            // (No mode change: it ends by DoChain alone.)
            s.queue.do_chain();
            return true;
        }
        false
    }

    /// Sonic Rave, slot 153 (0x1413E1FC0).
    fn sonic_rave(&mut self, cx: &mut Ctx, dt: f32) -> bool {
        match self.a.sub {
            0 => {
                // THE ORDER AS READ AT INSTRUCTION LEVEL (2026-10-12): `SetMovementMode(5 Flying)`
                // at 0x1413E2569, then `OnGround(pawn)` (0x1405AD850: movement mode Walking) at
                // 0x1413E257A picks the name; the mode change is immediate (0x140594A00 writes
                // MovementMode at once), so the ground clip M_1423 is never chosen: Sonic Rave
                // always starts with M_1424. `SONIC_RAVE_START_BY_CODE = false` gives the other
                // reading (the mode he had when the tick began); a probe of the clip name on the
                // first frame of state 33 would confirm (docs/sora-runtime-probe.md, request list).
                let grounded = on_ground(cx.pawn) && !SONIC_RAVE_START_BY_CODE;
                self.hover(cx.pawn);
                self.a.face_at = true;
                let name = if grounded { "M_1423" } else { "M_1424" };
                self.play(cx.pawn, name, -1.0, 2);
                self.a.sub = 1;
            }
            1 => {
                if cx.pawn.anim.ended {
                    self.channels(cx, 1100.0);
                    self.a.stop_dist = 0.0;
                    self.play(cx.pawn, "M_1425", -1.0, 2);
                    (self.a.sub, self.count, self.pending) = (5, 1, 0);
                    self.extra(cx, LMEX_RAVE);
                    // 0x14140DF20(ctrl, 0, 0, null, 0), then (ctrl, 1, 3, target (+0x118), 0): the
                    // lock-on camera on the dash's target for as long as the dashes last.
                    crate::combat::camera::off(cx);
                    crate::combat::camera::request(cx, true, 3, self.a.target);
                    form::world_slow(cx, false);
                }
            }
            5 => {
                self.a.face_timer = 0.0;
                if cx.pawn.queue.flags.enable_change && self.pending != 0 {
                    // The extra command was used: the next dash.
                    cx.pawn.cmc.stop_movement();
                    self.count += 1;
                    self.a.face_at = true;
                    self.channels(cx, 1100.0);
                    cx.pawn.queue.flags.enable_change = false;
                    let pending = self.pending;
                    self.remove_extra(cx);
                    if pending == LMEX_RAVE {
                        self.play(cx.pawn, "M_1425", -1.0, 2);
                        self.extra(cx, if self.count >= 6 { LMEX_FINISH } else { LMEX_RAVE });
                        self.pending = 0;
                    } else {
                        // LMEX_FINISH (`pending` is not cleared on this path): the attack camera off first.
                        crate::combat::camera::off(cx);
                        self.root_len = 1200.0;
                        self.play(cx.pawn, "M_1426", -1.0, 2);
                        self.a.sub = 0xB;
                    }
                    // 0x1413E22AB: with a target, SetRootScale(2 x dxy / root length), once per
                    // dash, the un-led lock-on position, no clamp; vertical 1.0.
                    if let Some(t) = self.a.target_info(cx) {
                        let dxy = (t.center - cx.pawn.cmc.location).truncate().length();
                        cx.pawn.root_scale = (2.0 * dxy / self.root_len, 1.0);
                    }
                } else if cx.pawn.anim.ended {
                    crate::combat::camera::off(cx);
                    return self.finish(cx.pawn);
                } else {
                    self.traction_tick(cx, dt);
                }
            }
            _ => {
                if self.a.sub == 0xB {
                    self.a.face_timer = 0.0;
                }
                if cx.pawn.anim.ended {
                    if self.extra_shown {
                        self.remove_extra(cx);
                    }
                    return self.finish(cx.pawn);
                }
                self.traction_tick(cx, dt);
            }
        }
        false
    }

    /// Last Arcanum, slot 154 (0x1413E1800).
    fn last_arcanum(&mut self, cx: &mut Ctx, dt: f32) -> bool {
        match self.a.sub {
            0 => {
                self.hover(cx.pawn);
                self.a.swing_started = false;
                self.channels(cx, 700.0);
                self.a.face_at = true;
                self.a.stop_dist = 160.0;
                // Blend-in 0.0 and mode 0 (the other clips: blend -1, mode 2).
                self.play(cx.pawn, "M_1427", 0.0, 0);
                (self.count, self.a.finish_end, self.a.sub, self.pending) = (0, false, 1, 0);
            }
            1 => {
                if self.count == 1 {
                    form::world_slow(cx, false);
                }
                if self.count >= 6 {
                    self.pending = 0;
                    self.extra(cx, LMEX_BREAK);
                    self.a.sub = 5;
                }
            }
            5..=10 if cx.pawn.queue.flags.enable_change && self.pending != 0 => {
                cx.pawn.cmc.stop_movement();
                self.a.face_at = true;
                let sub = self.a.sub;
                self.channels(cx, self.root_len);
                cx.pawn.queue.flags.enable_change = false;
                self.remove_extra(cx);
                let (root_len, name, next, extra) = match sub {
                    5 => (200.0, "M_1428", 6, Some(LMEX_BREAK)),
                    6 => (190.0, "M_1429", 7, Some(LMEX_BREAK)),
                    7 => (180.0, "M_1430", 8, Some(LMEX_BREAK)),
                    8 => (160.0, "M_1431", 9, Some(LMEX_BREAK)),
                    9 => (150.0, "M_1432", 10, Some(LMEX_THEEND)),
                    _ => {
                        // `M_1434` (root length 330) when the ground probe (range 800) returns a
                        // distance > 50; no ground within 800 cm gives `M_1433`.
                        let (hit, distance, _) = ground_probe(cx.pawn, cx.world, 800.0);
                        if hit && distance > 50.0 { (330.0, "M_1434", 0xB, None) } else { (200.0, "M_1433", 0xB, None) }
                    }
                };
                self.root_len = root_len;
                self.play(cx.pawn, name, -1.0, 2);
                self.a.sub = next;
                if let Some(kind) = extra {
                    // (`finishEnd` is NOT set here or anywhere in Last Arcanum: see the file head.)
                    self.extra(cx, kind);
                }
                self.pending = 0;
            }
            _ => {}
        }
        // Every tick, every sub-step (tail 0x1413E1E9D).
        if !cx.pawn.anim.ended {
            self.traction_tick(cx, dt);
            return false;
        }
        if self.extra_shown {
            self.remove_extra(cx);
        }
        // (vfx implementer 3) the tail's `(pawn+0x598)+0x200 .vt+0x1A8(-1, 1); pawn.vt+0x23C8()`.
        form::end_finish_effects(cx);
        self.finish(cx.pawn)
    }

    /// (F-SECOND) Ancient Light, slot 155 (0x1413E2810, decompiled C, the whole function): Second Form S's level 3
    /// (`LM_UNIONDISCODE` 0xAE). The numbers are the sheet's (sora-form-second `finish.L2.*`).
    fn union_discode(&mut self, cx: &mut Ctx, dt: f32) -> bool {
        use crate::combat::forms::second;
        match self.a.sub {
            0 => {
                // SetMovementMode(5 Flying, 1) (pawn vt+0x17A0); Play("M_1435", blend -1, mode 2); m_bFaceAt; sub 1; then
                // slot 164(500): a floor nearer than 500 cm -> sub 3 (the rise first).
                cx.pawn.cmc.set_flying();
                self.play(cx.pawn, "M_1435", -1.0, 2);
                self.a.face_at = true;
                self.a.sub = 1;
                if self.rise_target(cx, second::rise_height()) {
                    self.a.sub = 3;
                }
                return false;
            }
            1..=3 => {
                // sub 3: slot 165 (the rise); done -> sub 1. Sub 3 then runs sub 1's code (0x1413E2868) every tick.
                if self.a.sub == 3 && self.rise(cx, dt) {
                    self.a.sub = 1;
                }
                // sub 1: the fifth hit window has begun (+0x12C > 4): WorldSlow(off), sub 2.
                if self.a.sub != 2 && self.a.counters[0] > second::hits_before_slow_end() {
                    form::world_slow(cx, false);
                    self.a.sub = 2;
                }
                // sub 2: m_bEnableChange: SetMovementMode(3 Falling, 1), gravity on (vt+0x1728(1)), GravityScale = the
                // class default (vt+0x1658), sub 0xC.
                if cx.pawn.queue.flags.enable_change {
                    let s = &mut *cx.pawn;
                    let p = *s.params();
                    s.cmc.set_falling();
                    s.cmc.gravity_off = false;
                    s.cmc.gravity_scale = p.gravity_scale;
                    self.a.sub = 0xC;
                }
            }
            0xC => {}
            _ => return false,
        }
        // AnimEnded (anim interface vt+200): DoChain (vt+0x270), done.
        if cx.pawn.anim.ended {
            cx.pawn.queue.do_chain();
            return true;
        }
        false
    }

    /// (F-SECOND) Slot 164 (0x1413D12A0, h): the ground probe 0x14063A650(pawn, 0, h + 300); a floor nearer than h, or none
    /// but the floor test (0x1405ADAA0: movement vt+0x9D8) standing: the target (+0x290) = the actor location raised by
    /// h - distance (no hit: by h), true; else false.
    fn rise_target(&mut self, cx: &Ctx, height: f32) -> bool {
        let (hit, distance, _) = ground_probe(cx.pawn, cx.world, height + crate::combat::forms::second::probe_extra());
        let below = if hit {
            if distance >= height {
                return false;
            }
            distance
        } else if cx.pawn.cmc.floor_under(cx.world, cx.pawn.params()) {
            0.0
        } else {
            return false;
        };
        self.rise_to = cx.pawn.cmc.location + glam::Vec3::Z * (height - below);
        true
    }

    /// (F-SECOND) Slot 165 (0x1413E50D0, dt): dz = target z - actor z; dz > 0 and dt x 500 x dz < dz: StopMovement and
    /// Velocity.Z = dt x 500 x dz, false; else StopMovement and SetActorLocation(target, sweep) (0x143EAC3D0), true.
    fn rise(&mut self, cx: &mut Ctx, dt: f32) -> bool {
        let dz = self.rise_to.z - cx.pawn.cmc.location.z;
        let v = dt * crate::combat::forms::second::rise_rate() * dz;
        cx.pawn.cmc.stop_movement();
        if dz > 0.0 && v < dz {
            cx.pawn.cmc.velocity.z = v;
            return false;
        }
        let p = *cx.pawn.params();
        cx.pawn.cmc.sweep_to(cx.world, &p, self.rise_to);
        true
    }

    /// Slot 128 (dt): the traction delegate (attack.rs, spec 6.3), then its tail: the hover ends
    /// at `TresEnableChange`.
    pub(crate) fn traction_tick(&mut self, cx: &mut Ctx, dt: f32) {
        self.a.traction(cx, dt);
        let s = &mut *cx.pawn;
        if s.queue.flags.enable_change && self.a.hover {
            self.a.hover = false;
            let p = *s.params();
            s.cmc.gravity_scale = p.gravity_scale;
            s.cmc.gravity_off = false;
            if s.cmc.is_flying() {
                s.cmc.set_falling();
            }
        }
    }
}

/// The executor's target 0x14141B981 for the extra kinds 0xAA, 0xAF .. 0xB2 (pawn slot 1346
/// 0x14143EE20): the kind is stored as "pending" when the current state is a StyleFinish.
pub(crate) fn extra_pressed(state: &mut StateData, kind: i32) -> bool {
    match state {
        // (F-SHIELD) ... and STYLEFINISH_SH (38, a subclass)
        StateData::StyleFinish(f) | StateData::StyleFinishSh(super::combo_sh::StyleFinishSh(f, _)) => {
            f.pending = kind;
            true
        }
        _ => false,
    }
}

impl PlayerState for StyleFinish {
    const ID: StateId = StateId::Stylefinish;

    fn sub_step(&self) -> u8 {
        self.a.sub
    }

    /// `Begin` (0x1413CBF70) with the command begin event (base slot 103 0x14150A1F0).
    fn begin(&mut self, cx: &mut Ctx) {
        // (weapons P3) a form package's finish body (combat/forms `finishes`) is a known command too; one without a body
        // ends at once (DoFinish), and the form with it (`End`).
        let body = crate::combat::forms::of_finish(self.cmd);
        if !matches!(self.cmd, LM_STUNIMPACT | LM_SONICRAVE | LM_LASTARCANUM) && body.is_none() {
            cx.pawn.queue.do_finish();
            return;
        }
        form::world_slow(cx, true);
        // (AUDIT-A ARROWGUN) the begin event's 0x14140DF80(ctrl, 1, 1.0, 1.0): the dark screen on (master light x MyDarkScreenRate, 1 s)
        form::dark_screen(cx, true);
        let s = &mut *cx.pawn;
        s.combat.form.bad_status_block = true;
        // (AUDIT-A SECOND) the begin event's pawn vt+0x2068(1) (0x14150A1F0; slot 1037 0x1405B9120): pawn+0x630 bit 2,
        // invincible through every finish command (the end event's pawn vt+0x23B8 0x1405A5E00 clears it: `End`).
        s.status.damage_invalid = true;
        // (AUDIT-B W-DATA) ... and FUN_1405B8B30(pawn+0x460, 8) (asm/text/141500000.s line 10008, `mov dl,8`): PPL_90 for every
        // finish command (W-CORE's open list: "the formchange finish's command-begin event"; StyleFinish Begin 0x1413CBF70
        // line 137 calls slot 103 for every finish kind it accepts)
        s.actions.push_level = Some(super::attack::PUSH_LEVEL_PPL_90);
        s.cmc.stop_movement();
        s.cmc.gravity_off = true;
        self.a.face_yaw = s.yaw;
        self.a.target = s.combat.lockon.current;
        s.combat.swing_hit = false;
        s.root_scale = (1.0, 1.0);
        self.a.sync_ctx(s);
        if let Some(m) = body {
            (m.finish_begin)(self, cx);
        }
    }

    /// `Tick` (0x1413D4550).
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        // The target: dropped when not targetable, then the controller's current target.
        if self.a.target != TargetId::NONE && self.a.target_info(cx).is_none() {
            self.a.target = TargetId::NONE;
        }
        if self.a.target == TargetId::NONE {
            self.a.target = cx.pawn.combat.lockon.current;
        }
        let done = match self.cmd {
            LM_STUNIMPACT => self.stun_impact(cx),
            LM_SONICRAVE => self.sonic_rave(cx, dt),
            LM_LASTARCANUM => self.last_arcanum(cx, dt),
            // (F-SECOND) slot 155: Ancient Light (Second Form S's level 3).
            crate::combat::forms::second::LM_UNIONDISCODE => self.union_discode(cx, dt),
            // (weapons P3) the form package's per-command tick (slots 143 .. 161).
            other => crate::combat::forms::of_finish(other).is_none_or(|m| (m.finish_tick)(self, cx, dt)),
        };
        if !done {
            self.a.facing(cx, dt);
            self.a.sync_ctx(cx.pawn);
        }
    }

    /// `End` (0x1413CEBD0) with the command end event (base slot 111 0x14150C440).
    fn end(&mut self, cx: &mut Ctx) {
        // (F-HAMMER) the caught pawn is given back (Drill Punch: an interrupted finish too).
        if self.catch != TargetId::NONE {
            cx.host.targets.script_hold(std::mem::replace(&mut self.catch, TargetId::NONE), None);
        }
        // (AUDIT-A STAFF) StyleFinishWA's End 0x1413CF310: its cleanup 0x1413CFE90 first (an interrupted FINISH_WA too)
        if self.cmd == crate::combat::forms::staff::FINISH_WA {
            super::combo_wa::finish_cleanup(cx);
        }
        // ctrl.slot859(0, 0): the form ends with the finish (unless the style is fixed).
        // (F-RAGE) the end event 0x14150C440 skips slot 859 for RISKCHARGE (0xB3): Rage Form goes on.
        if self.cmd != crate::combat::forms::rage::RISKCHARGE { form::finish_ended(cx); } else { crate::combat::forms::rage::riskcharge_ended(cx); } // (AUDIT-C) 0x1413CF044: the eyes at n == 1
        // The end event's controller part [I 0x14150C4E4 ...]: the attack camera off, then the
        // world slow off.
        crate::combat::camera::off(cx);
        form::world_slow(cx, false);
        // (AUDIT-A ARROWGUN) the end event's 0x14140DF80(ctrl, 0, 1.0, 1.0): the dark screen off (SetMasterLightCoefficientDefault, 1 s)
        form::dark_screen(cx, false);
        if self.extra_shown {
            self.remove_extra(cx);
        }
        // (vfx implementer 3) equipment vt+0x1A8(-1, 1); pawn.vt+0x23C8(): the attached effects of
        // the finish end (`form::end_finish_effects`).
        form::end_finish_effects(cx);
        let s = &mut *cx.pawn;
        let p = *s.params();
        if !on_ground(s) {
            s.cmc.set_falling();
        } else {
            s.cmc.stop_movement();
        }
        s.cmc.gravity_off = false;
        s.cmc.gravity_scale = p.gravity_scale;
        (s.cmc.direct_xy, s.cmc.direct_z) = (None, None);
        s.root_scale = (1.0, 1.0);
        s.combat.form.command_in_action = 0;
        s.combat.attack_ctx = Default::default();
        s.combat.recoil = None;
        // (AUDIT-A SECOND) the end event's first call, pawn vt+0x23B8 (0x1405A5E00): among its resets vt+0x2068 (the
        // invincible bit off) and the body-push level back to the base param's.
        s.status.damage_invalid = false;
        s.actions.push_level = None; // (AUDIT-B W-DATA) MyBodyColl vt+0x720(m_pBaseParam +0x98), 0x1405A5E00
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        for event in &notifies.events {
            match (event.class(), event.phase) {
                (NotifyClass::TresAnimNotifyStateAttack, NotifyPhase::Begin) => {
                    self.a.swing_started = true;
                    // (F-SECOND) the Attack state's handler first (0x1413459E0 -> slot 115 0x141332790): +0x12C / +0x138,
                    // the count Ancient Light (slot 155) waits on, in every finish command as KH3 counts (the golden trace was
                    // re-blessed for it on 2026-11-03 by the coordinator's decision: build/sora_tmp/weapons/F-SECOND/STATE.md).
                    self.a.count_window(cx.pawn, event);
                    // The class's own `TresAtk` begin handler (0x1413DD7D0 [I]): after the Attack
                    // state's, `if commandInAction == 0xAD: inc dword [state+128h]`.
                    if self.cmd == LM_LASTARCANUM {
                        self.count += 1;
                    }
                }
                (NotifyClass::TresAnimNotifyStateTraction, NotifyPhase::Begin) => self.a.enable_traction = true,
                (NotifyClass::TresAnimNotifyStateTraction, NotifyPhase::End) => self.a.enable_traction = false,
                (NotifyClass::TresAnimNotifyFaceAt, NotifyPhase::Instant) => self.a.face_at = true,
                _ => {}
            }
        }
        self.a.sync_ctx(cx.pawn);
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// For hosts and tests (`Sora::finish`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinishView {
    pub cmd: i32,
    pub sub: u8,
    pub count: i32,
    pub pending: i32,
    pub root_len: f32,
    /// (F-HAMMER) The finish's accompany pawn (the Clock): its root (world) and yaw.
    pub accompany: Option<(glam::Vec3, f32)>,
}

pub(crate) fn view(state: &StateData) -> Option<FinishView> {
    match state {
        StateData::StyleFinish(f) | StateData::StyleFinishSh(super::combo_sh::StyleFinishSh(f, _)) => Some(FinishView { cmd: f.cmd, sub: f.a.sub, count: f.count, pending: f.pending, root_len: f.root_len, accompany: f.accompany }),
        // (F-HAMMER) STYLEFINISH_DR
        StateData::StyleFinishDr(super::combo_hm::StyleFinishDr(f)) => Some(FinishView { cmd: f.cmd, sub: f.a.sub, count: f.count, pending: f.pending, root_len: f.root_len, accompany: f.accompany }),
        _ => None,
    }
}
