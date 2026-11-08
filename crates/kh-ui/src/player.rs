//! The timeline player: a tree of instances with their current frames, evaluated the way the
//! Flash / Scaleform player does (docs/sora-ui-spec.md sections 1 and 2):
//!
//! - every sprite instance has a current frame and a playing flag; one `Movie::tick` is one frame
//!   of the 60 fps stage: every playing sprite goes to its next frame (the last frame wraps to
//!   the first), its frame script runs, then its children tick;
//! - a frame is reached by applying the timeline's place / move / remove operations; going
//!   backwards rebuilds the list from frame 1 and KEEPS an instance whose depth, character and
//!   placing frame are the same (so its own frame, text and script-set properties survive);
//! - frame scripts are the handful of statements the packed movies contain (`stop`, `play`,
//!   `gotoAndPlay`, `gotoAndStop` on the clip or a descendant, `visible = ...`, the `AnimEvent`
//!   dispatch): there is no ActionScript VM. The document classes' methods are ported by hand
//!   in `hud_*.rs` on top of the path functions of [`Movie`];
//! - a sprite that was told to go somewhere during a frame does not also advance in that frame.

use std::sync::Arc;

use crate::def::{Char, Control, ImageChar, MovieDef, Op, Shape, SpriteDef, Stmt, Target, TextDef};
use crate::geom::{Cx, Matrix};

/// A rectangle of a bitmap drawn by script (`Graphics.beginBitmapFill` + `drawRect`).
#[derive(Clone, Debug)]
pub struct GfxRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub fill: crate::def::BitmapFill,
}

#[derive(Clone, Debug)]
pub struct TextNode {
    pub def: Arc<TextDef>,
    /// `None`: the authored placeholder text.
    pub text: Option<String>,
    /// `TextFieldAdjust.updateText`: (scaleX, x of the field's left edge, field width).
    pub adjust: Option<(f32, f32, f32)>,
    /// (BN) Its inline icons (`<icon(Name)>`) are drawn as the game's HTML text draws an `IconTextData` image: at the
    /// table's own size (32 x 32) and `VSpace` (-7: the cell reaches 7 px under the baseline), not scaled to the font
    /// size (`render::icon_height`, the menus' LOCAL rule). Set by the reward panels (KH3's BONUS! picture).
    pub icon_native: bool,
}

impl TextNode {
    pub fn shown(&self) -> &str {
        self.text.as_deref().unwrap_or(&self.def.initial)
    }
}

#[derive(Clone, Debug)]
pub enum Kind {
    Shape(Arc<Shape>),
    Morph(Arc<Shape>, Arc<Shape>),
    Image(Arc<ImageChar>),
    Text(TextNode),
    Sprite(Box<Sprite>),
    /// A character the pack does not hold (an import): nothing is drawn.
    Missing,
}

/// One placed object of a display list.
#[derive(Clone, Debug)]
pub struct Node {
    pub ch: u32,
    pub depth: i32,
    /// The frame whose operation placed this character here.
    pub born: u32,
    /// Draw order when a script moved the object (`addChild` of an existing child): above every
    /// timeline depth, in the order of the calls.
    pub z: Option<i32>,
    pub name: Option<Arc<str>>,
    pub matrix: Matrix,
    pub cx: Cx,
    /// Clip depth: this object masks the depths up to this one (0 = not a mask).
    pub clip: i32,
    pub blend: u8,
    pub visible: bool,
    /// Set by a script: the timeline no longer decides.
    pub script_visible: Option<bool>,
    pub script_matrix: bool,
    pub ratio: f32,
    pub kind: Kind,
}

impl Node {
    pub fn is_visible(&self) -> bool {
        self.script_visible.unwrap_or(self.visible)
    }

    pub fn sprite(&self) -> Option<&Sprite> {
        match &self.kind {
            Kind::Sprite(s) => Some(s),
            _ => None,
        }
    }

    pub fn sprite_mut(&mut self) -> Option<&mut Sprite> {
        match &mut self.kind {
            Kind::Sprite(s) => Some(s),
            _ => None,
        }
    }

    pub fn label(&self) -> String {
        match &self.name {
            Some(n) => n.to_string(),
            None => format!("@{}", self.depth),
        }
    }
}

/// `AnimEvent.END` dispatched by a frame script.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimEnd {
    /// Instance path of the clip whose script dispatched it.
    pub path: String,
    /// The constant's name in the document class (`ANIM_END_NEXTLV_OUT` ...).
    pub name: String,
    pub value: i32,
}

/// What the player hands down while it works.
#[derive(Clone)]
pub struct Ctx {
    pub serial: u64,
    pub events: Vec<AnimEnd>,
    pub path: Vec<Arc<str>>,
    pub language: Arc<str>,
    pub world: Arc<str>,
    nesting: u32,
}

/// State of a `tres.controls.BitmapNumber` (AS3 class of the HUD movies; ported in `controls.rs`).
#[derive(Clone, Debug, PartialEq)]
pub struct NumberProps {
    pub bmp_name: String,
    pub bmp_width: i32,
    pub bmp_height: i32,
    pub bmp_start_x: i32,
    pub bmp_start_y: i32,
    pub num_digit: i32,
    pub zero_padding: bool,
    pub kerning: i32,
    /// `left`, `center`, `right`.
    pub align: &'static str,
    /// `center`, `left-up`, `up`, `right-up`, `left`, `right`, `left-down`, `down`, `right-down`.
    pub origin_anim: &'static str,
    /// False: one animation instance per digit (only the digits' places are ported).
    pub whole_anim: bool,
    pub number: i32,
}

