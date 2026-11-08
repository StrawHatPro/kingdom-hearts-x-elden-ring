//! (GOOFY) HIS BASIC ATTACK, WEAK ON PURPOSE (documentation/GOOFY_OVERHAUL.md 3): three shoves at 0.6 / 0.6 / 0.9, short reach,
//! one downward bonk in the air. The string never grows: its uses are poking, Taunt Hits (threat) and opening into a parry, so
//! any step cancels into the guard at any time with no recovery. His third hit is the finisher that rolls Unison / Fusion (R13).
//!
//! The steps run in the pad states COMBO (ground) and AIR_COMBO (air): the clip copies have no Attack / Traction notify and no
//! root motion (the hit is the kit's sphere), everything else of the clip plays as in KH3. Rows `step.*`, `combo.*`.
//!
//! THE FEEL (THE USER 2026-11-02: "Goofy's combat also feels EXTREMELY stilted, please fix this"; "His combo should move him a
//! little bit"): each ground step carries him forward by `step.<id>.travel` along the clip's own KH3 root curve (ours, eased,
//! for the bash that has none), never into an enemy body ahead and never over a drop; the clip's TurnToTarget windows keep him
//! turning to his target; the wind-up before a hit plays at `combo.windup` x. The air bonk's hit rides the shield (KH3's
//! sphere on the ATK_00 socket track) instead of a big sphere ahead of him (THE USER 2026-11-02: "His aerial hit's hitbox also
//! extends way too far away from him").
//!
//! THE FINISHER'S END LAG (THE USER 2026-11-02: "He needs some end lag at the end of his combo so you can't spam it, like
//! Sora."): the bash, the end of the string, holds him `combo.end_lag` after its hit, as Sora's finisher does (KH3's finishEnd
//! attack: no attack, stick or jump until its clip ends, guard / dodge only from its TresEnableChange). No new string starts
//! in that time, on the ground or in the air; the hop and the guard cancel it from `combo.end_lag_cancel`, the lag's clock
//! running on. The two shoves before it keep their chain and their guard cancel at any time.

use super::goofy::{self, g, gp, leave_to_ground, stick};
use super::goofy_shots::{self as shots, Area, SoloRow};
use crate::goofy::K;
use crate::host::PartyHost;
use crate::pawn::{forward, horizontal, turn_rate, yaw_of, NpcPawn, StepCtx, TurnKind};
use crate::states::NpcStateId;
use glam::{Vec2, Vec3};
use kh_core::enums::ETresDamageKind as Kind;

/// The pad states of the string.
pub const COMBO: u8 = 53;
pub const AIR_COMBO: u8 = 54;

/// One step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Step {
    #[default]
    Shove1,
    Shove2,
    Shove3,
    Bonk,
}

impl Step {
    pub fn name(self) -> &'static str {
        match self {
            Step::Shove1 => "shove1",
            Step::Shove2 => "shove2",
            Step::Shove3 => "shove3",
            Step::Bonk => "bonk",
        }
    }
}

/// A step's numbers (rows `step.<id>.*`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepDef {
    pub clip: &'static str,
    pub rate: f32,
    pub hit: f32,
    pub active: f32,
    pub link: f32,
    pub r#move: f32,
    pub power: f32,
    pub kind: &'static str,
    pub hit_se: &'static str,
    pub hitstop: f32,
    pub radius: f32,
    /// The attack row whose shape the hit borrows (the record's name: the hosts' logs).
    pub row: &'static str,
    /// The clip has no swing SE of its own.
    pub swing: bool,
    /// How far the step carries him forward (cm), given between these clip times (rows `step.<id>.travel*`; 0: none).
    pub travel: f32,
    pub travel_from: f32,
    pub travel_to: f32,
    /// The wind-up's play rate, x `rate` (`combo.windup` on the ground; the bonk's hit comes at once).
    pub windup: f32,
}

