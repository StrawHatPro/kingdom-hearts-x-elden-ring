//! (ER party S3) THE GO / NO-GO TEST OF THE ALLY BODIES (research ER_PARTY_RESEARCH.md 1.5, the 12 unknowns, and item 13: the user's KO rule), as the
//! USER runs it: `party.body_test = go` (any new value of the key starts it; `report` logs the list now; `bullet` fires
//! one body-owned bullet at the nearest enemy for item 10; `kinds` also tries the routes not needed yet for item 2;
//! `stop` ends it). Declared by `allybody.rs` through `#[path]` (this folder's `mod.rs` is stream S1's).
//!
//! This file is the bookkeeping only (pure, unit-tested): what each item stands at, the report lines, the verdict. The
//! pool (`allybody_live.rs`) feeds it what it sees in the game. Every line goes to kh-er.log with the prefix
//! `party-body-test:` and reads `[ n] PASS | FAIL | WAIT | INFO  <title>: <detail>`; a WAIT line says what the user can
//! do to settle it. The report is logged at `test.report` seconds after the start (10, 30, 60), then every 60 s, and
//! at every change of an item's class.

/// Where one item stands.
#[derive(Clone, Debug, PartialEq)]
pub enum St {
    /// Not settled yet: what would settle it.
    Wait(String),
    Pass(String),
    Fail(String),
    /// Measured, no pass mark.
    Info(String),
}

impl St {
    pub fn word(&self) -> &'static str {
        match self {
            St::Wait(_) => "WAIT",
            St::Pass(_) => "PASS",
            St::Fail(_) => "FAIL",
            St::Info(_) => "INFO",
        }
    }
    pub fn text(&self) -> &str {
        match self {
            St::Wait(t) | St::Pass(t) | St::Fail(t) | St::Info(t) => t,
        }
    }
    fn same_class(&self, other: &St) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// The 12 questions of research 1.5, short, and item 13 (the user's KO rule).
pub const TITLES: [&str; 14] = [
    "the debug creator makes a character (spawn_debug_character, frames until last_created_chr changes)",
    "which character ids load at once (Spirit Ash humanoid / c0000 with ChrInit / copy of a nearby enemy)",
    "the spirit team and the game's can_target(nearest hostile, body)",
    "enemies pick the body as their target, and switch between body and player",
    "hidden + passive, the body is still hit (HP drops, last_hit_by set) and stays where we write it",
    "the body survives (or is remade at once) through a load screen, a map-tile change, an origin shift, a cutscene",
    "the debug set's capacity, and its clean-up at quit-to-title",
    "a throw grabs the body / it fights our position writes",
    "the raised maximum holds (no one-shot kill); what the body's death does",
    "(stage 2) a body-owned bullet hits with an AtkParam_Npc row, and the hate moves to the body",
    "Elden Ring ailments build up on the body",
    "the frame cost with the bodies and the party running",
    // (the user's rule, 2026-10-15: "Enemies should ignore [dead friends] until they recover")
    "a KO'd ally is dropped as a target within N s / re-targetable after revive",
    // (ER party I, the BOSS stream's finding: the AI manager lists a character in its team lists only with ChrCtrl + 0xE8
    // bit 5 on, 0x140507340)
    "each body's AI switch (ChrCtrl + 0xE8 bit 5) is on while it is out, and hostiles near it can target it",
];

/// A world event item 6 watches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ev {
    Load,
    Block,
    Origin,
    Cutscene,
}

impl Ev {
    pub fn name(self) -> &'static str {
        match self {
            Ev::Load => "a load screen",
            Ev::Block => "a map-tile change",
            Ev::Origin => "an origin shift",
            Ev::Cutscene => "a cutscene",
        }
    }
}

/// How a body came through an event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Survived,
    /// Lost and remade after so many seconds.
    Remade(f32),
    /// Lost and not remade within the timeout.
    NotRemade,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Mean {
    sum: f64,
    n: u32,
}

impl Mean {
    fn add(&mut self, v: f32) {
        self.sum += v as f64;
        self.n += 1;
    }
    fn get(&self) -> Option<f32> {
        (self.n > 0).then(|| (self.sum / self.n as f64) as f32)
    }
}

/// Frames in each window of item 12.
pub const COST_FRAMES: u32 = 300;

pub struct Test {
    pub started: f32,
    pub items: [St; 14],
    /// Items whose class changed since the last report.
    changed: bool,
    next_report: usize,
    last_report: f32,
    times: Vec<f32>,
    events: Vec<(Ev, Outcome)>,
    base_dt: Mean,
    with_dt: Mean,
    own_us: Mean,
    /// Tries of the routes (route, result) for item 2.
    pub routes: Vec<(String, Result<f32, String>)>,
}

