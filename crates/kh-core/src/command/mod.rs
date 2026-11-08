//! OWNER: the magic implementer. THE COMMAND SYSTEM: the logic half of the game's
//! `TresUIP_HudCommand` and the controller's `CanExecuteCommand` / `ExecuteCommand` / `ExecCommand`
//! (docs/sora-magic-spec.md part J, with E I1 and A 2.1; combat strand A 1.1 for the Attack row;
//! display contract: docs/sora-ui-spec.md section 5).
//!
//! In the game the HUD part owns the menu state machine and reads the UI pad; here the core owns
//! it, so that the same inputs give the same commands whatever draws the menu. It runs in the
//! **U phase** of a step (after the state tick: strand A 1.1), reads `Input::pad`, and publishes
//! what the menu shows as a [`CommandSnapshot`] in the `UiSnapshot` (`UiSnapshot::command`), which
//! mirrors `kh_ui::CommandModel` field for field (the HUD implementer's display model).
//!
//! - Main list: Attack (27), Magic, Items, Link; D-pad up / down with wrap; Decide (the attack
//!   key) executes Attack or opens the list; D-pad right opens too. (THE USER 2026-11-02) the Link slot is "Summon" /
//!   "Unsummon" when the host has Elden Ring's spirits for this Sora (`summon.rs`).
//! - Magic list: the six magic slots, one per element, each at the highest learned tier (or the
//!   tier the Customize screen wrote: [`CommandState::set_magic_slot`]), in the order the elements
//!   are learned in the story (Fire, Water, Cure, Blizzard, Thunder, Aero). A row is greyed while
//!   MP Charge runs; `mp_short` only recolours it (it can still be cast) unless MP Safety is on.
//! - Items list: the distinct items of the pouch with their counts.
//! - Shortcuts: 3 pages x 4 slots, L1 held + Circle / Triangle / Square / Cross = slots 0..3,
//!   L1 + D-pad down / up = next / previous page with wrap. A new game has 12 empty slots; the
//!   level-99 test configuration fills two pages (loadout rows `shortcuts.a / .b / .c`).
//! - `ExecuteCommand(kind)` is the only writer of the pending command, on the press frame, with
//!   no buffer: a refused command is dropped.
//! - Default config (J 6.2): Command Selection Active (no slow motion), Cursor Position Reset
//!   (no cursor memory), Situation Commands Prioritize Older, Control Type A.
//!
//! NOT HERE (the combat implementer's, strand F): the situation-command QUEUE, the Triangle use
//! and L2 cycling. They call [`execute`] with the entry's kind; the snapshot has a place for
//! their entries (`CommandSnapshot::situations`).

pub mod customize;
pub mod leader; // (party X2) Customize follows the leader: the friends' own blocks and lists
pub mod summon; // (THE USER 2026-11-02: "Give Sora the ability to 'Summon'") the Summon command's rules

use crate::host::TargetId;
use crate::input::PadButton;
use crate::magic::items;
use crate::magic::spells::{self, Element};
use crate::magic::tables::Sheet;
use crate::pawn::Ctx;
use crate::sora::Input;
use crate::states::{attack, fire_magic, use_item, StateData, StateId};
use crate::status::MpCost;

/// The states of the HUD part that the port uses (the game's numbers; J 1.3, UI spec 5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum MenuState {
    /// 1: the main list.
    #[default]
    MainSel = 1,
    /// 2: a sub-list.
    SubSel = 2,
    /// 3: the shortcut panel (L1 held).
    Shortcut = 3,
    /// 6: the shotlock panel (the aim mode is open).
    ShootFlow = 6,
}

/// How a sub-list was left (UI spec 5.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseKind {
    /// Cancel: the main cursor returns to Attack.
    Cancel,
    /// D-pad left: the main cursor stays.
    Back,
    /// A command was executed from it.
    Decide,
}

/// One-shot facts of a step that the state itself does not show (for the display's calls and
/// UI sounds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEvent {
    /// A refused confirm / shortcut (static SE 4).
    Refused,
    SubOpened,
    SubClosed(CloseKind),
    /// A shortcut entry was executed.
    ShortcutExecuted,
    /// `ExecuteCommand(kind)` succeeded.
    Executed(u8),
}

/// The save-data options of J 6.2 (all default off).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Config {
    /// "Command Selection: Slow": object-type slow motion 0.3 while a sub-list is open.
    pub command_slow: bool,
    /// "Cursor Position: Lock": the list stays open after a command.
    pub cursor_lock: bool,
    /// "Situation Commands: Prioritize Newer" / "Control Type: Type B": read by the queue's owner.
    pub prioritize_newer: bool,
    pub control_type_b: bool,
}

/// One row of the menu as it is shown.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CommandRow {
    /// `ETresCommandKind` (0 = an empty slot; 28 / 57 = the Magic / Items group rows).
    pub kind: u8,
    /// The English name of the command's text (the HUD has the text tables).
    pub name: &'static str,
    /// False: greyed.
    pub enabled: bool,
    /// The command costs more MP than he has: recoloured, still castable without MP Safety.
    pub mp_short: bool,
    /// Pouch quantity of an item row (0 = not an item).
    pub quantity: u32,
    pub needs_target: bool,
    /// (GOOFY) A command that is recharging (THE USER 2026-11-02: "Goofy's abilities (except taunt) should cost more MP, and
    /// introduce a cooldown with a visible timer, they are way too spammable"): (seconds left, its full length); (0, 0) = ready.
    /// While it runs the command is refused; `enabled` leaves it out (it says what else greys the row). The HUD drains KH3's
    /// situation timer gauge under the entry's name and counts the seconds down in its number.
    pub cooldown: (f32, f32),
}

