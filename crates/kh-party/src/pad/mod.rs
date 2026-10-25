//! (party P) PLAYABLE DONALD AND GOOFY: a member driven by the pad instead of the AI (stream P; state note
//! `build/sora_tmp/party/IMPL_P_STATE.md`). KH3 has no playable Donald / Goofy: everything here is THE PORT'S
//! DESIGN, built from the members' own kits (their anim sets, action definitions, MP rows), with Sora's controls
//! (docs/sora-movement-spec.md 2.1 - 2.4, kh-core `controller.rs` / `states/run.rs` / `states/jump.rs`) and the
//! user's movement decisions applied the way they are applied to Sora:
//! - dec013 continuous turning (no run-start replay while the stick is rolled round), the turn-around only for a
//!   flick to the opposite direction, the start lunge from standstill kept (RunStartN, the members' own 1D blend
//!   space 0007 / L / R / BL / BR with its root motion);
//! - dec014 release-to-cut jump height with the minimum jump height;
//! - dec015 the walk <-> run speed ramp (140 <-> 680 over 0.25 s);
//! - dec016 speed-matched walk / run animation: the members' own `WalkRunN` / `WalkRunB` blend space (walk cycle
//!   at 140, run cycle with its lean samples at 680) on one phase advancing at speed / stride
//!   ([`LocoPose`], drawn by the host);
//! all switched by the same `kh_core::Tuning` as Sora's (the host passes his).
//!
//! What their kits lack (left out, not invented): dodge roll, guard, air dodge, glide, flowmotion, shotlock and
//! forms: X does nothing for them; there is no sprint (MySprintSpeed is a player-pawn field).
//!
//! The pad's frame comes in through [`Party::set_pad_frame`] before `Party::step`; the decisions that start
//! commands (attacks, spells, items) are made there, so B's command intake takes them in the same step;
//! locomotion and jumps run in the pad states `NpcStateId::Pad(n)` ([`pre_move`] before the movement tick,
//! [`tick`] as the state's MainProc). Attacks, spells, abilities and items are B's `MemberCommand::Action` /
//! `UseItem` (the members' real definitions and clips).

pub mod menu;
pub mod sprint; // (party X2) decision 9: the friend leader's sprint
// (party X3D) Donald's playable kit: finishers, X moves, Aero Float, Spell Volley, his own abilities
pub mod donald;
pub mod donald_kit;
pub mod donald_shots;
pub mod donald_combo; // (DON-KIT) Donald's missile combo, the Charged Missile, Wand Bonk
pub mod donald_moves; // (DON-KIT) Donald's dodges, Magic Block, Aero Float / Glide, the attune cast
// (party X3G) Goofy's playable kit: the shield guard and counters, X moves, finishers, tiers, mode, Discus Barrage
pub mod goofy;
pub mod goofy_kit;
pub mod goofy_shots;
pub mod goofy_combo; // (GOOFY-GK) Goofy's own ground / air combo (the tank rework)
#[cfg(test)]
mod donald_tests;

use crate::command::{ActionDef, CommandTicket, MemberCommand, TargetRef};
use crate::host::PartyHost;
use crate::ids::{Chara, MemberId};
use crate::pawn::{forward, horizontal, wrap180, yaw_of, NpcPawn, StepCtx};
use crate::states::NpcStateId;
use crate::tables;
use glam::{Vec2, Vec3};
use kh_core::command::{CommandSnapshot, MenuEvent};
use kh_core::host::{TargetFlags, TargetId, TargetInfo};
use kh_core::input::{PadButton, PadState as Buttons};
use kh_core::params::SORA_PARAMS;
use kh_core::Tuning;

/// The pad states (`NpcStateId::Pad(n)`).
pub const RUN: u8 = 1;
pub const JUMP: u8 = 2;
pub const AIR_ATTACK: u8 = 3;

/// What the host hands over each step.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PadFrame {
    /// The raw left stick (x right, y forward, -1 .. 1).
    pub stick: Vec2,
    /// The camera's yaw of the previous step (the controller steers by it, camera spec 1.4).
    pub camera_yaw: f32,
    /// The buttons: held, pressed this step (the jump is B, `PadButton::JUMP`).
    pub pad: Buttons,
}

/// The speed-matched locomotion pose (dec016) for the host's drawing: the walk cycle and the run cycle with its
/// lean samples, each at its own time on the shared phase, and the run weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocoPose {
    pub walk: &'static str,
    pub walk_time: f32,
    /// Up to two run samples (lean left / straight / lean right) and their weights (sum 1).
    pub run: [(&'static str, f32); 2],
    pub run_time: f32,
    /// 0 = the walk cycle only, 1 = the run cycle only.
    pub run_weight: f32,
}

/// Sora's Run-state bookkeeping, for the member (kh-core `states/run.rs` `Run`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RunBook {
    /// 0 loop, 1 start requested, 2 start / turn-around clip, 4 stop clip.
    pub sub: u8,
    pub request: bool,
    pub yaw: f32,
    pub magnitude: f32,
    pub walk: bool,
    pub grace: f32,
    initial_start: bool,
    from_rest: bool,
    age: f32,
    had_run: bool,
    ring: [f32; 16],
    ring_len: u8,
    /// Speed ramp: 0 walk gait, 1 run gait, < 0 unset.
    gait: f32,
    /// The start / turn-around clip's axis and the root time already applied.
    start_axis: f32,
    root_t: f32,
    /// The speed-matched phase (0..1) of the loop.
    phase: f32,
}

/// Sora's Jump-state bookkeeping, for the member (kh-core `states/jump.rs`, normal jump only).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct JumpBook {
    /// 0 start clip until the JumpStart notify, 1 launched (start clip), 2 rise loop, 3 fall entry, 4 falling,
    /// 10 / 11 landing.
    pub sub: u8,
    pub request: bool,
    pub yaw: f32,
    pub fall_height: f32,
    last_z: f32,
    pub air_attacks: u8,
}

/// The pad state of a member driven by a player (None on `NpcPawn::pad` = AI).
#[derive(Clone, Debug, PartialEq)]
pub struct PadState {
    pub frame: PadFrame,
    pub tuning: Tuning,
    pub stick_yaw: f32,
    pub stick_magnitude: f32,
    pub run: RunBook,
    pub jump: JumpBook,
    pub loco: Option<LocoPose>,
    /// The hard lock (RB) and the soft target (the nearest enemy in front) of this step.
    pub lock: TargetId,
    pub soft: TargetId,
    /// The lock changed this step (the host moves its lock-on camera).
    pub lock_changed: bool,
    /// The ground combo: steps done in this chain (1 .. 3) and a buffered A press (seconds left).
    pub combo: u8,
    pub buffered: f32,
    pub combo_ticket: CommandTicket,
    pub air_request: bool,
    /// THE UNISON / FUSION ROLL ON THE LEADER'S COMBO (stream D's D.6, the user's rule: the player-controlled leader
    /// starts team attacks): the combo wants its finisher (`finisher_pending`), the host rolls
    /// (`FriendState::roll_leader_unison`) and answers: Some(true) = Sora's core plays the unison (the finisher is
    /// skipped), Some(false) = the member's own finisher (Attack3_Jump).
    pub finisher_pending: bool,
    pub finisher_answer: Option<bool>,
    /// (FX1) THE HOST'S LATE "NO": the team move it answered "yes" to never started (AI Sora's latch ran out; kh-partyhost
    /// lead.rs `team_wait_step`). Seconds the member's own finisher may still start: Donald's / Goofy's kit plays it as
    /// soon as he can (free on the ground, at his Jump Strike's cancel point / landing, or at the chain point of a swing
    /// he started meanwhile).
    pub finisher_late_no: f32,
    pub menu: menu::FriendMenu,
    /// The tickets of the commands the pad issued (newest last, a few).
    pub issued: Vec<(CommandTicket, MemberCommand)>,
    /// The member's own movement parameters (restored when the pad lets go).
    saved: Option<kh_core::SoraParams>,
    /// The state at the last pre-move (the run's options are cleared when another state took over).
    last_state: NpcStateId,
    /// (party X2) Sora's magic for a leading Donald, Goofy's ability tiers, the sprint (crate::leader_magic).
    pub x2: crate::leader_magic::PadMagic,
    /// (party X3D) Donald's kit (pad/donald.rs); unused for Goofy.
    pub donald: donald::DonaldKit,
    /// (party X3G) Goofy's kit (pad/goofy.rs); unused for Donald.
    pub goofy: goofy::GoofyPad,
}

