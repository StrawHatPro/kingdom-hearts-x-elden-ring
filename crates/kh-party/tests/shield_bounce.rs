//! (THE USER 2026-11-02: "fix 'Shields don't count as guarding.' and in fact make the character who attacks a shield bounce
//! off playing their animation from KH3. This means Sora, Donald, Goofy and whoever we add in the future") The friends' side
//! (kh-party repel.rs): a host's raised shield (`TargetBody::shield`) guards a friend's swing from its front and bounces him
//! into KH3's TresNpcState_Reflect with HIS OWN set's AttackReflect, by the same classification as Sora's (kh-core
//! tests/shield_bounce.rs); from behind it is a plain hit; a body that does not guard is hit as before. The AI's clip
//! attacks (`actions::deliver`) and the pad kit's shapes (`goofy_shots::step_areas`) alike.

use glam::{Vec2, Vec3};
use kh_core::host::{HitRecord, HitResult, HurtShape, ReactionResult, Shield, TargetBody, TargetDefense, TargetFlags, TargetId, TargetInfo, Targets};
use kh_core::input::{PadButton, PadButtons, PadState};
use kh_core::test_world::TestWorld;
use kh_party::{ActionDef, Chara, MemberCommand, MemberId, NpcStateId, Party, PartyConfig, PartyHost, SoraView, TargetRef};

const DT: f32 = 1.0 / 60.0;

/// One dummy turned toward -X (yaw 180), with or without a raised shield facing `shield`.
struct Dummy {
    at: Vec3,
    shield: Option<i16>,
    hits: Vec<HitRecord>,
}

impl Dummy {
    fn info_of(&self) -> TargetInfo {
        TargetInfo { id: TargetId(1), position: self.at, center: self.at + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 10_000.0, hp_max: 10_000.0 }
    }
}

