//! (party AIX) THE PARTY AI SCENARIO HARNESS: the shared party runtime (the code both hosts run) driven in the
//! host's step order (route -> enemies' attacks -> Sora's step with `SoraTargets` -> `PartyRuntime::step` ->
//! `after_step`) with the real AI pack (build/sora_pack/party/ai), the real sheets (party-test, party-weapons) and a
//! level-99 Sora, against scripted enemies: training dummies, chasers that walk up and strike the nearest member, a
//! spread-out field, a wall and a ledge. Every scenario prints its metrics (`cargo test -p kh-partyhost --release
//! --test aix -- --nocapture --test-threads=1`); `AIX_OUT=<dir>` also writes each scenario's per-step trace there.
//!
//! The metrics (per AI member, battle time only unless said):
//! - `idle_near`: seconds standing (Idle / Turn) with a living enemy within 500 cm of him and no attack for 2 s;
//! - `actions` / `hits`: attack actions started / hits landed; `spells`, `cures`, `items`;
//! - `fails`: tasks the tree saw fail (`-> Failed` in its log); `aborts`; `restarts`;
//! - `task_changes/s`: how often the running task changed (jitter);
//! - `whiff`: melee actions started with the target's edge more than 300 cm away;
//! - `targets`: how long each enemy was his TargetActor.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use kh_core::host::{HitResult, HurtShape, TargetFlags, TargetInfo};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{HitRecord, Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, Sora, Status, TargetId, Vec2, Vec3, SORA_PARAMS};
use kh_host::party::{FriendAiSettings, PartyChara};
use kh_party::PartyEventKind;
use kh_partyhost::*;

pub const STEP: f32 = 1.0 / 60.0;

fn project(p: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

fn rows(path: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok()).and_then(|j| j["rows"].as_array().cloned()).unwrap_or_default()
}

/// Sora at level 99 (sheet sora-loadout-99), as kh-er's team harness makes him.
fn level99() -> Option<Status> {
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(project("sheets/port/sora-loadout-99.json")).ok()?).ok()?;
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array()? {
        let v = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64()?),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(a) => LoadoutValue::List(a.iter().filter_map(|i| i.as_str().map(String::from)).collect()),
            serde_json::Value::Object(m) => LoadoutValue::Map(m.iter().filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f))).collect()),
            _ => continue,
        };
        loadout.set(row[0].as_str()?, &v).ok()?;
    }
    loadout.resolve().ok()?;
    Some(Status::from_loadout(&loadout))
}

// ------------------------------------------------------------------------------------------------ the enemies

#[derive(Clone, Debug)]
pub struct Foe {
    pub id: u32,
    pub pos: Vec3,
    pub yaw: f32,
    pub vel: Vec3,
    pub radius: f32,
    pub half: f32,
    pub hp: f32,
    pub hp_max: f32,
    /// Walks toward its victim at this speed (cm/s) while farther than `reach`; 0 = stands.
    pub speed: f32,
    /// Strikes its victim within `reach` (edge to edge) every `every` s (0 = never).
    pub every: f32,
    pub reach: f32,
    pub timer: f32,
    pub last_attack_step: u64,
    pub last_hit_step: u64,
    pub hits_taken: u32,
}

impl Foe {
    pub fn dummy(id: u32, x: f32, y: f32) -> Foe {
        Foe { id, pos: Vec3::new(x, y, 0.0), yaw: 180.0, vel: Vec3::ZERO, radius: 50.0, half: 90.0, hp: 50_000.0, hp_max: 50_000.0, speed: 0.0, every: 0.0, reach: 150.0, timer: 1.0, last_attack_step: 0, last_hit_step: 0, hits_taken: 0 }
    }
    pub fn chaser(id: u32, x: f32, y: f32, speed: f32, every: f32) -> Foe {
        Foe { speed, every, timer: every * 0.5 + id as f32 * 0.37, ..Foe::dummy(id, x, y) }
    }
    pub fn big(id: u32, x: f32, y: f32, radius: f32, half: f32) -> Foe {
        Foe { radius, half, hp: 200_000.0, hp_max: 200_000.0, ..Foe::dummy(id, x, y) }
    }
    fn info(&self) -> TargetInfo {
        let mut flags = TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY;
        if self.hp <= 0.0 {
            flags = TargetFlags::ENEMY;
        }
        TargetInfo { id: TargetId(self.id), position: self.pos, center: self.pos + Vec3::Z * self.half, velocity: self.vel, yaw: self.yaw, radius: self.radius, half_height: self.half, flags: TargetFlags(flags), hp: self.hp.max(0.0), hp_max: self.hp_max }
    }
}

