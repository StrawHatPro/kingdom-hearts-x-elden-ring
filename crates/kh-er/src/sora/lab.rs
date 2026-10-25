//! THE TEST BENCH FOR ENEMY ANIMATIONS (IMPL_ER6_STATE.md): one-shot keys of the control file that
//! measure what Elden Ring's characters do. Nothing here runs without its key; EVERYTHING HERE
//! SWITCHES ITSELF OFF FOR GOOD THE MOMENT A REAL PAD IS TOUCHED (the user took the instance over:
//! a survey in progress stops, held bodies are given back, the global calm flags are put back).
//!
//! - `dbg.calm = 1` (level key): the game's own debug flags "no enemy attacks" and "no enemy
//!   moves" for every enemy, so that subjects stand still. Put back when the key goes.
//! - `dbg.fire = who,W_EventName`: a Havok behaviour event on that character (chranim.rs).
//! - `dbg.anim = who,id`: the event module's "play this animation id next frame" (W_Event<id>).
//! - `dbg.speed = who,x[,seconds]`: its animation speed held at x (default 3 s).
//! - `dbg.hold = who,height[,seconds[,ai bits]]`: the body held in the air (puppet.rs `hold`).
//! - `dbg.names = text`: the game's behaviour event names containing the text.
//! - `dbg.animq = who`: its animation queue logged at every change for 8 s.
//! - `dbg.survey = who,first,last[,air[,poise[,pin[,flat only]]]]` (pin = 1: held on the spot, no root motion): THE dmgLevel SURVEY: for each level one
//!   weapon-type and one flat-attack bullet at the character, 4.5 s apart; each shot is announced
//!   (`survey: FIRE ...`, the screenshot script waits for that line) and summed up (`survey:
//!   RESULT ...`: the animations it played with their lengths, how far it moved, HP and poise).
//!   `air` = 1: the body is held 2.2 m up for the whole run.
//! - `dbg.stats = n`: the bullet route's landed / missed score by character id.

use eldenring::cs::{FieldInsHandle, WorldChrManDbgFlags};
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use super::SoraHost;
use crate::chranim::{self, AnimNow};
use crate::control::{Control, OneShot};
use crate::erlog;
use crate::targets;

pub(super) struct Lab {
    calm: OneShotLevel,
    fire: OneShot,
    anim: OneShot,
    speed: OneShot,
    hold: OneShot,
    names: OneShot,
    animq: OneShot,
    survey: OneShot,
    stats: OneShot,
    /// A real pad was touched: the bench is closed for this session.
    closed: bool,
    calm_saved: Option<(bool, bool)>,
    watch: Option<Watch>,
    speed_hold: Option<(FieldInsHandle, f32, f32)>,
    run: Option<Survey>,
}

/// A level key: on while the file says 1.
struct OneShotLevel;

struct Watch {
    handle: FieldInsHandle,
    what: String,
    seconds: f32,
    limit: f32,
    last: AnimNow,
    queue: bool,
    last_queue: String,
    from: Vec3,
    seen: Vec<(i32, f32, f32)>,
    far: f32,
    high: f32,
}

struct Survey {
    handle: FieldInsHandle,
    npc_id: i32,
    first: u8,
    last: u8,
    air: bool,
    flat_only: bool,
    poise: f32,
    level: u8,
    flat: bool,
    /// Seconds into the present trial; negative = settling before the first.
    t: f32,
    fired: bool,
    hp: i32,
    poise_before: f32,
    from: Vec3,
    seen: Vec<(i32, f32, f32)>,
    last_anim: i32,
    far: f32,
    high: f32,
}

const TRIAL: f32 = 4.5;
const FIRE_AT: f32 = 0.5;

impl Lab {
    pub(super) fn new(control: &Control) -> Lab {
        Lab { calm: OneShotLevel, fire: OneShot::new("dbg.fire", control), anim: OneShot::new("dbg.anim", control), speed: OneShot::new("dbg.speed", control), hold: OneShot::new("dbg.hold", control), names: OneShot::new("dbg.names", control), animq: OneShot::new("dbg.animq", control), survey: OneShot::new("dbg.survey", control), stats: OneShot::new("dbg.stats", control), closed: false, calm_saved: None, watch: None, speed_hold: None, run: None }
    }
}

fn dbg_flags() -> Option<&'static mut WorldChrManDbgFlags> {
    // SAFETY: game thread; the SDK resolves the static by its pinned RVA (the exe-version gate holds).
    unsafe { WorldChrManDbgFlags::instance_mut() }.ok()
}

