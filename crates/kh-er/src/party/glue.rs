//! (ER party S1) THE ELDEN RING PARTY HOST'S CORE: `kh_partyhost::PartyRuntime` driven in the contract's step order
//! (docs/party-contracts.md ER.0), plus what the game needs on top of the test level's host, all game-free (the game
//! reads are in `party/mod.rs`; the offline harness at the end of this file runs it over synthetic geometry):
//!
//! - THE STEP ORDER (ER.0): `route` before Sora's step (the formation, the pad to a leading friend, AI Sora's input);
//!   Sora's step with `SoraTargets` (sora.rs); `step` after it: the waypoints, `PartyRuntime::step` (AI ticks,
//!   `Party::step`, `Team::step`, the heals), the poses (`PartyRuntime::pose`: the members' taps for the effect / sound
//!   side), `after_step` (the leader's lock change for the camera), the trail, the fallbacks, the bodies' want list.
//!   Nothing is stepped while Sora's steps are not (pause, menus, loads: the host does not call it).
//! - THE NAV (nav.rs): the leader's breadcrumb trail; the friends' MoveTo goes round corners by its waypoints; the AI's
//!   EQS and warp rule see it as their `Nav`.
//! - LADDERS (LADDER2, the user 2026-10-25: Y jumps the whole ladder, "the party leader and his party must end up where the
//!   elden ring character would normally finish climbing the ladder facing the same way as the tarnished"): when the
//!   leader leaps up a ladder (sora.rs, ladder.rs), every AI friend and AI Sora leap right after him from where they stand
//!   (`ladder.gap` s apart) onto places beside the landing (`ladder.side` cm: his right, his left, on the top floor),
//!   facing its heading (`ladder_jump`). No walking to the ladder, no climb; the trail gets a break there (a jump up).
//! - ANYWHERE, AT ALL TIMES (the user, U1: "they ... need to be able to be anywhere at all times, they should never be
//!   forced to despawn or break"): the runtime outlives every Sora host (party/mod.rs keeps it across loads, deaths,
//!   fast travel, quitting to the title); nothing here ever removes a member. A load, a warp of the player's or the end
//!   of a cutscene puts the friends beside the leader (RESEAT, while nothing is shown) and empties the trail; map tile
//!   changes and origin shifts need nothing (the KH3 frame moves with the game's origin); a fog wall or a door the body
//!   walks through: the friends who are cut off take KH3's warp to the leader (CATCH-UP). A friend who cannot move for
//!   `stuck.after` s with the leader far takes the same warp. In a real-time cutscene (U6) the friends are HIDDEN: not
//!   stepped, not drawn, their bodies parked at the leader and unhittable; they reappear beside him after it.
//!
//! Log lines `party: ...` (one per change); `party.trace = 1` adds the AI trace and a line a second of the nav's numbers.

use std::collections::VecDeque;

use glam::Vec3;
use kh_core::{Input, Output, Sora, TargetId};
use kh_host::formation::Formation;
use kh_host::party::{FriendAiSettings, PartyChara};
use kh_party::{CommandStatus, CommandTicket, LadderJump, MemberCommand, MemberId, MoveGoal, NpcStateId};
use kh_partyhost::{AllyWant, MemberTap, PartyPoser, PartyRuntime, PartyTargets, ProjectileTap, RouteIn, RuntimeConfig, StepIn};

use super::nav::{self, CastBudget, Casts, NavCache, NavRules, Steer, Trail, TrailNav, NAV_CAPSULE};

/// The host's numbers (sheet `sheets/port/party-er-host.json`; these defaults are the same rows).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostRules {
    pub nav: NavRules,
    /// Casts the nav may make per member per step (the soft budget: past it every nav answer is the optimistic one).
    pub casts_per_member: u32,
    /// (LADDER2) The ladder jump: the party lands this far beside the leader (cm) ...
    pub ladder_side: f32,
    /// ... and each one jumps this long after the one before (s).
    pub ladder_gap: f32,
    /// A friend with a MoveTo running, farther than `stuck_far` from the leader, who moved less than `stuck_moved` in
    /// `stuck_after` seconds takes KH3's warp.
    pub stuck_after: f32,
    pub stuck_moved: f32,
    pub stuck_far: f32,
    /// The catch-up after a body action (a fog wall, a door): `catchup_delay` s after it ends, a friend farther than
    /// `catchup_far` or without a clear line to the leader takes KH3's warp.
    pub catchup_delay: f32,
    pub catchup_far: f32,
    /// The warp's place behind the leader (cm).
    pub warp_behind: f32,
}

impl HostRules {
    pub const DEFAULT: HostRules = HostRules { nav: NavRules::DEFAULT, casts_per_member: 24, ladder_side: 90.0, ladder_gap: 0.2, stuck_after: 8.0, stuck_moved: 50.0, stuck_far: 400.0, catchup_delay: 0.5, catchup_far: 600.0, warp_behind: 250.0 };

    /// Sheet `party-er-host` (`{"rows": [[id, value, ...], ...]}`): the rules and the rows not understood.
    pub fn from_sheet(text: &str) -> Result<(HostRules, Vec<String>), String> {
        let json: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let rows = json["rows"].as_array().ok_or("no rows")?;
        let mut r = HostRules::DEFAULT;
        let mut problems = Vec::new();
        for row in rows {
            let id = row[0].as_str().unwrap_or("");
            let Some(v) = row[1].as_f64().map(|v| v as f32) else {
                problems.push(format!("{id}: not a number"));
                continue;
            };
            let n = &mut r.nav;
            match id {
                "nav.spacing" => n.spacing = v.max(10.0),
                "nav.max_crumbs" => n.max_crumbs = v.max(2.0) as usize,
                "nav.lift" => n.lift = v,
                "nav.probe_down" => n.probe_down = v,
                "nav.floor_every" => n.floor_every = v.max(50.0),
                "nav.search" => n.search = v.max(1.0) as usize,
                "nav.reach" => n.reach = v,
                "nav.jump_up_break" => n.jump_up_break = v,
                "nav.teleport" => n.teleport = v,
                "casts.per_member" => r.casts_per_member = v.max(0.0) as u32,
                "ladder.side" => r.ladder_side = v.clamp(30.0, 300.0),
                "ladder.gap" => r.ladder_gap = v.clamp(0.0, 1.0),
                "stuck.after" => r.stuck_after = v,
                "stuck.moved" => r.stuck_moved = v,
                "stuck.far" => r.stuck_far = v,
                "catchup.delay" => r.catchup_delay = v,
                "catchup.far" => r.catchup_far = v,
                "warp.behind" => r.warp_behind = v,
                _ => problems.push(format!("{id}: not a row of this host")),
            }
        }
        Ok((r, problems))
    }
}

