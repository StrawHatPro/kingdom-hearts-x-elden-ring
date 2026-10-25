//! (ZFIN) ZETTAFLARE'S FINALE ON AN ELDEN RING CHARACTER, the host's half (UNTESTED IN GAME): ZETTA's finale state per
//! target (targets.rs `script_finale`, asked every step while it lasts) goes to the renderer's draw route
//! (kh-er-render finale.rs: the black silhouette over the game's own pixels of that character, crumbling; a white-out of its
//! screen box when its pixels cannot be found), with the other characters near it kept apart. GENERIC: the target's row
//! (feet, the game's hit capsule radius and height) is all it needs; no per-enemy data.
//!
//! "Leaving only the white light": once a killed target has crumbled away (`crumble` 1) its body is hidden by the game's own
//! render switch (`ChrInsFlags1c5.enable_render`, as the hidden Tarnished and the graces); the switch is given back when the
//! character lives again (a respawn after a rest, a boss's next phase), never while the game keeps its dead body (2026-10-23:
//! no corpse, no ragdoll, ever; forgotten once the game has it no more), when the
//! finale ends before the crumble finished (it was never hidden), and at the mod's exit (`release`).

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{FieldInsHandle, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::Vec3;
use kh_core::host::ScriptFinale;
use kh_er_render::finale::{Capsule, FinaleTarget};

use crate::erlog;

/// A finale state not asked again within this (s) has ended (ZETTA asks it every step).
pub const STALE_S: f64 = 0.5;
/// A crumbled body is shown again by us this long (s) after it was hidden, if the game still has it.
pub const HIDE_HOLD_S: f64 = 30.0;
/// Characters this near (cm, feet) are the target's neighbours.
pub const NEIGHBOUR_CM: f32 = 1000.0;
/// A hidden body's last white shape is handed to the renderer this long (s) after the hide, for its fade (`r.finale_fade`,
/// at most this), whether ZETTA still asks or not.
pub const GHOST_S: f64 = 2.0;
/// `r.finale_over` by default: the words of the effect materials drawn over the silhouette (ZETTA's crumble fragments; a
/// material whose name holds one, e.g. an effect ZETTA names `..zfin_frag..`).
pub const OVER_DEFAULT: &str = "zfin_frag";

/// What the book needs of a target row this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    pub id: u32,
    pub feet: Vec3,
    pub radius: f32,
    pub half_height: f32,
    /// (round 7) the game draws it (an undrawn character has no pixels to keep apart)
    pub drawn: bool,
}

/// (round 7, Malenia: five more c2120 rows at her very place) A neighbour standing on the target itself (feet within 3/4 of
/// the larger radius, horizontally) cannot be told apart from it: it is no neighbour (its pixels are the target's).
pub fn coincident(a: Vec3, ra: f32, b: Vec3, rb: f32) -> bool {
    let d = Vec3::new(a.x - b.x, a.y - b.y, 0.0).length();
    d < 0.75 * ra.max(rb)
}

/// The book's decisions on the game's render switch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Hide(u32),
    Show(u32),
}

/// The pure part: what was asked and when, what is hidden since when.
#[derive(Default)]
pub struct Book {
    asked: Vec<(u32, ScriptFinale, f64)>,
    /// hidden bodies: id, since when, their capsule at the hide (for the fade of their last white shape)
    hidden: Vec<(u32, f64, Capsule)>,
    /// (ZETTA 2026-10-23, the user: "she played her normal death animation at phase 2 instead of being completely gone!")
    /// bodies Zettaflare killed: kept hidden while the game still has them alive (the hand-back's death comes while hidden),
    /// shown again only when that keep is taken back (a phase change the game made of the death)
    keep: Vec<u32>,
}

impl Book {
    /// ZETTA's state of target `id` at time `now` (s); `None` = its finale ended.
    pub fn ask(&mut self, id: u32, finale: Option<ScriptFinale>, now: f64) {
        self.asked.retain(|a| a.0 != id);
        if let Some(f) = finale {
            self.asked.push((id, f, now));
        }
    }

