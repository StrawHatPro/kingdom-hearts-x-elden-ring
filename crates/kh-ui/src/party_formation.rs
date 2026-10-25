//! (party PF) THE SAVE POINT'S PARTY FORMATION SCREEN, ICON-BASED (the user, 2026-10-15: "the party formation screen
//! is fucked. Make it less bad and more icon based"). Shown inside the camp menu's movie (the same layer, the same
//! frame, title strip, help line, button guide, munny and time) while the camp menu is on its Party Formation step;
//! the camp's own character list and status panels are hidden for it (and so are the 3D pictures that sit on them).
//!
//! NOT A KH3 SCREEN [L]: KH3 has no party formation (the world decides the party). What is the game's: the pictures
//! (`UI/Icon/MenuFace` busts of the save slots for the party slots, the HUD face sheets `01_hud/face/NpcEx*` for the
//! roster icons), the diamond pattern of the camp's character windows (`15_campmenu/CampMenu01`), the list cursor
//! of the menus (`ItemListLibrary` `ListCursorAnm_forS`, its `loop`), its crown (`ListCursorCrown`) as the leader's badge,
//! the finger cursor of the message buttons (`00_system/MesBtnYubi`), the font `$sougei`, the pad glyphs of the button
//! guide, the menu sounds (static SE 0 cursor, 1 decide, 4 refused). Composed here: the layout, the frames, the
//! animations (a joining member's picture grows out of his roster icon into his slot, a leaving one shrinks back into
//! it, the leader's crown flies to the new leader, the slots refill in order, a refused edit shakes and says why).
//!
//! THE FEED IS GENERIC ([`crate::camp::FormationRows`]: members, unlocked, leader): the test level and Elden Ring
//! (kh-er partyui.rs -> kh-host's HUD) hand the same rows; a character whose key has no picture shows a silhouette.

use std::collections::HashMap;
use std::sync::Arc;

use crate::camp::FormationRows;
use crate::def::{Align, BitmapFill, Edge, Fill, MovieDef, Pack, Paint, Rect0, Shape, Stroke, TextDef, Texture};
use crate::geom::{Cx, Matrix};
use crate::player::{Kind, Movie, Node, TextNode};

/// The names of this screen's script children of the movie's root all start so.
pub const PREFIX: &str = "pf_";

/// Stage seconds of one frame.
const FRAME: f32 = 1.0 / 60.0;

// ------------------------------------------------------------------------------------------------ layout [L]
// Stage pixels (1920 x 1080; the camp's frame: the title strip ends at x 155, the right rule at x 1810, the help band
// starts at y 870). Picture sizes are chosen so that no picture is drawn above its pixel size at 2560 x 1440 (stage
// scale 1.333): the 256-pixel busts at 192 / 160, the 128-pixel HUD faces at 96, the 64-pixel finger at 48.

/// The centre of the content between the title strip and the right rule.
pub const CENTRE_X: f32 = 982.0;
/// The three party slots: (x, y, width, height, picture size). Slot 0 is the leader's, the largest.
pub const SLOTS: [(f32, f32, f32, f32, f32); 3] = [(598.0, 182.0, 240.0, 304.0, 192.0), (894.0, 224.0, 208.0, 262.0, 160.0), (1158.0, 224.0, 208.0, 262.0, 160.0)];
/// The picture's inset in its slot.
const INSET: (f32, f32) = (24.0, 22.0);
/// The headers ("Party", "Members"): y of their bar.
const HEADER_PARTY_Y: f32 = 122.0;
const HEADER_ROSTER_Y: f32 = 528.0;
const HEADER_X: (f32, f32) = (560.0, 1404.0);
/// The roster row: the icons' centre line, their spacing, the face size, the disc radius.
pub const ROSTER_Y: f32 = 650.0;
pub const ROSTER_STEP: f32 = 184.0;
const FACE: f32 = 96.0;
const DISC: f32 = 58.0;

/// The roster icon's centre of row `i` of `n`.
pub fn roster_centre(i: usize, n: usize) -> (f32, f32) {
    (CENTRE_X + (i as f32 - (n.max(1) as f32 - 1.0) / 2.0) * ROSTER_STEP, ROSTER_Y)
}

// ------------------------------------------------------------------------------------------------ the pictures

/// The pictures of the screen, read from the packs once (`CampMenuView::load`).
#[derive(Default)]
pub struct FormationArt {
    /// key -> the save slot's bust (`/Game/UI/Icon/MenuFace/MenuFaceEx*`, 256 x 256; pack build/sora_pack/saveload).
    /// key -> (texture, its rectangle, its scale and shift inside the 256 x 256 bust box).
    portraits: HashMap<String, (Arc<Texture>, [f32; 4], (f32, f32, f32))>,
    /// key -> the HUD face sheet and its first cell (`01_hud/face/NpcEx*`, the cell 128 x 128).
    faces: HashMap<String, (Arc<Texture>, [f32; 4])>,
    /// The camp's character-window pattern (`15_campmenu/CampMenu01`, the 280 x 684 diamond field at its top left).
    pattern: Option<Arc<Texture>>,
    /// The finger cursor of the message buttons (`00_system/MesBtnYubi`, 64 x 64, pointing right).
    yubi: Option<Arc<Texture>>,
    /// `ItemListLibrary` (the list cursor of the menus' rows).
    list_library: Option<Arc<MovieDef>>,
    /// The crown of the camp menu's command cursor (`MainLibrary_fla.McMainCmdCursor_89`: the bitmap
    /// `90_rsl/menulibrary/MainLibrary_91`, its crown at (176, 6) .. (252, 48), 76 x 42 pixels).
    crown: Option<(Arc<Texture>, [f32; 4])>,
}

/// Sora's bust (the HUD face cell) inside the 256 box: (scale, shift x, shift y): its content (rows 20 .. 249, centre
/// column 132.5) at x 1.08, its centre on the middle, its chain on the bottom edge, its hair 9 pixels under the top.
pub const SORA_BUST_SHIFT: (f32, f32, f32) = (1.08, 128.0 - 132.5 * 1.08, 256.0 - 249.0 * 1.08);

/// The crown's rectangle in `MainLibrary_91` (read off its alpha).
const CROWN_SRC: [f32; 4] = [176.0, 6.0, 76.0, 42.0];
/// The crown's sizes: the leader's badge and the roster's mark, at most its 76 pixels at 2560 x 1440.
const CROWN_BADGE: f32 = 54.0;
const CROWN_MARK: f32 = 32.0;

