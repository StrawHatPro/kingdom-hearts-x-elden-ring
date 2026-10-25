//! (CV1) THE KH SPEECH BUBBLE of the conversation system: KH2's rounded speech bubble with a tail that tracks the speaker
//! (the user's reference `build/sora_tmp/er/conv/ref/ref_kh2_bubble_shape.png`), in the colour of the S8 talk window it
//! replaces (KH3's Dialog window in its DH colour: MEASURED in `tests/cv1_bubble.rs` `measure_the_old_window`), the
//! speaker's name in the markup's strong yellow on the first line, KH3's own Keyblade icon (the `MessageTex01` sheet of
//! the Message movie, its unused top-right cell) bobbing at the BOTTOM MIDDLE when a line waits for A, choices as KH
//! options with KH3's own pointing-glove cursor (`DialogYubi` of the Dialog movie) and the menu sounds.
//!
//! WHAT IS THE GAME'S [A]: the window colour (its gradient and rim, measured off KH3's Dialog movie as the S8 window drew
//! it), the text field (font, size, leading, DH colour) of `Dialog`'s `dialogText.textField`, the Keyblade and glove art,
//! the strong (name) colour of the markup, the menu sounds (open / cursor / decide / close).
//! WHAT IS COMPOSED HERE [L]: the shape (KH2's: a rounded rectangle and a curved tail, drawn as one outline), its size
//! from its content (the user: "the speech bubble should shrink or expand to accommodate the text": width from the
//! longest wrapped line between MIN_W and MAX_W, wrapped beyond, height from the lines; eased between lines), its place
//! (near the speaker, never over a face of the conversation, inside the stage), the tail (on the edge facing the speaker,
//! its tip at the speaker's head every frame, its base clamped to the straight part of the edge), the pop in / out, the
//! typewriter, the icon's bob, the cursor's nudge.
//!
//! THE MOVIE IS MADE HERE: a stage-sized movie with no timeline of its own; the bubble's pieces are script children of
//! its root (shapes rebuilt when the geometry changes, character ids from the geometry so the hosts' dirty rectangles
//! see the change). Hosts draw `movie` like any HUD movie (kh_host `Hud::movie("SpeechBubble")`).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::camp::CampKey;
use crate::def::{BitmapFill, Edge, Fill, ImageChar, MovieDef, Paint, Rect0, Shape, SpriteDef, Stroke, TextDef, Texture};
use crate::geom::{Cx, Matrix, Rect};
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_OPEN};
use crate::player::{Kind, Node, TextNode};
use crate::{Fonts, Movie, Pack, PackError, UiSound};

// ------------------------------------------------------------------------------------------------- the look

/// [A] THE COLOUR KEPT FROM TODAY'S WINDOW (straight RGBA 0..255), measured off the S8 window over black and white:
/// its top row and bottom row (a vertical gradient), alpha 0.99, and its light rim (top edge, alpha 0.83).
pub const FILL_TOP: [f32; 4] = [37.3, 37.3, 52.4, 252.0];
pub const FILL_BOTTOM: [f32; 4] = [68.5, 68.5, 83.7, 252.0];
pub const RIM: [f32; 4] = [112.0, 112.0, 124.0, 212.0];
/// [L] KH2's bubble sits on a soft shadow.
pub const SHADOW: [f32; 4] = [0.0, 0.0, 0.0, 84.0];
pub const SHADOW_OFFSET: (f32, f32) = (0.0, 6.0);
/// [A] The body text: the DH colour of the Dialog field (newgame.rs). A chosen option is drawn brighter [L].
pub const TEXT: [f32; 4] = crate::newgame::DH_TEXT_COLOR;
pub const TEXT_SELECTED: [f32; 4] = [248.0, 248.0, 255.0, 255.0];
pub const TEXT_DIM: [f32; 4] = [128.0, 138.0, 176.0, 255.0];

/// [L] Sizes (stage pixels, 1920 x 1080).
pub const MIN_W: f32 = 140.0;
pub const MAX_W: f32 = 860.0;
pub const PAD_X: f32 = 32.0;
pub const PAD_TOP: f32 = 22.0;
/// The bottom padding of a line: the Keyblade's row (it never covers text).
pub const PAD_ICON: f32 = 54.0;
/// ... of a choice (the cursor is the cue: no icon).
pub const PAD_BOTTOM: f32 = 22.0;
pub const RADIUS: f32 = 26.0;
pub const RIM_WIDTH: f32 = 2.0;
/// The options' indent (the glove sits in it) and the gap between the question and the options.
pub const OPTION_INDENT: f32 = 50.0;
pub const OPTION_GAP: f32 = 8.0;
/// The tail: its base on the edge, its length between these, its tip this far short of the speaker's head.
pub const TAIL_BASE: f32 = 48.0;
pub const TAIL_MIN: f32 = 26.0;
pub const TAIL_MAX: f32 = 120.0;
pub const TAIL_GAP: f32 = 6.0;
/// The bubble's edge this far above (below) the speaker's face circle.
pub const CLEAR_OF_FACE: f32 = 44.0;
/// The stage area the bubble stays in.
pub const SAFE: Rect = Rect { x0: 40.0, y0: 36.0, x1: 1880.0, y1: 1044.0 };
/// [L] Characters revealed per second (the S8 window's).
pub const REVEAL_PER_S: f32 = crate::talk::REVEAL_PER_S;
/// [L] The Keyblade icon's size (KH3's cell is 32 x 64: the key hangs point down, as KH2's), bob (stage px, Hz); the
/// glove's size and nudge.
pub const ICON_SIZE: (f32, f32) = (26.0, 39.0);
/// The key's part of its 32 x 64 cell (x, y, w, h of the texture: the cell's blank margins left out).
pub const ICON_SRC: (f32, f32, f32, f32) = (34.0, 4.0, 28.0, 42.0);
pub const ICON_BOB: (f32, f32) = (3.0, 1.6);
pub const GLOVE_SIZE: (f32, f32) = (44.0, 22.0);
pub const GLOVE_NUDGE: (f32, f32) = (4.0, 2.2);
/// [L] The pop in / out (seconds) and the size / place eases (1/s).
pub const POP_IN: f32 = 0.14;
pub const POP_OUT: f32 = 0.10;
pub const SIZE_EASE: f32 = 20.0;
pub const PLACE_EASE: f32 = 12.0;