#[derive(Clone, Debug)]
pub struct Sprite {
    pub movie: Arc<MovieDef>,
    pub def: Arc<SpriteDef>,
    pub frame: u32,
    pub playing: bool,
    serial: u64,
    /// Timeline children, sorted by depth.
    pub children: Vec<Node>,
    /// Children added by script (a loaded movie's content), drawn above the timeline's.
    pub extra: Vec<Node>,
    /// Script-drawn graphics, under the children.
    pub gfx: Vec<GfxRect>,
    /// A script emptied this clip (`removeChildren`): the timeline's children stay away.
    pub emptied: bool,
    /// `TextFieldAdjust`: (m_text, m_originalTextFieldW).
    pub text_adjust: Option<(String, f32)>,
    pub number: Option<Box<NumberProps>>,
    /// (loot) `tres.controls.Icon`: its default size and clip rectangle (`controls::icon_init`).
    pub icon: Option<crate::controls::IconProps>,
    /// (ASH-ROWS) Drawn WIDER than authored ([`Stretch`], [`crate::render`]): None = as authored.
    pub stretch: Option<Stretch>,
    next_z: i32,
}

/// (ASH-ROWS) A sprite drawn wider than authored, as a horizontal scaling grid draws it: what lies left of `from` as it
/// is, `from .. to` stretched by `by` more units, what lies right of `to` moved right by `by` (its own units, on whole
/// device pixels at the joints). The command menu's spirit-ash rows widen their plates with it, the end caps unchanged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stretch {
    pub from: f32,
    pub to: f32,
    pub by: f32,
}

fn make_node(movie: &Arc<MovieDef>, op: &Op, ch: u32, frame: u32, ctx: &mut Ctx) -> Node {
    let kind = match movie.chars.get(&ch) {
        Some(Char::Shape(s)) => Kind::Shape(s.clone()),
        Some(Char::Morph(a, b)) => Kind::Morph(a.clone(), b.clone()),
        Some(Char::Image(i)) => Kind::Image(i.clone()),
        Some(Char::Text(t)) => Kind::Text(TextNode { def: t.clone(), text: None, adjust: None, icon_native: false }),
        Some(Char::Sprite(def)) => {
            ctx.path.push(op.name.clone().unwrap_or_else(|| Arc::from(format!("@{}", op.depth))));
            let s = Sprite::new(movie.clone(), def.clone(), ctx);
            ctx.path.pop();
            Kind::Sprite(Box::new(s))
        }
        Some(Char::Library(lib, def)) => {
            ctx.path.push(op.name.clone().unwrap_or_else(|| Arc::from(format!("@{}", op.depth))));
            let s = Sprite::new(lib.clone(), def.clone(), ctx);
            ctx.path.pop();
            Kind::Sprite(Box::new(s))
        }
        None => Kind::Missing,
    };
    Node { ch, depth: op.depth, born: frame, z: None, name: None, matrix: Matrix::IDENTITY, cx: Cx::IDENTITY, clip: 0, blend: 0, visible: true, script_visible: None, script_matrix: false, ratio: 0.0, kind }
}

fn apply_attrs(node: &mut Node, op: &Op) {
    if let Some(m) = op.matrix {
        if !node.script_matrix {
            node.matrix = m;
        }
    }
    if let Some(cx) = op.cx {
        node.cx = cx;
    }
    if let Some(name) = &op.name {
        node.name = Some(name.clone());
    }
    if let Some(clip) = op.clip {
        node.clip = clip;
    }
    if let Some(blend) = op.blend {
        node.blend = blend;
    }
    if let Some(visible) = op.visible {
        node.visible = visible;
    }
    if let Some(ratio) = op.ratio {
        node.ratio = ratio;
    }
}

/// A display-list entry while a frame is being computed from scratch.
#[derive(Clone)]
struct Placed {
    depth: i32,
    ch: u32,
    born: u32,
    /// The accumulated attributes, as one operation.
    attrs: Op,
}

impl Sprite {
    pub fn new(movie: Arc<MovieDef>, def: Arc<SpriteDef>, ctx: &mut Ctx) -> Sprite {
        let mut s = Sprite { movie, playing: def.frames > 1, def, frame: 0, serial: ctx.serial, children: Vec::new(), extra: Vec::new(), gfx: Vec::new(), emptied: false, text_adjust: None, number: None, icon: None, stretch: None, next_z: 1 };
        s.apply_frame_ops(1, ctx);
        s.frame = 1;
        // The class constructor (the `tres.controls` base classes), then the frame script.
        match s.def.control {
            Control::BitmapNumber => {
                s.playing = false;
                crate::controls::number_init(&mut s);
            }
            Control::TextAdjust => crate::controls::text_adjust_init(&mut s),
            // (IMPL_ER23B) `ImageIcon.<init>`: `removeChildren()` (the white authoring placeholder goes)
            Control::ImageIcon => s.remove_children(),
            // (loot) `Icon.<init>`: default size and clip rectangle from the authoring placeholder, then `removeChildren()`
            Control::Icon => crate::controls::icon_init(&mut s),
            _ => {}
        }
        s.run_script(1, ctx);
        match s.def.control {
            Control::Language => {
                let label = if s.def.label_frame(&ctx.language).is_some() { ctx.language.to_string() } else { "none".to_string() };
                s.goto_label(&label, false, ctx);
            }
            Control::World => {
                // tres.controls.WorldObject: the label of System.world, else DEFAULT_LABEL "ex".
                let label = if s.def.label_frame(&ctx.world).is_some() { ctx.world.to_string() } else { "ex".to_string() };
                if !s.goto_label(&label, false, ctx) {
                    s.playing = false;
                }
            }
            _ => {}
        }
        s
    }

