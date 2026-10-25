//! Software rasteriser: the same rules as the offline compositor `tools/lib/gfx_render.js`, so
//! that both draw the same picture for the same display list: even-odd coverage from 4 vertical
//! sub-scanlines with exact horizontal span coverage, premultiplied float pixels, bitmap fills
//! sampled bilinearly at pixel centres with the colour transform applied on straight colour.

use crate::def::Texture;
use crate::geom::{Cx, Matrix, Rect};

/// Vertical sub-scanlines per pixel (`SS` of the reference).
pub const SS: usize = 4;

/// A premultiplied RGBA float surface covering the device rectangle `(x0, y0, w, h)`.
#[derive(Clone, Debug, Default)]
pub struct Canvas {
    pub x0: i32,
    pub y0: i32,
    pub w: usize,
    pub h: usize,
    pub px: Vec<f32>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas { x0: 0, y0: 0, w, h, px: vec![0.0; w * h * 4] }
    }

    pub fn region(x0: i32, y0: i32, w: usize, h: usize) -> Canvas {
        Canvas { x0, y0, w, h, px: vec![0.0; w * h * 4] }
    }

    /// Reuses the allocation: the surface now covers the given rectangle, cleared.
    pub fn reset(&mut self, x0: i32, y0: i32, w: usize, h: usize) {
        (self.x0, self.y0, self.w, self.h) = (x0, y0, w, h);
        self.px.clear();
        self.px.resize(w * h * 4, 0.0);
    }

    pub fn rect(&self) -> Rect {
        Rect::new(self.x0 as f32, self.y0 as f32, (self.x0 + self.w as i32) as f32, (self.y0 + self.h as i32) as f32)
    }

    /// Straight-alpha RGBA8 (what `toRGBA8(null)` of the reference gives).
    pub fn to_rgba8(&self, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.w * self.h * 4);
        for p in self.px.chunks_exact(4) {
            let a = p[3];
            if a <= 1.0e-6 {
                out.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let ia = 255.0 / a;
            let c = |v: f32| (v * ia + 0.5).clamp(0.0, 255.0) as u8;
            out.extend_from_slice(&[c(p[0]), c(p[1]), c(p[2]), (a * 255.0 + 0.5).clamp(0.0, 255.0) as u8]);
        }
    }

    /// Composited over an opaque background colour (0..255), as `toRGBA8(bg)`.
    pub fn to_rgba8_over(&self, bg: impl Fn(usize, usize) -> [f32; 3], out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.w * self.h * 4);
        for (i, p) in self.px.chunks_exact(4).enumerate() {
            let b = bg(i % self.w, i / self.w);
            let a = p[3];
            let c = |v: f32, b: f32| ((v + b * (1.0 - a)) * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
            out.extend_from_slice(&[c(p[0], b[0]), c(p[1], b[1]), c(p[2], b[2]), 255]);
        }
    }
}

/// A mask: coverage 0..1 over a device rectangle, 0 outside it.
#[derive(Clone, Debug, Default)]
pub struct Mask {
    pub x0: i32,
    pub y0: i32,
    pub w: usize,
    pub h: usize,
    pub alpha: Vec<f32>,
}

impl Mask {
    pub fn rect(&self) -> Rect {
        Rect::new(self.x0 as f32, self.y0 as f32, (self.x0 + self.w as i32) as f32, (self.y0 + self.h as i32) as f32)
    }

    #[inline]
    pub fn at(&self, x: i32, y: i32) -> f32 {
        let (lx, ly) = (x - self.x0, y - self.y0);
        if lx < 0 || ly < 0 || lx as usize >= self.w || ly as usize >= self.h {
            0.0
        } else {
            self.alpha[ly as usize * self.w + lx as usize]
        }
    }
}

/// Coverage of a set of edges over an integer rectangle.
#[derive(Clone, Debug, Default)]
pub struct Coverage {
    pub x0: i32,
    pub y0: i32,
    pub w: usize,
    pub h: usize,
    pub cov: Vec<f32>,
}

