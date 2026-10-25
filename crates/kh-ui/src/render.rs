//! Draws a [`Movie`]'s display tree into a [`Canvas`], with the rules of the offline compositor
//! `tools/lib/gfx_render.js` (masks by clip depth, colour transforms, blend modes add / screen /
//! multiply through a layer, text from the compacted fonts' outlines). Filters are ignored, as
//! there. Beyond the reference: script-drawn bitmap rectangles (`BitmapNumber`), script-added
//! children (a loaded face movie), `TextFieldAdjust` squeezing, pad-button icon placeholders.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::def::{Align, BitmapFill, Edge, Font, Paint, Seg, Shape, TextDef};
use crate::geom::{Cx, Matrix, Rect};
use crate::player::{GfxRect, Kind, Movie, Node, Sprite, TextNode};
use crate::raster::{self, Canvas, Coverage, Mask, PaintFn};

/// The UI fonts by class name; `default` stands in for a class that is not loaded (the
/// reference uses `$KH2Menu`).
#[derive(Clone, Default)]
pub struct Fonts {
    pub map: HashMap<String, Arc<Font>>,
    pub default: Option<Arc<Font>>,
    /// The icons of text (`<icon(Triangle)>`): `UI/IconText`, when the pack holds them.
    pub icons: Option<Arc<IconSet>>,
}

/// The icon textures of `UI/IconText` (pack file `data/icon_text.json`): the category icons and
/// the pad-button set of the PC build.
#[derive(Debug, Default)]
pub struct IconSet {
    pub textures: Vec<Arc<crate::def::Texture>>,
    /// Lower-case name -> (texture, x, y, w, h) in texture pixels.
    pub icons: HashMap<String, (usize, f32, f32, f32, f32)>,
}

/// (IMPL_ER23B) WHICH PAD SET the button icons of text come from: the folder of `/Game/UI/IconText/%s/IconText_pf`.
/// The game chooses it at run time (0x141595EA0: its setting auto / xboxone / ps4 / windows; "auto" asks the connected
/// device, 0x141595160, whose default is "xboxone") [decompiled C only]; the port's pad is an XInput pad, so the
/// default here is "xboxone" (A / B / X / Y, LB / RB / LT / RT). "windows" = the generic four-dot set drawn before.
/// Set BEFORE the fonts are loaded (test app: KH_UI_GLYPHS; Elden Ring: `hud.glyphs`).
static PAD_GLYPHS: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn set_pad_glyphs(folder: &str) {
    if let Ok(mut guard) = PAD_GLYPHS.lock() {
        *guard = Some(folder.to_string());
    }
}

pub fn pad_glyphs() -> String {
    PAD_GLYPHS.lock().ok().and_then(|g| g.clone()).or_else(|| std::env::var("KH_UI_GLYPHS").ok()).filter(|s| !s.is_empty()).unwrap_or_else(|| "xboxone".to_string())
}

impl IconSet {
    pub fn load(pack: &mut crate::def::Pack) -> Option<IconSet> {
        let j = pack.data("icon_text").ok()?;
        // (IMPL_ER23B) the chosen pad set: its picture stands for the table's "pad" texture; a table of its own wins
        let chosen = pad_glyphs();
        let pad_key = j["pad_sets"][chosen.as_str()].as_str().unwrap_or("pad").to_string();
        let mut set = IconSet::default();
        let mut index = HashMap::new();
        for (key, file) in j["textures"].as_object()? {
            let texture = pack.texture(file.as_str()?).ok()?;
            index.insert(key.clone(), set.textures.len());
            set.textures.push(texture);
        }
        for (name, icon) in j["icons"].as_object()? {
            let n = |k: &str| icon[k].as_f64().unwrap_or(0.0) as f32;
            let Some(tex) = icon["tex"].as_str().and_then(|t| index.get(if t == "pad" { pad_key.as_str() } else { t }).or_else(|| index.get(t))) else { continue };
            set.icons.insert(name.to_lowercase(), (*tex, n("x"), n("y"), n("w"), n("h")));
        }
        for (name, icon) in j["icons_of_set"][chosen.as_str()].as_object().into_iter().flatten() {
            let n = |k: &str| icon[k].as_f64().unwrap_or(0.0) as f32;
            let Some(tex) = icon["tex"].as_str().and_then(|t| index.get(t)) else { continue };
            set.icons.insert(name.to_lowercase(), (*tex, n("x"), n("y"), n("w"), n("h")));
        }
        Some(set)
    }
}

/// A piece of a text line: the game's text markup is `<icon(Name)>`, `<color(name)>` ...
/// `<color>` and `<br>` (docs/sora-ui-spec.md 10; the button guides and help lines use it).
#[derive(Clone, Debug, PartialEq)]
pub enum Run {
    /// Text and whether it is inside a `<color(...)>` span.
    Text(String, bool),
    Icon(String),
}

/// One line of marked-up text as runs.
pub fn parse_markup(line: &str) -> Vec<Run> {
    let mut runs = Vec::new();
    let (mut rest, mut strong, mut text) = (line, false, String::new());
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<icon(") {
            if let Some(end) = after.find(")>") {
                if !text.is_empty() {
                    runs.push(Run::Text(std::mem::take(&mut text), strong));
                }
                runs.push(Run::Icon(after[..end].to_string()));
                rest = &after[end + 2..];
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix("<color") {
            if let Some(end) = after.find('>') {
                if !text.is_empty() {
                    runs.push(Run::Text(std::mem::take(&mut text), strong));
                }
                strong = end > 0;
                rest = &after[end + 1..];
                continue;
            }
        }
        let c = rest.chars().next().unwrap();
        text.push(c);
        rest = &rest[c.len_utf8()..];
    }
    if !text.is_empty() {
        runs.push(Run::Text(text, strong));
    }
    runs
}

/// The height an inline icon is drawn at: the 30-pixel icons of the table at the 25-pixel menu
/// text (LOCAL: the native scale rule of the icon font was not read).
/// (BN) `IconTextData`: the cell size of the category icons (32 x 32) and their `VSpace` (-7), pack data/icon_text.json.
pub const ICON_TEXT_NATIVE: f32 = 32.0;
pub const ICON_TEXT_VSPACE: f32 = -7.0;

pub fn icon_height(size: f32) -> f32 {
    size * 1.2
}

/// Width of one line of marked-up text.
pub fn markup_width(fonts: &Fonts, font: &Font, line: &str, size: f32) -> f32 {
    if !line.contains('<') {
        return font.text_width(line, size);
    }
    parse_markup(line)
        .iter()
        .map(|run| match run {
            Run::Text(t, _) => font.text_width(t, size),
            Run::Icon(name) => {
                let h = icon_height(size);
                fonts.icons.as_ref().and_then(|set| set.icons.get(&name.to_lowercase())).map_or(h, |i| h * i.3 / i.4.max(1.0))
            }
        })
        .sum()
}

/// `wordWrap`: breaks the lines of a marked-up text at spaces so that each fits `width`; a
/// colour span that is open at a break is closed on the line and opened again on the next.
pub fn wrap_text(fonts: &Fonts, font: &Font, text: &str, size: f32, width: f32) -> String {
    let mut out = String::new();
    for (n, line) in text.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        if markup_width(fonts, font, line, size) <= width {
            out.push_str(line);
            continue;
        }
        let mut current = String::new();
        for word in line.split(' ') {
            let candidate = if current.is_empty() { word.to_string() } else { format!("{current} {word}") };
            if !current.is_empty() && markup_width(fonts, font, &candidate, size) > width {
                // The colour span still open at the break.
                let open = current.rfind("<color(").filter(|at| !current[*at..].contains("<color>")).map(|at| current[at..current[at..].find('>').map_or(current.len(), |e| at + e + 1)].to_string());
                out.push_str(&current);
                out.push('\n');
                current = match open {
                    Some(tag) => format!("{tag}{word}"),
                    None => word.to_string(),
                };
            } else {
                current = candidate;
            }
        }
        out.push_str(&current);
    }
    out
}

