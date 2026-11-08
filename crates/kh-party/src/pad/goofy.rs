//! (GOOFY) GOOFY'S MOVES, THE LAYER OVER THE RULES (documentation/GOOFY_OVERHAUL.md 3 - 7, 10; PLAN.md R1, R3, R11 - R14): the
//! buttons, his states and what a hit does to him. The rules are crate::goofy; the shoves pad/goofy_combo.rs, the hit spheres
//! pad/goofy_shots.rs, the later lanes' moves pad/goofy_arts.rs, the Breaker pad/goofy_breaker.rs.
//!
//! (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique, Goofy's big damage should solely come from his
//! guard break attacks from 6.3") A PARRY OPENS NO COUNTER: after any technique A is his plain string again. What a parry gives
//! is 0 damage, its Stagger ([`answered`]) and, when perfect, MP back.
//!
//! THE PAD (R1): A attack, B always jump. X neutral taps a Deflect and holds a Guard; X + stick toward the enemy is the
//! aggressive deflect, Pinning Guard and Shield Reflect; held away it is Bull Brace; sideways the dodge hop. No air guard, no
//! air dodge. Y is the situation command, LB the shortcut panel.
//!
//! EVERYTHING HERE RUNS ON `FriendStatus::goofy`, NOT ON THE PAD: `buttons` and `stick` read the pad of a leader, or the drive
//! `GoofyRun::ai` of an AI Goofy (GF-AI), so both play by one set of rules. Every number is a row of
//! `sheets/port/party-kit-goofy.json`.

use super::goofy_combo::{self as combo, AIR_COMBO, COMBO};
use super::goofy_shots::{self as shots, Area};
use super::{enter_jump, enter_run, JUMP, RUN};
use crate::command::TargetRef;
use crate::goofy::parry::{self, HitFacts, Mods};
use crate::goofy::{GoofyAbility as GA, GoofyEvent, GoofyMove, Parry, StaggerWhy, StickRel, Tech, ThreatWhy, Verdict, K};
use crate::host::PartyHost;
use crate::ids::Chara;
use crate::pawn::{forward, horizontal, wrap180, yaw_of, NpcPawn, StepCtx};
use crate::states::NpcStateId;
use crate::goofy::Telegraph;
use glam::{Vec2, Vec3};
use kh_core::host::{AttackClass, HitRecord, HitResult, ReactionResult, TargetId, TargetInfo};
use kh_core::input::{PadButton, PadState as Buttons};
use kh_core::params::SORA_PARAMS;

/// Goofy's pad states (`NpcStateId::Pad(n)`, his range 40 ..= 54).
pub const GUARD: u8 = 40;
pub const GUARD_HIT: u8 = 41;
pub const GUARD_BREAK: u8 = 42;
// (43 was the counter's state: retired with the counters, THE USER 2026-11-02)
pub const HOP: u8 = 44;
pub const PARRY: u8 = 45;
pub const BRACE: u8 = 46;
/// Free for the later lanes.
pub const ART: u8 = 47;
pub const BREAKER: u8 = 48;
pub const TAUNT: u8 = 49;

pub fn owns_state(n: u8) -> bool {
    (GUARD..=AIR_COMBO).contains(&n)
}

/// The taunt (who reads it: the hosts' enemy targeting, `Party::goofy_taunt`). Set by Taunt (pad/goofy_arts.rs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Taunt {
    pub centre: Vec3,
    pub radius: f32,
    pub left: f32,
}

/// The drive of an AI Goofy: what a pad would give (GF-AI fills it each step).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct AiDrive {
    pub pad: Buttons,
    pub stick_yaw: f32,
    pub stick_magnitude: f32,
    pub lock: TargetId,
    /// The Art he fires this step.
    pub art: Option<GoofyMove>,
}

