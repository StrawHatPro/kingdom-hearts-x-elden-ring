//! (LADDER2 2026-10-25) LADDERS: Y AT THE FOOT OF ONE IS THE PARTY'S JUMP UP ONTO THE GAME'S OWN TOP EXIT.
//!
//! THE USER (2026-10-25): "change how ladders work, make them jump up the entire height of the ladder and safely land on top
//! of it when Y is pressed instead of climbing it. Discard all the custom climb animations you made. Use Torrent's wind jump
//! sound effects when jumping up the ladder. Regardless of orientation, the party leader and his party must end up where the
//! elden ring character would normally finish climbing the ladder facing the same way as the tarnished. Disable the prompt
//! and functionality for going down the ladder, as the player can just fall down it".
//!
//! HOW (sora.rs runs it once a frame; the pure parts are here, kh-party's ladder.rs has the arc):
//! 1. Y at the ladder's "Climb" prompt reaches the game as every prompt's Y does: the game starts its own ladder on the
//!    hidden body (its approach 28999, then `CSChrLadderModule.state` 0 StartBottom). Its module now holds THE LADDER AS THE
//!    GAME KNOWS IT: `bottom` (where its Tarnished stands to get on) and `top` (where its Tarnished stands when the climb up
//!    is over: the user's log has every top exactly the ladder's height above the bottom, the body's standing height at the
//!    top before a StartTop the same; 1 m ahead of the bottom, the step off onto the ledge). [`Watch`] sees the new ladder.
//! 2. THE LANDING ([`landing`]): the module's `top`, and the heading the Tarnished faces there: the ladder's own direction,
//!    bottom to top across (the game turns its body that way on the ladder; the body's yaw only when the two are one above
//!    the other). Read from the game for every ladder, any orientation; no table.
//! 3. THE GAME LETS ITS BODY GO ([`LetGo`]): its own behaviour event `W_LadderDrop` as soon as the module has a state (asked
//!    again with `W_LadderFallStart`, at the end the module is written free and the graph asked for idle: exitnet.rs's
//!    repair), and the body is the leader's again, written to him through the leap (no gravity of its own, no fall damage:
//!    the passive switches and the fall guard as for every jump).
//! 4. THE LEAP (kh-party `ladder::Leap`): the leader with his own KH3 jump (JumpNormalStart, the rise and fall loops, Land)
//!    from where he stands to the landing, facing its heading; the AI allies the same right after him, each beside the
//!    landing ([`slots`]), each with Torrent's wind (ersound.rs) as he leaves the floor. Kinematic: the landing is the
//!    game's own exit, nothing is in the way of the arc, and no hit or fall rule takes anyone out of the air.
//! 5. GOING DOWN IS NOT OFFERED: the game's "Descend" prompt (ActionButtonText [`DESCEND_TEXT_ID`]) is not shown and Y never
//!    reaches the game for it (sora.rs `watch_ui`): the player walks off the edge and drops. A ladder the game still starts
//!    from the top (StartTop, by any other way) is let go at once with no leap.

use glam::Vec3;

/// The game's ladder state names (SDK `LadderState`, from common_define.hks).
pub fn state_name(state: i8) -> &'static str {
    match state {
        -1 => "None",
        0 => "StartBottom",
        1 => "StartTop",
        2 => "UpRight",
        3 => "UpLeft",
        4 => "DownRight",
        5 => "DownLeft",
        6 => "EndTop",
        7 => "EndBottom",
        8 => "IdleRight",
        9 => "IdleLeft",
        10 => "AttackUpRight",
        11 => "AttackUpLeft",
        12 => "AttackDownRight",
        13 => "AttackDownLeft",
        14 => "CoastStart",
        15 => "CoastRight",
        16 => "CoastStop",
        18 => "CoastLeft",
        20 => "CoastLanding",
        21 => "DamageSmall",
        22 => "DamageLarge",
        _ => "?",
    }
}

/// The ladder animations of the body (c0000) by default: what the user's log showed (28999, 28100, 28030, 28050).
pub const DEFAULT_ANIMS: (i32, i32) = (28000, 28999);

