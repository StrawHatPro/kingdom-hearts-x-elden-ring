//! (GF-INGAME) THE TEST PILOT: presses the leader's pad from the telegraph list, so a real enemy's real attack can be answered on
//! time, again and again, with nobody at the pad (the same scripted pad as `in.script`). OFF unless a test key asks; only ever
//! written into the test profile's ini.
//!
//! `test.auto = right` (the right answer to each class) | `tap` (a plain tap for all) | `guard` (X held) | `late` (a tap after the
//! window, to see a hit land). `test.auto_lead = s`: how long before a window it taps (default 0.06). (THE USER 2026-11-02: "Remove
//! countering with 'A' after a parry technique": the pilot's A into a counter went with the counters.) `test.auto_chase = 1`: the stick walks him to the nearest hostile (stops at 2 m); a larger number: to that target id; `-NNNN`: to the nearest of that chr id. `test.auto_head = x,z`:
//! with nobody to chase, the stick walks him that way across the world. `test.auto_y = s`: Y this long after a Breaker is offered. Every action is a line of the log (`auto:`).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use glam::Vec2;
use kh_core::host::AttackClass;
use kh_core::{PadButton, PadButtons, TargetId};

use crate::control::Control;
use crate::erlog;
use crate::script::Seg;
use crate::targets::ErTargets;
use crate::telegraph::Seen;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Dir {
    None,
    Toward(TargetId),
    Away(TargetId),
    Side(TargetId),
}

#[derive(Clone, Copy, Debug)]
struct Act {
    frames: u32,
    dir: Dir,
    buttons: PadButtons,
}

#[derive(Default)]
struct Auto {
    plan: VecDeque<Act>,
    /// Seconds until the next attack is looked at.
    rest: f32,
    /// Seconds a Breaker has been offered; Y was pressed for it.
    offered: f32,
    pressed_y: bool,
}

static AUTO: Mutex<Option<Auto>> = Mutex::new(None);
/// (the party's route) A Breaker is offered to the leader.
static BREAKER_OFFERED: AtomicBool = AtomicBool::new(false);

/// (the party's route) What the pilot reads of the leader's kit: Goofy Breaker on offer.
#[cfg(feature = "party")]
pub fn note_kit(breaker_offered: bool) {
    BREAKER_OFFERED.store(breaker_offered, Ordering::Relaxed);
}

fn pad(list: &[PadButton]) -> PadButtons {
    let mut b = PadButtons::NONE;
    for k in list {
        b.set(*k, true);
    }
    b
}

fn act(frames: u32, dir: Dir, buttons: &[PadButton]) -> Act {
    Act { frames, dir, buttons: pad(buttons) }
}

/// The answer to one attack, as a list of pad steps.
fn plan_for(mode: &str, s: &Seen, lead: f32) -> Option<(f32, Vec<Act>)> {
    let x = PadButton::FaceLeft;
    let who = s.attacker;
    let tap = |dir| vec![act(2, dir, &[]), act(3, dir, &[x]), act(4, dir, &[])];
    if mode == "tap" {
        return Some((lead, tap(Dir::None)));
    }
    if mode == "late" {
        return Some((-0.35, tap(Dir::None)));
    }
    if mode != "right" {
        return None;
    }
    Some(match s.class {
        AttackClass::Normal => (lead, tap(Dir::None)),
        AttackClass::Thrust => (lead, tap(Dir::Toward(who))),
        AttackClass::Projectile => (lead, tap(Dir::Toward(who))),
        AttackClass::Sweep => (0.28, vec![act(8, Dir::None, &[PadButton::FaceRight]), act(30, Dir::None, &[])]),
        AttackClass::Grab => (lead, vec![act(2, Dir::Side(who), &[]), act(4, Dir::Side(who), &[x]), act(30, Dir::None, &[])]),
        AttackClass::Magic => (0.75, vec![act(37, Dir::None, &[x]), act(2, Dir::None, &[]), act(4, Dir::None, &[x]), act(12, Dir::None, &[])]),
        AttackClass::Heavy => (0.7, vec![act(4, Dir::None, &[x]), act(66, Dir::Away(who), &[x]), act(4, Dir::None, &[])]),
    })
}

/// The stick of a step, for a leader at `me` (cm) under the camera's yaw.
fn stick(dir: Dir, me: glam::Vec3, camera_yaw: f32, targets: &ErTargets) -> Vec2 {
    let (who, turn) = match dir {
        Dir::None => return Vec2::ZERO,
        Dir::Toward(t) => (t, 0.0),
        Dir::Away(t) => (t, 180.0),
        Dir::Side(t) => (t, 90.0),
    };
    let Some(e) = targets.get(who) else { return Vec2::ZERO };
    let to = e.pos - me;
    let yaw = to.y.atan2(to.x).to_degrees() + turn;
    let a = (yaw - camera_yaw).to_radians();
    Vec2::new(a.sin(), a.cos())
}