/// The shotlock panel (UI spec 5.9): shown while the aim mode is open.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ShootFlowPanel {
    /// `ETresShootFlowKind` now (1 King of Hearts, 2 Ragnarok at MAX, 36 Focus Syphon).
    pub kind: u8,
    /// Can shoot / can airstep.
    pub enabled: [bool; 2],
    pub locks: i32,
    pub max_locks: i32,
    /// Seconds of aiming left and at entry.
    pub aim_time: f32,
    pub aim_time_start: f32,
    pub is_max: bool,
}

/// A situation command as the queue's owner publishes it (UI spec 5.6).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SituationRow {
    pub serial: u32,
    pub kind: u8,
    pub name: &'static str,
    /// 1 form change, 2 attraction, 3 grand magic, 4 team attack, 5 link.
    pub icon_type: i32,
    pub total: f32,
    pub remaining: f32,
    pub enabled: bool,
    pub refreshed: u32,
}

/// What the command menu shows after a step: `UiSnapshot::command`. Mirrors
/// `kh_ui::CommandModel`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CommandSnapshot {
    pub state: MenuState,
    /// Attack, Magic, Items, Link.
    pub main: [CommandRow; 4],
    /// The Link list is empty: three slots.
    pub link_none: bool,
    pub main_cursor: usize,
    /// The open list in `SubSel`: 1 Magic, 2 Items, 3 Link.
    pub list: usize,
    /// Rows of the lists 1..=3 (index 0 unused).
    pub lists: [Vec<CommandRow>; 4],
    pub row_cursor: usize,
    /// 0 = A, 1 = B, 2 = C.
    pub shortcut_page: usize,
    /// Rows in the order Circle, Triangle, Square, Cross (right, top, left, bottom face button).
    pub shortcut: [CommandRow; 4],
    pub shoot_flow: Option<ShootFlowPanel>,
    /// The situation commands (filled by the queue's owner; empty until then).
    pub situations: Vec<SituationRow>,
    pub situation_selected: Option<usize>,
    /// MP state the rows were computed from.
    pub mp_charging: bool,
    pub events: Vec<MenuEvent>,
}

/// A command a host asks for directly (a script word, a debug key): executed in the next UI
/// phase through the same `ExecuteCommand` as the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandRequest {
    Execute(u8),
    /// Shortcut slot 0..=3 of the current page.
    Shortcut(u8),
    ShortcutPage(i8),
}

/// The command system's state: the HUD part's fields and the controller's command fields.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandState {
    pub state: MenuState,
    /// HUD+0x6F0.
    pub main_cursor: u8,
    /// The sub-cursor of the open list; the open list (1 Magic, 2 Items, 3 Link).
    pub sub_cursor: u8,
    pub list: u8,
    /// `shortcut[3][4]` (save block S+0x00) and the current page (UI manager +0x6B54).
    pub shortcuts: [[u8; 4]; 3],
    pub shortcut_page: u8,
    /// `magicSlot[6]` (S+0x30): the command kind per slot, 0 = empty.
    pub magic_slots: [u8; 6],
    /// S+0x5C: the slot's tier was chosen by hand on the Customize screen (UI spec 10.6): a
    /// later tier bonus does not replace it.
    pub tier_chosen: [bool; 6],
    pub config: Config,
    /// ctrl+0x8C8: the command lock timer (set by slot 676, e.g. 0.1 s after a grand magic).
    pub lock_timer: f32,
    /// ctrl+0x8F4: the 0.1 s reaction block written when such a command is executed.
    pub reaction_block: f32,
    /// ctrl+0x87C: the pending command (non-zero only inside `ExecuteCommand`, or after a
    /// refused one); ctrl+0x880 / +0x884: the last command executed and its shotlock kind.
    pub pending: u8,
    pub last_executed: u8,
    pub last_shoot_flow: u8,
    /// HUD+0x9E8: the slow option is applied now.
    pub slow_applied: bool,
    /// The slots were filled from the status.
    pub filled: bool,
    /// Commands asked for by the host since the last step.
    pub requests: Vec<CommandRequest>,
    /// This step's one-shot facts.
    pub events: Vec<MenuEvent>,
    /// Commands executed so far (for tests).
    pub executed: u32,
    /// (XSI, 2026-10-16) The Customize screen's Shotlock choice: 0 = the Keyblade's own (King of Hearts, Ragnarok
    /// at MAX), 11 = Spectral Rays (abilities/xs/spectral.rs). Part of the save record (kh-host save.rs).
    pub shotlock: u8,
    /// (THE USER 2026-11-02: "In the equipment menu for Sora, make a new area for him to equip spirit ashes (4)") THE
    /// EQUIPPED SPIRIT ASHES: Elden Ring goods ids at level 0 (`summon::base_of`; 0 = empty), so an upgrade keeps the
    /// slot. Part of the save record (kh-host save.rs, additive).
    pub spirit_ashes: [u32; summon::SLOTS],
    /// (Summon) what the host says about the ashes, the bell and the spirit, renewed every frame (`summon::SummonView`).
    pub summon: summon::SummonView,
    /// (Summon) the chant runs (states/summon_chant.rs sets it).
    pub calling: bool,
}

