//! (SM) THE FRIENDS' SMOOTHNESS IN THE ELDEN RING HOST: S1's harness (the party core driven in sora.rs's step order: frame
//! -> route -> Sora's step -> step, the friends' posers loaded from build/sora_pack/party, the trail nav, the cast budget)
//! run by RENDERED FRAMES at Elden Ring's frame rate with uneven frame times (16 - 33 ms, a 50 ms hitch now and then;
//! also a steady 60 fps), the fixed 60 Hz steps taken by sora.rs's accumulator. Every frame the friends' DRAWN transform
//! is read as partydraw.rs draws it:
//! - `live` = `poser.pose(id, alpha)` with sora.rs's alpha (`r.interp` off, the default: 1 = the latest step);
//! - `interp` = `poser.pose(id, true alpha)` with Sora and the camera at the same alpha (`r.interp = 1`).
//! Metrics: crates/kh-partyhost/src/sm_metrics.rs. `cargo test -p kh-er --lib --release sm_ -- --nocapture
//! --test-threads=1` prints the table; `SM_OUT=<dir>` writes one CSV per run (the frame strips' input).

use glam::{Vec2, Vec3};
use kh_core::test_world::TestWorld;
use kh_core::{Input, TargetId};
use kh_host::party::PartyChara;

use super::*;

#[path = "../../../../kh-partyhost/src/sm_metrics.rs"]
pub(super) mod metrics; // (FOL2) pub(super): fol2_tests.rs prints this table with the FOL2 switch
use metrics::{DrawnSample, SimSample};

/// The frame clock: Elden Ring's frame times.
#[derive(Clone, Copy, Debug)]
pub enum Clock {
    /// 16 - 33 ms uniform, 1 in 60 frames a 50 ms hitch.
    Jitter,
    /// 60 fps with +-0.6 ms of noise (vsync).
    Steady,
}

struct Rng(u64);
impl Rng {
    fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn frame_dt(c: Clock, rng: &mut Rng) -> f32 {
    match c {
        Clock::Jitter => {
            if rng.unit() < 1.0 / 60.0 {
                0.050
            } else {
                0.016 + 0.017 * rng.unit()
            }
        }
        Clock::Steady => 1.0 / 60.0 + (rng.unit() - 0.5) * 0.0012,
    }
}

/// The posers from the pack (party/mod.rs `create`'s way); false without the pack.
fn load_posers(core: &mut PartyCore) -> bool {
    let pack = project("build/sora_pack/party");
    let read = |p: &std::path::Path| std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok());
    let index = read(&pack.join("party.json")).unwrap_or(serde_json::Value::Null);
    for id in [kh_partyhost::poser::DONALD, kh_partyhost::poser::GOOFY] {
        let npc = kh_partyhost::poser::npc_of(id).unwrap();
        let dir = pack.join(npc);
        let Some(manifest) = read(&dir.join("manifest.json")) else { return false };
        let entry = index["members"].as_array().into_iter().flatten().find(|m| m["member"].as_u64() == Some(id as u64)).cloned().unwrap_or(serde_json::Value::Null);
        let Ok(pack) = kh_anim::Pack::load(&dir) else { return false };
        let Ok(animator) = kh_anim::Animator::new(pack, kh_anim::Settings::default()) else { return false };
        let Ok(m) = kh_partyhost::PosedMember::new(id, animator, &manifest, &entry) else { return false };
        core.poser.members.insert(id, m);
    }
    true
}

/// The leg IK's world = the harness's geometry (party/mod.rs `PartyIk` over the game's casts).
struct Ik<'a>(&'a TestWorld);
impl kh_anim::IkWorld for Ik<'_> {
    fn sweep_sphere(&self, start: Vec3, end: Vec3, radius: f32) -> Option<kh_anim::SweepHit> {
        use kh_core::World;
        let r = radius.max(0.1);
        let h = self.0.sweep_capsule(start, end, r, r)?;
        Some(kh_anim::SweepHit { time: h.time, impact_point: h.position, impact_normal: h.impact_normal, start_penetrating: h.start_penetrating })
    }
}

