//! The list widgets of the menu movies, ported from their AS3 classes (the movies' own
//! `tres.controls.List`, `tres.controls.ScrollBar` and `tres.controls.TabControl`; listing in
//! `build/sora_pack/ui/as3/CampMenu.as3.txt`):
//!
//! - a `List` clip holds a `container` with ONE authored row, `listItem` (a `ListItem` subclass
//!   with a `rect` clip that gives the row pitch). The constructor remembers the row's class and
//!   removes it; `rowNum` / `columnNum` (set by the parent's `__setProp_*`: 10 rows, 1 column,
//!   `enableScrollLoop`, `enablePaging` for every list of the camp menu) create that many rows
//!   at `y = row * rect.height - rect.y`; the data behind the rows scrolls by `scrollPosition`;
//! - `moveAnchor` moves the selection (wrapping on a fresh press, stopping while the key
//!   repeats: `repeatStopUp` / `repeatStopDown` are true on these lists), `nextPage` / `prevPage`
//!   jump a page;
//! - the scroll bar's thumb is `trackHeight * pageSize` tall (not below its authored height) and
//!   sits at `(trackHeight - thumbHeight) / (max - min) * position + margin`.
//!
//! Only what the camp menu uses is ported: one column, no category rows (`categoryLevel` is 0
//! for every row the port fills).

use crate::geom::{Matrix, Rect};
use crate::player::{Kind, Movie, Sprite};

/// The untransformed bounds of what a clip holds (shapes and bitmaps, through its children).
pub fn local_bounds(s: &Sprite) -> Rect {
    let mut r = Rect::EMPTY;
    for node in s.children.iter().chain(&s.extra) {
        let b = match &node.kind {
            Kind::Shape(shape) => shape.bounds.rect(),
            Kind::Morph(shape, _) => shape.bounds.rect(),
            Kind::Image(image) => Rect::new(0.0, 0.0, image.size.0, image.size.1),
            Kind::Sprite(child) => local_bounds(child),
            _ => Rect::EMPTY,
        };
        if !b.is_empty() {
            r = r.union(&b.transform(&node.matrix));
        }
    }
    r
}

/// What the filler says about a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowState {
    /// `Button.enabled`: a disabled row shows `disable` / `disableSelect`.
    pub enabled: bool,
}

impl Default for RowState {
    fn default() -> RowState {
        RowState { enabled: true }
    }
}

/// `tres.controls.ScrollBar`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScrollBar {
    pub path: String,
    /// (m_height, m_trackHeight, m_thumbMinHeight, m_thumbMargin, bg height at scale 1, thumb
    /// height at scale 1, bg.y): taken at the first use (the constructor).
    init: Option<[f32; 7]>,
    shown: Option<(u32, u32, u32, bool)>,
}

impl ScrollBar {
    pub fn new(path: impl Into<String>) -> ScrollBar {
        ScrollBar { path: path.into(), init: None, shown: None }
    }

    fn construct(&mut self, movie: &mut Movie) -> Option<[f32; 7]> {
        if let Some(init) = self.init {
            // The clip may have been rebuilt by its parent's timeline.
            if movie.node(&self.path).is_some_and(|n| n.script_matrix) {
                return Some(init);
            }
        }
        let node = movie.node(&self.path)?;
        let scale_y = node.matrix.d;
        let s = node.sprite()?;
        let (bg, thumb) = (s.child("bg")?, s.child("thumb")?);
        let bg_h = local_bounds(bg.sprite()?).y1 - local_bounds(bg.sprite()?).y0;
        let thumb_h = local_bounds(thumb.sprite()?).y1 - local_bounds(thumb.sprite()?).y0;
        if bg_h <= 0.0 || thumb_h <= 0.0 {
            return None;
        }
        // <init>: thumbMinHeight = thumb.height; thumbMargin = thumb.y - bg.y; height = the
        // clip's own height (scaled by its placement), then scaleY = 1.
        let thumb_min = thumb_h * thumb.matrix.d;
        let margin = thumb.matrix.ty - bg.matrix.ty;
        let whole = local_bounds(s);
        let height = ((whole.y1 - whole.y0) * scale_y).max(0.0);
        let track = (height - margin * 2.0).max(0.0);
        let init = [height, track, thumb_min, margin, bg_h, thumb_h, bg.matrix.ty];
        let bar = movie.node_mut(&self.path)?;
        bar.matrix.d = 1.0;
        bar.script_matrix = true;
        if let Some(bg) = bar.sprite_mut().and_then(|s| s.child_mut("bg")) {
            bg.matrix.d = height / bg_h;
            bg.script_matrix = true;
        }
        self.init = Some(init);
        self.shown = None;
        Some(init)
    }

