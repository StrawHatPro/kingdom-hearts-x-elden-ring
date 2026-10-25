//! (DON-UI) DONALD'S RESONANCE BADGES on the status HUD (documentation/DONALD_OVERHAUL.md 4.1, 4.6, 7; R17). KH3 has no
//! such gauge: it is THE PORT'S, built from KH3's own HUD pieces recoloured, the way the GUARD bar was (hud_guard.rs).
//! The user (2026-10-22, on the first version): "remove the shotlock gauges, when you have two elements attuned the
//! second should be visible on top of the other and the pips should ALWAYS be right next to their element icon". So:
//!
//! - NO FOCUS / SHOTLOCK GAUGE: Donald has no shotlock; while he leads the leader slot's focus gauge (`sttsPc.pcEx`) is
//!   hidden (kh-host `Hud::don_frame`) and nothing here is made from it.
//! - ONE BADGE PER ATTUNED ELEMENT, just above the MP bar, right-aligned to the portrait: a dark KH3 MP-track plate
//!   (`hud_status/HudStatusMpBase9.png`, the MP bar's rounded base, drawn as a pill) holding, left to right, the
//!   ultimate mark, the element's IconText icon and its three Resonance pips. The pips sit right next to their icon in
//!   every state.
//!   - THE PIPS: KH3's style-gauge chevrons (`hud_command/StylePntGg.png`), lit in the element's colour (sheet
//!     `party-don-core` `color.*`), dark when empty, the one being filled at its share.
//!   - FULL POWER (Resonance 3): the plate gets a rim in the element's colour that pulses (KH3's MAX pulse rhythm).
//!   - THE ULTIMATE MARK (R17): KH3's IconText "Link" emblem: hidden / Waiting (dim) / Ready (lit) / Spent (dark).
//! - DUAL ATTUNEMENT (4.6): the second element's badge stacks ON TOP of the first, fully visible.
//! - THE ATTUNE CAST: the icon of the element being attuned blinks in the first badge (a badge of its own when nothing
//!   is attuned yet).
//! - THE WORN SPELLS (6.1): a member wearing Aero Barrier gets the Aero icon by his portrait, one wearing Ball Lightning
//!   the Thunder icon while it is active, with no count of the balls (the user, 2026-10-22).
//!
//! The host gives the values ([`ResonanceView`], from kh-party `donald::hud::DonHud`) and the look ([`ResonanceLayout`],
//! sheet `party-don-hud`) each update; `None` = nothing (not Donald, or not attuned and nothing worn).

use std::sync::Arc;

use crate::def::{BitmapFill, Texture};
use crate::geom::{Cx, Matrix};
use crate::hud_status::HudStatus;
use crate::player::{GfxRect, Movie};
use crate::render::IconSet;

/// The ultimate mark's state (kh-party `UltMark`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UltLook {
    #[default]
    Hidden,
    Waiting,
    Ready,
    Spent,
}

/// One attuned element as the HUD draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct RingLook {
    /// Straight RGBA 0 ..= 1.
    pub color: [f32; 4],
    /// 0 ..= `pips_max`, fractional.
    pub pips: f32,
    pub pips_max: u8,
    pub full: bool,
    pub ult: UltLook,
    /// The IconText icon of the element.
    pub icon: String,
}

/// Donald's status-HUD view.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ResonanceView {
    /// Primary first; at most two.
    pub rings: Vec<RingLook>,
    /// Attuning now: (the element's icon, its colour, the share done).
    pub attuning: Option<(String, [f32; 4], f32)>,
    /// Members wearing a spell: (status slot: 0 the leader, 1 / 2 the friends' slots in the HUD's order, barrier, orbs).
    pub worn: Vec<(usize, bool, u8)>,
    /// The leader's HP track frame (KH3's `hpMaxFrames`, 2 x max HP up to 668): where his HP bar starts (the leader's
    /// worn icon sits in the gap left of it, under the MP bar). 0 = unknown (the sheet's fallback place).
    pub leader_hp_frames: i32,
    /// (the user, 2026-10-22: "move all of the pips to the healthbar part of the HUD") the Arcane Flow stars and the
    /// Zettaflare tracker, in this cluster (a row under the badges); None = no row.
    pub stars: Option<StarsLook>,
}

/// The Arcane Flow row: the charges lit of the meter, the oldest charge's seconds left, the Zettaflare tracker
/// ((element icon, lit), once Zettaflare is learned).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StarsLook {
    pub lit: u8,
    pub max: u8,
    pub left: Option<f32>,
    pub zetta: Vec<(String, bool)>,
}

impl ResonanceView {
    pub fn is_empty(&self) -> bool {
        self.rings.is_empty() && self.attuning.is_none() && self.worn.is_empty() && self.stars.is_none()
    }
}

