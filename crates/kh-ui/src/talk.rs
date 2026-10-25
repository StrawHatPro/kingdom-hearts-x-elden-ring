//! (ER party S8) A KH-STYLE TALK WINDOW: a speaker's lines in KH3's own message window (movie Dialog, the window art
//! and text field of `Dialog.setText` in its text form, the DH colour), placed over the speaker on screen, the words
//! revealed letter by letter, the speaker's name on the first line in the markup's strong colour, the pad's decide
//! glyph when a line is all shown. Also the two short forms the unlock scenes need: a PROMPT ("[Y] Talk") over a
//! character and a BANNER ("Donald joined the party!") at the top centre.
//!
//! WHAT IS THE GAME'S [A]: the movie Dialog (window art per world label, its open / close animation `in` / `out`
//! with the ANIM_END events, the text field `dialogText.textField` with its font and leading), the text-form layout of
//! `Dialog.setText` (ported in [`text_layout`]: the window as wide as the text plus the text holder's margins on both
//! sides, clamped to WINDOW_SIZE_MIN / MAX, as high as the holder's top + the text + its bottom margin, centred on the
//! clip's origin), the DH colours (newgame.rs), the menu sounds.
//! WHAT IS COMPOSED HERE [L] (KH3 talks through its subtitle and message systems, not through a window over a field
//! character): the window's place over the speaker (its bottom 40 stage pixels above the given point, kept 24 pixels
//! inside the stage), the typewriter reveal (`REVEAL_PER_S` characters a second; decide shows the rest, a second decide
//! goes on), the name line, the decide glyph at the end of a finished line, the banner's place and time.

use std::sync::Arc;

use crate::camp::CampKey;
use crate::def::TextDef;
use crate::menu::{SE_MENU_CLOSE, SE_MENU_DECIDE, SE_MENU_OPEN};
use crate::player::Kind;
use crate::{Fonts, Matrix, Movie, MovieDef, Pack, PackError, UiSound};

const WIN: &str = "dialog.dialogAnm.dialogWin.dialogWin2";
const PO_ALL: &str = "dialog.dialogAnm.dialogPoAll.dialogPoAll2";
const TEXT: &str = "dialogText";
const FIELD: &str = "dialogText.textField";
/// `Dialog.WINDOW_SIZE_MIN` / `MAX`.
pub const WINDOW_MIN: (f32, f32) = (256.0, 128.0);
pub const WINDOW_MAX: (f32, f32) = (1024.0, 640.0);
/// [L] The widest a talk line runs before it wraps (stage pixels of text; the game's own maximum is the window's 1024).
pub const TALK_TEXT_MAX: f32 = 720.0;
/// [L] Characters revealed per second.
pub const REVEAL_PER_S: f32 = 45.0;
/// [L] The window's bottom this far above the speaker's point; the window kept this far inside the stage.
pub const ABOVE: f32 = 40.0;
pub const MARGIN: f32 = 24.0;
/// [L] The prompt's text: the pad's Y glyph and the word.
pub fn prompt_text(word: &str) -> String {
    format!("<icon(Triangle)> {word}")
}

/// What the window is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TalkKind {
    /// A speaker's line: revealed, decide goes on.
    Line,
    /// A prompt over a character (no input of its own).
    Prompt,
    /// A message at the top centre that leaves by itself.
    Banner,
}

/// What happened in a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TalkEvent {
    /// The window finished its `in` animation.
    Opened,
    /// Decide on a line that was all shown: the host gives the next line or closes.
    Advanced,
    /// The window finished its `out` animation (it is off the screen).
    Closed,
}

/// `Dialog.setText`'s text form: the window, the holder offset, the field's size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextLayout {
    pub window: (f32, f32),
    pub holder: (f32, f32),
    pub field: (f32, f32),
}

