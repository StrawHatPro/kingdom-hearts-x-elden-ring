//! (ALLY-BUFF) THE HELPFUL STATUSES ON WHOEVER HAS THEM (the user, 2026-11-03: "Donald's helpful spells like Aero shield
//! should appear on whoever has it in the HUD regardless of who is party leader. Positioning should all match Donald.").
//!
//! Before, the icons were part of Donald's own HUD (hud_resonance.rs, `ResonanceView::worn`): they were drawn only
//! while Donald led, so with Sora or Goofy leading a barrier on anyone showed nowhere. Now they are their own piece of
//! the status HUD, fed every frame whoever leads (kh-host `Hud::set_ally_buffs`), one list of icons per member, put by
//! the seat that member sits in now.
//!
//! THE PLACE IS DONALD'S (sheet `party-don-hud` rows `worn.*`, the user, 2026-10-22: "Have them show next to the ally's
//! picture in the empty grey space next to them. For the leader, have it appear inbetween the gap of the MP bar and the
//! start of the health bar. Make them large enough to be easily readable"):
//!
//! - THE LEADER'S SLOT (`sttsPc`, whoever leads): the first icon centred at `pc`, `size_pc` tall: above the MP bar's
//!   right end, left of the HP ring, next to the face. A leader with rows of his own in that space (Goofy: his GUARD bar
//!   over the MP bar, his Resolve pips over that) gets the same column, same x, raised over them
//!   ([`BuffLook::leader_lift`]): the icons never sit on his bar's label or his pips. An icon of the column that would
//!   sit on a leader piece beside it (Sora's focus gauge comes down by his face just above Donald's place: a second
//!   icon) slides left along its row, clear of it ([`BuffLook::leader_place`]).
//! - A FRIEND'S SLOT (`sttsNpc1` / `sttsNpc2`, and the guest slots should a member ever sit there): the first icon
//!   centred at `npc` in that slot's units (the grey plate right of the portrait), `size_npc` tall.
//! - SEVERAL AT ONCE: the same column on every slot: each further icon one `step` x its size ABOVE the one before (the
//!   leader's gap and a friend's plate are both taller than wide), in the order the host gives (the worn spell first).
//!
//! The icons are KH3's IconText icons by name (the host chooses them; Donald's: Aero for Aero Barrier, Thunder for Ball
//! Lightning, Light's for Sanctuary's invincibility). They are drawn in the leader's part (the friends' slots overlap
//! each other, a lower slot's own children are covered) and placed at the seat's part every update, as Donald's were.

use std::sync::Arc;

use crate::geom::{Cx, Rect};
use crate::hud_resonance::{ensure_art, icon_rect, set_art};
use crate::hud_status::{HudStatus, STATUS_INFO_MAX};
use crate::player::{GfxRect, Kind};
use crate::render::IconSet;

/// The leader's part (every icon is drawn under it).
const PC: &str = "sttsPc";
/// Each status slot's part (KH3's `m_statusStateAry`): the leader's, the two friends', the two guests'.
pub const SLOT_PARTS: [&str; STATUS_INFO_MAX] = ["sttsPc", "sttsNpc1", "sttsNpc2", "sttsSnpc1", "sttsSnpc2"];
/// The most icons one slot shows.
pub const MAX_PER_SLOT: usize = 4;

/// One slot's icons this frame: (status slot: 0 the leader, 1 / 2 the friends, 3 / 4 the guests; IconText names in order).
pub type SlotBuffs = (usize, Vec<String>);

/// The place and size (sheet `party-don-hud` rows `worn.*`, Donald's numbers): `sttsPc` units for the leader, the slot's
/// own units for a friend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuffLook {
    pub pc: (f32, f32),
    pub size_pc: f32,
    pub npc: (f32, f32),
    pub size_npc: f32,
    /// A further icon sits this share of its size above the one before.
    pub step: f32,
    /// The gap kept over a leader's piece the column is raised above (see [`BuffLook::leader_lift`]).
    pub clear: f32,
}

impl Default for BuffLook {
    fn default() -> BuffLook {
        BuffLook { pc: (79.0, 116.0), size_pc: 22.0, npc: (86.0, 37.0), size_npc: 24.0, step: 1.1, clear: 2.0 }
    }
}

impl BuffLook {
    /// THE ONE PLACING RULE: the centre and size of icon `k` on status slot `slot`, in the slot's own units (the leader's:
    /// `sttsPc`; a friend's: relative to his part's origin).
    pub fn place(&self, slot: usize, k: usize) -> (f32, f32, f32) {
        let (at, size) = if slot == 0 { (self.pc, self.size_pc) } else { (self.npc, self.size_npc) };
        (at.0, at.1 - k as f32 * self.step * size, size)
    }

