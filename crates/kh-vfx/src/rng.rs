//! The deterministic random stream of one effect instance (xorshift32). The game uses the
//! engine's global `SRand` / `FRandomStream`; the port only promises that the same spawn order
//! and the same steps give the same particles.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        // A zero state would stay zero.
        let mut rng = Rng(seed ^ 0x9E37_79B9);
        if rng.0 == 0 {
            rng.0 = 0x1234_5678;
        }
        // Decorrelate neighbouring seeds.
        for _ in 0..4 {
            rng.next_u32();
        }
        rng
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
}
