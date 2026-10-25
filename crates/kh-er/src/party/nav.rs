//! (ER party S1) THE FRIENDS' NAVIGATION IN ELDEN RING: the leader's breadcrumb trail, a `kh_ai::Nav` over it and the
//! game's casts, and the waypoint a friend steers by. PORT DESIGN: KH3 has a navmesh (Recast) for its friends; Elden
//! Ring's navmesh is not reachable from here, so the friends follow where the leader walked.
//!
//! - THE TRAIL ([`Trail`]): a point ("crumb") every `spacing` cm (sheet row `nav.spacing`, 50) of the leader's path
//!   while he stands on a floor, the newest last. Where the leader went in a way a friend cannot walk (a jump up onto a
//!   ledge, the ladder jump (LADDER2: the friends jump it with him, ladder.rs), a warp, a load) the trail has a BREAK: no
//!   route crosses it.
//!   The trail is cut (emptied) when the leader is put somewhere else (a load, a warp, the end of a cutscene).
//! - THE NAV ([`TrailNav`], kh-ai's `Nav`): `project` = a line down onto the floor; `path_exists` / `path_length` /
//!   `path` = the straight line when a capsule sweep finds it clear (and there is floor along it), else the route along
//!   the trail. EQS Pathfinding tests and the warp rule's path length use it.
//! - THE WAYPOINT ([`steer`]): when the straight line to a friend's MoveTo goal is blocked, the farthest point of the
//!   trail route he can reach straight (at most 3 ahead).
//!
//! The casts go through the host's `kh_core::World` (Elden Ring's Havok world in the game, synthetic boxes in the
//! tests). A cast budget per step ([`CastBudget`]) bounds what the nav may spend: past it every answer is the
//! optimistic one (clear, the straight length), so a friend never warps because the budget ran out.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};

use glam::Vec3;
use kh_core::World;

/// The nav's numbers (sheet `party-er-host`, rows `nav.*`; these defaults are the same rows).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavRules {
    /// A crumb every this many cm of the leader's path.
    pub spacing: f32,
    /// The trail keeps at most this many crumbs (the oldest go).
    pub max_crumbs: usize,
    /// The straight-line test's sweep is lifted this far above the capsule's own height (steps, small bumps), cm.
    pub lift: f32,
    /// The floor under a point is looked for this far below it, cm.
    pub probe_down: f32,
    /// A straight line longer than this is checked for floor every `floor_every` cm (pits), cm.
    pub floor_every: f32,
    /// Crumbs tested per end of a route (nearest first).
    pub search: usize,
    /// A crumb is reached within this horizontal distance, cm.
    pub reach: f32,
    /// The leader rose more than this in one stretch off the floor: a BREAK (a friend cannot follow a jump up), cm.
    pub jump_up_break: f32,
    /// The leader moved more than this between two frames without a load: a BREAK (a warp), cm.
    pub teleport: f32,
}

impl NavRules {
    pub const DEFAULT: NavRules = NavRules { spacing: 50.0, max_crumbs: 600, lift: 30.0, probe_down: 400.0, floor_every: 400.0, search: 8, reach: 60.0, jump_up_break: 60.0, teleport: 1000.0 };
}

impl Default for NavRules {
    fn default() -> NavRules {
        NavRules::DEFAULT
    }
}

/// One point of the leader's path (KH3 space, the actor origin at his feet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crumb {
    pub at: Vec3,
    /// No route crosses from the crumb before to this one.
    pub break_before: bool,
}

/// THE LEADER'S BREADCRUMB TRAIL (oldest first).
#[derive(Clone, Debug, Default)]
pub struct Trail {
    pub crumbs: VecDeque<Crumb>,
    /// The next crumb gets a break (set by `mark_break`, a jump up).
    pending_break: bool,
    /// Off the floor since: the height he left it at.
    air_from: Option<f32>,
    /// Cuts made (for the log).
    pub cuts: u32,
    /// The leader's last recorded feet (teleport detection).
    last: Option<Vec3>,
    /// The height he last stood at (a jump up is measured from it).
    floor_z: Option<f32>,
}