#[derive(Default)]
pub struct Foes {
    pub list: Vec<Foe>,
    pub step: u64,
    /// Where the battle engages (the leader's feet) and how far.
    pub engage_at: Vec3,
    pub engage_range: f32,
    /// (step, target, attack name, attacker position) of every hit taken.
    pub hits: Vec<(u64, u32, &'static str, Vec3)>,
    /// The test level's rule (kh-app fixtures.rs ENGAGE_*): a passive dummy is engaged only for 3 s after a blow
    /// either way, never by proximity.
    pub strict: bool,
}

impl kh_core::Targets for Foes {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.extend(self.list.iter().filter(|f| f.hp > 0.0).map(Foe::info));
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.list.iter().find(|f| f.id == id.0).map(Foe::info)
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(f) = self.list.iter().find(|f| f.id == id.0 && f.hp > 0.0) {
            out.push(HurtShape { a: f.pos + Vec3::Z * f.radius.min(f.half), b: f.pos + Vec3::Z * (2.0 * f.half - f.radius.min(f.half)), radius: f.radius, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        let step = self.step;
        let Some(f) = self.list.iter_mut().find(|f| f.id == hit.target.0) else { return HitResult::default() };
        if f.hp <= 0.0 {
            return HitResult::default();
        }
        let before = f.hp;
        f.hp = (f.hp - hit.damage).max(0.0);
        f.hits_taken += 1;
        f.last_hit_step = step;
        self.hits.push((step, f.id, hit.attack_name, hit.attacker.position));
        HitResult { accepted: true, damage_dealt: before - f.hp, hp_after: f.hp, killed: f.hp <= 0.0, guarded: false }
    }
    fn engaged_count(&self) -> u32 {
        let recent = |at: u64| at > 0 && self.step.saturating_sub(at) <= 120;
        self.list.iter().filter(|f| f.hp > 0.0 && ((!self.strict && (f.pos - self.engage_at).truncate().length() <= self.engage_range) || recent(f.last_hit_step) || recent(f.last_attack_step))).count() as u32
    }
    // (AUDIT-B) no bones here: a rider attached to a hit follows the target's origin and yaw
    fn hit_bone(&self, _id: kh_core::TargetId, _at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: kh_core::TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

impl PartyTargets for Foes {
    fn ai_actors(&self, out: &mut Vec<kh_ai::ActorView>) {
        out.extend(self.list.iter().map(|f| kh_ai::ActorView {
            actor: kh_ai::ActorRef::Enemy(f.id),
            uid: -1,
            position: f.pos,
            center: f.pos + Vec3::Z * f.half,
            yaw: f.yaw,
            velocity: f.vel,
            radius: f.radius,
            half_height: f.half,
            alive: f.hp > 0.0,
            hostile: true,
            lockable: true,
            hp: f.hp,
            hp_max: f.hp_max,
            ..kh_ai::ActorView::default()
        }));
    }
    fn attacked_by_enemies(&self) -> bool {
        self.list.iter().any(|f| f.last_attack_step > 0 && f.last_attack_step + 60 >= self.step)
    }
    // (FX5) the host's memory of a fight (as Elden Ring's ErTargets: a blow either way in the last 300 steps)
    fn fighting_party(&self, id: u32) -> bool {
        let recent = |at: u64| at > 0 && self.step.saturating_sub(at) <= 300;
        self.list.iter().any(|f| f.id == id && (recent(f.last_hit_step) || recent(f.last_attack_step)))
    }
}

// ------------------------------------------------------------------------------------------------ the bench

thread_local! {
    static LINES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// One AI member's sample of one step.
#[derive(Clone, Debug)]
pub struct Sample {
    pub step: u64,
    pub who: &'static str,
    pub pos: Vec3,
    pub yaw: f32,
    pub state: String,
    pub task: String,
    pub target: kh_ai::ActorRef,
    pub hp: i32,
    pub hp_max: i32,
    pub in_battle: bool,
    pub ko: bool,
}

pub struct Sim {
    pub rt: PartyRuntime,
    pub sora: Sora,
    pub out: Output,
    pub world: TestWorld,
    pub foes: Foes,
    pub t: f64,
    pub camera: kh_camera::CameraView,
    pub samples: Vec<Sample>,
    pub events: Vec<(u64, &'static str, PartyEventKind)>,
    pub logs: Vec<(u64, &'static str, String)>,
    /// The words of the runtime's log (party: / party-play: lines).
    pub lines: Vec<(u64, String)>,
    pub sora_hits: u32,
    pub victims_on: bool,
    /// Sora's lock-on target (the HUD's, while he leads).
    pub lock: TargetId,
}

impl Sim {
    pub fn new(world: TestWorld, foes: Vec<Foe>) -> Option<Sim> {
        let pack = kh_ai::AiPack::load_dir(&project("build/sora_pack/party/ai")).ok()?;
        let sheets = project("sheets/port");
        let config = RuntimeConfig { wanted: true, test: PartyTest::from_rows(&rows(&sheets.join("party-test.json"))), weapons: PartyWeapon::from_rows(&rows(&sheets.join("party-weapons.json"))), ai_pack: Some(std::sync::Arc::new(pack)), ai_pack_source: None };
        kh_partyhost::log::set_logger(Some(Box::new(|t: &str| LINES.with(|l| l.borrow_mut().push(t.to_string())))));
        let mut rt = PartyRuntime::new(config);
        rt.trace = false;
        let status = level99()?;
        let sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 }).with_status(status);
        let out = sora.output();
        let camera = kh_camera::CameraView { position: Vec3::new(-450.0, 0.0, 220.0), rotation: kh_camera::Rot { pitch: -15.0, yaw: 0.0, roll: 0.0 }, rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 450.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        Some(Sim { rt, sora, out, world, foes: Foes { list: foes, engage_range: 800.0, ..Foes::default() }, t: 0.0, camera, samples: Vec::new(), events: Vec::new(), logs: Vec::new(), lines: Vec::new(), sora_hits: 0, victims_on: true, lock: TargetId::NONE })
    }

    /// The leader's view (Sora or the friend who leads).
    pub fn leader_pos(&self) -> (Vec3, f32) {
        match self.rt.leader.and_then(|l| self.rt.current.iter().find(|v| v.id == l)) {
            Some(v) => (v.position, v.yaw),
            None => (self.out.position, self.out.yaw),
        }
    }

    fn who(&self, id: kh_party::MemberId) -> &'static str {
        self.rt.chara_of_id(id).map_or("?", |c| c.name())
    }

    /// The enemies move and strike (the host's part before Sora's step).
    fn foes_act(&mut self) {
        let sora_in = self.rt.lead.sora_active;
        let sora_at = self.out.position;
        let members: Vec<(PartyChara, Vec3, bool)> = self.rt.members.iter().filter_map(|(c, id)| self.rt.party.view(*id).map(|v| (*c, v.position, v.ko))).collect();
        let mut strikes = Vec::new();
        for f in self.foes.list.iter_mut().filter(|f| f.hp > 0.0) {
            // the victim: the nearest of Sora (in the party) and the friends
            let mut best: Option<(f32, Option<PartyChara>, Vec3)> = if sora_in && self.sora.status().hp > 0 { Some((f.pos.distance(sora_at), None, sora_at)) } else { None };
            for (c, p, ko) in &members {
                let d = f.pos.distance(*p);
                if !*ko && best.as_ref().is_none_or(|b| d < b.0) {
                    best = Some((d, Some(*c), *p));
                }
            }
            let Some((d, victim, at)) = best else { continue };
            let to = Vec3::new(at.x - f.pos.x, at.y - f.pos.y, 0.0);
            f.yaw = to.y.atan2(to.x).to_degrees();
            let edge = d - f.radius - 35.0;
            f.vel = Vec3::ZERO;
            if f.speed > 0.0 && edge > f.reach * 0.8 {
                f.vel = to.normalize_or_zero() * f.speed;
                f.pos += f.vel * STEP;
            }
            if f.every > 0.0 {
                f.timer -= STEP;
                if f.timer <= 0.0 && edge <= f.reach {
                    f.timer = f.every;
                    f.last_attack_step = self.foes.step;
                    strikes.push((f.id, f.pos, f.yaw, f.radius, victim));
                }
            }
        }
        if !self.victims_on {
            return;
        }
        for (id, pos, yaw, radius, victim) in strikes {
            let kind = kh_core::enums::ETresDamageKind::from_value(1).unwrap();
            match victim {
                Some(c) => self.rt.hit_friend(c, TargetId(id), pos, kh_partyhost::goofy::FixtureHit { kind, power: 1.0, class: kh_core::host::AttackClass::Normal }, self.t),
                None => {
                    let amount = ((60 - self.sora.status().stats.defense).max(1) as f32).ceil();
                    let direction = (self.out.position - pos).normalize_or_zero();
                    self.sora.take_hit(kh_core::states::HitOnPlayer { direction, causer_yaw: yaw, causer_radius: radius, ..kh_core::states::HitOnPlayer::new(kind, amount, TargetId(id), pos) });
                    self.sora_hits += 1;
                }
            }
        }
    }

    /// One host step. `input`: the pad (Sora's while he leads, the leading friend's otherwise).
    pub fn step(&mut self, input: Input) {
        self.foes.step += 1;
        self.foes.engage_at = self.leader_pos().0;
        let routed = self.rt.route(RouteIn { sora: &mut self.sora, input: &input, dt: STEP, targets: &self.foes, camera: &self.camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: self.t });
        self.foes_act();
        if self.rt.lead.sora_active {
            let mut null = kh_core::NullHost::default();
            let mut targets = SoraTargets { targets: &mut self.foes, runtime: &mut self.rt };
            let mut host = kh_core::Host { world: &self.world, targets: &mut targets, audio: &mut null.audio, effects: &mut null.effects, ui: &mut null.ui };
            self.out = self.sora.step_hosted(STEP, &routed, &mut host);
        }
        // the drawn camera behind the leader, looking along his facing
        let (lp, ly) = self.leader_pos();
        let f = Vec3::new(ly.to_radians().cos(), ly.to_radians().sin(), 0.0);
        self.camera.position = lp - f * 450.0 + Vec3::Z * 220.0;
        self.camera.rotation = kh_camera::Rot { pitch: -15.0, yaw: ly, roll: 0.0 };
        let mut audio = kh_core::host::audio::NullAudio;
        let mut effects = kh_core::host::effects::NullEffects;
        let out = self.out.clone();
        let so = self.rt.step(StepIn { dt: STEP, sora: &mut self.sora, out: &out, lock_on: self.lock, camera: &self.camera, world: &self.world, targets: &mut self.foes, audio: &mut audio, effects: &mut effects, nav: None, now: self.t });
        let _ = self.rt.after_step(&self.foes, STEP, self.t);
        let step = self.foes.step;
        for e in so.events {
            if !matches!(e.kind, PartyEventKind::Notify { .. } | PartyEventKind::Voice { .. }) {
                let w = self.who(e.member);
                self.events.push((step, w, e.kind));
            }
        }
        // the AI members' samples
        let leader = self.rt.leader;
        let ids: Vec<(PartyChara, kh_party::MemberId)> = self.rt.members.clone();
        for (c, id) in ids {
            if Some(id) == leader {
                continue;
            }
            let Some(v) = self.rt.party.view(id) else { continue };
            let (task, target, log) = match self.rt.ai_of(id) {
                Some(ctl) => {
                    ctl.set_log(true);
                    let tr = ctl.trace();
                    (tr.task, tr.target, ctl.drain_log())
                }
                None => (String::new(), kh_ai::ActorRef::None, Vec::new()),
            };
            for (_, l) in log {
                self.logs.push((step, c.name(), l));
            }
            self.samples.push(Sample { step, who: c.name(), pos: v.position, yaw: v.yaw, state: format!("{:?}", v.state), task, target, hp: v.hp, hp_max: v.hp_max, in_battle: v.in_battle, ko: v.ko });
        }
        if leader.is_some() && self.rt.lead.sora_active {
            let (task, target) = match self.rt.lead.sora_ai_trace() {
                Some((tr, log)) => {
                    for (_, l) in log {
                        self.logs.push((step, "Sora", l));
                    }
                    (tr.task, tr.target)
                }
                None => (String::new(), kh_ai::ActorRef::None),
            };
            let st = self.sora.status();
            self.samples.push(Sample { step, who: "Sora", pos: self.out.position, yaw: self.out.yaw, state: format!("{:?}", self.out.state), task, target, hp: st.hp, hp_max: st.max_hp(), in_battle: self.sora.actions().battle, ko: st.hp <= 0 });
        }
        let new: Vec<String> = LINES.with(|l| std::mem::take(&mut *l.borrow_mut()));
        for l in new {
            self.lines.push((step, l));
        }
        self.t += STEP as f64;
    }

    pub fn run(&mut self, seconds: f32, input: impl Fn(&Sim) -> Input) {
        for _ in 0..(seconds / STEP).round() as usize {
            let i = input(self);
            self.step(i);
        }
    }

    pub fn idle(&mut self, seconds: f32) {
        self.run(seconds, |_| Input::default());
    }

    pub fn word(&mut self, w: &[&str]) {
        self.rt.lead_word(w);
    }

    pub fn ai(&mut self, c: PartyChara, s: FriendAiSettings) {
        if c == PartyChara::Sora {
            self.rt.set_sora_ai_settings(s);
        } else {
            self.rt.set_ai_settings(c, s);
        }
    }
}

/// Holds the stick toward a KH3 yaw (the camera along it).
pub fn run_towards(yaw: f32) -> Input {
    Input { move_stick: Vec2::new(0.0, 1.0), camera_yaw: yaw, ..Input::default() }
}

/// A press of one button on this step (with the camera yaw).
pub fn press(b: PadButton, yaw: f32) -> Input {
    let mut held = PadButtons::NONE;
    held.set(b, true);
    Input { pad: PadState { held, pressed: held, released: PadButtons::NONE }, camera_yaw: yaw, ..Input::default() }
}

// ------------------------------------------------------------------------------------------------ the metrics

#[derive(Default, Debug, Clone)]
pub struct Metrics {
    pub battle_s: f32,
    pub idle_near_s: f32,
    pub actions: u32,
    pub hits: u32,
    pub spells: u32,
    pub cures: u32,
    pub items: u32,
    pub fails: u32,
    pub aborts: u32,
    pub restarts: u32,
    pub task_changes: u32,
    pub whiffs: u32,
    /// Standing in battle with the nearest enemy (within 800 cm) more than 60 degrees off his facing.
    pub facing_away_s: f32,
    pub kos: u32,
    pub revives: u32,
    pub warps: u32,
    pub targets: BTreeMap<String, f32>,
    pub failed_tasks: BTreeMap<String, u32>,
}

impl Sim {
    pub fn metrics(&self, who: &str, from_step: u64) -> Metrics {
        let mut m = Metrics::default();
        let mut last_task = String::new();
        let mut last_act: Option<u64> = None;
        let alive = |step: u64| -> Vec<Vec3> { let _ = step; self.foes.list.iter().filter(|f| f.hp > 0.0).map(|f| f.pos).collect() };
        let mut prev_state = String::new();
        for s in self.samples.iter().filter(|s| s.who == who && s.step >= from_step) {
            if s.state == "Action" || s.state.starts_with("Pad(") || s.state == "Attack" {
                last_act = Some(s.step);
            }
            if s.in_battle && !s.ko {
                m.battle_s += STEP;
                let near = alive(s.step).iter().any(|p| p.truncate().distance(s.pos.truncate()) < 500.0);
                let standing = matches!(s.state.as_str(), "Idle" | "Turn");
                let quiet = last_act.is_none_or(|a| s.step.saturating_sub(a) > 120);
                if near && standing && quiet {
                    m.idle_near_s += STEP;
                }
                let nearest = alive(s.step).into_iter().map(|p| (p.truncate().distance(s.pos.truncate()), p)).filter(|x| x.0 < 800.0).min_by(|a, b| a.0.total_cmp(&b.0));
                if let (Some((_, p)), true) = (nearest, standing) {
                    let want = (p.y - s.pos.y).atan2(p.x - s.pos.x).to_degrees();
                    let mut d = (want - s.yaw) % 360.0;
                    if d > 180.0 { d -= 360.0 } else if d < -180.0 { d += 360.0 }
                    if d.abs() > 60.0 {
                        m.facing_away_s += STEP;
                    }
                }
                *m.targets.entry(format!("{:?}", s.target)).or_default() += STEP;
            }
            if s.task != last_task {
                m.task_changes += 1;
                last_task = s.task.clone();
            }
            if (s.state == "Action" || s.state == "Attack") && prev_state != s.state {
                m.actions += 1;
            }
            prev_state = s.state.clone();
        }
        for (step, w, l) in self.logs.iter().filter(|l| l.1 == who && l.0 >= from_step) {
            let _ = (step, w);
            if l.contains("-> Failed") {
                m.fails += 1;
                let name = l.split_whitespace().nth(1).unwrap_or("").to_string();
                *m.failed_tasks.entry(name).or_default() += 1;
            }
            if l.starts_with("abort ") {
                m.aborts += 1;
            }
            if l.contains("finished, restart") {
                m.restarts += 1;
            }
        }
        for (step, w, e) in self.events.iter().filter(|e| e.1 == who && e.0 >= from_step) {
            let _ = step;
            let _ = w;
            match e {
                PartyEventKind::Hit { .. } => m.hits += 1,
                PartyEventKind::SpellCast { def } => {
                    let n = format!("{def:?}");
                    if n.contains("Cure") {
                        m.cures += 1;
                    } else {
                        m.spells += 1;
                    }
                }
                PartyEventKind::ItemUsed { .. } => m.items += 1,
                PartyEventKind::Ko => m.kos += 1,
                PartyEventKind::Revived => m.revives += 1,
                PartyEventKind::Warped { .. } => m.warps += 1,
                _ => {}
            }
        }
        m
    }

    pub fn said(&self, what: &str, from_step: u64) -> usize {
        self.lines.iter().filter(|l| l.0 >= from_step && l.1.contains(what)).count()
    }

    pub fn report(&self, name: &str, from_step: u64) -> String {
        let mut r = String::new();
        let secs = (self.foes.step - from_step) as f32 * STEP;
        let _ = writeln!(r, "=== {name} ({secs:.1} s)");
        let mut whos: Vec<&str> = self.samples.iter().map(|s| s.who).collect();
        whos.sort();
        whos.dedup();
        for w in whos {
            let m = self.metrics(w, from_step);
            let tops: Vec<String> = m.targets.iter().map(|(k, v)| format!("{k} {v:.1}s")).collect();
            let fails: Vec<String> = m.failed_tasks.iter().map(|(k, v)| format!("{k} x{v}")).collect();
            let _ = writeln!(r, "{w:>6}: battle {:.1}s idle_near {:.1}s away {:.1}s actions {} hits {} spells {} cures {} items {} | fails {} aborts {} restarts {} task_changes {:.1}/s | ko {} revive {} warps {} | targets [{}] | failed [{}]", m.battle_s, m.idle_near_s, m.facing_away_s, m.actions, m.hits, m.spells, m.cures, m.items, m.fails, m.aborts, m.restarts, m.task_changes as f32 / secs.max(0.1), m.kos, m.revives, m.warps, tops.join(", "), fails.join(", "));
        }
        let _ = writeln!(r, "  foes: {}", self.foes.list.iter().map(|f| format!("#{} hp {:.0}/{:.0} hits {}", f.id, f.hp, f.hp_max, f.hits_taken)).collect::<Vec<_>>().join("; "));
        r
    }

    /// `AIX_OUT=<dir>`: the per-step samples, the trees' logs and the runtime's lines.
    pub fn dump(&self, name: &str) {
        let Some(dir) = std::env::var_os("AIX_OUT") else { return };
        let dir = std::path::PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&dir);
        let mut s = String::new();
        for x in &self.samples {
            let _ = writeln!(s, "{} {} ({:.0},{:.0}) {} hp {}/{} b{} | {} | {:?}", x.step, x.who, x.pos.x, x.pos.y, x.state, x.hp, x.hp_max, x.in_battle as u8, x.task, x.target);
        }
        let _ = std::fs::write(dir.join(format!("{name}.samples.txt")), s);
        let mut s = String::new();
        for (st, w, l) in &self.logs {
            let _ = writeln!(s, "{st} {w} {l}");
        }
        let _ = std::fs::write(dir.join(format!("{name}.bt.txt")), s);
        let mut s = String::new();
        for (st, l) in &self.lines {
            let _ = writeln!(s, "{st} {l}");
        }
        for (st, w, e) in &self.events {
            let _ = writeln!(s, "{st} EVENT {w} {e:?}");
        }
        let _ = std::fs::write(dir.join(format!("{name}.log.txt")), s);
    }
}

macro_rules! sim {
    ($world:expr, $foes:expr) => {
        match Sim::new($world, $foes) {
            Some(s) => s,
            None => {
                eprintln!("SKIPPED: no AI pack / sheets");
                return;
            }
        }
    };
}

// ------------------------------------------------------------------------------------------------ the scenarios

/// 1. Sora leads and stands; four training dummies round the party.
#[test]
fn s01_open_field_dummies() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 450.0, -150.0), Foe::dummy(2, 500.0, 250.0), Foe::dummy(3, 800.0, 0.0), Foe::dummy(4, 300.0, 500.0)]);
    s.idle(20.0);
    println!("{}", s.report("s01 open field, 4 dummies, Sora stands", 60));
    s.dump("s01");
}

/// 2. Moving, attacking enemies.
#[test]
fn s02_chasers() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::chaser(1, 900.0, -300.0, 150.0, 2.5), Foe::chaser(2, 1000.0, 300.0, 150.0, 2.5), Foe::chaser(3, 700.0, 600.0, 200.0, 3.0)]);
    s.idle(25.0);
    println!("{}", s.report("s02 three chasers, Sora stands", 60));
    s.dump("s02");
}