/// Even-odd coverage of `edges` (device pixels, `[x0, y0, x1, y1]`), limited to `limit`.
/// `scratch` is reused between calls.
pub fn coverage(edges: &[[f32; 4]], limit: &Rect, out: &mut Coverage, xs: &mut Vec<f32>) -> bool {
    let mut b = Rect::EMPTY;
    for e in edges {
        b.add_point(e[0], e[1]);
        b.add_point(e[2], e[3]);
    }
    let x0 = b.x0.floor().max(limit.x0);
    let y0 = b.y0.floor().max(limit.y0);
    let x1 = b.x1.ceil().min(limit.x1);
    let y1 = b.y1.ceil().min(limit.y1);
    if !(x1 > x0 && y1 > y0) {
        return false;
    }
    let (bw, bh) = ((x1 - x0) as usize, (y1 - y0) as usize);
    out.x0 = x0 as i32;
    out.y0 = y0 as i32;
    out.w = bw;
    out.h = bh;
    out.cov.clear();
    out.cov.resize(bw * bh, 0.0);
    let inv_ss = 1.0 / SS as f32;
    let bwf = bw as f32;
    let lines = bh * SS;
    // The crossings of every sub-scanline, bucketed: each edge is visited once per sub-scanline
    // it spans (the reference tests every edge on every sub-scanline; the result is the same).
    let y_of = |s: i32| y0 + (s as f32 + 0.5) * inv_ss;
    // The first sub-scanline whose sample is at or after the height v (y_s >= v).
    let first_at_or_after = |v: f32| -> i32 {
        let mut s = (((v - y0) * SS as f32 - 0.5).ceil() as i32).clamp(0, lines as i32);
        while s > 0 && y_of(s - 1) >= v {
            s -= 1;
        }
        while s < lines as i32 && y_of(s) < v {
            s += 1;
        }
        s
    };
    let mut starts = vec![0u32; lines + 1];
    let mut spans: Vec<(i32, i32)> = Vec::with_capacity(edges.len());
    for e in edges {
        let (lo, hi) = if e[1] <= e[3] { (e[1], e[3]) } else { (e[3], e[1]) };
        // (ya <= y) != (yb <= y)  <=>  lo <= y < hi
        let (s0, s1) = if lo < hi { (first_at_or_after(lo), first_at_or_after(hi)) } else { (0, 0) };
        spans.push((s0, s1));
        for s in s0..s1 {
            starts[s as usize + 1] += 1;
        }
    }
    for s in 0..lines {
        starts[s + 1] += starts[s];
    }
    xs.clear();
    xs.resize(starts[lines] as usize, 0.0);
    let mut fill_at: Vec<u32> = starts[..lines].to_vec();
    for (e, (s0, s1)) in edges.iter().zip(&spans) {
        let (ya, yb) = (e[1], e[3]);
        for s in *s0..*s1 {
            let y = y_of(s);
            let at = &mut fill_at[s as usize];
            xs[*at as usize] = e[0] + (y - ya) * (e[2] - e[0]) / (yb - ya);
            *at += 1;
        }
    }
    for sy in 0..lines {
        let xs = &mut xs[starts[sy] as usize..starts[sy + 1] as usize];
        if xs.len() < 2 {
            continue;
        }
        xs.sort_unstable_by(|a, b| a.total_cmp(b));
        let row = &mut out.cov[(sy / SS) * bw..(sy / SS + 1) * bw];
        let mut k = 0;
        while k + 1 < xs.len() {
            let (mut a, mut b) = (xs[k] - x0, xs[k + 1] - x0);
            k += 2;
            if b <= 0.0 || a >= bwf {
                continue;
            }
            a = a.max(0.0);
            b = b.min(bwf);
            let (ia, ib) = (a.floor() as usize, b.floor() as usize);
            if ia == ib {
                row[ia] += (b - a) * inv_ss;
                continue;
            }
            row[ia] += (ia as f32 + 1.0 - a) * inv_ss;
            for v in &mut row[ia + 1..ib] {
                *v += inv_ss;
            }
            if ib < bw {
                row[ib] += (b - ib as f32) * inv_ss;
            }
        }
    }
    true
}

