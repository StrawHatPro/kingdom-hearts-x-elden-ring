//! (THE USER 2026-11-02: "make non-boss enemies bounce off of Goofy's shield if they're not doing a long combo string when Goofy
//! successfully performs a perfect deflect") AN ENEMY'S SWING KNOCKED BACK OFF GOOFY'S SHIELD: Elden Ring's own attack rebound
//! on the real character, asked through `StaggerAsk::Rebound` (`ErTargets::rebound`, targets.rs). Whether it is asked at all
//! (a perfect deflect, no boss, not mid-string) is kh-party's (goofy/parry.rs `bounces`, seams.rs `on_bounce`).
//!
//! WHERE ELDEN RING KEEPS AN ENEMY'S REBOUND (read from the game's own files with kh-erfile, 2026-11-03): every enemy's behaviour binder (`/chr/cNNNN.behbnd.dcx`, all 403 of them) holds the common enemy graph `c9997.hkx`, and
//! that graph's damage states hold `AttackBoundEnemy1` / `AttackBoundEnemy2` (events `W_AttackBoundEnemy1` / `2`), beside
//! `GuardBreak`, `DamageParryEnemy1/2` and `SABreak` (whose `W_SABreak` the Breaker's kneel fires by name: stance.rs). "Attack
//! bound" is the attack knocked back: the state the game plays when an NPC's swing is repelled (AtkParam_Npc `guardBreakRate`,
//! "the value that decides whether the attack is repelled", against the guard's repel power). The player's own graph (c0000,
//! which human NPCs use too) names it `W_Repelled_Large` / `W_Repelled_Small` instead; no enemy graph has those (the run of
//! 2026-11-03 fired them at c4311 / c4371 and nothing answered: build/sora_tmp/er/batchtest/logs).
//!
//! THE LADDER ([`frame`]; one step every `wait` seconds, row `deflect.bounce_wait`, until the swing has ended):
//! 1. `W_AttackBoundEnemy1` (the enemy graph's rebound) with `W_Repelled_Large` (the human graph's): each graph knows one of
//!    the two names and ignores the other (fired by name with `chranim::fire`);
//! 2. `W_AttackBoundEnemy2` with `W_Repelled_Small`: the other rebound of each graph, for a body whose first one has no clip;
//! 3. none answered: a flinch bullet (dmgLevel 2, the "stagger, no travel" of IMPL_ER6's survey; no poise damage, no armour
//!    broken), the same ask the puppets use for a hurt animation. An armoured body swings on: nothing past its own limits.
//!
//! ANSWERED: the attack animation it was in (x3000..x3999, `poise::attack_anim`) gave way to something that is no attack. A
//! swing that chains into its next attack meanwhile is the one the ladder goes on against. The log says which step answered
//! and what played.
//!
//! RULES: never on a body barred from the kit's tricks (`ErTargets::stagger_barred`: an event, a cutscene, a no-death state,
//! Zettaflare), a boss (the game's boss bar), a body whose stance break runs (the kneel is the bigger reaction) or one a KH3
//! reaction holds in the air; only on a body in an attack animation (a swing to knock back). One ladder per body; a body gone
//! is forgotten.

use std::sync::Mutex;

use eldenring::cs::FieldInsHandle;
use kh_core::TargetId;

use crate::erlog;
use crate::targets::ErTargets;

/// The game's rebound events, tried in this order: each step the enemy graph's (c9997) and the human graph's (c0000) name.
const EVENTS: [[&str; 2]; 2] = [["W_AttackBoundEnemy1", "W_Repelled_Large"], ["W_AttackBoundEnemy2", "W_Repelled_Small"]];
/// The last step's flinch bullet: dmgLevel 2 (Middle: IMPL_ER6, the Godrick soldier 8040, the wolf 8046, the noble 8040).
const FLINCH_LEVEL: u8 = 2;

