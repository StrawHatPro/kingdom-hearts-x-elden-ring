//! (ER-POPUPS, THE USER 2026-11-07: "evergaol pop ub boxes, as well as anything else still using Elden Ring's pop up boxes
//! needs a Kingdom Hearts equiveleant, same as what we've already replaced") ELDEN RING'S EVENT-SCRIPT POP-UP BOXES AS
//! KH3'S DIALOG WINDOW. State note and inventory: build/sora_tmp/er_popups/STATE.md, inventory.csv.
//!
//! WHAT THE GAME DOES (read from its own files: every EMEVD of build/sora_tmp/er/er33/game/event, the FMGs of
//! build/sora_tmp/er/conv/game/fmg; the code read in both executables with build/sora_tmp/er_popups/tools/dis.js; NOT SEEN
//! IN THE GAME: agents never launch it):
//! - The boxes of the world (the evergaols' "Enter evergaol?", the imp statues' "Use Stonesword Key?" and "No Stonesword Key
//!   in inventory", the waygates' "Travel to another location?", "Return to entrance?", "Cannot open from this side",
//!   "Locked by some contraption", "Held shut by a seal", the war plaques, the NPC invasions' "Invade ...'s world?", the DLC's
//!   "Head to the realm of shadow?" ...) are made by TWO instructions of EMEVD bank 2007 (dispatcher 2.7.1.0 RVA 0x571370 /
//!   2.7.2.0 0x571820, already detoured by dragonmsg.rs; the instruction's index at `[[ctx + 0xD0] + 4]`):
//!   - 2007[1] (107 uses) DisplayGenericDialog(msg i32, prompt i16, buttons i16, entity i32, distance f32): case 0x5713DB
//!     asks for the box (0x594A20 -> the open request 0x810300) and returns at once: the event goes on while the box is up.
//!   - 2007[10] (37 uses) DisplayGenericDialogAndSetEventFlags(msg, prompt, buttons, entity, distance, left flag u32, right
//!     flag u32, cancel flag u32): case 0x5715E0 asks for the box (the open request [`OPEN_FN`]) and gives the event a WAITER
//!     (0x57B520, vtable 0x2A5ED00: +0x18 bit 0 "done", +0x28 the msg, +0x60 / +0x64 / +0x68 the three flags; registered by
//!     0x584670) that HOLDS THE EVENT until it is done. Its frame update ([`WAITER_FN`]) reads the box's answer and sets ONE
//!     flag ON (the CSEventFlagMan writer): answer 1 (Yes / OK) -> the left flag (0x57BAD0); answer 0 or 2 (No, or B) -> the
//!     right flag (0x57B9B0); the player beyond `distance`, dead, or the box taken away -> the cancel flag (0x57BA40); a flag
//!     of 0 is not set. buttons 2 = Yes / No (prompt 0: every question of the game); 1 or 6 = one button.
//!   - The open request [`OPEN_FN`] `fn(out: *mut [u64; 2], msg, prompt, buttons) -> out` posts the box to the dialog
//!     manager (0x813C50); without a manager it writes {0, 0} and asks for nothing: no caller reads `out`.
//!   - The bank switch steps the event past ANY instruction whatever the case answers (0x583E57 -> 0x585DD0): only a waiter
//!     holds an event, so a question cannot be held by answering "not yet".
//! - The text: EventTextForMap (msgbnd file 34; the DLC's 367 / 467), the game's language; `<?gdsparam@N?>` is the name of
//!   goods N (GoodsName 10 / 319 / 419).
//!
//! WHAT WE DO (the route of the ally question, kh-host hud_erdialog.rs: KH3's Dialog movie, its buttons, KH3's words
//! "Yes" / "No" / "OK", the menus' sounds, the world held, the pad as remapped, the keyboard's menu words), per box TYPE,
//! never per message; key `er.dialog_kh` (default ON; `= 0` gives every box back to the game):
//! - 2007[1], a message with a text: queued for KH3's window (one button, OK) and the instruction answered "handled" (1) at
//!   once without the game's case: the event goes on while the message is up, as with the game's box.
//! - 2007[10], a question with a text: the game's own case runs (its waiter holds the event) with the open request
//!   suppressed ([`suppressed`]: {0, 0}, the request's own "no manager" answer), so Elden Ring's box is never made; the
//!   question goes to KH3's window (Yes / No when buttons = 2, else OK, the cursor on Yes). The waiter's update is detoured:
//!   for the waiter of a box of ours (bound by its msg and flags at its first update) it stays "not done" until the player
//!   answered, then sets THE SAME FLAG THE GAME'S WAITER SETS (A on Yes / OK -> left; No or B -> right; the window taken
//!   away -> cancel) and is done: the event continues exactly as after the game's own box. Every other waiter: the game's.
//! - The distance and death rules of the game's box need no copy: KH3's window holds the world while it is up (the player
//!   cannot walk away or be killed), as every KH3 window does.
//! - Every frame ([`frame`], from dragonmsg.rs's): queued boxes go to the HUD, answers come back; a box whose waiter stops
//!   asking for [`ABANDON_FRAMES`] frames while the world runs (its event ended: a load, the map unloaded) is taken off.
//! - A fault of ours (a missing address, an unreadable instruction, no text, no HUD) gives the game its own box.
//! - NOT HERE (inventory.csv): the talk scripts' Yes / No (talkkh, the bubble), the banners (erkeyui), the tutorials
//!   (tutorial.rs), the draconic-power line (dragonmsg.rs), the blinking messages / region names / network lines (2007[4] /
//!   [12] / [13] / [14] / [16]: not boxes, left as they are), the menu screens' own boxes (the map's travel question ...:
//!   inside Elden Ring's own menu screens, a different route).

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::havok::Located;

