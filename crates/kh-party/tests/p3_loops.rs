//! (party P3) Every looping sound and persistent effect a member's notify states start is stopped when the state's clip
//! ends for ANY reason: the action's next clip (Tornado M_1322 -> M_1323, Turbo M_1325 -> M_1326), a KO, a removal.
//! (The user's report: "Goofy's whirlwind effect and sfx persists after he uses it.")

use glam::Vec3;
use kh_core::host::{EffectHandle, EffectSpawn, HitRecord, SoundHandle, SoundPlay, TargetId};
use kh_core::test_world::TestWorld;
use kh_party::{ActionDef, Chara, MemberCommand, NpcStateId, Party, PartyConfig, PartyHost, SoraView, TargetRef};

const DT: f32 = 1.0 / 60.0;

/// Counts what is running: looping sounds and persistent effects by handle.
#[derive(Default)]
struct Seams {
    sounds: Vec<(u32, &'static str)>,
    effects: Vec<(u32, &'static str)>,
    started_sounds: usize,
    started_effects: usize,
}

impl kh_core::Audio for Seams {
    fn play(&mut self, handle: SoundHandle, play: &SoundPlay) {
        if play.looping {
            self.sounds.push((handle.0, play.name));
            self.started_sounds += 1;
        }
    }
    fn stop(&mut self, handle: SoundHandle, _fade_out: f32) {
        self.sounds.retain(|s| s.0 != handle.0);
    }
}

impl kh_core::Effects for Seams {
    fn spawn(&mut self, handle: EffectHandle, spawn: &EffectSpawn) {
        if spawn.persistent {
            self.effects.push((handle.0, spawn.name));
            self.started_effects += 1;
        }
    }
    fn stop(&mut self, handle: EffectHandle, _immediate: bool) {
        self.effects.retain(|s| s.0 != handle.0);
    }
}

struct Rig {
    world: TestWorld,
    party: Party,
    audio: Seams,
    effects: Seams,
    targets: kh_core::host::NullTargets,
}

impl Rig {
    fn new() -> (Rig, kh_party::MemberId) {
        let mut party = Party::new(PartyConfig::test_level99());
        let g = party.spawn(Chara::Goofy, Vec3::ZERO, 0.0);
        (Rig { world: TestWorld::floor(), party, audio: Seams::default(), effects: Seams::default(), targets: Default::default() }, g)
    }

    fn step(&mut self, n: usize) {
        let sora = SoraView { position: Vec3::new(-400.0, 0.0, 0.0), ..SoraView::default() };
        for _ in 0..n {
            let mut host = PartyHost { world: &self.world, targets: &mut self.targets, audio: &mut self.audio, effects: &mut self.effects };
            self.party.step(DT, &sora, &mut host);
        }
    }

    fn running(&self) -> (usize, usize) {
        (self.audio.sounds.len(), self.effects.effects.len())
    }
}

#[test]
fn tornados_loop_sound_and_whirlwind_stop_when_it_ends() {
    let (mut rig, g) = Rig::new();
    rig.step(5);
    rig.party.command(g, MemberCommand::Action { def: ActionDef::GoofyTornado, target: TargetRef::None });
    let mut saw_loop = false;
    let mut last = -1.0f32;
    for _ in 0..600 {
        rig.step(1);
        if rig.party.view(g).unwrap().anim.clip == "M_1322" {
            saw_loop |= rig.running().0 > 0 && rig.running().1 > 0;
            // M_1322 loops (PlayAnim mode 1): its pose never stands on the last frame
            let a = rig.party.view(g).unwrap().anim;
            assert!(a.time != last, "M_1322 wraps: its clock stood at {}", a.time);
            last = a.time;
        }
        if rig.party.view(g).unwrap().state == NpcStateId::Idle && saw_loop {
            break;
        }
    }
    assert!(saw_loop, "Tornado's M_1322 states started the loop sound (se00302_015) and vn_ex002_trn_loc00");
    rig.step(2);
    assert_eq!(rig.running(), (0, 0), "nothing of the whirlwind runs on: sounds {:?} effects {:?}", rig.audio.sounds, rig.effects.effects);
}

#[test]
fn a_ko_during_the_whirlwind_stops_it_too() {
    let (mut rig, g) = Rig::new();
    rig.step(5);
    rig.party.command(g, MemberCommand::Action { def: ActionDef::GoofyTornado, target: TargetRef::None });
    for _ in 0..200 {
        rig.step(1);
        if rig.party.view(g).unwrap().anim.clip == "M_1322" && rig.running().1 > 0 {
            break;
        }
    }
    assert!(rig.running().1 > 0, "the whirlwind runs");
    let mut hit = HitRecord::empty(TargetId(99));
    hit.damage = 9999.0;
    hit.damage_kind = kh_core::enums::ETresDamageKind::Small;
    assert!(rig.party.receive_hit(g, &hit).killed);
    rig.step(2);
    assert_eq!(rig.running(), (0, 0), "KO: sounds {:?} effects {:?}", rig.audio.sounds, rig.effects.effects);
}

#[test]
fn removing_goofy_mid_turbo_stops_its_loops() {
    let (mut rig, g) = Rig::new();
    rig.step(5);
    rig.party.command(g, MemberCommand::Action { def: ActionDef::GoofyTurbo, target: TargetRef::None });
    for _ in 0..200 {
        rig.step(1);
        if rig.party.view(g).unwrap().anim.clip == "M_1325" && rig.running().1 > 0 {
            break;
        }
    }
    assert!(rig.running().1 > 0, "Turbo's M_1325 effects run");
    rig.party.remove(g);
    rig.step(1);
    assert_eq!(rig.running(), (0, 0), "removed: sounds {:?} effects {:?}", rig.audio.sounds, rig.effects.effects);
}

#[test]
fn turbos_slide_effects_stop_at_its_end_clip() {
    let (mut rig, g) = Rig::new();
    rig.step(5);
    rig.party.command(g, MemberCommand::Action { def: ActionDef::GoofyTurbo, target: TargetRef::None });
    let mut started = false;
    for _ in 0..900 {
        rig.step(1);
        started |= rig.running().1 > 0;
        if started && rig.party.view(g).unwrap().state == NpcStateId::Idle {
            break;
        }
    }
    assert!(started);
    assert_eq!(rig.party.view(g).unwrap().state, NpcStateId::Idle, "Turbo ended");
    rig.step(2);
    assert_eq!(rig.running(), (0, 0), "sounds {:?} effects {:?}", rig.audio.sounds, rig.effects.effects);
}