impl Trail {
    /// The leader's feet this step. `on_floor`: he stands on a floor (only such points become crumbs).
    pub fn record(&mut self, feet: Vec3, on_floor: bool, rules: &NavRules) {
        if let Some(last) = self.last {
            if last.distance(feet) > rules.teleport {
                self.mark_break();
            }
        }
        self.last = Some(feet);
        if !on_floor {
            if self.air_from.is_none() {
                self.air_from = Some(self.floor_z.unwrap_or(feet.z));
            }
            return;
        }
        if let Some(z0) = self.air_from.take() {
            if feet.z > z0 + rules.jump_up_break {
                self.mark_break();
            }
        }
        self.floor_z = Some(feet.z);
        let far = self.crumbs.back().is_none_or(|c| c.at.distance(feet) >= rules.spacing);
        if far || self.pending_break {
            self.push(Crumb { at: feet, break_before: self.pending_break }, rules);
            self.pending_break = false;
        }
    }

    /// The next crumb cannot be walked to from the one before (a jump up, the ladder jump, a warp).
    pub fn mark_break(&mut self) {
        self.pending_break = true;
    }

    /// The KH3 frame was moved under the party (S2's seat counter-move: a cutscene, an action or a grab seats the leader
    /// on the body): every crumb by the same `delta`, so that they stay where they were in the world.
    pub fn shift(&mut self, delta: Vec3) {
        for c in self.crumbs.iter_mut() {
            c.at += delta;
        }
        if let Some(l) = self.last.as_mut() {
            *l += delta;
        }
        if let Some(z) = self.floor_z.as_mut() {
            *z += delta.z;
        }
    }

    /// The leader is somewhere else (a load, a warp, a cutscene's end): the trail is emptied.
    pub fn cut(&mut self) {
        self.crumbs.clear();
        self.pending_break = false;
        self.air_from = None;
        self.last = None;
        self.floor_z = None;
        self.cuts += 1;
    }

    fn push(&mut self, c: Crumb, rules: &NavRules) {
        self.crumbs.push_back(c);
        while self.crumbs.len() > rules.max_crumbs.max(2) {
            self.crumbs.pop_front();
            if let Some(first) = self.crumbs.front_mut() {
                first.break_before = false;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.crumbs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.crumbs.is_empty()
    }


    /// Crumb `i` and `i + 1` are joined (no break between them).
    fn joined(&self, i: usize) -> bool {
        self.crumbs.get(i + 1).is_some_and(|c| !c.break_before)
    }

    /// The crumb indices from `a` to `b` along the trail (either way), None when a break lies between.
    fn walk(&self, a: usize, b: usize) -> Option<Vec<usize>> {
        let mut out = vec![a];
        let mut i = a;
        while i != b {
            let next = if b > i { i + 1 } else { i - 1 };
            if !self.joined(i.min(next)) {
                return None;
            }
            out.push(next);
            i = next;
        }
        Some(out)
    }
}

/// One point of a route.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoutePoint {
    pub at: Vec3,
}

/// What the nav may cast this step (shared by every query of a step: a member's steer, the EQS, the warp rule).
#[derive(Debug, Default)]
pub struct CastBudget {
    pub left: Cell<u32>,
    pub used: Cell<u32>,
    /// Queries answered optimistically because the budget was spent.
    pub starved: Cell<u32>,
}

impl CastBudget {
    pub fn new(casts: u32) -> CastBudget {
        CastBudget { left: Cell::new(casts), used: Cell::new(0), starved: Cell::new(0) }
    }
    fn take(&self, n: u32) -> bool {
        if self.left.get() < n {
            self.starved.set(self.starved.get() + 1);
            return false;
        }
        self.left.set(self.left.get() - n);
        self.used.set(self.used.get() + n);
        true
    }
}

/// The capsule a friend's nav queries sweep with: (radius, half height, centre above the feet), cm.
pub type Capsule = (f32, f32, f32);

/// The bigger of the two friends' capsules (Goofy's: r 40, half height 89, centre 91): the shared nav's.
pub const NAV_CAPSULE: Capsule = (40.0, 89.0, 91.0);

/// The nav's answers kept for a few steps (EQS asks the same neighbours again and again): floors under quantised points,
/// straight lines between quantised ends. Emptied every `NavCache::STEPS` steps and when the trail is cut.
#[derive(Debug, Default)]
pub struct NavCache {
    floor: RefCell<HashMap<(i32, i32, i32), Option<Vec3>>>,
    clear: RefCell<HashMap<(i32, i32, i32, i32, i32, i32), bool>>,
    age: Cell<u32>,
}

impl NavCache {
    /// The answers live this many steps (1/6 s).
    pub const STEPS: u32 = 10;

