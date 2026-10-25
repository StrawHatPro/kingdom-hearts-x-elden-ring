//! (ER party S5) DONALD'S AND GOOFY'S SOUND AND EFFECTS in the Elden Ring host: the party half of fx.rs (declared there
//! by `#[path]`, behind the cargo feature `party`). Plan: build/sora_tmp/er/party/ER_PARTY_PLAN.md stream S5; state note
//! build/sora_tmp/er/party/IMPL_S5_STATE.md.
//!
//! WHAT ARRIVES: every party sound, voice and effect of a step is already on Sora's seams (`Fx::audio` / `Fx::effects`:
//! S1's party step hands the same queues to `PartyRuntime::step`), with `Attach::Party { member, bone, offset }` and the
//! party's own handle ranges (kh_core::host::party). sora.rs gives `Fx::set_party` the step's feed (S1's `StepFeed`:
//! the members' taps = bones / sockets / `wep:` sockets, grounded, anim-set row, weapon; the spell actors / team props)
//! right before `Fx::after_step`. This file turns that into:
//! - THE PLACES: `Attach::Party` on the member's bones for the audio side (`place`) and the effect side
//!   (`EffectHost::set_party`); the spell actors' body effects follow the actors;
//! - THE MEMBERS' AUDIO STATE: their anim-set row (the auto-SE enable windows, the random voice lists), their weapon
//!   (swing / hit sets), their motion sounds (footsteps from their own bones with FsTable_Donald / _Goofy, on the
//!   ground under them: the leader's when theirs is not known, fxmap.rs `member_surface`);
//! - THE LOOPS: the party's looping sounds (by handle, until the seam stops them or they end) and its persistent
//!   effects (`EffectHost::party_loops`) are counted per member; `party loops:` lines say them. P3's notify-state End
//!   (kh-party `end_cut_states`) stops them through the seams; when the party is switched off (`party.er = 0`) or
//!   hidden (a cutscene, the wait after a load) everything the party started is stopped here, so no loop outlives it.
//!
//! KH3's voice rules (one voice line per member: Replace / IfIdle / Overlap, the QuarterHP / RecoveryHP gates, the cast
//! voice NUMBERS) live in kh-party and kh-audio (`Engine::party_seam_play`): this host plays what they ask, unchanged.
//! The listener is the camera's (Sora's: fx.rs).

use std::collections::HashMap;

use glam::Vec3;
use kh_core::Attach;
use kh_host::seam::AudioCommand;
use kh_host::vfx::PartyMemberFx;

/// The party's own audio root (D5: `tools/sora_audio_pack.js --party-extra`): merged into the main pack by fx.rs.
pub fn audio_root(pack_dir: &std::path::Path) -> std::path::PathBuf {
    pack_dir.join("party").join("audio")
}

/// (ER party I) The character of a party member id of the taps (contract ER.1: Donald 0, Goofy 1; the staged friends'
/// own runtimes use the same ids).
pub fn chara_of_member(member: u8) -> Option<kh_host::party::PartyChara> {
    match member {
        0 => Some(kh_host::party::PartyChara::Donald),
        1 => Some(kh_host::party::PartyChara::Goofy),
        _ => None,
    }
}

/// One member's audio state of the step.
#[derive(Clone, Debug, Default)]
pub struct MemberAudio {
    pub step: u64,
    pub grounded: bool,
    pub anim: (String, f32),
    pub weapon: String,
}

/// The party's sound / effect state between steps (owned by `Fx`).
#[derive(Default)]
pub struct PartyFx {
    /// `Fx::set_party` was called for the coming step.
    pub fed: bool,
    /// The party runs and is shown this step.
    pub active: bool,
    pub was_active: bool,
    /// The party went off or hidden at this step: what it started is stopped (sounds and effects).
    pub ended: bool,
    /// The members of the step: `kh_core::PartyMember.0` -> pose, audio state.
    pub members: Vec<(u8, PartyMemberFx, MemberAudio)>,
    /// The party's spell actors / team props: body-effect handle, position, flight direction.
    pub projectiles: Vec<(u32, Vec3, Vec3)>,
    /// The party's looping sounds still running: handle -> name.
    pub loop_sounds: HashMap<u32, &'static str>,
    /// The member's tap step their motion-sound detector last ran on.
    last_steps: HashMap<u8, u64>,
    /// The loop counts last said (a line when they change to all-zero; once a second while not).
    pub told: String,
}