    /// One frame: the renderer's targets and the switch decisions. `alive(id)`: the character lives (None = the game has it
    /// no more).
    pub fn step(&mut self, rows: &[Row], now: f64, alive: &dyn Fn(u32) -> Option<bool>) -> (Vec<FinaleTarget>, Vec<Act>) {
        self.step_with(rows, now, alive, &|_| false)
    }

    /// As `step`; `breaks(id)`: the renderer breaks that silhouette apart from its snapshot (kh-er-render finale.rs
    /// `breaks_apart`): its body is hidden from its first crumble on (its pieces stand in for it), else once it has crumbled.
    pub fn step_with(&mut self, rows: &[Row], now: f64, alive: &dyn Fn(u32) -> Option<bool>, breaks: &dyn Fn(u32) -> bool) -> (Vec<FinaleTarget>, Vec<Act>) {
        self.asked.retain(|a| now - a.2 < STALE_S);
        let mut targets = Vec::new();
        let mut acts = Vec::new();
        for (id, f, _) in &self.asked {
            let Some(row) = rows.iter().find(|r| r.id == *id) else { continue };
            let capsule = |r: &Row| Capsule { feet: r.feet, radius: r.radius, half_height: r.half_height };
            let neighbours = rows.iter().filter(|r| r.id != *id && r.drawn && (r.feet - row.feet).length() < NEIGHBOUR_CM && !coincident(r.feet, r.radius, row.feet, row.radius)).map(capsule).collect();
            if f.silhouette && (f.crumble >= 1.0 || (f.crumble > 0.0 && breaks(*id))) && !self.hidden.iter().any(|h| h.0 == *id) {
                self.hidden.push((*id, now, capsule(row)));
                acts.push(Act::Hide(*id));
            }
            let hidden_for = self.hidden.iter().find(|h| h.0 == *id).map(|h| (now - h.1) as f32);
            targets.push(FinaleTarget { id: *id, body: capsule(row), immerse: f.immerse, silhouette: f.silhouette, crumble: f.crumble, neighbours, hidden_for, push: f.push, dark: f.dark });
        }
        // a hidden body ZETTA no longer asks for: its last white shape still fades
        for (id, at, body) in &self.hidden {
            if now - at < GHOST_S && !targets.iter().any(|t| t.id == *id) {
                targets.push(FinaleTarget { id: *id, body: *body, immerse: 1.0, silhouette: true, crumble: 1.0, neighbours: Vec::new(), hidden_for: Some((now - at) as f32), push: Default::default(), dark: 1.0 });
            }
        }
        let asked: Vec<u32> = self.asked.iter().map(|a| a.0).collect();
        let keep = self.keep.clone();
        self.hidden.retain(|(id, at, _)| {
            let lives = alive(*id);
            let back = match lives {
                None => now - at >= 2.0 * HIDE_HOLD_S, // (gone from the game: forgotten)
                // (ZETTA 2026-10-23) a body Zettaflare killed: never drawn again before its death (it dies hidden)
                Some(true) => !asked.contains(id) && !keep.contains(id),
                // (ZETTA 2026-10-23, the user: "Zettaflare should completely overrule any sort of animation for an enemy dying ...
                // ragdolling which is not correct") dead: hidden for as long as the game keeps the body (never its corpse)
                Some(false) => false,
            };
            if back && lives.is_some() {
                acts.push(Act::Show(*id));
            }
            !back
        });
        (targets, acts)
    }

    /// (ZETTA 2026-10-23) `on`: the body Zettaflare killed stays hidden, alive or dead, until `keep(id, false)`.
    pub fn keep(&mut self, id: u32, on: bool) {
        self.keep.retain(|k| *k != id);
        if on {
            self.keep.push(id);
        }
    }

    /// (ZETTA 2026-10-23) Is the body hidden by the finale now?
    pub fn is_hidden(&self, id: u32) -> bool {
        self.hidden.iter().any(|h| h.0 == id)
    }

    /// Every hidden body to be shown again (the mod's exit).
    pub fn drain_hidden(&mut self) -> Vec<u32> {
        self.asked.clear();
        self.keep.clear();
        self.hidden.drain(..).map(|h| h.0).collect()
    }
}