/// (IMPL_ER42) A Flash text field shows nothing outside its bounds (and the HUD's dirty rectangles cover only the
/// field's bounds: text drawn below them stays on screen when a shorter text follows, seen in the harness). A text for a
/// word-wrapped field is wrapped as the field wraps it ([`wrap_text`], box = field width - 4) and cut to the lines that fit the
/// field's height (first baseline at top + 2 + ascent, then one line height each); when lines were cut, the last kept
/// line ends in "...". A text that fits, a field without word wrap, or no font: unchanged.
pub fn fit_text(fonts: &Fonts, def: &TextDef, text: &str) -> String {
    let Some(font) = fonts.get(def.font_class.as_deref()) else { return text.to_string() };
    if !def.word_wrap || font.units_per_em <= 0.0 || def.size <= 0.0 {
        return text.to_string();
    }
    let sc = def.size / font.units_per_em;
    let line_h = (font.ascent + font.descent + font.leading) * sc + def.leading;
    if line_h <= 0.0 {
        return text.to_string();
    }
    let room = def.bounds.y1 - def.bounds.y0 - 2.0 - (font.ascent + font.descent) * sc;
    let max_lines = if room < 0.0 { 1 } else { (room / line_h).floor() as usize + 1 };
    let box_w = def.bounds.x1 - def.bounds.x0 - 4.0;
    let wrapped = wrap_text(fonts, font, text, def.size, box_w);
    let lines: Vec<&str> = wrapped.split('\n').collect();
    if lines.len() <= max_lines {
        return text.to_string();
    }
    let mut kept: Vec<String> = lines[..max_lines].iter().map(|l| l.to_string()).collect();
    while kept.len() > 1 && kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    let last = kept.pop().unwrap_or_default();
    let mut words: Vec<&str> = last.split(' ').collect();
    let ended = loop {
        let candidate = format!("{}...", words.join(" ").trim_end().trim_end_matches('.'));
        if words.len() <= 1 || markup_width(fonts, font, &candidate, def.size) <= box_w {
            break candidate;
        }
        words.pop();
    };
    kept.push(ended);
    kept.join("\n")
}

/// The colour of a `<color(strong)>` span (LOCAL: the native colour table of the text markup was
/// not read; the game shows these spans in yellow).
const STRONG: [f32; 4] = [255.0, 224.0, 96.0, 255.0];

impl Fonts {
    /// The four fonts of the pack.
    pub fn load(pack: &mut crate::def::Pack) -> Result<Fonts, crate::def::PackError> {
        let mut fonts = Fonts::default();
        for class in ["$KH2Menu", "$jimakuNE", "$JimakuJP", "$sougei"] {
            fonts.map.insert(class.to_string(), pack.font(class)?);
        }
        fonts.default = fonts.map.get("$KH2Menu").cloned();
        fonts.icons = IconSet::load(pack).map(Arc::new);
        Ok(fonts)
    }

    pub fn get(&self, class: Option<&str>) -> Option<&Arc<Font>> {
        class.and_then(|c| self.map.get(c)).or(self.default.as_ref())
    }
}

/// What a draw call did (for tests and the comparison report).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub shapes: u32,
    pub texts: u32,
    pub masks: u32,
    pub layers: u32,
    pub gfx_rects: u32,
    /// Nanoseconds by kind of work (profiling).
    pub ns_mask: u64,
    pub ns_layer: u64,
    pub ns_text: u64,
    pub ns_cover: u64,
    pub ns_fill: u64,
}

pub struct Renderer {
    pub fonts: Fonts,
    pub stats: Stats,
    cov: Coverage,
    xs: Vec<f32>,
    edges: Vec<[f32; 4]>,
    /// (IMPL_ER21) The device rectangle (whole pixels) drawing is held to while one slice of a scale-9 sprite is drawn.
    slice: Option<Rect>,
    /// (HB) A SPRITE used as a mask is being drawn: everything under it gives coverage only (see `set_solid_masks`).
    in_mask: bool,
}

