//! (GOOFY-GG) THE GUARD BAR of the status HUD (the user, 2026-10-16 23:40: "a new bar above MP called GUARD that is
//! silver and matches the rest of the HUD design"). KH3 has no such bar: it is THE PORT'S, built from KH3's own
//! HudStatus pieces so that it looks like the MP bar it sits on:
//!
//! - THE BAR is a second instance of the movie's own MP part (`sttsPc.pcMp`, `HudStatus_fla.McPcMp_90`, with its pack
//!   `McPcMpPack_91`: the track `pcMpDodai` / `pcMpDodaiC`, the fill `pcMpGg`, the under-fill `pcMpGgC`, the logos),
//!   placed above the MP bar (`hud.offset_*` rows), following the MP part's own matrix, colour and frame every update
//!   (so the slot's in / out and the leader switch's shrink / grow move it with the rest).
//! - LENGTH: the track's frame is KH3's MP rule (`gauge_frame(max, max, max x 1.8)`, the driver's `mp_max_frame`), so
//!   GUARD 100 is as long as MP 100 and the bar grows with GUARD's max (levels).
//! - FILL: `pcMpGg` at `gauge_frame(value, max, track frame)`, recoloured silver by a colour transform (the MP fill is a
//!   flat atlas blue).
//! - DEPLETION: `pcMpGgC` under the fill shows the lost part in red and follows the fill down with KH3's own red-gauge
//!   timer `tres.utils.FollowGauge` (800 ms, then the speed rule), exactly as the HP gauge's `pcHpGgR`.
//! - REGEN: while it refills the fill takes the darker regen silver.
//! - BROKEN: the pack plays KH3's MP-charge labels (`charge_in` ... `charge_out`: the reddened track `pcMpDodaiC`), the
//!   fill is hidden and `pcMpGgC` shows the refill in the broken grey, blinking with the label until it rearms.
//! - THE LABEL "GUARD" is set from the letters of KH3's HUD label atlas (`HudStatusTxt.png`, the font of the MP / FOCUS
//!   logos): G U A D from "LADUNG", R from "CHARGE", outline and shadow kept, the letter faces recoloured silver; it
//!   replaces the MP logo in the copy.
//!
//! The host gives the values ([`GuardValues`], from kh-party's `Guard`) and the look ([`GuardLook`], from the sheet
//! `party-guard`) each update; `None` = no bar (Sora or Donald leads).

use std::sync::Arc;

use crate::controls::FollowGauge;
use crate::def::{BitmapFill, Paint, Texture};
use crate::frames::gauge_frame;
use crate::geom::{Cx, Matrix};
use crate::hud_status::HudStatus;
use crate::player::{GfxRect, Kind};

/// The GUARD meter as the HUD draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuardValues {
    pub value: f32,
    pub max: f32,
    pub broken: bool,
    pub regenerating: bool,
    /// While broken: the share of the max at which it rearms.
    pub rearm_at: f32,
}

/// The look (sheet `party-guard` rows `hud.*`): colours as straight RGBA 0..1.
#[derive(Clone, Debug, PartialEq)]
pub struct GuardLook {
    pub fill: [f32; 4],
    pub shine: [f32; 4],
    pub loss: [f32; 4],
    pub regen: [f32; 4],
    pub broken: [f32; 4],
    pub blink_hz: f32,
    /// Stage units from the MP part's place (negative y = up).
    pub offset: (f32, f32),
}

const PC: &str = "sttsPc";
const MP: &str = "sttsPc.pcMp";
/// The copy's instance name.
pub const GUARD: &str = "sttsPc.pcGuard";
const PACK: &str = "sttsPc.pcGuard.pcMpPack";

/// The letters of "GUARD" in `HudStatusTxt.png`: (first, last) column of the letter face; rows 107 ..= 125 hold the
/// line ("CHARGE LADUNG CARICA"), the faces rows 111 ..= 121 [A: the atlas' pixels].
const LETTERS: [(u32, u32); 5] = [(128, 137), (106, 115), (84, 93), (36, 45), (95, 104)];
const ROWS: (u32, u32) = (107, 125);
const FACE_ROWS: (u32, u32) = (111, 121);
/// The atlas' "MP" logo: top-left of its glyphs (rows 49 ..= 64, columns 2 ..), where the copy's label is anchored.
const MP_GLYPH_ORIGIN: (f32, f32) = (2.0, 49.0);

fn is_face(p: &[u8]) -> bool {
    p[3] > 200 && p[0] > 140
}

