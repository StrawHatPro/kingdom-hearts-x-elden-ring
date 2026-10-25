//! (SM) THE FRIENDS' SMOOTHNESS IN THE TEST LEVEL'S SHAPE: K1's harness (`PartyRuntime` driven as kh-app's player.rs
//! drives it: route -> Sora's step -> step -> pose -> after_step, the straight nav) with the friends' posers from
//! build/sora_pack/party, run by RENDERED FRAMES with uneven frame times (16 - 33 ms, a 50 ms hitch now and then), the
//! fixed 60 Hz steps by an accumulator. The test level draws every friend at the frame's alpha (kh-app party_draw.rs
//! `Sim::alpha`): rule `interp`; `live` = the latest step (kh-er's default). Metrics: sm_metrics.rs.
//! `cargo test -p kh-partyhost --lib --release sm_ -- --nocapture --test-threads=1`.

use kh_core::host::TargetId;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Vec2, Vec3};
use kh_host::party::PartyChara;

use crate::k1_tests::Harness;
use crate::*;

#[path = "sm_metrics.rs"]
mod metrics;
use metrics::{DrawnSample, SimSample};

const DT: f32 = 1.0 / 60.0;

struct Rng(u64);
impl Rng {
    fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn frame_dt(rng: &mut Rng) -> f32 {
    if rng.unit() < 1.0 / 60.0 {
        0.050
    } else {
        0.016 + 0.017 * rng.unit()
    }
}

fn project(p: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

fn load_posers(poser: &mut PartyPoser) -> bool {
    let pack = project("build/sora_pack/party");
    let read = |p: &std::path::Path| std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok());
    let index = read(&pack.join("party.json")).unwrap_or(serde_json::Value::Null);
    for id in [poser::DONALD, poser::GOOFY] {
        let dir = pack.join(poser::npc_of(id).unwrap());
        let Some(manifest) = read(&dir.join("manifest.json")) else { return false };
        let entry = index["members"].as_array().into_iter().flatten().find(|m| m["member"].as_u64() == Some(id as u64)).cloned().unwrap_or(serde_json::Value::Null);
        let Ok(pack) = kh_anim::Pack::load(&dir) else { return false };
        let Ok(animator) = kh_anim::Animator::new(pack, kh_anim::Settings::default()) else { return false };
        let Ok(m) = PosedMember::new(id, animator, &manifest, &entry) else { return false };
        poser.members.insert(id, m);
    }
    true
}

struct Ik<'a>(&'a TestWorld);
impl kh_anim::IkWorld for Ik<'_> {
    fn sweep_sphere(&self, start: Vec3, end: Vec3, radius: f32) -> Option<kh_anim::SweepHit> {
        use kh_core::World;
        let r = radius.max(0.1);
        let h = self.0.sweep_capsule(start, end, r, r)?;
        Some(kh_anim::SweepHit { time: h.time, impact_point: h.position, impact_normal: h.impact_normal, start_penetrating: h.start_penetrating })
    }
}

