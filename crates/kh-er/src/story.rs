//! (IMPL_ER33, user decision 2026-10-13: "Remove it completely. Make the cutscenes happen automatically when you unlock
//! the correct amount of save points") THE STORY EVENTS ELDEN RING TIES TO RESTING AT A SITE OF GRACE, without Sora ever
//! attuning or sitting: the conditions (a table, sheet `sheets/port/sora-er-story.json`), the once-only and deferral
//! rules, and the black cover. Pure except `cover`; savepoint.rs applies it. State note: build/sora_tmp/er/IMPL_ER33_STATE.md.
//!
//! WHAT THE GAME DOES (PARSED from its own files, copies under build/sora_tmp/er/er33/: `event/common.emevd` events
//! 950 / 960 / 945 / 3043..3046, `script/talk/m00_00_00_00.talkesdbnd` t000001000 (the grace) and t000003000 (Melina)):
//! - the grace's talk script sets event flag 9000 while the player sits; event 950 (Melina's accord) waits for
//!   9000 AND the player within 2 m of one of eight graces AND (its counter of those graces discovered, or one of two
//!   of them), then plays cutscene 6042000N; everything after the scene (her offer, Accept / Refuse, the whistle) is HER
//!   TALK SCRIPT, which runs only inside the game's rest. The invitation to the Roundtable Hold and every "Talk to
//!   Melina" are talk-script states of the grace menu as well.
//! - So nothing of it can be started by a flag alone with confidence (method A): the scene would play, her talk would
//!   not. METHOD B is what is delivered: at the moment a row's condition is met the game's REAL rest is made for the
//!   body under a black cover, with Sora not taking the body's pose (he never attunes, never sits), and never on Save.
//!
//! (IMPL_ER36, the user after the first session: "the game forces you into the grace menu when you reach a checkpoint
//! with a blakc screen upon touching the grace. Fix this. The Talk to Melina option in the save point menu should just
//! talk to Melina and thats it, not open grace menu") WHAT THAT SESSION'S LOG SHOWED AND WHAT CHANGED:
//! - only THE ACCORD is made by itself now (kind `auto`): what follows its rest is a cutscene and her talk, never the
//!   game's grace menu (MEASURED: the rest ended by itself after her talk). The invitation to the Roundtable Hold and the
//!   talk after Morgott are kind `self` ("she speaks by herself at a rest", by the talk script): in the user's session
//!   she did NOT speak in five rests although every flag the script names held (NOT EXPLAINED), so the row fired at
//!   every new save point and left the player in the game's grace menu. A `self` row is never made unasked: it only
//!   puts "Talk to Melina" into the save-point menu, once per session.
//! - "Talk to Melina" is DRIVEN TO THE END (talkdrive.rs): cover, the rest, her talk selected in the game's menu for
//!   the player, cover off while she speaks, cover on and the rest left when the game's menu comes back. The game's
//!   grace menu is never shown and never left to the player.
//!
//! | key | default | meaning |
//! |---|---|---|
//! | `story.auto` | 1 | 0 = no rest is ever made by itself (the menu entry and `savepoint.rest_events` remain) |
//! | `story.talk` | 1 | the save-point menu's "Talk to Melina" entry while a row offers it; 0 = never |
//! | `story.cover` | 1 | 0 = no black cover over the rest (testing) |
//! | `story.log` | 0 | 1 = every row's verdict once a second (else only changes) |

use std::collections::BTreeMap;

/// What a row is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The rest is made by itself the moment the row is ready at a save point.
    Auto,
    /// The save-point menu shows "Talk to Melina" while the row is ready: an ENTRY OF THE GAME'S GRACE MENU exists then
    /// (the talk script's group x57), which talkdrive.rs selects.
    Menu,
    /// (IMPL_ER36, sheet value `self`) By the talk script Melina SPEAKS BY HERSELF when the player rests while the row
    /// holds (no menu entry of the game's): never made unasked; the save-point menu offers "Talk to Melina" for it until
    /// one driven rest in the session brought no talk of hers.
    Own,
}

/// One row of the table: an event of the game's that only its rest starts, and when the game allows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub id: String,
    pub kind: Kind,
    /// Any of these flags on = the event is over: never again.
    pub done_any: Vec<u32>,
    pub need_all: Vec<u32>,
    pub need_none: Vec<u32>,
    /// Empty = nothing asked.
    pub need_any: Vec<u32>,
    /// One of these flags was seen turning on in this session (empty = nothing asked): for events without a flag of
    /// their own that marks them done.
    pub rise_any: Vec<u32>,
    /// The save points (grace entity ids) it can happen at; empty = any.
    pub graces: Vec<u32>,
    /// ... and those of them where the count is not asked.
    pub graces_free: Vec<u32>,
    /// THE COUNT: this many of these flags (the discovery flags of the counted graces) must be on.
    pub count_flags: Vec<u32>,
    pub count_min: u32,
    /// (FX5) The body must carry NONE of these special effects (the talk script's `f61(id) == 0` tests: t000001000 x60 state 0 /
    /// t000003000 x72 state 0 for the Roundtable invitation; the CV3 run: 4270 on the body, "NOTHING OF MELINA'S CAME").
    pub no_effects: Vec<i32>,
}

/// What a row says now.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    Done(String),
    Ready(String),
    NotYet(String),
    /// A flag could not be read: nothing is done.
    Unknown(String),
}

impl Verdict {
    pub fn ready(&self) -> bool {
        matches!(self, Verdict::Ready(_))
    }
    pub fn text(&self) -> String {
        match self {
            Verdict::Done(t) => format!("DONE ({t})"),
            Verdict::Ready(t) => format!("READY ({t})"),
            Verdict::NotYet(t) => format!("not yet ({t})"),
            Verdict::Unknown(t) => format!("unknown ({t})"),
        }
    }
}

fn list(value: &serde_json::Value, key: &str) -> Vec<u32> {
    value.get(key).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|n| n.as_u64()).filter(|n| *n > 0 && *n <= u32::MAX as u64).map(|n| n as u32).collect()).unwrap_or_default()
}

