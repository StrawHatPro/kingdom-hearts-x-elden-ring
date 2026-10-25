//! (DON-PROG) AGGRO: HOW OFTEN ENEMIES CHOOSE DONALD (documentation/DONALD_OVERHAUL.md 5.4, 11 "Team role").
//!
//! ONLY WHOM AN ENEMY CHOOSES CHANGES, NEVER WHERE A HIT LANDS (ER_PHASE_NOTES "NO HIT REDIRECTION", binding; DON_PLAN
//! R21): a hit already thrown at Donald is his. None of this applies when he is alone (no other party member: sheet
//! `aggro.others_min`). The same rules hold for the leader Donald and AI Donald.
//!
//! | Ability | Rule here |
//! |---|---|
//! | Low Profile (10) | an enemy's choice weighs him x (1 - `aggro.low_profile`) = x0.8 |
//! | Hide Behind (25) | x (1 - `aggro.hide_behind`) = x0.6 more while another member is within `aggro.hide_behind_radius` (8 m) |
//! | Distraction (40) | an enemy that has chosen him switches to his nearest ally with `aggro.distraction` (30 %) |
//! | Taunt Pass (55) | an ally blocking / guarding (Goofy's GUARD) turns the enemies within `aggro.taunt_pass_radius` of Donald onto that ally |
//! | Out of Sight (70) | standing still (`aggro.out_of_sight_still` s below `aggro.out_of_sight_speed`) and casting (a cast begun within `aggro.out_of_sight_cast` s), the enemies farther than `aggro.out_of_sight_range` (15 m) lose interest: weight 0 |
//!
//! Low Profile and Hide Behind stack by multiplying (x0.48 with both: DON-PROG reading; the doc lists them as two
//! abilities, each "less often"). THE CHOICE ([`choose`]): the host's own rule picks the nearest candidate; when that is
//! Donald, the enemy keeps him only with his weight (else it takes the next nearest), then Distraction may still move it
//! to his nearest ally. Hosts: the test level's enemy fixtures (kh-partyhost `PartyRuntime::victim`), Elden Ring's hate
//! (kh-er party/donaggro.rs, through the taunt route: a minimal hate hit from the ally's body).

use super::abilities::{DonAbilities, DonAbility};
use super::ability_info::prog;

/// What the rules need to know about Donald now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AggroCtx {
    /// Other party members present (any leader): 0 = alone, nothing applies.
    pub others: usize,
    /// The nearest other member (cm), `f32::INFINITY` when none.
    pub nearest_ally: f32,
    /// He has stood still long enough ([`Stillness`]).
    pub still: bool,
    /// He is casting (a cast begun within the window: [`Stillness`]).
    pub casting: bool,
}

impl AggroCtx {
    /// Donald alone.
    pub const ALONE: AggroCtx = AggroCtx { others: 0, nearest_ally: f32::INFINITY, still: false, casting: false };
}

/// The rules apply (a party of at least two).
pub fn applies(ctx: &AggroCtx) -> bool {
    ctx.others as f32 >= prog().value("aggro.others_min").max(1.0)
}

/// Out of Sight holds for an enemy at `enemy_dist` (cm): it loses interest in him.
pub fn out_of_sight(a: &DonAbilities, ctx: &AggroCtx, enemy_dist: f32) -> bool {
    applies(ctx) && a.has(DonAbility::OutOfSight) && ctx.still && ctx.casting && enemy_dist > prog().value("aggro.out_of_sight_range")
}

/// THE WEIGHT an enemy at `enemy_dist` (cm) gives Donald when it chooses its target: 1 = unchanged, 0 = never.
pub fn pick_weight(a: &DonAbilities, ctx: &AggroCtx, enemy_dist: f32) -> f32 {
    if !applies(ctx) {
        return 1.0;
    }
    if out_of_sight(a, ctx, enemy_dist) {
        return 0.0;
    }
    let p = prog();
    let mut w = 1.0;
    if a.has(DonAbility::LowProfile) {
        w *= 1.0 - p.value("aggro.low_profile").clamp(0.0, 1.0);
    }
    if a.has(DonAbility::HideBehind) && ctx.nearest_ally <= p.value("aggro.hide_behind_radius") {
        w *= 1.0 - p.value("aggro.hide_behind").clamp(0.0, 1.0);
    }
    w
}