/// THE "GUARD" LABEL from KH3's HUD label atlas (module text). `atlas` is `HudStatusTxt.png`.
pub fn guard_label_texture(atlas: &Texture, look: &GuardLook) -> Texture {
    let (r0, r1) = ROWS;
    let h = r1 - r0 + 1;
    let faces: u32 = LETTERS.iter().map(|(a, b)| b - a + 1).sum();
    let w = faces + (LETTERS.len() as u32 - 1) + 4;
    let mut out = vec![0u8; (w * h * 4) as usize];
    let mut class = vec![0u8; (w * h) as usize];
    let px = |x: u32, y: u32| -> [u8; 4] {
        if x >= atlas.width || y >= atlas.height {
            return [0; 4];
        }
        let i = ((y * atlas.width + x) * 4) as usize;
        [atlas.rgba[i], atlas.rgba[i + 1], atlas.rgba[i + 2], atlas.rgba[i + 3]]
    };
    let top = [look.fill[0] + (1.0 - look.fill[0]) * 0.7, look.fill[1] + (1.0 - look.fill[1]) * 0.7, look.fill[2] + (1.0 - look.fill[2]) * 0.7];
    let bottom = [look.fill[0] * 0.70, look.fill[1] * 0.70, look.fill[2] * 0.74];
    let mut cursor = 2u32;
    for (a, b) in LETTERS {
        for sx in a.saturating_sub(2)..=b + 2 {
            let dx = cursor as i64 + sx as i64 - a as i64;
            if dx < 0 || dx >= w as i64 {
                continue;
            }
            for y in r0..=r1 {
                let mut p = px(sx, y);
                let face = is_face(&p) && (a..=b).contains(&sx);
                let c = if face { 2 } else if p[3] > 0 { 1 } else { 0 };
                if c == 0 {
                    continue;
                }
                if face {
                    // the face: a silver ramp top -> bottom, shaded by the letter's own brightness
                    let t = ((y as f32 - FACE_ROWS.0 as f32) / (FACE_ROWS.1 - FACE_ROWS.0) as f32).clamp(0.0, 1.0);
                    let shade = (p[0] as f32 / 255.0).clamp(0.8, 1.0);
                    for k in 0..3 {
                        p[k] = ((top[k] + (bottom[k] - top[k]) * t) * shade * 255.0).round().clamp(0.0, 255.0) as u8;
                    }
                } else {
                    // outline / shadow / a neighbour's face: neutral dark (the red tint taken out)
                    let l = if is_face(&p) { 20.0 } else { 0.3 * p[0] as f32 + 0.59 * p[1] as f32 + 0.11 * p[2] as f32 };
                    let l = (l * 0.6).round().clamp(0.0, 255.0) as u8;
                    p = [l, l, l, p[3]];
                }
                let di = (y - r0) as usize * w as usize + dx as usize;
                if c > class[di] || (c == class[di] && p[3] > out[di * 4 + 3]) {
                    class[di] = c;
                    out[di * 4..di * 4 + 4].copy_from_slice(&p);
                }
            }
        }
        cursor += b - a + 2;
    }
    Texture { file: "(GOOFY-GG) GUARD label from textures/Scaleform/01_hud/hud_status/HudStatusTxt.png".to_string(), width: w, height: h, rgba: out }
}

fn tint(c: [f32; 4]) -> Cx {
    Cx { mul: [0.0, 0.0, 0.0, c[3]], add: [c[0] * 255.0, c[1] * 255.0, c[2] * 255.0, 0.0] }
}

/// The bar's driver (one per HUD).
#[derive(Clone, Debug, Default)]
pub struct GuardBar {
    installed: bool,
    /// The bar is up.
    pub shown: bool,
    max_frame: i32,
    frame: i32,
    loss: FollowGauge,
    loss_frame: i32,
    broken: bool,
    blink: f32,
    /// What was last applied (tests / logs): (track frame, fill frame, under-fill frame, broken).
    pub last: (i32, i32, i32, bool),
}

