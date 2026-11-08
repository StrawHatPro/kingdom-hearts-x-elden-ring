//! (MIMIC-BOSS-KIT) THE COPIES' FULL KIT through the real cores and the real trees (the coordinator's rule: "a COPY has the
//! copied leader's FULL kit, under the same requirements the real character has"; THE USER 2026-11-05: "mimic tear boss
//! Donald should be able to use Zettaflare if it manages to reach the requirements during combat just like Donald"): the boss's
//! Sora and the summoned Mimic Sora form-change, switch Keyblades and use their Y commands (a reaction command too); the boss's
//! Donald casts Zettaflare at the party once its requirement is met (never before), with no cinematic; the boss's Goofy uses
//! his Arts / Breaker; nobody of them summons. The AI pack at build/sora_pack/party/ai (skipped without it, said on stderr).

use kh_core::host::{HitRecord, HitResult, HurtShape, TargetId, TargetInfo};
use kh_core::input::PadButton;
use kh_core::status::{KeybladeId, LoadoutValue};
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Pose, Sora, Status, Vec3, SORA_PARAMS};
use kh_host::formation::Formation;
use kh_host::party::PartyChara;
use kh_party::donald::zettaflare::{self, RunKind, ZettaLog};

use crate::mimicboss::*;
use crate::mimicboss::Victim;
use crate::*;

fn level99() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim_start_matches('\u{feff}')).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let v = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64().unwrap()),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(a) => LoadoutValue::List(a.iter().map(|i| i.as_str().unwrap().to_string()).collect()),
            serde_json::Value::Object(m) => LoadoutValue::Map(m.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap())).collect()),
            _ => continue,
        };
        loadout.set(row[0].as_str().unwrap(), &v).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
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
    let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party/ai");
    let ai_pack = kh_ai::AiPack::load_dir(&pack).ok().map(std::sync::Arc::new);
    if ai_pack.is_none() {
        eprintln!("no AI pack at build/sora_pack/party/ai: skipped");
        return None;
    }
    Some(PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack, ai_pack_source: None }))
}

/// The leader Sora at level 99 with three Keyblades in his slots (the Kingdom Key and the next two of the sheet).
fn sora_at(p: Vec3) -> Sora {
    let mut s = Sora::new(SORA_PARAMS, Pose { position: p, yaw: 0.0 }).with_status(level99());
    s.command_mut().fill(&level99());
    let more: Vec<KeybladeId> = KeybladeId::all().filter(|k| *k != KeybladeId::KINGDOM_KEY).take(2).collect();
    for (slot, id) in more.into_iter().enumerate() {
        s.status_mut().set_keyblade_slot(slot + 1, Some(id)).unwrap();
    }
    assert_eq!(s.status().keyblades().filled(), 3, "three Keyblades to switch between");
    s
}

fn party_step(rt: &mut PartyRuntime, sora: &mut Sora, world: &TestWorld, asked: Option<Formation>) {
    let cam = camera();
    let mut targets = NoEnemies;
    let input = Input::default();
    let _ = rt.route(RouteIn { sora, input: &input, dt: 1.0 / 60.0, targets: &targets, camera: &cam, asked_formation: asked, asked_ai: Vec::new(), nav: None, now: 0.0 });
    let out = sora.output();
    let mut audio = kh_core::host::audio::NullAudio;
    let mut effects = kh_core::host::effects::NullEffects;
    rt.step(StepIn { dt: 1.0 / 60.0, sora, out: &out, lock_on: TargetId::NONE, camera: &cam, world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: 0.0 });
    let _ = rt.after_step(&NoEnemies, 1.0 / 60.0, 0.0);
}

fn victim_of(c: PartyChara, s: &Sora) -> Victim {
    let o = s.output();
    let st = s.status();
    Victim { chara: c, feet: o.position, yaw: o.yaw, velocity: o.velocity, radius: 34.0, half_height: 90.0, hp: st.hp, hp_max: st.max_hp(), defense: st.defense(), airborne: !o.on_floor, down: st.hp < 1, tell: Default::default(), stance: Default::default() }
}

