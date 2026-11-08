//! (GOOFY) GOOFY'S PARRY TECHNIQUES PAGE (THE USER 2026-11-02: "Can we have a way to visually show in the camp menu what parry
//! techniques Goofy currently has, when to use them and how to perform them? (preferably using symbols from Kingdom Hearts'
//! controller icons)").
//!
//! WHERE IT LIVES: KH3's own Abilities screen has "Show Next List" (Y, `kh3_camp.ability.button_0012` / `_0015` / `_0025`): it
//! pages from the abilities a character equips to the lists of moves he simply HAS (the Keyblade's Form Actions, the
//! Shotlocks). Goofy's eight techniques are exactly that kind of list: learned by level, never equipped. So on Goofy's Abilities
//! screen (Menu > Abilities > Goofy, leader or AI member alike), Y shows this page and Y again (or B) goes back to his list.
//! A character without a page (Sora, Donald) has no "Show Next List" in the port, as before.
//!
//! WHAT IS KH3'S: the camp menu's frame, title strip, help band and button guide (the guide is KH3's own string), the list
//! cursor of the menus (`ItemListLibrary` `tres.rsl.ListItemBgI2`, `select`), the ability icons of `UI/IconText`
//! (`SetAbility` for a learned technique, `Ability` for one still locked), the pad icons of the button inputs and the left
//! stick (`UI/IconText`, the Xbox set), the fonts, the menu sounds (static SE 0 cursor, 2 the list change, 3 back).
//! COMPOSED here [L]: the table's layout and shapes, the rank pips, the Stagger bars, the column headings and every word of the
//! rows and the footer (the rows' words and the footer come from the host: kh-party `goofy::guide`).
//!
//! (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique ... The parries are there to make the enemy's
//! stagger go up fast while taking 0 damage yourself. Redesign the menu for this as well") NO "THEN (A)" COLUMN: a technique
//! opens no counter any more. The last column is its Stagger at his rank, a number and one to three bars, and a footer line
//! under the table says how the full gauge's break leads to Goofy Breaker (the doc's 6.3), with KH3's own Y icon.
//!
//! THE FEED IS THE HOST'S ([`ParryPage`]): his live level and ranks, built again whenever the camp menu supplies the
//! Abilities screen, so the page is true at any point of a playthrough.

use std::sync::Arc;

use crate::camp::{move_wrapping, CampKey};
use crate::def::{Align, Fill, MovieDef, Pack, Paint, Rect0, Shape, Stroke, TextDef};
use crate::geom::{Cx, Matrix};
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_OPEN};
use crate::party_formation::{alpha, gradient, node, polyline, rect_edges, shape, tint};
use crate::player::{Kind, Movie, Node, TextNode};
use crate::render::Fonts;
use crate::UiSound;

/// The names of this page's script children of the movie's root all start so.
pub const PREFIX: &str = "gp_";

// ------------------------------------------------------------------------------------------------ COMPOSED text [L]

/// The column headings.
pub const HEAD_TECHNIQUE: &str = "Technique";
pub const HEAD_RANK: &str = "Rank";
pub const HEAD_WHEN: &str = "The enemy does";
pub const HEAD_HOW: &str = "How to perform it";
pub const HEAD_STAGGER: &str = "Stagger";
/// A learned technique's rank ("Rank 2"), a locked one's level ("Learned at Lv. 15"), a rank a boss saved ("Rank 2 ready",
/// the boss popup's own wording: kh-party growth.rs `GrowthGrant::label`).
pub fn rank_line(rank: u8) -> String {
    format!("Rank {rank}")
}
pub fn learn_line(level: u32) -> String {
    format!("Learned at Lv. {level}")
}
pub fn ready_line(rank: u8) -> String {
    format!("Rank {rank} ready")
}
/// The help line's last sentence.
pub fn state_sentence(row: &ParryRow) -> String {
    match (row.rank, row.saved) {
        (0, 0) => format!("Learned at Lv. {}.", row.learn_level),
        (0, s) => format!("Learned at Lv. {}; a boss has already saved Rank {s}, which applies when he learns it.", row.learn_level),
        (r, _) => format!("Rank {r} of 3: always on, nothing to equip."),
    }
}

/// The help line under the page when the host gives none of its own.
pub const PAGE_HELP: &str = "What Goofy's parry techniques answer and how to perform them.";

