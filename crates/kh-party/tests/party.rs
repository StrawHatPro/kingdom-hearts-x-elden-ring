//! Stream B acceptance tests (docs/party-port-plan.md stream B): moves, spells, specials,
//! damage, KO / revive, items, warp and progression against a flat floor and training dummies.

use glam::Vec3;
use kh_core::host::{HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetId, TargetInfo, Targets};
use kh_core::status::{compute_damage, Attacker, DamageAttack, DamageContext, Defender, GameLevel};
use kh_core::test_world::TestWorld;
use kh_party::{ActionDef, Chara, CommandStatus, ItemKind, LocomotionDef, MemberCommand, MoveGoal, NpcStateId, Party, PartyConfig, PartyEventKind, PartyHost, SoraView, TargetRef};

const DT: f32 = 1.0 / 60.0;

/// Training dummies with a KH3-like status (Defense, no cap).
struct Dummies {
    list: Vec<(TargetId, Vec3, f32)>,
    hits: Vec<HitRecord>,
    defense: i32,
    guard: bool,
}

impl Dummies {
    fn one(at: Vec3) -> Dummies {
        Dummies { list: vec![(TargetId(1), at, 10_000.0)], hits: Vec::new(), defense: 10, guard: false }
    }
    fn info_of(&self, (id, p, hp): (TargetId, Vec3, f32)) -> TargetInfo {
        TargetInfo { id, position: p, center: p + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp, hp_max: 10_000.0 }
    }
}

