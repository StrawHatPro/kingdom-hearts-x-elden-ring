//! (party P) THE COMMAND MENU OF A PLAYABLE FRIEND (port design: KH3 has no playable Donald / Goofy).
//!
//! The same state machine as Sora's (kh-core `command::ui_tick`, magic spec J 2 / UI spec 5): main list Attack /
//! Magic / Items (Link absent), D-pad up / down with wrap, A on Attack attacks, A or D-pad right opens a list that
//! has an enabled row, in a list D-pad up / down / A decide / B cancel (cursor back to Attack) / D-pad left back
//! (cursor stays); LB held: the shortcut panel (A / B / X / Y = slots, our design: the friend's first four list
//! entries, there is no shortcut save data for a friend). The snapshot is kh-core's `CommandSnapshot`, so the game
//! HUD draws it unchanged.
//!
//! What differs from Sora's, by the user's request (2026-10-15): Goofy's "Magic" row is labelled "Abilities" and
//! lists his specials (Goofy Tornado / Goofy Turbo / Goofy Discus); Donald's lists his five spells (Fire, Blizzard,
//! Thunder, Aero, Cure: abilities DONALD_* of `TresAbilityData`, their MP costs from the same rows through
//! `FriendStatus::mp_cost`). The Items list is the friend's own item slots (`TresChrInitData` / the camp menu).

use crate::command::ActionDef;
use crate::ids::Chara;
use crate::status::FriendStatus;
use kh_core::command::{CommandRow, CommandSnapshot, MenuEvent, MenuState};
use kh_core::input::{PadButton, PadState as Buttons};
use kh_core::status::ItemKind;

/// One entry of the friend's Magic / Abilities list: the action and the text the HUD shows (`kh3_system` en).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spell {
    pub def: ActionDef,
    pub name: &'static str,
    /// `TresAbilityData` row name (MP cost, equipped check).
    pub ability: &'static str,
}

/// The friend's list in menu order (the order of `TresChrInitData` equipped abilities [A]).
pub fn spells(chara: Chara) -> &'static [Spell] {
    // (DON-PROG) Donald's KH3 five-spell list is gone with its abilities: his menu is the overhaul's (DON-UI, `don_tick`;
    // every Donald has `FriendStatus::donald`, so this list was never reached any more)
    const DONALD: [Spell; 0] = [];
    const GOOFY: [Spell; 3] = [
        Spell { def: ActionDef::GoofyTornado, name: "Goofy Tornado", ability: "GOOFY_TORNADE" },
        Spell { def: ActionDef::GoofyTurbo, name: "Goofy Turbo", ability: "GOOFY_TURBO" },
        Spell { def: ActionDef::GoofyDiscus, name: "Goofy Discus", ability: "GOOFY_FLIP" },
    ];
    match chara {
        Chara::Donald => &DONALD,
        Chara::Goofy => &GOOFY,
    }
}

/// The label of the second main row: "Magic" for Donald, "Abilities" for Goofy (the user's request).
pub fn list_label(chara: Chara) -> &'static str {
    match chara {
        Chara::Donald => "Magic",
        Chara::Goofy => "Abilities",
    }
}

/// What the menu asked for this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuAction {
    Attack,
    Spell(ActionDef),
    Item(ItemKind),
    /// (party X2) a leading Donald's Sora spell (command 29 .. 51), from his Magic list / shortcuts.
    SoraSpell(u8),
    /// (party X2) Y on a leading Donald's grand-magic offer (serial, S_* command).
    Grand(u32, u8),
    /// (party X2) a leading Goofy's ability at a tier (1 ..= 3).
    Tiered(ActionDef, u8),
    /// (DON-UI) Donald's spell from his menu / a shortcut (documentation/DONALD_OVERHAUL.md 6.1, 7): `target` is the
    /// party member the ally target list picked (Aero Barrier, Ball Lightning; `donald::cast::SORA_MEMBER` = Sora),
    /// None = the lock-on / soft target. DON-KIT routes it (`pad::donald::wants_cast`).
    DonCast { cmd: crate::donald::DonCmd, target: Option<crate::ids::MemberId> },
    /// (DON-UI) Attune to an element (the Attunement submenu or a shortcut; DON-KIT: `pad::donald::begin_attune`).
    DonAttune(crate::donald::Element),
    /// (DON-UI) Unattune: every attunement goes, plain missiles again (the Attunement submenu's last row or a shortcut;
    /// DON-KIT's route: `pad::donald::begin_unattune`).
    DonUnattune,
}

/// The friend menu's state.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FriendMenu {
    pub state: MenuState,
    pub main_cursor: u8,
    pub list: u8,
    pub sub_cursor: u8,
    pub events: Vec<MenuEvent>,
    /// (party X2) The leader's Customize block and Sora's magic (crate::leader_magic, fed by the host): when its `kind`
    /// is set the Magic / Abilities list, the shortcuts and the situation slot are the leader's own.
    pub leader: crate::leader_magic::MenuFeed,
    /// (DON-UI) Donald's own menu (the overhaul): its state and the host's feed.
    pub don: DonMenu,
}

/// (party X2) The leader's own action of a list entry (a Sora spell for Donald, a tiered ability for Goofy, an item).
fn leader_action(feed: &crate::leader_magic::MenuFeed, status: &FriendStatus, kind: u8) -> Option<MenuAction> {
    use kh_core::command::leader::{goofy, LeaderKind};
    if let Some(item) = kh_core::magic::items::item_of_command(kind) {
        return items(status).iter().any(|(i, n)| *i == item && *n > 0).then_some(MenuAction::Item(item));
    }
    match feed.kind? {
        LeaderKind::Donald => kh_core::magic::spells::is_magic(kind).then_some(MenuAction::SoraSpell(kind)),
        LeaderKind::Goofy => goofy::split(kind).map(|(index, tier)| MenuAction::Tiered(crate::leader_magic::goofy_def(index), tier)),
    }
}

/// (2026-10-28, the user: "remove the "error" sound from shortcut commands for every character, spamming magic makes it
/// constantly play", "only play the error sound when you don't have enough MP for it") A refused SHORTCUT plays the
/// refused SE only when its row is short of MP, or is an MP command while MP Charge runs; any other refusal is silent.
fn out_of_mp(status: &FriendStatus, row: &CommandRow) -> bool {
    row.mp_short || (status.mp_charge_left > 0.0 && kh_core::magic::items::item_of_command(row.kind).is_none())
}

/// (party X2) The row of a leader's entry (the HUD's row for a slot or a shortcut).
fn leader_row(feed: &crate::leader_magic::MenuFeed, status: &FriendStatus, kind: u8) -> CommandRow {
    use kh_core::command::leader::{goofy, LeaderKind};
    if kind == 0 {
        return CommandRow::default();
    }
    if let Some(item) = kh_core::magic::items::item_of_command(kind) {
        let n = items(status).iter().find(|(i, _)| *i == item).map_or(0, |e| e.1);
        return item_row(item, n);
    }
    match feed.kind {
        Some(LeaderKind::Donald) => crate::leader_magic::spell_row(status, feed, kind),
        Some(LeaderKind::Goofy) if goofy::is_goofy(kind) => crate::leader_magic::goofy_row(status, kind),
        _ => CommandRow::default(),
    }
}

fn spell_row(status: &FriendStatus, s: &Spell) -> CommandRow {
    let cost = status.mp_cost(s.ability);
    let enabled = cost.is_some() && status.mp_charge_left <= 0.0;
    let short = cost.is_some_and(|c| c > 0 && status.mp < c);
    CommandRow { kind: 28, name: s.name, enabled, mp_short: short, quantity: 0, needs_target: false }
}

/// The distinct items still held, with their counts (slot order).
pub fn items(status: &FriendStatus) -> Vec<(ItemKind, u32)> {
    let mut out: Vec<(ItemKind, u32)> = Vec::new();
    for s in status.items.iter().take(status.item_slots as usize) {
        if let (Some(item), true) = (s.item, s.present) {
            match out.iter_mut().find(|(k, _)| *k == item) {
                Some(e) => e.1 += 1,
                None => out.push((item, 1)),
            }
        }
    }
    out
}

fn item_row(item: ItemKind, count: u32) -> CommandRow {
    let kind = kh_core::magic::items::command_of_item(item).unwrap_or(0);
    CommandRow { kind, name: kh_core::magic::spells::command_name(kind), enabled: count > 0, mp_short: false, quantity: count, needs_target: false }
}

impl FriendMenu {
    /// (DON-UI) THE Y LIST'S SELECTED ROW (6.3): the Arcane spell Y casts now (DON-KIT's Y press), None when the cursor
    /// is on a team attack or nothing is offered. A greyed row is returned too: the caster refuses it (KH3's refused SE),
    /// so a Y on it never falls through to a team attack.
    pub fn y_selected(&self) -> Option<crate::donald::SpellRef> {
        match self.don.y_rows.get(self.don.y_cursor as usize)? {
            YRow::Arcane(o) => Some(o.spell),
            YRow::Team(_) => None,
        }
    }

    fn rows(&self, chara: Chara, status: &FriendStatus, list: u8) -> Vec<CommandRow> {
        // (party X2) the leader's own Magic / Abilities page (his Customize block's slots)
        if let (Some(kind), 1) = (self.leader.kind, list) {
            return self.leader.custom.slots(kind).into_iter().map(|k| leader_row(&self.leader, status, k)).collect();
        }
        match list {
            1 => spells(chara).iter().map(|s| spell_row(status, s)).collect(),
            2 => items(status).into_iter().map(|(i, n)| item_row(i, n)).collect(),
            _ => Vec::new(),
        }
    }

    fn close(&mut self, back: bool) {
        self.sub_cursor = 0;
        if !back {
            self.main_cursor = 0;
        }
        self.state = MenuState::MainSel;
        self.list = 0;
    }

