//! The situation-command queue (the Triangle slot): combat strand F section 11
//! (`build/sora_tmp/combat/P2_F_section.md`), with the container details of
//! `docs/sora-magic-spec.md` J 5 and the display contract of `docs/sora-ui-spec.md` 5.6 - 5.8.
//!
//! In the game the queue lives in the HUD part `TresUIP_HudCommand` (list hud+0x760, slot A
//! hud+0x770, slot B hud+0x7C8) and the controller only enqueues, deletes and executes. Here it
//! is part of the simulation (plain data on the pawn, `PawnCombat::situation`), ticked in the UI
//! phase of the step (after the state tick, like the attack request), and published through the
//! UI seam (`UiSnapshot::situation`) and as events.
//!
//! | game | here |
//! |---|---|
//! | `NotifySetActionCommand(kind, time, info)` ctrl slot 687 0x14140DA40 | [`enqueue`] |
//! | `NotifyDeleteActionCommand(kind)` ctrl slot 688 0x14140C790 | [`delete`] |
//! | HUD insert 0x1416158F0 (capacity 6, eviction, start delay 0.5 s, appear 0.2 s) | `Queue::insert` |
//! | HUD timers 0x141630AF0 | [`tick`] |
//! | `Select()` 0x141627B70, L2 0x14162BAF0 | `Queue::select`, `Queue::cycle` |
//! | Triangle 0x1416220D0 -> `CanExecuteCommand` -> `ExecuteCommand` -> executor 0x14141B470 | [`use_selected`] -> [`execute`] |
//! | reaction prompt 0x1414145C0 / 0x141414350 -> ex-kind entries 0x141575640 | [`set_reaction`], [`clear_reaction`] |
//! | finish extras (StyleFinish slot 162 0x1413E3690 -> 0x14158C180) | [`set_extra`], [`clear_extra`] |
//!
//! FOR THE MAGIC IMPLEMENTER: grand magic is enqueued with `situation::enqueue(cx, kind, -1.0)`
//! (lifetime `Param0` 20 s x (1 + 0.2 MAGIC_TIME), cap 99.9) from your roll, which the gauge
//! calls through `style::roll_grand_magic` (one line to replace there); when the player uses such
//! an entry [`execute`] calls `grand_magic` below (one line to replace: `TryMagic(kind, grand = 1)`).
//!
//! Not here: attraction, team attack, link and context commands (no producer in a Sora-only
//! port); `BP_SetRestrictSituation`; the OVER_TIME refresh rule is implemented, its ability id
//! (0xAC) is the stats implementer's `OverTime`.

use super::tables::{command_row, P};
use crate::enums::{ETresAbilityKind, ETresCommandKind};
use crate::host::EventKind;
use crate::input::PadButton;
use crate::pawn::Ctx;
use crate::sora::Input;
use crate::states::StateData;

/// The class of a command (0x141619E60; sheet `sora-combat-queue-classes`).
pub mod class {
    pub const GRAND_MAGIC: u8 = 1;
    /// Reaction / context commands: slot A.
    pub const REACTION: u8 = 2;
    pub const FORMCHANGE: u8 = 3;
    /// Finish commands and their extras: slot B.
    pub const FINISH: u8 = 4;
    pub const ATTRACTION: u8 = 5;
    /// Ex kinds 5 / 9 / 13, AF_FINISH ...: slot B.
    pub const SINGLE_B: u8 = 6;
    pub const TEAM: u8 = 7;
    pub const SHOTLOCK: u8 = 8;
    pub const LINK: u8 = 10;
}

/// Why an entry left (`command_removed(kind, used | expired | evicted | deleted)`: F 11.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Removal {
    Used,
    Expired,
    Evicted,
    Deleted,
}

/// Where an entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    /// The counted list (hud+0x760).
    List,
    /// Slot A (hud+0x770): one context / reaction command.
    A,
    /// Slot B (hud+0x7C8): one finish command (or a reaction command of the single-slot classes).
    B,
}

/// One entry (0x58 bytes in the game: magic spec J 5.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    /// +0x00: 1 = a command kind, 2 = an "ex kind" (a reaction command of the controller's prompt).
    pub ex: bool,
    /// +0x04.
    pub class: u8,
    /// +0x08 (or the ex kind, +0x10): an `ETresCommandKind` value.
    pub kind: i32,
    /// +0x14: total time, -1 = untimed.
    pub total: f32,
    /// +0x18.
    pub remaining: f32,
    /// +0x1C: 0.5 s before the countdown starts.
    pub start_delay: f32,
    /// +0x20: 0.2 s before the entry can be selected.
    pub appear: f32,
    /// +0x34.
    pub serial: u32,
    /// +0x38.
    pub selectable: bool,
}