struct NoEnemies;
impl kh_core::Targets for NoEnemies {
    fn enumerate(&self, _out: &mut Vec<kh_core::host::TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<kh_core::host::TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<kh_core::host::HurtShape>) {}
    fn apply_hit(&mut self, _hit: &kh_core::host::HitRecord) -> kh_core::host::HitResult {
        kh_core::host::HitResult::default()
    }
}
impl PartyTargets for NoEnemies {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

/// One step of the test level's order (k1_tests `Harness::step` + the pose), the stick toward `stick` (yaw, magnitude).
fn step(h: &mut Harness, poser: &mut PartyPoser, stick: Option<(f32, f32)>) {
    let (lp, ly) = h.leader_pos();
    let cam_yaw = stick.map_or(ly, |s| s.0);
    let (s, c) = cam_yaw.to_radians().sin_cos();
    let camera = kh_camera::CameraView { position: lp - Vec3::new(c, s, 0.0) * 450.0 + Vec3::Z * 220.0, rotation: kh_camera::Rot { pitch: -15.0, yaw: cam_yaw, roll: 0.0 }, rotation_target: kh_camera::Rot { pitch: -15.0, yaw: cam_yaw, roll: 0.0 }, look_point: lp, distance: 450.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
    let input = Input { move_stick: stick.map_or(Vec2::ZERO, |s| Vec2::new(0.0, s.1)), camera_yaw: cam_yaw, ..Input::default() };
    let input = h.rt.route(RouteIn { sora: &mut h.sora, input: &input, dt: DT, targets: &NoEnemies, camera: &camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: h.t });
    let mut targets = NoEnemies;
    let mut audio = kh_core::host::audio::NullAudio;
    let mut effects = kh_core::host::effects::NullEffects;
    let mut ui = kh_core::host::NullUi;
    h.out = {
        let mut st = SoraTargets { targets: &mut targets, runtime: &mut h.rt };
        let mut host = kh_core::Host { world: &h.world, targets: &mut st, audio: &mut audio, effects: &mut effects, ui: &mut ui };
        h.sora.step_hosted(DT, &input, &mut host)
    };
    let _ = h.rt.step(StepIn { dt: DT, sora: &mut h.sora, out: &h.out, lock_on: TargetId::NONE, camera: &camera, world: &h.world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: h.t });
    let _ = h.rt.pose(poser, &Ik(&h.world), DT);
    let _ = h.rt.after_step(&NoEnemies, DT, h.t);
    h.t += DT as f64;
}

fn stairs() -> TestWorld {
    let mut w = TestWorld::floor();
    for k in 0..14 {
        let x0 = 600.0 + 2500.0 * k as f32;
        for s in 0..6 {
            let lo = x0 + 60.0 * s as f32;
            let hi = x0 + 60.0 * 12.0 + 300.0 - 60.0 * s as f32;
            w = w.with_box(Vec3::new(lo, -3000.0, 0.0), Vec3::new(hi, 3000.0, 15.0 * (s + 1) as f32));
        }
    }
    w
}

type Drive = fn(f32) -> Option<(f32, f32)>;

pub fn scenarios() -> Vec<(&'static str, bool, Option<PartyChara>, Drive)> {
    vec![
        ("sprint straight", false, None, |_| Some((0.0, 1.0))),
        ("sprint curve 20/s", false, None, |t| Some((20.0 * t, 1.0))),
        ("sprint S-curves", false, None, |t| Some((55.0 * (std::f32::consts::TAU * t / 6.0).sin(), 1.0))),
        ("walk curves (0.4)", false, None, |t| Some((40.0 * (std::f32::consts::TAU * t / 8.0).sin(), 0.4))),
        ("stop / start", false, None, |t| if t % 4.5 < 3.0 { Some((90.0 * (t / 4.5).floor(), 1.0)) } else { None }),
        ("stairs", true, None, |_| Some((0.0, 1.0))),
        ("Donald leads S-curves", false, Some(PartyChara::Donald), |t| Some((55.0 * (std::f32::consts::TAU * t / 6.0).sin(), 1.0))),
        ("Goofy leads stop/start", false, Some(PartyChara::Goofy), |t| if t % 4.5 < 3.0 { Some((90.0 * (t / 4.5).floor(), 1.0)) } else { None }),
    ]
}

fn yaw_of_c2w(t: &kh_anim::Trs) -> f32 {
    let f = t.rotation * Vec3::Y;
    f.y.atan2(f.x).to_degrees()
}

fn wrap180(a: f32) -> f32 {
    (a + 540.0).rem_euclid(360.0) - 180.0
}

/// One scenario: rows (member, rule, report).
pub fn run(name: &str, steps_world: bool, lead: Option<PartyChara>, drive: Drive, secs: f32) -> Option<Vec<(String, String, metrics::Report)>> {
    let mut h = Harness::new()?;
    if steps_world {
        h.world = stairs();
    }
    let mut poser = PartyPoser::default();
    if !load_posers(&mut poser) {
        return None;
    }
    for _ in 0..30 {
        step(&mut h, &mut poser, None);
    }
    if let Some(c) = lead {
        h.rt.lead_word(&["leader", c.name()]);
    }
    for _ in 0..30 {
        step(&mut h, &mut poser, None);
    }
    let mut who: Vec<(&'static str, Option<u8>)> = PartyChara::FRIENDS.into_iter().filter(|c| Some(*c) != lead).map(|c| (c.name(), Some(poser::draw_id(c)))).collect();
    if lead.is_some() {
        who.push(("AI Sora", None));
    }
    let mut live: Vec<Vec<DrawnSample>> = vec![Vec::new(); who.len()];
    let mut interp: Vec<Vec<DrawnSample>> = vec![Vec::new(); who.len()];
    let mut sim: Vec<Vec<SimSample>> = vec![Vec::new(); who.len()];
    let mut rng = Rng(0x2545_F491_4F6C_DD1D ^ name.len() as u64);
    let (mut acc, mut t_disp, mut t_sim) = (0.0f32, 0.0f64, 0.0f32);
    let mut out_before = h.out.clone();
    let feet = |p: &PartyPoser, id: u8| {
        let sk = p.member(id).unwrap().animator.skeleton();
        (sk.index_of("L_ashi1").or(sk.index_of("L_ashi")).unwrap_or(0), sk.index_of("R_ashi1").or(sk.index_of("R_ashi")).unwrap_or(0))
    };
    while t_disp < secs as f64 {
        let dt = frame_dt(&mut rng);
        t_disp += dt as f64;
        acc = (acc + dt).min(0.25);
        while acc >= DT {
            acc -= DT;
            out_before = h.out.clone();
            step(&mut h, &mut poser, drive(t_sim));
            t_sim += DT;
            let (lp, _) = h.leader_pos();
            for (k, w) in who.iter().enumerate() {
                let (pos, yaw, clip_turn) = match w.1 {
                    Some(_) => {
                        let v = h.rt.view(PartyChara::FRIENDS.into_iter().find(|c| c.name() == w.0).unwrap()).unwrap();
                        (v.position, v.yaw, v.anim.clip.starts_with("RunStart") || v.anim.clip.starts_with("Turn"))
                    }
                    None => (h.out.position, h.out.yaw, format!("{:?}", h.out.movement_state).contains("Start")),
                };
                sim[k].push(SimSample { t: t_sim as f64, pos, yaw, leader: lp, clip_turn });
            }
        }
        let true_alpha = (acc / DT).clamp(0.0, 1.0);
        let leader_steps = match lead.map(poser::draw_id) {
            Some(id) => poser.member(id).unwrap().position,
            None => (out_before.position, h.out.position),
        };
        for (k, w) in who.iter().enumerate() {
            for (alpha, list) in [(1.0, &mut live[k]), (true_alpha, &mut interp[k])] {
                let leader = leader_steps.0.lerp(leader_steps.1, alpha);
                list.push(match w.1 {
                    Some(id) => {
                        let f = feet(&poser, id);
                        let p = poser.pose(id, alpha).unwrap();
                        let at = |i: usize| p.component.get(i).map_or(p.component_to_world.translation, |b| b.then(&p.component_to_world).translation);
                        DrawnSample { t: t_disp, pos: p.component_to_world.translation, yaw: yaw_of_c2w(&p.component_to_world), feet: Some([at(f.0), at(f.1)]), leader }
                    }
                    None => DrawnSample { t: t_disp, pos: out_before.position.lerp(h.out.position, alpha), yaw: out_before.yaw + wrap180(h.out.yaw - out_before.yaw) * alpha, feet: None, leader },
                });
            }
        }
    }
    let mut rows = Vec::new();
    for (k, w) in who.iter().enumerate() {
        rows.push((w.0.to_string(), "live".to_string(), metrics::report(&live[k], &sim[k], DT)));
        rows.push((w.0.to_string(), "interp".to_string(), metrics::report(&interp[k], &sim[k], DT)));
    }
    Some(rows)
}

/// The table (no assertions).
#[test]
fn sm_measure_test_level() {
    eprintln!("SM TL | scenario | member | rule | {}", metrics::HEADER);
    for (name, steps_world, lead, drive) in scenarios() {
        let Some(rows) = run(name, steps_world, lead, drive, 30.0) else {
            eprintln!("SKIPPED: no AI pack / party pack");
            return;
        };
        for (m, rule, rep) in &rows {
            eprintln!("SM TL | {name:<22} | {m:<7} | {rule:<6} | {}", metrics::line(rep));
        }
    }
}
