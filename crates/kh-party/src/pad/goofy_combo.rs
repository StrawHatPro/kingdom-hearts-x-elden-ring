//! (GOOFY-GK) GOOFY'S OWN GROUND AND AIR COMBO, the tank rework (build/sora_tmp/party/GK_GOOFY_DESIGN.md sections 2 / 3;
//! the user 2026-10-16 23:40: "extremely stiff in combat with no combo potential ... he should have combo potential on the
//! ground and his combos should be able to link into his abilities. He should feel weighty"). KH3 has no playable Goofy:
//! the RULES are the port's (after Sora's attack state: a queued press, a link point, a move point, finishers chosen by
//! ability), the PIECES are KH3's (his n_ex002 clips with their voice / swing / effect notifies, his shield's hit effect
//! and hit / swing sound sets). Every number is a row of `sheets/port/party-kit-goofy.json` (`step.*`, `combo.*`).
//!
//! The steps run in the pad states COMBO (ground) and AIR_COMBO (air): the clip copies have no Attack / Traction notify
//! and no root motion (the hit is the kit's sphere, the step's lunge replaces the root), everything else of the clip
//! plays as in KH3.

use super::goofy::{self, Finisher};
use super::goofy_kit::{GoofyAbility as GA, K};
use super::goofy_shots::{self as shots, Area, SoloRow};
use crate::host::PartyHost;
use crate::pawn::{forward, horizontal, wrap180, yaw_of, NpcPawn, StepCtx};
use crate::states::NpcStateId;
use glam::{Vec2, Vec3};
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::input::PadButton;

/// The pad states of the combo (Goofy's range 40 .. 55).
pub const COMBO: u8 = 53;
pub const AIR_COMBO: u8 = 54;

/// One step of his strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Step {
    #[default]
    Push,
    Dash,
    Sweep,
    Bash,
    Shove,
    Big,
    Quake,
    Upper,
    Air1,
    Air2,
    Air3,
}

impl Step {
    pub fn is_finisher(self) -> bool {
        matches!(self, Step::Big | Step::Quake | Step::Upper)
    }
    pub fn is_air(self) -> bool {
        matches!(self, Step::Air1 | Step::Air2 | Step::Air3)
    }
    pub fn name(self) -> &'static str {
        match self {
            Step::Push => "combo_push",
            Step::Dash => "combo_dash",
            Step::Sweep => "combo_sweep",
            Step::Bash => "combo_bash",
            Step::Shove => "combo_shove",
            Step::Big => "fin_big_bash",
            Step::Quake => "fin_shield_quake",
            Step::Upper => "fin_shield_upper",
            Step::Air1 => "air_strike",
            Step::Air2 => "air_smash",
            Step::Air3 => "air_shield_bash",
        }
    }
}

/// A step's numbers (sheet rows `step.<id>.*`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepDef {
    pub clip: &'static str,
    pub rate: f32,
    pub hit: f32,
    pub active: f32,
    pub link: f32,
    pub r#move: f32,
    pub lunge_from: f32,
    pub lunge_to: f32,
    pub lunge_max: f32,
    pub power: f32,
    pub kind: &'static str,
    pub hit_se: &'static str,
    pub hitstop: f32,
    pub radius: f32,
    /// The attack row whose shape the hit borrows (the record's name: the hosts' logs).
    pub row: &'static str,
    /// The clip has no swing SE of its own (the bash clips were the link pawn's).
    pub swing: bool,
}