/// Coverage of the axis-aligned rectangle `r`: what `coverage` gives for its four edges, without
/// walking edges (the bulk of the HUD's art is bitmap rectangles).
pub fn coverage_rect(r: &Rect, limit: &Rect, out: &mut Coverage) -> bool {
    let x0 = r.x0.floor().max(limit.x0);
    let y0 = r.y0.floor().max(limit.y0);
    let x1 = r.x1.ceil().min(limit.x1);
    let y1 = r.y1.ceil().min(limit.y1);
    if !(x1 > x0 && y1 > y0) {
        return false;
    }
    let (bw, bh) = ((x1 - x0) as usize, (y1 - y0) as usize);
    out.x0 = x0 as i32;
    out.y0 = y0 as i32;
    out.w = bw;
    out.h = bh;
    out.cov.clear();
    out.cov.resize(bw * bh, 0.0);
    let inv_ss = 1.0 / SS as f32;
    for row in 0..bh {
        // Sub-scanlines inside [top, bottom): the crossing rule of `coverage`.
        let mut n = 0;
        for s in 0..SS {
            let y = y0 + row as f32 + (s as f32 + 0.5) * inv_ss;
            if r.y0 <= y && y < r.y1 {
                n += 1;
            }
        }
        if n == 0 {
            continue;
        }
        let cy = n as f32 * inv_ss;
        let line = &mut out.cov[row * bw..(row + 1) * bw];
        for (i, v) in line.iter_mut().enumerate() {
            let px = x0 + i as f32;
            let c = (r.x1.min(px + 1.0) - r.x0.max(px)).clamp(0.0, 1.0);
            *v = c * cy;
        }
    }
    true
}

/// Flattens a quadratic curve (already in device space) the way the reference does.
pub fn flatten(a: (f32, f32), b: (f32, f32), d: (f32, f32), out: &mut Vec<[f32; 4]>) {
    let n = (((a.0 - 2.0 * b.0 + d.0).hypot(a.1 - 2.0 * b.1 + d.1)).sqrt() * 1.5).ceil().clamp(2.0, 24.0) as usize;
    let (mut lx, mut ly) = a;
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let u = 1.0 - t;
        let x = u * u * a.0 + 2.0 * u * t * b.0 + t * t * d.0;
        let y = u * u * a.1 + 2.0 * u * t * b.1 + t * t * d.1;
        out.push([lx, ly, x, y]);
        lx = x;
        ly = y;
    }
}

/// A prepared paint: premultiplied RGBA for a device pixel centre.
pub enum PaintFn<'a> {
    Solid([f32; 4]),
    Gradient { inv: Matrix, radial: bool, stops: Vec<(f32, [f32; 4])> },
    Bitmap { inv: Matrix, scale: (f32, f32), texture: &'a Texture, repeat: bool, smooth: bool, cx: Option<Cx> },
}

/// A straight colour (0..255) under a colour transform, premultiplied 0..1.
pub fn solid(c: [f32; 4], cx: &Cx) -> [f32; 4] {
    let a = ((c[3] * cx.mul[3] + cx.add[3]) / 255.0).clamp(0.0, 1.0);
    let f = |i: usize| ((c[i] * cx.mul[i] + cx.add[i]) / 255.0).clamp(0.0, 1.0) * a;
    [f(0), f(1), f(2), a]
}

#[inline]
fn texel(t: &Texture, mut ix: i32, mut iy: i32, repeat: bool) -> [f32; 4] {
    let (w, h) = (t.width as i32, t.height as i32);
    if repeat {
        ix = ix.rem_euclid(w);
        iy = iy.rem_euclid(h);
    } else {
        ix = ix.clamp(0, w - 1);
        iy = iy.clamp(0, h - 1);
    }
    let k = (iy * w + ix) as usize * 4;
    let d = &t.rgba[k..k + 4];
    const I: f32 = 1.0 / 255.0;
    let a = d[3] as f32 * I;
    [d[0] as f32 * I * a, d[1] as f32 * I * a, d[2] as f32 * I * a, a]
}