/// The running moves (`GoofyState::run`).
#[derive(Clone, Debug, PartialEq)]
pub struct GoofyRun {
    pub combo: combo::ComboRun,
    /// His clip stands still for this long more.
    pub hitstop: f32,
    /// A hop pressed in its cooldown's last moments: (stick yaw, seconds it is kept).
    pub hop_queued: Option<(f32, f32)>,
    /// The state at the last pre-move (the cleanup when something else took him out of a kit state).
    pub last_state: NpcStateId,
    /// An X press waiting for its move.
    pub x_buffer: f32,
    /// The A press is the kit's (decided in `decide`, run in `pre_move`).
    pub a_request: bool,
    /// The Y press is the Breaker's.
    pub y_request: bool,
    /// Hop / parry slide direction, world.
    pub dir: Vec3,
    /// Its speed, cm/s.
    pub slide: f32,
    /// Invincible seconds left; `hold_invincible` = until the running move's hit.
    pub invincible: f32,
    pub hold_invincible: bool,
    pub taunt: Option<Taunt>,
    /// The live hit spheres.
    pub shots: Vec<Area>,
    /// Effects and sounds a hit on him asked for (a hit has no host: played by the party's step).
    pub fx: Vec<(&'static str, Vec3)>,
    pub sfx: Vec<(&'static str, Vec3)>,
    /// Effects that sit ON his shield (its vfx_call socket, where KH3's own row attaches the guard barrier VN_bsh_inl0),
    /// whatever the pose: played by the party's step. (THE USER 2026-11-02: "this effect is not aligned properly")
    pub shield_fx: Vec<&'static str>,
    /// The camera nudge: seconds left, strength.
    pub nudge: (f32, f32),
    /// What the kit did (tests, the trace), newest last.
    pub log: Vec<&'static str>,
    /// An AI Goofy's buttons.
    pub ai: AiDrive,
    /// (GF-AI, THE USER 2026-11-03 follow-up: AI Goofy never went for a Breaker) the BROKEN enemy AI Goofy walks to for Goofy
    /// Breaker: while set, his tree's own moves give way to that walk ([`ai_chase_refuses`]).
    pub ai_chase: Option<TargetId>,
    /// (GF-BREAK) The BROKEN targets and the running Breaker.
    pub breaker: super::goofy_breaker::BreakerState,
    /// (GF-ARTS) The Shield Arts: the running one, the equipped two, Interpose's cover, the shield in flight.
    pub arts: super::goofy_arts::ArtsRun,
}

impl Default for GoofyRun {
    fn default() -> GoofyRun {
        GoofyRun { combo: Default::default(), hitstop: 0.0, hop_queued: None, last_state: NpcStateId::Idle, x_buffer: 0.0, a_request: false, y_request: false, dir: Vec3::X, slide: 0.0, invincible: 0.0, hold_invincible: false, taunt: None, shots: Vec::new(), fx: Vec::new(), sfx: Vec::new(), shield_fx: Vec::new(), nudge: (0.0, 0.0), log: Vec::new(), ai: AiDrive::default(), ai_chase: None, breaker: Default::default(), arts: Default::default() }
    }
}

// ---------------------------------------------------------------------------------------------- small helpers

fn is_goofy(p: &NpcPawn) -> bool {
    p.chara == Chara::Goofy && p.status.goofy.is_some()
}

pub(super) fn st(p: &NpcPawn) -> &crate::goofy::GoofyState {
    p.status.goofy.as_ref().expect("Goofy's state")
}

pub(super) fn stm(p: &mut NpcPawn) -> &mut crate::goofy::GoofyState {
    p.status.goofy.as_mut().expect("Goofy's state")
}

pub(super) fn g(p: &NpcPawn) -> &GoofyRun {
    &st(p).run
}

pub(super) fn gp(p: &mut NpcPawn) -> &mut GoofyRun {
    &mut stm(p).run
}

fn c(p: &NpcPawn) -> &crate::goofy::GoofyCore {
    &st(p).core
}

fn cm(p: &mut NpcPawn) -> &mut crate::goofy::GoofyCore {
    &mut stm(p).core
}

pub(super) fn has(p: &NpcPawn, k: GA) -> bool {
    p.status.gof_has(k)
}

pub(super) fn rank(p: &NpcPawn, t: Tech) -> u8 {
    p.status.gof_rank(t.mv())
}

/// GUARD (GG's meter): he can raise the shield.
pub(super) fn guard_ok(p: &NpcPawn) -> bool {
    p.status.guard.is_some_and(|g| g.can_guard())
}

pub(super) fn log(p: &mut NpcPawn, w: &'static str) {
    let l = &mut gp(p).log;
    l.push(w);
    if l.len() > 64 {
        l.remove(0);
    }
}

pub(super) fn state_n(p: &NpcPawn) -> Option<u8> {
    match p.state {
        NpcStateId::Pad(n) => Some(n),
        _ => None,
    }
}

fn in_state(p: &NpcPawn, n: u8) -> bool {
    p.state == NpcStateId::Pad(n)
}

/// A leader's pad, or an AI Goofy's drive.
fn buttons(p: &NpcPawn) -> Buttons {
    p.pad.as_ref().map_or(g(p).ai.pad, |d| d.frame.pad)
}

pub(super) fn stick(p: &NpcPawn) -> (f32, f32, bool) {
    let (yaw, mag) = p.pad.as_ref().map_or((g(p).ai.stick_yaw, g(p).ai.stick_magnitude), |d| (d.stick_yaw, d.stick_magnitude));
    (yaw, mag, mag >= SORA_PARAMS.input_stick_active_min)
}

pub(super) fn held(p: &NpcPawn, b: PadButton) -> bool {
    buttons(p).held.has(b)
}

fn pressed(p: &NpcPawn, b: PadButton) -> bool {
    buttons(p).pressed.has(b)
}

fn lb(p: &NpcPawn) -> bool {
    held(p, PadButton::LeftShoulder) || pressed(p, PadButton::LeftShoulder)
}

/// The lock, else the soft target.
pub(super) fn target_id(p: &NpcPawn) -> Option<TargetId> {
    match p.pad.as_ref() {
        Some(d) => match d.target() {
            TargetRef::Enemy(t) => Some(t),
            _ => None,
        },
        None => Some(g(p).ai.lock).filter(|t| *t != TargetId::NONE),
    }
}

/// A move's target: the lock, else the soft target.
pub(super) fn target_info(p: &NpcPawn, host: &PartyHost) -> Option<TargetInfo> {
    host.targets.info(target_id(p)?)
}

/// Turns him toward the target, else the held stick, by at most `max` degrees.
pub(super) fn face(p: &mut NpcPawn, want: Option<f32>, max: f32) {
    if let Some(w) = want {
        let d = wrap180(w - p.yaw);
        p.yaw = wrap180(p.yaw + d.clamp(-max, max));
    }
}

fn hard_lock(p: &NpcPawn) -> TargetId {
    p.pad.as_ref().map_or(g(p).ai.lock, |d| d.lock)
}

/// R1's "enemy" for the stick: the lock-on target, else the attacker of the nearest live telegraph, else the nearest hostile
/// the lock-on accepts.
fn enemy_of(p: &NpcPawn, cx: &StepCtx, host: &PartyHost) -> Option<TargetInfo> {
    if let Some(t) = host.targets.info(hard_lock(p)) {
        return Some(t);
    }
    let me = p.position();
    let dist = |t: &TargetInfo| horizontal(t.position - me).length();
    let live = cx.telegraphs.iter().filter_map(|t| host.targets.info(t.attacker)).filter(shots::lockable).min_by(|a, b| dist(a).total_cmp(&dist(b)));
    live.or_else(|| shots::enemies(host).into_iter().filter(|t| dist(t) <= K.input_enemy_near).min_by(|a, b| dist(a).total_cmp(&dist(b))))
}

fn yaw_to(p: &NpcPawn, at: Vec3) -> Option<f32> {
    let d = horizontal(at - p.position());
    (d.length() > 1.0).then(|| yaw_of(d))
}

/// Out of a kit state: guard while X is held, the run, idle or the fall.
pub(super) fn leave_to_ground(p: &mut NpcPawn) {
    let (yaw, mag, active) = stick(p);
    p.cmc.speed_limit_override = None;
    p.anim.speed = 1.0;
    if !p.on_ground() {
        if p.pad.is_some() {
            enter_jump(p, true);
        } else {
            p.to_idle();
        }
    } else if held(p, PadButton::FaceLeft) && !lb(p) && guard_ok(p) {
        start_guard(p);
    } else if active && p.pad.is_some() {
        p.to_idle();
        enter_run(p, yaw, mag, false);
        if let Some(d) = p.pad.as_mut() {
            d.run.sub = 0;
        }
    } else {
        p.to_idle();
    }
}

// ---------------------------------------------------------------------------------------------- decide (before the step)

/// The A / Y presses are the kit's (pad/mod.rs `decide`). True for every living pad Goofy: stream P's melee is not his.
pub fn decide(p: &mut NpcPawn, target: TargetRef) -> bool {
    let _ = target;
    if !is_goofy(p) || p.state == NpcStateId::Die {
        return false;
    }
    let team_running = matches!(p.state, NpcStateId::Fr(_));
    if pressed(p, PadButton::FaceTop) && !held(p, PadButton::LeftShoulder) && !team_running && super::goofy_breaker::breaker_offered(p) {
        gp(p).y_request = true;
        if let Some(pad) = p.pad.as_mut() {
            pad.x2.y_taken = true;
        }
    }
    let ai_a = if pressed(p, PadButton::FaceBottom) && !lb(p) { 1.0 } else { 0.0 }; // (GF-AI) his drive has no buffer
    let buffered = p.pad.as_ref().map_or(ai_a, |d| d.buffered);
    if buffered <= 0.0 {
        return true;
    }
    let take = |p: &mut NpcPawn| {
        gp(p).a_request = true;
        if let Some(pad) = p.pad.as_mut() {
            pad.buffered = 0.0;
        }
    };
    let wait = |p: &mut NpcPawn| {
        if let Some(pad) = p.pad.as_mut() {
            pad.buffered = (pad.buffered - 1.0 / 60.0).max(0.0);
        }
    };
    let st_n = state_n(p);
    let grounded = p.on_ground();
    match st_n {
        // the bash's end lag: the press waits in the buffer for it to end (no counter takes it any more: THE USER 2026-11-02,
        // "Remove countering with 'A' after a parry technique")
        // (THE USER 2026-11-02: "He needs some end lag at the end of his combo so you can't spam it, like Sora.")
        _ if combo::restart_held(p) => wait(p),
        Some(COMBO | AIR_COMBO | GUARD | BRACE | PARRY | GUARD_HIT) => take(p),
        Some(HOP) if p.anim.time >= K.hop_cancel => take(p),
        Some(HOP | GUARD_BREAK) => wait(p),
        _ if p.state == NpcStateId::SuperSlide && grounded => {
            crate::traverse::end(p);
            p.to_idle();
            log(p, "slide_attack");
            take(p);
        }
        _ if p.state == NpcStateId::Action => wait(p),
        _ if super::free(p) || ai_free(p) || (!grounded && (st_n == Some(JUMP) || p.state == NpcStateId::NavLinkJump)) => take(p),
        _ => wait(p),
    }
    true
}

// ---------------------------------------------------------------------------------------------- pre-move

/// After the command intake, before the movement tick (pad/mod.rs `pre_move`). True = the kit took the state (stream P's
/// state switch is skipped this step).
pub fn pre_move(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) -> bool {
    if !is_goofy(p) {
        return false;
    }
    shots::step_areas(p, host, cx.dt);
    super::goofy_arts::tidy_step(p); // (GF-ARTS) a hit or a friend's parry took him out of an Art
    if p.state == NpcStateId::Die {
        return false;
    }
    // SOMETHING ELSE TOOK HIM OUT OF A KIT STATE (a hit's reaction, an item): that state's clock, hover and rate end with it
    let last = g(p).last_state;
    if matches!(last, NpcStateId::Pad(n) if owns_state(n)) && !matches!(p.state, NpcStateId::Pad(n) if owns_state(n)) {
        if matches!(last, NpcStateId::Pad(COMBO | AIR_COMBO)) {
            combo::end(p);
            gp(p).combo.n = 0;
        }
        let k = gp(p);
        k.hitstop = 0.0;
        k.hold_invincible = false;
        k.breaker.run = None;
        k.slide = 0.0;
        if p.pad.is_none() {
            p.cmc.speed_limit_override = None; // (GF-AI) his Move state reads it
        }
    }
    if p.on_ground() && !in_state(p, AIR_COMBO) {
        gp(p).combo.air_n = 0;
    }
    combo::run_end_lag(p, cx.dt);
    // (FX1) THE HOST'S LATE "NO" (the team move it said "yes" to never started): his third shove now, when he can
    if p.pad.as_ref().is_some_and(|d| d.finisher_late_no > 0.0) {
        let in_chain = combo::link_open(p) && p.on_ground();
        if (super::free(p) && p.on_ground()) || in_chain {
            if let Some(d) = p.pad.as_mut() {
                (d.finisher_late_no, d.buffered, d.combo, d.finisher_answer, d.finisher_pending) = (0.0, 0.0, 0, None, false);
            }
            log(p, "late_no_finisher");
            if in_state(p, COMBO) {
                combo::end(p);
            }
            combo::finisher(p, host);
            gp(p).last_state = p.state;
            return true;
        }
        if let Some(d) = p.pad.as_mut() {
            d.finisher_late_no = (d.finisher_late_no - cx.dt).max(0.0);
        }
    }
    // the Superslide (B4's NPC SuperSlide state): the stick feeds it, B jumps out
    if p.state == NpcStateId::SuperSlide {
        let (yaw, mag, active) = stick(p);
        if pressed(p, PadButton::JUMP) && !lb(p) && p.on_ground() {
            crate::traverse::end(p);
            enter_jump(p, false);
            log(p, "slide_jump");
            return true;
        }
        if active {
            p.trav.feed = crate::traverse::Feed { on: true, yaw, magnitude: mag };
        }
        return false;
    }
    let took = pre_move_inputs(p, cx, host);
    gp(p).last_state = p.state;
    took
}

/// The button part of the pre-move (split off so the kit's last state is noted on every exit).
fn pre_move_inputs(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) -> bool {
    let dt = cx.dt;
    let x_held = held(p, PadButton::FaceLeft) && !held(p, PadButton::LeftShoulder);
    cm(p).hold_step(x_held, dt);
    // (GF-AI) what his brain chose of his Arts: the same start
    if let Some(mv) = gp(p).ai.art.take() {
        if super::goofy_arts::try_move(p, cx, host, mv) {
            return true;
        }
    }
    // LB + A / B: the two equipped Arts; LB + X: Taunt (GF-ARTS)
    let list_open = p.pad.as_ref().is_some_and(|d| d.menu.state == kh_core::command::MenuState::SubSel);
    if lb(p) && !list_open {
        if pressed(p, PadButton::FaceBottom) && super::goofy_arts::try_art(p, cx, host, 0) {
            return true;
        }
        if pressed(p, PadButton::FaceRight) && super::goofy_arts::try_art(p, cx, host, 1) {
            return true;
        }
        if pressed(p, PadButton::FaceLeft) && super::goofy_arts::try_taunt(p, cx, host) {
            return true;
        }
    }
    if std::mem::take(&mut gp(p).y_request) && super::goofy_breaker::try_breaker(p, cx, host) {
        return true;
    }
    if pressed(p, PadButton::FaceLeft) && !lb(p) {
        gp(p).x_buffer = K.feel_x_buffer;
    }
    if g(p).x_buffer > 0.0 {
        if try_x(p, cx, host) {
            gp(p).x_buffer = 0.0;
            return true;
        }
        gp(p).x_buffer = (g(p).x_buffer - dt).max(0.0);
    }
    // a hop pressed during its cooldown's last moments hops when the cooldown ends
    if let Some((yaw, left)) = g(p).hop_queued {
        let left = left - dt;
        gp(p).hop_queued = (left > 0.0).then_some((yaw, left));
        if c(p).hop_cd <= 0.0 && x_allowed(p) && p.on_ground() {
            gp(p).hop_queued = None;
            cancel_action(p);
            start_hop(p, yaw);
            return true;
        }
    }
    // B is always jump: out of the guard, a parry's recoil, a shove's hit, a hop's end
    let sub_sel = p.pad.as_ref().is_some_and(|d| d.menu.state == kh_core::command::MenuState::SubSel);
    if pressed(p, PadButton::JUMP) && !lb(p) && !sub_sel && p.on_ground() && (jump_open(p) || ai_free(p)) {
        cancel_action(p);
        p.cmc.speed_limit_override = None;
        if p.pad.is_some() {
            enter_jump(p, false);
        } else {
            // (GF-AI) a friend's own jump: the pad's jump state needs a pad
            let at = p.position();
            crate::jump::request_code_driven(p, at, 0.0, false, host.world);
        }
        log(p, "jump");
        return false;
    }
    if std::mem::take(&mut gp(p).a_request) && do_a(p, host) {
        return true;
    }
    state_n(p).is_some_and(owns_state)
}

/// B may jump now.
fn jump_open(p: &NpcPawn) -> bool {
    match state_n(p) {
        Some(GUARD | BRACE | PARRY | GUARD_HIT) => true,
        Some(COMBO) => combo::confirm_open(p),
        Some(HOP) => p.anim.time >= K.hop_cancel,
        Some(ART) => super::goofy_arts::jump_open(p),
        _ => false,
    }
}

/// X may act now. Any shove cancels into the guard at any time, with no recovery (3); the bash's end lag only from
/// `combo.end_lag_cancel` after its hit (THE USER 2026-11-02: "...end lag at the end of his combo... like Sora.": Sora's
/// finisher lets the guard and the dodge in only at its TresEnableChange).
fn x_allowed(p: &NpcPawn) -> bool {
    let st_n = state_n(p);
    let landing = st_n == Some(JUMP) && p.pad.as_ref().is_some_and(|d| d.jump.sub >= 10);
    let free_ground = p.state == NpcStateId::Idle || st_n == Some(RUN) || landing || ai_free(p);
    free_ground
        || matches!(st_n, Some(GUARD | BRACE | PARRY | GUARD_HIT))
        || (st_n == Some(COMBO) && combo::x_cancel_open(p))
        || (st_n == Some(HOP) && p.anim.time >= K.hop_cancel)
        || super::goofy_arts::guard_cancel(p)
}

/// (GF-AI) An AI Goofy on his feet in the states of his tree: free as a pad's idle and run are.
fn ai_free(p: &NpcPawn) -> bool {
    p.pad.is_none() && matches!(p.state, NpcStateId::Idle | NpcStateId::Move | NpcStateId::Turn)
}

/// (GF-AI) An AI Goofy is in one of the kit's states: no command of his tree runs it over.
pub fn ai_holds(p: &NpcPawn) -> bool {
    p.pad.is_none() && is_goofy(p) && matches!(p.state, NpcStateId::Pad(n) if owns_state(n))
}

/// (GF-AI, THE USER 2026-11-03 follow-up) AI Goofy walks to a BROKEN enemy for Goofy Breaker (`GoofyRun::ai_chase`): any other
/// move, turn, stop or action his KH3 tree asks meanwhile is refused (it asks again once the break is over), so the tree's
/// following does not pull him back each frame. The walk itself, a `MoveTo` that enemy, goes through.
pub fn ai_chase_refuses(p: &NpcPawn, cmd: &crate::command::MemberCommand) -> bool {
    use crate::command::{MemberCommand as C, MoveGoal};
    let Some(t) = p.pad.is_none().then(|| p.status.goofy.as_ref().and_then(|s| s.run.ai_chase)).flatten() else { return false };
    match cmd {
        C::MoveTo { goal: MoveGoal::Target(g), .. } => *g != t,
        C::MoveTo { .. } | C::TurnTo { .. } | C::Idle { .. } | C::Stop | C::Action { .. } | C::PlayMotion { .. } => true,
        _ => false,
    }
}

/// (GF-AI, THE USER 2026-11-03 follow-up) An AI Goofy walking to a BROKEN enemy or in his Breaker: he is fighting, whatever
/// the leader's battle flag says (`Party::set_in_battle`).
pub fn ai_breaking(p: &NpcPawn) -> bool {
    (p.pad.is_none() && p.status.goofy.as_ref().is_some_and(|s| s.run.ai_chase.is_some())) || (is_goofy(p) && in_state(p, BREAKER))
}

/// (GF-AI) X would act now.
pub fn ai_can_x(p: &NpcPawn) -> bool {
    p.on_ground() && x_allowed(p)
}

/// (GF-AI) What the AI may do now.
pub fn ai_stance(p: &NpcPawn) -> crate::goofy::ai::Stance {
    use crate::goofy::ai::Stance as S;
    match p.state {
        NpcStateId::Idle | NpcStateId::Move | NpcStateId::Turn if p.on_ground() => S::Free,
        NpcStateId::NavLinkJump => S::Airborne,
        NpcStateId::Pad(COMBO) => S::Striking,
        NpcStateId::Pad(n) if owns_state(n) && x_allowed(p) => S::Guarding,
        _ => S::Busy,
    }
}

/// (GF-AI) This step's buttons of an AI Goofy, before the party's step: the A / Y presses are read as a pad's are.
pub fn ai_drive(p: &mut NpcPawn, d: AiDrive) {
    gp(p).ai = d;
    decide(p, TargetRef::None);
}

/// (GF-AI) The pre-move of an AI Goofy: the same moves. A kit state he entered ends the running command of his tree (it would
/// wait on it for ever); the tree asks again when he is free.
pub fn ai_pre_move(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    if !is_goofy(p) {
        return;
    }
    pre_move(p, cx, host);
    if ai_holds(p) && p.command_now().is_some() {
        p.abort_command_pub();
    }
}

/// Ends the running kit move for a cancel.
pub(super) fn cancel_action(p: &mut NpcPawn) {
    if p.state == NpcStateId::Action {
        p.end_action();
        p.action = None;
        p.cmc.direct_xy = None;
        p.cmc.gravity_scale = p.params.gravity_scale;
        p.cmc.gravity_off = false;
        p.cmc.direct_z = None;
        p.anim.speed = 1.0;
    }
    if matches!(state_n(p), Some(COMBO | AIR_COMBO)) {
        combo::end(p);
    }
    if in_state(p, ART) || in_state(p, TAUNT) {
        super::goofy_arts::end(p); // (GF-ARTS)
    }
    if let Some(d) = p.pad.as_mut() {
        d.combo = 0;
    }
}

/// An ability from the list / a shortcut may start now (pad/mod.rs `decide`): a shove's link point, a hop's end, the guard.
pub fn link_for_ability(p: &NpcPawn) -> bool {
    is_goofy(p) && (combo::link_open(p) || (in_state(p, HOP) && p.anim.time >= K.hop_cancel) || in_state(p, GUARD))
}

/// X on the ground: by the stick against the enemy (R1). Sideways, or any stick with no enemy, hops; else the tap / the guard.
fn try_x(p: &mut NpcPawn, cx: &StepCtx, host: &PartyHost) -> bool {
    if !p.on_ground() || !x_allowed(p) {
        return false;
    }
    let enemy = enemy_of(p, cx, host);
    let enemy_yaw = enemy.and_then(|t| yaw_to(p, t.position));
    let (yaw, _, active) = stick(p);
    let rel = parry::stick_rel(yaw, active, enemy_yaw);
    // (GF-ARTS) X out of Turbo is the guard, wherever the stick steers
    if rel == StickRel::Side && !super::goofy_arts::guard_cancel(p) {
        if c(p).hop_cd <= 0.0 {
            cancel_action(p);
            start_hop(p, yaw);
            return true;
        }
        if c(p).hop_cd <= K.hop_buffer {
            gp(p).hop_queued = Some((yaw, K.hop_buffer + 0.02));
            return true;
        }
    }
    if !guard_ok(p) {
        return false;
    }
    cancel_action(p);
    cm(p).press(yaw, active, rel);
    start_guard(p);
    face(p, enemy_yaw, K.deflect_turn);
    log(p, "tap");
    true
}

/// The kit's A: his string, whatever came before (no counter after a parry: THE USER 2026-11-02). While the shield is out he
/// can only guard (8): no shove.
fn do_a(p: &mut NpcPawn, host: &mut PartyHost) -> bool {
    if super::goofy_arts::bare_handed(p) {
        return false;
    }
    if !p.on_ground() {
        return combo::press_air(p, host);
    }
    if in_state(p, HOP) {
        p.cmc.direct_xy = Some(Vec2::ZERO);
    }
    combo::press(p, host)
}

// ---------------------------------------------------------------------------------------------- guard, deflect stance, brace

pub(super) fn start_guard(p: &mut NpcPawn) {
    p.enter(NpcStateId::Pad(GUARD));
    p.cmc.stop_movement();
    p.cmc.walk_mode = false;
    p.anim.speed = 1.0;
    p.play_looped("SGuardIdle", K.guard_blend);
    log(p, "guard");
}

/// The shield held up (THE USER 2026-11-02: "Goofy's guard should not deplete while holding it, only from getting hit"):
/// nothing is spent; the refill waits while it is up.
fn guard_held(p: &mut NpcPawn) {
    if let Some(gd) = p.status.guard.as_mut() {
        gd.hold();
    }
}

fn guard_tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    let x = held(p, PadButton::FaceLeft);
    if (!x && c(p).tap_age >= K.deflect_stance && g(p).arts.stance <= 0.0) || !guard_ok(p) {
        p.cmc.speed_limit_override = None;
        leave_to_ground(p);
        return;
    }
    if !p.on_ground() {
        leave_to_ground(p);
        return;
    }
    guard_held(p);
    let (yaw, mag, active) = stick(p);
    // the stick held away: Bull Brace plants his feet
    let enemy = enemy_of(p, cx, host);
    let enemy_yaw = enemy.and_then(|t| yaw_to(p, t.position));
    if x && rank(p, Tech::BullBrace) > 0 && parry::stick_rel(yaw, active, enemy_yaw) == StickRel::Away {
        p.enter(NpcStateId::Pad(BRACE));
        p.cmc.stop_movement();
        p.cmc.speed_limit_override = Some(0.0);
        p.play(K.brace_clip, K.guard_blend);
        log(p, "brace");
        return;
    }
    // facing: the enemy, else the stick
    let want = enemy_yaw.or(active.then_some(yaw));
    face(p, want, K.guard_turn_rate * dt);
    if active && x {
        let a = yaw.to_radians();
        p.cmc.control_input = Vec3::new(a.cos(), a.sin(), 0.0);
        p.cmc.max_walk_speed = K.guard_walk_speed;
        p.cmc.max_acceleration = K.guard_walk_speed * SORA_PARAMS.accel_ground_factor;
        p.cmc.speed_limit_override = Some(K.guard_walk_speed * mag.min(1.0));
    } else {
        p.cmc.speed_limit_override = Some(0.0);
    }
    let moving = p.speed2d() > 20.0 || (active && x);
    let want_clip = if moving { "SGuardWalk" } else { "SGuardIdle" };
    if p.anim.clip.name != want_clip {
        p.play_looped(want_clip, K.guard_blend);
    }
}

fn brace_tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let x = held(p, PadButton::FaceLeft);
    let (yaw, _, active) = stick(p);
    let enemy_yaw = enemy_of(p, cx, host).and_then(|t| yaw_to(p, t.position));
    if !x || !guard_ok(p) || parry::stick_rel(yaw, active, enemy_yaw) != StickRel::Away {
        p.cmc.speed_limit_override = None;
        leave_to_ground(p);
        return;
    }
    face(p, enemy_yaw, K.guard_turn_rate * cx.dt);
    guard_held(p);
}