impl Targets for Dummies {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        for &t in &self.list {
            out.push(self.info_of(t));
        }
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.list.iter().find(|t| t.0 == id).map(|&t| self.info_of(t))
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(i) = self.info(id) {
            out.push(HurtShape { a: i.position + Vec3::Z * 50.0, b: i.position + Vec3::Z * 130.0, radius: 50.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        if self.guard {
            return HitResult { accepted: false, damage_dealt: 0.0, hp_after: 10_000.0, killed: false, guarded: true };
        }
        self.hits.push(*hit);
        let t = self.list.iter_mut().find(|t| t.0 == hit.target).unwrap();
        t.2 -= hit.damage;
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: t.2, killed: false, guarded: false }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: self.defense, max_hp: 10_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
    }
}

#[derive(Default)]
struct VoiceLog(Vec<(u8, i32)>, Vec<&'static str>);
impl kh_core::host::Audio for VoiceLog {
    fn play(&mut self, _h: kh_core::host::SoundHandle, p: &kh_core::host::SoundPlay) {
        self.1.push(p.name);
    }
    fn stop(&mut self, _h: kh_core::host::SoundHandle, _f: f32) {}
    fn party_voice_group_no(&mut self, m: kh_core::host::PartyMember, n: i32) {
        self.0.push((m.0, n));
    }
}

struct Rig {
    world: TestWorld,
    dummies: Dummies,
    audio: VoiceLog,
    effects: kh_core::host::effects::NullEffects,
    party: Party,
    sora: SoraView,
    events: Vec<PartyEventKind>,
    heals: Vec<i32>,
}

impl Rig {
    fn new(level99: bool, dummy: Option<Vec3>) -> Rig {
        let config = if level99 { PartyConfig::test_level99() } else { PartyConfig::default() };
        let mut party = Party::new(config);
        party.don_prog.kh3_ai = true; // (DON-PROG) these tests are KH3's AI Donald (his five spells, his melee); the overhaul replaces them in play
        party.spawn(Chara::Donald, Vec3::new(0.0, 180.0, 0.0), 0.0);
        party.spawn(Chara::Goofy, Vec3::new(0.0, -180.0, 0.0), 0.0);
        // (DON-PROG) KH3's AI Donald machinery as these tests know it: his five KH3 spell abilities (gone in play since the
        // overhaul replaced them; the gates fall back to KH3's own while he holds them)
        if let Some(s) = party.status_mut(kh_party::MemberId(0)) {
            for n in kh_party::donald::ability_info::KH3_SPELLS {
                s.abilities.set(kh_core::enums::ETresAbilityKind::from_game_name(n).unwrap(), 1, 1);
            }
        }
        let dummies = match dummy {
            Some(p) => Dummies::one(p),
            None => Dummies { list: Vec::new(), hits: Vec::new(), defense: 0, guard: false },
        };
        Rig { world: TestWorld::floor(), dummies, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), events: Vec::new(), heals: Vec::new() }
    }

    fn step(&mut self, n: usize) {
        for _ in 0..n {
            let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
            let out = self.party.step(DT, &self.sora, &mut host);
            self.events.extend(out.events.iter().map(|e| e.kind));
            self.heals.extend(out.sora_heals.iter().map(|h| h.amount));
        }
    }

    fn run_until_done(&mut self, id: kh_party::MemberId, t: kh_party::CommandTicket, max_s: f32) -> CommandStatus {
        for _ in 0..(max_s / DT) as usize {
            self.step(1);
            let s = self.party.command_status(id, t);
            if s != CommandStatus::Running {
                return s;
            }
        }
        CommandStatus::Running
    }

    fn hits(&self, attack: &str) -> Vec<&HitRecord> {
        self.dummies.hits.iter().filter(|h| h.attack_name == attack).collect()
    }
}

fn expected(rig: &Rig, id: kh_party::MemberId, row: &str, magic: bool, attr: kh_core::enums::ETresDamageAttribute, power: f32) -> f32 {
    let s = rig.party.status(id).unwrap();
    let _ = row;
    let att = if magic { DamageAttack::magic(power, attr) } else { DamageAttack::physical(power) };
    let def = Defender::of_target(Some(&TargetDefense { defense: 10, max_hp: 10_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 }));
    compute_damage(&s.as_attacker(), &def, &att, &DamageContext::new(GameLevel::Standard, 1)).amount
}

#[test]
fn donald_fire_hits_a_dummy_with_the_formula() {
    let mut rig = Rig::new(true, Some(Vec3::new(800.0, 180.0, 0.0)));
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    let mp0 = rig.party.status(d).unwrap().mp;
    let t = rig.party.command(d, MemberCommand::Action { def: ActionDef::DonaldFire, target: TargetRef::Enemy(TargetId(1)) });
    rig.run_until_done(d, t, 5.0);
    rig.step(120);
    assert_eq!(rig.party.status(d).unwrap().mp, mp0 - 10, "DONALD_FIRE m_UseMP 10");
    let hits = rig.hits("MAG_FIRE");
    assert_eq!(hits.len(), 1, "{:?}", rig.events);
    let want = expected(&rig, d, "MAG_FIRE", true, kh_core::enums::ETresDamageAttribute::Fire, 1.5);
    assert_eq!(hits[0].damage, want);
    // E.4: the cast sets the voice-group number (Donald 101..103) for the clip's MagicCast notify
    assert!(rig.audio.0.iter().any(|&(m, n)| m == d.0 && (101..=103).contains(&n)), "{:?}", rig.audio.0);
}

#[test]
fn donald_blizzard_thunder_aero_land() {
    for (def, row, at) in [(ActionDef::DonaldBlizzard, "MAG_BLIZZARD", 700.0), (ActionDef::DonaldThunder, "MAG_THUNDER", 900.0), (ActionDef::DonaldAero, "MAG_AERO", 150.0)] {
        let mut rig = Rig::new(true, Some(Vec3::new(at, 180.0, 0.0)));
        rig.step(2);
        let d = rig.party.find(Chara::Donald).unwrap();
        let t = rig.party.command(d, MemberCommand::Action { def, target: TargetRef::Enemy(TargetId(1)) });
        rig.run_until_done(d, t, 5.0);
        rig.step(90);
        assert!(!rig.hits(row).is_empty(), "{def:?} did not hit");
        assert!(rig.hits(row)[0].damage >= 1.0);
    }
}

#[test]
fn donald_cure_heals_sora_and_spends_all_mp() {
    let mut rig = Rig::new(true, None);
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    let t = rig.party.command(d, MemberCommand::Cure { target: TargetRef::Sora });
    assert_eq!(rig.run_until_done(d, t, 5.0), CommandStatus::Succeeded);
    // ATresProj_MagicCure 0x1414742A0: m_AppendCure 120 + MagicPower on the target
    let magic = rig.party.status(d).unwrap().magic();
    assert_eq!(rig.heals, vec![120 + magic]);
    assert_eq!(rig.audio.0, vec![(d.0, 1)], "Cure on Sora: target voice number 1 (0x1411C8ED0)");
    let s = rig.party.status(d).unwrap();
    assert_eq!(s.mp, 0);
    assert_eq!(s.mp_charge_left > 0.0, true);
    // a second spell is refused while MP charges
    let t2 = rig.party.command(d, MemberCommand::Action { def: ActionDef::DonaldFire, target: TargetRef::Sora });
    assert_eq!(rig.run_until_done(d, t2, 1.0), CommandStatus::Failed);
}

#[test]
fn melee_and_jump_attacks_hit() {
    for (chara, def, row) in [(Chara::Donald, ActionDef::DonaldBeat, "Attack1_Beat"), (Chara::Donald, ActionDef::DonaldSwing, "Attack2_Swing"), (Chara::Goofy, ActionDef::GoofyPush, "Attack1_Push"), (Chara::Goofy, ActionDef::GoofySweep, "Attack2_Sweep"), (Chara::Goofy, ActionDef::GoofyJump, "Attack3_JumpAscent")] {
        let y = if chara == Chara::Donald { 180.0 } else { -180.0 };
        let mut rig = Rig::new(true, Some(Vec3::new(140.0, y, 0.0)));
        rig.step(2);
        let m = rig.party.find(chara).unwrap();
        let t = rig.party.command(m, MemberCommand::Action { def, target: TargetRef::Enemy(TargetId(1)) });
        rig.run_until_done(m, t, 6.0);
        let any: Vec<_> = rig.dummies.hits.iter().map(|h| h.attack_name).collect();
        assert!(any.iter().any(|a| a.starts_with(&row[..9])), "{def:?}: {any:?}");
    }
}

/// UTresNpcAttackDefinitionJumpBase (Begin 0x141220750, Tick 0x141222A00, launch 0x14122B940 on the
/// JumpStart notify, attack start 0x14121DAE0, solver FUN_1405431C0).
#[test]
fn jump_attack_follows_jumpbase() {
    // the solver: same height, apex = m_JumpHeight, no fall: vz = sqrt(2 g h), t = vz / g
    let g = -980.0 * 1.5;
    let v = kh_party::actions::jump_velocity(Vec3::ZERO, Vec3::new(300.0, 0.0, 0.0), 0.0, 200.0, g, 1.0, 1.0, 0.0);
    assert!((v.z - (2.0f32 * 1470.0 * 200.0).sqrt()).abs() < 1e-3, "{v}");
    assert!((v.x - 300.0 / (v.z / 1470.0)).abs() < 1e-2, "{v}");
    // target 100 below: fall 100 adds sqrt(2 x 100 / g)
    let v2 = kh_party::actions::jump_velocity(Vec3::ZERO, Vec3::new(300.0, 0.0, -100.0), 0.0, 200.0, g, 1.0, 1.0, 0.0);
    let t = v2.z / 1470.0 + (200.0f32 / 1470.0).sqrt();
    assert!((v2.x - 300.0 / t).abs() < 1e-2, "{v2}");
    for chara in [Chara::Donald, Chara::Goofy] {
        let y = if chara == Chara::Donald { 180.0 } else { -180.0 };
        let mut rig = Rig::new(true, Some(Vec3::new(420.0, y, 0.0)));
        rig.step(2);
        let m = rig.party.find(chara).unwrap();
        let def = if chara == Chara::Donald { ActionDef::DonaldJump } else { ActionDef::GoofyJump };
        let t = rig.party.command(m, MemberCommand::Action { def, target: TargetRef::Enemy(TargetId(1)) });
        let mut clips: Vec<&str> = Vec::new();
        let (mut top, mut vz0) = (0.0f32, None);
        let mut decel = false;
        for _ in 0..(6.0 / DT) as usize {
            rig.step(1);
            let v = rig.party.view(m).unwrap();
            if clips.last() != Some(&v.anim.clip) {
                clips.push(v.anim.clip);
            }
            top = top.max(v.position.z);
            // (B4) the attack clip's TresAnimNotify_Timing (m_Param 0) -> +0x25A (0x14122DE40)
            decel |= rig.party.member(m).unwrap().action.is_some_and(|a| a.jump.decel);
            if vz0.is_none() && v.velocity.z > 0.0 {
                vz0 = Some(v.velocity.z);
            }
            if rig.party.command_status(m, t) != CommandStatus::Running {
                break;
            }
        }
        assert_eq!(rig.party.command_status(m, t), CommandStatus::Succeeded, "{chara:?} {clips:?}");
        assert_eq!(clips[0], "JumpNormalStart", "{clips:?}");
        assert!(clips.contains(&"M_1312") || clips.contains(&"M_1311"), "{chara:?} attack clip: {clips:?}");
        assert!(clips.contains(&"Land"), "{clips:?}");
        assert!(decel, "{chara:?}: the Timing notify of the attack clip turns the decel on");
        // the launch: DoJump with vz = sqrt(2 x |g| x JumpHeight 200) (pawn GravityScale 1.5, NPC m_JumpUpGravityScale 1)
        let want = (2.0f32 * 1470.0 * 200.0).sqrt();
        let got = vz0.expect("never rose");
        assert!((got - want).abs() < 30.0, "{chara:?} vz {got} want {want}");
        assert!(top > 150.0 && top < 230.0, "{chara:?} apex {top}");
        assert!(!rig.dummies.hits.is_empty(), "{chara:?} jump attack did not hit; {clips:?}");
        let v = rig.party.view(m).unwrap();
        assert!(v.on_ground);
    }
}

/// The shield's flight: ThrowWeapon 0x1411CD930 (m_fStartSpeed 2500 toward the target, homing), the shield's
/// TresWeaponMovementComponent (MaxSpeed 1500), the hit handler 0x1411CC550 (homing off, fade 0.35 s) and the
/// catch clip M_1329 taking it back (Tick 0x1411C7670); no hit -> the catch at m_fAttackTime 2 s.
#[test]
fn discus_flight_follows_the_throw_and_the_hit_handler() {
    for (at, hits) in [(Vec3::new(700.0, -180.0, 0.0), true), (Vec3::new(-3000.0, 3000.0, 0.0), false)] {
        let mut rig = Rig::new(true, Some(at));
        rig.step(2);
        let g = rig.party.find(Chara::Goofy).unwrap();
        let t = rig.party.command(g, MemberCommand::Action { def: ActionDef::GoofyDiscus, target: TargetRef::Enemy(TargetId(1)) });
        let (mut thrown_at, mut hit_at, mut catch_at, mut gone_at, mut loop_at) = (None, None, None, None, None);
        let mut first_speed = 0.0;
        for i in 0..(6.0 / DT) as usize {
            rig.step(1);
            let now = i as f32 * DT;
            if let Some(p) = rig.party.projectiles.first() {
                if thrown_at.is_none() {
                    thrown_at = Some(now);
                    first_speed = p.velocity.length();
                    let d = (rig.dummies.info(TargetId(1)).unwrap().center - p.position).normalize();
                    if hits {
                        assert!(p.velocity.normalize().dot(d) > 0.9, "thrown toward the target");
                        assert!(p.homing_on);
                    }
                }
                if hit_at.is_none() && !rig.dummies.hits.is_empty() {
                    hit_at = Some(now);
                    assert!(!p.homing_on, "homing off after the first hit");
                }
            } else if thrown_at.is_some() && gone_at.is_none() {
                gone_at = Some(now);
            }
            if loop_at.is_none() && rig.party.view(g).unwrap().anim.clip == "M_1328" {
                loop_at = Some(now);
            }
            if catch_at.is_none() && rig.party.view(g).unwrap().anim.clip == "M_1329" {
                catch_at = Some(now);
            }
            if rig.party.command_status(g, t) != CommandStatus::Running {
                break;
            }
        }
        assert_eq!(rig.party.command_status(g, t), CommandStatus::Succeeded);
        let (thrown, catch, gone) = (thrown_at.expect("never thrown"), catch_at.expect("no catch"), gone_at.expect("shield never back"));
        assert!(first_speed <= 1500.0 + 1.0, "MaxSpeed 1500 clamps the 2500 launch: {first_speed}");
        assert!((gone - catch).abs() < 2.0 * DT, "the catch clip takes the shield back: {catch} / {gone}");
        if hits {
            let hit = hit_at.expect("no hit");
            assert!((catch - hit - 0.35).abs() < 3.0 * DT, "fade m_fAttackFadeTime 0.35: hit {hit} catch {catch}");
        } else {
            assert!(rig.dummies.hits.is_empty());
            // +0x210 counts down in sub-step 3 only (the flight loop M_1328, after the throw clip)
            let lp = loop_at.expect("no flight loop");
            assert!(lp > thrown && (catch - lp - 2.0).abs() < 2.0 * DT, "m_fAttackTime 2: loop {lp} catch {catch}");
        }
    }
}

/// Attack_Tornado: the tres_colltest01 hull (20-gon prism r 100, z +-50) x (1.3, 1.3, 2.6) at kh_sk + 65 up.
#[test]
fn tornado_hull_is_the_convex_prism() {
    use kh_party::actions::{Prism, TORNADO_HULL};
    assert_eq!(TORNADO_HULL, (130.0, 130.0, 65.0));
    let pr = Prism { centre: Vec3::new(0.0, 0.0, 65.0), radius: 130.0, half_height: 130.0, yaw: 0.0, sides: 20 };
    // a vertex direction reaches 130, an edge midpoint only 130 cos 9 deg
    assert!(pr.distance(Vec3::new(130.0, 0.0, 65.0)) < 1e-3);
    let mid = 130.0 * (9.0f32).to_radians().cos();
    let a = (9.0f32).to_radians();
    assert!(pr.distance(Vec3::new(a.cos(), a.sin(), 0.0) * (mid + 10.0) + Vec3::Z * 65.0) - 10.0 < 1e-2);
    assert_eq!(pr.distance(Vec3::new(0.0, 0.0, 190.0)), 0.0, "inside: 65 + 130 = 195 top");
    assert!((pr.distance(Vec3::new(0.0, 0.0, 205.0)) - 10.0).abs() < 1e-3);
    assert!((pr.distance(Vec3::new(0.0, 0.0, -75.0)) - 10.0).abs() < 1e-3, "bottom at -65");
    // a dummy capsule (r 50) at 170 is touched, at 190 it is not
    assert!(pr.touches(Vec3::new(170.0, 0.0, 50.0), Vec3::new(170.0, 0.0, 130.0), 50.0));
    assert!(!pr.touches(Vec3::new(190.0, 0.0, 50.0), Vec3::new(190.0, 0.0, 130.0), 50.0));
}

#[test]
fn goofy_tornado_turbo_discus() {
    for (def, row, secs) in [(ActionDef::GoofyTornado, "Attack6_Tornado", 6.0), (ActionDef::GoofyTurbo, "Attack7_Turbo", 9.0), (ActionDef::GoofyDiscus, "Attack8_Discus", 7.0)] {
        let mut rig = Rig::new(true, Some(Vec3::new(600.0, -180.0, 0.0)));
        rig.step(2);
        let g = rig.party.find(Chara::Goofy).unwrap();
        let mp0 = rig.party.status(g).unwrap().mp;
        let t = rig.party.command(g, MemberCommand::Action { def, target: TargetRef::Enemy(TargetId(1)) });
        let s = rig.run_until_done(g, t, secs);
        assert_ne!(s, CommandStatus::Running, "{def:?} never ended");
        assert!(rig.party.status(g).unwrap().mp < mp0, "{def:?} spends MP");
        assert!(!rig.hits(row).is_empty(), "{def:?} did not hit; events {:?}", rig.events.iter().filter(|e| !matches!(e, PartyEventKind::Notify { .. })).collect::<Vec<_>>());
    }
}

#[test]
fn tornado_timeline_follows_the_decompiled_substeps() {
    let mut rig = Rig::new(true, Some(Vec3::new(2000.0, -180.0, 0.0)));
    rig.step(2);
    let g = rig.party.find(Chara::Goofy).unwrap();
    rig.party.command(g, MemberCommand::Action { def: ActionDef::GoofyTornado, target: TargetRef::Enemy(TargetId(1)) });
    rig.step(1);
    assert_eq!(rig.party.view(g).unwrap().anim.clip, "M_1321");
    let start_len = kh_party::tables::clip(Chara::Goofy, "M_1321").length;
    rig.step((start_len / DT) as usize + 2);
    let v = rig.party.view(g).unwrap();
    assert_eq!(v.anim.clip, "M_1322");
    // the chase at m_fMovemSpeed 400 toward the target for m_fMoveTime 3 s, hovering at 60 cm
    rig.step(60);
    let v = rig.party.view(g).unwrap();
    let speed = Vec3::new(v.velocity.x, v.velocity.y, 0.0).length();
    assert!((speed - 400.0).abs() < 1.0, "speed {speed}");
    assert!(v.position.z > 5.0, "hover {:?}", v.position);
    rig.step(130);
    assert_eq!(rig.party.view(g).unwrap().anim.clip, "M_1323");
}

#[test]
fn damage_max_and_ko_revives_after_40_and_35_s() {
    let mut rig = Rig::new(false, None);
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    let g = rig.party.find(Chara::Goofy).unwrap();
    let def = rig.party.defender(d).unwrap();
    let r = compute_damage(&Attacker::enemy(500), &def, &DamageAttack::physical(10.0), &DamageContext::new(GameLevel::Standard, 1));
    assert_eq!(r.base, 80.0); // m_DamageMax 24 limits the base (floor((24+S-D)/2)), friends at most 80 (0x1406326B5)
    for (id, revive) in [(d, 40.0), (g, 35.0)] {
        let mut hit = HitRecord::empty(TargetId(99));
        hit.damage = 9999.0;
        hit.damage_kind = kh_core::enums::ETresDamageKind::Small;
        let res = rig.party.receive_hit(id, &hit);
        assert!(res.killed);
        assert_eq!(rig.party.view(id).unwrap().state, NpcStateId::Die);
        assert!((rig.party.view(id).unwrap().revive_left - revive).abs() < 0.01);
    }
    // invincible while KO'd (Die Begin vt+0x2068)
    assert!(!rig.party.receive_hit(d, &HitRecord::empty(TargetId(99))).accepted);
    rig.step((35.5 / DT) as usize);
    assert!(rig.party.status(g).unwrap().hp > 0, "Goofy back at 35 s");
    assert_eq!(rig.party.status(d).unwrap().hp, 0, "Donald still down");
    rig.step((5.0 / DT) as usize);
    assert_eq!(rig.party.status(d).unwrap().hp, 35, "revive HP = 25 % of 140");
    rig.step((3.0 / DT) as usize);
    assert_ne!(rig.party.view(d).unwrap().state, NpcStateId::Die);
    assert!(rig.events.iter().filter(|e| matches!(e, PartyEventKind::Revived)).count() == 2);
}

/// C2 relay: the optional function names, and Cure on a KO'd friend (0x1414742A0: the amount halved,
/// rounding up; the Die state revives as soon as HP > 0, 0x141249F80 sub-step 1).
#[test]
fn c2_functions_and_cure_on_a_knocked_out_friend() {
    let mut rig = Rig::new(true, None);
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    let g = rig.party.find(Chara::Goofy).unwrap();
    let magic = rig.party.status(d).unwrap().magic();
    assert_eq!(rig.party.function(d, "GetMagic"), Some(magic as f32));
    assert_eq!(rig.party.function(d, "GetMPChargeTimeLeft"), Some(0.0));
    assert_eq!(rig.party.function(d, "CanCastCure"), Some(1.0));
    assert_eq!(rig.party.function(g, "CanCastCure"), Some(0.0));
    assert_eq!(rig.party.function(d, "IsEnableWarp"), Some(1.0));
    // C3 names
    assert_eq!(rig.party.function(d, "HasAbility:DONALD_FIRE"), Some(1.0));
    assert_eq!(rig.party.function(g, "HasAbility:GOOFY_FLIP"), Some(1.0));
    assert_eq!(rig.party.function(g, "HasAbility:DONALD_FIRE"), Some(0.0));
    assert!(rig.party.function(d, "GetAbilityNum:ITEM_UP").is_some());
    assert_eq!(rig.party.function(d, "IsCommandRestricted:6"), Some(0.0));
    assert_eq!(rig.party.function(d, "IsBadStatusBound"), Some(0.0));
    assert!(rig.party.function(d, "GetTimeMotionless").unwrap() > 0.0);
    assert_eq!(rig.party.function(d, "NoSuchName"), None);
    let mut hit = HitRecord::empty(TargetId(99));
    hit.damage = 9999.0;
    hit.damage_kind = kh_core::enums::ETresDamageKind::Small;
    assert!(rig.party.receive_hit(g, &hit).killed);
    let t = rig.party.command(d, MemberCommand::Cure { target: TargetRef::Member(g) });
    assert_eq!(rig.run_until_done(d, t, 5.0), CommandStatus::Succeeded);
    assert_eq!(rig.party.status(g).unwrap().hp, (120 + magic + 1) / 2);
    assert_eq!(rig.party.function(d, "CanCastCure"), Some(0.0), "MP recharging");
    assert!(rig.party.function(d, "GetMPChargeTimeLeft").unwrap() > 0.0);
    rig.step((3.0 / DT) as usize);
    assert_ne!(rig.party.view(g).unwrap().state, NpcStateId::Die, "revived by the cure");
}

#[test]
fn hyper_healing_halves_the_ko_time() {
    let mut rig = Rig::new(true, None);
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    for m in [d, rig.party.find(Chara::Goofy).unwrap()] {
        rig.party.status_mut(m).unwrap().abilities.set(kh_core::enums::ETresAbilityKind::HyperHealing, 1, 1);
    }
    let mut hit = HitRecord::empty(TargetId(99));
    hit.damage = 9999.0;
    rig.party.receive_hit(d, &hit);
    assert!((rig.party.view(d).unwrap().revive_left - 20.0).abs() < 0.01);
    let g = rig.party.find(Chara::Goofy).unwrap();
    rig.party.receive_hit(g, &hit);
    assert!((rig.party.view(g).unwrap().revive_left - 17.5).abs() < 0.01);
}

#[test]
fn a_hit_starts_the_damage_reaction() {
    let mut rig = Rig::new(false, None);
    rig.step(2);
    let g = rig.party.find(Chara::Goofy).unwrap();
    let mut hit = HitRecord::empty(TargetId(99));
    hit.damage = 10.0;
    hit.damage_kind = kh_core::enums::ETresDamageKind::Blow;
    hit.direction = Vec3::X;
    let hp0 = rig.party.status(g).unwrap().hp;
    rig.party.receive_hit(g, &hit);
    assert_eq!(rig.party.status(g).unwrap().hp, hp0 - 10);
    assert_eq!(rig.party.view(g).unwrap().state, NpcStateId::Damage);
    let x0 = rig.party.view(g).unwrap().position.x;
    rig.step(30);
    assert!(rig.party.view(g).unwrap().position.x > x0 + 20.0, "knocked back");
    rig.step(400);
    assert_eq!(rig.party.view(g).unwrap().state, NpcStateId::Idle);
}

#[test]
fn goofy_drinks_a_potion() {
    let mut rig = Rig::new(false, None);
    rig.step(2);
    let g = rig.party.find(Chara::Goofy).unwrap();
    rig.party.status_mut(g).unwrap().hp = 50;
    let t = rig.party.command(g, MemberCommand::UseItem { item: ItemKind::Potion, target: TargetRef::Member(g) });
    assert_eq!(rig.run_until_done(g, t, 3.0), CommandStatus::Succeeded);
    assert_eq!(rig.party.status(g).unwrap().hp, 130);
    assert_eq!(rig.party.status(g).unwrap().items.iter().filter(|s| s.present).count(), 1);
}

#[test]
fn move_to_and_warp() {
    let mut rig = Rig::new(false, None);
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    let t = rig.party.command(d, MemberCommand::MoveTo { goal: MoveGoal::Point(Vec3::new(600.0, 180.0, 0.0)), locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 50.0, follow_path: false });
    // (FOL2) the leader stands near: KH3's follow rule walks him there (AnalogInputModifier 0, m_MinSpeed 140), with the soft stop
    assert_eq!(rig.run_until_done(d, t, 6.0), CommandStatus::Succeeded);
    // (party K1) the reach test counts his capsule radius in (UE path following bReachTestIncludesAgentRadius): 50 + 30 cm
    assert!(rig.party.view(d).unwrap().position.x > 520.0, "{:?}", rig.party.view(d).unwrap().position);
    // UTresNpcState_Warp 0x14126DD90: Sora 5000 away (beyond +0xA0 3000), point at his back. The point is
    // kept one step after Begin, then +0x98 = 2 s of wait, then the move (no camera: never in view).
    rig.sora.position = Vec3::new(5600.0, 0.0, 0.0);
    let goal = Vec3::new(5300.0, 0.0, 0.0);
    let t = rig.party.command(d, MemberCommand::Warp { goal, yaw: 0.0 });
    rig.step(60);
    assert_eq!(rig.party.command_status(d, t), CommandStatus::Running, "still waiting +0x98");
    assert!(rig.party.view(d).unwrap().position.x < 1000.0);
    assert_eq!(rig.run_until_done(d, t, 1.5), CommandStatus::Succeeded);
    let v = rig.party.view(d).unwrap();
    assert!((v.position.x - 5300.0).abs() < 1.0, "{:?}", v.position);
    assert!(v.yaw.abs() < 1.0, "faces the leader: {}", v.yaw);
    // the camera sees the point: the move waits until the 2 x +0x88 limit runs out, then gives up
    rig.step(2);
    rig.sora.position = Vec3::new(-5000.0, 0.0, 0.0);
    rig.sora.camera = Some(kh_party::PartyCamera { position: Vec3::new(-5000.0, 0.0, 300.0), pitch: 0.0, yaw: 0.0, fov: 70.0, aspect: 16.0 / 9.0 });
    let far = Vec3::new(-4600.0, 0.0, 0.0);
    let t = rig.party.command(d, MemberCommand::Warp { goal: far, yaw: 180.0 });
    assert_eq!(rig.run_until_done(d, t, 5.0), CommandStatus::Failed);
    assert!(rig.party.view(d).unwrap().position.x > 5000.0, "not moved while in view");
    // a leader within 3000 ends the warp at the next re-check, no move
    rig.sora.camera = None;
    rig.sora.position = Vec3::new(5000.0, 0.0, 0.0);
    let t = rig.party.command(d, MemberCommand::Warp { goal: far, yaw: 180.0 });
    assert_eq!(rig.run_until_done(d, t, 0.5), CommandStatus::Succeeded);
    assert!(rig.party.view(d).unwrap().position.x > 5000.0);
}

#[test]
fn shared_exp_levels_the_party_with_their_own_tables() {
    let mut rig = Rig::new(false, None);
    let ups = rig.party.set_total_exp(28_500);
    let d = rig.party.find(Chara::Donald).unwrap();
    let g = rig.party.find(Chara::Goofy).unwrap();
    assert!(ups.contains(&(d, 1, 20)));
    assert!(ups.contains(&(g, 1, 19)));
    rig.step(1);
    assert!(rig.events.iter().any(|e| matches!(e, PartyEventKind::LevelUp { level: 20 })));
}

#[test]
#[cfg(feature = "ai")]
fn nobody_swims_on_dry_land() {
    let mut rig = Rig::new(true, None);
    rig.step(10);
    for c in [Chara::Donald, Chara::Goofy] {
        let m = rig.party.find(c).unwrap();
        assert!(!rig.party.view(m).unwrap().swimming);
        for f in ["IsSwimming", "IsUnderWater", "IsInWater"] {
            assert_eq!(rig.party.function(m, f), Some(0.0), "{c:?} {f}");
        }
        let body = kh_party::ai_body::PartyBody { party: &mut rig.party, member: m };
        assert!(!kh_ai::AiBody::view(&body).swimming);
        assert_eq!(kh_ai::AiBody::function(&body, "IsSwimming"), Some(0.0));
    }
}

#[test]
fn cure_bonus_reaches_the_team_within_300() {
    // 0x1414742A0: the others of the target's team within m_DetectRange 300 get 60 + MagicPower
    let mut rig = Rig::new(false, None);
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    let g = rig.party.find(Chara::Goofy).unwrap();
    rig.party.status_mut(g).unwrap().hp = 10;
    rig.party.status_mut(d).unwrap().hp = 10;
    let t = rig.party.command(d, MemberCommand::Cure { target: TargetRef::Sora });
    assert_eq!(rig.run_until_done(d, t, 5.0), CommandStatus::Succeeded);
    let magic = rig.party.status(d).unwrap().magic();
    assert_eq!(rig.heals, vec![120 + magic]);
    // Goofy (180 cm from Sora) and Donald himself (180 cm) are inside 300
    assert_eq!(rig.party.status(g).unwrap().hp, (10 + 60 + magic).min(rig.party.status(g).unwrap().max_hp()));
    assert_eq!(rig.party.status(d).unwrap().hp, (10 + 60 + magic).min(rig.party.status(d).unwrap().max_hp()));
}

#[test]
fn turn_uses_the_turn_clip_and_idle_turns_at_180() {
    let mut rig = Rig::new(false, None);
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    // Turn 0x140437890: TurnN with the delta along its root-yaw curve
    let p = rig.party.view(d).unwrap().position;
    let t = rig.party.command(d, MemberCommand::TurnTo { goal: MoveGoal::Point(p + Vec3::new(0.0, 500.0, 0.0)) });
    rig.step(1);
    assert_eq!(rig.party.view(d).unwrap().anim.clip, "TurnN");
    assert_eq!(rig.run_until_done(d, t, 3.0), CommandStatus::Succeeded);
    assert!((rig.party.view(d).unwrap().yaw - 90.0).abs() < 0.5, "{}", rig.party.view(d).unwrap().yaw);
    // the request's tolerance +0x9C is 0 (UTresBTTask_TurnTo 0x1404B69B0 -> vt+0x3C0 param 3): a 0.5 deg turn
    // still plays the turn clip
    assert_eq!(kh_party::pawn::TURN_DONE, 0.0);
    let y = (90.5f32).to_radians();
    let p2 = rig.party.view(d).unwrap().position;
    rig.party.command(d, MemberCommand::TurnTo { goal: MoveGoal::Point(p2 + Vec3::new(y.cos(), y.sin(), 0.0) * 500.0) });
    rig.step(1);
    assert_eq!(rig.party.view(d).unwrap().state, NpcStateId::Turn);
    rig.step(120);
    // Idle facing a focus: KH3 rotates at most 180 deg/s under the idle clip (m_IdleMaxRotationSpeed, 0x140445880);
    // (party K1) the port turns by the turn clip instead (the user: no turn without a turn animation): TurnN plays
    // and he ends facing the focus
    rig.party.command(d, MemberCommand::Focus(TargetRef::Point(p + Vec3::new(-500.0, 0.0, 0.0))));
    rig.step(2);
    assert_eq!(rig.party.view(d).unwrap().anim.clip, "TurnN");
    rig.step(90);
    let yaw = rig.party.view(d).unwrap().yaw;
    assert!(kh_party::pawn::wrap180(yaw - 180.0).abs() < 4.0, "faces the focus: {yaw}");
}

#[test]
#[cfg(feature = "ai")]
fn ai_relay_rules() {
    let mut rig = Rig::new(true, Some(Vec3::new(600.0, -180.0, 0.0)));
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    let g = rig.party.find(Chara::Goofy).unwrap();
    // a definition of the other character, or MP charging: Failed at command time, nothing queued
    let t = rig.party.command(d, MemberCommand::Action { def: ActionDef::GoofyTornado, target: TargetRef::Enemy(TargetId(1)) });
    assert_eq!(rig.party.command_status(d, t), CommandStatus::Failed);
    rig.party.status_mut(d).unwrap().mp_charge_left = 5.0;
    let t = rig.party.command(d, MemberCommand::Action { def: ActionDef::DonaldFire, target: TargetRef::Enemy(TargetId(1)) });
    assert_eq!(rig.party.command_status(d, t), CommandStatus::Failed);
    rig.step(1);
    assert_eq!(rig.party.view(d).unwrap().state, NpcStateId::Idle);
    // state names and functions for kh-ai
    {
        let body = kh_party::ai_body::PartyBody { party: &mut rig.party, member: g };
        assert_eq!(kh_ai::AiBody::view(&body).state, "TSID_STATE_IDLE");
        assert_eq!(kh_ai::AiBody::function(&body, "IsActionOff"), Some(1.0));
        assert_eq!(kh_ai::AiBody::function(&body, "NoSuchFunction"), None);
        // CanBattle 0x141257340: false out of battle
        assert_eq!(kh_ai::AiBody::function(&body, "CanBattle"), Some(0.0));
    }
    rig.party.set_in_battle(true);
    rig.step(20);
    assert_eq!(rig.party.function(g, "CanBattle"), Some(0.0), "not before 0.5 s of battle");
    rig.step(20);
    assert_eq!(rig.party.function(g, "CanBattle"), Some(1.0));
    // Stop cancels a running action (AbortTask 0x140492CB0)
    let t = rig.party.command(g, MemberCommand::Action { def: ActionDef::GoofyJump, target: TargetRef::Enemy(TargetId(1)) });
    rig.step(10);
    assert_eq!(rig.party.view(g).unwrap().state, NpcStateId::Action);
    rig.party.command(g, MemberCommand::Stop);
    assert_eq!(rig.party.command_status(g, t), CommandStatus::Aborted);
    rig.step(1);
    // (B5) back to Idle; in the air the Idle tick 0x14124CD40 asks for the jump at once (NavLinkJump, the fall)
    let v = rig.party.view(g).unwrap();
    assert!(v.state == NpcStateId::Idle || (!v.on_ground && v.state == NpcStateId::NavLinkJump), "{:?}", v.state);
    assert_eq!(rig.party.member(g).unwrap().cmc.gravity_scale, rig.party.member(g).unwrap().params.gravity_scale);
}

#[test]
fn spell_shapes_follow_the_blueprints() {
    use kh_party::projectile::{Projectile, ProjectileKind};
    let mut v = Vec::new();
    let fire = Projectile::spawn(ProjectileKind::Fire, kh_party::MemberId(0), Chara::Donald, Vec3::ZERO, 0.0, TargetId::NONE, None);
    fire.shapes(&mut v);
    assert_eq!((v.len(), v[0].radius), (1, 20.0));
    assert_eq!((fire.speed, fire.max_speed, fire.turn), (1000.0, 1500.0, 500.0));
    let mut th = Projectile::spawn_at(ProjectileKind::Thunder, kh_party::MemberId(0), Chara::Donald, Vec3::ZERO, 0.0, TargetId::NONE, Some(Vec3::new(500.0, 0.0, 90.0)), Some(Vec3::new(500.0, 0.0, 0.0)));
    assert_eq!(th.position, Vec3::new(500.0, 0.0, 0.0), "m_bIsDirectPos: at the target's feet");
    th.shapes(&mut v);
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].radius, 50.0);
    assert_eq!(v[1].radius, 200.0);
    th.age = 0.2;
    th.shapes(&mut v);
    assert_eq!(v[0].radius, 170.0); // 50 + 600 x 0.2
    th.age = 0.29;
    th.shapes(&mut v);
    assert_eq!(v[0].radius, 200.0); // capped by m_IncMaxSize
    let aero = Projectile::spawn(ProjectileKind::Aero, kh_party::MemberId(0), Chara::Donald, Vec3::ZERO, 0.0, TargetId::NONE, None);
    assert!((aero.life - 0.12).abs() < 1e-6);
    assert_eq!(aero.radius(), 120.0);
}