impl PadState {
    pub fn new(tuning: Tuning) -> PadState {
        PadState {
            frame: PadFrame::default(),
            tuning,
            stick_yaw: 0.0,
            stick_magnitude: 0.0,
            run: RunBook { gait: -1.0, ..RunBook::default() },
            jump: JumpBook::default(),
            loco: None,
            lock: TargetId::NONE,
            soft: TargetId::NONE,
            lock_changed: false,
            combo: 0,
            buffered: 0.0,
            combo_ticket: CommandTicket(0),
            air_request: false,
            finisher_pending: false,
            finisher_answer: None,
            finisher_late_no: 0.0, // (FX1)
            menu: menu::FriendMenu::default(),
            issued: Vec::new(),
            saved: None,
            last_state: NpcStateId::Idle,
            x2: Default::default(), // (party X2)
            donald: Default::default(), // (party X3D)
            goofy: Default::default(), // (party X3G)
        }
    }

    /// The target an attack / spell takes: the hard lock, else the soft target.
    pub fn target(&self) -> TargetRef {
        let t = if self.lock != TargetId::NONE { self.lock } else { self.soft };
        if t == TargetId::NONE { TargetRef::None } else { TargetRef::Enemy(t) }
    }
}

impl Default for PadState {
    fn default() -> PadState {
        PadState::new(Tuning::default())
    }
}

// ---------------------------------------------------------------------------------------------- party side

impl crate::party::Party {
    /// Hands a member to the pad (`Some(tuning)`: Sora's port options) or back to the AI (`None`).
    pub fn set_pad(&mut self, id: MemberId, tuning: Option<Tuning>) {
        let Some(m) = self.member_mut(id) else { return };
        match tuning {
            Some(t) => {
                if m.pad.is_none() {
                    let mut pad = PadState::new(t);
                    // Sora's jump on their bodies (our design: KH3 gives the NPC pawns no player jump; their
                    // movement component's jump phase is UTresCharMovementComponent's 1 / 0 / 1, which with
                    // Sora's 1480 cm/s would fly 7 m): the rise gravity of p_ex001_PawnBase.
                    pad.saved = Some(m.params);
                    m.params.gravity_jump_up_scale = SORA_PARAMS.gravity_jump_up_scale;
                    m.params.gravity_jump_duration_time = SORA_PARAMS.gravity_jump_duration_time;
                    m.params.gravity_jump_duration_scale = SORA_PARAMS.gravity_jump_duration_scale;
                    pad.run.yaw = m.yaw;
                    pad.jump.yaw = m.yaw;
                    m.pad = Some(pad);
                    m.status.set_leading(true); // (GOOFY-BAL) the leader's stats (crate::balance)
                    // whatever the AI was doing (a move, a turn) ends
                    if matches!(m.state, NpcStateId::Move | NpcStateId::Turn) {
                        m.command(MemberCommand::Stop);
                    }
                } else if let Some(p) = m.pad.as_mut() {
                    p.tuning = t;
                }
            }
            None => {
                m.status.set_leading(false); // (GOOFY-BAL) back to KH3's status as an AI friend
                if let Some(pad) = m.pad.take() {
                    if let Some(saved) = pad.saved {
                        m.params = saved;
                    }
                    m.cmc.speed_limit_override = None;
                    m.cmc.walk_mode = false;
                    m.anim.speed = 1.0;
                    if matches!(m.state, NpcStateId::Pad(_)) {
                        // (party P3) an air strike's hover ends too
                        if m.state == NpcStateId::Pad(AIR_ATTACK) {
                            m.cmc.gravity_off = false;
                            m.cmc.direct_z = None;
                        }
                        if !m.on_ground() {
                            m.cmc.set_falling();
                        }
                        m.to_idle();
                    }
                }
            }
        }
    }

    pub fn is_pad(&self, id: MemberId) -> bool {
        self.member(id).is_some_and(|m| m.pad.is_some())
    }

    /// The pad's frame of this step, BEFORE `Party::step`: the menu and the attack / spell / item decisions
    /// (their commands are taken by the step's command intake).
    pub fn set_pad_frame(&mut self, id: MemberId, frame: PadFrame) {
        let Some(m) = self.member_mut(id) else { return };
        if m.pad.is_none() {
            return;
        }
        decide(m, frame);
    }

    /// The friend's command menu as the HUD draws it.
    pub fn pad_menu(&self, id: MemberId) -> Option<CommandSnapshot> {
        let m = self.member(id)?;
        let pad = m.pad.as_ref()?;
        let mut snap = pad.menu.snapshot(m.chara, &m.status);
        goofy::situation(m, &mut snap); // (party X3G) the Shield Guardian offer / its Finish on his situation slot
        Some(snap)
    }

    pub fn pad_state(&self, id: MemberId) -> Option<&PadState> {
        self.member(id)?.pad.as_ref()
    }
}

/// The stick through the game's per-axis dead zone (`MassageAxisInput`, controller.rs `dead_zone`): (yaw in the
/// world by the camera, magnitude).
pub fn stick_of(stick: Vec2, camera_yaw: f32) -> (f32, f32) {
    let dz = SORA_PARAMS.input_axis_dead_zone;
    let axis = |v: f32| if v > 0.0 { (v - dz).max(0.0) / (1.0 - dz) } else { -(-v - dz).max(0.0) / (1.0 - dz) };
    let s = if stick.is_finite() { stick } else { Vec2::ZERO };
    let (fwd, right) = (axis(s.y), axis(s.x));
    (wrap180(camera_yaw + right.atan2(fwd).to_degrees()), (fwd * fwd + right * right).sqrt())
}

fn melee(chara: Chara, step: u8) -> ActionDef {
    match (chara, step) {
        (Chara::Donald, 1) => ActionDef::DonaldBeat,
        (Chara::Donald, 2) => ActionDef::DonaldSwing,
        (Chara::Donald, _) => ActionDef::DonaldJump,
        (Chara::Goofy, 1) => ActionDef::GoofyPush,
        (Chara::Goofy, 2) => ActionDef::GoofySweep,
        (Chara::Goofy, _) => ActionDef::GoofyJump,
    }
}

fn is_melee(def: Option<ActionDef>) -> bool {
    matches!(def, Some(ActionDef::DonaldBeat | ActionDef::DonaldSwing | ActionDef::GoofyPush | ActionDef::GoofySweep))
}

/// The end of the running clip's first attack window (the chain point of the pad combo, our design: the
/// friends' clips carry no combo window; the next hit starts when this one's window has closed).
fn attack_window_end(p: &NpcPawn) -> Option<f32> {
    p.anim.clip.attack_windows().map(|(_, end, _)| end).fold(None, |a: Option<f32>, e| Some(a.map_or(e, |a| a.max(e))))
}

/// May the pad start something new now (attack, spell, item, jump)?
fn free(p: &NpcPawn) -> bool {
    matches!(p.state, NpcStateId::Idle | NpcStateId::Pad(RUN)) || matches!(p.state, NpcStateId::Pad(JUMP) if p.pad.as_ref().is_some_and(|d| d.jump.sub >= 10))
}

/// The melee chain window of the running Action (after the hit window, or after EnableChange).
fn chain_open(p: &NpcPawn) -> bool {
    p.state == NpcStateId::Action && is_melee(p.action.and_then(|r| r.def)) && (p.action.is_some_and(|r| r.jump.enable_change) || attack_window_end(p).is_some_and(|e| p.anim.time >= e))
}

fn issue(p: &mut NpcPawn, cmd: MemberCommand) -> bool {
    let t = p.command(cmd);
    let ok = p.command_status(t) == crate::command::CommandStatus::Running;
    if let Some(pad) = p.pad.as_mut() {
        pad.issued.push((t, cmd));
        if pad.issued.len() > 8 {
            pad.issued.remove(0);
        }
    }
    ok
}

