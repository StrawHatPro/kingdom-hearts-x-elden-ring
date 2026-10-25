//! (DON1) THE STATUSES Donald's magic puts on enemies (documentation/DONALD_OVERHAUL.md 4.2, 4.5, 6.1): Wet, Chill
//! (stacks to Frozen), Frozen, Burn, Revealed (-> Exposed), Blind, and the large-enemy slow that replaces Frozen.
//! One [`TargetStatus`] per enemy in a party-wide [`StatusBook`] (everyone's hits read it: Exposed is "+25 % damage from
//! everyone in the party", and Sora's Thunder on a Wet enemy conducts). Numbers: sheet party-don-core.

use super::tune::DonTune;

/// How big an enemy is (6.1 Freeze: large enemies and bosses don't freeze; 4.2 Aero: launches).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub enum Size {
    #[default]
    Small,
    Medium,
    Large,
    Boss,
}

impl Size {
    /// Never Frozen: a freeze gives `big.chill_stacks` and the `big.slow` instead.
    pub fn freeze_proof(self) -> bool {
        self >= Size::Large
    }
}

/// One status a hit applies (the spell rows' `applies` tokens).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Apply {
    Burn,
    /// Chill stacks.
    Chill(u8),
    Frozen,
    Wet,
    Reveal,
    Blind,
    /// Launched (Aero spells; the missiles count hits through [`TargetStatus::aero_missile_hit`]).
    Launch,
}

impl Apply {
    pub fn parse(token: &str) -> Option<Apply> {
        let t = token.trim();
        Some(match t {
            "burn" => Apply::Burn,
            "frozen" | "freeze" => Apply::Frozen,
            "wet" => Apply::Wet,
            "reveal" => Apply::Reveal,
            "blind" => Apply::Blind,
            "launch" => Apply::Launch,
            _ => {
                let n = t.strip_prefix("chill")?;
                Apply::Chill(n.strip_prefix(':').map_or(Some(1), |n| n.trim().parse::<u8>().ok())?)
            }
        })
    }

    /// A row's `applies` cell ("burn,reveal", "chill:2", "-").
    pub fn parse_list(cell: &str) -> Vec<Apply> {
        cell.split(',').filter_map(Apply::parse).collect()
    }
}

/// The caster's full-power flags that lengthen statuses (4.2 Resonance 3 column).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FullFlags {
    /// Blizzard at full power: Frozen lasts `frozen.full_duration`.
    pub blizzard: bool,
    /// Water at full power: Wet lasts `wet.full_mult` x.
    pub water: bool,
    /// Aero at full power: launches work on medium enemies.
    pub aero: bool,
}

/// What a status change asks the host to show / do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StatusEvent {
    /// Frozen began (seconds).
    Froze(f32),
    /// Frozen ended without a shatter (thawed).
    Thawed,
    /// A large enemy / boss took the slow instead of freezing.
    BigSlowed,
    /// A Burn tick: a magic hit of this power (Sora scale, x the burner's Magic).
    BurnTick(f32),
    /// Launch it (a juggle; the host's launch reaction).
    Launch,
}

/// One enemy's statuses: seconds left of each (0 = none).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct TargetStatus {
    pub size: Size,
    pub wet: f32,
    pub chill: u8,
    pub chill_left: f32,
    pub frozen: f32,
    /// After a Frozen, this long before another (`frozen.rearm`).
    pub frozen_rearm: f32,
    pub big_slow: f32,
    pub burn: f32,
    burn_clock: f32,
    pub reveal: f32,
    /// The Reveal has turned into Exposed (4.5): +25 % damage from everyone until the Reveal ends.
    pub exposed: bool,
    pub blind: f32,
    /// Aero missile hits counted toward a launch (4.2).
    pub launch_hits: u8,
}

impl TargetStatus {
    pub fn new(size: Size) -> TargetStatus {
        TargetStatus { size, ..TargetStatus::default() }
    }

    pub fn is_wet(&self) -> bool {
        self.wet > 0.0
    }

    pub fn is_chilled(&self) -> bool {
        self.chill > 0 && self.chill_left > 0.0
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen > 0.0
    }

    pub fn is_burned(&self) -> bool {
        self.burn > 0.0
    }