#[test]
fn a_pending_fr_state_wins_over_later_ai_commands() {
    let mut rig = Rig::new(true, None);
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    let fr = rig.party.command(d, MemberCommand::EnterFrState { state: 207 });
    let ai = rig.party.command(d, MemberCommand::MoveTo { goal: MoveGoal::Point(Vec3::new(500.0, 0.0, 0.0)), locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 50.0, follow_path: false });
    assert_eq!(rig.party.command_status(d, ai), CommandStatus::Failed);
    assert_eq!(rig.party.command_status(d, fr), CommandStatus::Running);
    rig.step(1);
    assert_eq!(rig.party.view(d).unwrap().state, NpcStateId::Fr(207));
}

#[test]
fn clip_notifies_reach_the_seam() {
    // E.3: B's adapter maps the fired notifies of the member's clips to PartyNotify::dispatch
    let mut rig = Rig::new(true, Some(Vec3::new(140.0, 180.0, 0.0)));
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    let t = rig.party.command(d, MemberCommand::Action { def: ActionDef::DonaldBeat, target: TargetRef::Enemy(TargetId(1)) });
    rig.run_until_done(d, t, 4.0);
    assert!(!rig.audio.1.is_empty(), "no notify sound / voice reached the host");
}

#[test]
fn damage_clips_follow_the_npc_anim_name_functions() {
    use kh_party::pawn::{damage_clip, damage_dir_index};
    use kh_react::Phase;
    // 0x1412646C0 / 0x14125FC00 / 0x141262420
    assert_eq!(damage_dir_index(Vec3::new(-1.0, 0.0, 0.0), 0.0), 0, "hit from the front pushes back");
    assert_eq!(damage_dir_index(Vec3::new(1.0, 0.0, 0.0), 0.0), 1);
    assert_eq!(damage_clip(Chara::Donald, Phase::FlinchGround, 1), "M_0202");
    assert_eq!(damage_clip(Chara::Donald, Phase::FlinchAir, 3), "M_0210");
    assert_eq!(damage_clip(Chara::Goofy, Phase::BlownFlying, 2), "M_0217");
    assert_eq!(damage_clip(Chara::Goofy, Phase::Down, 0), "M_0213"); // (GOOFY-HR) phase 2 is the down + get-up (0x14125FC00)
    assert_eq!(damage_clip(Chara::Goofy, Phase::GettingUp, 3), "M_0222");
    assert_eq!(damage_clip(Chara::Donald, Phase::WallHit, 1), "M_0224");
    assert_eq!(damage_clip(Chara::Donald, Phase::Falling, 0), "JumpNormalDownLoop");
    assert_eq!(kh_party::tables::param("damage.wall_hit_speed"), 50.0);
}

