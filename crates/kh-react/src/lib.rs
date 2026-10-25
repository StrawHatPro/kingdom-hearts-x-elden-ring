//! What a Kingdom Hearts III enemy physically does when a hit lands, engine-free.
//!
//! The game decides a hit in two halves. The ATTACKER's half (which row hits, the damage, the
//! reaction KIND and the classification against the victim's body) is `kh_core::combat`. The
//! VICTIM's half is its damage state, `UTresEnemyState_Damage` : `UTresAICharState_Damage`: one
//! handler per damage kind that throws the body about with numbers from the victim's own
//! `TresDamageCommonParam` and ends when the victim's reaction clip ends. That half is this crate
//! (specification: docs/sora-enemy-reaction-spec.md; constants: sheets/port/sora-enemy-reaction.json).
//!
//! Units and axes are `kh-core`'s: centimetres, seconds, degrees, X forward, Y right, Z up; a
//! position is the victim's FOOT point. No engine: the host supplies one body sweep
//! ([`ReactWorld`]) and steps the reaction at 60 Hz.
//!
//! ```text
//! let mut reaction = EnemyReaction::new(Body::of(BodyClass::Medium));     // one per enemy, kept
//! // per hit (inside the host's `Targets::apply_hit`):
//! let request = ReactionRequest::from_hit(&hit, hp_lost, lethal);
//! reaction.hit(&request, Pose { position, yaw, grounded });
//! // per 60 Hz step:
//! let out = reaction.step(&world);
//! if out.suspended { /* write out.position / out.yaw to the enemy, play out.phase, keep its AI off */ }
//! if out.finished  { /* hand the enemy back to its AI, with out.velocity */ }
//! ```

pub mod params;
mod react;
mod world;

pub use kh_core::enums::ETresDamageKind;
pub use kh_core::host::{BodyReaction, ReactionResult};
pub use params::{Body, BodyClass, ClipTimes, DamageParams, EnemyReactionParams, ENEMY_REACTION_PARAMS};
pub use react::Clip as ReactClip; // (GOOFY-HR)
pub use react::{BadStatus, EnemyReaction, HitOutcome, Phase, Pose, ReactOutput, ReactionRequest, STEP};
pub use world::{CapsuleIn, Contact, FlatWorld, ReactWorld};