/// A blocked hit's or a parry's slide back; the clip's end hands back to the guard or the ground.
fn recoil_tick(p: &mut NpcPawn, cx: &StepCtx) {
    if g(p).hitstop > 0.0 {
        gp(p).hitstop -= cx.dt;
        p.anim.speed = 0.0;
        p.cmc.direct_xy = Some(Vec2::ZERO);
        if g(p).hitstop > 0.0 {
            return;
        }
        gp(p).hitstop = 0.0;
    }
    let rate = if in_state(p, PARRY) { K.deflect_rate } else { 1.0 };
    p.anim.speed = rate;
    if p.state_time < K.guard_pushback_time && p.on_ground() {
        let d = g(p).dir * g(p).slide;
        p.cmc.direct_xy = Some(Vec2::new(d.x, d.y));
    }
    if p.anim.finished {
        p.anim.speed = 1.0;
        leave_to_ground(p);
    }
}

/// GUARD BREAK: the meter emptied. He staggers `guard_break.time` (KH3's stun, THE USER 2026-11-02: "Make Goofy stagger when
/// his block breaks") and takes full damage (4.2); no guard until GG's rearm.
fn start_guard_break(p: &mut NpcPawn) {
    if let Some(gd) = p.status.guard.as_mut() {
        if !gd.broken {
            gd.break_now();
        }
    }
    log(p, "guard_break");
    let revenge = has(p, GA::RevengeBash);
    let core = cm(p);
    core.revenge = revenge;
    core.events.push(GoofyEvent::GuardBreak);
    p.enter(NpcStateId::Pad(GUARD_BREAK));
    p.cmc.speed_limit_override = None;
    p.cmc.stop_movement();
    p.anim.speed = 1.0;
    p.play(if p.on_ground() { K.guard_break_clip } else { K.guard_break_air_clip }, 0.1);
}

