//! (DON-KIT) DONALD'S MISSILE COMBO (documentation/DONALD_OVERHAUL.md 3.1 - 3.3, 4.2, 5.3, 9; plan DON_PLAN.md 3 / 4.1;
//! the pattern: GK_GOOFY_DESIGN.md 2 - 3, pad/goofy_combo.rs). The user (2026-10-22): "He needs to play just as the doc
//! specifies".
//!
//! - GROUND (A, A, A): wave LEFT (M_1301) -> wave RIGHT (M_1302) -> [Combo Plus / Combo Plus 2: the waves again] -> the
//!   SLAM (M_1311 on the ground, R1). Each wave fires one homing missile at its old hit time; the slam bursts 2 missiles
//!   at x0.75. A press during a step is queued and runs at the step's LINK point; the stick takes him out at the MOVE
//!   point; every step CANCELS into a spell, a shortcut or a dodge once its missile has left the wand
//!   ([`cancel_open`]).
//! - THE SLAM by what he wears and is attuned to (DON-KIT reading, the doc gives both without a rule between them): a
//!   FINISHER ABILITY replaces it (Missile Volley: 5 x 0.5; Arcane Bolt: one big piercing missile; both worn: Missile
//!   Volley with another enemy near the target, else Arcane Bolt), its missiles carrying his element; else the
//!   ATTUNED ELEMENT'S SLAM (4.2: Fire 3 fireballs, Blizzard an ice lance, Thunder a bolt from above, Water a wave,
//!   Aero a tornado, Light a healing pulse); else the plain 2 x 0.75.
//! - AIR (A in the air): two downward missiles (Air Combo Plus +1), then a 0.5 s float; Starfall (ability) ends it with
//!   a rain of missiles. No air finisher otherwise.
//! - HOLD A (R24): a press still held at the step's shot holds the pose; at 0.8 s (0.5 with Charged Missile+) the step
//!   becomes the CHARGE (its missile is not fired, he walks slowly); the release fires the Charged Missile: x2.5,
//!   pierces, its attunement effect doubled, Shatters Frozen (`HitTag::breaker`), no MP.
//! - WAND BONK (5.3): A with an enemy within 2 m swings the staff: weak, knocks it back 3 m (Wand Bonk+ more), not a
//!   combo step.
//!
//! The clips are copies without their melee Attack / Traction windows and without root motion (the hits are the
//! missiles); their voices, swing SEs and effects play as in KH3.

use super::donald::{self as kit, has, note, KitEvent, AIR, BONK, CHARGE, K, STEP};
use super::donald_shots::{self as shots, Area, AreaKind, MissileKind, MissileSpec};
use crate::donald::{DonAbility, Element};
use crate::host::PartyHost;
use crate::ids::Chara;
use crate::pawn::{forward, horizontal, wrap180, yaw_of, NpcPawn, StepCtx};

use glam::{Vec2, Vec3};
use kh_core::host::{TargetId, TargetInfo};
use kh_core::input::PadButton;

/// One step of the ground string.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Step {
    #[default]
    WaveL,
    WaveR,
    /// A Combo Plus step (the waves again, faster), left / right.
    PlusL,
    PlusR,
    Slam,
}

impl Step {
    pub fn name(self) -> &'static str {
        match self {
            Step::WaveL => "wave_left",
            Step::WaveR => "wave_right",
            Step::PlusL => "plus_left",
            Step::PlusR => "plus_right",
            Step::Slam => "slam",
        }
    }
}

/// A step's clip and times (clip seconds; real time = clip time / rate).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepDef {
    pub clip: &'static str,
    pub rate: f32,
    pub shot: f32,
    pub link: f32,
    pub r#move: f32,
    /// A held press holds the step at this clip time (before the shot; his feet on the ground there: the wave clips
    /// lift a foot around their shots, the user 2026-10-22: "his feet should be planted on the ground").
    pub hold: f32,
}