/// One frame: the pad the pilot holds, while a test key asks for it.
pub fn step(control: &Control, me: glam::Vec3, camera_yaw: f32, targets: &ErTargets, dt: f32) -> Option<Seg> {
    let mode = control.str("test.auto")?.trim().to_string();
    if mode.is_empty() || mode == "0" {
        return None;
    }
    let mut guard = AUTO.lock().unwrap_or_else(|e| e.into_inner());
    let a = guard.get_or_insert_with(Auto::default);
    a.rest = (a.rest - dt).max(0.0);
    let lead = control.f32("test.auto_lead").unwrap_or(0.06);
    if mode == "guard" {
        return Some(Seg { steps: 1, left: Vec2::ZERO, right: Vec2::ZERO, buttons: pad(&[PadButton::FaceLeft]) });
    }
    if mode == "mash_x" || mode == "mash_a" {
        a.offered += dt;
        let on = ((a.offered * 8.0) as u32).is_multiple_of(2);
        let key = if mode == "mash_x" { PadButton::FaceLeft } else { PadButton::FaceBottom };
        return Some(Seg { steps: 1, left: Vec2::ZERO, right: Vec2::ZERO, buttons: if on { pad(&[key]) } else { PadButtons::NONE } });
    }
    // Y into a Breaker
    if BREAKER_OFFERED.load(Ordering::Relaxed) {
        a.offered += dt;
        if let Some(at) = control.f32("test.auto_y").filter(|at| !a.pressed_y && a.offered >= *at) {
            a.pressed_y = true;
            a.plan.push_front(act(4, Dir::None, &[PadButton::FaceTop]));
            erlog!("auto: Y into the Breaker, {:.2} s after it was offered (asked {at:.2})", a.offered);
        }
    } else {
        a.offered = 0.0;
        a.pressed_y = false;
    }
    if a.plan.is_empty() && a.rest <= 0.0 {
        let list = crate::telegraph::list();
        let near = |s: &&Seen| s.flight || targets.get(s.attacker).is_some_and(|e| (e.pos - me).length() <= 900.0);
        if let Some(s) = list.iter().filter(near).min_by(|x, y| x.eta.total_cmp(&y.eta)) {
            if let Some((at, plan)) = plan_for(&mode, s, lead) {
                let due = if at < 0.0 { s.active } else { s.eta <= at };
                // (a shot from afar is answered in flight; next to the shooter it lands the frame it appears)
                let close = targets.get(s.attacker).is_some_and(|e| (e.pos - me).length() <= 350.0);
                if due && (s.flight || close || s.class != AttackClass::Projectile || mode != "right") {
                    erlog!("auto: {:?} perilous {} from t{} eta {:.2} active {} flight {}: {mode}", s.class, s.perilous, s.attacker.0, s.eta, s.active, s.flight);
                    a.plan.extend(plan);
                    a.rest = 0.5;
                }
            }
        }
    }
    if a.plan.is_empty() {
        if let Some(chase) = control.i64("test.auto_chase").filter(|c| *c != 0) {
            let near = targets.hostiles_by_distance();
            let want = match chase {
                1 => near.first(),
                c if c < 0 => near.iter().find(|(_, e)| i64::from(e.npc_id) == -c),
                c => near.iter().find(|(_, e)| i64::from(e.id.0) == c),
            };
            if let Some((_, e)) = want.filter(|(d, _)| *d > 200.0 && *d < 6000.0) {
                return Some(Seg { steps: 1, left: stick(Dir::Toward(e.id), me, camera_yaw, targets), right: Vec2::ZERO, buttons: PadButtons::NONE });
            }
        }
        if let Some((x, z)) = control.str("test.auto_head").and_then(|t| t.split_once(',')).and_then(|(x, z)| Some((x.trim().parse::<f32>().ok()?, z.trim().parse::<f32>().ok()?))) {
            let a = (z.atan2(x).to_degrees() - camera_yaw).to_radians();
            return Some(Seg { steps: 1, left: Vec2::new(a.sin(), a.cos()), right: Vec2::ZERO, buttons: PadButtons::NONE });
        }
    }
    let next = a.plan.front_mut()?;
    next.frames = next.frames.saturating_sub(1);
    let seg = Seg { steps: 1, left: stick(next.dir, me, camera_yaw, targets), right: Vec2::ZERO, buttons: next.buttons };
    if next.frames == 0 {
        a.plan.pop_front();
    }
    Some(seg)
}