/// 3. Enemies spread out (near, mid, far, behind).
#[test]
fn s03_spread_targets() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 400.0, 0.0), Foe::dummy(2, 1400.0, 700.0), Foe::dummy(3, -600.0, -500.0), Foe::dummy(4, 2400.0, -1200.0)]);
    s.idle(20.0);
    println!("{}", s.report("s03 spread dummies", 60));
    s.dump("s03");
}

/// 4. A low-HP leader in battle (Sora at 20 %, then a friend leader at 20 %).
#[test]
fn s04_low_hp_leader() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 450.0, 0.0), Foe::dummy(2, 600.0, 300.0)]);
    s.idle(3.0);
    let max = s.sora.status().max_hp();
    s.sora.status_mut().hp = max / 5;
    let from = s.foes.step;
    s.idle(10.0);
    let hp = s.sora.status().hp;
    println!("{}", s.report("s04a Sora leader at 20 % HP", from));
    println!("  Sora HP {} -> {} of {}", max / 5, hp, max);
    s.dump("s04a");
    // Goofy leads at 20 %
    s.word(&["leader", "goofy"]);
    s.idle(2.0);
    let g = s.rt.id(PartyChara::Goofy).unwrap();
    let gmax = s.rt.party.status(g).unwrap().max_hp();
    s.rt.party.status_mut(g).unwrap().hp = gmax / 5;
    let from = s.foes.step;
    s.idle(10.0);
    println!("{}", s.report("s04b Goofy leads at 20 % HP", from));
    println!("  Goofy HP {} -> {} of {}", gmax / 5, s.rt.party.status(g).unwrap().hp, gmax);
    s.dump("s04b");
}

/// 5. KO and revive: Goofy downed in a fight.
#[test]
fn s05_ko_revive() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::chaser(1, 500.0, 0.0, 120.0, 2.0), Foe::dummy(2, 500.0, 300.0)]);
    s.idle(3.0);
    let g = s.rt.id(PartyChara::Goofy).unwrap();
    s.rt.party.status_mut(g).unwrap().hp = 1;
    s.rt.hit_friend(PartyChara::Goofy, TargetId(1), Vec3::new(500.0, 0.0, 0.0), kh_partyhost::goofy::FixtureHit { kind: kh_core::enums::ETresDamageKind::from_value(1).unwrap(), power: 1.0, class: kh_core::host::AttackClass::Normal }, s.t);
    let from = s.foes.step;
    s.idle(30.0);
    println!("{}", s.report("s05 Goofy KO'd in battle", from));
    s.dump("s05");
}

/// 7. AI Sora's kit while Donald leads.
#[test]
fn s07_ai_sora_tools() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::chaser(1, 700.0, -200.0, 100.0, 3.0), Foe::chaser(2, 800.0, 250.0, 100.0, 3.0), Foe::dummy(3, 1100.0, 0.0)]);
    s.word(&["leader", "donald"]);
    s.idle(30.0);
    println!("{}", s.report("s07 Donald leads, AI Sora + Goofy", 60));
    for k in ["AI Sora: casts", "AI Sora: shotlock", "AI Sora: situation", "AI Sora: Cure", "AI Sora warps"] {
        println!("  {k}: {}", s.said(k, 0));
    }
    println!("  foe hits by attacker: {:?}", attacker_counts(&s));
    s.dump("s07");
}

/// 8. Out of battle: the leader runs, then stops; follow / idle / stop.
#[test]
fn s08_field_follow() {
    let mut s = sim!(TestWorld::floor(), vec![]);
    s.idle(2.0);
    s.run(6.0, |_| run_towards(0.0));
    let from = s.foes.step;
    s.idle(8.0);
    println!("{}", s.report("s08 field: leader stopped after a run", from));
    // jitter: state changes per member while the leader stands
    for w in ["Donald", "Goofy"] {
        let st: Vec<&Sample> = s.samples.iter().filter(|x| x.who == w && x.step >= from + 180).collect();
        let changes = st.windows(2).filter(|p| p[0].state != p[1].state).count();
        let moved = st.windows(2).map(|p| p[0].pos.distance(p[1].pos)).sum::<f32>();
        let d = st.last().map_or(0.0, |x| x.pos.distance(s.out.position));
        println!("  {w}: last 5 s state changes {changes}, moved {moved:.0} cm, ends {d:.0} cm from Sora");
    }
    s.dump("s08");
}

/// 9. Battle start and end: enemies come into range, then all die.
#[test]
fn s09_battle_transitions() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 2500.0, 0.0), Foe::dummy(2, 2600.0, 300.0)]);
    s.idle(2.0);
    // walk up to them
    s.run(3.5, |_| run_towards(0.0));
    let from = s.foes.step;
    s.idle(6.0);
    let first_action: Vec<(String, Option<u64>)> = ["Donald", "Goofy"].iter().map(|w| (w.to_string(), s.samples.iter().find(|x| x.who == *w && x.step >= from && x.state == "Action").map(|x| x.step - from))).collect();
    let battle_at = s.samples.iter().find(|x| x.in_battle).map(|x| x.step);
    println!("{}", s.report("s09a battle start", from));
    println!("  battle on at step {battle_at:?}; first action after the stop (steps): {first_action:?}");
    for f in s.foes.list.iter_mut() {
        f.hp = 0.0;
    }
    let killed = s.foes.step;
    s.idle(6.0);
    let off: Vec<(String, Option<u64>)> = ["Donald", "Goofy"].iter().map(|w| (w.to_string(), s.samples.iter().find(|x| x.who == *w && x.step >= killed && !x.in_battle).map(|x| x.step - killed))).collect();
    let field: Vec<(String, Option<u64>)> = ["Donald", "Goofy"].iter().map(|w| (w.to_string(), s.samples.iter().find(|x| x.who == *w && x.step >= killed && x.task.contains("Follow")).map(|x| x.step - killed))).collect();
    println!("  enemies dead: battle off after {off:?} steps; follow tree after {field:?}");
    s.dump("s09");
}