impl Default for CommandState {
    fn default() -> CommandState {
        CommandState { state: MenuState::MainSel, main_cursor: 0, sub_cursor: 0, list: 0, shortcuts: [[0; 4]; 3], shortcut_page: 0, magic_slots: [0; 6], tier_chosen: [false; 6], config: Config::default(), lock_timer: 0.0, reaction_block: 0.0, pending: 0, last_executed: 0, last_shoot_flow: 0, slow_applied: false, filled: false, requests: Vec::new(), events: Vec::new(), executed: 0, shotlock: 0, spirit_ashes: [0; summon::SLOTS], summon: summon::SummonView::default(), calling: false }
    }
}

/// The order the elements are learned in the story (J 2.2: vbonus_082 Fire then Water, 014 Cure,
/// 016 Blizzard, 021 Thunder, 024 Aero): the order of the magic slots at the end of the game.
pub const LEARN_ORDER: [Element; 6] = [Element::Fire, Element::Water, Element::Cure, Element::Blizzard, Element::Thunder, Element::Aero];

impl CommandState {
    /// Fills the magic slots (highest learned tier per element, learn order) and the shortcut
    /// pages from the status, once.
    pub fn fill(&mut self, status: &crate::status::Status) {
        if self.filled || status.placeholder {
            return;
        }
        self.filled = true;
        let mut k = 0;
        for element in LEARN_ORDER {
            if let Some(known) = status.magic.iter().find(|m| Element::from_name(&m.name) == Some(element)) {
                if known.tier >= 1 && k < 6 {
                    self.magic_slots[k] = element.command(known.tier.min(3) as u8);
                    k += 1;
                }
            }
        }
        for (page, names) in status.shortcuts.iter().enumerate().take(3) {
            for (slot, name) in names.iter().enumerate().take(4) {
                self.shortcuts[page][slot] = spells::command_by_name(name).unwrap_or(0);
            }
        }
    }

    /// (fixes pass 1) The status was REPLACED (a host's test-loadout switch): the magic slots,
    /// the hand-chosen bits and the shortcut pages are emptied and filled again from it. Not a
    /// game function: the game only ever fills them from the save data and the menus.
    pub fn refill(&mut self, status: &crate::status::Status) {
        (self.magic_slots, self.tier_chosen, self.shortcuts, self.shortcut_page, self.filled) = ([0; 6], [false; 6], [[0; 4]; 3], 0, false);
        self.fill(status);
    }

    /// (new game) A SPELL WAS LEARNED (0x14159EE70, called by the bonus handlers 0x140E9F040 /
    /// 0x140EC0220; magic spec J 2.2): a slot that holds a lower tier of the same element takes the
    /// new kind in place, unless its tier was chosen by hand (S+0x5C); an element no slot holds goes
    /// into the first empty slot. Returns the slot that changed.
    pub fn learn(&mut self, cmd: u8) -> Option<usize> {
        let new = spells::unit(cmd)?;
        if new.grand {
            return None;
        }
        if let Some(slot) = self.magic_slots.iter().position(|k| spells::unit(*k).is_some_and(|u| u.element == new.element)) {
            let old = spells::unit(self.magic_slots[slot])?;
            if self.tier_chosen[slot] || old.tier >= new.tier {
                return None;
            }
            self.magic_slots[slot] = cmd;
            return Some(slot);
        }
        let slot = self.magic_slots.iter().position(|k| *k == 0)?;
        self.magic_slots[slot] = cmd;
        Some(slot)
    }

    /// The Customize screen's tier choice (UI spec 10.6): a learned lower tier may be put into a
    /// magic slot. False when the slot does not hold that element or the tier is not learned.
    pub fn set_magic_slot(&mut self, status: &crate::status::Status, slot: usize, cmd: u8) -> bool {
        let (Some(new), Some(old)) = (spells::unit(cmd), self.magic_slots.get(slot).and_then(|c| spells::unit(*c))) else { return false };
        let learned = status.magic.iter().find(|m| Element::from_name(&m.name) == Some(new.element)).map_or(0, |m| m.tier);
        if new.grand || new.element != old.element || new.tier as u32 > learned {
            return false;
        }
        if self.magic_slots[slot] != cmd {
            // 0x140E95E20: the bit is set when the kind changed.
            self.tier_chosen[slot] = true;
        }
        self.magic_slots[slot] = cmd;
        true
    }

    /// The HUD masks the jump / cancel press while a sub-list is open (the cancel closes it).
    /// INFERRED: the controller takes its press masks from the HUD object (0x1415839C0; strand
    /// A 1.2), whose rule was not read.
    pub fn masks_jump(&self) -> bool {
        self.state == MenuState::SubSel
    }
}

fn needs_target(kind: u8) -> bool {
    matches!(kind, 49 | 50 | 51 | 58 | 59 | 61 | 62 | 64 | 68)
}