/// The bad-status states: Stun (shared UTresCharState_BadStatStun 0x141508930 / 0x14150EE70, clips by
/// 0x1412470D0), Sneeze (0x141240DE0 / 0x1412495C0), Freeze (0x1404330F0 / 0x140440660).
#[test]
fn bad_status_states_stun_sneeze_freeze() {
    for (name, state, time) in [("STUN", NpcStateId::BadStatStun, 3.0), ("SNEEZE", NpcStateId::BadStatSneeze, 3.0), ("FREEZE", NpcStateId::BadStatFreeze, 2.0), ("BURN", NpcStateId::BadStatBurn, 2.0)] {
        let mut rig = Rig::new(true, None);
        rig.step(2);
        let d = rig.party.find(Chara::Donald).unwrap();
        let mut hit = HitRecord::empty(TargetId(99));
        hit.damage = 1.0;
        hit.damage_kind = kh_core::enums::ETresDamageKind::Small;
        hit.info.bad_status = name;
        hit.info.bad_time = time;
        rig.party.receive_hit(d, &hit);
        // the flinch first, then the status's state once he stands
        let mut n = 0;
        while rig.party.view(d).unwrap().state != state && n < 180 {
            rig.step(1);
            n += 1;
        }
        assert_eq!(rig.party.view(d).unwrap().state, state, "{name}");
        let mut clips: Vec<&str> = Vec::new();
        let mut secs = 0.0;
        while rig.party.view(d).unwrap().state == state && secs < 10.0 {
            rig.step(1);
            secs += DT;
            let c = rig.party.view(d).unwrap().anim.clip;
            if clips.last() != Some(&c) {
                clips.push(c);
            }
        }
        assert_eq!(rig.party.view(d).unwrap().state, NpcStateId::Idle, "{name} ends: {clips:?}");
        match name {
            "STUN" => {
                assert!(clips.contains(&"BadStatStunLoop") && clips.contains(&"BadStatStunEnd"), "{clips:?}");
                assert!(secs > time, "the loop lasts the status time: {secs}");
            }
            "FREEZE" => assert!((secs - time).abs() < 0.1, "frozen for the status time: {secs}"),
            "BURN" => {
                assert!(clips.iter().filter(|c| **c == "BadStatBurnLoop").count() >= 2, "hops: {clips:?}");
                assert_eq!(clips.last(), Some(&"IdleN"), "{clips:?}");
                assert!(clips.contains(&"BadStatBurnEnd"));
                assert!(rig.party.status(d).unwrap().hp < rig.party.status(d).unwrap().max_hp(), "5 % per hop after the first");
            }
            _ => assert!(secs < 5.0),
        }
    }
}