/// (HB) MASKS ARE COVERAGE ONLY: a mask (clip depth) shows what is under its SHAPES, whatever their colour, alpha or
/// colour transform (Flash / Scaleform: the mask is its geometry, drawn into the stencil). A shape mask was always drawn
/// so; a SPRITE mask drew its shapes with their own paint, so a half-transparent mask fill let the masked objects through
/// at that alpha. KH3's status HUD masks Sora's HP ring (`pcHpGg`: #137 > #136, solid blue at alpha 127) and the focus
/// gauge (`pcExGg`: #78 > #77, black at alpha 127) with such sprites: the gauges came out at HALF strength over the red
/// lag gauge / the dark track (dull dark green and brown-orange), while the friends' rings (mask fill alpha 255) were
/// vivid. On by default; off = as before (Elden Ring: `hud.solid_masks = 0`; the test app: KH_UI_SOLID_MASKS=0).
static SOLID_MASKS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_solid_masks(on: bool) {
    SOLID_MASKS.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn solid_masks_on() -> bool {
    SOLID_MASKS.load(std::sync::atomic::Ordering::Relaxed)
}

/// (IMPL_ER21) SCALING GRIDS (`DefineScalingGrid`): on by default; off = every scaled sprite is stretched whole, as
/// before this switch existed (test app: KH_UI_SCALE9=0; Elden Ring: `hud.scale9 = 0`).
static SCALE9: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_scale9(on: bool) {
    SCALE9.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn scale9_on() -> bool {
    SCALE9.load(std::sync::atomic::Ordering::Relaxed)
}

/// (IMPL_ER21) ONE AXIS OF A SCALING GRID, in device pixels: the sprite's extent `b0..b1` (its own space) is placed by
/// `dev = k * x + t`; `p` = device pixels per unit of the PARENT's space (the size the caps keep); `g0..g1` = the grid.
/// Answers up to three pieces (from, to, scale, offset, device limit from, to): the caps at the parent's scale, the
/// middle stretched between them; the two joints lie on whole device pixels. `None`: nothing to slice on this axis
/// (no cap, or the scaled extent is smaller than the caps): the plain stretch.
pub fn scale9_axis(b0: f32, b1: f32, g0: f32, g1: f32, k: f32, t: f32, p: f32) -> Option<Vec<[f32; 6]>> {
    let (g0, g1) = (g0.clamp(b0, b1), g1.clamp(b0, b1));
    if !(k > 0.0 && p > 0.0 && g1 > g0 && b1 > b0) || (g0 - b0 <= 1e-3 && b1 - g1 <= 1e-3) {
        return None;
    }
    let (d0, d1) = (k * b0 + t, k * b1 + t);
    let (s0, s1) = ((d0 + (g0 - b0) * p).round(), (d1 - (b1 - g1) * p).round());
    if !(s1 > s0 && s0 >= d0 - 0.5 && s1 <= d1 + 0.5) {
        return None;
    }
    let mut out = Vec::with_capacity(3);
    if g0 - b0 > 1e-3 {
        let kl = (s0 - d0) / (g0 - b0);
        out.push([b0, g0, kl, d0 - kl * b0, f32::NEG_INFINITY, s0]);
    }
    let km = (s1 - s0) / (g1 - g0);
    out.push([g0, g1, km, s0 - km * g0, s0, s1]);
    if b1 - g1 > 1e-3 {
        let kr = (d1 - s1) / (b1 - g1);
        out.push([g1, b1, kr, s1 - kr * g1, s1, f32::INFINITY]);
    }
    Some(out)
}

/// A text of the form `<icon(Name)>` (pad button icons; the game substitutes textures of
/// `UI/IconText`, which the pack does not hold).
pub fn icon_name(text: &str) -> Option<&str> {
    text.trim().strip_prefix("<icon(")?.strip_suffix(")>")
}

fn order(s: &Sprite) -> Vec<&Node> {
    let mut list: Vec<&Node> = s.children.iter().collect();
    if list.iter().any(|n| n.z.is_some()) {
        // Stable: timeline depths first, then script-moved objects in the order of the moves.
        list.sort_by_key(|n| (n.z.is_some(), n.z.unwrap_or(n.depth)));
    }
    list
}

impl Renderer {
    pub fn new(fonts: Fonts) -> Renderer {
        Renderer { fonts, stats: Stats::default(), cov: Coverage::default(), xs: Vec::new(), edges: Vec::new(), slice: None, in_mask: false }
    }

    /// Where drawing may land: the canvas, the mask's box, and the scale-9 slice being drawn.
    fn limit_of(&self, canvas: &Canvas, clip: Option<&Mask>) -> Rect {
        let limit = clip.map_or(canvas.rect(), |c| c.rect().intersect(&canvas.rect()));
        match &self.slice {
            Some(slice) => limit.intersect(slice),
            None => limit,
        }
    }

    /// (IMPL_ER21) A SPRITE WITH A SCALING GRID, SCALED: drawn in up to nine slices, each with its own placement (the
    /// corners at the parent's scale, the edges stretched along one axis, the middle along both), each held to its
    /// device rectangle. `m` = the parent's placement, `o` = the sprite's node. False: not such a sprite (the caller
    /// draws it the plain way).
    fn draw_scale9(&mut self, canvas: &mut Canvas, o: &Node, s: &Sprite, m: &Matrix, cx: &Cx, clip: Option<&Mask>) -> bool {
        let Some(grid) = s.def.scale9 else { return false };
        let own = &o.matrix;
        if !scale9_on() || self.slice.is_some() || !m.is_axis_aligned() || own.b.abs() > 1e-4 || own.c.abs() > 1e-4 || !(own.a > 0.0 && own.d > 0.0 && m.a > 0.0 && m.d > 0.0) {
            return false;
        }
        let (scaled_x, scaled_y) = ((own.a - 1.0).abs() > 0.01, (own.d - 1.0).abs() > 0.01);
        if !scaled_x && !scaled_y {
            return false;
        }
        let b = self.sprite_bounds(s, &Matrix::IDENTITY, false, &Cx::IDENTITY);
        if b.is_empty() {
            return false;
        }
        let mm = m.mul(own);
        let whole = |b0: f32, b1: f32, k: f32, t: f32| vec![[b0, b1, k, t, f32::NEG_INFINITY, f32::INFINITY]];
        let xs = scaled_x.then(|| scale9_axis(b.x0, b.x1, grid.x0, grid.x1, mm.a, mm.tx, m.a)).flatten();
        let ys = scaled_y.then(|| scale9_axis(b.y0, b.y1, grid.y0, grid.y1, mm.d, mm.ty, m.d)).flatten();
        if xs.is_none() && ys.is_none() {
            return false;
        }
        let (xs, ys) = (xs.unwrap_or_else(|| whole(b.x0, b.x1, mm.a, mm.tx)), ys.unwrap_or_else(|| whole(b.y0, b.y1, mm.d, mm.ty)));
        for y in &ys {
            for x in &xs {
                self.slice = Some(Rect::new(x[4], y[4], x[5], y[5]));
                let piece = Matrix { a: x[2], b: 0.0, c: 0.0, d: y[2], tx: x[3], ty: y[3] };
                self.draw_sprite(canvas, s, &piece, cx, clip);
            }
        }
        self.slice = None;
        true
    }

    /// Draws the whole movie; `root` maps stage pixels to device pixels.
    pub fn draw(&mut self, canvas: &mut Canvas, movie: &Movie, root: &Matrix) {
        self.draw_sprite(canvas, &movie.root, root, &Cx::IDENTITY, None);
    }

    /// Device-space bounding box of everything the movie would draw.
    pub fn bounds(&self, movie: &Movie, root: &Matrix) -> Rect {
        self.sprite_bounds(&movie.root, root, false, &Cx::IDENTITY)
    }

    fn sprite_bounds(&self, s: &Sprite, m: &Matrix, as_mask: bool, cx: &Cx) -> Rect {
        let mut r = Rect::EMPTY;
        for g in &s.gfx {
            r = r.union(&Rect::new(g.x, g.y, g.x + g.w, g.y + g.h).transform(m));
        }
        let list = order(s);
        let mut i = 0;
        while i < list.len() {
            let o = list[i];
            if o.clip != 0 {
                // What a mask lets through is inside the mask.
                let mask = self.node_bounds(o, m, true, &Cx::IDENTITY);
                let mut inner = Rect::EMPTY;
                let mut j = i + 1;
                while j < list.len() && list[j].depth <= o.clip && list[j].z.is_none() {
                    if list[j].clip == 0 {
                        inner = inner.union(&self.node_bounds(list[j], m, false, cx));
                    }
                    j += 1;
                }
                if !mask.is_empty() && !inner.is_empty() {
                    let both = inner.intersect(&mask);
                    if !both.is_empty() {
                        r = r.union(&both);
                    }
                }
                i = j;
                continue;
            }
            r = r.union(&self.node_bounds(o, m, as_mask, cx));
            i += 1;
        }
        for o in &s.extra {
            r = r.union(&self.node_bounds(o, m, as_mask, cx));
        }
        r
    }

    fn node_bounds(&self, o: &Node, m: &Matrix, as_mask: bool, cx: &Cx) -> Rect {
        let cc = if as_mask { Cx::IDENTITY } else { cx.concat(&o.cx) };
        if !as_mask && (!o.is_visible() || cc.is_invisible()) {
            return Rect::EMPTY;
        }
        let mm = m.mul(&o.matrix);
        match &o.kind {
            Kind::Shape(s) => s.bounds.rect().transform(&mm),
            Kind::Morph(a, b) => a.bounds.rect().union(&b.bounds.rect()).transform(&mm),
            Kind::Image(i) => Rect::new(0.0, 0.0, i.size.0, i.size.1).transform(&mm),
            Kind::Text(t) => {
                let (tm, width) = text_matrix(t, &mm, m);
                let b = &t.def.bounds;
                Rect::new(b.x0 - 4.0, b.y0 - 4.0, b.x0 + width.max(b.x1 - b.x0) + 4.0, b.y1 + 8.0).transform(&tm)
            }
            Kind::Sprite(s) => self.sprite_bounds(s, &mm, false, &cc),
            Kind::Missing => Rect::EMPTY,
        }
    }

    fn draw_sprite(&mut self, canvas: &mut Canvas, s: &Sprite, m: &Matrix, cx: &Cx, clip: Option<&Mask>) {
        for g in &s.gfx {
            self.draw_gfx(canvas, g, m, cx, clip);
        }
        let list = order(s);
        let mut i = 0;
        while i < list.len() {
            let o = list[i];
            if o.clip != 0 {
                // A mask: its coverage applies to the following depths up to the clip depth.
                let mut j = i + 1;
                while j < list.len() && list[j].depth <= o.clip && list[j].z.is_none() {
                    j += 1;
                }
                let limit = self.limit_of(canvas, clip);
                // Only where the masked objects can draw does the mask matter.
                let mut content = Rect::EMPTY;
                for q in &list[i + 1..j] {
                    if q.clip == 0 {
                        content = content.union(&self.node_bounds(q, m, false, cx));
                    }
                }
                let mb = self.node_bounds(o, m, true, &Cx::IDENTITY).intersect(&limit).intersect(&content);
                if !mb.is_empty() && j > i + 1 {
                    let t0 = std::time::Instant::now();
                    let (x0, y0) = (mb.x0.floor() as i32, mb.y0.floor() as i32);
                    let (w, h) = ((mb.x1.ceil() as i32 - x0) as usize, (mb.y1.ceil() as i32 - y0) as usize);
                    let mut mask = Mask { x0, y0, w, h, alpha: Vec::new() };
                    match &o.kind {
                        Kind::Shape(shape) => {
                            // A shape mask is its coverage (source-over of white): one channel.
                            mask.alpha.resize(w * h, 0.0);
                            let mm = m.mul(&o.matrix);
                            let region = mask.rect();
                            for fill in &shape.fills {
                                let rect = if mm.is_axis_aligned() { rect_of(&fill.edges) } else { None };
                                let ok = match rect {
                                    Some(r) => raster::coverage_rect(&r.transform(&mm), &region, &mut self.cov),
                                    None => {
                                        self.device_edges(&fill.edges, &mm);
                                        raster::coverage(&self.edges, &region, &mut self.cov, &mut self.xs)
                                    }
                                };
                                if !ok {
                                    continue;
                                }
                                let c = &self.cov;
                                for yy in 0..c.h {
                                    let row = (c.y0 - y0) as usize + yy;
                                    let at = row * w + (c.x0 - x0) as usize;
                                    for (a, k) in mask.alpha[at..at + c.w].iter_mut().zip(&c.cov[yy * c.w..(yy + 1) * c.w]) {
                                        let k = k.clamp(0.0, 1.0);
                                        *a = k + *a * (1.0 - k);
                                    }
                                }
                            }
                            if let Some(outer) = clip {
                                for (k, a) in mask.alpha.iter_mut().enumerate() {
                                    *a = a.min(1.0) * outer.at(x0 + (k % w) as i32, y0 + (k / w) as i32);
                                }
                            }
                        }
                        _ => {
                            let mut mk = Canvas::region(x0, y0, w, h);
                            // (HB) everything under a sprite mask gives coverage only
                            let was = self.in_mask;
                            self.in_mask = was || solid_masks_on();
                            self.draw_node(&mut mk, o, m, &Cx::IDENTITY, None, true);
                            self.in_mask = was;
                            mask.alpha.reserve(w * h);
                            for (k, p) in mk.px.chunks_exact(4).enumerate() {
                                let outer = clip.map_or(1.0, |c| c.at(x0 + (k % w) as i32, y0 + (k / w) as i32));
                                mask.alpha.push(p[3].min(1.0) * outer);
                            }
                        }
                    }
                    self.stats.masks += 1;
                    self.stats.ns_mask += t0.elapsed().as_nanos() as u64;
                    for q in &list[i + 1..j] {
                        if q.clip == 0 {
                            self.draw_node(canvas, q, m, cx, Some(&mask), false);
                        }
                    }
                }
                i = j;
                continue;
            }
            self.draw_node(canvas, o, m, cx, clip, false);
            i += 1;
        }
        for o in &s.extra {
            self.draw_node(canvas, o, m, cx, clip, false);
        }
    }

    fn draw_node(&mut self, canvas: &mut Canvas, o: &Node, m: &Matrix, cx: &Cx, clip: Option<&Mask>, as_mask: bool) {
        if !as_mask && !o.is_visible() {
            return;
        }
        // (HB) under a sprite mask: a visible descendant gives its shapes' coverage (no colour, alpha or blend)
        let as_mask = as_mask || self.in_mask;
        let mm = m.mul(&o.matrix);
        let cc = if as_mask { Cx::IDENTITY } else { cx.concat(&o.cx) };
        if !as_mask && cc.is_invisible() {
            return;
        }
        let blend = if as_mask { 0 } else { o.blend };
        if matches!(blend, 8 | 4 | 3) {
            let limit = self.limit_of(canvas, clip);
            let b = self.node_bounds(o, m, false, cx).intersect(&limit);
            if b.is_empty() {
                return;
            }
            let (x0, y0) = (b.x0.floor() as i32, b.y0.floor() as i32);
            let t0 = std::time::Instant::now();
            let mut layer = Canvas::region(x0, y0, (b.x1.ceil() as i32 - x0) as usize, (b.y1.ceil() as i32 - y0) as usize);
            self.draw_kind(&mut layer, o, &mm, m, &cc, None, false);
            raster::blend_layer(canvas, &layer, blend, clip);
            self.stats.ns_layer += t0.elapsed().as_nanos() as u64;
            self.stats.layers += 1;
            return;
        }
        if !as_mask {
            if let Kind::Sprite(s) = &o.kind {
                if s.def.scale9.is_some() && self.draw_scale9(canvas, o, s, m, &cc, clip) {
                    return;
                }
            }
        }
        self.draw_kind(canvas, o, &mm, m, &cc, clip, as_mask);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_kind(&mut self, canvas: &mut Canvas, o: &Node, mm: &Matrix, parent: &Matrix, cx: &Cx, clip: Option<&Mask>, as_mask: bool) {
        match &o.kind {
            Kind::Sprite(s) => self.draw_sprite(canvas, s, mm, cx, clip),
            Kind::Shape(s) => self.draw_shape(canvas, s, mm, cx, clip, as_mask),
            Kind::Morph(a, b) => {
                let shape = morph(a, b, o.ratio);
                self.draw_shape(canvas, &shape, mm, cx, clip, as_mask);
            }
            Kind::Image(i) => {
                let g = GfxRect { x: 0.0, y: 0.0, w: i.size.0, h: i.size.1, fill: i.fill.clone() };
                if as_mask {
                    self.fill_rect_or_quad(canvas, &Rect::new(0.0, 0.0, g.w, g.h), mm, &PaintFn::Solid([1.0; 4]), clip);
                } else {
                    self.draw_gfx(canvas, &g, mm, cx, clip);
                }
            }
            Kind::Text(t) => self.draw_text(canvas, t, mm, parent, cx, clip),
            Kind::Missing => {}
        }
    }

    fn bitmap_paint<'a>(fill: &'a BitmapFill, m: &Matrix, cx: &Cx) -> PaintFn<'a> {
        PaintFn::Bitmap { inv: m.mul(&fill.matrix).inverse(), scale: fill.scale, texture: &fill.texture, repeat: fill.repeat, smooth: fill.smooth, cx: (!cx.is_identity()).then_some(*cx) }
    }

    fn paint_of<'a>(paint: &'a Paint, m: &Matrix, cx: &Cx) -> PaintFn<'a> {
        match paint {
            Paint::None => PaintFn::Solid([0.0; 4]),
            Paint::Solid(c) => PaintFn::Solid(raster::solid(*c, cx)),
            Paint::Gradient { radial, matrix, stops } => PaintFn::Gradient { inv: m.mul(matrix).inverse(), radial: *radial, stops: stops.iter().map(|(r, c)| (*r, raster::solid(*c, cx))).collect() },
            Paint::Bitmap(fill) => Self::bitmap_paint(fill, m, cx),
        }
    }

    /// Fills the shape-space rectangle `r` under `m`.
    fn fill_rect_or_quad(&mut self, canvas: &mut Canvas, r: &Rect, m: &Matrix, paint: &PaintFn, clip: Option<&Mask>) {
        let limit = self.limit_of(canvas, clip);
        if limit.is_empty() {
            return;
        }
        let ok = if m.is_axis_aligned() {
            raster::coverage_rect(&r.transform(m), &limit, &mut self.cov)
        } else {
            self.edges.clear();
            let p = [m.apply(r.x0, r.y0), m.apply(r.x1, r.y0), m.apply(r.x1, r.y1), m.apply(r.x0, r.y1)];
            for k in 0..4 {
                let (a, b) = (p[k], p[(k + 1) % 4]);
                self.edges.push([a.0, a.1, b.0, b.1]);
            }
            raster::coverage(&self.edges, &limit, &mut self.cov, &mut self.xs)
        };
        if ok {
            raster::fill(canvas, &self.cov, paint, clip);
        }
    }

    fn draw_gfx(&mut self, canvas: &mut Canvas, g: &GfxRect, m: &Matrix, cx: &Cx, clip: Option<&Mask>) {
        self.stats.gfx_rects += 1;
        // (HB) a drawn rectangle under a sprite mask gives coverage only
        let paint = if self.in_mask { PaintFn::Solid([1.0; 4]) } else { Self::bitmap_paint(&g.fill, m, cx) };
        self.fill_rect_or_quad(canvas, &Rect::new(g.x, g.y, g.x + g.w, g.y + g.h), m, &paint, clip);
    }

    fn device_edges(&mut self, edges: &[Edge], m: &Matrix) {
        self.edges.clear();
        for e in edges {
            match e {
                Edge::Line(l) => {
                    let (a, b) = (m.apply(l[0], l[1]), m.apply(l[2], l[3]));
                    self.edges.push([a.0, a.1, b.0, b.1]);
                }
                Edge::Curve(c) => raster::flatten(m.apply(c[0], c[1]), m.apply(c[2], c[3]), m.apply(c[4], c[5]), &mut self.edges),
            }
        }
    }

    fn draw_shape(&mut self, canvas: &mut Canvas, shape: &Shape, m: &Matrix, cx: &Cx, clip: Option<&Mask>, as_mask: bool) {
        self.stats.shapes += 1;
        let limit = self.limit_of(canvas, clip);
        if limit.is_empty() || shape.bounds.rect().transform(m).intersect(&limit).is_empty() {
            return;
        }
        let white = PaintFn::Solid([1.0; 4]);
        for fill in &shape.fills {
            if matches!(fill.paint, Paint::None) && !as_mask {
                continue;
            }
            // An axis-aligned rectangle under an axis-aligned matrix needs no edge walking.
            let rect = if m.is_axis_aligned() { rect_of(&fill.edges) } else { None };
            let t0 = std::time::Instant::now();
            let ok = match rect {
                Some(r) => raster::coverage_rect(&r.transform(m), &limit, &mut self.cov),
                None => {
                    self.device_edges(&fill.edges, m);
                    raster::coverage(&self.edges, &limit, &mut self.cov, &mut self.xs)
                }
            };
            self.stats.ns_cover += t0.elapsed().as_nanos() as u64;
            if !ok {
                continue;
            }
            let t0 = std::time::Instant::now();
            if as_mask {
                raster::fill(canvas, &self.cov, &white, clip);
            } else {
                raster::fill(canvas, &self.cov, &Self::paint_of(&fill.paint, m, cx), clip);
            }
            self.stats.ns_fill += t0.elapsed().as_nanos() as u64;
        }
        for stroke in &shape.strokes {
            // Union of the segments' quads through a max-coverage buffer, as the reference does.
            let wpx = (stroke.width * (m.a * m.d - m.b * m.c).abs().sqrt()).max(1.0) / 2.0;
            self.device_edges(&stroke.edges, m);
            let segments = std::mem::take(&mut self.edges);
            let mut all = Coverage::default();
            let mut quads: Vec<[[f32; 4]; 4]> = Vec::with_capacity(segments.len());
            for e in &segments {
                let (dx, dy) = (e[2] - e[0], e[3] - e[1]);
                let n = dx.hypot(dy).max(f32::MIN_POSITIVE);
                let n = if n > 0.0 { n } else { 1.0 };
                let (nx, ny, ex, ey) = (-dy / n * wpx, dx / n * wpx, dx / n * wpx, dy / n * wpx);
                let a = (e[0] - ex + nx, e[1] - ey + ny);
                let b = (e[2] + ex + nx, e[3] + ey + ny);
                let c = (e[2] + ex - nx, e[3] + ey - ny);
                let d = (e[0] - ex - nx, e[1] - ey - ny);
                quads.push([[a.0, a.1, b.0, b.1], [b.0, b.1, c.0, c.1], [c.0, c.1, d.0, d.1], [d.0, d.1, a.0, a.1]]);
            }
            let flat: Vec<[f32; 4]> = quads.iter().flatten().copied().collect();
            if raster::coverage(&flat, &limit, &mut all, &mut self.xs) {
                all.cov.iter_mut().for_each(|v| *v = 0.0);
                for q in &quads {
                    if !raster::coverage(q, &limit, &mut self.cov, &mut self.xs) {
                        continue;
                    }
                    for yy in 0..self.cov.h {
                        for xx in 0..self.cov.w {
                            let (x, y) = (self.cov.x0 + xx as i32 - all.x0, self.cov.y0 + yy as i32 - all.y0);
                            if x < 0 || y < 0 || x as usize >= all.w || y as usize >= all.h {
                                continue;
                            }
                            let k = y as usize * all.w + x as usize;
                            all.cov[k] = all.cov[k].max(self.cov.cov[yy * self.cov.w + xx]);
                        }
                    }
                }
                let paint = if as_mask { PaintFn::Solid([1.0; 4]) } else { PaintFn::Solid(raster::solid(stroke.color, cx)) };
                raster::fill(canvas, &all, &paint, clip);
            }
            self.edges = segments;
        }
    }

    fn draw_text(&mut self, canvas: &mut Canvas, t: &TextNode, mm: &Matrix, parent: &Matrix, cx: &Cx, clip: Option<&Mask>) {
        let text = t.shown();
        if text.is_empty() {
            return;
        }
        let def: &TextDef = &t.def;
        let (m, width) = text_matrix(t, mm, parent);
        let limit = self.limit_of(canvas, clip);
        if limit.is_empty() {
            return;
        }
        // (RC) A field with KH3's own inline-image rule (`icon_native`) lays even a lone icon out on the text line, as the
        // game's `<img src='img://icontext:%s' vspace='%d'>` (0x141578970) does; the others keep the centred stand-in.
        if let Some(icon) = icon_name(text).filter(|_| !t.icon_native) {
            self.draw_icon(canvas, icon, def, &m, cx, clip);
            return;
        }
        let Some(font) = self.fonts.get(def.font_class.as_deref()).cloned() else { return };
        self.stats.texts += 1;
        let t0 = std::time::Instant::now();
        let sc = def.size / font.units_per_em;
        let box_w = width - 4.0;
        let mut line_y = def.bounds.y0 + 2.0 + font.ascent * sc;
        let wrapped;
        let text = if def.word_wrap {
            wrapped = wrap_text(&self.fonts, &font, text, def.size, box_w);
            wrapped.as_str()
        } else {
            text
        };
        for line in text.split('\n') {
            let line = line.trim_end_matches('\r');
            let runs = if line.contains('<') { parse_markup(line) } else { vec![Run::Text(line.to_string(), false)] };
            let icon_h = icon_height(def.size);
            let widths: Vec<f32> = runs
                .iter()
                .map(|run| match run {
                    Run::Text(t, _) => t.chars().filter_map(|c| font.glyph(c as u32)).map(|g| g.advance * sc).sum(),
                    Run::Icon(name) if t.icon_native => self.fonts.icons.as_ref().and_then(|set| set.icons.get(&name.to_lowercase())).map_or(ICON_TEXT_NATIVE, |i| i.3), // (BN)
                    Run::Icon(name) => self.fonts.icons.as_ref().and_then(|set| set.icons.get(&name.to_lowercase())).map_or(icon_h, |i| icon_h * i.3 / i.4.max(1.0)),
                })
                .collect();
            let wd: f32 = widths.iter().sum();
            let mut x0 = def.bounds.x0 + 2.0;
            match def.align {
                Align::Right => x0 += box_w - wd,
                Align::Center => x0 += (box_w - wd) / 2.0,
                Align::Left => {}
            }
            for (run, width) in runs.iter().zip(&widths) {
                match run {
                    Run::Icon(name) if t.icon_native => {
                        // (BN) the table's size, its cell reaching -VSpace under the baseline
                        let h = self.fonts.icons.as_ref().and_then(|set| set.icons.get(&name.to_lowercase())).map_or(ICON_TEXT_NATIVE, |i| i.4);
                        let top = line_y - h - ICON_TEXT_VSPACE;
                        self.draw_icon_at(canvas, name, def, (x0, top, *width, h), &m, cx, clip);
                    }
                    Run::Icon(name) => {
                        // Centred on the line's x-height band.
                        let top = line_y - font.ascent * sc * 0.42 - icon_h / 2.0;
                        self.draw_icon_at(canvas, name, def, (x0, top, *width, icon_h), &m, cx, clip);
                    }
                    Run::Text(t, strong) => {
                        self.edges.clear();
                        let mut dx = 0.0;
                        for c in t.chars() {
                            let Some(g) = font.glyph(c as u32) else { continue };
                            let gm = m.mul(&Matrix::new(sc, 0.0, 0.0, sc, x0 + dx, line_y));
                            dx += g.advance * sc;
                            for contour in &g.contours {
                                let (mut x, mut y, mut sx, mut sy) = (0.0, 0.0, 0.0, 0.0);
                                for seg in contour {
                                    match *seg {
                                        Seg::Move(px, py) => {
                                            (x, y, sx, sy) = (px, py, px, py);
                                        }
                                        Seg::Line(px, py) => {
                                            let (a, b) = (gm.apply(x, y), gm.apply(px, py));
                                            self.edges.push([a.0, a.1, b.0, b.1]);
                                            (x, y) = (px, py);
                                        }
                                        Seg::Quad(qx, qy, px, py) => {
                                            raster::flatten(gm.apply(x, y), gm.apply(qx, qy), gm.apply(px, py), &mut self.edges);
                                            (x, y) = (px, py);
                                        }
                                    }
                                }
                                if x != sx || y != sy {
                                    let (a, b) = (gm.apply(x, y), gm.apply(sx, sy));
                                    self.edges.push([a.0, a.1, b.0, b.1]);
                                }
                            }
                        }
                        if raster::coverage(&self.edges, &limit, &mut self.cov, &mut self.xs) {
                            let colour = if *strong { PaintFn::Solid(raster::solid(STRONG, cx)) } else { PaintFn::Solid(raster::solid(def.color, cx)) };
                            raster::fill(canvas, &self.cov, &colour, clip);
                        }
                    }
                }
                x0 += width;
            }
            line_y += (font.ascent + font.descent + font.leading) * sc + if def.word_wrap { def.leading } else { 0.0 };
        }
        self.stats.ns_text += t0.elapsed().as_nanos() as u64;
    }

    /// PLACEHOLDER for the pad-button icons (`<icon(Triangle)>` ...): the game draws a texture of
    /// `UI/IconText` for the platform; the pack does not hold those, so a disc in the Xbox
    /// button's colour with its letter stands in, centred in the field.
    fn draw_icon(&mut self, canvas: &mut Canvas, icon: &str, def: &TextDef, m: &Matrix, cx: &Cx, clip: Option<&Mask>) {
        if let Some(i) = self.fonts.icons.as_ref().and_then(|set| set.icons.get(&icon.to_lowercase())).copied() {
            // The whole field is one icon: centred in the field.
            let b = def.bounds.rect();
            let h = icon_height(def.size);
            let w = h * i.3 / i.4.max(1.0);
            self.draw_icon_at(canvas, icon, def, ((b.x0 + b.x1 - w) / 2.0, (b.y0 + b.y1 - h) / 2.0, w, h), m, cx, clip);
            return;
        }
        self.draw_icon_disc(canvas, icon, def, m, cx, clip);
    }

    /// An icon of `UI/IconText` in the rectangle (x, y, w, h) of the field's space; the
    /// placeholder disc for a name the pack does not hold.
    #[allow(clippy::too_many_arguments)]
    fn draw_icon_at(&mut self, canvas: &mut Canvas, icon: &str, def: &TextDef, at: (f32, f32, f32, f32), m: &Matrix, cx: &Cx, clip: Option<&Mask>) {
        let Some((set, i)) = self.fonts.icons.clone().and_then(|set| set.icons.get(&icon.to_lowercase()).copied().map(|i| (set, i))) else {
            // Shift the placeholder to the rectangle's centre.
            let b = def.bounds.rect();
            let shift = Matrix::translate(at.0 + at.2 / 2.0 - (b.x0 + b.x1) / 2.0, at.1 + at.3 / 2.0 - (b.y0 + b.y1) / 2.0);
            self.draw_icon_disc(canvas, icon, def, &m.mul(&shift), cx, clip);
            return;
        };
        let k = at.3 / i.4.max(1.0);
        let fill = crate::def::BitmapFill { texture: set.textures[i.0].clone(), matrix: Matrix::new(k, 0.0, 0.0, k, at.0 - k * i.1, at.1 - k * i.2), scale: (1.0, 1.0), repeat: false, smooth: true };
        self.draw_gfx(canvas, &GfxRect { x: at.0, y: at.1, w: at.2, h: at.3, fill }, m, cx, clip);
    }

    fn draw_icon_disc(&mut self, canvas: &mut Canvas, icon: &str, def: &TextDef, m: &Matrix, cx: &Cx, clip: Option<&Mask>) {
        let (letter, colour): (&str, [f32; 4]) = match icon {
            "Circle" => ("B", [214.0, 54.0, 46.0, 255.0]),
            "Triangle" => ("Y", [226.0, 178.0, 30.0, 255.0]),
            "Square" => ("X", [42.0, 110.0, 214.0, 255.0]),
            "Cross" => ("A", [74.0, 168.0, 58.0, 255.0]),
            "L1" => ("LB", [110.0, 110.0, 118.0, 255.0]),
            "R1" => ("RB", [110.0, 110.0, 118.0, 255.0]),
            "L2" => ("LT", [110.0, 110.0, 118.0, 255.0]),
            "R2" => ("RT", [110.0, 110.0, 118.0, 255.0]),
            _ => ("?", [110.0, 110.0, 118.0, 255.0]),
        };
        let limit = self.limit_of(canvas, clip);
        let b = def.bounds.rect();
        let (cxp, cyp) = ((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0);
        let r = def.size * 0.6;
        self.edges.clear();
        const N: usize = 28;
        for k in 0..N {
            let (a0, a1) = (k as f32 / N as f32 * std::f32::consts::TAU, (k + 1) as f32 / N as f32 * std::f32::consts::TAU);
            let (p, q) = (m.apply(cxp + r * a0.cos(), cyp + r * a0.sin()), m.apply(cxp + r * a1.cos(), cyp + r * a1.sin()));
            self.edges.push([p.0, p.1, q.0, q.1]);
        }
        if raster::coverage(&self.edges, &limit, &mut self.cov, &mut self.xs) {
            raster::fill(canvas, &self.cov, &PaintFn::Solid(raster::solid(colour, cx)), clip);
        }
        let Some(font) = self.fonts.get(Some("$KH2Menu")).cloned() else { return };
        let size = def.size * if letter.len() > 1 { 0.62 } else { 0.9 };
        let sc = size / font.units_per_em;
        let wd = font.text_width(letter, size);
        let mut x = cxp - wd / 2.0;
        let base = cyp + size * 0.36;
        self.edges.clear();
        for c in letter.chars() {
            let Some(g) = font.glyph(c as u32) else { continue };
            let gm = m.mul(&Matrix::new(sc, 0.0, 0.0, sc, x, base));
            for contour in &g.contours {
                let (mut px, mut py, mut sx, mut sy) = (0.0, 0.0, 0.0, 0.0);
                for seg in contour {
                    match *seg {
                        Seg::Move(ax, ay) => (px, py, sx, sy) = (ax, ay, ax, ay),
                        Seg::Line(ax, ay) => {
                            let (a, b) = (gm.apply(px, py), gm.apply(ax, ay));
                            self.edges.push([a.0, a.1, b.0, b.1]);
                            (px, py) = (ax, ay);
                        }
                        Seg::Quad(qx, qy, ax, ay) => {
                            raster::flatten(gm.apply(px, py), gm.apply(qx, qy), gm.apply(ax, ay), &mut self.edges);
                            (px, py) = (ax, ay);
                        }
                    }
                }
                if px != sx || py != sy {
                    let (a, b) = (gm.apply(px, py), gm.apply(sx, sy));
                    self.edges.push([a.0, a.1, b.0, b.1]);
                }
            }
            x += g.advance * sc;
        }
        if raster::coverage(&self.edges, &limit, &mut self.cov, &mut self.xs) {
            raster::fill(canvas, &self.cov, &PaintFn::Solid(raster::solid([255.0; 4], cx)), clip);
        }
    }
}