impl FormationArt {
    pub fn load(pack: &mut Pack) -> FormationArt {
        let mut art = FormationArt::default();
        for (key, bust, face) in [("sora", "MenuFaceExSora", "NpcExSora"), ("donald", "MenuFaceExDonald", "NpcExDonald"), ("goofy", "MenuFaceExGoofy", "NpcExGoofy")] {
            if let Ok(t) = pack.texture(&format!("../saveload/textures/{bust}.png")) {
                art.portraits.insert(key.to_string(), (t, [0.0, 0.0, 256.0, 256.0], (1.0, 0.0, 0.0)));
            }
            if let Ok(t) = pack.texture(&format!("textures/Scaleform/01_hud/face/{face}.png")) {
                art.faces.insert(key.to_string(), (t, [0.0, 0.0, 128.0, 128.0]));
            }
        }
        // (the user, 2026-10-15: "make sora's picture look better when he is in the party ... Donald and Goofy look perfect")
        // Every Sora bust of UI/Icon/MenuFace (Ex, Ex2, Ca, Mi, Ts, Bx) is a tight face crop, his hair cut at the top and the
        // right: not Donald's and Goofy's head-and-shoulders. KH3's own full-head render of him is the HUD's face, cell 0
        // of `01_hud/face/PcExSora01` (256 x 256, the same pixel size as the busts): his whole hair, his eyes on their eye
        // line (about row 128 of 256, as theirs), his collar and chain at the bottom. Its content (rows 20 .. 249, centre
        // column 132) is grown to their head scale (x 1.08: at most 8 % above its pixels in the leader's slot at 2560 x
        // 1440, under them everywhere else), centred, and stood on the box's bottom edge as their busts stand.
        if let Ok(t) = pack.texture("textures/Scaleform/01_hud/face/PcExSora01.png") {
            if t.width >= 256 && t.height >= 256 {
                art.portraits.insert("sora".to_string(), (t, [0.0, 0.0, 256.0, 256.0], SORA_BUST_SHIFT));
            }
        }
        art.pattern = pack.texture("textures/Scaleform/15_campmenu/CampMenu01.png").ok();
        art.yubi = pack.texture("textures/Scaleform/00_system/MesBtnYubi.png").ok();
        art.list_library = pack.movie("ItemListLibrary").ok();
        art.crown = pack.texture("textures/Scaleform/90_rsl/menulibrary/MainLibrary_91.png").ok().filter(|t| t.width >= 252 && t.height >= 48).map(|t| (t, CROWN_SRC));
        art
    }

    /// Which pictures are there (tests).
    pub fn summary(&self) -> (usize, usize, bool, bool, bool) {
        (self.portraits.len(), self.faces.len(), self.pattern.is_some(), self.yubi.is_some(), self.list_library.is_some() && self.crown.is_some())
    }
}

// ------------------------------------------------------------------------------------------------ shapes

fn rect_edges(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Edge> {
    vec![Edge::Line([x0, y0, x1, y0]), Edge::Line([x1, y0, x1, y1]), Edge::Line([x1, y1, x0, y1]), Edge::Line([x0, y1, x0, y0])]
}

/// A circle of radius `r` about (0, 0) in eight quadratic arcs.
fn circle_edges(r: f32) -> Vec<Edge> {
    let mut out = Vec::new();
    let k = r / (std::f32::consts::PI / 8.0).cos();
    for i in 0..8 {
        let a0 = i as f32 * std::f32::consts::PI / 4.0;
        let a1 = a0 + std::f32::consts::PI / 4.0;
        let am = (a0 + a1) / 2.0;
        out.push(Edge::Curve([r * a0.cos(), r * a0.sin(), k * am.cos(), k * am.sin(), r * a1.cos(), r * a1.sin()]));
    }
    out
}

fn polyline(points: &[(f32, f32)], closed: bool) -> Vec<Edge> {
    let mut out: Vec<Edge> = points.windows(2).map(|w| Edge::Line([w[0].0, w[0].1, w[1].0, w[1].1])).collect();
    if closed && points.len() > 2 {
        let (a, b) = (points[points.len() - 1], points[0]);
        out.push(Edge::Line([a.0, a.1, b.0, b.1]));
    }
    out
}

fn bounds_of(edges: &[Edge], pad: f32) -> Rect0 {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for e in edges {
        let pts: Vec<(f32, f32)> = match e {
            Edge::Line(l) => vec![(l[0], l[1]), (l[2], l[3])],
            Edge::Curve(c) => vec![(c[0], c[1]), (c[2], c[3]), (c[4], c[5])],
        };
        for (x, y) in pts {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    Rect0 { x0: x0 - pad, y0: y0 - pad, x1: x1 + pad, y1: y1 + pad }
}

fn shape(fills: Vec<Fill>, strokes: Vec<Stroke>) -> Arc<Shape> {
    let mut edges: Vec<Edge> = fills.iter().flat_map(|f| f.edges.clone()).collect();
    edges.extend(strokes.iter().flat_map(|s| s.edges.clone()));
    let pad = strokes.iter().map(|s| s.width).fold(0.0, f32::max);
    Arc::new(Shape { id: 0, bounds: bounds_of(&edges, pad), fills, strokes, scale9: None })
}

/// A linear gradient across (x0..x1) (horizontal) or (y0..y1) (vertical) of the rectangle.
fn gradient(x0: f32, y0: f32, x1: f32, y1: f32, vertical: bool, stops: &[(f32, [f32; 4])]) -> Paint {
    let (mx, my, w, h) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0).max(1.0), (y1 - y0).max(1.0));
    let matrix = if vertical { Matrix::new(0.0, h / 32768.0, -w / 32768.0, 0.0, mx, my) } else { Matrix::new(w / 32768.0, 0.0, 0.0, h / 32768.0, mx, my) };
    Paint::Gradient { radial: false, matrix, stops: stops.to_vec() }
}

/// A rectangle of a texture's pixels (sx, sy, sw, sh) shown over (0, 0, w, h).
fn picture(texture: &Arc<Texture>, src: [f32; 4], w: f32, h: f32, repeat: bool) -> Arc<Shape> {
    let (kx, ky) = (w / src[2], h / src[3]);
    let fill = BitmapFill { texture: texture.clone(), matrix: Matrix::new(kx, 0.0, 0.0, ky, -src[0] * kx, -src[1] * ky), scale: (1.0, 1.0), repeat, smooth: true };
    shape(vec![Fill { paint: Paint::Bitmap(fill), edges: rect_edges(0.0, 0.0, w, h) }], Vec::new())
}

/// A rectangle of a texture's pixels shown over (0, 0, w, h), its picture scaled and shifted by `place` (scale, x, y; cut
/// at the rectangle).
fn picture_at(texture: &Arc<Texture>, src: [f32; 4], w: f32, h: f32, place: (f32, f32, f32)) -> Arc<Shape> {
    let (kx, ky) = (w / src[2] * place.0, h / src[3] * place.0);
    let fill = BitmapFill { texture: texture.clone(), matrix: Matrix::new(kx, 0.0, 0.0, ky, place.1 - src[0] * kx, place.2 - src[1] * ky), scale: (1.0, 1.0), repeat: false, smooth: true };
    shape(vec![Fill { paint: Paint::Bitmap(fill), edges: rect_edges(0.0, 0.0, w, h) }], Vec::new())
}

/// The camp window's diamond field over (0, 0, w, h) at `scale` of its pixels, from the texture's row `from_y` (the
/// field's top rows fade out: its middle is taken).
fn pattern(texture: &Arc<Texture>, w: f32, h: f32, scale: f32, from_y: f32) -> Arc<Shape> {
    let fill = BitmapFill { texture: texture.clone(), matrix: Matrix::new(scale, 0.0, 0.0, scale, 0.0, -from_y * scale), scale: (1.0, 1.0), repeat: false, smooth: true };
    shape(vec![Fill { paint: Paint::Bitmap(fill), edges: rect_edges(0.0, 0.0, w.min(280.0 * scale), h.min((684.0 - from_y) * scale)) }], Vec::new())
}

/// The KH crown drawn (when the pack has no `ListCursorCrown`): three points, the middle one highest, a band below.
fn crown_shape(w: f32) -> Arc<Shape> {
    let s = w / 64.0;
    let p = |x: f32, y: f32| (x * s, y * s);
    let outline = [p(-30.0, 14.0), p(-32.0, -16.0), p(-16.0, -2.0), p(0.0, -26.0), p(16.0, -2.0), p(32.0, -16.0), p(30.0, 14.0)];
    let fill = gradient(-32.0 * s, -26.0 * s, 32.0 * s, 14.0 * s, true, &[(0.0, [150.0, 210.0, 255.0, 255.0]), (0.55, [40.0, 120.0, 255.0, 255.0]), (1.0, [20.0, 70.0, 200.0, 255.0])]);
    let band = [p(-30.0, 8.0), p(30.0, 8.0), p(30.0, 18.0), p(-30.0, 18.0)];
    shape(
        vec![Fill { paint: fill, edges: polyline(&outline, true) }, Fill { paint: Paint::Solid([30.0, 80.0, 220.0, 255.0]), edges: polyline(&band, true) }],
        vec![Stroke { width: 2.5 * s, color: [255.0, 255.0, 255.0, 255.0], edges: polyline(&outline, true) }, Stroke { width: 2.5 * s, color: [255.0, 255.0, 255.0, 255.0], edges: polyline(&band, true) }],
    )
}

// ------------------------------------------------------------------------------------------------ animation

/// Where a picture is: its top-left, its size, its opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub alpha: f32,
}