    /// One frame of the menu with the pad's edges (`dead`: the member is KO'd, the HUD takes no input).
    /// Returns what was decided; a decided row that cannot run is reported by the caller (`refused`).
    pub fn tick(&mut self, chara: Chara, status: &FriendStatus, pad: &Buttons, dead: bool) -> Option<MenuAction> {
        // (DON-UI) Donald's own menu
        if Self::is_donald(chara, status) {
            return self.don_tick(status, pad, dead);
        }
        self.events.clear();
        if dead {
            if self.state == MenuState::SubSel {
                self.close(false);
            }
            return None;
        }
        let pressed = |b: PadButton| pad.pressed.has(b);
        let lb = pad.held.has(PadButton::LeftShoulder) || pressed(PadButton::LeftShoulder);
        if self.state == MenuState::MainSel && lb {
            self.state = MenuState::Shortcut;
        }
        // (YHUD) LT cycles the leader's team rows (KH3's L2 over the situation commands; Sora's own queue is never cycled
        // by a friend leader's pad: kh-partyhost lead.rs `route`)
        if !lb && pressed(PadButton::SITUATION_NEXT) && self.leader.team.len() >= 2 {
            self.leader.y_cursor = ((self.leader.y_cursor as usize + 1) % self.leader.team.len()) as u8;
        }
        // (party X2) Y on the leader's own situation slot (Donald's grand-magic offer): the front offer whose appear
        // delay is over (Sora's queue rule: kh-core combat/situation.rs); not on the shortcut panel (LB + Triangle)
        if !lb && pressed(PadButton::FaceTop) && !self.leader.sora_busy {
            if let Some(o) = self.leader.offers.iter().find(|o| o.delay <= 0.0) {
                return Some(MenuAction::Grand(o.serial, o.cmd));
            }
        }
        match self.state {
            MenuState::Shortcut if self.leader.kind.is_some() => {
                // (party X2) THE LEADER'S OWN SHORTCUT SETS (his Customize block): the face buttons fire the set on the
                // panel, D-pad up / down flip A / B / C (Sora's `page_step`, kh-core command/mod.rs)
                if !lb {
                    self.state = MenuState::MainSel;
                    return None;
                }
                if pressed(PadButton::DPadUp) || pressed(PadButton::DPadDown) {
                    let d = if pressed(PadButton::DPadUp) { -1 } else { 1 };
                    self.leader.page = ((self.leader.page as i8 + d).rem_euclid(3)) as u8;
                    return None;
                }
                let slot = [PadButton::FaceRight, PadButton::FaceTop, PadButton::FaceLeft, PadButton::FaceBottom].iter().position(|b| pressed(*b))?;
                let kind = self.leader.custom.shortcuts[self.leader.page.min(2) as usize][slot];
                if kind == 0 {
                    return None;
                }
                let row = leader_row(&self.leader, status, kind);
                match leader_action(&self.leader, status, kind).filter(|_| row.enabled) {
                    Some(a) => {
                        self.events.push(MenuEvent::ShortcutExecuted);
                        Some(a)
                    }
                    None => {
                        if out_of_mp(status, &row) {
                            self.events.push(MenuEvent::Refused);
                        }
                        None
                    }
                }
            }
            MenuState::Shortcut => {
                if !lb {
                    self.state = MenuState::MainSel;
                    return None;
                }
                // Circle, Triangle, Square, Cross = slots 0..3 (the order of Sora's panel).
                let slot = [PadButton::FaceRight, PadButton::FaceTop, PadButton::FaceLeft, PadButton::FaceBottom].iter().position(|b| pressed(*b))?;
                let s = shortcut(chara, slot)?;
                let row = spell_row(status, &s);
                if !row.enabled {
                    if out_of_mp(status, &row) {
                        self.events.push(MenuEvent::Refused);
                    }
                    return None;
                }
                self.events.push(MenuEvent::ShortcutExecuted);
                Some(MenuAction::Spell(s.def))
            }
            MenuState::MainSel => {
                let count = 3i8;
                if pressed(PadButton::DPadUp) {
                    self.main_cursor = (self.main_cursor as i8 - 1).rem_euclid(count) as u8;
                } else if pressed(PadButton::DPadDown) {
                    self.main_cursor = (self.main_cursor as i8 + 1).rem_euclid(count) as u8;
                }
                let decide = pressed(PadButton::ATTACK);
                if self.main_cursor == 0 {
                    return decide.then_some(MenuAction::Attack);
                }
                if decide || pressed(PadButton::DPadRight) {
                    let rows = self.rows(chara, status, self.main_cursor);
                    if rows.is_empty() || !rows.iter().any(|r| r.enabled) {
                        self.events.push(MenuEvent::Refused);
                    } else {
                        (self.state, self.list, self.sub_cursor) = (MenuState::SubSel, self.main_cursor, 0);
                        self.events.push(MenuEvent::SubOpened);
                    }
                }
                None
            }
            MenuState::SubSel => {
                let rows = self.rows(chara, status, self.list);
                let n = rows.len() as i8;
                if n == 0 {
                    self.close(false);
                    self.events.push(MenuEvent::SubClosed(kh_core::command::CloseKind::Cancel));
                    return None;
                }
                self.sub_cursor = self.sub_cursor.min(n as u8 - 1);
                if n >= 2 && pressed(PadButton::DPadUp) {
                    self.sub_cursor = (self.sub_cursor as i8 - 1).rem_euclid(n) as u8;
                } else if n >= 2 && pressed(PadButton::DPadDown) {
                    self.sub_cursor = (self.sub_cursor as i8 + 1).rem_euclid(n) as u8;
                }
                if pressed(PadButton::ATTACK) {
                    let row = rows[self.sub_cursor as usize];
                    if !row.enabled {
                        self.events.push(MenuEvent::Refused);
                        return None;
                    }
                    let leader = self.leader.kind.and_then(|k| self.leader.custom.slots(k).get(self.sub_cursor as usize).copied()).filter(|_| self.list == 1);
                    let action = if let Some(kind) = leader {
                        // (party X2) the leader's own slot
                        match leader_action(&self.leader, status, kind) {
                            Some(a) => a,
                            None => {
                                self.events.push(MenuEvent::Refused);
                                return None;
                            }
                        }
                    } else if self.list == 1 {
                        MenuAction::Spell(spells(chara)[self.sub_cursor as usize].def)
                    } else {
                        MenuAction::Item(items(status)[self.sub_cursor as usize].0)
                    };
                    self.close(false);
                    self.events.push(MenuEvent::SubClosed(kh_core::command::CloseKind::Decide));
                    return Some(action);
                }
                // (DON-UI) B: back one level, the main list with its cursor kept (the user, 2026-10-22, for ALL menus: "when you press B it should bring you back to the previous menu, not to the origin menu")
                if pressed(PadButton::JUMP) || pressed(PadButton::DPadLeft) {
                    self.close(true);
                    self.events.push(MenuEvent::SubClosed(kh_core::command::CloseKind::Back));
                }
                None
            }
            MenuState::ShootFlow => {
                self.state = MenuState::MainSel;
                None
            }
        }
    }

    /// The menu as the HUD draws it (kh-core's snapshot type; `situations` stay the host's: Sora's queue).
    pub fn snapshot(&self, chara: Chara, status: &FriendStatus) -> CommandSnapshot {
        // (DON-UI) Donald's own menu
        if let Some(s) = Self::is_donald(chara, status).then(|| self.don_snapshot(status)).flatten() {
            return s;
        }
        let magic = self.rows(chara, status, 1);
        let items = self.rows(chara, status, 2);
        let main = [
            CommandRow { kind: kh_core::magic::spells::cmd::ATTACK, name: "Attack", enabled: true, ..CommandRow::default() },
            CommandRow { kind: 28, name: list_label(chara), enabled: magic.iter().any(|r| r.enabled), ..CommandRow::default() },
            CommandRow { kind: 57, name: "Items", enabled: items.iter().any(|r| r.enabled), ..CommandRow::default() },
            CommandRow { kind: 0, name: "Link", enabled: false, ..CommandRow::default() },
        ];
        let mut shortcut_rows = [CommandRow::default(); 4];
        for (slot, row) in shortcut_rows.iter_mut().enumerate() {
            if self.leader.kind.is_some() {
                // (party X2) the leader's own set on the panel
                *row = leader_row(&self.leader, status, self.leader.custom.shortcuts[self.leader.page.min(2) as usize][slot]);
            } else if let Some(s) = shortcut(chara, slot) {
                *row = spell_row(status, &s);
            }
        }
        // (party X2) the leader's grand-magic offers on his situation slot (the host puts Sora's queue behind them)
        // (YHUD) while a team attack WITH THIS LEADER stands, his Y rows are those (selected by LT); Sora's other rows never
        let situations: Vec<kh_core::command::SituationRow> = if self.leader.sora_busy { self.leader.team.clone() } else { self.leader.offers.iter().map(crate::leader_magic::offer_row).collect() };
        let situation_selected = (!situations.is_empty()).then(|| if self.leader.sora_busy { (self.leader.y_cursor as usize).min(situations.len() - 1) } else { 0 });
        CommandSnapshot {
            state: self.state,
            main,
            link_none: true,
            main_cursor: self.main_cursor as usize,
            list: self.list as usize,
            lists: [Vec::new(), magic, items, Vec::new()],
            row_cursor: self.sub_cursor as usize,
            shortcut_page: if self.leader.kind.is_some() { self.leader.page.min(2) as usize } else { 0 },
            shortcut: shortcut_rows,
            shoot_flow: None,
            situations,
            situation_selected,
            mp_charging: status.mp_charge_left > 0.0,
            events: self.events.clone(),
        }
    }
}

/// The shortcut slots (our design: the first four list entries; Donald's Cure on Cross, the game's habit).
pub fn shortcut(chara: Chara, slot: usize) -> Option<Spell> {
    let list = spells(chara);
    match chara {
        // Circle Fire, Triangle Blizzard, Square Thunder, Cross Cure
        Chara::Donald => [0usize, 1, 2, 4].get(slot).and_then(|&i| list.get(i).copied()), // (DON-PROG) the old list is empty: none
        Chara::Goofy => list.get(slot).copied(),
    }
}

// ================================================================================================================
// (DON-UI) DONALD'S COMMAND MENU (documentation/DONALD_OVERHAUL.md 6.1, 6.3, 6.4, 7, 9; build/sora_tmp/party/DON_PLAN.md
// R14 / R17 / R18 / R19). The user (2026-10-22): "Okay, implement the entire doc. He needs to play just as the doc
// specifies". Port design on KH3's command menu (kh-core `command::ui_tick` states, KH3's sub list and its party target
// list as the second level); Goofy's menu above is unchanged.
//
// - MAIN LIST: Attack / Magic / Attunement / Items (the four KH3 slots; Attunement is one D-pad press from Magic and from
//   Items).
// - MAGIC: the element sub-lists (his Magic page's order, Customize) Fire / Blizzard / Thunder / Water / Aero / Light,
//   each holding line 1 (`DonCore::line1_pick`: the -za tier at full power, else the highest up to -ga, R14), line 2 at
//   its learned level and the ultimate (greyed until full power and unused); HYBRID under Light only with Dual
//   Attunement and both elements attuned (6.4). A or D-pad right opens an element (the list is rebuilt in place, KH3's
//   "list rebuilt while open"), D-pad left goes back to the elements, B closes (KH3's cancel). Greyed rows carry the
//   reason / the MP they need.
// - ATTUNEMENT: the elements he has (locked ones hidden), the current one marked (greyed: he is on it), each one's
//   Resonance, the Release mark (switching now would Release), Elemental Insight's best element against the lock-on
//   target.
// - THE ALLY TARGET LIST (6.1): Aero Barrier and Ball Lightning open KH3's party target list (the second level): Donald,
//   then each friend of the active party; D-pad up / down, A casts, B back; the default is the last target. From a
//   shortcut the same list opens (one extra A).
// - THE USER'S MENU RULES (2026-10-22, replacing the doc's first "memory"): Magic ALWAYS opens the element select (its
//   cursor on the last element used, never entering it); after a spell or an attunement is chosen the menu collapses
//   with the cursor on Attack (sheet `menu.keep_cursor` = 0); B goes back exactly one level in every list.
// - SHORTCUTS (7): LB + A / B / X / Y over his block's three palettes (KH3's, flipped by LB + D-pad up / down); any menu
//   spell, any Attunement or an item. A line-1 tier is cast as `DonCore::shortcut_tier` (R14). QUICK CAST (9): while LB
//   is held the panel shows each spell's MP and marks the one that would trigger a Reaction on the lock-on target
//   (`react::resolve` on a copy of its statuses).
// - THE Y LIST (6.3, R18): Sora's team attacks (they stack, as today) and `DonCore::y_offers` (greyed with the MP they
//   need), stacked as KH3's situation commands; LT cycles; [`FriendMenu::y_selected`] is DON-KIT's Y press.
// ================================================================================================================

use crate::donald::{self as don, DonCmd, DonaldState, ElemSet, Element, Refusal, SpellRef, TargetStatus, YOffer};
use crate::ids::MemberId;
use kh_core::command::leader::{donald as block, LeaderCustom};
use kh_core::command::SituationRow;

/// One member of the ally target list (the active party, in the host's order: Donald first, then the friends).
#[derive(Clone, Debug, PartialEq)]
pub struct AllyRow {
    /// The member (`donald::cast::SORA_MEMBER` = Sora).
    pub id: MemberId,
    pub name: &'static str,
    pub hp: i32,
    pub hp_max: i32,
}

/// What the host feeds Donald's menu every step (kh-partyhost custom.rs `feed_don_menus`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DonFeed {
    /// Fed this step.
    pub on: bool,
    /// His Customize block (his palettes, his Magic page's element order).
    pub custom: LeaderCustom,
    /// The ally target list's members.
    pub members: Vec<AllyRow>,
    /// The lock-on target and a copy of its statuses (Quick Cast's reaction mark).
    pub lock: Option<(u32, TargetStatus)>,
    /// The lock-on target's (weak, resisted) elements when the host knows them (Elemental Insight).
    pub affinity: Option<(ElemSet, ElemSet)>,
    /// Sora's team attacks on offer (they stack on Y with his Arcane spells).
    pub team: Vec<SituationRow>,
    /// (DON-UI) He is in the air: the ground-only spells (the ultimates, Ultima, Zettaflare) grey with "On the ground
    /// only" in Magic, the Y list and the shortcuts, until he lands (the user, 2026-10-22).
    pub airborne: bool,
    /// (DON-UI) He has a target (the hard lock or the soft target): Zettaflare is offered on Y only with one (the user,
    /// 2026-10-22: "Zettaflare isn't castable without a target").
    pub has_target: bool,
}

/// Which list of his menu is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DonList {
    #[default]
    None,
    /// Magic, first level: the elements (+ Hybrid).
    Elements,
    /// Magic, an element's (or the Hybrid) spells.
    Spells(Group),
    Attune,
    Items,
}

/// A Magic sub-list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Elem(Element),
    Hybrid,
}

/// The ally target list (KH3's party target list, the second level).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetPick {
    pub cmd: DonCmd,
    pub cursor: u8,
    /// Opened from a shortcut (B goes back to the main list, its cursor where it was: `prev_main`).
    pub shortcut: bool,
    pub prev_main: u8,
}

/// One row of the Y list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum YRow {
    /// Sora's team attack (index into the feed's `team`).
    Team(usize),
    Arcane(YOffer),
}