    /// One step older (emptied when too old).
    pub fn tick(&self) {
        self.age.set(self.age.get() + 1);
        if self.age.get() >= Self::STEPS {
            self.clear_all();
        }
    }

    pub fn clear_all(&self) {
        self.age.set(0);
        self.floor.borrow_mut().clear();
        self.clear.borrow_mut().clear();
    }
}

/// The game's casts as the nav asks them.
pub struct Casts<'a> {
    pub world: &'a dyn World,
    pub rules: NavRules,
    pub budget: &'a CastBudget,
    pub cache: &'a NavCache,
}

fn q(v: f32) -> i32 {
    (v / 25.0).round() as i32
}

impl<'a> Casts<'a> {
    pub fn new(world: &'a dyn World, rules: NavRules, budget: &'a CastBudget, cache: &'a NavCache) -> Casts<'a> {
        Casts { world, rules, budget, cache }
    }

    /// The floor under `p` within `up` above / `down` below (None = no floor). Over budget: `p` itself.
    pub fn floor(&self, p: Vec3, up: f32, down: f32) -> Option<Vec3> {
        let key = (q(p.x), q(p.y), q(p.z + up - down));
        if let Some(hit) = self.cache.floor.borrow().get(&key) {
            return *hit;
        }
        if !self.budget.take(1) {
            return Some(p);
        }
        let hit = self.world.line_trace(p + Vec3::Z * up, p - Vec3::Z * down).filter(|h| h.impact_normal.z > 0.5).map(|h| h.position);
        self.cache.floor.borrow_mut().insert(key, hit);
        hit
    }

    /// A friend walks straight from feet `a` to feet `b`: a line at knee height (`lift` + 15 cm above the feet: steps and
    /// small bumps pass under it) meets nothing, and there is floor along a long line. ONE line, not the capsule: the
    /// game's capsule sweep is several casts (the slim body's spheres, the floor layer), and EQS asks this for every item.
    /// Over budget: true.
    pub fn clear(&self, a: Vec3, b: Vec3, capsule: Capsule) -> bool {
        let key = (q(a.x), q(a.y), q(a.z), q(b.x), q(b.y), q(b.z));
        if let Some(c) = self.cache.clear.borrow().get(&key) {
            return *c;
        }
        if !self.budget.take(1) {
            return true;
        }
        let (_, _, centre) = capsule;
        let up = Vec3::Z * (self.rules.lift + 15.0);
        let mut ok = self.world.line_trace(a + up, b + up).is_none();
        let flat = Vec3::new(b.x - a.x, b.y - a.y, 0.0).length();
        if ok && flat > self.rules.floor_every {
            let n = ((flat / self.rules.floor_every) as usize).min(3);
            for k in 1..=n {
                let m = a.lerp(b, k as f32 / (n + 1) as f32);
                if self.floor(m, centre, self.rules.probe_down).is_none() {
                    ok = false;
                    break;
                }
            }
        }
        self.cache.clear.borrow_mut().insert(key, ok);
        ok
    }

    /// (ER party I) `clear`, NEVER answered optimistically: a one-off question (the catch-up after a body action, once per
    /// friend) gets its own casts (one line, at most three floors) on top of the step's budget, which it does not spend.
    /// Without it the catch-up asked after AIX's EQS (the FollowPlayer side contexts) had spent the step's budget, got
    /// "clear" for a friend behind the fog wall, and left him there.
    pub fn clear_now(&self, a: Vec3, b: Vec3, capsule: Capsule) -> bool {
        let before = self.budget.left.get();
        self.budget.left.set(before + 4);
        let ok = self.clear(a, b, capsule);
        self.budget.left.set(self.budget.left.get().min(before));
        ok
    }

    /// The route from `from` to `to` along the trail: `from`, the trail's points, `to`. None = no crumb
    /// either end sees, or a break lies between.
    pub fn route(&self, trail: &Trail, from: Vec3, to: Vec3, capsule: Capsule) -> Option<Vec<RoutePoint>> {
        if trail.is_empty() {
            return None;
        }
        let nearest = |p: Vec3| {
            let mut v: Vec<(f32, usize)> = trail.crumbs.iter().enumerate().map(|(i, c)| (c.at.distance_squared(p), i)).collect();
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            v.into_iter().take(self.rules.search.max(1)).map(|x| x.1).collect::<Vec<_>>()
        };
        let seen = |p: Vec3, i: usize, outward: bool| {
            let c = trail.crumbs[i].at;
            c.distance(p) <= self.rules.reach || if outward { self.clear(p, c, capsule) } else { self.clear(c, p, capsule) }
        };
        let exits: Vec<usize> = nearest(to).into_iter().filter(|&i| seen(to, i, false)).collect();
        if exits.is_empty() {
            return None;
        }
        // every entry he sees (nearest first) with the exit reachable along the trail from it: the shortest route wins
        let mut best: Option<(f32, Vec<RoutePoint>)> = None;
        for a in nearest(from) {
            if !seen(from, a, true) {
                continue;
            }
            for &b in &exits {
                if let Some(idx) = trail.walk(a, b) {
                    let mut out = vec![RoutePoint { at: from }];
                    out.extend(idx.iter().map(|&i| RoutePoint { at: trail.crumbs[i].at }));
                    out.push(RoutePoint { at: to });
                    let length = route_length(&out);
                    if best.as_ref().is_none_or(|b| length < b.0) {
                        best = Some((length, out));
                    }
                    break;
                }
            }
        }
        best.map(|b| b.1)
    }
}

/// The length of a route (cm).
pub fn route_length(r: &[RoutePoint]) -> f32 {
    r.windows(2).map(|w| w[0].at.distance(w[1].at)).sum()
}

/// What a friend heads for this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Steer {
    /// The straight line to his goal is clear (or nothing is known: no trail): no waypoint.
    Straight,
    /// Head for this point of the trail first.
    Via(Vec3),
    /// Blocked and no route: the stuck rule decides.
    Lost,
}