impl Test {
    pub fn new(now: f32, times: &[f32]) -> Test {
        let wait = |t: &str| St::Wait(t.to_string());
        Test {
            started: now,
            items: [
                wait("the first body is being asked for"),
                wait("no route tried yet"),
                wait("stand within 30 m of an enemy (the can-target check needs a hostile near)"),
                wait("stand near enemies and let them fight: an enemy must attack a body (Donald's or Goofy's stand-in)"),
                wait("let an enemy hit a body"),
                wait("rest at a grace / fast travel (load screen), walk a few hundred metres in the open world (tile change), watch a cutscene; then `party.body_test = report`"),
                wait("the debug set is read with the first body"),
                wait("let a grab / throw attack catch a body (rare: low priority)"),
                wait("the body's HP numbers are read with the first body"),
                wait("stand near an enemy and set `party.body_test = bullet`"),
                wait("let an enemy poison / rot / bleed a body (or stand in a poison swamp)"),
                wait("measuring (5 s without bodies, then 5 s with them)"),
                wait("let Donald or Goofy be knocked out near enemies, wait 10 s, then revive him (Cure, an item, or he gets up)"),
                wait("stand within 30 m of an enemy with a body out (its AI switch is read every 2 s)"),
            ],
            changed: true,
            next_report: 0,
            last_report: now,
            times: times.to_vec(),
            events: Vec::new(),
            base_dt: Mean::default(),
            with_dt: Mean::default(),
            own_us: Mean::default(),
            routes: Vec::new(),
        }
    }

    /// Item `n` (1-based) now stands at `st`. A change of class is reported at the next `due` and returns true (the
    /// caller logs the line at once too).
    pub fn set(&mut self, n: usize, st: St) -> bool {
        let Some(slot) = self.items.get_mut(n.wrapping_sub(1)) else { return false };
        // (a PASS is never taken back by a later WAIT; a FAIL or a better PASS text replaces it)
        if matches!(slot, St::Pass(_)) && matches!(st, St::Wait(_)) {
            return false;
        }
        let class = !slot.same_class(&st);
        *slot = st;
        self.changed |= class;
        class
    }

    pub fn line(&self, n: usize) -> String {
        let st = &self.items[n - 1];
        format!("party-body-test: [{n:2}] {}  {}: {}", st.word(), TITLES[n - 1], st.text())
    }

    /// A route was tried for item 2.
    pub fn route(&mut self, label: String, result: Result<f32, String>) {
        self.routes.push((label, result));
        let ok: Vec<String> = self.routes.iter().filter_map(|(l, r)| r.as_ref().ok().map(|s| format!("{l} in {s:.2} s"))).collect();
        let bad: Vec<String> = self.routes.iter().filter_map(|(l, r)| r.as_ref().err().map(|e| format!("{l}: {e}"))).collect();
        let text = format!("loaded: [{}]; failed: [{}]", ok.join(", "), bad.join(", "));
        let fast = self.routes.iter().any(|(_, r)| r.as_ref().is_ok_and(|s| *s <= 2.0));
        let st = if fast { St::Pass(text) } else if !ok.is_empty() { St::Info(format!("{text} (none within 2 s)")) } else { St::Fail(text) };
        self.set(2, st);
    }

    /// An event item 6 watches.
    pub fn event(&mut self, ev: Ev, outcome: Outcome) {
        self.events.push((ev, outcome));
        self.set(6, self.item6());
    }

    fn item6(&self) -> St {
        let said: Vec<String> = self.events.iter().map(|(e, o)| format!("{}: {}", e.name(), match o { Outcome::Survived => "survived".to_string(), Outcome::Remade(s) => format!("lost, remade after {s:.2} s"), Outcome::NotRemade => "LOST AND NOT REMADE".to_string() })).collect();
        if self.events.iter().any(|(_, o)| *o == Outcome::NotRemade) {
            return St::Fail(said.join("; "));
        }
        let seen = |e: Ev| self.events.iter().any(|(x, _)| *x == e);
        let missing: Vec<&str> = [Ev::Load, Ev::Block, Ev::Cutscene].into_iter().filter(|e| !seen(*e) && !(*e == Ev::Block && seen(Ev::Origin))).map(Ev::name).collect();
        if seen(Ev::Load) && (seen(Ev::Block) || seen(Ev::Origin)) {
            St::Pass(format!("{}{}", said.join("; "), if missing.is_empty() { String::new() } else { format!(" (not seen yet: {})", missing.join(", ")) }))
        } else {
            St::Wait(format!("seen so far: [{}]; still to see: {} (rest at a grace or fast travel, walk a few hundred metres in the open world, watch a cutscene)", said.join("; "), missing.join(", ")))
        }
    }

    /// One frame for item 12: the game's frame time, our own time, how many bodies are live.
    pub fn cost(&mut self, dt: f32, own_us: f32, live: usize, wanted: usize) {
        if live == 0 && self.base_dt.n < COST_FRAMES {
            self.base_dt.add(dt);
        } else if live >= wanted.max(1) && self.with_dt.n < COST_FRAMES {
            self.with_dt.add(dt);
            self.own_us.add(own_us);
        } else {
            return;
        }
        if self.base_dt.n >= COST_FRAMES && self.with_dt.n >= COST_FRAMES {
            let (a, b, us) = (self.base_dt.get().unwrap_or(0.0) * 1000.0, self.with_dt.get().unwrap_or(0.0) * 1000.0, self.own_us.get().unwrap_or(0.0));
            let text = format!("frame {a:.2} ms without bodies, {b:.2} ms with {live} body(ies) (+{:.2} ms); the pool's own work {us:.0} us a frame", b - a);
            self.set(12, if b - a < 1.0 && us < 300.0 { St::Pass(text) } else { St::Fail(format!("{text}: more than 1 ms / 300 us")) });
        }
    }