    fn apply_frame_ops(&mut self, frame: u32, ctx: &mut Ctx) {
        if self.emptied {
            return;
        }
        let def = self.def.clone();
        let Some(ops) = def.ops.get(frame as usize - 1) else { return };
        for op in ops {
            let at = self.children.binary_search_by_key(&op.depth, |n| n.depth);
            if op.remove {
                if let Ok(i) = at {
                    self.children.remove(i);
                }
                continue;
            }
            match (op.is_move, at, op.ch) {
                (true, Ok(i), ch) => {
                    if let Some(ch) = ch.filter(|ch| *ch != self.children[i].ch) {
                        let old = &self.children[i];
                        let mut node = make_node(&self.movie, op, ch, frame, ctx);
                        (node.matrix, node.cx, node.name, node.clip, node.blend, node.visible, node.ratio) = (old.matrix, old.cx, old.name.clone(), old.clip, old.blend, old.visible, old.ratio);
                        self.children[i] = node;
                    }
                    apply_attrs(&mut self.children[i], op);
                }
                (_, at, Some(ch)) => {
                    let mut node = make_node(&self.movie, op, ch, frame, ctx);
                    apply_attrs(&mut node, op);
                    match at {
                        Ok(i) => self.children[i] = node,
                        Err(i) => self.children.insert(i, node),
                    }
                }
                _ => {}
            }
        }
    }

    /// The display list of `frame` computed from frame 1 (depth order).
    fn list_at(&self, frame: u32) -> Vec<Placed> {
        let mut list: Vec<Placed> = Vec::new();
        for f in 1..=frame {
            for op in &self.def.ops[f as usize - 1] {
                let at = list.binary_search_by_key(&op.depth, |p| p.depth);
                if op.remove {
                    if let Ok(i) = at {
                        list.remove(i);
                    }
                    continue;
                }
                let merge = |into: &mut Op, op: &Op| {
                    if op.matrix.is_some() {
                        into.matrix = op.matrix;
                    }
                    if op.cx.is_some() {
                        into.cx = op.cx;
                    }
                    if op.name.is_some() {
                        into.name = op.name.clone();
                    }
                    if op.clip.is_some() {
                        into.clip = op.clip;
                    }
                    if op.blend.is_some() {
                        into.blend = op.blend;
                    }
                    if op.visible.is_some() {
                        into.visible = op.visible;
                    }
                    if op.ratio.is_some() {
                        into.ratio = op.ratio;
                    }
                };
                match (op.is_move, at, op.ch) {
                    (true, Ok(i), ch) => {
                        if let Some(ch) = ch.filter(|ch| *ch != list[i].ch) {
                            list[i].ch = ch;
                            list[i].born = f;
                        }
                        merge(&mut list[i].attrs, op);
                    }
                    (_, at, Some(ch)) => {
                        let placed = Placed { depth: op.depth, ch, born: f, attrs: Op { depth: op.depth, ..op.clone() } };
                        match at {
                            Ok(i) => list[i] = placed,
                            Err(i) => list.insert(i, placed),
                        }
                    }
                    _ => {}
                }
            }
        }
        list
    }

    fn rebuild(&mut self, frame: u32, ctx: &mut Ctx) {
        if self.emptied {
            return;
        }
        let list = self.list_at(frame);
        let mut old: Vec<Option<Node>> = std::mem::take(&mut self.children).into_iter().map(Some).collect();
        let mut children = Vec::with_capacity(list.len());
        for placed in list {
            let kept = old.iter_mut().find(|n| n.as_ref().is_some_and(|n| n.depth == placed.depth && n.ch == placed.ch && n.born == placed.born)).and_then(Option::take);
            let mut node = match kept {
                Some(mut node) => {
                    // The timeline's attributes of this frame, from a clean slate.
                    (node.cx, node.clip, node.blend, node.visible, node.ratio) = (Cx::IDENTITY, 0, 0, true, 0.0);
                    if !node.script_matrix {
                        node.matrix = Matrix::IDENTITY;
                    }
                    node
                }
                None => make_node(&self.movie, &placed.attrs, placed.ch, placed.born, ctx),
            };
            apply_attrs(&mut node, &placed.attrs);
            children.push(node);
        }
        self.children = children;
    }

    pub fn resolve(&self, target: &Target) -> Option<u32> {
        match target {
            Target::Frame(f) => Some(*f),
            Target::Label(l) => self.def.label_frame(l),
        }
    }

    /// `gotoAndPlay` / `gotoAndStop` by frame number.
    pub fn goto(&mut self, frame: u32, play: bool, ctx: &mut Ctx) {
        let frame = frame.clamp(1, self.def.frames);
        self.playing = play;
        self.serial = ctx.serial;
        if frame == self.frame {
            return;
        }
        if frame > self.frame {
            for f in self.frame + 1..=frame {
                self.apply_frame_ops(f, ctx);
            }
        } else {
            self.rebuild(frame, ctx);
        }
        self.frame = frame;
        self.run_script(frame, ctx);
    }

    /// `gotoAndPlay(label)` / `gotoAndStop(label)`; false when the clip has no such label (the
    /// call is then ignored, as the Flash player does after reporting an ArgumentError).
    pub fn goto_label(&mut self, label: &str, play: bool, ctx: &mut Ctx) -> bool {
        match self.def.label_frame(label) {
            Some(frame) => {
                self.goto(frame, play, ctx);
                true
            }
            None => false,
        }
    }

    pub fn current_label(&self) -> Option<&str> {
        self.def.label_at(self.frame)
    }