/// Donald's menu state (inside [`FriendMenu`]).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DonMenu {
    pub feed: DonFeed,
    pub list: DonList,
    pub target: Option<TargetPick>,
    /// The remembered Magic sub-list and its row, the Elements row, the Attunement row (7's memory).
    pub mem_group: Option<Group>,
    pub mem_spell: u8,
    pub mem_element: u8,
    pub mem_attune: u8,
    /// The last member an ally spell went to.
    pub last_target: Option<MemberId>,
    /// The palette on the panel (0 A, 1 B, 2 C).
    pub page: u8,
    /// The Y list and its cursor (as computed at the last tick).
    pub y_rows: Vec<YRow>,
    pub y_cursor: u8,
    /// Quick Cast's view of the panel at the last tick: each slot's MP and the slot that would react.
    pub quick: Option<([Option<i32>; 4], Option<usize>)>,
    /// This menu's own switches (None: the installed ones, [`don_menu_tune`]).
    pub tune: Option<DonMenuTune>,
}

/// What a row of his lists does.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DonAct {
    Open(Group),
    Cast(DonCmd),
    Attune(Element),
    Item(ItemKind),
    Unattune,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DonRow {
    row: CommandRow,
    act: DonAct,
}

/// A row text built at run time (the HUD's rows carry `&'static str`): interned, a few hundred at most (names x reasons
/// x MP numbers).
pub fn intern(s: String) -> &'static str {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    static SET: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let set = SET.get_or_init(|| Mutex::new(HashSet::new()));
    let mut set = set.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = set.get(s.as_str()) {
        return s;
    }
    let s: &'static str = Box::leak(s.into_boxed_str());
    set.insert(s);
    s
}

/// KH3's inline icon of the IconText atlas (the menus' element icons; the port's text markup `<icon(NAME)>`, KH3's
/// `<img src='img://icontext:NAME'>`).
pub fn icon(name: &str) -> String {
    format!("<icon({name})>")
}

/// The IconText icon of an element (KH3's Fire / Blizzard / Thunder / Water / Aero; Light: the gold sparkles "Ability").
pub fn element_icon(e: Element) -> &'static str {
    match e {
        Element::Fire => "Fire",
        Element::Blizzard => "Blizzard",
        Element::Thunder => "Thunder",
        Element::Water => "Water",
        Element::Aero => "Aero",
        Element::Light => "Ability",
    }
}

/// The short reason a greyed row shows (6.1 "greyed rows with the reason / MP").
pub fn refusal_text(r: Refusal) -> String {
    match r {
        Refusal::NotEnoughMp { need } => format!("{need} MP"),
        Refusal::NeedsFullPower => "Res 3".to_string(),
        Refusal::UltimateSpent => "Used".to_string(),
        Refusal::NeedsBothAttuned => "Dual".to_string(),
        Refusal::NotEnoughCharges { need } => format!("{need} Flow"),
        Refusal::ZettaflareNotReady => "Res 3 all".to_string(),
        Refusal::Attuning => "Attuning".to_string(),
        Refusal::NeedsGround => "On the ground only".to_string(),
        Refusal::NotLearned | Refusal::NotCastable => String::new(),
    }
}

/// The main rows' labels.
pub const DON_MAIN: [&str; 4] = ["Attack", "Magic", "Attunement", "Items"];
/// The main slot of each list.
pub const SLOT_MAGIC: u8 = 1;
pub const SLOT_ATTUNE: u8 = 2;
pub const SLOT_ITEMS: u8 = 3;

/// The menu's switches (sheet `party-don-hud` rows `menu.*`; the host installs them with [`set_don_menu_tune`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DonMenuTune {
    /// The main cursor stays on Magic / Attunement after a cast from it; false (the user's rule, 2026-10-22: "selecting
    /// a magic spell or attunement should automatically select Attack again"): back to Attack.
    pub keep_cursor: bool,
    /// Greyed rows carry their reason / MP.
    pub reasons: bool,
    /// Element icons before the element rows (KH3's IconText).
    pub icons: bool,
}

impl Default for DonMenuTune {
    fn default() -> DonMenuTune {
        DonMenuTune { keep_cursor: false, reasons: true, icons: true }
    }
}

static MENU_TUNE: std::sync::RwLock<Option<DonMenuTune>> = std::sync::RwLock::new(None);

/// The menu's switches now.
pub fn don_menu_tune() -> DonMenuTune {
    MENU_TUNE.read().ok().and_then(|g| *g).unwrap_or_default()
}

/// Installs the menu's switches (the host, from the sheet / ctl).
pub fn set_don_menu_tune(t: DonMenuTune) {
    if let Ok(mut g) = MENU_TUNE.write() {
        *g = Some(t);
    }
}

/// Everything the rows are computed from.
struct DonCtx<'a> {
    st: &'a DonaldState,
    mp: i32,
    max_mp: i32,
    d: std::sync::Arc<don::DonData>,
    tune: DonMenuTune,
    /// (DON-UI) In the air (the feed's): the ground-only spells refuse with `NeedsGround`.
    airborne: bool,
}

impl<'a> DonCtx<'a> {
    fn of(status: &'a FriendStatus, tune: Option<DonMenuTune>) -> Option<DonCtx<'a>> {
        let st = status.donald.as_ref()?;
        Some(DonCtx { st, mp: status.mp, max_mp: status.max_mp().max(1), d: don::data(), tune: tune.unwrap_or_else(don_menu_tune), airborne: false })
    }

    fn dual(&self) -> bool {
        self.st.abilities.mods().dual
    }

    fn check(&self, s: SpellRef) -> Result<don::CastPlan, Refusal> {
        let r = self.st.core.check_cast(s, &self.st.learned, self.mp, self.max_mp, self.dual(), &self.d);
        // (DON-UI) on the ground only: in the air a spell he could otherwise cast (or only lacks the MP for) says so
        match r {
            Ok(_) | Err(Refusal::NotEnoughMp { .. }) if self.airborne && s.ground_only() => Err(Refusal::NeedsGround),
            r => r,
        }
    }

    /// The spell a command casts now (a shortcut's -za falls back to -ga below full power; a line-2 id at his level).
    fn spell_of(&self, cmd: DonCmd) -> Option<SpellRef> {
        match cmd {
            DonCmd::Spell(SpellRef::Line2(e, _)) => self.st.learned.line2_spell(e),
            DonCmd::Spell(s @ SpellRef::Line1(..)) => Some(self.st.core.shortcut_tier(s, &self.st.learned, &self.d)),
            DonCmd::Spell(s) => Some(s),
            DonCmd::Attune(_) => None,
        }
    }

    /// A spell row: enabled when castable now, else greyed with its reason.
    fn spell_row(&self, s: SpellRef) -> CommandRow {
        let name = display_name(s, &self.d);
        let (enabled, short, reason) = match self.check(s) {
            Ok(_) => (true, false, None),
            Err(r) => (false, matches!(r, Refusal::NotEnoughMp { .. }), Some(r)),
        };
        let text = match reason.map(refusal_text).filter(|t| self.tune.reasons && !t.is_empty()) {
            Some(t) => intern(format!("{name}  {t}")),
            None => name,
        };
        CommandRow { kind: DonCmd::Spell(s).id().unwrap_or(28), name: text, enabled, mp_short: short, quantity: 0, needs_target: s.ally_target() }
    }

    /// The hybrid his two attunements make (6.4: with Dual Attunement and both attuned).
    fn hybrid(&self) -> Option<SpellRef> {
        if !self.dual() {
            return None;
        }
        let el: Vec<Element> = self.st.core.elements().iter().collect();
        match el.as_slice() {
            [a, b] => SpellRef::hybrid(*a, *b).filter(|s| self.st.learned.knows(*s)),
            _ => None,
        }
    }

    /// An element's spells: line 1 (R14's pick), line 2 at his level, the ultimate.
    fn group_spells(&self, g: Group) -> Vec<SpellRef> {
        match g {
            Group::Hybrid => self.hybrid().into_iter().collect(),
            Group::Elem(e) => {
                let l = &self.st.learned;
                let mut out = Vec::new();
                out.extend(self.st.core.line1_pick(e, l, &self.d));
                out.extend(l.line2_spell(e));
                if l.knows(SpellRef::Ultimate(e)) {
                    out.push(SpellRef::Ultimate(e));
                }
                out
            }
        }
    }

    /// The Magic page's element order (his Customize page, else the menu order), the elements he has a spell of.
    fn elements(&self, custom: &LeaderCustom) -> Vec<Element> {
        let mut out: Vec<Element> = Vec::new();
        for k in custom.magic_slots {
            if let Some(e) = block::element_of(k).and_then(Element::from_index) {
                if !out.contains(&e) {
                    out.push(e);
                }
            }
        }
        for e in Element::ALL {
            if !out.contains(&e) {
                out.push(e);
            }
        }
        out.retain(|e| !self.group_spells(Group::Elem(*e)).is_empty());
        out
    }

    fn element_label(&self, e: Element) -> &'static str {
        if self.tune.icons {
            intern(format!("{} {}", icon(element_icon(e)), e.label()))
        } else {
            e.label()
        }
    }

    /// (DON-UI) The Attunement list's last row, UNATTUNE (the user, 2026-10-22): greyed while he is not attuned (or a cast
    /// runs), with the Release mark when an attuned element is at Resonance 3.
    fn unattune_row(&self) -> DonRow {
        let core = &self.st.core;
        let enabled = !core.elements().is_empty() && core.attune.cast.is_none();
        let text = if !core.would_release_unattune(&self.d).is_empty() { "Unattune Release" } else { "Unattune" };
        DonRow { row: CommandRow { kind: block::UNATTUNE, name: text, enabled, mp_short: false, quantity: 0, needs_target: false }, act: DonAct::Unattune }
    }

    /// The Attunement rows' text: the icon and name, the Resonance pips of an attuned one, the Release mark,
    /// Elemental Insight's best mark.
    fn attune_label(&self, e: Element, best: Option<Element>, m: &don::DonMods) -> &'static str {
        let core = &self.st.core;
        let mut s = if self.tune.icons { format!("{} {}", icon(element_icon(e)), e.label()) } else { e.label().to_string() };
        if let Some(r) = core.resonance(e) {
            let pips = self.d.tune.res_pips.round().max(1.0) as usize;
            let lit = (r + 1e-4).floor().clamp(0.0, pips as f32) as usize;
            s.push(' ');
            s.push_str(&PIP_ON.repeat(lit));
            s.push_str(&PIP_OFF.repeat(pips - lit));
        }
        if core.would_release(e, m, &self.d).is_some() {
            s.push_str(" Release");
        }
        if best == Some(e) {
            s.push(' ');
            s.push_str(&icon("StatusUp"));
        }
        intern(s)
    }
}

/// A spell's name in his menus: the sheet's, a line-2 spell without its " Lv N" (KH3's rows name a spell, not its
/// level: "Flare", "Aero Barrier"; Banish II / III keep their own names).
pub fn display_name(s: SpellRef, d: &don::DonData) -> &'static str {
    let n = s.name(d);
    match n.find(" Lv ") {
        Some(i) => &n[..i],
        None => n,
    }
}

/// The Resonance pips in a row's text.
pub const PIP_ON: &str = "\u{25CF}";
pub const PIP_OFF: &str = "\u{25CB}";

impl DonMenu {
    fn ctx_rows(&self, cx: &DonCtx, status: &FriendStatus, list: DonList) -> Vec<DonRow> {
        match list {
            DonList::None => Vec::new(),
            DonList::Elements => {
                let mut out: Vec<DonRow> = Vec::new();
                for e in cx.elements(&self.feed.custom) {
                    let enabled = cx.group_spells(Group::Elem(e)).iter().any(|s| cx.check(*s).is_ok());
                    out.push(DonRow { row: CommandRow { kind: 28, name: cx.element_label(e), enabled, mp_short: false, quantity: 0, needs_target: false }, act: DonAct::Open(Group::Elem(e)) });
                }
                if let Some(h) = cx.hybrid() {
                    out.push(DonRow { row: CommandRow { kind: 28, name: "Hybrid", enabled: cx.check(h).is_ok(), mp_short: false, quantity: 0, needs_target: false }, act: DonAct::Open(Group::Hybrid) });
                }
                out
            }
            DonList::Spells(g) => cx.group_spells(g).into_iter().map(|s| DonRow { row: cx.spell_row(s), act: DonAct::Cast(DonCmd::Spell(s)) }).collect(),
            DonList::Attune => {
                let m = cx.st.abilities.mods();
                let best = self.best(cx);
                Element::ALL
                    .into_iter()
                    .filter(|e| cx.st.learned.attune_unlocked(*e))
                    .map(|e| {
                        let enabled = !cx.st.core.attuned(e) && cx.st.core.attune.cast.is_none();
                        DonRow { row: CommandRow { kind: DonCmd::Attune(e).id().unwrap_or(0), name: cx.attune_label(e, best, &m), enabled, mp_short: false, quantity: 0, needs_target: false }, act: DonAct::Attune(e) }
                    })
                    .chain(std::iter::once(cx.unattune_row()))
                    .collect()
            }
            DonList::Items => items(status).into_iter().map(|(i, n)| DonRow { row: item_row(i, n), act: DonAct::Item(i) }).collect(),
        }
    }