/// UTresAICharState_Reflect (Tick 0x140441FF0, clip 0x141264270): a repelled weapon recoils the friend.
#[test]
fn a_repelled_weapon_starts_reflect() {
    // (B4) the friend's own contact (notify slot 15 0x141271720) repels only on an attacker result REFLECT /
    // GUARD_REFLECT from Classify; every friend row has m_ReflectLevel 0, so its shapes are "no clash" (combat
    // spec 8.2.1 prim+0x986 bit 7) and a guarding target does NOT repel them (attacker NOREACTION).
    let mut rig = Rig::new(true, Some(Vec3::new(140.0, 180.0, 0.0)));
    rig.dummies.guard = true;
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    rig.party.command(d, MemberCommand::Action { def: ActionDef::DonaldBeat, target: TargetRef::Enemy(TargetId(1)) });
    for _ in 0..120 {
        rig.step(1);
        assert_ne!(rig.party.view(d).unwrap().state, NpcStateId::Reflect, "a guard does not repel a no-clash swing");
    }
    // the victim side (TakeDamage 0x1405BB850): a hit whose victim result is REFLECT -> iface vt+0x448 -> Reflect,
    // knocked away from the causer, no HP taken
    rig.step(10);
    let x0 = rig.party.view(d).unwrap().position.x;
    let hp0 = rig.party.view(d).unwrap().hp;
    let mut hit = HitRecord::empty(TargetId(1));
    hit.damage = 30.0;
    hit.info.victim_result = kh_core::host::ReactionResult::Reflect;
    hit.attacker = kh_core::host::HitAttacker { position: Vec3::new(x0 + 100.0, 180.0, 50.0), yaw: 180.0, radius: 50.0, valid: true };
    hit.direction = Vec3::new(-1.0, 0.0, 0.0);
    let r = rig.party.receive_hit(d, &hit);
    assert!(r.accepted);
    assert_eq!(rig.party.view(d).unwrap().hp, hp0, "a reflected hit takes no HP");
    let mut seen = false;
    for _ in 0..120 {
        rig.step(1);
        let v = rig.party.view(d).unwrap();
        if v.state == NpcStateId::Reflect {
            seen = true;
            assert_eq!(v.anim.clip, "AttackReflect");
        }
    }
    assert!(seen, "no Reflect");
    let x1 = rig.party.view(d).unwrap().position.x;
    assert!(x1 < x0 - 20.0, "knocked back from the causer: {x0} -> {x1}");
    assert_eq!(rig.party.view(d).unwrap().state, NpcStateId::Idle);
}

#[test]
fn spawn_as_keeps_ids() {
    // stream P relay: Party Formation re-adds friends at fixed ids (Donald 0, Goofy 1) whatever the order.
    let mut party = Party::new(PartyConfig::test_level99());
    let g = party.spawn_as(Chara::Goofy, kh_party::MemberId(1), Vec3::new(100.0, 0.0, 0.0), 0.0);
    assert_eq!(g, kh_party::MemberId(1));
    assert_eq!(party.members().collect::<Vec<_>>(), vec![kh_party::MemberId(1)]);
    let d = party.spawn_as(Chara::Donald, kh_party::MemberId(0), Vec3::ZERO, 0.0);
    assert_eq!(d, kh_party::MemberId(0));
    assert_eq!(party.find(Chara::Donald), Some(kh_party::MemberId(0)));
    party.despawn(kh_party::MemberId(0));
    assert_eq!(party.members().collect::<Vec<_>>(), vec![kh_party::MemberId(1)]);
    let d2 = party.spawn_as(Chara::Donald, kh_party::MemberId(0), Vec3::ZERO, 0.0);
    assert_eq!(d2, kh_party::MemberId(0));
    assert_eq!(party.member(kh_party::MemberId(1)).map(|m| m.chara), Some(Chara::Goofy));
}

// ---------------------------------------------------------------- (B4) traversal and situation states

fn state_of(rig: &Rig, id: kh_party::MemberId) -> NpcStateId {
    rig.party.view(id).unwrap().state
}

/// Donald alone (Goofy removed) at `at`.
fn donald_rig(world: TestWorld, at: Vec3) -> (Rig, kh_party::MemberId) {
    let mut rig = Rig::new(true, None);
    rig.world = world;
    rig.party.despawn(kh_party::MemberId(0));
    rig.party.despawn(kh_party::MemberId(1));
    let d = rig.party.spawn_as(Chara::Donald, kh_party::MemberId(0), at, 0.0);
    rig.sora.position = at + Vec3::new(-300.0, 0.0, 0.0);
    (rig, d)
}

fn move_to(x: f32, y: f32, z: f32, loco: LocomotionDef) -> MemberCommand {
    MemberCommand::MoveTo { goal: MoveGoal::Point(Vec3::new(x, y, z)), locomotion: loco, walk: false, acceptable_radius: 50.0, follow_path: false }
}

#[test]
fn slope_slide_from_the_run_on_a_flagged_slope() {
    use kh_core::surface;
    // a 30 deg plane descending toward +X, component 1 flagged SQEX_bEnableSlopeSlide
    let a = 30.0f32.to_radians();
    let n = Vec3::new(a.sin(), 0.0, a.cos());
    let world = TestWorld::default().with_plane_through(Vec3::ZERO, n).with_flags(1, surface::SLOPE_SLIDE);
    let (mut rig, d) = donald_rig(world, Vec3::ZERO);
    rig.step(3);
    let t = rig.party.command(d, move_to(3000.0, 0.0, -3000.0 * a.tan(), LocomotionDef::SlopeSlide));
    rig.step(20);
    // the Run tick's slope probe FUN_14063C870 (mode 0) -> SlopeSlide; the clip `SlopeSlide`
    assert_eq!(state_of(&rig, d), NpcStateId::SlopeSlide);
    assert_eq!(rig.party.view(d).unwrap().anim.clip, "SlopeSlide");
    // MaxAcceleration = MaxWalkSpeed x 60: the slide speed (the SlopeSlide locomotion 1600) within a few ticks
    rig.step(30);
    let v = rig.party.view(d).unwrap();
    assert!(v.velocity.truncate().length() > 1200.0, "slides at the locomotion speed: {:?}", v.velocity);
    assert_eq!(rig.run_until_done(d, t, 4.0), CommandStatus::Succeeded);
    // no input any more: the slide keeps going (the probe still passes)
    rig.step(5);
    assert_eq!(state_of(&rig, d), NpcStateId::SlopeSlide);
    // the floor stops being a slide floor: no input -> `RunEndR`, sub 1, then the default state
    rig.world = TestWorld::default().with_plane_through(Vec3::ZERO, n);
    rig.step(2);
    assert_eq!(rig.party.view(d).unwrap().anim.clip, "RunEndR");
    rig.step(120);
    assert_eq!(state_of(&rig, d), NpcStateId::Idle);
}

#[test]
fn super_slide_runs_on_the_ai_feed_and_ends_with_slide_end() {
    let (mut rig, d) = donald_rig(TestWorld::floor(), Vec3::ZERO);
    rig.step(3);
    // iface vt+0x240 0x141271170: Idle on the ground -> SuperSlide sub 1 (`SonicSlide`), then the loop
    assert!(rig.party.request_super_slide(d));
    assert_eq!(state_of(&rig, d), NpcStateId::SuperSlide);
    rig.party.command(d, move_to(0.0, 6000.0, 0.0, LocomotionDef::Move1));
    rig.step(1);
    assert_eq!(rig.party.view(d).unwrap().anim.clip, "SonicSlide");
    rig.step(90);
    let v = rig.party.view(d).unwrap();
    assert_eq!(v.state, NpcStateId::SuperSlide);
    assert_eq!(v.anim.clip, "SonicSlideLoop");
    // ground: Velocity = MaxWalkSpeed (m_SuperSlideSpeed 1500) along +0x8C; the facing eases there (dt x 6)
    assert!((v.velocity.truncate().length() - 1500.0).abs() < 1.0, "{:?}", v.velocity);
    assert!((v.yaw - 90.0).abs() < 5.0, "turned to the feed: {}", v.yaw);
    // the move is cancelled: no feed and no acceleration -> `SlideEnd` (sub 3), its end -> Idle
    rig.party.command(d, MemberCommand::Idle { time: 0.0 });
    rig.step(2);
    let v = rig.party.view(d).unwrap();
    assert!(v.state == NpcStateId::Idle || v.anim.clip == "SlideEnd", "{:?} {}", v.state, v.anim.clip);
}

#[test]
fn super_slide_without_feed_ends_with_slide_end() {
    let (mut rig, d) = donald_rig(TestWorld::floor(), Vec3::ZERO);
    rig.step(3);
    assert!(rig.party.request_super_slide(d));
    let mut clips: Vec<&'static str> = Vec::new();
    for _ in 0..300 {
        rig.step(1);
        let c = rig.party.view(d).unwrap().anim.clip;
        if clips.last() != Some(&c) {
            clips.push(c);
        }
    }
    assert!(clips.contains(&"SlideEnd"), "{clips:?}");
    assert_eq!(state_of(&rig, d), NpcStateId::Idle);
}

#[test]
fn rail_slide_attaches_runs_at_2000_and_jumps_off_the_end() {
    let node = |x: f32| kh_core::RailNode { position: Vec3::new(x, 0.0, 0.0), speed: None, detect_range_plus: 0.0 };
    let mut world = TestWorld::floor();
    world.rails.push(kh_core::Rail { id: 7, nodes: vec![node(200.0), node(1200.0), node(3200.0)], start_dir: 0, no_reverse: false, no_jump: false, narrow: false });
    let (mut rig, d) = donald_rig(world, Vec3::ZERO);
    rig.sora.position = Vec3::new(2000.0, 0.0, 0.0);
    rig.step(3);
    rig.party.command(d, move_to(6000.0, 0.0, 0.0, LocomotionDef::Move1));
    let mut entered = false;
    for _ in 0..120 {
        rig.step(1);
        if state_of(&rig, d) == NpcStateId::RailSlide {
            entered = true;
            break;
        }
    }
    // the Run tick's rail probe (vt+0xEA0, range 50) near the rail's first node
    assert!(entered, "the rail probe takes the rail");
    rig.step(2);
    assert_eq!(rig.party.view(d).unwrap().anim.clip, "RailSlideStart");
    // RailSlideStart, then the loop at MyRailSlideSpeed 2000
    let mut x0 = None;
    for _ in 0..60 {
        rig.step(1);
        if rig.party.view(d).unwrap().anim.clip == "RailSlideLoop" {
            x0 = Some(rig.party.view(d).unwrap().position.x);
            break;
        }
    }
    let x0 = x0.expect("the loop starts");
    rig.step(30);
    let x1 = rig.party.view(d).unwrap().position.x;
    assert!(((x1 - x0) - 1000.0).abs() < 40.0, "2000 cm/s: {}", x1 - x0);
    // the end: slot 145 0x141252210 -> the code-driven jump 500 ahead (B5: NavLinkJumpCodeDriven 0x2B), then Land
    let mut jumped = false;
    for _ in 0..120 {
        rig.step(1);
        if state_of(&rig, d) == NpcStateId::NavLinkJump {
            jumped = true;
            break;
        }
    }
    assert!(jumped, "leaves the rail end with a jump");
    // (B5) sub 0: JumpNormalStart; the launch (vt+0x3F8 0x140433FD0) at its TresAnimNotify_JumpStart (0.1 s)
    let mut clips: Vec<&str> = Vec::new();
    let mut launched_at = None;
    for i in 0..30 {
        rig.step(1);
        let v = rig.party.view(d).unwrap();
        if clips.last() != Some(&v.anim.clip) {
            clips.push(v.anim.clip);
        }
        if v.velocity.z > 0.0 {
            launched_at = Some(i);
            break;
        }
    }
    assert_eq!(clips.first(), Some(&"JumpNormalStart"), "{clips:?}");
    let i = launched_at.expect("the JumpStart notify launches the jump");
    assert!((4..=8).contains(&i), "launch at about 0.1 s: step {i}");
    let mut landed = false;
    for _ in 0..240 {
        rig.step(1);
        let v = rig.party.view(d).unwrap();
        if clips.last() != Some(&v.anim.clip) {
            clips.push(v.anim.clip);
        }
        if v.state == NpcStateId::Move || v.state == NpcStateId::Idle {
            landed = true;
            break;
        }
    }
    // rise loop, down loop (no JumpNormalDownStart in the set), Land (0x141263A00 names)
    assert!(clips.contains(&"JumpNormalRiseLoop") && clips.contains(&"JumpNormalDownLoop") && clips.contains(&"Land"), "{clips:?}");
    assert!(landed, "lands and runs on");
    let v = rig.party.view(d).unwrap();
    assert!(v.position.x > 3500.0, "{:?} {:?}", v.position, v.state);
}