// ------------------------------------------------------------------------------------------------ the feed

/// One technique as the host gives it. `when`, `how`, `detail` may hold KH3's text markup (`<icon(Name)>`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ParryRow {
    pub name: String,
    /// The level he learns it at (the sheet's).
    pub learn_level: u32,
    /// 0 = not learned yet, else 1 ..= 3.
    pub rank: u8,
    /// Not learned yet and a boss has saved a rank: the rank it will be learned at; else 0.
    pub saved: u8,
    /// The danger "!" warns of the attack it answers.
    pub perilous: bool,
    pub when: String,
    pub how: String,
    /// The Stagger it adds at his rank ("+25", "+28 to +46").
    pub stagger: String,
    /// Its Stagger as bars, 1 ..= 3.
    pub stagger_bars: u8,
    pub detail: String,
}

impl ParryRow {
    pub fn learned(&self) -> bool {
        self.rank > 0
    }

    /// The help line for the focused row: what it does and how, then its state.
    pub fn help(&self) -> String {
        format!("{}\n{}", self.detail, state_sentence(self))
    }
}

/// The page: its title, whose it is, his level, the rows in the order he learns them, the line under the table (KH3's text
/// markup: `<icon(Name)>`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ParryPage {
    pub title: String,
    pub name: String,
    pub level: u32,
    pub rows: Vec<ParryRow>,
    pub footer: String,
}

// ------------------------------------------------------------------------------------------------ the logic

/// The page's state inside the camp menu (`CampMenu::parry`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ParryScreen {
    /// The shown member's page (None: he has none, e.g. Sora or Donald).
    pub page: Option<ParryPage>,
    /// The page is up instead of the abilities list.
    pub on: bool,
    pub cursor: usize,
}

impl ParryScreen {
    /// The shown member has a page ("Show Next List" is offered).
    pub fn available(&self) -> bool {
        self.page.as_ref().is_some_and(|p| !p.rows.is_empty())
    }

    /// The host's page for the member on screen, again after every change (the cursor stays). None takes the page down.
    pub fn set(&mut self, page: Option<ParryPage>) {
        self.page = page;
        if !self.available() {
            self.on = false;
            self.cursor = 0;
        }
        let n = self.page.as_ref().map_or(0, |p| p.rows.len());
        self.cursor = self.cursor.min(n.saturating_sub(1));
    }

    pub fn focused(&self) -> Option<&ParryRow> {
        self.page.as_ref().and_then(|p| p.rows.get(self.cursor))
    }

    /// Y on the abilities list: the page comes up (KH3's "Show Next List"; static SE 2 as a list change [L]). False: none.
    pub fn open(&mut self, sounds: &mut Vec<UiSound>) -> bool {
        if !self.available() {
            return false;
        }
        self.on = true;
        self.cursor = 0;
        sounds.push(UiSound::Play(SE_MENU_OPEN));
        true
    }

    /// One event while the page is up: up / down walk the rows (a fresh press wraps, a held one stops at the end), Y shows
    /// the next list (back to the abilities), B goes back one level (the abilities list: THE USER 2026-10-22, "when you press
    /// B it should bring you back to the previous menu"). A does nothing: the page only shows.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        let n = self.page.as_ref().map_or(0, |p| p.rows.len());
        match key {
            CampKey::Up | CampKey::Down => {
                if let Some(to) = move_wrapping(self.cursor, n, key == CampKey::Down, repeat) {
                    self.cursor = to;
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                }
            }
            CampKey::Triangle => {
                self.on = false;
                sounds.push(UiSound::Play(SE_MENU_OPEN));
            }
            CampKey::Cancel => {
                self.on = false;
                sounds.push(UiSound::Play(SE_MENU_CLOSE));
            }
            _ => {}
        }
    }
}

// ------------------------------------------------------------------------------------------------ layout [L]
// Stage pixels (1920 x 1080): the content between the title strip (x 155) and the right rule (x 1810), above the help band
// (y 870), under the button guide (y 100).