fn guard_break_tick(p: &mut NpcPawn) {
    if p.state_time >= K.guard_break_time {
        leave_to_ground(p);
    } else if p.anim.finished && (p.anim.clip.name == K.guard_break_clip || p.anim.clip.name == K.guard_break_air_clip) {
        // the stun's start ran out first: its loop until the time is up
        let air = p.anim.clip.name == K.guard_break_air_clip;
        p.play_looped(if air { K.guard_break_air_loop_clip } else { K.guard_break_loop_clip }, 0.1);
    }
}

// ---------------------------------------------------------------------------------------------- a hit on him

/// The kit's reading of its state for a hit.
fn mods(p: &NpcPawn) -> Mods {
    let s = st(p);
    let resolve = s.core.resolve;
    Mods { ranks: p.status.growth.ranks, level: p.status.level, pips: resolve.pips(), in_zone: resolve.in_zone(), forward_plus: has(p, GA::ForwardDeflectPlus), second_deflect: has(p, GA::SecondDeflect) && !s.core.second_deflect_used, revenge: s.core.revenge, stagger_boost: has(p, GA::StaggerBoost) }
}

fn raised(p: &NpcPawn) -> bool {
    matches!(state_n(p), Some(GUARD | BRACE | PARRY | GUARD_HIT))
}