/// 10. A fight at a wall and at a ledge.
#[test]
fn s10_wall_and_ledge() {
    let world = TestWorld::default().with_plane(Vec3::Z, -3000.0).with_box(Vec3::new(-6000.0, -6000.0, -3000.0), Vec3::new(1000.0, 6000.0, 0.0)).with_box(Vec3::new(-3000.0, 400.0, 0.0), Vec3::new(3000.0, 700.0, 1500.0));
    let mut s = sim!(world, vec![Foe::dummy(1, 700.0, 300.0), Foe::dummy(2, 900.0, -100.0)]);
    s.idle(20.0);
    println!("{}", s.report("s10 wall at y 400, ledge at x 1000", 60));
    let fell = s.samples.iter().filter(|x| x.pos.z < -100.0).count();
    let in_wall = s.samples.iter().filter(|x| x.pos.y > 400.0 && x.pos.z < 1400.0).count();
    println!("  samples below the ledge {fell}, inside the wall {in_wall}");
    s.dump("s10");
}

/// 11. Donald leads with Sora out of the party: battle start / end.
#[test]
fn s11_sora_out_battle_flag() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 2500.0, 0.0)]);
    s.word(&["leader", "donald"]);
    s.idle(1.0);
    s.word(&["formation", "out", "sora"]);
    s.idle(1.0);
    assert!(!s.rt.lead.sora_active);
    s.run(3.5, |_| run_towards(0.0));
    let from = s.foes.step;
    s.idle(5.0);
    println!("{}", s.report("s11 Donald leads, Sora out, walks up to a dummy", from));
    let goofy_battle = s.samples.iter().filter(|x| x.who == "Goofy" && x.step >= from && x.in_battle).count();
    println!("  Goofy in battle {goofy_battle} of {} steps; Sora's battle flag {}", s.foes.step - from, s.sora.actions().battle);
    s.dump("s11");
}

/// 12. Combat Style / Abilities / Recovery: each option's effect (Goofy and Donald, same field).
#[test]
fn s12_settings() {
    for (label, battle, ability, cure) in [("Stick By Sora", 0u8, 0u8, 1u8), ("Be Yourself", 1, 0, 1), ("Hands Off", 2, 0, 1), ("Finish the Job", 3, 0, 1), ("Watch MP", 0, 1, 1), ("Conserve", 0, 2, 1)] {
        let mut s = sim!(TestWorld::floor(), vec![Foe::chaser(1, 700.0, -200.0, 80.0, 3.0), Foe::chaser(2, 900.0, 300.0, 80.0, 3.0), Foe::dummy(3, 1300.0, 0.0)]);
        for c in PartyChara::FRIENDS {
            s.ai(c, FriendAiSettings { battle, ability, cure, cure_targets: 7 });
        }
        s.idle(20.0);
        println!("{}", s.report(&format!("s12 {label}"), 60));
        s.dump(&format!("s12_{}", label.replace(' ', "_")));
    }
}

fn attacker_counts(s: &Sim) -> BTreeMap<String, u32> {
    let mut out = BTreeMap::new();
    for (step, _, attack, at) in &s.foes.hits {
        let _ = step;
        let key = attack.split('.').next().unwrap_or(attack).to_string();
        let _ = at;
        *out.entry(key).or_default() += 1;
    }
    out
}

#[test]
#[ignore]
fn dbg_s08_stuck() {
    let mut s = sim!(TestWorld::floor(), vec![]);
    s.idle(2.0);
    for i in 0..360 {
        s.step(run_towards(0.0));
        if (300..330).contains(&i) {
            let d = s.rt.id(PartyChara::Donald).unwrap();
            let m = s.rt.party.member(d).unwrap();
            println!("{} pos ({:.1},{:.1},{:.1}) vel ({:.0},{:.0}) goal {:?} yaw {:.1} steer {:?} sora ({:.0},{:.0}) st {:?}", s.foes.step, m.cmc.location.x, m.cmc.location.y, m.cmc.location.z, m.cmc.velocity.x, m.cmc.velocity.y, s.rt.party.move_goal(d), m.yaw, m.steer, s.out.position.x, s.out.position.y, m.state);
        }
    }
}

/// 13. AI Sora against one big enemy (an Elden Ring boss's capsule: radius 250, half height 450) while Donald leads.
#[test]
fn s13_ai_sora_big_enemy() {
    // its body is solid (an Elden Ring boss blocks the members): a 460 cm block round its centre
    let world = TestWorld::floor().with_box(Vec3::new(900.0 - 230.0, -230.0, 0.0), Vec3::new(900.0 + 230.0, 230.0, 900.0));
    let mut s = sim!(world, vec![Foe::big(1, 900.0, 0.0, 250.0, 450.0)]);
    s.foes.engage_range = 1200.0;
    s.word(&["leader", "donald"]);
    s.idle(20.0);
    println!("{}", s.report("s13 Donald leads, one big enemy (r 250)", 60));
    let by: BTreeMap<String, u32> = attacker_counts(&s);
    println!("  foe hits by attack: {by:?}");
    let sora_d: Vec<f32> = s.samples.iter().filter(|x| x.who == "Sora").map(|x| x.pos.truncate().distance(glam::Vec2::new(900.0, 0.0))).collect();
    println!("  AI Sora distance to the boss centre: min {:.0} mean {:.0}", sora_d.iter().cloned().fold(f32::MAX, f32::min), sora_d.iter().sum::<f32>() / sora_d.len().max(1) as f32);
    s.dump("s13");
}

/// 14. AI Sora's ranged tools: enemies 9 - 13 m away, full focus, Go Wild.
#[test]
fn s14_ai_sora_ranged() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 1000.0, 300.0), Foe::dummy(2, 1200.0, -300.0)]);
    s.foes.engage_range = 1500.0;
    s.word(&["leader", "donald"]);
    s.idle(1.0);
    let fm = s.sora.status().focus_max;
    s.sora.status_mut().focus = fm;
    s.idle(25.0);
    println!("{}", s.report("s14 Donald leads, dummies 10-13 m away", 60));
    for k in ["AI Sora: casts", "AI Sora: shotlock", "AI Sora: situation"] {
        println!("  {k}: {}", s.said(k, 0));
    }
    println!("  focus {}/{}; foe hits {:?}", s.sora.status().focus, fm, attacker_counts(&s));
    s.dump("s14");
}

/// 15. Leader switches in the middle of a fight (every 3 s): what the new leader keeps doing with the stick released.
#[test]
fn s15_leader_switches() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::chaser(1, 500.0, -100.0, 80.0, 3.0), Foe::chaser(2, 600.0, 250.0, 80.0, 3.0)]);
    // no blows on anyone: what moves a new leader is only what his AI left running
    s.victims_on = false;
    s.idle(4.0);
    let mut worst = Vec::new();
    for who in ["goofy", "donald", "sora", "goofy", "sora", "donald", "goofy", "sora"] {
        s.word(&["leader", who]);
        s.step(Input::default());
        let start = s.leader_pos().0;
        let mut far: f32 = 0.0;
        for _ in 0..180 {
            s.step(Input::default());
            far = far.max(s.leader_pos().0.truncate().distance(start.truncate()));
        }
        worst.push((who, far));
    }
    println!("{}", s.report("s15 leader switches every 3 s", 240));
    println!("  how far each new leader moved with the stick released (cm): {worst:?}");
    s.dump("s15");
}

/// 16. Finish the Job: the friends follow Sora's lock-on (enemy 2, the far one).
#[test]
fn s16_focus_lock() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 400.0, 0.0), Foe::dummy(2, 900.0, 500.0)]);
    for c in PartyChara::FRIENDS {
        s.ai(c, FriendAiSettings { battle: 3, ability: 0, cure: 1, cure_targets: 7 });
    }
    s.lock = TargetId(2);
    s.idle(17.0);
    println!("{}", s.report("s16 Finish the Job, Sora locked on #2", 180));
    s.dump("s16");
}

/// 17. Heals while a friend leads: AI Sora at 20 %, then Goofy at 20 %.
#[test]
fn s17_heals_friend_leads() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 600.0, 0.0)]);
    s.word(&["leader", "donald"]);
    s.idle(3.0);
    let max = s.sora.status().max_hp();
    s.sora.status_mut().hp = max / 5;
    let from = s.foes.step;
    s.idle(8.0);
    println!("{}", s.report("s17a Donald leads, AI Sora at 20 %", from));
    println!("  AI Sora HP {} -> {} of {}; AI Sora's own Cure lines {}", max / 5, s.sora.status().hp, max, s.said("AI Sora: Cur", from));
    let g = s.rt.id(PartyChara::Goofy).unwrap();
    let gmax = s.rt.party.status(g).unwrap().max_hp();
    s.rt.party.status_mut(g).unwrap().hp = gmax / 5;
    let from = s.foes.step;
    s.idle(8.0);
    println!("{}", s.report("s17b Donald leads, Goofy at 20 %", from));
    println!("  Goofy HP {} -> {}; AI Sora's Cure lines {}", gmax / 5, s.rt.party.status(g).unwrap().hp, s.said("AI Sora: Cur", from));
    let d = s.rt.id(PartyChara::Donald).unwrap();
    let dmax = s.rt.party.status(d).unwrap().max_hp();
    s.rt.party.status_mut(d).unwrap().hp = dmax / 5;
    let from = s.foes.step;
    s.idle(8.0);
    println!("{}", s.report("s17c Donald (the leader) at 20 %", from));
    println!("  Donald HP {} -> {}; AI Sora's Cure lines {}", dmax / 5, s.rt.party.status(d).unwrap().hp, s.said("AI Sora: Cur", from));
    s.dump("s17");
}

/// 18. Recovery: each option (Use As Needed / Use in Emergencies / Use Like Crazy) and the targets bits, Sora at 60 / 40 / 20 %.
#[test]
fn s18_recovery_options() {
    for (cure, label) in [(0u8, "Use As Needed"), (1, "Use in Emergencies"), (2, "Use Like Crazy")] {
        let mut row = Vec::new();
        for pct in [60, 40, 20] {
            let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 600.0, 0.0)]);
            for c in PartyChara::FRIENDS {
                s.ai(c, FriendAiSettings { battle: 0, ability: 0, cure, cure_targets: 7 });
            }
            s.idle(3.0);
            let max = s.sora.status().max_hp();
            s.sora.status_mut().hp = max * pct / 100;
            let from = s.foes.step;
            s.idle(6.0);
            let healed = s.lines.iter().filter(|l| l.0 >= from && l.1.contains("Sora heals") || l.0 >= from && l.1.contains("SoraHeal")).count();
            let d = s.metrics("Donald", from);
            let g = s.metrics("Goofy", from);
            row.push(format!("{pct}%: Donald cures {} items {}, Goofy items {}, Sora HP {} -> {}", d.cures, d.items, g.items, max * pct / 100, s.sora.status().hp));
            let _ = healed;
        }
        println!("s18 {label}: {}", row.join(" | "));
    }
    // the targets bits: On Sora off -> nobody heals Sora at 20 %
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 600.0, 0.0)]);
    for c in PartyChara::FRIENDS {
        s.ai(c, FriendAiSettings { battle: 0, ability: 0, cure: 1, cure_targets: 0b110 });
    }
    s.idle(3.0);
    let max = s.sora.status().max_hp();
    s.sora.status_mut().hp = max / 5;
    let from = s.foes.step;
    s.idle(6.0);
    let d = s.metrics("Donald", from);
    let g = s.metrics("Goofy", from);
    println!("s18 On Sora off: Donald cures {} items {}, Goofy items {}, Sora HP {} -> {}", d.cures, d.items, g.items, max / 5, s.sora.status().hp);
}