    fn run_script(&mut self, frame: u32, ctx: &mut Ctx) {
        let def = self.def.clone();
        let Some(script) = def.scripts.get(frame as usize - 1) else { return };
        if script.is_empty() || ctx.nesting > 24 {
            return;
        }
        ctx.nesting += 1;
        for stmt in script {
            match stmt {
                Stmt::Stop(path) => {
                    if let Some(s) = self.descend_mut(path) {
                        s.playing = false;
                    }
                }
                Stmt::Play(path) => {
                    if let Some(s) = self.descend_mut(path) {
                        s.playing = true;
                    }
                }
                Stmt::GotoPlay(path, target) | Stmt::GotoStop(path, target) => {
                    let play = matches!(stmt, Stmt::GotoPlay(..));
                    let depth = ctx.path.len();
                    ctx.path.extend(path.iter().map(|p| Arc::from(p.as_str())));
                    if let Some(s) = self.descend_mut(path) {
                        if let Some(frame) = s.resolve(target) {
                            s.goto(frame, play, ctx);
                        }
                    }
                    ctx.path.truncate(depth);
                }
                Stmt::Visible(path, visible) => {
                    if let Some((last, parents)) = path.split_last() {
                        if let Some(node) = self.descend_mut(parents).and_then(|s| s.child_mut(last)) {
                            node.script_visible = Some(*visible);
                        }
                    }
                }
                Stmt::AnimEnd(name, value) => ctx.events.push(AnimEnd { path: ctx.path.join("."), name: name.clone(), value: *value }),
                Stmt::Unknown(_) => {}
            }
        }
        ctx.nesting -= 1;
    }

    fn tick(&mut self, ctx: &mut Ctx) {
        if self.serial != ctx.serial && self.playing && self.def.frames > 1 {
            let next = if self.frame >= self.def.frames { 1 } else { self.frame + 1 };
            self.goto(next, true, ctx);
            // `goto` marks the clip as moved in this frame; an advance is not such a move for
            // the clip's own scripts, but nothing else looks at the mark before the next frame.
        }
        for i in 0..self.children.len() + self.extra.len() {
            let n = self.children.len();
            let node = if i < n { &mut self.children[i] } else { &mut self.extra[i - n] };
            let name = node.name.clone();
            let depth = node.depth;
            if let Kind::Sprite(s) = &mut node.kind {
                ctx.path.push(name.unwrap_or_else(|| Arc::from(format!("@{depth}"))));
                s.tick(ctx);
                ctx.path.pop();
            }
            // A script of the child may have changed this clip's list: stay within bounds.
            if i + 1 >= self.children.len() + self.extra.len() {
                break;
            }
        }
    }

    pub fn child(&self, name: &str) -> Option<&Node> {
        let by_depth = name.strip_prefix('@').and_then(|d| d.parse::<i32>().ok());
        self.children.iter().chain(&self.extra).find(|n| match by_depth {
            // (F-HUD fix) an attached Loader child without an instance name is NAMED "@depth" (attach_first_child)
            Some(d) => (n.name.is_none() || n.name.as_deref() == Some(name)) && n.depth == d,
            None => n.name.as_deref() == Some(name),
        })
    }

    pub fn child_mut(&mut self, name: &str) -> Option<&mut Node> {
        let by_depth = name.strip_prefix('@').and_then(|d| d.parse::<i32>().ok());
        self.children.iter_mut().chain(&mut self.extra).find(|n| match by_depth {
            // (F-HUD fix) an attached Loader child without an instance name is NAMED "@depth" (attach_first_child)
            Some(d) => (n.name.is_none() || n.name.as_deref() == Some(name)) && n.depth == d,
            None => n.name.as_deref() == Some(name),
        })
    }

    fn descend_mut<S: AsRef<str>>(&mut self, path: &[S]) -> Option<&mut Sprite> {
        let mut s = self;
        for name in path {
            s = s.child_mut(name.as_ref())?.sprite_mut()?;
        }
        Some(s)
    }

    fn descend<S: AsRef<str>>(&self, path: &[S]) -> Option<&Sprite> {
        let mut s = self;
        for name in path {
            s = s.child(name.as_ref())?.sprite()?;
        }
        Some(s)
    }

    /// `parent.addChild(child)` for a child that is already there: it moves to the top.
    pub fn bring_to_front(&mut self, name: &str) {
        let z = self.next_z;
        if let Some(node) = self.child_mut(name) {
            node.z = Some(z);
            self.next_z += 1;
        }
    }

    /// `removeChildren()`: the clip stays empty (what a script draws into it remains).
    pub fn remove_children(&mut self) {
        self.children.clear();
        self.extra.clear();
        self.emptied = true;
    }
}

/// A movie being played. A clone is a snapshot (a renderer on another thread draws it).
#[derive(Clone)]
pub struct Movie {
    pub def: Arc<MovieDef>,
    pub root: Sprite,
    ctx: Ctx,
    /// Frames ticked so far.
    pub frames: u64,
    /// Path lookups that found nothing (a port asked for a clip that is not on the display
    /// list): the newest few, for diagnosis.
    pub misses: Vec<String>,
    /// (fit) The description fields set by [`Movie::set_text_fit`] whose text is shrunk or scrolls.
    fits: Vec<FitField>,
}

/// (THE USER 2026-11-03: "...never cut off") One description field under the shared fit rule ([`crate::render::fit_box`]):
/// the field's own definition, the shrunk one it is drawn with, the whole text and its fit, and the first line its
/// window shows (a text longer than the field at the smallest size: [`Movie::scroll_fits`]).
#[derive(Clone, Debug)]
struct FitField {
    path: String,
    base: Arc<TextDef>,
    drawn: Arc<TextDef>,
    source: String,
    fit: crate::render::BoxFit,
    top: usize,
}

impl FitField {
    /// What the field shows: the whole text, or the window from `top`.
    fn shown(&self) -> String {
        if self.fit.whole() {
            self.source.clone()
        } else {
            self.fit.window(self.top)
        }
    }
}

/// (fit, THE USER 2026-11-03: "right stick scroll") THE MORE-ABOVE / MORE-BELOW MARKS of a scrolling field: KH3's own pad
/// icons `RstickUp` / `RstickDown` (IconText, the pad set the menus' guides use), each in a box of this size at the field's
/// right edge (top / bottom), where [`crate::render::fit_box`] keeps the lines away ([`crate::render::FIT_MARK_W`]). KH3
/// has no scroll mark on a help text (it has no help scroll); COMPOSED placement, KH3's icons.
const FIT_MARK_H: f32 = 24.0;
/// The marks' names, after the field's own, in the field's parent clip (script children).
const FIT_MARK_UP: &str = "~fitMoreUp";
const FIT_MARK_DOWN: &str = "~fitMoreDown";
/// A character number for the marks (no character of a movie has it; it only tells them apart in the dirty rectangles).
const FIT_MARK_CH: u32 = 0x7F60_0000;