fn victim_of_member(v: &kh_party::MemberView, defense: i32) -> Victim {
    let c = crate::runtime::party_chara(v.chara);
    Victim { chara: c, feet: v.position, yaw: v.yaw, velocity: v.velocity, radius: v.capsule.0.max(30.0), half_height: v.capsule.1.max(60.0), hp: v.hp, hp_max: v.hp_max, defense, airborne: !v.on_ground, down: v.ko, tell: Default::default(), stance: Default::default() }
}

/// What a Sora copy did in a run.
#[derive(Debug, Default)]
struct Kit {
    landed: usize,
    /// The first form it changed into (its own formchange: it started in none).
    form: Option<i32>,
    switches: u32,
    /// The situation commands its Y took (gone from its queue after the press), in order.
    used: Vec<i32>,
    /// It was in AIR_SLIDE after the blow (Aerial Recovery) and its reaction command, Blow Counter (ex kind 2), was used.
    recovered: bool,
    blow_counter: bool,
}

/// THE BLOW at a copy (a party member's launcher as its KH3 reaction, crate::mimicboss `reaction_of`), from `from`.
fn blow_from(from: Vec3, at: Vec3) -> kh_core::states::HitOnPlayer {
    let mut h = HitRecord::empty(TargetId(1));
    h.damage_kind = kh_core::enums::ETresDamageKind::Blow;
    h.direction = (at - from).normalize_or(Vec3::X);
    h.damage = 10.0;
    h.attacker = kh_core::host::HitAttacker { position: from, yaw: 0.0, radius: 34.0, valid: true };
    reaction_of(&h)
}

/// A Sora copy (`copy`, driven by `drive`) against a target standing as `victim` for `steps` fixed steps; a blow at it at
/// `blow_at`; a host opening its Summon at step 30 (shut again at its next AI step). `hit`: what a hit of the copy does.
fn sora_kit_run(copy: &mut Sora, steps: usize, blow_at: usize, victim: &mut dyn FnMut() -> Victim, drive: &mut dyn FnMut(&mut Sora, &BossTargets, f64) -> Input, hit: &mut dyn FnMut(&HitRecord, Vec3, f32)) -> Kit {
    let world = TestWorld::floor();
    let dt = 1.0 / 60.0;
    let mut kit = Kit::default();
    let switches0 = copy.status().keyblade_switches;
    let mut since_blow: Option<usize> = None;
    for k in 0..steps {
        let mut t = BossTargets { victims: vec![victim()], hits: Vec::new(), attack_rate: 1.0, orders: Vec::new() };
        // (the blow is given every half second from `blow_at` until the copy is in its blow: a hit in a command's 0.1 s reaction
        // block, a cast's armour or its invincible frames is not taken)
        if k >= blow_at && since_blow.is_none() {
            if copy.damage_reaction().is_some_and(|d| d.kind == kh_core::enums::ETresDamageKind::Blow) {
                since_blow = Some(0);
            } else if (k - blow_at).is_multiple_of(30) {
                let at = copy.output().position;
                copy.take_hit(blow_from(t.victims[0].feet, at));
            }
        }
        if k == 30 {
            copy.command_mut().summon.allowed = true;
            copy.command_mut().spirit_ashes = [207_000, 0, 0, 0];
        }
        let input = drive(copy, &t, k as f64 * dt as f64);
        assert!(!copy.command().summon.allowed && copy.command().spirit_ashes == [0; 4], "a copy never summons (its Summon rules shut at its AI step)");
        assert!(!input.pad.pressed.has(PadButton::DPadUp) && !input.pad.pressed.has(PadButton::DPadDown), "no menu move toward Summon");
        let before = copy.combat().situation.current().filter(|_| input.pad.pressed.has(PadButton::FaceTop));
        let out = {
            let (mut audio, mut effects, mut ui) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects, kh_core::host::NullUi);
            let mut host = kh_core::Host { world: &world, targets: &mut t, audio: &mut audio, effects: &mut effects, ui: &mut ui };
            copy.step_hosted(dt, &input, &mut host)
        };
        assert!(copy.command().main_cursor != 3, "the Summon row is never under its cursor");
        if let Some((_, kind)) = before {
            if !copy.combat().situation.has(kind) {
                kit.used.push(kind);
                kit.blow_counter |= kind == 2 && since_blow.is_some_and(|s| s < 240);
            }
        }
        if let Some(s) = since_blow.as_mut() {
            *s += 1;
            kit.recovered |= out.state == kh_core::StateId::AirSlide && *s < 120;
        }
        let f = copy.combat().form.cmd;
        if f != 0 && kit.form.is_none() {
            kit.form = Some(f);
        }
        for h in t.hits.drain(..) {
            hit(&h, out.position, out.yaw);
            kit.landed += 1;
        }
    }
    kit.switches = copy.status().keyblade_switches.wrapping_sub(switches0);
    kit
}

