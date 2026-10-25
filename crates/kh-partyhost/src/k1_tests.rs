//! (party K1) THE FOLLOW-SPEED HARNESS, test-level shaped: `PartyRuntime` driven exactly as kh-app's player.rs drives it
//! (route -> Sora's `step_hosted` with `SoraTargets` -> step -> after_step), the straight nav, a flat floor, the real AI
//! pack (build/sora_pack/party/ai) and sheets. The leader runs / sprints 30 s in a straight line or on a curve; the
//! metrics per AI member: the gap to the leader (max / mean), warps, frames the run loop plays without matching movement,
//! the yaw rate of turns without a turn clip, the feet's slide in the loop. `cargo test -p kh-partyhost --release k1 --
//! --nocapture` prints the table. Skipped (passes) without the pack.

use kh_core::host::{HitRecord, HitResult, HurtShape, TargetId, TargetInfo};
use kh_core::test_world::TestWorld;
use kh_core::{Input, Sora, Vec2, Vec3};
use kh_host::party::PartyChara;

use crate::*;

const DT: f32 = 1.0 / 60.0;

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
}

impl PartyTargets for NoEnemies {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

fn project(p: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

fn rows(path: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|j| j["rows"].as_array().cloned()).unwrap_or_default()
}

/// The leader's path: straight along +X, or a constant-rate curve (deg/s of heading).
#[derive(Clone, Copy, Debug)]
pub enum Path {
    Straight,
    Curve(f32),
    /// Straight, then a full reversal every `n` seconds (the turn-around case).
    Zigzag(f32),
}

#[derive(Clone, Debug, Default)]
pub struct Metrics {
    pub name: String,
    pub gap_max: f32,
    pub gap_mean: f32,
    /// Seconds beyond the follow distance (`FOLLOW_DISTANCE`).
    pub beyond: f32,
    pub warps: u32,
    /// Frames the run / walk loop played with a speed under 10 % of the clip's speed.
    pub run_in_place: u32,
    /// Max yaw rate (deg/s) while a standing clip (Idle*) plays.
    pub idle_yaw_rate: f32,
    /// Frames with an idle-clip yaw rate over 30 deg/s.
    pub idle_yaw_frames: u32,
    /// Max |actual speed - the loop's stride speed at its play rate| / the stride speed, in the loop at speed.
    pub slide_max: f32,
    pub slide_mean: f32,
    pub top_speed: f32,
    /// The clips seen while moving.
    pub clips: Vec<&'static str>,
    /// Move -> Idle while the leader runs (over 300 cm/s): a stop and a new start at the formation distance.
    pub stops: u32,
    /// The gap over the frames he is in a locomotion state (not acting: the AI's own actions, a Cure, stand him still).
    pub loco_gap_max: f32,
}

/// The follow distance the gap is judged against: the formation's farthest slot (EQS grid 2000 / spacing 500 of
/// n_Common_FollowPrecedeQuery) is not a fixed ring; KH3's own speed rule holds the gap at about
/// m_RangeSamePlayerSpeed (300) behind the leader. 600 cm = that plus the formation's spread.
pub const FOLLOW_DISTANCE: f32 = 600.0;

pub struct Harness {
    pub rt: PartyRuntime,
    pub sora: Sora,
    pub out: kh_core::Output,
    pub world: TestWorld,
    pub t: f64,
}

impl Harness {
    pub fn new() -> Option<Harness> {
        let pack = kh_ai::AiPack::load_dir(&project("build/sora_pack/party/ai")).ok()?;
        let sheets = project("sheets/port");
        let rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&rows(&sheets.join("party-test.json"))), weapons: PartyWeapon::from_rows(&rows(&sheets.join("party-weapons.json"))), ai_pack: Some(std::sync::Arc::new(pack)), ai_pack_source: None });
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&kh_core::SORA_PARAMS)), yaw: 0.0 });
        let out = sora.output();
        Some(Harness { rt, sora, out, world: TestWorld::floor(), t: 0.0 })
    }

    pub fn leader_pos(&self) -> (Vec3, f32) {
        let v = self.rt.leader_view_of(&self.out, self.sora.status());
        (v.position, v.yaw)
    }

    /// One fixed step: the stick full toward `heading` (KH3 yaw), or released.
    pub fn step(&mut self, heading: Option<f32>) {
        let (lp, ly) = self.leader_pos();
        let cam_yaw = heading.unwrap_or(ly);
        let (s, c) = cam_yaw.to_radians().sin_cos();
        let camera = kh_camera::CameraView { position: lp - Vec3::new(c, s, 0.0) * 450.0 + Vec3::Z * 220.0, rotation: kh_camera::Rot { pitch: -15.0, yaw: cam_yaw, roll: 0.0 }, rotation_target: kh_camera::Rot { pitch: -15.0, yaw: cam_yaw, roll: 0.0 }, look_point: lp, distance: 450.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        let input = Input { move_stick: if heading.is_some() { Vec2::new(0.0, 1.0) } else { Vec2::ZERO }, camera_yaw: cam_yaw, ..Input::default() };
        let targets = NoEnemies;
        let input = self.rt.route(RouteIn { sora: &mut self.sora, input: &input, dt: DT, targets: &targets, camera: &camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: self.t });
        let mut targets = NoEnemies;
        let mut audio = kh_core::host::audio::NullAudio;
        let mut effects = kh_core::host::effects::NullEffects;
        let mut ui = kh_core::host::NullUi;
        self.out = {
            let mut st = SoraTargets { targets: &mut targets, runtime: &mut self.rt };
            let mut host = kh_core::Host { world: &self.world, targets: &mut st, audio: &mut audio, effects: &mut effects, ui: &mut ui };
            self.sora.step_hosted(DT, &input, &mut host)
        };
        let _ = self.rt.step(StepIn { dt: DT, sora: &mut self.sora, out: &self.out, lock_on: TargetId::NONE, camera: &camera, world: &self.world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: self.t });
        let _ = self.rt.after_step(&NoEnemies, DT, self.t);
        self.t += DT as f64;
    }
}