/// The queue. Plain data, part of the save state.
#[derive(Clone, Debug, PartialEq)]
pub struct Queue {
    pub list: Vec<Entry>,
    pub slot_a: Option<Entry>,
    pub slot_b: Option<Entry>,
    /// The finish command a reaction command pushed out of slot B (the stack at hud+0x820).
    slot_b_stack: Option<Entry>,
    /// A finish extra shown in slot B over the finish command (`extra(cmd)`: 175 - 178).
    pub extra: Option<i32>,
    /// hud+0x9CC.
    serial: u32,
    /// hud+0x9C4: 1 = "Situation Commands: Prioritize Older" (the default), 0 = "Newer".
    pub older_first: bool,
    /// hud+0x6FC / +0x954: the entry the player chose with L2 (its kind), while it exists.
    choice: Option<i32>,
    /// hud+0x6F4: the selected list index; -1 = the event button (slot A, else slot B); -2 = nothing.
    pub selected: i32,
    /// hud+0x751: the timers stand (`BP_NotifyPauseActionCommandTimer`).
    pub paused: bool,
    /// Entries used / expired / evicted / deleted so far (for tests and the debug text).
    pub removed: u32,
    /// Finish commands the AUTO_FINISH ability executed (for tests and the debug text).
    pub auto_finished: u32,
}

impl Default for Queue {
    fn default() -> Queue {
        Queue { list: Vec::new(), slot_a: None, slot_b: None, slot_b_stack: None, extra: None, serial: 0, older_first: true, choice: None, selected: -2, paused: false, removed: 0, auto_finished: 0 }
    }
}

/// The class of a command kind (0x141619E60: Type 1 -> 2; Type 4 by Cate).
pub fn class_of(kind: i32) -> u8 {
    match command_row(kind) {
        Some(row) => match (row.cmd_type, row.cate) {
            (4, 11) => class::GRAND_MAGIC,
            (4, 14) => class::FORMCHANGE,
            (4, 15) | (4, 16) => class::FINISH,
            (4, 17) => if kind == 192 { class::SINGLE_B } else { class::ATTRACTION },
            (4, 18) => if matches!(kind, 211 | 214 | 241) { class::SINGLE_B } else { class::TEAM },
            (3, 19) => class::LINK,
            (5, 25) => class::SHOTLOCK,
            _ => class::REACTION,
        },
        // Rows the sheet does not carry: the reaction range 1 - 26 and the context commands.
        None => class::REACTION,
    }
}

/// The class of an ex-kind entry (a reaction command): ex kinds 1 - 3 -> 5 and 5 / 9 / 13 -> 6
/// take slot B, the others are class 2 (slot A) (F 11.1, decompiled C).
fn ex_class(kind: i32) -> (u8, Slot) {
    match kind {
        1..=3 => (class::ATTRACTION, Slot::B),
        5 | 9 | 13 => (class::SINGLE_B, Slot::B),
        _ => (class::REACTION, Slot::A),
    }
}

impl Queue {
    /// The counted entries (hud+0x6F8).
    pub fn count(&self) -> usize {
        self.list.len()
    }

    pub fn find(&self, kind: i32) -> Option<(Slot, &Entry)> {
        if let Some(e) = self.list.iter().find(|e| e.kind == kind) {
            return Some((Slot::List, e));
        }
        if let Some(e) = self.slot_a.as_ref().filter(|e| e.kind == kind) {
            return Some((Slot::A, e));
        }
        self.slot_b.as_ref().filter(|e| e.kind == kind).map(|e| (Slot::B, e))
    }

    pub fn has(&self, kind: i32) -> bool {
        self.find(kind).is_some()
    }

    fn new_entry(&mut self, ex: bool, class: u8, kind: i32, t: f32) -> Entry {
        let total = if t > 0.0 { t } else { -1.0 };
        self.serial += 1;
        // The appear time is 0 for class 2 / 0xB and for ex-kind entries (J 5.1).
        let appear = if ex || class == class::REACTION { 0.0 } else { P.queue_appear_time };
        Entry { ex, class, kind, total, remaining: total, start_delay: P.queue_start_delay, appear, serial: self.serial - 1, selectable: true }
    }

