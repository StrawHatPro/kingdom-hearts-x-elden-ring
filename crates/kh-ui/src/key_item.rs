//! (HUD2) KH3'S MAJOR ITEM POPUP, "OBTAINED": the movie `KeyItem` (`UI/Scaleform/20_keyitem/KeyItem`, AS3 class
//! `KeyItem`, build/sora_pack/ui/as3/KeyItem.as3.txt) and its native part `UTresUIP_KeyItem`.
//!
//! The user (2026-10-20): "make major item pop ups that match the photo exactly, replacing the map icon from the example
//! with the picture of what you've actually obtained ... It pauses the game and you must press A to close the popup ...
//! all of the appropriate SFX and VFX for this straight from KH3" (reference `party/ui2/ref/kh3_obtained_popup.png`).
//!
//! KH3's own widget, nothing composed:
//! - the clip `getItemL` (`McGetItemL_1`, 144 frames: `off` 1, `in` 7 .. 123 (stop + `ANIM_END_IN_LARGE_ITEM`),
//!   `loop` 61, `out` 125 .. 143 (`ANIM_END_OUT_LARGE_ITEM`, back to `off`)): the "OBTAINED" header plate (`txGetL`,
//!   the movie's own text), the item name (`txItemNameL.coItemNameL`, a `TextFieldAdjust`), the picture panel
//!   (`itemIconLAnm.itemIconL`, a `tres.controls.Icon`, with its loading ring `itemLoad`), the description box
//!   (`txItemCaption.coItemCaption`, a `TextFieldAlign`, centred);
//! - ITS VFX: the movie's own effect clips, started by `getItemL`'s frame 49 script: `getItemEffL` (`GetItemEffL_19`,
//!   43 frames, bitmaps `KeyItemEff_flash1/2`, `KeyItemEff_glw1`) and the light `getItemLightLAnm`;
//! - the AS3 interface: `openLargeItemGetByPath(icon, name, caption)`, `closeLargeItemGet()`, `forceClose()`, the
//!   two callbacks (in done / out done), ported method by method below;
//! - THE NATIVE PART [I, decomp]: open `0x14166F3B0` plays **static SE 10** (`0x14176F6B0(part, 10)`: se00000_011)
//!   and calls `openLargeItemGetByPath` with the item row's icon path (`TresItemData` +0x40), name (+0x50) and caption
//!   (+0x60); its input handler `0x141676400` (`UTresUIP_KeyItem`) takes the decide / cancel events (5 / 6) once the
//!   `in` callback has set its ready byte (+0xA1), plays **static SE 1** (`Decide`: se00000_002) and calls
//!   `closeLargeItemGet`. THE PORT closes on DECIDE ONLY (A on the pad, the confirm key on the keyboard): the user
//!   asked for "you must press A to close" (KH3 also closes on cancel).
//!
//! What is the host's: when it fires, the pause (the host holds its world while [`KeyItemPopup::is_open`]), the
//! picture handed in (`tres.controls.Icon` draws any texture at the clip's size, as the item pane's icon does).

use std::collections::VecDeque;
use std::sync::Arc;

use crate::def::{MovieDef, Texture};
use crate::player::Movie;
use crate::render::Fonts;
use crate::UiSound;

/// The movie's name in the pack.
pub const MOVIE: &str = "KeyItem";
/// `0x14166F3B0`: the open's static SE (se00000_011).
pub const SE_OPEN: u32 = 10;
/// `0x141676400`: the close on decide, static SE 1 (`TresUISystemSEDataSet.Decide`, se00000_002).
pub const SE_CLOSE: u32 = 1;

const GET_L: &str = "getItemL";
const ICON: &str = "getItemL.itemIconLAnm.itemIconL";
const LOAD: &str = "getItemL.itemLoad";
const NAME: &str = "getItemL.txItemNameL.coItemNameL";
const CAPTION: &str = "getItemL.txItemCaption.coItemCaption";

/// `KeyItem.ANIM_END_IN_LARGE_ITEM` / `ANIM_END_OUT_LARGE_ITEM`.
const ANIM_END_IN: i32 = 0;
const ANIM_END_OUT: i32 = 1;

/// Where the popup stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Nothing up.
    Closed,
    /// `getItemL` plays `in` (no input yet: the native ready byte is set by the `in` callback).
    In,
    /// Up and waiting for A.
    Wait,
    /// `getItemL` plays `out`.
    Out,
}

/// What one popup shows.
#[derive(Clone, Debug, Default)]
pub struct Obtained {
    /// The picture in the centre panel (None: the panel's loading ring stays: KH3's look of an icon not loaded).
    pub picture: Option<Arc<Texture>>,
    /// The name under the "OBTAINED" header.
    pub name: String,
    /// The description box (KH3's markup: `<br>`, `<color(..)>`).
    pub caption: String,
}

