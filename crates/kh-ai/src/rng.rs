//! The random stream (plan N6: one deterministic stream per controller).
//!
//! KH3's `FMath::FRand` as the decompiled AI nodes inline it (e.g. `BTTask_Wait` ExecuteTask
//! 0x143C5BD40, `BTDecorator_Cooldown` 0x143C66900, `TresBTComposite_Random` pick 0x14118A3B0)
//! [C]: a xorshift128 over a four-word state (thread-local in the game, `FUN_142cff460`), result
//! `(x >> 8) * 2^-24` in [0, 1). The game's state is shared by everything on the game thread; the
//! port gives each controller its own seeded state so replays stay bit-identical (a departure in
//! the exact sequence, not in the distribution).

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    s: [u32; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        // splitmix64 to fill the state (never all zero)
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut next = || {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^ (x >> 31)
        };
        let a = next();
        let b = next();
        let mut s = [a as u32, (a >> 32) as u32, b as u32, (b >> 32) as u32];
        if s == [0; 4] {
            s[0] = 1;
        }
        Rng { s }
    }

    /// One step of the game's xorshift128 (0x143C5BD40 inline): returns the new last word.
    pub fn next_u32(&mut self) -> u32 {
        let mut t = self.s[0] << 11 ^ self.s[0];
        self.s[0] = self.s[1];
        self.s[1] = self.s[2];
        let w = self.s[3];
        self.s[2] = w;
        t = (w >> 11 ^ t) >> 8 ^ w ^ t;
        self.s[3] = t;
        t
    }

    /// `FMath::FRand()` [0, 1): `(x >> 8) * 5.9604645e-08`.
    pub fn frand(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * 5.960_464_5e-8
    }

    /// The fork's `RandomDeviation` pattern (Wait, Cooldown, TagCooldown, SetTagCooldown, service
    /// intervals): `lo = max(0, v - dev)`, result `lo + frand * (v + dev - lo)` [C].
    pub fn deviate(&mut self, v: f32, dev: f32) -> f32 {
        let lo = (v - dev).max(0.0);
        ((dev + v) - lo) * self.frand() + lo
    }

    /// `FMath::FRandRange(min, max)`.
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        self.frand() * (max - min) + min
    }

    /// `FMath::RandRange(min, max)` inclusive ints.
    pub fn int_range(&mut self, min: i32, max: i32) -> i32 {
        if max <= min {
            return min;
        }
        let r = self.frand();
        (min + (r * (max - min + 1) as f32) as i32).min(max)
    }
}