/// What the AI members are this step: (name, position, yaw, velocity, clip, anim time).
fn ai_bodies(h: &Harness) -> Vec<(&'static str, Vec3, f32, Vec3, &'static str, f32, Option<kh_party::pad::LocoPose>)> {
    let mut v = Vec::new();
    for (c, id) in h.rt.members.iter() {
        if h.rt.leader == Some(*id) {
            continue;
        }
        if let Some(m) = h.rt.party.view(*id) {
            v.push((c.name(), m.position, m.yaw, m.velocity, m.anim.clip, m.anim.time, h.rt.party.ai_loco(*id)));
        }
    }
    if h.rt.friend_leads() {
        v.push(("AI Sora", h.out.position, h.out.yaw, h.out.velocity, "", 0.0, None));
    }
    v
}

fn wrap(a: f32) -> f32 {
    let mut a = a % 360.0;
    if a > 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

/// The loop clips and their stride speed at play rate 1 (cm/s): the run 680 (m_MaxSpeed), the walk 140 (m_MinSpeed).
fn loop_speed(clip: &str) -> Option<f32> {
    match clip {
        "RunLoopN" | "RunLoopB" | "RunLoopW" => Some(680.0),
        "WalkLoopN" | "WalkLoopW" | "WalkLoopB" => Some(140.0),
        "WalkRunN" | "WalkRunW" | "WalkRunB" => Some(-1.0),
        _ => None,
    }
}

/// Runs `secs` of the leader's path after a 1 s settle and returns each AI member's metrics.
pub fn measure(leader: Option<PartyChara>, path: Path, secs: f32) -> Option<Vec<Metrics>> {
    let mut h = Harness::new()?;
    for _ in 0..30 {
        h.step(None);
    }
    if let Some(c) = leader {
        h.rt.lead_word(&["leader", c.name()]);
    }
    for _ in 0..30 {
        h.step(None);
    }
    let names: Vec<&'static str> = ai_bodies(&h).iter().map(|b| b.0).collect();
    let mut m: Vec<Metrics> = names.iter().map(|n| Metrics { name: n.to_string(), ..Metrics::default() }).collect();
    let mut prev: Vec<(Vec3, f32, &'static str, f32)> = ai_bodies(&h).iter().map(|b| (b.1, b.2, b.4, b.5)).collect();
    let mut prev_pose: Vec<Option<kh_party::pad::LocoPose>> = ai_bodies(&h).iter().map(|b| b.6).collect();
    let mut prev_state: Vec<&'static str> = vec![""; 8];
    let mut prev_lp = h.leader_pos().0;
    let mut slide_n = vec![0u32; 8];
    let mut gap_sum = vec![0.0f32; m.len()];
    let steps = (secs / DT) as usize;
    let mut heading;

    for i in 0..steps {
        let t = i as f32 * DT;
        let want = match path {
            Path::Straight => 0.0,
            Path::Curve(rate) => rate * t,
            Path::Zigzag(n) => if ((t / n) as i32) % 2 == 0 { 0.0 } else { 180.0 },
        };
        heading = want;
        h.step(Some(heading));
        let (lp, _) = h.leader_pos();
        let leader_speed = Vec3::new(lp.x - prev_lp.x, lp.y - prev_lp.y, 0.0).length() / DT;
        prev_lp = lp;
        let bodies = ai_bodies(&h);
        for (k, b) in bodies.iter().enumerate() {
            let Some(mm) = m.get_mut(k) else { continue };
            let gap = Vec3::new(b.1.x - lp.x, b.1.y - lp.y, 0.0).length();
            mm.gap_max = mm.gap_max.max(gap);
            let st = h.rt.members.iter().find(|(c, _)| c.name() == b.0).and_then(|(_, id)| h.rt.party.view(*id)).map_or("", |v| v.state_name);
            if b.0 == "AI Sora" || matches!(st, "TresNpcState_Run" | "TresNpcState_Idle" | "TresNpcState_Turn" | "TresNpcState_Warp") {
                mm.loco_gap_max = mm.loco_gap_max.max(gap);
            }
            if prev_state[k] == "TresNpcState_Run" && st == "TresNpcState_Idle" && leader_speed > 300.0 {
                mm.stops += 1;
            }
            prev_state[k] = st;
            gap_sum[k] += gap;
            if gap > FOLLOW_DISTANCE {
                mm.beyond += DT;
            }
            let (pp, py, pclip, ptime) = prev[k];
            let moved = Vec3::new(b.1.x - pp.x, b.1.y - pp.y, 0.0).length();
            if moved > 200.0 {
                mm.warps += 1;
            }
            let speed = moved / DT;
            mm.top_speed = mm.top_speed.max(speed);
            if !b.4.is_empty() {
                if speed > 50.0 && !mm.clips.contains(&b.4) {
                    mm.clips.push(b.4);
                }
                let yaw_rate = wrap(b.2 - py).abs() / DT;
                if b.4.starts_with("Idle") && pclip == b.4 {
                    mm.idle_yaw_rate = mm.idle_yaw_rate.max(yaw_rate);
                    if yaw_rate > 30.0 {
                        mm.idle_yaw_frames += 1;
                    }
                }
                // the drawn cycle's ground speed: the speed-matched pose's phase (kh-party locomotion), else the slot clip's clock
                let c = chara_named(b.0);
                let (wl, rl) = (kh_party::tables::clip(c, "WalkLoopN").length.max(1e-3), kh_party::tables::clip(c, "RunLoopN").length.max(1e-3));
                let drawn = match (b.6, prev_pose[k]) {
                    (Some(p1), Some(p0)) => {
                        let mut dp = (p1.run_time - p0.run_time) / rl;
                        if dp < 0.0 {
                            dp += 1.0;
                        }
                        let w = p1.run_weight;
                        let stride = 140.0 * wl * (1.0 - w) + 680.0 * rl * w;
                        Some((dp / DT * stride, 140.0 + (680.0 - 140.0) * w))
                    }
                    (Some(_), None) => None,
                    _ => loop_speed(b.4).filter(|_| pclip == b.4).map(|ls| {
                        let ls = if ls < 0.0 { 680.0 } else { ls };
                        let len = kh_party::tables::clip(c, b.4).length.max(1e-3);
                        let mut dtime = b.5 - ptime;
                        if dtime < 0.0 {
                            dtime += len;
                        }
                        (dtime / DT / len * ls * len, ls)
                    }),
                };
                if let Some((dv, nominal)) = drawn {
                    if dv >= 0.1 * nominal && speed < 0.1 * nominal {
                        mm.run_in_place += 1;
                    }
                    if speed > 100.0 && dv > 1.0 {
                        let sl = (speed - dv).abs() / dv;
                        mm.slide_max = mm.slide_max.max(sl);
                        mm.slide_mean += sl;
                        slide_n[k] += 1;
                    }
                }
            }
            if std::env::var_os("K1_TRACE").is_some() && (i % 6 == 0 || std::env::var_os("K1_TRACE_ALL").is_some()) {
                let st = h.rt.members.iter().find(|(c, _)| c.name() == b.0).and_then(|(_, id)| h.rt.party.view(*id)).map_or("-", |v| v.state_name);
                let cmd = h.rt.members.iter().find(|(c, _)| c.name() == b.0).and_then(|(_, id)| h.rt.party.view(*id)).map(|v| format!("{:?}", v.command)).unwrap_or_default();
                let lo = h.rt.members.iter().find(|(c, _)| c.name() == b.0).and_then(|(_, id)| h.rt.party.member(*id)).map(|m| format!("hd {:.0} sub {} dec {:.0} hold {:.2} side {:.2} blk {:.2} fs {:.0}", m.loco.heading, m.loco.sub, m.loco.decel_v, m.loco.hold, m.loco.side, m.loco.blocked, m.loco.follow_speed)).unwrap_or_default();
                eprintln!("t={t:5.2} {:<7} gap {gap:6.0} speed {speed:5.0} state {st:<10} clip {:<10} yaw {:6.1} cmd {cmd} {lo}", b.0, b.4, b.2);
            }
            prev[k] = (b.1, b.2, b.4, b.5);
            prev_pose[k] = b.6;
        }
    }
    let _ = heading;
    for (k, mm) in m.iter_mut().enumerate() {
        mm.gap_mean = gap_sum[k] / steps as f32;
        if slide_n[k] > 0 {
            mm.slide_mean /= slide_n[k] as f32;
        }
    }
    Some(m)
}

fn chara_named(n: &str) -> kh_party::Chara {
    if n == "Goofy" {
        kh_party::Chara::Goofy
    } else {
        kh_party::Chara::Donald
    }
}

pub fn print(title: &str, ms: &[Metrics]) {
    for m in ms {
        eprintln!("K1 {title:<28} {:<8} gap max {:>6.0} (moving {:>5.0}) mean {:>5.0} stops {:>2} beyond {:>4.1}s warps {:>2} runInPlace {:>4} idleYaw max {:>5.0}/s frames {:>4} slide max {:>4.2} top {:>5.0} clips {:?}", m.name, m.gap_max, m.loco_gap_max, m.gap_mean, m.stops, m.beyond, m.warps, m.run_in_place, m.idle_yaw_rate, m.idle_yaw_frames, m.slide_max, m.top_speed, m.clips);
    }
}

macro_rules! run {
    ($leader:expr, $path:expr, $title:expr) => {
        match measure($leader, $path, 30.0) {
            Some(m) => {
                print($title, &m);
                m
            }
            None => {
                eprintln!("SKIPPED: no AI pack at build/sora_pack/party/ai");
                return;
            }
        }
    };
}

/// The table for the state note (no assertions: `k1_follow_*` below assert).
#[test]
fn k1_measure_all() {
    let _ = run!(None, Path::Straight, "Sora straight (sprint)");
    let _ = run!(None, Path::Curve(20.0), "Sora curve 20 deg/s");
    let _ = run!(None, Path::Zigzag(5.0), "Sora reversal every 5 s");
    let _ = run!(Some(PartyChara::Donald), Path::Straight, "Donald leads straight");
    let _ = run!(Some(PartyChara::Donald), Path::Curve(20.0), "Donald leads curve");
    let _ = run!(Some(PartyChara::Goofy), Path::Straight, "Goofy leads straight");
    let _ = run!(Some(PartyChara::Goofy), Path::Curve(20.0), "Goofy leads curve");
}

/// The acceptance runs: on open ground no warps, the mean gap within the follow distance, no cycle without movement,
/// no yaw under the idle clip, the loop's stride on the ground.
#[test]
fn k1_follow_keeps_up_without_warps_or_sliding() {
    for (lead, path, title) in [(None, Path::Straight, "Sora straight"), (None, Path::Curve(20.0), "Sora curve"), (Some(PartyChara::Donald), Path::Straight, "Donald leads"), (Some(PartyChara::Goofy), Path::Curve(20.0), "Goofy leads curve")] {
        let ms = run!(lead, path, title);
        for m in &ms {
            assert_eq!(m.warps, 0, "{title}: {} warped", m.name);
            assert!(m.gap_mean < FOLLOW_DISTANCE, "{title}: {} mean gap {:.0}", m.name, m.gap_mean);
            assert_eq!(m.run_in_place, 0, "{title}: {} ran in place", m.name);
            assert_eq!(m.idle_yaw_frames, 0, "{title}: {} turned under the idle clip", m.name);
            assert!(m.slide_max < 0.05, "{title}: {} feet slide {:.2}", m.name, m.slide_max);
        }
    }
}