fn assert_full_kit(who: &str, kit: &Kit) {
    eprintln!("{who}: {kit:?}");
    assert!(kit.landed > 0, "{who}: its hits landed");
    assert!(kit.form.is_some(), "{who}: it form-changed by itself (its own gauge, its own Y): {kit:?}");
    assert!(kit.switches > 0, "{who}: it switched Keyblades (D-pad right on Attack): {kit:?}");
    assert!(kit.recovered, "{who}: Aerial Recovery out of the blow: {kit:?}");
    assert!(kit.blow_counter, "{who}: its reaction command (Blow Counter) by Y: {kit:?}");
    use kh_core::combat::situation::{class, class_of};
    assert!(kit.used.iter().all(|k| !matches!(class_of(*k), class::TEAM | class::LINK) || *k == 2), "{who}: never a team row or a Link: {:?}", kit.used);
}

// ---------------------------------------------------------------------------------------------- the boss's Sora

#[test]
fn kit_the_boss_sora_form_changes_switches_keyblades_and_uses_its_reaction_command() {
    let Some(rt) = runtime() else { return };
    let mut lead = sora_at(Vec3::new(500.0, 0.0, 0.0));
    lead.status_mut().hp_limit_one = true; // (the leader stands the whole run)
    let mut boss = copy_sora(&lead, Pose { position: Vec3::ZERO, yaw: 0.0 });
    assert_eq!(boss.combat().form.cmd, 0, "the leader is in no form: the copy neither");
    let mut ai = BossAi::new(&rt);
    let cam = camera();
    let world = TestWorld::floor();
    let lead = std::cell::RefCell::new(lead);
    let kit = sora_kit_run(
        &mut boss,
        60 * 90,
        60 * 8,
        &mut || victim_of(PartyChara::Sora, &lead.borrow()),
        &mut |b, t, now| ai.input(&rt, b, BossScene { targets: t, camera: &cam, camera_yaw: 0.0, nav: None, dt: 1.0 / 60.0, now }),
        &mut |h, at, yaw| {
            let mut l = lead.borrow_mut();
            assert_eq!(h.target, victim_id(PartyChara::Sora));
            l.take_hit(player_hit_of(h, at, yaw));
            l.step(1.0 / 60.0, &Input::default(), &world);
        },
    );
    assert_full_kit("the boss's Sora", &kit);
}

// ---------------------------------------------------------------------------------------------- the summoned Mimic Sora