    /// THE LEADER'S COLUMN OVER HIS OWN EXTRA ROWS: Donald's place is "between the magic and health, in the empty space
    /// between the top of the MP bar and the health bar right next to the face" (the user, 2026-10-22). A leader with a
    /// bar of his own over the MP bar (Goofy's GUARD bar, the MP bar's copy `bar_dy` above it, negative = up) and a row
    /// over that (his Resolve pips' plate: left, top, right, bottom, `sttsPc` units) has those in that space: his column
    /// moves straight up, same x, to the same place over his top bar, and over the row when the row reaches into the
    /// column, `clear` above it. Returns the shift of the leader's column (0 or negative = up).
    pub fn leader_lift(&self, bar_dy: Option<f32>, row: Option<(f32, f32, f32, f32)>) -> f32 {
        let (x, y, size) = self.place(0, 0);
        let mut lift = bar_dy.unwrap_or(0.0).min(0.0);
        if let Some((_, top, _, _)) = row.filter(|r| r.0 < x + size / 2.0 && r.2 > x - size / 2.0) {
            lift = lift.min(top - self.clear - (y + size / 2.0));
        }
        lift
    }

    /// The leader's icon `k` with his room: Donald's column raised by `room.lift`; an icon that would sit on a leader piece
    /// beside the column (Sora's focus gauge, whose lower end comes down by his face just above Donald's place: a second
    /// icon would sit on its tip) slides left along its row, `clear` from that piece. The first icon at Donald's place
    /// never meets one on today's HUDs (the tests' pictures).
    pub fn leader_place(&self, k: usize, room: &LeaderRoom) -> (f32, f32, f32) {
        let (mut x, y, size) = self.place(0, k);
        let y = y + room.lift;
        let half = size / 2.0;
        for b in &room.beside {
            let meets = b.0 < x + half && b.2 > x - half && b.1 < y + half && b.3 > y - half;
            if meets {
                x = x.min(b.0 - self.clear - half);
            }
        }
        (x, y, size)
    }
}

/// A box (left, top, right, bottom) in `sttsPc` units.
pub type UnitBox = (f32, f32, f32, f32);

/// What the leader's own HUD pieces leave of Donald's place this frame ([`BuffLook::leader_lift`] and the pieces beside
/// the column, [`BuffLook::leader_place`]). Default: all of it (Donald's own HUD).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct LeaderRoom {
    pub lift: f32,
    pub beside: Vec<UnitBox>,
}

/// The box of what the part `path` of the status HUD shows now (its visible shapes, bitmaps and drawings, through its
/// visible children), in the units of its ancestor `within` (`path` starts with it); None when it or a part between is
/// hidden or missing, or it shows nothing.
pub fn shown_box(m: &crate::player::Movie, within: &str, path: &str) -> Option<UnitBox> {
    fn walk(s: &crate::player::Sprite) -> Rect {
        let mut r = Rect::EMPTY;
        for g in &s.gfx {
            r = r.union(&Rect::new(g.x, g.y, g.x + g.w, g.y + g.h));
        }
        for n in s.children.iter().chain(&s.extra).filter(|n| n.is_visible()) {
            let b = match &n.kind {
                Kind::Shape(shape) => shape.bounds.rect(),
                Kind::Morph(shape, _) => shape.bounds.rect(),
                Kind::Image(image) => Rect::new(0.0, 0.0, image.size.0, image.size.1),
                Kind::Sprite(child) => walk(child),
                _ => Rect::EMPTY,
            };
            if !b.is_empty() {
                r = r.union(&b.transform(&n.matrix));
            }
        }
        r
    }
    let n = m.node(path).filter(|n| n.is_visible())?;
    let Kind::Sprite(s) = &n.kind else { return None };
    let mut r = walk(s).transform(&n.matrix);
    // up through the parts between: each one's own place
    let mut at = path;
    while let Some((parent, _)) = at.rsplit_once('.').filter(|(p, _)| p.len() > within.len()) {
        r = r.transform(&m.node(parent).filter(|n| n.is_visible())?.matrix);
        at = parent;
    }
    (!r.is_empty()).then_some((r.x0, r.y0, r.x1, r.y1))
}

fn node(slot: usize, k: usize) -> String {
    format!("allyBuff{slot}_{k}")
}

/// The icons' driver (one per HUD).
#[derive(Clone, Debug, Default)]
pub struct BuffIcons {
    shown: bool,
    /// What was last drawn (tests / logs): (slot, index in the slot, icon, centre x, centre y, size), `sttsPc` units.
    pub last: Vec<(usize, usize, String, f32, f32, f32)>,
    /// The leader's room last applied.
    pub last_room: LeaderRoom,
}

impl BuffIcons {
    fn hide_all(m: &mut crate::player::Movie) {
        for slot in 0..STATUS_INFO_MAX {
            for k in 0..MAX_PER_SLOT {
                m.set_visible(&format!("{PC}.{}", node(slot, k)), false);
            }
        }
    }

