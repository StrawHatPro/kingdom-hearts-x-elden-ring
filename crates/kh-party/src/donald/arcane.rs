//! (DON1) ARCANE FLOW, the Y magic's meter (documentation/DONALD_OVERHAUL.md 6.3, 6.5): 4 charges, gained by playing
//! the attunement system well (a reaction, a Release, three elements within 10 s, a perfect Magic Block). In combat
//! each charge lasts `arcane.life` (20 s; Arcane Flow+ 30 s) from its gain; out of combat the meter empties. Casting a Y
//! spell spends only its own cost, the oldest charges first (they would expire first).

/// Why a charge came.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArcaneGain {
    Reaction,
    Release,
    ThreeElements,
    PerfectBlock,
}

/// The meter: the clock time each charge expires at, oldest first.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ArcaneFlow {
    pub expiry: Vec<f32>,
}

impl ArcaneFlow {
    pub fn charges(&self) -> u8 {
        self.expiry.len() as u8
    }

    /// One charge (none past `max`). True when it was added.
    pub fn gain(&mut self, now: f32, life: f32, max: u8) -> bool {
        if self.expiry.len() >= max as usize {
            // a full meter: the gain refreshes the oldest charge instead (nothing is lost by playing well)
            if let Some(first) = self.expiry.first_mut() {
                *first = now + life;
                self.expiry.sort_by(f32::total_cmp);
            }
            return false;
        }
        self.expiry.push(now + life);
        self.expiry.sort_by(f32::total_cmp);
        true
    }

    /// Spends `n` charges (the oldest). False (nothing spent) when there are fewer.
    pub fn spend(&mut self, n: u8) -> bool {
        if (n as usize) > self.expiry.len() {
            return false;
        }
        self.expiry.drain(..n as usize);
        true
    }

    /// Drops the charges expired at `now`; returns how many.
    pub fn expire(&mut self, now: f32) -> u8 {
        let before = self.expiry.len();
        self.expiry.retain(|t| *t > now);
        (before - self.expiry.len()) as u8
    }

    pub fn clear(&mut self) {
        self.expiry.clear();
    }

    /// Seconds left of the charge that expires first (the HUD's star blink).
    pub fn next_expiry(&self, now: f32) -> Option<f32> {
        self.expiry.first().map(|t| (t - now).max(0.0))
    }
}