/// A hit on him (pawn.rs `receive_hit`): None = the normal hit path. Pad or AI, the same rules.
pub fn on_hit(p: &mut NpcPawn, hit: &HitRecord) -> Option<HitResult> {
    if !is_goofy(p) {
        return None;
    }
    let real = hit.takes_hp() && hit.damage > 0.0 && hit.info.victim_result != ReactionResult::NoReaction; // (GF-AI) a blow Aero Barrier cut under 1 HP is still a blow
    let attacker = hit.attack_info.attacker;
    let class = parry::class_of(hit);
    let perilous = parry::perilous_of(hit);
    if real && over_sweep(p, class) {
        log(p, "over_sweep");
        return Some(HitResult { accepted: false, hp_after: p.status.hp as f32, ..HitResult::default() });
    }
    if real {
        cm(p).mark_hit(attacker);
    }
    if invincible(p) {
        log(p, "dodged");
        // a grab that closes on the hop's invincibility is the Slip Counter at once
        if real && class == AttackClass::Grab && in_state(p, HOP) && rank(p, Tech::SlipCounter) > 0 {
            evaded(p, Tech::SlipCounter, attacker.or(target_id(p)), perilous);
        }
        return Some(HitResult { accepted: false, hp_after: p.status.hp as f32, ..HitResult::default() });
    }
    if !real {
        return None;
    }
    if super::goofy_arts::spin_deflects(p, class) {
        return Some(super::goofy_arts::spin_deflect(p, hit, attacker)); // (GF-ARTS) Tornado
    }
    let from = if hit.attacker.valid { hit.attacker.position } else { p.position() - hit.direction * 100.0 };
    let to = horizontal(from - p.position());
    let off = if to.length() < 1.0 { 0.0 } else { wrap180(yaw_of(to) - p.yaw).abs() };
    let core = c(p);
    let stick_read = if hit.attacker.valid { parry::stick_rel(core.tap_yaw, core.tap_active, Some(yaw_of(to))) } else { core.tap_rel };
    let facts = HitFacts { class, raised: raised(p), front_block: off <= K.guard_arc_half, front_deflect: off <= K.deflect_arc_half, stick: stick_read, braced: in_state(p, BRACE).then_some(p.state_time) };
    let tap = core.tap();
    let m = mods(p);
    let verdict = parry::judge(&facts, &tap, &m);
    let verdict = if super::goofy_arts::bare_handed(p) { super::goofy_arts::bare_verdict(verdict, class, facts.front_block) } else { verdict }; // (GF-ARTS) the shield is out
    match verdict {
        Verdict::Parry(answer) => {
            // (THE USER 2026-11-02: "make non-boss enemies bounce off of Goofy's shield if they're not doing a long combo string when
            // Goofy successfully performs a perfect deflect") the string is read before this deflect joins it; the boss is the seam's
            let target = attacker.or(target_id(p));
            let bounce = parry::bounces(&answer, class, c(p).mid_string(target));
            let result = parried(p, hit, answer, class, perilous, -to.normalize_or_zero(), &m);
            if let Some(target) = target.filter(|_| bounce) {
                cm(p).events.push(GoofyEvent::Bounce { target });
            }
            Some(result)
        }
        Verdict::Block => block(p, hit, -to.normalize_or_zero(), attacker),
        Verdict::Through { failed } => {
            if failed {
                log(p, "parry_failed");
                let broke = p.status.guard.as_mut().is_some_and(|gd| gd.absorb(K.guard_fail_drain).broke);
                if broke {
                    start_guard_break(p);
                }
            }
            took_hit(p);
            None
        }
    }
}