/// THE TABLE from the sheet's text (rows `[id, value, ...]`, id `story.<name>`); the second value: rows not understood.
pub fn parse(text: &str) -> Result<(Vec<Rule>, Vec<String>), String> {
    let json: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let rows = json.get("rows").and_then(|r| r.as_array()).ok_or("no rows")?;
    let (mut rules, mut problems) = (Vec::new(), Vec::new());
    for row in rows {
        let (Some(id), Some(value)) = (row.get(0).and_then(|v| v.as_str()), row.get(1)) else { continue };
        if !id.starts_with("story.") || value.is_null() {
            continue;
        }
        let kind = match value.get("kind").and_then(|k| k.as_str()) {
            Some("auto") => Kind::Auto,
            Some("menu") => Kind::Menu,
            Some("self") => Kind::Own,
            _ => {
                problems.push(id.to_string());
                continue;
            }
        };
        let rule = Rule {
            id: id.to_string(),
            kind,
            done_any: list(value, "done_any"),
            need_all: list(value, "need_all"),
            need_none: list(value, "need_none"),
            need_any: list(value, "need_any"),
            rise_any: list(value, "rise_any"),
            graces: list(value, "graces"),
            graces_free: list(value, "graces_free"),
            count_flags: list(value, "count_flags"),
            count_min: value.get("count_min").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            no_effects: list(value, "no_effects").into_iter().map(|n| n as i32).collect(), // (FX5)
        };
        // (a row that asks nothing at all would rest him at every save point: refused)
        let asks = !rule.need_all.is_empty() || !rule.need_any.is_empty() || !rule.rise_any.is_empty() || (rule.count_min > 0 && !rule.count_flags.is_empty()) || !rule.graces.is_empty();
        if !asks || (rule.kind == Kind::Auto && rule.done_any.is_empty() && rule.rise_any.is_empty()) {
            problems.push(format!("{id} (asks nothing, or an automatic row without a flag that ends it)"));
            continue;
        }
        rules.push(rule);
    }
    Ok((rules, problems))
}

/// The sheet as it was when this was built: the defaults.
pub const SHEET: &str = include_str!("../../../sheets/port/sora-er-story.json");

pub fn default_rules() -> Vec<Rule> {
    parse(SHEET).map(|(rules, _)| rules).unwrap_or_default()
}

/// IS THE ROW READY? `grace`: the save point he stands at (`None`: nowhere, the row is judged without its place);
/// `flag`: the game's event flag (`None` = not readable); `risen`: flags seen turning on in this session.
pub fn evaluate(rule: &Rule, grace: Option<u32>, flag: &dyn Fn(u32) -> Option<bool>, risen: &[u32]) -> Verdict {
    let mut unknown = None;
    let mut read = |f: u32| -> bool {
        match flag(f) {
            Some(v) => v,
            None => {
                unknown = Some(f);
                false
            }
        }
    };
    if let Some(f) = rule.done_any.iter().copied().find(|f| read(*f)) {
        return Verdict::Done(format!("flag {f} is on"));
    }
    let mut not_yet = None;
    if let Some(f) = rule.need_all.iter().copied().find(|f| !read(*f)) {
        not_yet = Some(format!("flag {f} is off"));
    }
    if let Some(f) = rule.need_none.iter().copied().find(|f| read(*f)) {
        not_yet.get_or_insert(format!("flag {f} is on"));
    }
    if !rule.need_any.is_empty() && !rule.need_any.iter().copied().any(&mut read) {
        not_yet.get_or_insert(format!("none of the flags {:?} is on", rule.need_any));
    }
    if !rule.rise_any.is_empty() && !rule.rise_any.iter().any(|f| risen.contains(f)) {
        not_yet.get_or_insert(format!("none of the flags {:?} turned on in this session", rule.rise_any));
    }
    let counted = rule.count_flags.iter().copied().filter(|f| read(*f)).count() as u32;
    let count_ok = rule.count_min == 0 || rule.count_flags.is_empty() || counted >= rule.count_min;
    let place = match grace {
        Some(g) if !rule.graces.is_empty() && !rule.graces.contains(&g) => Some(format!("save point {g} is not one of its {} save points", rule.graces.len())),
        Some(g) if !count_ok && !rule.graces_free.contains(&g) => Some(format!("{counted} of {} counted save points unlocked, {} needed (or one of {:?})", rule.count_flags.len(), rule.count_min, rule.graces_free)),
        // (nowhere: ready when the count is, or when a save point without the count exists)
        None if !count_ok && rule.graces_free.is_empty() => Some(format!("{counted} of {} counted save points unlocked, {} needed", rule.count_flags.len(), rule.count_min)),
        _ => None,
    };
    if let Some(f) = unknown {
        return Verdict::Unknown(format!("flag {f} cannot be read"));
    }
    if let Some(why) = not_yet.or(place) {
        return Verdict::NotYet(why);
    }
    Verdict::Ready(match (grace, rule.count_min > 0 && !rule.count_flags.is_empty()) {
        (Some(g), true) if !count_ok => format!("save point {g} needs no count"),
        (_, true) if count_ok => format!("{counted} of {} counted save points unlocked (needed {})", rule.count_flags.len(), rule.count_min),
        (None, true) => format!("at one of {:?} (no count there); else {counted} of {} needed", rule.graces_free, rule.count_min),
        _ => "its flags hold".to_string(),
    })
}

/// Why a rest cannot be made this frame (the first that holds), or `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Blocked {
    /// A cutscene plays; a menu of the game's or ours is up; a command of the save-point menu is being carried out.
    pub cutscene: bool,
    pub menu: bool,
    pub busy: bool,
    /// The game offers no rest here now (its prompt was not seen on this visit, or is not up: enemies alerted, a boss).
    pub no_rest: bool,
    pub in_air: bool,
    pub dead: bool,
}

impl Blocked {
    pub fn why(&self) -> Option<&'static str> {
        if self.dead {
            Some("he is dead")
        } else if self.cutscene {
            Some("a cutscene plays")
        } else if self.menu {
            Some("a menu is up")
        } else if self.busy {
            Some("a command of the save-point menu is being carried out")
        } else if self.in_air {
            Some("he is in the air")
        } else if self.no_rest {
            Some("the game offers no rest here now (combat: enemies alerted nearby or a boss fight; or its prompt is not up yet)")
        } else {
            None
        }
    }
}

/// What a frame looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Ctx {
    /// The save point he stands in (its Save-command capsule), discovered and presented by the game: its grace's entity.
    pub at: Option<u32>,
    pub blocked: Blocked,
    /// `story.auto`.
    pub auto: bool,
}