pub fn def(s: Step) -> StepDef {
    match s {
        Step::WaveL => StepDef { clip: K.step_wave_l_clip, rate: K.step_wave_l_rate, shot: K.step_wave_l_shot, link: K.step_wave_l_link, r#move: K.step_wave_l_move, hold: K.step_wave_l_hold.min(K.step_wave_l_shot) },
        Step::WaveR => StepDef { clip: K.step_wave_r_clip, rate: K.step_wave_r_rate, shot: K.step_wave_r_shot, link: K.step_wave_r_link, r#move: K.step_wave_r_move, hold: K.step_wave_r_hold.min(K.step_wave_r_shot) },
        Step::PlusL => StepDef { rate: K.step_plus_rate, ..def(Step::WaveL) },
        Step::PlusR => StepDef { rate: K.step_plus_rate, ..def(Step::WaveR) },
        Step::Slam => {
            if K.slam_fallback != 0.0 {
                StepDef { clip: K.slam_fallback_clip, rate: K.slam_rate, shot: K.slam_fallback_shot, link: K.slam_fallback_shot + 0.15, r#move: K.slam_fallback_shot + 0.25, hold: K.slam_fallback_shot }
            } else {
                StepDef { clip: K.slam_clip, rate: K.slam_rate, shot: K.slam_shot, link: K.slam_link, r#move: K.slam_move, hold: K.slam_shot }
            }
        }
    }
}

/// The running string (`DonaldKit::combo`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ComboRun {
    pub step: Step,
    /// Single-missile steps done in this string (the slam comes after `regular_steps`).
    pub n: u8,
    /// An A press waits for the link point.
    pub queued: bool,
    /// The step's missile(s) left, and the state time it happened.
    pub shot: bool,
    pub shot_at: f32,
    /// A held since the step's start (the Charged Missile, R24).
    pub a_hold: f32,
    /// Air missiles fired this airtime; Starfall used.
    pub air_n: u8,
    pub starfall: bool,
    /// The air float after the last air missile: seconds left.
    pub float_left: f32,
    /// The Charged Missile: 0 holding, 1 released (to its shot), 2 fired.
    pub charge: u8,
    /// THE STRAFE / HOP between projectiles (3.1, the user 2026-10-22): the move running (`Move`), its clock, its
    /// world direction (unit), and the stick buffer (the last state time the stick was pushed, its yaw).
    pub mv: Move,
    pub mv_t: f32,
    pub mv_dir: Vec3,
    pub stick_at: f32,
    pub stick_yaw: f32,
}

/// A strafe / hop of the string (3.1): sideways (stick left / right) or a short hop forward / back, facing the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Move {
    #[default]
    None,
    Left,
    Right,
    Forward,
    Back,
}

impl Move {
    pub fn name(self) -> &'static str {
        match self {
            Move::None => "none",
            Move::Left => "strafe_left",
            Move::Right => "strafe_right",
            Move::Forward => "hop_forward",
            Move::Back => "hop_back",
        }
    }

    /// (clip, distance cm, time s)
    pub fn spec(self) -> (&'static str, f32, f32) {
        match self {
            Move::Left => (K.strafe_side_clip_l, K.strafe_side_distance, K.strafe_side_time),
            Move::Right => (K.strafe_side_clip_r, K.strafe_side_distance, K.strafe_side_time),
            Move::Forward => (K.strafe_fwd_clip, K.strafe_fwd_distance, K.strafe_fwd_time),
            Move::Back => (K.strafe_back_clip, K.strafe_back_distance, K.strafe_back_time),
            Move::None => ("", 0.0, 0.0),
        }
    }
}

/// Which move a stick yaw makes, his facing `facing` (the target's way): forward / back within `strafe.side_angle`,
/// else a strafe (yaw grows to his right, the stick right = +90 from the camera; a negative offset is his left).
pub fn move_of(stick_yaw: f32, facing: f32) -> (Move, Vec3) {
    let off = wrap180(stick_yaw - facing);
    if off.abs() <= K.strafe_side_angle {
        (Move::Forward, forward(facing))
    } else if off.abs() >= 180.0 - K.strafe_side_angle {
        (Move::Back, -forward(facing))
    } else if off < 0.0 {
        (Move::Left, forward(facing - 90.0))
    } else {
        (Move::Right, forward(facing + 90.0))
    }
}

fn cr(p: &NpcPawn) -> ComboRun {
    kit::kit_ref(p).map(|k| k.combo).unwrap_or_default()
}

fn crm(p: &mut NpcPawn) -> Option<&mut ComboRun> {
    kit::kit(p).map(|k| &mut k.combo)
}

/// Single-missile steps before the slam (base 2, Combo Plus / Combo Plus 2 one more each: 3 -> 4 -> 5 steps).
pub fn regular_steps(p: &NpcPawn) -> u8 {
    K.combo_base_steps as u8 + has(p, DonAbility::ComboPlus) as u8 + has(p, DonAbility::ComboPlus2) as u8
}

/// Air missiles per airtime (Air Combo Plus +1).
pub fn air_steps(p: &NpcPawn) -> u8 {
    K.air_count as u8 + has(p, DonAbility::AirComboPlus) as u8
}

/// The charge time (Charged Missile+ 0.5 s).
pub fn charge_hold(p: &NpcPawn) -> f32 {
    if has(p, DonAbility::ChargedMissilePlus) {
        K.charge_hold_plus
    } else {
        K.charge_hold
    }
}

/// THE CANCEL POINT (3.1): a combo step's missile has left the wand (+ `feel.cancel_after_shot`), or a Bonk's link.
pub fn cancel_open(p: &NpcPawn) -> bool {
    match kit::kit_state(p) {
        Some(STEP) | Some(AIR) => {
            let c = cr(p);
            c.shot && p.state_time >= c.shot_at + K.feel_cancel_after_shot
        }
        Some(BONK) => p.anim.time >= K.bonk_link,
        Some(CHARGE) => cr(p).charge >= 2,
        _ => false,
    }
}

/// A copy of one of his clips without its melee Attack / Traction windows and (optionally) its root motion.
pub fn clip_copy(name: &'static str, drop_root: bool) -> &'static crate::anim::ClipDef {
    use std::sync::Mutex;
    static CLIPS: Mutex<Vec<(&'static str, bool, &'static crate::anim::ClipDef)>> = Mutex::new(Vec::new());
    let mut all = CLIPS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = all.iter().find(|c| c.0 == name && c.1 == drop_root) {
        return c.2;
    }
    let base = crate::tables::clip(Chara::Donald, name);
    let notifies: Vec<crate::anim::ClipNotify> = base.notifies.iter().copied().filter(|n| !(matches!(n.data, crate::anim::ClipNotifyData::Attack { .. }) || n.class.contains("Traction") || n.class.ends_with("_Shot") || n.class.ends_with("_ThrowWeapon"))).collect();
    let notifies: &'static [crate::anim::ClipNotify] = Box::leak(notifies.into_boxed_slice());
    let c: &'static crate::anim::ClipDef = Box::leak(Box::new(crate::anim::ClipDef { notifies, root_motion: base.root_motion && !drop_root, root: if drop_root { &[] } else { base.root }, ..*base }));
    all.push((name, drop_root, c));
    c
}

/// The wand: the clip's ATK_01 socket track (actor frame), else in front of his chest.
pub fn muzzle(p: &NpcPawn) -> Vec3 {
    let keys = p.anim.clip.socket;
    let f = forward(p.yaw);
    if !keys.is_empty() {
        let v = crate::actions::socket_track_at(keys, p.anim.time);
        let right = Vec3::new(-f.y, f.x, 0.0);
        let at = p.position() + f * v.x + right * v.y + Vec3::Z * v.z;
        if (at - p.position()).length() < 400.0 {
            return at;
        }
    }
    p.position() + f * K.missile_forward + Vec3::Z * K.missile_up
}

/// Turns him toward the target (lock, else soft), else the stick, by at most `combo.turn`.
fn face(p: &mut NpcPawn, t: Option<TargetInfo>) {
    let me = p.position();
    let want = t.map(|t| horizontal(t.position - me)).filter(|v| v.length() > 1.0).map(yaw_of).or_else(|| {
        let (y, _, a) = kit::stick(p);
        a.then_some(y)
    });
    if let Some(w) = want {
        let d = wrap180(w - p.yaw);
        p.yaw = wrap180(p.yaw + d.clamp(-K.combo_turn, K.combo_turn));
    }
}

/// The enemy the Wand Bonk swings at: the nearest within `bonk.range` of his centre (to its surface).
pub fn bonk_target(p: &NpcPawn, host: &PartyHost) -> Option<TargetInfo> {
    let me = p.position();
    shots::enemies(host).into_iter().filter(|t| horizontal(t.position - me).length() - t.radius <= K.bonk_range && (t.position.z - me.z).abs() < 250.0).min_by(|a, b| a.position.distance(me).total_cmp(&b.position.distance(me)))
}

/// A (from the pad's buffer): the next step / a queue / the Bonk / an air missile. True = it acted.
pub fn press(p: &mut NpcPawn, host: &mut PartyHost) -> bool {
    match kit::kit_state(p) {
        Some(STEP) | Some(AIR) => {
            if let Some(c) = crm(p) {
                c.queued = true;
            }
            return true;
        }
        // THE BONK PLAYS OUT (the user 2026-10-22: "Donald shouldn't be able to spam wand bonk even without a target, he
        // can only use it again after the animation"): a press that would Bonk again is ignored (not kept) until its
        // clip has ended; one that starts the missile string waits for the link point as before
        Some(BONK) => {
            if !bonk_ready(p) && would_bonk(p, host) {
                return true;
            }
            if p.anim.time < K.bonk_link || kit::kit_ref(p).is_some_and(|k| k.run.sub == 1) {
                if let Some(c) = crm(p) {
                    c.queued = true;
                }
                return true;
            }
        }
        Some(CHARGE) => return true,
        _ => {}
    }
    // out of the Bonk early (the stick at its move point, a dodge) a press that would Bonk waits for its clip's end
    if !bonk_ready(p) && would_bonk(p, host) {
        return true;
    }
    if kit::kit_state(p).is_some() {
        kit::end_kit(p, "attack");
    }
    // THE USER (2026-10-22): "when Donald doesn't have a target, either soft locked or locked on, then he should just do a
    // wand bonk" (doc 3.1 / 5.3 updated)
    let no_target = kit::target_info(p, host).is_none();
    if p.on_ground() {
        if let Some(t) = bonk_target(p, host) {
            start_bonk(p, Some(t));
            return true;
        }
        if no_target {
            start_bonk(p, None);
            return true;
        }
        if let Some(c) = crm(p) {
            c.n = 0;
        }
        start_step(p, host, Step::WaveL);
        return true;
    }
    let c = cr(p);
    // in the air with no target: the air Wand Bonk (his air staff swing, M_1312), as many as the air string's steps
    if no_target {
        if c.air_n < air_steps(p) {
            start_bonk(p, None);
            if let Some(c) = crm(p) {
                c.air_n += 1;
            }
            return true;
        }
        return false;
    }
    if c.air_n < air_steps(p) {
        start_air(p, host, false);
        return true;
    }
    if c.air_n >= air_steps(p) && !c.starfall && has(p, DonAbility::Starfall) {
        start_air(p, host, true);
        return true;
    }
    false
}

/// The last Wand Bonk's clip has ended (its time from its start at its play rate).
pub fn bonk_ready(p: &NpcPawn) -> bool {
    kit::kit_ref(p).map_or(true, |k| k.clock >= k.bonk_ready_at - 1e-4)
}

/// A press now would be a Wand Bonk: an enemy within reach on the ground, or no target at all (ground or air).
fn would_bonk(p: &NpcPawn, host: &PartyHost) -> bool {
    kit::target_info(p, host).is_none() || (p.on_ground() && bonk_target(p, host).is_some())
}

/// Starts a ground step.
pub fn start_step(p: &mut NpcPawn, host: &mut PartyHost, s: Step) {
    let d = def(s);
    let t = kit::target_info(p, host);
    kit::enter_kit(p, STEP);
    p.cmc.stop_movement();
    face(p, t);
    p.anim.play(clip_copy(d.clip, true), K.combo_blend);
    p.anim.speed = d.rate;
    p.hit_list.clear();
    let a = kit::held(p, PadButton::ATTACK);
    if let Some(c) = crm(p) {
        c.step = s;
        c.queued = false;
        c.shot = false;
        c.a_hold = if a { 0.0 } else { -1.0 };
        c.charge = 0;
        c.mv = Move::None;
        c.mv_t = 0.0;
        c.stick_at = -1.0;
        if s != Step::Slam {
            c.n += 1;
        }
    }
    note(p, KitEvent::Started(s.name()));
}

/// The next step after a link (the queued press).
fn next(p: &mut NpcPawn, host: &mut PartyHost) {
    let c = cr(p);
    if c.step == Step::Slam {
        if let Some(c) = crm(p) {
            c.n = 0;
        }
        if let Some(t) = bonk_target(p, host) {
            start_bonk(p, Some(t));
            return;
        }
        if kit::target_info(p, host).is_none() {
            start_bonk(p, None); // no target: the Wand Bonk (the user's rule)
            return;
        }
        start_step(p, host, Step::WaveL);
        return;
    }
    // the target is gone mid-string: the Wand Bonk (the user's rule)
    if kit::target_info(p, host).is_none() {
        if let Some(c) = crm(p) {
            c.n = 0;
        }
        start_bonk(p, None);
        return;
    }
    if c.n >= regular_steps(p) {
        start_step(p, host, Step::Slam);
        return;
    }
    let s = match (c.n, c.n >= K.combo_base_steps as u8) {
        (n, false) if n % 2 == 1 => Step::WaveR,
        (_, false) => Step::WaveL,
        (n, true) if n % 2 == 1 => Step::PlusR,
        _ => Step::PlusL,
    };
    start_step(p, host, s);
}

/// The step's / the air missile's / the charge's / the Bonk's MainProc.
pub fn tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    if kit::held(p, PadButton::ATTACK) {
        if let Some(c) = crm(p) {
            if c.a_hold >= 0.0 {
                c.a_hold += dt;
            }
        }
    } else if let Some(c) = crm(p) {
        c.a_hold = -1.0;
    }
    if let Some(k) = kit::kit(p) {
        k.run.t += dt;
    }
    match kit::kit_state(p) {
        Some(STEP) => step_tick(p, host, dt),
        Some(AIR) => air_tick(p, host, dt),
        Some(CHARGE) => charge_tick(p, host, dt),
        Some(BONK) => bonk_tick(p, host),
        _ => {}
    }
}

fn step_tick(p: &mut NpcPawn, host: &mut PartyHost, dt: f32) {
    let c = cr(p);
    let d = def(c.step);
    if c.mv != Move::None {
        move_tick(p, host, dt);
        return;
    }
    // the stick's buffer for the strafe / hop window (3.1)
    let (s_yaw, _, s_active) = kit::stick(p);
    if s_active {
        let st = p.state_time;
        if let Some(c) = crm(p) {
            c.stick_at = st;
            c.stick_yaw = s_yaw;
        }
    }
    // back from the walk of a held press (the user: never still while A is held): the step's held pose
    if p.anim.clip.name != d.clip && !c.shot {
        p.anim.play(clip_copy(d.clip, true), K.charge_walk_blend);
        p.anim.time = d.hold;
    }
    p.anim.speed = d.rate;
    let t = p.anim.time;
    p.cmc.direct_xy = Some(Vec2::ZERO);
    // R24: a press still held at the step's hold frame (feet planted, just before the shot) holds the pose until it is
    // let go (the clip plays on to the shot: the missile) or charged (the charge); the stick walks him slowly meanwhile,
    // facing his target (3.3, the user 2026-10-22)
    if !c.shot && c.step != Step::Slam && c.a_hold >= 0.0 && t >= d.hold {
        if c.a_hold >= charge_hold(p) {
            start_charge(p, host);
            return;
        }
        hold_walk(p, host, dt, d.clip, d.hold);
        return;
    }
    if !c.shot && t >= d.shot {
        if c.step == Step::Slam {
            slam(p, host);
        } else {
            fire(p, host, MissileKind::Step, 1.0, None);
        }
        let st = p.state_time;
        if let Some(c) = crm(p) {
            c.shot = true;
            c.shot_at = st;
        }
    }
    let c = cr(p);
    // THE STRAFE / HOP (3.1, the user 2026-10-22): the stick held (or pushed within the buffer) once the missile has
    // left: a strafe or a short hop, facing the target; never in the slam step (he stands still for it)
    if c.step != Step::Slam && c.shot && p.state_time >= c.shot_at + K.strafe_after_shot && c.stick_at >= 0.0 && p.state_time - c.stick_at <= K.strafe_buffer {
        start_move(p, host, c.stick_yaw);
        return;
    }
    if c.queued && c.shot && t >= d.link {
        if let Some(c) = crm(p) {
            c.queued = false;
        }
        next(p, host);
        return;
    }
    let (yaw, mag, active) = kit::stick(p);
    if c.shot && t >= d.r#move && active && !c.queued {
        end(p);
        p.to_idle();
        super::enter_run(p, yaw, mag, false);
        if let Some(dd) = p.pad.as_mut() {
            dd.run.sub = 0;
        }
        return;
    }
    if !p.on_ground() && p.state_time > 0.05 {
        end(p);
        super::enter_jump(p, true);
        return;
    }
    if p.anim.finished {
        end(p);
        if let Some(c) = crm(p) {
            c.n = 0;
        }
        kit::back_to_pad(p);
    }
}

