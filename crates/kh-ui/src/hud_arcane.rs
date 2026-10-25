//! (DON-UI) DONALD'S ARCANE FLOW STARS, ZETTAFLARE TRACKER AND TARGET STATUS ICONS (documentation/DONALD_OVERHAUL.md
//! 6.3, 6.5, 7, 9). KH3 has none of these: they are THE PORT'S, drawn with KH3's own art (the IconText atlas of KH3's
//! menus, `textures/IconText/IconText.png`), placed in KH3's HUD movies:
//!
//! - THE STARS (6.3, 7: "the Arcane Flow charges (4 small stars) next to the Y command"): KH3's gold star icon
//!   (`s_ssrare`), one per charge of the meter, lit for each charge he has, dark for the others; the oldest charge's star
//!   blinks in its last seconds (it is about to run out, R11). In the command HUD (`HudCommand`, `cmdMenu`), to the right
//!   of the situation command row where Y's command stands.
//! - THE ZETTAFLARE TRACKER (6.5): once he knows Zettaflare, a small element icon per learned element after the stars,
//!   lit when that element reached Resonance 3 in this fight.
//! - THE TARGET'S STATUSES (7): over the lock-on cursor (`HudTarget`), one icon per status: Wet (the Water drop), Chill
//!   (the Blizzard snowflake, its stacks as small marks), Frozen (the ice crystal), Burn (the flame), Reveal (the gold
//!   sparkles), Exposed (the red "down" arrow: it takes more damage), Blind (the Dark icon). With Elemental Insight (9),
//!   the target's weak elements (the icon with KH3's cyan "up" arrow) and resisted ones (the red "down" arrow) under them.

use crate::geom::Cx;
use crate::hud_command::HudCommand;
use crate::hud_resonance::{ensure_art_of, icon_rect, set_art, DonArt};
use crate::hud_target::HudTarget;
use crate::player::GfxRect;

/// The command HUD's view.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ArcaneView {
    pub stars: u8,
    pub stars_max: u8,
    /// Seconds left of the oldest charge.
    pub star_left: Option<f32>,
    /// The Zettaflare tracker: (element icon, lit), once Zettaflare is learned.
    pub zetta: Vec<(String, bool)>,
}

/// The target's view: icons (IconText name, a count shown as small marks) and Elemental Insight's (icon, weak).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TargetIconsView {
    pub icons: Vec<(String, u8)>,
    pub insight: Vec<(String, bool)>,
}

/// The look (sheet `party-don-hud` rows `star.*`, `zetta.*`, `status.*`).
#[derive(Clone, Debug, PartialEq)]
pub struct ArcaneLayout {
    /// The first star's centre in `cmdMenu` units, the size, the step.
    pub star_at: (f32, f32),
    pub star_size: f32,
    pub star_step: f32,
    /// An empty star's colour scale.
    pub star_off: f32,
    /// The oldest star blinks when its charge has less than this left (s), at this rate.
    pub star_warn: f32,
    pub star_blink_hz: f32,
    /// The tracker after the stars: gap, icon size, step; an unlit icon's colour scale.
    pub zetta_gap: f32,
    pub zetta_size: f32,
    pub zetta_step: f32,
    pub zetta_off: f32,
    /// The statuses: the row's centre relative to the lock cursor (pixels), icon size, step; Insight's row below.
    pub status_at: (f32, f32),
    pub status_size: f32,
    pub status_step: f32,
    pub insight_dy: f32,
    pub insight_size: f32,
    /// The Y rows' star cost (the user, 2026-10-22: "the Y magic commands need to indicate how many stars they cost"):
    /// the Arcane Flow row's gold stars, right-aligned in the plate where KH3's magic hat sits (the hat hidden on his Y
    /// rows): each star's height and step, the row's offset from the hat's place, a greyed row's star brightness.
    pub y_star_size: f32,
    pub y_star_step: f32,
    pub y_star_at: (f32, f32),
    pub y_star_off: f32,
}