fn split(path: &str) -> Vec<&str> {
    path.split('.').filter(|p| !p.is_empty()).collect()
}

impl Movie {
    pub fn new(def: Arc<MovieDef>) -> Movie {
        let mut ctx = Ctx { serial: 1, events: Vec::new(), path: Vec::new(), language: Arc::from("none"), world: Arc::from("zz"), nesting: 0 };
        let root = Sprite::new(def.clone(), def.root.clone(), &mut ctx);
        Movie { def, root, ctx, frames: 0, misses: Vec::new(), fits: Vec::new() }
    }

    /// One frame of the stage (1/60 s).
    pub fn tick(&mut self) {
        self.ctx.path.clear();
        self.root.tick(&mut self.ctx);
        self.ctx.serial += 1;
        self.frames += 1;
        self.tick_fits();
    }

    /// (fit) The fitted fields after a frame: a field the timeline placed again (a pane's `off` / `in`) gets its fitted
    /// size, its text (the window where it was) and its marks back.
    fn tick_fits(&mut self) {
        let fits = std::mem::take(&mut self.fits);
        for f in &fits {
            self.show_fit(f);
        }
        self.fits = fits;
    }

    /// (fit) A fitted field as it must look: the shrunk definition, the text or the window from `top`, and the marks of a
    /// scrolling text (up: lines above the window; down: lines below it).
    fn show_fit(&mut self, f: &FitField) {
        let want = f.shown();
        let Some(Node { kind: Kind::Text(t), matrix, .. }) = self.node_mut(&f.path) else { return };
        if !Arc::ptr_eq(&t.def, &f.drawn) {
            t.def = f.drawn.clone();
        }
        if t.text.as_deref() != Some(want.as_str()) {
            t.text = Some(want);
        }
        let at = *matrix;
        let marks = [(FIT_MARK_UP, "<icon(RstickUp)>", f.top > 0, true), (FIT_MARK_DOWN, "<icon(RstickDown)>", f.top < f.fit.scroll_max(), false)];
        let parts = split(&f.path);
        let Some((name, parents)) = parts.split_last() else { return };
        let Some(parent) = self.root.descend_mut(parents) else { return };
        for (mark, icon, shown, top) in marks {
            let mark_name = format!("{name}{mark}");
            let present = parent.extra.iter().position(|n| n.name.as_deref() == Some(mark_name.as_str()));
            match (shown, present) {
                (false, Some(i)) => {
                    parent.extra.remove(i);
                }
                (true, Some(i)) => parent.extra[i].matrix = at,
                (true, None) => {
                    let b = f.drawn.bounds;
                    let (y0, y1) = if top { (b.y0, b.y0 + FIT_MARK_H) } else { (b.y1 - FIT_MARK_H, b.y1) };
                    let def = Arc::new(TextDef {
                        id: 0,
                        bounds: crate::def::Rect0 { x0: b.x1 - crate::render::FIT_MARK_W, y0, x1: b.x1 - 2.0, y1 },
                        font_class: f.drawn.font_class.clone(),
                        size: f.drawn.size,
                        color: f.drawn.color,
                        align: crate::def::Align::Center,
                        initial: String::new(),
                        word_wrap: false,
                        leading: 0.0,
                    });
                    let kind = Kind::Text(TextNode { def, text: Some(icon.to_string()), adjust: None, icon_native: false });
                    parent.extra.push(Node { ch: FIT_MARK_CH, depth: 0, born: 1, z: None, name: Some(Arc::from(mark_name.as_str())), matrix: at, cx: Cx::IDENTITY, clip: 0, blend: 0, visible: true, script_visible: None, script_matrix: true, ratio: 0.0, kind });
                }
                (false, None) => {}
            }
        }
    }

    /// (THE USER 2026-11-03: "the textbox for item descriptions in our KH camp menu needs to accommodate text so that it is
    /// never cut off") `.text = value` on a DESCRIPTION / HELP field through the shared fit rule ([`crate::render::fit_box`]):
    /// a text that fits is set as it is; a longer one is drawn smaller, down to the readable minimum; one still longer
    /// shows a window of its lines from the top, which the right stick moves ([`Movie::scroll_fits`]). Setting the same
    /// text again changes nothing (the window stays where it is). A `TextFieldAdjust` clip or a field without word wrap: as
    /// [`Movie::set_text`].
    pub fn set_text_fit(&mut self, path: &str, text: &str, fonts: &crate::render::Fonts) -> bool {
        if let Some(f) = self.fits.iter().find(|f| f.path == path) {
            if f.source == text {
                return true;
            }
        }
        let base = match self.node(path) {
            Some(Node { kind: Kind::Text(t), .. }) => self.fits.iter().find(|f| f.path == path).map_or_else(|| t.def.clone(), |f| f.base.clone()),
            _ => return self.set_text(path, text, fonts),
        };
        let fit = crate::render::fit_box(fonts, &base, text);
        self.unfit(path);
        if fit.size == base.size && fit.whole() {
            return self.set_text(path, text, fonts);
        }
        let drawn = Arc::new(TextDef { size: fit.size, leading: fit.leading, ..(*base).clone() });
        // (THE USER 2026-11-03: "right stick scroll") a new text starts at its top
        let field = FitField { path: path.to_string(), base, drawn, source: text.to_string(), fit, top: 0 };
        self.show_fit(&field);
        self.fits.push(field);
        true
    }