/// ActionButtonText's "Descend" (the climb down from the top; "Climb" is 3300): MEASURED in the user's log (`prompt: ... the
/// game's prompt: row -1 text 3301 "Descend"`). A text id, the same in every language.
pub const DESCEND_TEXT_ID: i32 = 3301;

/// The game's prompt is its ladder's climb down (by its text id; by its English text too).
pub fn is_descend_prompt(text_id: i32, text: Option<&str>) -> bool {
    text_id == DESCEND_TEXT_ID || text.is_some_and(|t| t.trim().eq_ignore_ascii_case("descend"))
}

/// THE BODY GETS ON A LADDER (pure; sora.rs feeds it once a frame): one start per ladder of the game's, from its approach
/// animation or its state until the body has neither (or a ladder animation without a state outlived `ANIM_ONLY_MAX`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Watch {
    busy: bool,
    anim_only_for: f32,
}

impl Watch {
    /// A ladder animation without a ladder state counts this long at most (the approach, a let-go's fall).
    pub const ANIM_ONLY_MAX: f32 = 3.0;

    /// One frame. `state`: the body's ladder state (as believed); `ladder_anim`: its animation is one of the ladder
    /// animations; `allowed`: the rule may apply (no cutscene, the leader alive, no load / warp this frame). True on the
    /// frame a new ladder starts.
    pub fn frame(&mut self, state: i8, ladder_anim: bool, allowed: bool, dt: f32) -> bool {
        if state >= 0 || !ladder_anim {
            self.anim_only_for = 0.0;
        } else {
            self.anim_only_for += dt;
        }
        let on = state >= 0 || ladder_anim && self.anim_only_for <= Self::ANIM_ONLY_MAX;
        if !on {
            self.busy = false;
            return false;
        }
        if self.busy || !allowed {
            return false;
        }
        self.busy = true;
        true
    }

}

/// THE GAME LETS ITS BODY GO (pure; sora.rs fires what it says): `W_LadderDrop` as soon as the module has a state, asked
/// again (`W_LadderFallStart`) after `RETRY_S`, written free after `FORCE_S`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LetGo {
    pub active: bool,
    pub for_s: f32,
    pub fired: Option<f32>,
    pub retried: bool,
    pub let_go: bool,
    /// Seconds since the let-go; its graph was asked for idle.
    pub free_for: f32,
    pub idled: bool,
    /// (no leap: a start at the top, an animation of the range without a ladder) only a ladder state is let go: an approach
    /// that never gets one ends quietly, nothing written or fired (a ladder kicked down from above plays out).
    pub quiet: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LetStep {
    /// Nothing to do this frame.
    Wait,
    /// Fire `W_LadderDrop` now.
    Drop,
    /// Fire `W_LadderFallStart` (the game kept its body on the ladder after the drop).
    Retry,
    /// The game let its body go now.
    LetGo,
    /// The game kept its body: write its module free and ask its graph for idle (exitnet.rs's repair).
    Force,
}

impl LetGo {
    pub const RETRY_S: f32 = 0.4;
    pub const FORCE_S: f32 = 1.2;
    /// The approach before the game's ladder state: this long at most, then the module is written free.
    pub const APPROACH_MAX_S: f32 = 0.8;
    /// After the let-go, a body still in a ladder animation (its fall) this long has its graph asked for idle (once).
    pub const IDLE_AFTER_S: f32 = 0.15;
    /// The let-go ends at the latest this long after it (the net and the stale rule take anything left).
    pub const OVER_S: f32 = 1.5;

    pub fn start(&mut self) {
        *self = LetGo { active: true, ..LetGo::default() };
    }

    /// `start` for a start that is no climb up (see `quiet`).
    pub fn start_quiet(&mut self) {
        *self = LetGo { active: true, quiet: true, ..LetGo::default() };
    }