    /// `setScrollProperty(0, max, pageSize)` + `setScrollPosition(position)` + `show()` /
    /// `hide()`.
    pub fn set(&mut self, movie: &mut Movie, max: u32, position: u32, page: f32, visible: bool) {
        let Some([_, track, thumb_min, margin, _, thumb_h, bg_y]) = self.construct(movie) else { return };
        let key = (max, position.min(max), (page * 10_000.0) as u32, visible);
        if self.shown == Some(key) {
            return;
        }
        self.shown = Some(key);
        movie.set_visible(&self.path, visible);
        let Some(thumb) = movie.node_mut(&self.path).and_then(|n| n.sprite_mut()).and_then(|s| s.child_mut("thumb")) else { return };
        let height = (track * page).max(thumb_min);
        thumb.matrix.d = height / thumb_h;
        let per = if max > 0 { (track - height) / max as f32 } else { 0.0 };
        thumb.matrix.ty = per * position.min(max) as f32 + margin + bg_y;
        thumb.script_matrix = true;
    }
}

/// `tres.controls.List` with one column.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListView {
    /// The list clip.
    pub path: String,
    /// The AS3 class of the authored `listItem`.
    pub item_class: &'static str,
    /// `rowNum`.
    pub rows: usize,
    pub scroll_loop: bool,
    /// `repeatStopUp` / `repeatStopDown`.
    pub repeat_stop: bool,
    /// `m_listDataNum`.
    pub count: usize,
    /// `m_focusIndex` (-1: no row).
    pub focus: i32,
    pub scroll: usize,
    /// `m_hasFocus`: the selected row shows `select`.
    pub has_focus: bool,
    pitch: f32,
    offset: (f32, f32),
    scrollbar: ScrollBar,
    /// Per row slot: (data index or -1, selected, enabled) as last shown.
    shown: Vec<(i64, bool, bool)>,
}

impl ListView {
    pub fn new(path: impl Into<String>, item_class: &'static str) -> ListView {
        let path = path.into();
        ListView { scrollbar: ScrollBar::new(format!("{path}.scrollbar")), path, item_class, rows: 10, scroll_loop: true, repeat_stop: true, count: 0, focus: -1, scroll: 0, has_focus: true, pitch: 48.0, offset: (0.0, 0.0), shown: Vec::new() }
    }

    pub fn item_path(&self, slot: usize) -> String {
        format!("{}.container.i{slot}", self.path)
    }

    /// The constructor and `createListItems`: the authored row gives the class and the pitch
    /// and goes; `rows` instances `i0 ..` take its place. True when the rows exist.
    fn ensure(&mut self, movie: &mut Movie) -> bool {
        let container = format!("{}.container", self.path);
        if movie.exists(&self.item_path(0)) {
            return true;
        }
        if !movie.exists(&container) {
            return false;
        }
        if let Some(rect) = movie.node(&format!("{container}.listItem.rect")) {
            if let Some(b) = rect.sprite().map(local_bounds).filter(|b| !b.is_empty()) {
                let b = b.transform(&rect.matrix);
                self.pitch = b.y1 - b.y0;
                self.offset = (b.x0, b.y0);
            }
        }
        movie.set_visible(&format!("{container}.listItem"), false);
        for slot in 0..self.rows {
            let name = format!("i{slot}");
            if !movie.add_instance(&container, self.item_class, &name) {
                return false;
            }
            let path = format!("{container}.{name}");
            movie.set_position(&path, -self.offset.0, slot as f32 * self.pitch - self.offset.1, 0.0);
            // `l4.removeChild(l4.rect)`.
            movie.set_visible(&format!("{path}.rect"), false);
            movie.set_visible(&path, false);
        }
        self.shown = vec![(-2, false, false); self.rows];
        true
    }