/// The aim of a missile: the target's centre, else ahead of him (the air ones pitched down).
fn aim_of(p: &NpcPawn, host: &PartyHost, origin: Vec3, air: bool) -> (Vec3, TargetId, Option<Vec3>) {
    if let Some(t) = kit::target_info(p, host) {
        return ((t.center - origin).normalize_or_zero(), t.id, None);
    }
    let f = forward(p.yaw);
    if air {
        let (s, c) = K.air_pitch.to_radians().sin_cos();
        let dir = Vec3::new(f.x * c, f.y * c, s);
        return (dir, TargetId::NONE, Some(origin + dir * K.combo_no_target_ahead));
    }
    let point = origin + f * K.combo_no_target_ahead;
    (f, TargetId::NONE, Some(point))
}

/// Fires one missile of the step (its element: the next one of his attunement(s), 4.6).
fn fire(p: &mut NpcPawn, host: &mut PartyHost, kind: MissileKind, mult: f32, yaw_off: Option<f32>) {
    let e = kit::next_missile_element(p);
    fire_elem(p, host, kind, mult, yaw_off, e, |s| s);
}

fn fire_elem(p: &mut NpcPawn, host: &mut PartyHost, kind: MissileKind, mult: f32, yaw_off: Option<f32>, e: Option<Element>, tune: impl FnOnce(MissileSpec) -> MissileSpec) {
    let origin = muzzle(p);
    let air = kind == MissileKind::Air;
    let (dir, target, aim) = aim_of(p, host, origin, air);
    let dir = match yaw_off {
        Some(off) => {
            let (s, c) = off.to_radians().sin_cos();
            Vec3::new(dir.x * c - dir.y * s, dir.x * s + dir.y * c, dir.z).normalize_or_zero()
        }
        None => dir,
    };
    let mut spec = MissileSpec::new(kind, e, origin, dir, target);
    spec.mult = mult;
    spec.aim = aim;
    spec.weaver = kit::weaver(p);
    let spec = tune(spec);
    note(p, KitEvent::Missile { kind, element: e, mult });
    shots::launch(p, host, spec);
}