/// The decisions of a step (before the party's step).
fn decide(p: &mut NpcPawn, frame: PadFrame) {
    let chara = p.chara;
    let dead = p.state == NpcStateId::Die;
    let Some(mut pad) = p.pad.take() else { return };
    pad.frame = frame;
    pad.x2.y_taken = false; // (party X2)
    pad.x2.queued = None; // (party X2) a cast parked last step was taken by that step's intake (or refused)
    let (yaw, mag) = stick_of(frame.stick, frame.camera_yaw);
    pad.stick_yaw = yaw;
    pad.stick_magnitude = mag;
    let action = pad.menu.tick(chara, &p.status, &frame.pad, dead);
    if action == Some(menu::MenuAction::Attack) {
        pad.buffered = BUFFER_TIME;
    }
    let target = pad.target();
    p.pad = Some(pad);
    match action {
        // (DON-KIT) the X3D Flare Force chain / follow-up casts are gone (doc 13): Donald's spells are DonCast (below)
        Some(menu::MenuAction::Spell(def)) => {
            // Cure: on himself (his Cure heals the party within 300 cm of the target too, 0x1414742A0).
            let t = if def == ActionDef::DonaldCure { TargetRef::Member(p.id) } else { target };
            let link = chara == Chara::Goofy && goofy::link_for_ability(p); // (GOOFY-GK) his abilities from a combo link point
            let ok = (free(p) || p.state == NpcStateId::Pad(JUMP) || chain_open(p) || link) && issue(p, MemberCommand::Action { def, target: t });
            refused(p, ok);
        }
        Some(menu::MenuAction::Item(item)) => {
            let ok = free(p) && issue(p, MemberCommand::UseItem { item, target: TargetRef::Member(p.id) });
            refused(p, ok);
        }
        // (party X2) a leading Donald's Sora spell (his Magic list / shortcuts): the same start windows as his spells,
        // plus a cast's follow-up window (crate::leader_magic::follow_open)
        // (DON-KIT) no Flare Force on the chain's 4th, no Quick Chant (doc 13)
        Some(menu::MenuAction::SoraSpell(cmd)) => {
            let ok = (free(p) || p.state == NpcStateId::Pad(JUMP) || chain_open(p) || crate::leader_magic::follow_open(p)) && crate::leader_magic::request_cast(p, cmd, target);
            refused(p, ok);
        }
        // (party X2) Y on his grand-magic offer
        Some(menu::MenuAction::Grand(serial, cmd)) => {
            // (party X3D) the coordinator's ruling: X2's Y is the trigger, the move is the kit's GRAND state (pad/donald.rs);
            // X2's own grand cast path (request_grand -> the action's MagicCast) is no longer called
            // (DON-KIT) Donald's Grand Magic on Y is gone (doc 6.3: the Arcane Y list replaces it, DON-UI / DON-MAGIC)
            let _ = (target, serial, cmd);
            refused(p, false);
        }
        // (party X2) a leading Goofy's ability at a tier (the action is his KH3 one; X3G reads `x2.tier`)
        Some(menu::MenuAction::Tiered(def, tier)) => {
            if let Some(pad) = p.pad.as_mut() {
                pad.x2.tier = tier;
            }
            // (GOOFY-GK) a combo step's link point (or a bash's / Hop's cancel point) links into the ability
            let link = chara == Chara::Goofy && goofy::link_for_ability(p);
            let ok = (free(p) || p.state == NpcStateId::Pad(JUMP) || chain_open(p) || link) && issue(p, MemberCommand::Action { def, target });
            refused(p, ok);
        }
        _ => {}
    }
    // (party X3G) Goofy's kit takes the A / Y presses that are its own (the guard's counters, the chain, the finisher,
    // the Jump Strike with no target, Bombardier Dive, Shield Guardian)
    if chara == Chara::Goofy && goofy::decide(p, target) {
        return;
    }
    // (DON-KIT) Donald's kit: his menu actions (DonCast / DonAttune), Y (the Arcane list), X; A runs in pre-move (missiles,
    // the Wand Bonk), the press buffered as the game's queued attack input
    if chara == Chara::Donald {
        donald::decide(p, action, target);
        if let Some(pad) = p.pad.as_mut() {
            if dead {
                pad.buffered = 0.0;
            }
            pad.buffered = (pad.buffered - 1.0 / 60.0).max(0.0);
        }
        return;
    }
    // The attack press (buffered for a short while, our design: as the game's queued attack input).
    let Some(mut pad) = p.pad.take() else { return };
    let mut start: Option<ActionDef> = None;
    let mut jump_point: Option<bool> = None; // (party X3D)
    if pad.buffered > 0.0 && !dead {
        let grounded = p.on_ground();
        let jump_air = matches!(p.state, NpcStateId::Pad(JUMP)) && pad.jump.sub < 10 && !grounded;
        if free(p) && grounded {
            pad.combo = 1;
            start = Some(melee(chara, 1));
        } else if chain_open(p) && pad.combo < 3 {
            // the third step is the jump attack (Attack3_Jump: Ascent + Descent rows); it needs a target to
            // jump at (UTresNpcAttackDefinitionJumpBase Begin 0x141220750), else the chain starts over
            let next = pad.combo + 1;
            if next == 3 && chara == Chara::Donald {
                // (party X3D) Donald's Jump Strike needs no target (a point ahead) and is no finisher: the unison roll
                // and the finisher are his 4th step (pad/donald.rs)
                pad.combo = 3;
                pad.finisher_answer = None;
                start = Some(melee(chara, 3));
                jump_point = Some(target == TargetRef::None);
            } else if next == 3 && target == TargetRef::None {
                pad.combo = 1;
                start = Some(melee(chara, 1));
            } else if next == 3 && pad.finisher_answer.is_none() {
                // the host rolls the unison first (the press stays buffered for the answer)
                pad.finisher_pending = true;
            } else if next == 3 && pad.finisher_answer == Some(true) {
                // the unison: Sora's core plays it, stream D puts this member into its FR state; no finisher
                pad.finisher_answer = None;
                pad.combo = 0;
                pad.buffered = 0.0;
            } else {
                pad.finisher_answer = None;
                pad.combo = next;
                start = Some(melee(chara, next));
            }
        } else if (jump_air || (p.state == NpcStateId::Pad(AIR_ATTACK) && air_chain_open(p))) && pad.jump.air_attacks < 2 {
            pad.air_request = true;
            pad.buffered = 0.0;
        }
        if start.is_some() {
            pad.buffered = 0.0;
        }
    }
    // (party P3) A press made during a melee swing (or an air strike) waits for its chain point instead of running out
    // after 0.3 s: mashing A carries the combo on, as Sora's queued attack input does (his attack state keeps the
    // request until the combo window). The Push's chain point is 0.63 s into M_1301: a press at 0.3 s was lost.
    let waiting = !dead && pad.combo < 3 && ((p.state == NpcStateId::Action && is_melee(p.action.and_then(|r| r.def)) && !chain_open(p)) || (p.state == NpcStateId::Pad(AIR_ATTACK) && !air_chain_open(p) && pad.jump.air_attacks < 2));
    if !waiting {
        pad.buffered = (pad.buffered - 1.0 / 60.0).max(0.0);
    }
    p.pad = Some(pad);
    if let Some(def) = start {
        // (party X3D) Donald's Jump Strike with no target jumps at the point ahead (stick, else facing)
        let target = if jump_point == Some(true) { donald::jump_point(p) } else { target };
        issue(p, MemberCommand::Action { def, target });
    }
}

const BUFFER_TIME: f32 = 0.3;

fn refused(p: &mut NpcPawn, ok: bool) {
    if !ok {
        if let Some(pad) = p.pad.as_mut() {
            pad.menu.events.push(MenuEvent::Refused);
        }
    }
}

fn air_chain_open(p: &NpcPawn) -> bool {
    attack_window_end(p).is_some_and(|e| p.anim.time >= e)
}

// ---------------------------------------------------------------------------------------------- targeting