/// The look (sheet `party-don-hud` rows `badge.*`, `pip.*`, `icon.*`, `ult.*`, `worn.*`, `blink.*`): `sttsPc` units.
#[derive(Clone, Debug, PartialEq)]
pub struct ResonanceLayout {
    /// The first badge's right edge and vertical centre; the second badge sits `row_step` above it.
    pub badge_right: f32,
    pub badge_y: f32,
    pub row_step: f32,
    /// The plate's height, its colour (multiplies the MP base's grey), the padding left / right of its content.
    pub plate_h: f32,
    pub plate: [f32; 4],
    pub plate_pad: f32,
    /// Full power: the rim's width and its pulse rate.
    pub rim: f32,
    pub rim_hz: f32,
    /// The pips: size and step; an empty pip's colour.
    pub pip_size: (f32, f32),
    pub pip_step: f32,
    pub pip_off: [f32; 4],
    /// The element icon's height and its gap to the first pip.
    pub icon_size: f32,
    pub icon_gap: f32,
    /// The ultimate mark's height, its gap to the icon, its dim / dark scales.
    pub ult_size: f32,
    pub ult_gap: f32,
    pub ult_dim: f32,
    pub ult_dark: f32,
    /// READY (the user, 2026-10-22: "make the icon showing that you can use the ultimate magic much brighter"): the mark
    /// at full brightness with a white-hot core (`ult_hot`, added white 0 ..= 255), KH3's HUD glow (`EffGlw1`) behind it
    /// in the element's colour, `ult_glow` x the mark's size, pulsing at `ult_pulse_hz` between `ult_glow_min` and 1.
    pub ult_hot: f32,
    pub ult_glow: f32,
    pub ult_glow_min: f32,
    pub ult_pulse_hz: f32,
    /// READY, WAY BRIGHTER (the user, 2026-10-22: "the icon that tells you ultimate magic is ready needs to be way
    /// brighter"): the mark grows to `ult_ready_scale` x its size, white-hot, with an additive copy over it at
    /// `ult_core` (over-bright); the halo (`EffGlw1`) is `ult_glow` x the mark wide and `ult_glow_h` x tall, its opacity
    /// boosted `ult_glow_gain` x (the texture's falloff is steep) so it spreads well past the badge; a normal-blend copy
    /// of it in the element's full colour at `ult_tint` (it shows on bright scenes, where an additive glow vanishes);
    /// KH3's cross glint (`CrossEff01`, its right half) over the mark, `ult_flare_w` x `ult_flare_h` the mark, twinkling
    /// with the pulse; on becoming Ready a burst: the glint `ult_burst_scale` x wider, fading over `ult_burst_s`.
    pub ult_ready_scale: f32,
    pub ult_core: f32,
    pub ult_glow_h: f32,
    pub ult_glow_gain: f32,
    pub ult_tint: f32,
    pub ult_flare_w: f32,
    pub ult_flare_h: f32,
    pub ult_burst_s: f32,
    pub ult_burst_scale: f32,
    /// The Arcane Flow row (over the badges): its label icon, KH3's Y button glyph ("Triangle" of the pad icon set the
    /// HUD's own Y prompts use: the user, 2026-10-22, "change the magic hat icon next to the stars to Y button"); the
    /// leader's worn icon's disc size (unused since the icon moved beside his face).
    pub stars_label: String,
    pub worn_disc: f32,
    /// THE WORN ICONS (the user, 2026-10-22: "Have them show next to the ally's picture in the empty grey space next to
    /// them. For the leader, have it appear inbetween the gap of the MP bar and the start of the health bar. Make them
    /// large enough to be easily readable"). The leader's: its right edge `worn_gap` left of where his HP bar starts
    /// (`hp_bar_x0` - the HP track frame, `sttsPc` units; `worn_pc_x` when the frame is unknown), centred on `worn_pc_y`,
    /// `worn_size_pc` tall. A friend's: centred at `worn_npc` in his slot's units (the grey plate right of the portrait),
    /// `worn_size_npc` tall.
    pub worn_gap: f32,
    pub hp_bar_x0: f32,
    pub worn_pc_x: f32,
    pub worn_pc_y: f32,
    pub worn_size_pc: f32,
    pub worn_npc: (f32, f32),
    pub worn_size_npc: f32,
    pub blink_hz: f32,
}

