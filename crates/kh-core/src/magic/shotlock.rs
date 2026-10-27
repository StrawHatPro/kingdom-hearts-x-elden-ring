//! The shotlock aim mode on the controller: slot 800 `ShootLockUpdate(bHold, dt)` 0x1414165E0
//! (docs/sora-magic-spec.md part F, which supersedes D S2), the airstep target choice
//! `AirstepTargetUpdate` 0x14141FDC0 (part H 4.2), `SelectKind`, `CanAddLock`, `GetConsumed`,
//! `StartCommand` and `Exit`.
//!
//! No state is pushed and no animation is played by the aim mode: while it is open the
//! controller dispatches no movement, jump or attack request (the caller skips them), so Run
//! brakes into Idle and a jump falls ballistically, on the slowed clock.
//!
//! What the host supplies: RB held (`Input::pad`), the attack and airstep buttons as presses,
//! the view of the last frame (`Input::view`: the reticle test), the targets (every `ALIVE |
//! LOCKABLE` target has shotlock and move-flow permission and is of another team; its `center` is
//! the component location), the world's line trace (line of sight), and the level's airstep
//! points (`PawnMagic::airstep_points`). What it gets: the per-type time dilation
//! (`Sora::time_dilation`), the events `AimMode`, `LockMarker`, `SoundState`, and the panel in
//! `UiSnapshot::command.shoot_flow`.
//!
//! Timers: `dt` is the CONTROLLER's dilated dt (the controller is object type VEHICLE and is
//! slowed with the world); the start delay and the aim time divide by the controller's time
//! dilation (real time), the lock interval divides by the stored slow rate 0.4 (F A3.3).

use glam::Vec3;

use super::spells::cmd;
use super::tables::Sheet;
use crate::enums::ETresAbilityKind;
use crate::host::{EventKind, TargetFlags, TargetId, TargetInfo};
use crate::input::PadButton;
use crate::pawn::Ctx;
use crate::sora::Input;
use crate::states::{athletic_flow, shoot_flow, StateData};

/// Flags of ctrl+0xA08.
pub mod flag {
    pub const ACTIVE: u32 = 0x01;
    pub const RELEASE_PENDING: u32 = 0x02;
    pub const WAIT_RELEASE: u32 = 0x10;
    pub const MAX: u32 = 0x40;
}

/// `ETresShootFlowKind` values of the Kingdom Key.
pub const KIND_KC_SHOT: u8 = 1;
pub const KIND_KC_RAGNAROK: u8 = 2;
pub const KIND_FOCUS_ASPIR: u8 = 36;
pub const KIND_AIRSTEP: u8 = 37;

/// One entry of the candidate / locked lists (0x34 bytes; F A2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LockEntry {
    pub target: TargetId,
    /// Distance, -1 until measured.
    pub dist: f32,
    /// 1 candidate, 2 locked.
    pub state: u8,
    pub lock_count: i32,
    pub timer: f32,
}

/// A level "airstep point" (`TresShootFlowActor` / `TresShootFlowComponent`; H 4.2.1): a scene
/// component with a position, a kind and a close range. A host affordance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirstepPoint {
    pub id: u32,
    pub position: Vec3,
    /// `ESQEX_FreeFlowKind`: 4 GROUND, 5 WALL (the component's default), 6 POLE ...
    pub kind: u8,
    /// `m_CloseRange` (default 100): the arrival radius is this + 100.
    pub close_range: f32,
    /// `m_ShootLockOnRange` (0 = the default limit 15000).
    pub range: f32,
}

impl AirstepPoint {
    pub fn wall(id: u32, position: Vec3) -> AirstepPoint {
        AirstepPoint { id, position, kind: 5, close_range: 100.0, range: 0.0 }
    }

    /// (integration pass) A point of kind 6 POLE: laid on a pole of the host's registry
    /// (`World::poles`); the arrival's `PoleCheck` hands over to POLE_TURN.
    pub fn pole(id: u32, position: Vec3) -> AirstepPoint {
        AirstepPoint { id, position, kind: 6, close_range: 100.0, range: 0.0 }
    }
}