impl Default for HostRules {
    fn default() -> HostRules {
        HostRules::DEFAULT
    }
}

/// Why the party is not stepped / shown now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Park {
    Live,
    /// A real-time cutscene (U6), the wait for the ground after the game put its player somewhere else.
    Hidden(&'static str),
}

/// The game's body (the player object) this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyObs {
    /// The body's feet, KH3 space.
    pub feet: Vec3,
    /// The body's yaw (KH3 degrees).
    pub yaw: f32,
}

/// What the host knows once a frame (before the steps).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameIn {
    /// `party.er` (default on).
    pub on: bool,
    /// The first frame of a new Sora host (a load, a respawn, fast travel, a new game).
    pub fresh: bool,
    /// The game put its player somewhere else this frame (sora.rs `relocated`).
    pub relocated: bool,
    /// Why nothing of the party is shown now (a cutscene, the wait for the ground), None = play.
    pub hidden: Option<&'static str>,
    /// The game moves the body in an action of its own (a door, a fog wall, a pick-up).
    pub body_action: bool,
    pub body: Option<BodyObs>,
    /// `party.pose` (default on): the members' poses are evaluated.
    pub pose: bool,
    /// `party.trace`.
    pub trace: bool,
    pub dt: f32,
    pub now: f64,
}

/// A hit on an AI ally's body, already through the host's damage-in rule (S3's `AllyHit`).
#[derive(Clone, Copy, Debug)]
pub enum AllyHitIn {
    /// Donald / Goofy: the final record (contract B.3 `receive_hit`).
    Friend(PartyChara, kh_core::HitRecord),
    /// AI Sora: his own reaction and guard rules.
    Sora(kh_core::states::HitOnPlayer),
}

/// Per friend (Donald 0, Goofy 1): the fallbacks' clocks.
#[derive(Clone, Copy, Debug, Default)]
struct Job {
    /// A fallback warp waits for the camera to look away: seconds waited.
    warp: Option<f32>,
    anchor: Option<Vec3>,
    still: f32,
}

/// (LADDER2) A friend's ladder jump given (its command, asked again while the pawn refuses it, for `LEAP_RETRY_S`).
#[derive(Clone, Copy, Debug)]
struct LeapAsk {
    id: MemberId,
    jump: LadderJump,
    ticket: CommandTicket,
    for_s: f32,
}

/// (LADDER2) A friend busy when the ladder jump was given is asked again this long.
pub const LEAP_RETRY_S: f32 = 2.0;

/// What one step leaves for the other streams (S2 leader body, S3 ally bodies, S4 drawing, S5 sound / effects, S7
/// cameras): read after `step`.
#[derive(Clone, Debug, Default)]
pub struct StepFeed {
    /// S3: the AI allies' bodies wanted (contract ER.5; parked: at the leader, unhittable).
    pub wants: Vec<AllyWant>,
    /// S5: the members' bones / sockets / weapon sockets of this step (contract ER.4 `MemberTap`).
    pub taps: Vec<MemberTap>,
    /// S5: the party's spell actors and team props (contract ER.7).
    pub projectiles: Vec<ProjectileTap>,
    /// S7: the leader's RB lock changed (the lock-on camera's target).
    pub lock_change: Option<kh_partyhost::LockChange>,
    /// The party stepped this step.
    pub stepped: bool,
    /// The party's events of the step (kh-party: items used, hits, KOs ...): the host's status rules read them.
    pub events: Vec<kh_party::PartyEvent>,
    /// (LADDER2) AI Sora (a friend leads) jumps the ladder after the leader: his leap (sora.rs runs it on his kh-core place).
    pub sora_leap: Option<LadderJump>,
    /// (LADDER2) Where a member left the floor on the ladder jump this step (KH3): the jump's sound (sora.rs plays it).
    pub ladder_sounds: Vec<Vec3>,
    /// S3: the save point rested the party (S6's `partyui::sync`): every AI ally's body is cured.
    pub cure_bodies: bool,
}

