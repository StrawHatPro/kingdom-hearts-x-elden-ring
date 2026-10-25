//! (ER party S7) THE OFFLINE TEAM-ATTACK HARNESS: each of the nine team attacks run by the player's command against
//! dummies in an Elden-Ring-shaped world stub, with the party core (glue.rs) driven exactly as sora.rs drives it
//! (frame -> route -> Sora's step with `SoraTargets` -> step) and KH3's camera stepped in cam.rs's order (the field /
//! lock-on rig on the LEADER, then `team::layer`, the same function cam.rs's `TeamCam` calls).
//!
//! THE WORLD STUB (kh-core `TestWorld`, KH3 cm): a plateau that ends in a 30 m cliff 13 m ahead, a rock wall on the
//! left, an overhang right where the field camera sits behind Sora; or a walled boss arena.
//! THE TARGETS: a real `ErTargets` (its own `enumerate` / `info` / `hurt_shapes` / flags / aim heights / puppets) over
//! stub rows; only `apply_hit` is mirrored (`ErStub`: the KH side of targets.rs `apply_hit` = the puppet's reaction, its
//! role and dmgLevel, the poise damage, the bullet's attack value; the game's own bullet call is the one thing missing
//! offline). Rows: a soldier, a knight with super armour and poise, a character the game does not draw (inactive: the
//! nearest one), one below the cliff; in the arena a boss.
//!
//! `KH_S7_OUT=<dir>` writes every camera frame (`<attack>.csv`) and the report (`report.txt`) there.

#![cfg(windows)]

use std::fmt::Write as _;

use glam::{Vec2, Vec3};
use kh_camera::{CameraInput, CameraRig, CameraTarget, CameraView, CAMERA_PARAMS};
use kh_core::host::{HitResult, HurtShape, PushBody, TargetBody, TargetInfo};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{EventKind, HitRecord, Input, Loadout, NullHost, Output, PadButton, PadButtons, PadState, Pose, Sora, Status, TargetId, Targets, SORA_PARAMS};
use kh_host::party::PartyChara;
use kh_party::NpcStateId;

use super::*;
use crate::coords::Frame;
use crate::ermap::{Params, Weight};
use crate::party::glue::{FrameIn, HostRules, BodyObs, PartyCore};
use crate::puppet::Role;
use crate::targets::{Enemy, ErTargets};

const STEP: f32 = 1.0 / 60.0;
/// A player presses Y again (a quarter second apart) at most this often when his press was refused.
const Y_TRIES: u32 = 8;

fn project(p: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

fn rows(path: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|j| j["rows"].as_array().cloned()).unwrap_or_default()
}

/// Sora at level 99 with the five unison / fusion abilities equipped (as kh-core tests/team.rs).
fn level99() -> Status {
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(project("sheets/port/sora-loadout-99.json")).unwrap()).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let v = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64().unwrap()),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(a) => LoadoutValue::List(a.iter().map(|i| i.as_str().unwrap().to_string()).collect()),
            serde_json::Value::Object(m) => LoadoutValue::Map(m.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap())).collect()),
            _ => continue,
        };
        let v = match (row[0].as_str().unwrap(), v) {
            ("abilities.equipped", LoadoutValue::List(mut l)) => {
                l.extend(["UNISON_FIRE", "UNISON_BLIZZARD", "UNISON_THUNDER", "FUSION_SPIN", "FUSION_ROCKET"].map(String::from));
                LoadoutValue::List(l)
            }
            (_, v) => v,
        };
        loadout.set(row[0].as_str().unwrap(), &v).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

// ---------------------------------------------------------------- the Elden-Ring-shaped targets

/// One hit as the bullet route would carry it.
#[derive(Clone, Debug)]
struct HitLog {
    step: u64,
    target: u32,
    attack: &'static str,
    kh: f32,
    kind: i32,
    /// The bullet's attack row: atkPhys, poise damage, dmgLevel; the reaction role and KH3's outcome.
    phys: i32,
    poise: f32,
    level: u8,
    role: Role,
    outcome: String,
    attacker: Vec3,
}

struct ErStub {
    t: ErTargets,
    hits: Vec<HitLog>,
    step: u64,
}

impl ErStub {
    fn new(rows: Vec<Enemy>) -> ErStub {
        let mut t = ErTargets::new(Frame::at(Vec3::ZERO), Params::default());
        t.list = rows;
        ErStub { t, hits: Vec::new(), step: 0 }
    }

    fn hits_on(&self, id: u32) -> usize {
        self.hits.iter().filter(|h| h.target == id).count()
    }
}

impl Targets for ErStub {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        self.t.enumerate(out)
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.t.info(id)
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        self.t.hurt_shapes(id, out)
    }
    /// THE KH SIDE OF targets.rs `apply_hit` (the order and the calls of that function), then the HP the game would take.
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        let ErTargets { list, puppets, params, .. } = &mut self.t;
        let Some(i) = list.iter().position(|e| e.id == hit.target) else { return HitResult::default() };
        let e = list[i].clone();
        if !e.hostile || !e.alive || !e.active {
            // (the real route never even gets these: they are no target)
            return HitResult { hp_after: e.hp.max(0) as f32, ..HitResult::default() };
        }
        let wanted = params.damage_out(hit.damage);
        let mut outcome = None;
        let airborne_before = puppets.airborne(e.id);
        if hit.takes_hp() && e.lift == 0.0 {
            let lethal = e.hp - wanted <= 0 || wanted >= e.hp + e.hp_max / 4;
            outcome = Some(puppets.hit(params, &e, hit, wanted > 0, lethal));
        }
        if !hit.takes_hp() {
            return HitResult { accepted: true, damage_dealt: 0.0, hp_after: e.hp as f32, killed: false, guarded: true };
        }
        let kind = hit.damage_kind.value() as i32;
        let role = outcome.as_ref().map_or(Role::None, |o| puppets.role_of(e.id, o, airborne_before));
        let level = puppets.level_for(e.npc_id, role);
        let poise = puppets.poise_for(params, &e, params.reaction(kind).poise * params.poise_scale, hit.info.reaction_power);
        let phys = wanted.clamp(1, 60000);
        puppets.asked(e.id, role, level);
        let before = e.hp;
        list[i].hp = (before - wanted).max(0);
        list[i].alive = list[i].hp > 0;
        self.hits.push(HitLog { step: self.step, target: e.id.0, attack: hit.attack_name, kh: hit.damage, kind, phys, poise, level, role, outcome: format!("{outcome:?}"), attacker: hit.attacker.position });
        HitResult { accepted: true, damage_dealt: params.damage_out_inverse(wanted.min(before)), hp_after: list[i].hp as f32, killed: list[i].hp <= 0, guarded: false }
    }
    fn terrain_dz(&self, id: TargetId) -> f32 {
        self.t.terrain_dz(id)
    }
    fn engaged_count(&self) -> u32 {
        self.t.engaged_count()
    }
    fn body(&self, id: TargetId, part: u32) -> TargetBody {
        self.t.body(id, part)
    }
    fn take_reaction_power(&mut self, id: TargetId, power: i32) -> i32 {
        self.t.take_reaction_power(id, power)
    }
    fn reaction_state(&self, id: TargetId) -> u8 {
        self.t.reaction_state(id)
    }
    fn push_body(&self, id: TargetId) -> Option<PushBody> {
        self.t.push_body(id)
    }
}