/// The hit lands: Resolve, the window, the chain.
fn took_hit(p: &mut NpcPawn) {
    let (keeper, extender) = (has(p, GA::ResolveKeeper), has(p, GA::ZoneExtender));
    let core = cm(p);
    core.resolve.on_hit(keeper, extender);
    core.on_hit_taken();
}

/// The shield takes it: GUARD pays, by the hit's power (4.1). An emptied meter is the Guard Break: the hit lands in full.
fn block(p: &mut NpcPawn, hit: &HitRecord, back: Vec3, attacker: Option<TargetId>) -> Option<HitResult> {
    let max_hp = p.status.max_hp().max(1) as f32;
    let cost = if has(p, GA::SteadyGuard) { K.guard_steady_mult } else { 1.0 } * super::goofy_arts::guard_mult(p);
    let drain = (K.guard_drain_share * hit.damage.max(0.0) / max_hp).max(K.guard_drain_min) * cost;
    let broke = p.status.guard.as_mut().is_some_and(|gd| gd.absorb(drain).broke);
    if let Some(target) = attacker.or(target_id(p)) {
        cm(p).events.push(GoofyEvent::Threat { target, amount: K.threat_guard, why: ThreatWhy::Guard });
    }
    if broke {
        start_guard_break(p);
        took_hit(p);
        return None;
    }
    log(p, "block");
    let at = shield_point(p);
    let k = gp(p);
    k.sfx.push((K.guard_block_se, at));
    k.shield_fx.push(K.guard_block_effect); // on the shield itself, not a point ahead of his capsule
    k.dir = back;
    k.slide = K.guard_pushback_speed;
    p.enter(NpcStateId::Pad(GUARD_HIT));
    p.cmc.speed_limit_override = None;
    p.anim.speed = 1.0;
    p.play("SGuardHit", 0.05);
    Some(HitResult { accepted: true, damage_dealt: 0.0, hp_after: p.status.hp as f32, killed: false, guarded: true })
}

fn shield_point(p: &NpcPawn) -> Vec3 {
    p.capsule_centre() + forward(p.yaw) * p.params.capsule_radius
}

/// The clang: sparks, the flash, the hit-stop's sound, the camera nudge (13). A right technique hits harder.
fn parry_cues(p: &mut NpcPawn, big: bool) {
    let at = shield_point(p);
    let k = gp(p);
    k.fx.push((K.deflect_spark, at));
    k.fx.push((K.deflect_effect, at));
    if big {
        k.fx.push((K.parry_right_effect, at));
        k.sfx.push((K.parry_right_se, at));
    }
    k.nudge = (K.nudge_time, if big { K.nudge_right } else { 1.0 });
}

/// (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique ... The parries are there to make the enemy's
/// stagger go up fast while taking 0 damage yourself") WHAT EVERY RIGHT ANSWER GIVES, a hit's parry or an evasion: Resolve, the
/// string, the technique's Stagger ([`parry::parry_stagger`]), threat, and on a perfect parry MP back ([`parry_mp`]). No counter
/// window opens.
fn answered(p: &mut NpcPawn, answer: Parry, target: Option<TargetId>, perilous: bool, m: &Mods) {
    let core = cm(p);
    let hits = core.string_hit();
    core.string_from = target; // (THE USER 2026-11-02, the bounce) whose string it is
    core.resolve.gain(if perilous && answer.right { K.resolve_right as u8 } else { K.resolve_perfect as u8 });
    if !core.resolve.in_zone() {
        core.resolve.zone_shield_used = false;
    }
    if answer.forgiven {
        core.second_deflect_used = true;
    }
    if answer.revenge {
        core.revenge = false;
    }
    let right_tech = answer.right && answer.tech != Tech::Deflect;
    core.events.push(GoofyEvent::Parried { tech: answer.tech, right: answer.right, target });
    if let Some(target) = target {
        core.events.push(GoofyEvent::Stagger { target, amount: parry::parry_stagger(&answer, hits, m), why: if right_tech { StaggerWhy::Technique } else { StaggerWhy::Deflect } });
        core.events.push(GoofyEvent::Threat { target, amount: K.threat_deflect, why: ThreatWhy::Deflect });
    }
    if parry::gives_mp(&answer) {
        parry_mp(p);
    }
}

/// (THE USER 2026-11-02: "MP back on a perfect parry is good") THE MP OF A PERFECT PARRY, the counters' MP's replacement: row
/// `parry.mp`, or `parry.mp_charge` off a running MP Charge (KH3's charge is never cut to nothing by it). Only gives MP: how an
/// Art pays is the party's MP rule (crate::mp_rule).
pub(super) fn parry_mp(p: &mut NpcPawn) {
    let s = &mut p.status;
    if s.mp_charge_left > 0.0 {
        s.mp_charge_left = (s.mp_charge_left - K.parry_mp_charge).max(0.01);
    } else {
        s.mp = (s.mp + K.parry_mp as i32).min(s.max_mp());
    }
}

/// A parry of a hit: no HP goes (R3: the host cancels it through `guarded`).
fn parried(p: &mut NpcPawn, hit: &HitRecord, answer: Parry, class: AttackClass, perilous: bool, back: Vec3, m: &Mods) -> HitResult {
    let target = hit.attack_info.attacker.or(target_id(p));
    answered(p, answer, target, perilous, m);
    // GUARD: a timed block costs nothing (THE USER 2026-11-02) and refills a bit (Resolve 2 refills more)
    let refund = K.deflect_guard_refund + if m.pips >= 2 { K.resolve_pip2_refund } else { 0.0 };
    if let Some(gd) = p.status.guard.as_mut() {
        gd.hold();
        gd.value = (gd.value + refund).min(gd.max);
    }
    log(p, match answer.tech {
        Tech::Deflect if answer.right => "deflect",
        Tech::Deflect => "plain_deflect",
        Tech::PinningGuard => "pinning_guard",
        Tech::HopCounter => "hop_counter",
        Tech::SlipCounter => "slip_counter",
        Tech::ShieldReflect => "shield_reflect",
        Tech::SpellWard => "spell_ward",
        Tech::BullBrace => "bull_brace",
        Tech::RhythmDeflect => "rhythm_deflect",
    });
    let _ = class;
    let big = answer.right && answer.tech != Tech::Deflect;
    parry_cues(p, big);
    let stop = if big { K.parry_right_hitstop } else { K.deflect_hitstop };
    let k = gp(p);
    k.hitstop = stop;
    k.dir = back;
    k.slide = K.deflect_push;
    p.enter(NpcStateId::Pad(PARRY));
    p.cmc.speed_limit_override = None;
    p.cmc.stop_movement();
    let clip = if answer.tech == Tech::BullBrace { K.brace_hit_clip } else { K.deflect_clip };
    p.play(clip, 0.03);
    p.anim.speed = K.deflect_rate;
    HitResult { accepted: true, damage_dealt: 0.0, hp_after: p.status.hp as f32, killed: false, guarded: true }
}