impl GuardBar {
    /// Makes the copy of the MP part (once per movie). False when the movie has no MP part.
    fn install(&mut self, hud: &mut HudStatus, look: &GuardLook) -> bool {
        let m = &mut hud.movie;
        if !m.exists(MP) {
            return false;
        }
        if !m.exists(GUARD) && !m.clone_child(PC, "pcMp", "pcGuard") {
            return false;
        }
        // the label: the MP logo's own bitmap fill, a GUARD texture of the same texel density in its place
        let logo = format!("{PACK}.coPcMpLogo");
        let fill = m.sprite(&format!("{MP}.pcMpPack.coPcMpLogo")).and_then(|s| {
            s.children.iter().find_map(|n| match &n.kind {
                Kind::Shape(sh) => sh.fills.iter().find_map(|f| match &f.paint {
                    Paint::Bitmap(b) => Some(b.clone()),
                    _ => None,
                }),
                _ => None,
            })
        });
        if let (Some(b), Some(s)) = (fill, m.sprite_mut(&logo)) {
            let label = Arc::new(guard_label_texture(&b.texture, look));
            let (sx, sy) = (b.scale.0.max(1e-4), b.scale.1.max(1e-4));
            // where the atlas' "MP" glyphs start on the logo clip, and one atlas texel in clip units
            // centred on the MP glyphs' rows (49 ..= 64), left-aligned with them
            let top = MP_GLYPH_ORIGIN.1 + 7.5 - (ROWS.1 - ROWS.0) as f32 / 2.0;
            let (x0, y0) = b.matrix.apply(MP_GLYPH_ORIGIN.0 / sx, top / sy);
            let matrix = Matrix::new(b.matrix.a, b.matrix.b, b.matrix.c, b.matrix.d, x0, y0);
            let (w, h) = (label.width as f32 / sx * b.matrix.a, label.height as f32 / sy * b.matrix.d);
            s.remove_children();
            s.gfx.clear();
            s.gfx.push(GfxRect { x: x0, y: y0, w, h, fill: BitmapFill { texture: label, matrix, scale: b.scale, repeat: false, smooth: true } });
        }
        for other in ["coPcMpLogoC", "coPcMpLogoC1", "coPcMpLogo1"] {
            m.set_visible(&format!("{PACK}.{other}"), false);
        }
        m.goto_and_play(PACK, "loop");
        self.installed = true;
        self.max_frame = -1;
        self.frame = -1;
        self.loss_frame = -1;
        self.broken = false;
        let _ = look;
        true
    }