/// One step with this input (Run::step, the stick given).
fn step_with(r: &mut Run, input: Input) {
    let input = r.core.route(&mut r.sora, &input, STEP, &r.world, &r.fakes, r.t);
    r.out = {
        let mut targets = kh_partyhost::SoraTargets { targets: &mut r.fakes, runtime: &mut r.core.rt };
        let mut host = kh_core::Host { world: &r.world, targets: &mut targets, audio: &mut r.null.audio, effects: &mut r.null.effects, ui: &mut r.null.ui };
        r.sora.step_hosted(STEP, &input, &mut host)
    };
    let lead = r.core.rt.leader_view_of(&r.out, r.sora.status());
    let f = forward(lead.yaw);
    r.core.camera.position = lead.position - f * 450.0 + Vec3::Z * 220.0;
    r.core.camera.rotation = kh_camera::Rot { pitch: -15.0, yaw: lead.yaw, roll: 0.0 };
    r.core.step(&mut r.sora, &r.out, STEP, TargetId::NONE, &r.world, &Ik(&r.world), &mut r.fakes, &mut r.null.audio, &mut r.null.effects, None, r.t);
    r.t += STEP as f64;
}

type Drive = Box<dyn FnMut(f32, Vec3) -> Option<(f32, f32)>>;

pub struct Scenario {
    pub name: &'static str,
    world: fn() -> TestWorld,
    lead: Option<PartyChara>,
    drive: fn() -> Drive,
}

fn open() -> TestWorld {
    TestWorld::floor()
}

/// A slalom of walls: the leader runs round their ends, the friends by the trail.
fn walls() -> TestWorld {
    let mut w = TestWorld::floor();
    for k in 0..12 {
        let x = 800.0 + 800.0 * k as f32;
        let (y0, y1) = if k % 2 == 0 { (-1400.0, 300.0) } else { (-300.0, 1400.0) };
        w = w.with_box(Vec3::new(x - 30.0, y0, 0.0), Vec3::new(x + 30.0, y1, 400.0));
    }
    w
}

/// Hills of steps (rise 15 cm, run 60 cm, 6 up, 3 m top, 6 down) every 25 m along +X.
fn stairs() -> TestWorld {
    let mut w = TestWorld::floor();
    for h in 0..14 {
        let x0 = 600.0 + 2500.0 * h as f32;
        for s in 0..6 {
            let lo = x0 + 60.0 * s as f32;
            let hi = x0 + 60.0 * 12.0 + 300.0 - 60.0 * s as f32;
            w = w.with_box(Vec3::new(lo, -3000.0, 0.0), Vec3::new(hi, 3000.0, 15.0 * (s + 1) as f32));
        }
    }
    w
}

fn straight() -> Drive {
    Box::new(|_, _| Some((0.0, 1.0)))
}
fn curve() -> Drive {
    Box::new(|t, _| Some((20.0 * t, 1.0)))
}
fn s_curves() -> Drive {
    Box::new(|t, _| Some((55.0 * (std::f32::consts::TAU * t / 6.0).sin(), 1.0)))
}
fn walk_curves() -> Drive {
    Box::new(|t, _| Some((40.0 * (std::f32::consts::TAU * t / 8.0).sin(), 0.4)))
}
fn stop_start() -> Drive {
    Box::new(|t, _| if t % 4.5 < 3.0 { Some((90.0 * (t / 4.5).floor(), 1.0)) } else { None })
}
fn slalom() -> Drive {
    let mut k = 0usize;
    Box::new(move |_, p| {
        // the waypoints round the walls' open ends
        let pts: Vec<Vec3> = (0..24).map(|i| {
            let wall = i / 2;
            let x = 800.0 + 800.0 * wall as f32 + if i % 2 == 0 { -400.0 } else { 400.0 };
            let y = if wall % 2 == 0 { 700.0 } else { -700.0 };
            Vec3::new(x, y, 0.0)
        }).collect();
        while k < pts.len() && Vec3::new(pts[k].x - p.x, pts[k].y - p.y, 0.0).length() < 150.0 {
            k += 1;
        }
        let to = pts.get(k).copied().unwrap_or(Vec3::new(20000.0, 0.0, 0.0));
        Some(((to.y - p.y).atan2(to.x - p.x).to_degrees(), 1.0))
    })
}

