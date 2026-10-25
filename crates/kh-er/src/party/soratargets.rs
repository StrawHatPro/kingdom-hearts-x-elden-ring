//! (ER party S1) SORA'S STEP WITH THE PARTY (contract ER.0 step 2): his targets are Elden Ring's characters (ErTargets,
//! everything a target is) wrapped by `kh_partyhost::SoraTargets`: the friends are his allies (`allies`: Cure / items
//! on friends, Assist Friends; `heal_ally` queued for the party's next step), and the team-attack seam (`friends` from
//! the team's rows, `friend_link` to `Team::request`, stream S7). The test level's `SoraTargets` (kh-app) is the same
//! type over its dummies.

use kh_core::{Input, Output, Sora};

use crate::targets::ErTargets;
use crate::world::ErWorld;

/// One fixed step of Sora's with the party's targets.
#[allow(clippy::too_many_arguments)]
pub fn step(sora: &mut Sora, dt: f32, input: &Input, world: &ErWorld, targets: &mut ErTargets, runtime: &mut kh_partyhost::PartyRuntime, fx: &mut crate::fx::Fx, ui: &mut crate::hud::Keep) -> Output {
    let mut targets = kh_partyhost::SoraTargets { targets, runtime };
    let mut host = kh_core::Host { world, targets: &mut targets, audio: &mut fx.audio, effects: &mut fx.effects, ui };
    sora.step_hosted(dt, input, &mut host)
}