/// One airstep candidate / target: an enemy or a level point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirEntry {
    /// The enemy (`TargetId::NONE` for a level point) / the point's id (0 for an enemy).
    pub target: TargetId,
    pub point: u32,
    /// `m_MoveFlowType`: 1 ENEMY ... 5 WALL.
    pub kind: u8,
    pub dist: f32,
    pub close_range: f32,
    pub own_range: f32,
}

/// The controller's aim-mode fields (F A2).
#[derive(Clone, Debug, PartialEq)]
pub struct AimMode {
    pub flags: u32,
    /// +0xA0C.
    pub hold_timer: f32,
    /// +0xA10: 0 off, 1 aiming, 2 fired, 3 airstep requested, 4 focus-empty aiming.
    pub mode: u8,
    /// +0xA18 / +0xA1C.
    pub total: i32,
    pub locked_targets: i32,
    /// +0xA24, +0xA28, +0xA38, +0xA3C, +0xA40, +0xA4C.
    pub lock_timer: f32,
    pub max_locks: i32,
    pub start_delay: f32,
    pub aim_time: f32,
    pub slow_rate: f32,
    pub kind: u8,
    /// +0x964.
    rr: usize,
    pub candidates: Vec<LockEntry>,
    pub locked: Vec<LockEntry>,
    pub airstep_candidates: Vec<AirEntry>,
    pub airstep_targets: Vec<AirEntry>,
    /// +0xB28 / +0xB2C.
    pub consumed: i32,
    pub focus0: i32,
    /// +0x9A4: the lock-on target saved at entry.
    pub saved_lock: TargetId,
    /// (IMPL_ER42, port, USER REQUEST "shotlock should aim at softlock target"; KH3 keeps none) the SOFT target at entry:
    /// `lockon.current` when no lock is on (`NONE` when locked: `saved_lock` holds it). Only the hosts' aim camera reads it.
    pub saved_soft: TargetId,
    pub can_airstep: bool,
    pub is_max: bool,
    /// Real seconds spent aiming after the start delay (for tests and the debug text).
    pub aimed: f32,
    /// (CHESTFIX, port) How often an aim mode left open by a dropped state was closed (`close_dropped` at a teleport,
    /// the net in `update`): the hosts log it.
    pub stale_cleared: u32,
}

impl Default for AimMode {
    fn default() -> AimMode {
        AimMode { flags: 0, hold_timer: 0.0, mode: 0, total: 0, locked_targets: 0, lock_timer: 0.0, max_locks: 0, start_delay: 0.0, aim_time: 0.0, slow_rate: 0.4, kind: 0, rr: 0, candidates: Vec::new(), locked: Vec::new(), airstep_candidates: Vec::new(), airstep_targets: Vec::new(), consumed: 0, focus0: 0, saved_lock: TargetId::NONE, saved_soft: TargetId::NONE, can_airstep: false, is_max: false, aimed: 0.0, stale_cleared: 0 }
    }
}

/// What a host / test reads (`Sora::aim`).
#[derive(Clone, Debug, PartialEq)]
pub struct AimView {
    pub mode: u8,
    pub kind: u8,
    pub total: i32,
    pub locked_targets: i32,
    pub max_locks: i32,
    pub aim_time: f32,
    pub is_max: bool,
    pub can_airstep: bool,
    /// (target, locks on it), in lock order.
    pub locks: Vec<(TargetId, i32)>,
    /// The airstep chain: (enemy, point id), nearest hop first, the chosen one last.
    pub airstep: Vec<(TargetId, u32)>,
    /// (CHESTFIX) `AimMode::stale_cleared`.
    pub stale_cleared: u32,
}

impl AimMode {
    /// (CHESTFIX, the user 2026-10-27: "he stopped moving and even swapping to him from the party menu I couldn't move as
    /// him") PORT ADDITION: THE RUNNING STATE WAS DROPPED WITHOUT ITS End (`Sora::teleport` makes a new Sora in Idle and
    /// keeps the pawn's magic). An aim mode that is open (aiming, or fired and waiting for ShootFlow / AthleticFlow to
    /// report back) is closed as `exit` closes it, and R1 must be let go before it opens again (WAIT_RELEASE). Before: a
    /// warp of AI Sora during his shotlock's ShootFlow (the user's log 660.360 "AI Sora warps next to Donald", Shootflow
    /// sub 5) kept mode 2 + RELEASE_PENDING for ever: `update` returned true on every step, so no movement, jump, attack
    /// or command reached him (as AI Sora, then as the leader after the swap). True when something was closed.
    pub fn close_dropped(&mut self) -> bool {
        if self.mode == 0 && self.flags & (flag::ACTIVE | flag::RELEASE_PENDING) == 0 {
            return false;
        }
        *self = AimMode { flags: flag::WAIT_RELEASE, slow_rate: self.slow_rate, stale_cleared: self.stale_cleared + 1, ..AimMode::default() };
        true
    }