/// The matrix a text field is drawn with and its layout width: the node's own, or what
/// `TextFieldAdjust.updateText` set (scaleX about the field's origin, left edge at `x`).
fn text_matrix(t: &TextNode, mm: &Matrix, parent: &Matrix) -> (Matrix, f32) {
    let b = &t.def.bounds;
    match t.adjust {
        Some((sx, x_edge, width)) => {
            // `mm` = parent * node matrix; only the node's translation in y is kept.
            let node_ty = {
                let inv = parent.inverse();
                inv.mul(mm).ty
            };
            (parent.mul(&Matrix::new(sx, 0.0, 0.0, 1.0, x_edge - sx * b.x0, node_ty)), width)
        }
        None => (*mm, b.x1 - b.x0),
    }
}

/// The rectangle a fill's edges describe, when they are four axis-aligned lines.
fn rect_of(edges: &[Edge]) -> Option<Rect> {
    if edges.len() != 4 {
        return None;
    }
    let mut r = Rect::EMPTY;
    for e in edges {
        let Edge::Line(l) = e else { return None };
        if l[0] != l[2] && l[1] != l[3] {
            return None;
        }
        r.add_point(l[0], l[1]);
        r.add_point(l[2], l[3]);
    }
    // Four axis-aligned edges with two distinct x and two distinct y values: a rectangle.
    let mut on_border = true;
    for e in edges {
        let Edge::Line(l) = e else { return None };
        let horizontal = l[1] == l[3];
        on_border &= if horizontal { (l[1] == r.y0 || l[1] == r.y1) && l[0].min(l[2]) == r.x0 && l[0].max(l[2]) == r.x1 } else { (l[0] == r.x0 || l[0] == r.x1) && l[1].min(l[3]) == r.y0 && l[1].max(l[3]) == r.y1 };
    }
    (on_border && !r.is_empty()).then_some(r)
}