    /// The HUD insert (0x1416158F0; F 11.3). Returns where the entry went and what it pushed out.
    fn insert(&mut self, ex: bool, kind: i32, t: f32, over_time: bool) -> (Slot, bool, Option<i32>) {
        // A kind already present is refreshed in place: never two entries of one kind.
        let refresh = |e: &mut Entry| {
            if over_time && e.total > 0.0 && e.total > t {
                e.remaining = e.total;
            } else {
                e.total = if t > 0.0 { t } else { -1.0 };
                e.remaining = e.total;
                e.appear = if e.ex || e.class == class::REACTION { 0.0 } else { P.queue_appear_time };
            }
        };
        if let Some(e) = self.list.iter_mut().find(|e| e.kind == kind && e.ex == ex) {
            refresh(e);
            return (Slot::List, true, None);
        }
        for (slot, which) in [(&mut self.slot_a, Slot::A), (&mut self.slot_b, Slot::B)] {
            if let Some(e) = slot.as_mut().filter(|e| e.kind == kind && e.ex == ex) {
                refresh(e);
                return (which, true, None);
            }
        }
        let (class, slot) = if ex { ex_class(kind) } else { (class_of(kind), Slot::List) };
        let slot = match class {
            _ if ex => slot,
            class::REACTION => Slot::A,
            class::FINISH | class::SINGLE_B => Slot::B,
            _ => Slot::List,
        };
        let entry = self.new_entry(ex, class, kind, t);
        match slot {
            Slot::A => {
                let out = self.slot_a.replace(entry).map(|e| e.kind);
                (Slot::A, false, out)
            }
            Slot::B => {
                let mut out = None;
                if let Some(old) = self.slot_b.take() {
                    // A reaction command over a finish command: the finish command waits underneath.
                    if ex && !old.ex {
                        self.slot_b_stack = Some(old);
                    } else {
                        out = Some(old.kind);
                    }
                }
                self.slot_b = Some(entry);
                (Slot::B, false, out)
            }
            Slot::List => {
                // Capacity: 6 counted entries. The 7th first removes the first entry in list
                // order, among indices 0..5, whose total time is >= 0.1 s (untimed ones are skipped).
                let mut out = None;
                let capacity = P.queue_capacity as usize;
                if self.list.len() >= capacity {
                    if let Some(i) = self.list.iter().take(capacity).position(|e| e.total >= P.queue_evict_min_total) {
                        out = Some(self.list.remove(i).kind);
                    }
                }
                self.list.push(entry);
                (Slot::List, false, out)
            }
        }
    }

    fn take(&mut self, kind: i32) -> Option<Slot> {
        if let Some(i) = self.list.iter().position(|e| e.kind == kind) {
            self.list.remove(i);
            return Some(Slot::List);
        }
        if self.slot_a.as_ref().is_some_and(|e| e.kind == kind) {
            self.slot_a = None;
            return Some(Slot::A);
        }
        if self.slot_b.as_ref().is_some_and(|e| e.kind == kind) {
            self.slot_b = self.slot_b_stack.take();
            return Some(Slot::B);
        }
        if self.slot_b_stack.as_ref().is_some_and(|e| e.kind == kind) {
            self.slot_b_stack = None;
            return Some(Slot::B);
        }
        None
    }

    /// `Select()` (0x141627B70; F 11.5): the default selection, unless the player's L2 choice
    /// still exists.
    fn select(&mut self) {
        let ready = |e: &Entry| e.selectable && e.appear <= 0.0;
        if let Some(kind) = self.choice {
            if let Some(i) = self.list.iter().position(|e| e.kind == kind && ready(e)) {
                self.selected = i as i32;
                return;
            }
            if kind == -1 && (self.slot_a.is_some() || self.slot_b.is_some() || self.extra.is_some()) {
                self.selected = -1;
                return;
            }
            self.choice = None;
        }
        self.selected = if self.slot_a.as_ref().is_some_and(ready) || self.slot_b.as_ref().is_some_and(ready) || self.extra.is_some() {
            -1
        } else {
            let candidates = self.list.iter().enumerate().filter(|(_, e)| ready(e));
            let best = if self.older_first { candidates.min_by_key(|(_, e)| e.serial) } else { candidates.max_by_key(|(_, e)| e.serial) };
            best.map_or(-2, |(i, _)| i as i32)
        };
    }

    /// L2 (0x14162BAF0(hud, +1, wrap)): the next selectable list entry; past the last one the
    /// event button when present (-1), else the first. True when the selection changed.
    fn cycle(&mut self) -> bool {
        let ready = |e: &Entry| e.selectable && e.appear <= 0.0;
        let single = self.slot_a.as_ref().is_some_and(ready) || self.slot_b.as_ref().is_some_and(ready);
        let before = self.selected;
        let n = self.list.len() as i32;
        // (found: Some(index), or Some(-1) for the event button)
        let mut found = None;
        let mut index = if before < 0 { -1 } else { before };
        for _ in 0..=n {
            index += 1;
            if index >= n {
                if single {
                    found = Some(-1);
                    break;
                }
                index = -1;
                continue;
            }
            if ready(&self.list[index as usize]) {
                found = Some(index);
                break;
            }
        }
        match found {
            Some(-1) => {
                self.selected = -1;
                self.choice = Some(-1);
            }
            Some(i) => {
                self.selected = i;
                self.choice = Some(self.list[i as usize].kind);
            }
            None => {}
        }
        self.selected != before
    }

