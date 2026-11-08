//! (MIMIC-STANCE) A friend's stance break (kh-core combat/stance.rs, the route Sora's core shares; THE USER 2026-11-05: "make
//! sure Goofy's mimic tear boss ... can stagger you by breaking your stance"): KH3's TresNpcState_BadStatStun with his own set's
//! BadStatStunStart / Loop / End for the window, no action (pad or AI), a hit's reaction does not end it, the release does,
//! Idle after. Every playable friend, the leader on the pad and the AI alike.

use glam::{Vec2, Vec3};
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{HitAttacker, HitRecord, HitResult, HurtShape, TargetId, TargetInfo, Targets};
use kh_core::input::{PadButton, PadButtons, PadState};
use kh_core::test_world::TestWorld;
use kh_party::{Chara, MemberId, NpcStateId, Party, PartyConfig, PartyHost, SoraView};

const DT: f32 = 1.0 / 60.0;

struct Nobody;

impl Targets for Nobody {
    fn enumerate(&self, _out: &mut Vec<TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
    fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
        HitResult::default()
    }
    fn hit_bone(&self, _id: kh_core::TargetId, _at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: kh_core::TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

struct Rig {
    world: TestWorld,
    targets: Nobody,
    audio: kh_core::host::audio::NullAudio,
    effects: kh_core::host::effects::NullEffects,
    party: Party,
    sora: SoraView,
    held: PadButtons,
    id: MemberId,
    pad: bool,
}

impl Rig {
    fn new(chara: Chara, pad: bool) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(chara, Vec3::ZERO, 0.0);
        if pad {
            party.set_pad(id, Some(kh_core::Tuning::USER));
        }
        let mut rig = Rig { world: TestWorld::floor(), targets: Nobody, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), held: PadButtons::NONE, id, pad };
        rig.sora.position = Vec3::new(-300.0, 0.0, 0.0);
        rig.step(&[]);
        for _ in 0..30 {
            rig.step(&[]);
        }
        rig
    }

    fn step(&mut self, held: &[PadButton]) -> NpcStateId {
        if self.pad {
            let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
            let pad = PadState { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
            self.held = now;
            self.party.set_pad_frame(self.id, kh_party::pad::PadFrame { stick: Vec2::ZERO, camera_yaw: 0.0, pad });
        }
        let mut host = PartyHost { world: &self.world, targets: &mut self.targets, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host);
        self.party.member(self.id).unwrap().state
    }

    fn clip(&self) -> &'static str {
        self.party.member(self.id).unwrap().anim.clip.name
    }
}

fn stun_clips(chara: Chara) -> [&'static str; 3] {
    let set = kh_party::tables::anim_set(chara);
    let pick = |a: &'static str, b: &'static str| if set.get(a).is_some() { a } else { b };
    [pick("BadStatStunStart", "M_0241"), pick("BadStatStunLoop", "M_0242"), pick("BadStatStunEnd", "M_0243")]
}

#[test]
fn stance_a_friend_is_held_in_kh3s_stun_for_the_window_and_recovers_pad_or_ai() {
    for chara in [Chara::Goofy, Chara::Donald] {
        for pad in [true, false] {
            let mut rig = Rig::new(chara, pad);
            let who = format!("{chara:?} {}", if pad { "on the pad" } else { "AI" });
            assert!(rig.party.member_mut(rig.id).unwrap().stance_break(2.5), "{who}");
            assert_eq!(rig.step(&[]), NpcStateId::BadStatStun, "{who}: the stun state at once");
            let clips = stun_clips(chara);
            let mut seen = vec![rig.clip()];
            let mut steps = 1;
            // the pad's every button does nothing meanwhile
            let buttons = [PadButton::FaceBottom, PadButton::FaceLeft, PadButton::FaceTop, PadButton::FaceRight];
            while rig.party.member(rig.id).unwrap().stance.broken() {
                let s = rig.step(&[buttons[steps % 4]]);
                assert_eq!(s, NpcStateId::BadStatStun, "{who}: no action while broken (step {steps})");
                if seen.last() != Some(&rig.clip()) {
                    seen.push(rig.clip());
                }
                steps += 1;
                assert!(steps < 400, "{who}: the break never ended");
            }
            assert!((148..=152).contains(&steps), "{who}: broken {steps} steps (2.5 s)");
            for _ in 0..200 {
                if rig.step(&[]) != NpcStateId::BadStatStun {
                    break;
                }
                if seen.last() != Some(&rig.clip()) {
                    seen.push(rig.clip());
                }
            }
            assert_ne!(rig.party.member(rig.id).unwrap().state, NpcStateId::BadStatStun, "{who}: up again");
            assert_eq!(seen[..3], clips, "{who}: KH3's stun reaction of his own set");
            assert_eq!(rig.party.member(rig.id).unwrap().bad.0, 0, "{who}: no status on him");
        }
    }
}

#[test]
fn stance_a_hit_does_not_end_a_friends_break_the_release_does() {
    let mut rig = Rig::new(Chara::Goofy, false);
    assert!(rig.party.member_mut(rig.id).unwrap().stance_break(2.5));
    for _ in 0..20 {
        rig.step(&[]);
    }
    let at = rig.party.member(rig.id).unwrap().position();
    let hit = HitRecord { position: at, direction: Vec3::X, damage: 3.0, damage_kind: Kind::Small, attack_name: "test.blow", attacker: HitAttacker { position: at - Vec3::X * 150.0, yaw: 0.0, radius: 40.0, valid: true }, ..HitRecord::empty(TargetId::NONE) };
    let _ = rig.party.receive_hit(rig.id, &hit);
    assert_eq!(rig.party.member(rig.id).unwrap().state, NpcStateId::Damage, "the hit's reaction plays");
    let mut back = false;
    for _ in 0..120 {
        back |= rig.step(&[]) == NpcStateId::BadStatStun;
        if back {
            break;
        }
    }
    assert!(back && rig.party.member(rig.id).unwrap().stance.broken(), "the break goes on after the reaction");
    rig.party.member_mut(rig.id).unwrap().stance_hold(3.0);
    assert!((rig.party.member(rig.id).unwrap().stance.left - 3.0).abs() < 1e-4, "a Breaker's hold");
    assert!(rig.party.member_mut(rig.id).unwrap().stance_release());
    // (the start clip plays out, then the end clip: KH3's state)
    let mut up = false;
    let mut trace = Vec::new();
    for k in 0..400 {
        let s = rig.step(&[]);
        if trace.last().map(|t: &(NpcStateId, &str, usize)| (t.0, t.1)) != Some((s, rig.clip())) {
            trace.push((s, rig.clip(), k));
        }
        if s != NpcStateId::BadStatStun {
            up = true;
            break;
        }
    }
    assert!(up, "up after the release: {trace:?}");
    // a knocked-out friend is never broken
    rig.party.member_mut(rig.id).unwrap().status.hp = 0;
    assert!(!rig.party.member_mut(rig.id).unwrap().stance_break(2.5));
}