/// `MP short` (controller slot 686 0x141403FC0; J 2.2).
fn mp_short(status: &crate::status::Status, kind: u8, combo: i32) -> bool {
    let Some(unit) = spells::unit(kind) else { return false };
    match unit.mp {
        MpCost::All => false,
        MpCost::Points(cost) => {
            let cost = status.effective_mp_cost(cost, combo);
            cost > 0 && !(status.has(crate::enums::ETresAbilityKind::MpLeave) && status.mp >= 1) && status.mp < cost
        }
    }
}

/// The row of a command with its per-frame flags (0x141632660; J 2.2, 2.3, 3.3).
fn row(cx: &mut Ctx, kind: u8) -> CommandRow {
    if kind == 0 {
        return CommandRow::default();
    }
    let name = spells::command_name(kind);
    if let Some(item) = items::item_of_command(kind) {
        let quantity = items::pouch(cx).count(item);
        return CommandRow { kind, name, enabled: quantity > 0, mp_short: false, quantity, needs_target: needs_target(kind), ..CommandRow::default() };
    }
    let status = &cx.pawn.status;
    let combo = cx.pawn.combat.combo;
    let short = mp_short(status, kind, combo);
    // `Available`: not while MP Charge runs; `CanUse`: MP Safety refuses an "MP short" row.
    let enabled = !status.mp_charging && !(status.has(crate::enums::ETresAbilityKind::MpSafety) && short) && spells::unit(kind).is_some();
    let enabled = enabled && nocombat_allows(kind); // (NOCOMBAT)
    CommandRow { kind, name, enabled, mp_short: short, quantity: 0, needs_target: needs_target(kind), ..CommandRow::default() }
}

/// (NOCOMBAT) The no-combat rule for one of Sora's command kinds (an item is never refused by it).
fn nocombat_allows(kind: u8) -> bool {
    crate::nocombat::allows(crate::nocombat::Fighter::Sora, crate::nocombat::act_of_kind(kind))
}

fn magic_list(cx: &mut Ctx) -> Vec<CommandRow> {
    let slots = cx.pawn.command.magic_slots;
    slots.iter().filter(|k| **k != 0).map(|k| row(cx, *k)).collect()
}

fn item_list(cx: &mut Ctx) -> Vec<CommandRow> {
    let list = items::pouch(cx).list();
    list.iter().filter_map(|(item, _)| items::command_of_item(*item)).map(|k| row(cx, k)).collect()
}

// ---------------------------------------------------------------------------------------------- (Summon)
// (THE USER 2026-11-02: "Give Sora the ability to 'Summon' from the command menu, but he uses Elden Ring's spirit ashes")
// the fourth main row (KH3's Link slot) and its list: the rules are summon.rs's.

/// The host's view with the core's own chant flag.
fn summon_view(cx: &Ctx) -> summon::SummonView {
    let mut view = cx.pawn.command.summon.clone();
    view.calling |= cx.pawn.command.calling;
    view
}

/// The Summon list: the equipped ashes in slot order (kind 248 + slot).
fn summon_list(cx: &Ctx) -> Vec<CommandRow> {
    let view = summon_view(cx);
    let status = &cx.pawn.status;
    let free = view.out.is_none() && !view.calling && view.bell && view.allowed && nocombat_allows(summon::KIND_SUMMON); // (NOCOMBAT)
    view.slots
        .iter()
        .enumerate()
        .filter_map(|(slot, ash)| {
            ash.as_ref().map(|ash| {
                let (lit, short) = summon::list_row(ash, status);
                CommandRow { kind: summon::KIND_SUMMON + slot as u8, name: summon::TEXT_SUMMON, enabled: lit && free, mp_short: short, ..CommandRow::default() }
            })
        })
        .collect()
}

/// The main row: "Summon" (greyed by `summon::main_row`'s rules) or, while a spirit is out, "Unsummon".
fn summon_main(cx: &Ctx) -> CommandRow {
    let view = summon_view(cx);
    let (name, grey) = summon::main_row(&view, &cx.pawn.status);
    CommandRow { kind: if view.out.is_some() { summon::KIND_UNSUMMON } else { 0 }, name, enabled: grey.is_none() && nocombat_allows(summon::KIND_SUMMON), ..CommandRow::default() }
}

/// Summons equipped `slot`'s ash: the cost is paid by KH3's `UseMagicPoint` (an overspend empties the bar and starts MP
/// Charge, as a spell's; MP Thrift lowers it as a spell's), and the chant begins (states/summon_chant.rs).
fn summon_execute(state: &mut StateData, cx: &mut Ctx, slot: usize) -> bool {
    let view = summon_view(cx);
    let Some(ash) = view.slots.get(slot).cloned().flatten() else { return false };
    if !nocombat_allows(summon::KIND_SUMMON + slot as u8) || !view.allowed || !view.bell || view.out.is_some() || view.calling || !summon::list_row(&ash, &cx.pawn.status).0 {
        return false;
    }
    if !gate(state, cx) || !crate::states::summon_chant::can_summon(state, cx) {
        return false;
    }
    // (THE USER 2026-11-03: "mimic tear should make Sora instantly go into MP recharge") the Mimic Tear takes it all: KH3's
    // `UseMagicPoint(-1)` (MP 0, the charge starts); the refund's base is the MP he had (summon.rs `pay_now`)
    let (cost, base) = summon::pay_now(&ash, &cx.pawn.status);
    if summon::takes_all(&ash) {
        cx.pawn.status.use_mp(-1);
    } else {
        cx.pawn.status.use_mp(cost);
    }
    cx.pawn.queue.set_state(crate::states::summon_chant::SummonChant::new(slot as u8, base));
    let kind = summon::KIND_SUMMON + slot as u8;
    let c = &mut cx.pawn.command;
    c.calling = true;
    c.last_executed = kind;
    c.executed += 1;
    c.events.push(MenuEvent::Executed(kind));
    true
}