    pub fn frame(&mut self, ladder_state: i8, dt: f32) -> LetStep {
        if !self.active || self.let_go {
            return LetStep::Wait;
        }
        self.for_s += dt;
        match self.fired {
            None if ladder_state >= 0 => {
                self.fired = Some(self.for_s);
                LetStep::Drop
            }
            None if self.for_s >= Self::APPROACH_MAX_S => {
                self.let_go = true;
                if self.quiet {
                    LetStep::LetGo
                } else {
                    LetStep::Force
                }
            }
            None => LetStep::Wait,
            Some(_) if ladder_state < 0 => {
                self.let_go = true;
                LetStep::LetGo
            }
            Some(at) if self.for_s - at >= Self::FORCE_S => {
                self.let_go = true;
                LetStep::Force
            }
            Some(at) if !self.retried && self.for_s - at >= Self::RETRY_S => {
                self.retried = true;
                LetStep::Retry
            }
            Some(_) => LetStep::Wait,
        }
    }

    /// After the let-go: true once when the body still plays a ladder animation `IDLE_AFTER_S` later (sora.rs then asks
    /// its graph for idle: a body in a ladder animation greys the game's prompts).
    pub fn idle_due(&mut self, ladder_anim: bool, dt: f32) -> bool {
        if !(self.active && self.let_go) {
            return false;
        }
        self.free_for += dt;
        if !self.idled && ladder_anim && self.free_for >= Self::IDLE_AFTER_S {
            self.idled = true;
            return true;
        }
        false
    }

    /// The let-go is over: the body out of its ladder animations, or `OVER_S` after the let-go.
    pub fn over(&self, ladder_anim: bool) -> bool {
        self.active && self.let_go && ((!ladder_anim && self.free_for > 0.0) || self.free_for >= Self::OVER_S)
    }

    /// The body may be moved by the leader (no let-go waiting for the game, or the game let go).
    pub fn body_free(&self) -> bool {
        !self.active || self.let_go
    }

    /// The game is being asked to let go (its own switches stay on its body meanwhile so its graph can drop it).
    pub fn letting_go(&self) -> bool {
        self.active && !self.let_go
    }

    pub fn end(&mut self) {
        *self = LetGo::default();
    }
}

/// The leap starts once the game let its body go, or this long after the ladder's start at the latest (s).
pub const LAUNCH_S: f32 = 0.5;

/// THE LANDING (KH3 space): the ladder module's top and the heading its Tarnished faces there (deg, KH3 yaw): the ladder's
/// own direction from its bottom to its top across, else `body_yaw` (the game turns its body to the ladder). None: the
/// module holds no ladder (not finite, too short, too long).
pub fn landing(bottom: Vec3, top: Vec3, body_yaw: f32) -> Option<(Vec3, f32)> {
    if !bottom.is_finite() || !top.is_finite() {
        return None;
    }
    let d = top - bottom;
    if d.length() > 20_000.0 || d.z.abs() < 50.0 {
        return None;
    }
    let across = Vec3::new(d.x, d.y, 0.0);
    let facing = if across.length() >= 15.0 { across.y.atan2(across.x).to_degrees() } else { body_yaw };
    Some((top, kh_core::leap::wrap180(facing)))
}

/// The body this near the ladder's bottom (KH3 cm: across, up / down) is at its foot.
pub const AT_FOOT_CM: (f32, f32) = (300.0, 200.0);

/// The game started its ladder AT ITS BOTTOM (a climb up): StartBottom or its approach (no state yet), the body at the
/// ladder's foot (`AT_FOOT_CM`: never a ladder the module still holds from before, nor a start at the top: StartTop, the
/// kick of a ladder from above).
pub fn from_bottom(state: i8, feet: Vec3, bottom: Vec3, top: Vec3) -> bool {
    let d = feet - bottom;
    let at_foot = Vec3::new(d.x, d.y, 0.0).length() <= AT_FOOT_CM.0 && d.z.abs() <= AT_FOOT_CM.1;
    match state {
        1 => false,
        0 => at_foot,
        _ => at_foot && feet.distance(bottom) < feet.distance(top),
    }
}

#[cfg_attr(not(feature = "party"), allow(dead_code))]
fn forward(yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    Vec3::new(c, s, 0.0)
}

