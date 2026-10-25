//! (YHUD) THE Y STACK BELONGS TO THE CONTROLLED CHARACTER (the user, 2026-10-23, Donald leading in Elden Ring: "some of
//! Sora's Y moves were still appearing in my HUD as Donald, ONLY the Y moves for the character you're playing as have
//! their Y moves show up on the HUD").
//!
//! THE RULE (generic, every leader, both hosts): the HUD's situation stack and the player's Y show and run only the Y
//! commands of the character the pad drives.
//! - Sora leads: Sora's own queue (kh-core `combat::situation`), unchanged; every team attack is his (they all include
//!   him). A friend's own Y (Donald's Arcane offers, Goofy's Bombardier) only exists on a pad friend: never on Sora's HUD.
//! - A friend leads: HIS OWN rows (Donald's Arcane `y_offers`, Goofy's Bombardier) plus the team attacks of Sora's queue
//!   THAT INCLUDE HIM (documentation/DONALD_OVERHAUL.md 6.6, "Donald's with Sora and with Goofy"; GK_GOOFY_DESIGN.md 4,
//!   "While AI Sora's team attack is on offer, Y and the slot are his"). Everything else of Sora's queue (his Grand
//!   Magic, formchanges, finishes, reaction / context commands, shotlock, links, other team attacks) is Sora's: not on
//!   the leader's HUD (kh-host `Hud::live_model`) and never run by the leader's Y (kh-partyhost lead.rs `route`: the Y
//!   reaches AI Sora only for the leader's selected team row, chosen in Sora's queue with `Queue::choose_serial`).
//!
//! Who a team command involves: the kinds of Sora's queue that the party offers (kh-party `team::TeamAttack::command`,
//! kh-core combat/friend.rs `needed`): 193 Trinity Guard (Donald AND Goofy), 195 Goofy Bombardier (Goofy), 207 Donald
//! Meteor and 208 Flare Force (Donald). Any other TEAM kind (another world's partner) involves neither friend.

use crate::ids::{Chara, MemberId};
use kh_core::combat::situation::{class, Queue};
use kh_core::command::{CommandSnapshot, SituationRow};

/// The friends a team command of Sora's queue involves (Sora is in every one: they run through him).
pub fn team_friends(kind: i32) -> &'static [Chara] {
    match kind {
        193 => &[Chara::Donald, Chara::Goofy],
        195 => &[Chara::Goofy],
        207 | 208 => &[Chara::Donald],
        _ => &[],
    }
}

/// A team command is the leader's (`leader` None = Sora leads: every team command is his).
pub fn team_is_leaders(kind: i32, leader: Option<Chara>) -> bool {
    match leader {
        None => true,
        Some(c) => team_friends(kind).contains(&c),
    }
}

/// The team attacks of Sora's queue that are the leading friend's, as Y rows (icon 4 "team"; the name is left empty:
/// the HUD names a row by its kind from the pack, kh-host `live_model`).
pub fn leader_team_rows(queue: &Queue, leader: Chara) -> Vec<SituationRow> {
    queue
        .list
        .iter()
        .filter(|e| !e.ex && e.class == class::TEAM && team_is_leaders(e.kind, Some(leader)))
        .map(|e| SituationRow { serial: e.serial, kind: e.kind.clamp(0, 255) as u8, name: "", icon_type: 4, total: e.total.max(0.0), remaining: e.remaining.max(0.0), enabled: e.appear <= 0.0, refreshed: 0 })
        .collect()
}

/// The serial of Sora's team row the leader's Y runs now: the leader's selected Y row when it is one of his team rows
/// (`team` = [`leader_team_rows`]). None: the leader's Y is his own (or nothing): it never reaches Sora's queue.
pub fn y_forward(snap: &CommandSnapshot, team: &[SituationRow]) -> Option<u32> {
    let row = snap.situations.get(snap.situation_selected?)?;
    team.iter().any(|t| t.serial == row.serial && t.enabled).then_some(row.serial)
}

impl crate::party::Party {
    /// (YHUD) The leader's team rows, after `set_leader_magic` each step: his menu stacks them on Y (first, as KH3's
    /// situation commands) and, while one stands, Y and the slot are the team attack's (`MenuFeed::sora_busy`).
    pub fn set_leader_team(&mut self, id: MemberId, rows: Vec<SituationRow>) {
        let Some(pad) = self.member_mut(id).and_then(|m| m.pad.as_mut()) else { return };
        let menu = &mut pad.menu.leader;
        menu.sora_busy = !rows.is_empty();
        menu.team = rows;
        if menu.y_cursor as usize >= menu.team.len() {
            menu.y_cursor = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::combat::situation::Entry;

    fn queue() -> Queue {
        let e = |class: u8, kind: i32, serial: u32| Entry { ex: false, class, kind, total: -1.0, remaining: -1.0, start_delay: 0.5, appear: 0.0, serial, selectable: true };
        let mut q = Queue::default();
        q.list = vec![e(class::GRAND_MAGIC, 30, 1), e(class::FORMCHANGE, 128, 2), e(class::TEAM, 193, 3), e(class::TEAM, 195, 4), e(class::TEAM, 207, 5), e(class::TEAM, 208, 6), e(class::TEAM, 199, 7)];
        q.slot_b = Some(e(class::FINISH, 171, 8));
        q
    }

    /// (YHUD) each leader's team rows: Donald 193 / 207 / 208, Goofy 193 / 195; never Sora's own nor another world's
    #[test]
    fn yhud_team_rows_are_the_leaders_only() {
        let q = queue();
        let serials = |c: Chara| leader_team_rows(&q, c).iter().map(|r| r.serial).collect::<Vec<_>>();
        assert_eq!(serials(Chara::Donald), vec![3, 5, 6]);
        assert_eq!(serials(Chara::Goofy), vec![3, 4]);
        assert!(team_is_leaders(199, None) && team_is_leaders(30, None), "Sora leads: every row of his queue is his");
        assert!(!team_is_leaders(199, Some(Chara::Donald)) && !team_is_leaders(199, Some(Chara::Goofy)));
    }

    /// (YHUD) the leader's Y reaches Sora's queue only on his selected, ready team row
    #[test]
    fn yhud_y_forward_only_on_the_leaders_team_row() {
        let q = queue();
        let team = leader_team_rows(&q, Chara::Donald);
        let arcane = SituationRow { serial: 0x5D00_1002, name: "Gravity", icon_type: 3, enabled: true, ..SituationRow::default() };
        let mut snap = CommandSnapshot { situations: team.iter().copied().chain([arcane]).collect(), situation_selected: Some(1), ..CommandSnapshot::default() };
        assert_eq!(y_forward(&snap, &team), Some(5));
        snap.situation_selected = Some(3);
        assert_eq!(y_forward(&snap, &team), None, "his Arcane row is his own cast");
        snap.situation_selected = None;
        assert_eq!(y_forward(&snap, &team), None);
        // a row of Sora's (not in the leader's team rows) is never forwarded
        snap.situations = vec![SituationRow { serial: 2, enabled: true, ..SituationRow::default() }];
        snap.situation_selected = Some(0);
        assert_eq!(y_forward(&snap, &team), None);
    }
}