    /// (YHUD, port seam: KH3 has no friend leader) THE CONTROLLED FRIEND'S CHOICE: select the counted entry with this
    /// serial as if the player had cycled to it with L2 (`choice` keeps it through `select()`), so a friend leader's Y
    /// runs HIS row of Sora's queue (a team attack with him), never the event button's finish or Sora's own formchange.
    /// False (nothing changed) when no ready entry has that serial.
    pub fn choose_serial(&mut self, serial: u32) -> bool {
        let Some(i) = self.list.iter().position(|e| e.serial == serial && e.selectable && e.appear <= 0.0) else { return false };
        self.selected = i as i32;
        self.choice = Some(self.list[i].kind);
        true
    }

    /// The entry Triangle would use now: (slot, kind).
    pub fn current(&self) -> Option<(Slot, i32)> {
        match self.selected {
            -1 => {
                // Slot A first; in slot B a finish extra covers the finish command.
                if let Some(e) = &self.slot_a {
                    return Some((Slot::A, e.kind));
                }
                // (An extra is on show while its finish command is already used: slot B is empty.)
                match &self.slot_b {
                    Some(e) => Some((Slot::B, if e.ex { e.kind } else { self.extra.unwrap_or(e.kind) })),
                    None => self.extra.map(|kind| (Slot::B, kind)),
                }
            }
            i if i >= 0 => self.list.get(i as usize).map(|e| (Slot::List, e.kind)),
            _ => None,
        }
    }
}

fn kind_of(kind: i32) -> ETresCommandKind {
    ETresCommandKind::from_value(kind).unwrap_or(ETresCommandKind::None)
}

/// `NotifySetActionCommand(kind, time, info)` (0x14140DA40; F 11.2). `time` -1.0 = the row's own
/// lifetime. Returns false when nothing was enqueued.
pub(crate) fn enqueue(cx: &mut Ctx, kind: i32, time: f32) -> bool {
    let abilities = cx.pawn.status.abilities;
    let row = command_row(kind);
    let mut t = row.filter(|r| r.cmd_type == 4 && r.param0 > 0.0).map_or(0.0, |r| r.param0);
    let mut time = time;
    match kind {
        // grand magic
        30..=32 | 34..=36 | 38..=40 | 42..=44 | 46..=48 | 110..=126 | 231 => t *= 1.0 + 0.2 * abilities.count(ETresAbilityKind::MagicTime) as f32,
        186..=191 => t *= 1.0 + 0.2 * abilities.count(ETresAbilityKind::AttractionTime) as f32,
        // formchange: not while the style is fixed (ctrl+0xB60)
        128..=154 | 221..=224 => {
            if cx.pawn.combat.form.style_fix {
                return false;
            }
            t *= 1.0 + P.form_command_life_per_form_time * abilities.count(ETresAbilityKind::FormTime) as f32;
        }
        // team attacks (the friend's COMBI_TIME: no friends here)
        193..=210 | 237..=240 | 242 => {
            if time != -1.0 {
                (t, time) = (time, -1.0);
            }
        }
        // one finish command at a time
        171 => {
            delete(cx, 172);
            delete(cx, 173);
        }
        172 => {
            for k in [171, 173, 174] {
                delete(cx, k);
            }
        }
        173 | 174 => {
            delete(cx, 171);
            delete(cx, 172);
        }
        109 | 192 => time = if kind == 109 { 0.0 } else { time },
        // The other finish / context rows of the table pass with their time; kinds outside the
        // jump table (33, 37, 41, 45, 49..74, 85, 86, 95..98, 101, 127, 155, 170, 175..178, the
        // reaction range) are not enqueued by this function.
        33 | 37 | 41 | 45 | 49..=74 | 85 | 86 | 95..=98 | 101 | 127 | 155 | 170 | 175..=178 | 0..=26 => return false,
        _ => {}
    }
    if time != -1.0 {
        t = time;
    }
    t = t.min(P.form_command_life_cap);
    add(cx, false, kind, t);
    true
}