/// THE TEXT FORM OF `Dialog.setText` [A]: `text_w` / `text_h` = the field's textWidth / textHeight, `text_pos` = the text
/// holder (`poDialogText`), `default_window` = the authored window.
pub fn text_layout(text_w: f32, text_h: f32, text_pos: (f32, f32), default_window: (f32, f32)) -> TextLayout {
    let width = (text_pos.0 * 2.0 + text_w).clamp(WINDOW_MIN.0, WINDOW_MAX.0);
    let field_w = width - text_pos.0 * 2.0 + 5.0;
    let field_h = text_h + 5.0;
    // l5 = poDialogText.y + field height + m_textPos.y (no buttons)
    let height = (text_pos.1 + field_h + text_pos.1).clamp(WINDOW_MIN.1, WINDOW_MAX.1);
    TextLayout { window: (width + 2.0, height + 2.0), holder: ((default_window.0 - width) * 0.5, (default_window.1 - height) * 0.5), field: (field_w, field_h) }
}

/// The visible characters of a marked-up text (tags `<...>` count as none).
pub fn visible_len(text: &str) -> usize {
    let mut n = 0;
    let mut tag = false;
    for c in text.chars() {
        match c {
            '<' => tag = true,
            '>' if tag => tag = false,
            _ if !tag => n += 1,
            _ => {}
        }
    }
    n
}

/// The first `count` visible characters of a marked-up text, tags kept whole (an open colour span is closed).
pub fn reveal(text: &str, count: usize) -> String {
    let mut out = String::new();
    let mut n = 0;
    let mut chars = text.chars().peekable();
    let mut open = false;
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut tag = String::from("<");
            for t in chars.by_ref() {
                tag.push(t);
                if t == '>' {
                    break;
                }
            }
            if tag.starts_with("<color(") {
                open = true;
            } else if tag == "<color>" {
                open = false;
            }
            if n < count || tag == "<color>" {
                out.push_str(&tag);
            }
            continue;
        }
        if n >= count {
            break;
        }
        out.push(c);
        n += 1;
    }
    if open {
        out.push_str("<color>");
    }
    out
}

pub struct TalkWindow {
    pub dialog: Movie,
    fonts: Fonts,
    open: bool,
    kind: TalkKind,
    /// The text as wrapped for its window, and the characters revealed.
    full: String,
    total: usize,
    shown: f32,
    leaving: bool,
    input_on: bool,
    /// The stage point the window sits over (Line / Prompt), its centre for a banner.
    anchor: (f32, f32),
    layout: TextLayout,
    default_window: (f32, f32),
    win_matrix: Matrix,
    text_pos: (f32, f32),
    field_def: Option<Arc<TextDef>>,
    banner_left: f32,
    /// [L] A prompt / banner's text is centred in its window (the game's minimum window is large for one word).
    centre: (f32, f32),
    events: Vec<TalkEvent>,
    /// (testing) what the window shows now.
    pub shown_text: String,
}

