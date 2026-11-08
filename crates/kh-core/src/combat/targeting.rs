//! Targeting and lock-on: the controller's `UpdateLockon` (slot 714 0x141404E50), read completely
//! in the disassembly by the second research pass (`build/sora_tmp/combat/P2_D_section.md`,
//! which replaces section 7 of docs/sora-combat-spec.md; "D 7.x" below are its subsections).
//!
//! The host's side: a target = one `TargetInfo` of `Targets::enumerate` (one lock-on component);
//! its `center` is the component location (searched, scored, homed on); it is targetable while
//! `ALIVE | LOCKABLE`; every enumerated target is of another team; the flags `ENEMY`, `GIMMICK`,
//! `PRIORITY_HIGH` are the owner tags and `m_LockonPriority`; `Targets::lock_on_range` is
//! `m_AutoLockOnRange`. The line of sight is `World::line_trace` (static world; pawns never
//! block it, D 7.4.1).
//!
//! Not here (not reachable with the Kingdom Key in base form, D 7.5): shooter mode, multi-lock,
//! the shotlock aim search, the dive-fall search, the UI's input lock, soft-only mode.
//! (F-ARROWGUN) The gun forms' shooter aim (step 5), the multi-lock (step 9) and the switch list outside the lock are
//! here now (combat/forms/arrowgun/aim.rs); the shooter request (step 6) runs right after this function (controller.rs).

use glam::Vec3;

use crate::host::{EventKind, TargetFlags, TargetId, TargetInfo};
use crate::input::PadButton;
use crate::pawn::Ctx;

/// The pawn's lock-on ranges (constructor 0x141423210; D 7.1): `MyAutoLockonLimit`,
/// `MyAutoLockonNear`, `MyManualLockonLimit`, `MyManualLockonOff`; the switch list's range and
/// length (controller +0x9C8, +0x9CC); the tap time of both buttons (D 7.2); the crowd radius.
pub const AUTO_LIMIT: f32 = 900.0;
pub const AUTO_NEAR: f32 = 150.0;
pub const MANUAL_LIMIT: f32 = 3500.0;
pub const MANUAL_OFF: f32 = 4200.0;
pub const SWITCH_RANGE: f32 = 2400.0;
pub const SWITCH_MAX: usize = 6;
pub const TAP_TIME: f32 = 0.266_666_68;
pub const CROWD_RADIUS: f32 = 400.0;
/// Search origin: root + (0, 0, 100) + facing x 30 (pawn slot 1058 0x141448550; D 7.4).
pub const ORIGIN_UP: f32 = 100.0;
pub const ORIGIN_PUSH: f32 = 30.0;

/// The HUD's cursor on a target (D 7.11): the auto cursor of the soft target, the lock cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CursorKind {
    Auto = 0,
    Lock = 1,
}

/// The controller's lock-on fields (D 7.1). Plain data, part of the save state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lockon {
    /// +0x934 / +0x938: RB tap flag (one frame) / hold time.
    pub rb_tap: bool,
    rb_time: f32,
    /// +0x93E / +0x940: RT tap flag / hold time.
    pub rt_tap: bool,
    rt_time: f32,
    /// +0x994: the current target, soft or locked.
    pub current: TargetId,
    /// +0x99C: the target of the last accepted attack request: pins `current` (D 7.6).
    pub attack_target: TargetId,
    /// +0x9AC: the best candidate of this frame's search, within 3500.
    pub candidate: TargetId,
    /// +0x95C: targets within 400 cm of the search origin.
    pub crowd: i32,
    /// +0x960: manual lock active.
    pub locked: bool,
    /// +0x964.
    switch_index: i32,
    /// The marker shown on `current` (the HUD's `Add` is idempotent: D 7.11).
    cursor: Option<(TargetId, CursorKind)>,
    /// (F-STAFF) +0x970: the soft-target range (MyAutoLockonLimit 900; StartStyle sets it to x 2.0 / x 3.0 for some forms,
    /// slot 738 0x141410FD0 resets it).
    pub soft_range: f32,
}

impl Default for Lockon {
    fn default() -> Lockon {
        Lockon { rb_tap: false, rb_time: 0.0, rt_tap: false, rt_time: 0.0, current: TargetId::NONE, attack_target: TargetId::NONE, candidate: TargetId::NONE, crowd: 0, locked: false, switch_index: 0, cursor: None, soft_range: AUTO_LIMIT }
    }
}

fn targetable(t: &TargetInfo) -> bool {
    t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE)
}

/// One candidate's adjusted distance and score from the search origin (D 7.4).
pub fn score_of(origin: Vec3, forward: Vec3, t: &TargetInfo, auto_range: f32) -> (f32, f32, f32) {
    let d = t.center - origin;
    let d3 = d.length();
    let mut dist = d3;
    if auto_range > 0.0 {
        dist = d3 - (auto_range - AUTO_LIMIT);
        if dist < AUTO_NEAR {
            dist = d3 * 0.01 + AUTO_NEAR;
        }
    }
    let flat = Vec3::new(d.x, d.y, 0.0);
    let dir = if flat.length_squared() < 1.0e-8 { Vec3::ZERO } else { flat.normalize() };
    // 0.8 dead ahead, 1.8 abeam, 2.8 behind.
    let factor = 1.8 - forward.dot(dir);
    (d3, dist, factor * dist)
}