    /// (THE USER 2026-11-03: "right stick scroll") THE RIGHT STICK ON THE DESCRIPTIONS: every fitted field whose text runs
    /// past it at the smallest size moves its window `lines` lines (negative: up), kept between the top and the last
    /// line; its marks follow. The host hands the stick's steps here (kh-host `MenuInput::take_scroll`: one line a
    /// press, KH3's menu key repeat while held). True: a window moved.
    pub fn scroll_fits(&mut self, lines: i32) -> bool {
        if lines == 0 {
            return false;
        }
        let mut fits = std::mem::take(&mut self.fits);
        let mut moved = false;
        for f in &mut fits {
            let top = (f.top as i64 + i64::from(lines)).clamp(0, f.fit.scroll_max() as i64) as usize;
            if top != f.top {
                f.top = top;
                moved = true;
                self.show_fit(f);
            }
        }
        self.fits = fits;
        moved
    }

    /// (fit) The first line a scrolling field's window shows (0 for any other field).
    pub fn fit_top(&self, path: &str) -> usize {
        self.fits.iter().find(|f| f.path == path).map_or(0, |f| f.top)
    }

    /// (fit) What the shared fit rule made of a field's text, when it is shrunk or scrolls (`None`: drawn as authored).
    pub fn text_fit(&self, path: &str) -> Option<&crate::render::BoxFit> {
        self.fits.iter().find(|f| f.path == path).map(|f| &f.fit)
    }

    /// (fit) The whole text of a field (a scrolling one shows a window of it).
    pub fn text_whole(&self, path: &str) -> Option<String> {
        self.fits.iter().find(|f| f.path == path).map(|f| f.source.clone()).or_else(|| self.text(path))
    }

    /// (fit) A field leaves the fit rule: its own definition again, no marks.
    fn unfit(&mut self, path: &str) {
        let Some(at) = self.fits.iter().position(|f| f.path == path) else { return };
        let f = self.fits.remove(at);
        if let Some(Node { kind: Kind::Text(t), .. }) = self.node_mut(path) {
            t.def = f.base;
        }
        let parts = split(path);
        if let Some((name, parents)) = parts.split_last() {
            if let Some(parent) = self.root.descend_mut(parents) {
                let marks = [format!("{name}{FIT_MARK_UP}"), format!("{name}{FIT_MARK_DOWN}")];
                parent.extra.retain(|n| !n.name.as_deref().is_some_and(|m| marks.iter().any(|k| k == m)));
            }
        }
    }

    /// The `AnimEvent.END` events dispatched since the last call.
    pub fn take_events(&mut self) -> Vec<AnimEnd> {
        std::mem::take(&mut self.ctx.events)
    }

    fn miss(&mut self, path: &str) {
        if self.misses.len() < 64 && !self.misses.iter().any(|m| m == path) {
            self.misses.push(path.to_string());
        }
    }

    pub fn sprite(&self, path: &str) -> Option<&Sprite> {
        self.root.descend(&split(path))
    }

    pub fn sprite_mut(&mut self, path: &str) -> Option<&mut Sprite> {
        self.root.descend_mut(&split(path))
    }

    pub fn node(&self, path: &str) -> Option<&Node> {
        let parts = split(path);
        let (last, parents) = parts.split_last()?;
        self.root.descend(parents)?.child(last)
    }

    pub fn node_mut(&mut self, path: &str) -> Option<&mut Node> {
        let parts = split(path);
        let (last, parents) = parts.split_last()?;
        self.root.descend_mut(parents)?.child_mut(last)
    }

    pub fn exists(&self, path: &str) -> bool {
        path.is_empty() || self.node(path).is_some()
    }

    fn with_sprite(&mut self, path: &str, f: impl FnOnce(&mut Sprite, &mut Ctx)) -> bool {
        let parts = split(path);
        self.ctx.path = parts.iter().map(|p| Arc::from(*p)).collect();
        let found = match self.root.descend_mut(&parts) {
            Some(s) => {
                f(s, &mut self.ctx);
                true
            }
            None => false,
        };
        self.ctx.path.clear();
        if !found {
            self.miss(path);
        }
        found
    }

    /// `clip.gotoAndPlay("label")`.
    pub fn goto_and_play(&mut self, path: &str, label: &str) -> bool {
        let mut ok = false;
        self.with_sprite(path, |s, ctx| ok = s.goto_label(label, true, ctx));
        ok
    }

    /// `clip.gotoAndStop("label")`.
    pub fn goto_and_stop(&mut self, path: &str, label: &str) -> bool {
        let mut ok = false;
        self.with_sprite(path, |s, ctx| ok = s.goto_label(label, false, ctx));
        ok
    }

    /// `clip.gotoAndStop(frame)`.
    pub fn goto_frame(&mut self, path: &str, frame: u32) -> bool {
        self.with_sprite(path, |s, ctx| s.goto(frame.max(1), false, ctx))
    }

    /// `clip.gotoAndPlay(frame)`.
    pub fn play_from(&mut self, path: &str, frame: u32) -> bool {
        self.with_sprite(path, |s, ctx| s.goto(frame.max(1), true, ctx))
    }

    pub fn stop(&mut self, path: &str) -> bool {
        self.with_sprite(path, |s, _| s.playing = false)
    }

    pub fn play(&mut self, path: &str) -> bool {
        self.with_sprite(path, |s, _| s.playing = true)
    }

    /// AS3 `currentLabel`: the label at or before the current frame.
    pub fn current_label(&self, path: &str) -> Option<&str> {
        self.sprite(path)?.current_label()
    }

    pub fn current_frame(&self, path: &str) -> Option<u32> {
        self.sprite(path).map(|s| s.frame)
    }

    /// `object.visible = v`.
    pub fn set_visible(&mut self, path: &str, visible: bool) -> bool {
        match self.node_mut(path) {
            Some(node) => {
                node.script_visible = Some(visible);
                true
            }
            None => {
                self.miss(path);
                false
            }
        }
    }