    pub fn view(&self) -> AimView {
        AimView { mode: self.mode, kind: self.kind, total: self.total, locked_targets: self.locked_targets, max_locks: self.max_locks, aim_time: self.aim_time, is_max: self.is_max, can_airstep: self.can_airstep, locks: self.locked.iter().map(|e| (e.target, e.lock_count)).collect(), airstep: self.airstep_targets.iter().map(|e| (e.target, e.point)).collect(), stale_cleared: self.stale_cleared }
    }
}

fn valid(t: &TargetInfo) -> bool {
    t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE)
}

/// `Param0` of a `TresShootFlowData` row.
pub fn param0(kind: u8) -> i32 {
    Sheet::Shot.num_or(&format!("aim2.table.k{kind}.param0"), 0.0) as i32
}

/// The pawn's reference point (pawn vt+0x2110): actor location + (0, 0, 100).
fn ref_point(cx: &Ctx) -> Vec3 {
    cx.pawn.cmc.location + Vec3::Z * Sheet::Shot.num("aim2.lock.entry_ref_height")
}

/// Sora's own lock-on point (pawn vt+0x6E0): the capsule centre.
pub(crate) fn lock_point(cx: &Ctx) -> Vec3 {
    cx.pawn.cmc.capsule_centre(cx.pawn.params())
}

/// `SelectKind(reset)` 0x141411B80 (A3.5) for Sora with the Kingdom Key.
fn select_kind(cx: &mut Ctx, mut reset: bool) {
    let empty = cx.pawn.status.focus_empty;
    let aspir = cx.pawn.status.has(ETresAbilityKind::FocusAspir);
    // (XSI) SPECTRAL RAYS (XS 2.8): the Customize screen's choice, while the ability is equipped (abilities/xs).
    let spectral = crate::abilities::xs::spectral::chosen(cx.pawn);
    let aim = &mut cx.pawn.magic.aim;
    if empty && aspir {
        aim.kind = KIND_FOCUS_ASPIR;
        reset = true;
    } else if spectral {
        aim.kind = crate::abilities::xs::spectral::KIND;
    } else {
        aim.kind = KIND_KC_SHOT;
    }
    aim.max_locks = param0(aim.kind);
    if !reset {
        if aim.max_locks <= aim.total && aim.max_locks > 0 {
            get_consumed(cx);
            cx.pawn.magic.aim.flags |= flag::MAX;
        }
        let aim = &mut cx.pawn.magic.aim;
        // (XSI) Spectral Rays has no MAX upgrade.
        if aim.flags & flag::MAX != 0 && aim.kind == KIND_KC_SHOT {
            aim.kind = KIND_KC_RAGNAROK;
        }
    }
    let aim = &mut cx.pawn.magic.aim;
    aim.is_max = aim.flags & flag::MAX != 0;
}

/// `CanAddLock` 0x1413F0320 (A3.6).
fn can_add_lock(cx: &mut Ctx) -> bool {
    let status = &cx.pawn.status;
    let v = if status.focus_empty { status.count(ETresAbilityKind::FocusAspir) as i32 } else { status.focus };
    if v == 0 {
        return false;
    }
    let aim = &mut cx.pawn.magic.aim;
    if aim.max_locks <= aim.total && aim.max_locks > 0 {
        aim.flags |= flag::MAX;
    }
    if aim.mode == 4 && aim.total > Sheet::Shot.int("aim2.lock.syphon_max") - 1 {
        aim.flags |= flag::MAX;
    }
    aim.is_max = aim.flags & flag::MAX != 0;
    !aim.is_max
}