struct Live {
    book: Book,
    handles: Vec<(u32, FieldInsHandle)>,
    started: Instant,
}

static LIVE: Mutex<Option<Live>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Live) -> R) -> Option<R> {
    let mut guard = LIVE.lock().ok()?;
    let live = guard.get_or_insert_with(|| Live { book: Book::default(), handles: Vec::new(), started: Instant::now() });
    Some(f(live))
}

/// targets.rs `script_finale`: ZETTA's state of a target (its handle for the render switch).
pub fn asked(id: u32, handle: Option<FieldInsHandle>, finale: Option<ScriptFinale>) {
    with(|l| {
        let now = l.started.elapsed().as_secs_f64();
        l.book.ask(id, finale, now);
        if let Some(h) = handle {
            l.handles.retain(|x| x.0 != id);
            l.handles.push((id, h));
        }
    });
}

/// (ZDRAW) The handle the finale knows of target `id` (zdraw.rs reads its body's model and bones by it).
pub fn handle_of(id: u32) -> Option<FieldInsHandle> {
    with(|l| l.handles.iter().find(|h| h.0 == id).map(|h| h.1.clone())).flatten()
}

/// The game's render switch of a character by handle. SAFETY of the callers: game thread.
fn set_render(handle: &FieldInsHandle, on: bool) -> Option<bool> {
    // SAFETY: game thread; the game's own lookup by handle (it checks the handle).
    let wcm = unsafe { WorldChrMan::instance_mut() }.ok()?;
    let chr = wcm.chr_ins_by_handle_mut(handle)?;
    if !crate::game::chr_complete(chr) {
        return None;
    }
    if chr.chr_flags1c5.enable_render() != on {
        chr.chr_flags1c5.set_enable_render(on);
    }
    Some(on)
}

fn lives(handle: &FieldInsHandle) -> Option<bool> {
    // SAFETY: as `set_render`.
    let wcm = unsafe { WorldChrMan::instance_mut() }.ok()?;
    let chr = wcm.chr_ins_by_handle_mut(handle)?;
    if !crate::game::chr_complete(chr) {
        return None;
    }
    Some(chr.modules.data.hp > 0 && !chr.chr_flags1c5.death_flag())
}

/// Once a frame (targets.rs `refresh`, game thread): the renderer's finale targets and the render switches.
pub fn frame(rows: &[Row]) {
    let Some((targets, acts, handles)) = with(|l| {
        let now = l.started.elapsed().as_secs_f64();
        let handles = l.handles.clone();
        let alive = |id: u32| handles.iter().find(|h| h.0 == id).and_then(|h| lives(&h.1));
        let (targets, acts) = l.book.step_with(rows, now, &alive, &kh_er_render::finale::breaks_apart);
        (targets, acts, handles)
    }) else {
        return;
    };
    kh_er_render::finale::set(targets);
    crate::zdraw::frame(); // (ZDRAW) the finale's targets drawn from their own models (called from here: never lost)
    for act in acts {
        let (id, on) = match act {
            Act::Hide(id) => (id, false),
            Act::Show(id) => (id, true),
        };
        if let Some(h) = handles.iter().find(|h| h.0 == id) {
            let done = set_render(&h.1, on);
            erlog!("zetta: target {id}: {} ({})", if on { "its body is drawn again" } else { "crumbled away: its body is hidden (only the white light is left)" }, if done.is_some() { "done" } else { "no character any more" });
        }
    }
}

/// (ZETTA 2026-10-23) targets.rs: the body Zettaflare killed stays hidden (`on`), or may be shown again (a phase change).
pub fn keep_hidden(id: u32, on: bool) {
    with(|l| l.book.keep(id, on));
}