/// THE PARTY'S PLACES ON THE LANDING (KH3 floor points): `n` places beside the leader's `to`, `side` cm to his right, his
/// left, then twice as far, each on a floor (`floor`: the floor near a point, None = none) the landing reaches in a clear
/// line (`clear`); a place with no floor tries one `side` further from the edge (ahead), and at the last the landing
/// itself (nobody falls).
#[cfg_attr(not(feature = "party"), allow(dead_code))]
pub fn slots(to: Vec3, facing: f32, n: usize, side: f32, floor: impl Fn(Vec3) -> Option<Vec3>, clear: impl Fn(Vec3, Vec3) -> bool) -> Vec<Vec3> {
    // (KH3 / UE axes: X forward, Y right, Z up; facing +X his right is +Y)
    let (f, right) = (forward(facing), forward(facing + 90.0));
    let mut tries: Vec<Vec3> = Vec::new();
    for k in 1..=3 {
        let s = side * k as f32;
        for d in [right * s, -right * s, right * s + f * side, -right * s + f * side] {
            tries.push(d);
        }
    }
    tries.extend([f * side, f * side * 2.0]);
    let mut taken: Vec<Vec3> = vec![to];
    let mut out = Vec::new();
    for _ in 0..n {
        let spot = tries.iter().filter_map(|d| {
            let p = floor(to + *d)?;
            ((p.z - to.z).abs() <= 60.0 && taken.iter().all(|t| t.distance(p) >= side * 0.6) && clear(to, p)).then_some(p)
        }).next();
        let p = spot.unwrap_or(to);
        taken.push(p);
        out.push(p);
    }
    out
}

/// What Sora's body slot shows during his leap: his own simulation's animation, or a KH3 clip of the leap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Show {
    Own,
    Clip { anim: kh_core::AnimId, time: f32, looping: bool, play: u32 },
}

/// SORA'S LEAP (sora.rs: the leader Sora or AI Sora): the arc in KH3 space (his origin), the clip's play count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoraLeap {
    pub leap: kh_core::leap::Leap,
    phase: kh_core::leap::Phase,
    since: f32,
    plays: u32,
}

impl SoraLeap {
    pub fn new(leap: kh_core::leap::Leap) -> SoraLeap {
        SoraLeap { leap, phase: kh_core::leap::Phase::Wait, since: 0.0, plays: 0 }
    }

    /// One step: the moment, and the clip his body slot shows (KH3's own jump: JumpNormalStart, the rise loop, the fall
    /// loop, Land).
    pub fn step(&mut self, dt: f32) -> (kh_core::leap::Sample, Show) {
        use kh_core::AnimId;
        use kh_core::leap::Phase;
        let s = self.leap.advance(dt);
        if s.phase != self.phase {
            self.phase = s.phase;
            self.since = 0.0;
            self.plays += 1;
        } else {
            self.since += dt;
        }
        let clip = |anim: AnimId, looping: bool, play: u32, since: f32| {
            let length = anim.data().length.max(0.01);
            Show::Clip { anim, time: if looping { since.rem_euclid(length) } else { since.min(length) }, looping, play }
        };
        let show = match s.phase {
            Phase::Wait | Phase::Done => Show::Own,
            Phase::Start => clip(AnimId::JumpNormalStart, false, self.plays, self.since),
            Phase::Rise => clip(AnimId::JumpNormalRiseLoop, true, self.plays, self.since),
            Phase::Fall => clip(AnimId::JumpNormalDownLoop, true, self.plays, self.since),
            Phase::Land => clip(AnimId::Land, false, self.plays, self.since),
        };
        (s, show)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    /// The user's ladder (er\logs\kh-er.log 85.6 s: bottom (9461, -862, -9), top (9387, -794, 1091), the body facing 138 on
    /// it): the landing is the top, facing the ladder's own heading; every orientation lands facing its own.
    #[test]
    fn the_landing_is_the_games_top_facing_the_ladders_heading() {
        let (bottom, top) = (Vec3::new(9461.0, -862.0, -9.0), Vec3::new(9387.0, -794.0, 1091.0));
        let (to, facing) = landing(bottom, top, 0.0).unwrap();
        assert_eq!(to, top);
        assert!((facing - 137.4).abs() < 1.0, "{facing}");
        for yaw in [-170.0f32, -90.0, -30.0, 0.0, 45.0, 90.0, 135.0, 180.0] {
            let across = Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0) * 100.0;
            let (_, f) = landing(Vec3::ZERO, across + Vec3::Z * 600.0, 999.0).unwrap();
            assert!(kh_core::leap::wrap180(f - yaw).abs() < 0.01, "{yaw} -> {f}");
        }
        // straight above: the body's yaw (the game turned it to the ladder)
        assert_eq!(landing(Vec3::ZERO, Vec3::new(3.0, 2.0, 500.0), 77.0).map(|l| l.1), Some(77.0));
        assert_eq!(landing(Vec3::ZERO, Vec3::new(1e9, 0.0, 0.0), 0.0), None);
        assert_eq!(landing(Vec3::ZERO, Vec3::new(f32::NAN, 0.0, 0.0), 0.0), None);
        assert!(from_bottom(0, bottom + Vec3::Z * 8.0, bottom, top) && !from_bottom(1, bottom, bottom, top));
        assert!(from_bottom(-1, bottom + Vec3::X * 30.0, bottom, top) && !from_bottom(-1, top, bottom, top));
        // a module still holding a ladder from before, far away: no leap
        assert!(!from_bottom(-1, bottom + Vec3::X * 2000.0, bottom, top) && !from_bottom(0, bottom + Vec3::new(0.0, 0.0, -900.0), bottom, top));
    }