#[test]
fn kit_the_summoned_mimic_sora_has_the_same_full_kit_on_the_players_side() {
    let Some(rt) = runtime() else { return };
    assert!(rt.leader.is_none(), "the player's Sora leads");
    let player = sora_at(Vec3::new(-300.0, 0.0, 0.0));
    let kit0 = crate::mimic::copy_kit(&player, 0);
    let mut mimic = crate::mimic::make_core(&kit0, Pose { position: Vec3::ZERO, yaw: 0.0 });
    assert_eq!(mimic.status().keyblades().filled(), 3, "his Keyblades copied");
    let mut ai = MimicAi::new(&rt);
    let cam = camera();
    let world = TestWorld::floor();
    // the enemy: a dummy 6 m ahead (never down), the player's Sora standing behind
    let enemy = Victim { chara: PartyChara::Goofy, feet: Vec3::new(600.0, 0.0, 0.0), yaw: 180.0, velocity: Vec3::ZERO, radius: 50.0, half_height: 90.0, hp: 1000, hp_max: 1000, defense: 0, airborne: false, down: false, tell: Default::default(), stance: Default::default() };
    let player = std::cell::RefCell::new(player);
    let kit = sora_kit_run(
        &mut mimic,
        60 * 90,
        60 * 8,
        &mut || enemy,
        &mut |m, t, now| {
            let mut p = player.borrow_mut();
            p.step(1.0 / 60.0, &Input::default(), &world);
            ai.input(&rt, m, MimicScene { sora: &p, lock_on: TargetId::NONE, targets: t, camera: &cam, camera_yaw: 0.0, nav: None, dt: 1.0 / 60.0, now })
        },
        &mut |h, _, _| assert_eq!(h.target, victim_id(PartyChara::Goofy), "the enemy, never the player"),
    );
    assert_full_kit("Mimic Sora", &kit);
}

#[test]
fn kit_mimic_sora_leaves_a_form_kept_in_another_keyblade_slot_out() {
    let mut player = sora_at(Vec3::ZERO);
    let mut k = *player.status().keyblades();
    // (a form waiting in a slot is P2's run-time store, made by a switch out of a form): the copy takes the slots through the
    // save's keys, which keep none
    let (slots, levels) = (k.slot_keys(), k.levels_by_key());
    let (fresh, notes) = kh_core::status::Keyblades::from_keys(&slots, 1, &levels);
    assert!(notes.is_empty(), "{notes:?}");
    assert!(fresh.slot_form(0).is_none() && fresh.slot_form(1).is_none() && fresh.slot_form(2).is_none(), "the save's keys keep no stored form");
    k.set_current(1).unwrap();
    player.status_mut().set_keyblades(k);
    let kit = crate::mimic::copy_kit(&player, 0);
    assert_eq!(kit.status.keyblades().slots(), player.status().keyblades().slots(), "his Keyblades, slot by slot");
    assert_eq!(kit.status.keyblades().current_slot(), 1, "the one he holds");
    assert!((0..3).all(|s| kit.status.keyblades().slot_form(s).is_none()), "no stored form comes with the copy");
}

// ---------------------------------------------------------------------------------------------- the friends' copies

/// A friend leader's real party (Sora, Donald, Goofy; `leader` leads, at level 99 with his whole kit), the solo fight begun,
/// and his boss copy at -7 m.
fn friend_fight(leader: PartyChara) -> Option<(PartyRuntime, Sora, FriendBoss)> {
    let mut rt = runtime()?;
    let mut sora = sora_at(Vec3::ZERO);
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
    assert!(is_marked_copy(&boss), "FriendBoss::new marks its member a copy");
    assert!(!rt.party.member(id).unwrap().don_zetta.copy, "the real leader is no copy");
    Some((rt, sora, boss))
}