/// THE WAYPOINT of a friend at feet `from` whose MoveTo goal is `goal`.
pub fn steer(c: &Casts, trail: &Trail, from: Vec3, goal: Vec3, capsule: Capsule) -> Steer {
    if c.clear(from, goal, capsule) {
        return Steer::Straight;
    }
    let Some(route) = c.route(trail, from, goal, capsule) else { return Steer::Lost };
    pick(c, &route, from, capsule)
}

fn pick(c: &Casts, route: &[RoutePoint], from: Vec3, capsule: Capsule) -> Steer {
    // route[0] = from; the points after it, up to three ahead (string pulling)
    let mut pick = None;
    for (k, p) in route.iter().enumerate().skip(1).take(3) {
        if k == 1 {
            pick = Some(p.at);
            continue;
        }
        if c.clear(from, p.at, capsule) {
            pick = Some(p.at);
        } else {
            break;
        }
    }
    match pick {
        // a crumb he is already on: the next one
        Some(p) if Vec3::new(p.x - from.x, p.y - from.y, 0.0).length() <= c.rules.reach * 0.5 => route.get(2).map_or(Steer::Straight, |n| Steer::Via(n.at)),
        Some(p) => Steer::Via(p),
        None => Steer::Lost,
    }
}

/// THE NAV the AI's EQS and warp rule see (kh-ai `Nav`).
pub struct TrailNav<'a> {
    pub casts: &'a Casts<'a>,
    pub trail: &'a Trail,
    pub capsule: Capsule,
}