fn set_cursor(cx: &mut Ctx, target: TargetId, kind: Option<CursorKind>) {
    let lock = &mut cx.pawn.combat.lockon;
    let wanted = kind.filter(|_| target != TargetId::NONE).map(|k| (target, k));
    if lock.cursor == wanted {
        return;
    }
    if let Some((old, _)) = lock.cursor.take() {
        cx.pawn.event(EventKind::LockCursor { target: old, kind: None });
    }
    if let Some((target, kind)) = wanted {
        cx.pawn.combat.lockon.cursor = Some((target, kind));
        cx.pawn.event(EventKind::LockCursor { target, kind: Some(kind) });
    }
}

/// `Unlock` (slot 752 0x1413FA580; D 7.8): drops the lock, the soft target and the pin.
pub(crate) fn unlock(cx: &mut Ctx) {
    set_cursor(cx, TargetId::NONE, None);
    let lock = &mut cx.pawn.combat.lockon;
    let was = lock.locked;
    (lock.current, lock.attack_target, lock.locked, lock.switch_index) = (TargetId::NONE, TargetId::NONE, false, 0);
    if was {
        cx.pawn.event(EventKind::CameraLock { target: TargetId::NONE });
    }
}

/// The controller's part C of the frame up to the attack requests (D 7.3): the two tap timers,
/// then `UpdateLockon`.
pub(crate) fn update(cx: &mut Ctx, dt: f32) {
    let pad = cx.pawn.input.pad;
    let (rb, rt) = (pad.held.has(PadButton::RightShoulder), pad.held.has(PadButton::RightTrigger));
    {
        // Slots 712 / 713 (D 7.2): a tap is a release after at most 0.2667 s.
        let lock = &mut cx.pawn.combat.lockon;
        lock.rb_tap = false;
        if rb {
            lock.rb_time += dt;
        } else if lock.rb_time > 0.0 {
            lock.rb_tap = lock.rb_time <= TAP_TIME;
            lock.rb_time = 0.0;
        }
        lock.rt_tap = false;
        if rt {
            lock.rt_time += dt;
        } else if lock.rt_time > 0.0 {
            lock.rt_tap = lock.rt_time <= TAP_TIME;
            lock.rt_time = 0.0;
        }
    }
    // ---- the search loop (D 7.4)
    let root = cx.pawn.cmc.location;
    let yaw = cx.pawn.yaw.to_radians();
    let forward = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
    let eye = root + Vec3::Z * ORIGIN_UP;
    let origin = eye + forward * ORIGIN_PUSH;
    let mut targets = Vec::new();
    cx.targets(&mut targets);
    let locked = cx.pawn.combat.lockon.locked;
    // (F-ARROWGUN) listMode = ctrl+0x9C4 (multi-lock) || locked || ctrl+0x968 (shooter aim) (D 7.4).
    let list_mode = locked || cx.pawn.combat.arrowgun.aim.multilock || cx.pawn.combat.arrowgun.aim.shooter;
    // (F-STAFF) controller+0x970.
    let soft_range = cx.pawn.combat.lockon.soft_range;
    let (mut best, mut best_score, mut best_dist, mut best_priority) = (TargetId::NONE, MANUAL_LIMIT + 5000.0, MANUAL_LIMIT + 5000.0, 0u8);
    let mut count = 0;
    let mut list: Vec<(f32, TargetId)> = Vec::new();
    for t in &targets {
        if !targetable(t) {
            continue;
        }
        let (d3, dist, score) = score_of(origin, forward, t, cx.host.targets.lock_on_range(t.id));
        // The crowd count: the raw 3D distance, before every later test.
        if d3 < CROWD_RADIUS {
            count += 1;
        }
        if !t.flags.has(TargetFlags::ENEMY) && dist > soft_range * 0.5 {
            continue;
        }
        let priority = t.flags.has(TargetFlags::PRIORITY_HIGH) as u8;
        if priority < best_priority {
            continue;
        }
        if score < best_score || (priority > best_priority && dist < soft_range) {
            // The line of sight is tested only for a would-be best (D 7.4.1).
            if cx.world.line_trace(eye, t.center).is_some() {
                continue;
            }
            (best, best_score, best_dist, best_priority) = (t.id, score, dist, priority);
        }
        if list_mode && dist < SWITCH_RANGE {
            list.push((dist, t.id));
        }
    }
    let valid = |id: TargetId| id != TargetId::NONE && targets.iter().any(|t| t.id == id && targetable(t));
    {
        let lock = &mut cx.pawn.combat.lockon;
        lock.crowd = count;
        lock.candidate = if best_dist < MANUAL_LIMIT { best } else { TargetId::NONE };
        if lock.attack_target != TargetId::NONE && !valid(lock.attack_target) {
            lock.attack_target = TargetId::NONE;
        }
    }
    // (F-ARROWGUN) ctrl+0x9E8: the switch list as the loop built it (empty outside listMode).
    cx.pawn.combat.arrowgun.aim.switch_list = list.iter().map(|e| e.1).collect();
    // (F-ARROWGUN) D 7.5 step 5: the shooter aim (slot 715 0x141406F80): the screen-centre pick is the current target.
    if cx.pawn.combat.arrowgun.aim.shooter {
        let (pick, found, crowd) = crate::combat::forms::arrowgun::aim::shooter_pick(cx, &targets);
        cx.pawn.combat.lockon.crowd += crowd;
        set_cursor(cx, pick, Some(CursorKind::Lock));
        cx.pawn.combat.lockon.current = pick;
        let aim = &mut cx.pawn.combat.arrowgun.aim;
        (aim.pick, aim.on_screen) = (pick, found);
        return;
    }
    let lock = cx.pawn.combat.lockon;
    let mut cur = lock.current;
    if lock.locked {
        // ---- while locked (D 7.7)
        let mut keep = valid(cur);
        if keep {
            let t = targets.iter().find(|t| t.id == cur).copied().unwrap_or_else(|| unreachable!());
            let mut d = (origin - t.center).length();
            let range = cx.host.targets.lock_on_range(cur);
            if range > 0.0 {
                d = (d - range).max(AUTO_NEAR);
            }
            keep = !lock.rb_tap && d <= MANUAL_OFF;
        }
        if !keep {
            unlock(cx);
            // (F-ARROWGUN) 0x141406EB4: in 0x80 / 0x8E the multi-lock (slot 790).
            crate::combat::forms::arrowgun::aim::on_unlock(cx);
            return;
        }
        if lock.rt_tap && !list.is_empty() {
            // The switch: nearest first, at most 6, by a running index.
            list.sort_by(|a, b| a.0.total_cmp(&b.0));
            list.truncate(SWITCH_MAX);
            // (F-ARROWGUN) sorted in place (0x1413E60E0 / 0x1414156A0 on ctrl+0x9E8).
            cx.pawn.combat.arrowgun.aim.switch_list = list.iter().map(|e| e.1).collect();
            let mut index = lock.switch_index + 1;
            if index >= list.len() as i32 {
                index = 0;
            }
            cx.pawn.combat.lockon.switch_index = index;
            let next = list[index as usize].1;
            if next != cur {
                cur = next;
                cx.pawn.combat.lockon.current = cur;
                set_cursor(cx, cur, Some(CursorKind::Lock));
                cx.pawn.event(EventKind::CameraLock { target: cur });
            }
        }
        return;
    }
    // (F-ARROWGUN) D 7.5 step 9: the multi-lock mode (0x141406894): the list nearest first, cut to 6, a type-0 marker on each
    // entry; RB tap (slot 711): slot 791(1) and lock list[0].
    if cx.pawn.combat.arrowgun.aim.multilock {
        list.sort_by(|a, b| a.0.total_cmp(&b.0));
        list.truncate(SWITCH_MAX);
        cx.pawn.combat.arrowgun.aim.switch_list = list.iter().map(|e| e.1).collect();
        crate::combat::forms::arrowgun::aim::show_markers(cx);
        if let Some(first) = list.first().map(|e| e.1).filter(|id| valid(*id) && lock.rb_tap) {
            crate::combat::forms::arrowgun::aim::multilock_end(cx);
            let l = &mut cx.pawn.combat.lockon;
            (l.locked, l.switch_index, l.current) = (true, 0, first);
            set_cursor(cx, first, Some(CursorKind::Lock));
            cx.pawn.event(EventKind::CameraLock { target: first });
        }
        return;
    }
    // ---- the normal path: soft target, pinning, locking on (D 7.6)
    if !valid(lock.attack_target) && cur != best {
        cur = best;
    }
    if cur != TargetId::NONE && !valid(cur) {
        cur = TargetId::NONE;
    }
    if cur != TargetId::NONE {
        if lock.rb_tap && best_dist < MANUAL_LIMIT {
            let l = &mut cx.pawn.combat.lockon;
            (l.locked, l.switch_index, l.current) = (true, 0, cur);
            set_cursor(cx, cur, Some(CursorKind::Lock));
            cx.pawn.event(EventKind::CameraLock { target: cur });
            return;
        }
        if best_dist > cx.pawn.combat.lockon.soft_range {
            // Nothing in soft range: the target and the pin go.
            cur = TargetId::NONE;
            cx.pawn.combat.lockon.attack_target = TargetId::NONE;
        }
    }
    cx.pawn.combat.lockon.current = cur;
    set_cursor(cx, cur, Some(CursorKind::Auto));
}