    /// Elemental Insight's best element against the lock-on target.
    fn best(&self, cx: &DonCtx) -> Option<Element> {
        if !cx.st.abilities.has(don::DonAbility::ElementalInsight) {
            return None;
        }
        let (w, r) = self.feed.affinity?;
        don::hud::best_element(w, r, cx.st.learned.elements())
    }

    /// His palettes (the block; before the host fed one: the start's defaults from his book).
    fn palettes(&self, cx: &DonCtx) -> [[u8; 4]; 3] {
        if self.feed.custom.filled {
            self.feed.custom.shortcuts
        } else {
            block::default_shortcuts(&book_of(cx.st))
        }
    }

    /// The panel's row of a block entry, and what it does.
    fn shortcut_row(&self, cx: &DonCtx, status: &FriendStatus, kind: u8) -> Option<DonRow> {
        if kind == 0 {
            return None;
        }
        if let Some(item) = kh_core::magic::items::item_of_command(kind) {
            let n = items(status).iter().find(|(i, _)| *i == item).map_or(0, |e| e.1);
            return Some(DonRow { row: item_row(item, n), act: DonAct::Item(item) });
        }
        if kind == block::UNATTUNE {
            let mut r = cx.unattune_row();
            r.row.name = block::name(kind);
            return Some(r);
        }
        match DonCmd::from_id(kind)? {
            DonCmd::Attune(e) => {
                let enabled = cx.st.learned.attune_unlocked(e) && !cx.st.core.attuned(e) && cx.st.core.attune.cast.is_none();
                Some(DonRow { row: CommandRow { kind, name: block::name(kind), enabled, mp_short: false, quantity: 0, needs_target: false }, act: DonAct::Attune(e) })
            }
            cmd => {
                let s = cx.spell_of(cmd)?;
                let mut row = cx.spell_row(s);
                row.kind = kind;
                // (2026-10-28, the user: "remove how much MP something costs from Donald's shortcut commands") a row short
                // of MP shows its plain name (still greyed, `mp_short` kept); the Magic list's rows keep their reason
                if row.mp_short {
                    row.name = display_name(s, &cx.d);
                }
                Some(DonRow { row, act: DonAct::Cast(DonCmd::Spell(s)) })
            }
        }
    }

    /// The four panel rows of the palette on show.
    fn panel(&self, cx: &DonCtx, status: &FriendStatus) -> [Option<DonRow>; 4] {
        let p = self.palettes(cx)[self.page.min(2) as usize];
        [0, 1, 2, 3].map(|i| self.shortcut_row(cx, status, p[i]))
    }

    /// QUICK CAST (9): each panel spell's MP and the slot whose spell would trigger a Reaction on the lock-on target.
    fn quick_cast(&self, cx: &DonCtx, panel: &[Option<DonRow>; 4]) -> Option<([Option<i32>; 4], Option<usize>)> {
        if !cx.st.abilities.has(don::DonAbility::QuickCast) {
            return None;
        }
        let mut costs = [None; 4];
        let mut glow = None;
        for (i, r) in panel.iter().enumerate() {
            let Some(DonRow { act: DonAct::Cast(DonCmd::Spell(s)), .. }) = r else { continue };
            costs[i] = match cx.check(*s) {
                Ok(p) => Some(p.mp),
                Err(Refusal::NotEnoughMp { need }) => Some(need),
                Err(_) => None,
            };
            if glow.is_none() && reacts_on(cx.st, *s, self.feed.lock.as_ref().map(|l| &l.1), &cx.d) {
                glow = Some(i);
            }
        }
        Some((costs, glow))
    }

    /// The Y list (6.3): Sora's team attacks first (as today), then the Arcane offers.
    /// (DON-UI stub, until ZETTA's cast gate lands in `DonCore::check_cast`) Zettaflare needs a target: the lock-on or
    /// the soft target (the feed's).
    fn y_target_ok(&self, s: SpellRef) -> bool {
        !matches!(s, SpellRef::Arcane(don::ArcaneLine::Zettaflare, _)) || self.feed.has_target
    }

    fn y_list(&self, cx: &DonCtx) -> Vec<YRow> {
        let mut out: Vec<YRow> = (0..self.feed.team.len()).map(YRow::Team).collect();
        // (DON-UI) the user, 2026-10-22: "the Y commands should not say things like 'on the ground only' or their MP cost,
        // they are either available or they are not same as whether you have the correct amount of stars": offered
        // only when castable right now (charges, MP, on the ground, every other condition), never greyed
        out.extend(cx.st.core.y_offers(&cx.st.learned, cx.mp, cx.max_mp, &cx.d).into_iter().filter(|o| o.affordable && cx.check(o.spell).is_ok() && self.y_target_ok(o.spell)).map(YRow::Arcane));
        out
    }
}

/// His learned magic as his block reads it.
pub fn book_of(st: &DonaldState) -> block::DonBook {
    block::DonBook { tier: st.learned.tier, line2: st.learned.line2, dual: st.abilities.mods().dual }
}

/// The spell would trigger a good Reaction on the target's statuses (Quick Cast's mark): `react::resolve` on a copy.
pub fn reacts_on(st: &DonaldState, s: SpellRef, target: Option<&TargetStatus>, d: &don::DonData) -> bool {
    let Some(t) = target else { return false };
    let mut copy = *t;
    let applies = s.row(d).map(|r| don::Apply::parse_list(r.applies)).unwrap_or_default();
    let full = st.full_flags(d);
    let out = don::resolve(&mut copy, don::HitTag::of(s.elements()), &applies, full, &d.tune, st.abilities.mods().reaction_plus);
    out.reactions.iter().any(|r| r.rewards())
}

/// The Y row as the HUD's situation command (icon 4 friend for a team attack, 3 magic for an Arcane spell; a greyed one
/// shows the MP it needs).
fn y_situation(feed: &DonFeed, r: &YRow, d: &don::DonData) -> SituationRow {
    match r {
        YRow::Team(i) => feed.team[*i],
        YRow::Arcane(o) => {
            // (DON-UI) THE STAR COST (the user, 2026-10-22: "the Y magic commands need to indicate how many stars they
            // cost"): drawn by the HUD on the row's plate (`DonMenuView::y_costs`); the serial carries the cost too
            // only the name (the user: no reason, no MP): a row on show is castable (`FriendMenu::y_list`)
            let n = match o.spell {
                SpellRef::Arcane(l, lv) => l as u32 * 8 + lv as u32,
                _ => 0,
            };
            let _ = feed;
            // the HUD sets a situation button's text once, when it is added: the serial is the spell and its cost
            SituationRow { serial: 0x5D00_0000 | (o.charges as u32 & 0xF) << 12 | n, kind: 0, name: o.spell.name(d), icon_type: 3, total: 0.0, remaining: 0.0, enabled: true, refreshed: 0 }
        }
    }
}

impl FriendMenu {
    /// (DON-UI) Donald's own menu runs for this member (Donald with the overhaul's state).
    pub fn is_donald(chara: Chara, status: &FriendStatus) -> bool {
        chara == Chara::Donald && status.donald.is_some()
    }

    fn don_close(&mut self, kind: kh_core::command::CloseKind, keep: Option<u8>) {
        self.don.list = DonList::None;
        self.don.target = None;
        self.sub_cursor = 0;
        self.list = 0;
        self.state = MenuState::MainSel;
        if kind != kh_core::command::CloseKind::Back {
            self.main_cursor = keep.unwrap_or(0);
        }
        self.events.push(MenuEvent::SubClosed(kind));
    }

    fn don_open(&mut self, list: DonList, slot: u8, cursor: u8) {
        self.don.list = list;
        self.state = MenuState::SubSel;
        self.list = slot;
        self.sub_cursor = cursor;
    }

    /// Opens the ally target list for `cmd` (the default: the last target, else Donald).
    fn don_pick(&mut self, cmd: DonCmd, shortcut: bool) {
        let cursor = self.don.last_target.and_then(|t| self.don.feed.members.iter().position(|m| m.id == t)).unwrap_or(0) as u8;
        self.don.target = Some(TargetPick { cmd, cursor, shortcut, prev_main: self.main_cursor });
        self.events.push(MenuEvent::SubOpened);
    }