/// The cockpit's `AddCommand` -> HUD insert, with the events.
fn add(cx: &mut Ctx, ex: bool, kind: i32, t: f32) {
    // (ER90, XSI request 2) RAGE RAID KEEPS SLOT B: while its offer runs (XS 2.11), another reaction command of slot B (the
    // guard's own counter prompt, Revenge Slash, mirrored in the step the block offered Raid) does not push it out: Y gives
    // Raid, as Attack does (states/attack.rs: Raid wins over the forced counter). Without Rage Raid: as before.
    if ex && raid_holds_slot_b(cx, kind) {
        return;
    }
    let over_time = cx.pawn.status.abilities.has(ETresAbilityKind::OverTime);
    let (slot, refreshed, out) = cx.pawn.combat.situation.insert(ex, kind, t, over_time);
    if let Some(old) = out {
        cx.pawn.combat.situation.removed += 1;
        cx.pawn.event(EventKind::SituationRemoved { kind: kind_of(old), why: Removal::Evicted });
    }
    let total = cx.pawn.combat.situation.find(kind).map_or(t, |(_, e)| e.total);
    if slot == Slot::List && !refreshed {
        // (The event the HUD fixtures already know: an entry of the counted list.)
        cx.pawn.event(EventKind::SituationCommand { kind: kind_of(kind), lifetime: total });
    } else {
        cx.pawn.event(EventKind::SituationSlot { kind: kind_of(kind), slot, total, refreshed });
    }
    cx.pawn.combat.situation.select();
}

/// (ER90) Rage Raid's command is in slot B, its offer runs, and `kind` is another ex kind of slot B.
fn raid_holds_slot_b(cx: &Ctx, kind: i32) -> bool {
    let raid = crate::abilities::xs::X.rage_raid_command as i32;
    kind != raid && ex_class(kind).1 == Slot::B && cx.pawn.combat.situation.slot_b.is_some_and(|e| e.ex && e.kind == raid) && crate::abilities::xs::rage::raid_due(cx).is_some()
}

/// `NotifyDeleteActionCommand(kind)` (0x14140C790).
pub(crate) fn delete(cx: &mut Ctx, kind: i32) -> bool {
    remove(cx, kind, Removal::Deleted)
}

fn remove(cx: &mut Ctx, kind: i32, why: Removal) -> bool {
    let queue = &mut cx.pawn.combat.situation;
    if queue.take(kind).is_none() {
        return false;
    }
    queue.removed += 1;
    queue.select();
    cx.pawn.event(EventKind::SituationRemoved { kind: kind_of(kind), why });
    true
}

/// A reaction command of the controller's prompt enters the HUD as an ex-kind entry in its
/// single slot (0x141575640 -> 0x1416158F0(hud, 2, 0, exKind, time, info)); its lifetime is the
/// prompt's timer, not a list countdown.
pub(crate) fn set_reaction(cx: &mut Ctx, kind: i32, time: f32) {
    add(cx, true, kind, time);
}

pub(crate) fn clear_reaction(cx: &mut Ctx, kind: i32) {
    remove(cx, kind, Removal::Expired);
}

/// `extra(cmd)` (StyleFinish slot 162 0x1413E3690): an extra command is shown in the finish slot.
pub(crate) fn set_extra(cx: &mut Ctx, kind: i32) {
    cx.pawn.combat.situation.extra = Some(kind);
    cx.pawn.event(EventKind::SituationSlot { kind: kind_of(kind), slot: Slot::B, total: -1.0, refreshed: false });
}

pub(crate) fn clear_extra(cx: &mut Ctx) {
    if let Some(kind) = cx.pawn.combat.situation.extra.take() {
        cx.pawn.event(EventKind::SituationRemoved { kind: kind_of(kind), why: Removal::Deleted });
    }
}