impl kh_ai::Nav for TrailNav<'_> {
    fn project(&self, p: Vec3, up: f32, down: f32) -> Option<Vec3> {
        self.casts.floor(p, up, down)
    }
    fn path_exists(&self, from: Vec3, to: Vec3) -> bool {
        self.casts.clear(from, to, self.capsule) || self.casts.route(self.trail, from, to, self.capsule).is_some()
    }
    fn path_length(&self, from: Vec3, to: Vec3) -> Option<f32> {
        if self.casts.clear(from, to, self.capsule) {
            return Some(from.distance(to));
        }
        self.casts.route(self.trail, from, to, self.capsule).map(|r| route_length(&r))
    }
    fn trace_blocked(&self, from: Vec3, to: Vec3) -> bool {
        if !self.casts.budget.take(1) {
            return false;
        }
        self.casts.world.line_trace(from, to).is_some()
    }
    /// (FOL2) the follow spots' winner: the straight walk by casts of its own (`clear_now`, never optimistic)
    /// (a spot far above or below his feet is no walk: the projection put it on a wall's top, a ledge)
    fn reachable_strict(&self, from: Vec3, to: Vec3) -> Option<bool> {
        let flat = Vec3::new(to.x - from.x, to.y - from.y, 0.0).length();
        if (to.z - from.z).abs() > 40.0 + 0.5 * flat {
            return Some(false);
        }
        Some(self.casts.clear_now(from, to, self.capsule))
    }
    /// (FOL2) the hallway probes: one line of the game's world (outside the step's budget: 14 lines a quarter second)
    fn ray(&self, from: Vec3, to: Vec3) -> Option<f32> {
        self.casts.world.line_trace(from, to).map(|h| (h.position - from).length())
    }
    fn path(&self, from: Vec3, to: Vec3, out: &mut Vec<Vec3>) -> bool {
        if self.casts.clear(from, to, self.capsule) {
            out.push(from);
            out.push(to);
            return true;
        }
        match self.casts.route(self.trail, from, to, self.capsule) {
            Some(r) => {
                out.extend(r.iter().map(|p| p.at));
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::test_world::TestWorld;

    /// A floor at 0 and a wall across x = 500 from y = -1000 to y = 300 (a corner to walk round at y > 300).
    fn corner() -> TestWorld {
        TestWorld::floor().with_box(Vec3::new(480.0, -1000.0, 0.0), Vec3::new(520.0, 300.0, 400.0))
    }

    fn walk(trail: &mut Trail, from: Vec3, to: Vec3, rules: &NavRules) {
        let n = (from.distance(to) / 10.0).ceil() as usize;
        for k in 0..=n {
            trail.record(from.lerp(to, k as f32 / n.max(1) as f32), true, rules);
        }
    }

    #[test]
    fn the_trail_drops_a_crumb_every_spacing() {
        let rules = NavRules::DEFAULT;
        let mut t = Trail::default();
        walk(&mut t, Vec3::ZERO, Vec3::new(1000.0, 0.0, 0.0), &rules);
        assert!((17..=21).contains(&t.len()), "about one a 50 cm: {}", t.len());
        // off the floor: nothing; a landing 1 m higher (a jump up) is a break
        t.record(Vec3::new(1000.0, 0.0, 100.0), false, &rules);
        t.record(Vec3::new(1060.0, 0.0, 100.0), true, &rules);
        assert!(t.crumbs.back().unwrap().break_before);
        t.cut();
        assert!(t.is_empty() && t.cuts == 1);
    }

    #[test]
    fn the_route_goes_round_the_corner() {
        let world = corner();
        let rules = NavRules::DEFAULT;
        let budget = CastBudget::new(10_000);
        let cache = NavCache::default();
        let casts = Casts::new(&world, rules, &budget, &cache);
        // the leader walked from (0, 0) up to (0, 600), round the wall's end and down to (1000, 0)
        let mut t = Trail::default();
        walk(&mut t, Vec3::ZERO, Vec3::new(0.0, 600.0, 0.0), &rules);
        walk(&mut t, Vec3::new(0.0, 600.0, 0.0), Vec3::new(1000.0, 600.0, 0.0), &rules);
        walk(&mut t, Vec3::new(1000.0, 600.0, 0.0), Vec3::new(1000.0, 0.0, 0.0), &rules);
        let (from, to) = (Vec3::new(100.0, 0.0, 0.0), Vec3::new(1000.0, 0.0, 0.0));
        assert!(!casts.clear(from, to, NAV_CAPSULE), "the wall is in the way");
        let route = casts.route(&t, from, to, NAV_CAPSULE).expect("a route along the trail");
        assert!(route.iter().any(|p| p.at.y >= 550.0), "it passes the wall's end: {route:?}");
        let length = route_length(&route);
        assert!(length > 1800.0 && length < 2600.0, "{length}");
        let nav = TrailNav { casts: &casts, trail: &t, capsule: NAV_CAPSULE };
        assert!(kh_ai::Nav::path_exists(&nav, from, to));
        assert!(kh_ai::Nav::path_length(&nav, from, to).unwrap() > 1500.0);
        assert_eq!(kh_ai::Nav::project(&nav, Vec3::new(10.0, 10.0, 50.0), 100.0, 200.0).map(|p| p.z.round()), Some(0.0));
        // the waypoint: a point of the trail he can reach straight, on his side of the wall
        match steer(&casts, &t, from, to, NAV_CAPSULE) {
            Steer::Via(p) => assert!(p.x < 480.0 && casts.clear(from, p, NAV_CAPSULE), "{p}"),
            other => panic!("{other:?}"),
        }
        // in the open: straight
        assert_eq!(steer(&casts, &t, Vec3::new(1000.0, 400.0, 0.0), to, NAV_CAPSULE), Steer::Straight);
    }

    /// (LADDER2) The leader's ladder jump up onto a ledge is a break of the trail (a jump up): no route crosses it.
    #[test]
    fn a_jump_up_onto_a_ledge_breaks_the_route() {
        let rules = NavRules::DEFAULT;
        // a ledge 400 high beyond x = 600
        let world = TestWorld::floor().with_box(Vec3::new(600.0, -1000.0, 0.0), Vec3::new(2000.0, 1000.0, 400.0));
        let budget = CastBudget::new(10_000);
        let cache = NavCache::default();
        let casts = Casts::new(&world, rules, &budget, &cache);
        let mut t = Trail::default();
        walk(&mut t, Vec3::ZERO, Vec3::new(540.0, 0.0, 0.0), &rules);
        // the leap: off the floor, up, landed 400 higher
        for z in [100.0, 300.0, 480.0] {
            t.record(Vec3::new(560.0, 0.0, z), false, &rules);
        }
        walk(&mut t, Vec3::new(660.0, 0.0, 400.0), Vec3::new(1200.0, 0.0, 400.0), &rules);
        let k = t.crumbs.iter().position(|c| c.at.z > 300.0).unwrap();
        assert!(t.crumbs[k].break_before, "the landing starts a new stretch");
        let goal = Vec3::new(1200.0, 0.0, 400.0);
        assert_eq!(steer(&casts, &t, Vec3::new(480.0, 0.0, 0.0), goal, NAV_CAPSULE), Steer::Lost);
    }

    #[test]
    fn over_budget_every_answer_is_optimistic() {
        let world = corner();
        let budget = CastBudget::new(0);
        let cache = NavCache::default();
        let casts = Casts::new(&world, NavRules::DEFAULT, &budget, &cache);
        assert!(casts.clear(Vec3::new(100.0, 0.0, 0.0), Vec3::new(1000.0, 0.0, 0.0), NAV_CAPSULE));
        assert!(budget.starved.get() > 0 && budget.used.get() == 0);
    }

    /// (ER party I) the catch-up's question is answered by a cast even with the step's budget spent, and spends none of it.
    #[test]
    fn erpi_clear_now_is_never_optimistic() {
        let world = corner();
        let budget = CastBudget::new(0);
        let cache = NavCache::default();
        let casts = Casts::new(&world, NavRules::DEFAULT, &budget, &cache);
        assert!(!casts.clear_now(Vec3::new(100.0, 0.0, 0.0), Vec3::new(1000.0, 0.0, 0.0), NAV_CAPSULE));
        assert_eq!(budget.left.get(), 0);
    }
}