/// One step of a friend copy against the real leader (its hits through the leader's own `receive_hit`); the forced hits
/// (Zettaflare's, `HIT_FLAG_FORCE`) that reached him.
fn friend_step(rt: &mut PartyRuntime, sora: &mut Sora, boss: &mut FriendBoss, leader: PartyChara, k: usize) -> (usize, usize) {
    let world = TestWorld::floor();
    let cam = camera();
    let dt = 1.0 / 60.0;
    let lead_id = rt.id(leader).unwrap();
    let lv = rt.current.iter().find(|v| v.id == lead_id).copied().unwrap();
    let mut t = BossTargets { victims: vec![victim_of_member(&lv, rt.party.status(lead_id).unwrap().defense())], hits: Vec::new(), attack_rate: 1.0, orders: Vec::new() };
    let (mut audio, mut effects) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects);
    boss.step(FriendBossStep { dt, world: &world, targets: &mut t, audio: &mut audio, effects: &mut effects, camera: &cam, nav: None, now: k as f64 * dt as f64 });
    let (mut landed, mut forced) = (0, 0);
    for h in t.hits.drain(..) {
        assert_eq!(h.target, victim_id(leader), "its hits land on the party, never the boss side");
        forced += (h.flags & kh_core::host::HIT_FLAG_FORCE != 0) as usize;
        let _ = rt.receive_hit(leader, &h);
        landed += 1;
    }
    party_step(rt, sora, &world, None);
    (landed, forced)
}

#[test]
fn kit_the_boss_donald_casts_zettaflare_at_the_party_once_he_meets_its_requirement() {
    let Some((mut rt, mut sora, mut boss)) = friend_fight(PartyChara::Donald) else { return };
    let id = boss.id().unwrap();
    // the party's own AI Donald may never cast it (the user, 2026-10-22); the copy may, under the leader's requirements
    assert!(zettaflare::may_cast_zettaflare(boss.rt.party.member(id).unwrap()));
    // 1. ten seconds of the fight with the requirement NOT met: no Zettaflare
    for k in 0..600 {
        friend_step(&mut rt, &mut sora, &mut boss, PartyChara::Donald, k);
        let ready = boss.rt.party.status(id).and_then(|s| s.donald.as_ref()).is_some_and(|st| st.core.zettaflare_ready(&st.learned, &kh_party::donald::data()));
        assert!(ready || boss.rt.party.don_zetta.runs.is_empty(), "no Zettaflare before its requirement (step {k})");
    }
    // 2. the requirement met (a full Arcane Flow, every learned element at Resonance 3 this fight, MP): Zettaflare at the party
    let lead_id = rt.id(PartyChara::Donald).unwrap();
    let lead_hp0 = rt.party.status(lead_id).unwrap().hp;
    assert_eq!(zettaflare::debug_make_ready(&mut boss.rt.party), 1);
    let (mut begun, mut forced, mut charged) = (false, 0, false);
    for k in 600..600 + 60 * 40 {
        let (_, f) = friend_step(&mut rt, &mut sora, &mut boss, PartyChara::Donald, k);
        forced += f;
        if let Some(run) = boss.rt.party.don_zetta.runs.iter().find(|r| r.kind == RunKind::Zetta) {
            begun = true;
            assert!(!run.cinematic, "a copy's Zettaflare never takes the camera");
            let s = boss.rt.party.status(id).unwrap();
            charged |= s.mp == 0 && s.mp_charge_left > 0.0;
        }
        assert!(rt.party.don_zetta.runs.is_empty(), "the real party plays no Zettaflare of the copy's");
        if forced > 0 && boss.rt.party.don_zetta.runs.is_empty() {
            break;
        }
    }
    assert!(begun, "the copy cast Zettaflare once its requirement was met");
    assert!(boss.rt.party.don_zetta.log.iter().any(|e| matches!(e, ZettaLog::Begun { caster, .. } if *caster == id)), "its own party's run");
    assert!(charged, "its whole MP spent, MP Charge after (Donald's rule)");
    assert!(forced > 0, "Zettaflare's hit reached the leader");
    assert!(rt.party.status(lead_id).unwrap().hp < lead_hp0, "through his own damage path");
    assert!(boss.rt.party.don_zetta.look_asked.is_none(), "no cinematic look asked");
}

