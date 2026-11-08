//! (MIMIC-FX) THE FRIEND COPIES' EFFECTS AND SOUNDS ARE THEIRS (copyfx.rs, declared there for tests): the boss's Donald copy
//! casting Zettaflare and the boss's Goofy copy using his Arts / Breaker, each a runtime whose member ids are the real party's,
//! through `FriendCopyFx` / `FriendCopyAudio`: every effect and sound in the copy's OWN range, every attach on him on HIS
//! member id, his effects spawned on HIS place by the effect side (the real Donald / Goofy elsewhere), his sounds placed on HIS
//! taps; the leader he hits keeps his own id. The AI pack at build/sora_pack/party/ai and the effect pack (skipped without).

use std::path::PathBuf;

use glam::Vec3;
use kh_core::host::party::{handle_member, PartyMember};
use kh_core::host::{HitRecord, HitResult, HurtShape, TargetId, TargetInfo};
use kh_core::test_world::TestWorld;
use kh_core::{Attach, Input, Pose, Sora, SORA_PARAMS};
use kh_host::formation::Formation;
use kh_host::party::PartyChara;
use kh_host::seam::{AudioCommand, AudioQueue, EffectCommand, EffectQueue};
use kh_host::vfx::{EffectHost, PartyMemberFx, Scene};
use kh_partyhost::mimicboss::{self as mb, BossTargets, FriendBoss, FriendBossStep, Victim};
use kh_partyhost::{PartyRuntime, PartyTargets, PartyTest, RouteIn, RuntimeConfig, StepIn};

use super::*;
use kh_audio::copies::{DONALD, GOOFY};

/// The real party member id of a friend (contract ER.1: Donald 0, Goofy 1; fx_party `chara_of_member`).
fn member_of_chara(c: PartyChara) -> Option<u8> {
    match c {
        PartyChara::Donald => Some(0),
        PartyChara::Goofy => Some(1),
        _ => None,
    }
}

fn project() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

struct NoEnemies;

impl kh_core::Targets for NoEnemies {
    fn enumerate(&self, _out: &mut Vec<TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
    fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
        HitResult::default()
    }
    fn hit_bone(&self, _id: TargetId, _at: Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

impl PartyTargets for NoEnemies {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

fn camera() -> kh_camera::CameraView {
    kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false }
}

fn runtime() -> Option<PartyRuntime> {
    let pack = project().join("build/sora_pack/party/ai");
    let ai_pack = kh_ai::AiPack::load_dir(&pack).ok().map(std::sync::Arc::new);
    if ai_pack.is_none() {
        eprintln!("SKIPPED: no AI pack at build/sora_pack/party/ai");
        return None;
    }
    Some(PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack, ai_pack_source: None }))
}

fn party_step(rt: &mut PartyRuntime, sora: &mut Sora, world: &TestWorld, asked: Option<Formation>) {
    let cam = camera();
    let mut targets = NoEnemies;
    let _ = rt.route(RouteIn { sora, input: &Input::default(), dt: 1.0 / 60.0, targets: &targets, camera: &cam, asked_formation: asked, asked_ai: Vec::new(), nav: None, now: 0.0 });
    let out = sora.output();
    let (mut audio, mut effects) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects);
    rt.step(StepIn { dt: 1.0 / 60.0, sora, out: &out, lock_on: TargetId::NONE, camera: &cam, world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: 0.0 });
    let _ = rt.after_step(&NoEnemies, 1.0 / 60.0, 0.0);
}