/// `GetConsumed` (slot 822 0x1413FD3D0): frozen while MAX holds or the focus-empty flag is set.
fn get_consumed(cx: &mut Ctx) -> i32 {
    let empty = cx.pawn.status.focus_empty;
    let aim = &mut cx.pawn.magic.aim;
    if empty || aim.flags & flag::MAX != 0 {
        return aim.consumed;
    }
    aim.consumed = aim.focus0 - (aim.aim_time / Sheet::Shot.num("aim2.focus.convert_rate")) as i32;
    aim.consumed
}

/// `Exit()` (slot 813 0x1413FAD90; A3.8): a cancel. No focus is spent.
pub(crate) fn exit(cx: &mut Ctx) {
    let aim = &mut cx.pawn.magic.aim;
    aim.flags = (aim.flags & !flag::ACTIVE) | flag::WAIT_RELEASE;
    aim.hold_timer = 0.0;
    if aim.mode == 0 {
        return;
    }
    aim.flags &= !flag::RELEASE_PENDING;
    (aim.mode, aim.rr, aim.total, aim.locked_targets, aim.aim_time, aim.consumed, aim.focus0) = (0, 0, 0, 0, 0.0, 0, 0);
    aim.candidates.clear();
    aim.locked.clear();
    aim.airstep_candidates.clear();
    aim.airstep_targets.clear();
    (aim.can_airstep, aim.is_max, aim.saved_lock, aim.saved_soft) = (false, false, TargetId::NONE, TargetId::NONE);
    super::set_aim_dilation(cx, 1.0, 1.0);
    cx.pawn.event(EventKind::SoundState { name: "STATE_SLOW_END" });
    cx.pawn.event(EventKind::AimMode { open: false, reason: 1 });
}

/// (CHESTFIX) The state a pending release waits for (the one whose End calls `release_done`) runs or is queued.
fn release_owner_alive(state: &StateData, cx: &Ctx) -> bool {
    use crate::states::StateId;
    matches!(state, StateData::ShootFlow(_) | StateData::AthleticFlow(_)) || matches!(cx.pawn.queue.queued_id(), Some(StateId::Shootflow | StateId::Athleticflow))
}

/// Controller slot 809 (vt+0x1948 0x14140E720): the attack / airstep state's End reports back:
/// the release-pending flag is cleared; the mode-2 / 3 branch then cleans up.
pub(crate) fn release_done(cx: &mut Ctx) {
    let aim = &mut cx.pawn.magic.aim;
    if aim.mode == 2 || aim.mode == 3 {
        aim.flags &= !flag::RELEASE_PENDING;
    }
}

/// `StartCommand(kind)` 0x14140F1F0 (A3.7): the focus is spent, the command is recorded, Sora's
/// own type goes back to full speed (the world stays slowed until the state ends).
fn start_command(cx: &mut Ctx, kind: u8) {
    if kind == KIND_FOCUS_ASPIR || kind == KIND_AIRSTEP {
        let aim = &mut cx.pawn.magic.aim;
        (aim.consumed, aim.focus0) = (0, 0);
    } else {
        let c = get_consumed(cx);
        let focus = cx.pawn.status.focus;
        cx.pawn.status.set_focus(focus - c);
        cx.pawn.status.refresh();
        let aim = &mut cx.pawn.magic.aim;
        (aim.consumed, aim.focus0) = (-1, 0);
    }
    cx.pawn.magic.aim.aim_time = 0.0;
    // ctrl.vt+0x1520(1.0), the command-begin event (0xFD, kind), ctrl+0x8F4 = 0.1.
    crate::command::lock(cx, 1.0);
    let c = &mut cx.pawn.command;
    (c.last_executed, c.last_shoot_flow, c.reaction_block) = (cmd::SHOOTFLOW, kind, 0.1);
    // SetTypeRate(CH_PLAYER, PLAYER, 1.0): Sora back to full speed.
    cx.set_time_dilation(super::MASK_PLAYER, 1.0);
    if kind == KIND_AIRSTEP {
        // SetTypeRates(CH_PLAYER, 0x4D9E, 0.1): everything in the mask at 0.1 (H 4.8); the
        // travel's launch then puts the player type back to 1.0.
        let rate = Sheet::Shot.num("airstep.start_time_dilation");
        super::set_aim_dilation(cx, rate, rate);
    }
    // The aim camera is released the moment the attack starts.
    cx.pawn.event(EventKind::AimMode { open: false, reason: if kind == KIND_AIRSTEP { 4 } else { 2 } });
}