/// The HUD tick (0x141630AF0; F 11.4, magic spec J 5.6), then L2 and Triangle. Called in the UI
/// phase of the step, after the state tick.
pub(crate) fn tick(state: &mut StateData, cx: &mut Ctx, input: &Input, dt: f32) {
    // The reaction prompts of the controller (the actions implementer's timers) as slot entries.
    super::guard::mirror_prompts(cx);
    if !cx.pawn.combat.situation.paused {
        // `dt x ObjTypeTimeDilation[PLAYER]` (`mov dl,1` @0x141630B59).
        let player_dt = dt * cx.pawn.time_dilation(crate::host::time::OBJ_PLAYER);
        let mut expired: Vec<i32> = Vec::new();
        let queue = &mut cx.pawn.combat.situation;
        for e in &mut queue.list {
            if e.appear > 0.0 {
                e.appear -= dt;
            }
            if e.start_delay >= 0.0 {
                e.start_delay -= dt;
                continue;
            }
            if e.remaining >= 0.0 {
                e.remaining -= player_dt;
                if e.remaining < 0.0 {
                    e.remaining = 0.0;
                    expired.push(e.kind);
                }
            }
        }
        // Slots A and B only tick their appear time and start delay.
        for e in queue.slot_a.iter_mut().chain(queue.slot_b.iter_mut()) {
            if e.appear > 0.0 {
                e.appear -= dt;
            }
            if e.start_delay >= 0.0 {
                e.start_delay -= dt;
            }
        }
        for kind in expired {
            // (The pawn's notify interface vt+0x1D0 is told the kind: the event.)
            remove(cx, kind, Removal::Expired);
        }
    }
    cx.pawn.combat.situation.select();
    auto_finish(state, cx);
    let pad = input.pad;
    // L2 (pad key 6): the next selectable entry, wrapping through the single slot.
    if pad.pressed.has(PadButton::SITUATION_NEXT) && cx.pawn.combat.situation.cycle() {
        cx.pawn.event(EventKind::SituationSelect { index: cx.pawn.combat.situation.selected });
    }
    // Triangle (controller slot 674 0x141404460: only with LB and RB up and no command cool-down).
    if pad.pressed.has(PadButton::SITUATION) && !pad.held.has(PadButton::LeftShoulder) && !pad.held.has(PadButton::RightShoulder) && !(cx.pawn.combat.command_cooldown > 0.0) {
        use_selected(state, cx);
    }
}

/// AUTO_FINISH (ability 70), the HUD's 0x141618830, called from the command menu update
/// 0x14162FF80 every frame (decompiled C; third pass, spec 11.5):
///
/// ```text
/// controller valid && hud+0x708 != 0 && hud+0x9BB (has AUTO_FINISH, refreshed by 0x141616B10)
/// && menu state (hud+0xAC8) in {1 MainSel, 2 SubSel, 3 Shortcut, 5 Attraction}
/// && kind = slot B's command (hud+0x7D0) != 0
/// && the slot-B stack (hud+0x820) has an entry whose remaining time (+0x18) <= 0.5
/// && kind not in {0x6D, 0xB3, 0xD3, 0xF1}, hud+0x73F clear, (hud+0x806 || kind != 0xC0)
/// && CanExecuteCommand(kind, 0) && ExecuteCommand(kind):  the entry is removed as used
/// ```
///
/// The finish command's entry lives with the form time (11.1), so: with the ability, half a
/// second before the form would run out the finish command is executed by itself. hud+0x708 was
/// not identified (the help text says "in combat": the battle flag stands in); the menu-state
/// test is not made (the port's menu has no state in which the finish slot is hidden).
fn auto_finish(state: &mut StateData, cx: &mut Ctx) {
    if !cx.pawn.status.abilities.has(crate::enums::ETresAbilityKind::AutoFinish) || !cx.pawn.actions.battle {
        return;
    }
    let form = cx.pawn.combat.form;
    let Some(kind) = cx.pawn.combat.situation.slot_b.map(|e| e.kind) else { return };
    if form.cmd == 0 || !(form.time_left <= AUTO_FINISH_TIME) || matches!(kind, 0 | 0x6D | 0xB3 | 0xC0 | 0xD3 | 0xF1) || cx.pawn.combat.situation.extra.is_some() {
        return;
    }
    if execute(state, cx, kind) {
        remove(cx, kind, Removal::Used);
        cx.pawn.combat.situation.auto_finished += 1;
    }
}

/// The remaining-time threshold of 0x141618830 (decompiled C: `<= 0.5`).
pub const AUTO_FINISH_TIME: f32 = 0.5;

/// Triangle (0x1416220D0 -> 0x141618A50): the entry is removed only if the executor accepted;
/// a refused use leaves it (no buffer).
pub(crate) fn use_selected(state: &mut StateData, cx: &mut Ctx) -> bool {
    let Some((slot, kind)) = cx.pawn.combat.situation.current() else { return false };
    if kind == 0 || !execute(state, cx, kind) {
        return false;
    }
    if cx.pawn.combat.situation.extra == Some(kind) {
        // (A finish extra is cleared by the finish state, which also re-offers it.)
        return true;
    }
    let _ = slot;
    remove(cx, kind, Removal::Used);
    true
}