#[test]
fn kit_the_boss_goofy_uses_his_arts_or_breaker_on_the_party() {
    let Some((mut rt, mut sora, mut boss)) = friend_fight(PartyChara::Goofy) else { return };
    let id = boss.id().unwrap();
    let mut words = Vec::new();
    let mut landed = 0;
    for k in 0..60 * 45 {
        landed += friend_step(&mut rt, &mut sora, &mut boss, PartyChara::Goofy, k).0;
        if let Some(w) = boss.rt.party.gof.ai.iter().find(|m| m.member == id).and_then(|m| m.last) {
            if words.last() != Some(&w) {
                words.push(w);
            }
        }
    }
    eprintln!("the boss's Goofy: {landed} hits; his words {words:?}");
    use kh_party::goofy::ai::AiWord;
    assert!(landed > 0, "his hits landed");
    assert!(words.iter().any(|w| matches!(w, AiWord::Art(_) | AiWord::Breaker | AiWord::Taunt | AiWord::Interpose)), "his Arts (or the Breaker): {words:?}");
}

// ---------------------------------------------------------------------------------------------- nobody summons

#[test]
fn kit_ai_sora_and_the_friends_never_reach_summon() {
    let Some(mut rt) = runtime() else { return };
    let mut sora = sora_at(Vec3::ZERO);
    let world = TestWorld::floor();
    party_step(&mut rt, &mut sora, &world, None);
    party_step(&mut rt, &mut sora, &world, Some(Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Donald).unwrap()));
    for _ in 0..3 {
        party_step(&mut rt, &mut sora, &world, None);
    }
    assert!(rt.friend_leads() && rt.lead.sora_active, "AI Sora drives the Sora core");
    // a host gives the Sora core no Summon while a friend leads (kh-er spirit.rs: `allowed` only while Sora leads)
    sora.command_mut().summon = crate::mimic::summon_view();
    let cam = camera();
    let enemy = Victim { chara: PartyChara::Goofy, feet: Vec3::new(700.0, 0.0, 0.0), yaw: 180.0, velocity: Vec3::ZERO, radius: 50.0, half_height: 90.0, hp: 1000, hp_max: 1000, defense: 0, airborne: false, down: false, tell: Default::default(), stance: Default::default() };
    let dt = 1.0 / 60.0;
    for k in 0..60 * 20 {
        let mut t = BossTargets { victims: vec![enemy], hits: Vec::new(), attack_rate: 1.0, orders: Vec::new() };
        let input = rt.route(RouteIn { sora: &mut sora, input: &Input::default(), dt, targets: &t, camera: &cam, asked_formation: None, asked_ai: Vec::new(), nav: None, now: k as f64 * dt as f64 });
        assert!(!input.pad.pressed.has(PadButton::DPadUp) && !input.pad.pressed.has(PadButton::DPadDown), "AI Sora never moves his menu toward Summon");
        let out = {
            let (mut audio, mut effects, mut ui) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects, kh_core::host::NullUi);
            let mut host = kh_core::Host { world: &world, targets: &mut t, audio: &mut audio, effects: &mut effects, ui: &mut ui };
            sora.step_hosted(dt, &input, &mut host)
        };
        let (mut audio, mut effects) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects);
        rt.step(StepIn { dt, sora: &mut sora, out: &out, lock_on: TargetId::NONE, camera: &cam, world: &world, targets: &mut t, audio: &mut audio, effects: &mut effects, nav: None, now: k as f64 * dt as f64 });
        let _ = rt.after_step(&t, dt, k as f64 * dt as f64);
        assert!(sora.command().main_cursor < 3 && sora.command().list != 3, "no Summon row or list for AI Sora");
        let (_, grey) = kh_core::command::summon::main_row(&sora.command().summon, sora.status());
        assert_eq!(grey, Some(kh_core::command::summon::Grey::NotAllowed));
    }
    // the friends' copies: their stand-in Sora (never driven) has no ashes and no Summon either
    let Some((_, _, boss)) = friend_fight(PartyChara::Goofy) else { return };
    assert!(!boss.stand_in.command().summon.allowed && boss.stand_in.command().spirit_ashes == [0; 4]);
}