/// THE CORE (one per game run: party/mod.rs keeps it across Sora hosts).
pub struct PartyCore {
    pub rt: PartyRuntime,
    /// The members' posers (S4 draws `poser.pose(id, alpha)`; loaded once by the host).
    pub poser: PartyPoser,
    pub rules: HostRules,
    pub trail: Trail,
    pub park: Park,
    /// `party.er` as last given.
    pub on: bool,
    pose_on: bool,
    trace: bool,
    reseat_pending: bool,
    catch_up_in: Option<f32>,
    was_action: bool,
    jobs: [Job; 2],
    /// AI Sora's (a friend leads): the host gives him no waypoints.
    sora_job: Job,
    /// (LADDER2) AI Sora is in his ladder jump this step (sora.rs sets it before `step`): no fallback for him.
    pub sora_leaping: bool,
    /// (LADDER2) The leader's ladder jump for the party's next step (the landing, KH3 floor point; its heading), and the
    /// friends' jumps given.
    leap: Option<(Vec3, f32)>,
    leap_asks: Vec<LeapAsk>,
    /// S8: a loaded save's unlock record (S6's sync hands it over; S8 takes it).
    pub restored_unlock_record: Option<serde_json::Value>,
    /// The drawn camera of the last frame (the AI's camera, the Warp state's view test).
    pub camera: kh_camera::CameraView,
    /// A formation / AI settings asked for (S6's Party Formation, a load, S8's unlocks), taken at the next `route`.
    pub asked_formation: Option<Formation>,
    pub asked_ai: Vec<(PartyChara, FriendAiSettings)>,
    /// S8: who is unlocked (None = everyone: no unlock rule has spoken yet).
    unlocked: Option<Vec<PartyChara>>,
    hits: Vec<AllyHitIn>,
    /// The latest step's feed (S2 .. S7 read it).
    pub feed: StepFeed,
    /// Lines said (the newest last; the host writes them to its log as they come, the harness reads them).
    pub lines: VecDeque<String>,
    pub sink: Option<fn(&str)>,
    // per second: steps, nav casts, starved queries
    sec: (f32, u32, u32, u32),
    quiet: u32,
    /// The game's world casts the party made since the last line (the host counts them round `step`).
    pub world_casts: u32,
    /// The weapon mounted per draw id (S4's model follows `FriendStatus::weapon`).
    mounted: [String; 3],
    /// The nav's answers of the last few steps (nav.rs `NavCache`; taken out of the core while a step uses it).
    cache: NavCache,
    /// (LDI) The game's body (the player object) as the latest frame read it: its feet (KH3) and yaw. A FRIEND LEADER is
    /// seated on it by S2 AFTER the party's step, so at a reseat his own view is still where he was before the load / warp:
    /// the reseat puts the others beside the body instead (the place the leader is seated at in the same step).
    body: Option<(Vec3, f32)>,
}

/// A fallback warp waits at most this long for the camera to look away (s).
pub const WARP_WAIT: f32 = 3.0;

fn index_of(c: PartyChara) -> usize {
    if c == PartyChara::Donald {
        0
    } else {
        1
    }
}

fn forward(yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    Vec3::new(c, s, 0.0)
}