/// THE SLAM (see the module text: finisher ability > the element's slam > the plain 2 x 0.75).
fn slam(p: &mut NpcPawn, host: &mut PartyHost) {
    let volley = has(p, DonAbility::MissileVolley);
    let bolt = has(p, DonAbility::ArcaneBolt);
    let pick_volley = match (volley, bolt) {
        (true, true) => crowd(p, host),
        (v, _) => v,
    };
    if pick_volley {
        let n = (K.volley_count as usize).max(1);
        for i in 0..n {
            let off = if n > 1 { -K.volley_spread * 0.5 + K.volley_spread * i as f32 / (n - 1) as f32 } else { 0.0 };
            fire(p, host, MissileKind::Slam, K.volley_mult, Some(off));
        }
        note(p, KitEvent::Started("missile_volley"));
        return;
    }
    if bolt {
        let k = shots::kind_of(K.charge_kind);
        fire(p, host, MissileKind::Bolt, K.bolt_mult, None);
        // the bolt is the last missile launched: big, piercing
        tune_last(p, |m| {
            m.spec.pierce = 3;
            m.pierce_left = 3;
            m.spec.kind_dmg = k;
        });
        note(p, KitEvent::Started("arcane_bolt"));
        return;
    }
    let e = kit::primary(p);
    let me = p.position();
    let target = kit::target_info(p, host);
    match e {
        Some(Element::Fire) => {
            let n = (K.slam_fire_count as usize).max(1);
            for i in 0..n {
                let off = if n > 1 { -K.slam_fire_spread * 0.5 + K.slam_fire_spread * i as f32 / (n - 1) as f32 } else { 0.0 };
                fire_elem(p, host, MissileKind::Slam, K.slam_fire_mult, Some(off), Some(Element::Fire), |s| s);
            }
        }
        Some(Element::Blizzard) => {
            fire_elem(p, host, MissileKind::Bolt, K.slam_blizzard_mult, None, Some(Element::Blizzard), |s| MissileSpec { pierce: 3, body: Some(K.slam_blizzard_fx), ..s });
        }
        Some(Element::Thunder) => {
            let at = target.map_or(me + forward(p.yaw) * 600.0, |t| t.position);
            let mut a = Area::new(AreaKind::Hit, at, K.slam_thunder_radius, 0.2, "DON_SLAM_THUNDER");
            (a.delay, a.power, a.element, a.fx) = (K.slam_thunder_delay, K.missile_power * K.slam_thunder_mult, Some(Element::Thunder), K.slam_thunder_fx);
            a.hit_fx = K.missile_thunder_hit_fx;
            a.hit_se = K.missile_thunder_hit_se;
            shots::add_area(p, host, a);
        }
        Some(Element::Water) => {
            let mut a = Area::new(AreaKind::Hit, me, K.slam_water_radius, 0.3, "DON_SLAM_WATER");
            (a.arc, a.power, a.element, a.push, a.fx) = (Some((p.yaw, K.slam_water_angle * 0.5)), K.missile_power * K.slam_water_mult, Some(Element::Water), K.slam_water_push, K.slam_water_fx);
            a.hit_fx = K.missile_water_hit_fx;
            a.hit_se = K.missile_water_hit_se;
            shots::add_area(p, host, a);
        }
        Some(Element::Aero) => {
            let at = target.map_or(me + forward(p.yaw) * 400.0, |t| t.position);
            let mut a = Area::new(AreaKind::Hit, at, K.slam_aero_radius, K.slam_aero_life, "DON_SLAM_AERO");
            (a.power, a.element, a.launch, a.fx) = (K.missile_power * K.slam_aero_mult, Some(Element::Aero), true, K.slam_aero_fx);
            a.hit_fx = K.missile_aero_hit_fx;
            a.hit_se = K.missile_aero_hit_se;
            shots::add_area(p, host, a);
        }
        Some(Element::Light) => {
            let mut a = Area::new(AreaKind::Heal, me, K.slam_light_radius, 0.2, "DON_SLAM_LIGHT");
            (a.heal_pct, a.fx) = (K.slam_light_heal_pct, K.slam_light_fx);
            shots::add_area(p, host, a);
            shots::sound(p, host, K.slam_light_se, me);
        }
        None => {
            let n = (K.slam_count as usize).max(1);
            for i in 0..n {
                let off = if n > 1 { -K.slam_spread * 0.5 + K.slam_spread * i as f32 / (n - 1) as f32 } else { 0.0 };
                fire(p, host, MissileKind::Slam, K.slam_mult, Some(off));
            }
        }
    }
    note(p, KitEvent::Started(match e {
        Some(Element::Fire) => "slam_fire",
        Some(Element::Blizzard) => "slam_blizzard",
        Some(Element::Thunder) => "slam_thunder",
        Some(Element::Water) => "slam_water",
        Some(Element::Aero) => "slam_aero",
        Some(Element::Light) => "slam_light",
        None => "slam_plain",
    }));
}