/// The soft target (our design, after kh-core's targeting: the nearest hittable target within 800 cm whose
/// direction is within 100 degrees of the stick / facing) and the RB lock-on (the nearest in 1500 cm, toggled by
/// RB, RT steps to the next nearest).
fn targeting(p: &mut NpcPawn, host: &mut PartyHost) {
    let mut list: Vec<TargetInfo> = Vec::new();
    host.targets.enumerate(&mut list);
    let me = p.position();
    let face = p.yaw;
    let is_donald = p.chara == Chara::Donald; // (party X3D)
    let is_goofy = p.chara == Chara::Goofy; // (party X3G)
    let Some(pad) = p.pad.as_mut() else { return };
    pad.lock_changed = false;
    let want = if pad.stick_magnitude >= SORA_PARAMS.input_stick_active_min { pad.stick_yaw } else { face };
    // (AUD) Sora's rule (kh-core combat/targeting.rs: targetable while ALIVE | LOCKABLE): a friend leader's soft target and
    // lock-on never pick a breakable pot, a friendly NPC or a merchant (HITTABLE without LOCKABLE in Elden Ring: the empty
    // "!HP" target gauge and the auto cursor on no enemy that ERT4 saw). His attack volumes still hit what they touch.
    let alive = |t: &TargetInfo| t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE);
    if pad.lock != TargetId::NONE && !list.iter().any(|t| t.id == pad.lock && alive(t) && t.position.distance(me) < LOCK_LOST) {
        pad.lock = TargetId::NONE;
        pad.lock_changed = true;
    }
    let mut near: Vec<(f32, TargetId)> = list.iter().filter(|t| alive(t)).map(|t| (horizontal(t.position - me).length(), t.id)).collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    // sticky: the soft target is kept while it stays in range (a combo that carries him past it keeps it)
    // (DON-KIT) Donald's missiles reach 25 m: his soft target is chosen within donald::K.combo_soft_range (doc 1 / 3.1)
    let soft_range = if is_donald { donald::K.combo_soft_range } else { SOFT_RANGE };
    let keep = list.iter().find(|t| t.id == pad.soft && alive(t) && horizontal(t.position - me).length() <= soft_range).map(|t| t.id);
    pad.soft = keep.unwrap_or_else(|| list
        .iter()
        .filter(|t| alive(t))
        .filter(|t| {
            let d = horizontal(t.position - me);
            d.length() <= soft_range && (d.length() < 1.0 || wrap180(yaw_of(d) - want).abs() <= SOFT_ANGLE)
        })
        .min_by(|a, b| a.position.distance(me).total_cmp(&b.position.distance(me)))
        .map_or(TargetId::NONE, |t| t.id));
    let pressed = pad.frame.pad.pressed;
    // (party X3D) Donald: RB held opens Spell Volley's aim, so the lock toggles on a SHORT TAP's release (Sora's rule,
    // aim2.input.tap_max_time 0.2667 s); Goofy keeps the press
    let rb = if is_donald { pressed.has(PadButton::RightShoulder) /* (DON-KIT) no shotlock: RB is lock-on only */ } else if is_goofy { goofy::rb_tap(pad) /* (party X3G) RB held = Discus Barrage */ } else { pressed.has(PadButton::RightShoulder) };
    if rb {
        if pad.lock != TargetId::NONE {
            pad.lock = TargetId::NONE;
        } else {
            pad.lock = near.iter().find(|(d, _)| *d <= LOCK_RANGE).map_or(TargetId::NONE, |n| n.1);
        }
        pad.lock_changed = true;
    } else if pressed.has(PadButton::RightTrigger) && pad.lock != TargetId::NONE {
        let in_range: Vec<TargetId> = near.iter().filter(|(d, _)| *d <= LOCK_RANGE).map(|n| n.1).collect();
        if let Some(i) = in_range.iter().position(|t| *t == pad.lock) {
            let next = in_range[(i + 1) % in_range.len()];
            if next != pad.lock {
                pad.lock = next;
                pad.lock_changed = true;
            }
        }
    }
}

const SOFT_RANGE: f32 = 800.0;
const SOFT_ANGLE: f32 = 100.0;
const LOCK_RANGE: f32 = 1500.0;
const LOCK_LOST: f32 = 3000.0;

// ---------------------------------------------------------------------------------------------- the hooks

/// Whether the pad drives the pawn's current state.
pub fn owns(p: &NpcPawn) -> bool {
    matches!(p.state, NpcStateId::Pad(_))
}

fn enter_run(p: &mut NpcPawn, yaw: f32, magnitude: f32, from_rest: bool) {
    let walk = magnitude < SORA_PARAMS.input_walk_run_threshold;
    p.enter(NpcStateId::Pad(RUN));
    if let Some(pad) = p.pad.as_mut() {
        pad.run = RunBook { sub: if walk { 0 } else { 1 }, request: true, yaw, magnitude, walk, from_rest, gait: -1.0, ..RunBook::default() };
        pad.loco = None;
    }
}

fn enter_jump(p: &mut NpcPawn, fall: bool) {
    p.enter(NpcStateId::Pad(JUMP));
    let (z, yaw) = (p.position().z, p.yaw);
    let set = p.cmc.max_walk_speed;
    let _ = set;
    p.cmc.max_walk_speed = p.params.speed_run;
    p.cmc.max_acceleration = p.params.speed_run * SORA_PARAMS.accel_air_factor;
    p.cmc.speed_limit_override = None;
    p.cmc.walk_mode = false;
    p.anim.speed = 1.0;
    let mut air = 0;
    if let Some(pad) = p.pad.as_mut() {
        air = if fall { pad.jump.air_attacks } else { 0 };
        pad.loco = None;
    }
    if fall {
        p.play("JumpNormalDownLoop", -1.0);
    } else {
        p.play("JumpNormalStart", -1.0);
    }
    if let Some(pad) = p.pad.as_mut() {
        pad.jump = JumpBook { sub: if fall { 4 } else { 0 }, request: false, yaw, fall_height: 0.0, last_z: z, air_attacks: air };
    }
}

/// The JumpStart notify time of the jump start clip (TresAnimNotify_JumpStart, 0.1 s in both sets).
fn jump_start_time(p: &NpcPawn) -> f32 {
    p.anim.clip.notifies.iter().find(|n| n.class == "TresAnimNotify_JumpStart").map_or(0.1, |n| n.start)
}

fn enable_change_time(p: &NpcPawn) -> Option<f32> {
    p.anim.clip.notifies.iter().find(|n| n.class == "TresAnimNotify_EnableChange").map(|n| n.start)
}

/// After the command intake, before the movement tick (the controller's part of Sora's frame).
pub fn pre_move(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    targeting(p, host);
    // Another state took the member out of the pad's run (an attack, a reaction, an item): the run's port
    // options on the movement component and the slot are cleared (Run::End: SetWalkMode(false), the limit).
    let last = p.pad.as_ref().map_or(p.state, |d| d.last_state);
    if last == NpcStateId::Pad(RUN) && p.state != NpcStateId::Pad(RUN) {
        p.cmc.speed_limit_override = None;
        p.cmc.walk_mode = false;
        if p.state != NpcStateId::BadStatFreeze {
            p.anim.speed = 1.0;
        }
    }
    if p.state != NpcStateId::Pad(RUN) {
        if let Some(d) = p.pad.as_mut() {
            d.loco = None;
        }
    }
    // (party P3) an air strike's hover ends with the strike, whatever took him out of it (a hit, a spell, an item)
    if last == NpcStateId::Pad(AIR_ATTACK) && p.state != NpcStateId::Pad(AIR_ATTACK) && p.cmc.gravity_off {
        end_air_hover(p);
    }
    // (LADDER2) a pad leader's ladder jump (crate::ladder) is a scripted leap: the pad moves nothing meanwhile
    if p.state == NpcStateId::Ladder {
        let state = p.state;
        if let Some(pad) = p.pad.as_mut() {
            pad.air_request = false;
            pad.last_state = state;
        }
        return;
    }
    // (ZETTA) Zettaflare / an ultimate's cast roots him from the channel's start to the cast clip's end (the user: "Donald
    // cannot move while casting Zettaflare"): the frame moves nothing (no stick, jump, dodge); a hit still interrupts it
    if crate::donald::zettaflare::hold(p) {
        let state = p.state;
        if let Some(pad) = p.pad.as_mut() {
            pad.last_state = state;
        }
        return;
    }
    // (party X3D) Donald's kit first: its states, the X / RB moves, the finishers, the cancels, the float
    if p.chara == Chara::Donald && donald::pre_move(p, cx, host) {
        let state = p.state;
        if let Some(pad) = p.pad.as_mut() {
            pad.air_request = false;
            pad.last_state = state;
        }
        return;
    }
    // (party X3G) Goofy's kit: its states, the X / A / RB moves, the cancels, Tornado Copter, Shield Guardian
    if p.chara == Chara::Goofy && goofy::pre_move(p, cx, host) {
        let state = p.state;
        if let Some(pad) = p.pad.as_mut() {
            pad.air_request = false;
            pad.last_state = state;
        }
        return;
    }
    let Some(pad) = p.pad.as_ref() else { return };
    let (yaw, mag, buttons, tuning) = (pad.stick_yaw, pad.stick_magnitude, pad.frame.pad, pad.tuning);
    let active = mag >= SORA_PARAMS.input_stick_active_min;
    let jump_pressed = buttons.pressed.has(PadButton::JUMP) && !buttons.held.has(PadButton::LeftShoulder) && pad.menu.state != kh_core::command::MenuState::SubSel
        // (party X2) the B that closed the Magic / Items list this step is the cancel, not a jump (Sora's HUD masks it:
        // kh-core CommandState::masks_jump); seen in the X2 run: Donald jumped when the list was cancelled
        && !pad.menu.events.iter().any(|e| matches!(e, MenuEvent::SubClosed(_)));
    let jump_held = buttons.held.has(PadButton::JUMP);
    let air_request = pad.air_request;
    // (GOOFY-HR) the shared hit reactions (crate::hurt), after the kit (its own recovery move took the press already):
    // the aerial recovery in the recoverable window, the quick get-up past the damage clip's EnableChange / CheckBadStat
    if crate::hurt::pad_air_recovery(p, jump_pressed) {
        enter_jump(p, true);
        if let Some(pad) = p.pad.as_mut() {
            pad.air_request = false;
            pad.last_state = NpcStateId::Pad(JUMP);
        }
        return;
    }
    crate::hurt::pad_exit(p, active, jump_pressed);
    // The melee recovery: after EnableChange the stick or the jump takes him out (as Sora's attack state hands
    // over at TresEnableChange); a buffered next hit wins.
    if p.state == NpcStateId::Action && is_melee(p.action.and_then(|r| r.def)) && p.action.is_some_and(|r| r.jump.enable_change) && (active || jump_pressed) && p.pad.as_ref().is_some_and(|d| d.buffered <= 0.0) {
        p.end_action();
        p.action = None;
        p.cmc.direct_xy = None;
        if jump_pressed && p.on_ground() {
            enter_jump(p, false);
        } else {
            enter_run(p, yaw, mag, false);
        }
        return;
    }
    match p.state {
        NpcStateId::Idle => {
            if !p.on_ground() {
                enter_jump(p, true);
            } else if jump_pressed {
                enter_jump(p, false);
            } else if active {
                enter_run(p, yaw, mag, true);
            } else {
                // TresPlayerState_Idle: no turning without a request
                p.anim.speed = 1.0;
            }
        }
        NpcStateId::Pad(RUN) => {
            if jump_pressed && p.on_ground() {
                enter_jump(p, false);
            } else if active {
                try_run(p, yaw, mag, tuning.continuous_turning, tuning.start_lunge_window);
            }
        }
        NpcStateId::Pad(JUMP) | NpcStateId::Pad(AIR_ATTACK) => {
            if let Some(pad) = p.pad.as_mut() {
                if active {
                    pad.jump.request = true;
                    pad.jump.yaw = yaw;
                }
            }
            if air_request && !p.on_ground() {
                start_air_attack(p);
            } else if p.state == NpcStateId::Pad(JUMP) && jump_pressed && p.pad.as_ref().is_some_and(|d| d.jump.sub >= 10) {
                enter_jump(p, false);
            }
        }
        _ => {}
    }
    let state = p.state;
    if let Some(pad) = p.pad.as_mut() {
        pad.air_request = false;
        pad.last_state = state;
    }
    // dec014 (PORT OPTION `Tuning::jump_cut_on_release`, sora.rs): letting go of B while rising from the jump
    // caps the upward speed at max(release speed, the speed still needed for the minimum height).
    let rising = p.state == NpcStateId::Pad(JUMP) && p.pad.as_ref().is_some_and(|d| matches!(d.jump.sub, 1 | 2));
    if tuning.jump_cut_on_release && !jump_held && !p.on_ground() && rising {
        let gravity = p.cmc.rise_gravity(&p.params);
        let risen = p.cmc.location.z - p.cmc.launch_z;
        let missing = (tuning.min_jump_height - risen).max(0.0);
        let needed = if tuning.min_jump_height > 0.0 && missing > 0.0 { (2.0 * gravity * (missing + gravity * dt * dt / 8.0)).sqrt() } else { 0.0 };
        let cap = tuning.release_rise_speed.max(0.0).max(needed);
        p.cmc.velocity.z = p.cmc.velocity.z.min(cap);
    }
}