impl PartyCore {
    pub fn new(config: RuntimeConfig, rules: HostRules) -> PartyCore {
        let nothing = kh_camera::CameraView { position: Vec3::ZERO, rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 0.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        PartyCore {
            rt: PartyRuntime::new(config),
            poser: PartyPoser::default(),
            rules,
            trail: Trail::default(),
            park: Park::Live,
            on: true,
            pose_on: true,
            trace: false,
            reseat_pending: true,
            catch_up_in: None,
            was_action: false,
            jobs: [Job::default(); 2],
            sora_job: Job::default(),
            sora_leaping: false,
            leap: None,
            leap_asks: Vec::new(),
            restored_unlock_record: None,
            camera: nothing,
            asked_formation: None,
            asked_ai: Vec::new(),
            unlocked: None,
            hits: Vec::new(),
            feed: StepFeed::default(),
            lines: VecDeque::new(),
            sink: None,
            sec: (0.0, 0, 0, 0),
            quiet: 0,
            world_casts: 0,
            mounted: Default::default(),
            cache: NavCache::default(),
            body: None, // (LDI)
        }
    }

    pub fn say(&mut self, text: String) {
        if let Some(f) = self.sink {
            f(&text);
        }
        self.lines.push_back(text);
        while self.lines.len() > 400 {
            self.lines.pop_front();
        }
    }

    /// S4: the friends are drawn now (play, the party on, someone in it).
    /// (LDI) ...and they have been put where they belong: while a RESEAT is pending (a load, a warp, the end of a park)
    /// their views are of the place before it (after a load: of the OLD host's frame, anywhere, even the origin), so they
    /// are not drawn until the reseat's step has put them beside the leader.
    pub fn visible(&self) -> bool {
        self.on && self.park == Park::Live && !self.rt.members.is_empty() && !self.reseat_pending
    }

    /// (LDI) A reseat waits for the next party step (the friends are not drawn until it is made).
    pub fn placing(&self) -> bool {
        self.reseat_pending
    }

    /// Sora is not in the active party: the host does not step him (ER.0 step 2).
    pub fn sora_skipped(&self) -> bool {
        self.on && !self.rt.lead.sora_active
    }

    /// S3: a hit on an AI ally's body (applied at the start of the next party step).
    pub fn queue_hit(&mut self, hit: AllyHitIn) {
        self.hits.push(hit);
    }

    /// S2's SEAT COUNTER-MOVE (`LeaderBody::apply`'s return: the KH3 frame moved under the members who do not lead, and
    /// they were moved back by `delta`): the trail and the clocks' places follow.
    pub fn counter_move(&mut self, delta: Vec3) {
        if delta == Vec3::ZERO {
            return;
        }
        self.trail.shift(delta);
        if let Some((to, _)) = self.leap.as_mut() {
            *to += delta;
        }
        for j in self.jobs.iter_mut() {
            if let Some(a) = j.anchor.as_mut() {
                *a += delta;
            }
        }
    }

    /// S8: WHO IS UNLOCKED (the user's rule, 2026-10-15: Goofy after Leonine Misbegotten, Donald after talking to him,
    /// Sora after Margit when he is not the starter). A locked character leaves the formation; one newly unlocked joins
    /// it (up to 3). Until S8 calls this, everyone is available.
    pub fn set_unlocked(&mut self, list: &[PartyChara], now: f64) {
        if self.unlocked.as_deref() == Some(list) {
            return;
        }
        let before = self.unlocked.clone();
        self.unlocked = Some(list.to_vec());
        let base = self.asked_formation.clone().unwrap_or_else(|| self.rt.lead.formation.clone());
        let mut active: Vec<PartyChara> = base.active().iter().copied().filter(|c| list.contains(c)).collect();
        for c in list {
            let new = before.as_ref().is_some_and(|b| !b.contains(c)) || before.is_none();
            if new && !active.contains(c) && active.len() < 3 {
                active.push(*c);
            }
        }
        active.sort_by_key(|c| *c as u8);
        let leader = if active.contains(&base.leader()) { base.leader() } else { active.first().copied().unwrap_or(PartyChara::Sora) };
        match Formation::new(&active, leader) {
            Ok(f) => {
                self.say(format!("party: t={now:.3}s unlocked {:?}: the formation is {:?}, leader {}", list.iter().map(|c| c.name()).collect::<Vec<_>>(), f.active().iter().map(|c| c.name()).collect::<Vec<_>>(), f.leader().name()));
                self.asked_formation = Some(f);
            }
            Err(e) => {
                let text = format!("party: t={now:.3}s unlocked {:?}: no formation ({})", list.iter().map(|c| c.name()).collect::<Vec<_>>(), e.text());
                self.say(text);
            }
        }
    }

    /// A LOADED SAVE'S UNLOCKS (S6's `partyui::sync` `restore_unlocked`): set as they are, WITHOUT the "newly unlocked
    /// join" rule of `set_unlocked` (the loaded formation stands as saved).
    pub fn restore_unlocked(&mut self, list: &[PartyChara], now: f64) {
        self.unlocked = Some(list.to_vec());
        self.say(format!("party: t={now:.3}s the loaded save's unlocks: {:?} (the saved formation stands)", list.iter().map(|c| c.name()).collect::<Vec<_>>()));
        // (ERT4) ...UNLESS IT HOLDS SOMEONE NOT UNLOCKED. A save with no party record (a new game as Goofy that has not
        // saved at a save point yet: the title's file carries the starter, no formation) loads KH3's default formation
        // (Sora, Donald, Goofy, Sora leading). ERT4's run: a KH3 game over -> Continue put Sora in the lead of a Goofy-only
        // character. The formation keeps only the unlocked; the leader stays if unlocked, else the first of them.
        let base = self.asked_formation.clone().unwrap_or_else(|| self.rt.lead.formation.clone());
        if list.is_empty() || base.active().iter().all(|c| list.contains(c)) {
            return;
        }
        let mut active: Vec<PartyChara> = base.active().iter().copied().filter(|c| list.contains(c)).collect();
        if active.is_empty() {
            active = list.iter().copied().take(3).collect();
        }
        active.sort_by_key(|c| *c as u8);
        let leader = if active.contains(&base.leader()) { base.leader() } else { list.iter().copied().find(|c| active.contains(c)).unwrap_or(active[0]) };
        if let Ok(f) = Formation::new(&active, leader) {
            self.say(format!("party: t={now:.3}s (ERT4) the loaded formation held characters not unlocked: it is {:?}, leader {}", f.active().iter().map(|c| c.name()).collect::<Vec<_>>(), f.leader().name()));
            self.asked_formation = Some(f);
        }
    }

    /// The characters S8 has unlocked (None = no unlock rule yet: everyone).
    pub fn unlocked(&self) -> Option<&[PartyChara]> {
        self.unlocked.as_deref()
    }

    /// (LADDER2) THE LEADER LEAPS UP A LADDER (sora.rs, the frame his leap starts): the landing (KH3, the floor point the
    /// game's Tarnished stands on after the climb) and its heading; the party jumps after him at the next step.
    pub fn ladder_jump(&mut self, to: Vec3, facing: f32) {
        if self.on && to.is_finite() && facing.is_finite() {
            self.leap = Some((to, facing));
        }
    }

    /// ONCE A FRAME, before the steps: the keys, loads / warps / cutscenes (park, reseat).
    pub fn frame(&mut self, f: FrameIn) {
        if f.on != self.on {
            self.on = f.on;
            let text = format!("party: t={:.3}s party.er = {}: {}", f.now, f.on as u8, if f.on { "Donald, Goofy and AI Sora run (put beside the leader)" } else { "Sora alone, as before the party; the friends wait, frozen and not drawn (party.er = 1 brings them back beside him)" });
            self.say(text);
            if f.on {
                self.reseat_pending = true;
                self.trail.cut();
                self.cache.clear_all();
            }
        }
        self.pose_on = f.pose;
        // (LDI) the body's place this frame (the friend leader's seat; see `body`)
        self.body = f.body.map(|l| (l.feet, l.yaw)).filter(|(p, y)| p.is_finite() && y.is_finite());
        if f.trace != self.trace {
            self.trace = f.trace;
            self.rt.trace = f.trace;
        }
        if f.fresh || f.relocated {
            self.reseat_pending = true;
            self.trail.cut();
            self.cache.clear_all();
            self.leap = None;
            self.leap_asks.clear();
            let text = format!("party: t={:.3}s {}: the friends are put beside the leader when he stands again; the trail is cut", f.now, if f.fresh { "a new Sora host (a load, a respawn, fast travel)" } else { "the game put its player somewhere else" });
            self.say(text);
        }
        match (self.park, f.hidden) {
            (Park::Live, Some(why)) => {
                self.park = Park::Hidden(why);
                self.say(format!("party: t={:.3}s HIDDEN ({why}): not stepped, not drawn, the bodies parked at the leader and unhittable", f.now));
            }
            (Park::Hidden(was), None) => {
                self.park = Park::Live;
                self.reseat_pending = true;
                self.trail.cut();
                self.cache.clear_all();
                self.say(format!("party: t={:.3}s shown again after {was}: the friends are put beside the leader", f.now));
            }
            (Park::Hidden(a), Some(b)) if a != b => self.park = Park::Hidden(b),
            _ => {}
        }
        // a body action (a door, a fog wall) over: the friends who are cut off catch up
        if self.was_action && !f.body_action {
            self.catch_up_in = Some(self.rules.catchup_delay);
        }
        self.was_action = f.body_action;
    }

    /// ER.0 STEP 1, BEFORE SORA'S STEP: the formation, the pad's frame to a leading friend, AI Sora's input. Returns Sora's
    /// input (the pad's when he leads).
    #[allow(clippy::too_many_arguments)]
    pub fn route(&mut self, sora: &mut Sora, input: &Input, dt: f32, world: &dyn kh_core::World, targets: &dyn PartyTargets, now: f64) -> Input {
        if !self.on || self.park != Park::Live {
            return *input;
        }
        let budget = CastBudget::new(self.rules.casts_per_member);
        let cache = std::mem::take(&mut self.cache);
        let casts = Casts::new(world, self.rules.nav, &budget, &cache);
        let nav = TrailNav { casts: &casts, trail: &self.trail, capsule: nav::NAV_CAPSULE };
        let asked_formation = self.asked_formation.take();
        let asked_ai = std::mem::take(&mut self.asked_ai);
        let input = self.rt.route(RouteIn { sora, input, dt, targets, camera: &self.camera, asked_formation, asked_ai, nav: Some(&nav), now });
        drop(casts);
        self.cache = cache;
        input
    }

    /// ER.0 STEPS 3 - 5, RIGHT AFTER SORA'S STEP.
    #[allow(clippy::too_many_arguments)]
    pub fn step(&mut self, sora: &mut Sora, out: &Output, dt: f32, lock_on: TargetId, world: &dyn kh_core::World, ik: &dyn kh_anim::IkWorld, targets: &mut dyn PartyTargets, audio: &mut dyn kh_core::Audio, effects: &mut dyn kh_core::Effects, sora_look: Option<Vec3>, now: f64) {
        self.feed = StepFeed::default();
        if !self.on {
            return;
        }
        if let Park::Hidden(_) = self.park {
            // parked: at the leader, unhittable (S3 keeps or re-makes the bodies there)
            let leader = self.rt.leader_view_of(out, sora.status());
            self.feed.wants = self.rt.ally_wants(out, sora.status()).into_iter().map(|w| AllyWant { feet: leader.position, hittable: false, invulnerable: true, ..w }).collect();
            return;
        }
        // S3's hits of the frame
        for h in std::mem::take(&mut self.hits) {
            match h {
                AllyHitIn::Friend(c, record) => {
                    if let Some(r) = self.rt.receive_hit(c, &record) {
                        // (FINAL4) a hit his guard took (or that did not land) bleeds nothing
                        if r.guarded || !r.accepted {
                            crate::blood::note_guarded(c);
                        }
                        if r.killed {
                            self.say(format!("party: t={now:.3}s {} is down (KO) after a blow of {:.0}", c.name(), record.damage));
                        }
                    }
                }
                AllyHitIn::Sora(hit) => sora.take_hit(hit),
            }
        }
        let budget = CastBudget::new(self.rules.casts_per_member * (self.rt.members.len() as u32 + 1));
        let cache = std::mem::take(&mut self.cache);
        cache.tick();
        let casts = Casts::new(world, self.rules.nav, &budget, &cache);
        if self.reseat_pending && !self.rt.members.is_empty() {
            self.reseat_pending = false;
            self.reseat(sora, out, &casts, now);
        }
        // (LADDER2) the leader's ladder jump: the party after him
        if let Some((to, facing)) = self.leap.take() {
            self.ladder_jumps(&casts, out, sora, to, facing, now);
        }
        self.leap_asks_again(dt, now);
        self.steer_members(&casts, &*targets, out, sora, now);
        let o = {
            let nav = TrailNav { casts: &casts, trail: &self.trail, capsule: NAV_CAPSULE };
            self.rt.step(StepIn { dt, sora: &mut *sora, out, lock_on, camera: &self.camera, world, targets: &mut *targets, audio, effects, nav: Some(&nav), now })
        };
        // (ZETTA 2026-10-23) a friend caught in a move loop (kh-partyhost hopguard.rs): put back by the leader (the warp)
        for id in std::mem::take(&mut self.rt.hop_loops) {
            if let Some(c) = self.rt.members.iter().find(|m| m.1 == id).map(|m| m.0) {
                self.warp(c, "a move loop: four take-offs in six seconds", now);
            }
        }
        // (LADDER2) who left the floor on the ladder jump this step (the leader too, a friend leading): its sound
        self.feed.ladder_sounds = self.rt.party.take_ladder_takeoffs().into_iter().map(|(_, at)| at).collect();
        self.feed.stepped = o.stepped;
        self.feed.events = o.events;
        if o.stepped {
            self.feed.projectiles = o.projectiles;
        }
        if self.rt.members.is_empty() {
            drop(casts);
            self.cache = cache;
            return;
        }
        // ER.0 step 4: the poses (the members' taps for the effect / sound side)
        if self.pose_on {
            self.rt.sora_look_point = sora_look;
            // (FACE) the members' voice lines (the audio host's last step) for their faces' lip-sync
            let voices = crate::fx::party_voices();
            if !voices.is_empty() {
                self.poser.set_voices(&voices);
            }
            self.feed.taps = self.rt.pose(&mut self.poser, ik, dt);
        }
        // ER.0 step 5: the leader's state, his lock for the lock-on camera
        self.feed.lock_change = self.rt.after_step(&*targets, dt, now);
        // the trail: the leader's feet after the step
        let leader = self.rt.leader_view_of(out, sora.status());
        self.trail.record(leader.position, leader.on_floor, &self.rules.nav);
        self.fallbacks(&casts, out, sora, dt, now);
        self.sora_fallback(&casts, out, sora, dt, now);
        self.pending_warps(&casts, out, sora, dt, now);
        drop(casts);
        self.cache = cache;
        self.feed.wants = self.rt.ally_wants(out, sora.status());
        // a line a second (party.trace) / when the budget starved
        self.sec.0 += dt;
        self.sec.1 += 1;
        self.sec.2 += budget.used.get();
        self.sec.3 += budget.starved.get();
        if self.sec.0 >= 1.0 {
            let (_, steps, used, starved) = self.sec;
            let world = std::mem::take(&mut self.world_casts);
            // every second with party.trace; else once every 30 s (the log stays readable)
            self.quiet = (self.quiet + 1) % 30;
            if self.trace || self.quiet == 1 {
                self.say(format!("party casts: {used} nav queries in {steps} steps ({:.1} a member a step, budget {}), {starved} answered optimistically for want of casts; {world} casts of the game's world by the party this second (movement + nav); trail {} crumbs ({} cut(s))", used as f32 / (steps.max(1) * self.rt.members.len().max(1) as u32) as f32, self.rules.casts_per_member, self.trail.len(), self.trail.cuts));
            }
            self.sec = (0.0, 0, 0, 0);
        }
    }

    fn goal_point(&self, goal: MoveGoal, out: &Output, sora: &Sora, targets: &dyn PartyTargets) -> Option<Vec3> {
        match goal {
            MoveGoal::Point(p) => Some(p),
            MoveGoal::Sora => Some(self.rt.leader_view_of(out, sora.status()).position),
            MoveGoal::Member(m) => self.rt.party.view(m).map(|v| v.position),
            MoveGoal::Target(t) => targets.info(t).map(|i| i.position),
        }
    }

    /// (LADDER2) THE PARTY JUMPS THE LADDER RIGHT AFTER THE LEADER: every AI friend (not knocked out) and AI Sora (a friend
    /// leads, Sora in the party) get a place beside the landing (ladder.rs `slots`: the leader's right, his left, on the
    /// top floor the landing reaches) and leap there from where they stand, one after the other (`ladder.gap`), facing the
    /// landing's heading. The trail gets a break (nobody walks that way).
    fn ladder_jumps(&mut self, casts: &Casts, out: &Output, sora: &Sora, to: Vec3, facing: f32, now: f64) {
        let mut who: Vec<Option<(PartyChara, MemberId)>> = Vec::new();
        for (c, id) in self.rt.members.clone() {
            if Some(id) == self.rt.leader || self.rt.party.view(id).is_none_or(|v| v.ko) {
                continue;
            }
            who.push(Some((c, id)));
        }
        if self.rt.leader.is_some() && self.rt.lead.sora_active && sora.status().hp > 0 {
            who.push(None);
        }
        let places = crate::ladder::slots(to, facing, who.len(), self.rules.ladder_side, |p| casts.floor(p, 150.0, 300.0), |a, b| casts.clear(a, b, NAV_CAPSULE));
        let mut said = Vec::new();
        for (k, (w, at)) in who.iter().zip(&places).enumerate() {
            let jump = LadderJump { to: *at, facing, delay: self.rules.ladder_gap * (k + 1) as f32 };
            match w {
                Some((c, id)) => {
                    self.rt.party.set_steer(*id, None);
                    let ticket = self.rt.party.command(*id, MemberCommand::LadderJump(jump));
                    self.leap_asks.retain(|a| a.id != *id);
                    self.leap_asks.push(LeapAsk { id: *id, jump, ticket, for_s: 0.0 });
                    self.jobs[index_of(*c)] = Job::default();
                    said.push(format!("{} at ({:.0}, {:.0}, {:.0}) after {:.2} s", c.name(), at.x, at.y, at.z, jump.delay));
                }
                None => {
                    self.feed.sora_leap = Some(jump);
                    self.sora_job = Job::default();
                    said.push(format!("AI Sora at ({:.0}, {:.0}, {:.0}) after {:.2} s", at.x, at.y, at.z, jump.delay));
                }
            }
        }
        self.trail.mark_break();
        let leader = self.rt.leader_view_of(out, sora.status());
        self.say(format!("party: t={now:.3}s THE LADDER JUMP: {} leaps to ({:.0}, {:.0}, {:.0}) facing {facing:.0}; the party after him: {}", leader.chara.name(), to.x, to.y, to.z, if said.is_empty() { "nobody".to_string() } else { said.join(", ") }));
    }

    /// (LADDER2) A friend who refused the ladder jump (busy in a reaction, an item, a team attack) is asked again, at once,
    /// until he takes it or `LEAP_RETRY_S` has passed (then the stuck fallback brings him).
    fn leap_asks_again(&mut self, dt: f32, now: f64) {
        let mut asks = std::mem::take(&mut self.leap_asks);
        asks.retain_mut(|a| {
            a.for_s += dt;
            match self.rt.party.command_status(a.id, a.ticket) {
                CommandStatus::Failed if a.for_s < LEAP_RETRY_S => {
                    a.ticket = self.rt.party.command(a.id, MemberCommand::LadderJump(LadderJump { delay: 0.0, ..a.jump }));
                    true
                }
                CommandStatus::Failed => {
                    let name = self.rt.chara_of_id(a.id).map_or("a friend", |c| c.name());
                    self.say(format!("party: t={now:.3}s {name} could not take the ladder jump in {LEAP_RETRY_S:.0} s: the stuck fallback brings him"));
                    false
                }
                CommandStatus::Running => a.for_s < 10.0,
                _ => false,
            }
        });
        self.leap_asks = asks;
    }

    /// THE WAYPOINTS of the AI friends' MoveTo (nav.rs `steer`).
    fn steer_members(&mut self, casts: &Casts, targets: &dyn PartyTargets, out: &Output, sora: &Sora, _now: f64) {
        for (_, id) in self.rt.members.clone() {
            if Some(id) == self.rt.leader {
                self.rt.party.set_steer(id, None);
                continue;
            }
            let Some(v) = self.rt.party.view(id) else { continue };
            if v.ko || v.state == NpcStateId::Ladder {
                self.rt.party.set_steer(id, None);
                continue;
            }
            let Some(goal) = self.rt.party.move_goal(id).and_then(|g| self.goal_point(g, out, sora, targets)) else {
                self.rt.party.set_steer(id, None);
                continue;
            };
            let capsule = (v.capsule.0.max(20.0), v.capsule.1.max(30.0), v.capsule.1.max(30.0) + 2.0);
            match nav::steer(casts, &self.trail, v.position, goal, capsule) {
                Steer::Straight | Steer::Lost => self.rt.party.set_steer(id, None),
                Steer::Via(p) => self.rt.party.set_steer(id, Some((goal, p))),
            }
        }
    }

    /// The floor near `around` at `yaw` / `length` that the friend can walk to from there; else `around`.
    fn spot(&self, casts: &Casts, around: Vec3, yaw: f32, length: f32) -> Vec3 {
        for (turn, scale) in [(0.0, 1.0), (40.0, 1.0), (-40.0, 1.0), (90.0, 1.0), (-90.0, 1.0), (180.0, 1.0), (0.0, 0.5), (180.0, 0.5)] {
            let p = around + forward(yaw + turn) * length * scale;
            let Some(floor) = casts.floor(p, 150.0, 300.0) else { continue };
            if (floor.z - around.z).abs() <= 150.0 && casts.clear(around, floor, NAV_CAPSULE) {
                return floor;
            }
        }
        around
    }

    /// RESEAT (a load, a warp, the end of a cutscene, `party.er` back on): every AI friend at his home place beside the
    /// leader (sheet party-test spawn.offsets), AI Sora too while a friend leads; nothing is shown at that moment.
    fn reseat(&mut self, sora: &mut Sora, out: &Output, casts: &Casts, now: f64) {
        let mut leader = self.rt.leader_view_of(out, sora.status());
        // (LDI) a friend leading: the body's place (he is seated there by S2 after this step; his own view is still the old one)
        let mut on_body = false;
        if let (Some(_), Some((feet, yaw))) = (self.rt.leader, self.body) {
            on_body = feet.distance(leader.position) > 1.0;
            (leader.position, leader.yaw) = (feet, yaw);
        }
        let mut said = Vec::new();
        for (c, id) in self.rt.members.clone() {
            if Some(id) == self.rt.leader {
                continue;
            }
            let (length, angle) = self.rt.test.offsets[index_of(c)];
            let at = self.spot(casts, leader.position, leader.yaw + angle, length);
            self.put(id, at, leader.yaw);
            // (LDI) no floor found anywhere around him (the world's collision not answering): at his feet, said
            let lost = at == leader.position && length > 0.0;
            said.push(format!("{} at ({:.0}, {:.0}, {:.0}){}", c.name(), at.x, at.y, at.z, if lost { " (NO FLOOR FOUND around the leader: at his feet)" } else { "" }));
        }
        if on_body {
            said.push("(the friend leader's seat: the game's body)".to_string());
        }
        if self.rt.leader.is_some() && self.rt.lead.sora_active {
            let at = self.spot(casts, leader.position, leader.yaw + 110.0, 180.0);
            sora.teleport(kh_core::Pose { position: at + Vec3::Z * kh_core::Sora::origin_height_on_floor(&kh_core::SORA_PARAMS), yaw: leader.yaw });
            said.push(format!("AI Sora at ({:.0}, {:.0}, {:.0})", at.x, at.y, at.z));
        }
        // the drawing does not slide them from where they were
        let views: Vec<_> = self.rt.members.iter().filter_map(|(_, id)| self.rt.party.view(*id)).collect();
        self.rt.current = views.clone();
        self.rt.previous = views;
        self.jobs = [Job::default(); 2];
        self.sora_job = Job::default();
        self.catch_up_in = None;
        self.say(format!("party: t={now:.3}s RESEAT beside {} at ({:.0}, {:.0}, {:.0}): {}", leader.chara.name(), leader.position.x, leader.position.y, leader.position.z, said.join(", ")));
    }

    /// THE FALLBACK WARP of a friend to the leader. KH3's own Warp state (kh-party) only moves a friend more than 30 m from
    /// the leader (its threshold ends it at once nearer), and a friend stuck behind a fog wall is
    /// nearer than that: the host moves him itself, AS KH3'S WARP DOES (at the moment neither he nor the place behind
    /// the leader is in the drawn camera's view), at most `WARP_WAIT` s later in view (logged) so that he never stays stuck.
    fn warp(&mut self, c: PartyChara, why: &str, now: f64) {
        let k = index_of(c);
        if self.jobs[k].warp.is_none() {
            self.jobs[k].warp = Some(0.0);
            self.say(format!("party: t={now:.3}s {} WARPS to the leader ({why}): when out of view", c.name()));
        }
    }

    /// AI SORA'S FALLBACK (a friend leads): he is stuck far from the leader: the same out-of-view warp as the friends'.
    fn sora_fallback(&mut self, _casts: &Casts, out: &Output, sora: &Sora, dt: f32, now: f64) {
        // (LADDER2) in his ladder jump (sora.rs holds his kh-core state meanwhile): the leap moves him, no clock runs
        if self.rt.leader.is_none() || !self.rt.lead.sora_active || sora.status().hp <= 0 || self.sora_leaping {
            self.sora_job = Job::default();
            return;
        }
        let leader = self.rt.leader_view_of(out, sora.status());
        let far = out.position.distance(leader.position);
        let j = &mut self.sora_job;
        match j.anchor {
            Some(a) if far > self.rules.stuck_far * 2.0 && a.distance(out.position) < self.rules.stuck_moved => j.still += dt,
            _ => {
                j.anchor = Some(out.position);
                j.still = 0.0;
            }
        }
        if j.warp.is_none() && j.still > self.rules.stuck_after {
            j.warp = Some(0.0);
            self.say(format!("party: t={now:.3}s AI Sora WARPS to the leader (stuck, {far:.0} cm): when out of view"));
        }
    }

    /// The fallback warps waiting for the camera to look away (`warp`).
    fn pending_warps(&mut self, casts: &Casts, out: &Output, sora: &mut Sora, dt: f32, now: f64) {
        let leader = self.rt.leader_view_of(out, sora.status());
        let lens = self.rt.ai_camera(&self.camera);
        let cam = kh_party::PartyCamera { position: lens.position, pitch: lens.pitch, yaw: lens.yaw, fov: lens.fov, aspect: lens.aspect };
        if let Some(waited) = self.sora_job.warp {
            let at = self.spot(casts, leader.position, leader.yaw + 180.0, self.rules.warp_behind);
            let seen = cam.sees(out.capsule_center, 90.0) || cam.sees(at + Vec3::Z * 90.0, 90.0);
            if seen && waited < WARP_WAIT {
                self.sora_job.warp = Some(waited + dt);
            } else {
                sora.teleport(kh_core::Pose { position: at + Vec3::Z * kh_core::Sora::origin_height_on_floor(&kh_core::SORA_PARAMS), yaw: leader.yaw });
                self.sora_job = Job::default();
                self.say(format!("party: t={now:.3}s AI Sora is put behind the leader at ({:.0}, {:.0}, {:.0}){}", at.x, at.y, at.z, if seen { " IN VIEW" } else { "" }));
            }
        }
        for (c, id) in self.rt.members.clone() {
            let k = index_of(c);
            let Some(waited) = self.jobs[k].warp else { continue };
            let Some(v) = self.rt.party.view(id) else { continue };
            if v.ko {
                self.jobs[k].warp = None;
                continue;
            }
            let at = self.spot(casts, leader.position, leader.yaw + 180.0, self.rules.warp_behind);
            let r = v.capsule.0.max(v.capsule.1).max(30.0);
            let seen = cam.sees(v.position + Vec3::Z * v.capsule.1, r) || cam.sees(at + Vec3::Z * r, r);
            if seen && waited < WARP_WAIT {
                self.jobs[k].warp = Some(waited + dt);
                continue;
            }
            self.put(id, at, leader.yaw);
            self.jobs[k] = Job::default();
            self.say(format!("party: t={now:.3}s {} is put {:.0} cm behind the leader at ({:.0}, {:.0}, {:.0}){}", c.name(), self.rules.warp_behind, at.x, at.y, at.z, if seen { format!(" IN VIEW (the camera did not look away in {WARP_WAIT:.0} s)") } else { String::new() }));
        }
    }

    /// A friend at these feet (KH3 cm) and yaw, standing, his AI begun afresh.
    fn put(&mut self, id: MemberId, at: Vec3, yaw: f32) {
        self.rt.party.command(id, MemberCommand::Stop);
        if let Some(m) = self.rt.party.member_mut(id) {
            kh_party::ladder::cancel(m); // (LADDER2) a ladder jump in the air ends here too
            let p = &m.params;
            let lift = (p.floor_dist_target - (p.capsule_center_z - p.capsule_half_height)).max(0.0);
            m.cmc.location = at + Vec3::Z * lift;
            m.cmc.stop_movement();
            m.yaw = yaw;
            m.steer = None;
        }
        if let Some(ai) = self.rt.ai_of(id) {
            ai.reset();
        }
    }

    /// The stuck rule, the catch-up after a body action.
    fn fallbacks(&mut self, casts: &Casts, out: &Output, sora: &Sora, dt: f32, now: f64) {
        let leader = self.rt.leader_view_of(out, sora.status());
        let catch_up = match self.catch_up_in.as_mut() {
            Some(t) => {
                *t -= dt;
                if *t <= 0.0 {
                    self.catch_up_in = None;
                    true
                } else {
                    false
                }
            }
            None => false,
        };
        for (c, id) in self.rt.members.clone() {
            if Some(id) == self.rt.leader {
                continue;
            }
            let Some(v) = self.rt.party.view(id) else { continue };
            let k = index_of(c);
            if v.ko || matches!(v.state, NpcStateId::Fr(_) | NpcStateId::Die) {
                self.jobs[k] = Job::default();
                continue;
            }
            // a warp in progress: the clocks wait (a warp that leaves him where he was does not reset them)
            if v.state == NpcStateId::Warp {
                continue;
            }
            // (LADDER2) in the ladder jump: the leap moves him, no clock runs
            if v.state == NpcStateId::Ladder {
                self.jobs[k] = Job::default();
                continue;
            }
            let far = v.position.distance(leader.position);
            if catch_up && (far > self.rules.catchup_far || !casts.clear_now(v.position, leader.position, NAV_CAPSULE)) { // (ER party I) never optimistic
                self.warp(c, "cut off by the body's action: a fog wall or a door", now);
                continue;
            }
            let job = &mut self.jobs[k];
            // he should be on his way: a MoveTo runs and the leader is far; or, whatever his tree does, the leader is twice
            // that far out of battle (a tree that finds no way gives no MoveTo), or five times that far at any time
            let moving = (self.rt.party.move_goal(id).is_some() && far > self.rules.stuck_far) || (far > self.rules.stuck_far * 2.0 && !v.in_battle) || far > self.rules.stuck_far * 5.0;
            match job.anchor {
                Some(a) if moving && a.distance(v.position) < self.rules.stuck_moved => job.still += dt,
                _ => {
                    job.anchor = Some(v.position);
                    job.still = 0.0;
                }
            }
            if job.still > self.rules.stuck_after {
                let t = job.still;
                self.warp(c, &format!("stuck {t:.1} s {far:.0} cm from the leader"), now);
            }
        }
    }

    /// The posers' weapons follow the friends' equipped weapons (S4 draws `PosedChar.weapon`). `read` gives a weapon
    /// model's manifest (the host reads `PACK/party/weapons/<model>/manifest.json`).
    pub fn mount_weapons(&mut self, read: &dyn Fn(&str) -> Option<serde_json::Value>) {
        for (c, id) in self.rt.members.clone() {
            let d = kh_partyhost::poser::draw_id(c);
            let Some(weapon) = self.rt.party.status(id).map(|s| s.weapon.to_string()) else { continue };
            if weapon.is_empty() || self.mounted[d as usize] == weapon {
                continue;
            }
            self.mounted[d as usize] = weapon.clone();
            let Some(m) = self.poser.members.get_mut(&d) else { continue };
            let result = m.weapon_def(&weapon).and_then(|def| read(&def.model).ok_or(format!("no manifest for {}", def.model)).and_then(|man| m.mount_weapon(&def, &man).map(|_| ())));
            let text = match result {
                Ok(()) => format!("party: {} holds {weapon}", c.name()),
                Err(e) => format!("party: {}: weapon {weapon} not mounted ({e})", c.name()),
            };
            self.say(text);
        }
    }
}

#[cfg(test)]
mod tests;