impl TalkWindow {
    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<TalkWindow, PackError> {
        let def: Arc<MovieDef> = pack.movie_linked("Dialog", &crate::camp_menu::LIBRARIES)?;
        let mut w = TalkWindow {
            dialog: Movie::new(def),
            fonts,
            open: false,
            kind: TalkKind::Line,
            full: String::new(),
            total: 0,
            shown: 0.0,
            leaving: false,
            input_on: false,
            anchor: (960.0, 540.0),
            layout: TextLayout { window: (0.0, 0.0), holder: (0.0, 0.0), field: (0.0, 0.0) },
            default_window: (510.0, 186.0),
            win_matrix: Matrix::IDENTITY,
            text_pos: (34.0, 40.0),
            field_def: None,
            banner_left: 0.0,
            centre: (0.0, 0.0),
            events: Vec::new(),
            shown_text: String::new(),
        };
        if let Some(node) = w.dialog.node(WIN) {
            w.win_matrix = node.matrix;
        }
        if let Some(all) = w.dialog.node("dialog.dialogAnm.dialogPoAll") {
            w.default_window = (-2.0 * all.matrix.tx, -2.0 * all.matrix.ty);
        }
        if let Some(n) = w.dialog.node(&format!("{PO_ALL}.poDialogText")) {
            w.text_pos = (n.matrix.tx, n.matrix.ty);
        }
        w.dialog.set_visible("dialogButton", false);
        w.dialog.goto_and_stop(&format!("{WIN}.dialogWinWin"), "dh");
        // the field in the DH colour (`m_colorDictionary["dh"]`, newgame.rs)
        if let Some(node) = w.dialog.node_mut(FIELD) {
            if let Kind::Text(t) = &mut node.kind {
                let mut d: TextDef = (*t.def).clone();
                d.color = crate::newgame::DH_TEXT_COLOR;
                d.word_wrap = true;
                t.def = Arc::new(d);
                w.field_def = Some(t.def.clone());
            }
        }
        w.dialog.set_visible(TEXT, false);
        Ok(w)
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn kind(&self) -> TalkKind {
        self.kind
    }

    /// A line is up and all of it is shown (decide goes on).
    pub fn finished(&self) -> bool {
        self.open && self.kind == TalkKind::Line && self.shown as usize >= self.total
    }

    /// Decide counts (the window is in and not leaving).
    pub fn accepts_input(&self) -> bool {
        self.open && self.kind == TalkKind::Line && self.input_on && !self.leaving
    }

    pub fn take_events(&mut self) -> Vec<TalkEvent> {
        std::mem::take(&mut self.events)
    }

    /// The words of the field wrapped to `max` stage pixels, their width and height.
    fn measure(&self, text: &str, max: f32) -> (String, f32, f32) {
        let Some(def) = self.field_def.clone() else { return (text.to_string(), 200.0, 40.0) };
        let Some(font) = self.fonts.get(def.font_class.as_deref()).cloned() else { return (text.to_string(), 200.0, 40.0) };
        let wrapped = crate::render::wrap_text(&self.fonts, &font, text, def.size, max);
        let width = wrapped.split('\n').map(|l| crate::render::markup_width(&self.fonts, &font, l, def.size)).fold(0.0, f32::max);
        let sc = def.size / font.units_per_em;
        let lines = wrapped.split('\n').count().max(1) as f32;
        let height = lines * ((font.ascent + font.descent + font.leading) * sc + def.leading) - def.leading + 4.0;
        (wrapped, width, height)
    }

    fn show(&mut self, kind: TalkKind, text: &str, anchor: (f32, f32), sounds: &mut Vec<UiSound>) {
        let max = if kind == TalkKind::Line { TALK_TEXT_MAX } else { WINDOW_MAX.0 - self.text_pos.0 * 2.0 };
        let (wrapped, width, height) = self.measure(text, max);
        self.layout = text_layout(width + 5.0, height, self.text_pos, self.default_window);
        self.centre = if kind == TalkKind::Line { (0.0, 0.0) } else { (((self.layout.window.0 - 2.0 - self.text_pos.0 * 2.0 - width) * 0.5).max(0.0), ((self.layout.window.1 - 2.0 - self.text_pos.1 * 2.0 - height) * 0.5).max(0.0)) };
        self.total = visible_len(&wrapped);
        self.full = wrapped;
        self.shown = if kind == TalkKind::Line { 0.0 } else { self.total as f32 };
        self.kind = kind;
        self.anchor = anchor;
        // the field as wide / high as the layout says (`textField.width / height`)
        if let (Some(def), Some(node)) = (self.field_def.clone(), self.dialog.node_mut(FIELD)) {
            if let Kind::Text(t) = &mut node.kind {
                let mut d: TextDef = (*def).clone();
                d.bounds.x1 = d.bounds.x0 + self.layout.field.0;
                d.bounds.y1 = d.bounds.y0 + self.layout.field.1;
                t.def = Arc::new(d);
            }
        }
        let (sx, sy) = (self.layout.window.0 / self.default_window.0, self.layout.window.1 / self.default_window.1);
        let base = self.win_matrix;
        if let Some(node) = self.dialog.node_mut(WIN) {
            node.matrix = Matrix::new(base.a * sx, base.b * sx, base.c * sy, base.d * sy, base.tx, base.ty);
            node.script_matrix = true;
        }
        self.dialog.set_position(PO_ALL, self.layout.holder.0, self.layout.holder.1, 0.0);
        if !self.open || self.leaving {
            sounds.push(UiSound::Play(SE_MENU_OPEN));
            self.dialog.goto_and_play("dialog", "in");
            self.input_on = false;
        }
        self.open = true;
        self.leaving = false;
        self.banner_left = 0.0;
        self.place();
        self.draw_text();
    }

    /// A speaker's line over `anchor` (stage pixels). `speaker` goes on its own first line in the strong colour.
    pub fn say(&mut self, speaker: Option<&str>, text: &str, anchor: (f32, f32), sounds: &mut Vec<UiSound>) {
        let full = match speaker.filter(|s| !s.is_empty()) {
            Some(name) => format!("<color(name)>{name}<color>\n{text}"),
            None => text.to_string(),
        };
        self.show(TalkKind::Line, &full, anchor, sounds);
    }

    /// A prompt over `anchor` (no input of its own; the host closes it).
    pub fn prompt(&mut self, text: &str, anchor: (f32, f32), sounds: &mut Vec<UiSound>) {
        if self.open && self.kind == TalkKind::Prompt && !self.leaving && self.full == text {
            self.anchor = anchor;
            return;
        }
        self.show(TalkKind::Prompt, text, anchor, sounds);
    }

    /// A message at the top centre for `seconds`, then it leaves by itself.
    pub fn banner(&mut self, text: &str, seconds: f32, sounds: &mut Vec<UiSound>) {
        self.show(TalkKind::Banner, text, (960.0, 260.0), sounds);
        self.banner_left = seconds.max(0.5);
    }

    /// The speaker moved (stage pixels).
    pub fn set_anchor(&mut self, anchor: (f32, f32)) {
        self.anchor = anchor;
    }

    /// The window leaves (its `out` animation, then `Closed`).
    pub fn close(&mut self, sounds: &mut Vec<UiSound>) {
        if self.open && !self.leaving {
            if self.kind == TalkKind::Line {
                sounds.push(UiSound::Play(SE_MENU_CLOSE));
            }
            self.dialog.goto_and_play("dialog", "out");
            self.leaving = true;
        }
    }

    /// Off at once (no animation, no event).
    pub fn clear(&mut self) {
        self.open = false;
        self.leaving = false;
        self.dialog.goto_and_play("dialog", "off");
        self.dialog.set_visible(TEXT, false);
        self.shown_text.clear();
    }

    /// A menu key: decide shows the rest of a line, or goes on when it is all shown.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        if repeat || key != CampKey::Decide || !self.accepts_input() {
            return;
        }
        if (self.shown as usize) < self.total {
            self.shown = self.total as f32;
        } else {
            sounds.push(UiSound::Play(SE_MENU_DECIDE));
            self.events.push(TalkEvent::Advanced);
        }
        self.draw_text();
    }

    /// The window's place: over the anchor (a banner: centred on it), inside the stage.
    fn place(&mut self) {
        let (w, h) = self.layout.window;
        let (x, y) = match self.kind {
            TalkKind::Banner => self.anchor,
            _ => (self.anchor.0, self.anchor.1 - ABOVE - h * 0.5),
        };
        let x = x.clamp(w * 0.5 + MARGIN, (1920.0 - w * 0.5 - MARGIN).max(w * 0.5 + MARGIN));
        let y = y.clamp(h * 0.5 + MARGIN, (1080.0 - h * 0.5 - MARGIN).max(h * 0.5 + MARGIN));
        self.dialog.set_position("dialog", x, y, 0.0);
    }

    fn draw_text(&mut self) {
        let mut text = reveal(&self.full, self.shown as usize);
        if self.kind == TalkKind::Line && self.shown as usize >= self.total && self.input_on {
            text.push_str(" <icon(Cross)>");
        }
        if self.shown_text != text {
            let fonts = self.fonts.clone();
            self.dialog.set_text(FIELD, &text, &fonts);
            self.shown_text = text;
        }
    }

    /// The text follows the text holder of the animated window (it is a child of it in the game: `Dialog.<init>`).
    fn follow_holder(&mut self) {
        let mut m = Matrix::IDENTITY;
        let mut at = String::new();
        let mut alpha = 1.0;
        let mut shown = true;
        for part in format!("{PO_ALL}.poDialogText").split('.') {
            at = if at.is_empty() { part.to_string() } else { format!("{at}.{part}") };
            let Some(node) = self.dialog.node(&at) else { return };
            m = m.mul(&node.matrix);
            alpha *= node.cx.mul[3];
            shown &= node.is_visible();
        }
        let open = self.open;
        let m = m.mul(&Matrix::new(1.0, 0.0, 0.0, 1.0, self.centre.0, self.centre.1));
        if let Some(node) = self.dialog.node_mut(TEXT) {
            node.matrix = m;
            node.script_matrix = true;
            node.cx.mul[3] = alpha;
            node.script_visible = Some(shown && open);
        }
    }

    /// One stage frame (60 a second).
    pub fn tick(&mut self) {
        if !self.open {
            return;
        }
        if self.kind == TalkKind::Line && self.input_on && (self.shown as usize) < self.total {
            self.shown = (self.shown + REVEAL_PER_S / 60.0).min(self.total as f32);
        }
        if self.kind == TalkKind::Banner && !self.leaving && self.input_on {
            self.banner_left -= 1.0 / 60.0;
            if self.banner_left <= 0.0 {
                let mut none = Vec::new();
                self.close(&mut none);
            }
        }
        self.place();
        self.dialog.tick();
        for event in self.dialog.take_events() {
            match event.name.as_str() {
                "ANIM_END_MES_WIN_IN" => {
                    self.input_on = true;
                    self.events.push(TalkEvent::Opened);
                }
                "ANIM_END_MES_WIN_OUT" => {
                    self.open = false;
                    self.leaving = false;
                    self.dialog.set_visible(TEXT, false);
                    self.shown_text.clear();
                    self.events.push(TalkEvent::Closed);
                    return;
                }
                _ => {}
            }
        }
        self.follow_holder();
        self.draw_text();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_form_layout() {
        // The authored holders of the Dialog movie: text (34, 40); window 510 x 186.
        let short = text_layout(100.0, 30.0, (34.0, 40.0), (510.0, 186.0));
        // width = max(256, 68 + 100); height = max(128, 40 + 35 + 40)
        assert_eq!(short.window, (258.0, 130.0));
        assert_eq!(short.field, (256.0 - 68.0 + 5.0, 35.0));
        let long = text_layout(700.0, 120.0, (34.0, 40.0), (510.0, 186.0));
        assert_eq!(long.window, (770.0, 207.0));
        assert_eq!(long.holder, ((510.0 - 768.0) * 0.5, (186.0 - 205.0) * 0.5));
    }

    #[test]
    fn reveal_keeps_tags_whole() {
        let t = "<color(name)>Donald<color>\nAw, phooey!";
        assert_eq!(visible_len(t), "Donald\nAw, phooey!".chars().count());
        assert_eq!(reveal(t, 3), "<color(name)>Don<color>");
        assert_eq!(reveal(t, 6), "<color(name)>Donald<color>");
        assert_eq!(reveal(t, 9), "<color(name)>Donald<color>\nAw");
        assert_eq!(reveal(t, 100), t);
        assert_eq!(reveal("<icon(Triangle)> Talk", 2), "<icon(Triangle)> T");
    }
}
