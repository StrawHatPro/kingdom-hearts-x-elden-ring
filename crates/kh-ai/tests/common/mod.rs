//! A tiny host for the tests: kinematic pawns (move / turn / action / item / cure / warp take
//! fixed times), a player, training dummies, a flat floor (StraightNav).
#![allow(dead_code)]

use glam::Vec3;
use kh_ai::*;
use std::sync::Arc;

pub fn pack() -> Arc<AiPack> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party/ai");
    Arc::new(AiPack::load_dir(&dir).expect("run `node tools/party_ai_pack.js` first"))
}

#[derive(Clone, Debug)]
pub struct Pawn {
    pub view: BodyView,
    pub cmd: Option<(u32, AiCommand, f32)>,
    pub ticket: u32,
    pub status: Vec<(u32, AiCommandStatus)>,
    pub log: Vec<AiCommand>,
    pub healed: Vec<(ActorRef, i32)>,
}

impl Pawn {
    pub fn new(member: u8, uid: i32, pos: Vec3) -> Pawn {
        Pawn {
            view: BodyView { member, uid, position: pos, hp: 200, hp_max: 200, mp: 100, mp_max: 100, can_battle: true, items: [Some("POTION"), Some("POTION"), None, None], ..Default::default() },
            cmd: None,
            ticket: 0,
            status: Vec::new(),
            log: Vec::new(),
            healed: Vec::new(),
        }
    }

    fn finish(&mut self, s: AiCommandStatus) {
        if let Some((t, _, _)) = self.cmd.take() {
            self.status.push((t, s));
        }
        self.view.state = "TSID_STATE_IDLE";
    }

    /// Advance the running command (the "pawn tick").
    pub fn step(&mut self, dt: f32, world: &World) {
        let Some((_, cmd, t)) = self.cmd.clone() else { return };
        let t = t + dt;
        if let Some(c) = &mut self.cmd {
            c.2 = t;
        }
        match cmd {
            AiCommand::MoveTo { goal, walk, acceptable_radius, .. } => {
                let gp = match goal {
                    AiGoal::Point(p) => Some(p),
                    AiGoal::Actor(a) => world.actor(a).map(|v| v.position),
                };
                let Some(gp) = gp else { return self.finish(AiCommandStatus::Failed) };
                // capsules do not overlap: an actor goal stops at the touching distance
                let touch = match goal {
                    AiGoal::Actor(a) => world.actor(a).map(|v| v.radius + self.view.radius).unwrap_or(0.0),
                    _ => 0.0,
                };
                let to = (gp - self.view.position).with_z(0.0);
                if to.length() <= acceptable_radius.max(5.0) + touch {
                    self.view.velocity = Vec3::ZERO;
                    return self.finish(AiCommandStatus::Succeeded);
                }
                let speed = if walk { 140.0 } else { 680.0 };
                let step = to.normalize() * (speed * dt).min(to.length());
                self.view.position += step;
                self.view.velocity = to.normalize() * speed;
                self.view.yaw = to.y.atan2(to.x).to_degrees();
                self.view.state = "TSID_STATE_AI_MOVE";
            }
            AiCommand::TurnTo { goal } => {
                if let Some(gp) = match goal {
                    AiGoal::Point(p) => Some(p),
                    AiGoal::Actor(a) => world.actor(a).map(|v| v.position),
                } {
                    let to = gp - self.view.position;
                    self.view.yaw = to.y.atan2(to.x).to_degrees();
                }
                self.finish(AiCommandStatus::Succeeded);
            }
            AiCommand::Action { .. } | AiCommand::Cure { .. } | AiCommand::UseItem { .. } => {
                self.view.state = "TSID_STATE_AI_ACTION";
                if t >= 1.0 {
                    if let AiCommand::Cure { target } = cmd {
                        self.healed.push((target, 100));
                        self.view.mp = 0;
                        self.view.mp_charging = true;
                    }
                    if let AiCommand::UseItem { item, target } = cmd {
                        self.healed.push((target, 80));
                        if let Some(s) = self.view.items.iter_mut().find(|s| **s == Some(item)) {
                            *s = None;
                        }
                    }
                    self.finish(AiCommandStatus::Succeeded);
                }
            }
            AiCommand::Warp { goal, yaw } => {
                self.view.position = goal;
                self.view.yaw = yaw;
                self.finish(AiCommandStatus::Succeeded);
            }
            AiCommand::Idle { .. } => {}
            _ => self.finish(AiCommandStatus::Succeeded),
        }
    }
}