/// THE RULES OF WHEN, pure. A row's rest is made
/// - at a save point HE UNLOCKED IN THIS SESSION, or, once per session and row, at the first save point he stands in
///   with the row ready (the character that is already past the count);
/// - never twice at the same save point for the same row in a session (a rest that brought nothing is not repeated
///   there; the menu entry stays);
/// - not while blocked: then as soon as the block is gone, while he still stands there, and at every later save point.
#[derive(Clone, Debug, Default)]
pub struct Story {
    pub rules: Vec<Rule>,
    /// (row, save point) pairs a rest was asked for in this session.
    tried: Vec<(usize, u32)>,
    /// Rows whose once-per-session chance at an old save point is used.
    session_used: Vec<usize>,
    /// Save points unlocked by touch in this session.
    unlocked: Vec<u32>,
    /// Flags of `rise_any` rows as last read, and those seen turning on.
    last: BTreeMap<u32, bool>,
    risen: Vec<u32>,
    /// For the log: each row's last verdict text (judged without a place), each refusal last told.
    told: BTreeMap<usize, String>,
    refused: Option<(usize, u32, &'static str)>,
    /// (IMPL_ER36) Rows of kind `self` whose driven rest brought no talk of Melina's in this session: not offered again.
    own_tried: Vec<usize>,
    /// (RC, the user 2026-10-20: "Melina cutscenes should play only when you press activate on the save point, not just by
    /// touching it") `story.on_y` (savepoint.rs, default 1): an automatic row is made only after Y AT THE SAVE POINT asked
    /// for it (`ask`), as the game's own event waits for the player resting at the grace (flag 9000, event 950).
    on_y: bool,
    /// (RC) The save point whose Y asked for the due row (cleared when he leaves it or the rest is made).
    asked: Option<u32>,
    /// (RC) (row, save point) last told to be waiting for Y (said once).
    told_wait: Option<(usize, u32)>,
}

/// (IMPL_ER36) What the menu's "Talk to Melina" stands for at a save point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    /// The row that makes the entry (for the log).
    pub row: String,
    /// A row of kind `self`: she is expected to speak by herself once the body rests.
    pub own: Option<usize>,
    /// The game's grace menu holds her entry: it is selected when she does not speak by herself.
    pub entry: bool,
    /// An automatic row's scene is expected (its rest was tried here before and brought nothing).
    pub scene: bool,
}

/// What a frame decided.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Decision {
    /// Make the rest now: (row, save point).
    pub rest: Option<(usize, u32)>,
    pub log: Vec<String>,
    /// (SV) A row is READY at the save point he stands in and only the game's prompt is missing (gracelock.rs keeps the
    /// game's grace actions switched off): the host lets the game's rest action be offered for this moment.
    pub waits_for_prompt: bool,
}

impl Story {
    pub fn new(rules: Vec<Rule>) -> Story {
        Story { rules, ..Story::default() }
    }

    /// A save point was unlocked by touch (its discovery flag written and read back).
    pub fn unlocked(&mut self, grace: u32) {
        if !self.unlocked.contains(&grace) && self.unlocked.len() < 4096 {
            self.unlocked.push(grace);
        }
    }

    /// (RC) `story.on_y`: the automatic rows wait for Y at the save point (true) or come by themselves (false, IMPL_ER36).
    pub fn set_on_y(&mut self, on: bool) {
        self.on_y = on;
    }

    /// (RC) Y AT THE SAVE POINT `grace` asked for its due automatic row (`due_on_y`): the next frames make its rest there
    /// (deferred as before while something blocks it).
    pub fn ask(&mut self, grace: u32) {
        self.asked = Some(grace);
    }

    /// (RC) The save point Y asked a story event at, still waiting for its rest.
    pub fn asked(&self) -> Option<u32> {
        self.asked
    }

    /// (RC) THE AUTOMATIC ROW Y AT THIS SAVE POINT WOULD START (ready here, not yet tried here, the session's chance at an
    /// old save point not used): savepoint.rs asks it at the press, before its menu opens. The game's own trigger is the
    /// player resting at the grace, so Y (our "rest") with Melina first.
    pub fn due_on_y(&self, grace: u32, flag: &dyn Fn(u32) -> Option<bool>) -> Option<usize> {
        (0..self.rules.len()).find(|k| self.auto_due(*k, grace, flag))
    }

    fn auto_due(&self, k: usize, grace: u32, flag: &dyn Fn(u32) -> Option<bool>) -> bool {
        self.rules[k].kind == Kind::Auto
            && !self.tried.contains(&(k, grace))
            && evaluate(&self.rules[k], Some(grace), flag, &self.risen).ready()
            && (self.unlocked.contains(&grace) || !self.session_used.contains(&k))
    }

    /// A death or a load: the game is back at its last Save, so what this session tried is forgotten (the flags say
    /// what really happened).
    pub fn reset(&mut self) {
        self.tried.clear();
        self.session_used.clear();
        self.unlocked.clear();
        self.refused = None;
        self.own_tried.clear();
        self.told.clear();
        (self.asked, self.told_wait) = (None, None); // (RC)
        // (a flag that is on after the load did not "turn on in this session"; what rose stays risen: the boss is dead)
    }