/// `TryRun` on the live Run state (run.rs `try_run`, RUN arm).
fn try_run(p: &mut NpcPawn, yaw: f32, magnitude: f32, continuous: bool, start_window: f32) {
    let Some(pad) = p.pad.as_mut() else { return };
    let r = &mut pad.run;
    let mut walk = magnitude < SORA_PARAMS.input_walk_run_threshold;
    if !walk {
        if r.walk {
            let start = r.from_rest && !r.had_run && r.age <= start_window;
            if r.sub == 0 && (!continuous || start) {
                r.sub = 1;
            }
        } else if r.sub == 4 {
            r.sub = 1;
        }
    } else {
        walk = r.sub == 0;
    }
    r.yaw = yaw;
    r.magnitude = magnitude;
    r.walk = walk;
    r.request = true;
}

fn start_air_attack(p: &mut NpcPawn) {
    let n = p.pad.as_ref().map_or(0, |d| d.jump.air_attacks);
    // Attack3_Jump's two strikes (ctor 0x14121BFA0 clip names): Up M_1312 (Attack3_JumpAscent), Down M_1311
    // (Attack3_JumpDescent); first the Up strike, the second press the Down strike (our design).
    let clip = if n == 0 { "M_1312" } else { "M_1311" };
    p.enter(NpcStateId::Pad(AIR_ATTACK));
    p.play(clip, 0.05);
    // (party P3) THE HOVER OF AN AIR STRIKE, as Sora's air attacks have it (kh-core attack.rs air mode: gravity off,
    // Flying, the hover ends at TresEnableChange, slot 128's tail 0x14134D510). Before, only the rise was stopped and
    // he fell at full gravity, so a strike from a low jump landed before its swing was over and the second press never
    // found its chain point. The hover ends at the clip's EnableChange (M_1312 0.94 s, M_1311 0.71 s) in `air_tick`.
    p.cmc.velocity.z = 0.0;
    p.cmc.gravity_off = true;
    p.cmc.set_flying();
    p.cmc.stop_movement();
    if let Some(pad) = p.pad.as_mut() {
        pad.jump.air_attacks = n + 1;
        // face the target
        let _ = pad;
    }
}

/// (party P3) The end of an air strike's hover: gravity back on, Falling.
fn end_air_hover(p: &mut NpcPawn) {
    p.cmc.gravity_off = false;
    p.cmc.direct_z = None;
    if !p.on_ground() {
        p.cmc.set_falling();
    }
}

/// The pad state's MainProc.
pub fn tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let _ = host;
    match p.state {
        NpcStateId::Pad(RUN) => run_tick(p, cx),
        NpcStateId::Pad(JUMP) => jump_tick(p, cx),
        NpcStateId::Pad(AIR_ATTACK) => air_tick(p, cx, host),
        NpcStateId::Pad(n) if goofy::owns_state(n) => goofy::tick(p, cx, host), // (party X3G) before Donald's n >= FIRST arm
        NpcStateId::Pad(n) if n >= donald::FIRST => donald::tick(p, cx, host), // (party X3D)
        _ => p.to_idle(),
    }
}

// (party K1) the locomotion helpers below are pub(crate): the AI members' Move state (crate::locomotion) uses them too
pub(crate) fn battle_set(p: &NpcPawn) -> bool {
    p.in_battle
}

/// The rows of the set in use (N weapon away / B battle pose).
pub(crate) fn row(p: &NpcPawn, n: &'static str, b: &'static str) -> &'static str {
    if battle_set(p) && tables::anim_set(p.chara).get(b).is_some() { b } else { n }
}

/// The start / turn-around clip without its first sample's root track (the table's RunStartN row carries the
/// 0007BL sample's): the pad applies the blended samples' root motion itself ([`start_root`]).
///
/// (party P3) THE START'S LENGTH AND BLEND. The table row's length is its FIRST sample's (0007BL / 1021BL, 0.30 s),
/// but the forward and side samples (0007 / 0007L / 0007R, 1021 / 1021L / 1021R) are 0.233 s: a start straight
/// ahead ran its clock 4 frames past the end of the samples it shows, so the pose stood still (the poser samples
/// each sequence at the slot's time) before the loop came in. The clip now has the blend space's play length at the
/// axis (stock UBlendSpace1D: the weighted sum of the two neighbouring samples' lengths, [`start_length`]), so a
/// start ends with its samples. Its blend-in is the player Run state's (Sora's RunStartN / RunStartB play: 0.067 s,
/// kh-core run.rs `play_as`); the friends' rows have m_InterpolationTime 0, which snapped from the idle / walk pose.
pub(crate) fn start_clip(chara: Chara, battle: bool, axis: f32) -> &'static crate::anim::ClipDef {
    use std::sync::Mutex;
    static CLIPS: Mutex<Vec<(Chara, bool, u32, &'static crate::anim::ClipDef)>> = Mutex::new(Vec::new());
    let base = tables::clip(chara, if battle { "RunStartB" } else { "RunStartN" });
    let length = start_length(chara, battle, axis).unwrap_or(base.length);
    // one clip per 1/120 s of length (the lengths lie between 0.233 and 0.300 s: a handful per set)
    let key = (length * 120.0).round().max(1.0) as u32;
    let mut all = CLIPS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = all.iter().find(|c| c.0 == chara && c.1 == battle && c.2 == key) {
        return c.3;
    }
    let blend_in = kh_core::AnimId::RunStartN.data().blend_in;
    let clip: &'static crate::anim::ClipDef = Box::leak(Box::new(crate::anim::ClipDef { root_motion: false, root: &[], length: key as f32 / 120.0, blend_in, ..*base }));
    all.push((chara, battle, key, clip));
    clip
}