impl Default for ArcaneLayout {
    fn default() -> ArcaneLayout {
        ArcaneLayout {
            star_at: (340.0, 892.0),
            star_size: 24.0,
            star_step: 25.0,
            star_off: 0.42,
            star_warn: 3.0,
            star_blink_hz: 4.0,
            zetta_gap: 10.0,
            zetta_size: 18.0,
            zetta_step: 19.0,
            zetta_off: 0.3,
            status_at: (0.0, -82.0),
            status_size: 24.0,
            status_step: 26.0,
            insight_dy: 22.0,
            insight_size: 18.0,
            y_star_size: 21.0,
            y_star_step: 18.0,
            y_star_at: (75.0, 17.0),
            y_star_off: 0.5,
        }
    }
}

const CMD: &str = "cmdMenu";
const CMD_CLASS: &str = "CmdName";
const TARGET_CLASS: &str = "CoPushBtnText";
/// The target art's parent: the lock-on plane (its cursors' parent).
pub const TARGET_PLANE: &str = "planeParent.lockonMarkPlane";

/// (DON-UI) THE Y ROWS' STAR COST: per situation button on show (AS slot, the stars it costs, enabled), the gold stars
/// in the button's plate where the magic hat is, the hat hidden; a greyed row's stars dim.
#[derive(Clone, Debug, Default)]
pub struct YCostStars {
    hid: [bool; crate::hud_command::ACTION_COMMAND_MAX],
    /// What was last drawn (tests): (slot, stars, lit), and the stars' boxes (left, top, right, bottom) in each
    /// button's `cmdAct1` units with that plate's name field's right edge.
    pub last: Vec<(usize, u8, bool)>,
}

impl YCostStars {
    pub fn update(&mut self, hud: &mut HudCommand, art: &DonArt, rows: &[(i32, u8, bool)], look: &ArcaneLayout) {
        let m = &mut hud.movie;
        self.last.clear();
        for slot in 0..crate::hud_command::ACTION_COMMAND_MAX {
            let base = format!("{}.cmdAct.cmdAct1", crate::hud_command::action_button_path(slot));
            let ico = format!("{base}.cmdActIco");
            let stars = format!("{base}.donYCost");
            let row = rows.iter().find(|r| r.0 == slot as i32 && r.1 > 0);
            let (Some(&(_, cost, enabled)), Some(set)) = (row, art.icons.as_ref()) else {
                m.set_visible(&stars, false);
                if std::mem::take(&mut self.hid[slot]) {
                    if let Some(n) = m.node_mut(&ico) {
                        n.script_visible = None;
                    }
                }
                continue;
            };
            if !m.exists(&base) || !ensure_art_of(m, CMD_CLASS, &base, "donYCost") {
                continue;
            }
            // the hat's place: the icon clip's origin in the button
            let (hx, hy) = m.node(&ico).map(|n| (n.matrix.tx, n.matrix.ty)).unwrap_or_default();
            m.set_visible(&ico, false);
            self.hid[slot] = true;
            let n = cost.min(8) as usize;
            let right = hx + look.y_star_at.0;
            let cy = hy + look.y_star_at.1;
            let x0 = right - look.y_star_size / 2.0 - (n as f32 - 1.0) * look.y_star_step;
            let gfx: Vec<GfxRect> = (0..n).filter_map(|i| icon_rect(set, "s_ssrare", x0 + i as f32 * look.y_star_step, cy, look.y_star_size)).collect();
            let k = if enabled { 1.0 } else { look.y_star_off };
            set_art(m, &stars, gfx, Cx { mul: [k, k, k, 1.0], add: [0.0; 4] }, true);
            self.last.push((slot, n as u8, enabled));
        }
    }
}

/// The stars' and the tracker's driver.
#[derive(Clone, Debug, Default)]
pub struct ArcaneStars {
    installed: bool,
    pub shown: bool,
    blink: f32,
    /// What was last applied (tests): stars lit, stars blinking, tracker lit / shown.
    pub last: (u8, bool, u8, u8),
}

fn star_path(i: usize) -> String {
    format!("{CMD}.donStar{i}")
}

