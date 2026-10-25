//! (ER party S1) THE OFFLINE ER-SHAPED HARNESS: the party core (glue.rs) driven exactly as sora.rs drives it (frame ->
//! route -> Sora's step with `SoraTargets` -> step), over synthetic geometry (kh-core's `TestWorld`: planes and boxes,
//! the game's casts' semantics), with the real AI pack (build/sora_pack/party/ai) and sheets. Sora is the leader and
//! walks by his own stick. What it shows: the friends follow on open ground, go round a wall by the trail without a
//! warp, a friend shut in takes KH3's warp, a relocation / a cutscene park and reseat them, they jump a ladder after the
//! leader. No game, no window.

use glam::{Vec2, Vec3};
use kh_core::test_world::TestWorld;
use kh_core::{Input, NullHost, Output, Pose, Sora, TargetId, SORA_PARAMS};
use kh_host::party::PartyChara;
use kh_party::NpcStateId;

use super::*;

const STEP: f32 = 1.0 / 60.0;

/// No enemies: the friends' targets and the AI's enemy list are empty.
#[derive(Default)]
struct Fakes(kh_core::host::NullTargets, Option<Vec3> /* (PORT30) one enemy here (id 1), else none */);

impl kh_core::Targets for Fakes {
    fn enumerate(&self, out: &mut Vec<kh_core::host::TargetInfo>) {
        self.0.enumerate(out);
        out.extend(self.info(TargetId(1)));
    }
    fn info(&self, id: TargetId) -> Option<kh_core::host::TargetInfo> {
        use kh_core::host::TargetFlags;
        match self.1 {
            Some(at) if id.0 == 1 => Some(kh_core::host::TargetInfo { id, position: at, center: at + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }),
            _ => self.0.info(id),
        }
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<kh_core::host::HurtShape>) {
        self.0.hurt_shapes(id, out)
    }
    fn apply_hit(&mut self, hit: &kh_core::HitRecord) -> kh_core::host::HitResult {
        self.0.apply_hit(hit)
    }
}