/// "Unsummon" (THE USER 2026-11-02: "pressing it just immediately ends the summon and gives back a refund of MP for how
/// long the timer had left"): the refund is given here (summon.rs `refund`, `give_refund`), the host ends the spirit on
/// the event. Not during the chant. The view's spirit is dropped at once so the row reads "Summon" from this frame.
fn unsummon(cx: &mut Ctx) -> bool {
    let view = summon_view(cx);
    let Some(out) = view.out else { return false };
    if view.calling || cx.pawn.status.hp < 1 || !nocombat_allows(summon::KIND_UNSUMMON) {
        return false;
    }
    let mp = summon::refund(out.cost, out.left, out.full);
    summon::give_refund(&mut cx.pawn.status, mp);
    cx.pawn.event(crate::host::EventKind::Summon { phase: summon::SummonPhase::Unsummon, slot: out.slot, mp });
    let c = &mut cx.pawn.command;
    c.summon.out = None;
    c.executed += 1;
    c.events.push(MenuEvent::Executed(summon::KIND_UNSUMMON));
    true
}

/// A summon kind asked for directly (a host's request; the menu's confirm).
fn summon_kind(state: &mut StateData, cx: &mut Ctx, kind: u8) -> bool {
    match summon::slot_of_kind(kind) {
        Some(slot) => summon_execute(state, cx, slot),
        None => kind == summon::KIND_UNSUMMON && unsummon(cx),
    }
}

/// The gate of every command (controller slot 681 0x1413EC9F0; J 4): not in aim mode, the
/// command lock timer has run out, the latch is clear, he can act, and the current state is not
/// one of the refusing ones (of which the port has DIE).
pub(crate) fn gate(state: &StateData, cx: &Ctx) -> bool {
    let s = &*cx.pawn;
    s.magic.aim.mode == 0 && s.command.lock_timer <= 0.0 && !s.magic.command_latch && s.status.hp >= 1 && !matches!(state, StateData::Die(_)) && s.queue.queued_id() != Some(StateId::Die)
}

/// `CanExecuteCommand(kind)` (controller slot 682 0x141401F80).
pub(crate) fn can_execute(state: &StateData, cx: &mut Ctx, kind: u8) -> bool {
    if !gate(state, cx) || !nocombat_allows(kind) {
        return false;
    }
    if kind == spells::cmd::ATTACK {
        // `iface.CanAttack()` runs again inside `TryAttack`: both see the same state.
        true
    } else if spells::is_item(kind) {
        use_item::can_use_item(state, cx, kind, true)
    } else {
        fire_magic::can_magic(state, cx.pawn, kind)
    }
}

/// Controller slot 676 (0x141415A50): arms the command lock timer and the one-frame latch.
pub(crate) fn lock(cx: &mut Ctx, seconds: f32) {
    cx.pawn.command.lock_timer = seconds;
    cx.pawn.magic.command_latch = true;
}

/// `ExecuteCommand(kind, info)` (controller slot 680 0x14140EAA0) with `ExecCommand` (slot 678
/// 0x14141B470): THE one way a command starts. The pending command lives only inside this call.
/// A command that needs a target targets Sora (no party in the port: J 2.8).
pub(crate) fn execute(state: &mut StateData, cx: &mut Ctx, kind: u8) -> bool {
    cx.pawn.command.pending = kind;
    if !exec_command(state, cx) {
        // "+0x87C is left as is": a refused command is dropped, never retried.
        return false;
    }
    cx.pawn.command.pending = 0;
    cx.pawn.command.executed += 1;
    cx.pawn.command.events.push(MenuEvent::Executed(kind));
    true
}