pub fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario { name: "sprint straight", world: open, lead: None, drive: straight },
        Scenario { name: "sprint curve 20/s", world: open, lead: None, drive: curve },
        Scenario { name: "sprint S-curves", world: open, lead: None, drive: s_curves },
        Scenario { name: "walk curves (0.4)", world: open, lead: None, drive: walk_curves },
        Scenario { name: "stop / start", world: open, lead: None, drive: stop_start },
        Scenario { name: "walls (trail nav)", world: walls, lead: None, drive: slalom },
        Scenario { name: "stairs", world: stairs, lead: None, drive: straight },
        Scenario { name: "Donald leads S-curves", world: open, lead: Some(PartyChara::Donald), drive: s_curves },
        Scenario { name: "Goofy leads stop/start", world: open, lead: Some(PartyChara::Goofy), drive: stop_start },
    ]
}

/// One drawn friend (or AI Sora).
struct Track {
    name: &'static str,
    id: Option<u8>,
    live: Vec<DrawnSample>,
    new: Vec<DrawnSample>,
    sim: Vec<SimSample>,
}

fn feet_of(p: &kh_partyhost::PosedChar, feet: (usize, usize)) -> [Vec3; 2] {
    let w = |i: usize| p.component.get(i).map_or(p.component_to_world.translation, |b| b.then(&p.component_to_world).translation);
    [w(feet.0), w(feet.1)]
}

fn yaw_of_c2w(t: &kh_anim::Trs) -> f32 {
    let f = t.rotation * Vec3::Y;
    f.y.atan2(f.x).to_degrees()
}

pub struct RunOut {
    pub rows: Vec<(String, String, metrics::Report)>,
    pub gap_mean: Vec<(String, f32)>,
    pub stops: Vec<(String, u32)>,
}