    /// One update of the status HUD (after the driver's own): `v` None = no bar.
    pub fn update(&mut self, hud: &mut HudStatus, v: Option<(&GuardValues, &GuardLook)>, dt: f32) {
        let Some((v, look)) = v else {
            if self.installed && self.shown {
                hud.movie.set_visible(GUARD, false);
            }
            self.shown = false;
            return;
        };
        if !hud.movie.exists(GUARD) {
            self.installed = false;
        }
        if !self.installed && !self.install(hud, look) {
            return;
        }
        let m = &mut hud.movie;
        // follow the MP part: matrix (+ the offset in its parent's units), colour, visibility, its own frame
        if let Some((matrix, cx, visible)) = m.node(MP).map(|n| (n.matrix, n.cx, n.is_visible())) {
            let frame = m.current_frame(MP);
            if let Some(g) = m.node_mut(GUARD) {
                let mut mx = matrix;
                mx.tx += look.offset.0;
                mx.ty += look.offset.1;
                g.matrix = mx;
                g.script_matrix = true;
                g.cx = cx;
                g.script_visible = Some(visible);
            }
            if let Some(f) = frame {
                if m.current_frame(GUARD) != Some(f) {
                    m.goto_frame(GUARD, f);
                }
            }
        }
        self.shown = true;
        // the track: KH3's MP length rule
        let max = v.max.round().max(1.0) as i32;
        let max_frame = gauge_frame(max, max, (max as f32 * 1.8) as i32).clamp(1, 500);
        if max_frame != self.max_frame {
            self.max_frame = max_frame;
            m.goto_frame(&format!("{PACK}.pcMpDodai"), max_frame as u32 + 1);
            m.goto_frame(&format!("{PACK}.pcMpDodaiC"), max_frame as u32 + 1);
        }
        let value = (v.value.max(0.0) + 0.5) as i32;
        let frame = gauge_frame(value.min(max), max, max_frame).clamp(0, 500);
        // the broken look: KH3's MP-charge labels on the copy
        if v.broken != self.broken {
            self.broken = v.broken;
            m.goto_and_play(PACK, if v.broken { "charge_in" } else { "charge_out" });
            self.loss = FollowGauge::default();
            self.loss_frame = frame;
        }
        // depletion: the red under-fill follows the fill down (FollowGauge, as the HP gauge's red part)
        if self.frame >= 0 && frame < self.frame && !v.broken {
            if let Some(f) = self.loss.set_gauge(self.frame.max(self.loss_frame) + 1, frame + 1) {
                self.loss_frame = f as i32 - 1;
            }
        } else if frame > self.frame {
            self.loss = FollowGauge::default();
            self.loss_frame = frame;
        }
        if let Some(f) = self.loss.advance(dt as f64 * 1000.0) {
            self.loss_frame = f as i32 - 1;
        }
        self.frame = frame;
        self.blink = (self.blink + dt * look.blink_hz.max(0.0)).fract();
        let gg = format!("{PACK}.pcMpGg");
        let ggc = format!("{PACK}.pcMpGgC");
        if v.broken {
            // the refill in the broken grey under a hidden fill; the grey and the label blink
            m.set_visible(&gg, false);
            m.set_visible(&ggc, frame > 0);
            m.goto_frame(&ggc, frame as u32 + 1);
            let on = self.blink < 0.5;
            let c = if on { look.broken } else { [look.broken[0] * 0.6, look.broken[1] * 0.6, look.broken[2] * 0.6, look.broken[3]] };
            if let Some(n) = m.node_mut(&ggc) {
                n.cx = tint(c);
            }
            if let Some(n) = m.node_mut(&format!("{PACK}.coPcMpLogo")) {
                n.cx = Cx { mul: [1.0, if on { 0.45 } else { 1.0 }, if on { 0.45 } else { 1.0 }, 1.0], add: [0.0; 4] };
                n.script_visible = Some(true); // the charge labels show the CHARGE logos: the copy keeps GUARD
            }
            self.last = (max_frame, 0, frame, true);
        } else {
            m.set_visible(&gg, true);
            m.goto_frame(&gg, frame as u32 + 1); // the driver's rule: `setMP(slot, frame + 1)`
            if let Some(n) = m.node_mut(&gg) {
                n.cx = tint(if v.regenerating { look.regen } else { look.fill });
            }
            let lost = self.loss_frame > frame;
            m.set_visible(&ggc, lost);
            if lost {
                m.goto_frame(&ggc, self.loss_frame as u32 + 1);
                if let Some(n) = m.node_mut(&ggc) {
                    n.cx = tint(look.loss);
                }
            }
            if let Some(n) = m.node_mut(&format!("{PACK}.coPcMpLogo")) {
                n.cx = Cx::IDENTITY;
                n.script_visible = Some(true);
            }
            self.last = (max_frame, frame, if lost { self.loss_frame } else { 0 }, false);
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// (GOOFY) RESOLVE (documentation/GOOFY_OVERHAUL.md 7, PLAN.md R20): three pips beside GUARD, KH3's style-gauge chevrons
// (`hud_command/StylePntGg.png`) on the MP bar's rounded base (`hud_status/HudStatusMpBase9.png`), as Donald's Resonance pips.
// They stand over the GUARD label, whose place does not change with the bar's length, and follow the GUARD part's matrix, colour and
// visibility, so they move with it through the leader switch.

/// Resolve as the HUD draws it (kh-party `GoofyHud`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ResolveValues {
    /// In thirds of a pip, 0 ..= 9.
    pub thirds: u8,
    /// In the Zone (3 pips).
    pub in_zone: bool,
}

/// The look (sheet `party-guard` rows `hud.resolve_*`): units of the status HUD, straight RGBA 0..1.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolveLook {
    /// The plate's centre, from the GUARD part's origin.
    pub at: (f32, f32),
    pub pip_size: (f32, f32),
    pub pip_step: f32,
    pub plate_pad: f32,
    pub plate_h: f32,
    pub plate: [f32; 4],
    pub lit: [f32; 4],
    pub off: [f32; 4],
    pub zone: [f32; 4],
    pub zone_hz: f32,
}

impl Default for ResolveLook {
    fn default() -> ResolveLook {
        ResolveLook { at: (0.0, 0.0), pip_size: (26.0, 13.0), pip_step: 23.0, plate_pad: 8.0, plate_h: 20.0, plate: [0.2, 0.2, 0.23, 0.88], lit: [0.92, 0.94, 1.0, 1.0], off: [0.3, 0.3, 0.34, 0.95], zone: [1.0, 0.8, 0.3, 1.0], zone_hz: 2.0 }
    }
}

const PIP_CLASS: &str = "CoPcMpLogo";
const PIPS: usize = 3;

fn pip_base(i: usize) -> String {
    format!("{PC}.gofPipBase{i}")
}

fn pip_lit(i: usize) -> String {
    format!("{PC}.gofPipLit{i}")
}

const PLATE: &str = "sttsPc.gofPipPlate";

/// The pips' driver (one per HUD).
#[derive(Clone, Debug, Default)]
pub struct ResolvePips {
    installed: bool,
    /// The pips are up.
    pub shown: bool,
    phase: f32,
    /// What was last applied (tests): each pip's share, in the Zone.
    pub last: ([f32; PIPS], bool),
    /// The plate's box (tests): left, top, right, bottom in the status HUD's units.
    pub plate_box: (f32, f32, f32, f32),
}

impl ResolvePips {
    fn hide(&self, hud: &mut HudStatus) {
        let m = &mut hud.movie;
        m.set_visible(PLATE, false);
        for i in 0..PIPS {
            m.set_visible(&pip_base(i), false);
            m.set_visible(&pip_lit(i), false);
        }
    }

