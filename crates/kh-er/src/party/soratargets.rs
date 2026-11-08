//! (ER party S1) SORA'S STEP WITH THE PARTY (contract ER.0 step 2): his targets are Elden Ring's characters (ErTargets,
//! everything a target is) wrapped by `kh_partyhost::SoraTargets`: the friends are his allies (`allies`: Cure / items
//! on friends, Assist Friends; `heal_ally` queued for the party's next step), and the team-attack seam (`friends` from
//! the team's rows, `friend_link` to `Team::request`, stream S7). The test level's `SoraTargets` (kh-app) is the same
//! type over its dummies.

use kh_core::{Input, Output, Sora};

use crate::targets::ErTargets;
use crate::world::ErWorld;

/// What hosts Sora's step: the world, his targets, the sound and effect seams, the HUD's keep.
pub struct SoraSeams<'a> {
    pub world: &'a ErWorld,
    pub targets: &'a mut ErTargets,
    pub fx: &'a mut crate::fx::Fx,
    pub ui: &'a mut crate::hud::Keep,
}

/// One fixed step of Sora's with the party's targets.
pub fn step(sora: &mut Sora, dt: f32, input: &Input, SoraSeams { world, targets, fx, ui }: SoraSeams<'_>, runtime: &mut kh_partyhost::PartyRuntime) -> Output {
    let mut targets = kh_partyhost::SoraTargets { targets, runtime };
    let mut host = kh_core::Host { world, targets: &mut targets, audio: &mut fx.audio, effects: &mut fx.effects, ui };
    sora.step_hosted(dt, input, &mut host)
}