/// The movie with its AS3 document class and the native part's state.
pub struct KeyItemPopup {
    pub movie: Movie,
    fonts: Fonts,
    stage: Stage,
    /// Popups asked for while one is up: shown one after the other.
    queue: VecDeque<Obtained>,
    /// What is up now (for the host's log and tests).
    pub current: Option<Obtained>,
    /// Popups shown since the start (tests).
    pub shown: u32,
    /// The caption field's authored place in its `TextFieldAlign` clip (taken at the first text).
    caption_at: Option<(f32, f32, Arc<crate::def::TextDef>)>,
    /// (HUD2 round 2) The caption box sized to its text: see [`CaptionLayout`].
    layout: CaptionLayout,
    /// The caption plate's authored y in `getItemL` (taken at the first open).
    plate_ty: Option<f32>,
    /// `getItemL`'s authored y on the stage.
    layout_base: Option<f32>,
    /// The full-stage dim's authored y in `getItemL`.
    dim_ty: Option<f32>,
}

impl KeyItemPopup {
    pub fn new(def: Arc<MovieDef>, fonts: Fonts) -> KeyItemPopup {
        let mut movie = Movie::new(def);
        // the document class at rest: the root at `off` (frame 3 stops it), getItemL at `off`
        movie.goto_and_play("", "off");
        movie.goto_and_play(GET_L, "off");
        KeyItemPopup { movie, fonts, stage: Stage::Closed, queue: VecDeque::new(), current: None, shown: 0, caption_at: None, layout: CaptionLayout::default(), plate_ty: None, layout_base: None, dim_ty: None }
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// Up (or still leaving): the host holds its world and gives the pad to the popup.
    pub fn is_open(&self) -> bool {
        self.stage != Stage::Closed || !self.queue.is_empty()
    }

    /// Popups waiting behind the one up.
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// Asks for a popup: shown at once when none is up, else after the ones before it.
    pub fn show(&mut self, what: Obtained, sounds: &mut Vec<UiSound>) {
        if self.stage == Stage::Closed && self.queue.is_empty() {
            self.open(what, sounds);
        } else {
            self.queue.push_back(what);
        }
    }

    /// `0x14166F3B0` + `openLargeItemGetByPath(icon, name, caption)`.
    fn open(&mut self, what: Obtained, sounds: &mut Vec<UiSound>) {
        sounds.push(UiSound::Play(SE_OPEN));
        // m_ItemIconL.load(a1); m_ItemLoad.gotoAndPlay("loop")
        let loaded = match self.movie.sprite_mut(ICON) {
            Some(s) => crate::controls::icon_load(s, what.picture.clone()) && what.picture.is_some(),
            None => false,
        };
        self.movie.goto_and_play(LOAD, "loop");
        // gotoAndPlay("loop"); the texts; getItemL.gotoAndPlay("in")
        self.movie.goto_and_play("", "loop");
        self.movie.set_text(NAME, &what.name.replace("<br>", " "), &self.fonts);
        self.layout = text_align_set(&mut self.movie, CAPTION, &what.caption, &self.fonts, &mut self.caption_at);
        self.movie.goto_and_play(GET_L, "in");
        if loaded {
            // onIconLoadComplete: the picture is there at once here (a texture of the pack, not a streamed asset)
            self.movie.goto_and_play(LOAD, "off");
        }
        self.apply_layout();
        self.stage = Stage::In;
        self.current = Some(what);
        self.shown += 1;
    }

    /// The decide event (A / the confirm key). True when it closed the popup (the native handler: only once the `in`
    /// callback set the ready byte; static SE 1, `closeLargeItemGet`).
    pub fn decide(&mut self, sounds: &mut Vec<UiSound>) -> bool {
        if self.stage != Stage::Wait {
            return false;
        }
        sounds.push(UiSound::Play(SE_CLOSE));
        // closeLargeItemGet(): gotoAndPlay("loop"); getItemL.gotoAndPlay("out")
        self.movie.goto_and_play("", "loop");
        self.movie.goto_and_play(GET_L, "out");
        self.stage = Stage::Out;
        true
    }

    /// `forceClose()`: everything off at once (a load, the host going away).
    pub fn force_close(&mut self) {
        self.movie.goto_and_play("", "off");
        self.movie.goto_and_play(GET_L, "off");
        if let Some(s) = self.movie.sprite_mut(ICON) {
            crate::controls::icon_load(s, None);
        }
        self.movie.goto_and_play(LOAD, "off");
        self.stage = Stage::Closed;
        self.queue.clear();
        self.current = None;
    }

    /// (HUD2 round 2) The caption's layout of the popup up now (tests, the host's log).
    pub fn caption_layout(&self) -> CaptionLayout {
        self.layout
    }

    /// (HUD2 round 2, the user: "make the box that holds the description dynamically sized so it always fits") THE CAPTION
    /// PLATE GROWS WITH ITS TEXT: the plate (`getItemL`'s depth-3 clip, KH3's 680 x 130 box art) is stretched in y to
    /// [`CaptionLayout::box_h`] from its authored top (its own x tween runs on: only its y scale and y are set, every
    /// frame, after the timeline), and the whole `getItemL` goes up by half of what was added, so the popup stays
    /// centred on the stage (the header plate, the picture panel and the box keep KH3's spacing).
    fn apply_layout(&mut self) {
        let k = self.layout.box_h / PLATE_H;
        let lift = (self.layout.box_h - PLATE_H) * 0.5;
        if let Some(get) = self.movie.sprite_mut(GET_L) {
            if let Some(plate) = get.children.iter_mut().find(|n| n.depth == PLATE_DEPTH) {
                let ty = *self.plate_ty.get_or_insert(plate.matrix.ty);
                plate.matrix.c = 0.0;
                plate.matrix.d = k;
                plate.matrix.ty = ty;
            }
            // the full-stage dim behind it (depth 1, 1920 x 1080) stays on the stage: it goes down by the lift
            if let Some(dim) = get.children.iter_mut().find(|n| n.depth == DIM_DEPTH) {
                let ty = *self.dim_ty.get_or_insert(dim.matrix.ty);
                dim.matrix.ty = ty + lift;
            }
        }
        if let Some(node) = self.movie.node_mut(GET_L) {
            let y = *self.layout_base.get_or_insert(node.matrix.ty);
            node.matrix.ty = y - lift;
            node.script_matrix = true;
        }
    }

    /// One stage frame: the timelines, then `onAnimEnd` of the two callbacks; the next queued popup opens when the
    /// last one has left.
    pub fn tick(&mut self, sounds: &mut Vec<UiSound>) {
        if self.stage == Stage::Closed && self.queue.is_empty() {
            return;
        }
        self.movie.tick();
        self.apply_layout(); // (HUD2) the timeline rewrites the plate's matrix in its tweens
        for event in self.movie.take_events() {
            match event.value {
                ANIM_END_IN if self.stage == Stage::In && event.name == "ANIM_END_IN_LARGE_ITEM" => self.stage = Stage::Wait,
                ANIM_END_OUT if event.name == "ANIM_END_OUT_LARGE_ITEM" => {
                    // m_ItemIconL.unload(); m_ItemLoad.gotoAndPlay("off"); the native part's out callback hides it
                    if let Some(s) = self.movie.sprite_mut(ICON) {
                        crate::controls::icon_load(s, None);
                    }
                    self.movie.goto_and_play(LOAD, "off");
                    self.movie.goto_and_play("", "off");
                    self.stage = Stage::Closed;
                    self.current = None;
                }
                _ => {}
            }
        }
        if self.stage == Stage::Closed {
            if let Some(next) = self.queue.pop_front() {
                self.open(next, sounds);
            }
        }
    }
}

/// KH3's caption plate: `getItemL` depth 3, 680 x 130 at y 212 (in `getItemL`); the caption field (`textField` of
/// `coItemCaption`) 95 high, its top 19 under the plate's, 16 above its bottom.
const PLATE_DEPTH: i32 = 3;
/// `getItemL`'s full-stage dim (a 1920 x 1080 shape centred on it).
const DIM_DEPTH: i32 = 1;
const PLATE_H: f32 = 130.0;
const FIELD_TOP: f32 = 19.0;
const FIELD_BOTTOM: f32 = 16.0;
/// The tallest the plate grows (478): the plate's bottom (540 + 212 + H, lifted by (H - 130) / 2) then stands 24 px above
/// the 1080 stage's bottom (the header plate's top is then 74 px under its top). Beyond, the text steps down in size.
pub const PLATE_MAX_H: f32 = 2.0 * (1080.0 - 24.0 - 540.0 - 212.0 - 65.0);
/// The smallest text scale a step-down goes to.
pub const MIN_TEXT_SCALE: f32 = 0.6;

/// (HUD2 round 2) How the caption is laid out (stage pixels, `getItemL` coordinates before the lift).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptionLayout {
    pub lines: usize,
    /// The text block's height (as drawn) and the scale it is drawn at (1 = KH3's size).
    pub text_h: f32,
    pub scale: f32,
    /// The plate's height (KH3's 130 when the text fits the authored field).
    pub box_h: f32,
    /// The text's top and bottom measured from the plate's top.
    pub text_top: f32,
    pub text_bottom: f32,
}