    pub fn is_revealed(&self) -> bool {
        self.reveal > 0.0
    }

    pub fn is_blind(&self) -> bool {
        self.blind > 0.0
    }

    /// Nothing on it.
    pub fn is_clear(&self) -> bool {
        !(self.is_wet() || self.is_chilled() || self.is_frozen() || self.is_burned() || self.is_revealed() || self.is_blind() || self.big_slow > 0.0)
    }

    /// Its speed share lost (Chill stacks, or the large-enemy slow; Frozen = 1, it cannot act).
    pub fn slow(&self, t: &DonTune) -> f32 {
        if self.is_frozen() {
            return 1.0;
        }
        let chill = if self.is_chilled() { self.chill as f32 * t.chill_slow_per_stack } else { 0.0 };
        let big = if self.big_slow > 0.0 { t.big_slow } else { 0.0 };
        chill.max(big).clamp(0.0, 0.95)
    }

    /// Damage it takes from anyone in the party (Exposed, 4.5).
    pub fn damage_taken_mult(&self, t: &DonTune) -> f32 {
        if self.exposed && self.is_revealed() {
            t.exposed_mult
        } else {
            1.0
        }
    }

    /// THE FREEZE RULE: Frozen (`frozen.duration`, or `frozen.full_duration` with Blizzard at full power), except a large
    /// enemy / boss (the slow and Chill stacks instead) or one still re-arming (one Chill stack instead).
    pub fn freeze(&mut self, full: FullFlags, t: &DonTune, out: &mut Vec<StatusEvent>) {
        self.freeze_for(if full.blizzard { t.frozen_full_duration } else { t.frozen_duration }, t, out);
    }

    /// The freeze rule with its own length (Freeze Lv 1..3: 3 / 4 / 5 s).
    pub fn freeze_for(&mut self, seconds: f32, t: &DonTune, out: &mut Vec<StatusEvent>) {
        let below = (t.chill_freeze_at as u8).saturating_sub(1).max(1);
        if self.size.freeze_proof() {
            self.chill = self.chill.max(t.big_chill_stacks as u8).min(below);
            self.chill_left = t.chill_duration;
            self.big_slow = t.big_slow_duration;
            out.push(StatusEvent::BigSlowed);
            return;
        }
        if self.frozen_rearm > 0.0 && !self.is_frozen() {
            self.chill = (self.chill + 1).min(below);
            self.chill_left = t.chill_duration;
            return;
        }
        self.frozen = self.frozen.max(seconds);
        self.chill = 0;
        self.chill_left = 0.0;
        out.push(StatusEvent::Froze(self.frozen));
    }

    /// Ends Frozen (Shatter, Steam Burst): the re-arm starts.
    pub fn unfreeze(&mut self, t: &DonTune) {
        if self.is_frozen() {
            self.frozen = 0.0;
            self.frozen_rearm = t.frozen_rearm;
        }
    }

    /// Applies one status.
    pub fn apply(&mut self, a: Apply, full: FullFlags, t: &DonTune, out: &mut Vec<StatusEvent>) {
        match a {
            Apply::Burn => {
                if self.burn <= 0.0 {
                    self.burn_clock = 0.0;
                }
                self.burn = self.burn.max(t.burn_duration);
            }
            Apply::Chill(n) => {
                if self.is_frozen() {
                    return;
                }
                if !self.is_chilled() {
                    self.chill = 0;
                }
                self.chill = self.chill.saturating_add(n.max(1));
                self.chill_left = t.chill_duration;
                if self.chill as f32 >= t.chill_freeze_at {
                    self.chill = 0;
                    self.freeze(full, t, out);
                }
            }
            Apply::Frozen => self.freeze(full, t, out),
            Apply::Wet => self.wet = self.wet.max(t.wet_duration * if full.water { t.wet_full_mult } else { 1.0 }),
            Apply::Reveal => {
                if !self.is_revealed() {
                    self.exposed = false;
                }
                self.reveal = self.reveal.max(t.reveal_duration);
            }
            Apply::Blind => self.blind = self.blind.max(t.blind_duration),
            Apply::Launch => {
                if self.launchable(full.aero) {
                    out.push(StatusEvent::Launch);
                }
            }
        }
    }