/// EMEVD 2007[1]: DisplayGenericDialog (a message; the event goes on).
pub const INSTR_MESSAGE: i32 = 1;
/// EMEVD 2007[10]: DisplayGenericDialogAndSetEventFlags (the event's waiter waits for the answer; one of three flags is set).
pub const INSTR_QUESTION: i32 = 10;
/// Frames a question's waiter may stop asking (while the world runs) before its window is taken off.
pub const ABANDON_FRAMES: u32 = 120;

/// The field box open request (see the header), found through its one call in the bank-2007 dispatcher's case 10:
/// `movsx r9d, [rsp+58h]; movsx r8d, [rsp+54h]; mov edx, [rsp+50h]; lea rcx, [rsp+60h]; call OPEN; mov r8d, esi; cmp esi, -1`.
pub(crate) const OPEN_FN: Located = Located {
    name: "field generic dialog open request (ER-POPUPS)",
    pattern: "44 0f bf 4c 24 58 44 0f bf 44 24 54 8b 54 24 50 48 8d 4c 24 60 e8 $ { ' } 44 8b c6 83 fe ff",
    capture: 1,
    rva_2_7_1_0: 0x0081_0300,
    rva_2_7_2_0: 0x0081_07b0, /* (ER-POPUPS) through the same call site, 0x571B32 */
};

/// The 2007[10] waiter's frame update (vtable 0x2A5ED00 slot 1): its prologue, the dialog manager's singleton read with the
/// game's "not made yet" path, `mov rsi, [rsi+80h]; xor bpl, bpl; xor r14b, r14b; mov dword [rsp+90h], -1; mov dword
/// [rsp+0A8h], -1`.
pub(crate) const WAITER_FN: Located = Located {
    name: "event generic dialog waiter update (ER-POPUPS)",
    pattern: "40 55 56 57 41 56 48 83 ec 68 48 8b 35 ? ? ? ? 48 8b f9 48 85 f6 75 2e 48 8d 0d ? ? ? ? e8 ? ? ? ? 4c 8b c8 4c 8d 05 ? ? ? ? ba b4 00 00 00 48 8d 0d ? ? ? ? e8 ? ? ? ? 48 8b 35 ? ? ? ? 48 8b b6 80 00 00 00 40 32 ed 45 32 f6 c7 84 24 90 00 00 00 ff ff ff ff c7 84 24 a8 00 00 00 ff ff ff ff",
    capture: 0,
    rva_2_7_1_0: 0x0057_b6b0,
    rva_2_7_2_0: 0x0057_bb60, /* (ER-POPUPS) */
};

/// The two addresses of this module (the install; gamever.rs's offline check).
pub(crate) const LOCATED: [&Located; 2] = [&OPEN_FN, &WAITER_FN];

/// The waiter's fields (2.7.1.0 = 2.7.2.0: the setter 0x57B690 / 0x57BB40 and the done write are byte-identical).
const WAITER_DONE: usize = 0x18;
const WAITER_MSG: usize = 0x28;
const WAITER_FLAGS: usize = 0x60;

