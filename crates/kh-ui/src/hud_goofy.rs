//! (GOOFY) GOOFY'S HUD PIECES OUTSIDE THE STATUS CLUSTER (documentation/GOOFY_OVERHAUL.md 4.3, 5; build/sora_tmp/party/GOOFY/PLAN.md
//! R20). KH3 has neither: they are THE PORT'S, built from what KH3's movies hold, with no new movie or packed texture.
//!
//! - THE STAGGER GAUGE (5): a second instance of `HudRight`'s armour gauge (`HudRight_fla.McArmorHpSet_83`, its clips `armorHpGg`,
//!   `armorHpGgR`, the glow `armorHpEff`), which KH3 only ever swaps in place of the HP gauge (`setEnableAHP`). It is an instance
//!   of its own under `hudRight`, so the pack's in / out timeline and KH3's Scan rule, which hide the HP gauge, never hide it:
//!   R20 "must not be gated by Donald's Scan". It sits under the HP gauge (`hud.stagger_*`). The fill is the share of the book's
//!   max; a loss runs through KH3's red-gauge timer `FollowGauge`, as `setAHP` does; BROKEN plays the glow and blinks the fill.
//! - THE RED "!" (4.3): no art exists, so one texture is made at run time from the "!" of KH3's menu font: a red ramp, a dark
//!   outline and a red glow. One [`PerilMarks`] clip per enemy on `HudTarget`'s lock-on plane, over the enemy's head.
//!   WHEN IT SHOWS, tuned to read at a glance: it pops [`PerilLook::lead`] seconds before the hit window opens (Danger
//!   Sense adds its lead), stays until the window opens, for at least [`PerilLook::min_show`] seconds so a late warning still
//!   reads, then fades in [`PerilLook::fade`]. An attack that is called off fades at once. The plain "!" never says which kind
//!   of attack.
//! - DANGER SENSE'S WORD (4.3, 11.3; THE USER 2026-11-02: "Danger Sense ability should indicate what kind of attack it will be"):
//!   with the ability worn, the cue carries its [`PerilKind`] and a word naming it (STRIKE, THRUST, SWEEP, GRAB, HEAVY,
//!   PROJECTILE, MAGIC: COMPOSED, no KH3 string; sheet `hud.peril_word_*`) sits under the "!" in KH3's menu font, each kind in a
//!   colour of its own with the "!"'s dark outline. It is drawn in the "!"'s own clip, so it pops, fades, goes and sits on the
//!   screen's edge exactly with it; on the edge the pair is kept inside the stage as a whole.

use std::sync::Arc;

use glam::Vec3;

use crate::controls::FollowGauge;
use crate::def::{BitmapFill, Glyph, Seg, Texture};
use crate::frames::gauge_frame;
use crate::geom::{Cx, Matrix, Rect};
use crate::hud_resonance::{ensure_art_of, set_art};
use crate::hud_side::HudRight;
use crate::hud_target::{HudTarget, View};
use crate::player::GfxRect;
use crate::raster::{self, Coverage};
use crate::render::Fonts;

const STAGGER_CLASS: &str = "HudRight_fla.McArmorHpSet_83";
const STAGGER_PARENT: &str = "hudRight";
/// The instance's path.
pub const STAGGER: &str = "hudRight.gofStagger";
const STAGGER_FILL: &str = "hudRight.gofStagger.armorHp.armorHpGg";
const STAGGER_LOSS: &str = "hudRight.gofStagger.armorHp.armorHpGgR";
const STAGGER_LABEL: &str = "hudRight.gofStagger.armorHp.coArmorHpTxt";
const STAGGER_BODY: &str = "hudRight.gofStagger.armorHp";
const STAGGER_LABEL_ART: &str = "hudRight.gofStagger.armorHp.gofLabel";
/// The class the label's art node is made from (a one-shape clip of the movie, emptied).
const STAGGER_ART_CLASS: &str = "CoArmorHpGgTex";
const STAGGER_GLOW: &str = "hudRight.gofStagger.armorHpEff";

/// One target's Stagger as the HUD draws it.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct StaggerValues {
    pub value: f32,
    pub max: f32,
    pub broken: bool,
}

/// The look (sheet `party-guard` rows `hud.stagger_*`): stage units, colours as straight RGBA 0..1.
#[derive(Clone, Debug, PartialEq)]
pub struct StaggerLook {
    /// The gauge's place on the stage part `hudRight`.
    pub at: (f32, f32),
    pub fill: [f32; 4],
    pub loss: [f32; 4],
    pub broken: [f32; 4],
    pub blink_hz: f32,
    /// The word in the gauge's tab, its right end and height centre in the gauge's units, its size.
    pub label: String,
    pub label_at: (f32, f32),
    pub label_size: (f32, f32),
    /// While the gauge shows, the right-hand popups stand this much lower: it is where their stack starts.
    pub panel_shift: f32,
}

impl Default for StaggerLook {
    fn default() -> StaggerLook {
        StaggerLook { at: (-75.0, 76.0), fill: [1.0; 4], loss: [1.0, 0.28, 0.18, 1.0], broken: [1.0, 1.0, 0.9, 1.0], blink_hz: 6.0, label: "STAGGER".to_string(), label_at: (-8.0, 18.0), label_size: (86.0, 13.0), panel_shift: 36.0 }
    }
}

/// A flat colour over the gauge's art.
fn tint(c: [f32; 4]) -> Cx {
    Cx { mul: [c[0], c[1], c[2], c[3]], add: [0.0; 4] }
}

/// The gauge's driver (one per HUD).
#[derive(Clone, Debug, Default)]
pub struct StaggerGauge {
    installed: bool,
    pub shown: bool,
    target: Option<u64>,
    loss: FollowGauge,
    frame: i32,
    loss_frame: i32,
    broken: bool,
    blink: f32,
    /// What was last applied (tests): (fill frame, red frame, broken).
    pub last: (i32, i32, bool),
}