impl kh_partyhost::PartyTargets for ErStub {
    /// As party/aiworld.rs `ai_actors` (the same rows, the same rules).
    fn ai_actors(&self, out: &mut Vec<kh_ai::ActorView>) {
        for e in &self.t.list {
            if !e.active {
                continue;
            }
            out.push(kh_ai::ActorView {
                actor: kh_ai::ActorRef::Enemy(e.id.0),
                uid: -1,
                position: e.pos,
                center: e.pos + Vec3::Z * self.t.aim_height(e),
                yaw: e.yaw,
                velocity: e.velocity,
                radius: e.radius,
                half_height: e.half_height,
                alive: e.alive,
                hostile: e.hostile && !e.friendly,
                lockable: e.hostile && !e.lock_disabled,
                airborne: e.airborne || !e.grounded,
                hp: e.hp as f32,
                hp_max: e.hp_max as f32,
                ..kh_ai::ActorView::default()
            });
        }
    }
    fn attacked_by_enemies(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------- the world stub

const SOLDIER: u32 = 1;
const KNIGHT: u32 = 2;
const HIDDEN: u32 = 3;
const BELOW: u32 = 4;
const BOSS: u32 = 5;
const ADJACENT: u32 = 6;

/// Plateau (top at z 0, ends at x 1300 in a 30 m cliff), a rock wall on the left (y 600..900, 15 m high), an overhang
/// behind Sora (x -900..-150, z 330..700) where the field camera wants to be.
fn open_world() -> (TestWorld, Vec<Enemy>) {
    let world = TestWorld::default()
        .with_plane(Vec3::Z, -3000.0)
        .with_box(Vec3::new(-6000.0, -6000.0, -3000.0), Vec3::new(1300.0, 6000.0, 0.0))
        .with_box(Vec3::new(-3000.0, 600.0, 0.0), Vec3::new(3000.0, 900.0, 1500.0))
        .with_box(Vec3::new(-900.0, -400.0, 330.0), Vec3::new(-150.0, 400.0, 700.0));
    let soldier = Enemy::stub(SOLDIER, Vec3::new(330.0, -70.0, 0.0), 40.0, 90.0, 4000, Weight::Small);
    let mut knight = Enemy::stub(KNIGHT, Vec3::new(400.0, 110.0, 0.0), 45.0, 100.0, 6000, Weight::Medium);
    (knight.poise_max, knight.poise, knight.hyper) = (60.0, 60.0, Some(true));
    // the nearest one of all, right in front: the game does not draw it (`target.visible_only`: inactive)
    let mut hidden = Enemy::stub(HIDDEN, Vec3::new(200.0, 0.0, 0.0), 40.0, 90.0, 4000, Weight::Small);
    hidden.active = false;
    let below = Enemy::stub(BELOW, Vec3::new(1900.0, 0.0, -3000.0), 40.0, 90.0, 4000, Weight::Small);
    (world, vec![soldier, knight, hidden, below])
}

/// A walled boss arena (24 m across, walls 20 m high, a fog-wall box in the gap behind) with a boss 7 m ahead.
fn arena() -> (TestWorld, Vec<Enemy>) {
    let world = TestWorld::floor()
        .with_box(Vec3::new(1200.0, -1300.0, 0.0), Vec3::new(1500.0, 1300.0, 2000.0))
        .with_box(Vec3::new(-1500.0, -1300.0, 0.0), Vec3::new(-1200.0, 1300.0, 2000.0))
        .with_box(Vec3::new(-1500.0, 1200.0, 0.0), Vec3::new(1500.0, 1500.0, 2000.0))
        .with_box(Vec3::new(-1500.0, -1500.0, 0.0), Vec3::new(1500.0, -1200.0, 2000.0));
    let mut boss = Enemy::stub(BOSS, Vec3::new(700.0, 0.0, 0.0), 250.0, 450.0, 40000, Weight::Large);
    (boss.poise_max, boss.poise) = (200.0, 200.0);
    let mut hidden = Enemy::stub(HIDDEN, Vec3::new(300.0, 200.0, 0.0), 40.0, 90.0, 4000, Weight::Small);
    hidden.active = false;
    (world, vec![boss, hidden])
}

/// How far `p` is outside every solid of the stub (negative: inside one).
fn clearance(world: &TestWorld, p: Vec3) -> f32 {
    let mut best = f32::MAX;
    for (n, o) in &world.planes {
        best = best.min(n.dot(p) - o);
    }
    for (lo, hi) in &world.boxes {
        let d = (*lo - p).max(p - *hi);
        let outside = d.max(Vec3::ZERO).length();
        let inside = d.max_element().min(0.0);
        best = best.min(if outside > 0.0 { outside } else { inside });
    }
    best
}

// ---------------------------------------------------------------- the bench

#[derive(Clone, Copy, Debug)]
struct CamFrame {
    step: u32,
    position: Vec3,
    pitch: f32,
    yaw: f32,
    fov: f32,
    team: bool,
    clearance: f32,
    top: &'static str,
}

struct Bench {
    core: PartyCore,
    sora: Sora,
    out: Output,
    before: Output,
    world: TestWorld,
    er: ErStub,
    null: NullHost,
    rig: Option<CameraRig>,
    view: CameraView,
    t: f64,
    held_before: PadButtons,
    frames: Vec<CamFrame>,
    lines: Vec<String>,
    events: Vec<EventKind>,
    /// Steps the friends' bodies were asked invulnerable while in an FR state / not.
    fr_wants: (u32, u32),
    fr_states: Vec<(PartyChara, NpcStateId)>,
    watch: Watch,
    /// The field target was the leader's (a friend leading) in this many steps.
    leader_target_steps: u32,
    /// The player's Y presses it took to start the offered attack.
    ys: u32,
    yhold: YHold,
    no_team_collision: bool,
    /// (INT) each step's drawn-relevant state (props, the camera, the bodies), for the frame-time replay
    int_log: Vec<IntStep>,
}

/// (INT) One step of the team attack bench as the drawing reads it.
#[derive(Clone)]
struct IntStep {
    props: Vec<(&'static str, usize, kh_anim::Trs)>,
    view: CameraView,
    sora: Vec3,
    friends: Vec<(PartyChara, Vec3)>,
}

impl Bench {
    fn new(world: TestWorld, enemies: Vec<Enemy>) -> Option<Bench> {
        let pack = kh_ai::AiPack::load_dir(&project("build/sora_pack/party/ai")).ok()?;
        let sheets = project("sheets/port");
        let config = kh_partyhost::RuntimeConfig { wanted: true, test: kh_partyhost::PartyTest::from_rows(&rows(&sheets.join("party-test.json"))), weapons: kh_partyhost::PartyWeapon::from_rows(&rows(&sheets.join("party-weapons.json"))), ai_pack: Some(std::sync::Arc::new(pack)), ai_pack_source: None };
        let rules = std::fs::read_to_string(sheets.join("party-er-host.json")).ok().and_then(|t| HostRules::from_sheet(&t).ok()).map_or(HostRules::DEFAULT, |r| r.0);
        let mut core = PartyCore::new(config, rules);
        core.sink = Some(|t| if std::env::var_os("KH_PARTY_LOG").is_some() { eprintln!("{t}") });
        let sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 }).with_status(level99());
        let out = sora.output();
        let nothing = CameraView { position: Vec3::ZERO, rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 0.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        Some(Bench { core, sora, before: out.clone(), out, world, er: ErStub::new(enemies), null: NullHost::default(), rig: None, view: nothing, t: 0.0, held_before: PadButtons::NONE, frames: Vec::new(), lines: Vec::new(), events: Vec::new(), fr_wants: (0, 0), fr_states: Vec::new(), watch: Watch::default(), leader_target_steps: 0, ys: 0, yhold: YHold::default(), no_team_collision: false, int_log: Vec::new() })
    }

    /// One frame of one fixed step, in sora.rs's order; then KH3's camera in cam.rs's order.
    fn step(&mut self, held: PadButtons, right_stick: Vec2) {
        let fin = FrameIn { on: true, fresh: false, relocated: false, hidden: None, body_action: false, trace: false, body: Some(BodyObs { feet: self.out.position, yaw: self.out.yaw }), pose: true, dt: STEP, now: self.t };
        self.core.frame(fin);
        let pad = PadState::from_held(held, self.held_before);
        self.held_before = held;
        let input = Input { camera_yaw: self.view.rotation.yaw, ..Input::default() }.with_pad(pad);
        // (as sora.rs) a friend leading: the refused Y held for Sora's executor
        let friend_leads = self.core.rt.friend_leads();
        let input = held_y_input(&mut self.yhold, STEP, friend_leads, &self.sora, input);
        self.er.step += 1;
        self.er.t.sora = self.out.position;
        self.er.t.step = self.out.step + 1;
        let input = self.core.route(&mut self.sora, &input, STEP, &self.world, &self.er, self.t);
        self.before = self.out.clone();
        self.out = if self.core.sora_skipped() {
            Output { events: Vec::new(), ..self.out.clone() }
        } else {
            let mut targets = kh_partyhost::SoraTargets { targets: &mut self.er, runtime: &mut self.core.rt };
            let mut host = kh_core::Host { world: &self.world, targets: &mut targets, audio: &mut self.null.audio, effects: &mut self.null.effects, ui: &mut self.null.ui };
            self.sora.step_hosted(STEP, &input, &mut host)
        };
        self.core.camera.position = self.view.position;
        self.core.camera.rotation = self.view.rotation;
        let lock = self.sora.combat().lockon.current;
        self.core.step(&mut self.sora, &self.out, STEP, lock, &self.world, &kh_anim::NoIkWorld, &mut self.er, &mut self.null.audio, &mut self.null.effects, None, self.t);
        self.t += STEP as f64;
        self.events.extend(self.out.events.iter().map(|e| e.kind));
        // the bodies' want list: a friend in an FR state is asked invulnerable (S3 makes the body unhittable)
        for w in &self.core.feed.wants {
            if let Some(v) = self.core.rt.view(w.chara) {
                if matches!(v.state, NpcStateId::Fr(_)) {
                    if w.invulnerable { self.fr_wants.0 += 1 } else { self.fr_wants.1 += 1 }
                    if !self.fr_states.contains(&(w.chara, v.state)) {
                        self.fr_states.push((w.chara, v.state));
                    }
                }
            }
        }
        if let Some(lead) = self.core.rt.leader.and_then(|id| self.core.rt.current.iter().find(|v| v.id == id)) {
            if matches!(lead.state, NpcStateId::Fr(_)) && !self.fr_states.contains(&(kh_partyhost::party_chara(lead.chara), lead.state)) {
                self.fr_states.push((kh_partyhost::party_chara(lead.chara), lead.state));
            }
        }
        // ---- KH3's camera (cam.rs `KhCam::step`): the field / lock-on rig on the LEADER, then the team cameras
        let sora_target = crate::cam::target_of(&self.out, &self.before, &self.sora);
        let friend = self.core.rt.camera_target(STEP);
        if friend.is_some() {
            self.leader_target_steps += 1;
        }
        let target: CameraTarget = friend.unwrap_or(sora_target);
        let rig = self.rig.get_or_insert_with(|| CameraRig::new(CAMERA_PARAMS, &target));
        for e in &self.out.events {
            if let (EventKind::CameraLock { target: id }, None) = (e.kind, friend) {
                rig.set_lockon(&target, self.er.t.info(id).map(|i| (id.0, i.position + Vec3::Z * i.half_height)));
            }
        }
        if let Some(change) = self.core.feed.lock_change.take() {
            rig.set_lockon(&target, change.at);
        }
        let lock_at = rig.locked().and_then(|id| self.er.t.info(TargetId(id))).map(|i| i.position + Vec3::Z * i.half_height);
        let mut view = rig.step(STEP, &CameraInput::default(), &target, lock_at, &self.world);
        let stick = CameraInput { right_stick: right_stick, recentre: false, extra_rotation: Vec2::ZERO };
        // (the control run: no collision pass for the team cameras, to show what the pass prevents)
        let empty = TestWorld::default();
        let cam_world: &TestWorld = if self.no_team_collision { &empty } else { &self.world };
        let layered = layer(&mut self.core.rt, STEP, &self.out.events, &self.out, &mut view, cam_world, stick);
        if layered.reset_field {
            rig.field.reset(&target, true);
        }
        for l in self.watch.step(&self.out.events, &layered, self.core.rt.team.running(), self.t) {
            self.lines.push(l);
        }
        self.view = view;
        let fov = crate::cam::team_fov_h(crate::cam::FOV_KH3, layered.fov, layered.shake.fov);
        self.frames.push(CamFrame { step: self.out.step as u32, position: view.position, pitch: view.rotation.pitch, yaw: view.rotation.yaw, fov, team: layered.fov.is_some(), clearance: clearance(&self.world, view.position), top: layered.cameras.last().copied().unwrap_or("field") });
        // (INT)
        let props = crate::party::team::props(&self.core.rt).iter().map(|p| (p.model, p.index, p.trs())).collect();
        let friends = self.core.rt.current.iter().map(|v| (kh_partyhost::party_chara(v.chara), v.position)).filter(|(c, _)| *c != PartyChara::Sora).collect();
        self.int_log.push(IntStep { props, view, sora: self.out.position, friends });
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.step(PadButtons::NONE, Vec2::ZERO);
        }
    }

    fn team_busy(&self) -> bool {
        self.core.rt.team.running() != 0 || sora_in_team_attack(self.out.state) || self.core.rt.cams.active() || self.core.rt.current.iter().any(|v| matches!(v.state, NpcStateId::Fr(_)))
    }

    fn started(&self, attack: TeamAttack) -> bool {
        match attack.start() {
            Start::Offer(k) => self.events.iter().any(|e| matches!(e, EventKind::TeamAttack { kind, phase: 0 } if *kind == k)),
            Start::Unison(_) => self.sora.combat().history.contains(&(attack.kind() as u8)),
        }
    }

    /// Gives the player's command for `attack` and runs it to its end (and the blend back).
    fn run(&mut self, attack: TeamAttack, steer: bool) -> (u32, u32) {
        let start = self.frames.len() as u32;
        match attack.start() {
            Start::Offer(kind) => {
                self.sora.combat_mut().friend.debug_fill_gauge();
                self.core.rt.team.debug_ready_all();
                self.core.rt.team.debug_offer(kind);
                // the offer's appear (0.2 s) + its start delay (0.5 s), then the player's Y
                self.frames(50);
                // (a player whose Y was refused presses again: Sora's executor refuses while something of his is
                // queued / he is hit: the entry stays, KH3 has no buffer)
                for _ in 0..Y_TRIES {
                    self.step(PadButtons::NONE.with(PadButton::SITUATION), Vec2::ZERO);
                    self.ys += 1;
                    for _ in 0..14 {
                        self.step(PadButtons::NONE, Vec2::ZERO);
                    }
                    if self.started(attack) {
                        break;
                    }
                }
            }
            Start::Unison(kind) => {
                let f = &mut self.sora.combat_mut().friend;
                (f.unison, f.leader_unison) = (kind, 0.5);
            }
        }
        let mut quiet = 0;
        let mut n = 0u32;
        while n < 1500 {
            n += 1;
            // Trinity Guard's own menu: Shield Bash on A every 20 steps; the right stick steers shield_01 for a while
            // and its finish on Y (the menu's 214 entry: the heavy bash) after 6 s
            let held = match (attack == TeamAttack::TrinityGuard, n) {
                (true, 360) => PadButtons::NONE.with(PadButton::SITUATION),
                (true, n) if n % 20 == 0 && n < 360 => PadButtons::NONE.with(PadButton::ATTACK),
                _ => PadButtons::NONE,
            };
            let stick = if steer && (60..120).contains(&n) { Vec2::new(1.0, 0.0) } else { Vec2::ZERO };
            self.step(held, stick);
            quiet = if self.team_busy() { 0 } else { quiet + 1 };
            if n > 30 && quiet > 90 && (self.started(attack) || n > 600) {
                break;
            }
        }
        (start, self.frames.len() as u32)
    }

    fn csv(&self, from: u32, to: u32) -> String {
        let mut s = String::from("step,cam_x,cam_y,cam_z,pitch,yaw,fov_h,team_camera,top_camera,clearance_cm\n");
        for f in &self.frames[from as usize..to as usize] {
            let _ = writeln!(s, "{},{:.1},{:.1},{:.1},{:.2},{:.2},{:.1},{},{},{:.1}", f.step, f.position.x, f.position.y, f.position.z, f.pitch, f.yaw, f.fov, f.team as u8, f.top, f.clearance);
        }
        s
    }
}

macro_rules! bench {
    ($w:expr) => {{
        let (world, enemies) = $w;
        match Bench::new(world, enemies) {
            Some(b) => b,
            None => {
                eprintln!("skipped: no AI pack (build/sora_pack/party/ai)");
                return;
            }
        }
    }};
}

fn out_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("KH_S7_OUT").map(std::path::PathBuf::from).inspect(|d| {
        let _ = std::fs::create_dir_all(d);
    })
}