/// The executor 0x14141B470 (`ExecuteCommand` writes ctrl+0x87C first), by command kind.
pub(crate) fn execute(state: &mut StateData, cx: &mut Ctx, kind: i32) -> bool {
    use super::form;
    let ok = match kind {
        // (XSI) ANCIENT LIGHT (XS 2.13): LM_UNIONDISCODE (174) outside a form is the XS offer.
        174 if cx.pawn.combat.form.cmd == 0 => crate::abilities::xs::ancient::execute(state, cx),
        // (XSI) RISKCHARGE (XS 2.12): the command outside a form.
        179 if cx.pawn.combat.form.cmd == 0 => crate::abilities::xs::rage::execute_riskcharge(state, cx),
        // (XSI) MYRIAD (XS 2.14): Second Form's finish command becomes Ultimate Form's finale.
        171..=173 if crate::abilities::xs::myriad::wanted(cx) => crate::abilities::xs::myriad::execute(state, cx),
        // (XSI) LIGHT WAVE (XS 2.9): KH3's unused Reserve248 is its entry.
        248 => crate::abilities::xs::light_wave::execute(state, cx),
        // 0x14141B9A6: formchange kinds 0x80..0x9A, 0xDD..0xE0.
        0x80..=0x9A | 0xDD..=0xE0 => form::start_style(state, cx, kind),
        // 0x14141B923: finish kinds (needs +0xB5D).
        0x6D | 0x9C..=0xA8 | 0xAB..=0xAE | 0xB3 | 0xE1 | 0xE2 => cx.pawn.combat.form.finish_pending && form::start_style(state, cx, kind),
        // 0x14141B981: finish extras: pawn slot 1346 stores the kind in the running StyleFinish.
        0xAA | 0xAF..=0xB2 => crate::states::style_finish::extra_pressed(state, kind),
        // Grand magic (class 1).
        110..=126 | 231 => grand_magic(state, cx, kind),
        // (party stream D) The team commands: Sora's FR state (combat/friend.rs).
        193 | 195 | 207 | 208 => super::friend::try_team_attack(state, cx, kind),
        // Reaction commands: the offered counter attack is the attack request's business
        // (`TryAttack`); Triangle on the prompt makes the same request.
        1..=26 => {
            let target = cx.pawn.combat.lockon.current;
            crate::states::attack::try_attack(state, cx, target)
        }
        _ => false,
    };
    if ok && !matches!(kind, 1..=26 | 0xAA | 0xAF..=0xB2) {
        // ctrl+0x8F4 = 0.1: no flinch for 0.1 s after a command was executed (strand E 9.6).
        cx.pawn.combat.reaction_block = P.hit_reaction_block_after_command;
        // 0x1414158F0: ctrl+0x880 = cmd; pawn+0xE2C = cmd ("command in action").
        cx.pawn.combat.form.command_in_action = kind;
    }
    ok
}

/// MAGIC IMPLEMENTER: the use of a grand-magic entry = `TryMagic(kind, grand = 1)` (magic spec J
/// 5.5: no MP cost; on success the controller clears its grand-magic bookkeeping, which
/// `style::clear_grand` does). Replace the body with your call.
fn grand_magic(state: &mut StateData, cx: &mut Ctx, kind: i32) -> bool {
    // (magic implementer) `CanExecuteCommand` then `ExecuteCommand(kind)` -> `TryMagic(...,
    // bGrand = 1)` (command/mod.rs): no MP, the command lock 0.1 s, the gauge and both tallies
    // cleared on success.
    let kind = kind as u8;
    crate::command::can_execute(state, cx, kind) && crate::command::execute(state, cx, kind)
}