/// The table's left and right edges.
pub const X0: f32 = 200.0;
pub const X1: f32 = 1790.0;
/// The title bar, the column headings, the first row, the row height.
const TITLE_Y: f32 = 132.0;
const HEAD_Y: f32 = 186.0;
const HEAD_H: f32 = 32.0;
pub const ROW_Y: f32 = 226.0;
pub const ROW_H: f32 = 74.0;
/// The footer line's gap under the eight rows (it ends above the help band at y 870).
pub const FOOTER_GAP: f32 = 8.0;
/// The columns' left edges and widths: icon + name, rank, when, how, Stagger.
const COL_ICON: f32 = X0 + 14.0;
const COL_NAME: (f32, f32) = (X0 + 54.0, 290.0);
const COL_RANK: (f32, f32) = (X0 + 350.0, 210.0);
const COL_WHEN: (f32, f32) = (X0 + 572.0, 300.0);
const COL_HOW: (f32, f32) = (X0 + 884.0, 536.0);
const COL_STAGGER: (f32, f32) = (X0 + 1430.0, X1 - (X0 + 1430.0) - 8.0);
/// The Stagger bars: width, height, step.
const BAR: (f32, f32, f32) = (24.0, 8.0, 30.0);
/// The room between two lines of the input column (its 30-pixel icons).
const HOW_LEADING: f32 = 6.0;
/// The perilous mark's room before the "when" words.
const MARK_W: f32 = 22.0;
/// Text sizes.
const SIZE_NAME: f32 = 26.0;
const SIZE_BODY: f32 = 21.0;
const SIZE_HEAD: f32 = 20.0;
const SIZE_TITLE: f32 = 24.0;
const SIZE_FOOTER: f32 = 18.0;
/// The danger mark's size and its narrowing (the menu font's "!" is wide: hud_goofy.rs draws it narrowed too).
const SIZE_MARK: f32 = 32.0;
const MARK_NARROW: f32 = 0.6;
/// The fonts: KH3's menu font (the title strip's capitals) for the page's title and the danger mark, the lists' font (the
/// abilities list's rows and headings) for the rest.
const FONT_MENU: &str = "$KH2Menu";
const FONT_BODY: &str = "$sougei";
/// The page fades in over this long.
const FADE_IN_S: f32 = 0.18;
/// Stage seconds of one frame.
const FRAME: f32 = 1.0 / 60.0;

const WHITE: [f32; 4] = [255.0, 255.0, 255.0, 255.0];
const SOFT: [f32; 4] = [205.0, 214.0, 236.0, 255.0];
const GOLD: [f32; 4] = [255.0, 224.0, 96.0, 255.0];
const RED: [f32; 4] = [235.0, 46.0, 40.0, 255.0];
/// A locked row's words.
const LOCKED_TINT: f32 = 0.55;

/// The characters' ids of this page's shapes and texts (the layer's change test hashes the id, the matrix, the colour).
const CH: u32 = 0x7F60_0000;

/// The pictures of the page, read from the pack once (`CampMenuView::load`).
#[derive(Default)]
pub struct ParryArt {
    /// `ItemListLibrary` (the list cursor of the menus' rows).
    list_library: Option<Arc<MovieDef>>,
}

impl ParryArt {
    pub fn load(pack: &mut Pack) -> ParryArt {
        ParryArt { list_library: pack.movie("ItemListLibrary").ok() }
    }
}

struct Shapes {
    title: Arc<Shape>,
    head: Arc<Shape>,
    row: [Arc<Shape>; 2],
    rule: Arc<Shape>,
    focus: Arc<Shape>,
    pip_on: Arc<Shape>,
    pip_off: Arc<Shape>,
    bar_on: Arc<Shape>,
    bar_off: Arc<Shape>,
}