fn tune_last(p: &mut NpcPawn, f: impl FnOnce(&mut shots::Missile)) {
    if let Some(m) = kit::kit(p).and_then(|k| k.shots.missiles.last_mut()) {
        f(m);
    }
}

/// Both finisher abilities worn: another enemy within `volley.crowd_radius` of his target.
fn crowd(p: &NpcPawn, host: &PartyHost) -> bool {
    let Some(t) = kit::target_info(p, host) else { return false };
    shots::enemies(host).iter().any(|o| o.id != t.id && horizontal(o.position - t.position).length() <= K.volley_crowd_radius)
}

// ------------------------------------------------------------------------------------------------ the air

fn start_air(p: &mut NpcPawn, host: &mut PartyHost, starfall: bool) {
    let t = kit::target_info(p, host);
    kit::enter_kit(p, AIR);
    face(p, t);
    p.cmc.velocity = Vec3::ZERO;
    p.cmc.gravity_off = true;
    p.cmc.set_flying();
    p.cmc.stop_movement();
    let clip = if starfall { "MagicAirCast" } else { K.air_clip };
    p.anim.play(clip_copy(clip, true), K.combo_blend);
    p.anim.speed = if starfall { 1.0 } else { K.air_rate };
    if let Some(c) = crm(p) {
        c.queued = false;
        c.shot = false;
        c.float_left = 0.0;
        if starfall {
            c.starfall = true;
        } else {
            c.air_n += 1;
        }
    }
    if let Some(k) = kit::kit(p) {
        k.run.sub = starfall as u8;
    }
    note(p, KitEvent::Started(if starfall { "starfall" } else { "air_missile" }));
}