impl PaintFn<'_> {
    #[inline]
    pub fn at(&self, x: f32, y: f32) -> [f32; 4] {
        match self {
            PaintFn::Solid(c) => *c,
            PaintFn::Gradient { inv, radial, stops } => {
                let (gx, gy) = inv.apply(x, y);
                let t = if *radial { gx.hypot(gy) / 16384.0 } else { (gx + 16384.0) / 32768.0 }.clamp(0.0, 1.0);
                if stops.is_empty() {
                    return [0.0; 4];
                }
                if t <= stops[0].0 {
                    return stops[0].1;
                }
                for i in 1..stops.len() {
                    if t <= stops[i].0 {
                        let (a, b) = (&stops[i - 1], &stops[i]);
                        let u = (t - a.0) / (b.0 - a.0).max(1.0e-6);
                        return [a.1[0] + (b.1[0] - a.1[0]) * u, a.1[1] + (b.1[1] - a.1[1]) * u, a.1[2] + (b.1[2] - a.1[2]) * u, a.1[3] + (b.1[3] - a.1[3]) * u];
                    }
                }
                stops[stops.len() - 1].1
            }
            PaintFn::Bitmap { inv, scale, texture, repeat, smooth, cx } => {
                let (bx, by) = inv.apply(x, y);
                let (u, v) = (bx * scale.0 - 0.5, by * scale.1 - 0.5);
                let o = if *smooth {
                    let (iu, iv) = (u.floor(), v.floor());
                    let (fu, fv) = (u - iu, v - iv);
                    let (iu, iv) = (iu as i32, iv as i32);
                    let t0 = texel(texture, iu, iv, *repeat);
                    let t1 = texel(texture, iu + 1, iv, *repeat);
                    let t2 = texel(texture, iu, iv + 1, *repeat);
                    let t3 = texel(texture, iu + 1, iv + 1, *repeat);
                    let mut o = [0.0; 4];
                    for k in 0..4 {
                        o[k] = (t0[k] * (1.0 - fu) + t1[k] * fu) * (1.0 - fv) + (t2[k] * (1.0 - fu) + t3[k] * fu) * fv;
                    }
                    o
                } else {
                    texel(texture, u.round() as i32, v.round() as i32, *repeat)
                };
                let Some(cx) = cx else { return o };
                // The colour transform acts on straight colour.
                let a = o[3];
                if a <= 1.0e-6 && cx.add[3] == 0.0 {
                    return o;
                }
                let ia = if a > 1.0e-6 { 1.0 / a } else { 0.0 };
                let na = (a * cx.mul[3] + cx.add[3] / 255.0).clamp(0.0, 1.0);
                let f = |i: usize| (o[i] * ia * cx.mul[i] + cx.add[i] / 255.0).clamp(0.0, 1.0) * na;
                [f(0), f(1), f(2), na]
            }
        }
    }
}