impl Shapes {
    fn new() -> Shapes {
        let w = X1 - X0;
        let title = shape(
            vec![Fill { paint: gradient(0.0, 0.0, w, 34.0, false, &[(0.0, [150.0, 160.0, 185.0, 150.0]), (0.6, [120.0, 130.0, 160.0, 70.0]), (1.0, [120.0, 130.0, 160.0, 0.0])]), edges: rect_edges(0.0, 0.0, w, 34.0) }],
            vec![Stroke { width: 1.5, color: [210.0, 220.0, 240.0, 150.0], edges: polyline(&[(0.0, 34.0), (w * 0.8, 34.0)], false) }],
        );
        // the list title's grey band (`abilityListTitle`'s look)
        let head = shape(vec![Fill { paint: gradient(0.0, 0.0, w, HEAD_H, true, &[(0.0, [120.0, 124.0, 134.0, 230.0]), (1.0, [70.0, 72.0, 80.0, 230.0])]), edges: rect_edges(0.0, 0.0, w, HEAD_H) }], Vec::new());
        let row = [0, 1].map(|k| {
            let a = if k == 0 { 150.0 } else { 120.0 };
            shape(vec![Fill { paint: gradient(0.0, 0.0, w, ROW_H, false, &[(0.0, [14.0, 26.0, 58.0, a]), (1.0, [8.0, 14.0, 34.0, a * 0.8])]), edges: rect_edges(0.0, 0.0, w, ROW_H - 2.0) }], Vec::new())
        });
        let rule = shape(Vec::new(), vec![Stroke { width: 1.5, color: [150.0, 185.0, 235.0, 120.0], edges: polyline(&[(0.0, 0.0), (w, 0.0)], false) }]);
        // a stand-in for the KH3 cursor when the pack has no `ItemListLibrary`
        let focus = shape(vec![Fill { paint: gradient(0.0, 0.0, w, ROW_H, false, &[(0.0, [40.0, 120.0, 240.0, 220.0]), (1.0, [20.0, 60.0, 160.0, 120.0])]), edges: rect_edges(0.0, 0.0, w, ROW_H - 2.0) }], Vec::new());
        let diamond = |r: f32| polyline(&[(0.0, -r), (r, 0.0), (0.0, r), (-r, 0.0)], true);
        let pip_on = shape(vec![Fill { paint: Paint::Solid(GOLD), edges: diamond(9.0) }], vec![Stroke { width: 1.5, color: [255.0, 250.0, 220.0, 255.0], edges: diamond(9.0) }]);
        let pip_off = shape(vec![Fill { paint: Paint::Solid([12.0, 18.0, 36.0, 220.0]), edges: diamond(9.0) }], vec![Stroke { width: 1.5, color: [150.0, 160.0, 185.0, 200.0], edges: diamond(9.0) }]);
        let (bw, bh, _) = BAR;
        let bar_on = shape(vec![Fill { paint: Paint::Solid(GOLD), edges: rect_edges(0.0, 0.0, bw, bh) }], vec![Stroke { width: 1.5, color: [255.0, 250.0, 220.0, 255.0], edges: rect_edges(0.0, 0.0, bw, bh) }]);
        let bar_off = shape(vec![Fill { paint: Paint::Solid([12.0, 18.0, 36.0, 220.0]), edges: rect_edges(0.0, 0.0, bw, bh) }], vec![Stroke { width: 1.5, color: [150.0, 160.0, 185.0, 200.0], edges: rect_edges(0.0, 0.0, bw, bh) }]);
        Shapes { title, head, row, rule, focus, pip_on, pip_off, bar_on, bar_off }
    }
}

fn text_def(class: &str, size: f32, color: [f32; 4], align: Align, w: f32, h: f32) -> Arc<TextDef> {
    Arc::new(TextDef { id: 0, bounds: Rect0 { x0: 0.0, y0: 0.0, x1: w, y1: h }, font_class: Some(class.to_string()), size, color, align, initial: String::new(), word_wrap: false, leading: 0.0 })
}

/// THE PAGE'S PICTURE while it is up: made when it comes up, its nodes rebuilt each frame into the movie root's script
/// children named [`PREFIX`]...
pub struct ParryScene {
    fonts: Fonts,
    shapes: Shapes,
    cursor_node: Option<Node>,
    age: f32,
    /// What it shows (the camp menu's copy, [`ParryScene::set`] each sync).
    screen: ParryScreen,
}

impl ParryScene {
    pub fn new(art: &ParryArt, fonts: &Fonts) -> ParryScene {
        let cursor_node = art.list_library.as_ref().and_then(|m| Movie::instance_of(m, "tres.rsl.ListItemBgI2", "gp_cursor", Some("select")));
        ParryScene { fonts: fonts.clone(), shapes: Shapes::new(), cursor_node, age: 0.0, screen: ParryScreen::default() }
    }

    /// The page's state as the menu holds it.
    pub fn set(&mut self, screen: &ParryScreen) {
        if self.screen != *screen {
            self.screen = screen.clone();
        }
    }

    pub fn screen(&self) -> &ParryScreen {
        &self.screen
    }