fn exec_command(state: &mut StateData, cx: &mut Ctx) -> bool {
    let kind = cx.pawn.command.pending;
    if !gate(state, cx) || !nocombat_allows(kind) {
        return false;
    }
    let target = cx.pawn.combat.lockon.current;
    // The facing yaw: the stick's world yaw when the stick magnitude >= 0.1, else the actor yaw.
    let stick = cx.pawn.stick_magnitude;
    let yaw = if stick >= Sheet::Spells.num("cast.request.stick_yaw_min_mag") { cx.pawn.stick_yaw } else { cx.pawn.yaw };
    let ok = if kind == spells::cmd::ATTACK {
        attack::try_attack(state, cx, target)
    } else if spells::is_item(kind) {
        cx.pawn.magic.pending_cmd = kind;
        use_item::try_use_item(state, cx, kind)
    } else if spells::is_cure(kind) && !spells::is_grand(kind) {
        // Cure: the target is the party member chosen in the menu = Sora.
        cx.pawn.magic.pending_cmd = kind;
        fire_magic::try_magic(state, cx, TargetId::NONE, true, stick, yaw, false)
    } else if spells::is_grand(kind) {
        cx.pawn.magic.pending_cmd = kind;
        let cure = spells::is_cure(kind);
        let ok = fire_magic::try_magic(state, cx, if cure { TargetId::NONE } else { target }, cure, stick, yaw, true);
        if ok {
            // 0x14141B82D / 0x14141B844: the command lock for 0.1 s, the reaction block 0.1 s,
            // and the grand-magic bookkeeping is cleared (gauge, decay timers, both arrays).
            let t = Sheet::Spells.num("cast.request.grand_ctrl_timer");
            lock(cx, t);
            cx.pawn.command.reaction_block = t;
            crate::combat::style::clear_grand(cx);
            crate::magic::grand::reset(&mut cx.pawn.magic.grand);
        }
        ok
    } else if spells::is_magic(kind) {
        cx.pawn.magic.pending_cmd = kind;
        fire_magic::try_magic(state, cx, target, false, stick, yaw, false)
    } else {
        false
    };
    if ok {
        // 0x1414158F0: pawn+0xE2C = kind (the command-begin event), ctrl+0x880 = kind.
        cx.pawn.command.last_executed = kind;
        cx.pawn.command.last_shoot_flow = 0;
    }
    ok
}

/// `SetSlow(on)` (0x141615760): only with the config option, only on a change.
fn set_slow(cx: &mut Ctx, on: bool) {
    let on = on && cx.pawn.command.config.command_slow;
    if cx.pawn.command.slow_applied == on {
        return;
    }
    cx.pawn.command.slow_applied = on;
    cx.set_time_dilation(Sheet::Items.int("menu2.slow.objtype_mask") as u32, if on { Sheet::Items.num("menu2.slow.rate") } else { 1.0 });
    if on {
        // ctrl.vt+0x1968(): leave the shotlock aim.
        crate::magic::shotlock::exit(cx);
    }
}

fn close(cx: &mut Ctx, kind: CloseKind) {
    let c = &mut cx.pawn.command;
    c.sub_cursor = 0;
    if kind != CloseKind::Back {
        c.main_cursor = 0;
    }
    c.state = MenuState::MainSel;
    c.list = 0;
    c.events.push(MenuEvent::SubClosed(kind));
    set_slow(cx, false);
}

/// The shortcut press path (0x141623360; J 3.4).
fn shortcut(state: &mut StateData, cx: &mut Ctx, slot: usize) {
    let page = cx.pawn.command.shortcut_page as usize;
    let kind = cx.pawn.command.shortcuts[page][slot];
    if kind == 0 {
        return;
    }
    let r = row(cx, kind);
    if !r.enabled || !can_execute(state, cx, kind) {
        // (2026-10-28, the user: "remove the "error" sound from shortcut commands for every character, spamming magic makes
        // it constantly play", "only play the error sound when you don't have enough MP for it") the refused SE only for
        // a row short of MP or an MP command during MP Charge; any other refusal (busy, mid-cast) is silent
        let out_of_mp = r.mp_short || (cx.pawn.status.mp_charging && items::item_of_command(kind).is_none());
        if out_of_mp || !nocombat_allows(kind) {
            // (NOCOMBAT) a shortcut greyed by the no-combat rule is refused aloud, as any greyed entry
            cx.pawn.command.events.push(MenuEvent::Refused);
        }
        return;
    }
    if execute(state, cx, kind) {
        cx.pawn.command.events.push(MenuEvent::ShortcutExecuted);
    }
}

fn page_step(cx: &mut Ctx, delta: i8) {
    let c = &mut cx.pawn.command;
    c.shortcut_page = ((c.shortcut_page as i8 + delta).rem_euclid(3)) as u8;
}

/// The controller's part: the command timers (slot 635 0x1413F9790), on the controller's clock.
pub(crate) fn controller_tick(cx: &mut Ctx, dt: f32) {
    let c = &mut cx.pawn.command;
    c.lock_timer = (c.lock_timer - dt).max(0.0);
    c.reaction_block = (c.reaction_block - dt).max(0.0);
    // The grand-magic tallies follow the gauge (it ticked just before this call).
    crate::magic::grand::sync(cx);
}