/// An evasion that answered its attack (Hop Counter, Slip Counter).
fn evaded(p: &mut NpcPawn, tech: Tech, target: Option<TargetId>, perilous: bool) {
    let m = mods(p);
    let answer = Parry { tech, right: true, aggressive: false, forgiven: false, revenge: false };
    answered(p, answer, target, perilous, &m);
    log(p, if tech == Tech::HopCounter { "hop_counter" } else { "slip_counter" });
    parry_cues(p, true);
}

/// How long he must be off the ground to be over a sweep.
fn hop_min_air(hop_rank: u8) -> f32 {
    if hop_rank >= 2 { K.hop_counter_min_air_r2 } else { K.hop_counter_min_air }
}

/// He is over a sweep that reaches him now. (In Elden Ring the swing's own box decides nothing here: the hidden body and a
/// swing's reach do not know his jump. MEASURED: Margit's sweeps landed 0.29 s into a jump. So the rule is the kit's, as in the test level.)
fn over_sweep(p: &NpcPawn, class: AttackClass) -> bool {
    let hop_rank = rank(p, Tech::HopCounter);
    class == AttackClass::Sweep && hop_rank > 0 && !p.on_ground() && c(p).air_for >= hop_min_air(hop_rank)
}

/// One step of Hop and Slip, for pad and AI alike: the hit windows of the host's list; an active window that ended with no hit
/// landed on him, while he was over it or hopping past it, is the parry (R5, the plan's contract).
pub fn evade_step(p: &mut NpcPawn, list: &[Telegraph], dt: f32, host: &PartyHost) {
    let on_ground = p.on_ground();
    let core = cm(p);
    core.air_for = if on_ground { 0.0 } else { core.air_for + dt };
    let hopping = in_state(p, HOP) && invincible(p);
    if hopping {
        cm(p).since_hop = 0.0;
    }
    let (hop_rank, slip_rank) = (rank(p, Tech::HopCounter), rank(p, Tech::SlipCounter));
    let grace = if slip_rank >= 2 { K.slip_grace_r2 } else { K.slip_grace };
    let hop_like = hopping || c(p).since_hop <= grace;
    // (MEASURED, Margit's 3036) a grab takes the body the frame its window opens, whatever the kit thinks of the hop
    cm(p).slip_open = hop_like && slip_rank > 0 && list.iter().any(|t| t.class == AttackClass::Grab && (t.active || t.eta <= K.slip_grace));
    if list.is_empty() && c(p).watches.is_empty() {
        return;
    }
    let min_air = hop_min_air(hop_rank);
    let me = p.position();
    let near = |t: TargetId| host.targets.info(t).is_some_and(|i| (horizontal(i.position - me).length() - i.radius).max(0.0) <= K.evade_range);
    let air_for = c(p).air_for;
    let ended = cm(p).watch_step(list, air_for, hop_like, &near);
    for w in ended.into_iter().filter(|w| !w.hit && w.near) {
        if w.class == AttackClass::Sweep && hop_rank > 0 && w.air >= min_air {
            evaded(p, Tech::HopCounter, Some(w.attacker), w.perilous);
        } else if w.class == AttackClass::Grab && slip_rank > 0 && w.hop {
            evaded(p, Tech::SlipCounter, Some(w.attacker), w.perilous);
        }
    }
}

/// R14: he parries a blow aimed at a friend (Ally Guard here, Interpose from GF-ARTS). The caller answers the friend's hit
/// as absorbed; here Goofy's side of it: GUARD, the clang, Stagger on the attacker. False: he cannot.
pub fn parry_for_friend(p: &mut NpcPawn, hit: &HitRecord) -> bool {
    if !is_goofy(p) || p.state == NpcStateId::Die || p.hurt.dying || in_state(p, GUARD_BREAK) || matches!(p.state, NpcStateId::Fr(_)) || super::goofy_arts::bare_handed(p) {
        return false;
    }
    let m = mods(p);
    let answer = Parry { tech: Tech::Deflect, right: true, aggressive: false, forgiven: false, revenge: false };
    let class = parry::class_of(hit);
    let from = if hit.attacker.valid { hit.attacker.position } else { p.position() + forward(p.yaw) * 100.0 };
    face(p, yaw_to(p, from), 180.0);
    let back = -forward(p.yaw);
    parried(p, hit, answer, class, false, back, &m);
    log(p, "ally_guard");
    true
}

/// The hop's and the running move's invincibility now.
pub fn invincible(p: &NpcPawn) -> bool {
    if !is_goofy(p) {
        return false;
    }
    let k = g(p);
    if k.invincible > 0.0 || k.hold_invincible {
        return true;
    }
    // (DON-KIT) Donald's Aero aura: the invincible window x (1 + evasion)
    in_state(p, HOP) && p.state_time >= K.hop_inv_from && p.state_time <= K.hop_inv_to * (1.0 + p.don_aura.evasion.max(0.0))
}

/// No flinch from this hit (pawn.rs `receive_hit`: HP without a reaction): the Guard Break's stagger plays on.
pub fn no_reaction(p: &NpcPawn, hit: &HitRecord) -> bool {
    let _ = hit;
    is_goofy(p) && in_state(p, GUARD_BREAK)
}

// ---------------------------------------------------------------------------------------------- the dodge

fn start_hop(p: &mut NpcPawn, yaw: f32) {
    let dir = forward(yaw);
    let f = forward(p.yaw);
    let right = Vec3::new(-f.y, f.x, 0.0);
    let side = dir.dot(right);
    p.enter(NpcStateId::Pad(HOP));
    p.cmc.speed_limit_override = None;
    p.cmc.stop_movement();
    p.anim.speed = 1.0;
    p.play(if side >= 0.0 { "HopR" } else { "HopL" }, 0.05);
    let cd = K.hop_cooldown / (1.0 + p.don_aura.evasion.max(0.0)); // (DON-KIT) Donald's Aero aura: a shorter cooldown
    cm(p).hop_cd = cd;
    let k = gp(p);
    k.dir = dir;
    k.hop_queued = None;
    log(p, "hop");
}

fn hop_tick(p: &mut NpcPawn) {
    if p.state_time < K.hop_time {
        let v = g(p).dir * (K.hop_distance / K.hop_time.max(0.01));
        p.cmc.direct_xy = Some(Vec2::new(v.x, v.y));
    } else if p.state_time - K.hop_time < 0.02 {
        p.cmc.direct_xy = Some(Vec2::ZERO);
    }
    // from the cancel point on: the stick runs, a buffered A starts the string, B jumps
    let (_, _, active) = stick(p);
    let wants = p.pad.as_ref().is_some_and(|d| d.buffered > 0.0) || pressed(p, PadButton::JUMP);
    if p.anim.finished || (p.anim.time >= K.hop_cancel && (active || wants)) {
        if !p.on_ground() {
            leave_to_ground(p);
        } else if pressed(p, PadButton::JUMP) && p.pad.is_some() {
            enter_jump(p, false);
        } else {
            leave_to_ground(p);
        }
    }
}

// ---------------------------------------------------------------------------------------------- landed hits