impl PartyFx {
    /// The step's feed: (member, step, position, yaw, bones, grounded, anim, weapon) per tap; the projectiles; whether the
    /// party is running and shown.
    pub fn feed(&mut self, taps: Vec<(u8, u64, Vec3, f32, HashMap<String, (Vec3, glam::Quat)>, bool, (String, f32), String)>, projectiles: Vec<(u32, Vec3, Vec3)>, active: bool) {
        self.fed = true;
        self.active = active;
        self.ended = self.was_active && !active;
        self.was_active = active;
        self.members = if active { taps.into_iter().map(|(m, step, position, yaw, bones, grounded, anim, weapon)| (m, PartyMemberFx { position, yaw, bones, anim: anim.0.clone() }, MemberAudio { step, grounded, anim, weapon })).collect() } else { Vec::new() };
        self.projectiles = if active { projectiles } else { Vec::new() };
    }

    pub fn member(&self, member: u8) -> Option<&PartyMemberFx> {
        self.members.iter().find(|m| m.0 == member).map(|m| &m.1)
    }

    /// Where an attach of a party member is now (`None`: not a party attach, or the member is not there).
    pub fn place(&self, at: &Attach) -> Option<Vec3> {
        match at {
            Attach::Party { member, bone, offset } => self.member(member.0).map(|m| m.xform(*bone, *offset).0),
            _ => None,
        }
    }

    /// The step's audio commands: the party's looping sounds started / stopped.
    pub fn track(&mut self, commands: &[AudioCommand]) {
        for c in commands {
            match c {
                AudioCommand::Play(h, play) if play.looping && kh_core::host::party::is_party_handle(h.0) => {
                    self.loop_sounds.insert(h.0, play.name);
                }
                AudioCommand::Stop(h, _) => {
                    self.loop_sounds.remove(&h.0);
                }
                _ => {}
            }
        }
    }

    /// After the engine's step: the looping sounds that are no longer playing (or were never started: no route) go.
    pub fn prune(&mut self, live: impl Fn(u32) -> bool) {
        self.loop_sounds.retain(|h, _| live(*h));
    }

    /// The loops per owner (`PartyMember.0`, 255 = the team pawns): (owner, looping sounds, persistent effects).
    pub fn loops(&self, effects: &[(u8, usize)]) -> Vec<(u8, usize, usize)> {
        let mut out: Vec<(u8, usize, usize)> = effects.iter().map(|(o, n)| (*o, 0, *n)).collect();
        for h in self.loop_sounds.keys() {
            let owner = kh_core::host::party::handle_member(*h).map_or(255, |m| m.0);
            match out.iter_mut().find(|e| e.0 == owner) {
                Some(e) => e.1 += 1,
                None => out.push((owner, 1, 0)),
            }
        }
        out.sort();
        out
    }

    /// A member's tap is new since his detector last ran (and how many steps it covers).
    pub fn new_steps(&mut self, member: u8, step: u64) -> Option<u64> {
        let last = self.last_steps.get(&member).copied().unwrap_or(0);
        if step <= last {
            return None;
        }
        self.last_steps.insert(member, step);
        Some(if last == 0 { 1 } else { (step - last).min(8) })
    }
}

/// The name of an owner of the loop counts.
pub fn owner_name(owner: u8) -> &'static str {
    match owner {
        0 => "Donald",
        1 => "Goofy",
        255 => "team",
        _ => "member",
    }
}