pub struct Body<'a>(pub &'a mut Pawn);

impl AiBody for Body<'_> {
    fn view(&self) -> BodyView {
        self.0.view
    }
    fn function(&self, name: &str) -> Option<f32> {
        match name {
            "CanBattle" => Some(self.0.view.in_battle as i32 as f32),
            _ => None,
        }
    }
    fn command(&mut self, cmd: AiCommand) -> u32 {
        self.0.log.push(cmd.clone());
        match cmd {
            AiCommand::Focus(_) | AiCommand::SetBattleFollowMode(_) => return 0,
            AiCommand::Stop => {
                if let Some((t, _, _)) = self.0.cmd.take() {
                    self.0.status.push((t, AiCommandStatus::Aborted));
                }
                self.0.view.state = "TSID_STATE_IDLE";
                self.0.view.velocity = Vec3::ZERO;
                return 0;
            }
            _ => {}
        }
        if let Some((t, _, _)) = self.0.cmd.take() {
            self.0.status.push((t, AiCommandStatus::Aborted));
        }
        self.0.ticket += 1;
        let t = self.0.ticket;
        self.0.cmd = Some((t, cmd, 0.0));
        t
    }
    fn command_status(&self, ticket: u32) -> AiCommandStatus {
        if let Some((t, _, _)) = &self.0.cmd {
            if *t == ticket {
                return AiCommandStatus::Running;
            }
        }
        self.0.status.iter().rev().find(|(t, _)| *t == ticket).map(|(_, s)| *s).unwrap_or_default()
    }
}

/// The flat-floor navigation of the tests (StraightNav's floor) with switches for the C4 EQS
/// tests: no paths, a fixed wall distance.
#[derive(Clone, Copy, Debug, Default)]
pub struct TestNav {
    pub floor_z: Option<f32>,
    pub no_paths: bool,
    pub wall: Option<f32>,
}

impl Nav for TestNav {
    fn project(&self, p: Vec3, up: f32, down: f32) -> Option<Vec3> {
        StraightNav { floor_z: self.floor_z }.project(p, up, down)
    }
    fn path_exists(&self, _from: Vec3, _to: Vec3) -> bool {
        !self.no_paths
    }
    fn wall_distance(&self, _p: Vec3, max: f32) -> Option<f32> {
        self.wall.filter(|w| *w <= max)
    }
}

pub struct World {
    pub player: PlayerView,
    pub dummies: Vec<ActorView>,
    pub allies: Vec<ActorView>,
    pub camera: CameraView,
    pub nav: TestNav,
    pub gimmicks: Vec<GimmickObstacle>,
    pub volumes: Vec<(String, Vec3, Vec3)>,
    pub focus: Option<(i32, i32)>,
}

impl World {
    pub fn new() -> World {
        let mut p = PlayerView::default();
        p.actor = ActorView { actor: ActorRef::Player, uid: 0, radius: 34.0, ..Default::default() };
        p.hp = 100;
        p.hp_max = 100;
        p.mp = 100;
        p.mp_max = 100;
        p.on_ground = true;
        World { player: p, dummies: Vec::new(), allies: Vec::new(), camera: CameraView { position: Vec3::new(-400.0, 0.0, 200.0), fov: 70.0, aspect: 1.77, ..Default::default() }, nav: TestNav { floor_z: Some(0.0), ..Default::default() }, gimmicks: Vec::new(), volumes: Vec::new(), focus: None }
    }
    pub fn add_dummy(&mut self, id: u32, pos: Vec3) {
        self.dummies.push(ActorView { actor: ActorRef::Enemy(id), uid: -1, position: pos, center: pos + Vec3::Z * 90.0, radius: 50.0, hostile: true, lockable: true, hp: 1000.0, hp_max: 1000.0, ..Default::default() });
    }
    pub fn set_player(&mut self, pos: Vec3, yaw: f32) {
        self.player.actor.position = pos;
        self.player.actor.center = pos + Vec3::Z * 90.0;
        self.player.actor.yaw = yaw;
        self.player.floor_position = pos;
    }
}