/// What a frame does with a ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Do {
    /// Fire these behaviour events (the enemy graph's name, the human graph's).
    Fire([&'static str; 2]),
    /// Ask the flinch bullet.
    Flinch,
    Wait,
    /// The swing ended: this plays now.
    Answered(i32),
    /// Nothing answered.
    GiveUp,
}

/// One body's ladder.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ladder {
    /// The attack animation it plays (the one to end).
    attack: i32,
    t: f32,
    /// Steps taken (0: none yet), and when the next one is due.
    step: u8,
    next: f32,
    wait: f32,
}

impl Ladder {
    fn new(attack: i32, wait: f32) -> Ladder {
        Ladder { attack, t: 0.0, step: 0, next: 0.0, wait: wait.max(1.0 / 60.0) }
    }

    /// One frame against the animation it plays `now`.
    fn step(&mut self, now: i32, dt: f32) -> Do {
        self.t += dt;
        if self.step > 0 && now != self.attack {
            if !crate::poise::attack_anim(now) {
                return Do::Answered(now);
            }
            self.attack = now; // its next swing of a chain: the ladder goes on against that one
        }
        if self.step > 0 && self.t < self.next {
            return Do::Wait;
        }
        self.step = self.step.saturating_add(1);
        self.next = self.t + self.wait;
        match EVENTS.get(self.step as usize - 1) {
            Some(names) => Do::Fire(*names),
            None if self.step as usize == EVENTS.len() + 1 => Do::Flinch,
            None => Do::GiveUp,
        }
    }
}

struct Bounce {
    id: TargetId,
    handle: FieldInsHandle,
    ladder: Ladder,
}

static BOUNCES: Mutex<Vec<Bounce>> = Mutex::new(Vec::new());

/// Asks `handle`'s swing to rebound, each step waiting `wait` seconds. False: nothing will happen (no body, or no swing running).
/// The caller has checked the body may be touched (`ErTargets::rebound`). A body already bouncing keeps its ladder (true).
pub fn begin(id: TargetId, handle: FieldInsHandle, wait: f32) -> bool {
    let mut list = BOUNCES.lock().unwrap_or_else(|e| e.into_inner());
    if list.iter().any(|b| b.handle == handle) {
        return true;
    }
    let Some(chr) = crate::targets::find_chr(&handle) else { return false };
    let now = crate::chranim::now(chr);
    if !crate::poise::attack_anim(now.id) {
        erlog!("rebound: target {}: no rebound: no swing running (it plays {})", id.0, now.id);
        return false;
    }
    erlog!("rebound: target {}: its swing {} ({:.2} of {:.2} s) bounces off Goofy's shield", id.0, now.id, now.time, now.length);
    list.push(Bounce { id, handle, ladder: Ladder::new(now.id, wait) });
    true
}

/// (THE USER 2026-11-02) A stance-break phase that owns the body: no rebound over it.
pub fn stance_holds(phase: crate::stance::Phase) -> bool {
    matches!(phase, crate::stance::Phase::Asking | crate::stance::Phase::Down { .. } | crate::stance::Phase::Frozen { .. })
}