/// Runs one scenario for `secs` of display time (after a 1 s settle) and returns its rows (member, rule, report).
pub fn run(s: &Scenario, clock: Clock, secs: f32, out_dir: Option<&std::path::Path>) -> Option<RunOut> {
    let mut r = Run::new((s.world)())?;
    if !load_posers(&mut r.core) {
        return None;
    }
    r.core.sink = None;
    r.frames(30, None);
    if let Some(c) = s.lead {
        r.core.asked_formation = Some(kh_host::formation::Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], c).unwrap());
    }
    r.frames(30, None);
    let mut drive = (s.drive)();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ s.name.len() as u64);
    let mut acc = 0.0f32;
    let mut t_disp = 0.0f64;
    let mut t_sim = 0.0f32;
    let mut out_before = r.out.clone();
    let foot = |r: &Run, id: u8| {
        let sk = r.core.poser.member(id).unwrap().animator.skeleton();
        (sk.index_of("L_ashi1").or(sk.index_of("L_ashi")).unwrap_or(0), sk.index_of("R_ashi1").or(sk.index_of("R_ashi")).unwrap_or(0))
    };
    let mut tracks: Vec<Track> = Vec::new();
    for c in PartyChara::FRIENDS {
        if Some(c) != s.lead {
            tracks.push(Track { name: c.name(), id: Some(kh_partyhost::poser::draw_id(c)), live: Vec::new(), new: Vec::new(), sim: Vec::new() });
        }
    }
    if s.lead.is_some() {
        tracks.push(Track { name: "AI Sora", id: None, live: Vec::new(), new: Vec::new(), sim: Vec::new() });
    }
    let mut gap = vec![0.0f32; tracks.len()];
    let mut gap_n = 0u32;
    let mut stops = vec![0u32; tracks.len()];
    let mut prev_state: Vec<NpcStateId> = vec![NpcStateId::Idle; tracks.len()];
    let mut csv = String::from("t,member,rule,x,y,z,yaw,lx,ly,lz,rx,ry,rz,leadx,leady,leadz\n");
    while t_disp < secs as f64 {
        let dt = frame_dt(clock, &mut rng);
        t_disp += dt as f64;
        acc = (acc + dt).min(0.25);
        r.frame(|_| {});
        while acc >= STEP {
            acc -= STEP;
            let lead_pos = r.core.rt.leader_view_of(&r.out, r.sora.status()).position;
            let stick = drive(t_sim, lead_pos);
            let input = Input { move_stick: stick.map_or(Vec2::ZERO, |s| Vec2::new(0.0, s.1)), camera_yaw: stick.map_or(r.out.yaw, |s| s.0), ..Input::default() };
            out_before = r.out.clone();
            step_with(&mut r, input);
            t_sim += STEP;
            let lead = r.core.rt.leader_view_of(&r.out, r.sora.status());
            let lead_moving = horizontal(lead.velocity).length() > 300.0;
            for (k, tr) in tracks.iter_mut().enumerate() {
                let (pos, yaw, clip_turn) = match tr.id {
                    Some(_) => {
                        let v = r.core.rt.view(PartyChara::FRIENDS.into_iter().find(|c| c.name() == tr.name).unwrap()).unwrap();
                        if prev_state[k] == NpcStateId::Move && v.state == NpcStateId::Idle && lead_moving {
                            stops[k] += 1;
                        }
                        prev_state[k] = v.state;
                        (v.position, v.yaw, v.anim.clip.starts_with("RunStart") || v.anim.clip.starts_with("Turn"))
                    }
                    None => (r.out.position, r.out.yaw, format!("{:?}", r.out.movement_state).contains("Start")),
                };
                if let (None, Ok(want)) = (tr.id, std::env::var("SM_TRACE")) {
                    if s.name.starts_with(want.as_str()) {
                        let prev = tr.sim.last().map_or((pos, yaw), |p| (p.pos, p.yaw));
                        eprintln!("TS {t_sim:6.3} AI Sora p ({:7.1},{:7.1}) v {:5.0} yaw {:6.1} dyaw {:6.1} st {:?} mv {:?} anim {:?} gap {:4.0}", pos.x, pos.y, horizontal(pos - prev.0).length() / STEP, yaw, wrap180(yaw - prev.1), r.out.state, r.out.movement_state, r.out.anim.name, horizontal(pos - lead.position).length());
                    }
                }
                if let (Some(_), Ok(want)) = (tr.id, std::env::var("SM_TRACE")) {
                    if s.name.starts_with(want.as_str()) {
                        let c = PartyChara::FRIENDS.into_iter().find(|c| c.name() == tr.name).unwrap();
                        let id = r.core.rt.id(c).unwrap();
                        let v = r.core.rt.view(c).unwrap();
                        let m = r.core.rt.party.member(id).unwrap();
                        let prev = tr.sim.last().map_or(pos, |p| p.pos);
                        eprintln!("TR {t_sim:6.3} {:<6} p ({:7.1},{:7.1},{:5.1}) v {:5.0} yaw {:6.1} hd {:6.1} st {:<10} clip {:<10} t {:4.2} sub {} fs {:4.0} mws {:4.0} cmd {:?} steer {:?} gap {:4.0} lead v {:4.0}", tr.name, pos.x, pos.y, pos.z, horizontal(pos - prev).length() / STEP, yaw, m.loco.heading, v.state_name, v.anim.clip, v.anim.time, m.loco.sub, m.loco.follow_speed, m.cmc.max_walk_speed, v.command.1, m.steer.map(|s| (s.1.x as i32, s.1.y as i32)), horizontal(pos - lead.position).length(), horizontal(lead.velocity).length());
                    }
                }
                tr.sim.push(SimSample { t: t_sim as f64, pos, yaw, leader: lead.position, clip_turn });
                gap[k] += horizontal(pos - lead.position).length();
            }
            gap_n += 1;
        }
        // the drawing (sora.rs submit / party_draw, cam.post: one alpha for Sora, the camera and the friends):
        // live = r.interp off (the default: alpha 1, the latest step); interp = r.interp 1 (the frame's true alpha)
        let true_alpha = (acc / STEP).clamp(0.0, 1.0);
        let leader_steps = match s.lead.map(kh_partyhost::poser::draw_id) {
            Some(id) => r.core.poser.member(id).unwrap().position,
            None => (out_before.position, r.out.position),
        };
        for tr in tracks.iter_mut() {
            let mut pair = Vec::new();
            for alpha in [1.0, true_alpha] {
                let leader = leader_steps.0.lerp(leader_steps.1, alpha);
                pair.push(match tr.id {
                    Some(id) => {
                        let f = foot(&r, id);
                        let p = r.core.poser.pose(id, alpha).unwrap();
                        DrawnSample { t: t_disp, pos: p.component_to_world.translation, yaw: yaw_of_c2w(&p.component_to_world), feet: Some(feet_of(&p, f)), leader }
                    }
                    // AI Sora: sora.rs submit (his lerp of out_before / out)
                    None => DrawnSample { t: t_disp, pos: out_before.position.lerp(r.out.position, alpha), yaw: out_before.yaw + wrap180(r.out.yaw - out_before.yaw) * alpha, feet: None, leader },
                });
            }
            let (live, new) = (pair[0], pair[1]);
            if out_dir.is_some() {
                for (rule, d) in [("live", &live), ("interp", &new)] {
                    let f = d.feet.unwrap_or([d.pos; 2]);
                    csv.push_str(&format!("{:.4},{},{rule},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}\n", d.t, tr.name, d.pos.x, d.pos.y, d.pos.z, d.yaw, f[0].x, f[0].y, f[0].z, f[1].x, f[1].y, f[1].z, d.leader.x, d.leader.y, d.leader.z));
                }
            }
            tr.live.push(live);
            tr.new.push(new);
        }
    }
    if let Some(dir) = out_dir {
        let _ = std::fs::create_dir_all(dir);
        let file = format!("{}_{}.csv", s.name.replace([' ', '/', '(', ')', '.'], "_"), match clock { Clock::Jitter => "jitter", Clock::Steady => "steady" });
        let _ = std::fs::write(dir.join(file), csv);
    }
    let mut rows = Vec::new();
    for tr in &tracks {
        rows.push((tr.name.to_string(), "live".to_string(), metrics::report(&tr.live, &tr.sim, STEP)));
        rows.push((tr.name.to_string(), "interp".to_string(), metrics::report(&tr.new, &tr.sim, STEP)));
    }
    let gap_mean = tracks.iter().enumerate().map(|(k, t)| (t.name.to_string(), gap[k] / gap_n.max(1) as f32)).collect();
    let stops = tracks.iter().enumerate().map(|(k, t)| (t.name.to_string(), stops[k])).collect();
    Some(RunOut { rows, gap_mean, stops })
}

