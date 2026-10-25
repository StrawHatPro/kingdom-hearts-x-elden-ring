//! Donald and Goofy as AI allies, engine-free: the NPC pawns (`ATresNpcPawn_n_ex001 / _n_ex002`
//! : `TresNpcPawnBase` : `TresAIPawnBase`), their states, actions, projectiles, damage, KO /
//! revive, items, warp, status and progression, from KH3's decompiled code and data.
//!
//! The contract with the other streams is `docs/party-contracts.md` section B; the plan is
//! `docs/party-port-plan.md` (stream B). Units and axes are KH3's, as `kh_core`'s.
//!
//! Engine-free rules: glam, std and kh-core only; no files, no clocks, no hash-map iteration;
//! the same commands against the same host answers give the same outputs.
//!
//! Modules:
//! - `ids`, `command`, `view`, `host`: the contract types.
//! - `party`: the `Party` and its per-step order.
//! - `pawn`: one member (movement component, anim slot, state, status, command).
//! - `states/`: the NPC states. `actions/`: the action definitions (attacks, spells, specials).
//! - `projectile`: Donald's spells and Goofy's discus / turbo bodies in flight.
//! - `status`: a friend's status (level from the shared EXP, stats, equipment, abilities, MP).
//! - `tables`: generated and hand-cited data (base / level rows, attack rows, action numbers).
//! - `anim`: the NPC's anim clock over `ClipDef` tables.
//! - `team/`: stream D's (team attacks).

#![forbid(unsafe_code)]

pub mod actions;
pub mod balance; // (GOOFY-BAL) the friend leaders' stats and damage taken, balanced like Sora
#[cfg(feature = "ai")]
pub mod ai_body;
pub mod boosts; // (IT) the roster's permanent stat boosts (one total for every character, unlocked or not)
pub mod donald; // (DON1) the Donald overhaul's shared foundation (documentation/DONALD_OVERHAUL.md; build/sora_tmp/party/DON_PLAN.md)
pub mod anim;
pub mod command;
pub mod hitfx; // (AUD) a friend's melee hit: its sound and effect on contact (the audit found none)
pub mod guard; // (GOOFY-GG) Goofy's GUARD gauge (his block's meter; the silver HUD bar above MP)
pub mod host;
pub mod face; // (ZETTA) a member's face-clip slot (any face clip over the body pose; the FACE stream builds on it)
pub mod hurt; // (GOOFY-HR) the hit reactions of every playable character (hurt / blow / down / get-up / air recovery)
pub mod ids;
pub mod notify_seam;
// (party P) stream P owns src/pad/** (the playable members)
pub mod pad;
pub mod party;
pub mod pawn;
pub mod projectile;
pub mod rng;
pub mod states;
pub mod status;
pub mod tables;
pub mod team;
// (B4) the traversal / situation states (SlopeSlide, SuperSlide, RailSlide, Hopping, SituationAction, the fall)
pub mod traverse;
// (B5) the friends' jump / fall (NavLinkJumpCodeDriven) and the ledge states (Climbing, Dangle)
pub mod climb;
pub mod jump;
// (LADDER2) port design: the ladder jump (KH3 has no ladders)
pub mod ladder;
pub mod leader_magic; // (party X2) a friend leader casts Sora's magic; Customize follows the leader
pub mod locomotion; // (party K1) the AI members' follow speed and locomotion
pub mod view;
pub mod yhud; // (YHUD) the Y stack and the player's Y belong to the controlled character

pub use command::{ActionDef, CommandStatus, CommandTicket, ItemKind, LocomotionDef, MemberCommand, MoveGoal, TargetRef};
pub use host::{NullPartyHost, PartyHost};
pub use ids::{Chara, MemberId};
pub use party::{Party, PartyConfig};
pub use states::NpcStateId;
pub use ladder::{LadderJump, Leap};
pub use status::FriendStatus;
pub use guard::{Guard, GuardView}; // (GOOFY-GG)
pub use view::{ActionCooldowns, AnimView, ItemSlot, MemberView, PartyCamera, PartyEvent, PartyEventKind, PartyOutput, SoraHeal, SoraView};