macro_rules! sd {
    ($c:ident, $ra:ident, $h:ident, $a:ident, $l:ident, $m:ident, $lf:ident, $lt:ident, $lm:ident, $p:ident, $k:ident, $se:ident, $hs:ident, $rd:ident, $row:expr, $sw:expr) => {
        StepDef { clip: K.$c, rate: K.$ra, hit: K.$h, active: K.$a, link: K.$l, r#move: K.$m, lunge_from: K.$lf, lunge_to: K.$lt, lunge_max: K.$lm, power: K.$p, kind: K.$k, hit_se: K.$se, hitstop: K.$hs, radius: K.$rd, row: $row, swing: $sw }
    };
}

pub fn def(s: Step) -> StepDef {
    match s {
        Step::Push => sd!(step_push_clip, step_push_rate, step_push_hit, step_push_active, step_push_link, step_push_move, step_push_lunge_from, step_push_lunge_to, step_push_lunge_max, step_push_power, step_push_kind, step_push_hit_se, step_push_hitstop, step_push_radius, "Attack1_Push", false),
        Step::Dash => sd!(step_dash_clip, step_dash_rate, step_dash_hit, step_dash_active, step_dash_link, step_dash_move, step_dash_lunge_from, step_dash_lunge_to, step_dash_lunge_max, step_dash_power, step_dash_kind, step_dash_hit_se, step_dash_hitstop, step_dash_radius, "FR_SHIELDGUARDIAN_SHIELDBASH", true),
        Step::Sweep => sd!(step_sweep_clip, step_sweep_rate, step_sweep_hit, step_sweep_active, step_sweep_link, step_sweep_move, step_sweep_lunge_from, step_sweep_lunge_to, step_sweep_lunge_max, step_sweep_power, step_sweep_kind, step_sweep_hit_se, step_sweep_hitstop, step_sweep_radius, "Attack2_Sweep", false),
        Step::Bash => sd!(step_bash_clip, step_bash_rate, step_bash_hit, step_bash_active, step_bash_link, step_bash_move, step_bash_lunge_from, step_bash_lunge_to, step_bash_lunge_max, step_bash_power, step_bash_kind, step_bash_hit_se, step_bash_hitstop, step_bash_radius, "FR_SHIELDGUARDIAN_SHIELDBASH", true),
        Step::Shove => sd!(step_shove_clip, step_shove_rate, step_shove_hit, step_shove_active, step_shove_link, step_shove_move, step_shove_lunge_from, step_shove_lunge_to, step_shove_lunge_max, step_shove_power, step_shove_kind, step_shove_hit_se, step_shove_hitstop, step_shove_radius, "Attack1_Push", false),
        Step::Big => sd!(step_big_clip, step_big_rate, step_big_hit, step_big_active, step_big_link, step_big_move, step_big_lunge_from, step_big_lunge_to, step_big_lunge_max, step_big_power, step_big_kind, step_big_hit_se, step_big_hitstop, step_big_radius, "FR_SHIELDGUARDIAN_SHIELDBASH2", true),
        Step::Quake => sd!(step_quake_clip, step_quake_rate, step_quake_hit, step_quake_active, step_quake_link, step_quake_move, step_quake_lunge_from, step_quake_lunge_to, step_quake_lunge_max, step_quake_power, step_quake_kind, step_quake_hit_se, step_quake_hitstop, step_quake_radius, "FR_SHIELDGUARDIAN_HEAVYBASH", false),
        Step::Upper => sd!(step_upper_clip, step_upper_rate, step_upper_hit, step_upper_active, step_upper_link, step_upper_move, step_upper_lunge_from, step_upper_lunge_to, step_upper_lunge_max, step_upper_power, step_upper_kind, step_upper_hit_se, step_upper_hitstop, step_upper_radius, "Attack3_JumpAscent", false),
        Step::Air1 => sd!(step_air1_clip, step_air1_rate, step_air1_hit, step_air1_active, step_air1_link, step_air1_move, step_air1_lunge_from, step_air1_lunge_to, step_air1_lunge_max, step_air1_power, step_air1_kind, step_air1_hit_se, step_air1_hitstop, step_air1_radius, "Attack3_JumpAscent", false),
        Step::Air2 => sd!(step_air2_clip, step_air2_rate, step_air2_hit, step_air2_active, step_air2_link, step_air2_move, step_air2_lunge_from, step_air2_lunge_to, step_air2_lunge_max, step_air2_power, step_air2_kind, step_air2_hit_se, step_air2_hitstop, step_air2_radius, "Attack3_JumpDescent", false),
        Step::Air3 => sd!(step_air3_clip, step_air3_rate, step_air3_hit, step_air3_active, step_air3_link, step_air3_move, step_air3_lunge_from, step_air3_lunge_to, step_air3_lunge_max, step_air3_power, step_air3_kind, step_air3_hit_se, step_air3_hitstop, step_air3_radius, "FR_SHIELDGUARDIAN_SHIELDBASH", true),
    }
}

/// The running string (`GoofyPad::combo`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ComboRun {
    pub step: Step,
    /// Ground steps done in this string (the finisher comes after `regular_steps`), air steps done this airtime.
    pub n: u8,
    pub air_n: u8,
    /// An A press waits for the link point.
    pub queued: bool,
    /// The hit sphere is out; the first landed hit's clip time.
    pub hit_out: bool,
    pub landed: Option<f32>,
    /// The lunge (cm/s, world) and the lunge's clip window.
    pub lunge: Vec2,
    /// A held since the step's press (Shield Quake).
    pub a_hold: f32,
    /// Shield Upper's phase: 0 rising, 1 falling, 2 landed.
    pub phase: u8,
    /// Seconds the finisher still waits for the host's unison answer (0 = not waiting).
    pub wait_answer: f32,
    pub asked: bool,
    /// The swing SE of a clip without one was played.
    pub swung: bool,
}

