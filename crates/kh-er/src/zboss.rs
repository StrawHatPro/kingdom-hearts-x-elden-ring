//! (ZETTA 2026-10-23) A BOSS ZETTAFLARE KILLS GOES QUIETLY. The user: "When you kill a boss with Zettaflare, they should not
//! have any dialogue that plays you just get the rewards. This also means skipping any cutscenes associated with them. Do not
//! let this softlock progression."
//!
//! Generic, no per-boss table, and always through the game's own skip so its state advances exactly as after a player's skip
//! (every flag its scripts set on a line's end or a cutscene's end is set; nothing is blocked):
//!
//! - ITS LAST WORDS: a character Zettaflare killed (its handle, told at the hand-back) speaks no more for
//!   [`SILENCE_S`] s: every talk object whose speaker it is (`talkkh::game::npc_of`, its ambient / death speech and any held
//!   line) has the game's own talk SKIP set on each of its updates (the byte a player's skip sets: the line ends, the
//!   talk script goes on), its subtitle is not posted, no KH bubble is made of it. Lines the game marks unskippable still
//!   end on their own, unseen.
//! - ITS CUTSCENE: a real-time cutscene or movie that begins within [`SKIP_WINDOW_S`] s of a Zettaflare BOSS kill (a
//!   boss bar on it at the hand-back) is skipped with the game's own skip: the pad's Menu (Start) then A pressed for Elden
//!   Ring through our XInput hook (`inputs::skip_cutscene`), as a player skips it; again every [`SKIP_RETRY_S`] s while it
//!   still plays, at most [`SKIP_TRIES`] times.
//! - THE REWARDS are untouched: the game's death gives its runes, items, defeat flag, Great Rune and remembrance; our
//!   boss rewards and the KH BONUS / OBTAINED panels follow its death as ever (bossreward.rs).

use std::sync::Mutex;
use std::time::Instant;

use crate::erlog;

/// How long a character Zettaflare killed stays silent (s).
pub const SILENCE_S: f64 = 120.0;
/// How long after a Zettaflare boss kill its cutscene is skipped (s).
pub const SKIP_WINDOW_S: f64 = 30.0; // (2026-10-24) the moments after the kill only: a cutscene the player starts later (the DLC's ending at its altar) plays
/// A skip press again while the cutscene still plays (s).
pub const SKIP_RETRY_S: f64 = 2.5;
pub const SKIP_TRIES: u32 = 4;
/// The cutscene manager's playing states (sora.rs `CUTSCENE_STATES`, MEASURED in IMPL_ER10).
pub const CUTSCENE_STATES: [i32; 6] = [2, 3, 4, 5, 6, 7];

#[derive(Clone, Copy, Debug)]
struct Killed {
    handle: u64,
    npc_id: i32,
    boss: bool,
    at: f64,
}