/// THE UI PHASE of a step: the HUD part's update (0x14162FF80): the state machine with this
/// step's pad edges, then the snapshot.
pub(crate) fn ui_tick(state: &mut StateData, cx: &mut Ctx, input: &Input) {
    let status = cx.pawn.status.clone();
    cx.pawn.command.fill(&status);
    cx.pawn.command.events.clear();
    let pad = input.pad;
    let pressed = |b: PadButton| pad.pressed.has(b);
    // Commands a host asked for directly.
    for request in std::mem::take(&mut cx.pawn.command.requests) {
        match request {
            CommandRequest::Execute(kind) if summon::is_summon_kind(kind) => {
                if !summon_kind(state, cx, kind) {
                    cx.pawn.command.events.push(MenuEvent::Refused);
                }
            }
            CommandRequest::Execute(kind) => {
                if !can_execute(state, cx, kind) || !execute(state, cx, kind) {
                    cx.pawn.command.events.push(MenuEvent::Refused);
                }
            }
            CommandRequest::Shortcut(slot) => shortcut(state, cx, (slot as usize).min(3)),
            CommandRequest::ShortcutPage(delta) => page_step(cx, delta),
        }
    }
    // The HUD handles input only while the player's state is not DIE (and not STYLECHANGE), and
    // not while the shotlock panel is up.
    let aiming = cx.pawn.magic.aim.mode != 0;
    let dead = matches!(state, StateData::Die(_)) || cx.pawn.queue.queued_id() == Some(StateId::Die);
    if aiming {
        if cx.pawn.command.state == MenuState::SubSel {
            close(cx, CloseKind::Cancel);
        }
        cx.pawn.command.state = MenuState::ShootFlow;
    } else if cx.pawn.command.state == MenuState::ShootFlow {
        cx.pawn.command.state = MenuState::MainSel;
    }
    if !aiming && !dead {
        let lb = pad.held.has(PadButton::LeftShoulder) || pressed(PadButton::LeftShoulder);
        // MAIN -> SHORTCUT on L1 (pressed or held); the face buttons are tested the same frame.
        if cx.pawn.command.state == MenuState::MainSel && lb {
            cx.pawn.command.state = MenuState::Shortcut;
        }
        match cx.pawn.command.state {
            MenuState::Shortcut => {
                if !lb {
                    cx.pawn.command.state = MenuState::MainSel;
                } else {
                    // Circle, Triangle, Square, Cross = slots 0..3.
                    let slot = [PadButton::FaceRight, PadButton::FaceTop, PadButton::FaceLeft, PadButton::FaceBottom].iter().position(|b| pressed(*b));
                    match slot {
                        Some(slot) => shortcut(state, cx, slot),
                        None if pressed(PadButton::DPadUp) => page_step(cx, -1),
                        None if pressed(PadButton::DPadDown) => page_step(cx, 1),
                        None => {}
                    }
                }
            }
            MenuState::MainSel => {
                // Link is absent for Sora in the port: the cursor runs over three slots. (Summon, THE USER 2026-11-02) a
                // host with spirits gives the player's Sora KH3's fourth slot back, as "Summon": four.
                let count = if cx.pawn.command.summon.allowed { 4i8 } else { 3i8 };
                let c = &mut cx.pawn.command;
                c.main_cursor = c.main_cursor.min(count as u8 - 1);
                if pressed(PadButton::DPadUp) {
                    c.main_cursor = (c.main_cursor as i8 - 1).rem_euclid(count) as u8;
                } else if pressed(PadButton::DPadDown) {
                    c.main_cursor = (c.main_cursor as i8 + 1).rem_euclid(count) as u8;
                }
                let cursor = c.main_cursor;
                let decide = pressed(PadButton::ATTACK);
                // (PROMPT-UI) KH3 0x14158C180 -> 0x14162A440(HudCommand, cmd, 0): while a finish's extra command is on show
                // (StyleFinish slot 162 0x1413E3690) the Attack slot holds it (hud+0x9A8[0], the "ATTACK!" notice), so
                // Decide on Attack executes the extra; 0x141627150 puts Attack (0x1B) back when it goes. Y on slot B too.
                let extra = cx.pawn.combat.situation.extra.filter(|_| decide && cursor == 0);
                if let Some(kind) = extra {
                    crate::combat::situation::execute(state, cx, kind);
                } else if cursor == 0 {
                    if decide && !nocombat_allows(spells::cmd::ATTACK) {
                        cx.pawn.command.events.push(MenuEvent::Refused); // (NOCOMBAT) the greyed Attack
                    } else if decide && can_execute(state, cx, spells::cmd::ATTACK) {
                        execute(state, cx, spells::cmd::ATTACK);
                    }
                    // (weapons P2) KH3's Keyblade switch: D-pad right / left on Attack (request slots 234 / 235;
                    // status/keyblade.rs `request_switch`); nothing happens with one Keyblade
                    if pressed(PadButton::DPadRight) != pressed(PadButton::DPadLeft) {
                        crate::status::keyblade::request_switch(state, cx, pressed(PadButton::DPadRight));
                    }
                } else if cursor == 3 {
                    // (Summon) "Unsummon" acts on Decide at once (a quick command, no list); "Summon" opens the list of the
                    // equipped ashes, or is refused (KH3's refused sound: no bell, nothing equipped, MP Charge, the chant)
                    let view = summon_view(cx);
                    if view.out.is_some() {
                        if decide && !unsummon(cx) {
                            cx.pawn.command.events.push(MenuEvent::Refused);
                        }
                    } else if decide || pressed(PadButton::DPadRight) {
                        let rows = summon_list(cx);
                        if summon::main_row(&view, &cx.pawn.status).1.is_some() || !rows.iter().any(|r| r.enabled) {
                            cx.pawn.command.events.push(MenuEvent::Refused);
                        } else {
                            let c = &mut cx.pawn.command;
                            (c.state, c.list, c.sub_cursor) = (MenuState::SubSel, 3, 0);
                            c.events.push(MenuEvent::SubOpened);
                            set_slow(cx, true);
                        }
                    }
                } else if decide || pressed(PadButton::DPadRight) {
                    // Open the sub-list: it must have entries and an enabled row.
                    let rows = if cursor == 1 { magic_list(cx) } else { item_list(cx) };
                    if rows.is_empty() || !rows.iter().any(|r| r.enabled) {
                        cx.pawn.command.events.push(MenuEvent::Refused);
                    } else {
                        let c = &mut cx.pawn.command;
                        (c.state, c.list, c.sub_cursor) = (MenuState::SubSel, cursor, 0);
                        c.events.push(MenuEvent::SubOpened);
                        set_slow(cx, true);
                    }
                }
            }
            MenuState::SubSel => {
                let rows = match cx.pawn.command.list {
                    1 => magic_list(cx),
                    3 => summon_list(cx), // (Summon)
                    _ => item_list(cx),
                };
                let n = rows.len() as i8;
                if n == 0 {
                    close(cx, CloseKind::Cancel);
                } else {
                    let c = &mut cx.pawn.command;
                    c.sub_cursor = c.sub_cursor.min(n as u8 - 1);
                    if n >= 2 && pressed(PadButton::DPadUp) {
                        c.sub_cursor = (c.sub_cursor as i8 - 1).rem_euclid(n) as u8;
                    } else if n >= 2 && pressed(PadButton::DPadDown) {
                        c.sub_cursor = (c.sub_cursor as i8 + 1).rem_euclid(n) as u8;
                    }
                    if pressed(PadButton::ATTACK) {
                        // Confirm (0x1416236C0; J 2.7).
                        let entry = rows[c.sub_cursor as usize];
                        if summon::is_summon_kind(entry.kind) {
                            // (Summon) the chosen ash: refused (with KH3's sound) when greyed, in MP Charge or not now
                            if !entry.enabled || !summon_kind(state, cx, entry.kind) {
                                cx.pawn.command.events.push(MenuEvent::Refused);
                            } else {
                                close(cx, CloseKind::Decide);
                            }
                        } else if !entry.enabled || !can_execute(state, cx, entry.kind) {
                            cx.pawn.command.events.push(MenuEvent::Refused);
                        } else if execute(state, cx, entry.kind) && !cx.pawn.command.config.cursor_lock {
                            close(cx, CloseKind::Decide);
                        }
                    } else if pressed(PadButton::JUMP) {
                        // (DON-UI) the user (2026-10-22), for ALL menus: "when you press B it should bring you back to the
                        // previous menu, not to the origin menu": B goes back one level (the main list, cursor kept), as
                        // D-pad left; KH3 itself returned the cursor to Attack (CloseKind::Cancel)
                        close(cx, CloseKind::Back);
                    } else if pressed(PadButton::DPadLeft) {
                        close(cx, CloseKind::Back);
                    }
                }
            }
            MenuState::ShootFlow => {}
        }
    }
}