/// Once a frame (beside `stance::frame`).
pub fn frame(targets: &mut ErTargets, dt: f32) {
    let mut list = BOUNCES.lock().unwrap_or_else(|e| e.into_inner());
    if list.is_empty() {
        return;
    }
    list.retain_mut(|b| {
        let Some(chr) = crate::targets::find_chr(&b.handle) else {
            erlog!("rebound: target {}: the body is gone: forgotten", b.id.0);
            return false;
        };
        if chr.modules.data.hp <= 0 || targets.stagger_barred(b.id).is_some() {
            return false;
        }
        let now = crate::chranim::now(chr);
        match b.ladder.step(now.id, dt) {
            Do::Wait => true,
            Do::Fire(names) => {
                let asked = names.map(|name| crate::chranim::fire(chr, name));
                erlog!("rebound: target {} (c{:04}): step {}: {} ({}) / {} ({}) asked in its swing {} at {:.2} s", b.id.0, chr.npc_id, b.ladder.step, names[0], asked[0], names[1], asked[1], now.id, now.time);
                true
            }
            Do::Flinch => {
                targets.ask_animation_with(b.id, FLINCH_LEVEL, 0.0);
                erlog!("rebound: target {}: step {}: no rebound event answered: a flinch bullet (dmgLevel {FLINCH_LEVEL}) asked", b.id.0, b.ladder.step);
                true
            }
            Do::Answered(anim) => {
                erlog!("rebound: target {}: ANSWERED by step {}: the swing ended {:.2} s after the ask, it plays {anim} ({:.2} s)", b.id.0, b.ladder.step, b.ladder.t, now.length);
                false
            }
            Do::GiveUp => {
                erlog!("rebound: target {}: nothing answered in {:.2} s: its swing {} plays on", b.id.0, b.ladder.t, now.id);
                false
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;
    const SWING: i32 = 3_003_001;

    /// What a ladder does frame by frame (the waits left out) against what the body plays, up to its end (`frame` drops it then).
    fn run(mut now: impl FnMut(u32) -> i32, frames: u32) -> Vec<(u32, Do)> {
        let mut l = Ladder::new(SWING, 0.2);
        let mut out = Vec::new();
        for f in 0..frames {
            let d = l.step(now(f), DT);
            if d != Do::Wait {
                out.push((f, d));
            }
            if matches!(d, Do::Answered(_) | Do::GiveUp) {
                break;
            }
        }
        out
    }

    #[test]
    fn the_ladder_tries_the_games_rebounds_then_a_flinch_then_gives_up() {
        let got = run(|_| SWING, 120);
        let does: Vec<Do> = got.iter().map(|g| g.1).collect();
        assert_eq!(does, vec![Do::Fire(EVENTS[0]), Do::Fire(EVENTS[1]), Do::Flinch, Do::GiveUp]);
        assert_eq!(EVENTS[0], ["W_AttackBoundEnemy1", "W_Repelled_Large"], "the enemy graph's rebound first, the human graph's beside it");
        assert_eq!(got[0].0, 0, "the first at once");
        assert!(got.windows(2).all(|w| (12..=13).contains(&(w[1].0 - w[0].0))), "one step every wait (0.2 s = 12 frames): {got:?}");
    }

    #[test]
    fn a_swing_that_ends_for_something_that_is_no_attack_is_answered() {
        // the rebound plays from the 5th frame (say 3002200 of its set, any id that is no x3000..x3999 attack)
        let got = run(|f| if f < 5 { SWING } else { 3_002_200 }, 60);
        assert_eq!(got, vec![(0, Do::Fire(EVENTS[0])), (5, Do::Answered(3_002_200))]);
        // a stagger or idle answers as well: the attack ended
        let got = run(|f| if f < 20 { SWING } else { 3_008_040 }, 60);
        assert_eq!(got.last(), Some(&(20, Do::Answered(3_008_040))));
    }

    #[test]
    fn a_swing_that_chains_into_its_next_attack_is_still_to_be_knocked_back() {
        let got = run(|f| if f < 3 { SWING } else { 3_003_002 }, 30);
        assert_eq!(got.iter().map(|g| g.1).collect::<Vec<_>>(), vec![Do::Fire(EVENTS[0]), Do::Fire(EVENTS[1]), Do::Flinch], "the chained swing is no answer");
    }

    #[test]
    fn only_a_running_stance_break_holds_the_body() {
        use crate::stance::Phase;
        assert!(stance_holds(Phase::Asking) && stance_holds(Phase::Down { step: 1, left: 1.0 }) && stance_holds(Phase::Frozen { left: 1.0 }));
        assert!(!stance_holds(Phase::Over) && !stance_holds(Phase::Refused));
    }
}