/// The loop counts as a log text ("0" when nothing runs).
pub fn loops_text(loops: &[(u8, usize, usize)]) -> String {
    let live: Vec<String> = loops.iter().filter(|l| l.1 + l.2 > 0).map(|(o, s, e)| format!("{} {s} sound(s) / {e} effect(s)", owner_name(*o))).collect();
    if live.is_empty() {
        "0".to_string()
    } else {
        live.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::host::party::{PartyMember, SeamHandles};
    use kh_core::{SoundHandle, SoundPlay};

    fn tap(member: u8, step: u64) -> (u8, u64, Vec3, f32, HashMap<String, (Vec3, glam::Quat)>, bool, (String, f32), String) {
        let mut bones = HashMap::new();
        bones.insert("atama".to_string(), (Vec3::new(10.0, 0.0, 120.0), glam::Quat::IDENTITY));
        bones.insert("wep:vfx_call".to_string(), (Vec3::new(40.0, 0.0, 150.0), glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)));
        (member, step, Vec3::new(100.0, 0.0, 0.0), 90.0, bones, true, ("MagicFire".to_string(), 0.2), "w_do010".to_string())
    }

    /// The attach rule: a bone (offset in its frame), the actor (offset turned by the yaw), a bone the skeleton lacks,
    /// a member who is not there; nothing while the party is hidden.
    #[test]
    fn party_attaches_resolve_on_the_members_bones() {
        let mut p = PartyFx::default();
        p.feed(vec![tap(0, 1)], vec![(0x2000_0005, Vec3::ONE, Vec3::X)], true);
        let d = PartyMember::DONALD;
        assert_eq!(p.place(&Attach::Party { member: d, bone: Some("atama"), offset: Vec3::ZERO }), Some(Vec3::new(10.0, 0.0, 120.0)));
        // the staff socket turned a quarter: +X of its frame is +Y of the world
        let v = p.place(&Attach::Party { member: d, bone: Some("wep:vfx_call"), offset: Vec3::X * 10.0 }).unwrap();
        assert!((v - Vec3::new(40.0, 10.0, 150.0)).length() < 1e-3, "{v}");
        // the actor: at the feet, the offset turned by the yaw (90: +X forward becomes +Y)
        let v = p.place(&Attach::Party { member: d, bone: None, offset: Vec3::X * 50.0 }).unwrap();
        assert!((v - Vec3::new(100.0, 50.0, 0.0)).length() < 1e-3, "{v}");
        // a bone he lacks: a metre above his feet
        assert_eq!(p.place(&Attach::Party { member: d, bone: Some("no_such"), offset: Vec3::ZERO }), Some(Vec3::new(100.0, 0.0, 100.0)));
        // Goofy is not there; Sora's attach is not this file's
        assert_eq!(p.place(&Attach::Party { member: PartyMember::GOOFY, bone: None, offset: Vec3::ZERO }), None);
        assert_eq!(p.place(&Attach::World(Vec3::ONE)), None);
        // hidden: no members, no projectiles
        p.feed(vec![tap(0, 2)], vec![(1, Vec3::ONE, Vec3::X)], false);
        assert!(p.members.is_empty() && p.projectiles.is_empty() && p.fed && !p.active);
        // the EffectHost's own rule is the same
        let fx = PartyMemberFx { position: Vec3::new(100.0, 0.0, 0.0), yaw: 90.0, ..Default::default() };
        assert!((fx.xform(None, Vec3::X * 50.0).0 - Vec3::new(100.0, 50.0, 0.0)).length() < 1e-3);
    }

    /// The loop counts: a looping party sound counts until it is stopped or no longer plays; one-shots and Sora's never.
    #[test]
    fn loops_count_until_stopped() {
        let mut p = PartyFx::default();
        let mut donald = SeamHandles::for_member(PartyMember::DONALD);
        let mut team = SeamHandles::team();
        let (a, b, c) = (donald.sound(), donald.sound(), team.sound());
        let looping = SoundPlay { looping: true, ..SoundPlay::at_sora(kh_core::SoundId(1), "tornado_loop") };
        let once = SoundPlay::at_sora(kh_core::SoundId(2), "swing");
        p.track(&[AudioCommand::Play(a, looping), AudioCommand::Play(b, once), AudioCommand::Play(c, looping), AudioCommand::Play(SoundHandle(7), looping)]);
        assert_eq!(p.loops(&[(0, 2)]), vec![(0, 1, 2), (255, 1, 0)]);
        assert_eq!(loops_text(&p.loops(&[(0, 2)])), "Donald 1 sound(s) / 2 effect(s), team 1 sound(s) / 0 effect(s)");
        p.track(&[AudioCommand::Stop(a, 0.2)]);
        p.prune(|h| h != c.0);
        assert_eq!(loops_text(&p.loops(&[])), "0");
    }

    /// The D5 test pattern on THIS host's loader path: the main pack + the party root merged by fx.rs's call; Donald's
    /// Cure target effect vm_cur_bds1 then plays its SeAttach se00301_040 (the party root's own wav) through the engine.
    #[test]
    fn the_er_loader_merges_the_party_root_and_the_cure_target_se_plays() {
        let pack_dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack"));
        if !pack_dir.join("audio").join("manifest.json").is_file() || !audio_root(pack_dir).join("manifest.json").is_file() {
            eprintln!("SKIPPED: no audio packs under {}", pack_dir.display());
            return;
        }
        let mut pack = kh_audio::Pack::load(&pack_dir.join("audio")).expect("the main audio pack");
        let triggers = pack.triggers.len();
        assert_eq!(pack.merge_party_root(&audio_root(pack_dir)), Ok(true));
        assert!(pack.triggers.len() > triggers);
        let mut engine = kh_audio::Engine::new(std::sync::Arc::new(pack), 1, kh_audio::Language::En);
        engine.effect_spawned(0x2000_0007, "vm_cur_bds1", Vec3::new(100.0, 0.0, 90.0));
        let _ = engine.update(1.0 / 60.0, &kh_audio::Listener::default(), &|_| Some(Vec3::ZERO));
        let log = engine.take_log();
        assert!(log.iter().any(|l| format!("{l:?}").contains("se00301_040")), "{log:?}");
    }

    #[test]
    fn a_members_detector_runs_once_per_new_tap() {
        let mut p = PartyFx::default();
        assert_eq!(p.new_steps(1, 5), Some(1));
        assert_eq!(p.new_steps(1, 5), None);
        assert_eq!(p.new_steps(1, 7), Some(2));
        assert_eq!(p.new_steps(1, 100), Some(8));
    }
}
