//! (AUDIT-A WORLDTIME) KH3'S WORLD TIME IN ELDEN RING: what Sora's formchanges and their finishers slow, applied to the
//! game generically (every Elden Ring enemy, no per-enemy table). The rule is `kh_host::worldtime` (its module text has the
//! trace: WorldSlow 0x14140E500 -> FUN_1405DA460(pawn, 1, 0x419C, 0.1) / FUN_1405DA3E0(pawn, 1, 0); Counter Shield's
//! tate_finish `SetGlobalGameSpeed` hit-stops); this module is the host's side of it.
//!
//! - **The global game speed** (the hit-stops; KH3 slows EVERYTHING, Sora included): one fixed step of the host is
//!   `1/60 x global` game seconds for Sora, his party, his effects, his prizes, the puppets and our cameras (sora.rs: the
//!   same `wdt` route as the boss finish), and Elden Ring's own clock (`CSFlipper::game_speed`, finish.rs: the one owner of
//!   that field, the forms' channel combined with the boss finish's as KH3's game state combines its channels).
//! - **WorldSlow** (the per-type table; ENEMY / GIMMICK / MAP / BG / PHYSX at 0.1, NOT Sora, NOT his party, NOT the camera):
//!   every character of the target list that is an enemy (`hostile`, not a summon, not a friendly NPC: KH3's ENEMY type)
//!   has its animation speed (`CSChrBehaviorModule::animation_speed`, chranim.rs: the route the puppets / stance / Zettaflare
//!   already use; root motion, TAE attack windows and the bullets a TAE spawns follow it) multiplied by the table's ENEMY
//!   rate, re-based whenever somebody else wrote it (a puppet's hold, a stance break) and given back exactly at the end
//!   (only where nobody wrote it since). The KH reactions the puppets step (launches, juggles) run at `global x [ENEMY]`.
//!   (WORLDTIME-2) THAT ANIMATION ROUTE IS NOW THE FALLBACK ONLY: chrtime.rs found Elden Ring's own per-character time
//!   (the hit-stop module's delta = `ChrIns + 0xB0`, the delta of every phase of the character, its AI included), the
//!   bullets' own delta, the effects' play-speed rate and the map clock's passage, and slows them all at KH3's rate.
//!   While its character detour is in, the enemies' animation speed is left alone (their delta already carries it).
//! - A net: the forms' global speed below 1 for longer than [`NET_S`] real seconds (a blueprint end that never came) puts
//!   it back, loudly. `world.kh = 0` (kh-er.ini): nothing of this is applied (the boss finish keeps its own route).

use std::collections::HashMap;

use eldenring::cs::FieldInsHandle;
use kh_host::worldtime::{WorldClock, WorldRates};

use crate::control::Control;
use crate::erlog;
use crate::targets::{find_chr, Enemy, ErTargets};

/// The forms' hit-stops are a fraction of a second (B418's ~0.4 s, B426's 0.05 s real): longer than this is a lost end.
pub const NET_S: f32 = 3.0;

/// A body whose animation speed we scale: the speed it had (`base`) and what we wrote over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Held {
    pub base: f32,
    pub wrote: f32,
}

/// The hold of a body that plays at `current` now, for the enemies' rate `rate` (`held` = last frame's). A speed that is
/// not what we wrote is somebody else's new base. None: nothing to hold (rate 1).
pub fn scale(held: Option<Held>, current: f32, rate: f32) -> Option<Held> {
    if rate >= 1.0 {
        return None;
    }
    let base = match held {
        Some(h) if h.wrote == current => h.base,
        _ => current,
    };
    Some(Held { base, wrote: (base * rate).clamp(0.0, 8.0) })
}

/// What to put back when a hold ends: its base, unless somebody else wrote the speed since (theirs stays).
pub fn give_back(held: Held, current: f32) -> Option<f32> {
    (held.wrote == current).then_some(held.base)
}

/// KH3's ENEMY type in Elden Ring: a live hostile character that is not a summon or a friendly NPC.
pub fn slowed(e: &Enemy) -> bool {
    e.alive && e.hostile && !e.friendly && !e.summon
}

#[derive(Default)]
pub struct ErWorldTime {
    clock: WorldClock,
    held: HashMap<u64, (FieldInsHandle, Held)>,
    low: f32,
    off: bool,
    /// (WORLDTIME-2) Elden Ring's own clocks: every enemy's delta, its bullets, the effects, the map clock (chrtime.rs).
    clocks: crate::chrtime::ErClocks,
}

impl ErWorldTime {
    /// One fixed step (sora.rs, right after the boss finish's step): `finish` = the finish's world rate. The table is read
    /// as Sora's last step left it (KH3: the game state applies a SetTypeRates at its next tick).
    pub fn step(&mut self, real_dt: f32, finish: f32, sora: &kh_core::Sora) -> WorldRates {
        if self.off {
            return WorldRates { global: finish, ..WorldRates::NORMAL };
        }
        let rates = self.clock.step(real_dt, finish, |t| sora.time_dilation(t));
        if self.clock.form_rate() < 1.0 {
            self.low += real_dt;
            if self.low > NET_S {
                erlog!("!!! worldtime: THE FORMS' GLOBAL GAME SPEED STAYED AT x{:.3} FOR {NET_S} s (a blueprint's end never came): put back to 1", self.clock.form_rate());
                self.clock.clear();
                self.low = 0.0;
            }
        } else {
            self.low = 0.0;
        }
        for line in self.clock.take_log() {
            erlog!("worldtime: {line}");
        }
        rates
    }

    /// The camera layer's request of this step (cam.rs `take_form_game_speed`: tate_finish's SetGlobalGameSpeed).
    pub fn request(&mut self, request: Option<(f32, f32)>) {
        if !self.off {
            self.clock.request(request);
        }
    }