impl Pose {
    fn lerp(&self, b: &Pose, u: f32) -> Pose {
        let l = |a: f32, b: f32| a + (b - a) * u;
        Pose { x: l(self.x, b.x), y: l(self.y, b.y), size: l(self.size, b.size), alpha: l(self.alpha, b.alpha) }
    }
}

fn ease_out_cubic(u: f32) -> f32 {
    1.0 - (1.0 - u).powi(3)
}

/// A move with a slight overshoot at the end (KH's menus pop their parts in).
fn ease_out_back(u: f32) -> f32 {
    let c = 1.4;
    1.0 + (c + 1.0) * (u - 1.0).powi(3) + c * (u - 1.0).powi(2)
}

#[derive(Clone, Copy, Debug)]
struct Tween {
    from: Pose,
    to: Pose,
    t: f32,
    dur: f32,
    back: bool,
}

impl Tween {
    fn at(p: Pose) -> Tween {
        Tween { from: p, to: p, t: 1.0, dur: 1.0, back: false }
    }

    fn now(&self) -> Pose {
        let u = (self.t / self.dur).clamp(0.0, 1.0);
        let e = if self.back { ease_out_back(u) } else { ease_out_cubic(u) };
        let mut p = self.from.lerp(&self.to, e);
        // the opacity never overshoots
        p.alpha = self.from.alpha + (self.to.alpha - self.from.alpha) * ease_out_cubic(u);
        p
    }

    fn retarget(&mut self, to: Pose, dur: f32, back: bool) {
        if to != self.to {
            *self = Tween { from: self.now(), to, t: 0.0, dur, back };
        }
    }

    fn step(&mut self, dt: f32) {
        self.t = (self.t + dt).min(self.dur);
    }

    fn moving(&self) -> bool {
        self.t < self.dur
    }
}

/// Seconds of the moves [L].
pub const JOIN_S: f32 = 0.34;
pub const LEAVE_S: f32 = 0.26;
pub const CROWN_S: f32 = 0.42;
pub const CURSOR_S: f32 = 0.12;
pub const SHAKE_S: f32 = 0.40;
pub const FADE_IN_S: f32 = 0.30;
pub const FADE_OUT_S: f32 = 0.18;
pub const MESSAGE_S: f32 = 2.6;
/// The help line's red flash after a refusal.
pub const REFUSAL_FLASH_S: f32 = 0.9;

// ------------------------------------------------------------------------------------------------ the scene

/// THE SCREEN'S STATE: the feed it shows, the cursor, and every moving part.
pub struct FormationScene {
    feed: FormationRows,
    cursor: usize,
    /// The slot each roster character holds (None: not in the party).
    slot_of: Vec<Option<usize>>,
    portraits: Vec<Tween>,
    crown: Tween,
    /// The crown's flight: its arc while it moves.
    crown_arc: bool,
    cursor_x: Tween,
    /// Per slot: the flash after a change (seconds left).
    flash: [f32; 3],
    /// The character whose edit was refused and the shake's time left.
    shake: Option<(usize, f32)>,
    message: Option<(String, f32)>,
    age: f32,
    closing: Option<f32>,
    /// Time for the pulses.
    clock: f32,
    shapes: Shapes,
    /// The live KH3 cursor and crown instances (their timelines play).
    cursor_node: Option<Node>,
    yubi: Option<Arc<Shape>>,
}

/// The shapes and text styles of the screen, made once.
struct Shapes {
    card_base: [Arc<Shape>; 3],
    card_pattern: [Option<Arc<Shape>>; 3],
    card_edge: [Arc<Shape>; 3],
    card_glow: [Arc<Shape>; 3],
    card_bar: [Arc<Shape>; 3],
    card_foot: [Arc<Shape>; 3],
    header: Arc<Shape>,
    disc: Arc<Shape>,
    ring: Arc<Shape>,
    glow: Arc<Shape>,
    check: Arc<Shape>,
    crown_small: Arc<Shape>,
    crown_big: Arc<Shape>,
    /// KH3's own crown (the command cursor's), when the pack has it: drawn instead of the two above.
    crown_kh: Option<(Arc<Shape>, Matrix)>,
    /// key -> bust (256 x 256 at 1:1) and face (96 x 96).
    busts: HashMap<String, Arc<Shape>>,
    faces: HashMap<String, Arc<Shape>>,
    unknown_face: Arc<Shape>,
    text: HashMap<&'static str, Arc<TextDef>>,
}

fn text_def(size: f32, color: [f32; 4], align: Align, w: f32, h: f32) -> Arc<TextDef> {
    Arc::new(TextDef { id: 0, bounds: Rect0 { x0: 0.0, y0: 0.0, x1: w, y1: h }, font_class: Some("$sougei".to_string()), size, color, align, initial: String::new(), word_wrap: false, leading: 0.0 })
}