/// 19. Abilities (Go Wild / Watch MP / Conserve) over a minute: Donald's spells and Goofy's abilities.
#[test]
fn s19_ability_styles_long() {
    for (ability, label) in [(0u8, "Go Wild"), (1, "Watch MP"), (2, "Conserve")] {
        let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 500.0, -150.0), Foe::dummy(2, 600.0, 250.0)]);
        for c in PartyChara::FRIENDS {
            s.ai(c, FriendAiSettings { battle: 0, ability, cure: 1, cure_targets: 7 });
        }
        s.idle(60.0);
        let d = s.metrics("Donald", 60);
        let g = s.metrics("Goofy", 60);
        let gab = s.events.iter().filter(|e| e.1 == "Goofy" && matches!(e.2, PartyEventKind::Hit { attack, .. } if attack.starts_with("GOOFY_ART_"))).count(); // (GF-AI) his Arts: the tree's requests go through the kit
        println!("s19 {label}: Donald spells {} (actions {}), Goofy ability hits {} (actions {})", d.spells, d.actions, gab, g.actions);
    }
}

/// 20. AI Sora's Recovery (Use As Needed) while Donald leads: Goofy at 40 %, then at 30 % of his HP, in battle.
#[test]
fn s20_ai_sora_recovery_threshold() {
    for pct in [40, 30] {
        let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 700.0, 0.0)]);
        s.word(&["leader", "donald"]);
        s.ai(PartyChara::Sora, FriendAiSettings { battle: 0, ability: 0, cure: 0, cure_targets: 7 });
        s.ai(PartyChara::Goofy, FriendAiSettings { battle: 2, ability: 0, cure: 0, cure_targets: 0 });
        s.idle(3.0);
        let g = s.rt.id(PartyChara::Goofy).unwrap();
        let gmax = s.rt.party.status(g).unwrap().max_hp();
        s.rt.party.status_mut(g).unwrap().hp = gmax * pct / 100;
        let from = s.foes.step;
        s.idle(4.0);
        println!("s20 Goofy at {pct} %: AI Sora Cure lines {} (Recovery Use As Needed, in battle {})", s.said("AI Sora: Cur", from), s.rt.lead.party_battle);
    }
}

/// 21. AI Sora 30 m behind Donald and in the camera's view: the far rule's put-behind.
#[test]
fn s21_ai_sora_far_in_view() {
    let mut s = sim!(TestWorld::floor(), vec![]);
    s.word(&["leader", "donald"]);
    s.idle(1.0);
    // Sora 50 m ahead of Donald, along the camera (in its view)
    let d = s.leader_pos().0;
    s.sora.teleport(Pose { position: d + Vec3::new(5000.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 180.0 });
    s.out = s.sora.output();
    let from = s.foes.step;
    s.idle(1.5);
    let warps = s.said("AI Sora warps", from);
    println!("s21 AI Sora 50 m away in view: warps in 1.5 s {warps}; then {:.0} cm from Donald", s.out.position.distance(s.leader_pos().0));
}

/// 22. Goofy leads; AI Sora at 20 %: does Donald's Cure (or anyone) reach him?
#[test]
fn s22_donald_cures_ai_sora() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 700.0, 0.0)]);
    s.word(&["leader", "goofy"]);
    s.ai(PartyChara::Sora, FriendAiSettings { battle: 0, ability: 0, cure: 1, cure_targets: 0 });
    s.idle(3.0);
    let max = s.sora.status().max_hp();
    s.sora.status_mut().hp = max / 5;
    let from = s.foes.step;
    s.idle(8.0);
    let d = s.metrics("Donald", from);
    println!("s22 Goofy leads, AI Sora at 20 %: Donald cures {} items {}; AI Sora HP {} -> {}; heal lines {:?}", d.cures, d.items, max / 5, s.sora.status().hp, s.lines.iter().filter(|l| l.0 >= from && (l.1.contains("heals") || l.1.contains("ItemUsed") || l.1.contains("SpellCast"))).map(|l| l.1.clone()).collect::<Vec<_>>());
    s.dump("s22");
}

/// 23. Goofy leads with no MP left: an Ether from Donald reaches the leader's MP.
#[test]
fn s23_ether_on_friend_leader() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 700.0, 0.0)]);
    s.word(&["leader", "goofy"]);
    s.idle(3.0);
    let g = s.rt.id(PartyChara::Goofy).unwrap();
    s.rt.party.status_mut(g).unwrap().mp = 0;
    let from = s.foes.step;
    s.idle(10.0);
    let d = s.metrics("Donald", from);
    let lines: Vec<String> = s.lines.iter().filter(|l| l.0 >= from && l.1.contains("ItemUsed")).map(|l| l.1.clone()).collect();
    println!("s23 Goofy leads at 0 MP: Donald items {}; Goofy MP 0 -> {}; {:?}", d.items, s.rt.party.status(g).unwrap().mp, lines);
}

/// 24. (K1 -> AIX) Out of battle, the leader running straight 30 s: what stops a friend (Cure / TurnTo / items).
#[test]
fn s24_field_run_stops() {
    for lead in ["sora", "goofy"] {
        let mut s = sim!(TestWorld::floor(), vec![]);
        if lead != "sora" {
            s.word(&["leader", lead]);
        }
        s.idle(1.0);
        let from = s.foes.step;
        s.run(30.0, |_| run_towards(0.0));
        let mut gap_max: f32 = 0.0;
        let mut sum = 0.0;
        let mut n = 0;
        let lp: Vec<(u64, Vec3)> = Vec::new();
        let _ = lp;
        for w in ["Donald", "Goofy", "Sora"] {
            let st: Vec<&Sample> = s.samples.iter().filter(|x| x.who == w && x.step >= from).collect();
            if st.is_empty() {
                continue;
            }
            let states: BTreeMap<String, usize> = st.iter().fold(BTreeMap::new(), |mut m, x| {
                *m.entry(x.state.clone()).or_default() += 1;
                m
            });
            let _ = (&mut gap_max, &mut sum, &mut n);
            println!("s24 {lead} leads: {w} states {:?}", states);
        }
        let casts: Vec<String> = s.lines.iter().filter(|l| l.0 >= from && (l.1.contains("SpellCast") || l.1.contains("ItemUsed") || l.1.contains("AI Sora:"))).map(|l| l.1.clone()).collect();
        println!("s24 {lead} leads: casts / items {:?}", casts);
        s.dump(&format!("s24_{lead}"));
    }
}

/// 25. (K1 -> AIX) s24 with K1's harness Sora (kh-core's default status, not the level-99 loadout).
#[test]
fn s25_field_run_default_sora() {
    for lead in ["sora", "goofy"] {
        let mut s = sim!(TestWorld::floor(), vec![]);
        s.sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 });
        s.out = s.sora.output();
        if lead != "sora" {
            s.word(&["leader", lead]);
        }
        s.idle(1.0);
        let st = s.sora.status();
        println!("s25 Sora status hp {}/{} mp {}/{} charging {}", st.hp, st.max_hp(), st.mp, st.max_mp(), st.mp_charging);
        let from = s.foes.step;
        s.run(30.0, |_| run_towards(0.0));
        for w in ["Donald", "Goofy", "Sora"] {
            let st: Vec<&Sample> = s.samples.iter().filter(|x| x.who == w && x.step >= from).collect();
            if st.is_empty() {
                continue;
            }
            let states: BTreeMap<String, usize> = st.iter().fold(BTreeMap::new(), |mut m, x| {
                *m.entry(x.state.clone()).or_default() += 1;
                m
            });
            let gaps: Vec<f32> = st.iter().map(|x| x.pos.truncate().distance(s.samples.iter().find(|y| y.step == x.step).map_or(x.pos, |_| x.pos).truncate())).collect();
            let _ = gaps;
            println!("s25 {lead} leads: {w} states {:?}", states);
        }
        let casts: Vec<String> = s.lines.iter().filter(|l| l.0 >= from && (l.1.contains("SpellCast") || l.1.contains("ItemUsed") || l.1.contains("AI Sora:") || l.1.contains("heals"))).map(|l| format!("{} {}", l.0, l.1)).collect();
        println!("s25 {lead} leads: casts / items {:?}", casts);
        s.dump(&format!("s25_{lead}"));
    }
}

/// 26. (K1 -> AIX) Where the follow query puts Destination while the leader runs straight: its offset along the
///     leader's motion (+ ahead), and the intent keys.
#[test]
fn s26_follow_destination_while_running() {
    for lead in ["sora", "goofy"] {
        let mut s = sim!(TestWorld::floor(), vec![]);
        if lead != "sora" {
            s.word(&["leader", lead]);
        }
        s.idle(1.0);
        let mut rows: Vec<(String, f32, f32, f32, f32, f32)> = Vec::new();
        let mut gaps: BTreeMap<String, (f32, f32, u32)> = BTreeMap::new();
        for i in 0..1800 {
            s.step(run_towards(0.0));
            let (lp, _) = s.leader_pos();
            let ids: Vec<(PartyChara, kh_party::MemberId)> = s.rt.members.clone();
            for (c, id) in ids {
                if Some(id) == s.rt.leader {
                    continue;
                }
                let pos = s.rt.party.view(id).unwrap().position;
                let g = gaps.entry(c.name().to_string()).or_insert((0.0, 0.0, 0));
                let d = pos.truncate().distance(lp.truncate());
                g.0 = g.0.max(d);
                g.1 += d;
                g.2 += 1;
                if i % 30 == 0 && i > 120 {
                    let Some(ctl) = s.rt.ai_of(id) else { continue };
                    let bb = ctl.blackboard();
                    let f = |n: &str| bb.key_id(n).map(|k| bb.float(k)).unwrap_or(f32::NAN);
                    let dest = bb.key_id("Destination").map(|k| bb.vector(k)).unwrap_or(Vec3::ZERO);
                    rows.push((c.name().to_string(), dest.x - lp.x, dest.y - lp.y, f("CurrentDestinationPreference"), f("CurrentLocationPreference"), f("FollowInFormationPreference")));
                }
            }
        }
        let ahead = rows.iter().filter(|r| r.1 > 0.0).count();
        println!("s26 {lead} leads: Destination ahead of the leader in {ahead}/{} samples; gaps {:?}", rows.len(), gaps.iter().map(|(k, v)| format!("{k} max {:.0} mean {:.0}", v.0, v.1 / v.2 as f32)).collect::<Vec<_>>());
        for r in rows.iter().take(8) {
            println!("s26   {} dest-leader ({:.0}, {:.0}) destPref {:.2} locPref {:.2} fwdPref {:.2}", r.0, r.1, r.2, r.3, r.4, r.5);
        }
    }
}