#[test]
fn rail_slide_turns_round_when_the_goal_is_behind() {
    let node = |x: f32| kh_core::RailNode { position: Vec3::new(x, 0.0, 0.0), speed: None, detect_range_plus: 0.0 };
    let mut world = TestWorld::floor();
    world.rails.push(kh_core::Rail { id: 7, nodes: vec![node(200.0), node(5200.0)], start_dir: 0, no_reverse: false, no_jump: false, narrow: false });
    let (mut rig, d) = donald_rig(world, Vec3::ZERO);
    rig.sora.position = Vec3::new(1000.0, 0.0, 0.0);
    rig.step(3);
    rig.party.command(d, move_to(6000.0, 0.0, 0.0, LocomotionDef::Move1));
    for _ in 0..120 {
        rig.step(1);
        if rig.party.view(d).unwrap().anim.clip == "RailSlideLoop" {
            break;
        }
    }
    rig.step(30);
    // the AI goal behind him: the base needs a tick without input first (flag 0x4000 of Reset), then after the
    // 1/3 s lock-out the turn-around (dot < -0.5) -> `RailSlideTurn`. The move stops for a tick (its radius
    // already reached), then runs back.
    rig.party.command(d, MemberCommand::MoveTo { goal: MoveGoal::Point(Vec3::new(-3000.0, 0.0, 0.0)), locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 1.0e6, follow_path: false });
    rig.step(2);
    assert_eq!(state_of(&rig, d), NpcStateId::RailSlide);
    rig.party.command(d, move_to(-3000.0, 0.0, 0.0, LocomotionDef::Move1));
    let mut turned = false;
    for _ in 0..60 {
        rig.step(1);
        if rig.party.view(d).unwrap().anim.clip == "RailSlideTurn" {
            turned = true;
            break;
        }
    }
    assert!(turned);
    assert_eq!(state_of(&rig, d), NpcStateId::RailSlide);
}

#[test]
fn hopping_follows_the_points_and_hops_off_the_last() {
    let hop = |id: u32, x: f32| kh_core::HopPoint { id, position: Vec3::new(x, 0.0, 0.0), check_radius: 100.0, idle_motion: 1, end_point: false, end_roll: false, active: true };
    let mut world = TestWorld::floor();
    world.hops.extend([hop(1, 300.0), hop(2, 550.0), hop(3, 800.0)]);
    let (mut rig, d) = donald_rig(world, Vec3::ZERO);
    rig.step(3);
    rig.party.command(d, move_to(3000.0, 0.0, 0.0, LocomotionDef::Move1));
    let mut clips: Vec<&'static str> = Vec::new();
    for _ in 0..600 {
        rig.step(1);
        let v = rig.party.view(d).unwrap();
        if clips.last() != Some(&v.anim.clip) {
            clips.push(v.anim.clip);
        }
    }
    // the hop probe 120 ahead -> HopL / HopR alternating onto each point, BalanceL / BalanceR on it; the
    // last point has none 150 - 350 ahead -> hop off -> the fall -> Land
    let hops = clips.iter().filter(|c| **c == "HopL" || **c == "HopR").count();
    assert!(hops >= 4, "three hops on and one off: {clips:?}");
    assert!(clips.iter().any(|c| c.starts_with("Balance")), "{clips:?}");
    assert!(clips.contains(&"Land"), "{clips:?}");
    let x = rig.party.view(d).unwrap().position.x;
    assert!(x > 900.0, "past the last point: {x} {clips:?}");
}

#[test]
fn situation_action_modes_and_npc_motion_play() {
    let (mut rig, d) = donald_rig(TestWorld::floor(), Vec3::ZERO);
    rig.step(3);
    // situation 8 (gimmick wait): SituationAction, inner GimmickWait: the idle clip looped
    assert!(rig.party.set_situation(d, 8, 0.0));
    rig.step(2);
    assert_eq!(state_of(&rig, d), NpcStateId::SituationAction);
    assert_eq!(rig.party.view(d).unwrap().anim.clip, "IdleN");
    rig.step(200);
    assert_eq!(state_of(&rig, d), NpcStateId::SituationAction, "waits until the situation is cleared");
    assert!(rig.party.set_situation(d, 0, 0.0));
    rig.step(1);
    assert_eq!(state_of(&rig, d), NpcStateId::Idle);
    // situation 9 (NPC motion play): start, the loop twice, the end; then the state ends
    assert!(rig.party.request_npc_motion(d, 2, [Some("TalkAct1"), Some("TalkAct2"), Some("TalkAct3")]));
    let mut seq: Vec<&'static str> = Vec::new();
    let mut last_time = 0.0;
    for _ in 0..1200 {
        rig.step(1);
        let v = rig.party.view(d).unwrap();
        if seq.last() != Some(&v.anim.clip) || v.anim.time < last_time {
            seq.push(v.anim.clip);
        }
        last_time = v.anim.time;
        if v.state == NpcStateId::Idle {
            break;
        }
    }
    assert_eq!(&seq[..4], &["TalkAct1", "TalkAct2", "TalkAct2", "TalkAct3"], "{seq:?}");
    assert_eq!(state_of(&rig, d), NpcStateId::Idle);
}

#[test]
fn goofy_coop_situation_offers_its_command_near_the_leader() {
    let mut rig = Rig::new(true, None);
    rig.step(3);
    let g = rig.party.find(Chara::Goofy).unwrap();
    rig.sora.position = Vec3::new(300.0, -180.0, 0.0);
    // n_ex002 SituationAction situation 7: GCoopStart turning to the leader, then GCoopLoop
    assert!(rig.party.set_situation(g, 7, 0.0));
    rig.step(2);
    assert_eq!(rig.party.view(g).unwrap().anim.clip, "GCoopStart");
    rig.step(300);
    assert_eq!(rig.party.view(g).unwrap().anim.clip, "GCoopLoop");
    assert!(rig.party.view(g).unwrap().yaw.abs() < 2.0, "faces the leader");
    // the coop actor within 500 of the leader: the situation command 0xCE; away: taken back
    rig.party.set_coop_target(g, Some(Vec3::new(500.0, -180.0, 0.0)));
    rig.step(2);
    assert!(rig.events.contains(&PartyEventKind::SituationCommand { command: 0xCE, on: true }));
    rig.party.set_coop_target(g, Some(Vec3::new(5000.0, -180.0, 0.0)));
    rig.step(2);
    assert!(rig.events.contains(&PartyEventKind::SituationCommand { command: 0xCE, on: false }));
}

#[test]
fn fr_hit_delivers_the_members_own_row() {
    // D3 relay (contract D.6): Goofy's FR_GOOFYSHOOT_RUSH through actions::deliver
    use kh_party::team::TeamParty;
    let mut rig = Rig::new(true, Some(Vec3::new(200.0, -180.0, 0.0)));
    rig.step(2);
    let g = rig.party.find(Chara::Goofy).unwrap();
    let d = rig.party.find(Chara::Donald).unwrap();
    let mut host = PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
    assert!(rig.party.fr_hit(g, "FR_GOOFYSHOOT_RUSH", TargetId(1), Vec3::new(200.0, -180.0, 90.0), &mut host));
    assert!(!rig.party.fr_hit(d, "FR_GOOFYSHOOT_RUSH", TargetId(1), Vec3::ZERO, &mut host), "not Donald's row");
    assert!(!rig.party.fr_hit(g, "NO_SUCH_ROW", TargetId(1), Vec3::ZERO, &mut host));
    let hits = rig.hits("FR_GOOFYSHOOT_RUSH");
    assert_eq!(hits.len(), 1);
    assert!(hits[0].damage > 0.0);
}

#[test]
fn walkrun_blend_x_is_the_yaw_delta_and_the_warp_path_is_the_nav_length() {
    // UTresAICharState_Run 0x14045AAF0: X = the yaw delta (deg), clamped to WalkRunN's BlendParameters[0]
    assert_eq!(kh_party::pawn::walkrun_axis_x(Chara::Donald, 120.0), 120.0);
    assert_eq!(kh_party::pawn::walkrun_axis_x(Chara::Goofy, 179.0), 179.0);
    assert_eq!(kh_party::pawn::walkrun_axis_x(Chara::Goofy, 250.0), 180.0);
    assert_eq!(kh_party::pawn::walkrun_axis_x(Chara::Donald, -350.0), -300.0);
    // Warp tick 0x14126DD90: FindPathSync's path length; the port's nav stand-in is the straight distance
    let l = kh_party::pawn::leader_path_length(Vec3::ZERO, Vec3::new(300.0, 400.0, 0.0));
    assert_eq!(l, Some(500.0));
}

#[test]
fn an_item_on_a_knocked_out_friend_follows_the_item_path() {
    // (B4) UseItem's Timing notify (0x141512C80) -> FUN_1405BCBE0: the item is spent at the notify, the effect
    // lands 0.5 s later (FUN_1405BD190); ItemEffect 0x1405BE820: x (1 + 0.5 x the USER's ITEM_UP) x 0.5 on a target
    // in Die (the friends' pawn type 10 lets it through); the KO'd friend gets up.
    let mut rig = Rig::new(true, None);
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    let g = rig.party.find(Chara::Goofy).unwrap();
    let mut hit = HitRecord::empty(TargetId(99));
    hit.damage = 9999.0;
    hit.damage_kind = kh_core::enums::ETresDamageKind::Small;
    assert!(rig.party.receive_hit(d, &hit).killed);
    rig.step(2);
    let item = rig.party.status(g).unwrap().items.iter().find(|s| s.present && s.item == Some(ItemKind::Potion)).map(|s| s.item.unwrap());
    let item = item.unwrap_or(ItemKind::Potion);
    rig.party.status_mut(g).unwrap().set_item_slot(0, Some(item));
    let boost = 1.0 + 0.5 * rig.party.status(g).unwrap().count(kh_core::enums::ETresAbilityKind::ItemUp) as f32;
    let t = rig.party.command(g, MemberCommand::UseItem { item: ItemKind::Potion, target: TargetRef::Member(d) });
    let mut used_at = None;
    for i in 0..300 {
        rig.step(1);
        if used_at.is_none() && rig.events.iter().any(|e| matches!(e, PartyEventKind::ItemUsed { .. })) {
            used_at = Some(i);
            assert_eq!(rig.party.status(d).unwrap().hp, 0, "spent at the notify, not applied yet");
        }
        if rig.party.status(d).unwrap().hp > 0 {
            let dt = i - used_at.expect("used before the effect");
            assert!((29..=31).contains(&dt), "the effect lands 0.5 s after the use: {dt} ticks");
            break;
        }
    }
    let want = (0.5f32 * boost * 80.0 + 0.5).floor() as i32;
    assert_eq!(rig.party.status(d).unwrap().hp, want);
    assert_eq!(rig.run_until_done(g, t, 3.0), CommandStatus::Succeeded);
    rig.step((3.0 / DT) as usize);
    assert_ne!(rig.party.view(d).unwrap().state, NpcStateId::Die, "gets up");
}