/// (party P3) The start blend space's play length at `axis`: the two neighbouring samples' lengths weighted as
/// their poses are (stock UBlendSpace1D `TickAssetPlayer`: NewAnimLength = sum of weight x sample length).
pub(crate) fn start_length(chara: Chara, battle: bool, axis: f32) -> Option<f32> {
    let s = start_samples(battle);
    let a = axis.clamp(-180.0, 180.0);
    let i = s.iter().rposition(|x| x.1 <= a).unwrap_or(0).min(3);
    let f = ((a - s[i].1) / (s[i + 1].1 - s[i].1)).clamp(0.0, 1.0);
    let (l0, l1) = (seq_length(chara, s[i].0)?, seq_length(chara, s[i + 1].0)?);
    Some(l0 + (l1 - l0) * f)
}

/// (party P3) A sequence's length (stream A's SequenceData).
#[cfg(kh_party_anim_full)]
fn seq_length(chara: Chara, name: &str) -> Option<f32> {
    let seqs = match chara {
        Chara::Donald => &crate::notify_seam::full::ex001::SEQUENCES[..],
        Chara::Goofy => &crate::notify_seam::full::ex002::SEQUENCES[..],
    };
    seqs.iter().find(|s| s.name == name).map(|s| s.length).filter(|l| *l > 0.0)
}

#[cfg(not(kh_party_anim_full))]
fn seq_length(_chara: Chara, _name: &str) -> Option<f32> {
    None
}

/// The samples of the start blend space (`RunStartN` / `RunStartB` BlendSpace1D, axis -180 .. 180, samples
/// 0007BL -180, 0007L -90, 0007 0, 0007R 90, 0007BR 180; B: 1021*) [A: the pack manifest].
fn start_samples(battle: bool) -> [(&'static str, f32); 5] {
    if battle {
        [("1021BL", -180.0), ("1021L", -90.0), ("1021", 0.0), ("1021R", 90.0), ("1021BR", 180.0)]
    } else {
        [("0007BL", -180.0), ("0007L", -90.0), ("0007", 0.0), ("0007R", 90.0), ("0007BR", 180.0)]
    }
}

/// The root offset (x, y, yaw; frame-0 actor space) of the start blend space at `axis`, time `t` (stock
/// BlendSpace1D weights between the two neighbouring samples on one normalised clock).
pub(crate) fn start_root(chara: Chara, battle: bool, axis: f32, t_norm: f32) -> (f32, f32, f32) {
    let s = start_samples(battle);
    let a = axis.clamp(-180.0, 180.0);
    let i = s.iter().rposition(|x| x.1 <= a).unwrap_or(0).min(3);
    let f = ((a - s[i].1) / (s[i + 1].1 - s[i].1)).clamp(0.0, 1.0);
    let r0 = seq_root(chara, s[i].0, t_norm);
    let r1 = seq_root(chara, s[i + 1].0, t_norm);
    (r0.0 + (r1.0 - r0.0) * f, r0.1 + (r1.1 - r0.1) * f, r0.2 + (r1.2 - r0.2) * f)
}

/// A sequence's root track at normalised time (stream A's SequenceData, `party_anim_n_ex00x.rs`).
#[cfg(kh_party_anim_full)]
pub(crate) fn seq_root(chara: Chara, name: &str, t_norm: f32) -> (f32, f32, f32) {
    let seqs = match chara {
        Chara::Donald => &crate::notify_seam::full::ex001::SEQUENCES[..],
        Chara::Goofy => &crate::notify_seam::full::ex002::SEQUENCES[..],
    };
    let Some(s) = seqs.iter().find(|s| s.name == name) else { return (0.0, 0.0, 0.0) };
    let keys = s.root;
    if keys.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let t = t_norm.clamp(0.0, 1.0) * s.length;
    if t <= keys[0].time {
        return (keys[0].x, keys[0].y, keys[0].yaw);
    }
    for w in keys.windows(2) {
        if t <= w[1].time {
            let f = if w[1].time > w[0].time { (t - w[0].time) / (w[1].time - w[0].time) } else { 1.0 };
            let l = |a: f32, b: f32| a + (b - a) * f;
            return (l(w[0].x, w[1].x), l(w[0].y, w[1].y), l(w[0].yaw, w[1].yaw));
        }
    }
    let k = keys[keys.len() - 1];
    (k.x, k.y, k.yaw)
}

#[cfg(not(kh_party_anim_full))]
pub(crate) fn seq_root(_chara: Chara, _name: &str, _t_norm: f32) -> (f32, f32, f32) {
    (0.0, 0.0, 0.0)
}

/// The cycles of the speed-matched loop: (walk sequence, its length, run samples (lean -, 0, +), run length, the
/// lean axis' half range) of `WalkRunN` / `WalkRunB` [A: the pack manifest's samples].
pub(crate) fn loco_set(p: &NpcPawn) -> (&'static str, f32, [&'static str; 3], f32, f32) {
    let battle = battle_set(p);
    let lean = match p.chara {
        Chara::Donald => 300.0,
        Chara::Goofy => 180.0,
    };
    let len = |row: &str, fallback: f32| {
        let c = tables::clip(p.chara, row);
        if c.length > 0.0 { c.length } else { fallback }
    };
    if battle {
        // WalkRunB: walk 1016, run 1017 / 1017L / 1017R; Goofy's mid sample 1016a at speed 410 is added by
        // `loco_mid` (party P2)
        let walk_len = match p.chara {
            Chara::Donald => 0.8666667,
            Chara::Goofy => len("WalkLoopB", 1.2),
        };
        ("1016", walk_len, ["1017L", "1017", "1017R"], len("RunLoopB", 0.6), lean)
    } else {
        ("0002", len("WalkLoopN", 0.8), ["0003L", "0003", "0003R"], len("RunLoopN", 0.6), lean)
    }
}

/// (party P2) THE MID-SPEED SAMPLE of a speed-matched set: (sequence, its length, its speed on the blend space's speed
/// axis). Goofy's `WalkRunB` (n_ex002 rt/body/WalkRunB [A: the pack manifest's samples]) has 1016a at Y 410 on all three
/// lean columns (-180 / 0 / +180), between 1016 at 140 and 1017 / 1017L / 1017R at 680; its length 0.8667 s (27 frames,
/// SequenceData of party_anim_n_ex002.rs). Donald's WalkRunB and both WalkRunN have no mid row.
pub(crate) fn loco_mid(p: &NpcPawn) -> Option<(&'static str, f32, f32)> {
    (p.chara == Chara::Goofy && battle_set(p)).then_some(("1016a", 0.8666667, 410.0))
}