    /// One start per ladder: the approach animation then the state count as one; the next ladder starts again; a load or a
    /// cutscene (not allowed) starts none.
    #[test]
    fn one_start_per_ladder() {
        let mut w = Watch::default();
        assert!(!w.frame(-1, false, true, DT));
        assert!(w.frame(-1, true, true, DT), "the approach");
        assert!((0..30).all(|_| !w.frame(0, true, true, DT)));
        assert!((0..30).all(|_| !w.frame(-1, true, true, DT)), "the let-go's fall: the same ladder");
        assert!(!w.frame(-1, false, true, DT));
        assert!(w.frame(0, true, true, DT), "the next ladder");
        let mut off = Watch::default();
        assert!((0..60).all(|_| !off.frame(0, true, false, DT)));
        // a ladder animation with no state that never ends counts 3 s at most: past it the next ladder may start
        let mut stuck = Watch::default();
        assert!(stuck.frame(-1, true, true, DT));
        let frames = (0..400).take_while(|_| !stuck.frame(-1, true, true, DT) && stuck.busy).count();
        assert!((frames as f32 * DT - Watch::ANIM_ONLY_MAX).abs() < 0.05, "{frames}");
        assert!(stuck.frame(0, true, true, DT), "a real ladder state after it starts a new one");
    }

    /// The let-go: the drop as soon as the module has a state, the game's answer, the retry, the forced end; an approach
    /// that never reaches a state ends too. Never waits for ever.
    #[test]
    fn the_let_go_always_ends() {
        let mut l = LetGo::default();
        l.start();
        assert!(!l.body_free() && l.letting_go());
        assert_eq!(l.frame(-1, DT), LetStep::Wait, "the approach");
        assert_eq!(l.frame(0, DT), LetStep::Drop);
        assert_eq!(l.frame(0, DT), LetStep::Wait);
        assert_eq!(l.frame(-1, DT), LetStep::LetGo);
        assert!(l.body_free());
        let mut k = LetGo::default();
        k.start();
        let steps: Vec<LetStep> = (0..200).map(|_| k.frame(9, DT)).collect();
        assert_eq!(steps.iter().filter(|s| **s == LetStep::Retry).count(), 1);
        let forced = steps.iter().position(|s| *s == LetStep::Force).unwrap();
        assert!(((forced) as f32 * DT - LetGo::FORCE_S).abs() < 0.05, "{forced}");
        assert!(k.body_free());
        let mut a = LetGo::default();
        a.start();
        let forced = (0..200).position(|_| a.frame(-1, DT) == LetStep::Force).unwrap();
        assert!(((forced + 1) as f32 * DT - LetGo::APPROACH_MAX_S).abs() < 0.03, "{forced}");
        // no climb up (a kick from above): nothing forced, nothing fired without a ladder state
        let mut q = LetGo::default();
        q.start_quiet();
        let steps: Vec<LetStep> = (0..200).map(|_| q.frame(-1, DT)).collect();
        assert!(!steps.contains(&LetStep::Force) && !steps.contains(&LetStep::Drop) && steps.contains(&LetStep::LetGo));
        // after the let-go: idle asked once while the fall plays, over when it ends (or at OVER_S)
        let idles = (0..30).filter(|_| l.idle_due(true, DT)).count();
        assert_eq!(idles, 1);
        assert!(!l.over(true) && l.over(false));
        let mut m = LetGo::default();
        m.start();
        m.frame(0, DT);
        m.frame(-1, DT);
        let over = (0..200).position(|_| { m.idle_due(true, DT); m.over(true) }).unwrap();
        assert!(((over + 1) as f32 * DT - LetGo::OVER_S).abs() < 0.03, "{over}");
    }