/// `fillCov` of the reference: source-over of the paint through the coverage (and the mask).
/// Bitmap and solid paints take their own loops (the same arithmetic, without the per-pixel
/// dispatch; a bitmap's texture coordinate is stepped along the row).
pub fn fill(canvas: &mut Canvas, c: &Coverage, paint: &PaintFn, clip: Option<&Mask>) {
    // The rows and columns of the coverage that lie on the canvas.
    let y_from = (canvas.y0 - c.y0).max(0) as usize;
    let y_to = ((canvas.y0 + canvas.h as i32 - c.y0).max(0) as usize).min(c.h);
    let x_from = (canvas.x0 - c.x0).max(0) as usize;
    let x_to = ((canvas.x0 + canvas.w as i32 - c.x0).max(0) as usize).min(c.w);
    if y_from >= y_to || x_from >= x_to {
        return;
    }
    let bitmap = match paint {
        PaintFn::Bitmap { inv, scale, texture, repeat, smooth: true, cx } => Some((inv, scale, *texture, *repeat, cx)),
        _ => None,
    };
    for y in y_from..y_to {
        let dy = c.y0 + y as i32;
        let cy = (dy - canvas.y0) as usize;
        let cov = &c.cov[y * c.w + x_from..y * c.w + x_to];
        let first = ((c.x0 + x_from as i32 - canvas.x0) as usize + cy * canvas.w) * 4;
        let row = &mut canvas.px[first..first + (x_to - x_from) * 4];
        // Texture coordinates of the row's first pixel centre and their step per pixel.
        let stepped = bitmap.map(|(inv, scale, ..)| {
            let (px, py) = ((c.x0 + x_from as i32) as f32 + 0.5, dy as f32 + 0.5);
            let (bx, by) = inv.apply(px, py);
            (bx * scale.0 - 0.5, by * scale.1 - 0.5, inv.a * scale.0, inv.b * scale.1)
        });
        for (i, (&k0, p)) in cov.iter().zip(row.chunks_exact_mut(4)).enumerate() {
            if k0 <= 0.0 {
                continue;
            }
            let mut k = k0.min(1.0);
            let dx = c.x0 + (x_from + i) as i32;
            if let Some(m) = clip {
                k *= m.at(dx, dy);
                if k <= 0.0 {
                    continue;
                }
            }
            let s = match (bitmap, stepped) {
                (Some((_, _, texture, repeat, cx)), Some((u0, v0, du, dv))) => {
                    let (u, v) = (u0 + du * i as f32, v0 + dv * i as f32);
                    match sample_smooth(texture, u, v, repeat, cx.is_none()) {
                        Some(o) => match cx {
                            None => o,
                            Some(cx) => apply_cx(o, cx),
                        },
                        None => continue,
                    }
                }
                _ => paint.at(dx as f32 + 0.5, dy as f32 + 0.5),
            };
            let sa = s[3] * k;
            if sa <= 0.0 && s[0] <= 0.0 && s[1] <= 0.0 && s[2] <= 0.0 {
                continue;
            }
            let inv = 1.0 - sa;
            p[0] = s[0] * k + p[0] * inv;
            p[1] = s[1] * k + p[1] * inv;
            p[2] = s[2] * k + p[2] * inv;
            p[3] = sa + p[3] * inv;
        }
    }
}

/// The colour transform on a premultiplied sample (it acts on straight colour).
#[inline]
fn apply_cx(o: [f32; 4], cx: &Cx) -> [f32; 4] {
    let a = o[3];
    if a <= 1.0e-6 && cx.add[3] == 0.0 {
        return o;
    }
    let ia = if a > 1.0e-6 { 1.0 / a } else { 0.0 };
    let na = (a * cx.mul[3] + cx.add[3] / 255.0).clamp(0.0, 1.0);
    let f = |i: usize| (o[i] * ia * cx.mul[i] + cx.add[i] / 255.0).clamp(0.0, 1.0) * na;
    [f(0), f(1), f(2), na]
}

/// Bilinear sample, premultiplied. `None` (only when `skip_empty`): all four texels are fully
/// transparent, so there is nothing to blend.
#[inline]
fn sample_smooth(t: &Texture, u: f32, v: f32, repeat: bool, skip_empty: bool) -> Option<[f32; 4]> {
    let (fu0, fv0) = (u.floor(), v.floor());
    let (fu, fv) = (u - fu0, v - fv0);
    let (iu, iv) = (fu0 as i32, fv0 as i32);
    let (w, h) = (t.width as i32, t.height as i32);
    const I: f32 = 1.0 / 255.0;
    if iu >= 0 && iv >= 0 && iu + 1 < w && iv + 1 < h {
        // All four texels inside: two slices, no wrapping.
        let k = (iv * w + iu) as usize * 4;
        let top = &t.rgba[k..k + 8];
        let k2 = k + w as usize * 4;
        let bottom = &t.rgba[k2..k2 + 8];
        if skip_empty && (top[3] | top[7] | bottom[3] | bottom[7]) == 0 {
            return None;
        }
        let tap = |d: &[u8]| {
            let a = d[3] as f32 * I;
            [d[0] as f32 * I * a, d[1] as f32 * I * a, d[2] as f32 * I * a, a]
        };
        let (t0, t1, t2, t3) = (tap(&top[..4]), tap(&top[4..]), tap(&bottom[..4]), tap(&bottom[4..]));
        let mut o = [0.0; 4];
        for c in 0..4 {
            o[c] = (t0[c] * (1.0 - fu) + t1[c] * fu) * (1.0 - fv) + (t2[c] * (1.0 - fu) + t3[c] * fu) * fv;
        }
        return Some(o);
    }
    let t0 = texel(t, iu, iv, repeat);
    let t1 = texel(t, iu + 1, iv, repeat);
    let t2 = texel(t, iu, iv + 1, repeat);
    let t3 = texel(t, iu + 1, iv + 1, repeat);
    let mut o = [0.0; 4];
    for c in 0..4 {
        o[c] = (t0[c] * (1.0 - fu) + t1[c] * fu) * (1.0 - fv) + (t2[c] * (1.0 - fu) + t3[c] * fu) * fv;
    }
    Some(o)
}