/// (ZETTA 2026-10-24) A row of a boss's final form Zettaflare kills with it: hidden now and for good (kept while alive, never
/// drawn again after its death; no white shape of it), never shown by the game's own reveal.
pub fn hide_for_good(id: u32, handle: FieldInsHandle) {
    with(|l| {
        let now = l.started.elapsed().as_secs_f64();
        l.handles.retain(|x| x.0 != id);
        l.handles.push((id, handle));
        if !l.book.hidden.iter().any(|h| h.0 == id) {
            l.book.hidden.push((id, now - GHOST_S - 1.0, Capsule { feet: glam::Vec3::ZERO, radius: 0.0, half_height: 0.0 }));
        }
        l.book.keep(id, true);
    });
    let _ = set_render(&handle, false);
}

/// (ZETTA 2026-10-24) Keep a row's render switch off every frame (the game's reveal of a final form puts it on).
pub fn hold_hidden(handle: &FieldInsHandle) {
    let _ = set_render(handle, false);
}

/// (ZETTA 2026-10-23) Is the body hidden by Zettaflare's finale now?
pub fn hidden_now(id: u32) -> bool {
    with(|l| l.book.is_hidden(id)).unwrap_or(false)
}

/// The mod's exit: every body we hid gets its render switch back.
pub fn release() {
    let Some((ids, handles)) = with(|l| (l.book.drain_hidden(), l.handles.clone())) else { return };
    for id in ids {
        if let Some(h) = handles.iter().find(|h| h.0 == id) {
            let _ = set_render(&h.1, true);
        }
    }
    kh_er_render::finale::set(Vec::new());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<Row> {
        vec![
            Row { id: 1, feet: Vec3::ZERO, radius: 40.0, half_height: 90.0, drawn: true },
            Row { id: 2, feet: Vec3::new(150.0, 0.0, 0.0), radius: 40.0, half_height: 90.0, drawn: true },
            Row { id: 3, feet: Vec3::new(5000.0, 0.0, 0.0), radius: 40.0, half_height: 90.0, drawn: true },
            // (round 7) Malenia's extra rows: one on her very place, one not drawn beside her
            Row { id: 4, feet: Vec3::new(10.0, 5.0, 0.0), radius: 45.0, half_height: 120.0, drawn: true },
            Row { id: 5, feet: Vec3::new(0.0, 120.0, 0.0), radius: 40.0, half_height: 90.0, drawn: false },
        ]
    }

    fn look(silhouette: bool, crumble: f32) -> Option<ScriptFinale> {
        Some(ScriptFinale { immerse: 1.0, silhouette, crumble, push: Default::default() /* (ZETTA round 5c) */, dark: 1.0 })
    }

    #[test]
    fn zfin_targets_carry_the_look_and_near_neighbours_only() {
        let mut b = Book::default();
        b.ask(1, look(true, 0.4), 0.0);
        let (t, acts) = b.step(&rows(), 0.1, &|_| Some(false));
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].hidden_for, None);
        assert_eq!((t[0].id, t[0].silhouette, t[0].crumble), (1, true, 0.4));
        assert_eq!(t[0].neighbours.len(), 1, "the far character, one on its very place (round 7) and one not drawn are no neighbours");
        assert!(acts.is_empty());
    }

    #[test]
    fn zfin_a_state_not_asked_again_ends() {
        let mut b = Book::default();
        b.ask(1, look(false, 0.0), 0.0);
        assert_eq!(b.step(&rows(), 0.4, &|_| Some(true)).0.len(), 1);
        assert!(b.step(&rows(), 0.6, &|_| Some(true)).0.is_empty());
        b.ask(1, look(false, 0.0), 1.0);
        b.ask(1, None, 1.1);
        assert!(b.step(&rows(), 1.1, &|_| Some(true)).0.is_empty());
    }

    #[test]
    fn zfin_crumbled_body_hidden_once_and_shown_again_by_every_exit() {
        // dead: hidden at crumble 1, held 30 s, then the switch back
        let mut b = Book::default();
        b.ask(1, look(true, 1.0), 0.0);
        let (t, acts) = b.step(&rows(), 0.0, &|_| Some(false));
        assert_eq!(acts, vec![Act::Hide(1)]);
        assert_eq!(t[0].hidden_for, Some(0.0), "its last white shape starts fading the frame it is hidden");
        b.ask(1, look(true, 1.0), 0.1);
        let (t, acts) = b.step(&rows(), 0.1, &|_| Some(false));
        assert!(acts.is_empty(), "hidden once");
        assert!((t[0].hidden_for.unwrap() - 0.1).abs() < 1e-6);
        // ZETTA stops asking: the shape is still handed over for its fade, then no more
        let (t, _) = b.step(&rows(), 1.0, &|_| Some(false));
        assert_eq!((t.len(), t[0].crumble, t[0].hidden_for.is_some()), (1, 1.0, true));
        assert!(b.step(&rows(), 2.5, &|_| Some(false)).0.is_empty());
        assert!(b.step(&rows(), 10.0, &|_| Some(false)).1.is_empty(), "held while dead");
        // (2026-10-23) never its corpse: hidden for as long as the game keeps the dead body
        assert!(b.step(&rows(), 300.0, &|_| Some(false)).1.is_empty(), "no corpse, ever");
        // lives again (a rest at a grace): shown at once
        let mut b = Book::default();
        b.ask(1, look(true, 1.0), 0.0);
        b.step(&rows(), 0.0, &|_| Some(false));
        assert_eq!(b.step(&rows(), 5.0, &|_| Some(true)).1, vec![Act::Show(1)]);
        // gone from the game: forgotten, nothing to switch
        let mut b = Book::default();
        b.ask(1, look(true, 1.0), 0.0);
        b.step(&rows(), 0.0, &|_| None);
        assert!(b.step(&rows(), 61.0, &|_| None).1.is_empty());
        assert!(b.hidden.is_empty());
        // the mod's exit
        let mut b = Book::default();
        b.ask(1, look(true, 1.0), 0.0);
        b.step(&rows(), 0.0, &|_| Some(false));
        assert_eq!(b.drain_hidden(), vec![1]);
        // (round 3) a silhouette breaking apart from its snapshot: hidden from its first crumble (its pieces stand in)
        let mut b = Book::default();
        b.ask(1, look(true, 0.0), 0.0);
        assert!(b.step_with(&rows(), 0.0, &|_| Some(false), &|_| true).1.is_empty(), "not before its crumble");
        b.ask(1, look(true, 0.01), 0.1);
        assert_eq!(b.step_with(&rows(), 0.1, &|_| Some(false), &|_| true).1, vec![Act::Hide(1)]);
        b.ask(2, look(true, 0.01), 0.1);
        assert!(b.step_with(&rows(), 0.1, &|_| Some(false), &|id| id == 1).1.is_empty(), "another body waits for crumble 1");
        // (2026-10-23, the user: "she played her normal death animation at phase 2 instead of being completely gone!") a body
        // Zettaflare killed stays hidden while the game still has it alive after its finale (it dies hidden), and after its
        // death; only a phase change (the keep taken back) shows it again
        let mut b = Book::default();
        b.ask(1, look(true, 1.0), 0.0);
        b.keep(1, true);
        assert_eq!(b.step(&rows(), 0.0, &|_| Some(true)).1, vec![Act::Hide(1)]);
        assert!(b.step(&rows(), 20.0, &|_| Some(true)).1.is_empty(), "the finale over, alive in the game: still hidden (no redraw)");
        assert!(b.step(&rows(), 20.1, &|_| Some(false)).1.is_empty(), "its death comes while hidden");
        assert!(b.step(&rows(), 400.0, &|_| Some(false)).1.is_empty(), "no death animation, no corpse, ever");
        let mut b = Book::default();
        b.ask(1, look(true, 1.0), 0.0);
        b.keep(1, true);
        b.step(&rows(), 0.0, &|_| Some(true));
        b.keep(1, false);
        assert_eq!(b.step(&rows(), 20.0, &|_| Some(true)).1, vec![Act::Show(1)], "a phase change the game made of it: shown");
        // a survivor (no silhouette) or a crumble not finished is never hidden
        let mut b = Book::default();
        b.ask(1, look(false, 1.0), 0.0);
        b.ask(2, look(true, 0.99), 0.0);
        assert!(b.step(&rows(), 0.0, &|_| Some(true)).1.is_empty());
    }
}