impl Targets for Dummy {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.push(self.info_of());
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        (id == TargetId(1)).then(|| self.info_of())
    }
    fn hurt_shapes(&self, _id: TargetId, out: &mut Vec<HurtShape>) {
        out.push(HurtShape { a: self.at + Vec3::Z * 40.0, b: self.at + Vec3::Z * 140.0, radius: 50.0, part: 0 });
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.hits.push(*hit);
        // a host's guard: no HP (KH3's TakeDamage rule for GUARD), the result says guarded
        let taken = if hit.takes_hp() { hit.damage } else { 0.0 };
        HitResult { accepted: true, damage_dealt: taken, hp_after: 10_000.0 - taken, killed: false, guarded: !hit.takes_hp() }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: 10, max_hp: 10_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
    }
    fn body(&self, _id: TargetId, _part: u32) -> TargetBody {
        match self.shield {
            Some(yaw) => TargetBody { guarding: true, shield: Some(Shield { yaw, half_arc: 90 }), ..TargetBody::DEFAULT },
            None => TargetBody::DEFAULT,
        }
    }
    // (AUDIT-B) no bones here: a rider attached to a hit follows the target's origin and yaw
    fn hit_bone(&self, _id: kh_core::TargetId, _at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: kh_core::TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

struct Rig {
    world: TestWorld,
    dummy: Dummy,
    audio: kh_core::host::audio::NullAudio,
    effects: kh_core::host::effects::NullEffects,
    party: Party,
    sora: SoraView,
    held: PadButtons,
}

impl Rig {
    fn new(chara: Chara, at: Vec3, shield: Option<i16>) -> (Rig, MemberId) {
        let mut party = Party::new(PartyConfig::test_level99());
        party.don_prog.kh3_ai = true; // KH3's AI Donald (his Beat) and Goofy (his Push), as tests/party.rs
        party.gof.kh3_ai = true;
        let id = party.spawn(chara, Vec3::ZERO, 0.0);
        let mut rig = Rig { world: TestWorld::floor(), dummy: Dummy { at, shield, hits: Vec::new() }, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), held: PadButtons::NONE };
        rig.sora.position = Vec3::new(-3000.0, 0.0, 0.0);
        rig.step(2);
        (rig, id)
    }

    fn step(&mut self, n: usize) {
        for _ in 0..n {
            let mut host = PartyHost { world: &self.world, targets: &mut self.dummy, audio: &mut self.audio, effects: &mut self.effects };
            self.party.step(DT, &self.sora, &mut host);
        }
    }

    fn pad_step(&mut self, id: MemberId, held: &[PadButton]) {
        let mut now = PadButtons::NONE;
        for b in held {
            now.set(*b, true);
        }
        let pad = PadState { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
        self.held = now;
        self.party.set_pad_frame(id, kh_party::pad::PadFrame { stick: Vec2::ZERO, camera_yaw: 0.0, pad });
        self.step(1);
    }

    /// Runs until he is in Reflect (or `steps` pass): the clip he plays there, where he stood when it began.
    fn bounce(&mut self, id: MemberId, steps: usize, mut drive: impl FnMut(&mut Rig, usize)) -> Option<(&'static str, Vec3)> {
        for i in 0..steps {
            drive(self, i);
            let v = self.party.view(id).unwrap();
            if v.state == NpcStateId::Reflect {
                return Some((v.anim.clip, v.position));
            }
        }
        None
    }
}

fn attack(chara: Chara) -> ActionDef {
    if chara == Chara::Donald { ActionDef::DonaldBeat } else { ActionDef::GoofyPush }
}

#[test]
fn a_shield_from_the_front_bounces_ai_donald_and_goofy_with_their_own_attack_reflect() {
    for chara in [Chara::Donald, Chara::Goofy] {
        let (mut rig, id) = Rig::new(chara, Vec3::new(140.0, 0.0, 0.0), Some(180));
        rig.party.command(id, MemberCommand::Action { def: attack(chara), target: TargetRef::Enemy(TargetId(1)) });
        let (clip, at) = rig.bounce(id, 360, |r, _| r.step(1)).unwrap_or_else(|| panic!("{chara:?} did not bounce off the shield: {:?}", r_hits(&rig)));
        assert_eq!(clip, "AttackReflect", "{chara:?}: his own set's KH3 recoil clip (sheet row repel.clip)");
        let h = rig.dummy.hits.last().unwrap();
        assert_eq!((h.info.victim_result, h.info.attacker_result), (ReactionResult::Guard, ReactionResult::Reflect), "{chara:?}");
        // the impact's hold, then knocked back away from the shield, and back to his idle at the clip's end
        rig.step(150);
        let v = rig.party.view(id).unwrap();
        assert!(v.position.x < at.x - 20.0, "{chara:?}: knocked back from the shield {} -> {}", at.x, v.position.x);
        assert_ne!(v.state, NpcStateId::Reflect, "{chara:?}: the Reflect ended");
    }
}

fn r_hits(rig: &Rig) -> Vec<(&'static str, ReactionResult, ReactionResult)> {
    rig.dummy.hits.iter().map(|h| (h.attack_name, h.info.victim_result, h.info.attacker_result)).collect()
}

#[test]
fn a_shield_turned_away_is_a_plain_hit_and_an_open_body_is_unchanged() {
    for chara in [Chara::Donald, Chara::Goofy] {
        for shield in [Some(0), None] {
            let (mut rig, id) = Rig::new(chara, Vec3::new(140.0, 0.0, 0.0), shield);
            rig.party.command(id, MemberCommand::Action { def: attack(chara), target: TargetRef::Enemy(TargetId(1)) });
            assert!(rig.bounce(id, 240, |r, _| r.step(1)).is_none(), "{chara:?} {shield:?}: no bounce");
            let h = rig.dummy.hits.first().unwrap_or_else(|| panic!("{chara:?} {shield:?}: no hit"));
            assert_eq!((h.info.victim_result, h.info.attacker_result), (ReactionResult::Damage, ReactionResult::NoReaction), "{chara:?} {shield:?}");
        }
    }
}

#[test]
fn the_pad_goofys_shove_bounces_off_a_shield_by_the_same_path() {
    // the kit's hit shapes (goofy_shots `step_areas`: a shape riding his body) go through the same classification and
    // repel.rs `after_swing`; Goofy at the origin faces +X, the shove's shape reaches the dummy
    let (mut rig, id) = Rig::new(Chara::Goofy, Vec3::new(170.0, 0.0, 0.0), Some(180));
    rig.party.set_pad(id, Some(kh_core::Tuning::USER));
    rig.step(10);
    let bounced = rig.bounce(id, 240, |r, i| r.pad_step(id, if i % 8 == 0 { &[PadButton::FaceBottom] } else { &[] }));
    let (clip, _) = bounced.unwrap_or_else(|| panic!("no bounce: {:?}", r_hits(&rig)));
    assert_eq!(clip, "AttackReflect");
    assert!(rig.dummy.hits.iter().any(|h| h.info.victim_result == ReactionResult::Guard && h.info.attacker_result == ReactionResult::Reflect), "{:?}", r_hits(&rig));
}