/// The buttons of a box (its TYPE).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Buttons {
    /// Yes / No (the game's buttons = 2).
    YesNo,
    /// One button, OK (the game's buttons 1 / 6, and every 2007[1] message).
    Ok,
}

/// The player's answer as the game's waiter knows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Yes / OK (the game's answer 1): the left flag.
    Left,
    /// No, or B (the game's answer 0 / 2): the right flag.
    Right,
    /// The box was taken away (the game's distance / death / closed rules): the cancel flag.
    Cancel,
}

impl Answer {
    /// The waiter's flag slot of this answer (+0x60 / +0x64 / +0x68).
    pub fn slot(self) -> usize {
        match self {
            Answer::Left => 0,
            Answer::Right => 1,
            Answer::Cancel => 2,
        }
    }
}

/// One box instruction as read from the event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub instr: i32,
    /// EventTextForMap id.
    pub msg: i32,
    pub prompt: i16,
    pub buttons: i16,
    /// [left, right, cancel] (2007[10] only; 0 = none).
    pub flags: [u32; 3],
}

impl Request {
    /// The instruction's arguments (the game's layout: 16 bytes for 2007[1], 28 for 2007[10]). None: another instruction
    /// or too few bytes.
    pub fn parse(instr: i32, args: &[u8]) -> Option<Request> {
        let need = match instr {
            INSTR_MESSAGE => 16,
            INSTR_QUESTION => 28,
            _ => return None,
        };
        if args.len() < need {
            return None;
        }
        let i32_at = |o: usize| i32::from_le_bytes([args[o], args[o + 1], args[o + 2], args[o + 3]]);
        let i16_at = |o: usize| i16::from_le_bytes([args[o], args[o + 1]]);
        let flags = if instr == INSTR_QUESTION { [i32_at(16) as u32, i32_at(20) as u32, i32_at(24) as u32] } else { [0; 3] };
        Some(Request { instr, msg: i32_at(0), prompt: i16_at(4), buttons: i16_at(6), flags })
    }

    /// The event waits for the answer (2007[10]).
    pub fn blocking(&self) -> bool {
        self.instr == INSTR_QUESTION
    }

    /// The window's buttons: Yes / No for the game's two-button boxes, else OK.
    pub fn window_buttons(&self) -> Buttons {
        if self.buttons == 2 {
            Buttons::YesNo
        } else {
            Buttons::Ok
        }
    }

    /// The flag the game's waiter sets for `answer` (None: no flag, or a message).
    pub fn flag_for(&self, answer: Answer) -> Option<u32> {
        if !self.blocking() {
            return None;
        }
        let f = self.flags[answer.slot()];
        (f != 0).then_some(f)
    }
}

/// THE TEXT OF A BOX from the game's message: `<?gdsparam@N?>` becomes goods N's name (`goods`), any other placeholder
/// `...`; other angle brackets (KH text markup) become parentheses; the game's line breaks are kept (the window breaks
/// lines as the box did); trailing blanks trimmed.
pub fn dialog_text(raw: &str, goods: &dyn Fn(i32) -> Option<String>) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let Some(end) = tail.find('>') else {
            out.push('(');
            rest = &tail[1..];
            continue;
        };
        let inner = &tail[1..end];
        if let Some(body) = inner.strip_prefix('?') {
            let body = body.trim_end_matches('?');
            let named = body.strip_prefix("gdsparam@").and_then(|n| n.trim().parse::<i32>().ok()).and_then(goods);
            out.push_str(named.as_deref().unwrap_or("..."));
        } else {
            out.push('(');
            out.push_str(inner.trim());
            out.push(')');
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out.replace('\r', "").lines().map(str::trim_end).collect::<Vec<_>>().join("\n").trim().to_string()
}

/// What the dispatcher's detour does with a box instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// The game's own case runs (its own box).
    Game,
    /// Answered "handled" (1) without the game's case (a message of ours).
    Handled,
    /// The game's case runs with its open request suppressed (a question of ours: its waiter holds the event).
    Suppress,
}

/// What the waiter's detour does this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    /// Not ours: the game's update runs.
    Game,
    /// Ours, not answered: not done.
    Hold,
    /// Ours, answered: set this flag (if any) and be done.
    Done(Option<u32>),
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    /// Not given to the HUD yet.
    Queued,
    /// Up in (or waiting for) KH3's window.
    Asked,
    /// Answered, not yet taken by the waiter (2007[10]).
    Answered(Answer),
}