    /// One update of the status HUD, after the stage frames and the leader's own pieces. `buffs`: each wearer's slot and
    /// icons (empty = nothing); `room`: what the leader's own pieces leave of Donald's place (default: all of it).
    pub fn update(&mut self, hud: &mut HudStatus, icons: Option<&Arc<IconSet>>, buffs: &[SlotBuffs], look: &BuffLook, room: &LeaderRoom) {
        let m = &mut hud.movie;
        self.last.clear();
        self.last_room.clone_from(room);
        let any = buffs.iter().any(|b| !b.1.is_empty());
        let (true, Some(set), true) = (any, icons, m.exists(PC)) else {
            if self.shown {
                Self::hide_all(m);
            }
            self.shown = false;
            return;
        };
        self.shown = true;
        let pc_at = m.node(PC).map(|n| (n.matrix.tx, n.matrix.ty)).unwrap_or_default();
        for (slot, part) in SLOT_PARTS.iter().enumerate() {
            let list = buffs.iter().find(|b| b.0 == slot).map(|b| b.1.as_slice()).unwrap_or_default();
            // the wearer's seat (a hidden or missing slot: nothing drawn for it)
            let seat = m.node(part).filter(|n| n.is_visible()).map(|n| (n.matrix.tx, n.matrix.ty));
            for k in 0..MAX_PER_SLOT {
                let name = node(slot, k);
                let path = format!("{PC}.{name}");
                let (Some(icon), Some((sx, sy))) = (list.get(k), seat) else {
                    m.set_visible(&path, false);
                    continue;
                };
                if !ensure_art(m, PC, &name) {
                    continue;
                }
                // the leader's: Donald's place in what his own pieces leave of it; a friend's: in his slot's units, moved to
                // his part's origin (drawn in the leader's part)
                let (x, y, size) = if slot == 0 {
                    look.leader_place(k, room)
                } else {
                    let (x, y, size) = look.place(slot, k);
                    (sx - pc_at.0 + x, sy - pc_at.1 + y, size)
                };
                let gfx: Vec<GfxRect> = icon_rect(set, icon, x, y, size).into_iter().collect();
                set_art(m, &path, gfx, Cx::IDENTITY, true);
                self.last.push((slot, k, icon.clone(), x, y, size));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_slot_stacks_upward_from_donalds_place() {
        let look = BuffLook::default();
        assert_eq!(look.place(0, 0), (79.0, 116.0, 22.0), "the leader's first icon: Donald's place");
        assert_eq!(look.place(1, 0), (86.0, 37.0, 24.0), "a friend's first icon: Donald's place");
        assert_eq!(look.place(2, 0), look.place(1, 0), "both friend slots alike");
        let (x, y, s) = look.place(0, 1);
        assert!((x, s) == (79.0, 22.0) && (y - (116.0 - 1.1 * 22.0)).abs() < 1e-4, "the second straight above");
        let (x, y, _) = look.place(2, 1);
        assert!(x == 86.0 && (y - (37.0 - 1.1 * 24.0)).abs() < 1e-4);
    }

    #[test]
    fn the_leaders_column_rises_over_his_own_rows_only() {
        let look = BuffLook::default();
        assert_eq!(look.leader_lift(None, None), 0.0, "Donald / Sora: Donald's place");
        assert_eq!(look.leader_lift(Some(-20.0), None), -20.0, "over a bar the MP bar's copy 20 above");
        // a row reaching into the column (its top at 70): the icon's bottom `clear` over it
        let lift = look.leader_lift(Some(-20.0), Some((0.0, 70.0, 75.0, 88.0)));
        assert!((116.0 + lift + 11.0 - (70.0 - 2.0)).abs() < 1e-4, "{lift}");
        // a row left of the column: only the bar counts
        assert_eq!(look.leader_lift(Some(-20.0), Some((0.0, 70.0, 60.0, 88.0))), -20.0);
    }

    #[test]
    fn a_leader_icon_on_a_piece_beside_the_column_slides_left_of_it() {
        let look = BuffLook::default();
        assert_eq!(look.leader_place(0, &LeaderRoom::default()), look.place(0, 0), "nothing in the way: Donald's place");
        // a piece over the second icon's place, from x 85 rightwards, down to y 100
        let room = LeaderRoom { lift: 0.0, beside: vec![(85.0, 0.0, 200.0, 100.0)] };
        assert_eq!(look.leader_place(0, &room), look.place(0, 0), "the first is clear of it");
        let (x, y, size) = look.leader_place(1, &room);
        assert_eq!(y, look.place(0, 1).1, "same row");
        assert!((x + size / 2.0 - (85.0 - look.clear)).abs() < 1e-4, "slid left, clear of it: {x}");
    }
}
