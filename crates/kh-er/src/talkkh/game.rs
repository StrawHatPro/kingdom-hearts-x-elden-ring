//! (CV2) THE ADAPTER'S FRAME (sora.rs, once a frame, after the HUD's and CV1's frames): the talk scripts' state and
//! the game's lines (hooks.rs) -> the conversation rules (rules.rs) -> CV1's conversation host (`crate::conv`): begin,
//! one pushed `Line` per game line, one external `Choice` per game menu, the game's voice reported, the runner's
//! `StopVoice` -> the game's skip, its `Chosen` -> the script's answer, the end with the talk. UNTESTED IN THE GAME.

use std::sync::atomic::Ordering;
use std::sync::Mutex;

use eldenring::cs::FieldInsHandle;
use fromsoftware_shared::FromStatic;

use super::hooks::{self, with_shared, INS_HANDLE, SKIP_INS, SUPPRESS};
use super::rules::{bubble_text, AmbientAct, AmbientBook, Ask, Bubble, Conversation, Dismissal, FrameIn, LineSnap};
use super::steps;
use crate::control::Control;
use crate::conv::{self, Begin, Body, CombatPolicy, ConvEvent, ConvId, Participant};
use crate::erlog;
use crate::game::plausible;

/// The owners' names in CV1's host (its logs, `conv::running`): the held conversation, the ambient ones.
pub const OWNER: &str = "talk";
pub const OWNER_AMBIENT: &str = "talk-ambient";

/// What sora.rs gives each frame.
#[derive(Clone, Debug, Default)]
pub struct TalkCtx {
    /// A real-time cutscene plays (its subtitles are its own: nothing is taken over).
    pub cutscene: bool,
    /// The leader is in a battle (an NPC's lines are then Elden Ring's own subtitles; CV1 ends a conversation in a fight).
    pub in_battle: bool,
    /// One of Elden Ring's own windows is up (the popup menu has a top job: a shop, the level up, a dialog of its own
    /// ...): the conversation of ours ends (it begins again with the next line / choice), the pad is the game's.
    pub er_menu: bool,
    /// (CV3) A driven story rest holds the pad (savepoint.rs, IMPL_ER36: Melina's talks at a grace): her talk is the
    /// player's conversation whatever its talk camera says (its lines are held bubbles that A advances, not ambient).
    pub story_talk: bool,
    /// (CV3) The leader's name (the conversation's Leader actor: before, empty).
    pub leader_name: String,
    pub dt: f32,
}

/// The step the runner shows, by its index in the conversation.
#[derive(Clone, Copy, Debug, PartialEq)]
enum StepRef {
    Line { ins: usize, row: i32 },
    Ask { serial: u32 },
}

struct Run {
    id: ConvId,
    talk_id: i32,
    ins: usize,
    steps: Vec<StepRef>,
    /// The runner's step now (its `Line` / `ChoiceUp` events).
    current: Option<usize>,
    pushed_line: Option<(usize, i32)>,
    pushed_ask: Option<u32>,
}

/// (CV2-b) An ambient conversation of a talk object (running or queued in CV1's host).
struct ARun {
    id: ConvId,
    ins: usize,
    talk_id: i32,
    /// The rows pushed, in step order.
    rows: Vec<i32>,
    current: Option<usize>,
}

#[derive(Default)]
struct Adapter {
    book: AmbientBook,
    ambient: Vec<ARun>,
    run: Option<Run>,
    conversation: Conversation,
    /// Seconds a pending choice has waited without a conversation of ours to show it.
    unshown: f32,
    /// The latest refusal of CV1's host (logged once).
    refused: Option<String>,
    was_enabled: bool,
    /// (CV3) The talk the leader walked away from (no held conversation of ours for it again while it goes on).
    dismissed: Option<Dismissal>,
}