// ------------------------------------------------------------------------------------------------- the geometry (pure)

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TailSide {
    Top,
    Bottom,
    /// No speaker on screen: no tail.
    None,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tail {
    pub side: TailSide,
    /// The base's centre on the edge (x), and the tip.
    pub base_x: f32,
    pub tip: (f32, f32),
}

impl Tail {
    pub const NONE: Tail = Tail { side: TailSide::None, base_x: 0.0, tip: (0.0, 0.0) };
}

/// A speaker's face on the stage: centre and radius (px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

impl Face {
    fn rect(&self) -> Rect {
        Rect::new(self.x - self.r, self.y - self.r, self.x + self.r, self.y + self.r)
    }
}

fn overlap(a: &Rect, b: &Rect) -> f32 {
    let i = a.intersect(b);
    if i.is_empty() {
        0.0
    } else {
        (i.x1 - i.x0) * (i.y1 - i.y0)
    }
}

/// THE TAIL of a bubble `rect` toward the speaker's face (None: no tail): on the edge that faces him, its base under him
/// as far as the straight part of the edge allows, its tip at his face (short of it by the face's radius and a gap),
/// its length clamped. Pure.
pub fn tail_for(rect: &Rect, face: Option<Face>) -> Tail {
    let Some(f) = face else { return Tail::NONE };
    let cy = (rect.y0 + rect.y1) * 0.5;
    let side = if f.y >= rect.y1 - 2.0 {
        TailSide::Bottom
    } else if f.y <= rect.y0 + 2.0 {
        TailSide::Top
    } else if f.y > cy {
        TailSide::Bottom
    } else {
        TailSide::Top
    };
    let r = RADIUS.min((rect.y1 - rect.y0) * 0.5).min((rect.x1 - rect.x0) * 0.5);
    let lo = rect.x0 + r + TAIL_BASE * 0.5 + 2.0;
    let hi = rect.x1 - r - TAIL_BASE * 0.5 - 2.0;
    let base_x = if lo <= hi { f.x.clamp(lo, hi) } else { (rect.x0 + rect.x1) * 0.5 };
    let edge_y = if side == TailSide::Bottom { rect.y1 } else { rect.y0 };
    let sign = if side == TailSide::Bottom { 1.0 } else { -1.0 };
    let (dx, dy) = (f.x - base_x, f.y - edge_y);
    let dist = dx.hypot(dy);
    // the tip leaves the edge outward: at least 0.45 of its length goes away from the bubble
    let mut dir = if dist > 1e-3 { (dx / dist, dy / dist) } else { (0.0, sign) };
    if dir.1 * sign < 0.45 {
        let along = dir.0.signum() * (1.0f32 - 0.45 * 0.45).sqrt();
        dir = (if dir.0.abs() < 1e-3 { 0.0 } else { along }, 0.45 * sign);
        let n = dir.0.hypot(dir.1);
        dir = (dir.0 / n, dir.1 / n);
    }
    let len = (dist - f.r - TAIL_GAP).clamp(TAIL_MIN, TAIL_MAX);
    Tail { side, base_x, tip: (base_x + dir.0 * len, edge_y + dir.1 * len) }
}

/// A circular arc as quadratic curves (`segs` of them), from angle `a0` to `a1` (radians, y down).
fn arc(c: (f32, f32), r: f32, a0: f32, a1: f32, segs: u32, out: &mut Vec<Edge>) {
    let step = (a1 - a0) / segs as f32;
    for k in 0..segs {
        let (a, b) = (a0 + step * k as f32, a0 + step * (k + 1) as f32);
        let m = (a + b) * 0.5;
        let rc = r / (step * 0.5).cos();
        out.push(Edge::Curve([c.0 + r * a.cos(), c.1 + r * a.sin(), c.0 + rc * m.cos(), c.1 + rc * m.sin(), c.0 + r * b.cos(), c.1 + r * b.sin()]));
    }
}

/// THE OUTLINE (one closed contour, stage px): the rounded rectangle with the tail let into its top or bottom edge. The
/// tail's two sides are curves bowed toward its middle line (KH2's pointed, curved tail).
pub fn outline(rect: &Rect, tail: &Tail) -> Vec<Edge> {
    let (x0, y0, x1, y1) = (rect.x0, rect.y0, rect.x1, rect.y1);
    let r = RADIUS.min((y1 - y0) * 0.5).min((x1 - x0) * 0.5).max(0.0);
    use std::f32::consts::{FRAC_PI_2, PI};
    let mut e = Vec::with_capacity(24);
    let half = TAIL_BASE * 0.5;
    // the tail from base point `a` to the tip and back to base point `b` (the order of the walk)
    let tail_edges = |a: (f32, f32), b: (f32, f32), tip: (f32, f32), e: &mut Vec<Edge>| {
        let mid_x = (a.0 + b.0) * 0.5;
        let bow = |p: (f32, f32)| {
            // the control point: the side's middle, pulled toward the tail's middle line and toward the base
            let m = ((p.0 + tip.0) * 0.5, (p.1 + tip.1) * 0.5);
            let toward_mid = (mid_x + (tip.0 - mid_x) * 0.5 - m.0) * 0.35;
            (m.0 + toward_mid, m.1 + (p.1 - m.1) * 0.25)
        };
        let ca = bow(a);
        e.push(Edge::Curve([a.0, a.1, ca.0, ca.1, tip.0, tip.1]));
        let cb = bow(b);
        e.push(Edge::Curve([tip.0, tip.1, cb.0, cb.1, b.0, b.1]));
    };
    // top edge, left to right
    if tail.side == TailSide::Top {
        e.push(Edge::Line([x0 + r, y0, tail.base_x - half, y0]));
        tail_edges((tail.base_x - half, y0), (tail.base_x + half, y0), tail.tip, &mut e);
        e.push(Edge::Line([tail.base_x + half, y0, x1 - r, y0]));
    } else {
        e.push(Edge::Line([x0 + r, y0, x1 - r, y0]));
    }
    arc((x1 - r, y0 + r), r, -FRAC_PI_2, 0.0, 2, &mut e);
    e.push(Edge::Line([x1, y0 + r, x1, y1 - r]));
    arc((x1 - r, y1 - r), r, 0.0, FRAC_PI_2, 2, &mut e);
    // bottom edge, right to left
    if tail.side == TailSide::Bottom {
        e.push(Edge::Line([x1 - r, y1, tail.base_x + half, y1]));
        tail_edges((tail.base_x + half, y1), (tail.base_x - half, y1), tail.tip, &mut e);
        e.push(Edge::Line([tail.base_x - half, y1, x0 + r, y1]));
    } else {
        e.push(Edge::Line([x1 - r, y1, x0 + r, y1]));
    }
    arc((x0 + r, y1 - r), r, FRAC_PI_2, PI, 2, &mut e);
    e.push(Edge::Line([x0, y1 - r, x0, y0 + r]));
    arc((x0 + r, y0 + r), r, PI, PI + FRAC_PI_2, 2, &mut e);
    e
}

/// Where a bubble of `size` goes: near the speaker's face (above him by preference, then shifted sideways, then below),
/// never over a face of `faces`, inside `safe`; `keep` = the current place, kept while it is still good (no jumping).
/// No speaker on screen: the lower middle of the stage (KH's subtitle place). Pure.
pub fn place(size: (f32, f32), speaker: Option<Face>, faces: &[Face], safe: &Rect, keep: Option<Rect>) -> Rect {
    let (w, h) = size;
    let clamp = |cx: f32, top: f32| -> Rect {
        let x0 = (cx - w * 0.5).clamp(safe.x0, (safe.x1 - w).max(safe.x0));
        let y0 = top.clamp(safe.y0, (safe.y1 - h).max(safe.y0));
        Rect::new(x0, y0, x0 + w, y0 + h)
    };
    let Some(s) = speaker else {
        return clamp((safe.x0 + safe.x1) * 0.5, safe.y1 - h - 110.0);
    };
    let face_cost = |r: &Rect| -> f32 { faces.iter().chain(std::iter::once(&s)).map(|f| overlap(r, &f.rect())).sum::<f32>() };
    let tail_len = |r: &Rect| -> f32 {
        let t = tail_for(r, Some(s));
        let edge = if t.side == TailSide::Bottom { r.y1 } else { r.y0 };
        (s.x - t.base_x).hypot(s.y - edge) - s.r
    };
    if let Some(k) = keep {
        let same_size = ((k.x1 - k.x0) - w).abs() < 0.5 && ((k.y1 - k.y0) - h).abs() < 0.5;
        let inside = k.x0 >= safe.x0 - 0.5 && k.x1 <= safe.x1 + 0.5 && k.y0 >= safe.y0 - 0.5 && k.y1 <= safe.y1 + 0.5;
        let beside = s.y > k.y0 - s.r && s.y < k.y1 + s.r;
        if same_size && inside && !beside && face_cost(&k) < 1.0 && tail_len(&k) < TAIL_MAX * 1.6 {
            return k;
        }
    }
    let above = s.y - s.r - CLEAR_OF_FACE - h;
    let below = s.y + s.r + CLEAR_OF_FACE;
    let mut best: Option<(f32, Rect)> = None;
    for (top, side_cost) in [(above, 0.0), (below, 260.0)] {
        for (k, shift) in [0.0f32, 0.3, -0.3, 0.6, -0.6, 1.0, -1.0].iter().enumerate() {
            let want = Rect::new(s.x + shift * w - w * 0.5, top, s.x + shift * w + w * 0.5, top + h);
            let r = clamp(s.x + shift * w, top);
            let moved = (r.x0 - want.x0).abs() + (r.y0 - want.y0).abs();
            // beside his face (not above or below it) the tail cannot point at him well
            let beside = if s.y > r.y0 - s.r && s.y < r.y1 + s.r { 500.0 } else { 0.0 };
            let cost = face_cost(&r) * 4.0 + beside + side_cost + k as f32 * 6.0 + moved * 0.6 + (tail_len(&r) - TAIL_MAX).max(0.0) * 2.0;
            if best.is_none_or(|(c, _)| cost < c) {
                best = Some((cost, r));
            }
        }
    }
    best.map_or_else(|| clamp(s.x, above), |(_, r)| r)
}

// ------------------------------------------------------------------------------------------------- the widget

/// What the bubble shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BubbleMode {
    Line,
    Choice,
}