fn gp(p: &mut NpcPawn) -> &mut goofy::GoofyPad {
    &mut p.pad.as_mut().expect("pad").goofy
}

fn g(p: &NpcPawn) -> &goofy::GoofyPad {
    &p.pad.as_ref().expect("pad").goofy
}

fn has(p: &NpcPawn, k: GA) -> bool {
    p.status.goofy_kit.has(k)
}

fn held(p: &NpcPawn, b: PadButton) -> bool {
    p.pad.as_ref().is_some_and(|d| d.frame.pad.held.has(b))
}

fn in_combo(p: &NpcPawn) -> bool {
    matches!(p.state, NpcStateId::Pad(COMBO | AIR_COMBO))
}

/// Regular ground steps before the finisher (base + Combo Plus).
pub fn regular_steps(p: &NpcPawn) -> u8 {
    K.combo_count_base as u8 + has(p, GA::ComboPlus) as u8 + has(p, GA::ComboPlus2) as u8
}

/// Air steps (Air Combo Plus adds the Air Shield Bash).
pub fn air_steps(p: &NpcPawn) -> u8 {
    2 + has(p, GA::AirComboPlus) as u8
}

/// The running step's def.
pub fn current(p: &NpcPawn) -> Option<StepDef> {
    in_combo(p).then(|| def(g(p).combo.step))
}

/// The link point of the running step has come (next press, abilities, Y, B, X).
pub fn link_open(p: &NpcPawn) -> bool {
    let Some(d) = current(p) else { return false };
    let c = g(p).combo;
    if c.step == Step::Upper {
        return c.phase >= 2 && p.anim.time >= 0.0;
    }
    c.hit_out && p.anim.time >= d.link && g(p).hitstop <= 0.0
}

/// The hit confirm: X (guard / Hop) after the first landed hit.
pub fn confirm_open(p: &NpcPawn) -> bool {
    if !in_combo(p) {
        return false;
    }
    let c = g(p).combo;
    link_open(p) || c.landed.is_some_and(|t| p.anim.time >= t + K.combo_confirm * def(c.step).rate) && g(p).hitstop <= 0.0
}

/// The move point: the stick takes him out.
pub fn move_open(p: &NpcPawn) -> bool {
    let Some(d) = current(p) else { return false };
    let c = g(p).combo;
    if c.step == Step::Upper {
        return c.phase >= 2 && p.state_time >= 0.0 && p.anim.time >= d.r#move.min(p.anim.clip.length);
    }
    p.anim.time >= d.r#move
}

/// The target a step turns to and lunges at: the lock, else the soft target.
fn target(p: &NpcPawn, host: &PartyHost) -> Option<kh_core::host::TargetInfo> {
    match p.pad.as_ref()?.target() {
        crate::command::TargetRef::Enemy(t) => host.targets.info(t),
        _ => None,
    }
}