impl Default for ResonanceLayout {
    fn default() -> ResonanceLayout {
        ResonanceLayout {
            badge_right: 66.0,
            badge_y: 112.0,
            row_step: 30.0,
            plate_h: 27.0,
            plate: [0.22, 0.22, 0.25, 0.92],
            plate_pad: 7.0,
            rim: 2.0,
            rim_hz: 1.2,
            pip_size: (26.0, 13.0),
            pip_step: 23.0,
            pip_off: [0.30, 0.30, 0.34, 0.95],
            icon_size: 24.0,
            icon_gap: 3.0,
            ult_size: 21.0,
            ult_gap: 3.0,
            ult_dim: 0.45,
            ult_dark: 0.22,
            ult_hot: 255.0,
            ult_glow: 6.0,
            ult_glow_min: 0.8,
            ult_pulse_hz: 1.1,
            ult_ready_scale: 1.25,
            ult_core: 1.0,
            ult_glow_h: 2.4,
            ult_glow_gain: 3.0,
            ult_tint: 0.85,
            ult_flare_w: 5.0,
            ult_flare_h: 1.5,
            ult_burst_s: 0.7,
            ult_burst_scale: 2.4,
            stars_label: "Triangle".to_string(), // KH3's Y button glyph (the pad set the HUD's Y prompts use)
            worn_disc: 1.3,
            worn_gap: 0.0,
            hp_bar_x0: 413.0,
            worn_pc_x: 79.0,
            worn_pc_y: 116.0,
            worn_size_pc: 22.0,
            worn_npc: (86.0, 37.0),
            worn_size_npc: 24.0,
            blink_hz: 3.0,
        }
    }
}

/// The art the badges draw with.
#[derive(Clone, Debug, Default)]
pub struct DonArt {
    /// KH3's IconText (`Fonts::icons`).
    pub icons: Option<Arc<IconSet>>,
    /// `hud_command/StylePntGg.png` (the style gauge's chevrons).
    pub chevron: Option<Arc<Texture>>,
    /// `hud_status/HudStatusMpBase9.png` (the MP bar's rounded base, 16 x 16).
    pub base: Option<Arc<Texture>>,
    /// `hud_command/EffGlw1.png` (KH3's HUD glow, 512 x 512): the Ready ultimate mark's halo.
    pub glow: Option<Arc<Texture>>,
    /// `hud_status/CrossEff01.png` (KH3's HUD glint, 256 x 128: a soft dot left, a four-point cross right): the Ready
    /// ultimate mark's glint.
    pub flare: Option<Arc<Texture>>,
}

/// The cross glint's rectangle in `CrossEff01.png` (its right half).
pub const FLARE_RECT: (f32, f32, f32, f32) = (128.0, 0.0, 128.0, 128.0);
/// The share of a glow / glint quad's half size that visibly draws at the boosted opacity (the textures' alpha falls
/// under ~8 / 255 past it; measured on `EffGlw1` / `CrossEff01`).
pub const GLOW_REACH: f32 = 0.62;
/// The same for the glint's arms (drawn at full opacity, their alpha falls slower).
pub const FLARE_REACH: f32 = 0.9;

/// The chevron's rectangle in `StylePntGg.png` (the white one: the element's colour multiplies it) [A: the texture].
pub const CHEVRON_RECT: (f32, f32, f32, f32) = (120.0, 0.0, 58.0, 25.0);

/// A pill of the 16 x 16 rounded base: its left half, a stretched middle, its right half.
pub fn pill(t: &Arc<Texture>, x: f32, y: f32, w: f32, h: f32) -> Vec<GfxRect> {
    let cap = (h / 2.0).min(w / 2.0);
    vec![tex_rect(t, (0.0, 0.0, 8.0, 16.0), (x, y, cap, h)), tex_rect(t, (7.0, 0.0, 2.0, 16.0), (x + cap, y, (w - 2.0 * cap).max(0.0), h)), tex_rect(t, (8.0, 0.0, 8.0, 16.0), (x + w - cap, y, cap, h))]
}

/// The class every art node is made from (a one-shape clip of the movie, emptied): `CoPcMpLogo`.
const ART_CLASS: &str = "CoPcMpLogo";

const PC: &str = "sttsPc";
/// The portraits' parts: the leader's, the two friends'.
const WORN_PARENTS: [&str; 3] = ["sttsPc", "sttsNpc1", "sttsNpc2"];

/// A colour transform that multiplies (keeps the art's shading) by `c`.
pub fn multiply(c: [f32; 4]) -> Cx {
    Cx { mul: c, add: [0.0; 4] }
}

/// A flat colour (the art's alpha kept), as the GUARD fill.
pub fn flat(c: [f32; 4]) -> Cx {
    Cx { mul: [0.0, 0.0, 0.0, c[3]], add: [c[0] * 255.0, c[1] * 255.0, c[2] * 255.0, 0.0] }
}

/// An empty art node `name` under `parent` (a script child; made once). False when the movie lacks the class.
pub fn ensure_art(m: &mut Movie, parent: &str, name: &str) -> bool {
    ensure_art_of(m, ART_CLASS, parent, name)
}