fn horizontal(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, 0.0)
}

fn wrap180(a: f32) -> f32 {
    (a + 540.0).rem_euclid(360.0) - 180.0
}

/// The table (no assertions; `sm_friends_are_smooth` asserts).
#[test]
fn sm_measure_all() {
    let out = std::env::var_os("SM_OUT").map(std::path::PathBuf::from);
    eprintln!("SM ER | scenario | clock | member | rule | {} | gap mean | stops", metrics::HEADER);
    for s in scenarios() {
        for clock in [Clock::Jitter, Clock::Steady] {
            if matches!(clock, Clock::Steady) && !matches!(s.name, "sprint straight" | "sprint S-curves" | "walls (trail nav)") {
                continue;
            }
            let Some(o) = run(&s, clock, 30.0, out.as_deref()) else {
                eprintln!("SKIPPED: no AI pack / party pack");
                return;
            };
            for (m, rule, rep) in &o.rows {
                let g = o.gap_mean.iter().find(|x| &x.0 == m).map_or(0.0, |x| x.1);
                let st = o.stops.iter().find(|x| &x.0 == m).map_or(0, |x| x.1);
                eprintln!("SM ER | {:<22} | {:<6} | {:<7} | {:<4} | {} | {:5.0} | {}", s.name, format!("{clock:?}"), m, rule, metrics::line(rep), g, st);
            }
        }
    }
}