impl PartyTargets for Fakes {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

fn project(p: &str) -> std::path::PathBuf {
    // (LDI) the workspace's own, else the first ancestor's that has it (a work copy under build/sora_tmp has no pack)
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    if root.join(p).exists() {
        return root.join(p);
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().map(|a| a.join(p)).find(|c| c.exists()).unwrap_or_else(|| root.join(p))
}

fn rows(path: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|j| j["rows"].as_array().cloned()).unwrap_or_default()
}

struct Run {
    core: PartyCore,
    sora: Sora,
    out: Output,
    world: TestWorld,
    fakes: Fakes,
    null: NullHost,
    t: f64,
    /// Steps in which a friend had a waypoint (the trail steered him).
    steered: u32,
}

impl Run {
    fn new(world: TestWorld) -> Option<Run> {
        let pack = kh_ai::AiPack::load_dir(&project("build/sora_pack/party/ai")).ok()?;
        let sheets = project("sheets/port");
        let config = kh_partyhost::RuntimeConfig { wanted: true, test: kh_partyhost::PartyTest::from_rows(&rows(&sheets.join("party-test.json"))), weapons: kh_partyhost::PartyWeapon::from_rows(&rows(&sheets.join("party-weapons.json"))), ai_pack: Some(std::sync::Arc::new(pack)), ai_pack_source: None };
        let rules = std::fs::read_to_string(sheets.join("party-er-host.json")).ok().and_then(|t| HostRules::from_sheet(&t).ok()).map_or(HostRules::DEFAULT, |r| r.0);
        let mut core = PartyCore::new(config, rules);
        // `KH_PARTY_LOG=1`: the party lines on stderr (cargo test -- --nocapture)
        core.sink = Some(|t| if std::env::var_os("KH_PARTY_LOG").is_some() { eprintln!("{t}") });
        let sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 });
        let out = sora.output();
        Some(Run { core, sora, out, world, fakes: Fakes::default(), null: NullHost::default(), t: 0.0, steered: 0 })
    }

    fn frame(&mut self, f: impl FnOnce(&mut FrameIn)) {
        let mut fin = FrameIn { on: true, fresh: false, relocated: false, hidden: None, body_action: false, trace: true, body: Some(BodyObs { feet: self.out.position, yaw: self.out.yaw }), pose: true, dt: STEP, now: self.t };
        f(&mut fin);
        self.core.frame(fin);
    }

    /// One frame of one step; `towards`: Sora runs toward that KH3 yaw (stick full ahead, the camera along it).
    fn step(&mut self, towards: Option<f32>) {
        let input = Input { move_stick: if towards.is_some() { Vec2::new(0.0, 1.0) } else { Vec2::ZERO }, camera_yaw: towards.unwrap_or(self.out.yaw), ..Input::default() };
        self.step_input(input);
    }

    /// (PORT30) one frame of one step with this input (its pad: presses).
    fn step_input(&mut self, input: Input) {
        let input = self.core.route(&mut self.sora, &input, STEP, &self.world, &self.fakes, self.t);
        self.out = {
            let mut targets = kh_partyhost::SoraTargets { targets: &mut self.fakes, runtime: &mut self.core.rt };
            let mut host = kh_core::Host { world: &self.world, targets: &mut targets, audio: &mut self.null.audio, effects: &mut self.null.effects, ui: &mut self.null.ui };
            self.sora.step_hosted(STEP, &input, &mut host)
        };
        // the drawn camera behind Sora (the AI's camera)
        let f = forward(self.out.yaw);
        self.core.camera.position = self.out.position - f * 450.0 + Vec3::Z * 220.0;
        self.core.camera.rotation = kh_camera::Rot { pitch: -15.0, yaw: self.out.yaw, roll: 0.0 };
        self.core.step(&mut self.sora, &self.out, STEP, TargetId::NONE, &self.world, &kh_anim::NoIkWorld, &mut self.fakes, &mut self.null.audio, &mut self.null.effects, None, self.t);
        self.t += STEP as f64;
        if self.core.rt.members.iter().any(|(_, id)| self.core.rt.party.member(*id).is_some_and(|m| m.steer.is_some())) {
            self.steered += 1;
        }
    }

    fn frames(&mut self, n: usize, towards: Option<f32>) {
        for _ in 0..n {
            self.frame(|_| {});
            self.step(towards);
        }
    }

    /// Runs toward `to` (KH3 cm) until within 80 cm or `max` frames.
    fn run_to(&mut self, to: Vec3, max: usize) {
        for _ in 0..max {
            let d = Vec3::new(to.x - self.out.position.x, to.y - self.out.position.y, 0.0);
            if d.length() < 80.0 {
                break;
            }
            self.frame(|_| {});
            self.step(Some(d.y.atan2(d.x).to_degrees()));
        }
    }

    /// Puts a friend at these feet (KH3 cm) as he stands (a test's own move: no reseat).
    fn put(&mut self, c: PartyChara, feet: Vec3) {
        let id = self.core.rt.id(c).unwrap();
        let m = self.core.rt.party.member_mut(id).unwrap();
        let lift = (m.params.floor_dist_target - (m.params.capsule_center_z - m.params.capsule_half_height)).max(0.0);
        m.cmc.location = feet + Vec3::Z * lift;
        m.cmc.stop_movement();
    }

    fn friend(&self, c: PartyChara) -> kh_party::MemberView {
        self.core.rt.view(c).expect("the friend is in the party")
    }

    fn said(&self, what: &str) -> usize {
        self.core.lines.iter().filter(|l| l.contains(what)).count()
    }

    fn log(&self) -> String {
        self.core.lines.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

macro_rules! harness {
    ($world:expr) => {
        match Run::new($world) {
            Some(r) => r,
            None => {
                eprintln!("SKIPPED: no AI pack at build/sora_pack/party/ai (node tools/party_ai_pack.js)");
                return;
            }
        }
    };
}

#[test]
fn the_friends_follow_on_open_ground() {
    let mut r = harness!(TestWorld::floor());
    r.frames(60, None);
    assert_eq!(r.core.rt.members.len(), 2, "{}", r.log());
    let start = [r.friend(PartyChara::Donald).position, r.friend(PartyChara::Goofy).position];
    r.run_to(Vec3::new(1500.0, 0.0, 0.0), 600);
    r.frames(240, None);
    for (k, c) in PartyChara::FRIENDS.into_iter().enumerate() {
        let v = r.friend(c);
        assert!(v.position.distance(r.out.position) < 700.0, "{} is {:.0} cm from Sora\n{}", c.name(), v.position.distance(r.out.position), r.log());
        assert!(v.position.distance(start[k]) > 600.0, "{} did not follow", c.name());
    }
    assert_eq!(r.said("WARPS"), 0, "{}", r.log());
    assert!(r.core.trail.len() > 20 && r.core.feed.wants.len() == 2 && r.core.visible());
}

#[test]
fn the_friends_go_round_a_wall_by_the_trail() {
    // a wall across x = 500 from y = -600 to y = 300: the leader walks round its north end
    let wall = (Vec3::new(470.0, -600.0, 0.0), Vec3::new(530.0, 300.0, 400.0));
    let mut r = harness!(TestWorld::floor().with_box(wall.0, wall.1));
    r.frames(60, None);
    r.run_to(Vec3::new(0.0, 650.0, 0.0), 400);
    r.run_to(Vec3::new(900.0, 650.0, 0.0), 400);
    r.run_to(Vec3::new(900.0, -200.0, 0.0), 400);
    r.frames(60, None);
    // the friends are put back behind the wall (west of it, where the walk began): the straight line to the leader is
    // blocked, only the trail leads round
    r.steered = 0;
    r.put(PartyChara::Donald, Vec3::new(100.0, 0.0, 0.0));
    r.put(PartyChara::Goofy, Vec3::new(100.0, -200.0, 0.0));
    let mut inside = 0;
    for _ in 0..600 {
        r.frame(|_| {});
        r.step(None);
        for c in PartyChara::FRIENDS {
            let p = r.friend(c).position;
            if p.x > wall.0.x && p.x < wall.1.x && p.y > wall.0.y && p.y < wall.1.y {
                inside += 1;
            }
        }
    }
    for c in PartyChara::FRIENDS {
        let v = r.friend(c);
        assert!(v.position.x > 560.0 && v.position.distance(r.out.position) < 800.0, "{} at ({:.0}, {:.0}): not round the wall\n{}", c.name(), v.position.x, v.position.y, r.log());
    }
    assert_eq!(inside, 0, "nobody walks through the wall");
    assert!(r.steered > 0, "the trail gave waypoints");
    assert_eq!(r.said("WARPS"), 0, "round the wall without a warp:\n{}", r.log());
}

#[test]
fn a_friend_shut_in_takes_khs_warp() {
    let mut r = harness!(TestWorld::floor());
    r.frames(30, None);
    // a closed box round Goofy (walls 20 thick, 3 m high)
    let g = r.friend(PartyChara::Goofy).position;
    for (lo, hi) in [((-150.0, -150.0), (150.0, -130.0)), ((-150.0, 130.0), (150.0, 150.0)), ((-150.0, -150.0), (-130.0, 150.0)), ((130.0, -150.0), (150.0, 150.0))] {
        r.world.boxes.push((Vec3::new(g.x + lo.0, g.y + lo.1, 0.0), Vec3::new(g.x + hi.0, g.y + hi.1, 300.0)));
    }
    r.run_to(Vec3::new(1600.0, 0.0, 0.0), 600);
    let mut warped = false;
    for _ in 0..900 {
        r.frame(|_| {});
        r.step(None);
        warped |= r.friend(PartyChara::Goofy).state == NpcStateId::Warp;
    }
    let v = r.friend(PartyChara::Goofy);
    assert!(warped || r.said("Goofy WARPS") > 0, "{}", r.log());
    assert!(v.position.distance(r.out.position) < 800.0, "Goofy is out of the box beside Sora: {:.0} cm\n{}", v.position.distance(r.out.position), r.log());
}

#[test]
fn a_relocation_and_a_cutscene_park_and_reseat_the_friends() {
    let mut r = harness!(TestWorld::floor());
    r.frames(60, None);
    // the game puts its player 50 m away (a warp): the friends are beside him at the next step, the trail is cut
    let to = Vec3::new(5000.0, 3000.0, Sora::origin_height_on_floor(&SORA_PARAMS));
    r.sora.teleport(Pose { position: to, yaw: 90.0 });
    r.frame(|f| f.relocated = true);
    r.step(None);
    for c in PartyChara::FRIENDS {
        assert!(r.friend(c).position.distance(to) < 300.0, "{} beside him after the warp\n{}", c.name(), r.log());
    }
    assert!(r.said("RESEAT") >= 2);
    // a cutscene: not stepped, not drawn, the bodies parked unhittable at the leader
    let before = r.friend(PartyChara::Donald).position;
    for _ in 0..60 {
        r.frame(|f| f.hidden = Some("a real-time cutscene"));
        r.step(None);
    }
    assert!(!r.core.visible() && r.friend(PartyChara::Donald).position == before);
    assert!(r.core.feed.wants.iter().all(|w| !w.hittable && w.invulnerable && w.feet == r.out.position), "{:?}", r.core.feed.wants);
    // after it, beside the leader again
    r.sora.teleport(Pose { position: to + Vec3::new(800.0, 0.0, 0.0), yaw: 0.0 });
    r.frame(|_| {});
    r.step(None);
    assert!(r.core.visible() && r.friend(PartyChara::Donald).position.distance(r.out.position) < 300.0, "{}", r.log());
    // party.er = 0: frozen, not drawn, never removed; 1: back beside him
    r.frame(|f| f.on = false);
    r.step(None);
    assert!(!r.core.visible() && r.core.rt.members.len() == 2 && r.core.feed.wants.is_empty());
    r.frame(|_| {});
    r.step(None);
    assert!(r.core.visible() && r.core.feed.wants.len() == 2);
}

/// (LADDER2) THE PARTY JUMPS THE LADDER RIGHT AFTER THE LEADER (the user: "the party leader and his party must end up where
/// the elden ring character would normally finish climbing the ladder facing the same way as the tarnished"): the leader
/// leaps onto the landing on the ledge (sora.rs: here put there), the party core gives Donald and Goofy their ladder jump
/// from where they stand: each lands beside the landing on the ledge, facing its heading, one take-off sound each, no warp.
#[test]
fn the_party_jumps_the_ladder_after_the_leader() {
    // the floor at 0 up to x = 600, a ledge 400 high beyond it (a solid block)
    let mut r = harness!(TestWorld::floor().with_box(Vec3::new(600.0, -2000.0, -10.0), Vec3::new(4000.0, 2000.0, 400.0)));
    r.frames(60, None);
    r.run_to(Vec3::new(520.0, 0.0, 0.0), 300);
    r.frames(30, None);
    let (to, facing) = (Vec3::new(660.0, 0.0, 400.0), 0.0);
    r.core.ladder_jump(to, facing);
    r.sora.teleport(Pose { position: to + Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS), yaw: facing });
    let mut sounds = 0;
    for _ in 0..240 {
        r.frame(|_| {});
        r.step(None);
        sounds += r.core.feed.ladder_sounds.len();
    }
    assert!(r.said("THE LADDER JUMP") == 1, "{}", r.log());
    for c in PartyChara::FRIENDS {
        let v = r.friend(c);
        assert!(v.position.z > 390.0 && v.position.x > 600.0, "{} is on the ledge: ({:.0}, {:.0}, {:.0})
{}", c.name(), v.position.x, v.position.y, v.position.z, r.log());
        assert!(v.position.distance(to) > 40.0, "{} beside the leader, not on him", c.name());
        assert_ne!(v.state, NpcStateId::Ladder);
    }
    assert!(r.friend(PartyChara::Donald).position.distance(r.friend(PartyChara::Goofy).position) > 50.0, "not on each other");
    assert_eq!(r.said("WARPS"), 0, "{}", r.log());
    assert_eq!(sounds, 2, "a take-off sound for each friend
{}", r.log());
}

#[test]
fn the_host_sheet_reads_every_row() {
    let text = std::fs::read_to_string(project("sheets/port/party-er-host.json")).expect("sheets/port/party-er-host.json");
    let (rules, problems) = HostRules::from_sheet(&text).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(rules, HostRules::DEFAULT, "the sheet's values are the built-in defaults");
}

#[test]
fn a_fog_wall_the_leader_walks_through_does_not_leave_the_friends_behind() {
    // a closed wall across x = 500 (no end to walk round): the game's own action takes the body (and the leader) through
    let mut r = harness!(TestWorld::floor().with_box(Vec3::new(480.0, -5000.0, 0.0), Vec3::new(520.0, 5000.0, 600.0)));
    r.frames(60, None);
    r.run_to(Vec3::new(400.0, 0.0, 0.0), 300);
    for k in 0..90 {
        let at = Vec3::new(400.0 + 300.0 * k as f32 / 90.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        r.sora.teleport(Pose { position: at, yaw: 0.0 });
        r.frame(|f| f.body_action = true);
        r.step(None);
    }
    r.run_to(Vec3::new(1000.0, 0.0, 0.0), 200);
    r.frames(400, None);
    for c in PartyChara::FRIENDS {
        let v = r.friend(c);
        assert!(v.position.x > 520.0 && v.position.distance(r.out.position) < 700.0, "{} through the wall: ({:.0}, {:.0})\n{}", c.name(), v.position.x, v.position.y, r.log());
    }
    assert!(r.said("cut off by the body's action") > 0, "{}", r.log());
}


#[test]
fn with_donald_leading_sora_and_goofy_follow_him() {
    let mut r = harness!(TestWorld::floor());
    r.frames(30, None);
    r.core.asked_formation = Some(kh_host::formation::Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Donald).unwrap());
    r.frames(30, None);
    assert_eq!(r.core.rt.leader_chara(), PartyChara::Donald, "{}", r.log());
    let start = r.core.rt.leader_view_of(&r.out, r.sora.status()).position;
    // the pad (Sora's stick) drives Donald now: toward +X for 4 s
    r.frames(240, Some(0.0));
    r.frames(240, None);
    let lead = r.core.rt.leader_view_of(&r.out, r.sora.status()).position;
    assert!(lead.x - start.x > 500.0, "Donald ran: {start} -> {lead}\n{}", r.log());
    assert!(r.out.position.distance(lead) < 900.0, "AI Sora follows Donald: {:.0} cm\n{}", r.out.position.distance(lead), r.log());
    assert!(r.friend(PartyChara::Goofy).position.distance(lead) < 900.0, "Goofy follows Donald\n{}", r.log());
    // the bodies wanted: Goofy and AI Sora (not the leader)
    let wants: Vec<PartyChara> = r.core.feed.wants.iter().map(|w| w.chara).collect();
    assert!(wants.contains(&PartyChara::Sora) && wants.contains(&PartyChara::Goofy) && !wants.contains(&PartyChara::Donald), "{wants:?}");
}

#[test]
fn a_loaded_saves_unlocks_keep_its_formation() {
    let mut r = harness!(TestWorld::floor());
    r.frames(10, None);
    // the save had Sora and Donald unlocked, Donald slotted out: restoring the unlocks does not slot him in
    let saved = kh_host::formation::Formation::new(&[PartyChara::Sora], PartyChara::Sora).unwrap();
    r.core.asked_formation = Some(saved.clone());
    r.frames(5, None);
    r.core.restore_unlocked(&[PartyChara::Sora, PartyChara::Donald], r.t);
    r.frames(5, None);
    assert_eq!(r.core.unlocked(), Some(&[PartyChara::Sora, PartyChara::Donald][..]));
    assert!(r.core.asked_formation.is_none() && r.core.rt.lead.current_formation() == saved, "{}", r.log());
    // a NEW unlock (S8's set_unlocked) does join him
    r.core.set_unlocked(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], r.t);
    r.frames(5, None);
    assert!(r.core.rt.lead.current_formation().is_active(PartyChara::Goofy), "{}", r.log());
}

/// (ERT4) The user's case in ERT4's run: a new game as Goofy (unlocked [Goofy]) whose file has no party record yet (KH3's
/// default formation Sora, Donald, Goofy, Sora leading) is loaded again (a KH3 game over -> Continue): Goofy alone leads.
#[test]
fn ert4_a_loaded_formation_never_holds_a_locked_character() {
    let mut r = harness!(TestWorld::floor());
    r.frames(10, None);
    let default = kh_host::formation::Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Sora).unwrap();
    r.core.asked_formation = Some(default);
    r.frames(5, None);
    r.core.restore_unlocked(&[PartyChara::Goofy], r.t);
    r.frames(5, None);
    let f = r.core.rt.lead.current_formation();
    assert_eq!(f.active(), vec![PartyChara::Goofy], "{}", r.log());
    assert_eq!(f.leader(), PartyChara::Goofy, "{}", r.log());
    assert_eq!(r.core.rt.leader_chara(), PartyChara::Goofy, "{}", r.log());
    // Goofy and Donald unlocked, the default formation: Sora drops out, Goofy (the first unlocked) leads
    r.core.asked_formation = Some(kh_host::formation::Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Sora).unwrap());
    r.frames(5, None);
    r.core.restore_unlocked(&[PartyChara::Goofy, PartyChara::Donald], r.t);
    r.frames(5, None);
    let f = r.core.rt.lead.current_formation();
    assert_eq!(f.active(), vec![PartyChara::Donald, PartyChara::Goofy], "{}", r.log());
    assert_eq!(f.leader(), PartyChara::Goofy, "{}", r.log());
}