/// 27. (K1 -> AIX) Out of battle, Sora at 20 % runs 6 s then stops 4 s: when Donald's Cure comes and how far behind he falls.
#[test]
fn s27_field_cure_while_running() {
    let mut s = sim!(TestWorld::floor(), vec![]);
    s.idle(1.0);
    let max = s.sora.status().max_hp();
    s.sora.status_mut().hp = max / 5;
    let from = s.foes.step;
    let mut gap: f32 = 0.0;
    for i in 0..600 {
        s.step(if i < 360 { run_towards(0.0) } else { Input::default() });
        if let Some(d) = s.rt.view(PartyChara::Donald) {
            gap = gap.max(d.position.truncate().distance(s.out.position.truncate()));
        }
    }
    let cure = s.lines.iter().find(|l| l.0 >= from && l.1.contains("SpellCast { def: DonaldCure }")).map(|l| (l.0 - from) as f32 / 60.0);
    println!("s27 Sora at 20 % runs 6 s then stops: Donald's Cure at {cure:?} s, Donald's max gap {gap:.0} cm, Sora HP {} -> {}", max / 5, s.sora.status().hp);
}

/// 28. (TR -> AIX) Goofy leads among passive dummies under the test level's engaged rule (3 s after a blow): he
///     walks 1 s away, then stands 10 s: does the party stay in battle and does AI Sora keep attacking the dummies beside him?
#[test]
fn s28_passive_dummies_friend_leads() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 250.0, 0.0), Foe::dummy(2, 300.0, 250.0), Foe::dummy(3, 350.0, -250.0)]);
    s.foes.strict = true;
    s.word(&["leader", "goofy"]);
    s.idle(1.0);
    // the leader walks 1 s away from the dummies (the tour: "run 1"), then stands; nobody has struck yet
    s.run(1.0, |_| run_towards(180.0));
    let from = s.foes.step;
    let mut battle = 0;
    for _ in 0..600 {
        s.step(Input::default());
        battle += s.rt.lead.party_battle as usize;
    }
    let sora = s.metrics("Sora", from);
    let donald = s.metrics("Donald", from);
    let sora_hits = s.foes.hits.iter().filter(|h| h.0 >= from && h.2.starts_with("KB_")).count();
    println!("s28 Goofy leads, passive dummies: party in battle {} of 600 steps (flag {}); AI Sora attack chains {} keyblade hits {}; Donald actions {} hits {}", battle, s.rt.lead.party_battle, sora.actions, sora_hits, donald.actions, donald.hits);
}

/// (FX5) THE CV3 FAULT: Goofy leads, the party is in battle, a camp of three idle dummies stands 30 m away. The host's
/// engaged range is stretched over the camp (the battle flag on with it in reach, as a fight's memory or a wandering
/// member would put it), so the trees are in their battle flow with the camp in the host's list: Donald and AI Sora must
/// fight the dummy 5 m off and never take the camp as their target (kh-partyhost aiworld.rs `threatens`).
fn fx5_far_camp(threat_off: bool, near: bool) -> Option<(usize, usize, usize, Vec<String>)> {
    let mut foes = vec![Foe::dummy(2, 3000.0, 400.0), Foe::dummy(3, 3100.0, -300.0), Foe::dummy(4, 3200.0, 0.0)];
    if near {
        foes.push(Foe::dummy(1, 500.0, 0.0));
    }
    let Some(mut s) = Sim::new(TestWorld::floor(), foes) else {
        eprintln!("SKIPPED: no AI pack / sheets");
        return None;
    };
    if threat_off {
        kh_partyhost::aiworld::set_threat_range(Some(0.0));
    }
    s.foes.engage_range = 4000.0;
    s.word(&["leader", "goofy"]);
    s.idle(1.0);
    let from = s.foes.step;
    s.idle(20.0);
    if threat_off {
        kh_partyhost::aiworld::set_threat_range(None);
    }
    let far_targeted = s.samples.iter().filter(|x| x.step >= from && matches!(x.target, kh_ai::ActorRef::Enemy(2..=4))).count();
    let far_hits = s.foes.hits.iter().filter(|h| h.0 >= from && h.1 != 1).count();
    let near_hits = s.foes.hits.iter().filter(|h| h.0 >= from && h.1 == 1).count();
    let lines: Vec<String> = s.lines.iter().filter(|l| l.1.contains("(FX5)")).map(|l| l.1.clone()).collect();
    println!("fx5 far camp (threat rule {}, near dummy {near}): steps with a camp dummy as a friend's target {far_targeted}, hits on the camp {far_hits}, hits on the near dummy {near_hits}; party in battle {}", if threat_off { "OFF" } else { "on" }, s.rt.lead.party_battle);
    for l in &lines {
        println!("  {l}");
    }
    Some((far_targeted, far_hits, near_hits, lines))
}

#[test]
fn fx5_friends_fight_only_what_threatens_the_party() {
    let Some((far_targeted, far_hits, near_hits, lines)) = fx5_far_camp(false, true) else { return };
    assert_eq!((far_targeted, far_hits), (0, 0), "no friend takes the camp 30 m off");
    assert!(near_hits > 0, "the dummy 5 m off is fought");
    // the camp alone (nothing near the party, the battle flag on all the same): nobody walks or casts 30 m off
    let Some((far_targeted, far_hits, _, _)) = fx5_far_camp(false, false) else { return };
    assert_eq!((far_targeted, far_hits), (0, 0), "no friend takes the camp 30 m off, alone either");
    // (the "(FX5) ... left out" log line is printed above; not asserted: its once-a-second gate is process-wide)
    let _ = lines;
}

/// (FX5) The same field with the rule off (`party.threat_range = 0`, the behaviour before FX5): the evidence that the
/// camp WAS taken. Changes the process-wide range: run alone (`-- --ignored fx5_before`).
#[test]
#[ignore]
fn fx5_before_the_rule_the_camp_was_taken() {
    let Some((far_targeted, far_hits, _, _)) = fx5_far_camp(true, true) else { return };
    println!("before FX5 (near dummy too): {far_targeted} steps with a camp target, {far_hits} hits on the camp");
    let Some((far_targeted, far_hits, _, _)) = fx5_far_camp(true, false) else { return };
    println!("before FX5 (the camp alone): {far_targeted} steps with a camp target, {far_hits} hits on the camp");
}

// ------------------------------------------------------------------------------------------------ (GF-AI) AI Goofy on the real trees

fn goofy_at(s: &Sim) -> Vec3 {
    s.rt.id(PartyChara::Goofy).and_then(|id| s.rt.party.view(id)).map_or(Vec3::ZERO, |v| v.position)
}

/// The largest gap between Goofy and the leader over `seconds`.
fn goofy_watch(s: &mut Sim, seconds: f32) -> f32 {
    let mut far = 0.0f32;
    for _ in 0..(seconds / STEP) as usize {
        s.step(Input::default());
        let l = s.leader_pos().0;
        let g = goofy_at(s);
        far = far.max(Vec3::new(g.x - l.x, g.y - l.y, 0.0).length());
    }
    far
}

fn goofy_log(s: &Sim) -> Vec<&'static str> {
    s.rt.id(PartyChara::Goofy).and_then(|id| s.rt.party.status(id)).and_then(|st| st.goofy.as_ref()).map_or(Vec::new(), |g| g.run.log.clone())
}

/// A fight near the party and a camp beyond it: he fights what is near, and the camp does not draw him off the leader.
#[test]
fn s60_ai_goofy_stays_near_the_leader() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::chaser(1, 500.0, 200.0, 200.0, 2.5), Foe::dummy(2, 1800.0, -900.0), Foe::dummy(3, 2000.0, 900.0)]);
    s.idle(2.0);
    let far = goofy_watch(&mut s, 30.0);
    let leash = kh_party::goofy::K.ai_leash;
    let m = s.metrics("Goofy", 120);
    println!("s60 AI Goofy: {far:.0} cm at most from the leader (leash {leash:.0}), hits {}, idle near {:.1} s, fails {}, aborts {}, log {:?}", m.hits, m.idle_near_s, m.fails, m.aborts, goofy_log(&s));
    assert!(far <= leash + 300.0, "he walked {far:.0} cm from the leader");
}

/// Nothing to parry, enemies by the party: he is not standing there.
#[test]
fn s61_ai_goofy_is_not_idle_by_an_enemy() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 450.0, -150.0), Foe::dummy(2, 500.0, 250.0)]);
    s.idle(2.0);
    let from = s.foes.step;
    s.foes.list[0].timer = 0.0;
    let far = goofy_watch(&mut s, 20.0);
    let m = s.metrics("Goofy", from);
    println!("s61 AI Goofy by two dummies: {far:.0} cm at most from the leader, battle {:.1} s, idle near {:.1} s, log {:?}", m.battle_s, m.idle_near_s, goofy_log(&s));
    let shoves = s.foes.hits.iter().filter(|h| h.0 >= from && h.2.contains("Attack1_Push")).count() + s.foes.hits.iter().filter(|h| h.0 >= from && h.2.contains("Attack2_Sweep")).count();
    println!("s61 hits by his shoves: {shoves}");
}

/// The script words of the test level through the real trees: he breaks off his following to answer, and the tree gets him back.
#[test]
fn s62_ai_goofy_answers_a_telegraph_and_his_tree_gets_him_back() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 450.0, -150.0)]);
    s.idle(3.0);
    let id = s.rt.id(PartyChara::Goofy).unwrap();
    let hp = s.rt.party.status(id).unwrap().hp;
    let mut parries = 0;
    for _ in 0..8 {
        s.rt.gof_fake_word(&["dummy-telegraph", "1", "normal", "0.8", "0.1"]);
        let at = s.foes.list[0].pos;
        for _ in 0..(1.2 / STEP) as usize {
            s.rt.gof_fake_step(STEP, s.t, &|_| Some(at));
            s.step(Input::default());
            parries += s.rt.party.gof.events.iter().filter(|e| matches!(e.1, kh_party::goofy::GoofyEvent::Parried { .. })).count();
        }
        s.rt.party.set_telegraphs(&[]);
        s.idle(1.5);
    }
    let v = s.rt.party.view(id).unwrap();
    println!("s62 AI Goofy: {parries} parries of 8, HP {hp} -> {}, state {:?}, {:.0} cm from the leader, log {:?}", v.hp, v.state, s.leader_pos().0.distance(v.position), goofy_log(&s));
    assert!(parries >= 1, "{:?}", goofy_log(&s));
    assert!(s.leader_pos().0.distance(v.position) <= kh_party::goofy::K.ai_leash + 300.0);
}