/// The real party (Sora, Donald, Goofy) with `leader` leading at level 99, the solo fight begun, his boss copy at -7 m.
fn friend_fight(leader: PartyChara) -> Option<(PartyRuntime, Sora, FriendBoss)> {
    let mut rt = runtime()?;
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::ZERO, yaw: 0.0 });
    let world = TestWorld::floor();
    party_step(&mut rt, &mut sora, &world, None);
    party_step(&mut rt, &mut sora, &world, Some(Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], leader).unwrap()));
    for _ in 0..3 {
        party_step(&mut rt, &mut sora, &world, None);
    }
    let id = rt.id(leader).unwrap();
    let chara = if leader == PartyChara::Donald { kh_party::Chara::Donald } else { kh_party::Chara::Goofy };
    *rt.party.status_mut(id).unwrap() = kh_party::status::FriendStatus::test_level99(chara);
    assert!(rt.solo_begin(&mut sora, "Mimic Tear", 0.0));
    let boss = FriendBoss::new(&rt, &sora, leader, Vec3::new(-700.0, 0.0, 0.0), 0.0).expect("the copy");
    Some((rt, sora, boss))
}

fn victim_of_member(v: &kh_party::MemberView, defense: i32) -> Victim {
    let c = kh_partyhost::party_chara(v.chara);
    Victim { chara: c, feet: v.position, yaw: v.yaw, velocity: v.velocity, radius: v.capsule.0.max(30.0), half_height: v.capsule.1.max(60.0), hp: v.hp, hp_max: v.hp_max, defense, airborne: !v.on_ground, down: v.ko, tell: Default::default(), stance: Default::default() }
}

/// What a friend copy's run left.
#[derive(Default)]
struct Seen {
    effects: Vec<EffectCommand>,
    sounds: Vec<AudioCommand>,
    /// Effects on his own member that the effect side started, and how far each stood from him / from the real friend.
    on_him: Vec<(String, f32, f32)>,
    names: Vec<String>,
    /// Sounds on him: where the party feed places them, and their distance from him.
    placed: Vec<f32>,
    /// His Zettaflare ran.
    zetta: bool,
    /// His Goofy AI words (Arts, Taunt, Interpose, Breaker ...) and the effects drawn on him while one plays (2 s after it).
    words: Vec<String>,
    art_fx: Vec<String>,
}