#[derive(Clone, Debug)]
struct Entry {
    id: u64,
    req: Request,
    text: String,
    /// The game's waiter of a question (0: not bound yet; always 0 for a message).
    waiter: usize,
    state: State,
    /// Frames (with the world running) since the waiter last asked (or since the question was made).
    idle: u32,
}

/// What [`Dialogs::step`] asks of the HUD (the kh-host `Hud` in the game; a mock in the tests).
pub trait DialogHud {
    /// Shows box `id`; false: it cannot.
    fn ask(&mut self, id: u64, text: &str, buttons: Buttons) -> bool;
    /// The answer to box `id`, once (true = A on Yes / OK).
    fn take(&mut self, id: u64) -> Option<bool>;
    /// Box `id` is up or waits.
    fn pending(&self, id: u64) -> bool;
    /// Takes box `id` off.
    fn cancel(&mut self, id: u64);
}

/// THE BOXES' STATE MACHINE (pure: no game, no HUD).
#[derive(Default)]
pub struct Dialogs {
    serial: u64,
    entries: Vec<Entry>,
    /// Log lines of the latest calls (taken by the frame).
    pub notes: Vec<String>,
}

impl Dialogs {
    /// THE DISPATCHER'S BOX INSTRUCTION `req` with its resolved text (None: none); `ready`: KH3's window can show it (the
    /// key, the HUD).
    pub fn instruction(&mut self, req: Request, text: Option<String>, ready: bool) -> Route {
        let Some(text) = text.filter(|t| !t.trim().is_empty()) else {
            self.notes.push(format!("2007[{}] msg {}: no text: Elden Ring's own box", req.instr, req.msg));
            return Route::Game;
        };
        if !ready {
            self.notes.push(format!("2007[{}] msg {}: KH3's window not available: Elden Ring's own box", req.instr, req.msg));
            return Route::Game;
        }
        self.serial += 1;
        let id = self.serial;
        self.notes.push(format!("box #{id}: 2007[{}] msg {} (prompt {}, buttons {} -> {:?}{}) as KH3's window: \"{}\"", req.instr, req.msg, req.prompt, req.buttons, req.window_buttons(), if req.blocking() { format!(", flags {:?}; the game's waiter holds the event", req.flags) } else { "; the event goes on".to_string() }, text.replace('\n', " / ")));
        self.entries.push(Entry { id, req, text, waiter: 0, state: State::Queued, idle: 0 });
        if req.blocking() {
            Route::Suppress
        } else {
            Route::Handled
        }
    }

    /// THE WAITER'S FRAME UPDATE: `waiter` with the msg and flags it holds. A waiter not bound yet is bound to the oldest
    /// question of ours with the same msg and flags that has none.
    pub fn waiter(&mut self, waiter: usize, msg: i32, flags: [u32; 3]) -> Wait {
        let at = match self.entries.iter().position(|e| e.waiter == waiter && waiter != 0) {
            Some(at) => at,
            None => {
                let Some(at) = self.entries.iter().position(|e| e.waiter == 0 && e.req.blocking() && e.req.msg == msg && e.req.flags == flags) else { return Wait::Game };
                self.entries[at].waiter = waiter;
                self.notes.push(format!("box #{}: the game's waiter {waiter:#x} is bound (msg {msg}, flags {flags:?})", self.entries[at].id));
                at
            }
        };
        let e = &mut self.entries[at];
        e.idle = 0;
        let State::Answered(a) = e.state else { return Wait::Hold };
        let (id, flag) = (e.id, e.req.flag_for(a));
        self.entries.remove(at);
        self.notes.push(format!("box #{id} (msg {msg}): the waiter takes the answer {a:?}{}; the event goes on", flag.map_or(" (no flag)".to_string(), |f| format!(": flag {f} ON"))));
        Wait::Done(flag)
    }

    fn drop_entry(&mut self, id: u64, why: &str) {
        if let Some(at) = self.entries.iter().position(|e| e.id == id) {
            let e = self.entries.remove(at);
            self.notes.push(format!("box #{id} (2007[{}] msg {}) dropped: {why}", e.req.instr, e.req.msg));
        }
    }