/// `Enter()` (A3.2).
fn enter(cx: &mut Ctx) {
    let s = Sheet::Shot;
    let reference = ref_point(cx);
    let range = s.num("aim2.lock.range");
    let mut targets = Vec::new();
    cx.targets(&mut targets);
    let mut candidates = Vec::new();
    let mut airstep = Vec::new();
    for t in &targets {
        if !valid(t) {
            continue;
        }
        if (reference - t.center).length_squared() < range * range {
            candidates.push(LockEntry { target: t.id, dist: -1.0, state: 1, lock_count: 0, timer: 0.0 });
        }
        // Enemies have move-flow permission, kind ENEMY, close range 100, no own range.
        airstep.push(AirEntry { target: t.id, point: 0, kind: 1, dist: -1.0, close_range: 100.0, own_range: 0.0 });
    }
    for point in &cx.pawn.magic.airstep_points {
        airstep.push(AirEntry { target: TargetId::NONE, point: point.id, kind: point.kind, dist: -1.0, close_range: point.close_range, own_range: point.range });
    }
    // The lock-on is remembered and cleared (ctrl.vt+0x1780(0)).
    let lock = cx.pawn.combat.lockon;
    let saved = if lock.locked { lock.current } else { TargetId::NONE };
    // (IMPL_ER42, port) the soft target, for the hosts' aim camera only
    let soft = if lock.locked { TargetId::NONE } else { lock.current };
    crate::combat::targeting::unlock(cx);
    let empty = cx.pawn.status.focus_empty;
    let focus = cx.pawn.status.focus;
    let max_focus = cx.pawn.status.max_focus();
    {
        let aim = &mut cx.pawn.magic.aim;
        aim.flags |= flag::ACTIVE;
        aim.candidates = candidates;
        aim.airstep_candidates = airstep;
        aim.locked.clear();
        aim.airstep_targets.clear();
        aim.saved_lock = saved;
        aim.saved_soft = soft;
        (aim.total, aim.locked_targets, aim.rr, aim.hold_timer, aim.aimed) = (0, 0, 0, 0.0, 0.0);
    }
    select_kind(cx, true);
    let convert = s.num("aim2.focus.convert_rate");
    let aim = &mut cx.pawn.magic.aim;
    aim.start_delay = s.num("aim2.entry.start_delay");
    aim.lock_timer = 0.0;
    aim.consumed = -1;
    aim.slow_rate = s.num("aim2.slow.rate");
    let mut slow = false;
    if !empty {
        aim.mode = 1;
        aim.focus0 = focus;
        aim.aim_time = focus as f32 * convert;
        slow = aim.aim_time > 0.0;
    } else {
        // Sora only: the focus-syphon aim; no slow motion, no sound state.
        aim.mode = 4;
        aim.focus0 = max_focus;
        aim.aim_time = max_focus as f32 * convert;
    }
    if slow {
        // SetTypeRates(CH_PLAYER, 0x4D9E, 0.4, blend 0): PLAYER and the world types.
        let rate = cx.pawn.magic.aim.slow_rate;
        super::set_aim_dilation(cx, rate, rate);
        cx.pawn.event(EventKind::SoundState { name: "STATE_SLOW_START" });
    }
    cx.pawn.event(EventKind::AimMode { open: true, reason: 0 });
}

/// The reticle test (A4.2) and the line of sight (A4.3) for a component position.
fn in_reticle(_cx: &Ctx, input: &Input, p: Vec3, radius_sq: f32) -> Option<f32> {
    let view = input.view.as_ref()?;
    let (cx_px, cy_px, scale) = view.centre_and_scale();
    let s = view.project(p);
    if s.z <= 0.0 {
        return None;
    }
    let q = (s.x - cx_px) * (s.x - cx_px) + (s.y - cy_px) * (s.y - cy_px);
    (q <= scale * scale * radius_sq).then_some(q)
}

fn line_of_sight(cx: &Ctx, p: Vec3) -> bool {
    cx.world.line_trace(ref_point(cx), p).is_none()
}

fn point_position(cx: &Ctx, e: &AirEntry) -> Option<Vec3> {
    if e.target != TargetId::NONE {
        cx.target(e.target).filter(valid).map(|t| t.center)
    } else {
        cx.pawn.magic.airstep_points.iter().find(|p| p.id == e.point).map(|p| p.position)
    }
}

