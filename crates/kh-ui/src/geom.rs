//! 2D affine matrices, colour transforms and rectangles, with the conventions of the UI pack
//! (`build/sora_pack/ui/movies/*.json`, field `units`): a matrix `[a, b, c, d, tx, ty]` maps
//! `(x, y) -> (a*x + c*y + tx, b*x + d*y + ty)`; a colour transform is four multipliers
//! (1.0 = unchanged) and four additions in 0..255.

/// `[a, b, c, d, tx, ty]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Default for Matrix {
    fn default() -> Self {
        Matrix::IDENTITY
    }
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    pub const fn new(a: f32, b: f32, c: f32, d: f32, tx: f32, ty: f32) -> Matrix {
        Matrix { a, b, c, d, tx, ty }
    }

    pub const fn translate(tx: f32, ty: f32) -> Matrix {
        Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx, ty }
    }

    pub const fn scale(sx: f32, sy: f32) -> Matrix {
        Matrix { a: sx, b: 0.0, c: 0.0, d: sy, tx: 0.0, ty: 0.0 }
    }

    /// `self * m`: `m` is applied first (the child's matrix), then `self` (the parent's).
    pub fn mul(&self, m: &Matrix) -> Matrix {
        let p = self;
        Matrix {
            a: p.a * m.a + p.c * m.b,
            b: p.b * m.a + p.d * m.b,
            c: p.a * m.c + p.c * m.d,
            d: p.b * m.c + p.d * m.d,
            tx: p.a * m.tx + p.c * m.ty + p.tx,
            ty: p.b * m.tx + p.d * m.ty + p.ty,
        }
    }

    pub fn inverse(&self) -> Matrix {
        let m = self;
        let mut det = m.a * m.d - m.b * m.c;
        if det == 0.0 {
            det = 1.0e-12;
        }
        Matrix { a: m.d / det, b: -m.b / det, c: -m.c / det, d: m.a / det, tx: (m.c * m.ty - m.d * m.tx) / det, ty: (m.b * m.tx - m.a * m.ty) / det }
    }

    #[inline]
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (self.a * x + self.c * y + self.tx, self.b * x + self.d * y + self.ty)
    }

    /// True when the matrix neither rotates nor shears.
    pub fn is_axis_aligned(&self) -> bool {
        self.b.abs() < 1.0e-6 && self.c.abs() < 1.0e-6
    }

    pub fn bits(&self) -> [u32; 6] {
        [self.a.to_bits(), self.b.to_bits(), self.c.to_bits(), self.d.to_bits(), self.tx.to_bits(), self.ty.to_bits()]
    }
}

/// Colour transform: `out = in * mul + add` per channel, on straight (not premultiplied) colour
/// in 0..255.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cx {
    pub mul: [f32; 4],
    pub add: [f32; 4],
}

impl Default for Cx {
    fn default() -> Self {
        Cx::IDENTITY
    }
}

impl Cx {
    pub const IDENTITY: Cx = Cx { mul: [1.0; 4], add: [0.0; 4] };

    /// `self` is the parent's transform, `c` the child's.
    pub fn concat(&self, c: &Cx) -> Cx {
        let mut out = *self;
        for i in 0..4 {
            out.mul[i] = self.mul[i] * c.mul[i];
            out.add[i] = self.add[i] + c.add[i] * self.mul[i];
        }
        out
    }

    pub fn is_identity(&self) -> bool {
        *self == Cx::IDENTITY
    }

    /// Nothing of the object can be seen.
    pub fn is_invisible(&self) -> bool {
        self.mul[3] <= 0.0 && self.add[3] <= 0.0
    }
}

/// An axis-aligned rectangle; empty when `x1 <= x0` or `y1 <= y0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    pub const EMPTY: Rect = Rect { x0: f32::INFINITY, y0: f32::INFINITY, x1: f32::NEG_INFINITY, y1: f32::NEG_INFINITY };

    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect { x0, y0, x1, y1 }
    }

    pub fn is_empty(&self) -> bool {
        !(self.x1 > self.x0 && self.y1 > self.y0)
    }

    pub fn add_point(&mut self, x: f32, y: f32) {
        self.x0 = self.x0.min(x);
        self.y0 = self.y0.min(y);
        self.x1 = self.x1.max(x);
        self.y1 = self.y1.max(y);
    }

    pub fn union(&self, o: &Rect) -> Rect {
        if o.is_empty() {
            return *self;
        }
        if self.is_empty() {
            return *o;
        }
        Rect { x0: self.x0.min(o.x0), y0: self.y0.min(o.y0), x1: self.x1.max(o.x1), y1: self.y1.max(o.y1) }
    }

    pub fn intersect(&self, o: &Rect) -> Rect {
        Rect { x0: self.x0.max(o.x0), y0: self.y0.max(o.y0), x1: self.x1.min(o.x1), y1: self.y1.min(o.y1) }
    }

    /// The bounding box of this rectangle under `m`.
    pub fn transform(&self, m: &Matrix) -> Rect {
        if self.is_empty() {
            return Rect::EMPTY;
        }
        let mut r = Rect::EMPTY;
        for (x, y) in [(self.x0, self.y0), (self.x1, self.y0), (self.x1, self.y1), (self.x0, self.y1)] {
            let (px, py) = m.apply(x, y);
            r.add_point(px, py);
        }
        r
    }
}