impl StaggerGauge {
    fn install(&mut self, hud: &mut HudRight, look: &StaggerLook) -> bool {
        let m = &mut hud.movie;
        if !m.exists(STAGGER) && !m.add_instance(STAGGER_PARENT, STAGGER_CLASS, "gofStagger") {
            return false;
        }
        m.stop(STAGGER_FILL);
        m.stop(STAGGER_LOSS);
        m.set_visible(STAGGER_LABEL, false); // the movie's text says "ARMOR": the sheet's label takes its place in the tab
        m.set_visible(STAGGER, false);
        if let Some(label) = label_texture(&hud.fonts, &look.label).map(Arc::new) {
            let m = &mut hud.movie;
            if ensure_art_of(m, STAGGER_ART_CLASS, STAGGER_BODY, "gofLabel") {
                // right-aligned in the tab, the sheet's size (the tab is small: the word is drawn narrower than the font's)
                let (w, h) = look.label_size;
                let (x, y) = (look.label_at.0 - w, look.label_at.1 - h / 2.0);
                let fill = BitmapFill { matrix: Matrix::new(w / label.width as f32, 0.0, 0.0, h / label.height as f32, x, y), texture: label, scale: (1.0, 1.0), repeat: false, smooth: true };
                set_art(m, STAGGER_LABEL_ART, vec![GfxRect { x, y, w, h, fill }], Cx::IDENTITY, true);
            }
        }
        self.installed = true;
        true
    }

