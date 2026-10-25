//! Donald's and Goofy's AI (docs/party-port-plan.md stream C; contract C of docs/party-contracts.md).
//!
//! KH3's friends are UE 4.17 behaviour trees + blackboards + EQS with Square Enix node classes.
//! This crate runs the game's own trees (decoded by `tools/party_ai_pack.js` into
//! `build/sora_pack/party/ai/`) with:
//!
//! - `bb`: the blackboard (typed keys, parent chains, change observers);
//! - `bt`: the behaviour-tree component (search, decorators with flow aborts, services, subtrees
//!   with dynamic injection, latent tasks), stock UE 4.17 semantics [U];
//! - `eqs`: the environment-query runtime (generators, tests, contexts, scoring);
//! - `nodes`: the stock and Tres node classes, transcribed from the decompiled C;
//! - `controller`: the friend AI controller (styles, coordinator, cure / item choice, warp);
//! - `api`: the seam (`AiBody` for the pawn, `AiWorld` for the world, `Nav`).
//!
//! Engine-free: glam, serde_json and std; no files (the host passes the pack's text), no clocks
//! (time is the sum of the `dt` passed in), deterministic RNG streams per controller (plan N6).

pub mod api;
pub mod bb;
pub mod bt;
pub mod controller;
pub mod cure;
pub mod eqs;
pub mod formation; // (FOL2) the follow spots: reachable, corridor-aware
pub mod nodes;
pub mod pack;
pub mod rng;

pub use api::*;
pub use controller::{AiController, AiShared, AiTrace};
pub use pack::AiPack;
