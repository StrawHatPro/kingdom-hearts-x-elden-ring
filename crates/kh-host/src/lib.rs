//! What every host of Sora does the same way, whatever engine it sits in (the Bevy test app
//! `kh-app`, the Elden Ring mod `kh-er`): FIX ONCE, as `kh-look` does for the character's look.
//!
//! - [`seam`]: the queues that implement `kh_core::Audio` and `kh_core::Effects` (the core calls
//!   the seam in the middle of a fixed step; a host acts on the queue after the step);
//! - [`audio`]: the glue between the core's audio seam and `kh_audio::Engine` (where a sound is:
//!   bones and sockets of the step's pose; the per-command rules; one engine step per simulation
//!   step), the device output of `kh_audio::Mixer`, the offline render, and [`audio::HostAudio`],
//!   the whole audio side as one object for a host without an ECS.
//!
//! - [`vfx`]: the effect side between the core's effect seam and `kh_vfx` (what a spawn means,
//!   where each effect sits, one evaluator step per simulation step, the frame's draw list);
//! - [`rig`]: Sora without a game (flat floor, a dummy, scripted pad input, the pose of every
//!   step), for the offline proofs of a host's sound and effects.
//!
//! No Bevy, no game SDK. Sound never feeds back into the simulation.

pub mod audio;
// (boss rewards, IMPL_ER31) which boss of the host game gives Sora what: the table, the once-only record, the grant.
pub mod bossrewards;
// (game over) KH3's game-over sequence: the times, the void scene's pose source, camera and lights as plain data.
pub mod gameover;
// (hud) The game HUD and the camp menu: kh-ui driven from the core, the layer pictures, the menus' pad input.
// (CV1) the conversation runner (steps, choices, directives, locks, camera cues) for every host
pub mod conv;
pub mod hud;
pub mod hud_layers;
// (loot) KH3's item table and the one door through which a host hands Sora an item (prize boxes, rewards).
pub mod items;
pub mod menu;
// (IMPL_ER21) Sora's model in the camp menu's character panel: where the picture goes, the UI actor's data, a posed copy.
pub mod menu_portrait;
// (party, stream F) Donald and Goofy in the HUD and the camp menu: slots, AI settings, the members the menus edit.
pub mod party;
// (party P) the party formation rules (stream P: playable Donald / Goofy, save-point Party Formation)
pub mod formation;
// (prize) the prize orbs of a host: kh-prize stepped with Sora as the getter, drawn as free effects.
pub mod prize;
pub mod rig;
// (save) Sora's save data: the versioned file per host character.
pub mod save;
// (savepoint) KH3's save point as a host places it: its effects, the recover rule, the Save command's reactor.
pub mod savepoint;
/// (er-host, IMPL_ER26) The save-point menu of a host with another game's resting places (entries, order, Pass Time).
pub mod savepoint_menu;
pub mod seam;
// (SHOP) the KH shop that stands in for a host's merchants: area tier, stock, prices, the sell rule, the trades.
pub mod shop;
pub mod vfx;