    /// ONE FRAME (the game's thread): queued boxes go to the HUD, answers come back, questions whose waiter stopped asking
    /// go. `world_runs`: no KH window holds the world (only then does a silent waiter count as gone).
    pub fn step(&mut self, hud: &mut dyn DialogHud, world_runs: bool) {
        let mut gone: Vec<(u64, &'static str)> = Vec::new();
        for e in self.entries.iter_mut() {
            match e.state {
                State::Queued => {
                    if hud.ask(e.id, &e.text, e.req.window_buttons()) {
                        e.state = State::Asked;
                    } else if e.req.blocking() {
                        e.state = State::Answered(Answer::Cancel);
                        self.notes.push(format!("box #{} could not be shown: its waiter gets the cancel answer", e.id));
                    } else {
                        gone.push((e.id, "it could not be shown"));
                    }
                }
                State::Asked => match hud.take(e.id) {
                    Some(yes) if e.req.blocking() => e.state = State::Answered(if yes { Answer::Left } else { Answer::Right }),
                    Some(_) => gone.push((e.id, "acknowledged")),
                    None if !hud.pending(e.id) => {
                        // the HUD let it go without an answer (a load, the HUD reset): as the game's box taken away
                        if e.req.blocking() {
                            e.state = State::Answered(Answer::Cancel);
                        } else {
                            gone.push((e.id, "taken off by the HUD"));
                        }
                    }
                    None => {}
                },
                State::Answered(_) => {}
            }
            if e.req.blocking() && world_runs {
                e.idle += 1;
                if e.idle > ABANDON_FRAMES {
                    hud.cancel(e.id);
                    gone.push((e.id, "its waiter stopped asking (its event ended)"));
                }
            }
        }
        for (id, why) in gone {
            self.drop_entry(id, why);
        }
    }

    /// A load / the host going away: every box goes (the HUD's are reset by the HUD itself).
    pub fn clear(&mut self) {
        if !self.entries.is_empty() {
            self.notes.push(format!("{} box(es) dropped (reset)", self.entries.len()));
        }
        self.entries.clear();
    }

    /// Boxes known (queued, up or answered).
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

static DIALOGS: Mutex<Option<Dialogs>> = Mutex::new(None);
/// The rule is on and KH3's window can show boxes (set every frame by [`frame`]).
static READY: AtomicBool = AtomicBool::new(false);
static TOLD_READY: AtomicU32 = AtomicU32::new(u32::MAX);
/// Both detours are in (without them no question is taken: the game's box).
static HOOKED: AtomicBool = AtomicBool::new(false);
/// The open request answers {0, 0} (set around the game's case 10 for a question of ours, the game's thread).
static SUPPRESS: AtomicBool = AtomicBool::new(false);
static OPEN_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static WAITER_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
/// Open requests answered {0, 0} (for the log).
static SUPPRESSED: AtomicU32 = AtomicU32::new(0);

fn with_dialogs<R>(f: impl FnOnce(&mut Dialogs) -> R) -> R {
    let mut guard = DIALOGS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(guard.get_or_insert_with(Dialogs::default))
}

/// The instruction's index and pointer; None when unreadable. Read only through the system's copy (probe.rs).
fn instruction(ctx: usize) -> Option<(i32, usize)> {
    let ins = crate::probe::word(ctx + 0xD0)?;
    Some((crate::probe::u32_at(ins + 4)? as i32, ins))
}

/// The instruction's first `n` argument bytes (the dispatcher's own reading, as dragonmsg.rs).
fn arguments(ctx: usize, ins: usize, n: usize) -> Option<Vec<u8>> {
    let args = match crate::probe::word(ctx + 0xD8).filter(|a| *a != 0) {
        Some(a) => a,
        None => {
            // 0x140CE2340: [[ctx + 0xC8] + 8] + [that + 0x78] + the instruction's argument offset
            let base = crate::probe::word(crate::probe::word(ctx + 0xC8)? + 8).filter(|b| *b != 0)?;
            let rel = crate::probe::word(base + 0x78).filter(|r| *r != usize::MAX)?;
            base.checked_add(rel)?.checked_add(crate::probe::word(ins + 0x10)?)?
        }
    };
    let mut out = Vec::with_capacity(n);
    for o in (0..n).step_by(4) {
        out.extend_from_slice(&crate::probe::u32_at(args + o)?.to_le_bytes());
    }
    Some(out)
}

/// The box's text in the game's language (EventTextForMap 34, the DLC's 367 / 467; goods names GoodsName 10 / 319 / 419).
fn text_of(msg: i32) -> Option<String> {
    let raw = crate::talkkh::game::msg(&[34, 367, 467], msg)?;
    let goods = |id: i32| crate::talkkh::game::msg(&[10, 319, 419], id).map(|t| t.trim().to_string());
    Some(dialog_text(&raw, &goods))
}

/// (dragonmsg.rs's detour of the bank-2007 dispatcher, the game's thread) THE BOX INSTRUCTIONS' ROUTE. Never panics (a
/// fault of ours gives the game its box).
pub fn on_instruction(ctx: usize) -> Route {
    let Some((index, ins)) = instruction(ctx) else { return Route::Game };
    if index != INSTR_MESSAGE && index != INSTR_QUESTION {
        return Route::Game;
    }
    if !READY.load(Ordering::Relaxed) || (index == INSTR_QUESTION && !HOOKED.load(Ordering::Relaxed)) {
        return Route::Game;
    }
    std::panic::catch_unwind(|| {
        let need = if index == INSTR_QUESTION { 28 } else { 16 };
        let Some(req) = arguments(ctx, ins, need).and_then(|a| Request::parse(index, &a)) else { return Route::Game };
        let text = text_of(req.msg);
        with_dialogs(|d| d.instruction(req, text, true))
    })
    .unwrap_or(Route::Game)
}

/// Runs `f` (the game's case 10) with the open request suppressed.
pub fn suppressed<R>(f: impl FnOnce() -> R) -> R {
    SUPPRESS.store(true, Ordering::Relaxed);
    let r = f();
    SUPPRESS.store(false, Ordering::Relaxed);
    r
}

type OpenFn = unsafe extern "C" fn(usize, usize, usize, usize) -> usize;
type WaiterFn = unsafe extern "C" fn(usize, usize, usize, usize) -> usize;

/// The open request: {0, 0} while [`suppressed`] (the request's own answer without a dialog manager), else the game's.
unsafe extern "C" fn open_hook(out: usize, msg: usize, prompt: usize, buttons: usize) -> usize {
    if SUPPRESS.load(Ordering::Relaxed) && out != 0 {
        // SAFETY: `out` is the caller's 16-byte result slot (the request writes two words there on every path).
        (out as *mut u64).write_unaligned(0);
        ((out + 8) as *mut u64).write_unaligned(0);
        SUPPRESSED.fetch_add(1, Ordering::Relaxed);
        return out;
    }
    let original = OPEN_ORIGINAL.load(Ordering::Relaxed);
    if original == 0 {
        return out;
    }
    // SAFETY: the trampoline of the game's own function; the four argument registers passed through unchanged.
    let f: OpenFn = std::mem::transmute(original);
    f(out, msg, prompt, buttons)
}

/// The waiter's frame update: a waiter of ours waits for KH3's answer; the others are the game's.
unsafe extern "C" fn waiter_hook(this: usize, rdx: usize, r8: usize, r9: usize) -> usize {
    let mine = std::panic::catch_unwind(|| {
        let msg = crate::probe::u32_at(this + WAITER_MSG)? as i32;
        let flags = [crate::probe::u32_at(this + WAITER_FLAGS)?, crate::probe::u32_at(this + WAITER_FLAGS + 4)?, crate::probe::u32_at(this + WAITER_FLAGS + 8)?];
        Some(with_dialogs(|d| d.waiter(this, msg, flags)))
    })
    .ok()
    .flatten()
    .unwrap_or(Wait::Game);
    match mine {
        Wait::Game => {}
        Wait::Hold => {
            // SAFETY: the live waiter the game passes; its done byte (+0x18 bit 0, written so by the game's own update).
            *((this + WAITER_DONE) as *mut u8) &= !1;
            return 0;
        }
        Wait::Done(flag) => {
            if let Some(f) = flag {
                crate::loot::set_event_flag(f);
            }
            // SAFETY: as above: done (the event continues).
            *((this + WAITER_DONE) as *mut u8) |= 1;
            return 0;
        }
    }
    let original = WAITER_ORIGINAL.load(Ordering::Relaxed);
    if original == 0 {
        return 0;
    }
    // SAFETY: the trampoline of the game's own function; the four argument registers passed through unchanged.
    let f: WaiterFn = std::mem::transmute(original);
    f(this, rdx, r8, r9)
}

/// (dragonmsg.rs's install) Detours the open request and the waiter's update (both or neither for the questions; the
/// messages need neither).
pub fn install() {
    let [Some(open), Some(waiter)] = LOCATED.map(crate::havok::locate) else {
        crate::erlog!("erdialog: (ER-POPUPS) the box open request / the question waiter are not located: Elden Ring's own questions (its messages are still KH3's)");
        return;
    };
    // SAFETY: the functions' starts at their located addresses (pattern + recorded RVA); the hooks take their four register
    // arguments and pass them on unchanged (no stack arguments: both are called with rcx .. r9 only).
    let a = unsafe { kh_er_render::inline_hook(open, open_hook as *const () as usize, &OPEN_ORIGINAL, "generic dialog open request") };
    let b = unsafe { kh_er_render::inline_hook(waiter, waiter_hook as *const () as usize, &WAITER_ORIGINAL, "generic dialog waiter update") };
    match (a, b) {
        (Ok(()), Ok(())) => {
            HOOKED.store(true, Ordering::SeqCst);
            crate::erlog!("erdialog: (ER-POPUPS) the box open request and the question waiter are detoured: Elden Ring's event-script questions as KH3's window");
        }
        (a, b) => crate::erlog!("erdialog: (ER-POPUPS) NOT detoured (open request {a:?}, waiter {b:?}): Elden Ring's own questions (a detour that went in passes everything through)"),
    }
}

pub fn uninstalled() {
    HOOKED.store(false, Ordering::SeqCst);
    OPEN_ORIGINAL.store(0, Ordering::SeqCst);
    WAITER_ORIGINAL.store(0, Ordering::SeqCst);
}

/// The payload's shutdown: every new box is the game's again (a question already ours keeps its waiter's answer route).
pub fn release() {
    READY.store(false, Ordering::Relaxed);
}

/// The kh-host HUD as the boxes' window.
struct HudWindow<'a>(&'a mut kh_host::hud::Hud);

impl DialogHud for HudWindow<'_> {
    fn ask(&mut self, id: u64, text: &str, buttons: Buttons) -> bool {
        let b = match buttons {
            Buttons::YesNo => kh_host::hud::ErButtons::YesNo,
            Buttons::Ok => kh_host::hud::ErButtons::Ok,
        };
        self.0.ask_er_dialog(id, text, b)
    }
    fn take(&mut self, id: u64) -> Option<bool> {
        self.0.take_er_dialog_answer(id)
    }
    fn pending(&self, id: u64) -> bool {
        self.0.er_dialog_pending().contains(&id)
    }
    fn cancel(&mut self, id: u64) {
        self.0.cancel_er_dialog(id);
    }
}