impl ArcaneStars {
    /// One update of the command HUD (after its stage frames). `v` None = nothing.
    pub fn update(&mut self, hud: &mut HudCommand, art: &DonArt, v: Option<&ArcaneView>, look: &ArcaneLayout, dt: f32) {
        let m = &mut hud.movie;
        let Some(v) = v else {
            if self.installed && self.shown {
                for i in 0..8 {
                    m.set_visible(&star_path(i), false);
                }
                m.set_visible(&format!("{CMD}.donZetta"), false);
                m.set_visible(&format!("{CMD}.donZettaLit"), false);
                m.set_visible(&format!("{CMD}.donStarPlate"), false);
            }
            self.shown = false;
            return;
        };
        if !self.installed {
            if !m.exists(CMD) {
                return;
            }
            // the plate first: behind the stars
            if !ensure_art_of(m, CMD_CLASS, CMD, "donStarPlate") {
                return;
            }
            for i in 0..8 {
                if !ensure_art_of(m, CMD_CLASS, CMD, &format!("donStar{i}")) {
                    return;
                }
            }
            if !ensure_art_of(m, CMD_CLASS, CMD, "donZetta") {
                return;
            }
            self.installed = true;
        }
        let Some(set) = art.icons.as_ref() else { return };
        self.shown = true;
        self.blink = (self.blink + dt * look.star_blink_hz.max(0.0)).fract();
        let visible = m.node(CMD).is_some_and(|n| n.is_visible());
        let max = v.stars_max.clamp(1, 8) as usize;
        let warn = v.stars > 0 && v.star_left.is_some_and(|l| l < look.star_warn);
        for i in 0..8 {
            let path = star_path(i);
            if i >= max {
                m.set_visible(&path, false);
                continue;
            }
            let x = look.star_at.0 + i as f32 * look.star_step;
            let gfx: Vec<GfxRect> = icon_rect(set, "s_ssrare", x, look.star_at.1, look.star_size).into_iter().collect();
            let lit = i < v.stars as usize;
            // the oldest charge is the first star: it blinks before it runs out
            let off = Cx { mul: [look.star_off, look.star_off, look.star_off, 0.9], add: [0.0; 4] };
            let cx = if lit && !(i == 0 && warn && self.blink >= 0.5) { Cx::IDENTITY } else { off };
            set_art(m, &path, gfx, cx, visible);
        }
        // the tracker after the stars, one small element icon each
        let x0 = look.star_at.0 + (max as f32 - 0.5) * look.star_step + look.zetta_gap + look.zetta_size / 2.0;
        let mut gfx = Vec::new();
        let mut dim_all = true;
        for (k, (icon, lit)) in v.zetta.iter().enumerate() {
            // every icon dark in one node; the lit ones are drawn again on top in full colour (below)
            gfx.extend(icon_rect(set, icon, x0 + k as f32 * look.zetta_step, look.star_at.1, look.zetta_size));
            dim_all &= !lit;
        }
        let zetta_cx = Cx { mul: [look.zetta_off, look.zetta_off, look.zetta_off, 0.85], add: [0.0; 4] };
        set_art(m, &format!("{CMD}.donZetta"), gfx, zetta_cx, visible && !v.zetta.is_empty());
        // the lit icons on top, full colour
        if !ensure_art_of(m, CMD_CLASS, CMD, "donZettaLit") {
            return;
        }
        // the plate: KH3's MP base as a dark pill behind the row (it starts at the command menu's title bar)
        if let Some(t) = &art.base {
            let h = look.star_size + 8.0;
            let x0p = look.star_at.0 - look.star_size / 2.0 - 8.0;
            let end = if v.zetta.is_empty() { look.star_at.0 + (max as f32 - 1.0) * look.star_step + look.star_size / 2.0 } else { x0 + (v.zetta.len() as f32 - 1.0) * look.zetta_step + look.zetta_size / 2.0 };
            set_art(m, &format!("{CMD}.donStarPlate"), crate::hud_resonance::pill(t, x0p, look.star_at.1 - h / 2.0, end + 10.0 - x0p, h), crate::hud_resonance::multiply([0.2, 0.2, 0.23, 0.88]), visible);
        }
        let lit: Vec<GfxRect> = v.zetta.iter().enumerate().filter(|(_, (_, l))| *l).filter_map(|(k, (icon, _))| icon_rect(set, icon, x0 + k as f32 * look.zetta_step, look.star_at.1, look.zetta_size)).collect();
        let n_lit = lit.len() as u8;
        set_art(m, &format!("{CMD}.donZettaLit"), lit, Cx::IDENTITY, visible && !v.zetta.is_empty() && !dim_all);
        self.last = (v.stars.min(max as u8), warn, n_lit, v.zetta.len() as u8);
    }
}