pub fn def(s: Step) -> StepDef {
    match s {
        Step::Shove1 => StepDef { clip: K.step_shove1_clip, rate: K.step_shove1_rate, hit: K.step_shove1_hit, active: K.step_shove1_active, link: K.step_shove1_link, r#move: K.step_shove1_move, power: K.step_shove1_power, kind: K.step_shove1_kind, hit_se: K.step_shove1_hit_se, hitstop: K.step_shove1_hitstop, radius: K.step_shove1_radius, row: "Attack1_Push", swing: false, travel: K.step_shove1_travel, travel_from: K.step_shove1_travel_from, travel_to: K.step_shove1_travel_to, windup: K.combo_windup },
        Step::Shove2 => StepDef { clip: K.step_shove2_clip, rate: K.step_shove2_rate, hit: K.step_shove2_hit, active: K.step_shove2_active, link: K.step_shove2_link, r#move: K.step_shove2_move, power: K.step_shove2_power, kind: K.step_shove2_kind, hit_se: K.step_shove2_hit_se, hitstop: K.step_shove2_hitstop, radius: K.step_shove2_radius, row: "Attack2_Sweep", swing: false, travel: K.step_shove2_travel, travel_from: K.step_shove2_travel_from, travel_to: K.step_shove2_travel_to, windup: K.combo_windup },
        Step::Shove3 => StepDef { clip: K.step_shove3_clip, rate: K.step_shove3_rate, hit: K.step_shove3_hit, active: K.step_shove3_active, link: K.step_shove3_link, r#move: K.step_shove3_move, power: K.step_shove3_power, kind: K.step_shove3_kind, hit_se: K.step_shove3_hit_se, hitstop: K.step_shove3_hitstop, radius: K.step_shove3_radius, row: "FR_SHIELDGUARDIAN_SHIELDBASH", swing: true, travel: K.step_shove3_travel, travel_from: K.step_shove3_travel_from, travel_to: K.step_shove3_travel_to, windup: K.combo_windup },
        Step::Bonk => StepDef { clip: K.step_bonk_clip, rate: K.step_bonk_rate, hit: K.step_bonk_hit, active: K.step_bonk_active, link: K.step_bonk_link, r#move: K.step_bonk_move, power: K.step_bonk_power, kind: K.step_bonk_kind, hit_se: K.step_bonk_hit_se, hitstop: K.step_bonk_hitstop, radius: K.step_bonk_radius, row: "Attack3_JumpDescent", swing: false, travel: 0.0, travel_from: 0.0, travel_to: 0.0, windup: 1.0 },
    }
}

/// The play rate at clip time `t`: the wind-up before the hit at `windup` x the step's rate (THE USER 2026-11-02: "Goofy's
/// combat also feels EXTREMELY stilted, please fix this": the 0.45 clip s wind-up made a press land 0.36 s late), the hit and
/// its follow-through at the step's rate.
pub fn rate_at(d: &StepDef, t: f32) -> f32 {
    if t < d.hit { d.rate * d.windup } else { d.rate }
}

/// The part of a root track that moves: (the last key still at the start, the first key at the end, the forward cm).
fn root_window(c: &crate::anim::ClipDef) -> Option<(f32, f32, f32)> {
    let (first, last) = (c.root.first()?, c.root.last()?);
    let total = last.x - first.x;
    if total < 1.0 {
        return None;
    }
    let from = c.root.iter().take_while(|k| k.x - first.x <= 0.005 * total).last().map_or(first.time, |k| k.time);
    let to = c.root.iter().find(|k| k.x - first.x >= 0.995 * total).map_or(last.time, |k| k.time);
    (to > from).then_some((from, to, total))
}

/// The share (0 .. 1) of a step's travel due by clip time `t` (THE USER 2026-11-02: "His combo should move him a little
/// bit"): KH3's own root curve of the clip (M_1301 / M_1302) laid over the step's travel window, else (SGuardShieldBash has
/// no root track) an ease in and out over it.
pub fn travel_share(d: &StepDef, t: f32) -> f32 {
    let u = ((t - d.travel_from) / (d.travel_to - d.travel_from).max(1e-4)).clamp(0.0, 1.0);
    let base = shots::clip(d.clip);
    match root_window(base) {
        Some((from, to, total)) => ((base.root_at(from + u * (to - from)).x - base.root_at(0.0).x) / total).clamp(0.0, 1.0),
        None => u * u * (3.0 - 2.0 * u),
    }
}

/// How much of `want` cm ahead he may travel now: never into the body of an enemy ahead (its radius + his + `combo.travel_gap`,
/// centre to centre, so a step stops at the body and never goes through it), and nothing at all when his front edge would be
/// over a drop deeper than his step height (the floor finder's own: his walk could not step back up it).
fn travel_room(p: &NpcPawn, host: &PartyHost, want: f32) -> f32 {
    let me = p.position();
    let f = forward(p.yaw);
    let mine = p.params.capsule_radius;
    let mut room = want;
    for t in shots::enemies(host) {
        if (t.center.z - p.capsule_centre().z).abs() > t.half_height + p.params.capsule_half_height {
            continue; // above or below him: not in his way
        }
        let rel = horizontal(t.position - me);
        let along = rel.dot(f);
        let reach = mine + t.radius + K.combo_travel_gap;
        let side = (rel - f * along).length();
        if along <= 0.0 || side >= reach {
            continue;
        }
        room = room.min((along - (reach * reach - side * side).sqrt()).max(0.0));
    }
    if room > 0.0 {
        let edge = me + f * (room + mine);
        let step = p.params.floor_max_step_height;
        if host.world.line_trace(edge + Vec3::Z * step, edge - Vec3::Z * step).is_none() {
            room = 0.0;
        }
    }
    room
}

/// This step's share of the travel, as his velocity for the next movement (frame-rate independent: the distance due by the
/// clip's time, less what was given, over the step's dt). What a body or a drop holds back is not owed later.
fn travel(p: &mut NpcPawn, host: &PartyHost, d: &StepDef, t: f32, dt: f32) {
    let due = d.travel * travel_share(d, t);
    let want = due - g(p).combo.travelled;
    let go = if want > 0.0 && dt > 0.0 && p.on_ground() { travel_room(p, host, want) } else { 0.0 };
    let k = gp(p);
    k.combo.travelled = k.combo.travelled.max(due);
    let v = forward(p.yaw) * (go / dt.max(1e-4));
    p.cmc.direct_xy = Some(Vec2::new(v.x, v.y));
}

/// The shield's sphere now (the clip's ATK_00 socket track: his frame, x forward, y right, z up from his feet), with KH3's
/// socket-space offset turned by the socket (COMBAT-FIX: `actions::GOOFY_SHIELD_OFFSET`, group "None").
fn shield_at(p: &NpcPawn) -> Option<Vec3> {
    let keys = p.anim.clip.socket;
    (!keys.is_empty()).then(|| crate::actions::socket_point_at(keys, p.anim.time, crate::actions::GOOFY_SHIELD_OFFSET))
}

/// The running string (`GoofyRun::combo`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ComboRun {
    pub step: Step,
    /// Ground steps done in this string, bonks this airtime.
    pub n: u8,
    pub air_n: u8,
    /// An A press waits for the link point.
    pub queued: bool,
    /// The hit sphere is out; the first landed hit's clip time.
    pub hit_out: bool,
    pub landed: Option<f32>,
    /// Seconds the third shove still waits for the host's unison answer.
    pub wait_answer: f32,
    pub asked: bool,
    /// The swing SE of a clip without one was played.
    pub swung: bool,
    /// The step's travel given out so far (cm; what a body or a drop held back counts as given).
    pub travelled: f32,
    /// Seconds the bash's end lag still runs (from its hit, its hit-stop not counted; `combo.end_lag`): no new string until 0,
    /// whatever state he is in.
    pub end_lag: f32,
}

fn in_combo(p: &NpcPawn) -> bool {
    matches!(p.state, NpcStateId::Pad(COMBO | AIR_COMBO))
}

/// THE FINISHER'S END LAG holds him in the bash now (THE USER 2026-11-02: "He needs some end lag at the end of his combo so
/// you can't spam it, like Sora."): no link, no stick, no jump, no Art or Breaker out of it.
pub fn end_lag_holds(p: &NpcPawn) -> bool {
    in_combo(p) && g(p).combo.end_lag > 0.0
}

/// A new string, ground or air, is refused now: the bash's end lag still runs (out of a hop or the guard that cancelled it too).
pub fn restart_held(p: &NpcPawn) -> bool {
    g(p).combo.end_lag > 0.0
}

/// X (the hop, the guard) may cancel the running step now: a shove at any time (section 3), the bash's end lag only from
/// `combo.end_lag_cancel` after its hit (Sora's rule: his finishEnd attack lets the guard and the dodge in only at its
/// TresEnableChange, 0.23 s after its hit; before that nothing cancels it).
pub fn x_cancel_open(p: &NpcPawn) -> bool {
    !end_lag_holds(p) || K.combo_end_lag - g(p).combo.end_lag >= K.combo_end_lag_cancel - 1e-4
}

/// The end lag's clock (pad/goofy.rs `pre_move`, every step): it stands still with the bash's hit-stop, as the clip does.
pub fn run_end_lag(p: &mut NpcPawn, dt: f32) {
    if in_combo(p) && g(p).hitstop > 0.0 {
        return;
    }
    let k = gp(p);
    k.combo.end_lag = (k.combo.end_lag - dt).max(0.0);
}

/// The running step's def.
pub fn current(p: &NpcPawn) -> Option<StepDef> {
    in_combo(p).then(|| def(g(p).combo.step))
}

/// The link point of the running step has come.
pub fn link_open(p: &NpcPawn) -> bool {
    let Some(d) = current(p) else { return false };
    g(p).combo.hit_out && p.anim.time >= d.link && g(p).hitstop <= 0.0 && !end_lag_holds(p)
}

/// B out of a step: after its hit landed, or from the link point (never in the bash's end lag).
pub fn confirm_open(p: &NpcPawn) -> bool {
    in_combo(p) && g(p).hitstop <= 0.0 && !end_lag_holds(p) && (link_open(p) || g(p).combo.landed.is_some())
}

/// The move point: the stick takes him out (once the bash's end lag is over).
pub fn move_open(p: &NpcPawn) -> bool {
    !end_lag_holds(p) && current(p).is_some_and(|d| p.anim.time >= d.r#move)
}

/// A on the ground: the first step, or the next one at its link point. Refused in the bash's end lag.
pub fn press(p: &mut NpcPawn, host: &mut PartyHost) -> bool {
    if restart_held(p) {
        return false;
    }
    if in_combo(p) {
        gp(p).combo.queued = true;
        return true;
    }
    gp(p).combo.n = 0;
    start(p, host, Step::Shove1);
    true
}

/// A in the air: the one bonk of an airtime (none while the bash's end lag runs: a jump out of the guard that cancelled it).
pub fn press_air(p: &mut NpcPawn, host: &mut PartyHost) -> bool {
    if in_combo(p) || g(p).combo.air_n >= 1 || restart_held(p) {
        return false;
    }
    start(p, host, Step::Bonk);
    true
}

/// Starts a step.
pub fn start(p: &mut NpcPawn, host: &mut PartyHost, s: Step) {
    let d = def(s);
    let air = s == Step::Bonk;
    p.cmc.speed_limit_override = None;
    p.cmc.walk_mode = false;
    p.enter(NpcStateId::Pad(if air { AIR_COMBO } else { COMBO }));
    if air {
        p.cmc.velocity.z = 0.0;
        p.cmc.gravity_off = true;
        p.cmc.set_flying();
    }
    p.cmc.stop_movement();
    // face the target (Sora's TurnToTarget, snapped at the start), else the stick; the clip's TurnToTarget window and the
    // step's travel go on from there (`tick`)
    let me = p.position();
    let want = goofy::target_info(p, host).map(|t| horizontal(t.position - me)).filter(|v| v.length() > 1.0).map(yaw_of).or_else(|| {
        let (y, _, a) = stick(p);
        a.then_some(y)
    });
    goofy::face(p, want, K.combo_turn);
    let clip = shots::clip_copy(d.clip, true, false, true);
    p.anim.play(clip, K.combo_blend);
    p.anim.speed = rate_at(&d, 0.0);
    p.hit_list.clear();
    let mut c = g(p).combo;
    c.step = s;
    c.queued = false;
    c.hit_out = false;
    c.landed = None;
    c.wait_answer = 0.0;
    c.asked = false;
    c.swung = false;
    c.travelled = 0.0;
    if air {
        c.air_n += 1;
    } else {
        c.n += 1;
    }
    let k = gp(p);
    k.combo = c;
    k.hitstop = 0.0;
    goofy::log(p, s.name());
}

/// The third shove now (the queued press after the host's answer, the FX1 late "no").
pub fn finisher(p: &mut NpcPawn, host: &mut PartyHost) {
    gp(p).combo.n = 2;
    start(p, host, Step::Shove3);
    if let Some(d) = p.pad.as_mut() {
        d.combo = 0;
    }
}

/// The next step after a link (the queued press). The third hit is the finisher: AI Sora's Unison / Fusion roll first (D.6,
/// R13: the host answers next step).
fn next(p: &mut NpcPawn, host: &mut PartyHost) {
    let c = g(p).combo;
    match c.step {
        Step::Bonk => {}
        Step::Shove3 => {
            gp(p).combo.n = 0;
            start(p, host, Step::Shove1);
        }
        Step::Shove1 => start(p, host, Step::Shove2),
        Step::Shove2 => {
            let answer = p.pad.as_ref().and_then(|d| d.finisher_answer);
            match answer {
                Some(true) => {
                    if let Some(d) = p.pad.as_mut() {
                        (d.finisher_answer, d.finisher_pending, d.combo) = (None, false, 0);
                    }
                    gp(p).combo = ComboRun::default();
                    goofy::log(p, "team_finisher");
                    leave_to_ground(p);
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
                None if g(p).combo.wait_answer <= 0.0 || p.pad.is_none() => {
                    if let Some(d) = p.pad.as_mut() {
                        d.finisher_pending = false;
                    }
                    finisher(p, host);
                }
                None => gp(p).combo.queued = true,
            }
        }
    }
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
    // HIT-STOP: his clip stands still
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
    let t = p.anim.time;
    p.anim.speed = rate_at(&d, t);
    // KH3's TurnToTarget windows of the clip (M_1301 0.26 - 0.51, M_1302 0.00 - 0.34, M_1311 0.00 - 0.39 clip s; the turn rate
    // of the friends' attacks): he keeps turning to his target, so the step and its travel follow it (THE USER 2026-11-02:
    // "Goofy's combat also feels EXTREMELY stilted, please fix this")
    if p.anim.window_open("TurnToTarget") {
        if let Some(at) = goofy::target_info(p, host).map(|i| i.position) {
            p.turn_toward(at, dt, turn_rate(TurnKind::Attack));
        }
    }
    // the ground step's travel (THE USER 2026-11-02: "His combo should move him a little bit")
    if s != Step::Bonk {
        travel(p, host, &d, t, dt);
    }
    // the bonk hovers to its link point, then drops (a downward bonk)
    if s == Step::Bonk && p.cmc.gravity_off {
        p.cmc.direct_z = Some(0.0);
        if t >= d.link {
            p.cmc.gravity_off = false;
            p.cmc.direct_z = None;
            p.cmc.set_falling();
        }
    }
    // the swing SE of a clip without one
    if d.swing && !g(p).combo.swung && t >= d.hit - K.combo_swing_lead {
        gp(p).combo.swung = true;
        let at = p.position() + Vec3::Z * K.combo_up;
        shots::sound(p, host, K.combo_bash_swing_se, at);
    }
    // the hit
    if !g(p).combo.hit_out && t >= d.hit {
        gp(p).combo.hit_out = true;
        let kind = Kind::from_game_name(d.kind).unwrap_or(Kind::Small);
        let base = SoloRow::any(d.row).unwrap_or_else(|| SoloRow::plain(d.row, kind));
        let row = SoloRow { power: d.power, ..base }.with_kind(kind);
        // (COMBAT-FIX) every step's sphere is on the shield (THE USER 2026-11-06: "Goofy's ground combo hit box also extends
        // out way too far"): the clip's ATK_00 track, else (SGuardShieldBash has none) KH3's own shape of the bash, the Trinity
        // Guard pawn's FR_SHIELDGUARDIAN_SHIELDBASH sphere (80 ahead, 120 up, radius step.shove3.radius 100)
        let off = shield_at(p).unwrap_or(Vec3::new(K.combo_forward, 0.0, K.combo_forward_up));
        let tag = if s == Step::Shove3 { "combo_fin" } else { "combo" };
        let mut a = Area::new(row, shots::local(p, off), (d.active / d.rate.max(0.01)).max(0.05), d.radius, tag);
        a.follow = Some(off);
        a.hit_effect = K.combo_hit_effect;
        a.hit_se = d.hit_se;
        shots::add(p, a);
        // THE BASH'S END LAG starts with its hit (THE USER 2026-11-02: "He needs some end lag at the end of his combo so you
        // can't spam it, like Sora."): a press made before it does not carry through it (Sora's CanAttack refuses the finishEnd
        // attack's presses, nothing is queued)
        if s == Step::Shove3 {
            let k = gp(p);
            k.combo.end_lag = K.combo_end_lag;
            k.combo.queued = false;
        }
    }
    // THE AIR BONK'S HIT RIDES THE SHIELD (THE USER 2026-11-02: "His aerial hit's hitbox also extends way too far away from
    // him"): KH3's own shape, the SPHERE of w_gu_WepBase's group "None" on the ATK_00 socket, along M_1311's socket track (it
    // swings from over his head to 82 cm ahead of his feet), not a 160 cm sphere 130 cm ahead of him. (COMBAT-FIX) The ground
    // steps' too (THE USER 2026-11-06: "Goofy's ground combo hit box also extends out way too far"): M_1301's / M_1302's
    // windows open the same group, so their 50 cm sphere rides the shield along those clips' tracks
    if g(p).combo.hit_out {
        if let Some(off) = shield_at(p) {
            if let Some(a) = gp(p).shots.iter_mut().rev().find(|a| a.tag == "combo") {
                a.follow = Some(off);
            }
        }
    }
    // the next press at the link point
    if link_open(p) && g(p).combo.queued {
        gp(p).combo.queued = false;
        next(p, host);
        return;
    }
    // the stick at the move point
    let (yaw, mag, active) = stick(p);
    if move_open(p) && active && !g(p).combo.queued && p.pad.is_some() {
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
    // a ground step that ran off a ledge: the step ends, he falls
    if s != Step::Bonk && !p.on_ground() && p.state_time > 0.05 && !p.cmc.gravity_off {
        end(p);
        gp(p).combo.n = 0;
        leave_to_ground(p);
        return;
    }
    // the bonk that has landed
    if s == Step::Bonk && p.on_ground() && p.state_time > 0.05 {
        end(p);
        p.cmc.stop_movement();
        gp(p).combo.air_n = 0;
        if p.pad.is_some() {
            p.play("Land", -1.0);
            p.enter(NpcStateId::Pad(super::JUMP));
            if let Some(dd) = p.pad.as_mut() {
                dd.jump.sub = 10;
                dd.jump.air_attacks = 0;
            }
        } else {
            p.to_idle();
        }
        return;
    }
    if p.anim.finished {
        end(p);
        let k = gp(p);
        k.combo.n = 0;
        k.combo.end_lag = 0.0; // the bash's clip ends its end lag at the latest
        leave_to_ground(p);
    }
}

/// The step's first landed hit: the hit-stop.
pub fn on_landed(p: &mut NpcPawn) {
    let t = p.anim.time;
    let hs = def(g(p).combo.step).hitstop;
    let k = gp(p);
    if k.combo.landed.is_none() {
        k.combo.landed = Some(t);
        k.hitstop = hs;
    }
}

/// The step ends (whatever took him out): hover off, rate back.
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
    k.hitstop = 0.0;
    k.combo.queued = false;
}