    pub fn scroll_max(&self) -> usize {
        self.count.saturating_sub(self.rows)
    }

    /// `createListData(n)` / `setListDataNum(n)`: the focus is kept when it still names a row.
    pub fn set_count(&mut self, count: usize) {
        self.count = count;
        if count == 0 {
            self.focus = -1;
        } else if self.focus < 0 || self.focus as usize >= count {
            self.focus = 0;
        }
        self.scroll = self.scroll.min(self.scroll_max());
        self.adjust_display();
    }

    /// `adjustDisplay(focus)`: a focused row outside the window scrolls into it.
    pub fn adjust_display(&mut self) {
        if self.focus < 0 {
            return;
        }
        let focus = self.focus as usize;
        if focus < self.scroll || focus >= self.scroll + self.rows {
            self.scroll = focus.min(self.scroll_max());
        }
    }

    pub fn set_focus(&mut self, index: usize) {
        if index < self.count {
            self.focus = index as i32;
            self.adjust_display();
        }
    }

    fn adjust_scroll(&mut self, to: usize, upward: bool) {
        if upward {
            if self.scroll > to {
                self.scroll = to;
            }
        } else {
            let least = (to + 1).saturating_sub(self.rows);
            if self.scroll < least {
                self.scroll = least;
            }
        }
    }

    /// `moveAnchor(up / down, repeat)`. True when the selection moved (the focus controller
    /// then plays the cursor sound).
    pub fn move_anchor(&mut self, down: bool, repeat: bool) -> bool {
        if self.count == 0 || self.focus < 0 {
            return false;
        }
        let (focus, last) = (self.focus as usize, self.count - 1);
        let (to, upward) = if down {
            if focus < last {
                (focus + 1, false)
            } else if self.scroll_loop && !(repeat && self.repeat_stop) {
                (0, true)
            } else {
                return false;
            }
        } else if focus > 0 {
            (focus - 1, true)
        } else if self.scroll_loop && !(repeat && self.repeat_stop) {
            (last, false)
        } else {
            return false;
        };
        self.adjust_scroll(to, upward);
        let moved = to != focus;
        self.focus = to as i32;
        moved
    }

    /// `nextPage(repeat)` (`enablePaging`, only when the data does not fit the rows).
    pub fn next_page(&mut self, repeat: bool) -> bool {
        if self.count <= self.rows || self.focus < 0 {
            return false;
        }
        let (focus, last) = (self.focus as usize, self.count - 1);
        if focus == last {
            if repeat || !self.scroll_loop {
                return false;
            }
            self.scroll = 0;
            self.focus = 0;
            return true;
        }
        let mut to = focus + self.rows;
        if self.scroll < self.scroll_max() {
            let mut scroll = self.scroll + self.rows;
            if scroll > self.scroll_max() {
                to -= scroll - self.scroll_max();
                scroll = self.scroll_max();
            }
            self.scroll = scroll;
        }
        self.focus = to.min(last) as i32;
        true
    }

    /// `prevPage(repeat)`: without category rows it does not wrap at the top.
    pub fn prev_page(&mut self, _repeat: bool) -> bool {
        if self.count <= self.rows || self.focus < 0 {
            return false;
        }
        let focus = self.focus as i64;
        let mut to = focus - self.rows as i64;
        if self.scroll > 0 {
            let scroll = self.scroll as i64 - self.rows as i64;
            if scroll < 0 {
                to -= scroll;
            }
            self.scroll = scroll.max(0) as usize;
        }
        self.focus = to.max(0) as i32;
        self.focus as i64 != focus
    }

