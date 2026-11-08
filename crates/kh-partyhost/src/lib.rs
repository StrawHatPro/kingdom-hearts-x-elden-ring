//! THE ENGINE-FREE PARTY HOST (Donald, Goofy, AI Sora) that the Bevy test level (kh-app) and the Elden Ring mod
//! (kh-er, stream S1 onward) both drive. Plan: build/sora_tmp/er/party/ER_PARTY_PLAN.md stream S0; contract:
//! docs/party-contracts.md section "ER". State note: build/sora_tmp/er/party/IMPL_S0_STATE.md.
//!
//! No Bevy, no files, no clocks: the host reads the packs and sheets and hands them in, steps the runtime in its
//! fixed step, and draws / plays what comes out.
//!
//! ONE FIXED STEP of a host (the test level's order, player.rs):
//! 1. [`PartyRuntime::route`] before Sora's step: the formation, the leader's pad frame, AI Sora's input (returned:
//!    Sora's input this step);
//! 2. the host steps Sora with [`SoraTargets`] (its targets + the friends as his allies + the team-attack seam);
//! 3. [`PartyRuntime::step`]: AI ticks, `Party::step`, `Team::step`, Sora's heals, the log;
//! 4. [`PartyRuntime::pose`] with the host's [`PartyPoser`]: the members' poses (taps for the effect / sound side);
//! 5. [`PartyRuntime::after_step`]: the leader's state, a lock-on camera change;
//! 6. after the host's field camera: [`PartyRuntime::team_camera`] (the team attacks' CameraAnims).
//!
//! The HUD feed (`hud_slots`, `menu_members`, `level_ups`, `ai_members`, `hud_snapshot`, `leader_chara`), the
//! camp-menu edits (`apply_edit`), the words (`word`, `lead_word`) and the save record (`to_save` / `from_save`)
//! are read / called whenever the host's screens want them.

#![forbid(unsafe_code)]

pub mod log;

pub mod aiworld;
pub mod caught; // (GRABS) the caught gate: an Elden Ring throw holds an AI-driven character (no action, KH3's caught motion, the guards)
pub mod boosts; // (IT) the roster's permanent stat boosts: received once, given to every character, saved once
pub mod custom; // (party X2) Sora's magic for a friend leader, Customize per leader, the block in the save
pub mod goofy; // (GOOFY) the test level's fake attacks
pub mod donald; // (DON1) Donald's overhaul in the host: the roster's learned magic, its save record, the sheets at run time
pub mod growth; // (GF-STATS) the friends' boss rewards: raw HP / MP / slots, Goofy's rank steps, their save record
pub mod lead;
pub mod loadout;
pub mod mimicboss; // (MIMIC-BOSS) the two Mimic Tear bosses: the shared list, the solo fight, the copy of the leader, its hostile side
pub mod mimic; // (MIMIC SORA, THE USER 2026-11-02) what Mimic Sora copies of Sora at the summon, his core, the Mimic Tear's upgrades
pub mod parry_page; // (GOOFY, THE USER 2026-11-02) Goofy's Parry Techniques page of the camp menu, from his live level and ranks
pub mod poser;
pub mod progression; // (ER party S6) the progression rules shared by both hosts
pub mod runtime;
pub mod hopguard; // (ZETTA 2026-10-23) the AI friends never loop a move (Zettaflare's hand-back, the hop loop)
pub mod save;
pub mod seam;