fn air_tick(p: &mut NpcPawn, host: &mut PartyHost, dt: f32) {
    let starfall = kit::kit_ref(p).is_some_and(|k| k.run.sub == 1);
    let shot_t = if starfall { 0.5 } else { K.air_shot };
    let link_t = if starfall { 0.6 } else { K.air_link };
    let t = p.anim.time;
    let c = cr(p);
    // the hover until the shot, then the float of the last missile
    if p.cmc.gravity_off {
        p.cmc.direct_z = Some(0.0);
        p.cmc.direct_xy = Some(Vec2::ZERO);
    }
    if !c.shot && t >= shot_t {
        if starfall {
            starfall_rain(p, host);
        } else {
            fire(p, host, MissileKind::Air, 1.0, None);
        }
        let st = p.state_time;
        let last = starfall || c.air_n >= air_steps(p);
        if let Some(c) = crm(p) {
            c.shot = true;
            c.shot_at = st;
            c.float_left = if last { K.air_float } else { 0.0 };
        }
    }
    let c = cr(p);
    if c.shot && c.queued && t >= link_t {
        if let Some(cc) = crm(p) {
            cc.queued = false;
        }
        if c.air_n < air_steps(p) {
            start_air(p, host, false);
            return;
        }
        if !c.starfall && has(p, DonAbility::Starfall) {
            start_air(p, host, true);
            return;
        }
    }
    if c.shot {
        if let Some(cc) = crm(p) {
            cc.float_left -= dt;
        }
        let over = cr(p).float_left <= 0.0 && t >= link_t + 0.1;
        if over || p.anim.finished {
            end(p);
            if p.on_ground() {
                kit::back_to_pad(p);
            } else {
                super::enter_jump(p, true);
            }
        }
    }
}

/// Starfall (9): a rain of missiles from above onto the point below (his target, else ahead).
fn starfall_rain(p: &mut NpcPawn, host: &mut PartyHost) {
    let me = p.position();
    let t = kit::target_info(p, host);
    let ground = t.map_or_else(|| {
        let ahead = me + forward(p.yaw) * 400.0;
        host.world.line_trace(ahead + Vec3::Z * 200.0, ahead - Vec3::Z * 3000.0).map_or(Vec3::new(ahead.x, ahead.y, me.z - 300.0), |h| h.position)
    }, |t| t.position);
    let n = (K.starfall_count as usize).max(1);
    for i in 0..n {
        let a = (i as f32 / n as f32) * std::f32::consts::TAU + 0.4;
        let r = K.starfall_radius * if i % 2 == 0 { 0.4 } else { 0.85 };
        let landing = ground + Vec3::new(a.cos() * r, a.sin() * r, 0.0);
        let origin = landing + Vec3::Z * K.starfall_height + Vec3::new(-a.cos() * 60.0, -a.sin() * 60.0, 0.0);
        let e = kit::next_missile_element(p);
        let mut spec = MissileSpec::new(MissileKind::Starfall, e, origin, (landing - origin).normalize_or_zero(), t.map_or(TargetId::NONE, |t| t.id));
        spec.mult = K.starfall_mult;
        spec.aim = Some(landing);
        spec.weaver = kit::weaver(p);
        note(p, KitEvent::Missile { kind: MissileKind::Starfall, element: e, mult: K.starfall_mult });
        shots::launch(p, host, spec);
    }
}

// ------------------------------------------------------------------------------------------------ moving while casting

/// A held A (the held step, the charge): the stick walks him at `charge.walk` with his walk animation, facing his
/// target; let go of the stick, the held cast pose (`pose_clip` at `pose_time`) again (3.3, the user 2026-10-22: "while
/// holding A he should still have some movement instead of being completely still").
pub fn hold_walk(p: &mut NpcPawn, host: &mut PartyHost, dt: f32, pose_clip: &'static str, pose_time: f32) {
    let (yaw, _, active) = kit::stick(p);
    if active {
        let f = forward(yaw) * K.charge_walk;
        p.cmc.direct_xy = Some(Vec2::new(f.x, f.y));
        // the cycle by the stick's way from his facing (he faces the target): forward his battle walk, sideways his
        // side hop slowed into a step cycle, back the battle walk played backwards (KH3 has no strafe / back walk
        // for Donald: his anim set and rt/body hold none); a change of way cross-fades over charge.walk_blend
        let (clip, rate, reverse) = walk_clip(move_of(yaw, p.yaw).0);
        if p.anim.clip.name != clip {
            p.play_looped(clip, K.charge_walk_blend);
        }
        p.anim.axis = 0.0;
        p.anim.axis_y = K.charge_walk;
        if reverse {
            // the clock run backwards by hand (the anim's own clock only runs forward)
            let len = p.anim.clip.length.max(1e-4);
            p.anim.speed = 0.0;
            p.anim.time = (p.anim.time - dt * p.anim.clip.rate * rate).rem_euclid(len);
        } else {
            p.anim.speed = rate;
        }
    } else {
        p.cmc.direct_xy = Some(Vec2::ZERO);
        if p.anim.clip.name != pose_clip {
            p.anim.play(clip_copy(pose_clip, true), K.charge_walk_blend);
        }
        p.anim.time = pose_time;
        p.anim.speed = 0.0;
    }
    if let Some(t) = kit::target_info(p, host) {
        p.turn_toward(t.position, dt, crate::pawn::turn_rate(crate::pawn::TurnKind::Attack));
    }
}