// ------------------------------------------------------------------------------------------------ (party K1)
// THE FOLLOW-SPEED RUNS in the ER-shaped harness: the leader runs (Sora sprints after 1 s) 30 s straight or on a curve,
// or Donald leads; per AI member the gap to the leader (max / mean), the warps (the host's "WARPS" lines and jumps over
// 200 cm in a step), the steps the trail steered. KH3's FollowPlayer speed and the K1 locomotion are kh-party's
// (crates/kh-party/src/locomotion.rs); S1's trail steering runs unchanged. `cargo test -p kh-er --lib --release k1 --
// --nocapture` prints the table.

/// (name, gap max, gap mean, warps)
fn k1_run(lead: Option<PartyChara>, curve: f32, secs: f32) -> Option<(Vec<(&'static str, f32, f32, u32)>, u32, String)> {
    let mut r = Run::new(TestWorld::floor())?;
    r.frames(30, None);
    if let Some(c) = lead {
        r.core.asked_formation = Some(kh_host::formation::Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], c).unwrap());
    }
    r.frames(30, None);
    let names: Vec<(&'static str, Option<PartyChara>)> = {
        let mut v: Vec<(&'static str, Option<PartyChara>)> = PartyChara::FRIENDS.into_iter().filter(|c| Some(*c) != lead).map(|c| (c.name(), Some(c))).collect();
        if lead.is_some() {
            v.push(("AI Sora", None));
        }
        v
    };
    let pos = |r: &Run, c: Option<PartyChara>| match c {
        Some(c) => r.friend(c).position,
        None => r.out.position,
    };
    let mut prev: Vec<Vec3> = names.iter().map(|n| pos(&r, n.1)).collect();
    let mut stats: Vec<(&'static str, f32, f32, u32)> = names.iter().map(|n| (n.0, 0.0, 0.0, 0)).collect();
    let warps0 = r.said("WARPS");
    let steps = (secs / STEP) as usize;
    for i in 0..steps {
        let heading = curve * i as f32 * STEP;
        r.frame(|_| {});
        r.step(Some(heading));
        let lead_pos = r.core.rt.leader_view_of(&r.out, r.sora.status()).position;
        for (k, n) in names.iter().enumerate() {
            let p = pos(&r, n.1);
            let gap = Vec3::new(p.x - lead_pos.x, p.y - lead_pos.y, 0.0).length();
            stats[k].1 = stats[k].1.max(gap);
            if std::env::var_os("K1_TRACE").is_some() && i % 30 == 0 && n.1.is_none() { eprintln!("t={:.1} AI Sora gap {gap:.0} state {:?} mv {:?} lead {:?} pos {:?}", i as f32 * STEP, r.out.state, r.out.movement_state, lead_pos, p); }
            stats[k].2 += gap / steps as f32;
            if Vec3::new(p.x - prev[k].x, p.y - prev[k].y, 0.0).length() > 200.0 {
                stats[k].3 += 1;
            }
            prev[k] = p;
        }
    }
    let host_warps = (r.said("WARPS") - warps0) as u32;
    Some((stats, host_warps, r.log()))
}

#[test]
fn k1_the_friends_keep_up_on_open_ground() {
    for (title, lead, curve) in [("Sora straight (sprint)", None, 0.0), ("Sora curve 20 deg/s", None, 20.0), ("Donald leads straight", Some(PartyChara::Donald), 0.0), ("Donald leads curve", Some(PartyChara::Donald), 20.0)] {
        let Some((stats, host_warps, log)) = k1_run(lead, curve, 30.0) else {
            eprintln!("SKIPPED: no AI pack at build/sora_pack/party/ai (node tools/party_ai_pack.js)");
            return;
        };
        for s in &stats {
            eprintln!("K1 ER {title:<24} {:<8} gap max {:>6.0} mean {:>5.0} warps {}", s.0, s.1, s.2, s.3);
        }
        assert_eq!(host_warps, 0, "{title}: the host warped a friend on open ground\n{log}");
        for s in &stats {
            assert_eq!(s.3, 0, "{title}: {} jumped (a warp) on open ground\n{log}", s.0);
            assert!(s.2 < 600.0, "{title}: {} trails {:.0} cm on average", s.0, s.2);
        }
    }
}

// (SM) the friends' smoothness by rendered frames (uneven frame times): sm_tests.rs
#[path = "sm_tests.rs"]
mod sm;

// (FOL2) the follow metrics harness (stop-and-wait, corridors, walk-run): fol2_tests.rs
#[path = "fol2_tests.rs"]
mod fol2;

const LOADING: &str = "the loading screen (placed when it lifts)";

/// (LDI, the user 2026-10-22: "Donald and Goofy spawn in when Sora is finally placed properly") A LOAD: the new host's
/// first frames are under the loading screen (sora.rs holds the party: hidden "the loading screen"); the game moves its
/// player once more in the dark. Held, the friends are neither stepped nor drawn (no fall through a world still streaming
/// in, no drawing at the old host's place); when the hold ends they are put beside his FINAL place in the same step that
/// makes them drawn again (never drawn before it), and their next step is a small one.
#[test]
fn a_load_holds_the_friends_and_places_them_beside_his_final_place() {
    let mut r = harness!(TestWorld::floor());
    r.frames(60, None);
    let before: Vec<Vec3> = PartyChara::FRIENDS.iter().map(|c| r.friend(*c).position).collect();
    let to = Vec3::new(-7000.0, 4000.0, Sora::origin_height_on_floor(&SORA_PARAMS));
    for k in 0..240 {
        if k == 30 {
            r.sora.teleport(Pose { position: to, yaw: 45.0 }); // the game puts its player elsewhere in the dark
        }
        r.frame(|f| {
            f.fresh = k == 0;
            f.relocated = k == 30;
            f.hidden = Some(LOADING);
        });
        assert!(!r.core.visible(), "frame {k}: never drawn under the loading screen");
        r.step(None);
        assert!(!r.core.visible(), "frame {k}: never drawn under the loading screen");
    }
    for (c, b) in PartyChara::FRIENDS.iter().zip(&before) {
        assert_eq!(r.friend(*c).position, *b, "{}: not stepped while held", c.name());
    }
    assert!(r.core.feed.wants.iter().all(|w| !w.hittable && w.invulnerable), "the bodies parked unhittable meanwhile");
    // the screen lifts: until the step that places them they are not drawn ...
    r.frame(|_| {});
    assert!(!r.core.visible() && r.core.placing());
    r.step(None);
    // ... and from it on they are, beside his final place, on the floor
    assert!(r.core.visible() && !r.core.placing(), "{}", r.log());
    let at: Vec<Vec3> = PartyChara::FRIENDS.iter().map(|c| r.friend(*c).position).collect();
    for (c, p) in PartyChara::FRIENDS.iter().zip(&at) {
        let d = Vec3::new(p.x - to.x, p.y - to.y, 0.0).length();
        assert!((50.0..300.0).contains(&d), "{} beside his final place: {d:.0} cm\n{}", c.name(), r.log());
    }
    assert!(r.said("RESEAT") >= 2 && r.said("shown again after the loading screen") == 1, "{}", r.log());
    r.frames(1, None);
    for (c, p) in PartyChara::FRIENDS.iter().zip(&at) {
        assert!(r.friend(*c).position.distance(*p) < 25.0, "{}: a large first step ({:.0} cm)", c.name(), r.friend(*c).position.distance(*p));
    }
}

/// (LDI) A FRIEND LEADING (Donald): S2 seats him on the game's body only after the party's step, so his own view is still
/// the place before the load at the reseat: the others (Goofy, AI Sora) are put beside THE BODY's place (the frame's
/// `ladder.feet`), not beside where he stood before the load.
#[test]
fn a_load_with_a_friend_leading_places_the_others_beside_the_body() {
    let mut r = harness!(TestWorld::floor());
    r.frames(30, None);
    r.core.asked_formation = Some(kh_host::formation::Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Donald).unwrap());
    r.frames(30, None);
    assert!(r.core.rt.leader.is_some(), "Donald leads\n{}", r.log());
    let body = Vec3::new(9000.0, -6000.0, 0.0);
    for k in 0..60 {
        r.frame(|f| {
            f.fresh = k == 0;
            f.hidden = Some(LOADING);
            f.body = Some(BodyObs { feet: body, yaw: -60.0 });
        });
        r.step(None);
    }
    r.frame(|f| f.body = Some(BodyObs { feet: body, yaw: -60.0 }));
    r.step(None);
    let goofy = r.friend(PartyChara::Goofy).position;
    assert!(Vec3::new(goofy.x - body.x, goofy.y - body.y, 0.0).length() < 300.0, "Goofy beside the body: ({:.0}, {:.0})\n{}", goofy.x, goofy.y, r.log());
    let sora = r.sora.output().position;
    assert!(Vec3::new(sora.x - body.x, sora.y - body.y, 0.0).length() < 400.0, "AI Sora beside the body: ({:.0}, {:.0})\n{}", sora.x, sora.y, r.log());
    assert!(r.said("the friend leader's seat: the game's body") >= 1, "{}", r.log());
}