/// `AirstepTargetUpdate(dt) -> bool` (0x14141FDC0; H 4.2.3): the point nearest to the screen
/// centre within 80 px is chosen; level points within 160 px that are nearer than it and see it
/// become intermediate hops, nearest first. Enemies are never chained.
fn airstep_target_update(cx: &mut Ctx, input: &Input) -> bool {
    let s = Sheet::Shot;
    cx.pawn.magic.aim.airstep_targets.clear();
    if !cx.pawn.magic.airstep_unlocked {
        return false;
    }
    let eye = ref_point(cx);
    let limit = s.num("airstep2.select.limit");
    let sentinel = s.num("airstep2.select.range_sentinel");
    let (pick, near) = (s.num("airstep2.select.pick_radius_sq"), s.num("airstep2.select.near_radius_sq"));
    let mut candidates = cx.pawn.magic.aim.airstep_candidates.clone();
    let mut best: Option<(usize, f32)> = None;
    let mut tmp: Vec<(AirEntry, Vec3)> = Vec::new();
    for e in candidates.iter_mut() {
        let Some(p) = point_position(cx, e) else { continue };
        let d2 = (eye - p).length_squared();
        let range = if e.own_range > 0.0 && e.own_range != sentinel { e.own_range } else { limit };
        if d2 > range * range {
            continue;
        }
        let Some(q) = in_reticle(cx, input, p, near) else { continue };
        if !line_of_sight(cx, p) {
            continue;
        }
        e.dist = d2.sqrt();
        // "q < pick2": strictly smaller, and the bound shrinks to the best so far.
        let bound = best.map_or(pick * input.view.as_ref().map_or(1.0, |v| v.centre_and_scale().2.powi(2)), |b| b.1);
        if q < bound {
            best = Some((tmp.len(), q));
        }
        tmp.push((*e, p));
    }
    cx.pawn.magic.aim.airstep_candidates = candidates;
    let Some((best_index, _)) = best else { return false };
    let (chosen, chosen_position) = tmp[best_index];
    if chosen.kind == 0 {
        return false;
    }
    let mut chain = Vec::new();
    if chosen.kind < 4 {
        chain.push(chosen);
    } else {
        let mut sorted = tmp.clone();
        sorted.sort_by(|a, b| a.0.dist.total_cmp(&b.0.dist));
        for (e, p) in sorted {
            if e == chosen {
                chain.push(e);
                break;
            }
            // An intermediate hop: a level point that sees the chosen one.
            if e.kind > 3 && cx.world.line_trace(p, chosen_position).is_none() {
                chain.push(e);
            }
        }
    }
    cx.pawn.magic.aim.airstep_targets = chain;
    true
}