    /// `refreshListData`: the rows of the window are filled (the native `OnListRefreshData`
    /// handler writes the row clips), the selected row shows its state, the scroll bar follows.
    /// `fill(movie, row clip path, data index)`.
    pub fn refresh(&mut self, movie: &mut Movie, mut fill: impl FnMut(&mut Movie, &str, usize) -> RowState) {
        if !self.ensure(movie) {
            return;
        }
        // A list state that was built anew over rows that exist already.
        if self.shown.len() != self.rows {
            self.shown = vec![(-2, false, false); self.rows];
        }
        for slot in 0..self.rows {
            let path = self.item_path(slot);
            let index = self.scroll + slot;
            if index >= self.count {
                if self.shown[slot].0 != -1 {
                    movie.set_visible(&path, false);
                    self.shown[slot] = (-1, false, false);
                }
                continue;
            }
            movie.set_visible(&path, true);
            let state = fill(movie, &path, index);
            let selected = self.has_focus && self.focus == index as i32;
            let now = (index as i64, selected, state.enabled);
            if self.shown[slot] != now {
                // `Button.updateState` -> `setState` with its fallbacks for a missing label.
                let labels: &[&str] = match (state.enabled, selected) {
                    (true, true) => &["select"],
                    (true, false) => &["normal"],
                    (false, true) => &["disableSelect", "disable"],
                    (false, false) => &["disable"],
                };
                for label in labels {
                    if movie.goto_and_play(&path, label) {
                        break;
                    }
                }
                self.shown[slot] = now;
            }
        }
        let scrolls = self.count > self.rows;
        let page = if self.count > 0 { (self.rows as f32 / self.count as f32).min(1.0) } else { 1.0 };
        self.scrollbar.set(movie, self.scroll_max() as u32, self.scroll as u32, page, scrolls);
    }

    /// Forces every row to be shown again (after the clip was rebuilt or the data changed its
    /// meaning under the same indices).
    pub fn invalidate(&mut self) {
        for slot in &mut self.shown {
            *slot = (-2, false, false);
        }
    }
}

/// (IMPL_ER23B) The component parameters of `CoTabItem.imageIcon` (ItemListLibrary, method 312): the sheet of the
/// tab icons, its cell and its column count.
pub const TAB_ICON_BITMAP: &str = "ItemListLibrary_bmpIcon";
pub const TAB_ICON_CELL: (f32, f32) = (68.0, 32.0);
pub const TAB_ICON_COLUMNS: i32 = 3;

/// `tres.controls.TabControl` (`tres.rsl.ListTabCamp`): `count` tab items spread over the
/// authored `rect`, the selected one on `select`, the name of the selected tab in `textField`,
/// `switchL` / `switchR` flash on a change.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabBar {
    pub path: String,
    pub count: usize,
    pub select: usize,
    built: usize,
}

impl TabBar {
    pub fn new(path: impl Into<String>) -> TabBar {
        TabBar { path: path.into(), count: 0, select: 0, built: 0 }
    }