/// The target statuses' driver.
#[derive(Clone, Debug, Default)]
pub struct TargetIcons {
    installed: bool,
    pub shown: bool,
    /// What was last applied (tests): icons, insight icons, the position.
    pub last: (usize, usize, (f32, f32)),
}

impl TargetIcons {
    /// One update of the target HUD (after its projection pass): `cursor` the lock-on cursor's clip path (its position
    /// is the target's on screen), `v` None = nothing.
    pub fn update(&mut self, hud: &mut HudTarget, art: &DonArt, cursor: Option<&str>, v: Option<&TargetIconsView>, look: &ArcaneLayout) {
        let m = &mut hud.movie;
        let path = format!("{TARGET_PLANE}.donStatus");
        let ins = format!("{TARGET_PLANE}.donInsight");
        let pos = cursor.and_then(|c| m.node(c).filter(|n| n.is_visible()).map(|n| (n.matrix.tx, n.matrix.ty)));
        let (Some(v), Some((x, y)), Some(set)) = (v, pos, art.icons.as_ref()) else {
            if self.installed && self.shown {
                m.set_visible(&path, false);
                m.set_visible(&ins, false);
            }
            self.shown = false;
            return;
        };
        if !self.installed {
            if !ensure_art_of(m, TARGET_CLASS, TARGET_PLANE, "donStatus") || !ensure_art_of(m, TARGET_CLASS, TARGET_PLANE, "donInsight") {
                return;
            }
            self.installed = true;
        }
        self.shown = true;
        let row = |items: &[(String, f32)], cy: f32, size: f32, step: f32| -> Vec<GfxRect> {
            let n = items.len() as f32;
            let x0 = x + look.status_at.0 - (n - 1.0) * step / 2.0;
            items.iter().enumerate().filter_map(|(k, (icon, scale))| icon_rect(set, icon, x0 + k as f32 * step, cy, size * scale)).collect()
        };
        // the statuses: a Chill's stacks as small snowflakes under its icon
        let mut gfx = row(&v.icons.iter().map(|(i, _)| (i.clone(), 1.0)).collect::<Vec<_>>(), y + look.status_at.1, look.status_size, look.status_step);
        let n = v.icons.len() as f32;
        let x0 = x + look.status_at.0 - (n - 1.0) * look.status_step / 2.0;
        for (k, (icon, count)) in v.icons.iter().enumerate() {
            for c in 0..*count {
                let s = look.status_size * 0.32;
                let cx0 = x0 + k as f32 * look.status_step - (*count as f32 - 1.0) * s / 2.0 + c as f32 * s;
                gfx.extend(icon_rect(set, icon, cx0, y + look.status_at.1 + look.status_size * 0.62, s));
            }
        }
        set_art(&mut hud.movie, &path, gfx, Cx::IDENTITY, !v.icons.is_empty());
        // Elemental Insight: each weak / resisted element with KH3's up / down arrow
        let mut items: Vec<GfxRect> = Vec::new();
        let n = v.insight.len() as f32;
        let step = look.insight_size * 1.9;
        let iy = y + look.status_at.1 + if v.icons.is_empty() { 0.0 } else { look.insight_dy + look.status_size * 0.3 };
        let ix0 = x + look.status_at.0 - (n - 1.0) * step / 2.0;
        for (k, (icon, weak)) in v.insight.iter().enumerate() {
            let cx0 = ix0 + k as f32 * step;
            items.extend(icon_rect(set, icon, cx0 - look.insight_size * 0.35, iy, look.insight_size));
            items.extend(icon_rect(set, if *weak { "StatusUp" } else { "StatusDown" }, cx0 + look.insight_size * 0.5, iy + look.insight_size * 0.1, look.insight_size * 0.7));
        }
        set_art(&mut hud.movie, &ins, items, Cx::IDENTITY, !v.insight.is_empty());
        self.last = (v.icons.len(), v.insight.len(), (x, y));
    }
}