    /// Small enemies launch; medium ones too with Aero at full power (4.2).
    pub fn launchable(&self, aero_full: bool) -> bool {
        self.size == Size::Small || (aero_full && self.size == Size::Medium)
    }

    /// An Aero missile landed: true on the `launch.hits`-th hit of a launchable enemy (the count starts over).
    pub fn aero_missile_hit(&mut self, aero_full: bool, t: &DonTune) -> bool {
        if !self.launchable(aero_full) {
            return false;
        }
        self.launch_hits = self.launch_hits.saturating_add(1);
        if self.launch_hits as f32 >= t.launch_hits {
            self.launch_hits = 0;
            return true;
        }
        false
    }

    /// The clock (frame-rate independent: every timer counts real seconds; Burn ticks at its period whatever dt is).
    pub fn tick(&mut self, dt: f32, t: &DonTune, out: &mut Vec<StatusEvent>) {
        let dt = dt.max(0.0);
        let down = |v: &mut f32| *v = (*v - dt).max(0.0);
        if self.is_frozen() {
            down(&mut self.frozen);
            if !self.is_frozen() {
                self.frozen_rearm = t.frozen_rearm;
                out.push(StatusEvent::Thawed);
            }
        } else {
            down(&mut self.frozen_rearm);
        }
        down(&mut self.wet);
        down(&mut self.chill_left);
        if self.chill_left <= 0.0 {
            self.chill = 0;
        }
        down(&mut self.big_slow);
        down(&mut self.blind);
        down(&mut self.reveal);
        if !self.is_revealed() {
            self.exposed = false;
        }
        if self.burn > 0.0 {
            let live = dt.min(self.burn);
            self.burn_clock += live;
            let period = t.burn_tick.max(0.05);
            while self.burn_clock + 1e-4 >= period {
                self.burn_clock -= period;
                out.push(StatusEvent::BurnTick(t.burn_tick_power));
            }
            down(&mut self.burn);
        }
    }

    /// A hit on a Frozen enemy that did not break it: the ice shortens (6.1).
    pub fn chip_ice(&mut self, t: &DonTune) {
        if self.is_frozen() {
            self.frozen = (self.frozen - t.frozen_hit_shorten).max(0.01);
        }
    }
}

/// Every enemy's statuses (party-wide), by the host's target id. Kept in id order (deterministic, no hash map).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StatusBook {
    pub entries: Vec<(u32, TargetStatus)>,
}

impl StatusBook {
    pub fn get(&self, id: u32) -> Option<&TargetStatus> {
        self.entries.binary_search_by_key(&id, |e| e.0).ok().map(|i| &self.entries[i].1)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut TargetStatus> {
        self.entries.binary_search_by_key(&id, |e| e.0).ok().map(|i| &mut self.entries[i].1)
    }

    /// The enemy's statuses, made (with its size) on first use; the size follows the host's latest word.
    pub fn entry(&mut self, id: u32, size: Size) -> &mut TargetStatus {
        let i = match self.entries.binary_search_by_key(&id, |e| e.0) {
            Ok(i) => i,
            Err(i) => {
                self.entries.insert(i, (id, TargetStatus::new(size)));
                i
            }
        };
        let s = &mut self.entries[i].1;
        s.size = size;
        s
    }

    /// Every enemy's clock; the events by enemy. Clear entries are dropped.
    pub fn tick(&mut self, dt: f32, t: &DonTune) -> Vec<(u32, StatusEvent)> {
        let mut out = Vec::new();
        let mut ev = Vec::new();
        for (id, s) in self.entries.iter_mut() {
            ev.clear();
            s.tick(dt, t, &mut ev);
            out.extend(ev.iter().map(|e| (*id, *e)));
        }
        self.entries.retain(|(_, s)| !s.is_clear() || s.frozen_rearm > 0.0 || s.launch_hits > 0);
        out
    }

    /// An enemy gone (dead, unloaded).
    pub fn forget(&mut self, id: u32) {
        self.entries.retain(|e| e.0 != id);
    }

    /// Every Wet enemy (Conduct's chain, Thunderstorm).
    pub fn wet_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.entries.iter().filter(|e| e.1.is_wet()).map(|e| e.0)
    }
}