    /// ONE FRAME. `flag`: the game's event flags.
    pub fn frame(&mut self, ctx: Ctx, flag: &dyn Fn(u32) -> Option<bool>, verbose: bool) -> Decision {
        let mut d = Decision::default();
        // ---- flags turning on (for the rows that have no flag of their own that ends them)
        for f in self.rules.iter().flat_map(|r| r.rise_any.iter().copied()).collect::<Vec<_>>() {
            let Some(now) = flag(f) else { continue };
            if let Some(false) = self.last.insert(f, now) {
                if now && !self.risen.contains(&f) {
                    self.risen.push(f);
                    d.log.push(format!("flag {f} TURNED ON in this session"));
                }
            }
        }
        // ---- every row judged without a place: said when it changes
        for (k, rule) in self.rules.iter().enumerate() {
            let text = evaluate(rule, None, flag, &self.risen).text();
            if verbose || self.told.get(&k) != Some(&text) {
                // (IMPL_ER36) a menu row's flags are set and cleared by map events as he walks past a grace (MEASURED: flag
                // 1042372701 flipped 22 times in a session): said only with `story.log = 1`
                if rule.kind != Kind::Menu || verbose {
                    d.log.push(format!("{} ({}): {text}{}", rule.id, match rule.kind { Kind::Auto => "automatic", Kind::Menu => "menu entry", Kind::Own => "she speaks by herself at a rest: offered in the menu, never made unasked" }, if rule.kind == Kind::Auto && text.starts_with("READY") { if rule.graces.is_empty() { ": it happens at the next save point he stands in" } else { ": it happens at the next of ITS save points he stands in" } } else { "" }));
                }
                self.told.insert(k, text);
            }
        }
        // ---- the automatic rows at the save point he stands in
        let Some(grace) = ctx.at else {
            self.refused = None;
            self.asked = None; // (RC) he left the save point Y asked at
            return d;
        };
        if self.asked.is_some_and(|a| a != grace) {
            self.asked = None; // (RC)
        }
        if !ctx.auto {
            return d;
        }
        for k in 0..self.rules.len() {
            if self.rules[k].kind != Kind::Auto || self.tried.contains(&(k, grace)) {
                continue;
            }
            let verdict = evaluate(&self.rules[k], Some(grace), flag, &self.risen);
            if !verdict.ready() {
                continue;
            }
            let fresh = self.unlocked.contains(&grace);
            if !fresh && self.session_used.contains(&k) {
                continue;
            }
            // (RC) only after Y at this save point asked for it (`story.on_y`): touching it is not enough
            if self.on_y && self.asked != Some(grace) {
                if self.told_wait != Some((k, grace)) {
                    self.told_wait = Some((k, grace));
                    d.log.push(format!("{} at save point {grace}: {}: (RC) IT WAITS FOR Y AT THE SAVE POINT (story.on_y = 1: Melina's events start only when the player activates it)", self.rules[k].id, verdict.text()));
                }
                return d;
            }
            if let Some(why) = ctx.blocked.why() {
                d.waits_for_prompt = ctx.blocked.no_rest && Blocked { no_rest: false, ..ctx.blocked }.why().is_none(); // (SV)
                if self.refused != Some((k, grace, why)) {
                    self.refused = Some((k, grace, why));
                    d.log.push(format!("{} at save point {grace}: {} BUT NOT NOW: {why}; it follows as soon as that is over while he stands here, else at the next save point", self.rules[k].id, verdict.text()));
                }
                return d;
            }
            self.refused = None;
            self.tried.push((k, grace));
            if !self.session_used.contains(&k) {
                self.session_used.push(k);
            }
            d.log.push(format!("{} at save point {grace}: {}: TRIGGERED ({}): the game's real rest is made for the body under the black cover; Sora neither attunes nor sits", self.rules[k].id, verdict.text(), if fresh { "this save point was unlocked in this session" } else { "the character is already past the condition: the once-a-session chance at an old save point" }));
            d.rest = Some((k, grace));
            self.asked = None; // (RC)
            return d;
        }
        d
    }

    /// (IMPL_ER36) DOES THE MENU SHOW "Talk to Melina" at this save point, and what is the driven rest to expect?
    /// `entry_gate`: the game's grace menu can hold her entry here at all (talkdrive::menu_gate). In this order:
    /// a row of kind `self` that is ready and was not tried in this session (she is expected to speak by herself; a
    /// menu row that is ready as well is the fallback inside the same rest), a menu row that is ready (the entry is
    /// selected), an automatic row that is ready and whose rest was already tried here (its scene is expected).
    pub fn talk_offer(&self, grace: u32, flag: &dyn Fn(u32) -> Option<bool>, entry_gate: bool) -> Option<Offer> {
        self.talk_offer_lifting(grace, flag, entry_gate, &|_| false, false).map(|(o, _)| o)
    }

    /// (FX5) `talk_offer` with THE WHOLE CONDITION of a `self` row: the body's special effects the talk script tests
    /// (`Rule::no_effects`, `has`: the body carries this effect now). A row whose effects block is NOT offered (the game
    /// would not have her speak: the CV3 run's "NOTHING OF MELINA'S CAME" with 4270 on the body), unless `lift`
    /// (`story.lift_blockers` 1; (FINAL5) default 0, savepoint.rs): then it is offered and the second value lists the effects the driven rest
    /// takes off the body before it rests and gives back after (savepoint.rs).
    pub fn talk_offer_lifting(&self, grace: u32, flag: &dyn Fn(u32) -> Option<bool>, entry_gate: bool, has: &dyn Fn(i32) -> bool, lift: bool) -> Option<(Offer, Vec<i32>)> {
        let ready = |r: &Rule| evaluate(r, Some(grace), flag, &self.risen).ready();
        let entry = self.rules.iter().find(|r| r.kind == Kind::Menu && entry_gate && ready(r)).map(|r| r.id.clone());
        let own = self.rules.iter().enumerate().find(|(k, r)| r.kind == Kind::Own && !self.own_tried.contains(k) && ready(r) && (lift || blockers(r, has).is_empty()));
        if let Some((k, r)) = own {
            return Some((Offer { row: r.id.clone(), own: Some(k), entry: entry.is_some(), scene: false }, blockers(r, has)));
        }
        self.talk_offer_rest(grace, flag, entry).map(|o| (o, Vec::new()))
    }

    /// (FX5) The `self` rows that are ready here but held back by an effect on the body (for the log): (row, effects).
    pub fn held_by_effects(&self, grace: u32, flag: &dyn Fn(u32) -> Option<bool>, has: &dyn Fn(i32) -> bool) -> Vec<(String, Vec<i32>)> {
        self.rules
            .iter()
            .enumerate()
            .filter(|(k, r)| r.kind == Kind::Own && !self.own_tried.contains(k) && evaluate(r, Some(grace), flag, &self.risen).ready())
            .map(|(_, r)| (r.id.clone(), blockers(r, has)))
            .filter(|(_, b)| !b.is_empty())
            .collect()
    }

    fn talk_offer_rest(&self, grace: u32, flag: &dyn Fn(u32) -> Option<bool>, entry: Option<String>) -> Option<Offer> {
        let ready = |r: &Rule| evaluate(r, Some(grace), flag, &self.risen).ready();
        if let Some(row) = entry {
            return Some(Offer { row, own: None, entry: true, scene: false });
        }
        self.rules.iter().enumerate().find(|(k, r)| r.kind == Kind::Auto && self.tried.contains(&(*k, grace)) && ready(r)).map(|(_, r)| Offer { row: r.id.clone(), own: None, entry: false, scene: true })
    }