    /// The party's places: beside the leader to his right and left on the top floor, never on him nor on each other, never
    /// over the drop (no floor there: ahead instead), and the landing itself when nothing else is floor.
    #[test]
    fn the_party_lands_beside_the_leader() {
        let to = Vec3::new(0.0, 0.0, 500.0);
        // a ledge: floor only ahead of x = -10 (the drop behind the landing)
        let ledge = |p: Vec3| (p.x >= -10.0).then_some(Vec3::new(p.x, p.y, 500.0));
        let s = slots(to, 0.0, 2, 90.0, ledge, |_, _| true);
        assert_eq!(s.len(), 2);
        assert!((s[0] - Vec3::new(0.0, 90.0, 500.0)).length() < 1e-3, "his right first (facing +X: +Y): {:?}", s[0]);
        assert!((s[1] - Vec3::new(0.0, -90.0, 500.0)).length() < 1e-3, "{:?}", s[1]);
        // a corridor 1 m wide: nothing beside him, ahead instead
        let corridor = |p: Vec3| (p.y.abs() < 50.0 && p.x >= -10.0).then_some(Vec3::new(p.x, p.y, 500.0));
        let s = slots(to, 0.0, 2, 90.0, corridor, |_, _| true);
        assert!(s.iter().all(|p| p.x > 50.0 && (p.z - 500.0).abs() < 1e-3), "{s:?}");
        assert!(s[0].distance(s[1]) >= 54.0);
        // nothing at all: the landing
        assert_eq!(slots(to, 0.0, 1, 90.0, |_| None, |_, _| true), vec![to]);
        // a wall between: not there
        let s = slots(to, 0.0, 1, 90.0, ledge, |_, b| b.y < 0.0);
        assert!(s[0].y < 0.0, "{s:?}");
    }

    /// Sora's clips through the leap: KH3's own jump, start, rise, fall, land, then his own animation again.
    #[test]
    fn soras_leap_shows_his_own_jump() {
        use kh_core::AnimId;
        let mut s = SoraLeap::new(kh_core::leap::Leap::new(Vec3::ZERO, 0.0, Vec3::new(100.0, 0.0, 800.0), 0.0, 0.0));
        let mut seen = Vec::new();
        for _ in 0..400 {
            let (_, show) = s.step(DT);
            let anim = match show {
                Show::Clip { anim, .. } => Some(anim),
                Show::Own => None,
            };
            if seen.last() != Some(&anim) {
                seen.push(anim);
            }
        }
        assert_eq!(seen, vec![Some(AnimId::JumpNormalStart), Some(AnimId::JumpNormalRiseLoop), Some(AnimId::JumpNormalDownLoop), Some(AnimId::Land), None]);
        assert!(s.leap.done());
    }

    #[test]
    fn the_descend_prompt_is_known_by_its_id() {
        assert!(is_descend_prompt(3301, None));
        assert!(is_descend_prompt(-1, Some("Descend")));
        assert!(!is_descend_prompt(3300, Some("Climb")));
        assert_eq!(state_name(1), "StartTop");
    }
}