    /// `object.x = x; object.y = y; object.rotation = degrees` (scale 1).
    pub fn set_position(&mut self, path: &str, x: f32, y: f32, rotation_deg: f32) -> bool {
        match self.node_mut(path) {
            Some(node) => {
                let (s, c) = rotation_deg.to_radians().sin_cos();
                node.matrix = Matrix::new(c, s, -s, c, x, y);
                node.script_matrix = true;
                true
            }
            None => {
                self.miss(path);
                false
            }
        }
    }

    /// `.text = value` on a text field, or on a `TextFieldAdjust` clip (then its `textField`
    /// child gets the text, squeezed as `updateText` does). `fonts` gives the advance widths.
    pub fn set_text(&mut self, path: &str, text: &str, fonts: &crate::render::Fonts) -> bool {
        self.unfit(path); // (fit) a plain text: the field's own size again
        let found = match self.node_mut(path) {
            Some(Node { kind: Kind::Text(t), .. }) => {
                t.text = Some(text.to_string());
                true
            }
            Some(Node { kind: Kind::Sprite(s), .. }) => crate::controls::text_adjust_set(s, text, fonts),
            _ => false,
        };
        if !found {
            self.miss(path);
        }
        found
    }

    /// (BN) A text field's inline icons at the IconText table's own size ([`TextNode::icon_native`]). False: no such field.
    pub fn set_text_icon_native(&mut self, path: &str, on: bool) -> bool {
        match self.node_mut(path) {
            Some(Node { kind: Kind::Text(t), .. }) => {
                t.icon_native = on;
                true
            }
            _ => false,
        }
    }

    /// The text a field (or a `TextFieldAdjust` clip) shows.
    pub fn text(&self, path: &str) -> Option<String> {
        match &self.node(path)?.kind {
            Kind::Text(t) => Some(t.shown().to_string()),
            Kind::Sprite(s) => match &s.child("textField")?.kind {
                Kind::Text(t) => Some(t.shown().to_string()),
                _ => None,
            },
            _ => None,
        }
    }

    /// `Loader` content: the first child of `other`'s root timeline becomes a script child of the
    /// clip at `path`, at (0, 0), under the name it has in its own movie. Returns that name.
    pub fn attach_first_child(&mut self, path: &str, other: &Arc<MovieDef>) -> Option<String> {
        let op = other.root.ops.first()?.iter().find(|op| !op.remove && op.ch.is_some())?.clone();
        let parts = split(path);
        self.ctx.path = parts.iter().map(|p| Arc::from(*p)).collect();
        let mut node = make_node(other, &op, op.ch?, 1, &mut self.ctx);
        self.ctx.path.clear();
        apply_attrs(&mut node, &op);
        node.matrix.tx = 0.0;
        node.matrix.ty = 0.0;
        node.script_matrix = true;
        node.visible = true;
        let name = node.label();
        if node.name.is_none() {
            node.name = Some(Arc::from(name.as_str()));
        }
        let host = self.root.descend_mut(&parts)?;
        host.extra.clear();
        host.extra.push(node);
        Some(name)
    }

    /// (party P) Scales the clip at `path` by `s` about its own origin (its matrix's linear part).
    pub fn scale_node(&mut self, path: &str, s: f32) -> bool {
        match self.node_mut(path) {
            Some(node) => {
                node.matrix.a *= s;
                node.matrix.b *= s;
                node.matrix.c *= s;
                node.matrix.d *= s;
                node.script_matrix = true;
                true
            }
            None => false,
        }
    }

    /// `parent.addChild(new Class())`: a new instance of the symbol with that AS3 class becomes a
    /// script child of the clip at `path`, under `name`. False when the class or the clip is unknown.
    pub fn add_instance(&mut self, path: &str, class: &str, name: &str) -> bool {
        let parts = split(path);
        // The class is looked up in the movie the host clip belongs to (a library symbol's own
        // movie), then in this movie.
        let Some(host_movie) = self.root.descend(&parts).map(|s| s.movie.clone()) else { return false };
        let (movie, ch) = match host_movie.symbols.get(class).copied() {
            Some(ch) => (host_movie, ch),
            None => match self.def.symbols.get(class) {
                Some(ch) => (self.def.clone(), *ch),
                None => return false,
            },
        };
        self.ctx.path = parts.iter().map(|p| Arc::from(*p)).collect();
        let op = Op { depth: 0, ch: Some(ch), name: Some(Arc::from(name)), ..Op::default() };
        let mut node = make_node(&movie, &op, ch, 1, &mut self.ctx);
        self.ctx.path.clear();
        node.name = Some(Arc::from(name));
        node.script_matrix = true;
        let Some(host) = self.root.descend_mut(&parts) else { return false };
        host.extra.retain(|n| n.name.as_deref() != Some(name));
        host.extra.push(node);
        true
    }

    /// (CAMP-OPTIONS) A new instance of character `ch` of ANOTHER movie (`def`: e.g. KH3's PC SettingMenu's category icons in
    /// the camp menu's tab bar) as the script child `name` of the clip at `path`, at the origin. False when the clip is unknown.
    pub fn add_foreign(&mut self, path: &str, def: &Arc<MovieDef>, ch: u32, name: &str) -> bool {
        let parts = split(path);
        if self.root.descend(&parts).is_none() || def.sprite(ch).is_none() {
            return false;
        }
        self.ctx.path = parts.iter().map(|p| Arc::from(*p)).collect();
        let op = Op { depth: 0, ch: Some(ch), name: Some(Arc::from(name)), ..Op::default() };
        let mut node = make_node(def, &op, ch, 1, &mut self.ctx);
        self.ctx.path.clear();
        node.name = Some(Arc::from(name));
        node.script_matrix = true;
        let Some(host) = self.root.descend_mut(&parts) else { return false };
        host.extra.retain(|n| n.name.as_deref() != Some(name));
        host.extra.push(node);
        true
    }