#[test]
fn idle_finished_and_the_damage_reaction_count() {
    // (B4, C4 relay) UTresNpcState_Idle IdleFinished: out of battle at the idle clip's end, in battle once per
    // idle clip length, and on leaving idle (End 0x141245380)
    let mut rig = Rig::new(true, None);
    rig.step(2);
    let d = rig.party.find(Chara::Donald).unwrap();
    let t = rig.party.command(d, MemberCommand::Idle { time: f32::MAX });
    let len = rig.party.member(d).unwrap().anim.clip.length;
    let st = rig.run_until_done(d, t, len + 1.0);
    assert_eq!(st, CommandStatus::Succeeded, "IdleN length {len}");
    rig.party.set_in_battle(true);
    rig.step(2);
    let t = rig.party.command(d, MemberCommand::Idle { time: f32::MAX });
    let len = rig.party.member(d).unwrap().anim.clip.length;
    assert_eq!(rig.run_until_done(d, t, len + 0.5), CommandStatus::Succeeded);
    // leaving idle (a hit): the Idle task succeeds; the accepted reaction counts
    let t = rig.party.command(d, MemberCommand::Idle { time: f32::MAX });
    rig.step(1);
    assert_eq!(rig.party.function(d, "GetDamageReactionCount"), Some(0.0));
    let mut hit = HitRecord::empty(TargetId(9));
    hit.damage = 5.0;
    hit.damage_kind = kh_core::enums::ETresDamageKind::Small;
    hit.direction = Vec3::new(-1.0, 0.0, 0.0);
    rig.party.receive_hit(d, &hit);
    assert_eq!(rig.party.command_status(d, t), CommandStatus::Succeeded);
    assert_eq!(rig.party.function(d, "GetDamageReactionCount"), Some(1.0));
}

// ---------------------------------------------------------------- (B5) the jump, the ledges, the ice rails, the warp query

/// Steps until `f` holds (at most `n` steps); the clips seen on the way.
fn step_until(rig: &mut Rig, d: kh_party::MemberId, n: usize, f: impl Fn(&kh_party::MemberView) -> bool) -> (bool, Vec<&'static str>) {
    let mut clips: Vec<&'static str> = Vec::new();
    for _ in 0..n {
        rig.step(1);
        let v = rig.party.view(d).unwrap();
        if clips.last() != Some(&v.anim.clip) {
            clips.push(v.anim.clip);
        }
        if f(&v) {
            return (true, clips);
        }
    }
    (false, clips)
}

#[test]
fn walking_off_a_ledge_is_the_navlink_fall_and_a_long_drop_rolls() {
    // a 300 cm high platform ending at x = 200; the floor at 0 below
    let world = TestWorld::floor().with_box(Vec3::new(-2000.0, -2000.0, 0.0), Vec3::new(200.0, 2000.0, 300.0));
    let (mut rig, d) = donald_rig(world, Vec3::new(0.0, 0.0, 300.0));
    rig.step(5);
    rig.party.command(d, move_to(1500.0, 0.0, 0.0, LocomotionDef::Move1));
    // the Run tick 0x140442430: not on the ground -> RequestJump 0x14054C2E0 -> NavLinkJump (0x2B), the fall: no
    // JumpNormalDownStart in the set -> JumpNormalDownLoop (vt+0x398 0x140458D00, sub 4)
    let (ok, _) = step_until(&mut rig, d, 120, |v| v.state == NpcStateId::NavLinkJump);
    assert!(ok, "falls off the edge into NavLinkJump");
    assert_eq!(rig.party.view(d).unwrap().anim.clip, "JumpNormalDownLoop");
    assert_eq!(kh_party::ai_body::state_name(NpcStateId::NavLinkJump), "TSID_STATE_NAVLINK_JUMP");
    // landing (phase 3): a 300 cm drop is under 2000 -> `Land`, then the default state (the move runs: Move)
    let (ok, clips) = step_until(&mut rig, d, 240, |v| v.state == NpcStateId::Move);
    assert!(ok, "lands and runs on: {clips:?}");
    assert!(clips.contains(&"Land") && !clips.contains(&"RollLand"), "{clips:?}");
    assert!(rig.party.view(d).unwrap().position.z.abs() < 1.0);

    // 2100 cm: RollLand (+0x148 >= 2000, the set has it: +0x151)
    let world = TestWorld::floor().with_box(Vec3::new(-2000.0, -2000.0, 0.0), Vec3::new(200.0, 2000.0, 2100.0));
    let (mut rig, d) = donald_rig(world, Vec3::new(0.0, 0.0, 2100.0));
    rig.step(5);
    rig.party.command(d, move_to(1500.0, 0.0, 0.0, LocomotionDef::Move1));
    let (ok, clips) = step_until(&mut rig, d, 600, |v| v.state == NpcStateId::Move && v.position.z < 1.0);
    assert!(ok, "{clips:?}");
    assert!(clips.contains(&"RollLand"), "{clips:?}");
}

#[test]
fn the_idle_member_in_the_air_falls_through_navlink_jump() {
    let (mut rig, d) = donald_rig(TestWorld::floor(), Vec3::new(0.0, 0.0, 400.0));
    // spawned in the air: Idle -> the Idle tick 0x14124CD40 -> RequestJump
    let (ok, _) = step_until(&mut rig, d, 30, |v| v.state == NpcStateId::NavLinkJump);
    assert!(ok);
    // commands other than MoveTo / Stop are refused until the landing clip's EnableChange (vt+0x388 0x1404325F0)
    let t = rig.party.command(d, MemberCommand::Action { def: ActionDef::DonaldFire, target: TargetRef::Point(Vec3::new(500.0, 0.0, 0.0)) });
    assert_eq!(rig.party.command_status(d, t), CommandStatus::Failed);
    let (ok, clips) = step_until(&mut rig, d, 240, |v| v.state == NpcStateId::Idle);
    assert!(ok && clips.contains(&"Land"), "{clips:?}");
}

#[test]
fn a_code_driven_jump_flies_to_its_target() {
    let (mut rig, d) = donald_rig(TestWorld::floor(), Vec3::ZERO);
    rig.step(5);
    let w = rig.world.clone();
    // JumpCodeDriven (iface vt+0x3A8): the launch at JumpNormalStart's JumpStart notify, FUN_140578330 to the target with
    // h0 100; with the AI move feeding (+0xA4 = +0xA6) he steers along +0xA0 at |v.xy|
    rig.party.command(d, move_to(800.0, 0.0, 0.0, LocomotionDef::Move1));
    assert!(rig.party.request_jump_to(d, Vec3::new(600.0, 0.0, 0.0), &w));
    assert_eq!(state_of(&rig, d), NpcStateId::NavLinkJump);
    let (ok, clips) = step_until(&mut rig, d, 300, |v| v.state != NpcStateId::NavLinkJump);
    assert!(ok, "{clips:?}");
    assert_eq!(clips.first(), Some(&"JumpNormalStart"), "{clips:?}");
    let x = rig.party.view(d).unwrap().position.x;
    assert!(x > 450.0, "lands near the target: x {x}");
}

#[test]
fn the_run_climbs_an_auto_run_ledge() {
    use kh_core::surface;
    // a 120 cm box ahead (100 < h <= 150: the middle class, ClimbingMR, lift 0.25), its top flagged SQEX_bEnableAutoRun
    let world = TestWorld::floor().with_box(Vec3::new(300.0, -400.0, 0.0), Vec3::new(1200.0, 400.0, 120.0));
    let c = world.last_box();
    let world = world.with_flags(c, surface::AUTO_RUN);
    let (mut rig, d) = donald_rig(world, Vec3::ZERO);
    rig.step(5);
    rig.party.command(d, move_to(1000.0, 0.0, 120.0, LocomotionDef::Move1));
    // the Run tick's climb probe (FUN_140634E60 -> FUN_140636BF0: sphere 35 from 260 down to 80, 70 ahead) -> vt+0x538
    let (ok, _) = step_until(&mut rig, d, 240, |v| v.state == NpcStateId::Climbing);
    assert!(ok, "climbs");
    rig.step(1);
    assert_eq!(rig.party.view(d).unwrap().anim.clip, "ClimbingMR");
    assert_eq!(kh_party::ai_body::state_name(NpcStateId::Climbing), "TSID_STATE_CLIMBING");
    // the traction lift and the clip's root motion put him on the top; LightLandR (sub 4), then the run again
    let (ok, clips) = step_until(&mut rig, d, 240, |v| v.state == NpcStateId::Move);
    assert!(ok, "{clips:?}");
    assert!(clips.contains(&"LightLandR"), "{clips:?}");
    rig.step(10);
    let v = rig.party.view(d).unwrap();
    assert!((v.position.z - 120.0).abs() < 2.0 && v.position.x > 300.0, "on the top: {:?}", v.position);
    // without the flag nothing is climbed (the probe needs SQEX_bEnableAutoRun on the top)
    let world = TestWorld::floor().with_box(Vec3::new(300.0, -400.0, 0.0), Vec3::new(1200.0, 400.0, 120.0));
    let (mut rig, d) = donald_rig(world, Vec3::ZERO);
    rig.step(5);
    rig.party.command(d, move_to(1000.0, 0.0, 120.0, LocomotionDef::Move1));
    let (climbed, _) = step_until(&mut rig, d, 120, |v| v.state == NpcStateId::Climbing);
    assert!(!climbed);
}

#[test]
fn the_ledge_probe_and_the_dangle_state() {
    use kh_core::surface;
    // a 600 cm wall from x = 400, its top edge hangable (SQEX_bEnableDangle)
    let world = TestWorld::floor().with_box(Vec3::new(400.0, -400.0, 0.0), Vec3::new(1000.0, 400.0, 600.0));
    let c = world.last_box();
    let world = world.with_flags(c, surface::DANGLE);
    let (mut rig, d) = donald_rig(world, Vec3::new(370.0, 0.0, 450.0));
    let w = rig.world.clone();
    // FUN_140638740 with the jump's arguments: Donald's m_DangleOffsetPos (-30, 0, -70) -> up 195, down 50
    let pw = rig.party.member(d).unwrap();
    let up = kh_party::climb::dangle_probe_up(Chara::Donald);
    // (party I1) an NPC runs attempt 3 only (FUN_140638900 0x140639360): a sphere of 1.5 x 30 on his own axis, so
    // 70 cm from the wall nothing is found (the player's sphere 50 cm ahead would have); against the wall it is
    assert_eq!(kh_party::climb::ledge_probe_radius(pw), 45.0);
    assert!(kh_party::climb::ledge_probe(pw, &w, Vec3::new(330.0, 0.0, 450.0), 0.0, up, 50.0).is_none(), "attempts 0..2 are the player's");
    let ledge = kh_party::climb::ledge_probe(pw, &w, Vec3::new(370.0, 0.0, 450.0), 0.0, up, 50.0).expect("the edge at his axis");
    assert!((ledge.contact.z - 600.0).abs() < 1.0 && (ledge.contact.x - 400.0).abs() < 2.0, "{ledge:?}");
    assert!((ledge.position.z - 530.0).abs() < 1.0 && (ledge.position.x - 370.0).abs() < 2.0, "hang 30 back, 70 down: {ledge:?}");
    // RequestDangle (iface vt+0x208): GripStart (Custom mode, hung), GripLoop; the AI move toward the top (within 90 deg)
    // climbs: GripRise, then the default state
    let pw = rig.party.member_mut(d).unwrap();
    kh_party::climb::request_dangle(pw, ledge, false, &w);
    assert_eq!(state_of(&rig, d), NpcStateId::Dangle);
    rig.step(2);
    assert_eq!(rig.party.view(d).unwrap().anim.clip, "GripStart");
    let (ok, clips) = step_until(&mut rig, d, 120, |v| v.anim.clip == "GripLoop");
    assert!(ok, "{clips:?}");
    rig.party.command(d, move_to(700.0, 0.0, 600.0, LocomotionDef::Move1));
    let (ok, clips) = step_until(&mut rig, d, 240, |v| v.state != NpcStateId::Dangle);
    assert!(ok && clips.contains(&"GripRise"), "{clips:?}");
}