/// What one run showed.
struct Seen {
    attack: TeamAttack,
    world: &'static str,
    started: bool,
    steps: u32,
    team_steps: u32,
    cameras: Vec<&'static str>,
    min_clearance: f32,
    max_jump: f32,
    field_back: bool,
    hits: Vec<HitLog>,
    fr_wants: (u32, u32),
    fr_states: Vec<(PartyChara, NpcStateId)>,
    ys: u32,
    hidden_hits: usize,
}

impl Seen {
    fn line(&self) -> String {
        let mut by: std::collections::BTreeMap<u32, (usize, i32, f32, Vec<String>)> = Default::default();
        for h in &self.hits {
            let e = by.entry(h.target).or_default();
            e.0 += 1;
            e.1 += h.phys;
            e.2 += h.poise;
            let r = format!("{:?}/lvl{}", h.role, h.level);
            if !e.3.contains(&r) {
                e.3.push(r);
            }
        }
        format!(
            "{:<16} [{}] started {} (Y presses {}) | {} steps, a team camera had the view in {} | cameras {:?} | camera clearance min {:.0} cm, largest move {:.0} cm/step, field camera back {} | hits {} | {} | FR bodies invulnerable {}/{} | FR states {:?}",
            self.attack.name(), self.world, self.started, self.ys, self.steps, self.team_steps, self.cameras, self.min_clearance, self.max_jump, self.field_back, self.hits.len(),
            by.iter().map(|(id, (n, phys, poise, roles))| format!("target {id}: {n} hit(s), atkPhys {phys}, poise {poise:.0}, {}", roles.join(" "))).collect::<Vec<_>>().join("; "),
            self.fr_wants.0, self.fr_wants.0 + self.fr_wants.1, self.fr_states,
        )
    }
}