impl SoraHost {
    /// A real pad was touched: the bench closes and puts back everything it holds.
    pub(super) fn lab_close(&mut self, why: &str) {
        if self.lab.closed {
            return;
        }
        self.lab.closed = true;
        self.lab_restore();
        erlog!("lab: CLOSED ({why}): test keys do nothing for the rest of this session");
    }

    /// Everything the bench holds goes back (also when Sora stops).
    pub(super) fn lab_restore(&mut self) {
        if let Some((attack, moves)) = self.lab.calm_saved.take() {
            if let Some(flags) = dbg_flags() {
                flags.all_no_attack = attack;
                flags.all_no_move = moves;
                erlog!("lab: the game's enemy attack / move flags put back ({attack}, {moves})");
            }
        }
        if let Some((handle, _, _)) = self.lab.speed_hold.take() {
            if let Some(chr) = targets::find_chr(&handle) {
                chranim::set_speed(chr, 1.0);
            }
        }
        if self.lab.run.take().is_some() {
            erlog!("survey: ABORTED");
        }
        self.lab.watch = None;
        self.targets.puppets.release_held();
    }

    pub(super) fn lab_step(&mut self, control: &Control, dt: f32) {
        let _ = &self.lab.calm;
        if self.lab.closed {
            self.targets.puppets.hold_tick(dt); // (ZETTA) held bodies (Zettaflare's hold) are written every frame, the lab closed or not
            return;
        }
        // ---- dbg.calm
        let calm = control.on("dbg.calm");
        match (calm, self.lab.calm_saved) {
            (true, None) => {
                if let Some(flags) = dbg_flags() {
                    self.lab.calm_saved = Some((flags.all_no_attack, flags.all_no_move));
                    erlog!("lab: every enemy is kept from attacking and moving (the game's debug flags; they were {}, {})", flags.all_no_attack, flags.all_no_move);
                }
            }
            (true, Some(_)) => {
                if let Some(flags) = dbg_flags() {
                    flags.all_no_attack = true;
                    flags.all_no_move = true;
                }
            }
            (false, Some((attack, moves))) => {
                if let Some(flags) = dbg_flags() {
                    flags.all_no_attack = attack;
                    flags.all_no_move = moves;
                    erlog!("lab: the game's enemy attack / move flags put back ({attack}, {moves})");
                }
                self.lab.calm_saved = None;
            }
            (false, None) => {}
        }
        if let Some(text) = self.lab.stats.fired(control) {
            erlog!("lab: bullets ({text}): {}", self.targets.stats_line());
        }
        if let Some(text) = self.lab.names.fired(control) {
            let names = chranim::event_names(text.trim());
            erlog!("lab: {} behaviour event name(s) contain '{}'", names.len(), text.trim());
            for chunk in names.chunks(12) {
                erlog!("lab:   {}", chunk.iter().map(|(n, id)| format!("{n}={id}")).collect::<Vec<_>>().join(" "));
            }
        }
        if let Some(text) = self.lab.fire.fired(control) {
            let parts: Vec<&str> = text.split(',').map(str::trim).collect();
            match (self.pick(parts.first().copied().unwrap_or("")), parts.get(1)) {
                (Some(e), Some(name)) => {
                    let done = targets::find_chr(&e.handle).map(|chr| chranim::fire(chr, name));
                    erlog!("lab: event '{name}' asked of target {} (c{:04}): {done:?}; it played {}", e.id.0, e.npc_id, e.anim);
                    self.lab_watch(&e, format!("fire {name}"), 6.0, false);
                }
                _ => erlog!("lab: fire {text}: who,W_Name"),
            }
        }
        if let Some(text) = self.lab.anim.fired(control) {
            let parts: Vec<&str> = text.split(',').map(str::trim).collect();
            match (self.pick(parts.first().copied().unwrap_or("")), parts.get(1).and_then(|v| v.parse::<i32>().ok())) {
                (Some(e), Some(id)) => {
                    let done = targets::find_chr(&e.handle).filter(|chr| crate::game::plausible(crate::word!(chr.modules.event))).map(|chr| chr.modules.event.as_mut().request_animation_id = id);
                    erlog!("lab: animation id {id} asked of target {} (c{:04}) through its event module: {}", e.id.0, e.npc_id, done.is_some());
                    self.lab_watch(&e, format!("anim {id}"), 6.0, false);
                }
                _ => erlog!("lab: anim {text}: who,id"),
            }
        }
        if let Some(text) = self.lab.speed.fired(control) {
            let parts: Vec<&str> = text.split(',').map(str::trim).collect();
            let num = |k: usize| parts.get(k).and_then(|v| v.parse::<f32>().ok());
            match (self.pick(parts.first().copied().unwrap_or("")), num(1)) {
                (Some(e), Some(x)) => {
                    self.lab.speed_hold = Some((e.handle, x, num(2).unwrap_or(3.0)));
                    erlog!("lab: animation speed of target {} (c{:04}) held at {x}", e.id.0, e.npc_id);
                    self.lab_watch(&e, format!("speed {x}"), num(2).unwrap_or(3.0) + 2.0, false);
                }
                _ => erlog!("lab: speed {text}: who,x[,seconds]"),
            }
        }
        if let Some(text) = self.lab.hold.fired(control) {
            let parts: Vec<&str> = text.split(',').map(str::trim).collect();
            let num = |k: usize, d: f32| parts.get(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(d);
            match self.pick(parts.first().copied().unwrap_or("")) {
                Some(e) => {
                    let (height, seconds, bits) = (num(1, 2.2), num(2, 3.0), num(3, 3.0) as u32);
                    let held = self.targets.puppets.hold(&e, height, seconds, bits);
                    erlog!("lab: target {} (c{:04}) held {height} m up for {seconds} s with suspension bits {bits}: {held}; it played {}", e.id.0, e.npc_id, e.anim);
                    self.lab_watch(&e, format!("hold bits {bits}"), seconds + 6.0, false);
                }
                None => erlog!("lab: hold {text}: no such target"),
            }
        }
        if let Some(text) = self.lab.animq.fired(control) {
            match self.pick(text.trim()) {
                Some(e) => self.lab_watch(&e, "queue".into(), 8.0, true),
                None => erlog!("lab: animq {text}: no such target"),
            }
        }
        if let Some(text) = self.lab.survey.fired(control) {
            let parts: Vec<&str> = text.split(',').map(str::trim).collect();
            let num = |k: usize, d: f32| parts.get(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(d);
            match self.pick(parts.first().copied().unwrap_or("")) {
                Some(e) => {
                    let (first, last, air) = (num(1, 0.0) as u8, num(2, 13.0) as u8, num(3, 0.0) > 0.0);
                    self.targets.bullets.pick(control);
                    let (pin, flat_only) = (num(5, 0.0) > 0.0, num(6, 0.0) > 0.0);
                    if air || pin {
                        self.targets.puppets.hold(&e, if air { 2.2 } else { 0.02 }, (last.saturating_sub(first) as f32 + 1.0) * 2.0 * TRIAL + 6.0, control.i64("pup.ai").map_or(3, |v| v as u32));
                    }
                    erlog!("survey: START c{:04} (target {}, {:?}, r {:.0} h {:.0} cm, hp {}/{}, poise {:.0}) levels {first}..{last} {}", e.npc_id, e.id.0, e.weight, e.radius, e.half_height * 2.0, e.hp, e.hp_max, e.poise_max, if air { "HELD IN THE AIR" } else { "on the ground" });
                    self.lab.run = Some(Survey { handle: e.handle, npc_id: e.npc_id, first, last, air, flat_only, poise: num(4, 0.0), level: first, flat: flat_only, t: -1.5, fired: false, hp: e.hp, poise_before: e.poise, from: e.er_pos, seen: Vec::new(), last_anim: 0, far: 0.0, high: 0.0 });
                }
                None => erlog!("survey: {text}: no such target"),
            }
        }
        // ---- the things that run over time
        if let Some((handle, x, left)) = self.lab.speed_hold.as_mut() {
            *left -= dt;
            let over = *left <= 0.0;
            if let Some(chr) = targets::find_chr(handle) {
                chranim::set_speed(chr, if over { 1.0 } else { *x });
            }
            if over {
                self.lab.speed_hold = None;
                erlog!("lab: animation speed given back");
            }
        }
        self.targets.puppets.hold_tick(dt);
        self.lab_watch_step(dt);
        self.lab_survey_step(dt);
    }

    fn lab_watch(&mut self, e: &targets::Enemy, what: String, limit: f32, queue: bool) {
        let last = targets::find_chr(&e.handle).map(|c| chranim::now(c)).unwrap_or_default();
        self.lab.watch = Some(Watch { handle: e.handle, what, seconds: 0.0, limit, last, queue, last_queue: String::new(), from: e.er_pos, seen: vec![(last.id, last.length, 0.0)], far: 0.0, high: 0.0 });
    }

    fn lab_watch_step(&mut self, dt: f32) {
        let Some(w) = self.lab.watch.as_mut() else { return };
        w.seconds += dt;
        let Some(chr) = targets::find_chr(&w.handle) else {
            erlog!("lab: watch ({}): the character is gone", w.what);
            self.lab.watch = None;
            return;
        };
        let now = chranim::now(chr);
        let at = crate::game::v3(&chr.modules.physics.position);
        w.far = w.far.max(((at - w.from) * Vec3::new(1.0, 0.0, 1.0)).length());
        w.high = w.high.max(at.y - w.from.y);
        if w.queue {
            let ta = &chr.modules.time_act;
            let text = format!("w{} r{} {:?}", ta.write_idx, ta.read_idx, ta.anim_queue.iter().map(|a| a.anim_id).collect::<Vec<_>>());
            if text != w.last_queue {
                erlog!("lab: {:.2} s queue {}", w.seconds, chranim::queue(chr));
                w.last_queue = text;
            }
        }
        if now.id != w.last.id || now.time + 0.05 < w.last.time {
            erlog!("lab: {:.2} s ({}) animation {} ({:.2} s long) at {:.2} s; before it {} reached {:.2} / {:.2} s; speed {:.2}", w.seconds, w.what, now.id, now.length, now.time, w.last.id, w.last.time, w.last.length, chranim::speed(chr));
            w.seen.push((now.id, now.length, w.seconds));
        }
        w.last = now;
        if w.seconds > w.limit {
            erlog!("lab: watch over ({}): animations {:?}; farthest {:.2} m, highest {:+.2} m; hp {}", w.what, w.seen, w.far, w.high, chr.modules.data.hp);
            self.lab.watch = None;
        }
    }

    fn lab_survey_step(&mut self, dt: f32) {
        let Some(mut s) = self.lab.run.take() else { return };
        let Some(id) = self.targets.id_of(&s.handle) else {
            erlog!("survey: ABORTED: the subject is unknown");
            return;
        };
        let Some(e) = self.targets.get(id).cloned() else {
            erlog!("survey: ABORTED: the subject left the area");
            self.targets.puppets.release_held();
            return;
        };
        if !e.alive {
            erlog!("survey: ABORTED: the subject died (level {} {})", s.level, if s.flat { "flat" } else { "weapon" });
            self.targets.puppets.release_held();
            return;
        }
        s.t += dt;
        let now = targets::find_chr(&s.handle).map(|c| chranim::now(c)).unwrap_or_default();
        let name = |s: &Survey| format!("chr=c{:04} level={} mode={} where={}", s.npc_id, s.level, if s.flat { "flat" } else { "weapon" }, if s.air { "air" } else { "ground" });
        if !s.fired && s.t >= FIRE_AT {
            s.fired = true;
            (s.hp, s.poise_before, s.from, s.far, s.high) = (e.hp, e.poise, e.er_pos, 0.0, 0.0);
            s.seen = vec![(now.id, now.length, 0.0)];
            s.last_anim = now.id;
            // (weapon-type: the row's correction carries the damage; flat: the row's own number)
            let d = crate::bullet::Shape::default();
            let shape = if s.flat { crate::bullet::Shape { flags: crate::bullet::FLAT_ATTACK, ..d } } else { crate::bullet::Shape { flags: 0, corr: 5, ..d } };
            let attack = crate::bullet::Attack { phys: 1, poise: s.poise, stamina: 0, level: s.level, ..crate::bullet::Attack::PLAIN };
            let result = self.targets.test_attack(id, attack, true, Some(shape));
            erlog!("survey: FIRE {} anim={} ({})", name(&s), now.id, match &result { Ok(_) => "bullet out".to_string(), Err(why) => format!("FAILED: {why}") });
        }
        if s.fired {
            if now.id != s.last_anim {
                s.seen.push((now.id, now.length, s.t - FIRE_AT));
                s.last_anim = now.id;
            }
            s.far = s.far.max(((e.er_pos - s.from) * Vec3::new(1.0, 0.0, 1.0)).length());
            s.high = s.high.max(e.er_pos.y - s.from.y);
        }
        if s.t >= TRIAL {
            let anims: Vec<String> = s.seen.iter().map(|(id, len, at)| format!("{id}({len:.2}s)@{at:.2}")).collect();
            erlog!("survey: RESULT {} anims=[{}] moved={:.2} rose={:+.2} hp={}->{} poise={:.0}->{:.0}", name(&s), anims.join(" "), s.far, s.high, s.hp, e.hp, s.poise_before, e.poise);
            (s.t, s.fired) = (0.0, false);
            if !s.flat {
                s.flat = true;
            } else if s.level < s.last {
                s.flat = s.flat_only;
                s.level += 1;
            } else {
                erlog!("survey: DONE c{:04} {}", s.npc_id, if s.air { "air" } else { "ground" });
                self.targets.puppets.release_held();
                return;
            }
        }
        self.lab.run = Some(s);
    }
}