/// DISTRACTION: an enemy that has just chosen Donald switches to his nearest ally (`roll` uniform in 0..1).
pub fn switch_on_target(a: &DonAbilities, ctx: &AggroCtx, roll: f32) -> bool {
    applies(ctx) && ctx.nearest_ally.is_finite() && a.has(DonAbility::Distraction) && roll < prog().value("aggro.distraction")
}

/// TAUNT PASS: an ally blocks / guards; an enemy `enemy_to_donald` (cm) from Donald turns onto that ally.
pub fn taunt_pass(a: &DonAbilities, ctx: &AggroCtx, enemy_to_donald: f32) -> bool {
    applies(ctx) && a.has(DonAbility::TauntPass) && enemy_to_donald <= prog().value("aggro.taunt_pass_radius")
}

/// One candidate of an enemy's choice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cand {
    /// From the enemy (cm).
    pub dist: f32,
    pub donald: bool,
    /// From Donald (cm; Distraction's "nearest ally").
    pub to_donald: f32,
}

/// THE CHOICE among `cands` (the host's nearest rule with Donald's weight and Distraction): the index chosen. `keep` and
/// `distract` are two uniform rolls in 0..1 (the host's own random or a stable hash). `None` when `cands` is empty.
pub fn choose(cands: &[Cand], a: &DonAbilities, ctx: &AggroCtx, keep: f32, distract: f32) -> Option<usize> {
    let mut order: Vec<usize> = (0..cands.len()).collect();
    order.sort_by(|x, y| cands[*x].dist.total_cmp(&cands[*y].dist));
    let first = *order.first()?;
    if !cands[first].donald || !applies(ctx) {
        return Some(first);
    }
    let others: Vec<usize> = order.iter().copied().filter(|i| !cands[*i].donald).collect();
    if others.is_empty() {
        return Some(first);
    }
    let w = pick_weight(a, ctx, cands[first].dist);
    if keep >= w {
        return Some(others[0]);
    }
    if switch_on_target(a, ctx, distract) {
        return others.iter().copied().min_by(|x, y| cands[*x].to_donald.total_cmp(&cands[*y].to_donald));
    }
    Some(first)
}

/// A stable roll in 0..1 from a few numbers (the hosts without a random source of their own: the same enemy, the same
/// second, the same answer).
pub fn roll(a: u64, b: u64, salt: u64) -> f32 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt.wrapping_mul(0x1656_67B1_9E37_79F9);
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

/// OUT OF SIGHT'S TWO CONDITIONS, kept per Donald by a host (frame-rate independent).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stillness {
    /// Seconds below the still speed.
    pub still_for: f32,
    /// Seconds since a cast began (large: none).
    pub since_cast: f32,
    /// A cast ran last step (the edge of a new one).
    pub casting_now: bool,
}

impl Default for Stillness {
    fn default() -> Stillness {
        Stillness::new()
    }
}

impl Stillness {
    pub fn new() -> Stillness {
        Stillness { still_for: 0.0, since_cast: 1.0e6, casting_now: false }
    }

    /// One step: his horizontal speed (cm/s), a cast began this step.
    pub fn tick(&mut self, speed: f32, cast_began: bool, dt: f32) {
        let p = prog();
        if speed <= p.value("aggro.out_of_sight_speed") {
            self.still_for += dt;
        } else {
            self.still_for = 0.0;
        }
        self.since_cast = if cast_began { 0.0 } else { self.since_cast + dt };
    }

    pub fn still(&self) -> bool {
        self.still_for >= prog().value("aggro.out_of_sight_still")
    }

    pub fn casting(&self) -> bool {
        self.since_cast <= prog().value("aggro.out_of_sight_cast")
    }
}