/// (party P2) The speed-matched pose with a mid sample, as the blend space's bilinear grid has it along its speed axis:
/// below the mid speed walk <-> mid (the mid row has no lean: 1016a in every column), above it mid <-> the run samples
/// with their lean. Returned in `LocoPose`'s two-cycle form: the lower cycle as `walk`, the upper as `run`.
/// `w` = the run weight of the two-cycle blend (0 at the walk speed, 1 at the run speed), `wm` = the mid's place on it.
pub(crate) fn loco_three(walk: (&'static str, f32), mid: (&'static str, f32), runs: [(&'static str, f32); 2], run_len: f32, phase: f32, w: f32, wm: f32) -> LocoPose {
    if w <= wm {
        let t = if wm > 0.0 { (w / wm).clamp(0.0, 1.0) } else { 1.0 };
        LocoPose { walk: walk.0, walk_time: phase * walk.1, run: [(mid.0, 1.0), (mid.0, 0.0)], run_time: phase * mid.1, run_weight: t }
    } else {
        let t = if wm < 1.0 { ((w - wm) / (1.0 - wm)).clamp(0.0, 1.0) } else { 1.0 };
        LocoPose { walk: mid.0, walk_time: phase * mid.1, run: runs, run_time: phase * run_len, run_weight: t }
    }
}

pub(crate) fn stop_clip(p: &NpcPawn, is_run: bool) -> &'static str {
    let reverse = p.anim.window_open("ReverseLeg");
    match (is_run, reverse) {
        (true, false) => row(p, "RunEndR", "RunEndRB"),
        (true, true) => row(p, "RunEndL", "RunEndLB"),
        (false, false) => row(p, "WalkEndR", "WalkEndRB"),
        (false, true) => row(p, "WalkEndL", "WalkEndLB"),
    }
}

/// `TresPlayerState_Run::Tick` (run.rs `run_tick`) on the member's clips, with the user's options.
fn run_tick(p: &mut NpcPawn, cx: &StepCtx) {
    let dt = cx.dt;
    let sp = SORA_PARAMS;
    if !p.on_ground() {
        // Fall(yaw): the probe of the game is the movement's floor here [D]
        enter_jump(p, true);
        return;
    }
    let Some(mut pad) = p.pad.take() else { return };
    let tuning = pad.tuning;
    let r = &mut pad.run;
    let is_run = r.grace > 0.0 || !r.walk;
    let delta = wrap180(r.yaw - p.yaw);
    r.age += dt;
    if r.sub == 1 || (r.sub == 0 && !r.walk) {
        r.had_run = true;
    }
    let mut swing = 0.0f32;
    if r.sub == 0 {
        let window = (tuning.reversal_window as usize).min(15);
        let have = r.ring_len as usize;
        for old in &r.ring[have.saturating_sub(window)..have] {
            swing = swing.max(wrap180(r.yaw - old).abs());
        }
        if have == 16 {
            r.ring.copy_within(1.., 0);
            r.ring[15] = r.yaw;
        } else {
            r.ring[have] = r.yaw;
            r.ring_len += 1;
        }
    }
    let battle = battle_set(p);
    let yaw0 = p.yaw;
    let mut fresh = false;
    match r.sub {
        0 => {
            if !r.walk {
                let flick = swing >= tuning.reversal_angle;
                if delta.abs() >= sp.turn_turn_around_angle && (!tuning.continuous_turning || flick) {
                    // the turn-around: the start blend space at the delta (BL / BR: the 180-degree samples)
                    r.ring_len = 0;
                    r.gait = 1.0;
                    p.cmc.stop_movement();
                    p.anim.play(start_clip(p.chara, battle, delta), -1.0);
                    // (party P3) the speed-matched loop's pose is not drawn over the clip (it froze on its last phase)
                    pad.loco = None;
                    p.anim.axis = delta;
                    r.start_axis = delta;
                    r.root_t = 0.0;
                    r.sub = 2;
                    r.initial_start = false;
                    p.cmc.max_walk_speed = p.params.speed_run;
                }
            }
            p.cmc.max_walk_speed = p.params.speed_run;
            if r.sub == 0 && (!matches!(p.anim.clip.name, "WalkLoopN" | "RunLoopN" | "WalkLoopB" | "RunLoopB") || !tuning.speed_matched_anim) {
                let want = if is_run { row(p, "RunLoopN", "RunLoopB") } else { row(p, "WalkLoopN", "WalkLoopB") };
                if p.anim.clip.name != want {
                    p.play(want, -1.0);
                    fresh = true;
                }
            }
        }
        1 => {
            p.anim.play(start_clip(p.chara, battle, delta), -1.0);
            // (party P3) THE START FROM A WALK (a real stick passes the walk zone for a few frames from standstill):
            // the walk's speed-matched pose stayed in `loco`, and the host draws `loco` over the slot, so the start
            // was drawn as the walk frozen on its last phase, sliding by the start's root motion. Cleared here.
            pad.loco = None;
            p.anim.axis = delta;
            r.start_axis = delta;
            r.root_t = 0.0;
            r.initial_start = true;
            r.sub = 2;
            r.gait = 1.0;
        }
        2 => {
            if p.anim.finished {
                let want = if is_run { row(p, "RunLoopN", "RunLoopB") } else { row(p, "WalkLoopN", "WalkLoopB") };
                p.play(want, -1.0);
                fresh = true;
                r.initial_start = false;
                r.sub = 0;
                r.phase = 0.0;
            }
        }
        _ => {
            // 4: the stop clip; Turn(yaw, dt x 12); its end -> Idle
            p.yaw = wrap180(p.yaw + wrap180(r.yaw - p.yaw) * (dt * sp.turn_run_stop_rate).clamp(0.0, 1.0));
            if p.anim.finished {
                p.pad = Some(pad);
                p.to_idle();
                if let Some(d) = p.pad.as_mut() {
                    d.loco = None;
                }
                p.cmc.speed_limit_override = None;
                p.cmc.walk_mode = false;
                return;
            }
            p.pad = Some(pad);
            return;
        }
    }
    // (party X2) decision 9: Sora's sprint on the friend leader (pad/sprint.rs): MaxWalkSpeed 980 after 1 s of running
    // (GOOFY-GK) Goofy leading runs / sprints slower than Sora and Donald (the user: "Make Goofy run slower than Donald and
    // Sora"; goofy_kit K.move_run / move_sprint). His speed-matched cycle keeps its own 680 samples (it plays slower).
    let run_cap = if p.chara == Chara::Goofy { goofy_kit::K.move_run } else if p.chara == Chara::Donald { donald::K.move_run /* (DON-KIT) doc 8 */ } else { p.params.speed_run };
    sprint::tick(&mut pad.x2, r.sub == 0 && !r.walk, p.state_time, dt, run_cap, &mut p.cmc.max_walk_speed);
    if p.chara == Chara::Goofy && pad.x2.sprinting {
        p.cmc.max_walk_speed = goofy_kit::K.move_sprint; // (GOOFY-GK)
    }
    if p.chara == Chara::Donald && pad.x2.sprinting {
        p.cmc.max_walk_speed = donald::K.move_sprint; // (DON-KIT) doc 8: sprint 900 after 1 s (Sora 980)
    }
    // The start / turn-around clip's root motion (the blended samples' track, our application: a direct XY
    // velocity for the next movement tick and the yaw at once).
    if r.sub == 2 {
        let len = p.anim.clip.length.max(1e-4);
        let t1 = (p.anim.time / len).clamp(0.0, 1.0);
        let a = start_root(p.chara, battle, r.start_axis, r.root_t);
        let b = start_root(p.chara, battle, r.start_axis, t1);
        r.root_t = t1;
        // the delta in the actor frame at the previous time (keys are frame-0)
        let (s, c) = (-a.2).to_radians().sin_cos();
        let (dx, dy) = ((b.0 - a.0) * c - (b.1 - a.1) * s, (b.0 - a.0) * s + (b.1 - a.1) * c);
        let f = forward(p.yaw);
        let right = Vec3::new(-f.y, f.x, 0.0);
        let world = f * dx + right * dy;
        if dt > 0.0 {
            p.cmc.direct_xy = Some(Vec2::new(world.x / dt, world.y / dt));
        }
        p.yaw = wrap180(p.yaw + (b.2 - a.2));
    }
    // COMMON
    if !r.request {
        if r.sub == 0 || (r.sub == 2 && !r.initial_start) {
            let acc = p.cmc.acceleration;
            if acc.x * acc.x + acc.y * acc.y <= sp.ground_stop_accel_sq_max && r.sub == 0 {
                p.cmc.stop_movement();
                let stop = stop_clip(p, is_run);
                p.play(stop, -1.0);
                r.grace = 0.0;
                r.sub = 4;
                pad.loco = None;
                p.cmc.speed_limit_override = None;
                p.pad = Some(pad);
                return;
            }
        }
        if r.sub == 0 {
            p.yaw = wrap180(p.yaw + wrap180(r.yaw - p.yaw) * (sp.turn_run_alpha_per_60hz_frame * dt * 60.0).clamp(0.0, 1.0));
        }
    } else {
        p.cmc.walk_mode = !is_run;
        r.grace = if !r.walk { sp.input_run_grace_time } else { (r.grace - dt).max(0.0) };
        if r.sub == 0 {
            p.yaw = wrap180(p.yaw + wrap180(r.yaw - p.yaw) * (sp.turn_run_alpha_per_60hz_frame * dt * 60.0).clamp(0.0, 1.0));
            p.cmc.max_acceleration = p.cmc.max_walk_speed * sp.accel_ground_factor;
            let a = r.yaw.to_radians();
            p.cmc.control_input = Vec3::new(a.cos(), a.sin(), 0.0);
        }
    }
    // dec015 PORT OPTION `Tuning::speed_ramp`
    if r.sub == 0 {
        if tuning.speed_ramp {
            let target = if is_run { 1.0 } else { 0.0 };
            if r.gait < 0.0 || !(tuning.speed_ramp_time > 0.0) {
                r.gait = target;
            } else if r.request {
                let step = dt / tuning.speed_ramp_time;
                r.gait = if r.gait < target { (r.gait + step).min(target) } else { (r.gait - step).max(target) };
            }
            let (walk, run) = (p.cmc.max_walk_speed * sp.speed_walk_rate, p.cmc.max_walk_speed);
            p.cmc.speed_limit_override = Some(walk + (run - walk) * r.gait);
        } else {
            p.cmc.speed_limit_override = None;
        }
    }
    // dec016 PORT OPTION `Tuning::speed_matched_anim`: the walk and run cycles of WalkRunN / WalkRunB on one phase
    if r.sub == 0 && tuning.speed_matched_anim {
        let (walk_seq, walk_len, runs, run_len, lean) = loco_set(p);
        let (walk_speed, run_speed) = (p.params.speed_run * sp.speed_walk_rate, p.params.speed_run);
        let v = p.cmc.velocity;
        let speed = (v.x * v.x + v.y * v.y).sqrt();
        let weight = if fresh { if is_run { 1.0 } else { 0.0 } } else { ((speed - walk_speed) / (run_speed - walk_speed)).clamp(0.0, 1.0) };
        let mut stride = walk_speed * walk_len + (run_speed * run_len - walk_speed * walk_len) * weight;
        // (party P2) Goofy's battle set: the stride through the mid sample too (piecewise, as the pose below)
        let mid = loco_mid(p).map(|(seq, len, at)| (seq, len, ((at - walk_speed) / (run_speed - walk_speed).max(1.0)).clamp(0.0, 1.0), at));
        if let Some((_, mid_len, wm, at)) = mid {
            stride = if weight <= wm {
                let t = if wm > 0.0 { weight / wm } else { 1.0 };
                walk_speed * walk_len + (at * mid_len - walk_speed * walk_len) * t
            } else {
                let t = if wm < 1.0 { (weight - wm) / (1.0 - wm) } else { 1.0 };
                at * mid_len + (run_speed * run_len - at * mid_len) * t
            };
        }
        let rate = if fresh { 1.0 / if is_run { run_len } else { walk_len } } else { speed.max(walk_speed) / stride.max(1.0) };
        r.phase = (r.phase + rate * dt).fract();
        // the lean (X of the 2D rows): the turn rate, deg/s [inf, as kh-party's Move state reads it]
        let turn_rate = if dt > 0.0 { wrap180(p.yaw - yaw0) / dt } else { 0.0 };
        let x = (turn_rate / lean).clamp(-1.0, 1.0);
        let samples = if x < 0.0 { [(runs[1], 1.0 + x), (runs[0], -x)] } else { [(runs[1], 1.0 - x), (runs[2], x)] };
        pad.loco = Some(match mid {
            // (party P2) 1016a between the walk and the run (Goofy's WalkRunB)
            Some((mid_seq, mid_len, wm, _)) => loco_three((walk_seq, walk_len), (mid_seq, mid_len), samples, run_len, r.phase, weight, wm),
            None => LocoPose { walk: walk_seq, walk_time: r.phase * walk_len, run: samples, run_time: r.phase * run_len, run_weight: weight },
        });
        // the slot's own clip follows the dominant cycle on the phase (its notifies: sounds, face, ReverseLeg)
        let dominant = if weight >= 0.5 { row(p, "RunLoopN", "RunLoopB") } else { row(p, "WalkLoopN", "WalkLoopB") };
        if p.anim.clip.name != dominant {
            p.play(dominant, 0.0);
        }
        let len = p.anim.clip.length.max(1e-4);
        // time for the next tick's advance: set the rate so the clock lands on the phase
        p.anim.speed = (rate * len) / p.anim.clip.rate.max(1e-4);
        let want_t = r.phase * len;
        if (p.anim.time - want_t).abs() > 0.05 * len {
            p.anim.time = want_t;
        }
        p.anim.axis = turn_rate;
        p.anim.axis_y = speed;
    } else if r.sub != 2 {
        pad.loco = None;
        p.anim.speed = 1.0;
    }
    r.request = false;
    p.pad = Some(pad);
}

/// `TresPlayerState_Jump::Tick` (jump.rs) on the member's clips: the normal jump only.
fn jump_tick(p: &mut NpcPawn, cx: &StepCtx) {
    let dt = cx.dt;
    let sp = SORA_PARAMS;
    let Some(mut pad) = p.pad.take() else { return };
    let j = &mut pad.jump;
    let z = p.position().z;
    match j.sub {
        0 => {
            // the JumpStart notify of JumpNormalStart (TresAnimNotify_JumpStart): the launch with Sora's power
            if p.anim.time >= jump_start_time(p) {
                let power = if p.chara == Chara::Donald { sp.jump_power_normal * donald::K.move_jump_scale /* (DON-KIT) doc 8: a lower jump */ } else { sp.jump_power_normal };
                p.cmc.press_jump(power);
                j.sub = 1;
            }
        }
        1 => {
            if p.anim.finished {
                p.play("JumpNormalRiseLoop", -1.0);
                j.sub = 2;
            }
            j.last_z = z;
        }
        2 => {
            if p.cmc.velocity.z <= 0.0 {
                p.play("JumpNormalDownLoop", -1.0);
                j.sub = 4;
                j.last_z = z;
                p.cmc.gravity_scale = p.params.gravity_scale;
            }
        }
        4 => {
            if p.on_ground() {
                p.cmc.stop_movement();
                // LandName: LightLand (L / R by the leg) up to land.light_height_max, else Land
                let clip = if !(j.fall_height > sp.land_light_height_max) { if p.anim.window_open("ReverseLeg") { "LightLandL" } else { "LightLandR" } } else { "Land" };
                // (party X3D) Donald from a high fall lands with RollLand (XD 1.4: the 800 / 2000 cm rule)
                let clip = if p.chara == Chara::Donald && j.fall_height > donald::K.land_roll_height { "RollLand" } else { clip };
                p.play(clip, -1.0);
                j.sub = 10;
                j.air_attacks = 0;
            }
        }
        10 | 11 => {
            j.sub = 11;
            let change = enable_change_time(p).is_some_and(|t| p.anim.time >= t);
            if (j.request && change) || p.anim.finished {
                p.cmc.stop_movement();
                let (yaw, mag) = (pad.stick_yaw, pad.stick_magnitude);
                let active = mag >= sp.input_stick_active_min;
                p.pad = Some(pad);
                if active {
                    enter_run(p, yaw, mag, false);
                    if let Some(d) = p.pad.as_mut() {
                        d.run.sub = 0;
                    }
                } else {
                    p.to_idle();
                }
                return;
            }
        }
        _ => {}
    }
    // TAIL: Turn(yaw, dt x 6); input when requested and not landing
    p.yaw = wrap180(p.yaw + wrap180(j.yaw - p.yaw) * (dt * sp.turn_air_rate).clamp(0.0, 1.0));
    if j.request && !matches!(j.sub, 10 | 11) {
        let a = j.yaw.to_radians();
        p.cmc.control_input = Vec3::new(a.cos(), a.sin(), 0.0);
    }
    j.request = false;
    if j.sub == 4 {
        let zz = p.position().z;
        j.fall_height = if p.cmc.velocity.z >= 0.0 { 0.0 } else { j.fall_height + (j.last_z - zz) };
        j.last_z = zz;
    }
    p.pad = Some(pad);
}

/// The air strike (M_1312 / M_1311 with their Attack windows: B's hit test delivers the hits).
fn air_tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    let target = p.pad.as_ref().map_or(TargetRef::None, |d| d.target());
    if let Some(t) = p.target_point(target, cx, host) {
        if p.anim.window_open("TurnToTarget") || p.anim.time < 0.15 {
            p.turn_toward(t, dt, crate::pawn::turn_rate(crate::pawn::TurnKind::Attack));
        }
    }
    // (party P3) the hover ends at the strike's EnableChange (or its end): gravity on, falling
    if p.cmc.gravity_off && (enable_change_time(p).is_some_and(|t| p.anim.time >= t) || p.anim.finished) {
        end_air_hover(p);
    }
    if p.on_ground() {
        p.cmc.stop_movement();
        p.play("Land", -1.0);
        if let Some(d) = p.pad.as_mut() {
            d.jump.sub = 10;
            d.jump.air_attacks = 0;
        }
        p.enter(NpcStateId::Pad(JUMP));
        return;
    }
    if p.anim.finished {
        p.play("JumpNormalDownLoop", -1.0);
        let z = p.position().z;
        if let Some(d) = p.pad.as_mut() {
            d.jump.sub = 4;
            d.jump.last_z = z;
            d.jump.fall_height = 0.0;
        }
        p.enter(NpcStateId::Pad(JUMP));
    }
}

#[cfg(test)]
mod tests;