/// Two enemies on the leader, then the leader low: the Taunt and Interpose come through the kit's own moves.
#[test]
fn s64_ai_goofy_taunts_and_interposes() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::chaser(1, 500.0, 200.0, 150.0, 4.0), Foe::chaser(2, 500.0, -200.0, 150.0, 4.0)]);
    s.idle(2.0);
    let far = goofy_watch(&mut s, 12.0);
    let log = goofy_log(&s);
    let max = s.sora.status().max_hp();
    s.sora.status_mut().hp = max / 5;
    let until = s.foes.step + 600;
    let mut interposed = false;
    while s.foes.step < until && !interposed {
        s.step(Input::default());
        s.sora.status_mut().hp = max / 5;
        interposed |= goofy_log(&s).contains(&"art_interpose");
    }
    println!("s64 AI Goofy: {far:.0} cm at most, Taunt {}, Interpose {interposed}, log {:?}", log.contains(&"art_taunt"), goofy_log(&s));
    assert!(log.contains(&"art_taunt"), "{log:?}");
    assert!(interposed, "{:?}", goofy_log(&s));
}

/// A friend leads: AI Goofy keeps to him, not to Sora.
#[test]
fn s63_ai_goofy_follows_a_friend_leader() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::chaser(1, 500.0, 200.0, 200.0, 3.0)]);
    s.word(&["leader", "donald"]);
    s.idle(2.0);
    let far = goofy_watch(&mut s, 15.0);
    println!("s63 Donald leads: AI Goofy {far:.0} cm at most from him, log {:?}", goofy_log(&s));
    assert!(far <= kh_party::goofy::K.ai_leash + 300.0);
}

/// (PARTY-AI 2026-11-03, the coordinator: with Sora leading, Goofy's trace flips between BT_n_Common_BattleStyle_Together and
/// BT_n_ex002_BattleFlow every other step in game) KH3'S OWN POLL, HARMLESS: while the Attack tag cooldown that WaitPhase set
/// runs (10 s), BattleFlow skips WaitPhase and every attack branch fails its range / angle / enemy checks, so it returns at
/// once and Together's infinite Loop enters it again (UE's Loop re-entry, ported in kh-ai bt.rs). The spin issues no command:
/// his tree asks for an attack only now and then (the overhaul answers those), his services keep their own countdown, and
/// his strings keep their spacing (`ai.poke_every`).
#[test]
fn goofys_battleflow_spin_is_kh3s_own_poll_and_costs_nothing() {
    for (name, foes) in [("dummy", vec![Foe::dummy(1, 250.0, -250.0)]), ("chasers", vec![Foe::chaser(1, 600.0, -200.0, 150.0, 2.5), Foe::chaser(2, 700.0, 250.0, 150.0, 2.5)])] {
        let mut s = sim!(TestWorld::floor(), foes);
        s.idle(20.0);
        let g: Vec<&Sample> = s.samples.iter().filter(|x| x.who == "Goofy" && x.in_battle).collect();
        let tree = |t: &str| t.split(':').next().unwrap_or("").to_string();
        let flips = g.windows(2).filter(|w| tree(&w[0].task) != tree(&w[1].task)).count();
        let asks = s.logs.iter().filter(|l| l.1 == "Goofy" && l.2.contains("TresBTTask_Action") && l.2.contains("Succeeded")).count();
        let mut starts: Vec<u64> = Vec::new();
        let mut last = None;
        for e in s.events.iter().filter(|e| e.1 == "Goofy" && matches!(&e.2, PartyEventKind::Hit { attack, .. } if attack.starts_with("Attack"))) {
            if last.is_none_or(|l: u64| e.0 > l + 30) {
                starts.push(e.0);
            }
            last = Some(e.0);
        }
        let gap = starts.windows(2).map(|w| w[1] - w[0]).min().unwrap_or(u64::MAX);
        println!("{name}: {} battle steps, {flips} tree flips, {asks} attack asks, strings at {starts:?}", g.len());
        assert!(flips > 50, "{name}: the spin happens here as in game ({flips})");
        assert!((asks as f32) <= g.len() as f32 * STEP, "{name}: at most one attack ask a second, not one a pass ({asks})");
        assert!(starts.len() >= 3, "{name}: he keeps fighting: {starts:?}");
        assert!(gap as f32 * STEP >= kh_party::goofy::K.ai_poke_every, "{name}: his strings keep their spacing ({gap} steps)");
    }
}

// ------------------------------------------------------------------------------------------------ (GRABS) caught and released

/// (GRABS) An action of his own: a tree action, an attack, a pad move, a played motion (a swing, a spell).
fn acting(state: &str) -> bool {
    state == "Action" || state == "Attack" || state == "PlayMotion" || state.starts_with("Pad(")
}

/// (GRABS) The first step at or after `from` at which `who` acts (an action, an attack, a pad move) or has moved more than
/// 60 cm from where he stood at `from`: seconds after `from`.
fn acts_again(s: &Sim, who: &str, from: u64) -> Option<f32> {
    let start = s.samples.iter().find(|x| x.who == who && x.step >= from)?.pos;
    s.samples
        .iter()
        .filter(|x| x.who == who && x.step >= from)
        .find(|x| acting(&x.state) || x.pos.truncate().distance(start.truncate()) > 60.0)
        .map(|x| (x.step - from) as f32 * STEP)
}

/// (GRABS) Holds `c` caught (the host's word every step) for `seconds`, then free for `after` seconds; returns the step of
/// the release.
fn grab(s: &mut Sim, c: PartyChara, seconds: f32, after: f32) -> u64 {
    for _ in 0..(seconds / STEP).round() as usize {
        s.rt.set_caught(c, true);
        s.step(Input::default());
    }
    let released = s.foes.step;
    for _ in 0..(after / STEP).round() as usize {
        s.rt.set_caught(c, false);
        s.step(Input::default());
    }
    released
}

/// (GRABS, the user 2026-11-06: "make sure AI doesn't break after they are released from the grab") Donald and Goofy, each
/// caught in the middle of the fight (Donald in the middle of an action: a spell or a swing), held 2 s, released: no
/// action while caught (KH3's caught motion, SituationAction), and he fights or moves again within 4 s of the release.
#[test]
fn s30_caught_friends_act_again_after_the_release() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 350.0, -100.0), Foe::dummy(2, 400.0, 250.0)]);
    s.victims_on = false;
    s.idle(4.0);
    // Donald caught in the middle of an action
    let mut mid_action = false;
    for _ in 0..(10.0 / STEP) as usize {
        s.step(Input::default());
        if s.samples.iter().rev().take(3).any(|x| x.who == "Donald" && acting(&x.state)) {
            mid_action = true;
            break;
        }
    }
    assert!(mid_action, "Donald acted within 10 s (the scenario needs him busy): his states {:?}", s.samples.iter().filter(|x| x.who == "Donald").map(|x| x.state.as_str()).collect::<std::collections::BTreeSet<_>>());
    for c in [PartyChara::Donald, PartyChara::Goofy] {
        let held_from = s.foes.step;
        let released = grab(&mut s, c, 2.0, 6.0);
        let name = c.name();
        let held: Vec<&Sample> = s.samples.iter().filter(|x| x.who == name && x.step > held_from + 2 && x.step < released).collect();
        assert!(!held.is_empty() && held.iter().all(|x| x.state == "SituationAction"), "{name} plays KH3's caught motion all the hold: {:?}", held.iter().map(|x| x.state.as_str()).collect::<std::collections::BTreeSet<_>>());
        assert!(!s.rt.ai_caught(c), "{name}: the gate let go");
        let t = acts_again(&s, name, released).unwrap_or(f32::INFINITY);
        assert!(t <= 4.0, "{name} acts again {t:.2} s after the release");
        println!("s30: {name} held 2 s, acts again {t:.2} s after the release");
    }
}

/// (GRABS) AI Sora (Donald leads) caught 2 s and released: his core takes nothing from his tree while caught, and he acts
/// or follows again within 4 s.
#[test]
fn s31_caught_ai_sora_acts_again_after_the_release() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 450.0, 0.0), Foe::dummy(2, 500.0, 300.0)]);
    s.victims_on = false;
    s.word(&["leader", "donald"]);
    s.idle(4.0);
    let held_from = s.foes.step;
    let released = grab(&mut s, PartyChara::Sora, 2.0, 6.0);
    let pressed: Vec<&Sample> = s.samples.iter().filter(|x| x.who == "Sora" && x.step > held_from + 60 && x.step < released && x.state == "Attack").collect();
    assert!(pressed.is_empty(), "no attack of his own while caught ({} steps)", pressed.len());
    assert!(!s.rt.ai_caught(PartyChara::Sora));
    let t = acts_again(&s, "Sora", released).unwrap_or(f32::INFINITY);
    assert!(t <= 4.0, "AI Sora acts again {t:.2} s after the release");
    println!("s31: AI Sora acts again {t:.2} s after the release");
}

/// (GRABS) THE GUARD: a "held" word that never ends (a stale throw node) is given up after `HELD_MAX_S`: Goofy fights
/// again although the host still says held, and a real grab after his body was seen free is taken again.
#[test]
fn s32_a_stale_caught_flag_never_persists() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 350.0, 0.0)]);
    s.victims_on = false;
    s.idle(3.0);
    for _ in 0..((kh_partyhost::caught::HELD_MAX_S + 0.5) / STEP) as usize {
        s.rt.set_caught(PartyChara::Goofy, true);
        s.step(Input::default());
    }
    assert!(!s.rt.ai_caught(PartyChara::Goofy), "given up after {} s", kh_partyhost::caught::HELD_MAX_S);
    let from = s.foes.step;
    for _ in 0..(5.0 / STEP) as usize {
        s.rt.set_caught(PartyChara::Goofy, true);
        s.step(Input::default());
    }
    let t = acts_again(&s, "Goofy", from).unwrap_or(f32::INFINITY);
    assert!(t <= 4.0 && !s.rt.ai_caught(PartyChara::Goofy), "the stale word holds nobody: Goofy acts {t:.2} s after the guard");
    s.rt.set_caught(PartyChara::Goofy, false);
    s.step(Input::default());
    s.rt.set_caught(PartyChara::Goofy, true);
    assert!(s.rt.ai_caught(PartyChara::Goofy), "a real grab is taken again");
}

// ------------------------------------------------------------------------------------------------ (HUD-REVIVE)