fn stick(p: &NpcPawn) -> (f32, f32, bool) {
    let d = p.pad.as_ref().expect("pad");
    (d.stick_yaw, d.stick_magnitude, d.stick_magnitude >= kh_core::params::SORA_PARAMS.input_stick_active_min)
}

/// The gap from him to the target's surface less the reach (cm).
fn gap(p: &NpcPawn, t: &kh_core::host::TargetInfo) -> f32 {
    horizontal(t.position - p.position()).length() - t.radius - p.params.capsule_radius - K.combo_reach
}

/// A (decided in `goofy::decide`, run in pre-move): the first step / the next one. True = it acted.
pub fn press(p: &mut NpcPawn, host: &mut PartyHost) -> bool {
    if in_combo(p) {
        gp(p).combo.queued = true;
        return true;
    }
    let grounded = p.on_ground();
    if grounded {
        let first = match target(p, host) {
            Some(t) => {
                let d = horizontal(t.position - p.position()).length();
                if d >= K.combo_dash_min && d <= K.combo_dash_max {
                    Step::Dash
                } else {
                    Step::Push
                }
            }
            None => Step::Push,
        };
        gp(p).combo.n = 0;
        start(p, host, first);
        return true;
    }
    if g(p).combo.air_n < air_steps(p) {
        let s = match g(p).combo.air_n {
            0 => Step::Air1,
            1 => Step::Air2,
            _ => Step::Air3,
        };
        start(p, host, s);
        return true;
    }
    false
}

/// Starts a step (also the Turbo Bash link: `start(.., Step::Big)`).
pub fn start(p: &mut NpcPawn, host: &mut PartyHost, s: Step) {
    let d = def(s);
    let air = s.is_air();
    p.cmc.speed_limit_override = None;
    p.cmc.walk_mode = false;
    p.enter(NpcStateId::Pad(if air { AIR_COMBO } else { COMBO }));
    if air || s == Step::Upper {
        p.cmc.velocity.z = 0.0;
        p.cmc.gravity_off = true;
        p.cmc.set_flying();
    }
    p.cmc.stop_movement();
    // face the target (Sora's TurnToTarget, snapped at the start), else the stick
    let me = p.position();
    let t = target(p, host);
    // Shield Upper is chosen by pulling the stick AWAY: he keeps facing his string's line (a target ahead of him only),
    // he must not turn round to the stick (test level run 4)
    let t = if s == Step::Upper { t.filter(|t| wrap180(yaw_of(horizontal(t.position - me)) - p.yaw).abs() <= 90.0) } else { t };
    let want = t.map(|t| horizontal(t.position - me)).filter(|v| v.length() > 1.0).map(yaw_of).or_else(|| {
        let (y, _, a) = stick(p);
        (a && s != Step::Upper).then_some(y)
    });
    if let Some(w) = want {
        let dd = wrap180(w - p.yaw);
        p.yaw = wrap180(p.yaw + dd.clamp(-K.combo_turn, K.combo_turn));
    }
    // the lunge: toward the target by the gap (capped), else a short step ahead
    let dist = match t {
        Some(t) => gap(p, &t).clamp(0.0, d.lunge_max),
        None => K.combo_lunge_free.min(d.lunge_max),
    };
    let window = ((d.lunge_to - d.lunge_from) / d.rate.max(0.01)).max(0.02);
    let f = forward(p.yaw) * (dist / window);
    let clip = goofy::clip_copy(d.clip, true, false, true);
    p.anim.play(clip, K.combo_blend);
    p.anim.speed = d.rate;
    p.hit_list.clear();
    let mut c = g(p).combo;
    c.step = s;
    c.queued = false;
    c.hit_out = false;
    c.landed = None;
    c.lunge = Vec2::new(f.x, f.y);
    c.a_hold = 0.0;
    c.phase = 0;
    c.wait_answer = 0.0;
    c.asked = false;
    c.swung = false;
    if air {
        c.air_n += 1;
    } else if !s.is_finisher() {
        c.n += 1;
    }
    let k = gp(p);
    k.combo = c;
    k.hitstop = 0.0;
    k.hit_at = None;
    // finishers are invincible to their hit (Sora's Finish rule)
    k.hold_invincible = s.is_finisher();
    goofy::log_pub(p, s.name());
}

