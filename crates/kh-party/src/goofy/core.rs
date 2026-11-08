//! (GOOFY) GOOFY'S COMBAT STATE (documentation/GOOFY_OVERHAUL.md 4, 6, 7): the tap clock and the mash, Resolve, deflect strings,
//! the evasion book. Engine-free, on `FriendStatus` (an AI Goofy has it too). (THE USER 2026-11-02: "Remove countering with 'A'
//! after a parry technique") no counter window, no counter chain and no free Art after a counter any more.

use super::parry::{StickRel, TapFacts};
use super::seams::GoofyEvent;
use super::tune::K;
use super::Telegraph;
use kh_core::host::{AttackClass, TargetId};

/// RESOLVE (7): three pips, kept in thirds. It fills only from clean play and empties when he is hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Resolve {
    pub thirds: u8,
    /// Zone Extender spent its one hit in this Zone.
    pub zone_shield_used: bool,
}

/// A full meter.
pub const RESOLVE_FULL: u8 = 9;

impl Resolve {
    pub const fn pips(self) -> u8 {
        self.thirds / 3
    }

    pub const fn in_zone(self) -> bool {
        self.thirds >= RESOLVE_FULL
    }

    pub fn gain(&mut self, thirds: u8) {
        self.thirds = (self.thirds + thirds).min(RESOLVE_FULL);
    }

    /// He got hit. Resolve Keeper: one pip only. Zone Extender: In the Zone lasts through one hit.
    pub fn on_hit(&mut self, keeper: bool, extender: bool) {
        if self.in_zone() && extender && !self.zone_shield_used {
            self.zone_shield_used = true;
            return;
        }
        self.thirds = if keeper { self.thirds.saturating_sub(3) } else { 0 };
        self.zone_shield_used = false;
    }
}

/// One enemy attack whose hit window is open, watched for Hop and Slip (R5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Watch {
    pub attacker: TargetId,
    pub class: AttackClass,
    pub perilous: bool,
    /// The longest he had been off the ground during the window (seconds).
    pub air: f32,
    /// A dodge hop overlapped it, or ended just before.
    pub hop: bool,
    /// A hit landed from it.
    pub hit: bool,
    /// The attacker stood close enough for it to be meant for him.
    pub near: bool,
    pub seen: bool,
}

/// The state.
#[derive(Clone, Debug, PartialEq)]
pub struct GoofyCore {
    /// Seconds since the last accepted X press.
    pub tap_age: f32,
    pub mash: u8,
    /// The stick at that press: world yaw, pushed, and its reading against the enemy of that moment (for a hit with no
    /// known attacker).
    pub tap_yaw: f32,
    pub tap_active: bool,
    pub tap_rel: StickRel,
    /// X held now; the last finished hold; seconds since it ended; the two as the last press found them.
    pub hold: f32,
    pub last_hold: f32,
    pub since_release: f32,
    pub prev_hold: f32,
    pub prev_gap: f32,
    pub resolve: Resolve,
    /// Deflects in a row, seconds since the last one.
    pub string: u8,
    pub string_age: f32,
    /// (THE USER 2026-11-02, the bounce) Whose blow the last deflect of the string answered (None: the host could not say).
    pub string_from: Option<TargetId>,
    pub second_deflect_used: bool,
    /// Revenge Bash stands ready.
    pub revenge: bool,
    pub hop_cd: f32,
    /// Seconds since the last hop's invincibility ended.
    pub since_hop: f32,
    /// Seconds he has been off the ground now (0 on it).
    pub air_for: f32,
    /// (GF-INGAME) A grab closes on a hop that counts for Slip Counter: the host's body refuses it too.
    pub slip_open: bool,
    /// Second Wind may fire (HP has been at or above the line).
    pub second_wind_ready: bool,
    /// Ally Guard's cooldown.
    pub ally_cd: f32,
    pub watches: Vec<Watch>,
    /// What happened this step (drained by `hooks::step`).
    pub events: Vec<GoofyEvent>,
}

impl Default for GoofyCore {
    fn default() -> GoofyCore {
        GoofyCore { tap_age: 99.0, mash: 0, tap_yaw: 0.0, tap_active: false, tap_rel: StickRel::Neutral, hold: 0.0, last_hold: 0.0, since_release: 99.0, prev_hold: 0.0, prev_gap: 99.0, resolve: Resolve::default(), string: 0, string_age: 99.0, string_from: None, second_deflect_used: false, revenge: false, hop_cd: 0.0, since_hop: 99.0, air_for: 0.0, slip_open: false, second_wind_ready: true, ally_cd: 0.0, watches: Vec::new(), events: Vec::new() }
    }
}