/// The walk cycle of a way (stick against his facing) while A is held: (clip, play rate, played backwards).
pub fn walk_clip(way: Move) -> (&'static str, f32, bool) {
    match way {
        Move::Left => (K.charge_walk_left_clip, K.charge_walk_side_rate, false),
        Move::Right => (K.charge_walk_right_clip, K.charge_walk_side_rate, false),
        Move::Back => (K.charge_walk_back_clip, K.charge_walk_back_rate, K.charge_walk_back_reverse != 0.0),
        Move::Forward | Move::None => (K.charge_walk_clip, 1.0, false),
    }
}

/// Starts a strafe / hop of the string from the stick's yaw, facing the target (lock, else soft; else his facing).
fn start_move(p: &mut NpcPawn, host: &mut PartyHost, stick_yaw: f32) {
    let me = p.position();
    let facing = kit::target_info(p, host).map(|t| horizontal(t.position - me)).filter(|v| v.length() > 1.0).map(yaw_of).unwrap_or(p.yaw);
    let (m, dir) = move_of(stick_yaw, facing);
    p.yaw = facing;
    let (clip, _, time) = m.spec();
    p.anim.play(clip_copy(clip, true), 0.05);
    let len = p.anim.clip.length / p.anim.clip.rate.max(1e-4);
    p.anim.speed = if time > 0.0 { (len / time).max(0.5) } else { 1.0 };
    if let Some(c) = crm(p) {
        c.mv = m;
        c.mv_t = 0.0;
        c.mv_dir = dir;
        c.stick_at = -1.0;
    }
    note(p, KitEvent::Started(m.name()));
}

/// The strafe / hop: its distance over its time (frame-rate independent: the velocity is distance / time and the
/// last step is clipped to the time), turned to keep facing the target; then the queued press goes on with the
/// string, else the stick runs, else he stands.
fn move_tick(p: &mut NpcPawn, host: &mut PartyHost, dt: f32) {
    let c = cr(p);
    let (_, dist, time) = c.mv.spec();
    let left = (time - c.mv_t).max(0.0);
    let step = dt.min(left);
    let speed = if time > 0.0 { dist / time } else { 0.0 };
    let v = if dt > 0.0 { c.mv_dir * speed * (step / dt) } else { Vec3::ZERO };
    p.cmc.direct_xy = Some(Vec2::new(v.x, v.y));
    if let Some(t) = kit::target_info(p, host) {
        p.turn_toward(t.position, dt, crate::pawn::turn_rate(crate::pawn::TurnKind::Attack));
    }
    if let Some(cc) = crm(p) {
        cc.mv_t += dt;
    }
    if !p.on_ground() && p.state_time > 0.05 {
        if let Some(cc) = crm(p) {
            cc.mv = Move::None;
        }
        end(p);
        super::enter_jump(p, true);
        return;
    }
    if cr(p).mv_t < time {
        return;
    }
    // landed: the string goes on with a press now or within `strafe.link` (the combo continues after the move)
    p.cmc.direct_xy = Some(Vec2::ZERO);
    let queued = cr(p).queued;
    if queued {
        if let Some(cc) = crm(p) {
            cc.mv = Move::None;
            cc.queued = false;
        }
        next(p, host);
        return;
    }
    if cr(p).mv_t < time + K.strafe_link {
        return;
    }
    if let Some(cc) = crm(p) {
        cc.mv = Move::None;
    }
    let (yaw, mag, active) = kit::stick(p);
    end(p);
    if let Some(cc) = crm(p) {
        cc.n = 0;
    }
    if active {
        p.to_idle();
        super::enter_run(p, yaw, mag, false);
        if let Some(dd) = p.pad.as_mut() {
            dd.run.sub = 0;
        }
    } else {
        kit::back_to_pad(p);
    }
}

// ------------------------------------------------------------------------------------------------ the charge

fn start_charge(p: &mut NpcPawn, host: &mut PartyHost) {
    let t = kit::target_info(p, host);
    kit::enter_kit(p, CHARGE);
    face(p, t);
    p.anim.play(clip_copy(K.charge_clip, true), K.combo_blend);
    p.anim.time = K.charge_pose;
    p.anim.speed = 0.0;
    if let Some(c) = crm(p) {
        c.charge = 0;
        c.shot = false;
        c.n = 0;
    }
    let at = muzzle(p);
    let fx = shots::effect(p, host, K.charge_charged_fx, at, 1.5);
    shots::sound(p, host, K.charge_charged_se, at);
    if let Some(k) = kit::kit(p) {
        k.run.fx = fx;
    }
    note(p, KitEvent::Started("charged"));
}