    /// One update of the right-hand HUD, after its stage frames: `v` = the target and its values, None = no gauge.
    pub fn update(&mut self, hud: &mut HudRight, v: Option<(u64, &StaggerValues)>, look: &StaggerLook, dt: f32) {
        let Some((key, v)) = v else {
            if self.shown {
                hud.movie.goto_and_play(STAGGER, "out");
                hud.movie.goto_and_play(STAGGER_GLOW, "out");
            }
            self.shown = false;
            self.target = None;
            hud.set_stack_shift(0.0);
            return;
        };
        if !hud.movie.exists(STAGGER) {
            self.installed = false; // a new movie (the popups' reset)
            self.shown = false;
        }
        if !self.installed && !self.install(hud, look) {
            return;
        }
        let max = v.max.max(1.0);
        let frame = gauge_frame((v.value.clamp(0.0, max) / max * 1000.0).round() as i32, 1000, 500).clamp(0, 500);
        let m = &mut hud.movie;
        let fresh = self.target != Some(key);
        if fresh {
            self.target = Some(key);
            self.frame = frame;
            self.loss = FollowGauge::with_scale(2.0);
            self.loss_frame = frame;
            self.broken = false;
        }
        if !self.shown {
            self.shown = true;
            m.set_visible(STAGGER, true);
            if let Some(n) = m.node_mut(STAGGER) {
                n.matrix = Matrix::new(1.0, 0.0, 0.0, 1.0, look.at.0, look.at.1);
                n.script_matrix = true;
            }
            m.goto_and_play(STAGGER, "in");
            m.goto_and_play(STAGGER_GLOW, "in");
        }
        // a loss: the red part follows the fill down (`setAHP`, KH3's red-gauge timer)
        if frame < self.frame {
            if let Some(f) = self.loss.set_gauge(self.frame.max(self.loss_frame) + 1, frame + 1) {
                self.loss_frame = f as i32 - 1;
            }
        } else if frame > self.frame {
            self.loss = FollowGauge::with_scale(2.0);
            self.loss_frame = frame;
        }
        if let Some(f) = self.loss.advance(dt as f64 * 1000.0) {
            self.loss_frame = f as i32 - 1;
        }
        self.frame = frame;
        if v.broken != self.broken {
            self.broken = v.broken;
            if v.broken {
                m.goto_and_play(STAGGER_GLOW, "in");
            }
        }
        self.blink = (self.blink + dt * look.blink_hz.max(0.0)).fract();
        m.goto_frame(STAGGER_FILL, frame as u32 + 1);
        let lost = self.loss_frame > frame;
        m.set_visible(STAGGER_LOSS, lost);
        if lost {
            m.goto_frame(STAGGER_LOSS, self.loss_frame as u32 + 1);
            if let Some(n) = m.node_mut(STAGGER_LOSS) {
                n.cx = tint(look.loss);
            }
        }
        // BROKEN: the full gauge flashes towards the sheet's colour, over the art
        let cx = if v.broken && self.blink < 0.5 { Cx { mul: [1.0; 4], add: [look.broken[0] * 150.0, look.broken[1] * 150.0, look.broken[2] * 150.0, 0.0] } } else { tint(look.fill) };
        if let Some(n) = m.node_mut(STAGGER_FILL) {
            n.cx = cx;
        }
        self.last = (frame, if lost { self.loss_frame } else { 0 }, v.broken);
        hud.set_stack_shift(look.panel_shift);
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// THE RED "!"

/// One perilous attack on its way: `key` = the attacker (the host's id), `eta` = seconds until its hit window opens, `active` =
/// the window is open, `kind` = the kind of attack to name under the "!" (None: the plain "!"; the host fills it only while
/// Goofy wears Danger Sense).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerilCue {
    pub key: u64,
    pub eta: f32,
    pub active: bool,
    pub kind: Option<PerilKind>,
}

/// (Danger Sense) The kind of attack a "!" names: kh-core's `AttackClass`, one for one (this crate does not know kh-core;
/// kh-partyhost `custom::peril_kind` maps it). (THE USER 2026-11-02: "Danger Sense ability should indicate what kind of attack it
/// will be")
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PerilKind {
    Normal,
    Thrust,
    Sweep,
    Grab,
    Heavy,
    Projectile,
    Magic,
}

impl PerilKind {
    /// Every kind, in the order of [`PerilLook::words`].
    pub const ALL: [PerilKind; 7] = [PerilKind::Normal, PerilKind::Thrust, PerilKind::Sweep, PerilKind::Grab, PerilKind::Heavy, PerilKind::Projectile, PerilKind::Magic];
}

/// (Danger Sense) One kind's word under the "!": its text (COMPOSED, no KH3 string) and its face colour, straight RGBA 0..1.
#[derive(Clone, Debug, PartialEq)]
pub struct PerilWord {
    pub text: String,
    pub color: [f32; 4],
}

impl PerilWord {
    fn new(text: &str, color: [f32; 4]) -> PerilWord {
        PerilWord { text: text.to_string(), color }
    }
}

/// The look and the timing (sheet `party-guard` rows `hud.peril_*`): seconds, stage pixels, straight RGBA 0..1.
#[derive(Clone, Debug, PartialEq)]
pub struct PerilLook {
    /// It shows this long before the hit window opens.
    pub lead: f32,
    /// A late warning stays up at least this long.
    pub min_show: f32,
    /// The pop: this long, from this scale down to 1.
    pub pop: f32,
    pub pop_scale: f32,
    pub fade: f32,
    /// Its height on the stage, and how far over the enemy's head its centre sits.
    pub size: f32,
    pub lift: f32,
    pub color: [f32; 4],
    pub outline: [f32; 4],
    pub glow: [f32; 4],
    /// (GF-POLISH) A mark stays this far inside the stage: an attacker off the screen shows on the edge toward it.
    pub edge: f32,
    /// (GF-POLISH) The static SE of a mark's pop (below 0: none), and the least time between two.
    pub se: i32,
    pub se_gap: f32,
    /// (Danger Sense) The word under the "!": its height on the stage (its edge included), the gap from the foot of the "!"'s
    /// glyph to its top, the widest it is drawn (a longer word is drawn narrower).
    pub word_h: f32,
    pub word_gap: f32,
    pub word_max_w: f32,
    /// (Danger Sense) One word per [`PerilKind`], in [`PerilKind::ALL`]'s order.
    pub words: [PerilWord; 7],
}

impl PerilLook {
    /// The word of a kind.
    pub fn word(&self, kind: PerilKind) -> &PerilWord {
        &self.words[kind as usize]
    }
}

impl Default for PerilLook {
    fn default() -> PerilLook {
        // the words: COMPOSED (no KH3 string), the sheet's
        let words = [
            PerilWord::new("STRIKE", [1.0, 1.0, 1.0, 1.0]),
            PerilWord::new("THRUST", [1.0, 0.86, 0.25, 1.0]),
            PerilWord::new("SWEEP", [0.45, 1.0, 0.45, 1.0]),
            PerilWord::new("GRAB", [0.95, 0.5, 1.0, 1.0]),
            PerilWord::new("HEAVY", [1.0, 0.56, 0.16, 1.0]),
            PerilWord::new("PROJECTILE", [0.38, 0.9, 1.0, 1.0]),
            PerilWord::new("MAGIC", [0.62, 0.62, 1.0, 1.0]),
        ];
        PerilLook { lead: 0.5, min_show: 0.3, pop: 0.08, pop_scale: 1.7, fade: 0.18, size: 96.0, lift: 58.0, color: [0.96, 0.07, 0.05, 1.0], outline: [0.13, 0.0, 0.0, 1.0], glow: [1.0, 0.16, 0.08, 0.75], edge: 72.0, se: 19, se_gap: 0.2, word_h: 28.0, word_gap: 5.0, word_max_w: 170.0, words }
    }
}

const STAGE: (f32, f32) = (1920.0, 1080.0);

/// (GF-POLISH) WHERE THE MARK OF AN ATTACKER OFF THE SCREEN SITS: on the stage's edge, `margin` in, on the line from the stage's centre
/// toward it. `clip`: its x and y in clip space, not divided by w: those keep their side behind the camera, where the divided
/// ones mirror. Dead behind, it is the bottom edge, nearest to him.
pub fn edge_place(clip: (f32, f32), margin: f32) -> (f32, f32) {
    let (half_w, half_h) = (STAGE.0 / 2.0, STAGE.1 / 2.0);
    let (dx, dy) = (clip.0 * half_w, -clip.1 * half_h);
    let (dx, dy) = if dx.abs() + dy.abs() < 1.0e-4 { (0.0, 1.0) } else { (dx, dy) };
    let (room_x, room_y) = ((half_w - margin).max(0.0), (half_h - margin).max(0.0));
    let scale = (if dx != 0.0 { room_x / dx.abs() } else { f32::MAX }).min(if dy != 0.0 { room_y / dy.abs() } else { f32::MAX });
    (half_w + dx * scale, half_h + dy * scale)
}

/// How many enemies can show one at once.
pub const PERIL_MARKS: usize = 12;
const MARK_W: usize = 96;
const MARK_H: usize = 144;
/// The menu font's "!" is wide: drawn at this share of its width.
const MARK_NARROW: f32 = 0.72;
/// The room above and below the glyph in the "!"'s texture, for its outline and glow (texture pixels).
const MARK_ROOM: f32 = 22.0;
/// (Danger Sense) The word's texture: this tall (about twice the sheet's height, so it stays sharp) with this outline.
const WORD_H: usize = 56;
const WORD_EDGE: i32 = 5;

fn mark_path(i: usize) -> String {
    format!("{}.gofPeril{i}", crate::hud_arcane::TARGET_PLANE)
}

/// The bounds of glyphs laid out along a line, in font units.
fn glyph_bounds(glyphs: &[(&Glyph, f32)]) -> Rect {
    let mut b = Rect::EMPTY;
    for (glyph, dx) in glyphs {
        for seg in glyph.contours.iter().flatten() {
            match *seg {
                Seg::Move(x, y) | Seg::Line(x, y) => b.add_point(x + dx, y),
                Seg::Quad(qx, qy, x, y) => {
                    b.add_point(qx + dx, qy);
                    b.add_point(x + dx, y);
                }
            }
        }
    }
    b
}

/// The coverage of glyphs laid out along a line, through `m`, on a canvas of `w` x `h`: an even-odd fill, as the text renderer's.
fn glyph_coverage(glyphs: &[(&Glyph, f32)], m: &Matrix, w: usize, h: usize) -> Option<Coverage> {
    let mut edges = Vec::new();
    for (glyph, dx) in glyphs {
        for contour in &glyph.contours {
            let (mut x, mut y, mut sx, mut sy) = (0.0, 0.0, 0.0, 0.0);
            for seg in contour {
                match *seg {
                    Seg::Move(px, py) => (x, y, sx, sy) = (px + dx, py, px + dx, py),
                    Seg::Line(px, py) => {
                        let (a, c) = (m.apply(x, y), m.apply(px + dx, py));
                        edges.push([a.0, a.1, c.0, c.1]);
                        (x, y) = (px + dx, py);
                    }
                    Seg::Quad(qx, qy, px, py) => {
                        raster::flatten(m.apply(x, y), m.apply(qx + dx, qy), m.apply(px + dx, py), &mut edges);
                        (x, y) = (px + dx, py);
                    }
                }
            }
            if x != sx || y != sy {
                let (a, c) = (m.apply(x, y), m.apply(sx, sy));
                edges.push([a.0, a.1, c.0, c.1]);
            }
        }
    }
    let (mut cov, mut xs) = (Coverage::default(), Vec::new());
    raster::coverage(&edges, &Rect::new(0.0, 0.0, w as f32, h as f32), &mut cov, &mut xs).then_some(cov)
}

/// A coverage read anywhere (0 outside it).
fn cov_at(cov: &Coverage, x: i32, y: i32) -> f32 {
    let (lx, ly) = (x - cov.x0, y - cov.y0);
    if lx < 0 || ly < 0 || lx as usize >= cov.w || ly as usize >= cov.h {
        0.0
    } else {
        cov.cov[ly as usize * cov.w + lx as usize]
    }
}

/// A line of text in KH3's menu font, laid out to fill a canvas `h` pixels tall with `edge` pixels of room round it: its coverage
/// and the canvas's width. None when the UI pack has no font or the font lacks one of its letters.
fn text_coverage(fonts: &Fonts, text: &str, h: usize, edge: i32) -> Option<(Coverage, usize)> {
    let font = fonts.default.as_ref()?;
    let mut glyphs = Vec::new();
    let mut pen = 0.0;
    for c in text.chars() {
        let g = font.glyph(c as u32)?;
        glyphs.push((g, pen));
        pen += g.advance;
    }
    let b = glyph_bounds(&glyphs);
    if b.x1 <= b.x0 || b.y1 <= b.y0 {
        return None;
    }
    let k = (h as f32 - 2.0 * edge as f32 - 2.0) / (b.y1 - b.y0);
    let w = ((b.x1 - b.x0) * k).ceil() as usize + 2 * edge as usize + 4;
    let m = Matrix::new(k, 0.0, 0.0, k, edge as f32 + 2.0 - b.x0 * k, edge as f32 + 1.0 - b.y0 * k);
    Some((glyph_coverage(&glyphs, &m, w, h)?, w))
}

/// The most coverage within `edge` pixels of a pixel: the outline's strength there.
fn near_cov(cov: &Coverage, x: i32, y: i32, edge: i32) -> f32 {
    let mut near = 0.0f32;
    for dy in -edge..=edge {
        for dx in -edge..=edge {
            if dx * dx + dy * dy <= edge * edge {
                near = near.max(cov_at(cov, x + dx, y + dy));
            }
        }
    }
    near
}

/// The gauge's word as a texture, at twice the size it draws: the HUD labels' white face and dark edge, KH3's menu font.
fn label_texture(fonts: &Fonts, text: &str) -> Option<Texture> {
    const H: usize = 34;
    let edge = 3i32;
    let (cov, w) = text_coverage(fonts, text, H, edge)?;
    let mut rgba = vec![0u8; w * H * 4];
    for y in 0..H as i32 {
        for x in 0..w as i32 {
            let face = cov_at(&cov, x, y);
            let near = near_cov(&cov, x, y, edge);
            let a = face + near * (1.0 - face);
            let v = if a > 0.0 { face / a } else { 0.0 } * 255.0 * (0.92 - 0.2 * y as f32 / H as f32) + 26.0;
            let i = (y as usize * w + x as usize) * 4;
            rgba[i] = v.clamp(0.0, 255.0) as u8;
            rgba[i + 1] = v.clamp(0.0, 255.0) as u8;
            rgba[i + 2] = v.clamp(0.0, 255.0) as u8;
            rgba[i + 3] = (a * 255.0 + 0.5) as u8;
        }
    }
    Some(Texture { file: "(GOOFY) the Stagger gauge's word, made at run time from the menu font".to_string(), width: w as u32, height: H as u32, rgba })
}

/// (Danger Sense) A KIND'S WORD UNDER THE "!" as a texture, KH3's menu font: the kind's colour with a lighter top (the "!"'s own
/// ramp), the "!"'s dark outline round it. None when the UI pack has no font or the font lacks a letter of it. (THE USER
/// 2026-11-02: "Danger Sense ability should indicate what kind of attack it will be")
pub fn peril_word_texture(fonts: &Fonts, word: &PerilWord, outline: [f32; 4]) -> Option<Texture> {
    let (cov, w) = text_coverage(fonts, &word.text, WORD_H, WORD_EDGE)?;
    let mut rgba = vec![0u8; w * WORD_H * 4];
    for y in 0..WORD_H as i32 {
        let hot = (1.0 - y as f32 / WORD_H as f32 * 1.8).clamp(0.0, 1.0) * 0.5;
        let face_rgb = [0, 1, 2].map(|k| word.color[k] + (1.0 - word.color[k]) * hot);
        for x in 0..w as i32 {
            let face = cov_at(&cov, x, y) * word.color[3];
            let edge = near_cov(&cov, x, y, WORD_EDGE) * outline[3];
            let a = face + edge * (1.0 - face);
            let i = (y as usize * w + x as usize) * 4;
            for k in 0..3 {
                let c = if a > 0.0 { (face_rgb[k] * face + outline[k] * edge * (1.0 - face)) / a } else { 0.0 };
                rgba[i + k] = (c * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
            }
            rgba[i + 3] = (a * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        }
    }
    Some(Texture { file: "(GOOFY) Danger Sense's word under the red !, made at run time from the menu font".to_string(), width: w as u32, height: WORD_H as u32, rgba })
}

/// (Danger Sense) Where a mark's clip sits: `at` kept inside the stage's margin, the word under it too when there is one. `word`
/// = its drawn width and how far its bottom is under the clip's centre. The "!" keeps [`PerilLook::edge`] from the stage's
/// sides; the word keeps the room the "!"'s own drawing keeps inside that margin.
fn mark_place(at: (f32, f32), look: &PerilLook, word: Option<(f32, f32)>) -> (f32, f32) {
    let (h, w) = (look.size, look.size * MARK_W as f32 / MARK_H as f32);
    let (half_w, bottom) = word.map_or((0.0, 0.0), |(ww, b)| (ww / 2.0, b));
    let side = (look.edge + (half_w - w / 2.0).max(0.0)).min(STAGE.0 / 2.0);
    let foot = (look.edge + (bottom - h / 2.0).max(0.0)).min(STAGE.1 - look.edge);
    (at.0.clamp(side, STAGE.0 - side), at.1.max(look.edge).min(STAGE.1 - foot))
}

/// THE "!" OF KH3'S MENU FONT as a texture: the glyph's coverage, a ramp from a bright red top to the sheet's colour, a dark
/// outline, a soft glow around it (module text). None when the UI pack has no font.
pub fn peril_texture(fonts: &Fonts, look: &PerilLook) -> Option<Texture> {
    let font = fonts.default.as_ref()?;
    let glyphs = [(font.glyphs.get(&33)?, 0.0)];
    let b = glyph_bounds(&glyphs);
    let (gw, gh) = (b.x1 - b.x0, b.y1 - b.y0);
    if gw <= 0.0 || gh <= 0.0 {
        return None;
    }
    // the glyph fills the height, narrowed so it reads as a "!" at a glance, with room for the outline and the glow
    let room = MARK_ROOM;
    let ky = (MARK_H as f32 - 2.0 * room) / gh;
    let kx = ky * MARK_NARROW;
    let (ox, oy) = ((MARK_W as f32 - gw * kx) / 2.0 - b.x0 * kx, room - b.y0 * ky);
    let cov = glyph_coverage(&glyphs, &Matrix::new(kx, 0.0, 0.0, ky, ox, oy), MARK_W, MARK_H)?;
    let at = |x: i32, y: i32| cov_at(&cov, x, y);
    let (outline_px, glow_px) = (5i32, 18i32);
    let mut rgba = vec![0u8; MARK_W * MARK_H * 4];
    for y in 0..MARK_H as i32 {
        for x in 0..MARK_W as i32 {
            let inner = at(x, y);
            // the nearest glyph pixel, by a search of the glow's square
            let mut near = f32::MAX;
            if inner < 1.0 {
                for dy in -glow_px..=glow_px {
                    for dx in -glow_px..=glow_px {
                        if at(x + dx, y + dy) > 0.5 {
                            near = near.min(((dx * dx + dy * dy) as f32).sqrt());
                        }
                    }
                }
            } else {
                near = 0.0;
            }
            let edge = (outline_px as f32 + 0.5 - near).clamp(0.0, 1.0);
            let halo = if near < glow_px as f32 { (1.0 - near / glow_px as f32).powi(2) * look.glow[3] } else { 0.0 };
            // glow, then the outline over it, then the face over that
            let t = (y as f32 - room) / (MARK_H as f32 - 2.0 * room);
            let hot = (1.0 - t * 1.6).clamp(0.0, 1.0);
            let face = [look.color[0] + (1.0 - look.color[0]) * hot * 0.6, look.color[1] + (1.0 - look.color[1]) * hot * 0.42, look.color[2] + (1.0 - look.color[2]) * hot * 0.3];
            let mut c = [look.glow[0], look.glow[1], look.glow[2]];
            let mut a = halo;
            let mut over = |rgb: [f32; 3], alpha: f32| {
                let out = alpha + a * (1.0 - alpha);
                if out > 0.0 {
                    for k in 0..3 {
                        c[k] = (rgb[k] * alpha + c[k] * a * (1.0 - alpha)) / out;
                    }
                }
                a = out;
            };
            over([look.outline[0], look.outline[1], look.outline[2]], edge * look.outline[3]);
            over(face, inner * look.color[3]);
            let i = (y as usize * MARK_W + x as usize) * 4;
            for k in 0..3 {
                rgba[i + k] = (c[k] * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
            }
            rgba[i + 3] = (a * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        }
    }
    Some(Texture { file: "(GOOFY) the red ! made at run time from the menu font".to_string(), width: MARK_W as u32, height: MARK_H as u32, rgba })
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Mark {
    key: u64,
    clip: usize,
    age: f32,
    /// Seconds into its fade (None: not fading).
    fade: Option<f32>,
    /// (Danger Sense) The kind its word names: the cue's while it stands, the last one while it fades.
    kind: Option<PerilKind>,
}

/// What one mark looks like right now (tests, logs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerilShown {
    pub key: u64,
    pub alpha: f32,
    pub scale: f32,
    pub at: (f32, f32),
    pub on_screen: bool,
    /// (Danger Sense) The kind whose word is drawn under it, and the word's width on the stage (None, 0: the plain "!").
    pub word: Option<PerilKind>,
    pub word_w: f32,
}

/// The marks' driver (one per HUD).
#[derive(Clone, Debug, Default)]
pub struct PerilMarks {
    marks: Vec<Mark>,
    /// Attackers whose current attack already had its mark.
    spent: Vec<u64>,
    texture: Option<Arc<Texture>>,
    /// (Danger Sense) The kinds' words, made when first needed (None: the font could not draw it).
    words: Vec<(PerilKind, Option<Arc<Texture>>)>,
    installed: usize,
    /// (GF-POLISH) Marks that popped since [`PerilMarks::take_se`] last asked, and the seconds until a cue may sound again.
    popped: u32,
    se_wait: f32,
    /// What was last applied (tests).
    pub last: Vec<PerilShown>,
}

impl PerilMarks {
    /// The timing alone, one step: which enemies show a mark. `extra` = Danger Sense's lead.
    pub fn step(&mut self, cues: &[PerilCue], extra: f32, look: &PerilLook, dt: f32) {
        self.se_wait = (self.se_wait - dt).max(0.0);
        for m in &mut self.marks {
            m.age += dt;
            if let Some(f) = &mut m.fade {
                *f += dt;
            }
        }
        self.marks.retain(|m| m.fade.is_none_or(|f| f < look.fade.max(0.0)));
        // one cue per enemy: its next attack that has not opened, else the open one
        let mut keys: Vec<u64> = cues.iter().map(|c| c.key).collect();
        keys.sort_unstable();
        keys.dedup();
        let lead = look.lead + extra.max(0.0);
        for key in &keys {
            let cue = cues.iter().filter(|c| c.key == *key).min_by(|a, b| (a.active, a.eta).partial_cmp(&(b.active, b.eta)).unwrap_or(std::cmp::Ordering::Equal)).copied();
            let Some(cue) = cue else { continue };
            let open = cue.active || cue.eta <= 0.0;
            if !open {
                self.spent.retain(|k| k != key); // a new attack: the last one's mark does not count
            }
            let live = self.marks.iter().position(|m| m.key == *key && m.fade.is_none());
            match live {
                Some(i) => {
                    self.marks[i].kind = cue.kind; // (Danger Sense) the word names the attack the "!" stands for now
                    if open && self.marks[i].age >= look.min_show {
                        self.marks[i].fade = Some(0.0);
                        self.spent.push(*key);
                    }
                }
                None if !self.spent.contains(key) && (open || cue.eta <= lead) => {
                    self.marks.retain(|m| m.key != *key);
                    let used = |c: usize| self.marks.iter().any(|m| m.clip == c);
                    if let Some(clip) = (0..PERIL_MARKS).find(|c| !used(*c)) {
                        self.marks.push(Mark { key: *key, clip, age: 0.0, fade: None, kind: cue.kind });
                        self.popped += 1;
                    }
                }
                None => {}
            }
        }
        // the attack is over or called off
        for m in &mut self.marks {
            if m.fade.is_none() && !keys.contains(&m.key) {
                m.fade = Some(0.0);
            }
        }
        self.spent.retain(|k| keys.contains(k));
    }

    /// (GF-POLISH) The static SE to play now: a mark popped since the last call and the gap has passed. A crowd's marks make one cue.
    pub fn take_se(&mut self, look: &PerilLook) -> Option<u32> {
        let popped = std::mem::take(&mut self.popped) > 0;
        if !popped || look.se < 0 || self.se_wait > 0.0 {
            return None;
        }
        self.se_wait = look.se_gap;
        Some(look.se as u32)
    }

    /// The enemies showing a mark now, fading ones too (tests).
    pub fn showing(&self) -> Vec<u64> {
        self.marks.iter().map(|m| m.key).collect()
    }

    /// (Danger Sense) The marks now with the kind each names, fading ones too (tests).
    pub fn showing_kinds(&self) -> Vec<(u64, Option<PerilKind>)> {
        self.marks.iter().map(|m| (m.key, m.kind)).collect()
    }

    /// (LEADER RESET) Every mark goes at once, unfaded (a new leader / a new character: no warning of the old fight stays on
    /// the target HUD); the clips stay made for the next marks.
    pub fn clear(&mut self, hud: &mut HudTarget) {
        self.marks.clear();
        self.spent.clear();
        self.popped = 0;
        self.last.clear();
        for clip in 0..self.installed {
            hud.movie.set_visible(&mark_path(clip), false);
        }
    }

    /// (Danger Sense) A kind's word, made once.
    fn word_texture(&mut self, fonts: &Fonts, kind: PerilKind, look: &PerilLook) -> Option<Arc<Texture>> {
        if let Some((_, t)) = self.words.iter().find(|w| w.0 == kind) {
            return t.clone();
        }
        let t = peril_word_texture(fonts, look.word(kind), look.outline).map(Arc::new);
        self.words.push((kind, t.clone()));
        t
    }

    /// One update of the target HUD, after its projection pass. `top_of` = the top of an enemy's body in the view's space.
    pub fn update(&mut self, hud: &mut HudTarget, view: Option<&View>, top_of: &dyn Fn(u64) -> Option<Vec3>, look: &PerilLook) {
        self.last.clear();
        if self.texture.is_none() && !self.marks.is_empty() {
            self.texture = peril_texture(&hud.fonts, look).map(Arc::new);
        }
        let Some(texture) = self.texture.clone() else { return };
        let kinds: Vec<PerilKind> = self.marks.iter().filter_map(|k| k.kind).collect();
        for kind in kinds {
            let _ = self.word_texture(&hud.fonts, kind, look);
        }
        let m = &mut hud.movie;
        if self.installed > 0 && !m.exists(&mark_path(0)) {
            self.installed = 0; // a new movie
        }
        while self.installed < self.marks.iter().map(|k| k.clip + 1).max().unwrap_or(0) {
            if !ensure_art_of(m, crate::hud_arcane::TARGET_CLASS, crate::hud_arcane::TARGET_PLANE, &format!("gofPeril{}", self.installed)) {
                return;
            }
            self.installed += 1;
        }
        for clip in 0..self.installed {
            let Some(mark) = self.marks.iter().find(|k| k.clip == clip) else {
                m.set_visible(&mark_path(clip), false);
                continue;
            };
            let path = mark_path(clip);
            let place = view.zip(top_of(mark.key)).map(|(v, p)| {
                let q = v.project(p, (0.0, 0.0));
                if q.on_screen {
                    return ((q.x as f32, q.y as f32 - look.lift), true);
                }
                let c = v.clip_from_world * p.extend(1.0);
                (edge_place((c.x, c.y), look.edge), false)
            });
            let Some((at, on_screen)) = place else {
                m.set_visible(&path, false);
                self.last.push(PerilShown { key: mark.key, alpha: 0.0, scale: 0.0, at: (0.0, 0.0), on_screen: false, word: None, word_w: 0.0 });
                continue;
            };
            let pop = (mark.age / look.pop.max(1e-3)).clamp(0.0, 1.0);
            let settle = 1.0 - (1.0 - pop) * (1.0 - pop);
            let scale = look.pop_scale + (1.0 - look.pop_scale) * settle;
            let alpha = pop.min(1.0) * mark.fade.map_or(1.0, |f| (1.0 - f / look.fade.max(1e-3)).clamp(0.0, 1.0));
            // the drawing's own size; the clip's matrix holds the place and the pop
            let (h, w) = (look.size, look.size * MARK_W as f32 / MARK_H as f32);
            let fill = BitmapFill { texture: texture.clone(), matrix: Matrix::new(w / MARK_W as f32, 0.0, 0.0, h / MARK_H as f32, -w / 2.0, -h / 2.0), scale: (1.0, 1.0), repeat: false, smooth: true };
            let mut gfx = vec![GfxRect { x: -w / 2.0, y: -h / 2.0, w, h, fill }];
            // (Danger Sense) the kind's word under the glyph's foot, in the same clip: it pops, fades and goes with the "!"
            let word = mark.kind.and_then(|k| self.words.iter().find(|t| t.0 == k).and_then(|t| t.1.clone()).map(|t| (k, t)));
            let word = word.map(|(k, t)| {
                let wh = look.word_h.max(1.0);
                let ww = (wh * t.width as f32 / t.height as f32).min(look.word_max_w.max(1.0));
                let top = h / 2.0 - h * MARK_ROOM / MARK_H as f32 + look.word_gap;
                let fill = BitmapFill { matrix: Matrix::new(ww / t.width as f32, 0.0, 0.0, wh / t.height as f32, -ww / 2.0, top), texture: t, scale: (1.0, 1.0), repeat: false, smooth: true };
                gfx.push(GfxRect { x: -ww / 2.0, y: top, w: ww, h: wh, fill });
                (k, ww, top + wh)
            });
            set_art(m, &path, gfx, Cx { mul: [1.0, 1.0, 1.0, alpha], add: [0.0; 4] }, true);
            // (a place the projection's margin lets past the stage comes back in with the rest; the word with it)
            let at = mark_place(at, look, word.map(|w| (w.1, w.2)));
            if let Some(n) = m.node_mut(&path) {
                n.matrix = Matrix::new(scale, 0.0, 0.0, scale, at.0, at.1);
                n.script_matrix = true;
            }
            self.last.push(PerilShown { key: mark.key, alpha, scale, at, on_screen, word: word.map(|w| w.0), word_w: word.map_or(0.0, |w| w.1) });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn cue(key: u64, eta: f32, active: bool) -> PerilCue {
        PerilCue { key, eta, active, kind: None }
    }

    #[test]
    fn every_kind_has_a_word_and_a_colour_of_its_own() {
        let look = PerilLook::default();
        for (i, a) in PerilKind::ALL.iter().enumerate() {
            assert_eq!(*a as usize, i, "ALL is in the words' order");
            let w = look.word(*a);
            assert!(!w.text.is_empty() && w.text.chars().all(|c| c.is_ascii_uppercase()), "{a:?}: {w:?}");
            assert!(w.color[3] > 0.99, "{a:?} is opaque");
            for b in &PerilKind::ALL[i + 1..] {
                assert_ne!(w.text, look.word(*b).text, "{a:?} / {b:?}");
                assert_ne!(w.color, look.word(*b).color, "{a:?} / {b:?}");
            }
        }
    }

    #[test]
    fn the_mark_carries_its_cues_kind_and_keeps_it_while_it_fades() {
        let look = PerilLook::default();
        let mut p = PerilMarks::default();
        p.step(&[PerilCue { kind: Some(PerilKind::Sweep), ..cue(1, 0.3, false) }, cue(2, 0.3, false)], 0.0, &look, DT);
        let mut s = p.showing_kinds();
        s.sort_unstable_by_key(|m| m.0);
        assert_eq!(s, vec![(1, Some(PerilKind::Sweep)), (2, None)], "the plain cue has no word");
        // the next attack the "!" stands for is another kind: the word follows
        p.step(&[PerilCue { kind: Some(PerilKind::Grab), ..cue(1, 0.2, false) }], 0.0, &look, DT);
        assert_eq!(p.showing_kinds(), vec![(1, Some(PerilKind::Grab)), (2, None)]);
        // the attack is called off: it fades with its word, then both are gone together
        p.step(&[], 0.0, &look, DT);
        assert_eq!(p.showing_kinds()[0], (1, Some(PerilKind::Grab)), "fading, still named");
        for _ in 0..30 {
            p.step(&[], 0.0, &look, DT);
        }
        assert!(p.showing_kinds().is_empty());
    }

    #[test]
    fn the_word_keeps_inside_the_stage_with_the_mark() {
        let look = PerilLook::default();
        // no word: the "!"'s own margin, as before
        assert_eq!(mark_place((0.0, 2000.0), &look, None), (look.edge, 1080.0 - look.edge));
        assert_eq!(mark_place((500.0, 400.0), &look, Some((150.0, 70.0))), (500.0, 400.0), "on the screen it stays over the head");
        // on the bottom edge and a side, a wide word under it: the pair moves in, the word keeps the "!"'s room
        let (h, w) = (look.size, look.size * MARK_W as f32 / MARK_H as f32);
        let (room_x, room_y) = (look.edge - w / 2.0, look.edge - h / 2.0);
        let (x, y) = mark_place((0.0, 2000.0), &look, Some((170.0, 70.0)));
        assert!((x - 85.0 - room_x).abs() < 0.01 && (1080.0 - (y + 70.0) - room_y).abs() < 0.01, "{x}, {y}");
        let (x, _) = mark_place((5000.0, 500.0), &look, Some((170.0, 70.0)));
        assert!((1920.0 - (x + 85.0) - room_x).abs() < 0.01, "{x}");
    }

    /// One attack: `eta0` seconds of warning, then a window of `window` seconds. Returns (first frame shown, first frame gone) in
    /// seconds from the first telegraph.
    fn run(eta0: f32, window: f32, extra: f32, look: &PerilLook) -> (f32, f32) {
        let mut p = PerilMarks::default();
        let (mut eta, mut win, mut t) = (eta0, window, 0.0);
        let (mut shown, mut gone) = (None, None);
        for _ in 0..600 {
            let cues: Vec<PerilCue> = if eta > 0.0 {
                vec![cue(1, eta, false)]
            } else if win > 0.0 {
                vec![cue(1, 0.0, true)]
            } else {
                Vec::new()
            };
            p.step(&cues, extra, look, DT);
            if !p.showing().is_empty() && shown.is_none() {
                shown = Some(t);
            }
            if p.showing().is_empty() && shown.is_some() && gone.is_none() {
                gone = Some(t);
            }
            if eta > 0.0 {
                eta -= DT;
            } else {
                win -= DT;
            }
            t += DT;
        }
        (shown.expect("it showed"), gone.expect("it went"))
    }

    #[test]
    fn the_mark_pops_half_a_second_before_contact_and_is_gone_soon_after() {
        let look = PerilLook::default();
        let (shown, gone) = run(1.5, 0.2, 0.0, &look);
        assert!((1.5 - shown - look.lead).abs() < 0.03, "shows {:.2} s before contact", 1.5 - shown);
        assert!(gone >= 1.5 && gone <= 1.5 + look.fade + 0.03, "gone {gone:.2}, contact at 1.5");
    }

    #[test]
    fn danger_sense_shows_it_earlier() {
        let look = PerilLook::default();
        let (plain, _) = run(1.5, 0.2, 0.0, &look);
        let (sense, _) = run(1.5, 0.2, 0.15, &look);
        assert!((plain - sense - 0.15).abs() < 0.03, "{plain} vs {sense}");
    }

    #[test]
    fn a_late_warning_still_shows_long_enough_to_read() {
        let look = PerilLook::default();
        let (shown, gone) = run(0.1, 0.6, 0.0, &look);
        assert!(shown < 0.02, "at once");
        assert!(gone - shown >= look.min_show + look.fade - 0.03, "{shown} .. {gone}");
    }

    #[test]
    fn only_one_mark_per_enemy_and_each_enemy_its_own() {
        let look = PerilLook::default();
        let mut p = PerilMarks::default();
        p.step(&[cue(1, 0.3, false), cue(1, 0.2, false), cue(2, 0.1, false), cue(2, 0.4, false)], 0.0, &look, DT);
        let mut s = p.showing();
        s.sort_unstable();
        assert_eq!(s, vec![1, 2]);
    }

    #[test]
    fn the_second_attack_of_a_string_gets_its_own_mark_and_a_held_window_gets_one() {
        let look = PerilLook::default();
        let mut p = PerilMarks::default();
        for _ in 0..40 {
            p.step(&[cue(1, 0.1, false)], 0.0, &look, DT);
        }
        // the window opens and stays open: one mark, it fades, it does not come back
        for _ in 0..120 {
            p.step(&[cue(1, 0.0, true)], 0.0, &look, DT);
        }
        assert!(p.showing().is_empty(), "faded while the window stays open");
        // the next attack of the string
        p.step(&[cue(1, 0.3, false)], 0.0, &look, DT);
        assert_eq!(p.showing(), vec![1]);
    }

    #[test]
    fn an_attack_called_off_fades_at_once() {
        let look = PerilLook::default();
        let mut p = PerilMarks::default();
        p.step(&[cue(1, 0.2, false)], 0.0, &look, DT);
        assert_eq!(p.showing(), vec![1]);
        let mut gone_after = 0.0;
        while !p.showing().is_empty() {
            p.step(&[], 0.0, &look, DT);
            gone_after += DT;
        }
        assert!(gone_after <= look.fade + 0.03, "{gone_after}");
    }

    #[test]
    fn an_attacker_off_the_screen_shows_on_the_edge_toward_it() {
        let m = 72.0;
        // right of the screen, above it, left of it, below it
        assert_eq!(edge_place((3.0, 0.0), m), (1920.0 - m, 540.0));
        assert_eq!(edge_place((0.0, 2.0), m), (960.0, m));
        assert_eq!(edge_place((-3.0, 0.0), m), (m, 540.0));
        assert_eq!(edge_place((0.0, -2.0), m), (960.0, 1080.0 - m));
        // a corner: it keeps the line from the centre, inside the margin
        let (x, y) = edge_place((1.0, 1.0), m);
        assert!(x <= 1920.0 - m + 0.01 && y >= m - 0.01, "{x}, {y}");
        assert!(((x - 960.0) / 960.0 - (540.0 - y) / 540.0).abs() < 0.01, "{x}, {y}");
    }

    #[test]
    fn dead_behind_the_camera_the_mark_is_at_the_bottom_and_never_lost() {
        assert_eq!(edge_place((0.0, 0.0), 72.0), (960.0, 1080.0 - 72.0));
        let (x, y) = edge_place((-1.0e-6, 1.0e-6), 72.0);
        assert!(x.is_finite() && y.is_finite());
        assert!((72.0..=1848.0).contains(&x) && (72.0..=1008.0).contains(&y));
    }

    #[test]
    fn behind_the_camera_each_side_stays_its_side() {
        let eye = Vec3::new(0.0, 0.0, 0.0);
        let m = glam::Mat4::perspective_rh(1.0, 16.0 / 9.0, 10.0, 100_000.0) * glam::Mat4::look_at_rh(eye, Vec3::Y, Vec3::Z);
        let place = |p: Vec3| {
            let c = m * p.extend(1.0);
            assert!(c.w < 0.0, "it is behind the camera");
            edge_place((c.x, c.y), 72.0)
        };
        assert!(place(Vec3::new(500.0, -300.0, 0.0)).0 > 960.0, "behind, on the right");
        assert!(place(Vec3::new(-500.0, -300.0, 0.0)).0 < 960.0, "behind, on the left");
        assert!(place(Vec3::new(0.0, -300.0, -500.0)).1 > 540.0, "behind, below: the bottom");
    }

    #[test]
    fn a_pop_sounds_once_and_a_crowd_makes_one_cue() {
        let look = PerilLook::default();
        let mut p = PerilMarks::default();
        assert_eq!(p.take_se(&look), None, "nothing popped");
        p.step(&[cue(1, 0.1, false), cue(2, 0.1, false), cue(3, 0.1, false)], 0.0, &look, DT);
        assert_eq!(p.take_se(&look), Some(19));
        assert_eq!(p.take_se(&look), None, "one cue for the pops of a frame");
        // another mark inside the gap: silent; after it, heard
        p.step(&[cue(1, 0.1, false), cue(2, 0.1, false), cue(3, 0.1, false), cue(4, 0.1, false)], 0.0, &look, DT);
        assert_eq!(p.take_se(&look), None);
        for _ in 0..(look.se_gap / DT) as usize + 1 {
            p.step(&[], 0.0, &look, DT);
        }
        p.step(&[cue(9, 0.1, false)], 0.0, &look, DT);
        assert_eq!(p.take_se(&look), Some(19));
        // a held mark is not a new one
        let mut q = PerilMarks::default();
        q.step(&[cue(1, 0.1, false)], 0.0, &look, DT);
        assert!(q.take_se(&look).is_some());
        for _ in 0..30 {
            q.step(&[cue(1, 0.1, false)], 0.0, &look, DT);
            assert_eq!(q.take_se(&look), None);
        }
    }

    #[test]
    fn a_sheet_with_no_cue_is_silent() {
        let look = PerilLook { se: -1, ..PerilLook::default() };
        let mut p = PerilMarks::default();
        p.step(&[cue(1, 0.1, false)], 0.0, &look, DT);
        assert_eq!(p.take_se(&look), None);
    }

    #[test]
    fn more_enemies_than_clips_never_panics_and_a_far_warning_waits() {
        let look = PerilLook::default();
        let mut p = PerilMarks::default();
        let many: Vec<PerilCue> = (0..30).map(|k| cue(k, 0.1, false)).collect();
        p.step(&many, 0.0, &look, DT);
        assert_eq!(p.showing().len(), PERIL_MARKS);
        let mut q = PerilMarks::default();
        q.step(&[cue(1, 2.0, false)], 0.0, &look, DT);
        assert!(q.showing().is_empty());
    }
}