impl GoofyCore {
    /// An accepted X press: the tap clock, the mash.
    pub fn press(&mut self, yaw: f32, active: bool, rel: StickRel) {
        self.mash = if self.tap_age < K.deflect_mash_gap { self.mash.saturating_add(1) } else { 0 };
        self.tap_age = 0.0;
        (self.tap_yaw, self.tap_active, self.tap_rel) = (yaw, active, rel);
        (self.prev_hold, self.prev_gap) = (self.last_hold, self.since_release);
    }

    /// X this step (held or not).
    pub fn hold_step(&mut self, held: bool, dt: f32) {
        if held {
            self.hold += dt;
        } else if self.hold > 0.0 {
            self.last_hold = self.hold;
            self.hold = 0.0;
            self.since_release = 0.0;
        } else {
            self.since_release += dt;
        }
    }

    /// The tap clock as a hit reads it.
    pub fn tap(&self) -> TapFacts {
        TapFacts { age: self.tap_age, mash: self.mash, prev_hold: self.prev_hold, prev_gap: self.prev_gap, in_string: self.in_string() }
    }

    /// A deflect string runs.
    pub fn in_string(&self) -> bool {
        self.string > 0 && self.string_age <= K.rhythm_gap
    }

    /// (THE USER 2026-11-02: "...if they're not doing a long combo string...") `attacker` is in the middle of a string he is
    /// deflecting: Rhythm Deflect's string runs ([`Self::in_string`]) and its last deflect answered this attacker's blow (or one
    /// the host could not name: then nobody is known to be fresh). Read before this blow's own deflect joins the string.
    pub fn mid_string(&self, attacker: Option<TargetId>) -> bool {
        self.in_string() && (self.string_from.is_none() || attacker.is_none() || self.string_from == attacker)
    }

    /// One fixed step of the clocks.
    pub fn tick(&mut self, dt: f32) {
        self.tap_age += dt;
        self.string_age += dt;
        self.since_hop += dt;
        self.hop_cd = (self.hop_cd - dt).max(0.0);
        self.ally_cd = (self.ally_cd - dt).max(0.0);
        if self.string_age > K.rhythm_gap {
            self.string = 0;
            self.second_deflect_used = false;
        }
    }

    /// A deflect joined the string; returns the deflects in a row.
    pub fn string_hit(&mut self) -> u8 {
        self.string = if self.in_string() { self.string.saturating_add(1) } else { 1 };
        self.string_age = 0.0;
        self.string
    }

    /// He took a hit: the string goes.
    pub fn on_hit_taken(&mut self) {
        self.string = 0;
    }

    /// One step of the evasion book against the host's telegraph list. `near` says whether an attacker stands close enough
    /// to be meant for him. Returns the watches whose hit window ended.
    pub fn watch_step(&mut self, list: &[Telegraph], air_for: f32, hopping: bool, near: &dyn Fn(TargetId) -> bool) -> Vec<Watch> {
        for w in self.watches.iter_mut() {
            w.seen = false;
        }
        for t in list.iter().filter(|t| t.active) {
            let i = match self.watches.iter().position(|w| w.attacker == t.attacker && w.class == t.class) {
                Some(i) => i,
                None => {
                    self.watches.push(Watch { attacker: t.attacker, class: t.class, perilous: t.perilous, air: 0.0, hop: false, hit: false, near: false, seen: false });
                    self.watches.len() - 1
                }
            };
            let w = &mut self.watches[i];
            w.seen = true;
            w.near |= near(t.attacker);
            // (MEASURED: real hit windows run 0.06 - 0.2 s) his time in the air, not the part of it inside the window
            w.air = w.air.max(air_for);
            w.hop |= hopping;
        }
        let mut ended = Vec::new();
        self.watches.retain(|w| {
            if !w.seen {
                ended.push(*w);
            }
            w.seen
        });
        ended
    }

    /// A hit reached him from `attacker` (None: anyone): the attacks it came from count as landed.
    pub fn mark_hit(&mut self, attacker: Option<TargetId>) {
        for w in self.watches.iter_mut().filter(|w| attacker.is_none_or(|a| a == w.attacker)) {
            w.hit = true;
        }
    }
}