/// [`ensure_art`] from another movie's one-shape class (`CmdName` in HudCommand, `CoPushBtnText` in HudTarget).
pub fn ensure_art_of(m: &mut Movie, class: &str, parent: &str, name: &str) -> bool {
    let path = format!("{parent}.{name}");
    if m.exists(&path) {
        return true;
    }
    if !m.add_instance(parent, class, name) {
        return false;
    }
    m.stop(&path);
    if let Some(s) = m.sprite_mut(&path) {
        s.remove_children();
        s.gfx.clear();
    }
    true
}

/// One IconText icon as a bitmap rectangle centred on (x, y), `size` tall.
pub fn icon_rect(set: &IconSet, icon: &str, x: f32, y: f32, size: f32) -> Option<GfxRect> {
    let i = set.icons.get(&icon.to_lowercase()).copied()?;
    let k = size / i.4.max(1.0);
    let (w, h) = (i.3 * k, size);
    let (x0, y0) = (x - w / 2.0, y - h / 2.0);
    let fill = BitmapFill { texture: set.textures.get(i.0)?.clone(), matrix: Matrix::new(k, 0.0, 0.0, k, x0 - k * i.1, y0 - k * i.2), scale: (1.0, 1.0), repeat: false, smooth: true };
    Some(GfxRect { x: x0, y: y0, w, h, fill })
}

/// A sub-rectangle of a texture at (x, y, w, h).
pub fn tex_rect(t: &Arc<Texture>, src: (f32, f32, f32, f32), at: (f32, f32, f32, f32)) -> GfxRect {
    let (kx, ky) = (at.2 / src.2.max(1.0), at.3 / src.3.max(1.0));
    let fill = BitmapFill { texture: t.clone(), matrix: Matrix::new(kx, 0.0, 0.0, ky, at.0 - kx * src.0, at.1 - ky * src.1), scale: (1.0, 1.0), repeat: false, smooth: true };
    GfxRect { x: at.0, y: at.1, w: at.2, h: at.3, fill }
}

/// Sets an art node's drawing, colour and visibility.
pub fn set_art(m: &mut Movie, path: &str, gfx: Vec<GfxRect>, cx: Cx, visible: bool) {
    if let Some(s) = m.sprite_mut(path) {
        s.gfx = gfx;
    }
    if let Some(n) = m.node_mut(path) {
        n.cx = cx;
        n.script_visible = Some(visible);
    }
}

/// The badges' driver (one per HUD).
#[derive(Clone, Debug, Default)]
pub struct ResonanceRing {
    pub shown: bool,
    blink: f32,
    pulse: f32,
    ult_pulse: f32,
    /// What was last applied (tests / logs): per badge (its row: 0 bottom, 1 on top; pips lit; full power), the
    /// ultimate marks, and each badge's (icon centre, first pip's left, row centre y) in `sttsPc` units.
    pub last: Vec<(u32, u8, bool)>,
    pub last_ult: Vec<UltLook>,
    pub last_place: Vec<(f32, f32, f32)>,
    /// The worn icons drawn: (slot, centre x, centre y) in `sttsPc` units.
    pub last_worn: Vec<(usize, f32, f32)>,
    star_blink: f32,
    /// The Arcane Flow row last drawn: (stars lit, the oldest blinking, tracker lit, tracker size) and its plate
    /// (left, top, right, bottom) in `sttsPc` units.
    pub last_stars: (u8, bool, u8, u8),
    pub last_stars_plate: Option<(f32, f32, f32, f32)>,
    pub last_zetta_plate: Option<(f32, f32, f32, f32)>,
    /// Each badge's plate (left, top, right, bottom).
    pub last_plates: Vec<(f32, f32, f32, f32)>,
    /// The Ready marks' glow and glint, where they visibly reach (left, top, right, bottom), and the mark's own box.
    pub last_glow: Vec<(f32, f32, f32, f32)>,
    pub last_ult_box: Vec<(f32, f32, f32, f32)>,
    /// Per badge row: the mark's state last update and the burst's time left.
    ult_was: [UltLook; 2],
    ult_burst: [f32; 2],
}

/// Made in this order (the halo's tint and the rim behind the plate; the additive glow on the plate; the mark, its
/// over-bright copy and the glint on top; the pips made after).
const PARTS: [&str; 8] = ["Tint", "Rim", "Plate", "Glow", "Icon", "Ult", "Core", "Flare"];

fn part(row: usize, p: &str) -> String {
    format!("{PC}.donBadge{row}{p}")
}

fn pip_path(row: usize, k: usize) -> String {
    format!("{PC}.donBadge{row}Pip{k}")
}