fn run_one(world: (TestWorld, Vec<Enemy>), name: &'static str, attack: TeamAttack, leader: Option<&str>) -> Option<(Seen, String, Vec<String>)> {
    let (w, e) = world;
    let mut b = Bench::new(w, e)?;
    if let Some(l) = leader {
        b.core.rt.lead_word(&["leader", l]);
    }
    b.frames(90);
    let hits_before = b.er.hits.len();
    let (from, to) = b.run(attack, attack == TeamAttack::TrinityGuard);
    let span = &b.frames[from as usize..to as usize];
    let max_jump = span.windows(2).map(|w| (w[1].position - w[0].position).length()).fold(0.0f32, f32::max);
    let mut cameras = Vec::new();
    for f in span {
        if f.top != "field" && !cameras.contains(&f.top) {
            cameras.push(f.top);
        }
    }
    let seen = Seen {
        attack,
        world: name,
        started: b.started(attack),
        steps: to - from,
        team_steps: span.iter().filter(|f| f.team).count() as u32,
        cameras,
        min_clearance: span.iter().map(|f| f.clearance).fold(f32::MAX, f32::min),
        max_jump,
        field_back: span.last().is_some_and(|f| !f.team),
        hits: b.er.hits[hits_before..].to_vec(),
        fr_wants: b.fr_wants,
        fr_states: b.fr_states.clone(),
        ys: b.ys,
        hidden_hits: b.er.hits_on(HIDDEN),
    };
    let csv = b.csv(from, to);
    Some((seen, csv, b.lines.clone()))
}