/// `steps` of the copy of `leader` (made ready for Zettaflare with `zetta`) against the real leader, his seams through the copy
/// route; the effect side with both the real friend (his own member id) and the copy (the copy's) in its party.
fn friend_run(leader: PartyChara, member: u8, steps: usize, zetta_at: Option<usize>) -> Option<Seen> {
    let pack = project().join("build").join("sora_pack");
    if !pack.join("vfx").join("effects").is_dir() {
        eprintln!("SKIPPED: no effect pack");
        return None;
    }
    let (mut rt, mut sora, mut boss) = friend_fight(leader)?;
    let mut host = EffectHost::load(&pack).expect("the effect pack");
    let _ = host.load_party_root(&pack);
    let world = TestWorld::floor();
    let cam = camera();
    let dt = 1.0 / 60.0;
    let own = boss.id().map(|id| id.0);
    let real_id = rt.id(leader).map(|id| id.0).expect("the real leader");
    let mut seen = Seen::default();
    let mut art_until = 0usize;
    let mut party = crate::fx::party::PartyFx::default();
    for k in 0..steps {
        // (as MIMIC-BOSS-KIT's test: the requirement met ten seconds into the fight)
        if zetta_at == Some(k) {
            assert_eq!(kh_party::donald::zettaflare::debug_make_ready(&mut boss.rt.party), 1);
        }
        seen.zetta |= boss.rt.party.don_zetta.runs.iter().any(|r| r.kind == kh_party::donald::zettaflare::RunKind::Zetta);
        // (a Goofy copy) his word of this step: an Art / Taunt / Interpose / Breaker opens a 2 s window
        if let Some(w) = boss.id().and_then(|id| boss.rt.party.gof.ai.iter().find(|m| m.member == id)).and_then(|m| m.last) {
            let text = format!("{w:?}");
            if seen.words.last() != Some(&text) {
                if matches!(w, kh_party::goofy::ai::AiWord::Art(_) | kh_party::goofy::ai::AiWord::Breaker | kh_party::goofy::ai::AiWord::Taunt | kh_party::goofy::ai::AiWord::Interpose) {
                    art_until = k + 120;
                }
                seen.words.push(text);
            }
        }
        let lead_id = rt.id(leader).unwrap();
        // the leader stands through it (his KO would end the copy's fight before its Zettaflare)
        if let Some(s) = rt.party.status_mut(lead_id) {
            s.hp = s.max_hp();
        }
        let lv = rt.current.iter().find(|v| v.id == lead_id).copied().unwrap();
        let mut t = BossTargets { victims: vec![victim_of_member(&lv, rt.party.status(lead_id).unwrap().defense())], hits: Vec::new(), attack_rate: 1.0, orders: Vec::new() };
        // mimic_boss.rs's retarget: a victim is the real one (the player Sora, a friend leader's own member id)
        let victims: Vec<(PartyChara, Vec3)> = t.victims.iter().map(|v| (v.chara, v.feet)).collect();
        let retarget = move |a: Attach| -> Attach {
            match a {
                Attach::Target { target, offset } => match mb::victim_chara(target) {
                    Some(PartyChara::Sora) => Attach::Sora { bone: None, offset },
                    Some(ch) => match member_of_chara(ch) {
                        Some(m) => Attach::Party { member: PartyMember(m), bone: None, offset },
                        None => victims.iter().find(|v| v.0 == ch).map_or(a, |v| Attach::World(v.1 + offset)),
                    },
                    None => a,
                },
                other => other,
            }
        };
        let (mut eq, mut aq) = (EffectQueue::default(), AudioQueue::default());
        let stepped = {
            let mut fxw = FriendCopyFx { inner: &mut eq, member, own, retarget: Some(&retarget) };
            let mut aw = FriendCopyAudio { inner: &mut aq, member, own, retarget: Some(&retarget) };
            boss.step(FriendBossStep { dt, world: &world, targets: &mut t, audio: &mut aw, effects: &mut fxw, camera: &cam, nav: None, now: k as f64 * f64::from(dt) })
        };
        for h in t.hits.drain(..) {
            let _ = rt.receive_hit(leader, &h);
        }
        party_step(&mut rt, &mut sora, &world, None);
        // the party feed: the real friend (his own id) and the copy (his), as fx.rs `set_party` gives them
        let copy_view = boss.view().expect("the copy's view");
        let tap = |m: u8, v: &kh_party::MemberView| -> crate::fx::party::PartyTap { (m, k as u64 + 1, v.position, v.yaw, std::collections::HashMap::new(), true, (String::new(), 0.0), String::new()) };
        let projectiles: Vec<(u32, Vec3, Vec3)> = stepped.projectiles.iter().map(|p| (own_handle(member, p.effect), p.position, p.forward)).collect();
        party.feed(vec![tap(real_id, &lv), tap(member, &copy_view)], projectiles.clone(), true);
        host.set_party(vec![(real_id, PartyMemberFx { position: lv.position, yaw: lv.yaw, ..Default::default() }), (member, PartyMemberFx { position: copy_view.position, yaw: copy_view.yaw, ..Default::default() })], projectiles);
        let out = sora.output();
        let tap0 = kh_host::audio::PoseTap::default();
        let none = |_: TargetId| None;
        host.step(dt, &eq.0, &Scene { out: &out, sora: &sora, tap: &tap0, target: &none, ground: Some(0.0), eye: cam.position });
        // where the effect side put his own ones, and where his sounds are placed
        for c in &eq.0 {
            if let EffectCommand::Spawn(h, s) = c {
                if let (Attach::Party { member: m, .. }, true) = (s.at, host.drawn_of(*h) > 0) {
                    if m.0 == member {
                        let at = host.places_of(h.0);
                        for p in at {
                            seen.on_him.push((s.name.to_string(), p.distance(copy_view.position), p.distance(lv.position)));
                        }
                    }
                }
                if k < art_until && host.drawn_of(*h) > 0 && !seen.art_fx.contains(&s.name.to_string()) {
                    seen.art_fx.push(s.name.to_string());
                }
                if !seen.names.contains(&s.name.to_string()) && host.drawn_of(*h) > 0 {
                    seen.names.push(s.name.to_string());
                }
            }
        }
        for c in &aq.0 {
            if let AudioCommand::Play(_, p) = c {
                if let Attach::Party { member: m, .. } = p.at {
                    if m.0 == member {
                        seen.placed.push(party.place(&p.at).map_or(f32::MAX, |x| x.distance(copy_view.position)));
                    }
                }
            }
        }
        seen.effects.extend(eq.0);
        seen.sounds.extend(aq.0);
    }
    Some(seen)
}