    /// (IMPL_ER36) A driven rest for a row of kind `self` ended without a talk of Melina's: the row is not offered
    /// again in this session (the game did not have her speak; its own flags decide when she will).
    pub fn own_tried(&mut self, k: usize) {
        if !self.own_tried.contains(&k) {
            self.own_tried.push(k);
        }
    }

    pub fn rule_id(&self, k: usize) -> &str {
        self.rules.get(k).map_or("?", |r| r.id.as_str())
    }
}

/// (FX5) The special effects of `rule.no_effects` the body carries now (`has`): what keeps the game from having her
/// speak by herself.
pub fn blockers(rule: &Rule, has: &dyn Fn(i32) -> bool) -> Vec<i32> {
    rule.no_effects.iter().copied().filter(|e| has(*e)).collect()
}

/// (FINAL5) `story.lift_blockers`: DEFAULT 0 (absent = off). Taking the game's special effects off the body for the driven
/// rest and giving them back never ran in game, so by default a blocked `self` row is offered only when the game's own talk
/// condition holds (faithful to Elden Ring). `story.lift_blockers = 1` turns FX5's lifting on to try it.
pub fn lift_blockers(control: &crate::control::Control) -> bool {
    control.flag("story.lift_blockers") == Some(true)
}

/// The body's animations of attuning to and resting at a site of grace (MEASURED IMPL_ER17: 26011 / 68000 touch;
/// 26031, 68010..68012, 68050, 68211, 68212 rest): Sora does not take the body's pose during them unless
/// `savepoint.sit = 1` (sora.rs asks).
pub fn grace_animation(anim: i32) -> bool {
    matches!(anim, 26011 | 26031) || (68000..=68299).contains(&anim)
}

// ---------------------------------------------------------------- the black cover (the overlay's plate)