impl Shapes {
    fn new(art: &FormationArt) -> Shapes {
        let per_slot = |f: &dyn Fn(f32, f32, f32) -> Arc<Shape>| -> [Arc<Shape>; 3] { [0, 1, 2].map(|k| f(SLOTS[k].2, SLOTS[k].3, SLOTS[k].4)) };
        let card_base = per_slot(&|w, h, _| shape(vec![Fill { paint: gradient(0.0, 0.0, w, h, true, &[(0.0, [22.0, 34.0, 66.0, 235.0]), (0.7, [10.0, 16.0, 36.0, 235.0]), (1.0, [6.0, 8.0, 18.0, 240.0])]), edges: rect_edges(0.0, 0.0, w, h) }], Vec::new()));
        let card_pattern = [0, 1, 2].map(|k| art.pattern.as_ref().map(|t| pattern(t, SLOTS[k].2, SLOTS[k].4 + INSET.1 + 10.0, 0.86, 300.0)));
        let card_edge = per_slot(&|w, h, _| shape(Vec::new(), vec![Stroke { width: 2.0, color: [170.0, 196.0, 240.0, 210.0], edges: rect_edges(0.0, 0.0, w, h) }]));
        let card_glow = per_slot(&|w, h, _| shape(Vec::new(), vec![Stroke { width: 6.0, color: [80.0, 190.0, 255.0, 120.0], edges: rect_edges(-3.0, -3.0, w + 3.0, h + 3.0) }, Stroke { width: 2.5, color: [190.0, 240.0, 255.0, 255.0], edges: rect_edges(0.0, 0.0, w, h) }]));
        // the name block of the camp's character window: a grey bar, then the black stats strip
        let card_bar = per_slot(&|w, _, p| {
            let (y0, y1) = (INSET.1 + p + 10.0, INSET.1 + p + 46.0);
            shape(vec![Fill { paint: gradient(0.0, y0, w, y1, true, &[(0.0, [110.0, 110.0, 116.0, 255.0]), (1.0, [64.0, 64.0, 70.0, 255.0])]), edges: rect_edges(0.0, y0, w, y1) }], vec![Stroke { width: 1.5, color: [220.0, 220.0, 228.0, 200.0], edges: polyline(&[(0.0, y0), (w, y0)], false) }])
        });
        let card_foot = per_slot(&|w, h, p| {
            let y0 = INSET.1 + p + 46.0;
            shape(vec![Fill { paint: Paint::Solid([4.0, 6.0, 12.0, 235.0]), edges: rect_edges(0.0, y0, w, h) }], Vec::new())
        });
        let hw = HEADER_X.1 - HEADER_X.0;
        let header = shape(
            vec![Fill { paint: gradient(0.0, 0.0, hw, 30.0, false, &[(0.0, [150.0, 160.0, 185.0, 150.0]), (0.6, [120.0, 130.0, 160.0, 70.0]), (1.0, [120.0, 130.0, 160.0, 0.0])]), edges: rect_edges(0.0, 0.0, hw, 30.0) }],
            vec![Stroke { width: 1.5, color: [210.0, 220.0, 240.0, 150.0], edges: polyline(&[(0.0, 30.0), (hw * 0.8, 30.0)], false) }],
        );
        let disc = shape(vec![Fill { paint: Paint::Gradient { radial: true, matrix: Matrix::new(DISC / 16384.0, 0.0, 0.0, DISC / 16384.0, 0.0, 0.0), stops: vec![(0.0, [40.0, 70.0, 130.0, 240.0]), (0.8, [14.0, 24.0, 52.0, 240.0]), (1.0, [8.0, 12.0, 28.0, 240.0])] }, edges: circle_edges(DISC) }], Vec::new());
        let ring = shape(Vec::new(), vec![Stroke { width: 2.5, color: [180.0, 200.0, 240.0, 220.0], edges: circle_edges(DISC) }]);
        let glow = shape(Vec::new(), vec![Stroke { width: 10.0, color: [70.0, 180.0, 255.0, 110.0], edges: circle_edges(DISC + 5.0) }, Stroke { width: 3.0, color: [200.0, 245.0, 255.0, 255.0], edges: circle_edges(DISC + 1.0) }]);
        let check = shape(
            vec![Fill { paint: Paint::Solid([30.0, 110.0, 230.0, 255.0]), edges: circle_edges(16.0) }],
            vec![Stroke { width: 2.0, color: [255.0, 255.0, 255.0, 255.0], edges: circle_edges(16.0) }, Stroke { width: 4.0, color: [255.0, 255.0, 255.0, 255.0], edges: polyline(&[(-7.5, 0.5), (-2.0, 6.5), (8.0, -6.0)], false) }],
        );
        let crown_small = crown_shape(34.0);
        let crown_big = crown_shape(64.0);
        // KH3's crown, its centre at (0, 0), CROWN_BADGE wide
        let crown_kh = art.crown.as_ref().map(|(t, src)| {
            let h = CROWN_BADGE * src[3] / src[2];
            (picture(t, *src, CROWN_BADGE, h, false), Matrix::translate(-CROWN_BADGE / 2.0, -h / 2.0))
        });
        let busts = art.portraits.iter().map(|(k, (t, src, shift))| (k.clone(), picture_at(t, *src, 256.0, 256.0, *shift))).collect();
        let faces = art.faces.iter().map(|(k, (t, cell))| (k.clone(), picture(t, *cell, FACE, FACE, false))).collect();
        // a character without a picture: a head-and-shoulders silhouette
        let unknown_face = shape(
            vec![Fill { paint: Paint::Solid([200.0, 200.0, 210.0, 255.0]), edges: { let mut e = circle_edges(22.0); for x in e.iter_mut() { if let Edge::Curve(c) = x { c[1] -= 12.0; c[3] -= 12.0; c[5] -= 12.0; } } e } }, Fill { paint: Paint::Solid([200.0, 200.0, 210.0, 255.0]), edges: polyline(&[(-36.0, 44.0), (-30.0, 22.0), (0.0, 12.0), (30.0, 22.0), (36.0, 44.0)], true) }],
            Vec::new(),
        );
        let white = [255.0, 255.0, 255.0, 255.0];
        let mut text = HashMap::new();
        text.insert("header", text_def(21.0, white, Align::Left, 400.0, 30.0));
        text.insert("name0", text_def(25.0, white, Align::Center, SLOTS[0].2, 36.0));
        text.insert("name1", text_def(23.0, white, Align::Center, SLOTS[1].2, 36.0));
        text.insert("lv_label", text_def(18.0, [250.0, 230.0, 40.0, 255.0], Align::Right, 120.0, 30.0));
        text.insert("tag", text_def(18.0, [190.0, 200.0, 220.0, 255.0], Align::Left, 140.0, 30.0));
        text.insert("tag_leader", text_def(18.0, [90.0, 215.0, 255.0, 255.0], Align::Left, 140.0, 30.0));
        text.insert("empty", text_def(22.0, [150.0, 160.0, 185.0, 200.0], Align::Center, SLOTS[1].2, 34.0));
        text.insert("empty0", text_def(24.0, [150.0, 160.0, 185.0, 200.0], Align::Center, SLOTS[0].2, 34.0));
        text.insert("roster", text_def(22.0, white, Align::Center, 172.0, 32.0));
        text.insert("question", text_def(64.0, [230.0, 236.0, 255.0, 255.0], Align::Center, 100.0, 80.0));
        Shapes { card_base, card_pattern, card_edge, card_glow, card_bar, card_foot, header, disc, ring, glow, check, crown_small, crown_big, crown_kh, busts, faces, unknown_face, text }
    }
}