/// What happened in a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BubbleEvent {
    /// The pop-in finished: A counts from now.
    Opened,
    /// A line is all shown (the typewriter reached its end, or A showed the rest).
    Revealed,
    /// A on a line that was all shown: the host gives the next step.
    Advanced,
    /// The cursor moved to this option.
    Moved(usize),
    /// A on an option (or B on a choice that has a cancel option: that option).
    Chosen(usize),
    /// The pop-out finished (off the screen).
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Closed,
    Opening(f32),
    Open,
    Closing(f32),
}

/// One measured text: wrapped (markup kept), its width and height.
#[derive(Clone, Debug, Default, PartialEq)]
struct Measured {
    text: String,
    w: f32,
    h: f32,
}

pub struct SpeechBubble {
    pub movie: Movie,
    fonts: Fonts,
    field: Option<Arc<TextDef>>,
    keyblade: Option<Arc<ImageChar>>,
    glove: Option<Arc<ImageChar>>,
    phase: Phase,
    mode: BubbleMode,
    name: Option<Measured>,
    body: Measured,
    options: Vec<Measured>,
    cursor: usize,
    cancel: Option<usize>,
    total: usize,
    shown: f32,
    reveal_cps: f32,
    /// The size asked by the content and the size drawn (eased).
    target_size: (f32, f32),
    size: (f32, f32),
    /// The place asked and the place drawn (top-left; eased).
    target_rect: Option<Rect>,
    at: (f32, f32),
    speaker: Option<Face>,
    faces: Vec<Face>,
    safe: Rect,
    /// A new line / choice was given: the place is chosen afresh (no easing from the old one).
    fresh: bool,
    clock: f32,
    events: Vec<BubbleEvent>,
    /// The host's override of the Keyblade cue (None: shown when a line is all shown).
    cue: Option<bool>,
    /// (C.5) An ambient line: no key counts (A, the cursor), no Keyblade cue.
    passive: bool,
    /// (testing) the drawn rectangle, the tail, the text shown.
    pub drawn: Option<(Rect, Tail)>,
    pub shown_text: String,
}