/// An area of the kit hit a target (goofy_shots `step_areas`): the shove's hit-stop, its Stagger and threat; an Art's.
pub fn on_area_hit(p: &mut NpcPawn, a: &Area, target: TargetId) {
    match a.tag {
        "combo" | "combo_fin" => {
            combo::on_landed(p);
            let core = cm(p);
            core.events.push(GoofyEvent::Stagger { target, amount: K.stagger_basic, why: StaggerWhy::Basic });
            core.events.push(GoofyEvent::Threat { target, amount: K.threat_basic, why: ThreatWhy::Basic });
        }
        "art" => super::goofy_arts::on_landed(p, a, target),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------- the state's MainProc

/// MainProc of Goofy's pad states (pad/mod.rs `tick`).
pub fn tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    match state_n(p) {
        Some(GUARD) => guard_tick(p, cx, host),
        Some(BRACE) => brace_tick(p, cx, host),
        Some(GUARD_HIT | PARRY) => recoil_tick(p, cx),
        Some(GUARD_BREAK) => guard_break_tick(p),
        Some(HOP) => hop_tick(p),
        Some(COMBO | AIR_COMBO) => combo::tick(p, cx, host),
        Some(BREAKER) => super::goofy_breaker::tick(p, cx, host),
        Some(ART | TAUNT) => super::goofy_arts::tick(p, cx, host),
        _ => p.to_idle(),
    }
}

// ---------------------------------------------------------------------------------------------- party side

impl crate::party::Party {
    /// (GF-INGAME) He is out of reach now: the hop's invincibility, or a grab closing on a Slip. The host's body must refuse the
    /// hit as well: a grab is the game's throw, and it never asks the kit.
    pub fn goofy_evades(&self, id: crate::MemberId) -> bool {
        self.member(id).is_some_and(|m| m.status.goofy.is_some() && (invincible(m) || c(m).slip_open))
    }

    /// The taunt of member `id`: enemies within `radius` of `centre` should pick him as their target for `left` seconds.
    /// The hosts' enemy AI reads it (kh-app: the dummies' victim; Elden Ring: the leader body's hate).
    pub fn goofy_taunt(&self, id: crate::MemberId) -> Option<Taunt> {
        self.member(id)?.status.goofy.as_ref()?.run.taunt
    }

    /// What the HUD draws for member `id`: Resolve.
    pub fn goofy_hud(&self, id: crate::MemberId) -> Option<crate::goofy::GoofyHud> {
        Some(crate::goofy::seams::hud(self.member(id)?.status.goofy.as_ref()?))
    }

    /// The camera nudge of a parry now: (turn in degrees, sway in cm), both fading to 0 (kh-partyhost adds them to the shake).
    pub fn goofy_nudge(&self, id: crate::MemberId) -> Option<(f32, f32)> {
        let (left, power) = self.member(id)?.status.goofy.as_ref()?.run.nudge;
        (left > 0.0).then(|| {
            let fade = (left / K.nudge_time.max(0.01)) * power;
            (K.nudge_deg * fade, K.nudge_cm * fade)
        })
    }

    /// DEBUG words for test runs (kh-app `party-word x3g ...`): `gauges`, `kit`, `resolve N` (thirds), `rank MOVE N` (the banked
    /// ranks of a move, 0 .. 2), `stagger` (the book), `stagger lock N` / `near N` / `TARGET N` (Stagger on the lock, the nearest enemy, a target),
    /// `threat` (the book), `taunt` (his Taunt with no button, no state: proves the hosts' side, leader or AI).
    pub fn goofy_word(&mut self, words: &[&str]) -> String {
        if words == ["ai"] {
            return crate::goofy::ai::word(self); // (GF-AI) what AI Goofy is thinking
        }
        let Some(id) = self.find(Chara::Goofy) else { return "goofy: not in the party".to_string() };
        let Some(m) = self.member_mut(id) else { return "goofy: not in the party".to_string() };
        let at = m.position();
        let Some(s) = m.status.goofy.as_mut() else { return "goofy: no state".to_string() };
        match words {
            ["taunt"] => {
                s.run.taunt = Some(Taunt { centre: at, radius: K.art_taunt_radius, left: K.art_taunt_time });
                format!("goofy: Taunt at {at:?}, {} cm, {} s", K.art_taunt_radius, K.art_taunt_time)
            }
            ["give", name] => match GA::ALL.into_iter().find(|k| k.label().replace(' ', "").eq_ignore_ascii_case(name)) {
                Some(k) => {
                    s.abilities.give(k);
                    s.abilities.set_equipped(k, true);
                    format!("goofy: {} given and equipped", k.label())
                }
                None => format!("goofy: no ability {name}"),
            },
            ["resolve", n] => {
                s.core.resolve.thirds = n.parse::<u8>().unwrap_or(0).min(crate::goofy::RESOLVE_FULL);
                format!("goofy: Resolve {} thirds", s.core.resolve.thirds)
            }
            ["rank", name, n] => match GoofyMove::ALL.into_iter().find(|l| l.token().ends_with(name)) {
                Some(mv) => {
                    let mut parts: Vec<u64> = m.status.growth.ranks.parts().iter().map(|v| *v as u64).collect();
                    let i = GoofyMove::ALL.iter().position(|x| *x == mv).unwrap_or(0);
                    parts[i] = n.parse::<u64>().unwrap_or(0);
                    m.status.growth.ranks = crate::goofy::RankBank::from_parts(&parts);
                    format!("goofy: {} rank {}", mv.label(), m.status.growth.ranks.rank(mv))
                }
                None => format!("goofy: no move {name}"),
            },
            ["gauges"] => {
                let gd = m.status.guard.unwrap_or_default();
                let s = m.status.goofy.as_ref().map_or(&[][..], |g| g.run.log.as_slice());
                format!("goofy: GUARD {:.0}/{:.0}{} HP {} MP {} resolve {} string {} hop cd {:.2} state {:?} log {:?}", gd.value, gd.max, if gd.broken { " BROKEN" } else { "" }, m.status.hp, m.status.mp, m.status.goofy.as_ref().map_or(0, |g| g.core.resolve.thirds), m.status.goofy.as_ref().map_or(0, |g| g.core.string), m.status.goofy.as_ref().map_or(0.0, |g| g.core.hop_cd), m.state, s)
            }
            ["stagger"] => format!("goofy: stagger {}", self.gof.book.summary()),
            ["threat"] => format!("goofy: {}", self.gof.threat.summary()),
            ["stagger", who, n] => {
                let lock = m.pad.as_ref().map_or(TargetId::NONE, |d| d.lock);
                // (FINAL-FIX) `other`: the nearest enemy within his walk range that is not his own lock (goofy/hooks.rs WORD_OTHER)
                let target = match *who {
                    "lock" => lock,
                    "other" => crate::goofy::hooks::WORD_OTHER,
                    _ => TargetId(who.parse::<u32>().unwrap_or(0)),
                };
                self.gof.word.push((target, n.parse::<f32>().unwrap_or(0.0)));
                if target == crate::goofy::hooks::WORD_OTHER {
                    format!("goofy: stagger {n} on the nearest enemy that is not his lock (within his walk range)")
                } else {
                    format!("goofy: stagger {n} on target {} (0: the nearest)", target.0)
                }
            }
            ["kit"] => {
                let list = s.abilities.list().iter().map(|(k, on)| format!("{}{}", k.label(), if *on { "*" } else { "" })).collect::<Vec<_>>().join(", ");
                format!("goofy: kit {list}; AP {}/{}", m.status.ap_in_use(), m.status.max_ap())
            }
            _ => format!("goofy: {}: gauges | kit | give ABILITY | resolve N | rank MOVE N | stagger [lock|near|other|TARGET N]", words.join(" ")),
        }
    }

}

#[cfg(test)]
#[path = "goofy_tests.rs"]
mod tests;