/// THE NINE, one by one, against the dummies of the open world stub (Sora leading), and four in the boss arena.
#[test]
fn s7_every_team_attack_hits_er_targets_through_the_bullet_rules_and_its_cameras_stay_out_of_the_terrain() {
    if kh_ai::AiPack::load_dir(&project("build/sora_pack/party/ai")).is_err() {
        eprintln!("skipped: no AI pack");
        return;
    }
    let out = out_dir();
    let mut report = String::new();
    let params = Params::default();
    let mut runs: Vec<(TeamAttack, &'static str, Option<&str>)> = TeamAttack::ALL.iter().map(|a| (*a, "open", None)).collect();
    runs.extend([TeamAttack::DonaldMeteor, TeamAttack::FlareForce, TeamAttack::GoofyBombardier, TeamAttack::TrinityGuard].map(|a| (a, "arena", None)));
    let mut failures = Vec::new();
    for (attack, world, leader) in runs {
        let mut w = if world == "open" { open_world() } else { arena() };
        // UNISON THUNDER: a body right beside Sora too (the scene D6's column fix was measured in: 3 hits; ER party I)
        if attack == TeamAttack::UnisonThunder {
            w.1.push(Enemy::stub(ADJACENT, Vec3::new(75.0, -20.0, 0.0), 40.0, 90.0, 4000, Weight::Small));
        }
        let Some((seen, csv, lines)) = run_one(w, world, attack, leader) else { return };
        let _ = writeln!(report, "{}", seen.line());
        for l in &lines {
            let _ = writeln!(report, "    {l}");
        }
        if let Some(d) = &out {
            let _ = std::fs::write(d.join(format!("{}_{}.csv", attack.name().replace(' ', "_"), world)), csv);
            let mut hits = String::from("step,target,attack,kh_damage,kh_kind,atk_phys,poise,dmg_level,role,kh3_outcome,attacker_x,attacker_y,attacker_z\n");
            for h in &seen.hits {
                let _ = writeln!(hits, "{},{},{},{:.1},{},{},{:.1},{},{:?},\"{}\",{:.0},{:.0},{:.0}", h.step, h.target, h.attack, h.kh, h.kind, h.phys, h.poise, h.level, h.role, h.outcome.replace('"', "'"), h.attacker.x, h.attacker.y, h.attacker.z);
            }
            let _ = std::fs::write(d.join(format!("{}_{}_hits.csv", attack.name().replace(' ', "_"), world)), hits);
        }
        let mut fail = |why: String| failures.push(format!("{} [{world}]: {why}", attack.name()));
        if !seen.started {
            fail("did not start".into());
        }
        // (ER party I) Unison Thunder is no exception any more: D6 made its shot the real 800 cm column (kh-core), so its
        // hits are required like every other team attack's (it went from 0 hits to 3 here)
        if !seen.hits.iter().any(|h| is_team_row(h.attack)) {
            fail("no hit of the team attack itself landed".into());
        }
        // a character the game does not draw is never hit (nor the one below the cliff, 19 m off and 30 m down)
        if seen.hidden_hits > 0 || seen.hits.iter().any(|h| h.target == HIDDEN) {
            fail("the undrawn character was hit".into());
        }
        // every hit is the bullet the route would fire: KH3's damage x damage.out.scale (at least 1)
        for h in &seen.hits {
            if h.phys != params.damage_out(h.kh).clamp(1, 60000) {
                fail(format!("atkPhys {} for {} KH damage", h.phys, h.kh));
            }
            if h.target == BOSS && h.role == Role::Blow {
                fail(format!("the boss was thrown by {}", h.attack));
            }
        }
        if seen.min_clearance < -0.5 {
            fail(format!("a camera went {:.0} cm into the terrain", -seen.min_clearance));
        }
        if !seen.field_back {
            fail("the team camera did not hand back to the field camera".into());
        }
        if seen.fr_wants.1 > 0 {
            fail(format!("{} step(s) of a friend's FR state with his body hittable", seen.fr_wants.1));
        }
    }
    if let Some(d) = &out {
        let _ = std::fs::write(d.join("report.txt"), &report);
    }
    eprintln!("{report}");
    assert!(failures.is_empty(), "{}\n{report}", failures.join("\n"));
}

/// THE USER'S RULE: only the character the player controls starts a team attack; an AI member never does. Donald leads:
/// Goofy offers his attack and the gauge is full, AI Sora runs his own AI for 8 s and nothing starts; the player's Y
/// starts it. The field camera follows Donald throughout.
#[test]
fn s7_only_the_players_leader_starts_a_team_attack_and_the_camera_follows_him() {
    let mut b = bench!(open_world());
    b.core.rt.lead_word(&["leader", "donald"]);
    b.frames(60);
    assert_eq!(b.core.rt.leader_chara(), PartyChara::Donald);
    // ready: gauge full, both friends' link points at their thresholds, Goofy offers Bombardier
    b.sora.combat_mut().friend.debug_fill_gauge();
    b.core.rt.team.debug_ready_all();
    b.core.rt.team.debug_offer(195);
    for _ in 0..480 {
        b.step(PadButtons::NONE, Vec2::ZERO);
        assert!(!b.events.iter().any(|e| matches!(e, EventKind::TeamAttack { phase: 0, .. })), "a team attack started without the player's command");
        assert!(!sora_in_team_attack(b.out.state));
    }
    assert!(b.sora.combat().situation.list.iter().any(|e| e.kind == 195), "the offer waits in the situation list");
    assert!(b.leader_target_steps >= 480, "the field camera follows Donald ({} steps)", b.leader_target_steps);
    let donald = b.core.rt.view(PartyChara::Donald).unwrap().position;
    assert!((b.view.look_point - donald).truncate().length() < 150.0, "the camera looks at Donald");
    // (YHUD, the user 2026-10-23: "ONLY the Y moves for the character you're playing as") Goofy Bombardier is Sora + Goofy:
    // not Donald's, so the player's Y with Donald leading does not start it
    b.step(PadButtons::NONE.with(PadButton::SITUATION), Vec2::ZERO);
    for _ in 0..30 {
        b.step(PadButtons::NONE, Vec2::ZERO);
    }
    assert!(!b.events.iter().any(|e| matches!(e, EventKind::TeamAttack { kind: 195, phase: 0 })), "Donald's Y never starts Goofy Bombardier (not his)");
    // Donald's own team attacks (207 Donald Meteor; 208 Flare Force is run by s7_a_friend_leaders_team_attack): his Y starts it
    b.core.rt.team.debug_ready_all();
    b.core.rt.team.debug_offer(207);
    for _ in 0..30 {
        b.step(PadButtons::NONE, Vec2::ZERO);
    }
    b.step(PadButtons::NONE.with(PadButton::SITUATION), Vec2::ZERO);
    for _ in 0..30 {
        b.step(PadButtons::NONE, Vec2::ZERO);
    }
    assert!(b.events.iter().any(|e| matches!(e, EventKind::TeamAttack { kind: 207, phase: 0 })), "the player's Y starts Donald Meteor (his)");
    // Sora leading: no Y, no team attack either
    let mut s = bench!(open_world());
    s.frames(30);
    s.sora.combat_mut().friend.debug_fill_gauge();
    s.core.rt.team.debug_ready_all();
    s.core.rt.team.debug_offer(207);
    s.frames(400);
    assert!(!s.events.iter().any(|e| matches!(e, EventKind::TeamAttack { phase: 0, .. })));
}

/// A friend leading runs one through his own Y (Flare Force with Donald leading): the hits land, the camera is back on
/// Donald after it.
#[test]
fn s7_a_friend_leaders_team_attack() {
    let Some((seen, _, lines)) = run_one(open_world(), "open", TeamAttack::FlareForce, Some("donald")) else { return };
    eprintln!("{}\n{}", seen.line(), lines.join("\n"));
    assert!(seen.started && !seen.hits.is_empty() && seen.field_back, "{}", seen.line());
    // ONE press of the player's Y (held for Sora's executor while AI Sora is busy: `YHold`)
    assert_eq!(seen.ys, 1, "{}", seen.line());
    assert!(!seen.hits.iter().any(|h| h.target == HIDDEN));
}

/// Pure parts: the nine's kinds, Sora's FR states, the pitch of his draw, the props feed.
#[test]
fn s7_the_nine_kinds_the_pitch_and_the_props() {
    let kinds: Vec<i32> = TeamAttack::ALL.iter().map(|a| a.kind()).collect();
    assert_eq!(kinds, vec![0x3F, 0x40, 0x41, 0x42, 0x43, 207, 208, 195, 193]);
    for a in TeamAttack::ALL {
        assert_eq!(TeamAttack::of_kind(a.kind()), Some(a));
    }
    assert!(sora_in_team_attack(kh_core::StateId::FrFlareforce) && !sora_in_team_attack(kh_core::StateId::Idle));
    // a pitch of 90 (nose up) turns his forward (mesh frame: the actor yaw) to straight up, about his feet
    let mut t = kh_anim::Trs { translation: Vec3::new(1.0, 2.0, 3.0), rotation: glam::Quat::from_rotation_z((30.0f32 - 90.0).to_radians()), scale: Vec3::ONE };
    let reference = kh_partyhost::poser::component_to_world_pitched(t.translation, 30.0, 25.0);
    pitch_sora(&mut t, Some(25.0));
    assert!(t.rotation.angle_between(reference.rotation) < 1.0e-4, "the pitched draw = kh-partyhost's pitched actor");
    assert_eq!(t.translation, Vec3::new(1.0, 2.0, 3.0));
    let before = t;
    pitch_sora(&mut t, None);
    assert_eq!(t.rotation, before.rotation);
    // the props of a fresh runtime: none in flight
    let rt = kh_partyhost::PartyRuntime::new(kh_partyhost::RuntimeConfig { wanted: false, test: kh_partyhost::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
    assert!(props(&rt).is_empty());
    eprintln!("gshoot tables compiled: {}", gshoot_compiled());
}

/// The held Y: only while a friend leads, a team command is current, no shoulder held, Sora not yet in his FR state.
#[test]
fn s7_the_players_y_is_held_for_sora_while_a_friend_leads() {
    let mut h = YHold::default();
    // Sora leading: nothing
    assert!(!h.step(STEP, false, true, false, true, false));
    assert!(!h.step(STEP, false, false, false, true, false));
    // a friend leading: the press goes as it is, then held for Y_HOLD s
    assert!(!h.step(STEP, true, true, false, true, false));
    let held = (0..60).filter(|_| h.step(STEP, true, false, false, true, false)).count();
    assert_eq!(held, (Y_HOLD / STEP).ceil() as usize);
    // ended at once by the FR state, by another current command, by LB / RB
    for (team_current, sora_in_fr) in [(true, true), (false, false)] {
        let mut h = YHold::default();
        h.step(STEP, true, true, false, true, false);
        assert!(!h.step(STEP, true, false, false, team_current, sora_in_fr));
        assert!(!h.step(STEP, true, false, false, true, false), "not again");
    }
    let mut h = YHold::default();
    assert!(!h.step(STEP, true, true, true, true, false));
    assert!(!h.step(STEP, true, false, false, true, false));
}

/// The once-per-attack log lines.
#[test]
fn s7_the_watch_says_start_and_end() {
    let mut w = Watch::default();
    let ev = |kind, phase| kh_core::Event { step: 1, kind: EventKind::TeamAttack { kind, phase } };
    let lo = LayerOut { fov: Some(70.0), cameras: vec!["dona_flair"], ..LayerOut::default() };
    let l = w.step(&[ev(208, 0)], &lo, 208, 1.0);
    assert!(l[0].contains("Flare Force STARTS"));
    w.step(&[], &lo, 208, 1.1);
    let l = w.step(&[ev(208, 255)], &LayerOut::default(), 0, 2.0);
    assert!(l[0].contains("Flare Force ENDS") && l[0].contains("dona_flair"), "{l:?}");
}

/// THE CONTROL: the same team attacks with the team cameras' collision pass given an EMPTY world. Where KH3's authored
/// camera paths would enter the stub's rock (the arena walls, the overhang, the plateau under the cliff), the pass is what
/// keeps them out: with it the clearance stays at the probe radius (20 cm, the main test), without it it goes negative.
#[test]
fn s7_control_without_the_collision_pass_the_cameras_enter_the_rock() {
    let mut worst: Vec<(String, f32)> = Vec::new();
    for (attack, world) in [(TeamAttack::DonaldMeteor, "arena"), (TeamAttack::GoofyBombardier, "arena"), (TeamAttack::TrinityGuard, "arena"), (TeamAttack::FlareForce, "open"), (TeamAttack::GoofyBombardier, "open"), (TeamAttack::TrinityGuard, "open")] {
        let (w, e) = if world == "open" { open_world() } else { arena() };
        let Some(mut b) = Bench::new(w, e) else { return };
        b.no_team_collision = true;
        b.frames(90);
        let (from, to) = b.run(attack, false);
        let min = b.frames[from as usize..to as usize].iter().map(|f| f.clearance).fold(f32::MAX, f32::min);
        worst.push((format!("{} [{world}]", attack.name()), min));
    }
    let text = worst.iter().map(|(n, m)| format!("{n}: clearance min {m:.0} cm WITHOUT the pass")).collect::<Vec<_>>().join("\n");
    eprintln!("{text}");
    if let Some(d) = out_dir() {
        let _ = std::fs::write(d.join("control_no_collision.txt"), &text);
    }
    assert!(worst.iter().any(|(_, m)| *m < 0.0), "the control should show a camera inside the rock somewhere:\n{text}");
}

/// shield_01 (Trinity Guard's field-type camera) turns with the player's right stick; nothing else differs between the runs.
#[test]
fn s7_trinity_guards_shield_01_is_steered_by_the_right_stick() {
    let mut yaw = Vec::new();
    for steer in [false, true] {
        let Some(mut b) = Bench::new(open_world().0, open_world().1) else { return };
        b.frames(90);
        let (from, _) = b.run(TeamAttack::TrinityGuard, steer);
        // the stick is held in run steps 60..120 (after the 50 + 15 steps of the offer and the press)
        let f = b.frames[from as usize + 65 + 125];
        yaw.push((f.yaw, f.top));
    }
    eprintln!("shield_01 without / with the stick: {yaw:?}");
    assert_eq!(yaw[1].1, "shield_01", "{yaw:?}");
    let d = (yaw[1].0 - yaw[0].0 + 540.0).rem_euclid(360.0) - 180.0;
    assert!(d.abs() > 10.0, "the stick turned shield_01 by {d:.1} deg");
}

/// A hit row of a team attack (not the friends' own AI attacks around it).
fn is_team_row(name: &str) -> bool {
    name.starts_with("FR_") || name.starts_with("UNISON_") || name.starts_with("FUSION_")
}

/// (INT) A TEAM ATTACK BY RENDERED FRAMES: the bench's steps (Donald Meteor's rocks, Flare Force's missiles; the team
/// cameras; Sora; the friends) replayed under Elden Ring-like frame times (16 - 33 ms, a 50 ms hitch 1 frame in 60) on
/// sora.rs's accumulator, drawn as kh-er draws them: `live` (r.interp 0: the latest step) and `interp` (interp.rs `Props`
/// for the props, cam.rs `post`'s blend with `camera_cut`, the bodies between the steps). The drawing's own error = the
/// frame displacement against each object's step path (linear between the steps) at its best latency. Prints `INT TEAM`.
#[test]
fn int_team_attack_drawn_between_the_steps() {
    struct Rng(u64);
    impl Rng {
        fn unit(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 40) as f32 / (1u64 << 24) as f32
        }
    }
    // (object key, rule) -> drawn (t, place); object key -> step (t, place)
    type Key = String;
    fn judder(sim: &[(f64, Vec3)], drawn: &[(f64, Vec3)]) -> (f32, f32) {
        let lin = |t: f64| -> Option<Vec3> {
            if sim.len() < 2 || t < sim[0].0 || t > sim[sim.len() - 1].0 {
                return None;
            }
            let i = sim.partition_point(|x| x.0 < t).clamp(1, sim.len() - 1);
            let (a, b) = (sim[i - 1], sim[i]);
            if (b.0 - a.0) > STEP as f64 * 1.5 || a.1.distance(b.1) > crate::interp::SNAP_CM {
                return None; // (not there at a step between, or a jump)
            }
            Some(a.1.lerp(b.1, ((t - a.0) / (b.0 - a.0).max(1e-9)) as f32))
        };
        let mut best = (0.0f32, f32::MAX);
        for ms in 0..=50 {
            let lag = ms as f64 / 1000.0;
            let mut f = Vec::new();
            for w in drawn.windows(2) {
                if w[1].0 - w[0].0 > 0.06 {
                    continue;
                }
                let (Some(a), Some(b)) = (lin(w[0].0 - lag), lin(w[1].0 - lag)) else { continue };
                f.push(((w[1].1 - w[0].1) - (b - a)).length());
            }
            if f.len() > 10 {
                let rms = (f.iter().map(|x| x * x).sum::<f32>() / f.len() as f32).sqrt();
                if rms < best.1 {
                    best = (ms as f32, rms);
                }
            }
        }
        best
    }
    for (attack, world) in [(TeamAttack::DonaldMeteor, "open"), (TeamAttack::FlareForce, "open"), (TeamAttack::GoofyBombardier, "open")] {
        let mut b = bench!(if world == "open" { open_world() } else { arena() });
        b.frames(90);
        let first = b.int_log.len();
        b.run(attack, false);
        let log = b.int_log[first..].to_vec();
        let mut rng = Rng(0x2545_F491_4F6C_DD1D ^ attack.name().len() as u64);
        let (mut acc, mut t_disp, mut k) = (0.0f32, 0.0f64, 0usize);
        let mut sim: std::collections::BTreeMap<Key, Vec<(f64, Vec3)>> = Default::default();
        let mut drawn: std::collections::BTreeMap<(Key, &'static str), Vec<(f64, Vec3)>> = Default::default();
        let mut props = crate::interp::Props::default();
        let (mut cuts, mut frames) = (0u32, 0u32);
        while k < log.len() {
            let dt = if rng.unit() < 1.0 / 60.0 { 0.050 } else { 0.016 + 0.017 * rng.unit() };
            t_disp += dt as f64;
            acc = (acc + dt).min(0.25);
            while acc >= STEP && k < log.len() {
                acc -= STEP;
                let s = &log[k];
                let t = (k + 1) as f64 * STEP as f64;
                props.record(s.props.iter().copied());
                for (m, i, tr) in &s.props {
                    sim.entry(format!("prop {m} {i}")).or_default().push((t, tr.translation));
                }
                sim.entry("camera".into()).or_default().push((t, s.view.position));
                sim.entry("Sora".into()).or_default().push((t, s.sora));
                for (c, p) in &s.friends {
                    sim.entry(c.name().to_string()).or_default().push((t, *p));
                }
                k += 1;
            }
            if k < 2 {
                continue;
            }
            frames += 1;
            let (before, now) = (&log[k - 2], &log[k - 1]);
            let alpha = (acc / STEP).clamp(0.0, 1.0);
            let cut = crate::interp::camera_cut(&before.view, &now.view);
            cuts += u32::from(cut);
            for (rule, a) in [("live", 1.0f32), ("interp", alpha)] {
                for (m, i, tr) in &now.props {
                    let p = props.at(m, *i, *tr, a);
                    drawn.entry((format!("prop {m} {i}"), rule)).or_default().push((t_disp, p.translation));
                }
                let (cam, _) = now.view.lerp(&before.view, if cut { 1.0 } else { a });
                drawn.entry(("camera".into(), rule)).or_default().push((t_disp, cam));
                let lerp = |x: Vec3, y: Vec3| if x.distance(y) > crate::interp::SNAP_CM { y } else { x.lerp(y, a) };
                drawn.entry(("Sora".into(), rule)).or_default().push((t_disp, lerp(before.sora, now.sora)));
                for (c, p) in &now.friends {
                    let q = before.friends.iter().find(|x| x.0 == *c).map_or(*p, |x| lerp(x.1, *p));
                    drawn.entry((c.name().to_string(), rule)).or_default().push((t_disp, q));
                }
            }
        }
        // the props together: the mean of their errors
        let mut lines = Vec::new();
        for group in ["prop", "camera", "Sora", "Donald", "Goofy"] {
            for rule in ["live", "interp"] {
                let keys: Vec<&Key> = sim.keys().filter(|k| k.starts_with(group)).collect();
                let res: Vec<(f32, f32)> = keys.iter().filter_map(|k| drawn.get(&((*k).clone(), rule)).map(|d| judder(&sim[*k], d))).filter(|r| r.1 < f32::MAX).collect();
                if res.is_empty() {
                    continue;
                }
                let n = res.len() as f32;
                let (ms, err) = (res.iter().map(|r| r.0).sum::<f32>() / n, res.iter().map(|r| r.1).sum::<f32>() / n);
                lines.push((group, rule, ms, err, keys.len()));
            }
        }
        for (group, rule, ms, err, n) in &lines {
            eprintln!("INT TEAM | {:<16} | {:<6} | {:<6} | {n} object(s): the drawn place = its step path {ms:.0} ms late; frame displacement error RMS {err:.2} cm | {frames} frames, {} steps, {cuts} camera cut frame(s) not blended", attack.name(), group, rule, log.len());
        }
        for group in ["prop", "camera", "Sora"] {
            let get = |rule: &str| lines.iter().find(|l| l.0 == group && l.1 == rule).map(|l| (l.2, l.3));
            if let (Some(live), Some(int)) = (get("live"), get("interp")) {
                assert!(int.1 < 0.6 && int.1 <= live.1 * 0.35 + 0.05, "{} / {group}: frame displacement error interp {:.2} vs live {:.2}", attack.name(), int.1, live.1);
            }
        }
    }
}