    /// (DON-UI) ONE FRAME OF DONALD'S MENU (the section text above).
    fn don_tick(&mut self, status: &FriendStatus, pad: &Buttons, dead: bool) -> Option<MenuAction> {
        use kh_core::command::CloseKind;
        self.events.clear();
        let cx = DonCtx { airborne: self.don.feed.airborne, ..DonCtx::of(status, self.don.tune)? }; // (DON-UI) ground-only spells grey in the air
        let pressed = |b: PadButton| pad.pressed.has(b);
        let lb = pad.held.has(PadButton::LeftShoulder) || pressed(PadButton::LeftShoulder);
        // the Y list (DON-KIT's Y reads it) and the panel's Quick Cast view, every frame
        let y_rows = self.don.y_list(&cx);
        let selected = self.don.y_rows.get(self.don.y_cursor as usize).copied();
        let same = |a: &YRow, b: &YRow| match (a, b) {
            (YRow::Arcane(x), YRow::Arcane(y)) => x.spell == y.spell,
            (YRow::Team(x), YRow::Team(y)) => self.don.feed.team.get(*x).map(|r| r.serial) == self.don.feed.team.get(*y).map(|r| r.serial),
            _ => false,
        };
        // the cursor follows its row when the list changes (a team attack came / went, a spell became listed)
        let follow = selected.and_then(|sel| y_rows.iter().position(|r| same(r, &sel))).unwrap_or(0);
        self.don.y_cursor = follow.min(y_rows.len().saturating_sub(1)) as u8;
        self.don.y_rows = y_rows;
        if !dead && pressed(PadButton::SITUATION_NEXT) && self.don.y_rows.len() >= 2 {
            self.don.y_cursor = ((self.don.y_cursor as usize + 1) % self.don.y_rows.len()) as u8;
        }
        let panel = self.don.panel(&cx, status);
        self.don.quick = self.don.quick_cast(&cx, &panel);
        if dead {
            if self.state == MenuState::SubSel {
                self.don_close(CloseKind::Cancel, None);
            }
            return None;
        }
        if self.state == MenuState::MainSel && lb {
            self.state = MenuState::Shortcut;
        }
        match self.state {
            MenuState::Shortcut => {
                if !lb {
                    self.state = MenuState::MainSel;
                    return None;
                }
                if pressed(PadButton::DPadUp) || pressed(PadButton::DPadDown) {
                    let d = if pressed(PadButton::DPadUp) { -1 } else { 1 };
                    self.don.page = ((self.don.page as i8 + d).rem_euclid(3)) as u8;
                    return None;
                }
                let slot = [PadButton::FaceRight, PadButton::FaceTop, PadButton::FaceLeft, PadButton::FaceBottom].iter().position(|b| pressed(*b))?;
                let Some(r) = panel[slot].filter(|r| r.row.enabled) else {
                    if panel[slot].is_some_and(|r| matches!(r.act, DonAct::Cast(_)) && out_of_mp(status, &r.row)) {
                        self.events.push(MenuEvent::Refused);
                    }
                    return None;
                };
                match r.act {
                    DonAct::Cast(cmd @ DonCmd::Spell(s)) if s.ally_target() => {
                        // the ally target list opens over the element's list (one extra A)
                        let g = Group::Elem(s.element().unwrap_or(Element::Aero));
                        let row = cx.group_spells(g).iter().position(|x| *x == s).unwrap_or(0) as u8;
                        let prev = self.main_cursor;
                        self.main_cursor = SLOT_MAGIC;
                        self.don_open(DonList::Spells(g), SLOT_MAGIC, row);
                        self.events.push(MenuEvent::SubOpened);
                        self.don_pick(cmd, true);
                        if let Some(p) = self.don.target.as_mut() {
                            p.prev_main = prev;
                        }
                        None
                    }
                    DonAct::Cast(cmd) => {
                        self.events.push(MenuEvent::ShortcutExecuted);
                        Some(MenuAction::DonCast { cmd, target: None })
                    }
                    DonAct::Attune(e) => {
                        self.events.push(MenuEvent::ShortcutExecuted);
                        Some(MenuAction::DonAttune(e))
                    }
                    DonAct::Unattune => {
                        self.events.push(MenuEvent::ShortcutExecuted);
                        Some(MenuAction::DonUnattune)
                    }
                    DonAct::Item(i) => {
                        self.events.push(MenuEvent::ShortcutExecuted);
                        Some(MenuAction::Item(i))
                    }
                    DonAct::Open(_) => None,
                }
            }
            MenuState::MainSel => {
                if pressed(PadButton::DPadUp) {
                    self.main_cursor = (self.main_cursor as i8 - 1).rem_euclid(4) as u8;
                } else if pressed(PadButton::DPadDown) {
                    self.main_cursor = (self.main_cursor as i8 + 1).rem_euclid(4) as u8;
                }
                let decide = pressed(PadButton::ATTACK);
                if self.main_cursor == 0 {
                    return decide.then_some(MenuAction::Attack);
                }
                if !(decide || pressed(PadButton::DPadRight)) {
                    return None;
                }
                let (list, cursor) = match self.main_cursor {
                    // (DON-UI) Magic ALWAYS opens the element select (the user, 2026-10-22: "Pressing Magic in the command menu
                    // should ALWAYS take you to the element select as Donald, not straight to the last element you did's
                    // spells"): its cursor rests on the last element used, never entering it
                    SLOT_MAGIC => (DonList::Elements, self.don.mem_element),
                    SLOT_ATTUNE => (DonList::Attune, self.don.mem_attune),
                    _ => (DonList::Items, 0),
                };
                let rows = self.don.ctx_rows(&cx, status, list);
                if rows.is_empty() || !rows.iter().any(|r| r.row.enabled) {
                    self.events.push(MenuEvent::Refused);
                    return None;
                }
                self.don_open(list, self.main_cursor, cursor.min(rows.len() as u8 - 1));
                self.events.push(MenuEvent::SubOpened);
                None
            }
            MenuState::SubSel => {
                // THE ALLY TARGET LIST (the second level)
                if let Some(mut pick) = self.don.target {
                    let n = self.don.feed.members.len().max(1) as i8;
                    if pressed(PadButton::DPadUp) {
                        pick.cursor = (pick.cursor as i8 - 1).rem_euclid(n) as u8;
                    } else if pressed(PadButton::DPadDown) {
                        pick.cursor = (pick.cursor as i8 + 1).rem_euclid(n) as u8;
                    }
                    self.don.target = Some(pick);
                    if pressed(PadButton::ATTACK) {
                        // the spell may have become uncastable meanwhile (MP, an attune cast)
                        if cx.spell_of(pick.cmd).is_none_or(|s| cx.check(s).is_err()) {
                            // (2026-10-28, `out_of_mp`) from a shortcut, the refused SE only for MP
                            let mp = cx.spell_of(pick.cmd).is_some_and(|s| matches!(cx.check(s), Err(Refusal::NotEnoughMp { .. })));
                            if !pick.shortcut || mp {
                                self.events.push(MenuEvent::Refused);
                            }
                            return None;
                        }
                        let who = self.don.feed.members.get(pick.cursor as usize).map(|m| m.id);
                        if who.is_some() {
                            self.don.last_target = who;
                        }
                        if !pick.shortcut {
                            self.don_remember(&cx, status);
                        }
                        let keep = if pick.shortcut { None } else { cx.tune.keep_cursor.then_some(SLOT_MAGIC) };
                        self.don_close(CloseKind::Decide, keep);
                        if pick.shortcut {
                            self.events.push(MenuEvent::ShortcutExecuted);
                        }
                        return Some(MenuAction::DonCast { cmd: pick.cmd, target: who });
                    }
                    // B: back one level (the user, 2026-10-22, for ALL menus: "when you press B it should bring you back to the previous menu, not to the origin menu"): the spell list, or (opened from a shortcut) the main list as it was
                    if pressed(PadButton::JUMP) || pressed(PadButton::DPadLeft) {
                        if pick.shortcut {
                            self.don_close(CloseKind::Back, None);
                            self.main_cursor = pick.prev_main;
                        } else {
                            self.don.target = None;
                            self.events.push(MenuEvent::SubClosed(CloseKind::Back));
                        }
                    }
                    return None;
                }
                let rows = self.don.ctx_rows(&cx, status, self.don.list);
                let n = rows.len() as i8;
                if n == 0 {
                    // the list emptied (a hybrid's pair let go): back to the elements, else closed
                    if matches!(self.don.list, DonList::Spells(_)) {
                        self.don.list = DonList::Elements;
                        self.sub_cursor = 0;
                        return None;
                    }
                    self.don_close(CloseKind::Cancel, None);
                    return None;
                }
                self.sub_cursor = self.sub_cursor.min(n as u8 - 1);
                if n >= 2 && pressed(PadButton::DPadUp) {
                    self.sub_cursor = (self.sub_cursor as i8 - 1).rem_euclid(n) as u8;
                } else if n >= 2 && pressed(PadButton::DPadDown) {
                    self.sub_cursor = (self.sub_cursor as i8 + 1).rem_euclid(n) as u8;
                }
                let r = rows[self.sub_cursor as usize];
                let open = pressed(PadButton::ATTACK) || (pressed(PadButton::DPadRight) && matches!(r.act, DonAct::Open(_)));
                if open {
                    if !r.row.enabled {
                        self.events.push(MenuEvent::Refused);
                        return None;
                    }
                    match r.act {
                        DonAct::Open(g) => {
                            self.don.mem_element = self.sub_cursor;
                            let back = if self.don.mem_group == Some(g) { self.don.mem_spell } else { 0 };
                            let len = self.don.ctx_rows(&cx, status, DonList::Spells(g)).len().max(1) as u8;
                            self.don.list = DonList::Spells(g);
                            self.sub_cursor = back.min(len - 1);
                            self.events.push(MenuEvent::SubOpened);
                            return None;
                        }
                        DonAct::Cast(cmd @ DonCmd::Spell(s)) if s.ally_target() => {
                            self.don_pick(cmd, false);
                            return None;
                        }
                        DonAct::Cast(cmd) => {
                            self.don_remember(&cx, status);
                            self.don_close(CloseKind::Decide, cx.tune.keep_cursor.then_some(SLOT_MAGIC));
                            return Some(MenuAction::DonCast { cmd, target: None });
                        }
                        DonAct::Attune(e) => {
                            self.don.mem_attune = self.sub_cursor;
                            self.don_close(CloseKind::Decide, cx.tune.keep_cursor.then_some(SLOT_ATTUNE));
                            return Some(MenuAction::DonAttune(e));
                        }
                        DonAct::Unattune => {
                            self.don.mem_attune = self.sub_cursor;
                            self.don_close(CloseKind::Decide, cx.tune.keep_cursor.then_some(SLOT_ATTUNE));
                            return Some(MenuAction::DonUnattune);
                        }
                        DonAct::Item(i) => {
                            self.don_close(CloseKind::Decide, None);
                            return Some(MenuAction::Item(i));
                        }
                    }
                }
                // B (and D-pad left): back one level (the user, 2026-10-22, for ALL menus: "when you press B it should bring you back to the previous menu, not to the origin menu"): an element's spells -> the elements, a first-level list -> the
                // main list with its cursor kept
                if pressed(PadButton::JUMP) || pressed(PadButton::DPadLeft) {
                    if let DonList::Spells(g) = self.don.list {
                        // back to the elements, on the element it came from
                        let elems = self.don.ctx_rows(&cx, status, DonList::Elements);
                        self.don.list = DonList::Elements;
                        self.sub_cursor = elems.iter().position(|e| e.act == DonAct::Open(g)).unwrap_or(0) as u8;
                    } else {
                        self.don_close(CloseKind::Back, None);
                    }
                }
                None
            }
            MenuState::ShootFlow => {
                self.state = MenuState::MainSel;
                None
            }
        }
    }

    /// The memory of a cast from the Magic lists: its sub-list and row.
    fn don_remember(&mut self, cx: &DonCtx, status: &FriendStatus) {
        if let DonList::Spells(g) = self.don.list {
            self.don.mem_group = Some(g);
            self.don.mem_spell = self.sub_cursor;
            let elems = self.don.ctx_rows(cx, status, DonList::Elements);
            if let Some(i) = elems.iter().position(|e| e.act == DonAct::Open(g)) {
                self.don.mem_element = i as u8;
            }
        }
    }

    /// (DON-UI) Donald's snapshot (the HUD's model, kh-core's type; the ally target list and Quick Cast's mark are the
    /// side channel [`FriendMenu::don_view`]).
    fn don_snapshot(&self, status: &FriendStatus) -> Option<CommandSnapshot> {
        let cx = DonCtx { airborne: self.don.feed.airborne, ..DonCtx::of(status, self.don.tune)? }; // (DON-UI) ground-only spells grey in the air
        let d = &self.don;
        let rows = |l: DonList| d.ctx_rows(&cx, status, l).into_iter().map(|r| r.row).collect::<Vec<_>>();
        let magic_open = matches!(d.list, DonList::Elements | DonList::Spells(_));
        let elements = rows(DonList::Elements);
        let magic = if magic_open { rows(d.list) } else { elements.clone() };
        let attune = rows(DonList::Attune);
        let items = rows(DonList::Items);
        let main = [
            CommandRow { kind: kh_core::magic::spells::cmd::ATTACK, name: DON_MAIN[0], enabled: true, ..CommandRow::default() },
            CommandRow { kind: 28, name: DON_MAIN[1], enabled: elements.iter().any(|r| r.enabled), ..CommandRow::default() },
            CommandRow { kind: 28, name: DON_MAIN[2], enabled: attune.iter().any(|r| r.enabled), ..CommandRow::default() },
            CommandRow { kind: 57, name: DON_MAIN[3], enabled: items.iter().any(|r| r.enabled), ..CommandRow::default() },
        ];
        let panel = d.panel(&cx, status);
        let mut shortcut = [CommandRow::default(); 4];
        for (slot, r) in panel.iter().enumerate() {
            if let Some(r) = r {
                // (2026-10-28, the user: "remove how much MP something costs from Donald's shortcut commands") no MP after
                // the name, Quick Cast's included: Quick Cast keeps only its Reaction mark (`quick`'s glow)
                shortcut[slot] = r.row;
            }
        }
        let situations: Vec<SituationRow> = d.y_rows.iter().map(|r| y_situation(&d.feed, r, &cx.d)).collect();
        let situation_selected = (!situations.is_empty()).then(|| (d.y_cursor as usize).min(situations.len() - 1));
        Some(CommandSnapshot {
            state: self.state,
            main,
            link_none: false,
            main_cursor: self.main_cursor as usize,
            list: self.list as usize,
            lists: [Vec::new(), magic, attune, items],
            row_cursor: self.sub_cursor as usize,
            shortcut_page: d.page.min(2) as usize,
            shortcut,
            shoot_flow: None,
            situations,
            situation_selected,
            mp_charging: status.mp_charge_left > 0.0,
            events: self.events.clone(),
        })
    }

    /// (DON-UI) THE HUD'S SIDE CHANNEL of Donald's menu (what kh-core's snapshot has no field for): the ally target list
    /// (names, HP, cursor) and Quick Cast's view of the panel. None for anyone else.
    pub fn don_view(&self, chara: Chara, status: &FriendStatus) -> Option<DonMenuView> {
        if !Self::is_donald(chara, status) {
            return None;
        }
        let d = &self.don;
        let target = d.target.map(|p| (d.feed.members.iter().map(|m| (m.id, m.name, m.hp, m.hp_max)).collect(), p.cursor as usize));
        let list = match d.list {
            DonList::None => 0,
            DonList::Elements => 1,
            DonList::Spells(Group::Elem(e)) => 2 + e.index() as u8,
            DonList::Spells(Group::Hybrid) => 8,
            DonList::Attune => 9,
            DonList::Items => 10,
        };
        let data = don::data();
        let y_costs = d
            .y_rows
            .iter()
            .filter_map(|r| match r {
                YRow::Arcane(o) => {
                    let s = y_situation(&d.feed, r, &data);
                    Some((s.serial, o.charges, s.enabled))
                }
                YRow::Team(_) => None,
            })
            .collect();
        Some(DonMenuView { target, quick: d.quick, y_rows: d.y_rows.len(), list, y_costs })
    }
}