static COVER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// THE BLACK OVER THE WHOLE PICTURE (the overlay's plate, as the game-over scene's: over every layer, independent of
/// the HUD's own fading, and it lapses by itself 300 ms after the last call: nothing can leave the screen covered).
/// Called EVERY FRAME while wanted; `false` takes it off at once.
pub fn cover(on: bool) {
    use std::sync::atomic::Ordering::Relaxed;
    if on {
        kh_er_render::hud::set_plate(Some((kh_er_render::hud::MAX_LAYERS, [0.0, 0.0, 0.0, 1.0], 300)));
        COVER.store(true, Relaxed);
    } else if COVER.swap(false, Relaxed) {
        kh_er_render::hud::set_plate(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GATEFRONT: u32 = 1042371950;
    const ELLEH: u32 = 1042361950;
    const STORMHILL: u32 = 1041381950;

    fn flags(on: &[u32]) -> impl Fn(u32) -> Option<bool> + '_ {
        move |f| Some(on.contains(&f))
    }

    fn rule<'a>(rules: &'a [Rule], id: &str) -> &'a Rule {
        rules.iter().find(|r| r.id == id).unwrap_or_else(|| panic!("no row {id}"))
    }

    #[test]
    fn the_sheet_is_read_and_holds_the_games_own_conditions() {
        let (rules, problems) = parse(SHEET).expect("the sheet parses");
        assert!(problems.is_empty(), "{problems:?}");
        let accord = rule(&rules, "story.accord");
        // (PARSED: common.emevd event 950: eight graces, two of them without the count; event 960's eight flags)
        assert_eq!((accord.kind, accord.done_any.as_slice(), accord.graces.len(), accord.count_flags.len(), accord.count_min), (Kind::Auto, &[951][..], 8, 8, 3));
        assert_eq!(accord.graces_free, [1043371950, GATEFRONT]);
        assert!(accord.graces.contains(&ELLEH) && accord.count_flags.contains(&76100) && accord.count_flags.contains(&76111));
        let invite = rule(&rules, "story.roundtable");
        assert_eq!((invite.kind, invite.done_any.contains(&105), invite.need_any.contains(&10000851), invite.need_none.contains(&10009655)), (Kind::Own, true, true, true));
        assert!(invite.graces.is_empty());
        // (IMPL_ER36) ONLY THE ACCORD is made by itself: what follows its rest is a scene, never the game's grace menu
        assert_eq!(rules.iter().filter(|r| r.kind == Kind::Auto).map(|r| r.id.as_str()).collect::<Vec<_>>(), ["story.accord"]);
        assert_eq!(rule(&rules, "story.morgott").kind, Kind::Own);
        assert!(rules.iter().filter(|r| r.kind == Kind::Menu).count() >= 20);
        assert!(rules.iter().all(|r| r.kind == Kind::Menu || !r.done_any.is_empty() || !r.rise_any.is_empty()));
        // a row that asks nothing is turned down
        let (none, bad) = parse(r#"{"rows":[["story.x",{"kind":"auto","done_any":[5]}],["story.y",{"kind":"what"}],["other",1]]}"#).unwrap();
        assert!(none.is_empty());
        assert_eq!(bad.len(), 2);
    }

    #[test]
    fn the_accord_needs_three_counted_save_points_or_one_of_the_two() {
        let rules = default_rules();
        let accord = rule(&rules, "story.accord");
        // a new character: the First Step (76101, not counted) and the Church of Elleh (76100)
        let on = [76101, 76100];
        assert!(!evaluate(accord, Some(ELLEH), &flags(&on), &[]).ready());
        // Gatefront needs no count (the user's own log: the scene came there with one counted grace before it)
        assert!(evaluate(accord, Some(GATEFRONT), &flags(&[76101, 76100, 76111]), &[]).ready());
        // three of the eight, at one of the eight
        let three = [76100, 76108, 76104];
        assert!(evaluate(accord, Some(STORMHILL), &flags(&three), &[]).ready());
        assert!(evaluate(accord, Some(ELLEH), &flags(&three), &[]).ready());
        // ... but not at a save point that is none of the eight (the First Step, a cave)
        assert!(!evaluate(accord, Some(1042361951), &flags(&three), &[]).ready());
        assert!(!evaluate(accord, Some(31001950), &flags(&three), &[]).ready());
        // done: never again
        assert_eq!(evaluate(accord, Some(GATEFRONT), &flags(&[951, 76111]), &[]), Verdict::Done("flag 951 is on".to_string()));
        // judged nowhere (the start of a session): ready because Gatefront would do
        assert!(evaluate(accord, None, &flags(&[]), &[]).ready());
        // an unreadable flag: nothing
        assert!(matches!(evaluate(accord, Some(GATEFRONT), &|_| None, &[]), Verdict::Unknown(_)));
    }

    #[test]
    fn the_invitation_waits_for_the_accord_and_for_margit_or_a_grace_outside_limgrave() {
        let rules = default_rules();
        let invite = rule(&rules, "story.roundtable");
        assert!(!evaluate(invite, Some(ELLEH), &flags(&[951, 4680]), &[]).ready());
        assert!(!evaluate(invite, Some(ELLEH), &flags(&[10000851]), &[]).ready(), "no accord yet");
        assert!(evaluate(invite, Some(10001952), &flags(&[951, 4680, 10000851]), &[]).ready(), "Margit was met: at any save point, Stormveil's too");
        assert!(evaluate(invite, Some(ELLEH), &flags(&[951, 4680, 3062]), &[]).ready(), "a grace of Liurnia is discovered");
        // she was heard and told to wait: the menu entry takes over
        assert!(!evaluate(invite, Some(ELLEH), &flags(&[951, 4680, 10000851, 10009655]), &[]).ready());
        let wait = rule(&rules, "story.talk.roundtable");
        assert!(evaluate(wait, Some(ELLEH), &flags(&[4680, 10009655]), &[]).ready());
        assert!(!evaluate(wait, Some(ELLEH), &flags(&[4680, 10009655, 105]), &[]).ready());
        // the Roundtable Hold was reached: over
        assert!(matches!(evaluate(invite, Some(ELLEH), &flags(&[4680, 10000851, 105]), &[]), Verdict::Done(_)));
    }

    fn ctx(at: Option<u32>) -> Ctx {
        Ctx { at, blocked: Blocked::default(), auto: true }
    }

    #[test]
    fn a_row_triggers_once_when_the_save_point_that_fulfils_it_is_unlocked() {
        let mut story = Story::new(default_rules());
        let k = story.rules.iter().position(|r| r.id == "story.accord").unwrap();
        // two counted save points: nothing, wherever he stands
        let two = [76100, 76108];
        assert_eq!(story.frame(ctx(Some(ELLEH)), &flags(&two), false).rest, None);
        // the third is unlocked by touch: the very frame he stands in it
        story.unlocked(STORMHILL);
        let three = [76100, 76108, 76102];
        let d = story.frame(ctx(Some(STORMHILL)), &flags(&three), false);
        assert_eq!(d.rest, Some((k, STORMHILL)));
        assert!(d.log.iter().any(|l| l.contains("TRIGGERED")), "{:?}", d.log);
        // never again there, however long he stands (the scene did not come, or he refused her: the flag is not on)
        assert!((0..600).all(|_| story.frame(ctx(Some(STORMHILL)), &flags(&three), false).rest.is_none()));
        // not at an old save point either (the once-a-session chance is used) ...
        assert_eq!(story.frame(ctx(Some(ELLEH)), &flags(&three), false).rest, None);
        // ... but at the next one he unlocks
        story.unlocked(GATEFRONT);
        assert_eq!(story.frame(ctx(Some(GATEFRONT)), &flags(&[76100, 76108, 76102, 76111]), false).rest, Some((k, GATEFRONT)));
        // done: nothing more, ever
        story.unlocked(1044351950);
        assert_eq!(story.frame(ctx(Some(1044351950)), &flags(&[951, 76100, 76108, 76102, 76111]), false).rest, None);
    }

    #[test]
    fn a_character_already_past_the_count_gets_it_at_the_first_save_point_of_the_session_once() {
        let mut story = Story::new(default_rules());
        let k = story.rules.iter().position(|r| r.id == "story.accord").unwrap();
        let past = [76100, 76108, 76102, 76111];
        // the session starts away from any save point: said, nothing done
        let d = story.frame(ctx(None), &flags(&past), false);
        assert_eq!(d.rest, None);
        assert!(d.log.iter().any(|l| l.contains("story.accord") && l.contains("READY")), "{:?}", d.log);
        // (said once, not every frame)
        assert!(story.frame(ctx(None), &flags(&past), false).log.is_empty());
        // a save point that is not one of the eight: nothing
        assert_eq!(story.frame(ctx(Some(1042361951)), &flags(&past), false).rest, None);
        // the first of the eight he stands in, although it was discovered long ago
        assert_eq!(story.frame(ctx(Some(ELLEH)), &flags(&past), false).rest, Some((k, ELLEH)));
        assert_eq!(story.frame(ctx(Some(GATEFRONT)), &flags(&past), false).rest, None, "once a session at old save points");
        // a death / load: the session's tries are forgotten
        story.reset();
        assert_eq!(story.frame(ctx(Some(GATEFRONT)), &flags(&past), false).rest, Some((k, GATEFRONT)));
    }

    #[test]
    fn a_blocked_trigger_is_deferred_said_once_and_follows_when_the_block_is_gone() {
        let mut story = Story::new(default_rules());
        let on = [76100, 76111];
        story.unlocked(GATEFRONT);
        let fight = Ctx { at: Some(GATEFRONT), blocked: Blocked { no_rest: true, ..Blocked::default() }, auto: true };
        let first = story.frame(fight, &flags(&on), false);
        assert_eq!(first.rest, None);
        assert_eq!(first.log.iter().filter(|l| l.contains("BUT NOT NOW") && l.contains("combat")).count(), 1, "{:?}", first.log);
        // (in combat for ten seconds: no rest, no second line)
        assert!((0..600).all(|_| { let d = story.frame(fight, &flags(&on), false); d.rest.is_none() && d.log.is_empty() }));
        // a cutscene of another kind, a menu: each reason said once
        let cut = Ctx { blocked: Blocked { cutscene: true, ..Blocked::default() }, ..fight };
        assert!(story.frame(cut, &flags(&on), false).log.iter().any(|l| l.contains("a cutscene plays")));
        // he leaves and comes back with the enemies gone: now
        assert_eq!(story.frame(ctx(None), &flags(&on), false).rest, None);
        assert!(story.frame(ctx(Some(GATEFRONT)), &flags(&on), false).rest.is_some());
        // story.auto = 0: never by itself
        let mut off = Story::new(default_rules());
        off.unlocked(GATEFRONT);
        assert!(off.frame(Ctx { at: Some(GATEFRONT), blocked: Blocked::default(), auto: false }, &flags(&on), false).rest.is_none());
    }

    /// (IMPL_ER36) THE USER'S SESSION: after the accord the invitation's row held at every new save point of Stormveil and
    /// each got a hidden rest that ended in the game's grace menu. A row of kind `self` never makes a rest by itself.
    #[test]
    fn a_talk_row_is_never_made_unasked_and_is_offered_once_a_session() {
        let mut story = Story::new(default_rules());
        let k = story.rules.iter().position(|r| r.id == "story.roundtable").unwrap();
        let on = [951, 953, 4680, 10000851, 71002, 71001];
        // unlocked this session, standing in it, nothing in the way: NO rest, at none of them, however long
        for grace in [10001952u32, 10001951, 10001958, 10001954] {
            story.unlocked(grace);
            assert!((0..120).all(|_| story.frame(ctx(Some(grace)), &flags(&on), false).rest.is_none()), "{grace}");
        }
        // the menu offers it: she is expected to speak by herself; no entry of the game's menu stands behind it here
        assert_eq!(story.talk_offer(10001952, &flags(&on), true), Some(Offer { row: "story.roundtable".to_string(), own: Some(k), entry: false, scene: false }));
        // ... with a talk of the game's own menu ready as well (flag 1042372701: set near that grace), that is the fallback
        let with_entry = [951, 953, 4680, 10000851, 1042372701];
        assert_eq!(story.talk_offer(10001952, &flags(&with_entry), true), Some(Offer { row: "story.roundtable".to_string(), own: Some(k), entry: true, scene: false }));
        // the driven rest brought no talk of hers: not offered again in this session, at no save point
        story.own_tried(k);
        assert_eq!(story.talk_offer(10001952, &flags(&on), true), None);
        assert_eq!(story.talk_offer(10001954, &flags(&on), true), None);
        // (the menu row is still offered by itself)
        assert_eq!(story.talk_offer(10001952, &flags(&with_entry), true).map(|o| (o.row, o.own, o.entry)), Some(("story.talk.limgrave_1".to_string(), None, true)));
        // a death / load: a new session's one offer
        story.reset();
        assert!(story.talk_offer(10001952, &flags(&on), true).is_some());
        // she was heard (10009655) and told to wait: the game's own entry takes over; reached the Hold: nothing
        assert_eq!(story.talk_offer(10001952, &flags(&[951, 953, 4680, 10000851, 10009655]), true).map(|o| o.row), Some("story.talk.roundtable".to_string()));
        assert_eq!(story.talk_offer(10001952, &flags(&[951, 953, 4680, 10000851, 10009655, 105]), true), None);
    }

    #[test]
    fn the_talk_after_morgott_is_offered_only_in_the_session_he_falls() {
        let mut story = Story::new(default_rules());
        let k = story.rules.iter().position(|r| r.id == "story.morgott").expect("the row");
        let morgott = story.rules[k].rise_any[0];
        // loaded with the boss long dead: nothing (no flag marks the talk as heard: a dead entry every session otherwise)
        assert!((0..10).all(|_| story.frame(ctx(Some(ELLEH)), &flags(&[4680, 953, morgott]), false).rest.is_none()));
        assert_eq!(story.talk_offer(ELLEH, &flags(&[4680, 953, morgott]), true), None);
        // seen dying in this session: offered (never made unasked)
        let mut live = Story::new(default_rules());
        assert!(live.frame(ctx(None), &flags(&[4680, 953]), false).rest.is_none());
        assert!(live.frame(ctx(None), &flags(&[4680, 953, morgott]), false).log.iter().any(|l| l.contains("TURNED ON")));
        live.unlocked(11001950);
        assert!(live.frame(ctx(Some(11001950)), &flags(&[4680, 953, morgott]), false).rest.is_none());
        assert_eq!(live.talk_offer(11001950, &flags(&[4680, 953, morgott]), true).and_then(|o| o.own), Some(k));
    }

    #[test]
    fn the_menu_entry_shows_only_while_the_game_offers_a_talk() {
        let mut story = Story::new(default_rules());
        // an ordinary save point, nothing pending
        assert_eq!(story.talk_offer(ELLEH, &flags(&[951, 953, 4680]), true), None);
        // the Forge of the Giants (PARSED: flag 1054532702 = the talk that burns the Erdtree is offered)
        let forge = story.talk_offer(1054531950, &flags(&[953, 4680, 1054532702]), true).expect("the entry");
        assert_eq!((forge.row.as_str(), forge.own, forge.entry, forge.scene), ("story.talk.forge_burn", None, true, false));
        // ... but not where the game's menu cannot hold her entry (the gate: talkdrive::menu_gate said no)
        assert_eq!(story.talk_offer(1054531950, &flags(&[953, 4680, 1054532702]), false), None);
        // the accord was refused: she can be asked again (the one entry of hers before it is accepted)
        assert_eq!(story.talk_offer(ELLEH, &flags(&[951, 953]), true).map(|o| o.row), Some("story.talk.accord_again".to_string()));
        // an automatic row whose rest was tried here and brought nothing (the game did not rest): its scene can be asked for
        let on = [76100, 76111];
        assert_eq!(story.talk_offer(GATEFRONT, &flags(&on), true), None, "before the automatic try: no entry");
        story.unlocked(GATEFRONT);
        assert!(story.frame(ctx(Some(GATEFRONT)), &flags(&on), false).rest.is_some());
        assert_eq!(story.talk_offer(GATEFRONT, &flags(&on), true), Some(Offer { row: "story.accord".to_string(), own: None, entry: false, scene: true }));
        assert_eq!(story.talk_offer(ELLEH, &flags(&on), true), None, "only where it was tried");
        // (the menu rows' flags flip as he walks: nothing of them is logged without story.log)
        let mut quiet = Story::new(default_rules());
        quiet.frame(ctx(None), &flags(&[953, 4680]), false);
        assert!(quiet.frame(ctx(None), &flags(&[953, 4680, 1042372701]), false).log.is_empty());
        assert!(quiet.frame(ctx(None), &flags(&[953, 4680]), false).log.is_empty());
        assert!(quiet.frame(ctx(None), &flags(&[953, 4680, 1042372701]), true).log.iter().any(|l| l.contains("story.talk.limgrave_1")));
    }

    #[test]
    fn sora_never_takes_the_bodys_grace_poses() {
        for anim in [26011, 68000, 26031, 68010, 68011, 68012, 68050, 68211, 68212] {
            assert!(grace_animation(anim), "{anim}");
        }
        // the other own actions stay his (remains, the double door)
        assert!(!grace_animation(60071) && !grace_animation(60030) && !grace_animation(0) && !grace_animation(63000));
    }

    /// (FX5) THE CV3 RUN: every flag of the Roundtable invitation held, the body carried special effect 4270, the menu offered
    /// "Talk to Melina" and the driven rest brought "NOTHING OF MELINA'S CAME". The talk script tests six effects of the body;
    /// the offer now holds the whole condition: blocked = not offered, or (story.lift_blockers) offered with the effects to
    /// take off for the rest. Without any of them nothing changes.
    #[test]
    fn fx5_melinas_invitation_is_offered_only_as_the_talk_script_would_have_her_speak() {
        let story = Story::new(default_rules());
        let k = story.rules.iter().position(|r| r.id == "story.roundtable").unwrap();
        assert_eq!(story.rules[k].no_effects, vec![4270, 4271, 4272, 4280, 4282, 4286], "the sheet holds the script's six tests");
        // the CV3 flags at the grace menu (er_r3_run1b 50.858 s): 951 953 4680 10000851 on, 10009655 off
        let on = [951, 953, 4680, 10000851];
        let cv3_body = |id: i32| id == 4270;
        let clean = |_: i32| false;
        // a clean body: offered as before, nothing to take off
        assert_eq!(story.talk_offer_lifting(GATEFRONT, &flags(&on), true, &clean, false), Some((Offer { row: "story.roundtable".to_string(), own: Some(k), entry: false, scene: false }, vec![])));
        assert_eq!(story.talk_offer(GATEFRONT, &flags(&on), true).map(|o| o.row), Some("story.roundtable".to_string()));
        // 4270 on the body, lifting off: NOT offered (the game would not have her speak), and said why
        assert_eq!(story.talk_offer_lifting(GATEFRONT, &flags(&on), true, &cv3_body, false), None);
        assert_eq!(story.held_by_effects(GATEFRONT, &flags(&on), &cv3_body), vec![("story.roundtable".to_string(), vec![4270])]);
        // ... lifting on (`story.lift_blockers 1`; (FINAL5) not the default): offered, the rest takes 4270 off
        assert_eq!(story.talk_offer_lifting(GATEFRONT, &flags(&on), true, &cv3_body, true).map(|(o, l)| (o.row, l)), Some(("story.roundtable".to_string(), vec![4270])));
        // a game menu entry of hers still stands by itself when the invitation is blocked (lifting off)
        let with_entry = [951, 953, 4680, 10000851, 1042372701];
        assert_eq!(story.talk_offer_lifting(10001952, &flags(&with_entry), true, &cv3_body, false).map(|(o, l)| (o.row, o.own, l)), Some(("story.talk.limgrave_1".to_string(), None, vec![])));
        // the after-Morgott row tests no effect: an effect never holds it back
        let m = story.rules.iter().position(|r| r.id == "story.morgott").unwrap();
        assert!(story.rules[m].no_effects.is_empty());
    }

    /// (FINAL5) the coordinator's decision: lifting is OFF unless the ctl says `story.lift_blockers = 1` (the effects' removal and
    /// re-application never ran in game); the key stays so 1 can be tried.
    #[test]
    fn final5_lifting_the_blockers_is_off_by_default() {
        let dir = std::env::temp_dir().join(format!("kh-er-final5-lift-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let absent = crate::control::Control::new(dir.join("no-such.ctl"));
        assert!(!lift_blockers(&absent), "absent key = off");
        for (text, want) in [("story.lift_blockers = 1
", true), ("story.lift_blockers=on
", true), ("story.lift_blockers = 0
", false), ("# story.lift_blockers = 1
", false)] {
            let path = dir.join("t.ctl");
            std::fs::write(&path, text).unwrap();
            let mut c = crate::control::Control::new(path.clone());
            c.poll();
            assert_eq!(lift_blockers(&c), want, "{text:?}");
            let _ = std::fs::remove_file(&path);
        }
        let _ = std::fs::remove_dir(&dir);
    }

    /// (RC, the user 2026-10-20: "Melina cutscenes should play only when you press activate on the save point, not just by
    /// touching it") With `story.on_y` the accord waits for Y at the save point; Y there starts it; once.
    #[test]
    fn rc_the_accord_waits_for_y_at_the_save_point() {
        let mut story = Story::new(default_rules());
        story.set_on_y(true);
        let k = story.rules.iter().position(|r| r.id == "story.accord").unwrap();
        let past = [76100, 76108, 76102, 76111];
        // standing in the save point (touching it): nothing, said once
        let d = story.frame(ctx(Some(ELLEH)), &flags(&past), false);
        assert_eq!(d.rest, None);
        assert!(d.log.iter().any(|l| l.contains("WAITS FOR Y")), "{:?}", d.log);
        assert!((0..600).all(|_| { let d = story.frame(ctx(Some(ELLEH)), &flags(&past), false); d.rest.is_none() && d.log.is_empty() && !d.waits_for_prompt }));
        // Y there: it is due, and its rest is made at once
        assert_eq!(story.due_on_y(ELLEH, &flags(&past)), Some(k));
        story.ask(ELLEH);
        assert_eq!(story.frame(ctx(Some(ELLEH)), &flags(&past), false).rest, Some((k, ELLEH)));
        assert_eq!(story.asked(), None);
        assert_eq!(story.due_on_y(ELLEH, &flags(&past)), None, "tried here: not again");
        // asked at one save point, then standing at another: forgotten there
        story.reset();
        story.ask(ELLEH);
        assert_eq!(story.frame(ctx(Some(GATEFRONT)), &flags(&past), false).rest, None);
        assert_eq!(story.asked(), None);
        // a blocked press is deferred as before (the game's rest not offered yet: the grace actions are let through, SV)
        story.ask(GATEFRONT);
        let fight = Ctx { at: Some(GATEFRONT), blocked: Blocked { no_rest: true, ..Blocked::default() }, auto: true };
        let d = story.frame(fight, &flags(&past), false);
        assert!(d.rest.is_none() && d.waits_for_prompt);
        assert_eq!(story.frame(ctx(Some(GATEFRONT)), &flags(&past), false).rest, Some((k, GATEFRONT)));
        // story.on_y = 0: by itself on entry, as before
        let mut old = Story::new(default_rules());
        assert_eq!(old.frame(ctx(Some(ELLEH)), &flags(&past), false).rest, Some((k, ELLEH)));
    }
}