impl AiWorld for World {
    fn player(&self) -> PlayerView {
        self.player
    }
    fn camera(&self) -> CameraView {
        self.camera
    }
    fn enemies(&self, out: &mut Vec<ActorView>) {
        out.extend(self.dummies.iter().copied());
    }
    fn allies(&self, out: &mut Vec<ActorView>) {
        out.extend(self.allies.iter().copied());
    }
    fn actor(&self, a: ActorRef) -> Option<ActorView> {
        match a {
            ActorRef::Player => Some(self.player.actor),
            ActorRef::Enemy(_) => self.dummies.iter().find(|d| d.actor == a).copied(),
            ActorRef::Member(_) => self.allies.iter().find(|d| d.actor == a).copied(),
            ActorRef::None => None,
        }
    }
    fn nav(&self) -> &dyn Nav {
        &self.nav
    }
    fn gimmick_obstacles(&self, out: &mut Vec<GimmickObstacle>) {
        out.extend(self.gimmicks.iter().copied());
    }
    fn volumes(&self, class: &str, out: &mut Vec<(Vec3, Vec3)>) {
        out.extend(self.volumes.iter().filter(|(c, _, _)| c == class).map(|(_, a, b)| (*a, *b)));
    }
    fn player_focus(&self) -> Option<(i32, i32)> {
        self.focus
    }
}

/// The party in a test: Donald (member 0) and Goofy (member 1).
pub struct Party {
    pub pack: Arc<AiPack>,
    pub ais: Vec<AiController>,
    pub pawns: Vec<Pawn>,
    pub shared: AiShared,
    pub world: World,
    pub time: f32,
}

impl Party {
    pub fn new() -> Party {
        let pack = pack();
        let lib = Arc::new(kh_ai::bt::Library::compile(&pack));
        let ais = vec![
            AiController::with_library(pack.clone(), lib.clone(), "n_ex001", 0, 11),
            AiController::with_library(pack.clone(), lib, "n_ex002", 1, 12),
        ];
        let mut world = World::new();
        world.set_player(Vec3::ZERO, 0.0);
        let pawns = vec![Pawn::new(0, UID_DONALD, Vec3::new(-150.0, 150.0, 0.0)), Pawn::new(1, UID_GOOFY, Vec3::new(-150.0, -150.0, 0.0))];
        let mut p = Party { pack, ais, pawns, shared: AiShared::new(), world, time: 0.0 };
        p.pawns[0].view.items = [Some("POTION"), Some("ETHER"), None, None];
        p.sync_allies();
        p
    }

    pub fn sync_allies(&mut self) {
        self.world.allies = self
            .pawns
            .iter()
            .map(|pw| ActorView { actor: ActorRef::Member(pw.view.member), uid: pw.view.uid, position: pw.view.position, center: pw.view.position + Vec3::Z * 70.0, yaw: pw.view.yaw, velocity: pw.view.velocity, radius: pw.view.radius, hp: pw.view.hp as f32, hp_max: pw.view.hp_max as f32, alive: !pw.view.ko, state: pw.view.state, ..Default::default() })
            .collect();
    }

    pub fn step(&mut self, dt: f32) {
        self.time += dt;
        for i in 0..self.ais.len() {
            let (ai, pawn) = (&mut self.ais[i], &mut self.pawns[i]);
            ai.tick(dt, &mut Body(pawn), &self.world, &mut self.shared);
        }
        for i in 0..self.pawns.len() {
            let w = &self.world;
            self.pawns[i].step(dt, w);
        }
        self.sync_allies();
    }

    pub fn run(&mut self, seconds: f32) {
        let n = (seconds * 60.0).round() as i32;
        for _ in 0..n {
            self.step(1.0 / 60.0);
        }
    }

    pub fn set_battle(&mut self, on: bool) {
        self.world.player.in_battle = on;
        for p in &mut self.pawns {
            p.view.in_battle = on;
        }
    }

    pub fn path(&self, i: usize) -> String {
        self.ais[i].trace().path.join(" || ")
    }
}