/// The characters' ids of this screen's shapes and texts: one per distinct picture (the layer's change test hashes
/// the character id, the matrix and the colour transform).
const CH: u32 = 0x7F50_0000;

fn node(name: String, ch: u32, kind: Kind, matrix: Matrix, cx: Cx) -> Node {
    Node { ch, depth: 0, born: 1, z: None, name: Some(Arc::from(name.as_str())), matrix, cx, clip: 0, blend: 0, visible: true, script_visible: None, script_matrix: true, ratio: 0.0, kind }
}

fn alpha(a: f32) -> Cx {
    Cx { mul: [1.0, 1.0, 1.0, a.clamp(0.0, 1.0)], add: [0.0; 4] }
}

fn tint(rgb: f32, a: f32) -> Cx {
    Cx { mul: [rgb, rgb, rgb, a.clamp(0.0, 1.0)], add: [0.0; 4] }
}

impl FormationScene {
    pub fn new(art: &FormationArt, feed: &FormationRows, cursor: usize) -> FormationScene {
        let shapes = Shapes::new(art);
        let cursor_node = art.list_library.as_ref().and_then(|m| Movie::instance_of(m, "tres.rsl.ListItemBgI2", "pf_cursor", Some("select")));
        let yubi = art.yubi.as_ref().map(|t| picture(t, [0.0, 0.0, t.width as f32, t.height as f32], 48.0, 48.0, false));
        let n = feed.members.len();
        let mut scene = FormationScene {
            feed: feed.clone(),
            cursor,
            slot_of: vec![None; n],
            portraits: Vec::new(),
            crown: Tween::at(Pose { x: 0.0, y: 0.0, size: 1.0, alpha: 1.0 }),
            crown_arc: false,
            cursor_x: Tween::at(Pose { x: roster_centre(cursor, n).0, y: ROSTER_Y, size: 1.0, alpha: 1.0 }),
            flash: [0.0; 3],
            shake: None,
            message: None,
            age: 0.0,
            closing: None,
            clock: 0.0,
            shapes,
            cursor_node,
            yubi,
        };
        scene.slot_of = slots_of(feed);
        scene.portraits = (0..n).map(|i| Tween::at(scene.rest_pose(i))).collect();
        scene.crown = Tween::at(crown_pose(0));
        scene
    }

    /// Where character `i`'s bust rests: its slot, or (out of the party) shrunk into its roster icon, unseen.
    fn rest_pose(&self, i: usize) -> Pose {
        match self.slot_of.get(i).copied().flatten() {
            Some(k) => {
                let (x, y, _, _, p) = SLOTS[k];
                Pose { x: x + INSET.0, y: y + INSET.1, size: p, alpha: 1.0 }
            }
            None => {
                let (cx, cy) = roster_centre(i, self.feed.members.len());
                Pose { x: cx - FACE / 2.0, y: cy - FACE / 2.0, size: FACE, alpha: 0.0 }
            }
        }
    }