/// The finisher the string ends with (design 2): Rocket (stick toward / target in band) / Upper (stick away) / Spin
/// (crowded), each when worn; else Big Bash (Shield Quake by holding A). Fusions are `goofy::start_finisher`'s.
pub enum Fin {
    Step(Step),
    Fusion(Finisher),
}

pub fn choose(p: &NpcPawn, host: &PartyHost) -> Fin {
    let me = p.position();
    let t = target(p, host);
    let (yaw, _, active) = stick(p);
    // the stick is read against HIS FACING (the string turned him to its target at every step): the soft target follows
    // the stick (pad/mod.rs targeting), so measured against the target a pushed stick re-picked a dummy behind him and
    // the "toward" rocket flew backwards (test run 3). The rocket's target must lie ahead of him.
    let off = wrap180(yaw - p.yaw).abs();
    let ahead = t.filter(|t| wrap180(yaw_of(horizontal(t.position - me)) - p.yaw).abs() <= K.combo_rocket_toward);
    let t = ahead.or(t);
    let dist = ahead.map(|t| horizontal(t.position - me).length());
    let crowd = shots::enemies(host).iter().filter(|e| horizontal(e.position - me).length() <= K.fin_crowd_radius).count() >= K.fin_crowd_count as usize;
    if has(p, GA::FusionRocket) && t.is_some() && active && off <= K.combo_rocket_toward && dist.is_some_and(|d| d >= K.fin_rocket_min && d <= K.fin_rocket_max) {
        return Fin::Fusion(Finisher::Rocket);
    }
    if has(p, GA::ShieldUpper) && active && off >= K.combo_upper_away {
        return Fin::Step(Step::Upper);
    }
    // A still held at the finisher's moment with Shield Quake worn: the player asked for the Quake (Big Bash turns
    // into it while A stays held), over the crowd's Spin. The rocket needs the stick toward the target (no automatic
    // rocket: it carried him off on its own in test run 2).
    if has(p, GA::ShieldQuake) && held(p, PadButton::FaceBottom) {
        return Fin::Step(Step::Big);
    }
    if has(p, GA::FusionSpin) && crowd {
        return Fin::Fusion(Finisher::Spin);
    }
    Fin::Step(Step::Big)
}

/// Starts the finisher now (the queued press after the last step, the FX1 late "no", the Turbo Bash link).
pub fn finisher(p: &mut NpcPawn, host: &mut PartyHost) {
    match choose(p, host) {
        Fin::Step(s) => {
            start(p, host, s);
        }
        Fin::Fusion(f) => {
            goofy::start_fusion(p, host, f);
        }
    }
    if let Some(d) = p.pad.as_mut() {
        d.combo = 0;
    }
    gp(p).combo.n = 0;
}

/// The next step after a link (the queued press).
fn next(p: &mut NpcPawn, host: &mut PartyHost) {
    let c = g(p).combo;
    if c.step.is_air() {
        if c.air_n < air_steps(p) {
            let s = if c.air_n == 1 { Step::Air2 } else { Step::Air3 };
            start(p, host, s);
        }
        return;
    }
    if c.step.is_finisher() {
        // a new string after the finisher's link
        gp(p).combo.n = 0;
        start(p, host, Step::Push);
        return;
    }
    if c.n >= regular_steps(p) {
        // the finisher: AI Sora's unison / fusion roll first (stream D's D.6: the host answers next step)
        let answer = p.pad.as_ref().and_then(|d| d.finisher_answer);
        match answer {
            Some(true) => {
                if let Some(d) = p.pad.as_mut() {
                    (d.finisher_answer, d.finisher_pending, d.combo) = (None, false, 0);
                }
                gp(p).combo = ComboRun::default();
                goofy::log_pub(p, "team_finisher");
                goofy::leave_pub(p);
            }
            Some(false) => {
                if let Some(d) = p.pad.as_mut() {
                    d.finisher_answer = None;
                }
                finisher(p, host);
            }
            None if !c.asked => {
                if let Some(d) = p.pad.as_mut() {
                    d.finisher_pending = true;
                }
                let k = gp(p);
                k.combo.asked = true;
                k.combo.wait_answer = K.combo_answer_wait;
                k.combo.queued = true;
            }
            None => {
                if g(p).combo.wait_answer <= 0.0 {
                    if let Some(d) = p.pad.as_mut() {
                        d.finisher_pending = false;
                    }
                    finisher(p, host);
                } else {
                    gp(p).combo.queued = true;
                }
            }
        }
        return;
    }
    let s = match c.n {
        1 => Step::Sweep,
        2 => Step::Bash,
        _ => Step::Shove,
    };
    start(p, host, s);
}

