//! (ER party S8) UNLOCKING DONALD, GOOFY AND SORA IN ELDEN RING (the user, 2026-10-15, U2: "You must unlock them. Goofy
//! is fighting Leonine Misbegotten (neither does any damage until you enter the boss arena, but he and the boss are
//! actually fighting.) beating the boss will unlock him for your party. Donald is stood in front of the Finger Reader in
//! Limgrave, he is angry at her, playing angry animations. You have to talk to him and he tells you how he got a bogus
//! reading from the finger reader using the KH style speech bubbles. Once you talk to him, he will become unlocked. (Play
//! a special animation for when a character is unlocked.) At the beginning of the game you can choose who you want to
//! start playing as: Sora, Donald or Goofy. If you pick someone other than Sora, then Sora is found fighting Margit (same
//! rules as Goofy fighting his boss before you enter the arena) This skips Margit's intro cutscene and the battle starts
//! as soon as you enter the arena."). Plan: build/sora_tmp/er/party/ER_PARTY_PLAN.md (S8 was added after it); state
//! note build/sora_tmp/er/party/IMPL_S8_STATE.md; sheet sheets/port/party-er-unlocks.json (every Elden Ring id and place
//! parsed from the install).
//!
//! - `rules.rs`: the game-free rules (who is unlocked and its record, the fight / talk phases, the boss puppet, Sora's
//!   stage pilot, the celebration's clock), unit-tested;
//! - `sheet.rs`: the sheet;
//! - `host.rs` (Windows): the game half (flags, the bosses by entity id, the staged friends, the talk window, the
//!   celebration, the testing aids). Everything behind the cargo feature `party` and the key `party.unlocks`.

pub mod rules;
// (ER boss) the real boss's AI switch: the game-free rule (pure, unit-tested)
pub mod aiswitch;
pub mod sheet;
#[cfg(windows)]
mod double;
#[cfg(windows)]
mod host;
#[cfg(windows)]
pub use double::creator_busy;
#[cfg(windows)]
pub use host::*;
#[cfg(all(test, windows))]
mod stage_tests;