    /// The forms' own global speed (for Elden Ring's game speed, finish.rs `form_rate`).
    pub fn form_rate(&self) -> f32 {
        if self.off {
            1.0
        } else {
            self.clock.form_rate()
        }
    }

    /// ONCE A FRAME after the steps: the enemies' animation speed at the table's ENEMY rate; holds given back at its end.
    pub fn post(&mut self, targets: &ErTargets, control: &Control) {
        let off = control.flag("world.kh") == Some(false);
        if off != self.off {
            erlog!("worldtime: KH3's world time (WorldSlow, the hit-stops) is {}", if off { "OFF (world.kh = 0)" } else { "on" });
            if off {
                self.stop();
            }
            self.off = off;
        }
        let rates = if self.off { WorldRates::NORMAL } else { self.clock.rates() };
        self.clocks.always = control.f32("world.proof").is_some_and(|v| v >= 1.0); // (WORLDTIME-2) the proof lines without a slow (testing aid)
        crate::fxtime::TRACE.store(control.f32("world.proof") == Some(2.0), std::sync::atomic::Ordering::Relaxed); // (WORLDTIME-2 fx) where unowned effects are made
        self.clocks.test_shot = control.str("world.test_shot").map(str::to_string); // (WORLDTIME-2) an enemy's test bullet (testing aid)
        self.clocks.test_fx = control.str("world.test_fx").map(str::to_string); // (WORLDTIME-2 fx) one special effect on two owners (testing aid)
        self.clocks.frame(&rates, targets); // (WORLDTIME-2) the enemies' own delta, their bullets, the effects, the map clock
        // (WORLDTIME-2) the animation-speed route only where the characters' own delta could not be detoured (else it
        // would slow them twice: their delta already carries the animation)
        let rate = if crate::chrtime::chr_hooked() { 1.0 } else { rates.enemy_type };
        let mut seen = Vec::new();
        if rate < 1.0 {
            for e in targets.list.iter().filter(|e| slowed(e)) {
                let Some(chr) = targets.chr_mut(e.id) else { continue };
                let bits = crate::game::handle_bits(&e.handle);
                let current = crate::chranim::speed(chr);
                if let Some(h) = scale(self.held.get(&bits).map(|x| x.1), current, rate) {
                    crate::chranim::set_speed(chr, h.wrote);
                    if !self.held.contains_key(&bits) {
                        erlog!("worldtime: target {} (c{}) slowed with the world: animation x{:.3} (was {current:.3})", e.id.0, e.chr_type, h.wrote);
                    }
                    self.held.insert(bits, (e.handle, h));
                    seen.push(bits);
                }
            }
        }
        let ended: Vec<u64> = self.held.keys().filter(|b| !seen.contains(b)).copied().collect();
        for bits in ended {
            if let Some((handle, h)) = self.held.remove(&bits) {
                Self::release(&handle, h);
            }
        }
    }

    fn release(handle: &FieldInsHandle, h: Held) {
        if let Some(chr) = find_chr(handle) {
            if let Some(base) = give_back(h, crate::chranim::speed(chr)) {
                crate::chranim::set_speed(chr, base);
            }
        }
    }

    /// Everything back now (Sora stops, a load, `world.kh = 0`).
    pub fn stop(&mut self) {
        for (_, (handle, h)) in self.held.drain() {
            Self::release(&handle, h);
        }
        self.clocks.stop(); // (WORLDTIME-2)
        self.clock.clear();
        self.low = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_is_slowed_by_the_rate_and_given_back_exactly() {
        let h = scale(None, 1.0, 0.1).expect("held");
        assert_eq!(h, Held { base: 1.0, wrote: 0.1 });
        // the next frame it still plays what we wrote: the same hold
        assert_eq!(scale(Some(h), 0.1, 0.1), Some(h));
        // the end: back to its base
        assert_eq!(give_back(h, 0.1), Some(1.0));
        assert_eq!(scale(Some(h), 0.1, 1.0), None);
    }

    #[test]
    fn somebody_elses_write_is_the_new_base() {
        let h = scale(None, 1.0, 0.1).expect("held");
        // a puppet froze it (0) / a stance break slowed its kneel (0.5) while the world was slow
        let frozen = scale(Some(h), 0.0, 0.1).expect("held");
        assert_eq!(frozen, Held { base: 0.0, wrote: 0.0 });
        let kneel = scale(Some(frozen), 0.5, 0.1).expect("held");
        assert_eq!(kneel, Held { base: 0.5, wrote: 0.05 });
        // given back to the kneel's 0.5; a speed somebody wrote after our last write is theirs
        assert_eq!(give_back(kneel, 0.05), Some(0.5));
        assert_eq!(give_back(kneel, 1.0), None);
        // Zettaflare's fast-forward (up to 32) stays within what chranim::set_speed writes
        assert_eq!(scale(None, 32.0, 0.5).map(|h| h.wrote), Some(8.0));
    }

    #[test]
    fn the_hit_stop_and_the_boss_finish_share_one_clock() {
        // finish.rs `post`: the forms' channel and the finish's combine as KH3's channels (the lowest below 1)
        assert!((kh_host::worldtime::combine(0.1, 0.05) - 0.05).abs() < 1e-7);
        let mut c = WorldClock::default();
        c.request(Some((0.1, 0.0)));
        let r = c.step(1.0 / 60.0, 1.0, |t| if t == kh_host::worldtime::OBJ_ENEMY { 0.1 } else { 1.0 });
        assert!((r.global - 0.1).abs() < 1e-7 && (r.enemy() - 0.01).abs() < 1e-7, "a grown attack's hit-stop: the enemies at x0.01");
        assert_eq!((r.player_type, r.friend_type), (1.0, 1.0));
    }
}