/// What the HUD shows of Donald's menu beyond kh-core's snapshot.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DonMenuView {
    /// The ally target list: (member, name, HP, max HP) rows and the cursor.
    pub target: Option<(Vec<(MemberId, &'static str, i32, i32)>, usize)>,
    /// Quick Cast: each panel slot's MP and the slot that would trigger a Reaction.
    pub quick: Option<([Option<i32>; 4], Option<usize>)>,
    /// Rows on the Y stack.
    pub y_rows: usize,
    /// (DON-UI) His Y rows' star cost: (situation serial, stars, enabled) (the HUD draws the stars on each row).
    pub y_costs: Vec<(u32, u8, bool)>,
    /// Which of his lists is open (0 none, 1 the elements, 2 ..= 7 an element's spells, 8 the hybrid, 9 Attunement,
    /// 10 Items): the HUD replays KH3's sub-list opening when Magic's list changes in place (its row count changes).
    pub list: u8,
}

// ------------------------------------------------------------------------------------------- (DON-UI) the menu's tests
#[cfg(test)]
mod don_menu_tests {
    use super::*;
    use crate::donald::{attune::Slot, DonAbility, DonLearned, StatusBook};
    use kh_core::input::PadButtons;

    const SORA: MemberId = MemberId(7);

    fn wear(st: &mut DonaldState, k: DonAbility) {
        st.abilities.give(k);
        st.abilities.set_equipped(k, true);
    }

    struct T {
        m: FriendMenu,
        s: FriendStatus,
        held: PadButtons,
    }

    impl T {
        fn new(learned: DonLearned) -> T {
            let mut s = FriendStatus::new(Chara::Donald, 0);
            s.donald.as_mut().unwrap().learned = learned;
            s.mp = s.max_mp();
            let mut m = FriendMenu::default();
            m.don.feed = DonFeed {
                on: true,
                members: vec![
                    AllyRow { id: MemberId(0), name: "Donald", hp: 100, hp_max: 120 },
                    AllyRow { id: MemberId(1), name: "Goofy", hp: 150, hp_max: 200 },
                    AllyRow { id: SORA, name: "Sora", hp: 90, hp_max: 140 },
                ],
                ..DonFeed::default()
            };
            T { m, s, held: PadButtons::NONE }
        }

        fn st(&mut self) -> &mut DonaldState {
            self.s.donald.as_mut().unwrap()
        }

        fn attune(&mut self, e: Element, res: f32) {
            let st = self.st();
            let old = st.core.attune.slots[0];
            st.core.attune.slots = [Some(Slot { element: e, res, period: 1, ult_used: false, full_at: None }), old.filter(|_| st.abilities.mods().dual)];
        }

        /// One frame with `buttons` down (pressed = newly down), then one frame with nothing (the release).
        fn press(&mut self, buttons: &[PadButton]) -> Option<MenuAction> {
            let now = buttons.iter().fold(PadButtons::NONE, |p, b| p.with(*b));
            let a = self.m.tick(Chara::Donald, &self.s, &Buttons::from_held(now, self.held), false);
            self.held = now;
            a
        }

        fn up(&mut self) -> Option<MenuAction> {
            let a = self.m.tick(Chara::Donald, &self.s, &Buttons::from_held(PadButtons::NONE, self.held), false);
            self.held = PadButtons::NONE;
            a
        }

        fn tap(&mut self, b: PadButton) -> Option<MenuAction> {
            let a = self.press(&[b]);
            let r = self.up();
            a.or(r)
        }

        fn snap(&self) -> CommandSnapshot {
            self.m.snapshot(Chara::Donald, &self.s)
        }

        fn names(&self) -> Vec<String> {
            let s = self.snap();
            s.lists[s.list].iter().map(|r| strip(r.name)).collect()
        }
    }

    /// A row text without its inline icons.
    fn strip(t: &str) -> String {
        let mut out = String::new();
        let mut depth = 0;
        for c in t.chars() {
            match c {
                '<' => depth += 1,
                '>' if depth > 0 => depth -= 1,
                _ if depth == 0 => out.push(c),
                _ => {}
            }
        }
        out.trim().to_string()
    }

    fn cast(spell: SpellRef) -> Option<MenuAction> {
        Some(MenuAction::DonCast { cmd: DonCmd::Spell(spell), target: None })
    }

    use PadButton::{DPadDown as DOWN, DPadLeft as LEFT, DPadRight as RIGHT, DPadUp as UP, FaceBottom as A, FaceLeft as X, FaceRight as B, LeftShoulder as LB};

    #[test]
    fn the_main_list_is_attack_magic_attunement_items_and_attunement_is_one_press_from_magic_and_items() {
        let mut t = T::new(DonLearned::start());
        let s = t.snap();
        assert_eq!(s.main.map(|r| r.name), ["Attack", "Magic", "Attunement", "Items"]);
        assert!(!s.link_none, "four slots");
        assert_eq!(t.tap(A), Some(MenuAction::Attack));
        t.tap(DOWN);
        assert_eq!(t.m.main_cursor, SLOT_MAGIC);
        t.tap(DOWN);
        assert_eq!(t.m.main_cursor, SLOT_ATTUNE, "Magic -> Attunement: one press");
        t.tap(DOWN);
        assert_eq!(t.m.main_cursor, SLOT_ITEMS);
        t.tap(UP);
        assert_eq!(t.m.main_cursor, SLOT_ATTUNE, "Items -> Attunement: one press");
        t.tap(DOWN);
        t.tap(DOWN);
        assert_eq!(t.m.main_cursor, 0, "wraps to Attack");
        // Goofy's menu is unchanged
        let g = FriendStatus::new(Chara::Goofy, 0);
        let gm = FriendMenu::default().snapshot(Chara::Goofy, &g);
        assert_eq!(gm.main.map(|r| r.name), ["Attack", "Abilities", "Items", "Link"]);
        assert!(gm.link_none);
    }

    #[test]
    fn magic_lists_the_elements_then_each_element_s_three_lines() {
        // (DONLINE2, the user 2026-10-27: "Donald should not start with access to Flood or Flare") the start + Flare / Flood Lv 1
        // from their line-2 bosses, so that line 2 is listed
        let mut l = DonLearned::start();
        l.line2[Element::Fire.index()] = 1;
        l.line2[Element::Water.index()] = 1;
        let mut t = T::new(l);
        t.tap(DOWN);
        t.tap(A);
        assert_eq!(t.m.don.list, DonList::Elements);
        assert_eq!(t.names(), vec!["Fire", "Water"], "only the elements he has");
        t.tap(A);
        assert_eq!(t.m.don.list, DonList::Spells(Group::Elem(Element::Fire)));
        assert_eq!(t.names(), vec!["Fire", "Flare"], "line 1 at his tier, line 2 at his level");
        assert_eq!(t.tap(LEFT), None);
        assert_eq!(t.m.don.list, DonList::Elements, "D-pad left: back to the elements");
        assert_eq!(t.m.sub_cursor, 0, "on the element it came from");
        t.tap(DOWN);
        t.tap(RIGHT);
        assert_eq!(t.names(), vec!["Water", "Flood"], "D-pad right opens too");
        // B: back exactly one level (the user's rule for all menus): the spells -> the elements -> the main list (Magic)
        assert_eq!(t.tap(B), None);
        assert_eq!((t.m.state, t.m.don.list, t.m.sub_cursor), (MenuState::SubSel, DonList::Elements, 1), "back to the elements, on Water");
        t.tap(B);
        assert_eq!((t.m.state, t.m.main_cursor), (MenuState::MainSel, SLOT_MAGIC), "back to the main list, on Magic");

        // everything learned: -ga below full power, the ultimate greyed with its reason
        let mut t = T::new(DonLearned::all());
        t.tap(DOWN);
        t.tap(A);
        assert_eq!(t.names(), vec!["Fire", "Blizzard", "Thunder", "Water", "Aero", "Light"]);
        t.tap(A);
        let s = t.snap();
        let rows = &s.lists[1];
        assert_eq!(strip(rows[0].name), "Firaga");
        assert_eq!(strip(rows[1].name), "Flare");
        assert!(rows[2].name.starts_with("Inferno") && rows[2].name.contains("Res 3") && !rows[2].enabled, "the ultimate greyed until full power: {}", rows[2].name);
        // full power: Firaza and Inferno; then spent
        t.attune(Element::Fire, 3.0);
        let s = t.snap();
        assert_eq!(s.lists[1].iter().map(|r| strip(r.name)).collect::<Vec<_>>(), vec!["Firaza", "Flare", "Inferno"]);
        assert!(s.lists[1].iter().all(|r| r.enabled));
        t.st().core.attune.slots[0].as_mut().unwrap().ult_used = true;
        let s = t.snap();
        assert!(!s.lists[1][2].enabled && s.lists[1][2].name.contains("Used"));
        // not enough MP: greyed with the MP it needs
        t.s.mp = 0;
        let s = t.snap();
        assert!(!s.lists[1][0].enabled && s.lists[1][0].mp_short && s.lists[1][0].name.ends_with(" MP"), "{}", s.lists[1][0].name);
    }

    #[test]
    fn hybrid_sits_under_light_only_with_dual_and_both_attuned() {
        let mut t = T::new(DonLearned::all());
        t.attune(Element::Fire, 1.0);
        t.tap(DOWN);
        t.tap(A);
        assert!(!t.names().contains(&"Hybrid".to_string()), "no Dual Attunement");
        t.tap(B); // back one level: the main list, the cursor on Magic
        wear(t.st(), DonAbility::DualAttunement);
        t.attune(Element::Water, 0.0);
        assert_eq!(t.st().core.elements().len(), 2);
        t.tap(A);
        let names = t.names();
        assert_eq!(names.last().map(String::as_str), Some("Hybrid"), "{names:?}");
        assert_eq!(names[names.len() - 2], "Light", "under Light");
        for _ in 0..6 {
            t.tap(DOWN);
        }
        t.tap(A);
        assert_eq!(t.names(), vec!["Scald"], "Fire + Water");
        assert_eq!(t.tap(A), cast(SpellRef::Hybrid(Element::Fire, Element::Water)));
    }

    #[test]
    fn ally_spells_open_the_party_target_list_and_remember_the_last_target() {
        let mut t = T::new(DonLearned::all());
        let barrier = SpellRef::Line2(Element::Aero, 3);
        t.tap(DOWN);
        t.tap(A);
        for _ in 0..4 {
            t.tap(DOWN);
        }
        t.tap(A); // Aero
        t.tap(DOWN); // Aero Barrier
        assert_eq!(t.tap(A), None);
        let v = t.m.don_view(Chara::Donald, &t.s).unwrap();
        let (rows, cursor) = v.target.expect("the target list");
        assert_eq!(rows.iter().map(|r| r.1).collect::<Vec<_>>(), vec!["Donald", "Goofy", "Sora"], "Donald, then each friend");
        assert_eq!(cursor, 0, "the default: Donald");
        assert_eq!(rows[1].2, 150, "HP shown");
        // B goes back to the spell list
        t.tap(B);
        assert!(t.m.don.target.is_none() && t.m.state == MenuState::SubSel);
        t.tap(A);
        t.tap(DOWN);
        assert_eq!(t.tap(A), Some(MenuAction::DonCast { cmd: DonCmd::Spell(barrier), target: Some(MemberId(1)) }));
        // the cast collapses the menu onto Attack; the next time the list opens on Goofy (the last target)
        assert_eq!((t.m.state, t.m.main_cursor), (MenuState::MainSel, 0));
        t.tap(DOWN);
        t.tap(A);
        assert_eq!((t.m.don.list, t.m.sub_cursor), (DonList::Elements, 4), "the element select, on Aero");
        t.tap(A);
        assert_eq!((t.m.don.list, t.m.sub_cursor), (DonList::Spells(Group::Elem(Element::Aero)), 1), "Aero's list on Aero Barrier");
        t.tap(A);
        assert_eq!(t.m.don.target.unwrap().cursor, 1, "the last target");
        t.tap(UP);
        t.tap(UP);
        assert_eq!(t.tap(A), Some(MenuAction::DonCast { cmd: DonCmd::Spell(barrier), target: Some(SORA) }), "wraps to Sora");
        // from a shortcut: the same list, one extra A
        let mut t = T::new(DonLearned::all());
        t.m.don.feed.custom.filled = true;
        t.m.don.feed.custom.shortcuts[0] = [block::line2(2), 0, 0, 0];
        assert_eq!(t.press(&[LB]), None);
        assert_eq!(t.press(&[LB, B]), None);
        t.up();
        assert!(t.m.don.target.is_some_and(|p| p.shortcut));
        assert_eq!(t.tap(A), Some(MenuAction::DonCast { cmd: DonCmd::Spell(SpellRef::Line2(Element::Thunder, 3)), target: Some(MemberId(0)) }));
        assert_eq!(t.m.main_cursor, 0, "a shortcut leaves the cursor where KH3 does");
        // B on a shortcut's target list: back to the main list as it was (one level)
        t.press(&[LB]);
        t.press(&[LB, B]);
        t.up();
        assert!(t.m.don.target.is_some());
        t.tap(B);
        assert_eq!((t.m.state, t.m.main_cursor, t.m.don.target), (MenuState::MainSel, 0, None));
    }

    #[test]
    fn magic_always_opens_the_element_select_and_a_choice_returns_to_attack() {
        // the user's rules (2026-10-22): "selecting a magic spell or attunement should automatically select Attack
        // again"; "Pressing Magic in the command menu should ALWAYS take you to the element select"
        let mut t = T::new(DonLearned::all());
        t.tap(DOWN);
        t.tap(A);
        t.tap(DOWN);
        t.tap(DOWN); // Thunder
        t.tap(A);
        assert_eq!(t.tap(A), cast(SpellRef::Line1(Element::Thunder, 3)));
        assert_eq!((t.m.state, t.m.main_cursor), (MenuState::MainSel, 0), "the cursor back on Attack");
        // Magic: the element select, its cursor on the last element used (Thunder), never entering it
        t.tap(DOWN);
        t.tap(A);
        assert_eq!((t.m.don.list, t.m.sub_cursor), (DonList::Elements, 2));
        // into Thunder; B back to the elements (on Thunder), B back to the main list (on Magic)
        t.tap(A);
        assert_eq!(t.m.don.list, DonList::Spells(Group::Elem(Element::Thunder)));
        t.tap(B);
        assert_eq!((t.m.don.list, t.m.sub_cursor), (DonList::Elements, 2));
        t.tap(B);
        assert_eq!((t.m.state, t.m.main_cursor), (MenuState::MainSel, SLOT_MAGIC));
        // Attunement: a choice returns to Attack; its list opens on the last row
        t.tap(DOWN);
        t.tap(A);
        t.tap(DOWN);
        assert_eq!(t.tap(A), Some(MenuAction::DonAttune(Element::Blizzard)));
        assert_eq!((t.m.state, t.m.main_cursor), (MenuState::MainSel, 0));
        t.tap(DOWN);
        t.tap(DOWN);
        t.tap(A);
        assert_eq!((t.m.don.list, t.m.sub_cursor), (DonList::Attune, 1));
        t.tap(B);
        assert_eq!((t.m.state, t.m.main_cursor), (MenuState::MainSel, SLOT_ATTUNE), "B: back to the main list, on Attunement");
        // Items: B back to the main list too
        t.tap(DOWN);
        t.tap(A);
        assert_eq!(t.m.don.list, DonList::Items);
        t.tap(B);
        assert_eq!((t.m.state, t.m.main_cursor), (MenuState::MainSel, SLOT_ITEMS));
        // the sheet can keep the cursor on the list instead (row menu.keep_cursor = 1)
        t.m.don.tune = Some(DonMenuTune { keep_cursor: true, ..DonMenuTune::default() });
        t.tap(UP);
        t.tap(A);
        assert_eq!(t.tap(A), Some(MenuAction::DonAttune(Element::Blizzard)), "the list on its last row");
        assert_eq!(t.m.main_cursor, SLOT_ATTUNE);
    }

    #[test]
    fn goofys_b_goes_back_one_level() {
        let g = FriendStatus::new(Chara::Goofy, 0);
        let mut m = FriendMenu::default();
        let mut held = PadButtons::NONE;
        let mut tap = |m: &mut FriendMenu, b: PadButton| {
            let now = PadButtons::NONE.with(b);
            m.tick(Chara::Goofy, &g, &Buttons::from_held(now, held), false);
            held = now;
            m.tick(Chara::Goofy, &g, &Buttons::from_held(PadButtons::NONE, held), false);
            held = PadButtons::NONE;
        };
        tap(&mut m, DOWN);
        tap(&mut m, A);
        assert_eq!(m.state, MenuState::SubSel);
        tap(&mut m, B);
        assert_eq!((m.state, m.main_cursor), (MenuState::MainSel, 1), "back to the main list, on Abilities");
    }

    #[test]
    fn unattune_clears_every_attunement_releasing_the_full_ones() {
        // the user (2026-10-22): "Donald needs a way at the bottom of his attunement submenu to unattune and fire his
        // default missiles again"
        let d = don::data();
        let mut st = DonaldState::new();
        st.learned = DonLearned::all();
        let m = st.abilities.mods();
        assert_eq!(st.core.begin_unattune(&m, &d), Err(Refusal::NotCastable), "not attuned: nothing to let go");
        // Dual: Water at 1, Fire at 3 (its ultimate unused)
        wear(&mut st, DonAbility::DualAttunement);
        let m = st.abilities.mods();
        st.core.attune.slots = [Some(Slot { element: Element::Water, res: 1.0, period: 2, ult_used: false, full_at: None }), Some(Slot { element: Element::Fire, res: 3.0, period: 1, ult_used: false, full_at: Some(0.0) })];
        assert_eq!(st.core.would_release_unattune(&d), ElemSet::one(Element::Fire));
        // a cast like attuning: its length, interruptible (nothing changes)
        let len = st.core.begin_unattune(&m, &d).unwrap();
        assert!((len - d.tune.attune_cast).abs() < 1e-6, "the attune cast's length");
        assert_eq!(st.core.begin_unattune(&m, &d), Err(Refusal::Attuning));
        assert!(st.core.interrupt_attune());
        assert_eq!(st.core.elements().len(), 2, "an interrupted unattune changes nothing");
        assert!(!st.core.attune.unattuning);
        st.core.take_events();
        // again, to its end: both go, Fire is Released (+1 Arcane Flow), the badges and the ultimate mark go
        st.core.begin_unattune(&m, &d).unwrap();
        st.core.tick(len + 0.01, true, &m, &d);
        assert!(st.core.elements().is_empty());
        let ev = st.core.take_events();
        assert!(ev.contains(&don::DonEvent::Released { element: Element::Fire }) && !ev.contains(&don::DonEvent::Released { element: Element::Water }), "{ev:?}");
        assert_eq!(st.core.arcane.charges(), 1);
        assert_eq!(st.core.missile_element(0), None, "plain arcane missiles again");
        assert_eq!(st.core.ultimate_mark(Element::Fire, &st.learned, &d), don::UltMark::Hidden);
        let h = crate::donald::hud::DonHud::with(&st, &StatusBook::default(), None, None, &d);
        assert!(h.rings.is_empty() && h.attuning.is_none(), "no badge");
        // Swift / Instant Attune shorten it the same way; Instant: done at once
        st.core.attune.slots = [Some(Slot { element: Element::Water, res: 0.0, period: 3, ult_used: false, full_at: None }), None];
        wear(&mut st, DonAbility::InstantAttune);
        let m = st.abilities.mods();
        assert_eq!(st.core.begin_unattune(&m, &d), Ok(0.0));
        assert!(st.core.elements().is_empty());
        // attuning again starts a fresh period
        st.core.begin_attune(Element::Fire, &st.learned.clone(), &m, &d).unwrap();
        assert!(st.core.attuned(Element::Fire) && !st.core.attune.slot(Element::Fire).unwrap().ult_used);
    }

    #[test]
    fn unattune_is_the_attunement_lists_last_row_and_a_shortcut() {
        let mut t = T::new(DonLearned::all());
        t.tap(DOWN);
        t.tap(DOWN);
        t.tap(A);
        let s = t.snap();
        let last = s.lists[2].last().unwrap();
        assert_eq!((strip(last.name), last.enabled), ("Unattune".to_string(), false), "greyed while not attuned");
        t.tap(B);
        t.attune(Element::Fire, 3.0);
        t.tap(A);
        let s = t.snap();
        let last = s.lists[2].last().unwrap();
        assert!(last.enabled && last.name.contains("Release"), "Fire at 3: unattuning Releases it: {}", last.name);
        for _ in 0..6 {
            t.tap(DOWN);
        }
        assert_eq!(t.tap(A), Some(MenuAction::DonUnattune));
        assert_eq!((t.m.state, t.m.main_cursor), (MenuState::MainSel, 0), "back to Attack");
        // on a shortcut, as the Attunements
        t.m.don.feed.custom.filled = true;
        t.m.don.feed.custom.shortcuts[0] = [block::UNATTUNE, 0, 0, 0];
        t.press(&[LB]);
        assert_eq!(strip(t.snap().shortcut[0].name), "Unattune");
        assert_eq!(t.press(&[LB, B]), Some(MenuAction::DonUnattune));
        assert!(block::assignable(&book_of(t.st()), &[]).iter().any(|a| a.kind == block::UNATTUNE), "Customize lists it");
    }

    /// (DON-UI) THE USER'S RULE (2026-10-22): the ultimates, Ultima and Zettaflare are cast on the ground only. In the air
    /// they grey with "On the ground only" in Magic and on the shortcuts and leave the Y list; back on landing.
    #[test]
    fn ground_only_spells_grey_in_the_air_and_ungrey_on_landing() {
        let mut t = T::new(DonLearned::all());
        t.attune(Element::Fire, 3.0);
        t.m.don.feed.custom.filled = true;
        t.m.don.feed.custom.shortcuts[0] = [block::ultimate(0), block::line1(0, 1), 0, 0];
        for _ in 0..8 {
            let now = t.st().core.clock;
            t.st().core.arcane.gain(now, 20.0, 8);
        }
        t.tap(DOWN);
        t.tap(A);
        t.tap(A);
        let on_ground = |t: &T, air: bool| {
            let s = t.snap();
            let ult = &s.lists[1][2];
            assert!(strip(ult.name).starts_with("Inferno"));
            assert_eq!(ult.enabled, !air, "Inferno {}", ult.name);
            assert_eq!(ult.name.contains("On the ground only"), air, "{}", ult.name);
            assert!(s.lists[1][0].enabled && !s.lists[1][0].name.contains("ground"), "Firaza never grounded: {}", s.lists[1][0].name);
            // the Y list: Ultima offered on the ground, not at all in the air (no greyed row, no reason)
            assert_eq!(s.situations.iter().any(|r| r.name.starts_with("Ultima")), !air, "Ultima on Y");
            assert!(s.situations.iter().all(|r| r.enabled && !r.name.contains("ground")), "the Y rows on show are castable, names only");
            assert!(s.situations.iter().any(|r| r.name.starts_with("Comet")), "the other Y spells stay up");
            assert_eq!((s.shortcut[0].enabled, s.shortcut[0].name.contains("On the ground only")), (!air, air), "the shortcut: {}", s.shortcut[0].name);
            assert!(s.shortcut[1].enabled);
        };
        t.up();
        on_ground(&t, false);
        t.m.don.feed.airborne = true;
        t.up();
        on_ground(&t, true);
        assert_eq!(refusal_text(Refusal::NeedsGround), "On the ground only");
        // a Spent ultimate keeps its own reason in the air
        t.st().core.attune.slots[0].as_mut().unwrap().ult_used = true;
        assert!(t.snap().lists[1][2].name.contains("Used"));
        t.st().core.attune.slots[0].as_mut().unwrap().ult_used = false;
        // landing
        t.m.don.feed.airborne = false;
        t.up();
        on_ground(&t, false);
        assert!(SpellRef::Arcane(don::ArcaneLine::Zettaflare, 1).ground_only() && SpellRef::Ultimate(Element::Aero).ground_only());
        // Zettaflare on Y needs a target (the lock-on or the soft target)
        t.m.don.feed.has_target = false;
        assert!(!t.m.don.y_target_ok(SpellRef::Arcane(don::ArcaneLine::Zettaflare, 1)) && t.m.don.y_target_ok(SpellRef::Arcane(don::ArcaneLine::Ultima, 1)));
        t.m.don.feed.has_target = true;
        assert!(t.m.don.y_target_ok(SpellRef::Arcane(don::ArcaneLine::Zettaflare, 1)));
        assert!(!SpellRef::Arcane(don::ArcaneLine::Meteor, 1).ground_only() && !SpellRef::Line1(Element::Fire, 4).ground_only());
    }

    #[test]
    fn three_palettes_flip_with_lb_and_the_d_pad_and_hold_spells_and_attunements() {
        let mut t = T::new(DonLearned::start());
        // no block fed yet: the start's palettes; (DONLINE2) no Flare / Flood at the start: their slots stay empty until learned
        assert_eq!(t.press(&[LB]), None);
        assert_eq!(t.m.state, MenuState::Shortcut);
        let s = t.snap();
        assert_eq!(s.shortcut.map(|r| strip(r.name)), ["Fire", "", "Water", ""].map(String::from));
        assert_eq!(t.press(&[LB, B]), cast(SpellRef::Line1(Element::Fire, 1)), "LB + B: the first slot");
        t.press(&[LB]);
        t.press(&[LB, DOWN]);
        assert_eq!(t.snap().shortcut_page, 1);
        assert_eq!(strip(t.snap().shortcut[0].name), "Attune Fire");
        assert_eq!(t.press(&[LB, B]), Some(MenuAction::DonAttune(Element::Fire)), "an Attunement on a shortcut");
        t.press(&[LB]);
        t.press(&[LB, DOWN]);
        t.press(&[LB]);
        t.press(&[LB, DOWN]);
        assert_eq!(t.snap().shortcut_page, 0, "three palettes, wrapping");
        t.up();
        assert_eq!(t.m.state, MenuState::MainSel);
        // a -za on a shortcut casts -ga below full power (R14); Firaza at full power
        let mut t = T::new(DonLearned::all());
        t.m.don.feed.custom.filled = true;
        t.m.don.feed.custom.shortcuts[0] = [block::line1(0, 4), block::attune(3), 0, block::line1(5, 1)];
        t.press(&[LB]);
        assert_eq!(t.press(&[LB, B]), cast(SpellRef::Line1(Element::Fire, 3)));
        t.attune(Element::Fire, 3.0);
        t.press(&[LB]);
        assert_eq!(t.press(&[LB, B]), cast(SpellRef::Line1(Element::Fire, 4)));
        t.press(&[LB]);
        assert_eq!(t.press(&[LB, A]), cast(SpellRef::Line1(Element::Light, 1)), "LB + A: the bottom slot");
        t.press(&[LB]);
        assert_eq!(t.press(&[LB, X]), None, "empty slot");
    }

    #[test]
    fn quick_cast_shows_the_mp_and_marks_the_spell_that_would_react() {
        let mut t = T::new(DonLearned::all());
        t.m.don.feed.custom.filled = true;
        t.m.don.feed.custom.shortcuts[0] = [block::line1(0, 1), block::line1(2, 1), block::line1(3, 1), block::attune(4)];
        let mut wet = TargetStatus::new(don::Size::Medium);
        wet.wet = 5.0;
        t.m.don.feed.lock = Some((42, wet));
        t.press(&[LB]);
        assert_eq!(t.m.don.quick, None, "no Quick Cast: no costs");
        wear(t.st(), DonAbility::QuickCast);
        t.press(&[LB]);
        let (costs, glow) = t.m.don.quick.expect("Quick Cast");
        assert!(costs[0].is_some_and(|c| c > 0) && costs[3].is_none(), "{costs:?}");
        assert_eq!(glow, Some(1), "Thunder on a Wet target: Conduct");
        assert!(!t.snap().shortcut[0].name.contains("MP"), "no MP on the shortcuts (the user, 2026-10-28): {}", t.snap().shortcut[0].name);
        assert_eq!(t.m.don_view(Chara::Donald, &t.s).unwrap().quick.unwrap().1, Some(1));
        // Fire on Wet is a Fizzle: no mark
        t.m.don.feed.custom.shortcuts[0][1] = 0;
        t.press(&[LB]);
        assert_eq!(t.m.don.quick.unwrap().1, None, "Fizzle is not a reaction worth marking");
    }

    #[test]
    fn a_refused_shortcut_buzzes_only_for_mp() {
        // (2026-10-28, the user: "only play the error sound when you don't have enough MP for it")
        use kh_core::command::leader::donald as block;
        let mut t = T::new(DonLearned::all());
        t.m.don.feed.custom.filled = true;
        t.m.don.feed.custom.shortcuts[0] = [block::line1(0, 1), block::ultimate(0), 0, 0];
        t.press(&[LB]);
        // Inferno below full power: greyed for Resonance, not MP: silent
        assert_eq!(t.press(&[LB, PadButton::FaceTop]), None);
        assert!(!t.m.events.contains(&MenuEvent::Refused), "{:?}", t.m.events);
        t.press(&[LB]);
        // no MP: Fire is greyed for MP: the refused SE
        t.s.mp = 0;
        assert_eq!(t.press(&[LB, B]), None);
        assert!(t.m.events.contains(&MenuEvent::Refused), "{:?}", t.m.events);
    }

    #[test]
    fn attunement_marks_the_current_the_resonance_the_release_and_the_best_element() {
        let mut t = T::new(DonLearned::all());
        t.attune(Element::Fire, 3.0);
        t.tap(DOWN);
        t.tap(DOWN);
        t.tap(A);
        let s = t.snap();
        let rows = &s.lists[2];
        assert_eq!(rows.len(), 7, "six elements and Unattune");
        assert!(!rows[0].enabled, "the current one: greyed (he is on it)");
        assert!(strip(rows[0].name).contains(&PIP_ON.repeat(3)), "its Resonance: {}", rows[0].name);
        assert!(strip(rows[3].name).contains("Release"), "switching now Releases Fire: {}", rows[3].name);
        assert!(rows[0].name.contains("<icon(Fire)>"), "KH3's element icon");
        // below 3: no Release
        t.st().core.attune.slots[0].as_mut().unwrap().res = 2.0;
        let s = t.snap();
        assert!(!s.lists[2][3].name.contains("Release"));
        assert!(strip(s.lists[2][0].name).contains(&format!("{}{}", PIP_ON.repeat(2), PIP_OFF)));
        // locked elements are hidden
        let mut t = T::new(DonLearned::start());
        t.tap(DOWN);
        t.tap(DOWN);
        t.tap(A);
        assert_eq!(t.names().len(), 3, "Fire, Water, Unattune");
        assert_eq!(t.tap(A), Some(MenuAction::DonAttune(Element::Fire)));
        // Elemental Insight: the best element against the lock-on target
        let mut t = T::new(DonLearned::all());
        t.m.don.feed.affinity = Some((ElemSet::one(Element::Thunder), ElemSet::one(Element::Fire)));
        t.tap(DOWN);
        t.tap(DOWN);
        t.tap(A);
        assert!(!t.snap().lists[2][2].name.contains("StatusUp"), "no Elemental Insight");
        wear(t.st(), DonAbility::ElementalInsight);
        let s = t.snap();
        assert!(s.lists[2][2].name.contains("<icon(StatusUp)>"), "Thunder marked: {}", s.lists[2][2].name);
        assert!(!s.lists[2][0].name.contains("StatusUp"));
    }

    #[test]
    fn the_y_list_stacks_team_attacks_and_arcane_spells_and_lt_cycles() {
        let mut t = T::new(DonLearned::all());
        t.up();
        assert!(t.m.y_selected().is_none(), "no charges: nothing");
        assert!(t.snap().situations.is_empty());
        for _ in 0..2 {
            let now = t.st().core.clock;
            t.st().core.arcane.gain(now, 20.0, 4);
        }
        t.up();
        let s = t.snap();
        assert_eq!(s.situations.iter().map(|r| strip(r.name)).collect::<Vec<_>>(), vec!["Comet II", "Graviga"], "every Y spell at or below 2 charges");
        // (the user: "the Y magic commands need to indicate how many stars they cost") Comet 2 stars, Gravity 1 (user 2026-10-23)
        let v = t.m.don_view(Chara::Donald, &t.s).unwrap();
        assert_eq!(v.y_costs.iter().map(|c| (c.1, c.2)).collect::<Vec<_>>(), vec![(2, true), (1, true)], "the stars each costs, for the HUD");
        assert_eq!(v.y_costs.iter().map(|c| c.0).collect::<Vec<_>>(), s.situations.iter().map(|r| r.serial).collect::<Vec<_>>(), "by the rows' serials");
        assert!(s.situations.iter().all(|r| r.icon_type == 3 && r.enabled));
        assert_eq!(t.m.y_selected(), Some(SpellRef::Arcane(don::ArcaneLine::Comet, 2)));
        t.tap(PadButton::LeftTrigger);
        assert_eq!(t.m.y_selected(), Some(SpellRef::Arcane(don::ArcaneLine::Gravity, 3)), "LT cycles");
        assert_eq!(t.snap().situation_selected, Some(1));
        // a team attack stacks with them (first, as today); the cursor stays on its row
        t.m.don.feed.team = vec![SituationRow { serial: 9, kind: 1, name: "Flare Force", icon_type: 4, total: 5.0, remaining: 5.0, enabled: true, refreshed: 0 }];
        t.up();
        assert_eq!(t.snap().situations.len(), 3);
        assert_eq!(t.m.y_selected(), Some(SpellRef::Arcane(don::ArcaneLine::Gravity, 3)));
        t.tap(PadButton::LeftTrigger);
        assert_eq!(t.m.y_selected(), None, "on the team attack: Y is Sora's (as today)");
        // (the user: "they are either available or they are not") below their MP: not offered at all, no greyed row,
        // no MP text; only the team attack stays
        t.s.mp = 1;
        t.up();
        let s = t.snap();
        assert_eq!(s.situations.len(), 1, "only the team attack");
        assert!(s.situations.iter().all(|r| r.enabled && !r.name.contains("MP")));
        assert_eq!(t.m.y_selected(), None);
    }

    #[test]
    fn the_hud_snapshot_of_donald() {
        use crate::donald::hud::{DonHud, StatusIcon};
        let mut st = DonaldState::new();
        st.learned = DonLearned::all();
        let d = don::data();
        // not attuned, no charges: nothing to draw
        let h = DonHud::with(&st, &StatusBook::default(), None, None, &d);
        assert!(h.rings.is_empty() && h.attuning.is_none() && h.stars == 0 && h.target.is_none());
        assert_eq!(h.zetta.map(|z| (z.need.len(), z.got.len())), Some((6, 0)), "the Zettaflare tracker: one icon per learned element");
        st.core.attune.slots = [Some(Slot { element: Element::Water, res: 2.5, period: 2, ult_used: false, full_at: None }), Some(Slot { element: Element::Fire, res: 3.0, period: 1, ult_used: true, full_at: Some(0.0) })];
        st.core.attune.cast = Some(don::attune::AttuneCast { element: Element::Aero, left: 0.15, total: 0.6 });
        let now = st.core.clock;
        st.core.arcane.gain(now, 20.0, 4);
        st.core.fight_full.insert(Element::Fire);
        let mut book = StatusBook::default();
        let s = book.entry(7, don::Size::Medium);
        s.wet = 3.0;
        s.chill = 2;
        s.chill_left = 2.0;
        s.reveal = 1.0;
        let h = DonHud::with(&st, &book, Some(7), Some((ElemSet::one(Element::Thunder), ElemSet::NONE)), &d);
        assert_eq!(h.rings.len(), 2, "two rings with Dual");
        assert_eq!((h.rings[0].element, h.rings[0].lit(), h.rings[0].full), (Element::Water, 2, false), "primary first, 2.5 pips: 2 lit");
        assert_eq!(h.rings[0].color, Element::Water.color(&d.tune));
        assert_eq!((h.rings[1].lit(), h.rings[1].full, h.rings[1].ult), (3, true, don::UltMark::Spent));
        assert_eq!(h.rings[0].ult, don::UltMark::Waiting, "learned, unused, below 3: dim");
        let (e, _, share) = h.attuning.unwrap();
        assert_eq!(e, Element::Aero);
        assert!((share - 0.75).abs() < 1e-4);
        assert_eq!((h.stars, h.stars_max), (1, 4));
        assert!(h.star_left.is_some_and(|l| (l - 20.0).abs() < 1e-3));
        assert_eq!(h.zetta.unwrap().got, ElemSet::one(Element::Fire));
        let tg = h.target.clone().unwrap();
        assert_eq!(tg.icons, vec![StatusIcon::Chill(2), StatusIcon::Wet, StatusIcon::Reveal]);
        assert!(!tg.scan && tg.insight.is_none() && h.best.is_none(), "Scan / Elemental Insight not worn");
        wear(&mut st, DonAbility::Scan);
        wear(&mut st, DonAbility::ElementalInsight);
        let h = DonHud::with(&st, &book, Some(7), Some((ElemSet::one(Element::Thunder), ElemSet::NONE)), &d);
        let tg = h.target.unwrap();
        assert!(tg.scan, "Scan: the enemy's HP gauge (KH3)");
        assert_eq!(tg.insight, Some((ElemSet::one(Element::Thunder), ElemSet::NONE)));
        assert_eq!(h.best, Some(Element::Thunder));
        // a Frozen target shows Frozen in place of Chill; Exposed in place of Reveal
        let s = book.entry(7, don::Size::Medium);
        s.frozen = 2.0;
        s.exposed = true;
        let h = DonHud::with(&st, &book, Some(7), None, &d);
        assert_eq!(h.target.unwrap().icons, vec![StatusIcon::Frozen, StatusIcon::Wet, StatusIcon::Exposed]);
    }

    #[test]
    fn kh_core_s_donald_ids_are_doncmd_s() {
        for id in (block::FIRST..=block::LAST).filter(|id| *id != block::UNATTUNE) {
            let c = DonCmd::from_id(id).unwrap_or_else(|| panic!("{id:#x}"));
            assert_eq!(c.id(), Some(id));
            let d = don::data();
            let name = match c {
                DonCmd::Spell(SpellRef::Line2(e, _)) => SpellRef::Line2(e, 1).name(&d),
                DonCmd::Spell(s) => s.name(&d),
                DonCmd::Attune(_) => block::name(id),
            };
            assert_eq!(block::name(id), name, "kh-core's name of {id:#x} is the sheet's");
        }
        for k in 0..=255u8 {
            let want = DonCmd::from_sora_cmd(k).and_then(|c| c.id());
            if let Some(w) = want {
                assert_eq!(block::migrate(k), w, "{k}");
            }
        }
    }
}