/// The step's MainProc (pad/goofy.rs `tick`).
pub fn tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    let d = def(g(p).combo.step);
    let s = g(p).combo.step;
    {
        let k = gp(p);
        k.combo.wait_answer = (k.combo.wait_answer - dt).max(0.0);
    }
    if held(p, PadButton::FaceBottom) {
        gp(p).combo.a_hold += dt;
    }
    // HIT-STOP: his clip and his lunge stand still
    if g(p).hitstop > 0.0 {
        gp(p).hitstop -= dt;
        p.anim.speed = 0.0;
        p.cmc.direct_xy = Some(Vec2::ZERO);
        if p.cmc.gravity_off {
            p.cmc.direct_z = Some(0.0);
        }
        if g(p).hitstop <= 0.0 {
            gp(p).hitstop = 0.0;
            p.anim.speed = d.rate;
        }
        return;
    }
    p.anim.speed = d.rate;
    let t = p.anim.time;
    // Big Bash -> Shield Quake: A still held
    if s == Step::Big && has(p, GA::ShieldQuake) && g(p).combo.a_hold >= K.combo_quake_hold && !g(p).combo.hit_out {
        let left = t;
        start(p, host, Step::Quake);
        p.anim.time = left.min(def(Step::Quake).hit - 0.05);
        return;
    }
    // the lunge
    if s == Step::Upper {
        upper_tick(p, dt);
    } else if t >= d.lunge_from && t < d.lunge_to {
        let l = g(p).combo.lunge;
        p.cmc.direct_xy = Some(l);
    } else if t >= d.lunge_to && t < d.lunge_to + 0.05 {
        p.cmc.direct_xy = Some(Vec2::ZERO);
    }
    // the hover of an air step until its move point (Sora's air attack: gravity off until EnableChange)
    if s.is_air() && p.cmc.gravity_off {
        p.cmc.direct_z = Some(0.0);
        if t >= d.r#move || (s == Step::Air2 && t >= d.link) {
            p.cmc.gravity_off = false;
            p.cmc.direct_z = None;
            p.cmc.set_falling();
        }
    }
    // the swing SE of a bash clip (it has none of its own)
    if d.swing && !g(p).combo.swung && t >= d.hit - K.combo_swing_lead {
        gp(p).combo.swung = true;
        let at = p.position() + Vec3::Z * K.combo_up;
        shots::sound(p, host, K.combo_bash_swing_se, at);
    }
    // the hit
    if !g(p).combo.hit_out && t >= d.hit {
        gp(p).combo.hit_out = true;
        gp(p).hold_invincible = false;
        let kind = Kind::from_game_name(d.kind).unwrap_or(Kind::Small);
        let base = SoloRow::any(d.row).unwrap_or(SoloRow { name: d.row, power: 1.0, kind, attribute: kh_core::enums::ETresDamageAttribute::Physical, power_scale: 1.0, parabola_angle: 0.0, move_length: 0.0, brake: 0.0, effect_time: 0.0, reaction_power: 0, armor_attack: 0, magic: false, invalid_guard: false });
        let row = SoloRow { power: d.power, ..base }.with_kind(kind);
        let off = Vec3::new(K.combo_forward, 0.0, K.combo_up);
        let tag = if s.is_finisher() { "combo_fin" } else { "combo" };
        let mut a = Area::new(row, shots::local(p, off), (d.active / d.rate.max(0.01)).max(0.05), d.radius, tag);
        a.follow = Some(off);
        a.hit_effect = K.combo_hit_effect;
        a.hit_se = d.hit_se;
        if s == Step::Quake {
            a.size = (d.radius, K.combo_quake_grow, K.combo_quake_cap);
            a.follow = None;
            shots::effect(p, host, K.combo_quake_effect, p.position(), 1.0);
        }
        if s == Step::Upper {
            shots::effect(p, host, K.combo_upper_effect, a.centre, 1.0);
        }
        shots::add(p, a);
    }
    // the next press at the link point
    let link = link_open(p);
    if link && g(p).combo.queued {
        gp(p).combo.queued = false;
        next(p, host);
        return;
    }
    // the stick at the move point
    let (yaw, mag, active) = stick(p);
    if move_open(p) && active && !g(p).combo.queued {
        end(p);
        if p.on_ground() {
            p.to_idle();
            super::enter_run(p, yaw, mag, false);
            if let Some(dd) = p.pad.as_mut() {
                dd.run.sub = 0;
            }
        } else {
            super::enter_jump(p, true);
        }
        return;
    }
    // a ground step that ran off a ledge: the step ends, he falls (the air combo may follow)
    if !s.is_air() && s != Step::Upper && !p.on_ground() && p.state_time > 0.05 && !p.cmc.gravity_off {
        end(p);
        gp(p).combo.n = 0;
        super::enter_jump(p, true);
        return;
    }
    // an air step that has landed
    if s.is_air() && p.on_ground() && p.state_time > 0.05 {
        end(p);
        p.cmc.stop_movement();
        p.play("Land", -1.0);
        p.enter(NpcStateId::Pad(super::JUMP));
        if let Some(dd) = p.pad.as_mut() {
            dd.jump.sub = 10;
            dd.jump.air_attacks = 0;
        }
        gp(p).combo.air_n = 0;
        return;
    }
    if p.anim.finished && (s != Step::Upper || g(p).combo.phase >= 2) {
        end(p);
        let pad_combo = p.pad.as_ref().map_or(0, |d| d.combo);
        let _ = pad_combo;
        gp(p).combo.n = 0;
        if s.is_air() && !p.on_ground() {
            super::enter_jump(p, true);
        } else {
            goofy::leave_pub(p);
        }
    }
}