    /// (CAMP-OPTIONS) A new instance of the character of the node at `source` (a full path anywhere in the movie: a library
    /// symbol another page placed, e.g. the Items screen's `ListTabCamp`) as the script child `name` of the clip at `path`,
    /// at the origin. False when either is unknown.
    pub fn add_copy_of(&mut self, path: &str, source: &str, name: &str) -> bool {
        let source_parts = split(source);
        let Some((last, parent)) = source_parts.split_last() else { return false };
        let Some(source_host) = self.root.descend(parent) else { return false };
        let Some(child) = source_host.children.iter().chain(&source_host.extra).find(|n| n.name.as_deref() == Some(*last)) else { return false };
        let (movie, ch) = (source_host.movie.clone(), child.ch);
        let parts = split(path);
        if self.root.descend(&parts).is_none() {
            return false;
        }
        self.ctx.path = parts.iter().map(|p| Arc::from(*p)).collect();
        let op = Op { depth: 0, ch: Some(ch), name: Some(Arc::from(name)), ..Op::default() };
        let mut node = make_node(&movie, &op, ch, 1, &mut self.ctx);
        self.ctx.path.clear();
        node.name = Some(Arc::from(name));
        node.script_matrix = true;
        let Some(host) = self.root.descend_mut(&parts) else { return false };
        host.extra.retain(|n| n.name.as_deref() != Some(name));
        host.extra.push(node);
        true
    }

    /// (party P) A new instance of the same character as the timeline child `source` of the clip at `path` (a symbol
    /// the movie placed from a linked library, whose class the movie's own table does not name) becomes a script
    /// child under `name`, at `source`'s matrix. False when the clip or the child is unknown.
    pub fn clone_child(&mut self, path: &str, source: &str, name: &str) -> bool {
        let parts = split(path);
        let Some(host) = self.root.descend(&parts) else { return false };
        let Some(child) = host.children.iter().find(|n| n.name.as_deref() == Some(source)) else { return false };
        let (movie, ch, matrix) = (host.movie.clone(), child.ch, child.matrix);
        self.ctx.path = parts.iter().map(|p| Arc::from(*p)).collect();
        let op = Op { depth: 0, ch: Some(ch), name: Some(Arc::from(name)), ..Op::default() };
        let mut node = make_node(&movie, &op, ch, 1, &mut self.ctx);
        self.ctx.path.clear();
        node.name = Some(Arc::from(name));
        node.matrix = matrix;
        node.script_matrix = true;
        let Some(host) = self.root.descend_mut(&parts) else { return false };
        host.extra.retain(|n| n.name.as_deref() != Some(name));
        host.extra.push(node);
        true
    }

    /// (party PF) A FREE INSTANCE of the symbol of `movie` with that AS3 class / export name (a library's own symbol:
    /// `ItemListLibrary_fla.ListCursorAnm_forS_32` ...), named `name`, to be pushed into a clip's script children by the
    /// caller (`Sprite::extra`). None when the movie has no such symbol.
    pub fn instance_of(movie: &Arc<MovieDef>, class: &str, name: &str, label: Option<&str>) -> Option<Node> {
        let ch = *movie.symbols.get(class)?;
        let mut ctx = Ctx { serial: 1, events: Vec::new(), path: vec![Arc::from(name)], language: Arc::from("none"), world: Arc::from("zz"), nesting: 0 };
        let op = Op { depth: 0, ch: Some(ch), name: Some(Arc::from(name)), ..Op::default() };
        let mut node = make_node(movie, &op, ch, 1, &mut ctx);
        node.name = Some(Arc::from(name));
        node.script_matrix = true;
        // its timeline at that label, playing (`gotoAndPlay`)
        if let (Some(label), Kind::Sprite(s)) = (label, &mut node.kind) {
            ctx.serial += 1;
            s.goto_label(label, true, &mut ctx);
        }
        Some(node)
    }

    /// `parent.removeChild(child)` for a script child.
    pub fn remove_instance(&mut self, path: &str, name: &str) {
        if let Some(s) = self.sprite_mut(path) {
            s.extra.retain(|n| n.name.as_deref() != Some(name));
        }
    }

    /// Removes what `attach_first_child` added.
    pub fn detach(&mut self, path: &str) {
        if let Some(s) = self.sprite_mut(path) {
            s.extra.clear();
        }
    }

    /// Every sprite's frame, text and script-set visibility by instance path: the `state` object
    /// of the reference renderer `tools/lib/gfx_render.js` (frames / text / visible).
    pub fn dump_state(&self) -> serde_json::Value {
        let mut frames = serde_json::Map::new();
        let mut text = serde_json::Map::new();
        let mut visible = serde_json::Map::new();
        fn walk(s: &Sprite, path: &str, frames: &mut serde_json::Map<String, serde_json::Value>, text: &mut serde_json::Map<String, serde_json::Value>, visible: &mut serde_json::Map<String, serde_json::Value>) {
            frames.insert(path.to_string(), s.frame.into());
            for node in s.children.iter().chain(&s.extra) {
                let p = if path.is_empty() { node.label() } else { format!("{path}.{}", node.label()) };
                if let Some(v) = node.script_visible {
                    visible.insert(p.clone(), v.into());
                }
                match &node.kind {
                    Kind::Sprite(child) => walk(child, &p, frames, text, visible),
                    Kind::Text(t) => {
                        if let Some(value) = &t.text {
                            text.insert(p, value.clone().into());
                        }
                    }
                    _ => {}
                }
            }
        }
        walk(&self.root, "", &mut frames, &mut text, &mut visible);
        serde_json::json!({ "frames": frames, "text": text, "visible": visible })
    }
}