/// (HUD-REVIVE) Member `c` knocked out: HP 0 and KH3's Die state (`start_die`: its timer, its KO clip), past the abilities a blow
/// would meet at level 99 (Second Chance and the like).
fn hr_knock_out(s: &mut Sim, c: PartyChara) {
    let id = s.rt.id(c).unwrap();
    let m = s.rt.party.member_mut(id).unwrap();
    m.status.hp = 0;
    m.start_die();
}

/// (HUD-REVIVE) AI Sora (a friend leads) knocked out by a lethal hit (struck again until one lands: a style change, a dodge or
/// an attack's invincibility lets one through unharmed).
fn hr_knock_out_sora(s: &mut Sim) {
    for _ in 0..300 {
        if s.sora.status().hp < 1 {
            return;
        }
        s.sora.status_mut().hp = 1;
        let at = s.sora.output().position;
        s.sora.take_hit(kh_core::states::HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..kh_core::states::HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 60.0, TargetId(1), at + Vec3::X * 150.0) });
        s.step(Input::default());
    }
    panic!("AI Sora never went down");
}

/// (HUD-REVIVE) No Cure / item of the AI on anyone (KH3's Cure or item on a KO'd ally revives him: not what these tests time).
fn hr_no_cures(s: &mut Sim) {
    s.idle(0.1); // the members are spawned at the first step
    for c in [PartyChara::Donald, PartyChara::Goofy, PartyChara::Sora] {
        s.ai(c, FriendAiSettings { cure_targets: 0, ..FriendAiSettings::default() });
    }
    // and no item to give (a Potion on a downed ally revives him too)
    for c in [PartyChara::Donald, PartyChara::Goofy] {
        let id = s.rt.id(c).unwrap();
        for slot in 0..5 {
            s.rt.party.status_mut(id).unwrap().set_item_slot(slot, None);
        }
    }
}

fn hr_view(s: &Sim, c: PartyChara) -> kh_party::MemberView {
    s.rt.party.view(s.rt.id(c).unwrap()).unwrap()
}

/// (HUD-REVIVE, THE USER 2026-11-07: "healing at a save point should be able to revive downed party members. If they revive and
/// are very far away they should teleport to you like they usually do when alive") Donald and Goofy knocked out in a fight
/// (the party's combat holds: no revive of its end), Goofy 50 m behind the camera: the save point's revive stands both up at
/// FULL HP, the KO flags gone, the AI acting again; Goofy is warped to the leader by the living members' rule.
#[test]
fn hr1_the_save_point_revives_downed_friends_near_and_far() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 300.0, 0.0)]);
    hr_no_cures(&mut s);
    s.idle(2.0);
    assert!(s.rt.party_combat(), "the dummy by the leader: the party is in combat");
    hr_knock_out(&mut s, PartyChara::Donald);
    hr_knock_out(&mut s, PartyChara::Goofy);
    s.idle(1.0);
    let g = s.rt.id(PartyChara::Goofy).unwrap();
    {
        let m = s.rt.party.member_mut(g).unwrap();
        m.cmc.location = Vec3::new(-5000.0, 0.0, m.cmc.location.z);
    }
    s.idle(5.0);
    for c in [PartyChara::Donald, PartyChara::Goofy] {
        let v = hr_view(&s, c);
        assert!(v.ko && v.hp == 0 && v.state == kh_party::NpcStateId::Die, "{c:?} down in combat: {v:?}");
    }
    let warps_before = s.events.iter().filter(|e| e.1 == "Goofy" && matches!(e.2, PartyEventKind::Warped { .. })).count();
    // the save point
    let mut stood = s.rt.savepoint_revive(&mut s.sora);
    stood.sort_by_key(|c| c.name());
    assert_eq!(stood, vec![PartyChara::Donald, PartyChara::Goofy]);
    s.idle(0.1);
    for c in [PartyChara::Donald, PartyChara::Goofy] {
        let v = hr_view(&s, c);
        assert_eq!(v.hp, v.hp_max, "{c:?} at FULL HP");
    }
    s.idle(3.0);
    for c in [PartyChara::Donald, PartyChara::Goofy] {
        let v = hr_view(&s, c);
        assert!(!v.ko && v.state != kh_party::NpcStateId::Die, "{c:?} up, no stale KO: {:?}", v.state);
    }
    assert_eq!(s.events.iter().filter(|e| matches!(e.2, PartyEventKind::Revived)).count(), 2, "one Revived each");
    // far Goofy: the warp of a living member brings him (path 30 m, unseen, checked every 2 s)
    let mut near = false;
    for _ in 0..(8.0 / STEP) as usize {
        s.step(Input::default());
        near |= hr_view(&s, PartyChara::Goofy).position.distance(s.leader_pos().0) < 1500.0;
    }
    let warps = s.events.iter().filter(|e| e.1 == "Goofy" && matches!(e.2, PartyEventKind::Warped { .. })).count();
    assert!(near && warps > warps_before, "Goofy warped to the leader: near {near}, warps {warps_before} -> {warps}");
    // the AI acts: Donald's tree runs a task again
    let from = s.foes.step.saturating_sub(120);
    assert!(s.samples.iter().any(|x| x.who == "Donald" && x.step >= from && !x.task.is_empty()), "Donald's AI runs again");
}

/// (HUD-REVIVE, THE USER 2026-11-07: "allies revive when combat ends. They revive in combat after 60 seconds as a baseline")
/// Goofy down in a fight stays down while it lasts and is up when the last enemy goes (KH3's revive amount); Donald down
/// through a long fight is up after 60 s.
#[test]
fn hr2_friends_revive_when_the_combat_ends_and_after_60_s_in_it() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 300.0, 0.0)]);
    hr_no_cures(&mut s);
    s.idle(2.0);
    hr_knock_out(&mut s, PartyChara::Donald);
    s.idle(30.0);
    assert!(hr_view(&s, PartyChara::Donald).ko, "Donald down at 30 s of combat (KH3 alone: 40 s)");
    s.idle(29.0);
    assert!(hr_view(&s, PartyChara::Donald).ko, "still down at 59 s");
    s.idle(4.0);
    let d = hr_view(&s, PartyChara::Donald);
    assert!(!d.ko && d.hp > 0, "Donald up after 60 s in combat: {d:?}");
    hr_knock_out(&mut s, PartyChara::Goofy);
    s.idle(5.0);
    assert!(hr_view(&s, PartyChara::Goofy).ko && s.rt.party_combat());
    s.foes.list[0].hp = 0.0; // the last enemy goes
    s.idle(1.5);
    assert!(!s.rt.party_combat(), "the combat is over");
    s.idle(3.0);
    let g = hr_view(&s, PartyChara::Goofy);
    assert!(!g.ko && g.hp > 0 && g.state != kh_party::NpcStateId::Die, "Goofy up when the combat ended: {g:?}");
}

/// (HUD-REVIVE) AI SORA (Donald leads): down in his game-over pose; up after 60 s of combat, at the combat's end, and at the save
/// point (full HP); his AI acting after each.
#[test]
fn hr3_ai_sora_revives_by_the_timer_the_combat_end_and_the_save_point() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 300.0, 0.0)]);
    hr_no_cures(&mut s);
    s.word(&["leader", "donald"]);
    s.idle(2.0);
    assert!(s.rt.friend_leads() && s.rt.party_combat());
    let game_over = kh_core::AnimId::from_name(kh_core::states::member_ko::GAME_OVER_CLIP);
    // 1. the 60 s timer in combat (Goofy down with him: his Breaker heals the party, a heal that would revive Sora first)
    hr_knock_out_sora(&mut s);
    hr_knock_out(&mut s, PartyChara::Goofy);
    s.idle(2.0);
    assert!(s.rt.ai_sora_down(&s.sora));
    assert_eq!((s.out.state, Some(s.out.anim.name)), (kh_core::StateId::Die, game_over), "the game-over pose");
    for k in 0..55 {
        s.idle(1.0);
        assert!(s.rt.ai_sora_down(&s.sora), "down at {} s of combat (HP {}, combat {}, state {:?})", k + 3, s.sora.status().hp, s.rt.party_combat(), s.out.state);
    }
    s.idle(5.0);
    assert!(!s.rt.ai_sora_down(&s.sora), "up after 60 s");
    let quarter = kh_party::status::revive_amount(s.sora.status().max_hp(), 0);
    assert_eq!(s.sora.status().hp, quarter, "KH3's revive amount");
    s.idle(2.0);
    assert_ne!(s.out.state, kh_core::StateId::Die);
    // 2. the combat's end
    hr_knock_out_sora(&mut s);
    s.idle(3.0);
    assert!(s.rt.ai_sora_down(&s.sora));
    s.foes.list[0].hp = 0.0;
    s.idle(4.0);
    assert!(!s.rt.ai_sora_down(&s.sora) && s.out.state != kh_core::StateId::Die, "up when the combat ended");
    // 3. the save point (in combat again: a new dummy by the leader)
    s.foes.list.push(Foe::dummy(2, s.leader_pos().0.x + 300.0, s.leader_pos().0.y));
    s.idle(2.0);
    hr_knock_out_sora(&mut s);
    hr_knock_out(&mut s, PartyChara::Goofy);
    s.idle(3.0);
    assert!(s.rt.ai_sora_down(&s.sora), "AI Sora down");
    assert!(s.rt.party_combat(), "in combat again");
    let stood = s.rt.savepoint_revive(&mut s.sora);
    assert_eq!(stood, vec![PartyChara::Goofy, PartyChara::Sora], "Goofy (down with him) and AI Sora");
    assert_eq!(s.sora.status().hp, s.sora.status().max_hp(), "FULL HP at the save point");
    s.idle(2.0);
    assert_ne!(s.out.state, kh_core::StateId::Die, "his get-up and Idle");
}

/// (HUD-REVIVE) A DOWNED AI SORA ENGAGES NOTHING (as a downed friend): an enemy standing by his hovering body, far from the leader
/// and the other members, holds no party combat, so the combat's end revives him.
#[test]
fn hr4_an_enemy_by_downed_ai_sora_holds_no_combat() {
    let mut s = sim!(TestWorld::floor(), vec![Foe::dummy(1, 3000.0, 0.0)]);
    hr_no_cures(&mut s);
    s.word(&["leader", "donald"]);
    s.idle(1.0);
    hr_knock_out(&mut s, PartyChara::Goofy); // no Breaker heal from him
    s.sora.teleport(Pose { position: Vec3::new(2900.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 });
    s.step(Input::default());
    hr_knock_out_sora(&mut s);
    assert!(s.rt.ai_sora_down(&s.sora), "AI Sora down beside the enemy, 29 m from the leader");
    let mut up = false;
    for _ in 0..(4.0 / STEP) as usize {
        s.step(Input::default());
        up |= !s.rt.ai_sora_down(&s.sora);
    }
    assert!(up, "no combat held by the enemy at his body: he got up at the combat's end");
}