/// Shield Upper: a hop with the rising swing, the fall, the landing.
fn upper_tick(p: &mut NpcPawn, dt: f32) {
    let _ = dt;
    let rise = (K.step_upper_hit / K.step_upper_rate.max(0.01)).max(0.05);
    match g(p).combo.phase {
        0 => {
            p.cmc.direct_z = Some(K.combo_upper_hop / rise);
            p.cmc.direct_xy = Some(g(p).combo.lunge);
            if p.state_time >= rise {
                gp(p).combo.phase = 1;
                p.cmc.gravity_off = false;
                p.cmc.direct_z = None;
                // the hop's own rise ends here: no leftover climb speed (run 5: he floated 1.5 s)
                p.cmc.velocity.z = 0.0;
                p.cmc.direct_xy = Some(Vec2::ZERO);
                p.cmc.set_falling();
            }
        }
        1 => {
            if p.on_ground() && p.state_time > rise + 0.05 {
                gp(p).combo.phase = 2;
                p.cmc.stop_movement();
                p.play("Land", 0.05);
            }
        }
        _ => {}
    }
}

/// The step ends (whatever took him out): hover off, rate back, the lunge stops.
pub fn end(p: &mut NpcPawn) {
    p.anim.speed = 1.0;
    p.cmc.direct_xy = None;
    if p.cmc.gravity_off {
        p.cmc.gravity_off = false;
        p.cmc.direct_z = None;
        if !p.on_ground() {
            p.cmc.set_falling();
        }
    }
    let k = gp(p);
    k.hold_invincible = false;
    k.hitstop = 0.0;
    k.combo.queued = false;
}