/// The queue, the gauge and the form as the HUD shows them (`UiSnapshot::situation`).
pub(crate) fn hud(s: &crate::pawn::Pawn) -> crate::ui_model::SituationHud {
    use crate::ui_model::{FormHud, SituationEntry, SituationHud};
    let entry = |e: &Entry| SituationEntry { kind: kind_of(e.kind), class: e.class, ex: e.ex, total: e.total, remaining: e.remaining, appearing: e.appear > 0.0, counting: e.start_delay < 0.0, serial: e.serial };
    let q = &s.combat.situation;
    let max = if P.form_gauge_max > 0.0 { P.form_gauge_max } else { 100.0 };
    let ratio = (s.combat.style.gauge / max).clamp(0.0, 1.0);
    let f = &s.combat.form;
    SituationHud {
        list: q.list.iter().map(entry).collect(),
        slot_a: q.slot_a.as_ref().map(entry),
        slot_b: q.slot_b.as_ref().map(entry),
        extra: q.extra.map(kind_of),
        selected: q.selected,
        gauge_ratio: ratio,
        // 0x14162DC40: phase 0 below 0.25, 1 below 0.5, 2 below 0.75, 3 below 1.0.
        gauge_phase: ((ratio * 4.0) as u8).min(3),
        form: (f.cmd != 0).then(|| FormHud { kind: kind_of(f.cmd), total: f.time_full, left: f.time_left.max(0.0), finish_level: f.finish_level, wear_form: f.wear_form, world_slow: f.world_slow }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(q: &Queue) -> Vec<i32> {
        q.list.iter().map(|e| e.kind).collect()
    }

    // Strand F 11.3: capacity 6 counted entries, eviction, refresh in place, positions.
    #[test]
    fn insert_capacity_eviction_and_refresh() {
        let mut q = Queue::default();
        // Six timed entries (grand magic 110..115) fill the list.
        for kind in 110..116 {
            assert_eq!(q.insert(false, kind, 20.0, false), (Slot::List, false, None));
        }
        assert_eq!((q.count(), kinds(&q)), (6, vec![110, 111, 112, 113, 114, 115]));
        // New entry: total = remaining = t, start delay 0.5 s, appear 0.2 s, serials count up.
        let e = q.list[0];
        assert_eq!((e.total, e.remaining, e.start_delay, e.appear, e.serial, q.list[5].serial), (20.0, 20.0, 0.5, 0.2, 0, 5));
        // The same kind again: refreshed in place, never two entries.
        q.list[2].remaining = 3.0;
        assert_eq!(q.insert(false, 112, 20.0, false), (Slot::List, true, None));
        assert_eq!((q.count(), q.list[2].remaining), (6, 20.0));
        // OVER_TIME keeps the longer time.
        q.list[2].remaining = 3.0;
        q.insert(false, 112, 5.0, true);
        assert_eq!((q.list[2].total, q.list[2].remaining), (20.0, 20.0));
        // The 7th removes the first entry in list order whose total >= 0.1 s, then is appended.
        q.list[0].total = -1.0; // untimed: skipped
        assert_eq!(q.insert(false, 148, 30.0, false), (Slot::List, false, Some(111)));
        assert_eq!(kinds(&q), vec![110, 112, 113, 114, 115, 148]);
        // Classes: formchange 3 in the list; a finish command goes to slot B, uncounted.
        assert_eq!((class_of(148), class_of(171), class_of(110), class_of(175)), (class::FORMCHANGE, class::FINISH, class::GRAND_MAGIC, class::FINISH));
        assert_eq!(q.insert(false, 171, 27.0, false).0, Slot::B);
        assert_eq!((q.count(), q.slot_b.unwrap().kind), (6, 171));
        // A reaction command (ex kind 2, BLOWCOUNTER) takes slot B over the finish command, which
        // comes back when it leaves; ex kind 7 (a risk-dodge counter) is class 2: slot A.
        assert_eq!(q.insert(true, 2, 2.0, false).0, Slot::B);
        assert_eq!(q.slot_b.unwrap().kind, 2);
        assert_eq!(q.take(2), Some(Slot::B));
        assert_eq!(q.slot_b.unwrap().kind, 171);
        assert_eq!(q.insert(true, 7, 0.6, false).0, Slot::A);
    }

    // Strand F 11.5: default selection, the appear time, L2.
    #[test]
    fn selection_prefers_the_slots_then_the_oldest_and_l2_cycles() {
        let mut q = Queue::default();
        q.insert(false, 148, 30.0, false);
        q.insert(false, 110, 20.0, false);
        // Nothing is selectable during the 0.2 s appear time.
        q.select();
        assert_eq!((q.selected, q.current()), (-2, None));
        for e in &mut q.list {
            e.appear = 0.0;
        }
        q.select();
        assert_eq!(q.current(), Some((Slot::List, 148)), "Prioritize Older: the smallest serial");
        q.older_first = false;
        q.select();
        assert_eq!(q.current(), Some((Slot::List, 110)), "Prioritize Newer: the largest serial");
        q.older_first = true;
        // L2: the next entry, wrapping; the choice wins over the default while its entry exists.
        q.select();
        assert!(q.cycle());
        assert_eq!(q.current(), Some((Slot::List, 110)));
        q.select();
        assert_eq!(q.current(), Some((Slot::List, 110)), "the L2 choice is kept");
        assert!(q.cycle());
        assert_eq!(q.current(), Some((Slot::List, 148)));
        // A finish command in slot B is the default Triangle target; L2 reaches the list.
        q.choice = None;
        q.insert(false, 171, 27.0, false);
        q.slot_b.as_mut().unwrap().appear = 0.0;
        q.select();
        assert_eq!(q.current(), Some((Slot::B, 171)));
        assert!(q.cycle());
        assert_eq!(q.current(), Some((Slot::List, 148)));
        q.cycle();
        q.cycle();
        assert_eq!(q.current(), Some((Slot::B, 171)), "past the last entry: the event button");
        // The chosen entry goes: back to the default.
        q.cycle();
        assert_eq!(q.current(), Some((Slot::List, 148)));
        q.take(148);
        q.select();
        assert_eq!(q.current(), Some((Slot::B, 171)));
    }
}