/// The snapshot of the menu after the step (`UiSnapshot::command`).
pub(crate) fn snapshot(cx: &mut Ctx) -> CommandSnapshot {
    let magic = magic_list(cx);
    let items = item_list(cx);
    // (Summon) KH3's fourth slot as "Summon" / "Unsummon" when the host has spirits for this Sora
    let summoning = cx.pawn.command.summon.allowed;
    let summons = if summoning { summon_list(cx) } else { Vec::new() };
    let fourth = if summoning { summon_main(cx) } else { CommandRow { kind: 0, name: "Link", enabled: false, ..CommandRow::default() } };
    let c = cx.pawn.command.clone();
    let main = [
        CommandRow { kind: spells::cmd::ATTACK, name: "Attack", enabled: true, ..CommandRow::default() },
        CommandRow { kind: 28, name: "Magic", enabled: magic.iter().any(|r| r.enabled), ..CommandRow::default() },
        CommandRow { kind: 57, name: "Items", enabled: items.iter().any(|r| r.enabled), ..CommandRow::default() },
        fourth,
    ];
    let page = c.shortcut_page as usize;
    let mut shortcut_rows = [CommandRow::default(); 4];
    for (slot, kind) in c.shortcuts[page].iter().enumerate() {
        shortcut_rows[slot] = row(cx, *kind);
    }
    let aim = &cx.pawn.magic.aim;
    let shoot_flow = (aim.mode != 0).then_some(ShootFlowPanel { kind: aim.kind, enabled: [aim.total > 0, aim.can_airstep], locks: aim.total, max_locks: aim.max_locks, aim_time: aim.aim_time, aim_time_start: aim.focus0 as f32 * crate::status::runtime::FOCUS_CONVERT_RATE, is_max: aim.is_max });
    let mut snap = CommandSnapshot {
        state: c.state,
        main,
        link_none: !summoning,
        main_cursor: c.main_cursor as usize,
        list: c.list as usize,
        lists: [Vec::new(), magic, items, summons],
        row_cursor: c.sub_cursor as usize,
        shortcut_page: page,
        shortcut: shortcut_rows,
        shoot_flow,
        situations: Vec::new(),
        situation_selected: None,
        mp_charging: cx.pawn.status.mp_charging,
        events: c.events,
    };
    crate::nocombat::grey(&mut snap); // (NOCOMBAT) every entry but Items greys in the zone
    snap
}