    /// `createTab(count)`, then `setTabItemIconIndex(k, icons[k])` for every tab (IMPL_ER23B: the native side calls it
    /// right after `createTab`, e.g. `UTresUIP_CampMenuBackyard` 0x1415C1950; a tab without an entry keeps the
    /// component's own `index` = 1).
    pub fn build(&mut self, movie: &mut Movie, count: usize, icons: &[i32]) {
        self.count = count;
        self.select = self.select.min(count.saturating_sub(1));
        if self.built == count && movie.exists(&format!("{}.t0", self.path)) {
            return;
        }
        let item = format!("{}.tabItem", self.path);
        let (Some(rect), Some(template)) = (movie.node(&format!("{}.rect", self.path)), movie.node(&item)) else { return };
        let (Some(rect_bounds), Some(item_sprite)) = (rect.sprite().map(local_bounds), template.sprite()) else { return };
        let Some(class) = item_sprite.def.class.clone() else { return };
        let item_width = { let b = local_bounds(item_sprite).transform(&template.matrix); b.x1 - b.x0 };
        let rect_box = rect_bounds.transform(&rect.matrix);
        // <init>: m_tabStartPos = (rect.x - tabItem.width / 2, rect.y); m_tabRectWidth = rect.width.
        let (start_x, start_y, rect_width) = (rect.matrix.tx - item_width / 2.0, rect.matrix.ty, rect_box.x1 - rect_box.x0);
        movie.set_visible(&format!("{}.rect", self.path), false);
        movie.set_visible(&item, false);
        for old in 0..self.built.max(16) {
            movie.remove_instance(&self.path, &format!("t{old}"));
        }
        let width = rect_width / count.max(1) as f32;
        for k in 0..count {
            let name = format!("t{k}");
            if !movie.add_instance(&self.path, &class, &name) {
                return;
            }
            let path = format!("{}.{name}", self.path);
            movie.set_position(&path, start_x + (k as f32 + 0.5) * width, start_y, 0.0);
            // `setBGWidth(width)`.
            let Some(tab) = movie.sprite_mut(&path) else { continue };
            let Some(bg) = tab.child_mut("bg") else { continue };
            let bg_width = bg.sprite().map(local_bounds).map_or(0.0, |b| b.x1 - b.x0);
            let (mut scale, bg_x) = (1.0, bg.matrix.tx);
            if bg_width > 0.0 {
                scale = width / bg_width;
                bg.matrix.a = scale;
                bg.script_matrix = true;
            }
            for (side, sign) in [("leftAlignObject", -1.0), ("rightAlignObject", 1.0)] {
                if let Some(object) = tab.child_mut(side) {
                    object.script_visible = Some(scale > 0.9);
                    object.matrix.tx = bg_x + sign * width * 0.5;
                    object.script_matrix = true;
                }
            }
            // `CoTabItem.__setProp_imageIcon_CoTabItem_imageIcon_0` (bmpName, 68 x 32 cells, row 3, column 3, index 1),
            // then the native `setTabItemIconIndex`.
            if let Some(icon) = tab.child_mut("imageIcon").and_then(|n| n.sprite_mut()) {
                crate::controls::image_icon_set(icon, TAB_ICON_BITMAP, TAB_ICON_CELL.0, TAB_ICON_CELL.1, 3, TAB_ICON_COLUMNS, icons.get(k).copied().unwrap_or(1));
            }
            movie.goto_and_play(&path, if k == self.select { "select" } else { "normal" });
        }
        self.built = count;
    }

    /// The pad icons of the two switch arrows (the native side writes the L2 / R2 icon texts).
    pub fn set_switch_icons(&self, movie: &mut Movie, left: &str, right: &str, fonts: &crate::render::Fonts) {
        movie.set_text(&format!("{}.switchL.txBtn", self.path), left, fonts);
        movie.set_text(&format!("{}.switchR.txBtn", self.path), right, fonts);
    }

    /// `selectIndex = index` + `setText(name)`; `flash` = the switch arrow that plays `anim`
    /// (`nextTab` / `prevTab`).
    pub fn select(&mut self, movie: &mut Movie, index: usize, name: &str, flash: Option<bool>, fonts: &crate::render::Fonts) {
        if index < self.count && index != self.select {
            movie.goto_and_play(&format!("{}.t{}", self.path, self.select), "normal");
            movie.goto_and_play(&format!("{}.t{index}", self.path), "select");
            self.select = index;
        }
        movie.set_text(&format!("{}.textField", self.path), name, fonts);
        if let Some(right) = flash {
            movie.goto_and_play(&format!("{}.{}", self.path, if right { "switchR" } else { "switchL" }), "anim");
        }
    }
}