/// Composites a layer onto the canvas with a blend mode (8 add, 4 screen, 3 multiply; the
/// formulas of the reference). The layer covers its own rectangle.
/// (party X2) The coverage rule of ADD / SCREEN layers before 2026-10-16 (raised by the texel's alpha), for the
/// before / after comparison (kh-ui example `x2_blend_check`). Off = the current rule.
static LEGACY_ADD_COVERAGE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_legacy_add_coverage(on: bool) {
    LEGACY_ADD_COVERAGE.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn blend_layer(canvas: &mut Canvas, layer: &Canvas, mode: u8, clip: Option<&Mask>) {
    let legacy = LEGACY_ADD_COVERAGE.load(std::sync::atomic::Ordering::Relaxed);
    for ly in 0..layer.h {
        let dy = layer.y0 + ly as i32;
        let cy = dy - canvas.y0;
        if cy < 0 || cy as usize >= canvas.h {
            continue;
        }
        for lx in 0..layer.w {
            let s = &layer.px[(ly * layer.w + lx) * 4..(ly * layer.w + lx) * 4 + 4];
            let sa = s[3];
            if sa <= 0.0 {
                continue;
            }
            let dx = layer.x0 + lx as i32;
            let cxp = dx - canvas.x0;
            if cxp < 0 || cxp as usize >= canvas.w {
                continue;
            }
            let kk = clip.map_or(1.0, |m| m.at(dx, dy));
            if kk <= 0.0 {
                continue;
            }
            let i = (cy as usize * canvas.w + cxp as usize) * 4;
            let d = &mut canvas.px[i..i + 4];
            // (party X2) ADD / SCREEN ONTO A TRANSPARENT HUD CANVAS: the game draws these straight onto the scene, where
            // a dark texel adds nothing; this canvas is later laid OVER the scene (straight alpha, both hosts), so the
            // coverage it gains is the coverage that hides the scene. Raising it by the texel's ALPHA turned the dark
            // parts of an additive glow into a dark veil (the shotlock MAX flash `flowLockMax`: a grey square and a
            // black disc over the scene, Sora's too). The coverage now rises by the light added (the brightest
            // channel), the colour as before: black adds nothing, a bright texel is near the game's look. On an
            // opaque destination (the menus' backgrounds) nothing changes.
            let light = (s[0].max(s[1]).max(s[2]) * kk).clamp(0.0, 1.0);
            let light = if legacy { sa * kk } else { light };
            match mode {
                8 => {
                    for c in 0..3 {
                        d[c] = (d[c] + s[c] * kk).min(1.0);
                    }
                    d[3] = if legacy { (d[3] + light).min(1.0) } else { (d[3] + light * (1.0 - d[3])).min(1.0) };
                }
                4 => {
                    for c in 0..3 {
                        d[c] = d[c] + s[c] * kk - d[c] * s[c] * kk;
                    }
                    d[3] = (d[3] + light * (1.0 - d[3])).min(1.0);
                }
                _ => {
                    let da = d[3];
                    for c in 0..3 {
                        d[c] = s[c] * kk * d[c] + s[c] * kk * (1.0 - da) + d[c] * (1.0 - sa * kk);
                    }
                    d[3] = sa * kk + d[3] * (1.0 - sa * kk);
                }
            }
        }
    }
}