static ADAPTER: Mutex<Option<Adapter>> = Mutex::new(None);
static SKIP_IDS: Mutex<Vec<i32>> = Mutex::new(Vec::new());
static MASK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (sora.rs) A conversation of ours runs: Elden Ring sees nothing of the pad (CV1 holds the leader; the bubble has A /
/// B / the D-pad; the game's own line skip comes from the adapter only).
pub fn masks_game_pad() -> bool {
    MASK.load(Ordering::Relaxed)
}

/// `talk.kh.skip`.
pub(crate) fn skip_ids() -> Vec<i32> {
    SKIP_IDS.try_lock().map(|s| s.clone()).unwrap_or_default()
}

/// A message of the game's repository (the game's language), the first of `categories` that has it: TalkMsg 1 / 360,
/// EventTextForTalk 33 / 366, GR_Dialogues 204 / 372, GR_MenuText 200 / 368, NpcName 18 / 328 (the msgbnd's file ids,
/// READ in the game's menu / item msgbnd copies).
pub(crate) fn msg(categories: &[u32], id: i32) -> Option<String> {
    if id <= 0 {
        return None;
    }
    // SAFETY: game thread; the singleton lives as long as the game; the SDK checks the file table's bounds.
    let msgs = unsafe { eldenring::cs::MsgRepository::instance() }.ok()?;
    for c in categories {
        if let Some(text) = msgs.get_msg(*c, id as u32) {
            let end = text.iter().position(|c| *c == 0).unwrap_or(text.len()).min(2000);
            let t = String::from_utf16_lossy(&text[..end]);
            if !t.trim().is_empty() && !t.contains("[ERROR]") {
                return Some(t);
            }
        }
    }
    None
}

/// The TalkParam row's voice id (the line's own; -1: not readable).
fn voice_of(row: i32) -> i32 {
    if row <= 0 {
        return -1;
    }
    // SAFETY: game thread; the singleton lives as long as the game; the SDK checks its holder table.
    let Ok(repo) = (unsafe { eldenring::cs::SoloParamRepository::instance() }) else { return -1 };
    repo.get::<eldenring::cs::TalkParam>(row as u32).map_or(-1, |r| r.voice_id())
}

/// The character of a talk object (its handle at + 0x40) and its name (NpcParam nameId -> NpcName).
pub(crate) fn npc_of(ins: usize) -> Option<(FieldInsHandle, String)> { // (SHOP) pub(crate): the merchant's name in the KH shop
    let mut b = [0u8; 8];
    if !plausible(ins) || !crate::probe::read(ins + INS_HANDLE, &mut b) {
        return None;
    }
    // SAFETY: FieldInsHandle is repr(C) of two 4-byte fields (game::handle_bits reads it the other way).
    let handle: FieldInsHandle = unsafe { std::mem::transmute_copy::<[u8; 8], FieldInsHandle>(&b) };
    if handle.is_empty() {
        return None;
    }
    let chr = crate::targets::find_chr(&handle)?;
    let param = chr.npc_param_id;
    // SAFETY: game thread; the singleton lives as long as the game; the SDK checks its holder table.
    let name_id = unsafe { eldenring::cs::SoloParamRepository::instance() }.ok().and_then(|r| r.get::<eldenring::cs::NpcParam>(param as u32).map(|n| n.name_id())).unwrap_or(-1);
    let name = msg(&[18, 328], name_id).map(|n| n.trim().to_string()).unwrap_or_default();
    Some((handle, name))
}

/// ONCE A FRAME.
pub fn frame(control: &Control, ctx: TalkCtx) {
    let enabled = control.flag("talk.kh") != Some(false) && hooks::COMPLETE.load(Ordering::Relaxed);
    let log = control.on("talk.kh.log");
    if let Ok(mut s) = SKIP_IDS.try_lock() {
        *s = control.str("talk.kh.skip").map(|t| t.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
    }
    hooks::ENABLED.store(enabled || crate::shopkh::wants_talk_hooks(control), Ordering::Relaxed); // (SHOP) the hooks also run for the KH shop and the smiths
    let bubble = crate::hud::with_hud(|hud| hud.bubble_available()).unwrap_or(false);
    let other_conv = conv::running().filter(|(_, owner)| *owner != OWNER && *owner != OWNER_AMBIENT);
    let ready = enabled && !ctx.cutscene && !ctx.er_menu && bubble && control.flag("conv.on") != Some(false) && other_conv.is_none();

    let mut guard = ADAPTER.lock().unwrap_or_else(|e| e.into_inner());
    let a = guard.get_or_insert_with(Adapter::default);
    if a.was_enabled != enabled {
        a.was_enabled = enabled;
        erlog!("talk: {}", if enabled { "Elden Ring's character conversations are KH bubbles (talk.kh)" } else if hooks::COMPLETE.load(Ordering::Relaxed) { "talk.kh = 0: Elden Ring's own subtitles, menus and dialogs" } else { "the talk functions are not detoured: Elden Ring's own subtitles, menus and dialogs" });
    }
    // ---- the shared state (the hooks') of this frame
    let skip = skip_ids();
    let facts = with_shared(|s| {
        s.talks.enabled = enabled;
        s.talks.ready = ready;
        s.talks.skip_ids = skip.clone();
        if !enabled {
            s.talks.clear();
            s.lines.clear();
        }
        for l in s.lines.iter_mut() {
            l.age = l.age.saturating_add(1);
        }
        s.lines.retain(|l| l.age < 600);
        (s.lines.clone(), s.talks.ask.clone(), s.talks.camera_talks().to_vec(), std::mem::take(&mut s.talks.notes))
    });
    let Some((lines, ask, camera_talks, notes)) = facts else { return };
    for n in notes {
        erlog!("talk: {n}");
    }
    // the ages were counted up after the hooks' writes of this frame: a line updated this frame reads 1
    let lines: Vec<LineSnap> = lines.into_iter().map(|mut l| {
        l.age = l.age.saturating_sub(1);
        l
    }).collect();
    // (CV3) her story talk counts as engaged (her lines are the player's conversation, also with no talk camera)
    // (CV3) her story talk counts as engaged while she speaks or asks (no talk camera there); for the ambient book its
    // lines are held ones
    let story_talk = ctx.story_talk.then_some(crate::talkdrive::TALK_MELINA);
    let fin = FrameIn { lines: lines.clone(), ask: ask.as_ref().map(|q| (q.serial, q.talk_id)), camera_talks: camera_talks.clone(), in_battle: ctx.in_battle && !ctx.story_talk /* (CV3) a story rest: the body rests, no battle line */, ready, story_talk, dt: ctx.dt };
    let camera_talks = super::rules::engaged_talks(camera_talks, story_talk);
    let out = a.conversation.step(&fin);
    if let Some(n) = &out.note {
        erlog!("talk: {n}");
    }

    // ---- (CV3) the talk the leader walked away from: forgotten when it is over
    if let Some(d) = a.dismissed.as_mut() {
        if !d.step(&lines, &camera_talks, ask.as_ref().map(|q| q.talk_id), ctx.dt) {
            erlog!("talk: talk {} (walked away from) is over: talking to its character again begins a new conversation", d.talk_id);
            a.dismissed = None;
        }
    }
    let dismissed_ins = a.dismissed.map(|d| d.ins);
    // ---- the conversation of ours: begin it when the player is engaged (not for the talk he walked away from)
    let engaged_ins = match out.bubble {
        Some(Bubble::Line { ins, .. }) if out.engaged => Some(ins),
        Some(Bubble::Ask { .. }) => ask.as_ref().map(|q| q.ins),
        _ => None,
    }
    .filter(|ins| Some(*ins) != dismissed_ins);
    // another NPC's engaged talk: this conversation ends, his begins
    if let (Some(run), Some(ins)) = (a.run.as_ref(), engaged_ins) {
        if run.ins != ins && plausible(ins) {
            let id = run.id;
            erlog!("talk: another character's conversation ({}): #{id} ends, his begins", lines.iter().find(|l| l.ins == ins).map_or(0, |l| l.talk_id));
            conv::end(id, "another character's conversation");
            a.run = None;
        }
    }
    if a.run.is_none() && ready {
        if let Some(ins) = engaged_ins {
            let talk_id = lines.iter().find(|l| l.ins == ins).map(|l| l.talk_id).or(ask.as_ref().filter(|q| q.ins == ins).map(|q| q.talk_id)).unwrap_or(0);
            match npc_of(ins) {
                Some((handle, name)) => {
                    let [leader, npc] = steps::actors(&ctx.leader_name, &name); // (CV3) the leader's name (was empty)
                    let parts = vec![Participant { actor: leader, body: Body::Leader, height_cm: None }, Participant { actor: npc, body: Body::Chr(handle), height_cm: None }];
                    // (CV3) the defaults: hands-free in a fight (A attacks, lines go on, a choice waits), the player free,
                    // no conversation camera unless `conv.camera = 1`
                    let b = Begin { open_ended: true, policy: CombatPolicy::HandsFree, camera: true, hold_party: true, ..Begin::new(OWNER, parts, Vec::new()) };
                    match conv::begin(b) {
                        Ok(id) => {
                            a.refused = None;
                            erlog!("talk: #{id} BEGINS: {} (talk {talk_id}){}", if name.is_empty() { "(no name)" } else { &name }, if ask.is_some() { ", with a choice" } else { "" });
                            a.run = Some(Run { id, talk_id, ins, steps: Vec::new(), current: None, pushed_line: None, pushed_ask: None });
                        }
                        Err(e) => {
                            if a.refused.as_deref() != Some(e.as_str()) {
                                erlog!("talk: the conversation of talk {talk_id} was refused ({e}): Elden Ring's own subtitles and menus");
                                a.refused = Some(e);
                            }
                        }
                    }
                }
                None => {
                    if a.refused.as_deref() != Some("no character") {
                        erlog!("talk: talk {talk_id}: its character is not found (handle at + 0x40): Elden Ring's own subtitles and menus");
                        a.refused = Some("no character".into());
                    }
                }
            }
        }
    }

    // ---- feed it
    let mut answers = Vec::new();
    let mut ended_by_host = false;
    let mut dismiss: Option<Dismissal> = None;
    if let Some(run) = a.run.as_mut() {
        // a new line of its NPC = a new Line step (one subtitle line = one bubble)
        if let Some(Bubble::Line { ins, row, msg: m, .. }) = out.bubble {
            if ins == run.ins && run.pushed_line != Some((ins, row)) {
                run.pushed_line = Some((ins, row));
                let snap = lines.iter().find(|l| l.ins == ins);
                match msg(&[1, 360], m).map(|t| bubble_text(&t)).filter(|t| !t.is_empty()) {
                    Some(text) => {
                        let voice = voice_of(row);
                        if conv::push(run.id, steps::line(&text, row, voice, snap.map_or(-1.0, |l| l.duration))) {
                            run.steps.push(StepRef::Line { ins, row });
                            if log {
                                erlog!("talk: #{} line [{row}] voice {voice}: {text}", run.id);
                            }
                        }
                    }
                    None => erlog!("talk: #{} line [{row}]: TalkMsg {m} has no text: no bubble for it", run.id),
                }
            }
        }
        // a new choice = an external Choice step
        if let Some(q) = ask.as_ref().filter(|q| q.ins == run.ins || q.talk_id == run.talk_id) {
            if run.pushed_ask != Some(q.serial) {
                run.pushed_ask = Some(q.serial);
                if conv::push(run.id, steps::choice(q)) {
                    run.steps.push(StepRef::Ask { serial: q.serial });
                }
            }
        }
        // the runner's events
        for e in conv::take_events(run.id) {
            match e {
                ConvEvent::Line { index, .. } | ConvEvent::ChoiceUp { index } => run.current = Some(index),
                ConvEvent::StopVoice { .. } => {
                    if let Some(StepRef::Line { ins, row }) = run.current.and_then(|k| run.steps.get(k).copied()) {
                        let still = lines.iter().any(|l| l.ins == ins && l.row == row && l.active && !l.unskippable);
                        if still {
                            SKIP_INS.store(ins, Ordering::Relaxed);
                        }
                        if log {
                            erlog!("talk: #{} A before the voice ended: {}", run.id, if still { "the game's own skip for this line" } else { "the line refuses the game's skip (or is over)" });
                        }
                    }
                }
                ConvEvent::Chosen { index, option } => {
                    if let Some(StepRef::Ask { serial }) = run.steps.get(index).copied() {
                        answers.push((serial, option));
                    }
                }
                ConvEvent::Direct(d) => conv::directive_done(run.id, d.actor()), // (the game's NPC stays where its talk keeps it)
                ConvEvent::Ended(why) => {
                    erlog!("talk: #{} ended by the conversation host ({why:?})", run.id);
                    ended_by_host = true;
                    // (CV3) walked away (or a fight under the End policy): the game's talk may go on: no new held
                    // conversation for it (the begin / end loop of the user's session), its lines ambient, its choice "Leave"
                    if matches!(why, crate::conv::EndReason::WalkedAway | crate::conv::EndReason::Combat) {
                        dismiss = Some(Dismissal::new(run.ins, run.talk_id));
                    }
                }
                _ => {}
            }
        }
        // the game's voice of the line the runner shows
        let playing = match run.current.and_then(|k| run.steps.get(k).copied()) {
            Some(StepRef::Line { ins, row }) => lines.iter().any(|l| l.ins == ins && l.row == row && l.active && l.age <= 2 && l.elapsed < l.duration),
            _ => false,
        };
        if !ended_by_host {
            conv::voice_playing(run.id, Some(playing));
        }
    }
    if ended_by_host {
        a.run = None;
        if let Some(d) = dismiss {
            erlog!("talk: talk {}: the leader walked away: no new conversation for it while it goes on; its lines are ambient bubbles, a choice of it is answered \"leave\" (CV3)", d.talk_id);
            a.dismissed = Some(d);
        }
    }
    // (CV3) a choice of the talk walked away from: its cancel option (Leave / No), as the player has left
    if let (Some(d), Some(q)) = (a.dismissed, ask.as_ref()) {
        if q.ins == d.ins || q.talk_id == d.talk_id {
            let option = q.cancel.unwrap_or(q.options.len().saturating_sub(1));
            erlog!("talk: talk {}: its {:?} is answered with option {option} (the leader walked away)", q.talk_id, q.kind);
            answers.push((q.serial, option));
        }
    }
    // ---- the end: the talk is over (no line, no choice, not engaged), or the bubble cannot show
    if let Some(run) = a.run.as_ref() {
        if out.ended || !ready || (out.bubble.is_none() && !out.engaged) {
            erlog!("talk: #{} ends ({})", run.id, if ctx.er_menu { "one of Elden Ring's own windows opened" } else if !ready { "the bubble cannot show any more" } else { "the Elden Ring talk ended" });
            conv::end(run.id, "the Elden Ring talk ended");
            a.run = None;
        }
    }
    // ---- (CV2-b) the ambient lines: proximity lines, hostile taunts, battle lines (C.5)
    ambient(a, &lines, &camera_talks, &ctx, ready, log);
    // Elden Ring's own subtitle of a character's talk is not posted while the bubble can show (held, ambient, or an
    // ambient line CV1 dropped for a held conversation: one bubble on screen)
    SUPPRESS.store(ready, Ordering::Relaxed);
    MASK.store(a.run.is_some(), Ordering::Relaxed);
    // ---- the answers, and a choice no conversation of ours shows: given back to Elden Ring's own menu
    let pending_unshown = ask.as_ref().is_some_and(|q| a.run.as_ref().is_none_or(|r| r.pushed_ask != Some(q.serial)));
    a.unshown = if pending_unshown { a.unshown + ctx.dt } else { 0.0 };
    let give_back = a.unshown >= 0.25;
    drop(guard);
    let back: Option<Ask> = with_shared(|s| {
        for (serial, option) in &answers {
            s.talks.answer(*serial, *option);
        }
        if give_back {
            s.events.clear();
            s.talks.give_back()
        } else {
            None
        }
    })
    .flatten();
    if let Some(q) = back {
        let ran = hooks::run_for_the_game(q.event_obj, q.cmd, &q.args);
        erlog!("talk: talk {}: its {:?} {} to Elden Ring's own menu", q.talk_id, q.kind, if ran { "is given back" } else { "could NOT be given back (no command function)" });
    }
}

/// (CV2-b) THE AMBIENT LINES (C.5): an ambient conversation per talk object that speaks outside the held conversation
/// (nobody held, no camera, no A; a fight does not end it; CV1 queues it behind another ambient one and refuses it while
/// a held one runs = the line is dropped), one Line step per subtitle line, the game's voice reported.
fn ambient(a: &mut Adapter, lines: &[LineSnap], camera_talks: &[i32], ctx: &TalkCtx, ready: bool, log: bool) {
    let held_ins = a.run.as_ref().map(|r| r.ins);
    let acts = if ready {
        a.book.step_with(lines, camera_talks, ctx.in_battle && !ctx.story_talk, held_ins, a.dismissed.map(|d| d.ins), ctx.dt) // (CV3)
    } else {
        a.book.clear();
        a.ambient.iter().map(|r| AmbientAct::End { ins: r.ins }).collect()
    };
    for act in acts {
        match act {
            AmbientAct::Begin { ins, talk_id } => {
                // (ZETTA 2026-10-23, zboss.rs) no bubble of a character Zettaflare killed (its lines are skipped the game's way)
                if crate::zboss::ins_known_silenced(ins) {
                    continue;
                }
                let Some((handle, name)) = npc_of(ins) else {
                    if log {
                        erlog!("talk: ambient line of talk {talk_id}: its character is not found: Elden Ring's own subtitle");
                    }
                    continue;
                };
                let npc = steps::actors("", &name)[1].clone();
                let mut b = Begin::ambient(OWNER_AMBIENT, vec![Participant { actor: npc, body: Body::Chr(handle), height_cm: None }], Vec::new());
                b.open_ended = true;
                match conv::begin(b) {
                    Ok(id) => {
                        if log {
                            erlog!("talk: ambient #{id}: {} (talk {talk_id}){}", if name.is_empty() { "(no name)" } else { &name }, if ctx.in_battle { " in a battle" } else { "" });
                        }
                        a.ambient.push(ARun { id, ins, talk_id, rows: Vec::new(), current: None });
                    }
                    Err(e) => {
                        if log {
                            erlog!("talk: ambient line of talk {talk_id} dropped ({e})");
                        }
                    }
                }
            }
            AmbientAct::Line { ins, row, msg: m } => {
                let Some(ar) = a.ambient.iter_mut().find(|r| r.ins == ins) else { continue }; // (dropped by CV1's priority)
                let Some(text) = msg(&[1, 360], m).map(|t| bubble_text(&t)).filter(|t| !t.is_empty()) else { continue };
                let length = lines.iter().find(|l| l.ins == ins).map_or(-1.0, |l| l.duration);
                if conv::push(ar.id, steps::ambient_line(&text, row, voice_of(row), length)) {
                    ar.rows.push(row);
                    if log {
                        erlog!("talk: ambient #{} line [{row}]: {text}", ar.id);
                    }
                }
            }
            AmbientAct::End { ins } => {
                if let Some(k) = a.ambient.iter().position(|r| r.ins == ins) {
                    let r = a.ambient.remove(k);
                    conv::end(r.id, "the ambient lines are over");
                }
            }
        }
    }
    // their events and voices
    let mut gone = Vec::new();
    for ar in a.ambient.iter_mut() {
        for e in conv::take_events(ar.id) {
            match e {
                ConvEvent::Line { index, .. } => ar.current = Some(index),
                ConvEvent::Ended(why) => {
                    if log {
                        erlog!("talk: ambient #{} (talk {}) ended ({why:?})", ar.id, ar.talk_id);
                    }
                    gone.push(ar.ins);
                }
                _ => {}
            }
        }
        let playing = ar.current.and_then(|k| ar.rows.get(k)).is_some_and(|row| lines.iter().any(|l| l.ins == ar.ins && l.row == *row && l.active && l.age <= 2 && l.elapsed < l.duration));
        conv::voice_playing(ar.id, Some(playing));
    }
    for ins in gone {
        a.ambient.retain(|r| r.ins != ins);
        a.book.forget(ins);
    }
}