    /// One update of the status HUD, after the GUARD bar's: `v` None = no pips.
    pub fn update(&mut self, hud: &mut HudStatus, art: &crate::hud_resonance::DonArt, v: Option<(&ResolveValues, &ResolveLook)>, dt: f32) {
        use crate::hud_resonance::{ensure_art_of, multiply, pill, set_art, tex_rect, CHEVRON_RECT};
        let Some((v, look)) = v else {
            if self.installed && self.shown {
                self.hide(hud);
            }
            self.shown = false;
            return;
        };
        let m = &mut hud.movie;
        let Some((matrix, cx, visible)) = m.node(GUARD).map(|n| (n.matrix, n.cx, n.is_visible())) else { return };
        if !self.installed || !m.exists(PLATE) {
            if !ensure_art_of(m, PIP_CLASS, PC, "gofPipPlate") {
                return;
            }
            for i in 0..PIPS {
                if !ensure_art_of(m, PIP_CLASS, PC, &format!("gofPipBase{i}")) || !ensure_art_of(m, PIP_CLASS, PC, &format!("gofPipLit{i}")) {
                    return;
                }
            }
            self.installed = true;
        }
        let (Some(chevron), Some(base)) = (art.chevron.as_ref(), art.base.as_ref()) else { return };
        self.shown = true;
        self.phase = (self.phase + dt * look.zone_hz.max(0.0)).fract();
        let row = (PIPS as f32 - 1.0) * look.pip_step + look.pip_size.0;
        let width = row + 2.0 * look.plate_pad;
        let (x0, y0) = (look.at.0 - width / 2.0, look.at.1 - look.plate_h / 2.0);
        set_art(m, PLATE, pill(base, x0, y0, width, look.plate_h), multiply(look.plate), visible);
        let (pw, ph) = look.pip_size;
        let py = look.at.1 - ph / 2.0;
        // In the Zone the pips pulse between the lit colour and the Zone's
        let zone = if v.in_zone {
            let k = 0.5 + 0.5 * (self.phase * std::f32::consts::TAU).sin();
            std::array::from_fn(|c| look.zone[c] + (look.lit[c] - look.zone[c]) * k * 0.45)
        } else {
            look.lit
        };
        let mut shares = [0.0f32; PIPS];
        for (i, share) in shares.iter_mut().enumerate() {
            let s = (v.thirds as f32 / 3.0 - i as f32).clamp(0.0, 1.0);
            *share = s;
            let x = x0 + look.plate_pad + i as f32 * look.pip_step;
            set_art(m, &pip_base(i), vec![tex_rect(chevron, CHEVRON_RECT, (x, py, pw, ph))], multiply(look.off), visible);
            // the pip being filled brightens by thirds, as Donald's Resonance pip does: a chevron's own shape hides a part fill
            let c: [f32; 4] = if s >= 1.0 { zone } else { std::array::from_fn(|k| look.off[k] + (zone[k] - look.off[k]) * (0.3 + 0.45 * s)) };
            set_art(m, &pip_lit(i), vec![tex_rect(chevron, CHEVRON_RECT, (x, py, pw, ph))], multiply(c), visible && s > 0.0);
        }
        // the art follows the GUARD part's place and its fade
        let nodes = std::iter::once(PLATE.to_string()).chain((0..PIPS).flat_map(|i| [pip_base(i), pip_lit(i)]));
        for path in nodes {
            if let Some(n) = m.node_mut(&path) {
                n.matrix = matrix;
                n.script_matrix = true;
                let own = n.cx;
                n.cx = Cx { mul: [own.mul[0] * cx.mul[0], own.mul[1] * cx.mul[1], own.mul[2] * cx.mul[2], own.mul[3] * cx.mul[3]], add: own.add };
            }
        }
        self.last = (shares, v.in_zone);
        self.plate_box = (matrix.tx + x0, matrix.ty + y0, matrix.tx + x0 + width, matrix.ty + y0 + look.plate_h);
    }
}