    pub fn step(&mut self) {
        self.age += FRAME;
    }

    /// One line's height of a font class at `size`, and `text` broken into lines that fit `width`.
    fn wrapped(&self, class: &str, size: f32, width: f32, text: &str) -> (String, usize, f32) {
        let Some(font) = self.fonts.get(Some(class)) else { return (text.to_string(), text.split('\n').count(), size * 1.2) };
        let line_h = (font.ascent + font.descent + font.leading) * size / font.units_per_em.max(1.0);
        let out = crate::render::wrap_text(&self.fonts, font, text, size, width - 4.0);
        let n = out.split('\n').count();
        (out, n, line_h)
    }

    /// THE PAGE'S NODES, back to front.
    pub fn nodes(&self) -> Vec<Node> {
        let screen = &self.screen;
        let mut out = Vec::new();
        let Some(page) = screen.page.as_ref().filter(|_| screen.on) else { return out };
        let fade = (self.age / FADE_IN_S).clamp(0.0, 1.0);
        let s = &self.shapes;
        let shp = |name: String, ch: u32, sh: &Arc<Shape>, m: Matrix, cx: Cx| node(name, ch, Kind::Shape(sh.clone()), m, cx);
        let txt = |name: String, def: Arc<TextDef>, text: &str, x: f32, y: f32, cx: Cx| node(name, CH + 2, Kind::Text(TextNode { def, text: Some(text.to_string()), adjust: None, icon_native: false }), Matrix::translate(x, y), cx);
        // the title bar: the page's name, his name and level
        out.push(shp(format!("{PREFIX}title"), CH + 1, &s.title, Matrix::translate(X0, TITLE_Y), alpha(fade)));
        out.push(txt(format!("{PREFIX}title_t"), text_def(FONT_MENU, SIZE_TITLE, WHITE, Align::Left, 700.0, 34.0), &page.title, X0 + 18.0, TITLE_Y + 1.0, alpha(fade)));
        let who = format!("{}   Lv. {}", page.name, page.level);
        out.push(txt(format!("{PREFIX}who"), text_def(FONT_MENU, SIZE_TITLE, WHITE, Align::Right, 500.0, 34.0), &who, X1 - 518.0, TITLE_Y + 1.0, alpha(fade)));
        // the column headings
        out.push(shp(format!("{PREFIX}head"), CH + 3, &s.head, Matrix::translate(X0, HEAD_Y), alpha(fade)));
        for (k, (label, (x, w))) in [(HEAD_TECHNIQUE, COL_NAME), (HEAD_RANK, COL_RANK), (HEAD_WHEN, COL_WHEN), (HEAD_HOW, COL_HOW), (HEAD_STAGGER, COL_STAGGER)].into_iter().enumerate() {
            let x = if k == 0 { COL_ICON } else { x };
            out.push(txt(format!("{PREFIX}head{k}"), text_def(FONT_BODY, SIZE_HEAD, WHITE, Align::Left, w + 40.0, HEAD_H), label, x, HEAD_Y + 2.0, alpha(fade)));
        }
        // the rows' bands, the focused one's KH3 cursor, the rules
        for (i, _) in page.rows.iter().enumerate() {
            let y = ROW_Y + i as f32 * ROW_H;
            out.push(shp(format!("{PREFIX}row{i}"), CH + 4 + (i % 2) as u32, &s.row[i % 2], Matrix::translate(X0, y), alpha(fade)));
            out.push(shp(format!("{PREFIX}row{i}_rule"), CH + 6, &s.rule, Matrix::translate(X0, y + ROW_H - 1.0), alpha(fade)));
        }
        if let Some(i) = Some(screen.cursor).filter(|i| *i < page.rows.len()) {
            let y = ROW_Y + i as f32 * ROW_H;
            match self.cursor_node.clone() {
                Some(mut c) => {
                    // the list rows' select bar, drawn over (-4, -4) .. (724, 84) of its origin, stretched over the row
                    let (sx, sy) = ((X1 - X0) / 728.0, (ROW_H - 2.0) / 88.0);
                    c.matrix = Matrix::new(sx, 0.0, 0.0, sy, X0 + 4.0 * sx, y + 4.0 * sy);
                    c.cx = alpha(fade * 0.95);
                    out.push(c);
                }
                None => out.push(shp(format!("{PREFIX}focus"), CH + 7, &s.focus, Matrix::translate(X0, y), alpha(fade))),
            }
        }
        // the rows' words
        for (i, r) in page.rows.iter().enumerate() {
            let y = ROW_Y + i as f32 * ROW_H;
            let cx = if r.learned() { alpha(fade) } else { tint(LOCKED_TINT, fade) };
            let centred = |n: usize, line_h: f32| y + ((ROW_H - n as f32 * line_h) / 2.0 - 3.0).max(2.0);
            // KH3's ability icon: set (learned, always on) or not; a lone icon is drawn centred in its field: the row's height
            let icon = if r.learned() { "<icon(SetAbility)>" } else { "<icon(Ability)>" };
            out.push(txt(format!("{PREFIX}r{i}_icon"), text_def(FONT_MENU, SIZE_NAME, WHITE, Align::Left, 36.0, ROW_H), icon, COL_ICON, y, cx));
            let (name, n, lh) = self.wrapped(FONT_BODY, SIZE_NAME, COL_NAME.1, &r.name);
            out.push(txt(format!("{PREFIX}r{i}_name"), text_def(FONT_BODY, SIZE_NAME, WHITE, Align::Left, COL_NAME.1, ROW_H), &name, COL_NAME.0, centred(n, lh), cx));
            // rank: three pips and "Rank N", or the learn level (and a saved rank waiting)
            if r.learned() {
                out.push(txt(format!("{PREFIX}r{i}_rank"), text_def(FONT_BODY, SIZE_BODY, GOLD, Align::Left, COL_RANK.1, 30.0), &rank_line(r.rank), COL_RANK.0, y + 9.0, cx));
                for k in 0..3u8 {
                    let (sh, ch) = if k < r.rank { (&s.pip_on, CH + 8) } else { (&s.pip_off, CH + 9) };
                    out.push(shp(format!("{PREFIX}r{i}_pip{k}"), ch, sh, Matrix::translate(COL_RANK.0 + 12.0 + k as f32 * 28.0, y + ROW_H - 26.0), cx));
                }
            } else {
                let lines = if r.saved > 0 { 2 } else { 1 };
                let (_, _, lh) = self.wrapped(FONT_BODY, SIZE_BODY, COL_RANK.1, "x");
                let top = centred(lines, lh);
                out.push(txt(format!("{PREFIX}r{i}_learn"), text_def(FONT_BODY, SIZE_BODY, SOFT, Align::Left, COL_RANK.1, 30.0), &learn_line(r.learn_level), COL_RANK.0, top, alpha(fade)));
                if r.saved > 0 {
                    out.push(txt(format!("{PREFIX}r{i}_ready"), text_def(FONT_BODY, SIZE_BODY, GOLD, Align::Left, COL_RANK.1, 30.0), &ready_line(r.saved), COL_RANK.0, top + lh, alpha(fade)));
                }
            }
            // when: the danger "!" first for a perilous attack
            if r.perilous {
                let (_, _, lh) = self.wrapped(FONT_MENU, SIZE_MARK, MARK_W, "!");
                let def = text_def(FONT_MENU, SIZE_MARK, RED, Align::Left, MARK_W / MARK_NARROW, ROW_H);
                let at = Matrix::new(MARK_NARROW, 0.0, 0.0, 1.0, COL_WHEN.0 - 2.0, centred(1, lh));
                out.push(node(format!("{PREFIX}r{i}_mark"), CH + 2, Kind::Text(TextNode { def, text: Some("!".to_string()), adjust: None, icon_native: false }), at, cx));
            }
            let (when, n, lh) = self.wrapped(FONT_BODY, SIZE_BODY, COL_WHEN.1 - MARK_W, &r.when);
            out.push(txt(format!("{PREFIX}r{i}_when"), text_def(FONT_BODY, SIZE_BODY, WHITE, Align::Left, COL_WHEN.1 - MARK_W, ROW_H), &when, COL_WHEN.0 + MARK_W, centred(n, lh), cx));
            // the input: KH3's own inline-image rule (the icons at their table size, 30 pixels: the stick's arrows read)
            // (broken here, narrower than the field for the icons' extra size: the field's own wrap then keeps these lines)
            let (how, n, lh) = self.wrapped(FONT_BODY, SIZE_BODY, COL_HOW.1 - 30.0, &r.how);
            let def = Arc::new(TextDef { word_wrap: true, leading: HOW_LEADING, ..(*text_def(FONT_BODY, SIZE_BODY, WHITE, Align::Left, COL_HOW.1, ROW_H)).clone() });
            let top = y + ((ROW_H - n as f32 * lh - (n as f32 - 1.0) * HOW_LEADING) / 2.0).max(2.0);
            out.push(node(format!("{PREFIX}r{i}_how"), CH + 2, Kind::Text(TextNode { def, text: Some(how), adjust: None, icon_native: true }), Matrix::translate(COL_HOW.0, top), cx));
            // the Stagger it adds (THE USER 2026-11-02: "The parries are there to make the enemy's stagger go up fast"): the number,
            // three bars under it
            out.push(txt(format!("{PREFIX}r{i}_stagger"), text_def(FONT_BODY, SIZE_BODY, GOLD, Align::Left, COL_STAGGER.1, 30.0), &r.stagger, COL_STAGGER.0, y + 9.0, cx));
            for k in 0..3u8 {
                let (sh, ch) = if k < r.stagger_bars { (&s.bar_on, CH + 10) } else { (&s.bar_off, CH + 11) };
                out.push(shp(format!("{PREFIX}r{i}_bar{k}"), ch, sh, Matrix::translate(COL_STAGGER.0 + 2.0 + k as f32 * BAR.2, y + ROW_H - 30.0), cx));
            }
        }
        // the footer: the break leads to Goofy Breaker (KH3's inline icons at their table size)
        if !page.footer.is_empty() {
            let w = X1 - X0 - 36.0;
            let (footer, n, lh) = self.wrapped(FONT_BODY, SIZE_FOOTER, w - 4.0, &page.footer);
            let def = Arc::new(TextDef { word_wrap: true, leading: HOW_LEADING, ..(*text_def(FONT_BODY, SIZE_FOOTER, SOFT, Align::Left, w, n as f32 * (lh + HOW_LEADING))).clone() });
            let y = ROW_Y + page.rows.len() as f32 * ROW_H + FOOTER_GAP;
            out.push(node(format!("{PREFIX}footer"), CH + 2, Kind::Text(TextNode { def, text: Some(footer), adjust: None, icon_native: true }), Matrix::translate(X0 + 18.0, y), alpha(fade)));
        }
        out
    }

