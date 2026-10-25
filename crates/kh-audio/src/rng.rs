//! The driver's random numbers.
//!
//! The game calls the C runtime's `rand()` on the SEAD thread, seeded once from the clock
//! (docs/sora-audio-spec.md 2): its pick sequences are not reproducible, only their
//! distributions. The port keeps the generator (the MSVC CRT's linear congruential one, 15 bits
//! per call) and lets the HOST seed it, so that a run repeats. It is never the simulation's
//! state: sound must not be able to change what Sora does.

/// `rand()` of the Microsoft C runtime: `seed = seed * 214013 + 2531011; (seed >> 16) & 0x7FFF`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrtRand(u32);

impl CrtRand {
    pub const fn new(seed: u32) -> CrtRand {
        CrtRand(seed)
    }

    /// 0 ..= 32767.
    pub fn rand(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.0 >> 16) & 0x7FFF
    }
}

/// The generator of `TresAnimNotify_PlayVoice`'s weighted pick (`r = (xorshift >> 1) % total`,
/// spec 5.1). The game's own state is a global of the exe; the port's is seeded by the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XorShift(u32);

impl XorShift {
    pub const fn new(seed: u32) -> XorShift {
        XorShift(if seed == 0 { 0x9E37_79B9 } else { seed })
    }

    pub fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crt_rand_is_the_msvc_sequence() {
        // srand(1): the first values every MSVC program prints.
        let mut r = CrtRand::new(1);
        assert_eq!([r.rand(), r.rand(), r.rand(), r.rand(), r.rand()], [41, 18467, 6334, 26500, 19169]);
    }
}