#[test]
fn the_friends_ride_sora_s_ice_rails() {
    // a Blizzard ice rail (kh-core magic::RailNode chain) next to Donald: the shared rail probe finds it
    let node = |id: u32, previous: u32, x: f32| kh_core::magic::RailNode { id, previous, position: Vec3::new(x, 0.0, 0.0), tangent: Vec3::X, life: 3.0 };
    let (mut rig, d) = donald_rig(TestWorld::floor(), Vec3::ZERO);
    rig.sora.position = Vec3::new(2000.0, 0.0, 0.0);
    rig.step(3);
    rig.party.set_ice_rails(&[node(11, 0, 100.0), node(12, 11, 600.0), node(13, 12, 1600.0)]);
    rig.party.command(d, move_to(3000.0, 0.0, 0.0, LocomotionDef::Move1));
    let (ok, _) = step_until(&mut rig, d, 120, |v| v.state == NpcStateId::RailSlide);
    assert!(ok, "rides the ice rail");
    let (ok, clips) = step_until(&mut rig, d, 120, |v| v.anim.clip == "RailSlideLoop");
    assert!(ok, "{clips:?}");
    // the rail goes with its nodes
    rig.party.set_ice_rails(&[]);
    rig.step(2);
    assert_ne!(state_of(&rig, d), NpcStateId::RailSlide);
}

#[test]
fn the_off_rail_warp_runs_the_friend_data_query() {
    let node = |x: f32| kh_core::RailNode { position: Vec3::new(x, 0.0, 0.0), speed: None, detect_range_plus: 0.0 };
    let mut world = TestWorld::floor();
    world.rails.push(kh_core::Rail { id: 7, nodes: vec![node(200.0), node(20200.0)], start_dir: 0, no_reverse: false, no_jump: false, narrow: false });
    let (mut rig, d) = donald_rig(world, Vec3::ZERO);
    rig.sora.position = Vec3::new(-5000.0, 0.0, 0.0);
    rig.step(3);
    rig.party.command(d, move_to(30000.0, 0.0, 0.0, LocomotionDef::Move1));
    let (ok, _) = step_until(&mut rig, d, 120, |v| v.state == NpcStateId::RailSlide);
    assert!(ok);
    // farther than 3000 from the leader: the state asks for TresFriendData.WarpQuery (0x14124E890 -> 0x143C5A420)
    rig.step(30);
    assert!(rig.party.warp_query_wanted(d));
    {
        let body = kh_party::ai_body::PartyBody { party: &mut rig.party, member: d };
        assert!(kh_ai::AiBody::wants_warp_query(&body));
    }
    // the delegate 0x141250A00: the point kept, 1 s delay, then (out of view: no camera) the move there
    let point = Vec3::new(-4800.0, 0.0, 0.0);
    rig.party.answer_warp_query(d, Some(point));
    assert!(!rig.party.warp_query_wanted(d));
    rig.step((1.2 / DT) as usize);
    let v = rig.party.view(d).unwrap();
    assert!(v.position.distance(point) < 5.0, "warped to the query's point: {:?}", v.position);
}

#[test]
fn the_slope_slide_pitch_is_in_the_view() {
    use kh_core::surface;
    let a = 30.0f32.to_radians();
    let n = Vec3::new(a.sin(), 0.0, a.cos());
    let world = TestWorld::default().with_plane_through(Vec3::ZERO, n).with_flags(1, surface::SLOPE_SLIDE);
    let (mut rig, d) = donald_rig(world, Vec3::ZERO);
    rig.step(3);
    rig.party.command(d, move_to(3000.0, 0.0, -3000.0 * a.tan(), LocomotionDef::SlopeSlide));
    rig.step(80);
    let v = rig.party.view(d).unwrap();
    assert_eq!(v.state, NpcStateId::SlopeSlide);
    // the tick 0x14126C950 eases the pitch toward the slope's (-30) + 25 at dt x 6
    assert!((v.pitch + 5.0).abs() < 1.0, "pitch {}", v.pitch);
}

/// (party I1) The game's request gates: RequestAction 0x140547B40 -> the definition's CanStart 0x140518070 (IDLE or
/// a state of m_ViableStates: AI_MOVE for every friend action), the TurnTo gate FUN_140530C80 (IDLE / TURN), and the
/// warp set directly by the pawn tick 0x14127B980 (no gate). A falling friend refuses actions and turns but takes
/// the warp; a running action is not replaced by another (no AI_ActionAbortTiming id in the friends' trees).
#[test]
fn i1_the_request_gates_follow_the_game() {
    let (mut rig, d) = donald_rig(TestWorld::floor(), Vec3::new(0.0, 0.0, 400.0));
    let (ok, _) = step_until(&mut rig, d, 30, |v| v.state == NpcStateId::NavLinkJump);
    assert!(ok);
    let fire = MemberCommand::Action { def: ActionDef::DonaldFire, target: TargetRef::Point(Vec3::new(500.0, 0.0, 0.0)) };
    let t = rig.party.command(d, fire);
    assert_eq!(rig.party.command_status(d, t), CommandStatus::Failed, "no action in NAVLINK_JUMP (not in m_ViableStates)");
    let t = rig.party.command(d, MemberCommand::TurnTo { goal: MoveGoal::Point(Vec3::new(0.0, 500.0, 0.0)) });
    assert_eq!(rig.party.command_status(d, t), CommandStatus::Failed, "no turn in NAVLINK_JUMP (FUN_140530C80)");
    let t = rig.party.command(d, MemberCommand::Warp { goal: Vec3::new(300.0, 0.0, 0.0), yaw: 0.0 });
    assert_ne!(rig.party.command_status(d, t), CommandStatus::Failed, "the warp is set directly (0x14127B980)");
    // on the ground: an action starts from Idle; a second one while it runs is refused
    let (mut rig, d) = donald_rig(TestWorld::floor(), Vec3::ZERO);
    rig.step(5);
    let t1 = rig.party.command(d, MemberCommand::Action { def: ActionDef::DonaldBeat, target: TargetRef::Point(Vec3::new(200.0, 0.0, 0.0)) });
    rig.step(3);
    assert_eq!(rig.party.view(d).unwrap().state, NpcStateId::Action);
    let t2 = rig.party.command(d, MemberCommand::Action { def: ActionDef::DonaldSwing, target: TargetRef::Point(Vec3::new(200.0, 0.0, 0.0)) });
    assert_eq!(rig.party.command_status(d, t2), CommandStatus::Failed, "a running action is not replaced (FUN_140520410)");
    assert_eq!(rig.party.command_status(d, t1), CommandStatus::Running);
}

/// (party I1) The launch's ledge search (0x140433FD0) with the NPC ledge probe (FUN_140638900 attempt 3: a sphere of
/// 1.5 x the capsule radius on the arc point's axis): a target on top of a 700 cm wall (more than +0xB34 500 above
/// him) is reached by aiming at the ledge where the arc meets the wall, then hanging there (Dangle).
#[test]
fn i1_the_launch_finds_the_ledge_where_the_arc_meets_the_wall() {
    use kh_core::surface;
    let world = TestWorld::floor().with_box(Vec3::new(400.0, -400.0, 0.0), Vec3::new(1000.0, 400.0, 700.0));
    let c = world.last_box();
    let world = world.with_flags(c, surface::DANGLE);
    let (mut rig, d) = donald_rig(world, Vec3::new(100.0, 0.0, 0.0));
    rig.step(5);
    let w = rig.world.clone();
    let pw = rig.party.member_mut(d).unwrap();
    assert_eq!(kh_party::climb::ledge_arc(pw, Vec3::new(600.0, 0.0, 700.0), &w), Some(true), "a ledge on the arc");
    let (mut rig, d) = donald_rig(rig.world.clone(), Vec3::new(100.0, 0.0, 0.0));
    rig.step(5);
    let w = rig.world.clone();
    rig.party.command(d, move_to(600.0, 0.0, 700.0, LocomotionDef::Move1));
    assert!(rig.party.request_jump_to(d, Vec3::new(600.0, 0.0, 700.0), &w));
    let (ok, clips) = step_until(&mut rig, d, 400, |v| v.state == NpcStateId::Dangle);
    assert!(ok, "hangs from the ledge: {clips:?}");
}

/// (party I1) A test world whose one box moves (a lift): the queries are made in the box's frame, the hits moved back.
struct Lift {
    inner: TestWorld,
    offset: Vec3,
    gone: bool,
}

impl kh_core::World for Lift {
    fn sweep_capsule(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Option<HitRecordWorld> {
        self.sweep_capsule_all(start, end, radius, half_height).into_iter().next()
    }
    fn sweep_capsule_all(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Vec<HitRecordWorld> {
        if self.gone {
            return Vec::new();
        }
        self.inner.sweep_capsule_all(start - self.offset, end - self.offset, radius, half_height).into_iter().map(|mut h| {
            h.position += self.offset;
            h
        }).collect()
    }
    fn line_trace(&self, start: Vec3, end: Vec3) -> Option<HitRecordWorld> {
        if self.gone {
            return None;
        }
        self.inner.line_trace(start - self.offset, end - self.offset).map(|mut h| {
            h.position += self.offset;
            h
        })
    }
    fn surface_flags(&self, component: u32) -> u32 {
        self.inner.surface_flags(component)
    }
    fn component_motion(&self, component: u32) -> Option<kh_core::ComponentMotion> {
        (component == 1).then(|| kh_core::ComponentMotion { location: self.offset, rotation: glam::Quat::IDENTITY, scale: Vec3::ONE, movable: true })
    }
}
type HitRecordWorld = kh_core::Hit;

/// (party I1) Moving ledges (UTresCharState_Dangle Begin 0x141508DE0 / tick 0x14150F220): the hang point is kept in
/// the component's space (+0xA4), a Movable ledge carries the hanging friend (the base, vt+0xA60), and the STAY tick's
/// support test FUN_140638690 lets him go when no ledge top is left under the contact.
#[test]
fn i1_a_moving_ledge_carries_the_hanging_friend() {
    use kh_core::surface;
    let inner = TestWorld::default().with_box(Vec3::new(400.0, -400.0, 0.0), Vec3::new(1000.0, 400.0, 600.0));
    let c = inner.last_box();
    assert_eq!(c, 1);
    let mut lift = Lift { inner: inner.with_flags(c, surface::DANGLE), offset: Vec3::ZERO, gone: false };
    let (mut rig, d) = donald_rig(TestWorld::floor(), Vec3::new(370.0, 0.0, 450.0));
    let up = kh_party::climb::dangle_probe_up(Chara::Donald);
    let ledge = kh_party::climb::ledge_probe(rig.party.member(d).unwrap(), &lift, Vec3::new(370.0, 0.0, 450.0), 0.0, up, 50.0).expect("the lift's edge");
    kh_party::climb::request_dangle(rig.party.member_mut(d).unwrap(), ledge, false, &lift);
    assert_eq!(state_of(&rig, d), NpcStateId::Dangle);
    let z0 = rig.party.view(d).unwrap().position.z;
    let step = |rig: &mut Rig, lift: &Lift| {
        let mut host = PartyHost { world: lift, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
        rig.party.step(DT, &rig.sora, &mut host);
    };
    for i in 0..30 {
        lift.offset = Vec3::new(0.0, 0.0, 2.0 * (i + 1) as f32);
        step(&mut rig, &lift);
    }
    let v = rig.party.view(d).unwrap();
    assert_eq!(v.state, NpcStateId::Dangle);
    assert!((v.position.z - (z0 + 60.0)).abs() < 0.5, "carried up 60: {} -> {}", z0, v.position.z);
    // the ledge top is gone from under the contact: from the input block on (m_bEnableChange / GripLoop) the STAY
    // support test fails and he lets go
    lift.gone = true;
    let mut left = false;
    for _ in 0..240 {
        step(&mut rig, &lift);
        left |= rig.party.view(d).unwrap().state != NpcStateId::Dangle;
    }
    assert!(left, "no support: the hang ends");
}
