//! (party X3G) GOOFY AS THE CONTROLLED LEADER: the shield guard, the counter chain and the finishers
//! (build/sora_tmp/party/XD_PLAYABLE_KITS.md section 2, approved by the user 2026-10-15 with its 10 recommended
//! defaults: decision 4 the solo multipliers, 5 the held guard, 6 the tiers follow the magic tiers, 8 Tornado Copter
//! with Sora's Glide, 10 the clip fallback decided by capture). KH3 has no playable Goofy: every RULE here is the port's
//! design, the PIECES are KH3's (his n_ex002 clips, the Trinity Guard / Goofy Bombardier link pawns' shots of stream D3,
//! the Fusion Spin actor of stream D6b, Sora's attack rows). Every number is a row of `sheets/port/party-kit-goofy.json`
//! (`goofy_kit::K`).
//!
//! THE MOVES (pad, Xbox layout; keyboard through the host's mapping):
//! - X held (stick neutral when pressed): SHIELD GUARD (`SGuardIdle` / `SGuardWalk` at 180 cm/s), frontal 120 deg,
//!   with a gauge that breaks when empty (`SGuardDamage`). A block within 0.25 s of raising it is a PERFECT GUARD: no
//!   cost, `VN_slg_hit0`, a 1 s counter window.
//! - A in guard, in the counter window, or within 0.2 s of letting go of X: the COUNTER CHAIN `SGuardShieldBash`,
//!   `SGuardShieldBash2`, then `SGuardHeavyBash` (hold A 0.6 s; it uses the whole gauge). Holding A 0.6 s from a guard
//!   bash turns it into the Heavy Bash too. With Shield Charge, A with the stick pushed while guarding is the charge.
//! - X + stick on the ground: HOP (`HopL` / `HopR`, 300 cm, invincible 0.03 - 0.30 s). In the air X + stick: SHIELD
//!   SURF (`M_1325` without its Attack window, 0.35 s, once per airtime); X held in the air: AIR GUARD
//!   (`SGuardJumpRise` / `SGuardFall`), A there: `SGuardAirShieldBash`.
//! - The combo: Push, Sweep, Jump Strike (no target: a point ahead), FINISHER: Fusion Spin (crowded or no lock), Fusion
//!   Rocket (the target 3 - 12 m away), or `SGuardShieldBash2` with no Fusion worn. Air: strike, strike, BOMBARDIER
//!   DIVE (`GShootRise`, a -45 deg dive in `GShootEnd`, the explosion at the landing). Finishers are invincible until
//!   their release.
//! - Y on the Shield Guardian offer: 20 s of the Trinity Guard moveset (no X needed, frontal blocks free, no
//!   knock-down), ended by a free Heavy Bash (Y or the time).
//! - RB held: DISCUS BARRAGE (Sora's aim rules, up to 8 locks; A or letting go of RB throws: the shield bounces lock to
//!   lock at x0.5).
//! - B held while falling: TORNADO COPTER (with Sora's Glide).
//! - Tornado / Turbo / Discus tiers (the menu's `PadState::x2.tier`, X2): steer / pull / launch, hop / speed-up / BLOW,
//!   ricochet / shockwave.
//! - Feel: a melee step, a bash or a finisher after its release may be cancelled into the Hop, the guard or a jump 0.05 s
//!   after its first hit landed (or its window closed).
//!
//! Hooks (marked "(party X3G)" in the shared files): pad/mod.rs `decide` / `pre_move` / `tick` / `pad_menu`, pawn.rs
//! `receive_hit` (guard, invincibility, Stalwart), actions.rs (`begin` pays nothing for the barrage, Tornado steer,
//! `deliver` the pull and the landed-hit count), party.rs (the Discus bounces, the party step).

use super::goofy_kit::{GoofyAbility as GA, K};
use super::goofy_shots::{self as shots, Area, Push, SoloRow};
use super::{enter_jump, enter_run, issue, AIR_ATTACK, JUMP, RUN};
use crate::actions::ActionRun;
use crate::command::{ActionDef, MemberCommand, TargetRef};
use crate::host::PartyHost;
use crate::ids::Chara;
use crate::pawn::{forward, horizontal, wrap180, yaw_of, NpcPawn, StepCtx};
use crate::states::NpcStateId;
use glam::{Vec2, Vec3};
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{HitRecord, HitResult, TargetId};
use kh_core::input::PadButton;
use kh_core::params::SORA_PARAMS;

/// Goofy's pad states (`NpcStateId::Pad(n)`; stream P uses 1..3, X3D its own range).
pub const GUARD: u8 = 40;
pub const GUARD_HIT: u8 = 41;
pub const GUARD_BREAK: u8 = 42;
pub const BASH: u8 = 43;
pub const HOP: u8 = 44;
pub const SURF: u8 = 45;
pub const FINISH: u8 = 46;
pub const DIVE: u8 = 47;
pub const COPTER: u8 = 48;
pub const CHARGE: u8 = 49;
pub const AIM: u8 = 50;
pub const AIR_GUARD: u8 = 51;
/// (X3G-b) the aerial recovery out of a blow.
pub const RECOVER: u8 = 52;

pub fn owns_state(n: u8) -> bool {
    (GUARD..=RECOVER).contains(&n) || n == super::goofy_combo::COMBO || n == super::goofy_combo::AIR_COMBO // (GOOFY-GK)
}

use super::goofy_combo::{AIR_COMBO, COMBO}; // (GOOFY-GK)

/// Which bash is running.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BashKind {
    #[default]
    Bash1,
    Bash2,
    Heavy,
    Air,
    /// The 4th combo step with no Fusion worn (SGuardShieldBash2).
    Plain,
}

/// Which finisher is running.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Finisher {
    #[default]
    Spin,
    Rocket,
}

/// The taunt of Over Here! (who reads it: the hosts' enemy targeting, `Party::goofy_taunt`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Taunt {
    pub centre: Vec3,
    pub radius: f32,
    pub left: f32,
}