/// The native scroll of the Config screen (`UTresASProxyRSLConfig`: constructor `0x141530D90`
/// sets 10 visible rows, margin 0, maximum scroll 16; rule `0x14154E060`): the window follows
/// the focused row, the pack moves up by the focused window's first row.
pub fn window_scroll(focus: usize, scroll: usize, visible: usize, margin: usize, max: usize) -> usize {
    let (focus, scroll, visible, margin) = (focus as i64, scroll as i64, visible as i64, margin as i64);
    let mut to = scroll;
    if focus - scroll < visible - margin {
        if focus - scroll <= margin - 1 {
            to = focus - margin;
        }
    } else {
        to = focus + 1 + (margin - visible);
    }
    to.clamp(0, max as i64) as usize
}

/// A matrix helper for scripts that move a clip without touching its scale.
pub fn set_y(movie: &mut Movie, path: &str, y: f32) {
    if let Some(node) = movie.node_mut(path) {
        node.matrix = Matrix::new(node.matrix.a, node.matrix.b, node.matrix.c, node.matrix.d, node.matrix.tx, y);
        node.script_matrix = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(count: usize) -> ListView {
        let mut l = ListView::new("x", "X");
        l.set_count(count);
        l
    }

    #[test]
    fn move_anchor_scrolls_wraps_and_stops() {
        let mut l = list(88);
        assert_eq!((l.focus, l.scroll, l.scroll_max()), (0, 0, 78));
        // Down through the window: the scroll starts when the focus leaves the 10 rows.
        for _ in 0..9 {
            assert!(l.move_anchor(true, false));
        }
        assert_eq!((l.focus, l.scroll), (9, 0));
        l.move_anchor(true, true);
        assert_eq!((l.focus, l.scroll), (10, 1));
        // Up from the top: a fresh press wraps to the last row, a held key stops.
        let mut l = list(88);
        assert!(!l.move_anchor(false, true), "repeatStopUp");
        assert!(l.move_anchor(false, false));
        assert_eq!((l.focus, l.scroll), (87, 78));
        assert!(!l.move_anchor(true, true), "repeatStopDown");
        assert!(l.move_anchor(true, false));
        assert_eq!((l.focus, l.scroll), (0, 0));
        // A list that fits: no scroll.
        let mut l = list(3);
        l.move_anchor(false, false);
        assert_eq!((l.focus, l.scroll, l.scroll_max()), (2, 0, 0));
        assert!(!list(0).move_anchor(true, false));
    }

    #[test]
    fn pages() {
        let mut l = list(25);
        assert!(l.next_page(false));
        assert_eq!((l.focus, l.scroll), (10, 10));
        assert!(l.next_page(false));
        // The scroll stops at its maximum; the focus keeps its row in the window.
        assert_eq!((l.focus, l.scroll), (15, 15));
        assert!(l.next_page(false));
        assert_eq!((l.focus, l.scroll), (24, 15));
        assert!(!l.next_page(true), "held at the end");
        assert!(l.next_page(false));
        assert_eq!((l.focus, l.scroll), (0, 0));
        l.set_focus(24);
        assert!(l.prev_page(false));
        assert_eq!((l.focus, l.scroll), (14, 5));
        l.prev_page(false);
        l.prev_page(false);
        assert_eq!((l.focus, l.scroll), (0, 0));
        assert!(!list(5).next_page(false), "no paging when the data fits");
    }

    #[test]
    fn config_window_rule() {
        // 26 rows, 10 visible, maximum scroll 16.
        assert_eq!(window_scroll(0, 0, 10, 0, 16), 0);
        assert_eq!(window_scroll(9, 0, 10, 0, 16), 0);
        assert_eq!(window_scroll(10, 0, 10, 0, 16), 1);
        assert_eq!(window_scroll(25, 0, 10, 0, 16), 16);
        assert_eq!(window_scroll(15, 16, 10, 0, 16), 15);
        assert_eq!(window_scroll(20, 16, 10, 0, 16), 16);
    }
}