impl Default for CaptionLayout {
    fn default() -> Self {
        CaptionLayout { lines: 0, text_h: 0.0, scale: 1.0, box_h: PLATE_H, text_top: 0.0, text_bottom: 0.0 }
    }
}

/// `tres.controls.TextFieldAlign` (`CoItemCaption`: `alignH` / `alignV` "Center", set by the clip's component
/// inspector), `set text` -> `updateText()`: the field gets `System.getDisplayString(text)`, then is moved inside its
/// clip: x = (width - textWidth) / 2; y = (height - textHeight) / 2, or 0 when the text scrolls (more lines than fit).
/// `textWidth` / `textHeight` = the widest line and the lines' heights, as the renderer lays them out (`render.rs`
/// `draw_text`: lines of (ascent + descent + leading) x size / em, plus the field's leading when it wraps).
/// (HUD2 round 2) A text taller than the field no longer runs out of the box: the field keeps KH3's top place and the
/// plate is given the height it needs (KH3's margins above and below kept); past [`PLATE_MAX_H`] the text steps down
/// in size (the field scaled, never under [`MIN_TEXT_SCALE`]).
fn text_align_set(movie: &mut Movie, clip: &str, text: &str, fonts: &Fonts, at: &mut Option<(f32, f32, Arc<crate::def::TextDef>)>) -> CaptionLayout {
    // (the game's text markup `<br>` is a line break, as everywhere the pack's strings are shown)
    let shown = crate::controls::display_string(text).replace("<br>", "\n");
    let path = format!("{clip}.textField");
    let mut out = CaptionLayout::default();
    let Some(node) = movie.node_mut(&path) else { return out };
    let crate::player::Kind::Text(t) = &node.kind else { return out };
    // the field as authored (a step-down of the popup before widened its copy)
    let (x0, y0, def) = at.get_or_insert_with(|| (node.matrix.tx, node.matrix.ty, t.def.clone())).clone();
    let Some(font) = fonts.get(def.font_class.as_deref()).cloned() else { return out };
    let (w, h) = (def.bounds.x1 - def.bounds.x0, def.bounds.y1 - def.bounds.y0);
    let lead = if def.word_wrap { def.leading } else { 0.0 };
    let step = (font.ascent + font.descent + font.leading) * def.size / font.units_per_em + lead;
    // the lines at a text scale `s`: wrapped to the field's width on the stage ((w - 4) / s in the field's own units)
    let layout = |s: f32| -> (usize, f32, f32) {
        let text = if def.word_wrap { crate::render::wrap_text(fonts, &font, &shown, def.size, (w - 4.0) / s) } else { shown.clone() };
        let lines: Vec<&str> = text.split('\n').collect();
        let tw = lines.iter().map(|l| crate::render::markup_width(fonts, &font, l, def.size)).fold(0.0, f32::max);
        (lines.len(), tw, lines.len() as f32 * step - lead)
    };
    let (n, tw, th) = layout(1.0);
    let mut field = def.clone();
    if th <= h - 4.0 {
        // KH3's own case: centred in the field, the plate as authored
        let x = (w - tw) * 0.5;
        let y = (h - th) * 0.5 - 2.0;
        node.matrix = crate::geom::Matrix::new(1.0, 0.0, 0.0, 1.0, x0 + x - 2.0, y0 + y);
        out = CaptionLayout { lines: n, text_h: th, scale: 1.0, box_h: PLATE_H, text_top: FIELD_TOP + y + 2.0, text_bottom: 0.0 };
    } else {
        // taller than the field: from the field's top, the plate grown to hold it; past the plate's maximum the text steps
        // down in steps of 5 % (rewrapped to the same width on the stage) until it fits, never under MIN_TEXT_SCALE
        let room = PLATE_MAX_H - FIELD_TOP - FIELD_BOTTOM - 4.0;
        let (mut s, mut fit) = (1.0f32, (n, tw, th));
        while fit.2 * s > room && s > MIN_TEXT_SCALE + 1.0e-3 {
            s = (s - 0.05).max(MIN_TEXT_SCALE);
            fit = layout(s);
        }
        let (n, tw, th) = fit;
        // the field's copy is as wide in its own units as the stage width it must cover
        let mut wide = (*def).clone();
        wide.bounds.x1 = wide.bounds.x0 + w / s;
        field = Arc::new(wide);
        let x = (w - tw * s) * 0.5;
        node.matrix = crate::geom::Matrix::new(s, 0.0, 0.0, s, x0 + x - 2.0 * s, y0);
        out = CaptionLayout { lines: n, text_h: th * s, scale: s, box_h: (FIELD_TOP + 2.0 + th * s + 2.0 + FIELD_BOTTOM).max(PLATE_H), text_top: FIELD_TOP + 2.0, text_bottom: 0.0 };
    }
    if let crate::player::Kind::Text(t) = &mut node.kind {
        t.def = field;
        t.text = Some(shown);
    }
    out.text_bottom = out.text_top + out.text_h;
    out
}