/// Goofy's kit state on the pad (`PadState::goofy`).
#[derive(Clone, Debug, PartialEq)]
pub struct GoofyPad {
    /// (GOOFY-GK) the shield's meter is GG's GUARD (`FriendStatus::guard`, crate::guard); the kit keeps none of its own.
    /// (GOOFY-GK) the ground / air combo (pad/goofy_combo.rs).
    pub combo: super::goofy_combo::ComboRun,
    /// (GOOFY-GK) hit-stop left (his clip and lunge frozen), the Hop cooldown left and a Hop press kept for its end (the
    /// stick yaw, seconds it is kept), the Bombardier (Y) cooldown left and its request, Brace stock and its age.
    pub hitstop: f32,
    pub hop_cd: f32,
    pub hop_queued: Option<(f32, f32)>,
    pub bomb_cd: f32,
    pub bomb_request: bool,
    pub brace: u8,
    pub brace_age: f32,
    /// (GOOFY-GK) the state at the last pre-move (the cleanup when something else took him out of a kit state).
    pub last_state: NpcStateId,
    /// Seconds since the shield came up (the Perfect Guard window), the counter window left, seconds since a block.
    pub guard_age: f32,
    pub counter: f32,
    pub since_block: f32,
    /// Seconds since X was let go (the release grace); X press buffer.
    pub x_release: f32,
    pub x_buffer: f32,
    /// The A press is the kit's (decided in `decide`, run in `pre_move`).
    pub a_request: bool,
    /// The counter chain: bashes done, the running bash, its hit delivered, it came from a held guard, A held time.
    pub chain: u8,
    pub bash: BashKind,
    pub bash_hit: Option<f32>,
    pub from_guard: bool,
    pub a_hold: f32,
    /// (GOOFY-GK) this Heavy Bash paid its GUARD at the start (the guard-hold one).
    pub heavy_paid: bool,
    pub payback: bool,
    /// (GOOFY-GK) the running guard bash: its power scale (Brace, the counter), forced BLOW, its lunge (cm/s, world).
    pub bash_mult: f32,
    pub bash_blow: bool,
    pub bash_lunge: Vec2,
    /// Hop / Surf / Charge direction; surfs used this airtime; the dive used this airtime.
    pub dir: Vec3,
    pub surf_used: u8,
    pub dive_used: bool,
    pub dive_sub: u8,
    /// (GOOFY-GK) the rise of the running Bombardier (ground / air).
    pub dive_rise: f32,
    /// The finisher: which, queued after the Jump Strike, its release passed, the rush's root scale.
    pub fin: Finisher,
    pub fin_queued: bool,
    pub released: bool,
    pub root_scale: f32,
    pub root_prev: Vec2,
    /// Invincible seconds left; `hold_invincible` = until the running move's release.
    pub invincible: f32,
    pub hold_invincible: bool,
    /// The state time of the running melee's first landed hit (the 0.05 s cancel).
    pub hit_at: Option<f32>,
    /// Shield Guardian: the gauge, seconds left, the finish asked, the start asked.
    pub mode_gauge: f32,
    pub mode_left: f32,
    pub mode_finish: bool,
    pub mode_start: bool,
    pub taunt: Option<Taunt>,
    /// Discus Barrage: focus, RB held time, the lock before this RB press, the locks, the lock timer.
    pub focus: f32,
    pub rb_held: f32,
    pub lock_prev: TargetId,
    pub lock_before_press: TargetId,
    pub locks: Vec<TargetId>,
    pub lock_timer: f32,
    /// The aim is open (RB's release is not a lock tap).
    pub aiming: bool,
    /// The running throw's bounce queue (Barrage locks), the ricochets left, the targets bounced from, the power of
    /// the bounces (1 the first throw).
    pub queue: Vec<TargetId>,
    pub bounces: u8,
    pub bounced: Vec<TargetId>,
    pub bounce_mult: f32,
    pub pays_nothing: bool,
    /// The running throw is Discus Barrage.
    pub barrage: bool,
    pub barrage_next: bool,
    /// Targets the bouncing shield may hit again once the timer runs out.
    pub rehit: Vec<(TargetId, f32)>,
    /// The running special's tier and its one-shot extras done.
    pub tier: u8,
    pub extra: u8,
    /// The kit's live hit spheres.
    pub shots: Vec<Area>,
    /// The mesh pitch (UE degrees) while diving: the hosts' drawing (`Party::pad_mesh_pitch`).
    pub pitch: Option<f32>,
    /// The copter's persistent effect; the solo Fusion Spin's swirls.
    pub copter_fx: u32,
    pub fin_fx: Vec<u32>,
    /// Effects a hit on him asked for (spawned at the next pre-move: the hit has no host).
    pub fx: Vec<(&'static str, Vec3)>,
    /// (GOOFY-GK) sounds a hit on him asked for (the block clang), played at the next pre-move.
    pub sfx: Vec<(&'static str, Vec3)>,
    /// (X3G-b) Sora owns AIR_RECOVERY (the leader feed; true with no feed).
    pub air_recovery: bool,
    /// What the kit did (tests, the trace), newest last.
    pub log: Vec<&'static str>,
}

impl Default for GoofyPad {
    fn default() -> GoofyPad {
        GoofyPad {
            combo: Default::default(),
            hitstop: 0.0,
            hop_cd: 0.0,
            hop_queued: None,
            bomb_cd: 0.0,
            bomb_request: false,
            brace: 0,
            brace_age: 0.0,
            last_state: NpcStateId::Idle,
            guard_age: 99.0,
            counter: 0.0,
            since_block: 99.0,
            x_release: 99.0,
            x_buffer: 0.0,
            a_request: false,
            chain: 0,
            bash: BashKind::Bash1,
            bash_hit: None,
            from_guard: false,
            a_hold: 0.0,
            heavy_paid: false,
            payback: false,
            bash_mult: 1.0,
            bash_blow: false,
            bash_lunge: Vec2::ZERO,
            dir: Vec3::X,
            surf_used: 0,
            dive_used: false,
            dive_sub: 0,
            dive_rise: 0.0,
            fin: Finisher::Spin,
            fin_queued: false,
            released: false,
            root_scale: 1.0,
            root_prev: Vec2::ZERO,
            invincible: 0.0,
            hold_invincible: false,
            hit_at: None,
            mode_gauge: 0.0,
            mode_left: 0.0,
            mode_finish: false,
            mode_start: false,
            taunt: None,
            focus: K.barrage_focus_max,
            rb_held: 0.0,
            lock_prev: TargetId::NONE,
            lock_before_press: TargetId::NONE,
            locks: Vec::new(),
            lock_timer: 0.0,
            aiming: false,
            queue: Vec::new(),
            bounces: 0,
            bounced: Vec::new(),
            bounce_mult: 1.0,
            pays_nothing: false,
            barrage: false,
            barrage_next: false,
            rehit: Vec::new(),
            tier: 1,
            extra: 0,
            shots: Vec::new(),
            pitch: None,
            copter_fx: 0,
            fin_fx: Vec::new(),
            fx: Vec::new(),
            sfx: Vec::new(),
            air_recovery: true,
            log: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------------------------- small helpers

fn is_goofy(p: &NpcPawn) -> bool {
    p.chara == Chara::Goofy && p.pad.is_some()
}

fn gp(p: &mut NpcPawn) -> &mut GoofyPad {
    &mut p.pad.as_mut().expect("pad").goofy
}

fn g(p: &NpcPawn) -> &GoofyPad {
    &p.pad.as_ref().expect("pad").goofy
}

fn has(p: &NpcPawn, k: GA) -> bool {
    p.status.goofy_kit.has(k)
}

/// (GOOFY-GK) GUARD (GG's meter): he can raise the shield.
pub(super) fn guard_ok(p: &NpcPawn) -> bool {
    p.status.guard.is_some_and(|g| g.can_guard())
}

/// (GOOFY-GK) GUARD is broken.
pub(super) fn guard_broken(p: &NpcPawn) -> bool {
    p.status.guard.is_some_and(|g| g.broken)
}

/// (GOOFY-GK) for pad/goofy_combo.rs.
pub(super) fn log_pub(p: &mut NpcPawn, w: &'static str) {
    log(p, w);
}

/// (GOOFY-GK) for pad/goofy_combo.rs: out to the run / idle / guard / fall.
pub(super) fn leave_pub(p: &mut NpcPawn) {
    leave_to_ground(p);
}

fn log(p: &mut NpcPawn, w: &'static str) {
    let l = &mut gp(p).log;
    l.push(w);
    if l.len() > 64 {
        l.remove(0);
    }
}

fn state_n(p: &NpcPawn) -> Option<u8> {
    match p.state {
        NpcStateId::Pad(n) => Some(n),
        _ => None,
    }
}

fn in_state(p: &NpcPawn, n: u8) -> bool {
    p.state == NpcStateId::Pad(n)
}

fn stick(p: &NpcPawn) -> (f32, f32, bool) {
    let d = p.pad.as_ref().expect("pad");
    (d.stick_yaw, d.stick_magnitude, d.stick_magnitude >= SORA_PARAMS.input_stick_active_min)
}

fn held(p: &NpcPawn, b: PadButton) -> bool {
    p.pad.as_ref().is_some_and(|d| d.frame.pad.held.has(b))
}

fn pressed(p: &NpcPawn, b: PadButton) -> bool {
    p.pad.as_ref().is_some_and(|d| d.frame.pad.pressed.has(b))
}

fn lb(p: &NpcPawn) -> bool {
    held(p, PadButton::LeftShoulder) || pressed(p, PadButton::LeftShoulder)
}

/// A clip of his set by name.
fn clip(name: &str) -> &'static crate::anim::ClipDef {
    crate::tables::clip(Chara::Goofy, name)
}

/// A copy of a clip without some notify classes / its root motion (cached, leaked once: a handful per run).
pub(super) fn clip_copy(name: &'static str, drop_attack: bool, drop_effects: bool, drop_root: bool) -> &'static crate::anim::ClipDef {
    clip_copy_at(name, drop_attack, drop_effects, drop_root, None)
}

/// `clip_copy` that also leaves out the one AttachEffect state starting at `effect_at` (the routed rocket trail).
fn clip_copy_at(name: &'static str, drop_attack: bool, drop_effects: bool, drop_root: bool, effect_at: Option<f32>) -> &'static crate::anim::ClipDef {
    use std::sync::Mutex;
    static CLIPS: Mutex<Vec<(&'static str, u8, &'static crate::anim::ClipDef)>> = Mutex::new(Vec::new());
    let key = drop_attack as u8 | (drop_effects as u8) << 1 | (drop_root as u8) << 2 | (effect_at.is_some() as u8) << 3;
    let mut all = CLIPS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = all.iter().find(|c| c.0 == name && c.1 == key) {
        return c.2;
    }
    let base = clip(name);
    let notifies: Vec<crate::anim::ClipNotify> = base
        .notifies
        .iter()
        .copied()
        .filter(|n| !(drop_attack && (matches!(n.data, crate::anim::ClipNotifyData::Attack { .. }) || n.class.contains("Traction"))))
        .filter(|n| !(drop_effects && n.class.contains("AttachEffect")))
        .filter(|n| !(effect_at.is_some_and(|t| n.class.contains("AttachEffect") && (n.start - t).abs() < 0.01)))
        .collect();
    let notifies: &'static [crate::anim::ClipNotify] = Box::leak(notifies.into_boxed_slice());
    let c: &'static crate::anim::ClipDef = Box::leak(Box::new(crate::anim::ClipDef { notifies, root_motion: base.root_motion && !drop_root, root: if drop_root { &[] } else { base.root }, ..*base }));
    all.push((name, key, c));
    c
}

/// The target point a move turns to: the hard lock, else the soft target.
fn target_info(p: &NpcPawn, host: &PartyHost) -> Option<kh_core::host::TargetInfo> {
    match p.pad.as_ref()?.target() {
        TargetRef::Enemy(t) => host.targets.info(t),
        _ => None,
    }
}

/// Turns him toward the lock / soft target, else the held stick, by at most `max` degrees.
fn face(p: &mut NpcPawn, host: &PartyHost, max: f32) {
    let want = target_info(p, host).map(|t| horizontal(t.position - p.position())).filter(|d| d.length() > 1.0).map(yaw_of).or_else(|| {
        let (y, _, active) = stick(p);
        active.then_some(y)
    });
    if let Some(w) = want {
        let d = wrap180(w - p.yaw);
        p.yaw = wrap180(p.yaw + d.clamp(-max, max));
    }
}

fn leave_to_ground(p: &mut NpcPawn) {
    let (yaw, mag, active) = stick(p);
    p.cmc.speed_limit_override = None;
    if !p.on_ground() {
        enter_jump(p, true);
    } else if held(p, PadButton::FaceLeft) && guard_ok(p) {
        start_guard(p);
    } else if active {
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

/// The A / Y presses that are the kit's (pad/mod.rs `decide`, before stream P's combo). True = the A press is taken.
/// (GOOFY-GK) EVERY A of a pad Goofy is the kit's now: his own ground / air combo (pad/goofy_combo.rs) replaces stream
/// P's ActionDef combo (Push, Sweep, Jump Strike) for the leader; Y is Bombardier (the Shield Guardian offer is gone from
/// his Y: the user, 2026-10-17 00:40).
pub fn decide(p: &mut NpcPawn, target: TargetRef) -> bool {
    let _ = target;
    if !is_goofy(p) || p.state == NpcStateId::Die {
        return false;
    }
    let st = state_n(p);
    let grounded = p.on_ground();
    let def = p.action.and_then(|r| r.def);
    // (GOOFY-GK) Y: Bombardier (not while AI Sora's team attack holds the slot, nor on the LB shortcut panel)
    let y = pressed(p, PadButton::FaceTop) && !held(p, PadButton::LeftShoulder);
    let sora_busy = p.pad.as_ref().is_some_and(|d| d.menu.leader.sora_busy);
    if y && !sora_busy && !matches!(p.state, NpcStateId::Fr(_)) /* a team attack running: Y is its own (Trinity Guard's Finish) */ {
        let pad = p.pad.as_mut().expect("pad");
        pad.x2.y_taken = true;
        if pad.goofy.bomb_cd > 0.0 {
            pad.menu.events.push(kh_core::command::MenuEvent::Refused);
        } else {
            pad.goofy.bomb_request = true;
        }
    }
    let buffered = p.pad.as_ref().expect("pad").buffered;
    if buffered <= 0.0 {
        return false;
    }
    let k = g(p).clone_light();
    let guard_window = k.counter > 0.0 || k.x_release < K.guard_release_grace;
    let take = |p: &mut NpcPawn| {
        let pad = p.pad.as_mut().expect("pad");
        pad.goofy.a_request = true;
        pad.buffered = 0.0;
        true
    };
    let wait = |p: &mut NpcPawn| {
        let pad = p.pad.as_mut().expect("pad");
        pad.buffered = (pad.buffered - 1.0 / 60.0).max(0.0);
        true
    };
    match st {
        Some(GUARD | GUARD_HIT | AIR_GUARD | AIM) => return take(p),
        // the chain: the next bash once this one has hit (a press before it waits)
        Some(BASH) => {
            if k.bash_hit && k.bash != BashKind::Heavy {
                return take(p);
            }
            return wait(p);
        }
        // (GOOFY-GK) a combo step: the press is queued for its link point
        Some(COMBO | AIR_COMBO) => return take(p),
        Some(FINISH | DIVE | CHARGE | GUARD_BREAK | RECOVER) => return wait(p),
        Some(HOP) => return if cancel_open(p) { take(p) } else { wait(p) },
        _ => {}
    }
    // the counter window, the release grace: A bashes instead of starting the combo
    if guard_window && p_state_free(st, def) {
        return take(p);
    }
    // (X3G-b) the recovery's air counter (Sora's BLOW_COUNTER window): A in the air is the air bash
    if k.counter > 0.0 && !grounded && st == Some(JUMP) {
        return take(p);
    }
    // A out of a slide (a slope's SuperSlide): the slide ends, the combo starts
    if p.state == NpcStateId::SuperSlide && grounded {
        crate::traverse::end(p);
        p.to_idle();
        log(p, "slide_attack");
        return take(p);
    }
    // (GOOFY-GK) A during Turbo's ride: the ride ends into Big Bash (the link); during his other specials it waits
    if p.state == NpcStateId::Action {
        if def == Some(ActionDef::GoofyTurbo) && p.action.is_some_and(|r| r.sub == 1) {
            return take(p);
        }
        return wait(p);
    }
    // (GOOFY-GK) free on the ground or in the air: the combo
    let free_air = !grounded && matches!(st, Some(JUMP | COPTER | AIR_ATTACK));
    if super::free(p) || free_air {
        return take(p);
    }
    wait(p)
}

/// A few of the kit's flags, copied (decide reads them while it changes the pad).
#[derive(Clone, Copy, Debug)]
struct Light {
    counter: f32,
    x_release: f32,
    bash_hit: bool,
    bash: BashKind,
}

impl GoofyPad {
    fn clone_light(&self) -> Light {
        Light { counter: self.counter, x_release: self.x_release, bash_hit: self.bash_hit.is_some(), bash: self.bash }
    }
}

/// The states from which the counter window's A is a bash (free, or a melee's chain point).
fn p_state_free(st: Option<u8>, def: Option<ActionDef>) -> bool {
    st.is_none() && def.is_none() || matches!(st, Some(RUN | HOP))
}

// ---------------------------------------------------------------------------------------------- pre-move

/// After the command intake, before the movement tick (pad/mod.rs `pre_move`). True = the kit took the state (stream
/// P's state switch is skipped this step).
pub fn pre_move(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) -> bool {
    if !is_goofy(p) {
        return false;
    }
    let dt = cx.dt;
    timers(p, dt);
    if !in_state(p, COPTER) && g(p).copter_fx != 0 {
        stop_copter_fx(p, host);
    }
    if !in_state(p, FINISH) && !g(p).fin_fx.is_empty() {
        stop_fin_fx(p, host);
    }
    if !in_state(p, DIVE) {
        gp(p).pitch = None;
    }
    for (name, at) in std::mem::take(&mut gp(p).fx) {
        shots::effect(p, host, name, at, 1.0);
    }
    for (name, at) in std::mem::take(&mut gp(p).sfx) {
        shots::sound(p, host, name, at); // (GOOFY-GK)
    }
    shots::step_areas(p, host, dt);
    if p.state == NpcStateId::Die {
        end_mode(p);
        return false;
    }
    if p.on_ground() && !in_state(p, SURF) && !in_state(p, DIVE) {
        let k = gp(p);
        k.surf_used = 0;
        k.dive_used = false;
    }
    // a new state this step: the melee's hit clock starts over
    if p.state_time == 0.0 && !owns_state(state_n(p).unwrap_or(0)) {
        gp(p).hit_at = None;
    }
    // (GOOFY-GK) SOMETHING ELSE TOOK HIM OUT OF A KIT STATE (a hit's reaction, an ability, an item): the hover, the
    // lunge, the clip rate, the hit-stop and the finisher's invincibility of that state end with it
    let last = g(p).last_state;
    if matches!(last, NpcStateId::Pad(n) if owns_state(n)) && !matches!(p.state, NpcStateId::Pad(n) if owns_state(n)) {
        if matches!(last, NpcStateId::Pad(COMBO | AIR_COMBO)) {
            super::goofy_combo::end(p);
            gp(p).combo.n = 0;
        }
        let k = gp(p);
        k.hitstop = 0.0;
        k.hold_invincible = false;
    }
    if p.on_ground() && !in_state(p, AIR_COMBO) {
        gp(p).combo.air_n = 0;
    }
    // (GOOFY-GK) Y: Bombardier (decided in `decide`), from free, a Hop's cancel point, the guard, a combo step's link point
    if std::mem::take(&mut gp(p).bomb_request) {
        let st = state_n(p);
        let can = super::free(p) || matches!(st, Some(JUMP | COPTER | GUARD | AIR_GUARD)) || super::goofy_combo::link_open(p) || (st == Some(HOP) && cancel_open(p));
        if can && g(p).bomb_cd <= 0.0 {
            if in_state(p, COMBO) || in_state(p, AIR_COMBO) {
                super::goofy_combo::end(p);
            }
            stop_copter_fx(p, host);
            p.cmc.speed_limit_override = None;
            start_dive(p, host);
            gp(p).last_state = p.state;
            return true;
        }
        if let Some(d) = p.pad.as_mut() {
            d.menu.events.push(kh_core::command::MenuEvent::Refused);
        }
    }
    // (FX1) THE HOST'S LATE "NO" (the team move it said "yes" to never started): his own finisher now
    // (`goofy_combo::finisher`) when he is free on the ground or at a combo step's link point; else it waits
    // (LATE_NO_TIME)
    if p.pad.as_ref().is_some_and(|d| d.finisher_late_no > 0.0) {
        let in_chain = super::goofy_combo::link_open(p) && p.on_ground();
        if (super::free(p) && p.on_ground()) || in_chain {
            if let Some(d) = p.pad.as_mut() {
                (d.finisher_late_no, d.buffered, d.combo, d.finisher_answer, d.finisher_pending) = (0.0, 0.0, 0, None, false);
            }
            log(p, "late_no_finisher");
            if in_state(p, COMBO) {
                super::goofy_combo::end(p);
            }
            super::goofy_combo::finisher(p, host);
            gp(p).last_state = p.state;
            return true;
        }
        if let Some(d) = p.pad.as_mut() {
            d.finisher_late_no = (d.finisher_late_no - dt).max(0.0);
        }
    }
    specials(p, cx, host);
    // (X3G-b) the Superslide (B4's NPC SuperSlide state): the stick feeds it, B jumps out
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
    // (X3G-b) the aerial recovery: B while blown back in the air (Sora's AIR_RECOVERY rule)
    if p.state == NpcStateId::Damage && !p.on_ground() && p.state_time >= K.recover_delay && pressed(p, PadButton::JUMP) && !lb(p) && g(p).air_recovery && blown(p) {
        start_recover(p, host);
        return true;
    }
    let took = pre_move_inputs(p, cx, host);
    gp(p).last_state = p.state;
    took
}

/// (GOOFY-GK) the button part of the pre-move (split off so the kit's last state is noted on every exit).
fn pre_move_inputs(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) -> bool {
    if aim(p, cx, host) {
        return true;
    }
    // X (not the LB + X shortcut)
    if pressed(p, PadButton::FaceLeft) && !lb(p) {
        gp(p).x_buffer = K.feel_buffer;
    }
    if gp(p).x_buffer > 0.0 && try_x(p, host) {
        gp(p).x_buffer = 0.0;
        return true;
    }
    // (GOOFY-GK) a Hop pressed during its cooldown's last moments: it hops when the cooldown ends
    if let Some((yaw, left)) = g(p).hop_queued {
        let left = left - cx.dt;
        gp(p).hop_queued = (left > 0.0).then_some((yaw, left));
        if g(p).hop_cd <= 0.0 && hop_allowed(p) {
            gp(p).hop_queued = None;
            cancel_action(p);
            start_hop(p, yaw);
            return true;
        }
    }
    // B out of a cancel window: the jump
    let sub_sel = p.pad.as_ref().is_some_and(|d| d.menu.state == kh_core::command::MenuState::SubSel);
    if pressed(p, PadButton::JUMP) && !lb(p) && !sub_sel && p.on_ground() && cancel_open(p) && !in_state(p, GUARD) && (!in_state(p, COMBO) || super::goofy_combo::link_open(p)) /* (GOOFY-GK) a step: B from its link point */ {
        cancel_action(p);
        enter_jump(p, false);
        log(p, "cancel_jump");
        return false;
    }
    if gp(p).a_request {
        gp(p).a_request = false;
        if do_a(p, host) {
            return true;
        }
    }
    // Tornado Copter: B held while falling
    if in_state(p, JUMP) && held(p, PadButton::JUMP) && !p.on_ground() && p.cmc.velocity.z < 0.0 && copter_unlocked(p) && p.pad.as_ref().is_some_and(|d| d.jump.sub == 4) {
        start_copter(p, host);
        return true;
    }
    state_n(p).is_some_and(owns_state)
}

fn timers(p: &mut NpcPawn, dt: f32) {
    let guarding = matches!(state_n(p), Some(GUARD | GUARD_HIT | AIR_GUARD | CHARGE));
    let x_held = held(p, PadButton::FaceLeft);
    let mode = gp(p).mode_left > 0.0;
    let st_time = p.state_time;
    let k = gp(p);
    k.x_buffer = (k.x_buffer - dt).max(0.0);
    k.counter = (k.counter - dt).max(0.0);
    k.since_block += dt;
    k.invincible = (k.invincible - dt).max(0.0);
    k.guard_age += dt;
    if !x_held {
        k.x_release += dt;
    } else {
        k.x_release = 99.0;
    }
    let _ = st_time;
    // (GOOFY-GK) the cooldowns, the Brace's life
    k.hop_cd = (k.hop_cd - dt).max(0.0);
    k.bomb_cd = (k.bomb_cd - dt).max(0.0);
    k.brace_age += dt;
    if k.brace_age > K.brace_keep {
        k.brace = 0;
    }
    // (GOOFY-GK) GUARD (GG's meter; FriendStatus::tick refills it): held up it drains slowly and holds the refill
    let _ = mode;
    if guarding {
        if let Some(gd) = p.status.guard.as_mut() {
            if !gd.drain_per_sec(K.guard_hold_drain, dt) && !matches!(state_n(p), Some(GUARD_BREAK)) {
                start_guard_break(p);
            }
        }
    }
    let k = gp(p);
    if let Some(t) = k.taunt.as_mut() {
        t.left -= dt;
    }
    if k.taunt.is_some_and(|t| t.left <= 0.0) {
        k.taunt = None;
    }
    if !matches!(state_n(p), Some(AIM)) {
        let k = gp(p);
        k.focus = (k.focus + K.barrage_focus_regen * dt).min(K.barrage_focus_max);
    }
    if mode {
        let k = gp(p);
        k.mode_left -= dt;
        if k.mode_left <= 0.0 {
            k.mode_left = 0.001;
            k.mode_finish = true;
        }
    }
}

//// The cancel point of a bash / finisher after its first hit (or its window's end); (GOOFY-GK) a combo step's hit
/// confirm / link point; a Hop's end; the end clip of Tornado / Turbo.
pub fn cancel_open(p: &NpcPawn) -> bool {
    let Some(pad) = p.pad.as_ref() else { return false };
    let k = &pad.goofy;
    let after = |t: Option<f32>| t.is_some_and(|t| p.state_time >= t + K.feel_cancel_after_hit);
    if k.hitstop > 0.0 {
        return false;
    }
    match p.state {
        NpcStateId::Action => {
            let def = p.action.and_then(|r| r.def);
            // (GOOFY-GK) the end clip of Tornado (M_1323) / Turbo (M_1326) after `tornado.end_cancel`
            if matches!(def, Some(ActionDef::GoofyTornado | ActionDef::GoofyTurbo)) && p.action.is_some_and(|r| r.sub == 2) {
                return p.anim.time >= K.tornado_end_cancel;
            }
            if !matches!(def, Some(ActionDef::GoofyPush | ActionDef::GoofySweep | ActionDef::GoofyJump)) {
                return false;
            }
            let window_end = super::attack_window_end(p).is_some_and(|e| p.anim.time >= e + K.feel_cancel_after_hit);
            after(k.hit_at) || window_end
        }
        NpcStateId::Pad(BASH) => k.bash != BashKind::Heavy && k.bash_hit.is_some_and(|t| p.anim.time >= t + K.feel_cancel_after_hit) || k.bash == BashKind::Heavy && k.bash_hit.is_some() && p.anim.time >= K.heavy_hit_time + 0.3,
        NpcStateId::Pad(FINISH) => k.released && after(k.hit_at.or(Some(release_time(k.fin)))),
        NpcStateId::Pad(HOP) => p.anim.time >= K.hop_cancel,
        NpcStateId::Pad(COMBO | AIR_COMBO) => super::goofy_combo::confirm_open(p),
        _ => false,
    }
}

fn release_time(f: Finisher) -> f32 {
    match f {
        Finisher::Spin => K.fin_spin_time,
        Finisher::Rocket => K.fin_rocket_release,
    }
}

/// Ends the running action / kit state for a cancel (the definition's End, the hover off).
fn cancel_action(p: &mut NpcPawn) {
    if p.state == NpcStateId::Action {
        p.end_action();
        p.action = None;
        p.cmc.direct_xy = None;
        p.cmc.gravity_scale = p.params.gravity_scale;
        p.cmc.gravity_off = false;
        p.cmc.direct_z = None;
        p.anim.speed = 1.0;
    }
    if matches!(state_n(p), Some(BASH | FINISH)) {
        p.cmc.gravity_off = false;
        p.cmc.direct_z = None;
        p.anim.speed = 1.0;
        let k = gp(p);
        k.hold_invincible = false;
        k.chain = 0;
    }
    // (GOOFY-GK) a combo step
    if matches!(state_n(p), Some(COMBO | AIR_COMBO)) {
        super::goofy_combo::end(p);
    }
    if let Some(d) = p.pad.as_mut() {
        d.combo = 0;
    }
}

/// (GOOFY-GK) pad/mod.rs `decide`, the Tiered arm: an ability (Tornado / Turbo / Discus, LB or the list) may start now
/// because a combo step reached its link point, a bash / Hop / finisher its cancel point, or he guards. The state the
/// action interrupts is cleaned up by the next pre-move (`last_state`).
pub fn link_for_ability(p: &NpcPawn) -> bool {
    if !is_goofy(p) {
        return false;
    }
    super::goofy_combo::link_open(p) || (matches!(state_n(p), Some(BASH | HOP | FINISH)) && cancel_open(p)) || matches!(state_n(p), Some(GUARD))
}

/// (GOOFY-GK) The Hop may start from here (free on the ground, a cancel point, a blocked hit's recoil).
fn hop_allowed(p: &NpcPawn) -> bool {
    let st = state_n(p);
    let landing = st == Some(JUMP) && p.pad.as_ref().is_some_and(|d| d.jump.sub >= 10);
    p.on_ground() && (p.state == NpcStateId::Idle || st == Some(RUN) || landing || st == Some(GUARD_HIT) || st == Some(GUARD) || cancel_open(p))
}

/// X: Hop / guard on the ground, the air guard in the air, from the states that allow it. (GOOFY-GK) no Superslide on X
/// while running and no Shield Surf in the air any more (the user: "remove his shield surf when you press X while
/// running", "Remove Goofy's air dodge"); the Hop has a cooldown.
fn try_x(p: &mut NpcPawn, host: &PartyHost) -> bool {
    let st = state_n(p);
    let grounded = p.on_ground();
    let landing = st == Some(JUMP) && p.pad.as_ref().is_some_and(|d| d.jump.sub >= 10);
    let free_ground = p.state == NpcStateId::Idle || st == Some(RUN) || landing;
    let free_air = matches!(st, Some(JUMP | COPTER)) && !grounded && !landing;
    let cancel = cancel_open(p);
    let allowed = free_ground || free_air || cancel || (st == Some(GUARD_HIT));
    if !allowed {
        return false;
    }
    let (yaw, _, active) = stick(p);
    let _ = host;
    if grounded {
        if active {
            if g(p).hop_cd > 0.0 {
                // (GOOFY-GK) the cooldown: a late press is kept for its end, an early one refused (KH3's refused SE)
                if g(p).hop_cd <= K.hop_buffer {
                    gp(p).hop_queued = Some((yaw, K.hop_buffer + 0.02));
                } else if let Some(d) = p.pad.as_mut() {
                    d.menu.events.push(kh_core::command::MenuEvent::Refused);
                    log(p, "hop_refused");
                }
                return true;
            }
            cancel_action(p);
            start_hop(p, yaw);
            return true;
        }
        if !guard_ok(p) {
            return false;
        }
        cancel_action(p);
        start_guard(p);
        return true;
    }
    if !guard_ok(p) {
        return false;
    }
    cancel_action(p);
    end_copter(p);
    start_air_guard(p);
    true
}

/// The kit's A (decided in `decide`).
fn do_a(p: &mut NpcPawn, host: &mut PartyHost) -> bool {
    let st = state_n(p);
    let grounded = p.on_ground();
    if st == Some(AIM) {
        return fire_barrage(p, host);
    }
    // (GOOFY-GK) Turbo's ride: A ends it into Big Bash (the link)
    if p.state == NpcStateId::Action && p.action.and_then(|r| r.def) == Some(ActionDef::GoofyTurbo) {
        cancel_action(p);
        p.cmc.stop_movement();
        log(p, "turbo_bash");
        super::goofy_combo::start(p, host, super::goofy_combo::Step::Big);
        return true;
    }
    let guard_ctx = matches!(st, Some(GUARD | GUARD_HIT | AIR_GUARD | BASH)) || ((g(p).counter > 0.0 || g(p).x_release < K.guard_release_grace) && p_state_free(st, p.action.and_then(|r| r.def)));
    if !grounded {
        if st == Some(AIR_GUARD) || (g(p).counter > 0.0 && matches!(st, Some(JUMP))) {
            start_bash(p, host, BashKind::Air);
            return true;
        }
        // (GOOFY-GK) the air combo
        if st == Some(BASH) {
            return false;
        }
        if st == Some(HOP) {
            cancel_action(p);
        }
        return super::goofy_combo::press(p, host);
    }
    if !guard_ctx {
        // (GOOFY-GK) the ground combo (a Hop's cancel point ends the Hop)
        if st == Some(HOP) {
            p.cmc.direct_xy = Some(Vec2::ZERO);
        }
        return super::goofy_combo::press(p, host);
    }
    let (_, mag, _) = stick(p);
    let x_held = held(p, PadButton::FaceLeft) && st == Some(GUARD);
    if x_held && has(p, GA::ShieldCharge) && mag >= K.charge_stick {
        if p.status.guard.as_mut().is_some_and(|gd| gd.spend(K.guard_charge_cost)) {
            start_charge(p);
            return true;
        }
    }
    let next = if st == Some(BASH) {
        match (g(p).bash, g(p).chain) {
            (BashKind::Bash1, _) => BashKind::Bash2,
            (BashKind::Bash2, _) => BashKind::Heavy,
            _ => BashKind::Bash1,
        }
    } else {
        BashKind::Bash1
    };
    if next == BashKind::Heavy && !guard_ok(p) {
        start_bash(p, host, BashKind::Bash1);
        return true;
    }
    gp(p).from_guard = x_held;
    start_bash(p, host, next);
    true
}

// ---------------------------------------------------------------------------------------------- the guard

fn start_guard(p: &mut NpcPawn) {
    let was_guard = matches!(state_n(p), Some(GUARD | GUARD_HIT | AIR_GUARD));
    p.enter(NpcStateId::Pad(GUARD));
    p.cmc.stop_movement();
    p.cmc.walk_mode = false;
    p.anim.speed = 1.0;
    p.play_looped("SGuardIdle", K.guard_blend);
    let k = gp(p);
    if !was_guard {
        k.guard_age = 0.0;
    }
    k.chain = 0;
    log(p, "guard");
}

fn start_air_guard(p: &mut NpcPawn) {
    let rising = p.cmc.velocity.z > 0.0;
    p.enter(NpcStateId::Pad(AIR_GUARD));
    p.cmc.gravity_off = false;
    p.cmc.direct_z = None;
    if !p.on_ground() {
        p.cmc.set_falling();
    }
    p.play(if rising { "SGuardJumpRise" } else { "SGuardFall" }, K.guard_blend);
    if !matches!(g(p).log.last(), Some(&"guard")) {
        gp(p).guard_age = 0.0;
    }
    log(p, "air_guard");
}

fn guard_tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    let mode = g(p).mode_left > 0.0;
    let x = held(p, PadButton::FaceLeft) || mode;
    if !x || guard_broken(p) {
        p.cmc.speed_limit_override = None;
        leave_to_ground(p);
        return;
    }
    if !p.on_ground() {
        start_air_guard(p);
        return;
    }
    // B: the guard jump (shield up)
    if pressed(p, PadButton::JUMP) && !lb(p) {
        p.cmc.speed_limit_override = None;
        p.cmc.press_jump(SORA_PARAMS.jump_power_normal);
        p.enter(NpcStateId::Pad(AIR_GUARD));
        p.play("SGuardJumpStart", K.guard_blend);
        log(p, "guard_jump");
        return;
    }
    let (yaw, mag, active) = stick(p);
    // facing: the lock, else the stick
    let lock = p.pad.as_ref().map_or(TargetId::NONE, |d| d.lock);
    let want = host.targets.info(lock).map(|t| yaw_of(horizontal(t.position - p.position()))).or(active.then_some(yaw));
    if let Some(w) = want {
        let d = wrap180(w - p.yaw);
        let s = K.guard_turn_rate * dt;
        p.yaw = wrap180(p.yaw + d.clamp(-s, s));
    }
    if active {
        let a = yaw.to_radians();
        p.cmc.control_input = Vec3::new(a.cos(), a.sin(), 0.0);
        p.cmc.max_walk_speed = K.guard_walk_speed;
        p.cmc.max_acceleration = K.guard_walk_speed * SORA_PARAMS.accel_ground_factor;
        p.cmc.speed_limit_override = Some(K.guard_walk_speed * mag.min(1.0));
    } else {
        p.cmc.speed_limit_override = Some(0.0);
    }
    // the hold A in guard: the bash turns heavy (in BASH); here: the clip by speed
    let moving = p.speed2d() > 20.0 || active;
    let want_clip = if moving { "SGuardWalk" } else { "SGuardIdle" };
    if p.anim.clip.name != want_clip {
        p.play_looped(want_clip, K.guard_blend);
    }
}

fn air_guard_tick(p: &mut NpcPawn, cx: &StepCtx) {
    let _ = cx;
    let mode = g(p).mode_left > 0.0;
    let x = held(p, PadButton::FaceLeft) || mode;
    if p.on_ground() && p.state_time > 0.05 {
        p.cmc.stop_movement();
        if x && !guard_broken(p) {
            start_guard(p);
            p.play("SGuardLand", K.guard_blend);
        } else {
            p.play("Land", -1.0);
            p.enter(NpcStateId::Pad(JUMP));
            if let Some(d) = p.pad.as_mut() {
                d.jump.sub = 10;
                d.jump.air_attacks = 0;
            }
        }
        return;
    }
    if !x || guard_broken(p) {
        enter_jump(p, true);
        return;
    }
    let (yaw, _, active) = stick(p);
    if active {
        let a = yaw.to_radians();
        p.cmc.control_input = Vec3::new(a.cos(), a.sin(), 0.0);
        p.cmc.max_walk_speed = K.guard_walk_speed;
    }
    p.cmc.gravity_scale = p.params.gravity_scale * K.air_guard_gravity;
    let want = if p.cmc.velocity.z > 0.0 { if p.anim.clip.name == "SGuardJumpStart" && !p.anim.finished { "SGuardJumpStart" } else { "SGuardJumpRise" } } else { "SGuardFall" };
    if p.anim.clip.name != want && (p.anim.finished || want == "SGuardFall") {
        p.play(want, K.guard_blend);
    }
}

/// A hit on him (pawn.rs `receive_hit`, first): None = the normal hit path. Some = the kit answered (invincible,
/// guarded, a Perfect Guard).
pub fn on_hit(p: &mut NpcPawn, hit: &HitRecord) -> Option<HitResult> {
    if !is_goofy(p) {
        return None;
    }
    if invincible(p) {
        log(p, "dodged");
        return Some(HitResult { accepted: false, hp_after: p.status.hp as f32, ..HitResult::default() });
    }
    let guarding = matches!(state_n(p), Some(GUARD | GUARD_HIT | AIR_GUARD | CHARGE)) || (g(p).mode_left > 0.0 && !matches!(state_n(p), Some(BASH | GUARD_BREAK)));
    if !guarding || guard_broken(p) {
        return None;
    }
    let unblockable = hit.info.flags50 & kh_core::host::HitInfo::FLAG50_IGNORE_GUARD != 0 || matches!(hit.damage_kind, Kind::Catch | Kind::Bind);
    let from = if hit.attacker.valid { hit.attacker.position } else { p.position() - hit.direction * 100.0 };
    let to = horizontal(from - p.position());
    let frontal = to.length() < 1.0 || wrap180(yaw_of(to) - p.yaw).abs() <= K.guard_arc_half;
    if unblockable || !frontal {
        return None;
    }
    let mode = g(p).mode_left > 0.0;
    let window = if has(p, GA::IronWall) { K.perfect_window_iron } else { K.perfect_window };
    let perfect = g(p).guard_age <= window;
    let max_hp = p.status.max_hp().max(1) as f32;
    let at = p.capsule_centre() + forward(p.yaw) * p.params.capsule_radius;
    let hp = p.status.hp as f32;
    gp(p).since_block = 0.0;
    if perfect {
        gp(p).counter = K.perfect_counter_window;
        // (GOOFY-GK) a Perfect Guard gives GUARD back
        if let Some(gd) = p.status.guard.as_mut() {
            gd.value = (gd.value + K.guard_perfect_refund).min(gd.max);
            gd.hold();
        }
        if has(p, GA::Rally) {
            let mp = (p.status.mp + K.rally_mp as i32).min(p.status.max_mp());
            if p.status.mp_charge_left <= 0.0 {
                p.status.mp = mp;
            }
        }
        log(p, "perfect_guard");
        pending_fx(p, K.perfect_effect, at);
    } else if !mode {
        // (GOOFY-GK) the block is paid from GUARD (GG's meter); an emptied meter is the GUARD BREAK
        let drain = (K.guard_drain_share * hit.damage.max(0.0) / max_hp).max(K.guard_drain_min);
        let broke = p.status.guard.as_mut().is_some_and(|gd| gd.absorb(drain).broke);
        if broke {
            start_guard_break(p);
            return Some(HitResult { accepted: true, damage_dealt: 0.0, hp_after: hp, killed: false, guarded: true });
        }
        log(p, "block");
    } else {
        log(p, "block");
    }
    // (GOOFY-GK) BRACE: each block is stored for the next guard bash (max brace.max); full = the shield flares
    {
        let k = gp(p);
        k.brace = (k.brace + 1).min(K.brace_max as u8);
        k.brace_age = 0.0;
    }
    if g(p).brace == K.brace_max as u8 {
        pending_fx(p, K.brace_effect, at);
    }
    gp(p).sfx.push((K.guard_block_se, at));
    pending_fx(p, K.guard_block_effect, at);
    // the blocked hit's reaction: SGuardHit (air SGuardAirHit) with a short slide back
    let air = !p.on_ground();
    p.enter(NpcStateId::Pad(GUARD_HIT));
    p.cmc.speed_limit_override = None;
    p.play(if air { "SGuardAirHit" } else { "SGuardHit" }, 0.05);
    gp(p).dir = -to.normalize_or_zero();
    Some(HitResult { accepted: true, damage_dealt: 0.0, hp_after: hp, killed: false, guarded: true })
}

/// Effects a hit on him wants (the hit has no host: spawned at the next pre-move).
fn pending_fx(p: &mut NpcPawn, name: &'static str, at: Vec3) {
    gp(p).fx.push((name, at));
}

/// The hop / surf / finisher invincibility now.
pub fn invincible(p: &NpcPawn) -> bool {
    let Some(pad) = p.pad.as_ref() else { return false };
    let k = &pad.goofy;
    if k.invincible > 0.0 || k.hold_invincible {
        return true;
    }
    // (DON-KIT) Donald's Aero aura: the invincible window x (1 + evasion)
    matches!(p.state, NpcStateId::Pad(HOP)) && p.state_time >= K.hop_inv_from && p.state_time <= K.hop_inv_to * (1.0 + p.don_aura.evasion.max(0.0))
}

/// Stalwart (and the mode): no flinch from this hit (pawn.rs `receive_hit`: HP without a reaction).
pub fn no_reaction(p: &NpcPawn, hit: &HitRecord) -> bool {
    if !is_goofy(p) {
        return false;
    }
    if p.pad.as_ref().is_some_and(|d| d.goofy.mode_left > 0.0) {
        return true;
    }
    let busy = p.state == NpcStateId::Action || matches!(state_n(p), Some(GUARD | GUARD_HIT | BASH | FINISH | DIVE | CHARGE | AIR_GUARD | COMBO | AIR_COMBO)); // (GOOFY-GK) + his combo
    busy && has(p, GA::Stalwart) && matches!(hit.damage_kind, Kind::Small | Kind::Beat)
}

fn guard_hit_tick(p: &mut NpcPawn, cx: &StepCtx) {
    if p.state_time < K.guard_pushback_time && p.on_ground() {
        let d = g(p).dir * K.guard_pushback_speed;
        p.cmc.direct_xy = Some(Vec2::new(d.x, d.y));
    }
    let _ = cx;
    if p.anim.finished {
        let x = held(p, PadButton::FaceLeft) || g(p).mode_left > 0.0;
        if p.on_ground() {
            if x {
                p.enter(NpcStateId::Pad(GUARD));
                p.play_looped("SGuardIdle", K.guard_blend);
            } else {
                leave_to_ground(p);
            }
        } else if x {
            start_air_guard(p);
        } else {
            enter_jump(p, true);
        }
    }
}

/// (GOOFY-GK) GUARD BREAK: the meter emptied (a block, the hold drain, a spend): the open stagger SGuardDamage /
/// SGuardAirDamage (its own se00302_026 and voice), no guard until GG's rearm.
fn start_guard_break(p: &mut NpcPawn) {
    if let Some(gd) = p.status.guard.as_mut() {
        if !gd.broken {
            gd.break_now();
        }
    }
    log(p, "guard_break");
    let k = gp(p);
    k.brace = 0;
    k.counter = 0.0;
    p.enter(NpcStateId::Pad(GUARD_BREAK));
    p.cmc.speed_limit_override = None;
    p.cmc.stop_movement();
    p.play(if p.on_ground() { "SGuardDamage" } else { "SGuardAirDamage" }, 0.1);
}

fn guard_break_tick(p: &mut NpcPawn) {
    if p.anim.finished {
        leave_to_ground(p);
    }
}

// ---------------------------------------------------------------------------------------------- bashes, charge

fn start_bash(p: &mut NpcPawn, host: &mut PartyHost, kind: BashKind) {
    let payback = has(p, GA::PaybackBash) && g(p).since_block <= K.payback_after_block;
    let air = kind == BashKind::Air;
    let chain = match kind {
        BashKind::Bash1 => 1,
        BashKind::Bash2 => 2,
        BashKind::Heavy => 3,
        _ => 0,
    };
    p.cmc.speed_limit_override = None;
    p.enter(NpcStateId::Pad(BASH));
    if p.on_ground() {
        p.cmc.stop_movement();
    } else {
        p.cmc.velocity.z = 0.0;
        p.cmc.gravity_off = true;
        p.cmc.set_flying();
        p.cmc.stop_movement();
    }
    face(p, host, K.bash_turn);
    let name = match kind {
        BashKind::Bash1 => "SGuardShieldBash",
        BashKind::Bash2 | BashKind::Plain => "SGuardShieldBash2",
        BashKind::Heavy => "SGuardHeavyBash",
        BashKind::Air => "SGuardAirShieldBash",
    };
    p.play(name, 0.05);
    // (GOOFY-GK) THE LUNGE (the user: "Goofy also doesn't move at all when he does a shield bash"): toward the target by
    // the gap (capped at bash.lunge_max), else bash.lunge_free ahead, between bash.lunge_from and the hit
    let hit_t = bash_hit_time(kind);
    let dist = match target_info(p, host) {
        Some(t) => (horizontal(t.position - p.position()).length() - t.radius - p.params.capsule_radius - K.combo_reach).clamp(0.0, K.bash_lunge_max),
        None => K.bash_lunge_free,
    };
    let window = (hit_t - K.bash_lunge_from).max(0.05);
    let f = forward(p.yaw) * (dist / window);
    // (GOOFY-GK) the counter (a bash inside the Perfect Guard window) and BRACE (the blocks stored)
    let counter = g(p).counter > 0.0 && kind != BashKind::Plain;
    let brace = if matches!(kind, BashKind::Bash1 | BashKind::Bash2 | BashKind::Heavy | BashKind::Air) { g(p).brace } else { 0 };
    let k = gp(p);
    k.bash = kind;
    k.chain = chain;
    k.bash_hit = None;
    k.payback = payback;
    k.a_hold = 0.0;
    k.heavy_paid = false;
    k.bash_lunge = Vec2::new(f.x, f.y);
    // Payback Bash (ability): the counter's x1.5 BLOW after ANY block (within payback.after_block)
    let pay = (counter || payback) && kind != BashKind::Plain;
    k.bash_mult = (if counter { K.counter_mult } else if pay { K.payback_mult } else { 1.0 }) * (1.0 + K.brace_per * brace as f32);
    k.bash_blow = pay || brace >= K.brace_max as u8;
    k.brace = 0;
    k.hitstop = 0.0;
    if counter {
        k.counter = 0.0;
    }
    k.hold_invincible = kind == BashKind::Plain || counter;
    let _ = air;
    log(p, match kind {
        BashKind::Bash1 => "bash1",
        BashKind::Bash2 => "bash2",
        BashKind::Heavy => "heavy",
        BashKind::Air => "air_bash",
        BashKind::Plain => "plain_finisher",
    });
}

fn bash_row(p: &NpcPawn, kind: BashKind) -> Option<SoloRow> {
    let (name, mult) = match kind {
        BashKind::Bash1 | BashKind::Air => (K.bash_row, K.bash_mult),
        BashKind::Bash2 => (K.bash2_row, K.bash2_mult),
        BashKind::Plain => (K.bash2_row, K.fin_plain_mult),
        BashKind::Heavy => (K.heavy_row, K.heavy_mult),
    };
    let mut r = shots::row(name, mult)?;
    // (GOOFY-GK) the counter / Payback / Brace scale (set at the bash's start)
    r = r.scaled(g(p).bash_mult);
    if g(p).bash_blow && kind != BashKind::Heavy {
        r = r.with_kind(Kind::Blow);
    }
    Some(r)
}

fn bash_hit_time(kind: BashKind) -> f32 {
    match kind {
        BashKind::Bash1 => K.bash_hit_time,
        BashKind::Bash2 | BashKind::Plain => K.bash2_hit_time,
        BashKind::Heavy => K.heavy_hit_time,
        BashKind::Air => K.air_bash_hit_time,
    }
}

fn bash_tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    let kind = g(p).bash;
    let mode = g(p).mode_left > 0.0;
    // A held: the guard bash becomes the Heavy Bash; the chain's Heavy needs the hold (else it is a Bash)
    let a_down = held(p, PadButton::FaceBottom);
    if a_down {
        gp(p).a_hold += dt;
    }
    // (GOOFY-GK) HIT-STOP: the clip and the lunge stand still
    if g(p).hitstop > 0.0 {
        gp(p).hitstop -= dt;
        p.anim.speed = 0.0;
        p.cmc.direct_xy = Some(Vec2::ZERO);
        if g(p).hitstop <= 0.0 {
            gp(p).hitstop = 0.0;
            p.anim.speed = 1.0;
        }
        return;
    }
    // (GOOFY-GK) the lunge
    let lt = bash_hit_time(kind);
    if p.anim.time >= K.bash_lunge_from && p.anim.time < lt {
        p.cmc.direct_xy = Some(g(p).bash_lunge);
    } else if p.anim.time >= lt && p.anim.time < lt + 0.05 {
        p.cmc.direct_xy = Some(Vec2::ZERO);
    }
    match kind {
        BashKind::Bash1 if g(p).from_guard && a_down && g(p).a_hold >= K.heavy_hold && guard_ok(p) => {
            // (GOOFY-GK) the guard Heavy Bash spends GUARD (short of it: the Shield Bash goes on)
            if p.status.guard.as_mut().is_some_and(|gd| gd.spend(K.guard_heavy_cost)) {
                gp(p).from_guard = false;
                let (mult, blow) = (g(p).bash_mult, g(p).bash_blow);
                start_bash(p, host, BashKind::Heavy);
                let k = gp(p);
                (k.bash_mult, k.bash_blow) = (k.bash_mult.max(mult), k.bash_blow || blow);
                k.heavy_paid = true;
                p.anim.time = K.heavy_hold.min(K.heavy_hit_time - 0.05);
                return;
            }
        }
        BashKind::Heavy if g(p).bash_hit.is_none() && !a_down && g(p).a_hold < K.heavy_hold && p.state_time > 0.02 && !mode_finishing(p) && !g(p).heavy_paid => {
            // let go too early: the chain's third press is a Shield Bash
            start_bash(p, host, BashKind::Bash1);
            return;
        }
        _ => {}
    }
    // the hit
    let t = bash_hit_time(kind);
    if g(p).bash_hit.is_none() && p.anim.time >= t {
        gp(p).bash_hit = Some(t);
        if let Some(row) = bash_row(p, kind) {
            if kind == BashKind::Heavy {
                let mut a = Area::new(row, shots::local(p, Vec3::new(K.heavy_forward, 0.0, K.bash_up)), K.heavy_life, K.heavy_radius, "heavy");
                a.size = (K.heavy_radius, K.heavy_grow, K.heavy_max);
                shots::effect(p, host, K.heavy_effect, a.centre, 1.0);
                shots::add(p, a);
                // (GOOFY-GK) the chain's Heavy (third press, held) spends GUARD at its hit; the guard-hold Heavy paid already
                if !mode && !g(p).heavy_paid {
                    if let Some(gd) = p.status.guard.as_mut() {
                        let _ = gd.spend(K.guard_heavy_cost.min(gd.value));
                    }
                }
            } else {
                let tag = if kind == BashKind::Plain { "plain" } else { "bash" };
                let a = shots::bash_area(p, row, tag);
                shots::effect(p, host, K.bash_effect, a.centre, 1.0);
                shots::add(p, a);
            }
        }
        if kind == BashKind::Plain {
            gp(p).hold_invincible = false;
            gp(p).released = true;
        }
    }
    if kind == BashKind::Heavy && mode_finishing(p) && p.anim.finished {
        end_mode(p);
    }
    // the air bash hovers until its end
    if kind == BashKind::Air && p.anim.finished {
        p.cmc.gravity_off = false;
        p.cmc.direct_z = None;
        if !p.on_ground() {
            p.cmc.set_falling();
        }
        if held(p, PadButton::FaceLeft) && !guard_broken(p) {
            start_air_guard(p);
        } else {
            enter_jump(p, true);
        }
        return;
    }
    if p.anim.finished {
        gp(p).chain = 0;
        if kind == BashKind::Plain {
            if let Some(d) = p.pad.as_mut() {
                d.combo = 0;
            }
        }
        leave_to_ground(p);
    }
}

fn mode_finishing(p: &NpcPawn) -> bool {
    p.pad.as_ref().is_some_and(|d| d.goofy.mode_left > 0.0 && d.goofy.mode_finish)
}

fn start_charge(p: &mut NpcPawn) {
    let (yaw, _, _) = stick(p);
    p.enter(NpcStateId::Pad(CHARGE));
    p.cmc.speed_limit_override = None;
    p.play_looped("SGuardWalk", 0.05);
    p.anim.speed = 2.0;
    p.yaw = yaw;
    gp(p).dir = forward(yaw);
    if let Some(row) = shots::row(K.bash_row, K.charge_mult) {
        let mut a = Area::new(row, shots::local(p, Vec3::new(K.charge_radius * 0.5, 0.0, K.bash_up)), K.charge_time, K.charge_radius, "charge");
        a.follow = Some(Vec3::new(K.charge_radius * 0.5, 0.0, K.bash_up));
        a.push = Push::Forward;
        shots::add(p, a);
    }
    log(p, "charge");
}

fn charge_tick(p: &mut NpcPawn) {
    let d = g(p).dir * K.charge_speed;
    p.cmc.direct_xy = Some(Vec2::new(d.x, d.y));
    if p.state_time >= K.charge_time {
        p.anim.speed = 1.0;
        p.cmc.direct_xy = Some(Vec2::ZERO);
        if held(p, PadButton::FaceLeft) && !guard_broken(p) {
            p.enter(NpcStateId::Pad(GUARD));
            p.play_looped("SGuardIdle", K.guard_blend);
        } else {
            leave_to_ground(p);
        }
    }
}

// ---------------------------------------------------------------------------------------------- hop, surf, copter

fn start_hop(p: &mut NpcPawn, yaw: f32) {
    let dir = forward(yaw);
    let f = forward(p.yaw);
    let right = Vec3::new(-f.y, f.x, 0.0);
    let side = dir.dot(right);
    p.enter(NpcStateId::Pad(HOP));
    p.cmc.speed_limit_override = None;
    p.cmc.stop_movement();
    p.play(if side >= 0.0 { "HopR" } else { "HopL" }, 0.05);
    gp(p).dir = dir;
    // (GOOFY-GK) the cooldown (the user: "a reasonable cool down so it can't be spammed")
    gp(p).hop_cd = K.hop_cooldown / (1.0 + p.don_aura.evasion.max(0.0)); // (DON-KIT) Donald's Aero aura: a shorter cooldown
    gp(p).hop_queued = None;
    log(p, "hop");
}

fn hop_tick(p: &mut NpcPawn) {
    if p.state_time < K.hop_time {
        let v = g(p).dir * (K.hop_distance / K.hop_time.max(0.01));
        p.cmc.direct_xy = Some(Vec2::new(v.x, v.y));
    } else if p.state_time - K.hop_time < 0.02 {
        p.cmc.direct_xy = Some(Vec2::ZERO);
    }
    // from the cancel point on: the stick runs, a buffered A starts the combo (stream P's decide sees him free)
    let (_, _, active) = stick(p);
    let wants = p.pad.as_ref().is_some_and(|d| d.buffered > 0.0) || pressed(p, PadButton::JUMP);
    if p.anim.finished || (p.anim.time >= K.hop_cancel && (active || wants)) {
        if !p.on_ground() {
            enter_jump(p, true);
        } else if pressed(p, PadButton::JUMP) {
            enter_jump(p, false);
        } else {
            leave_to_ground(p);
        }
    }
}

// (GOOFY-GK) Shield Surf (the air dodge) is removed: the user, 2026-10-16 23:40 "Remove Goofy's air dodge".

/// Tornado Copter is unlocked: Sora's Glide brought it (kit ability KIT_TORNADO_COPTER).
pub fn copter_unlocked(p: &NpcPawn) -> bool {
    has(p, GA::TornadoCopter)
}

fn start_copter(p: &mut NpcPawn, host: &mut PartyHost) {
    p.enter(NpcStateId::Pad(COPTER));
    p.cmc.gravity_off = true;
    p.cmc.set_flying();
    p.anim.play_loop(clip_copy("M_1322", true, true, true), 0.15);
    // trn_loc00 at half scale on him (the clip's own AttachEffect is taken out: contact off, the effect scaled)
    let m = kh_core::host::PartyMember(p.id.0);
    let h = p.seam.spawn(host.effects, &kh_core::host::EffectSpawn { id: kh_core::host::EffectId(kh_core::notify::notify_asset_id("vn_ex002_trn_loc00")), name: "vn_ex002_trn_loc00", at: kh_core::host::Attach::Party { member: m, bone: None, offset: Vec3::ZERO }, rotation: Vec3::ZERO, scale: K.copter_effect_scale, persistent: true });
    gp(p).copter_fx = h.0;
    log(p, "copter");
}

fn end_copter(p: &mut NpcPawn) {
    if in_state(p, COPTER) {
        p.cmc.gravity_off = false;
        p.cmc.direct_z = None;
    }
}

fn copter_tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    if !held(p, PadButton::JUMP) || p.on_ground() {
        stop_copter_fx(p, host);
        p.cmc.gravity_off = false;
        p.cmc.direct_z = None;
        if p.on_ground() {
            p.play("Land", -1.0);
            p.enter(NpcStateId::Pad(JUMP));
            if let Some(d) = p.pad.as_mut() {
                d.jump.sub = 10;
                d.jump.air_attacks = 0;
            }
        } else {
            p.cmc.set_falling();
            enter_jump(p, true);
        }
        return;
    }
    let (yaw, _, active) = stick(p);
    if active {
        p.yaw = wrap180(p.yaw + wrap180(yaw - p.yaw) * (K.copter_turn * dt * 60.0).clamp(0.0, 1.0));
    }
    let f = forward(p.yaw) * K.copter_speed;
    p.cmc.direct_xy = Some(Vec2::new(f.x, f.y));
    p.cmc.direct_z = Some(-K.copter_sink);
}

fn stop_fin_fx(p: &mut NpcPawn, host: &mut PartyHost) {
    for h in std::mem::take(&mut gp(p).fin_fx) {
        host.effects.stop(kh_core::host::EffectHandle(h), false);
    }
}

fn stop_copter_fx(p: &mut NpcPawn, host: &mut PartyHost) {
    let h = gp(p).copter_fx;
    if h != 0 {
        host.effects.stop(kh_core::host::EffectHandle(h), false);
        gp(p).copter_fx = 0;
    }
}

// ---------------------------------------------------------------------------------------------- aerial recovery (X3G-b)

/// Blown back in the air: the damage clip is a blow's flight or the fall (pawn.rs `damage_clip`: M_0211 / M_0214 /
/// M_0217 / M_0220 for BlownFlying .. LaunchedFalling, JumpNormalDownLoop for Falling).
fn blown(p: &NpcPawn) -> bool {
    // (GOOFY-HR) the shared rule (crate::hurt::thrown: kh-react's flight families): the blow's start clip is 0.07 s and
    // the flight now plays its loop (M_0212 ..), the launch M_0227 / M_0228 / M_0237; the clip-name list held only the start
    crate::hurt::thrown(p)
}

/// Sora's Aerial Recovery on Goofy's own clip: the reaction is cut, he stops in the air (gravity off for the clip,
/// Sora's AIR_SLIDE recovery: StopMovement, gravity off, GHOST_INVINCIBLE), turns to the attacker side he was hit
/// from, then falls with his air moves back; A in the next 2 s is the air counter.
fn start_recover(p: &mut NpcPawn, host: &mut PartyHost) {
    let _ = host;
    p.release_reaction();
    p.enter(NpcStateId::Pad(RECOVER));
    p.cmc.stop_movement();
    p.cmc.velocity = Vec3::ZERO;
    p.cmc.gravity_off = true;
    p.cmc.set_flying();
    p.anim.play(clip_copy(K.recover_clip, true, false, true), 0.05);
    let k = gp(p);
    k.invincible = K.recover_invincible;
    k.counter = K.recover_counter;
    k.surf_used = 0;
    if let Some(d) = p.pad.as_mut() {
        d.jump.air_attacks = 0;
    }
    log(p, "air_recovery");
}

fn recover_tick(p: &mut NpcPawn) {
    p.cmc.direct_xy = Some(Vec2::ZERO);
    p.cmc.direct_z = Some(0.0);
    if p.state_time >= K.recover_length {
        p.cmc.gravity_off = false;
        p.cmc.direct_z = None;
        p.cmc.set_falling();
        enter_jump(p, true);
    }
}

// ---------------------------------------------------------------------------------------------- finishers

/// (GOOFY-GK) The Fusion finishers of his string (pad/goofy_combo.rs `finisher` chooses them).
pub(super) fn start_fusion(p: &mut NpcPawn, host: &mut PartyHost, fin: Finisher) {
    cancel_action(p);
    p.enter(NpcStateId::Pad(FINISH));
    p.cmc.stop_movement();
    p.cmc.speed_limit_override = None;
    face(p, host, 180.0);
    let k = gp(p);
    k.fin = fin;
    k.released = false;
    k.hold_invincible = true;
    k.hit_at = None;
    k.root_scale = 1.0;
    k.root_prev = Vec2::ZERO;
    match fin {
        Finisher::Spin => {
            // DECISION 10 by capture: the solo GooFSpin reads as Goofy swung by an absent Sora: the fallback shield bash
            // (sheet fin.spin_clip) with Fusion Spin's effects and hit
            p.play(K.fin_spin_clip, 0.05);
            log(p, "fusion_spin");
        }
        Finisher::Rocket => {
            // the rush's root motion, scaled to end at the target (applied here: the copy has no root track)
            let c = clip("GooFRocket");
            let end = c.root_at(c.length);
            let len = Vec2::new(end.x, end.y).length();
            if K.fin_rocket_reach > 0.0 && len > 1.0 {
                if let Some(t) = target_info(p, host) {
                    let d = horizontal(t.position - p.position()).length() - t.radius - p.params.capsule_radius;
                    gp(p).root_scale = (d / len).clamp(K.fin_rocket_scale_min, K.fin_rocket_scale_max);
                }
            }
            // ROUTED (sheet fin.rocket_drop_effects): the clip's own effects draw as tiles in the port's vfx; Goofy's Turbo
            // trail is carried instead (`finish_tick`)
            p.anim.play(clip_copy("GooFRocket", false, K.fin_rocket_drop_effects > 0.0, true), 0.05);
            log(p, "fusion_rocket");
        }
    }
}

fn finish_tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    let fin = g(p).fin;
    let t = p.anim.time;
    match fin {
        Finisher::Spin => {
            if !g(p).released && t >= K.fin_spin_time {
                let k = gp(p);
                k.released = true;
                k.hold_invincible = false;
                let m = kh_core::host::PartyMember(p.id.0);
                for name in [K.fin_spin_effect_a, K.fin_spin_effect_b] {
                    let h = p.seam.spawn(host.effects, &kh_core::host::EffectSpawn { id: kh_core::host::EffectId(kh_core::notify::notify_asset_id(name)), name, at: kh_core::host::Attach::Party { member: m, bone: None, offset: Vec3::ZERO }, rotation: Vec3::ZERO, scale: 1.0, persistent: true });
                    gp(p).fin_fx.push(h.0);
                }
                let at = p.position() + Vec3::Z * K.fin_spin_up;
                shots::sound(p, host, K.fin_spin_se, at);
                if let Some(row) = shots::row("FUSION_SPIN", K.fin_mult) {
                    let off = Vec3::new(0.0, 0.0, K.fin_spin_up);
                    let mut a = Area::new(row, shots::local(p, off), K.fin_spin_life, K.fin_spin_size, "fusion_spin");
                    a.size = (K.fin_spin_size, K.fin_spin_grow, K.fin_spin_cap);
                    a.follow = Some(off);
                    a.hit_effect = K.fin_hit_effect;
                    a.hit_se = K.fin_hit_se;
                    shots::add(p, a);
                }
            }
        }
        Finisher::Rocket => {
            // the scaled root motion of the rush (the clip's own track, frame 0 actor space)
            let c = clip("GooFRocket");
            let r = c.root_at(t.min(c.length));
            let now = Vec2::new(r.x, r.y) * g(p).root_scale;
            let d = now - g(p).root_prev;
            gp(p).root_prev = now;
            if dt > 0.0 {
                let f = forward(p.yaw);
                let right = Vec3::new(-f.y, f.x, 0.0);
                let w = f * d.x + right * d.y;
                p.cmc.direct_xy = Some(Vec2::new(w.x, w.y) / dt);
            }
            if t >= K.fin_rocket_from && !g(p).shots.iter().any(|a| a.tag == "fusion_rocket") && g(p).extra == 0 {
                gp(p).extra = 1;
                if K.fin_rocket_drop_effects > 0.0 {
                    let m = kh_core::host::PartyMember(p.id.0);
                    let name = K.fin_rocket_trail;
                    let h = p.seam.spawn(host.effects, &kh_core::host::EffectSpawn { id: kh_core::host::EffectId(kh_core::notify::notify_asset_id(name)), name, at: kh_core::host::Attach::Party { member: m, bone: None, offset: Vec3::ZERO }, rotation: Vec3::ZERO, scale: 1.0, persistent: true });
                    gp(p).fin_fx.push(h.0);
                }
                if let Some(row) = shots::row("FUSION_ROCKET", K.fin_mult) {
                    let off = Vec3::new(K.fin_rocket_forward, 0.0, K.fin_rocket_up);
                    let mut a = Area::new(row, shots::local(p, off), (K.fin_rocket_to - K.fin_rocket_from).max(0.05), K.fin_rocket_radius, "fusion_rocket");
                    a.follow = Some(off);
                    a.push = Push::Forward;
                    a.hit_effect = K.fin_hit_effect;
                    a.hit_se = K.fin_hit_se;
                    shots::add(p, a);
                }
            }
            if !g(p).released && t >= K.fin_rocket_release {
                let k = gp(p);
                k.released = true;
                k.hold_invincible = false;
            }
        }
    }
    if (fin == Finisher::Spin && t >= K.fin_spin_effect_end) || (fin == Finisher::Rocket && t >= K.fin_rocket_to) {
        stop_fin_fx(p, host);
    }
    if p.anim.finished {
        stop_fin_fx(p, host);
        let k = gp(p);
        k.extra = 0;
        k.hold_invincible = false;
        if let Some(d) = p.pad.as_mut() {
            d.combo = 0;
        }
        p.cmc.direct_xy = Some(Vec2::ZERO);
        leave_to_ground(p);
    }
}

// ---------------------------------------------------------------------------------------------- Bombardier Dive

fn start_dive(p: &mut NpcPawn, host: &mut PartyHost) {
    if p.state == NpcStateId::Pad(AIR_ATTACK) {
        p.cmc.gravity_off = false;
    }
    p.enter(NpcStateId::Pad(DIVE));
    face(p, host, 180.0);
    p.cmc.stop_movement();
    p.cmc.gravity_off = true;
    p.cmc.set_flying();
    p.play("GShootRise", 0.05);
    // (GOOFY-GK) Bombardier is his Y move now (ground or air) with a cooldown; from the ground it rises bomb.ground_rise
    let rise = if p.on_ground() { K.bomb_ground_rise } else { K.dive_rise_height };
    let k = gp(p);
    k.dive_rise = rise;
    k.bomb_cd = K.bomb_cooldown;
    k.dive_used = true;
    k.dive_sub = 0;
    k.hold_invincible = true;
    k.extra = 0;
    log(p, "bombardier");
}

fn dive_tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    match g(p).dive_sub {
        0 => {
            p.cmc.direct_xy = Some(Vec2::ZERO);
            p.cmc.direct_z = Some(g(p).dive_rise / K.dive_rise_time.max(0.01)); // (GOOFY-GK)
            if p.state_time >= K.dive_rise_time {
                gp(p).dive_sub = 1;
                p.play("GShootEnd", 0.05);
                // the dive: -45 deg ahead, or straight at a target below and ahead
                let a = K.dive_angle.to_radians();
                let mut dir = forward(p.yaw) * a.cos() + Vec3::Z * a.sin();
                if let Some(t) = target_info(p, host) {
                    let to = t.position - p.position();
                    let pitch = to.z.atan2(horizontal(to).length());
                    if pitch < -0.2 && pitch > -1.4 {
                        dir = to.normalize_or_zero();
                        p.yaw = yaw_of(horizontal(to));
                    }
                }
                gp(p).dir = dir;
                if let Some(row) = shots::row(K.dive_rush_row, K.dive_rush_mult) {
                    let mut ar = Area::new(row, shots::local(p, Vec3::new(0.0, 0.0, 90.0)), K.dive_max_time, K.dive_rush_radius, "dive_rush");
                    ar.follow = Some(Vec3::new(0.0, 0.0, 90.0));
                    shots::add(p, ar);
                }
            }
        }
        1 => {
            if p.anim.finished {
                p.anim.speed = 0.0;
            }
            let v = g(p).dir * K.dive_speed;
            p.cmc.direct_xy = Some(Vec2::new(v.x, v.y));
            p.cmc.direct_z = Some(v.z);
            // the mesh pitched along the velocity (D4's FRGoofyShoot rule: velocity pitch + 90)
            let vp = v.z.atan2(horizontal(v).length()).to_degrees();
            gp(p).pitch = Some(vp + 90.0);
            if p.on_ground() || p.state_time > K.dive_rise_time + K.dive_max_time || p.cmc.last_block.is_some() && p.state_time > K.dive_rise_time + 0.1 {
                explode(p, host);
            }
        }
        _ => {
            if p.anim.finished {
                p.cmc.direct_xy = Some(Vec2::ZERO);
                leave_to_ground(p);
            }
        }
    }
    let _ = dt;
}

fn explode(p: &mut NpcPawn, host: &mut PartyHost) {
    gp(p).shots.retain(|a| a.tag != "dive_rush");
    if let Some(row) = shots::row(K.dive_exp_row, K.dive_exp_mult) {
        let mut a = Area::new(row, p.position() + Vec3::Z * 50.0, K.dive_exp_life, K.dive_exp_radius, "dive_exp");
        a.size = (K.dive_exp_radius, K.dive_exp_grow, K.dive_exp_max);
        shots::effect(p, host, K.dive_exp_effect, p.position(), 1.0);
        shots::add(p, a);
    }
    p.anim.speed = 1.0;
    p.cmc.gravity_off = false;
    p.cmc.direct_z = None;
    p.cmc.direct_xy = Some(Vec2::ZERO);
    let k = gp(p);
    k.hold_invincible = false;
    k.pitch = None;
    k.dive_sub = 2;
    if p.on_ground() {
        p.play("Land", -1.0);
    } else {
        p.cmc.set_falling();
        enter_jump(p, true);
    }
    log(p, "bombardier_exp");
}

// ---------------------------------------------------------------------------------------------- the mode

// (GOOFY-GK) the Shield Guardian mode is no longer started from Goofy's Y (the user, 2026-10-17 00:40); its end paths
// stay harmless (mode_left is never set).

fn end_mode(p: &mut NpcPawn) {
    if g(p).mode_left > 0.0 {
        let k = gp(p);
        k.mode_left = 0.0;
        k.mode_finish = false;
        log(p, "mode_end");
    }
}

/// The mode's finish (Y, or the time): the free Heavy Bash.
fn mode_tick_finish(p: &mut NpcPawn, host: &mut PartyHost) -> bool {
    if !mode_finishing(p) {
        return false;
    }
    if in_state(p, BASH) && g(p).bash == BashKind::Heavy {
        return false;
    }
    if !p.on_ground() || matches!(state_n(p), Some(GUARD_HIT)) {
        return false;
    }
    gp(p).a_hold = K.heavy_hold;
    start_bash(p, host, BashKind::Heavy);
    gp(p).a_hold = K.heavy_hold;
    true
}

/// The situation slot's Shield Guardian row (pad/mod.rs `pad_menu`): the offer, or "Finish" in the mode.
pub fn situation(p: &NpcPawn, snap: &mut kh_core::command::CommandSnapshot) {
    if !is_goofy(p) {
        return;
    }
    // (GOOFY-GK) THE Y ROW IS BOMBARDIER (the user, 2026-10-17 00:40: "Remove Shield Guardian from Goofy's Y move"; 23:40:
    // "His bombadier should be a Y move ... his Y prompt doesn't go down at all"). Ready: "Bombardier" (no timer). Used: that
    // row leaves (KH3's used animation) and a COOLING row with KH3's situation timer comes in, greyed, its digits and gauge
    // counting the cooldown down to 0; then the ready row comes back (the add flash). While AI Sora's team attack holds Y,
    // the slot is his (stream X2's rule: `sora_busy`).
    if p.pad.as_ref().is_some_and(|d| d.menu.leader.sora_busy) {
        return;
    }
    snap.situations.insert(0, bomb_row(g(p)));
    snap.situation_selected = Some(0);
}

/// (GOOFY-GK) The Y row of Bombardier (ready / cooling).
pub fn bomb_row(k: &GoofyPad) -> kh_core::command::SituationRow {
    if k.bomb_cd > 0.0 {
        kh_core::command::SituationRow { serial: 0x5A00_0011, kind: K.bomb_kind as u8, name: K.bomb_name, icon_type: K.bomb_icon as i32, total: K.bomb_cooldown, remaining: k.bomb_cd, enabled: false, refreshed: 0 }
    } else {
        kh_core::command::SituationRow { serial: 0x5A00_0010, kind: K.bomb_kind as u8, name: K.bomb_name, icon_type: K.bomb_icon as i32, total: 0.0, remaining: 0.0, enabled: true, refreshed: 0 }
    }
}

// ---------------------------------------------------------------------------------------------- Discus Barrage

fn aim(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) -> bool {
    let dt = cx.dt;
    let rb = held(p, PadButton::RightShoulder);
    let lock_now = p.pad.as_ref().map_or(TargetId::NONE, |d| d.lock);
    if pressed(p, PadButton::RightShoulder) {
        let k = gp(p);
        k.lock_before_press = k.lock_prev;
        k.rb_held = 0.0;
    }
    if rb {
        gp(p).rb_held += dt;
    } else if !p.pad.as_ref().is_some_and(|d| d.frame.pad.released.has(PadButton::RightShoulder)) {
        gp(p).rb_held = 0.0;
    }
    gp(p).lock_prev = lock_now;
    let st = state_n(p);
    if st != Some(AIM) && !rb {
        gp(p).aiming = false;
    }
    if st == Some(AIM) {
        return aim_tick(p, dt, host);
    }
    let can = (p.state == NpcStateId::Idle || matches!(st, Some(RUN | GUARD))) && p.on_ground() || st == Some(JUMP) && !p.on_ground();
    if rb && g(p).rb_held >= K.barrage_hold && can && g(p).focus >= 1.0 && g(p).mode_left <= 0.0 {
        // the RB press toggled the lock (stream P's tap rule): the aim keeps the lock as it was
        let before = g(p).lock_before_press;
        if let Some(d) = p.pad.as_mut() {
            if d.lock != before {
                d.lock = before;
                d.lock_changed = true;
            }
        }
        p.enter(NpcStateId::Pad(AIM));
        p.cmc.speed_limit_override = None;
        p.cmc.stop_movement();
        if !p.on_ground() {
            p.cmc.gravity_off = true;
            p.cmc.set_flying();
        }
        p.play_looped(if p.on_ground() { "IdleB" } else { "JumpNormalDownLoop" }, 0.1);
        let k = gp(p);
        k.locks.clear();
        k.lock_timer = 0.0;
        k.aiming = true;
        log(p, "aim");
        return true;
    }
    false
}

fn aim_tick(p: &mut NpcPawn, dt: f32, host: &mut PartyHost) -> bool {
    let cam = p.pad.as_ref().map_or(p.yaw, |d| d.frame.camera_yaw);
    p.yaw = wrap180(p.yaw + wrap180(cam - p.yaw) * (12.0 * dt).clamp(0.0, 1.0));
    p.cmc.direct_xy = Some(Vec2::ZERO);
    if !p.on_ground() {
        p.cmc.direct_z = Some(0.0);
    }
    let k = gp(p);
    k.focus = (k.focus - dt / K.barrage_focus_rate.max(1e-4)).max(0.0);
    k.lock_timer += dt;
    while gp(p).lock_timer >= K.barrage_lock_interval && gp(p).locks.len() < K.barrage_max_locks as usize {
        gp(p).lock_timer -= K.barrage_lock_interval;
        let me = p.position();
        let list = shots::enemies(host);
        let cand = list
            .iter()
            .filter(|t| t.position.distance(me) <= K.barrage_lock_range)
            .filter(|t| {
                let d = horizontal(t.position - me);
                d.length() < 1.0 || wrap180(yaw_of(d) - cam).abs() <= K.barrage_cone
            })
            .min_by(|a, b| {
                let na = g(p).locks.iter().filter(|l| **l == a.id).count();
                let nb = g(p).locks.iter().filter(|l| **l == b.id).count();
                na.cmp(&nb).then(a.position.distance(me).total_cmp(&b.position.distance(me)))
            })
            .map(|t| t.id);
        match cand {
            Some(id) => gp(p).locks.push(id),
            None => break,
        }
    }
    let rb = held(p, PadButton::RightShoulder);
    if !rb || g(p).focus <= 0.0 {
        if g(p).locks.is_empty() {
            p.cmc.gravity_off = false;
            p.cmc.direct_z = None;
            leave_to_ground(p);
            log(p, "aim_cancel");
        } else {
            fire_barrage(p, host);
        }
    }
    true
}

fn fire_barrage(p: &mut NpcPawn, host: &mut PartyHost) -> bool {
    let locks = std::mem::take(&mut gp(p).locks);
    let Some(first) = locks.first().copied() else { return false };
    p.cmc.gravity_off = false;
    p.cmc.direct_z = None;
    if !p.on_ground() {
        p.cmc.set_falling();
    }
    p.to_idle();
    let k = gp(p);
    k.queue = locks[1..].to_vec();
    k.bounce_mult = K.barrage_mult;
    k.pays_nothing = true;
    k.barrage_next = true;
    k.bounced.clear();
    let _ = host;
    let ok = issue(p, MemberCommand::Action { def: ActionDef::GoofyDiscus, target: TargetRef::Enemy(first) });
    log(p, "barrage");
    if !ok {
        gp(p).pays_nothing = false;
        gp(p).barrage_next = false;
        gp(p).queue.clear();
    }
    ok
}

/// pad/mod.rs `targeting`: RB toggles the lock on a SHORT TAP's release (held, it opens the aim: Sora's rule
/// aim2.input.tap_max_time, as X3D's Donald).
pub fn rb_tap(pad: &super::PadState) -> bool {
    pad.frame.pad.released.has(PadButton::RightShoulder) && !pad.goofy.aiming && pad.goofy.rb_held <= K.barrage_hold
}

/// actions.rs `begin`: the barrage's throw costs no MP (the focus paid for it).
pub fn pays_nothing(p: &mut NpcPawn) -> bool {
    if !is_goofy(p) {
        return false;
    }
    let k = gp(p);
    std::mem::take(&mut k.pays_nothing)
}

// ---------------------------------------------------------------------------------------------- the specials' tiers

/// The tier of the running special (1..=3; the menu's pick, X2's `PadState::x2.tier`; 0 = tier 1).
fn tier(p: &NpcPawn) -> u8 {
    g(p).tier.clamp(1, 3)
}

/// Per step, before the movement (pre-move): the specials' tier extras that do not need the definition's tick.
fn specials(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let _ = cx;
    let Some(run) = p.action.filter(|_| p.state == NpcStateId::Action) else { return };
    let Some(def) = run.def else { return };
    if p.state_time == 0.0 {
        // a new action: its tier and bounces
        let t = p.pad.as_ref().map_or(1, |d| d.x2.tier).clamp(1, 3);
        let ricochet = if has(p, GA::Ricochet) { K.ricochet_bounces as u8 } else { 0 };
        let barrage = std::mem::take(&mut gp(p).barrage_next);
        let k = gp(p);
        k.tier = t;
        k.extra = 0;
        k.bounced.clear();
        k.rehit.clear();
        k.barrage = barrage;
        if def == ActionDef::GoofyDiscus && !barrage {
            k.bounces = match t {
                3 => K.discus_t3_bounces as u8,
                2 => K.discus_t2_bounces as u8,
                _ => 0,
            } + ricochet;
            k.bounce_mult = K.discus_bounce_mult;
        }
        if !matches!(def, ActionDef::GoofyDiscus) {
            k.queue.clear();
        }
        // (GOOFY-GK) the leader's start-up: the start / throw clips play faster (design 9)
        let rate = match def {
            ActionDef::GoofyTornado => K.tornado_start_rate,
            ActionDef::GoofyTurbo => K.turbo_start_rate,
            ActionDef::GoofyDiscus => K.discus_throw_rate,
            _ => 1.0,
        };
        p.anim.speed = rate;
        return;
    }
    match def {
        ActionDef::GoofyTornado => {
            // tier 3: the launcher at the spin's end
            if run.sub >= 2 && tier(p) >= 3 && g(p).extra & 2 == 0 {
                gp(p).extra |= 2;
                if let Some(row) = SoloRow::goofy("Attack6_Tornado") {
                    let row = SoloRow { power: K.tornado_t3_power, ..row }.with_kind(Kind::SwirlUp);
                    let a = Area::new(row, p.position() + Vec3::Z * 90.0, 0.1, K.tornado_t3_radius, "tornado_launch");
                    shots::add(p, a);
                    log(p, "tornado_launch");
                }
            }
        }
        ActionDef::GoofyTurbo => {
            let t = tier(p);
            if run.sub == 1 {
                let mut r = run;
                let base = crate::tables::aparam(Chara::Goofy, "Attack7_Turbo", "fMoveSpeed");
                // (GOOFY-GK) the leader's ride: turbo.ride (tier 3 x turbo.t3_time), not the AI's 6 s
                if g(p).extra & 1 == 0 {
                    gp(p).extra |= 1;
                    r.timer = K.turbo_ride * if t >= 3 { K.turbo_t3_time } else { 1.0 };
                }
                if t >= 3 {
                    r.speed = base * (1.0 + K.turbo_t3_per_hit * p.hits_this_action as f32).min(K.turbo_t3_max);
                }
                p.action = Some(r);
                // tier 2: B hops mid-ride
                if t >= 2 && pressed(p, PadButton::JUMP) && p.on_ground() {
                    p.cmc.jump_z_velocity = K.turbo_t2_hop;
                    p.cmc.do_jump();
                    log(p, "turbo_hop");
                }
            }
            if run.sub >= 2 && t >= 3 && g(p).extra & 2 == 0 {
                gp(p).extra |= 2;
                if let Some(row) = SoloRow::goofy("Attack7_Turbo") {
                    let row = SoloRow { power: K.turbo_t3_end_power, ..row }.with_kind(Kind::Blow);
                    shots::add(p, Area::new(row, p.position() + Vec3::Z * 90.0, 0.1, K.turbo_t3_end_radius, "turbo_blow"));
                    log(p, "turbo_blow");
                }
            }
        }
        ActionDef::GoofyDiscus => {
            // tier 3: the catch's shockwave
            if run.sub >= 4 && tier(p) >= 3 && g(p).extra & 2 == 0 {
                gp(p).extra |= 2;
                if let Some(row) = shots::row(K.bash_row, K.discus_wave_mult) {
                    let a = Area::new(row, p.position() + Vec3::Z * 80.0, 0.1, K.discus_wave_radius, "discus_wave");
                    shots::effect(p, host, K.bash_effect, a.centre, 1.0);
                    shots::add(p, a);
                    log(p, "discus_wave");
                }
            }
        }
        _ => {}
    }
}

/// actions.rs `tornado_tick` sub-step 1: tier 2+ the stick steers the spin (true = the chase is skipped this tick);
/// tier 2+ one more loop, tier 3 the hover in the air.
pub fn tornado_steer(p: &mut NpcPawn, run: &mut ActionRun, dt: f32) -> bool {
    if !is_goofy(p) {
        return false;
    }
    let t = tier(p);
    // (GOOFY-GK) the leader steers his Tornado at every tier (tracking, design 9); tier 2+ keeps its extra loop
    if t >= 2 && g(p).extra & 1 == 0 {
        gp(p).extra |= 1;
        run.timer += K.tornado_t2_extra;
    }
    if t >= 3 && K.tornado_t3_air > 0.0 && !run.flying && !p.on_ground() {
        p.cmc.direct_z = Some(0.0);
    }
    let (yaw, mag, active) = stick(p);
    if !active {
        return false;
    }
    let d = wrap180(yaw - p.yaw);
    let s = K.tornado_t2_steer * dt;
    p.yaw = wrap180(p.yaw + d.clamp(-s, s));
    let f = forward(p.yaw) * K.tornado_t2_speed * mag.min(1.0);
    p.cmc.direct_xy = Some(Vec2::new(f.x, f.y));
    true
}

/// actions.rs `deliver`: the knock direction of a definition's hit (tier 2+ Tornado: inward, the pull).
pub fn hit_direction(p: &NpcPawn, row: &crate::tables::AttackRow, target_pos: Option<Vec3>) -> Option<Vec3> {
    if !is_goofy(p) || row.name != "Attack6_Tornado" || tier(p) < 2 {
        return None;
    }
    let at = target_pos?;
    let d = horizontal(p.position() - at).normalize_or_zero();
    (d != Vec3::ZERO).then_some(d)
}

/// party.rs `step_projectiles`, a Discus pawn hit: the bounce to the next target (Barrage queue or a ricochet). True =
/// the shield was sent on (the definition's catch logic is skipped).
pub fn discus_bounce(m: &mut NpcPawn, pr: &mut crate::projectile::Projectile, hit: TargetId, host: &PartyHost) -> bool {
    if !is_goofy(m) || pr.kind != crate::projectile::ProjectileKind::Discus {
        return false;
    }
    let from = pr.position;
    gp(m).bounced.push(hit);
    let next = loop {
        if gp(m).queue.is_empty() {
            break None;
        }
        let id = gp(m).queue.remove(0);
        if let Some(t) = host.targets.info(id).filter(shots::lockable) /* (GOOFY-GK) Sora's choice rule */ {
            break Some(t);
        }
    };
    let next = next.or_else(|| {
        if g(m).bounces == 0 {
            return None;
        }
        let skip = g(m).bounced.clone();
        let t = shots::nearest(host, from, K.discus_bounce_range, &skip)?;
        gp(m).bounces -= 1;
        Some(t)
    });
    let Some(t) = next else {
        // the queue is done: a barrage's shield comes home
        if g(m).barrage && pr.free.is_none() {
            pr.free = Some(0.0);
            pr.returning = true;
            pr.homing_on = false;
            pr.target = TargetId::NONE;
            pr.target_point = None;
            crate::actions::discus_make_free(m);
            return true;
        }
        return false;
    };
    pr.target = t.id;
    pr.target_point = Some(t.center);
    pr.homing_on = true;
    pr.caught = false;
    pr.hits = [TargetId::NONE; 8];
    pr.hits[0] = hit;
    gp(m).rehit.push((hit, K.discus_rehit));
    let dir = (t.center - from).normalize_or_zero();
    if dir != Vec3::ZERO {
        pr.velocity = dir * pr.speed.max(pr.velocity.length());
    }
    log(m, "bounce");
    true
}

/// party.rs: the Discus hit's damage scale (a barrage's every hit and a ricochet's x0.5; the first throw x1).
pub fn discus_scale(m: &NpcPawn) -> f32 {
    if !is_goofy(m) {
        return 1.0;
    }
    let k = g(m);
    if k.barrage {
        K.barrage_mult
    } else if !k.bounced.is_empty() {
        k.bounce_mult
    } else {
        1.0
    }
}

// ---------------------------------------------------------------------------------------------- landed hits

/// Every damaging hit of his that landed (the kit's own and actions.rs `deliver`): the Shield Guardian gauge, the
/// melee cancel clock.
pub fn on_hit_landed(p: &mut NpcPawn, damage: f32) {
    if !is_goofy(p) {
        return;
    }
    let t = p.state_time;
    let k = gp(p);
    if k.hit_at.is_none() {
        k.hit_at = Some(t);
    }
    // (GOOFY-GK) no Shield Guardian gauge any more (the user, 2026-10-17 00:40)
    let _ = damage;
}

/// An area of the kit hit a target (goofy_shots `step_areas`): Over Here!'s taunt on a bash.
pub fn on_area_hit(p: &mut NpcPawn, tag: &'static str, target: TargetId) {
    let _ = target;
    if matches!(tag, "bash" | "heavy" | "charge") && has(p, GA::OverHere) {
        let c = p.position();
        gp(p).taunt = Some(Taunt { centre: c, radius: K.over_here_radius, left: K.over_here_time });
    }
    if tag == "bash" || tag == "heavy" {
        let t = p.state_time;
        let k = gp(p);
        if k.hit_at.is_none() {
            k.hit_at = Some(t);
            // (GOOFY-GK) the weight: a guard bash's first landed hit freezes him (his clip, his lunge)
            k.hitstop = if tag == "heavy" { K.step_quake_hitstop } else { K.step_bash_hitstop };
        }
    }
    // (GOOFY-GK) a combo step's first landed hit: the hit confirm clock and the hit-stop
    if tag == "combo" || tag == "combo_fin" {
        let t = p.anim.time;
        let hs = super::goofy_combo::def(g(p).combo.step).hitstop;
        let k = gp(p);
        if k.combo.landed.is_none() {
            k.combo.landed = Some(t);
            k.hit_at = Some(t);
            k.hitstop = hs;
        }
    }
}

// ---------------------------------------------------------------------------------------------- the state's MainProc

/// MainProc of Goofy's pad states (pad/mod.rs `tick`).
pub fn tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    if mode_tick_finish(p, host) {
        return;
    }
    match state_n(p) {
        Some(GUARD) => guard_tick(p, cx, host),
        Some(GUARD_HIT) => guard_hit_tick(p, cx),
        Some(GUARD_BREAK) => guard_break_tick(p),
        Some(BASH) => bash_tick(p, cx, host),
        Some(HOP) => hop_tick(p),
        Some(FINISH) => finish_tick(p, cx, host),
        Some(DIVE) => dive_tick(p, cx, host),
        Some(COPTER) => copter_tick(p, cx, host),
        Some(CHARGE) => charge_tick(p),
        Some(AIM) => {}
        Some(AIR_GUARD) => air_guard_tick(p, cx),
        Some(RECOVER) => recover_tick(p),
        Some(COMBO | AIR_COMBO) => super::goofy_combo::tick(p, cx, host), // (GOOFY-GK)
        _ => p.to_idle(),
    }
    if g(p).mode_left > 0.0 && g(p).mode_left <= 0.002 && !in_state(p, BASH) {
        end_mode(p);
    }
}

/// The pad let go of him (`Party::set_pad(None)`), or he is removed: the kit's loops stop.
pub fn release(p: &mut NpcPawn, host: &mut PartyHost) {
    if is_goofy(p) {
        stop_copter_fx(p, host);
    }
}

// ---------------------------------------------------------------------------------------------- party side

impl crate::party::Party {
    /// (party X3G) Over Here!'s taunt of member `id` (Goofy leading): enemies within `radius` of `centre` should pick
    /// him as their target for `left` seconds. The hosts' enemy AI reads it (kh-app: the dummies' victim; Elden Ring:
    /// the leader body's hate, contract X3G).
    pub fn goofy_taunt(&self, id: crate::MemberId) -> Option<Taunt> {
        self.member(id)?.pad.as_ref()?.goofy.taunt
    }

    /// (party X3G) The member's extra mesh pitch while the pad drives a move that pitches him (Bombardier Dive): the
    /// hosts' drawing, as `Team::mesh_pitch`.
    pub fn pad_mesh_pitch(&self, id: crate::MemberId) -> Option<f32> {
        self.member(id)?.pad.as_ref()?.goofy.pitch
    }

    /// (party X3G) Goofy's gauges for the HUD: (shield gauge, its max, broken), (Shield Guardian gauge or seconds left
    /// in the mode, its max), (focus, its max).
    pub fn goofy_gauges(&self, id: crate::MemberId) -> Option<((f32, f32, bool), (f32, f32), (f32, f32))> {
        let m = self.member(id)?;
        let k = &m.pad.as_ref()?.goofy;
        // (GOOFY-GK) the shield's meter is GG's GUARD; the second pair is the Bombardier cooldown (left, total)
        let gd = m.status.guard?;
        Some(((gd.value, gd.max, gd.broken), (k.bomb_cd, K.bomb_cooldown), (k.focus, K.barrage_focus_max)))
    }

    /// (party X3G) The Discus Barrage locks of member `id` while he aims (the HUD's lock marks).
    pub fn goofy_locks(&self, id: crate::MemberId) -> Option<&[TargetId]> {
        let m = self.member(id)?;
        (m.state == NpcStateId::Pad(AIM)).then(|| m.pad.as_ref().map(|d| d.goofy.locks.as_slice()))?
    }

    /// (party X3G) DEBUG words for test runs (kh-app `party-word x3g ...`): `mode-full` fills the pad Goofy's Shield
    /// Guardian gauge, `gauges` prints his gauges and the kit's last moves.
    pub fn goofy_word(&mut self, words: &[&str]) -> String {
        let ids: Vec<crate::MemberId> = self.members().collect();
        let Some(id) = ids.into_iter().find(|i| self.member(*i).is_some_and(|m| m.chara == Chara::Goofy && m.pad.is_some())) else {
            return "x3g: no pad-driven Goofy (leader goofy first)".to_string();
        };
        match words {
            ["mode-full"] => {
                if let Some(pad) = self.member_mut(id).and_then(|m| m.pad.as_mut()) {
                    pad.goofy.mode_gauge = K.mode_gauge;
                }
                "x3g: Shield Guardian gauge full".to_string()
            }
            ["gauges"] => {
                let k = &self.member(id).and_then(|m| m.pad.as_ref()).map(|d| d.goofy.clone()).unwrap_or_default();
                let gd = self.member(id).and_then(|m| m.status.guard).unwrap_or_default();
                let (sy, sm, yaw, a) = self.member(id).and_then(|m| m.pad.as_ref().map(|d| (d.stick_yaw, d.stick_magnitude, m.yaw, d.frame.pad.held.has(PadButton::FaceBottom)))).unwrap_or_default();
                format!("x3g: GUARD {:.0}/{:.0}{} brace {} hop cd {:.2} bomb cd {:.1} combo {:?} focus {:.0} counter {:.2} taunt {:?} stick {sy:.0} {sm:.2} yaw {yaw:.0} A {a} log {:?}", gd.value, gd.max, if gd.broken { " BROKEN" } else { "" }, k.brace, k.hop_cd, k.bomb_cd, k.combo.step, k.focus, k.counter, k.taunt, k.log)
            }
            // (GOOFY-GK) the kit as worn (owned abilities, * = worn), his AP
            ["kit"] => {
                let s = self.member(id).map(|m| &m.status);
                let list = s.map(|s| s.goofy_kit.list().iter().map(|(k, on)| format!("{}{}", k.label(), if *on { "*" } else { "" })).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                format!("x3g: kit {list}; AP {}/{}", s.map_or(0, |s| s.ap_in_use()), s.map_or(0, |s| s.max_ap()))
            }
            _ => format!("x3g: {}: mode-full | gauges | kit", words.join(" ")),
        }
    }

    /// (party X3G-b) While member `id` (Goofy leading) is in Shield Guardian mode: his actor location (feet) and yaw, the
    /// target of the event cameras' `shield_01` (kh-partyhost `team_camera`).
    pub fn goofy_mode_actor(&self, id: crate::MemberId) -> Option<(Vec3, f32)> {
        let m = self.member(id)?;
        (m.pad.as_ref()?.goofy.mode_left > 0.0).then(|| (m.position(), m.yaw))
    }

    /// (party X3G) The party step's part of the kit: Sora's side of the shared progression (Fusion Spin / Rocket and
    /// Glide owned by Sora bring Goofy's Fusions and Tornado Copter, decisions 3 / 8), Goofy's own abilities by level.
    pub fn goofy_party_step(&mut self, dt: f32) {
        // the bouncing shield may hit a target again after `discus.rehit`
        let mut free: Vec<(crate::MemberId, TargetId)> = Vec::new();
        for id in self.members().collect::<Vec<_>>() {
            let Some(pad) = self.member_mut(id).and_then(|m| m.pad.as_mut()) else { continue };
            for r in pad.goofy.rehit.iter_mut() {
                r.1 -= dt;
                if r.1 <= 0.0 {
                    free.push((id, r.0));
                }
            }
            pad.goofy.rehit.retain(|r| r.1 > 0.0);
        }
        for (id, t) in free {
            for pr in self.projectiles.iter_mut().filter(|p| p.owner == id && p.kind == crate::projectile::ProjectileKind::Discus) {
                for h in pr.hits.iter_mut().filter(|h| **h == t) {
                    *h = TargetId::NONE;
                }
            }
        }
        let sora = self.x2.feed.as_ref().map(|f| f.sora_abilities);
        let ids: Vec<crate::MemberId> = self.members().collect();
        for id in ids {
            let Some(m) = self.member_mut(id) else { continue };
            if m.chara != Chara::Goofy {
                continue;
            }
            m.status.kit_goofy_sync_level();
            if let Some(s) = &sora {
                m.status.kit_goofy_sync_sora(s);
                // (X3G-b) the aerial recovery is Sora's AIR_RECOVERY (shared like the magic)
                let air = kh_core::enums::ETresAbilityKind::from_game_name(K.recover_needs_ability).is_none_or(|k| s.owned(k) > 0);
                if let Some(pad) = m.pad.as_mut() {
                    pad.goofy.air_recovery = air;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "goofy_tests.rs"]
mod tests;