const CH_SHADOW: u32 = 0x2000_0000;
const CH_BODY: u32 = 0x2100_0000;
const CH_TEXT: u32 = 0x2200_0000;
const CH_KEY: u32 = 0x2300_0001;
const CH_GLOVE: u32 = 0x2300_0002;

fn geometry_id(base: u32, rect: &Rect, tail: &Tail) -> u32 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for v in [rect.x0, rect.y0, rect.x1, rect.y1, tail.base_x, tail.tip.0, tail.tip.1] {
        ((v * 4.0).round() as i32).hash(&mut h);
    }
    (tail.side as u8).hash(&mut h);
    base | (h.finish() as u32 & 0x00FF_FFFF)
}

fn empty_movie() -> Arc<MovieDef> {
    let root = SpriteDef { id: 0, frames: 1, labels: Vec::new(), ops: vec![Vec::new()], scripts: vec![Vec::new()], ..SpriteDef::default() };
    Arc::new(MovieDef { name: "SpeechBubble".into(), stage: (1920.0, 1080.0), fps: 60.0, chars: HashMap::new(), root: Arc::new(root), symbols: HashMap::new(), bitmaps: HashMap::new(), consts: HashMap::new(), unknown_statements: Vec::new() })
}

fn node(ch: u32, name: &str, kind: Kind) -> Node {
    Node { ch, depth: 0, born: 1, z: None, name: Some(Arc::from(name)), matrix: Matrix::IDENTITY, cx: Cx::IDENTITY, clip: 0, blend: 0, visible: true, script_visible: None, script_matrix: true, ratio: 0.0, kind }
}

/// A cell of a texture as an image of `size` (the cell's pixels stretched over it).
fn cell(texture: Arc<Texture>, src: (f32, f32, f32, f32), size: (f32, f32)) -> Arc<ImageChar> {
    let (sx, sy) = (size.0 / src.2, size.1 / src.3);
    // bitmap pixels -> image space: the cell's corner at (0, 0), scaled to the size
    let matrix = Matrix::new(sx, 0.0, 0.0, sy, -src.0 * sx, -src.1 * sy);
    Arc::new(ImageChar { size, fill: BitmapFill { texture, matrix, scale: (1.0, 1.0), repeat: false, smooth: true } })
}

impl SpeechBubble {
    /// The text field of KH3's Dialog movie (font, size, leading), its Keyblade (Message's `MessageTex01`, top-right cell)
    /// and its glove (`DialogYubi`). Missing art leaves the piece out (the bubble still works).
    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<SpeechBubble, PackError> {
        let dialog = pack.movie_linked("Dialog", &crate::camp_menu::LIBRARIES)?;
        let probe = Movie::new(dialog);
        let field = match probe.node("dialogText.textField").map(|n| &n.kind) {
            Some(Kind::Text(t)) => {
                let mut d: TextDef = (*t.def).clone();
                d.color = TEXT;
                d.word_wrap = true;
                d.align = crate::def::Align::Left;
                Some(Arc::new(d))
            }
            _ => None,
        };
        let keyblade = pack.texture("textures/Scaleform/92_message/MessageTex01.png").ok().map(|t| cell(t, ICON_SRC, ICON_SIZE));
        let glove = pack.texture("textures/Scaleform/92_message/DialogYubi.png").ok().map(|t| {
            let (w, h) = (t.width as f32, t.height as f32);
            cell(t, (0.0, 0.0, w, h), GLOVE_SIZE)
        });
        Ok(SpeechBubble::with_parts(fonts, field, keyblade, glove))
    }

    /// (tests) A bubble from its parts.
    pub fn with_parts(fonts: Fonts, field: Option<Arc<TextDef>>, keyblade: Option<Arc<ImageChar>>, glove: Option<Arc<ImageChar>>) -> SpeechBubble {
        SpeechBubble {
            movie: Movie::new(empty_movie()),
            fonts,
            field,
            keyblade,
            glove,
            phase: Phase::Closed,
            mode: BubbleMode::Line,
            name: None,
            body: Measured::default(),
            options: Vec::new(),
            cursor: 0,
            cancel: None,
            total: 0,
            shown: 0.0,
            reveal_cps: REVEAL_PER_S,
            target_size: (MIN_W, 80.0),
            size: (MIN_W, 80.0),
            target_rect: None,
            at: (0.0, 0.0),
            speaker: None,
            faces: Vec::new(),
            safe: SAFE,
            fresh: true,
            clock: 0.0,
            events: Vec::new(),
            cue: None,
            passive: false,
            drawn: None,
            shown_text: String::new(),
        }
    }