    pub fn feed(&self) -> &FormationRows {
        &self.feed
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The slot of roster character `i` as shown (0 = the leader's).
    pub fn slot(&self, i: usize) -> Option<usize> {
        self.slot_of.get(i).copied().flatten()
    }

    /// Where roster character `i`'s bust is drawn now.
    pub fn portrait_now(&self, i: usize) -> Option<Pose> {
        self.portraits.get(i).map(|t| t.now())
    }

    /// The crown's place now (its centre).
    pub fn crown_now(&self) -> Pose {
        self.crown.now()
    }

    /// Something is still moving.
    pub fn animating(&self) -> bool {
        self.portraits.iter().any(|t| t.moving()) || self.crown.moving() || self.shake.is_some() || self.age < FADE_IN_S
    }

    /// THE HOST'S NEW FEED (after an edit, accepted or refused) and the cursor: the moves begin.
    pub fn set(&mut self, feed: &FormationRows, cursor: usize) {
        if cursor != self.cursor {
            self.cursor = cursor;
            let (x, y) = roster_centre(cursor, feed.members.len());
            self.cursor_x.retarget(Pose { x, y, size: 1.0, alpha: 1.0 }, CURSOR_S, false);
        }
        if *feed == self.feed {
            return;
        }
        let new_slots = slots_of(feed);
        if new_slots.len() != self.portraits.len() {
            // a different roster: no moves, the new places at once
            self.feed = feed.clone();
            self.slot_of = new_slots;
            self.portraits = (0..self.slot_of.len()).map(|i| Tween::at(self.rest_pose(i))).collect();
            return;
        }
        let refused = feed.refused.clone().filter(|_| feed.stamp != self.feed.stamp || feed.refused != self.feed.refused);
        let old_slots = std::mem::replace(&mut self.slot_of, new_slots);
        self.feed = feed.clone();
        for i in 0..self.portraits.len() {
            let (was, now) = (old_slots[i], self.slot_of[i]);
            if was == now {
                continue;
            }
            let to = self.rest_pose(i);
            match (was, now) {
                // joins: out of his roster icon, growing, with KH's overshoot
                (None, Some(k)) => {
                    let mut from = self.rest_pose_out(i);
                    from.alpha = 0.0;
                    self.portraits[i] = Tween { from, to, t: 0.0, dur: JOIN_S, back: true };
                    self.flash[k] = 0.35;
                }
                // leaves: back into his icon
                (Some(_), None) => self.portraits[i].retarget(to, LEAVE_S, false),
                // changes slot (a new leader, the slots refilling)
                (Some(_), Some(k)) => {
                    self.portraits[i].retarget(to, JOIN_S, true);
                    self.flash[k] = 0.25;
                }
                (None, None) => {}
            }
        }
        // the crown stays on slot 0; a new leader: it flies over (out of the old slot of the new leader)
        let leader_moved = old_slots.get(feed.leader).copied().flatten().is_some_and(|k| k != 0);
        if leader_moved {
            let k = old_slots[feed.leader].unwrap_or(0);
            self.crown = Tween { from: crown_pose(k), to: crown_pose(0), t: 0.0, dur: CROWN_S, back: true };
            self.crown_arc = true;
        }
        if let Some(text) = refused {
            self.shake = Some((self.cursor, SHAKE_S));
            self.message = Some((text, MESSAGE_S));
        } else {
            self.message = None;
        }
    }

    fn rest_pose_out(&self, i: usize) -> Pose {
        let (cx, cy) = roster_centre(i, self.feed.members.len());
        Pose { x: cx - FACE / 2.0, y: cy - FACE / 2.0, size: FACE, alpha: 0.0 }
    }

    /// A REFUSED EDIT's reason while it is fresh, and how strongly the help line that says it flashes (1 at the refusal,
    /// 0 after [`REFUSAL_FLASH_S`]).
    pub fn refusal(&self) -> Option<(&str, f32)> {
        self.message.as_ref().map(|(text, t)| (text.as_str(), ((t - (MESSAGE_S - REFUSAL_FLASH_S)) / REFUSAL_FLASH_S).clamp(0.0, 1.0)))
    }

    /// The screen goes (the camp menu's `out`).
    pub fn close(&mut self) {
        if self.closing.is_none() {
            self.closing = Some(0.0);
        }
    }

    /// One stage frame.
    pub fn step(&mut self) {
        let dt = FRAME;
        self.age += dt;
        self.clock += dt;
        for t in &mut self.portraits {
            t.step(dt);
        }
        self.crown.step(dt);
        if !self.crown.moving() {
            self.crown_arc = false;
        }
        self.cursor_x.step(dt);
        for f in &mut self.flash {
            *f = (*f - dt).max(0.0);
        }
        if let Some((i, t)) = self.shake {
            self.shake = (t - dt > 0.0).then_some((i, t - dt));
        }
        if let Some((text, t)) = self.message.take() {
            if t - dt > 0.0 {
                self.message = Some((text, t - dt));
            }
        }
        if let Some(c) = self.closing.as_mut() {
            *c += dt;
        }
    }

    /// The whole screen's opacity and its slide in.
    fn presence(&self) -> (f32, f32) {
        let a_in = ease_out_cubic((self.age / FADE_IN_S).clamp(0.0, 1.0));
        let a_out = self.closing.map_or(1.0, |c| 1.0 - (c / FADE_OUT_S).clamp(0.0, 1.0));
        (a_in * a_out, (1.0 - a_in) * 24.0)
    }

    fn shake_of(&self, i: usize) -> f32 {
        match self.shake {
            Some((j, t)) if j == i => {
                let u = 1.0 - t / SHAKE_S;
                (u * 9.0 * std::f32::consts::TAU).sin() * 11.0 * (1.0 - u)
            }
            _ => 0.0,
        }
    }

    /// THE SCREEN'S NODES, in drawing order (back to front), for the movie root's script children.
    pub fn nodes(&mut self) -> Vec<Node> {
        let (fade, drop) = self.presence();
        let mut out = Vec::new();
        if fade <= 0.0 {
            return out;
        }
        let s = &self.shapes;
        let n = self.feed.members.len();
        let txt = |name: String, ch: u32, def: &Arc<TextDef>, text: &str, x: f32, y: f32, cx: Cx| node(name, ch, Kind::Text(TextNode { def: def.clone(), text: Some(text.to_string()), adjust: None, icon_native: false }), Matrix::translate(x, y), cx);
        let shp = |name: String, ch: u32, sh: &Arc<Shape>, m: Matrix, cx: Cx| node(name, ch, Kind::Shape(sh.clone()), m, cx);
        // the headers
        for (k, (y, label)) in [(HEADER_PARTY_Y, "Party"), (HEADER_ROSTER_Y, "Members")].into_iter().enumerate() {
            out.push(shp(format!("{PREFIX}hdr{k}"), CH + 1, &s.header, Matrix::translate(HEADER_X.0, y - drop * 0.5), alpha(fade)));
            out.push(txt(format!("{PREFIX}hdr{k}_t"), CH + 2, &s.text["header"], label, HEADER_X.0 + 18.0, y - drop * 0.5, alpha(fade)));
        }
        // who holds each slot
        let mut holder = [None; 3];
        for (i, k) in self.slot_of.iter().enumerate() {
            if let Some(k) = k {
                holder[*k] = Some(i);
            }
        }
        let focused_slot = self.slot_of.get(self.cursor).copied().flatten();
        // the slot frames
        for k in 0..3 {
            let (x, y, w, _, p) = SLOTS[k];
            let y = y - drop;
            let at = Matrix::translate(x, y);
            let size = k.min(1) as u32;
            out.push(shp(format!("{PREFIX}card{k}"), CH + 10 + size, &s.card_base[k], at, alpha(fade)));
            if let Some(pat) = &s.card_pattern[k] {
                // the field is black with alpha (the camp lays it over a grey window): here a light blue over the dark card
                let lit = if holder[k].is_some() { 0.30 } else { 0.16 };
                out.push(shp(format!("{PREFIX}card{k}_pat"), CH + 13 + size, pat, Matrix::translate(x, y), Cx { mul: [0.0, 0.0, 0.0, lit * fade], add: [120.0, 150.0, 215.0, 0.0] }));
            }
            out.push(shp(format!("{PREFIX}card{k}_foot"), CH + 16 + size, &s.card_foot[k], at, alpha(fade)));
            out.push(shp(format!("{PREFIX}card{k}_bar"), CH + 19 + size, &s.card_bar[k], at, alpha(fade)));
            let flash = self.flash[k] / 0.35;
            let edge_cx = Cx { mul: [1.0, 1.0, 1.0, fade], add: [flash * 120.0, flash * 120.0, flash * 120.0, 0.0] };
            out.push(shp(format!("{PREFIX}card{k}_edge"), CH + 22 + size, &s.card_edge[k], at, edge_cx));
            if focused_slot == Some(k) {
                // steady (a pulse here would redraw the whole screen every frame: only the roster row pulses)
                let pulse = 1.0;
                out.push(shp(format!("{PREFIX}card{k}_glow"), CH + 25 + size, &s.card_glow[k], at, alpha(fade * pulse)));
            }
            let name_def = if k == 0 { &s.text["name0"] } else { &s.text["name1"] };
            let bar_y = y + INSET.1 + p + 10.0;
            match holder[k] {
                Some(i) => {
                    let m = &self.feed.members[i];
                    out.push(txt(format!("{PREFIX}card{k}_name"), CH + 30 + size, name_def, &m.name, x, bar_y + 1.0, alpha(fade)));
                    let foot_y = y + INSET.1 + p + 50.0;
                    let (tag, tag_def) = if k == 0 { ("LEADER".to_string(), &s.text["tag_leader"]) } else { (format!("SLOT {}", k + 1), &s.text["tag"]) };
                    out.push(txt(format!("{PREFIX}card{k}_tag"), CH + 33 + size, tag_def, &tag, x + 16.0, foot_y, alpha(fade)));
                    if m.level > 0 {
                        out.push(txt(format!("{PREFIX}card{k}_lv"), CH + 36, &s.text["lv_label"], &format!("LV {}", m.level), x + w - 136.0, foot_y, alpha(fade)));
                    }
                }
                None => {
                    let def = if k == 0 { &s.text["empty0"] } else { &s.text["empty"] };
                    out.push(txt(format!("{PREFIX}card{k}_empty"), CH + 38 + size, def, "Empty", x, y + INSET.1 + p / 2.0 - 16.0, alpha(fade)));
                    out.push(txt(format!("{PREFIX}card{k}_name"), CH + 30 + size, name_def, "-", x, bar_y + 1.0, alpha(fade * 0.6)));
                }
            }
        }
        // the roster: discs, faces, rings, marks, names
        let cur = self.cursor_x.now();
        for (i, m) in self.feed.members.iter().enumerate() {
            let (cx0, cy) = roster_centre(i, n);
            let cx = cx0 + self.shake_of(i);
            let cy = cy - drop * 0.5;
            let focused = i == self.cursor;
            let lift = if focused { 1.08 } else { 1.0 };
            let at = Matrix::new(lift, 0.0, 0.0, lift, cx, cy);
            out.push(shp(format!("{PREFIX}icon{i}_disc"), CH + 50, &s.disc, at, alpha(fade)));
            let in_party = self.slot_of.get(i).copied().flatten();
            let face = s.faces.get(&m.key).unwrap_or(&s.unknown_face);
            let face_m = at.mul(&Matrix::translate(-FACE / 2.0, -FACE / 2.0 - 4.0));
            let face_ch = CH + 60 + s.faces.keys().position(|k| *k == m.key).map_or(15, |p| p as u32);
            let face_cx = if m.locked {
                // a silhouette: the face's shape in the disc's dark blue
                Cx { mul: [0.0, 0.0, 0.0, fade], add: [2.0, 4.0, 12.0, 0.0] }
            } else if in_party.is_some() && !focused {
                tint(0.55, fade)
            } else {
                alpha(fade)
            };
            let face_m = if s.faces.contains_key(&m.key) { face_m } else { at.mul(&Matrix::translate(0.0, -2.0)) };
            out.push(shp(format!("{PREFIX}icon{i}_face"), face_ch, face, face_m, face_cx));
            out.push(shp(format!("{PREFIX}icon{i}_ring"), CH + 51, &s.ring, at, alpha(fade)));
            if m.locked {
                out.push(txt(format!("{PREFIX}icon{i}_q"), CH + 52, &s.text["question"], "?", cx - 50.0, cy - 44.0, alpha(fade)));
            }
            match in_party {
                Some(0) => crown_nodes(&mut out, s, &format!("{PREFIX}icon{i}_mark"), Matrix::translate(cx + 38.0, cy + 40.0), CROWN_MARK, fade, false),
                Some(_) => out.push(shp(format!("{PREFIX}icon{i}_mark"), CH + 54, &s.check, Matrix::translate(cx + 40.0, cy + 40.0), alpha(fade))),
                None => {}
            }
        }
        // the KH3 list cursor under the focused name, the glow ring, then the names over it
        if n > 0 {
            let (x, y) = (cur.x + self.shake_of(self.cursor), ROSTER_Y - drop * 0.5);
            if let Some(mut c) = self.cursor_node.clone() {
                // the list rows' select bar (`ListItemBgI2`, `select`): drawn over (-4, -4) .. (724, 84) of its origin,
                // squeezed under the focused name (its gradients and shine have no fixed-size parts)
                let (sx, sy) = (184.0 / 728.0, 36.0 / 88.0);
                c.matrix = Matrix::new(sx, 0.0, 0.0, sy, x - 92.0 + 4.0 * sx, y + DISC + 13.0 + 4.0 * sy);
                c.cx = alpha(fade * 0.95);
                out.push(c);
            }
            let pulse = 0.65 + 0.35 * (self.clock * std::f32::consts::TAU / 1.1).sin();
            out.push(shp(format!("{PREFIX}glow"), CH + 55, &s.glow, Matrix::new(1.08, 0.0, 0.0, 1.08, x, y), alpha(fade * pulse)));
            if let Some(y_shape) = &self.yubi {
                let bob = (self.clock * std::f32::consts::TAU * 1.4).sin() * 6.0;
                out.push(shp(format!("{PREFIX}yubi"), CH + 56, y_shape, Matrix::translate(x - DISC * 1.08 - 58.0 + bob, y - 24.0), alpha(fade)));
            }
        }
        for (i, m) in self.feed.members.iter().enumerate() {
            let (cx0, cy) = roster_centre(i, n);
            let cx = cx0 + self.shake_of(i);
            let name = if m.locked { "???" } else { m.name.as_str() };
            let shade = if i == self.cursor { 1.0 } else if m.locked { 0.6 } else { 0.85 };
            out.push(txt(format!("{PREFIX}icon{i}_name"), CH + 57, &s.text["roster"], name, cx - 86.0, cy + DISC + 14.0 - drop * 0.5, tint(shade, fade)));
        }
        // the party's busts, over everything (a joining one flies up out of the roster)
        for (i, m) in self.feed.members.iter().enumerate() {
            let Some(p) = self.portraits.get(i).map(|t| t.now()) else { continue };
            if p.alpha <= 0.01 {
                continue;
            }
            let Some(bust) = s.busts.get(&m.key) else { continue };
            let k = p.size / 256.0;
            let dx = self.shake_of(i);
            let slot_drop = if self.slot_of.get(i).copied().flatten().is_some() { drop } else { drop * 0.5 };
            out.push(shp(format!("{PREFIX}bust{i}"), CH + 70 + i as u32, bust, Matrix::new(k, 0.0, 0.0, k, p.x + dx, p.y - slot_drop), alpha(p.alpha * fade)));
        }
        // the leader's crown on slot 0's top edge (flying over on a change)
        {
            let c = self.crown.now();
            let u = (self.crown.t / self.crown.dur).clamp(0.0, 1.0);
            let arc = if self.crown_arc { -(u * std::f32::consts::PI).sin() * 70.0 } else { 0.0 };
            let (x, y) = (c.x, c.y + arc - drop);
            let spin = if self.crown_arc { (u * std::f32::consts::PI).sin() * 0.25 } else { 0.0 };
            let (sn, cs) = spin.sin_cos();
            crown_nodes(&mut out, s, &format!("{PREFIX}crown"), Matrix::new(cs, sn, -sn, cs, x, y), CROWN_BADGE, fade, true);
        }
        out
    }

    /// PUTS THE SCREEN INTO THE MOVIE: the root's script children named [`PREFIX`]... are replaced (the KH3 cursor
    /// keeps its own timeline: it is carried over from the last frame).
    pub fn apply(&mut self, movie: &mut Movie) {
        // the live cursor instance (its loop plays with the movie's ticks)
        if let Some(pos) = movie.root.extra.iter().position(|n| n.name.as_deref() == Some("pf_cursor")) {
            self.cursor_node = Some(movie.root.extra.remove(pos));
        }
        let nodes = self.nodes();
        clear(movie);
        movie.root.extra.extend(nodes);
    }
}

/// THE LEADER'S CROWN at `at` (its centre), `size` wide: KH3's own (the command cursor's cyan crown) over a dark
/// shadow and, for the badge, a white rim; the drawn one when the pack has none.
fn crown_nodes(out: &mut Vec<Node>, s: &Shapes, name: &str, at: Matrix, size: f32, fade: f32, rim: bool) {
    match &s.crown_kh {
        Some((sh, norm)) => {
            let k = size / CROWN_BADGE;
            let place = |dx: f32, dy: f32, grow: f32| at.mul(&Matrix::new(k * grow, 0.0, 0.0, k * grow, dx, dy)).mul(norm);
            out.push(node(format!("{name}_shadow"), CH + 90, Kind::Shape(sh.clone()), place(2.0, 3.0, 1.0), Cx { mul: [0.0, 0.0, 0.0, 0.55 * fade], add: [0.0; 4] }));
            if rim {
                out.push(node(format!("{name}_rim"), CH + 90, Kind::Shape(sh.clone()), place(0.0, 0.0, 1.14), Cx { mul: [0.0, 0.0, 0.0, fade], add: [235.0, 248.0, 255.0, 0.0] }));
            }
            out.push(node(name.to_string(), CH + 90, Kind::Shape(sh.clone()), place(0.0, 0.0, 1.0), alpha(fade)));
        }
        None => {
            let sh = if rim { &s.crown_big } else { &s.crown_small };
            out.push(node(name.to_string(), CH + 91 + rim as u32, Kind::Shape(sh.clone()), at, alpha(fade)));
        }
    }
}

/// Takes the screen out of the movie.
pub fn clear(movie: &mut Movie) {
    movie.root.extra.retain(|n| !n.name.as_deref().is_some_and(|s| s.starts_with(PREFIX)));
}

/// The slot of each roster character: the leader slot 0, the other active members 1, 2 in their order.
pub fn slots_of(feed: &FormationRows) -> Vec<Option<usize>> {
    let mut out = vec![None; feed.members.len()];
    if feed.active.contains(&feed.leader) {
        if let Some(s) = out.get_mut(feed.leader) {
            *s = Some(0);
        }
    }
    let mut k = 1;
    for &i in &feed.active {
        if i == feed.leader || k > 2 {
            continue;
        }
        if let Some(s) = out.get_mut(i) {
            *s = Some(k);
            k += 1;
        }
    }
    out
}

/// The crown's centre over slot `k`'s top edge.
fn crown_pose(k: usize) -> Pose {
    let (x, y, w, _, _) = SLOTS[k];
    Pose { x: x + w / 2.0, y: y - 6.0, size: 1.0, alpha: 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camp::FormationMember;

    fn feed(active: &[usize], leader: usize, locked: &[usize], refused: Option<&str>, stamp: u32) -> FormationRows {
        let names = [("sora", "Sora"), ("donald", "Donald"), ("goofy", "Goofy")];
        FormationRows {
            members: names.iter().enumerate().map(|(i, (k, n))| FormationMember { key: k.to_string(), name: n.to_string(), level: 99, locked: locked.contains(&i) }).collect(),
            active: active.to_vec(),
            leader,
            refused: refused.map(str::to_string),
            stamp,
            ..FormationRows::default()
        }
    }

    #[test]
    fn the_leader_takes_slot_zero_and_the_others_follow_in_order() {
        assert_eq!(slots_of(&feed(&[0, 1, 2], 0, &[], None, 0)), vec![Some(0), Some(1), Some(2)]);
        assert_eq!(slots_of(&feed(&[0, 1, 2], 1, &[], None, 0)), vec![Some(1), Some(0), Some(2)]);
        assert_eq!(slots_of(&feed(&[0, 2], 2, &[], None, 0)), vec![Some(1), None, Some(0)]);
        assert_eq!(slots_of(&feed(&[1], 1, &[0, 2], None, 0)), vec![None, Some(0), None]);
    }

    #[test]
    fn a_join_grows_out_of_the_icon_and_a_leader_change_flies_the_crown() {
        let art = FormationArt::default();
        let mut scene = FormationScene::new(&art, &feed(&[0, 1], 0, &[], None, 0), 0);
        // Goofy out of the party: his bust is unseen, in his roster icon
        assert_eq!(scene.portrait_now(2).unwrap().alpha, 0.0);
        scene.set(&feed(&[0, 1, 2], 0, &[], None, 1), 2);
        let start = scene.portrait_now(2).unwrap();
        assert!(start.alpha < 0.05 && (start.size - FACE).abs() < 0.5, "starts as the icon: {start:?}");
        for _ in 0..8 {
            scene.step();
        }
        let mid = scene.portrait_now(2).unwrap();
        assert!(mid.size > FACE && mid.alpha > 0.2 && scene.animating(), "mid-flight: {mid:?}");
        for _ in 0..40 {
            scene.step();
        }
        let end = scene.portrait_now(2).unwrap();
        assert_eq!((end.x, end.y, end.size), (SLOTS[2].0 + INSET.0, SLOTS[2].1 + INSET.1, SLOTS[2].4));
        // Donald leads: he goes to slot 0, Sora to slot 1, the crown flies from Donald's old slot
        scene.set(&feed(&[0, 1, 2], 1, &[], None, 2), 1);
        assert_eq!((scene.slot(0), scene.slot(1), scene.slot(2)), (Some(1), Some(0), Some(2)));
        let c0 = scene.crown_now();
        assert!((c0.x - (SLOTS[1].0 + SLOTS[1].2 / 2.0)).abs() < 0.5, "the crown leaves slot 1: {c0:?}");
        for _ in 0..60 {
            scene.step();
        }
        let c1 = scene.crown_now();
        assert!((c1.x - (SLOTS[0].0 + SLOTS[0].2 / 2.0)).abs() < 0.5, "the crown is on slot 0: {c1:?}");
    }

    #[test]
    fn a_refusal_shakes_and_says_why_and_locked_members_are_silhouettes() {
        let art = FormationArt::default();
        let mut scene = FormationScene::new(&art, &feed(&[1], 1, &[0, 2], None, 0), 0);
        scene.set(&feed(&[1], 1, &[0, 2], Some("That character is not available."), 1), 0);
        assert_eq!(scene.refusal(), Some(("That character is not available.", 1.0)), "the reason is held, the help line flashes");
        assert!(scene.shake.is_some_and(|(i, _)| i == 0), "the refused icon shakes");
        for _ in 0..30 {
            scene.step();
        }
        let nodes = scene.nodes();
        assert!(nodes.iter().any(|n| n.name.as_deref() == Some("pf_icon0_q")), "a locked icon has its question mark");
        let face = nodes.iter().find(|n| n.name.as_deref() == Some("pf_icon0_face")).unwrap();
        assert_eq!(face.cx.mul[0], 0.0, "a locked face is a silhouette");
        for _ in 0..200 {
            scene.step();
        }
        assert!(scene.shake.is_none() && scene.message.is_none(), "both end");
    }
}