/// (dragonmsg.rs's frame, every frame, the game's thread) The key `er.dialog_kh` (default ON), the HUD's side, the log.
pub fn frame(control: &crate::control::Control) {
    let on = control.flag("er.dialog_kh") != Some(false);
    let (ready, notes) = with_dialogs(|dialogs| {
        let available = crate::hud::with_hud(|hud| {
            let available = hud.er_dialog_available();
            let world_runs = !hud.paused();
            dialogs.step(&mut HudWindow(hud), world_runs);
            available
        });
        if available.is_none() {
            dialogs.clear();
        }
        (on && available == Some(true), std::mem::take(&mut dialogs.notes))
    });
    READY.store(ready, Ordering::Relaxed);
    if TOLD_READY.swap(u32::from(ready), Ordering::Relaxed) != u32::from(ready) {
        let questions = if HOOKED.load(Ordering::Relaxed) { "2007[10] questions too" } else { "NOT its 2007[10] questions (detours missing)" };
        crate::erlog!("erdialog: (ER-POPUPS) Elden Ring's event-script boxes {}", if ready { format!("as KH3's Dialog window (2007[1] messages; {questions})") } else if !on { "are Elden Ring's own (er.dialog_kh = 0)".to_string() } else { "are Elden Ring's own (no KH HUD / Dialog movie)".to_string() });
    }
    for n in notes {
        crate::erlog!("erdialog: (ER-POPUPS) {n}");
    }
    let s = SUPPRESSED.swap(0, Ordering::Relaxed);
    if s > 0 {
        crate::erlog!("erdialog: (ER-POPUPS) {s} open request(s) of Elden Ring's box answered {{0, 0}} (its box not made)");
    }
}

#[cfg(test)]
#[path = "erdialog_tests.rs"]
mod tests;