    pub fn is_open(&self) -> bool {
        !matches!(self.phase, Phase::Closed)
    }

    pub fn mode(&self) -> BubbleMode {
        self.mode
    }

    /// A line is up and all of it shows.
    pub fn finished(&self) -> bool {
        self.is_open() && self.mode == BubbleMode::Line && self.shown as usize >= self.total
    }

    /// Keys count (popped in, not leaving).
    pub fn accepts_input(&self) -> bool {
        matches!(self.phase, Phase::Open) && !self.passive
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn take_events(&mut self) -> Vec<BubbleEvent> {
        std::mem::take(&mut self.events)
    }

    /// The size the content asks (testing).
    pub fn target_size(&self) -> (f32, f32) {
        self.target_size
    }

    /// The size drawn now (eased toward the target).
    pub fn size(&self) -> (f32, f32) {
        self.size
    }

    fn line_h(&self) -> f32 {
        let Some(def) = self.field.as_ref() else { return 34.0 };
        let Some(font) = self.fonts.get(def.font_class.as_deref()) else { return 34.0 };
        (font.ascent + font.descent + font.leading) * (def.size / font.units_per_em) + def.leading
    }

    fn measure(&self, text: &str, max: f32) -> Measured {
        let Some(def) = self.field.clone() else { return Measured { text: text.to_string(), w: 8.0 * text.chars().count() as f32, h: 34.0 } };
        let Some(font) = self.fonts.get(def.font_class.as_deref()).cloned() else { return Measured { text: text.to_string(), w: 200.0, h: 34.0 } };
        let wrapped = crate::render::wrap_text(&self.fonts, &font, text, def.size, max);
        let w = wrapped.split('\n').map(|l| crate::render::markup_width(&self.fonts, &font, l, def.size)).fold(0.0, f32::max);
        let lines = wrapped.split('\n').count().max(1) as f32;
        let h = lines * self.line_h() - def.leading + 4.0;
        Measured { text: wrapped, w, h }
    }

    /// THE SIZE FROM THE CONTENT: the widest of name, text and options (+ the padding), between MIN_W and MAX_W (the
    /// text wrapped at MAX_W's inside), and the height of what is in it.
    fn content_size(&self) -> (f32, f32) {
        let mut w: f32 = self.body.w;
        let mut h = PAD_TOP;
        if let Some(n) = &self.name {
            w = w.max(n.w);
            h += self.line_h();
        }
        if !self.body.text.is_empty() {
            h += self.body.h;
        }
        if self.mode == BubbleMode::Choice {
            if !self.body.text.is_empty() || self.name.is_some() {
                h += OPTION_GAP;
            }
            for o in &self.options {
                w = w.max(OPTION_INDENT + o.w);
                h += o.h.max(self.line_h());
            }
            h += PAD_BOTTOM;
        } else {
            h += PAD_ICON;
        }
        ((w + 2.0 * PAD_X + 6.0).clamp(MIN_W, MAX_W).round(), h.round())
    }

    fn begin(&mut self, sounds: &mut Vec<UiSound>) {
        self.target_size = self.content_size();
        match self.phase {
            Phase::Closed | Phase::Closing(_) => {
                sounds.push(UiSound::Play(SE_MENU_OPEN));
                self.phase = Phase::Opening(0.0);
                self.size = self.target_size;
            }
            _ => {}
        }
        self.fresh = true;
        self.target_rect = None;
        self.cue = None;
        self.rebuild();
    }

    /// A LINE: `name` in yellow on the first line (None: no name), `text` revealed letter by letter (`reveal_cps`
    /// characters a second; 0 = all at once; None = KH's speed). A shows the rest, then goes on (`Advanced`).
    pub fn say(&mut self, name: Option<&str>, text: &str, reveal_cps: Option<f32>, sounds: &mut Vec<UiSound>) {
        let max = MAX_W - 2.0 * PAD_X - 6.0;
        self.mode = BubbleMode::Line;
        self.name = name.filter(|n| !n.is_empty()).map(|n| self.measure(&format!("<color(name)>{n}<color>"), max));
        self.body = self.measure(text, max);
        self.options.clear();
        self.total = crate::talk::visible_len(&self.body.text);
        self.reveal_cps = reveal_cps.unwrap_or(REVEAL_PER_S);
        self.shown = if self.reveal_cps <= 0.0 { self.total as f32 } else { 0.0 };
        self.begin(sounds);
    }

    /// A CHOICE: an optional question (with the name) over KH options; the glove points at `start`; D-pad / stick move
    /// it (cursor sound), A takes it (`Chosen`), B takes `cancel` when there is one.
    pub fn choose(&mut self, name: Option<&str>, prompt: &str, options: &[String], start: usize, cancel: Option<usize>, sounds: &mut Vec<UiSound>) {
        let max = MAX_W - 2.0 * PAD_X - 6.0;
        self.mode = BubbleMode::Choice;
        self.passive = false;
        self.name = name.filter(|n| !n.is_empty()).map(|n| self.measure(&format!("<color(name)>{n}<color>"), max));
        self.body = if prompt.is_empty() { Measured::default() } else { self.measure(prompt, max) };
        self.options = options.iter().map(|o| self.measure(o, max - OPTION_INDENT)).collect();
        self.cursor = start.min(options.len().saturating_sub(1));
        self.cancel = cancel.filter(|c| *c < options.len());
        self.total = crate::talk::visible_len(&self.body.text);
        self.shown = self.total as f32;
        self.begin(sounds);
    }

    /// The speaker's face on the stage this frame (None: not on screen: no tail, the lower middle). The tail follows it
    /// every frame.
    pub fn set_speaker(&mut self, face: Option<Face>) {
        self.speaker = face.filter(|f| f.x.is_finite() && f.y.is_finite() && f.r.is_finite());
    }

    /// The faces of the others in the conversation (the bubble keeps off them).
    pub fn set_faces(&mut self, faces: &[Face]) {
        self.faces = faces.iter().copied().filter(|f| f.x.is_finite() && f.y.is_finite()).collect();
    }

    /// The stage area the bubble stays in (default [`SAFE`]).
    pub fn set_safe(&mut self, safe: Rect) {
        self.safe = safe;
    }

    /// The host's say on the Keyblade cue (None: shown when a line is all shown; Some(false): never, an auto line).
    pub fn set_cue(&mut self, cue: Option<bool>) {
        self.cue = cue;
    }

    /// (C.5) An ambient line: the bubble takes no key and shows no cue until a line / choice that is not.
    pub fn set_passive(&mut self, passive: bool) {
        self.passive = passive;
    }

    pub fn passive(&self) -> bool {
        self.passive
    }

    /// Shows the rest of the line at once.
    pub fn reveal_all(&mut self) {
        if (self.shown as usize) < self.total {
            self.shown = self.total as f32;
            self.events.push(BubbleEvent::Revealed);
        }
    }

    /// The bubble leaves (its pop-out, then `Closed`).
    pub fn close(&mut self, sounds: &mut Vec<UiSound>) {
        if matches!(self.phase, Phase::Open | Phase::Opening(_)) {
            sounds.push(UiSound::Play(SE_MENU_CLOSE));
            self.phase = Phase::Closing(0.0);
        }
    }

    /// Off at once (no pop, no event).
    pub fn clear(&mut self) {
        self.phase = Phase::Closed;
        self.movie.root.extra.clear();
        self.drawn = None;
        self.shown_text.clear();
        self.target_rect = None;
    }

    /// A menu key: decide shows the rest of a line / goes on / takes the option; up / down move the glove; cancel takes
    /// the cancel option of a choice.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        if !self.accepts_input() {
            return;
        }
        match (self.mode, key) {
            (BubbleMode::Line, CampKey::Decide) if !repeat => {
                if (self.shown as usize) < self.total {
                    self.reveal_all();
                } else {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    self.events.push(BubbleEvent::Advanced);
                }
            }
            (BubbleMode::Choice, CampKey::Up | CampKey::Down) if !self.options.is_empty() => {
                let n = self.options.len();
                let next = if key == CampKey::Up { (self.cursor + n - 1) % n } else { (self.cursor + 1) % n };
                if next != self.cursor && !(repeat && ((key == CampKey::Up && next > self.cursor) || (key == CampKey::Down && next < self.cursor))) {
                    self.cursor = next;
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    self.events.push(BubbleEvent::Moved(next));
                }
            }
            (BubbleMode::Choice, CampKey::Decide) if !repeat && !self.options.is_empty() => {
                sounds.push(UiSound::Play(SE_MENU_DECIDE));
                self.events.push(BubbleEvent::Chosen(self.cursor));
            }
            (BubbleMode::Choice, CampKey::Cancel) if !repeat => {
                if let Some(c) = self.cancel {
                    self.cursor = c;
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.events.push(BubbleEvent::Chosen(c));
                }
            }
            _ => {}
        }
        self.rebuild();
    }

    /// One stage frame (1/60 s).
    pub fn tick(&mut self) {
        self.step(1.0 / 60.0);
    }

    /// `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        if !self.is_open() {
            return;
        }
        self.clock += dt;
        self.phase = match self.phase {
            Phase::Opening(t) if t + dt >= POP_IN => {
                self.events.push(BubbleEvent::Opened);
                Phase::Open
            }
            Phase::Opening(t) => Phase::Opening(t + dt),
            Phase::Closing(t) if t + dt >= POP_OUT => {
                self.phase = Phase::Closed;
                self.movie.root.extra.clear();
                self.drawn = None;
                self.shown_text.clear();
                self.target_rect = None;
                self.events.push(BubbleEvent::Closed);
                return;
            }
            Phase::Closing(t) => Phase::Closing(t + dt),
            p => p,
        };
        if self.mode == BubbleMode::Line && matches!(self.phase, Phase::Open) && (self.shown as usize) < self.total {
            self.shown = (self.shown + self.reveal_cps * dt).min(self.total as f32);
            if self.shown as usize >= self.total {
                self.events.push(BubbleEvent::Revealed);
            }
        }
        // THE SIZE eases to the content's (KH's quick resize between lines, no jump)
        let k = 1.0 - (-SIZE_EASE * dt).exp();
        self.size.0 += (self.target_size.0 - self.size.0) * k;
        self.size.1 += (self.target_size.1 - self.size.1) * k;
        if (self.size.0 - self.target_size.0).abs() < 0.5 && (self.size.1 - self.target_size.1).abs() < 0.5 {
            self.size = self.target_size;
        }
        self.rebuild_step(dt);
    }

    fn rebuild(&mut self) {
        self.rebuild_step(0.0);
    }

    /// Places the bubble, makes its pieces.
    fn rebuild_step(&mut self, dt: f32) {
        if !self.is_open() {
            return;
        }
        // THE PLACE: chosen for the target size (kept while good), eased toward; a new line snaps
        let keep = if self.fresh { None } else { self.target_rect };
        let target = place(self.target_size, self.speaker, &self.faces, &self.safe, keep);
        self.target_rect = Some(target);
        let want = (target.x0 + (target.x1 - target.x0 - self.size.0) * 0.5, target.y0 + (target.y1 - target.y0 - self.size.1) * 0.5);
        if self.fresh || dt <= 0.0 && self.at == (0.0, 0.0) {
            self.at = want;
        } else {
            let k = 1.0 - (-PLACE_EASE * dt).exp();
            self.at.0 += (want.0 - self.at.0) * k;
            self.at.1 += (want.1 - self.at.1) * k;
        }
        self.fresh = false;
        let rect = Rect::new(self.at.0.round(), self.at.1.round(), (self.at.0 + self.size.0).round(), (self.at.1 + self.size.1).round());
        let tail = tail_for(&rect, self.speaker);
        self.drawn = Some((rect, tail));
        // THE POP: scale and alpha about the tail's tip (the bubble's middle without one)
        let (s, alpha) = match self.phase {
            Phase::Opening(t) => {
                let u = (t / POP_IN).clamp(0.0, 1.0);
                let back = 1.0 + 2.2 * (u - 1.0).powi(3) + 1.2 * (u - 1.0).powi(2); // ease-out with a little overshoot
                (0.6 + 0.4 * back, u.min(1.0))
            }
            Phase::Closing(t) => {
                let u = (t / POP_OUT).clamp(0.0, 1.0);
                (1.0 - 0.18 * u, 1.0 - u)
            }
            _ => (1.0, 1.0),
        };
        let pivot = if tail.side == TailSide::None { ((rect.x0 + rect.x1) * 0.5, (rect.y0 + rect.y1) * 0.5) } else { tail.tip };
        let g = Matrix::translate(pivot.0, pivot.1).mul(&Matrix::scale(s, s)).mul(&Matrix::translate(-pivot.0, -pivot.1));
        let mut cx = Cx::IDENTITY;
        cx.mul[3] = alpha;
        let edges = outline(&rect, &tail);
        let mut nodes: Vec<Node> = Vec::new();
        // the shadow, the body (gradient + rim)
        let b = Rect0 { x0: rect.x0.min(tail.tip.0) - 4.0, y0: rect.y0.min(tail.tip.1) - 4.0, x1: rect.x1.max(tail.tip.0) + 4.0, y1: rect.y1.max(tail.tip.1) + 4.0 };
        let shadow = Shape { id: 0, bounds: b, fills: vec![Fill { paint: Paint::Solid(SHADOW), edges: edges.clone() }], strokes: Vec::new(), scale9: None };
        let mut n = node(geometry_id(CH_SHADOW, &rect, &tail), "shadow", Kind::Shape(Arc::new(shadow)));
        n.matrix = g.mul(&Matrix::translate(SHADOW_OFFSET.0, SHADOW_OFFSET.1));
        n.cx = cx;
        nodes.push(n);
        let gradient = Matrix::new(0.0, (rect.y1 - rect.y0) / 32768.0, (rect.x1 - rect.x0) / 32768.0, 0.0, (rect.x0 + rect.x1) * 0.5, (rect.y0 + rect.y1) * 0.5);
        let body = Shape { id: 0, bounds: b, fills: vec![Fill { paint: Paint::Gradient { radial: false, matrix: gradient, stops: vec![(0.0, FILL_TOP), (1.0, FILL_BOTTOM)] }, edges: edges.clone() }], strokes: vec![Stroke { width: RIM_WIDTH, color: RIM, edges }], scale9: None };
        let mut n = node(geometry_id(CH_BODY, &rect, &tail), "body", Kind::Shape(Arc::new(body)));
        n.matrix = g;
        n.cx = cx;
        nodes.push(n);
        // the words: name, text, options
        let mut y = rect.y0 + PAD_TOP;
        let x = rect.x0 + PAD_X;
        let mut shown_all = String::new();
        let text_node = |nodes: &mut Vec<Node>, k: u32, name: &str, text: &str, m: &Measured, at: (f32, f32), colour: [f32; 4]| {
            let Some(def) = self.field.clone() else { return };
            let mut d: TextDef = (*def).clone();
            d.bounds = Rect0 { x0: 0.0, y0: 0.0, x1: m.w + 12.0, y1: m.h + 8.0 };
            d.color = colour;
            let mut n = node(CH_TEXT + k, name, Kind::Text(TextNode { def: Arc::new(d), text: Some(text.to_string()), adjust: None, icon_native: false }));
            n.matrix = g.mul(&Matrix::translate(at.0 - 2.0, at.1 - 2.0));
            n.cx = cx;
            nodes.push(n);
        };
        if let Some(name) = self.name.clone() {
            text_node(&mut nodes, 1, "name", &name.text, &name, (x, y), TEXT);
            shown_all.push_str(&name.text);
            shown_all.push('\n');
            y += self.line_h();
        }
        if !self.body.text.is_empty() {
            let revealed = crate::talk::reveal(&self.body.text, self.shown as usize);
            let body = self.body.clone();
            text_node(&mut nodes, 2, "text", &revealed, &body, (x, y), TEXT);
            shown_all.push_str(&revealed);
            y += self.body.h;
        }
        if self.mode == BubbleMode::Choice {
            if !self.body.text.is_empty() || self.name.is_some() {
                y += OPTION_GAP;
            }
            let options = self.options.clone();
            for (k, o) in options.iter().enumerate() {
                let colour = if k == self.cursor { TEXT_SELECTED } else { TEXT_DIM };
                text_node(&mut nodes, 10 + k as u32, &format!("option{k}"), &o.text, o, (x + OPTION_INDENT, y), colour);
                if k == self.cursor {
                    if let Some(glove) = self.glove.clone() {
                        let nudge = GLOVE_NUDGE.0 * (self.clock * GLOVE_NUDGE.1 * std::f32::consts::TAU).sin();
                        let line = self.line_h();
                        let mut n = node(CH_GLOVE, "glove", Kind::Image(glove));
                        n.matrix = g.mul(&Matrix::translate((x + OPTION_INDENT - GLOVE_SIZE.0 - 6.0 + nudge).round(), (y + (line - GLOVE_SIZE.1) * 0.5 - 1.0).round()));
                        n.cx = cx;
                        nodes.push(n);
                    }
                    shown_all.push_str(&format!("\n> {}", o.text));
                } else {
                    shown_all.push_str(&format!("\n  {}", o.text));
                }
                y += o.h.max(self.line_h());
            }
        }
        // THE KEYBLADE at the bottom middle, bobbing, when a line waits for A
        let cue = self.cue.unwrap_or(true) && !self.passive && self.mode == BubbleMode::Line && self.shown as usize >= self.total && matches!(self.phase, Phase::Open);
        if cue {
            if let Some(key) = self.keyblade.clone() {
                let bob = ICON_BOB.0 * (self.clock * ICON_BOB.1 * std::f32::consts::TAU).sin();
                let cx_mid = (rect.x0 + rect.x1) * 0.5;
                let mut n = node(CH_KEY, "keyblade", Kind::Image(key));
                n.matrix = g.mul(&Matrix::translate((cx_mid - ICON_SIZE.0 * 0.5).round(), (rect.y1 - PAD_ICON * 0.5 - ICON_SIZE.1 * 0.5 - 2.0 + bob).round()));
                n.cx = cx;
                nodes.push(n);
                shown_all.push_str(" [keyblade]");
            }
        }
        self.shown_text = shown_all;
        self.movie.root.extra = nodes;
    }

    /// The Keyblade cue is drawn now (testing).
    pub fn cue_shown(&self) -> bool {
        self.movie.root.extra.iter().any(|n| n.ch == CH_KEY)
    }

    /// The Keyblade's rectangle on the stage now (testing).
    pub fn cue_rect(&self) -> Option<Rect> {
        self.movie.root.extra.iter().find(|n| n.ch == CH_KEY).map(|n| Rect::new(0.0, 0.0, ICON_SIZE.0, ICON_SIZE.1).transform(&n.matrix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(x: f32, y: f32) -> Face {
        Face { x, y, r: 30.0 }
    }

    #[test]
    fn the_tail_points_down_at_a_speaker_below_and_up_at_one_above() {
        let r = Rect::new(800.0, 200.0, 1200.0, 320.0);
        let t = tail_for(&r, Some(face(1000.0, 500.0)));
        assert_eq!(t.side, TailSide::Bottom);
        assert!(t.tip.1 > r.y1 && t.tip.1 < 500.0 - 30.0, "{t:?}");
        let t = tail_for(&r, Some(face(900.0, 60.0)));
        assert_eq!(t.side, TailSide::Top);
        assert!(t.tip.1 < r.y0);
        // a speaker far to the left: the base clamps to the straight edge, the tip leans toward him
        let t = tail_for(&r, Some(face(100.0, 520.0)));
        assert_eq!(t.base_x, r.x0 + RADIUS + TAIL_BASE * 0.5 + 2.0);
        assert!(t.tip.0 < t.base_x && t.tip.1 > r.y1);
        assert!(tail_for(&r, None).side == TailSide::None);
    }

    #[test]
    fn the_place_is_above_the_speaker_and_off_every_face() {
        let size = (400.0, 120.0);
        let s = face(960.0, 600.0);
        let r = place(size, Some(s), &[], &SAFE, None);
        assert!(r.y1 <= s.y - s.r - CLEAR_OF_FACE + 0.5, "{r:?}");
        assert!((r.x0 + r.x1) * 0.5 - 960.0 < 1.0);
        // another face right where the bubble would go: it moves aside
        let other = face(960.0, 380.0);
        let r2 = place(size, Some(s), &[other], &SAFE, None);
        assert_eq!(overlap(&r2, &other.rect()), 0.0, "{r2:?}");
        // a speaker at the top of the screen: below him, inside the stage
        let r3 = place(size, Some(face(960.0, 80.0)), &[], &SAFE, None);
        assert!(r3.y0 >= 80.0 + 30.0, "{r3:?}");
        // the edge of an ultrawide picture (the stage's x beyond 1920): inside the stage
        let r4 = place(size, Some(face(2300.0, 600.0)), &[], &SAFE, None);
        assert!(r4.x1 <= SAFE.x1 + 0.01);
        // kept while good
        let r5 = place(size, Some(face(965.0, 602.0)), &[], &SAFE, Some(r));
        assert_eq!(r5, r);
    }

    #[test]
    fn the_outline_is_closed() {
        for side in [TailSide::Top, TailSide::Bottom, TailSide::None] {
            let r = Rect::new(100.0, 100.0, 400.0, 200.0);
            let tail = Tail { side, base_x: 250.0, tip: (260.0, if side == TailSide::Top { 60.0 } else { 240.0 }) };
            let e = outline(&r, &tail);
            let ends = |e: &Edge| match e {
                Edge::Line(l) => ((l[0], l[1]), (l[2], l[3])),
                Edge::Curve(c) => ((c[0], c[1]), (c[4], c[5])),
            };
            for k in 0..e.len() {
                let (_, end) = ends(&e[k]);
                let (start, _) = ends(&e[(k + 1) % e.len()]);
                assert!((end.0 - start.0).abs() < 0.01 && (end.1 - start.1).abs() < 0.01, "{side:?} edge {k}: {end:?} -> {start:?}");
            }
        }
    }
}