/// Slot 800 `ShootLockUpdate(bHold, dt)` 0x1414165E0, called from the controller's input
/// update after the lock-on tap detector. `dt` = the controller's dilated dt, `real_dt` = the
/// step's. Returns true while the aim mode is open (the caller then skips the normal dispatch).
pub(crate) fn update(state: &mut StateData, cx: &mut Ctx, input: &Input, dt: f32, real_dt: f32) -> bool {
    let s = Sheet::Shot;
    if !cx.pawn.magic.shotlock_unlocked || cx.pawn.status.placeholder {
        return false;
    }
    // ---- A3.1: the gate. "hold = 0" in mode 1 / 4 means cancel.
    let mut hold = input.pad.held.has(PadButton::RightShoulder);
    if cx.pawn.status.hp < 1 {
        hold = false;
    }
    match state {
        // DAMAGE: only Damage+0x88 == 1 keeps the hold.
        StateData::Damage(d) if d.kind.value() != 1 => hold = false,
        StateData::Die(_) => hold = false,
        _ => {}
    }
    if cx.pawn.command.slow_applied {
        // The command sub-menu's slow option is on: the aim mode was left when it opened.
        hold = false;
    }
    let mode = cx.pawn.magic.aim.mode;
    // ---- mode 0: entry (A3.2)
    if mode == 0 {
        let cooldown = cx.pawn.combat.command_cooldown > 0.0;
        let latch = cx.pawn.magic.command_latch;
        let aim = &mut cx.pawn.magic.aim;
        if !cooldown && hold && !latch {
            if aim.flags & flag::WAIT_RELEASE != 0 {
                return false;
            }
            aim.flags &= !(flag::MAX | flag::RELEASE_PENDING | 0x04);
            (aim.aim_time, aim.consumed, aim.focus0) = (0.0, -1, 0);
            aim.hold_timer += dt;
            if aim.hold_timer > s.num("aim2.entry.hold_time") {
                enter(cx);
                return true;
            }
        } else {
            aim.hold_timer = 0.0;
            aim.flags &= !(flag::ACTIVE | flag::WAIT_RELEASE);
        }
        return false;
    }
    // ---- modes 2 and 3: waiting for the state (A3.4)
    if mode == 2 || mode == 3 {
        // (CHESTFIX, the user 2026-10-27: "Sora ... stopped moving and even swapping to him from the party menu I couldn't
        // move as him") PORT ADDITION, THE NET: a release is pending only while the state that reports back (ShootFlow /
        // AthleticFlow: their End calls `release_done`) runs or is queued. Pending with neither (that state was dropped
        // without its End) it is stale and would hold the aim mode, and with it every movement, jump, attack and command
        // (this function's `true`), for ever: cleared, and the branch below closes the mode.
        if cx.pawn.magic.aim.flags & flag::RELEASE_PENDING != 0 && !release_owner_alive(state, cx) {
            let aim = &mut cx.pawn.magic.aim;
            aim.flags &= !flag::RELEASE_PENDING;
            aim.stale_cleared += 1;
        }
        if cx.pawn.magic.aim.flags & flag::RELEASE_PENDING != 0 {
            return true;
        }
        let aim = &mut cx.pawn.magic.aim;
        (aim.hold_timer, aim.mode, aim.total, aim.locked_targets) = (0.0, 0, 0, 0);
        aim.flags &= !flag::ACTIVE;
        aim.candidates.clear();
        aim.locked.clear();
        aim.airstep_targets.clear();
        (aim.can_airstep, aim.is_max) = (false, false);
        // ResetTypeRates(CH_PLAYER, blend 0).
        super::set_aim_dilation(cx, 1.0, 1.0);
        cx.pawn.event(EventKind::AimMode { open: false, reason: if mode == 2 { 3 } else { 5 } });
        return false;
    }
    // ---- modes 1 and 4: aiming (A3.3)
    {
        let aim = &mut cx.pawn.magic.aim;
        if aim.start_delay > 0.0 {
            aim.start_delay -= real_dt;
            if aim.aim_time <= 0.0 {
                aim.aim_time = 0.0;
            }
            return true;
        }
        if aim.aim_time > 0.0 {
            aim.aim_time = (aim.aim_time - real_dt).max(0.0);
        }
        aim.aimed += real_dt;
    }
    let range = s.num("aim2.lock.range");
    let eye = lock_point(cx);
    let syphon = mode == 4;
    let mut can_lock_phase = true;
    // The locked list is re-tested every tick with the current positions (stops at the first removal).
    if !cx.pawn.magic.aim.locked.is_empty() {
        let locked = cx.pawn.magic.aim.locked.clone();
        let gone = locked.iter().position(|e| match cx.target(e.target).filter(valid) {
            Some(t) => (eye - t.center).length_squared() > range * range,
            None => true,
        });
        if let Some(i) = gone {
            let aim = &mut cx.pawn.magic.aim;
            aim.total -= aim.locked[i].lock_count;
            let removed = aim.locked.remove(i);
            aim.locked_targets = aim.locked.len() as i32;
            aim.rr = 0;
            cx.pawn.event(EventKind::LockMarker { target: removed.target, locks: 0 });
            select_kind(cx, true);
            let aim = &mut cx.pawn.magic.aim;
            aim.flags &= !flag::MAX;
            aim.is_max = false;
            can_lock_phase = false;
        }
    }
    if can_lock_phase && cx.pawn.magic.aim.aim_time > 0.0 && can_add_lock(cx) {
        let reticle = s.num("aim2.lock.reticle_radius_sq");
        if !syphon {
            let aim = &mut cx.pawn.magic.aim;
            aim.lock_timer = (aim.lock_timer - dt / aim.slow_rate).max(0.0);
        }
        let n = cx.pawn.magic.aim.candidates.len();
        let rr = cx.pawn.magic.aim.rr;
        for k in 0..n {
            let idx = (rr + k) % n;
            let mut e = cx.pawn.magic.aim.candidates[idx];
            let Some(t) = cx.target(e.target).filter(valid) else { continue };
            if syphon {
                // Mode 4: each entry has its own timer.
                if e.timer > 0.0 {
                    e.timer -= dt;
                    cx.pawn.magic.aim.candidates[idx] = e;
                    continue;
                }
            } else if cx.pawn.magic.aim.lock_timer > 0.0 {
                continue;
            }
            if e.dist == -1.0 {
                let d2 = (eye - t.center).length_squared();
                if d2 > range * range {
                    continue;
                }
                e.dist = d2.sqrt();
                cx.pawn.magic.aim.candidates[idx] = e;
            }
            if in_reticle(cx, input, t.center, reticle).is_none() || !line_of_sight(cx, t.center) {
                continue;
            }
            e.state = 2;
            e.lock_count += 1;
            e.timer = if syphon { e.dist * s.num("aim2.lock.syphon_timer_per_cm") } else { s.num("aim2.lock.marker_timer") };
            let kind = cx.pawn.magic.aim.kind;
            let aim = &mut cx.pawn.magic.aim;
            aim.candidates[idx] = e;
            if !syphon {
                let p0 = param0(kind);
                aim.lock_timer = if p0 >= 1 { s.num("aim2.lock.interval_numerator") / p0 as f32 } else { s.num("aim2.lock.interval_fallback") };
            }
            if e.lock_count > 1 {
                // The entry of the same target is replaced by a copy; a target that was dropped
                // from the list and is locked again is counted without an entry (the game's quirk).
                if let Some(slot) = aim.locked.iter_mut().find(|l| l.target == e.target) {
                    *slot = e;
                }
            } else {
                aim.locked.push(e);
                aim.locked_targets += 1;
            }
            aim.total += 1;
            aim.rr = if syphon { aim.rr + 1 } else { idx + 1 };
            cx.pawn.event(EventKind::LockMarker { target: e.target, locks: e.lock_count });
            break;
        }
        select_kind(cx, false);
    }
    let can_airstep = airstep_target_update(cx, input);
    cx.pawn.magic.aim.can_airstep = can_airstep;
    // The release test: R1 let go (or any gate of A3.1) cancels.
    let keep = if syphon { hold && cx.pawn.status.focus_empty } else { hold };
    if !keep {
        exit(cx);
        return false;
    }
    if input.pad.pressed.has(PadButton::ATTACK) {
        let aim = &mut cx.pawn.magic.aim;
        if (syphon && aim.locked.is_empty()) || aim.total < 1 {
            return true;
        }
        (aim.mode, aim.rr) = (2, 0);
        aim.flags |= flag::RELEASE_PENDING;
        select_kind(cx, false);
        cx.pawn.event(EventKind::SoundState { name: "STATE_SLOW_END" });
        let aim = &mut cx.pawn.magic.aim;
        aim.airstep_candidates.clear();
        aim.airstep_targets.clear();
        let kind = aim.kind;
        // TryShootFlow (iface slot 95 0x1414413A0): push state 41.
        if shoot_flow::try_shoot_flow(state, cx, kind) {
            start_command(cx, kind);
        } else {
            exit(cx);
            return false;
        }
        return true;
    }
    if input.pad.pressed.has(PadButton::FaceLeft) {
        if cx.pawn.magic.aim.airstep_targets.is_empty() {
            return true;
        }
        let aim = &mut cx.pawn.magic.aim;
        (aim.mode, aim.rr, aim.locked_targets, aim.total) = (3, 0, 0, 0);
        aim.flags |= flag::RELEASE_PENDING;
        aim.candidates.clear();
        aim.locked.clear();
        cx.pawn.event(EventKind::SoundState { name: "STATE_SLOW_END" });
        // TryAthleticFlow (iface slot 97 0x1414372F0).
        if athletic_flow::try_athletic_flow(state, cx) {
            cx.pawn.magic.aim.kind = KIND_AIRSTEP;
            start_command(cx, KIND_AIRSTEP);
        } else {
            exit(cx);
            return false;
        }
        return true;
    }
    true
}
