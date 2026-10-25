//! Sora's simulation, engine-free: Kingdom Hearts III's player character, written from the
//! specifications under `docs/` (`sora-movement-spec.md`, `sora-animation-spec.md` and, as they
//! are implemented, the combat / magic / actions / stats / audio / UI / VFX specs).
//!
//! This crate knows nothing about Bevy, windows, files or clocks: it depends on `glam` and `std`
//! only, because it is compiled both into the Bevy test level (`kh-app`) and, later, into a DLL
//! hosted by another game. Everything a host has to supply or consume is in this file's re-exports.
//! The contract in prose: `docs/sora-port-plan.md` (section 3: the host seam).
//!
//! # Conventions (KH3's own, so numbers from the decompilation apply unchanged)
//!
//! - Lengths in **centimetres**, time in **seconds**, angles in **degrees**.
//! - Axes as in UE4: **X forward, Y right, Z up**, left-handed.
//! - Yaw is measured about Z from +X toward +Y: the facing direction of yaw `a` is
//!   `(cos a, sin a, 0)`. Yaw 90 faces +Y (to the right of yaw 0).
//! - A position is the **actor origin**, which for Sora is at foot level: the collision capsule
//!   (radius 60, half height 89) has its centre 91 cm above it.
//!
//! # The fixed-step contract
//!
//! [`Sora::step_hosted`] (or [`Sora::step`], with a world only) advances the simulation by
//! exactly `dt_fixed` seconds. The host accumulates real time and calls it zero or more times a
//! frame, always with the same `dt_fixed`, and draws between the two latest [`Output`]s (see
//! [`Output::lerp`]). The core has no globals, reads no clock and iterates no hash map: the same
//! sequence of `(dt_fixed, Input)` against the same [`World`] and [`host::Targets`] answers gives
//! bit-identical outputs on the same build.
//!
//! # Modules, and who owns them
//!
//! Stage 1 (movement; behaviour pinned by `tests/sora.rs` and `tests/golden.rs`):
//! - `sora`: the API and the per-step order (controller, movement with the pose tick, states).
//! - `controller`: the controller's input update; calls the request interface.
//! - `states/`: the state-machine host and one file per `TresPlayerState_*` (`states/mod.rs`
//!   says how a state is added). `pawn`: what a state can reach (`Ctx`).
//! - `cmc`: the movement component (UE 4.17 walking and falling with the game's changes).
//! - `anim`: animation clock, notifies, blend-space sampling, root motion; generated tables.
//! - `params`: generated `SoraParams`. `enums`: generated reflected enums. `input`: pad buttons.
//! - `world`: collision queries the host answers. `test_world`: planes and boxes for tests.
//!
//! The ground for the systems to come (frames only; each `mod.rs` names its spec and owner):
//! - `host/`: the seam for targets, audio, effects, UI, and the per-step event list.
//! - `status/`: level, EXP, gauges, stats, abilities, magic, items (data) and loadouts.
//! - `ui_model/`: the plain-data HUD snapshot.
//! - `combat/`, `magic/`, `abilities/`: empty frames for the rules several states share.

#![forbid(unsafe_code)]

pub mod abilities;
pub mod actions;
pub mod anim;
// (party stream B, 2026-10-15) public so kh-party's NPC pawns run the same movement component; no behaviour change.
pub mod cmc;
pub mod combat;
pub mod command;
pub mod config;
mod controller;
pub mod enums;
pub mod host;
pub mod input;
pub mod leap; // (LADDER2) the ladder jump's arc (port design: the hosts with ladders)
pub mod magic;
pub mod notify;
pub mod params;
mod pawn;
pub mod sora;
pub mod states;
pub mod status;
pub mod test_world;
pub mod ui_model;
pub mod world;

pub use anim::{AnimId, AnimRequest, Locomotion, NotifyClass, NotifyData, NotifyDef, NotifyEvent, NotifyPhase, PlayArgs};
pub use glam::{Vec2, Vec3};
pub use host::{PartyMember, Attach, Audio, EffectHandle, EffectId, EffectSpawn, Effects, Event, EventKind, HitRecord, HitResult, Host, HurtShape, NullHost, RecordingHost, SoundHandle, SoundId, SoundPlay, TargetFlags, TargetId, TargetInfo, Targets, UiSink};
pub use input::{PadButton, PadButtons, PadState};
pub use params::{SoraParams, SORA_PARAMS, SORA_PARAMS_ORIGIN, SORA_PARAM_INFORMATIONAL, SORA_PARAM_ROWS};
pub use sora::{CameraState, DebugEmit, Input, MovementState, Output, Pose, Sora, Tuning};
pub use states::{StateFlags, StateId};
pub use status::{Loadout, Status, StatusRequest};
pub use ui_model::{UiPopup, UiSnapshot};
pub use world::{surface, ComponentMotion, Hit, HopPoint, Pole, Rail, RailNode, World};

/// Which mover `Sora::step` runs: KH3's `TresCharMovementComponent` with the
/// `TresPlayerState_Idle / _Run / _Jump` states, stage-1 subset.
pub const MOVER: &str = "kh3-tres-stage1";