impl ResonanceRing {
    fn draw_stars(&mut self, m: &mut Movie, art: &DonArt, v: Option<&StarsLook>, look: &ResonanceLayout, sl: &crate::hud_arcane::ArcaneLayout, badges: usize) {
        let names: Vec<String> = ["donStarsPlate", "donZettaPlate", "donStarsLabel", "donZettaDim", "donZettaLit"].iter().map(|n| n.to_string()).chain((0..8).map(|i| format!("donStar{i}"))).collect();
        let (Some(v), Some(set), Some(base)) = (v, art.icons.as_ref(), art.base.as_ref()) else {
            for n in &names {
                m.set_visible(&format!("{PC}.{n}"), false);
            }
            self.last_stars_plate = None;
            self.last_zetta_plate = None;
            return;
        };
        for n in &names {
            if !ensure_art(m, PC, n) {
                return;
            }
        }
        // above the badges (none: the first row above the MP bar)
        let cy = look.badge_y - badges as f32 * look.row_step;
        let max = v.max.clamp(1, 8) as usize;
        let n = v.zetta.len();
        let sw = (max as f32 - 1.0) * sl.star_step + sl.star_size;
        let right = look.badge_right - look.plate_pad;
        let left = right - sw - look.icon_gap - look.icon_size;
        let ph = look.plate_h;
        let plate_left = left - look.plate_pad;
        set_art(m, &format!("{PC}.donStarsPlate"), pill(base, plate_left, cy - ph / 2.0, look.badge_right - plate_left, ph), multiply(look.plate), true);
        self.last_stars_plate = Some((plate_left, cy - ph / 2.0, look.badge_right, cy + ph / 2.0));
        let gfx = icon_rect(set, &look.stars_label, left + look.icon_size / 2.0, cy, look.icon_size).into_iter().collect();
        set_art(m, &format!("{PC}.donStarsLabel"), gfx, Cx::IDENTITY, true);
        let x0 = left + look.icon_size + look.icon_gap + sl.star_size / 2.0;
        let warn = v.lit > 0 && v.left.is_some_and(|l| l < sl.star_warn);
        for i in 0..8 {
            let path = format!("{PC}.donStar{i}");
            if i >= max {
                m.set_visible(&path, false);
                continue;
            }
            let gfx = icon_rect(set, "s_ssrare", x0 + i as f32 * sl.star_step, cy, sl.star_size).into_iter().collect();
            let lit = i < v.lit as usize && !(i == 0 && warn && self.star_blink >= 0.5);
            let cx = if lit { Cx::IDENTITY } else { Cx { mul: [sl.star_off, sl.star_off, sl.star_off, 0.9], add: [0.0; 4] } };
            set_art(m, &path, gfx, cx, true);
        }
        // the Zettaflare tracker: its own row right above (once Zettaflare is learned), right-aligned as the others
        let zy = cy - look.row_step;
        let zw = if n > 0 { (n as f32 - 1.0) * sl.zetta_step + sl.zetta_size } else { 0.0 };
        let z0 = right - zw + sl.zetta_size / 2.0;
        let zl = right - zw - look.plate_pad;
        set_art(m, &format!("{PC}.donZettaPlate"), pill(base, zl, zy - ph / 2.0, look.badge_right - zl, ph), multiply(look.plate), n > 0);
        self.last_zetta_plate = (n > 0).then_some((zl, zy - ph / 2.0, look.badge_right, zy + ph / 2.0));
        let cy = zy;
        let all: Vec<GfxRect> = v.zetta.iter().enumerate().filter_map(|(k, (icon, _))| icon_rect(set, icon, z0 + k as f32 * sl.zetta_step, cy, sl.zetta_size)).collect();
        set_art(m, &format!("{PC}.donZettaDim"), all, Cx { mul: [sl.zetta_off, sl.zetta_off, sl.zetta_off, 0.85], add: [0.0; 4] }, n > 0);
        let lit: Vec<GfxRect> = v.zetta.iter().enumerate().filter(|(_, (_, l))| *l).filter_map(|(k, (icon, _))| icon_rect(set, icon, z0 + k as f32 * sl.zetta_step, cy, sl.zetta_size)).collect();
        let n_lit = lit.len() as u8;
        set_art(m, &format!("{PC}.donZettaLit"), lit, Cx::IDENTITY, n_lit > 0);
        self.last_stars = (v.lit.min(max as u8), warn, n_lit, n as u8);
    }

    fn hide_all(m: &mut Movie) {
        for n in ["donStarsPlate", "donZettaPlate", "donStarsLabel", "donZettaDim", "donZettaLit", "donWornDisc"] {
            m.set_visible(&format!("{PC}.{n}"), false);
        }
        for i in 0..8 {
            m.set_visible(&format!("{PC}.donStar{i}"), false);
        }
        for row in 0..2 {
            for p in PARTS {
                m.set_visible(&part(row, p), false);
            }
            for k in 0..3 {
                m.set_visible(&pip_path(row, k), false);
            }
        }
        for slot in 0..WORN_PARENTS.len() {
            m.set_visible(&format!("{PC}.donWorn{slot}"), false);
        }
    }