fn charge_tick(p: &mut NpcPawn, host: &mut PartyHost, dt: f32) {
    let c = cr(p);
    let a = kit::held(p, PadButton::ATTACK);
    match c.charge {
        0 => {
            // held: the pose, or the slow walk with his walk animation (3.3: never frozen), turned to the target
            hold_walk(p, host, dt, K.charge_clip, K.charge_pose);
            if !a {
                if p.anim.clip.name != K.charge_clip {
                    p.anim.play(clip_copy(K.charge_clip, true), K.charge_walk_blend);
                    p.anim.time = K.charge_pose;
                }
                p.anim.speed = 1.0;
                p.cmc.direct_xy = Some(Vec2::ZERO);
                if let Some(c) = crm(p) {
                    c.charge = 1;
                }
            }
        }
        1 => {
            p.anim.speed = 1.0;
            if p.anim.time >= K.charge_shot {
                let e = kit::next_missile_element(p);
                let k = shots::kind_of(K.charge_kind);
                fire_elem(p, host, MissileKind::Charged, K.charge_mult, None, e, |s| MissileSpec { pierce: 3, breaker: true, double: true, scale: K.charge_scale, kind_dmg: k, ..s });
                let st = p.state_time;
                if let Some(c) = crm(p) {
                    c.charge = 2;
                    c.shot = true;
                    c.shot_at = st;
                }
            }
        }
        _ => {
            let st = p.state_time - cr(p).shot_at;
            let (yaw, mag, active) = kit::stick(p);
            if st >= K.charge_recover && active {
                end(p);
                p.to_idle();
                super::enter_run(p, yaw, mag, false);
                return;
            }
            if st >= K.charge_recover + 0.3 || p.anim.finished {
                end(p);
                kit::back_to_pad(p);
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------ Wand Bonk

/// THE WAND BONK: on an enemy within 2 m (5.3), or with no target at all (the user's rule); in the air his air staff
/// swing with a short hover.
fn start_bonk(p: &mut NpcPawn, t: Option<TargetInfo>) {
    let air = !p.on_ground();
    kit::enter_kit(p, BONK);
    p.cmc.stop_movement();
    face(p, t);
    if air {
        p.cmc.velocity = Vec3::ZERO;
        p.cmc.gravity_off = true;
        p.cmc.set_flying();
    }
    p.anim.play(clip_copy(if air { K.bonk_air_clip } else { K.bonk_clip }, true), K.combo_blend);
    p.anim.speed = if air { K.bonk_air_rate } else { K.bonk_rate };
    // the next Bonk only once this clip has played out
    let len = p.anim.clip.length / (p.anim.clip.rate * p.anim.speed).max(1e-4);
    if let Some(k) = kit::kit(p) {
        k.bonk_ready_at = k.clock + len;
    }
    if let Some(c) = crm(p) {
        c.queued = false;
        c.shot = false;
    }
    if let Some(k) = kit::kit(p) {
        k.run.target = t.map_or(kh_core::host::TargetId::NONE, |t| t.id);
        k.run.sub = air as u8;
    }
    note(p, KitEvent::Started(if air { "air_wand_bonk" } else { "wand_bonk" }));
}

fn bonk_tick(p: &mut NpcPawn, host: &mut PartyHost) {
    let air = kit::kit_ref(p).is_some_and(|k| k.run.sub == 1);
    p.anim.speed = if air { K.bonk_air_rate } else { K.bonk_rate };
    p.cmc.direct_xy = Some(Vec2::ZERO);
    let t = p.anim.time;
    if air {
        // the hover of the air swing, then the fall
        if p.cmc.gravity_off {
            p.cmc.direct_z = Some(0.0);
        }
        let c = cr(p);
        if !c.shot && t >= K.bonk_air_hit {
            bonk_hit(p, host);
        }
        let c = cr(p);
        if c.queued && c.shot && t >= K.bonk_air_hit + 0.1 && bonk_ready(p) {
            if let Some(cc) = crm(p) {
                cc.queued = false;
            }
            if cr(p).air_n < air_steps(p) {
                end(p);
                start_bonk(p, None);
                if let Some(c) = crm(p) {
                    c.air_n += 1;
                }
                return;
            }
        }
        if t >= K.bonk_air_move || p.anim.finished || (p.on_ground() && p.state_time > 0.1) {
            end(p);
            if p.on_ground() {
                kit::back_to_pad(p);
            } else {
                super::enter_jump(p, true);
            }
        }
        return;
    }
    let c = cr(p);
    if !c.shot && t >= K.bonk_hit {
        bonk_hit(p, host);
    }
    bonk_ground_rest(p, host, t);
}

/// The Bonk's hit sphere in front of him.
fn bonk_hit(p: &mut NpcPawn, host: &mut PartyHost) {
    {
        let f = forward(p.yaw);
        let centre = p.position() + f * K.bonk_forward + Vec3::Z * 90.0;
        let mut a = Area::new(AreaKind::Hit, centre, K.bonk_radius, 0.1, "DON_WAND_BONK");
        a.power = K.bonk_power;
        a.kind_dmg = shots::kind_of(K.bonk_kind);
        a.push = if has(p, DonAbility::WandBonkPlus) { K.bonk_knockback_plus } else { K.bonk_knockback };
        a.physical = true;
        a.hit_fx = K.bonk_hit_fx;
        a.hit_se = K.bonk_hit_se;
        shots::add_area(p, host, a);
        let st = p.state_time;
        if let Some(c) = crm(p) {
            c.shot = true;
            c.shot_at = st;
        }
    }
}

fn bonk_ground_rest(p: &mut NpcPawn, host: &mut PartyHost, t: f32) {
    let c = cr(p);
    // (a queued press that would Bonk again waits for the clip's end: bonk_ready; only the string goes on here)
    if c.queued && t >= K.bonk_link && !would_bonk(p, host) {
        if let Some(cc) = crm(p) {
            cc.queued = false;
        }
        end(p);
        // not a combo step: the next press starts the string (or another Bonk; no target: a Bonk)
        if let Some(bt) = bonk_target(p, host) {
            start_bonk(p, Some(bt));
        } else if kit::target_info(p, host).is_none() {
            start_bonk(p, None);
        } else {
            if let Some(cc) = crm(p) {
                cc.n = 0;
            }
            start_step(p, host, Step::WaveL);
        }
        return;
    }
    let (yaw, mag, active) = kit::stick(p);
    if t >= K.bonk_move && active {
        end(p);
        p.to_idle();
        super::enter_run(p, yaw, mag, false);
        return;
    }
    if p.anim.finished {
        end(p);
        kit::back_to_pad(p);
    }
}

/// The step ends (whatever took him out): rate back, hover off, the charge glow stops.
pub fn end(p: &mut NpcPawn) {
    if !matches!(kit::kit_state(p), Some(STEP | AIR | CHARGE | BONK)) {
        return;
    }
    p.anim.speed = 1.0;
    p.cmc.direct_xy = None;
    if p.cmc.gravity_off {
        p.cmc.gravity_off = false;
        p.cmc.direct_z = None;
        if !p.on_ground() {
            p.cmc.set_falling();
        }
    }
    if let Some(c) = crm(p) {
        c.queued = false;
    }
}