/// A morph shape at `t` (0..1): the pack holds the start and end shapes with matching edges.
fn morph(a: &Shape, b: &Shape, t: f32) -> Shape {
    let lerp = |x: f32, y: f32| x + (y - x) * t;
    let mut out = a.clone();
    for (fill, end) in out.fills.iter_mut().zip(&b.fills) {
        for (e, f) in fill.edges.iter_mut().zip(&end.edges) {
            match (e, f) {
                (Edge::Line(l), Edge::Line(m)) => (0..4).for_each(|i| l[i] = lerp(l[i], m[i])),
                (Edge::Curve(l), Edge::Curve(m)) => (0..6).for_each(|i| l[i] = lerp(l[i], m[i])),
                _ => {}
            }
        }
        if let (Paint::Solid(c), Paint::Solid(d)) = (&mut fill.paint, &end.paint) {
            (0..4).for_each(|i| c[i] = lerp(c[i], d[i]));
        }
    }
    let (ba, bb) = (a.bounds.rect(), b.bounds.rect());
    let u = ba.union(&bb);
    out.bounds = crate::def::Rect0 { x0: u.x0, y0: u.y0, x1: u.x1, y1: u.y1 };
    out
}

/// A hash of everything that decides the picture: equal hashes = nothing to redraw.
pub fn state_hash(movie: &Movie) -> u64 {
    fn walk(s: &Sprite, h: &mut std::collections::hash_map::DefaultHasher) {
        s.def.id.hash(h);
        for g in &s.gfx {
            (g.x.to_bits(), g.y.to_bits(), g.w.to_bits(), g.h.to_bits(), g.fill.matrix.bits()).hash(h);
        }
        for o in s.children.iter().chain(&s.extra) {
            if o.clip == 0 && (!o.is_visible() || o.cx.is_invisible()) {
                // Not drawn: nothing below it matters.
                (o.depth, 0u8).hash(h);
                continue;
            }
            (o.ch, o.depth, o.z, o.matrix.bits(), o.clip, o.blend, o.is_visible(), o.ratio.to_bits()).hash(h);
            o.cx.mul.map(f32::to_bits).hash(h);
            o.cx.add.map(f32::to_bits).hash(h);
            match &o.kind {
                Kind::Sprite(c) => walk(c, h),
                Kind::Text(t) => {
                    t.shown().hash(h);
                    t.adjust.map(|(a, b, c)| (a.to_bits(), b.to_bits(), c.to_bits())).hash(h);
                }
                _ => {}
            }
        }
        0xFFu8.hash(h);
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    walk(&movie.root, &mut h);
    h.finish()
}

// ---------------------------------------------------------------------------------------------
// Dirty rectangles: what changed between two states of a movie, so that a host redraws only
// that part of its picture (a full-screen menu with one moving cursor).

fn hash_node(o: &Node, h: &mut std::collections::hash_map::DefaultHasher) {
    (o.ch, o.depth, o.z, o.matrix.bits(), o.clip, o.blend, o.is_visible(), o.ratio.to_bits()).hash(h);
    o.cx.mul.map(f32::to_bits).hash(h);
    o.cx.add.map(f32::to_bits).hash(h);
    match &o.kind {
        Kind::Sprite(s) => {
            s.def.id.hash(h);
            for g in &s.gfx {
                (g.x.to_bits(), g.y.to_bits(), g.w.to_bits(), g.h.to_bits(), g.fill.matrix.bits()).hash(h);
            }
            for c in s.children.iter().chain(&s.extra) {
                hash_node(c, h);
            }
            0xFFu8.hash(h);
        }
        Kind::Text(t) => {
            t.shown().hash(h);
            t.adjust.map(|(a, b, c)| (a.to_bits(), b.to_bits(), c.to_bits())).hash(h);
        }
        _ => {}
    }
}

impl Renderer {
    /// Every drawn piece of the movie as (hash of its place and state, device bounds). A piece
    /// whose hash is in the previous list too is unchanged; see [`dirty_rect`].
    pub fn parts(&self, movie: &Movie, root: &Matrix) -> Vec<(u64, Rect)> {
        let mut out = Vec::new();
        self.sprite_parts(&movie.root, root, &Cx::IDENTITY, 0x9E37_79B9_7F4A_7C15, &mut out);
        out
    }

    fn sprite_parts(&self, s: &Sprite, m: &Matrix, cx: &Cx, path: u64, out: &mut Vec<(u64, Rect)>) {
        use std::collections::hash_map::DefaultHasher;
        let start = |tag: u32, index: usize| {
            let mut h = DefaultHasher::new();
            (path, tag, index, m.bits()).hash(&mut h);
            cx.mul.map(f32::to_bits).hash(&mut h);
            cx.add.map(f32::to_bits).hash(&mut h);
            h
        };
        for (index, g) in s.gfx.iter().enumerate() {
            let mut h = start(1, index);
            (g.x.to_bits(), g.y.to_bits(), g.w.to_bits(), g.h.to_bits(), g.fill.matrix.bits()).hash(&mut h);
            out.push((h.finish(), Rect::new(g.x, g.y, g.x + g.w, g.y + g.h).transform(m)));
        }
        for (index, o) in s.children.iter().chain(&s.extra).enumerate() {
            if o.clip != 0 {
                // A mask: one piece, whatever is inside it (it changes what shows within its bounds).
                let mut h = start(2, index);
                hash_node(o, &mut h);
                out.push((h.finish(), self.node_bounds(o, m, true, &Cx::IDENTITY)));
                continue;
            }
            let cc = cx.concat(&o.cx);
            if !o.is_visible() || cc.is_invisible() {
                continue;
            }
            match &o.kind {
                Kind::Sprite(c) if o.blend <= 1 => {
                    let mut h = DefaultHasher::new();
                    (path, 3u32, index, o.depth, o.ch, o.z).hash(&mut h);
                    self.sprite_parts(c, &m.mul(&o.matrix), &cc, h.finish(), out);
                }
                _ => {
                    // A leaf, or a sprite composited as a layer: one piece.
                    let mut h = start(4, index);
                    hash_node(o, &mut h);
                    out.push((h.finish(), self.node_bounds(o, m, false, cx)));
                }
            }
        }
    }
}

/// The device rectangle that differs between two [`Renderer::parts`] lists (empty = nothing).
pub fn dirty_rect(before: &[(u64, Rect)], now: &[(u64, Rect)]) -> Rect {
    use std::collections::HashMap;
    let mut count: HashMap<u64, i32> = HashMap::with_capacity(before.len());
    for (hash, _) in before {
        *count.entry(*hash).or_insert(0) += 1;
    }
    let mut dirty = Rect::EMPTY;
    for (hash, rect) in now {
        match count.get_mut(hash) {
            Some(n) if *n > 0 => *n -= 1,
            _ => {
                if !rect.is_empty() {
                    dirty = dirty.union(rect);
                }
            }
        }
    }
    for (hash, rect) in before {
        if let Some(n) = count.get_mut(hash) {
            if *n > 0 {
                *n -= 1;
                if !rect.is_empty() {
                    dirty = dirty.union(rect);
                }
            }
        }
    }
    dirty
}

#[cfg(test)]
mod fit_tests {
    use super::*;
    use crate::def::{Glyph, Rect0};

    /// A font whose every glyph is 1 em wide (size 10: 10 px a character), ascent 0.8, descent 0.2.
    fn fonts() -> Fonts {
        let glyphs = (32u32..127).map(|c| (c, Glyph { advance: 1.0, contours: Vec::new() })).collect();
        let font = Arc::new(Font { class_name: "t".to_string(), units_per_em: 1.0, ascent: 0.8, descent: 0.2, leading: 0.0, glyphs });
        Fonts { map: HashMap::new(), default: Some(font), icons: None }
    }

    fn field(height: f32, wrap: bool) -> TextDef {
        // 104 wide: a box of 100 px = 10 characters a line; lines 10 px apart, the first ends at 2 + 10.
        TextDef { id: 1, bounds: Rect0 { x0: 0.0, y0: 0.0, x1: 104.0, y1: height }, font_class: None, size: 10.0, color: [255.0; 4], align: Align::Left, initial: String::new(), word_wrap: wrap, leading: 0.0 }
    }

    /// (IMPL_ER42) An Elden Ring description longer than the item pane's help field is cut to the lines that fit, ending
    /// in "..."; a text that fits and a field without word wrap are left alone.
    #[test]
    fn a_long_text_is_cut_to_the_lines_the_field_holds() {
        let fonts = fonts();
        let text = "aaaa bbbb cccc dddd\n\neeee ffff gggg";
        // 3 lines fit in 32 px (2 + 10 + 10 + 10): the third is empty and goes, the dots end the last text line.
        assert_eq!(fit_text(&fonts, &field(32.0, true), text), "aaaa bbbb
cccc...");
        // 4 lines: the paragraph break stays; the last line keeps what fits next to the dots.
        assert_eq!(fit_text(&fonts, &field(42.0, true), text), "aaaa bbbb
cccc dddd

eeee...");
        // Everything fits (6 lines in 62 px): unchanged.
        assert_eq!(fit_text(&fonts, &field(62.0, true), text), text);
        assert_eq!(fit_text(&fonts, &field(22.0, false), text), text, "no word wrap: unchanged");
        assert_eq!(fit_text(&fonts, &field(22.0, true), "short"), "short");
    }
}

#[cfg(test)]
mod scale9_tests {
    use super::scale9_axis;

    /// (IMPL_ER21) The camp panel's EXP gauge track: a 30 px bar with the grid 15..25, placed 5.333 times as wide.
    #[test]
    fn the_caps_keep_their_size_and_the_middle_takes_the_rest() {
        // stage = device (parent scale 1), the sprite at x 1221
        let pieces = scale9_axis(0.0, 30.0, 15.0, 25.0, 5.333, 1221.0, 1.0).unwrap();
        assert_eq!(pieces.len(), 3);
        let at = |x: f32| {
            let p = pieces.iter().find(|p| p[0] <= x && x <= p[1]).unwrap();
            p[2] * x + p[3]
        };
        // the left cap is 15 px, the right one 5 px, the whole still spans 160 px
        assert!((at(0.0) - 1221.0).abs() < 1e-3 && (at(15.0) - 1236.0).abs() < 1e-3);
        assert!((at(25.0) - 1376.0).abs() < 0.51 && (at(30.0) - 1380.99).abs() < 0.02);
        // the joints are whole device pixels and the pieces' limits meet
        assert_eq!((pieces[0][5], pieces[1][4], pieces[1][5], pieces[2][4]), (1236.0, 1236.0, 1376.0, 1376.0));
        // a parent scale of 2 keeps the caps at 2 device pixels a unit
        let pieces = scale9_axis(0.0, 30.0, 15.0, 25.0, 10.666, 0.0, 2.0).unwrap();
        assert!((pieces[0][2] - 2.0).abs() < 1e-3 && (pieces[2][2] - 2.0).abs() < 0.05);
    }

    #[test]
    fn nothing_to_slice() {
        // a grid that covers the whole extent (the gauge's vertical axis: -2..16 over 0..14): the plain stretch
        assert!(scale9_axis(0.0, 14.0, -2.0, 16.0, 2.0, 0.0, 1.0).is_none());
        // scaled smaller than its caps
        assert!(scale9_axis(0.0, 30.0, 15.0, 25.0, 0.3, 0.0, 1.0).is_none());
    }
}