#[derive(Default)]
struct State {
    killed: Vec<Killed>,
    /// talk objects checked: (object, silenced)
    ins: Vec<(usize, bool)>,
    skips: u32,
    last_press: f64,
    playing: bool,
    suppressed_lines: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static CLOCK: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn now() -> f64 {
    CLOCK.get_or_init(Instant::now).elapsed().as_secs_f64()
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    let mut g = STATE.lock().ok()?;
    Some(f(g.get_or_insert_with(State::default)))
}

/// The hand-back: Zettaflare kills this character (`boss`: a boss bar on it).
pub fn killed(handle: u64, npc_id: i32, boss: bool) {
    let t = now();
    with(|s| {
        s.killed.retain(|k| k.handle != handle);
        s.killed.push(Killed { handle, npc_id, boss, at: t });
        s.ins.clear();
        if boss {
            s.skips = 0;
        }
    });
    erlog!("zboss: c{npc_id:04} killed by Zettaflare{}: its last words are skipped for {SILENCE_S} s (the game's own talk skip, no subtitle, no bubble){}", if boss { " (A BOSS)" } else { "" }, if boss { format!("; a cutscene within {SKIP_WINDOW_S} s is skipped with the game's own skip") } else { String::new() });
}

/// It lived after all (the game's phase change): it may speak, nothing is skipped for it.
pub fn revoke(handle: u64) {
    with(|s| {
        s.killed.retain(|k| k.handle != handle);
        s.ins.clear();
    });
}

/// Pure: is a speaker silenced (killed `since` s ago)?
pub fn silent(since: Option<f64>) -> bool {
    since.is_some_and(|s| (0.0..SILENCE_S).contains(&s))
}

/// Pure: is a skip press due now? `playing` the cutscene plays, `since_boss_kill` s since the latest Zettaflare boss kill,
/// `tries` presses so far, `since_press` s since the last press.
pub fn skip_due(playing: bool, since_boss_kill: Option<f64>, tries: u32, since_press: f64) -> bool {
    playing && since_boss_kill.is_some_and(|s| (0.0..SKIP_WINDOW_S).contains(&s)) && tries < SKIP_TRIES && (tries == 0 || since_press >= SKIP_RETRY_S)
}

/// Anything recently killed (the talk hook's cheap test before it looks a speaker up).
pub fn any_recent() -> bool {
    let t = now();
    with(|s| s.killed.iter().any(|k| silent(Some(t - k.at)))).unwrap_or(false)
}

/// The talk object `ins`: silenced? (`speaker` looks its character's handle up, asked once per object).
pub fn ins_silenced(ins: usize, speaker: impl FnOnce() -> Option<u64>) -> bool {
    let t = now();
    let known = with(|s| s.ins.iter().find(|i| i.0 == ins).map(|i| i.1)).flatten();
    if let Some(v) = known {
        return v;
    }
    let who = speaker();
    with(|s| {
        let hit = who.and_then(|h| s.killed.iter().find(|k| k.handle == h && silent(Some(t - k.at))).copied());
        s.ins.retain(|i| i.0 != ins);
        s.ins.push((ins, hit.is_some()));
        if s.ins.len() > 64 {
            s.ins.remove(0);
        }
        if let Some(k) = hit {
            s.suppressed_lines += 1;
            erlog!("zboss: c{:04}'s talk (object {ins:#x}) SILENCED: Zettaflare killed it {:.1} s ago: its lines are skipped with the game's own skip, no subtitle, no bubble ({} talk object(s) so far)", k.npc_id, t - k.at, s.suppressed_lines);
        }
        hit.is_some()
    })
    .unwrap_or(false)
}

/// Is this talk object known silenced (no lookup)?
pub fn ins_known_silenced(ins: usize) -> bool {
    with(|s| s.ins.iter().any(|i| i.0 == ins && i.1)).unwrap_or(false)
}

/// Once a frame: the cutscene after a Zettaflare boss kill skipped with the game's own skip. `remo`: the cutscene
/// manager's state.
#[cfg(windows)]
pub fn frame(remo: Option<i32>) {
    let t = now();
    let playing = remo.is_some_and(|s| CUTSCENE_STATES.contains(&s));
    let press = with(|s| {
        let since = s.killed.iter().filter(|k| k.boss).map(|k| t - k.at).reduce(f64::min);
        if playing && !s.playing {
            if let Some(k) = since {
                if k < SKIP_WINDOW_S {
                    erlog!("zboss: a cutscene begins {k:.1} s after a Zettaflare boss kill: it is SKIPPED with the game's own skip (Menu, then A, as a player)");
                }
            }
        }
        s.playing = playing;
        if skip_due(playing, since, s.skips, t - s.last_press) {
            s.skips += 1;
            s.last_press = t;
            Some(s.skips)
        } else {
            None
        }
    })
    .flatten();
    if let Some(n) = press {
        let pressed = crate::inputs::skip_cutscene();
        erlog!("zboss: the cutscene's skip pressed (try {n} of {SKIP_TRIES}): {}", if pressed { "Menu, then A, for Elden Ring" } else { "NOT pressed (no pad Elden Ring reads, or a script of ours runs): the cutscene plays" });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (2026-10-23, the user: "they should not have any dialogue that plays you just get the rewards") its last words are
    /// silenced for a while after Zettaflare's kill, never before it, never forever
    #[test]
    fn zboss_a_zettaflare_kill_is_silent_for_a_while() {
        assert!(silent(Some(0.0)) && silent(Some(10.0)), "its death speech (10 s after: Margit's)");
        assert!(!silent(Some(SILENCE_S + 1.0)) && !silent(None) && !silent(Some(-1.0)));
    }

    /// (2026-10-23, "This also means skipping any cutscenes associated with them. Do not let this softlock progression.")
    /// a cutscene after a Zettaflare boss kill gets the game's own skip, pressed again while it still plays, a few times at
    /// most; none for a cutscene long after, none without a boss kill
    #[test]
    fn zboss_its_cutscene_is_skipped_the_games_way() {
        assert!(skip_due(true, Some(12.0), 0, 0.0), "the first press at once");
        assert!(!skip_due(true, Some(12.0), 1, 1.0), "not before the retry");
        assert!(skip_due(true, Some(15.0), 1, SKIP_RETRY_S), "again while it plays");
        assert!(!skip_due(true, Some(15.0), SKIP_TRIES, 99.0), "a few times at most");
        assert!(!skip_due(false, Some(1.0), 0, 0.0), "nothing plays: nothing pressed");
        assert!(!skip_due(true, None, 0, 0.0), "no Zettaflare boss kill: the game's cutscene plays");
        assert!(!skip_due(true, Some(SKIP_WINDOW_S + 1.0), 0, 0.0), "long after: plays");
    }

    /// the talk lookup: a speaker Zettaflare killed is silenced, another speaks; asked once per talk object
    #[test]
    fn zboss_only_its_own_talk_is_silenced() {
        killed(0xdead_0001, 2130, true);
        assert!(ins_silenced(0x1000, || Some(0xdead_0001)));
        assert!(ins_known_silenced(0x1000));
        assert!(!ins_silenced(0x2000, || Some(0x0bad_0002)), "another character speaks");
        let mut asked = false;
        assert!(ins_silenced(0x1000, || { asked = true; None }) && !asked, "asked once per object");
        revoke(0xdead_0001);
        assert!(!ins_silenced(0x1000, || Some(0xdead_0001)), "a phase change: it may speak");
    }
}