    /// PUTS THE PAGE INTO THE MOVIE: the root's script children named [`PREFIX`]... are replaced (the KH3 cursor keeps its
    /// own timeline: it is carried over from the last frame).
    pub fn apply(&mut self, movie: &mut Movie) {
        if let Some(pos) = movie.root.extra.iter().position(|n| n.name.as_deref() == Some("gp_cursor")) {
            self.cursor_node = Some(movie.root.extra.remove(pos));
        }
        let nodes = self.nodes();
        clear(movie);
        movie.root.extra.extend(nodes);
    }
}

/// Takes the page out of the movie.
pub fn clear(movie: &mut Movie) {
    movie.root.extra.retain(|n| !n.name.as_deref().is_some_and(|s| s.starts_with(PREFIX)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(ranks: &[(u8, u8)]) -> ParryPage {
        let rows = ranks.iter().enumerate().map(|(i, (rank, saved))| ParryRow { name: format!("T{i}"), learn_level: i as u32 * 5, rank: *rank, saved: *saved, when: "w".into(), how: "<icon(Square)>".into(), stagger: format!("+{}", 20 + i), stagger_bars: (i % 3) as u8 + 1, detail: "d".into(), perilous: i % 2 == 1 }).collect();
        ParryPage { title: "Parry Techniques".into(), name: "Goofy".into(), level: 7, rows, footer: "Fill the gauge, then <icon(Triangle)>.".into() }
    }

    #[test]
    fn y_opens_only_with_a_page_and_b_or_y_goes_back() {
        let mut sounds = Vec::new();
        let mut s = ParryScreen::default();
        assert!(!s.open(&mut sounds) && !s.on && sounds.is_empty(), "no page: no next list");
        s.set(Some(page(&[(1, 0), (1, 0), (0, 2)])));
        assert!(s.open(&mut sounds) && s.on);
        assert_eq!(sounds, vec![UiSound::Play(SE_MENU_OPEN)]);
        // up wraps to the last row on a fresh press; a held key stops at the ends
        s.key(CampKey::Up, false, &mut sounds);
        assert_eq!(s.cursor, 2);
        s.key(CampKey::Down, true, &mut sounds);
        assert_eq!(s.cursor, 2, "held: stops at the end");
        s.key(CampKey::Down, false, &mut sounds);
        assert_eq!(s.cursor, 0);
        s.key(CampKey::Decide, false, &mut sounds);
        assert!(s.on, "A only shows");
        s.key(CampKey::Cancel, false, &mut sounds);
        assert!(!s.on);
        assert_eq!(sounds.last(), Some(&UiSound::Play(SE_MENU_CLOSE)));
        s.open(&mut sounds);
        s.key(CampKey::Triangle, false, &mut sounds);
        assert!(!s.on, "Y again: the next list (the abilities)");
        // the page going away takes it down
        s.open(&mut sounds);
        s.set(None);
        assert!(!s.on && !s.available());
    }

    #[test]
    fn the_cursor_survives_a_new_feed() {
        let mut sounds = Vec::new();
        let mut s = ParryScreen::default();
        s.set(Some(page(&[(1, 0); 8])));
        s.open(&mut sounds);
        for _ in 0..5 {
            s.key(CampKey::Down, false, &mut sounds);
        }
        s.set(Some(page(&[(2, 0); 8])));
        assert_eq!((s.on, s.cursor), (true, 5));
        s.set(Some(page(&[(1, 0); 3])));
        assert_eq!(s.cursor, 2, "clamped to a shorter page");
    }

    #[test]
    fn the_rows_texts() {
        let p = page(&[(2, 0), (0, 0), (0, 3)]);
        assert_eq!(rank_line(p.rows[0].rank), "Rank 2");
        assert_eq!(learn_line(p.rows[1].learn_level), "Learned at Lv. 5");
        assert_eq!(ready_line(p.rows[2].saved), "Rank 3 ready");
        assert!(p.rows[0].help().ends_with("Rank 2 of 3: always on, nothing to equip."));
        assert!(p.rows[1].help().ends_with("Learned at Lv. 5."));
        assert!(p.rows[2].help().contains("saved Rank 3"));
    }

    #[test]
    fn the_nodes_follow_the_page() {
        let mut sounds = Vec::new();
        let mut s = ParryScreen::default();
        let mut scene = ParryScene::new(&ParryArt::default(), &Fonts::default());
        scene.set(&s);
        assert!(scene.nodes().is_empty(), "nothing while off");
        s.set(Some(page(&[(1, 0), (0, 0), (0, 2), (3, 0)])));
        s.open(&mut sounds);
        scene.set(&s);
        let nodes = scene.nodes();
        let named = |n: &str| nodes.iter().any(|x| x.name.as_deref() == Some(n));
        assert!(named("gp_r0_pip0") && named("gp_r3_pip2") && !named("gp_r1_pip0"), "pips on learned rows only");
        assert!(named("gp_r1_learn") && !named("gp_r1_ready") && named("gp_r2_ready"), "locked rows: level, and a saved rank");
        assert!(named("gp_r1_mark") && !named("gp_r0_mark"), "the danger mark on perilous rows");
        assert!(named("gp_focus"), "the stand-in cursor without the pack");
        // (THE USER 2026-11-02: "Redesign the menu for this as well") the Stagger column and the footer, no counter column
        assert!(named("gp_r0_stagger") && named("gp_r3_stagger") && named("gp_footer"), "the Stagger column and the footer");
        let lit = |r: usize| (0..3).filter(|k| nodes.iter().any(|x| x.name.as_deref() == Some(format!("gp_r{r}_bar{k}").as_str()) && x.ch == CH + 10)).count();
        assert_eq!((lit(0), lit(1), lit(2), lit(3)), (1, 2, 3, 1), "lit bars follow the rows");
        assert!(nodes.iter().all(|n| !n.name.as_deref().is_some_and(|x| x.contains("counter"))), "no counter column");
        assert!(!HEAD_STAGGER.contains('<'), "a plain heading: no (A) icon");
        assert!(nodes.iter().all(|n| n.name.as_deref().is_some_and(|x| x.starts_with(PREFIX))));
    }
}