    /// The report is due (a report time passed, or an item changed class).
    pub fn due(&mut self, now: f32) -> bool {
        let t = now - self.started;
        let timed = match self.times.get(self.next_report) {
            Some(at) if t >= *at => {
                self.next_report += 1;
                true
            }
            Some(_) => false,
            None => now - self.last_report >= 60.0,
        };
        if timed || std::mem::take(&mut self.changed) {
            self.last_report = now;
            return true;
        }
        false
    }

    /// GO when items 1-6 pass, NO-GO when one of them failed, else INCOMPLETE (plan: items 1-6 decide).
    pub fn verdict(&self) -> String {
        let core = &self.items[..6];
        let fails: Vec<String> = (1..=6).filter(|n| matches!(core[n - 1], St::Fail(_))).map(|n| n.to_string()).collect();
        let open: Vec<String> = (1..=6).filter(|n| !matches!(core[n - 1], St::Pass(_) | St::Fail(_))).map(|n| n.to_string()).collect();
        if !fails.is_empty() {
            format!("NO-GO: item(s) {} failed (a friend without a working body cannot be hit; hits on the leader stay the leader's; spawning keeps retrying)", fails.join(", "))
        } else if open.is_empty() {
            "GO: items 1-6 passed (route A, the ally bodies, works in this game)".to_string()
        } else {
            format!("INCOMPLETE: item(s) {} still waiting (see their WAIT lines)", open.join(", "))
        }
    }

    pub fn report(&self, now: f32) -> Vec<String> {
        let mut out = vec![format!("party-body-test: ==== GO / NO-GO of the ally bodies (research 1.5), {:.0} s after the start ====", now - self.started)];
        out.extend((1..=TITLES.len()).map(|n| self.line(n))); // (ER party I) 14 items
        out.push(format!("party-body-test: verdict: {}", self.verdict()));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_go_no_go_incomplete() {
        let mut t = Test::new(0.0, &[10.0]);
        assert!(t.verdict().starts_with("INCOMPLETE"));
        for n in 1..=6 {
            t.set(n, St::Pass("ok".into()));
        }
        assert!(t.verdict().starts_with("GO"));
        t.set(4, St::Fail("no enemy ever attacked".into()));
        assert!(t.verdict().starts_with("NO-GO: item(s) 4"));
        // a pass is not taken back by a wait
        t.set(1, St::Wait("x".into()));
        assert_eq!(t.items[0], St::Pass("ok".into()));
        let lines = t.report(12.0);
        assert_eq!(lines.len(), 16); // (ER party I) 14 items + the head and the verdict
        assert!(lines[1].starts_with("party-body-test: [ 1] PASS  the debug creator"));
        assert!(lines[4].contains("] FAIL  enemies pick"));
    }

    #[test]
    fn reports_at_the_times_and_at_changes() {
        let mut t = Test::new(100.0, &[10.0, 30.0]);
        assert!(t.due(100.0)); // the start
        assert!(!t.due(105.0));
        assert!(t.due(110.5));
        assert!(!t.due(111.0));
        t.set(3, St::Pass("can".into()));
        assert!(t.due(112.0));
        assert!(t.due(131.0));
        assert!(!t.due(150.0));
        assert!(t.due(191.5)); // every 60 s after the list
    }

    #[test]
    fn item_6_needs_a_load_and_a_tile_change_and_fails_on_a_body_not_remade() {
        let mut t = Test::new(0.0, &[]);
        t.event(Ev::Load, Outcome::Remade(0.4));
        assert!(matches!(t.items[5], St::Wait(_)));
        t.event(Ev::Origin, Outcome::Survived);
        assert!(matches!(t.items[5], St::Pass(_)), "{:?}", t.items[5]);
        t.event(Ev::Cutscene, Outcome::NotRemade);
        assert!(matches!(t.items[5], St::Fail(_)));
    }

    #[test]
    fn item_2_and_item_12() {
        let mut t = Test::new(0.0, &[]);
        t.route("buddy c4070".into(), Err("not seen in 5 s".into()));
        assert!(matches!(t.items[1], St::Fail(_)));
        t.route("copy c4311".into(), Ok(0.3));
        assert!(matches!(t.items[1], St::Pass(_)));
        for _ in 0..COST_FRAMES {
            t.cost(1.0 / 60.0, 0.0, 0, 2);
        }
        for _ in 0..COST_FRAMES {
            t.cost(1.0 / 58.0, 120.0, 2, 2);
        }
        assert!(matches!(t.items[11], St::Pass(_)), "{:?}", t.items[11]);
    }
}