    /// One update of the status HUD, after the driver's own and the stage frames. `v` None = nothing shown.
    pub fn update(&mut self, hud: &mut HudStatus, art: &DonArt, v: Option<&ResonanceView>, look: &ResonanceLayout, stars_look: &crate::hud_arcane::ArcaneLayout, dt: f32) {
        let m = &mut hud.movie;
        let Some(v) = v.filter(|v| !v.is_empty()) else {
            if self.shown {
                Self::hide_all(m);
            }
            self.shown = false;
            self.last.clear();
            self.last_ult.clear();
            self.ult_was = [UltLook::Hidden; 2];
            self.last_place.clear();
            return;
        };
        if !m.exists(PC) {
            return;
        }
        self.shown = true;
        self.blink = (self.blink + dt * look.blink_hz.max(0.0)).fract();
        self.pulse = (self.pulse + dt * look.rim_hz.max(0.0)).fract();
        self.ult_pulse = (self.ult_pulse + dt * look.ult_pulse_hz.max(0.0)).fract();
        let visible = m.node(PC).is_some_and(|p| p.is_visible());
        self.last.clear();
        self.last_ult.clear();
        self.last_place.clear();
        self.last_plates.clear();
        self.last_glow.clear();
        self.last_ult_box.clear();
        // the badges: the primary at the bottom, the second element stacked on top of it
        let mut rows: Vec<(RingLook, bool)> = v.rings.iter().take(2).map(|r| (r.clone(), false)).collect();
        if rows.is_empty() {
            if let Some((icon, c, _)) = &v.attuning {
                rows.push((RingLook { color: *c, pips: 0.0, pips_max: 3, full: false, ult: UltLook::Hidden, icon: icon.clone() }, true));
            }
        }
        for row in 0..2 {
            for p in PARTS {
                if !ensure_art(m, PC, &format!("donBadge{row}{p}")) {
                    return;
                }
            }
            for k in 0..3 {
                if !ensure_art(m, PC, &format!("donBadge{row}Pip{k}")) {
                    return;
                }
            }
            let Some((g, attune_only)) = rows.get(row).filter(|_| visible) else {
                for p in PARTS {
                    m.set_visible(&part(row, p), false);
                }
                for k in 0..3 {
                    m.set_visible(&pip_path(row, k), false);
                }
                self.ult_was[row] = UltLook::Hidden;
                continue;
            };
            // the stars row is the first above the MP bar; the badges stack over it
            // (the user, 2026-10-22: "place the three pip meters below the other ones") the badges are the rows nearest
            // the MP bar; the Arcane Flow row and the tracker stack over them
            let cy = look.badge_y - row as f32 * look.row_step;
            let max = g.pips_max.clamp(1, 3) as usize;
            // right to left: the pips, the icon, the ultimate mark
            let pips_left = look.badge_right - look.plate_pad - (max as f32 - 1.0) * look.pip_step - look.pip_size.0;
            let icon_x = pips_left - look.icon_gap - look.icon_size / 2.0;
            let ult_on = g.ult != UltLook::Hidden;
            let ult_x = icon_x - look.icon_size / 2.0 - look.ult_gap - look.ult_size / 2.0;
            let left = if ult_on { ult_x - look.ult_size / 2.0 } else { icon_x - look.icon_size / 2.0 } - look.plate_pad;
            let (pw, ph) = (look.badge_right - left, look.plate_h);
            self.last_plates.push((left, cy - ph / 2.0, look.badge_right, cy + ph / 2.0));
            // the plate (KH3's MP base, dark) and, at full power, its rim in the element's colour, pulsing
            if let Some(t) = &art.base {
                set_art(m, &part(row, "Plate"), pill(t, left, cy - ph / 2.0, pw, ph), multiply(look.plate), true);
                let r = look.rim;
                let a = 0.55 + 0.45 * (self.pulse * std::f32::consts::TAU).cos().abs();
                set_art(m, &part(row, "Rim"), pill(t, left - r, cy - ph / 2.0 - r, pw + 2.0 * r, ph + 2.0 * r), Cx { mul: [0.0, 0.0, 0.0, g.color[3] * a], add: [g.color[0] * 255.0, g.color[1] * 255.0, g.color[2] * 255.0, 0.0] }, g.full);
            }
            // the element icon (the attune cast's element blinks in the first badge)
            let icon = match (&v.attuning, row) {
                (Some((a, _, _)), 0) if *attune_only || self.blink < 0.5 => a.as_str(),
                _ => g.icon.as_str(),
            };
            let icon_cx = if *attune_only && self.blink >= 0.5 { Cx { mul: [1.0, 1.0, 1.0, 0.35], add: [0.0; 4] } } else { Cx::IDENTITY };
            let gfx = art.icons.as_ref().and_then(|set| icon_rect(set, icon, icon_x, cy, look.icon_size)).into_iter().collect();
            set_art(m, &part(row, "Icon"), gfx, icon_cx, true);
            // the ultimate mark
            // (the user, 2026-10-22: "the icon that tells you ultimate magic is ready needs to be way brighter"): Ready =
            // a white-hot mark, grown, with an over-bright additive copy; a wide halo in the element's colour (additive on
            // the plate, a normal-blend tint behind it for bright scenes); KH3's cross glint over it; a burst on becoming
            // Ready. The pulse never drops under `ult_glow_min`.
            let wave = 0.5 + 0.5 * (self.ult_pulse * std::f32::consts::TAU).cos();
            let ready = g.ult == UltLook::Ready;
            if ready && self.ult_was[row] != UltLook::Ready {
                self.ult_burst[row] = look.ult_burst_s.max(0.0);
            }
            self.ult_was[row] = g.ult;
            self.ult_burst[row] = (self.ult_burst[row] - dt).max(0.0);
            let burst = if look.ult_burst_s > 0.0 { self.ult_burst[row] / look.ult_burst_s } else { 0.0 };
            let a = (look.ult_glow_min + (1.0 - look.ult_glow_min) * wave).max(burst);
            let (gw, gh) = (look.ult_size * look.ult_glow * (1.0 + 0.3 * burst), look.ult_size * look.ult_glow_h);
            let halo = |t: &Arc<Texture>, k: f32| tex_rect(t, (0.0, 0.0, 512.0, 512.0), (ult_x - gw * k / 2.0, cy - gh * k / 2.0, gw * k, gh * k));
            let sat = g.color[0].max(g.color[1]).max(g.color[2]).max(1e-3);
            let full = [g.color[0] / sat, g.color[1] / sat, g.color[2] / sat];
            if let (Some(t), true) = (&art.glow, ready) {
                let gain = look.ult_glow_gain.max(0.0);
                set_art(m, &part(row, "Tint"), vec![halo(t, 1.0)], flat([full[0], full[1], full[2], look.ult_tint * a * gain]), true);
                let light = [(full[0] + 0.3).min(1.0), (full[1] + 0.3).min(1.0), (full[2] + 0.3).min(1.0), a * gain];
                // the halo twice over and a hot inner one, additive (as KH3's HUD glows)
                set_art(m, &part(row, "Glow"), vec![halo(t, 1.0), halo(t, 1.0), halo(t, 0.5)], flat(light), true);
                if let Some(n) = m.node_mut(&part(row, "Glow")) {
                    n.blend = 8;
                }
                let (rx, ry) = (gw / 2.0 * GLOW_REACH, gh / 2.0 * GLOW_REACH);
                self.last_glow.push((ult_x - rx, cy - ry, ult_x + rx, cy + ry));
            } else {
                m.set_visible(&part(row, "Tint"), false);
                m.set_visible(&part(row, "Glow"), false);
            }
            if let (Some(t), true) = (&art.flare, ready) {
                let (fw, fh) = (look.ult_size * look.ult_flare_w * (1.0 + (look.ult_burst_scale - 1.0).max(0.0) * burst), look.ult_size * look.ult_flare_h * (1.0 + 0.1 * burst));
                let tw = (0.7 + 0.3 * wave).max(burst);
                let c = [(full[0] * 0.35 + 0.65).min(1.0), (full[1] * 0.35 + 0.65).min(1.0), (full[2] * 0.35 + 0.65).min(1.0), tw];
                set_art(m, &part(row, "Flare"), vec![tex_rect(t, FLARE_RECT, (ult_x - fw / 2.0, cy - fh / 2.0, fw, fh)); 2], flat(c), true);
                if let Some(n) = m.node_mut(&part(row, "Flare")) {
                    n.blend = 8;
                }
                let (rx, ry) = (fw / 2.0 * FLARE_REACH, fh / 2.0 * FLARE_REACH);
                self.last_glow.push((ult_x - rx, cy - ry, ult_x + rx, cy + ry));
            } else {
                m.set_visible(&part(row, "Flare"), false);
            }
            let hot = look.ult_hot * (0.85 + 0.15 * wave);
            let ult_cx = match g.ult {
                UltLook::Ready => Cx { mul: [1.0; 4], add: [hot, hot, hot, 0.0] },
                UltLook::Waiting => Cx { mul: [look.ult_dim, look.ult_dim, look.ult_dim, 0.85], add: [0.0; 4] },
                UltLook::Spent => Cx { mul: [look.ult_dark, look.ult_dark, look.ult_dark, 0.9], add: [0.0; 4] },
                UltLook::Hidden => Cx::IDENTITY,
            };
            let us = if ready { look.ult_size * look.ult_ready_scale.max(0.1) } else { look.ult_size };
            let gfx: Vec<GfxRect> = art.icons.as_ref().and_then(|set| icon_rect(set, "Link", ult_x, cy, us)).into_iter().collect();
            if let Some(r) = gfx.first() {
                self.last_ult_box.push((r.x, r.y, r.x + r.w, r.y + r.h));
            }
            set_art(m, &part(row, "Core"), gfx.clone(), Cx { mul: [0.0, 0.0, 0.0, look.ult_core * a], add: [255.0, 255.0, 255.0, 0.0] }, ready && look.ult_core > 0.0);
            if let Some(n) = m.node_mut(&part(row, "Core")) {
                n.blend = 8;
            }
            set_art(m, &part(row, "Ult"), gfx, ult_cx, ult_on);
            // the pips, right next to the icon
            let lit = (g.pips + 1e-4).floor() as usize;
            for k in 0..3 {
                let path = pip_path(row, k);
                let (Some(t), true) = (&art.chevron, k < max) else {
                    m.set_visible(&path, false);
                    continue;
                };
                let x = pips_left + k as f32 * look.pip_step;
                let gfx = vec![tex_rect(t, CHEVRON_RECT, (x, cy - look.pip_size.1 / 2.0, look.pip_size.0, look.pip_size.1))];
                let c = if k < lit {
                    multiply(g.color)
                } else if k == lit && g.pips > k as f32 {
                    let f = 0.35 + 0.4 * (g.pips - k as f32);
                    multiply([g.color[0] * f, g.color[1] * f, g.color[2] * f, g.color[3]])
                } else {
                    multiply(look.pip_off)
                };
                set_art(m, &path, gfx, c, true);
            }
            self.last.push((row as u32, lit.min(max) as u8, g.full));
            self.last_ult.push(g.ult);
            self.last_place.push((icon_x, pips_left, cy));
        }
        // THE ARCANE FLOW ROW (over the badges; the first row above the MP bar without them): a plate as the badges', the label icon, the stars (lit for
        // each charge, the oldest blinking in its last seconds), the Zettaflare tracker after them
        self.star_blink = (self.star_blink + dt * stars_look.star_blink_hz.max(0.0)).fract();
        let badges = rows.len().min(2);
        self.draw_stars(m, art, v.stars.as_ref().filter(|_| visible), look, stars_look, badges);
        // the worn spells by the portraits: drawn in the leader's part (the friends' slots overlap each other, a lower
        // slot's own children are covered), placed at the seat's part each update
        let pc_at = m.node(PC).map(|n| (n.matrix.tx, n.matrix.ty)).unwrap_or_default();
        self.last_worn.clear();
        for (slot, parent) in WORN_PARENTS.iter().enumerate() {
            let path = format!("{PC}.donWorn{slot}");
            let w = v.worn.iter().find(|w| w.0 == slot);
            // the leader's icon on a dark disc (it sits on the bars' junction): the disc made first, behind it
            if w.is_some() && ((slot == 0 && !ensure_art(m, PC, "donWornDisc")) || !ensure_art(m, PC, &format!("donWorn{slot}"))) {
                continue;
            }
            if slot == 0 {
                m.set_visible(&format!("{PC}.donWornDisc"), false);
            }
            let seat = m.node(parent).filter(|n| n.is_visible()).map(|n| (n.matrix.tx, n.matrix.ty));
            match (w, art.icons.as_ref(), seat) {
                // Aero Barrier: the Aero icon; Ball Lightning: the Thunder icon while it is active, no count of the balls
                // (the user, 2026-10-22: "have an icon on the HUD, but no need to specify how many balls there are")
                // ONE icon per character (the user: "each character can only have one effect like that active on them at a
                // time"; the rule itself is allyfx's): the barrier's, else the orbs'
                (Some(&(_, barrier, orbs)), Some(set), Some((sx, sy))) if barrier || orbs > 0 => {
                    let icon = if barrier { "Aero" } else { "Thunder" };
                    let (x, y, size) = if slot == 0 {
                        // (the user, 2026-10-22: "between the magic and health, in the empty space between the top of the MP bar
                        // and the health bar right next to the face"): above the MP bar's right end, left of the HP ring
                        (look.worn_pc_x, look.worn_pc_y, look.worn_size_pc)
                    } else {
                        (sx - pc_at.0 + look.worn_npc.0, sy - pc_at.1 + look.worn_npc.1, look.worn_size_npc)
                    };
                    let gfx: Vec<GfxRect> = icon_rect(set, icon, x, y, size).into_iter().collect();
                    set_art(m, &path, gfx, Cx::IDENTITY, true);

                    self.last_worn.push((slot, x, y));
                }
                _ => {
                    m.set_visible(&path, false);
                }
            }
        }
    }
}