pub use aiworld::{member_actor, EnemyFeed, LeaderAiWorld, PartyTargets, SoraAiWorld, SORA_ACTOR, STRAIGHT_NAV};
pub use lead::{BossAi, BossScene, Lead, LockChange, Leadership, MimicAi, MimicScene, RouteIn, BOSS_ACTOR, MIMIC_ACTOR}; // (MIMIC-BOSS) + BossAi, BossScene, BOSS_ACTOR
pub use loadout::{PartyLoadout, PartyTest, PartyWeapon};
pub use poser::{MemberTap, MenuPoser, PartyPoser, PosedChar, PosedMember, PoseInput};
pub use runtime::{ai_settings_of, chara_of, party_chara, AiPackSource, LevelUp, PartyRuntime, ProjectileTap, RuntimeConfig, SoraTargets, StepIn, StepOut, Victim, ALLY_BASE};
pub use save::{FriendRecord, PartySave, FRIENDS_RECORD_VERSION};
pub use progression::{read_party_record, starting_formation, ExpGiven, FriendReward, ZeroExp}; // (ER party S6)
pub use seam::{AllyWant, LeaderHit, LeaderSink, LeaderView, RuntimeLeaderSink, SORA_CAPSULE};
pub use kh_party::{LadderJump, MemberCommand};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod k1_tests; // (party K1) the follow-speed harness
#[cfg(test)]
mod fx1_tests; // (FX1) a friend leader's finisher never vanishes (the team roll's gate and late "no")
#[cfg(test)]
mod gg_tests; // (GOOFY-GG) Goofy's GUARD gauge in the host (HUD feed, levels, save, refills)
#[cfg(test)]
mod sm_tests; // (SM) the friends' smoothness by rendered frames (uneven frame times)
#[cfg(test)]
mod fx5_tests; // (FX5) the friends fight what threatens the party (the trees' enemy list)
#[cfg(test)]
mod sorako_tests; // (HUD-REVIVE SORAKO) AI Sora's KO: on the floor in his down pose, no warp, revive -> get-up -> Idle, AI again
#[cfg(test)]
mod combat_tests; // (HUD-REVIVE-COMBAT) one party combat state: party_combat = the members' = the HUD's, AI Sora follows it
#[cfg(test)]
mod eq_tests; // (EQ) a friend's camp-menu edits in the runtime and the save, as Sora's
#[cfg(test)]
mod face_tests; // (FACE) the friends' faces measured on their posed skeletons (eyes, beak, the face slots)
#[cfg(test)]
mod yhud_tests; // (YHUD) the Y stack and the player's Y are the controlled character's, in both host paths
#[cfg(test)]
mod zetta_tests; // (ZETTA round 5) enemies never choose Donald in Zettaflare, his invincibility (the test level's enemy choice)
#[cfg(test)]
mod threat_tests;
#[cfg(test)]
mod mimicboss_tests; // (MIMIC-BOSS) the Mimic Tear bosses: the gate, the solo fight, the copy, the hostile AI // (GF-THREAT) a fixture dummy's choice with Taunt and the threat book
#[cfg(test)]
mod mimicboss_kit_tests; // (MIMIC-BOSS-KIT) the copies' full kit: forms, Keyblade switch, Y commands, Zettaflare, no Summon
#[cfg(test)]
mod mimicboss_stance_tests; // (MIMIC-STANCE) a Goofy copy parries the leader, breaks his stance, slams him with Breaker
#[cfg(test)]
mod freeze_fx_tests; // (FREEZE-FX) the party freezes a Sora copy: KH3's frozen look on him
#[cfg(test)]
mod mimicboss_status_tests; // (MIMIC-BOSS hotfix 3, MB3-FREEZE) a copy's statuses on the party: KH3's player-side times
#[cfg(test)]
mod mimicboss_rise_tests; // (MIMIC-BOSS hotfix 3, MB3-FOGRISE) the rise at the fog gate: Elden Ring's own event, the pool's wake, the user's log numbers
#[cfg(test)]
mod mimicboss_fight_tests; // (MIMIC-BOSS hotfix 3) the fight keeps its body whatever its summon does and however far the leader runs
#[cfg(test)]
mod mimicboss_heal_tests; // (MIMIC-BOSS hotfix 3) a copy heals the same share of its own pool as the leader of his
#[cfg(test)]
mod mimicboss_combat_tests; // (MIMIC-BOSS hotfix 3, MB3-COMBAT) every party hit stuns every copy, the finishers' KH3 damage both ways, the copies' own knockdown on a stance break