/// Every effect and sound of the copy is in his range; nothing of his sits on the real friend's id except as a victim.
fn assert_his_own(who: &str, seen: &Seen, member: u8, real: u8) {
    let handle_of = |c: &EffectCommand| match c {
        EffectCommand::Spawn(h, _) | EffectCommand::Stop(h, _) | EffectCommand::Tint(h, _) | EffectCommand::Color(h, _) | EffectCommand::Place(h, _) | EffectCommand::Transform(h, ..) | EffectCommand::Timing(h, _) => h.0,
    };
    for c in &seen.effects {
        assert_eq!(handle_member(handle_of(c)).map(|m| m.0), Some(member), "{who}: {c:?} in his own range");
    }
    for c in &seen.sounds {
        if let AudioCommand::Play(h, p) = c {
            if h.0 != 0 {
                assert_eq!(handle_member(h.0).map(|m| m.0), Some(member), "{who}: {p:?} in his own range");
            }
            if let Attach::Party { member: m, .. } = p.at {
                assert!(m.0 == member || m.0 == real, "{who}: {p:?}");
            }
        }
    }
    assert!(!seen.on_him.is_empty(), "{who}: effects on him were started");
    for (name, from_him, from_real) in &seen.on_him {
        assert!(from_him < from_real, "{who}: {name} stands {from_him:.0} cm from him, {from_real:.0} cm from the real one");
        assert!(*from_him < 400.0, "{who}: {name} on him ({from_him:.0} cm)");
    }
    assert!(!seen.placed.is_empty() && seen.placed.iter().all(|d| *d < 250.0), "{who}: his sounds on him: {:?}", seen.placed);
}

#[test]
fn mimic_fx_the_boss_donalds_zettaflare_and_spells_are_on_him() {
    let Some(seen) = friend_run(PartyChara::Donald, DONALD, 60 * 50, Some(600)) else { return };
    eprintln!("Donald's copy: {} effect commands, {} sounds; drawn: {:?}", seen.effects.len(), seen.sounds.len(), seen.names);
    assert_his_own("Donald's copy", &seen, DONALD, 0);
    assert!(seen.zetta, "his Zettaflare ran");
    assert!(seen.names.iter().any(|n| n.to_lowercase().starts_with("vd_zetta_") || n.to_lowercase().starts_with("vd_kg702")), "his Zettaflare's effects drawn: {:?}", seen.names);
}

#[test]
fn mimic_fx_the_boss_goofys_arts_and_breaker_are_on_him() {
    let Some(seen) = friend_run(PartyChara::Goofy, GOOFY, 60 * 45, None) else { return };
    eprintln!("Goofy's copy: {} effect commands, {} sounds; drawn: {:?}; his words {:?}; drawn in his Arts: {:?}", seen.effects.len(), seen.sounds.len(), seen.names, seen.words, seen.art_fx);
    assert_his_own("Goofy's copy", &seen, GOOFY, 1);
    assert!(seen.words.iter().any(|w| w.starts_with("Art(")), "his Arts: {:?}", seen.words);
    assert!(!seen.art_fx.is_empty(), "his Art's effects drawn (on him: assert_his_own)");
}
