//! (FOL2) HOW THE FRIENDS FOLLOW: THE FOLLOW METRICS HARNESS (test-only). S1's ER-shaped harness (glue/tests.rs `Run`: the
//! party core in sora.rs's order, the trail nav, the cast budget, the real AI pack and sheets) on scripted scenarios, read
//! per fixed step. The user's report (2026-10-20): after the leader stops they walk round him (good) but then dart quickly
//! between two points; in narrow hallways they bunch up although there is room; the follow should feel natural.
//!
//! Scenarios (each with the friends' posers, so the foot slide is the drawn feet's):
//! - `stop-and-wait` (open ground) / `stop-and-wait wall` (he stops 1.5 m from a wall) / `stop-and-wait corridor`: the
//!   leader runs 4 s then stands 20 s. Per friend over the 20 s: direction reversals (0.1 s velocity windows over 60 cm/s
//!   whose heading turns more than 120 degrees from the last such window within 0.8 s), speed spikes (the speed rising over
//!   350 cm/s from under 250: a run), short runs (a moving leg under 250 cm whose peak is over 350 cm/s), the legs, the
//!   time moving, the peak speed.
//! - `corridor 320` / `corridor 420` (a straight hallway that wide, the leader runs 6 s down it then stands 10 s): the member
//!   spacing (least distance friend-friend / friend-leader, the settled mean), the least wall clearance (capsule skin to
//!   wall) and whether the friends line up (their offsets along the hallway).
//! - `walk-run`: the stick 0.4 (walk) / 1.0 (run) every 4 s for 24 s on open ground: the friends' speed against the
//!   leader's in each phase (settled part), steps where a friend runs while the leader walks (gap under 450 cm), speed
//!   spikes, the acceleration's 95th percentile, the foot slide.
//! `cargo test -p kh-er --lib --release fol2_ -- --nocapture --test-threads=1`; `FOL2_TRACE=<scenario prefix>` prints a
//! line per 6 steps; `FOL2_OUT=<dir>` writes one CSV per scenario (the plots' input).

use glam::{Vec2, Vec3};
use kh_core::test_world::TestWorld;
use kh_core::{Input, TargetId};
use kh_host::party::PartyChara;

use super::*;

#[path = "../../../../kh-partyhost/src/sm_metrics.rs"]
mod metrics;
use metrics::{DrawnSample, SimSample};

fn load_posers(core: &mut PartyCore) -> bool {
    let pack = project("build/sora_pack/party");
    let read = |p: &std::path::Path| std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok());
    let index = read(&pack.join("party.json")).unwrap_or(serde_json::Value::Null);
    for id in [kh_partyhost::poser::DONALD, kh_partyhost::poser::GOOFY] {
        let dir = pack.join(kh_partyhost::poser::npc_of(id).unwrap());
        let Some(manifest) = read(&dir.join("manifest.json")) else { return false };
        let entry = index["members"].as_array().into_iter().flatten().find(|m| m["member"].as_u64() == Some(id as u64)).cloned().unwrap_or(serde_json::Value::Null);
        let Ok(pack) = kh_anim::Pack::load(&dir) else { return false };
        let Ok(animator) = kh_anim::Animator::new(pack, kh_anim::Settings::default()) else { return false };
        let Ok(m) = kh_partyhost::PosedMember::new(id, animator, &manifest, &entry) else { return false };
        core.poser.members.insert(id, m);
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

/// One step with the stick (yaw, magnitude) or none.
fn step_with(r: &mut Run, stick: Option<(f32, f32)>) {
    let input = Input { move_stick: stick.map_or(Vec2::ZERO, |s| Vec2::new(0.0, s.1)), camera_yaw: stick.map_or(r.out.yaw, |s| s.0), ..Input::default() };
    r.frame(|_| {});
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

/// A hallway along +X from x0 to x1, `width` cm between the wall faces (centred on y = 0), 4 m high.
fn hallway(w: TestWorld, x0: f32, x1: f32, width: f32) -> TestWorld {
    let h = width * 0.5;
    w.with_box(Vec3::new(x0, h, 0.0), Vec3::new(x1, h + 60.0, 400.0)).with_box(Vec3::new(x0, -h - 60.0, 0.0), Vec3::new(x1, -h, 400.0))
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Wait,
    Corridor,
    WalkRun,
    /// (FOL2 r2) the friends put 18 m behind a running leader at 1 s (behind the camera): KH3's 2.5x catch-up and back into view
    CatchUp,
}

pub struct Scenario {
    pub name: &'static str,
    kind: Kind,
    world: fn() -> TestWorld,
    /// The hallway's half width (wall clearance), 0 = none.
    half: f32,
    drive: fn(f32) -> Option<(f32, f32)>,
    secs: f32,
    /// The measured part starts here (s of the drive).
    from: f32,
}

pub fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario { name: "stop-and-wait", kind: Kind::Wait, world: TestWorld::floor, half: 0.0, drive: |t| if t < 4.0 { Some((0.0, 1.0)) } else { None }, secs: 64.0, from: 4.0 },
        // he runs 4 s (about 27 m) and stops 1.5 m short of a wall across his way
        Scenario { name: "stop-and-wait wall", kind: Kind::Wait, world: || TestWorld::floor().with_box(Vec3::new(2860.0, -1500.0, 0.0), Vec3::new(2920.0, 1500.0, 400.0)), half: 0.0, drive: |t| if t < 4.0 { Some((0.0, 1.0)) } else { None }, secs: 64.0, from: 4.0 },
        Scenario { name: "stop-and-wait corridor", kind: Kind::Wait, world: || hallway(TestWorld::floor(), 600.0, 9000.0, 360.0), half: 180.0, drive: |t| if t < 4.0 { Some((0.0, 1.0)) } else { None }, secs: 64.0, from: 4.0 },
        Scenario { name: "corridor 320", kind: Kind::Corridor, world: || hallway(TestWorld::floor(), 600.0, 9000.0, 320.0), half: 160.0, drive: |t| if t < 6.0 { Some((0.0, 1.0)) } else { None }, secs: 16.0, from: 1.5 },
        Scenario { name: "corridor 420", kind: Kind::Corridor, world: || hallway(TestWorld::floor(), 600.0, 9000.0, 420.0), half: 210.0, drive: |t| if t < 6.0 { Some((0.0, 1.0)) } else { None }, secs: 16.0, from: 1.5 },
        Scenario { name: "walk-run", kind: Kind::WalkRun, world: TestWorld::floor, half: 0.0, drive: |t| Some((0.0, if (t / 4.0) as i32 % 2 == 0 { 0.4 } else { 1.0 })), secs: 24.0, from: 0.0 },
        Scenario { name: "catch-up", kind: Kind::CatchUp, world: TestWorld::floor, half: 0.0, drive: |_| Some((0.0, 1.0)), secs: 14.0, from: 1.25 },
    ]
}

/// One friend's numbers.
#[derive(Clone, Debug, Default)]
pub struct Follow {
    pub name: String,
    pub reversals: u32,
    pub spikes: u32,
    pub short_runs: u32,
    pub legs: u32,
    pub moving: f32,
    pub peak: f32,
    /// Corridor: least / settled-mean distance to the other friend, least distance to the leader, least wall clearance.
    pub min_mate: f32,
    pub mean_mate: f32,
    pub min_lead: f32,
    pub min_wall: f32,
    /// Corridor (settled): the other friend's offset along the hallway / across it (mean |dx|, |dy|).
    pub along: f32,
    pub across: f32,
    /// Walk-run: friend speed / leader speed while he walks / runs (settled), steps running while he walks.
    pub walk_ratio: f32,
    pub run_ratio: f32,
    pub run_in_walk: u32,
    pub accel_p95: f32,
    pub slide: f32,
    pub sim_jerk: f32,
    pub yaw_snaps: u32,
    /// (FOL2 r2) after the leader stops: how far past his final place a friend went along the leader's way, and how far he
    /// walked back to it (cm); the peak acceleration (cm/s2, the 99th percentile and the largest).
    pub over: f32,
    pub back: f32,
    pub acc_p99: f32,
    pub acc_max: f32,
}

pub const HEADER: &str = "rev | spikes | shortrun | legs | moving | peak | mate min/mean | lead min | wall min | along/across | walk/run ratio | run-in-walk | acc p95 | slide | jerk | yawsnap | over/back | acc p99/max";

pub fn line(f: &Follow) -> String {
    format!("{:3} | {:3} | {:3} | {:3} | {:4.0}% | {:4.0} | {:4.0}/{:4.0} | {:4.0} | {:5.1} | {:4.0}/{:4.0} | {:4.2}/{:4.2} | {:4} | {:5.0} | {:4.1}% | {:5.0} | {:2} | {:4.0}/{:4.0} | {:5.0}/{:6.0}", f.reversals, f.spikes, f.short_runs, f.legs, 100.0 * f.moving, f.peak, f.min_mate, f.mean_mate, f.min_lead, f.min_wall, f.along, f.across, f.walk_ratio, f.run_ratio, f.run_in_walk, f.accel_p95, f.slide, f.sim_jerk, f.yaw_snaps, f.over, f.back, f.acc_p99, f.acc_max)
}

struct Track {
    chara: PartyChara,
    pos: Vec<Vec3>,
    radius: f32,
    drawn: Vec<DrawnSample>,
    sim: Vec<SimSample>,
}

fn h(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, 0.0)
}

fn pct(v: &mut [f32], p: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f32 * p).round() as usize]
}

/// The motion numbers of a track over steps [a, b): reversals, spikes, short runs, legs, moving fraction, peak.
fn motion(pos: &[Vec3], a: usize, b: usize, f: &mut Follow) {
    let speed = |k: usize| if k == 0 { 0.0 } else { h(pos[k] - pos[k - 1]).length() / STEP };
    // 0.1 s windows
    let mut last: Option<(usize, Vec3)> = None;
    let mut k = a + 6;
    while k < b {
        let d = h(pos[k] - pos[k - 6]);
        let v = d.length() / (6.0 * STEP);
        if v > 60.0 {
            let dir = d / d.length();
            if let Some((at, pd)) = last {
                if (k - at) as f32 * STEP <= 0.8 && dir.dot(pd) < (120.0f32).to_radians().cos() {
                    f.reversals += 1;
                }
            }
            last = Some((k, dir));
        }
        k += 6;
    }
    // spikes, legs (moving over 40 cm/s for 3 steps; ends under 20 for 6 steps)
    let (mut high, mut moving_steps) = (false, 0u32);
    let (mut in_leg, mut leg_len, mut leg_peak, mut above, mut below) = (false, 0.0f32, 0.0f32, 0u32, 0u32);
    for k in a.max(1)..b {
        let v = speed(k);
        f.peak = f.peak.max(v);
        if !high && v > 350.0 {
            high = true;
            f.spikes += 1;
        } else if high && v < 250.0 {
            high = false;
        }
        if v > 40.0 {
            moving_steps += 1;
        }
        if !in_leg {
            above = if v > 40.0 { above + 1 } else { 0 };
            if above >= 3 {
                in_leg = true;
                leg_len = 0.0;
                leg_peak = 0.0;
                below = 0;
            }
        }
        if in_leg {
            leg_len += v * STEP;
            leg_peak = leg_peak.max(v);
            below = if v < 20.0 { below + 1 } else { 0 };
            if below >= 6 || k + 1 == b {
                in_leg = false;
                f.legs += 1;
                if leg_len < 250.0 && leg_peak > 350.0 {
                    f.short_runs += 1;
                }
            }
        }
    }
    f.moving = moving_steps as f32 / (b - a).max(1) as f32;
}

pub fn run(s: &Scenario) -> Option<Vec<Follow>> {
    // FOL2=off: the follow as before FOL2 (the switch off), else on
    let on = std::env::var("FOL2").map_or(true, |v| v != "off");
    kh_party::locomotion::FOL2_ON.store(on, std::sync::atomic::Ordering::Relaxed);
    kh_ai::formation::ON.store(on, std::sync::atomic::Ordering::Relaxed);
    let mut r = Run::new((s.world)())?;
    if !load_posers(&mut r.core) {
        return None;
    }
    r.core.sink = None;
    for _ in 0..60 {
        step_with(&mut r, None);
    }
    let trace = std::env::var("FOL2_TRACE").ok().filter(|w| s.name.starts_with(w.as_str()));
    let mut tracks: Vec<Track> = PartyChara::FRIENDS.into_iter().map(|c| Track { chara: c, pos: Vec::new(), radius: 40.0, drawn: Vec::new(), sim: Vec::new() }).collect();
    let mut lead_pos: Vec<Vec3> = Vec::new();
    let mut stick_mag: Vec<f32> = Vec::new();
    let foot = |r: &Run, id: u8| {
        let sk = r.core.poser.member(id).unwrap().animator.skeleton();
        (sk.index_of("L_ashi1").or(sk.index_of("L_ashi")).unwrap_or(0), sk.index_of("R_ashi1").or(sk.index_of("R_ashi")).unwrap_or(0))
    };
    let mut csv = String::from("t,member,x,y,yaw,state,lx,ly\n");
    let n = (s.secs / STEP) as usize;
    for k in 0..n {
        let t = k as f32 * STEP;
        let stick = (s.drive)(t);
        if s.kind == Kind::CatchUp && k == (1.0 / STEP) as usize {
            let lead = r.core.rt.leader_view_of(&r.out, r.sora.status()).position;
            r.put(PartyChara::Donald, Vec3::new(lead.x - 1800.0, 150.0, 0.0));
            r.put(PartyChara::Goofy, Vec3::new(lead.x - 1900.0, -150.0, 0.0));
        }
        step_with(&mut r, stick);
        let lead = r.core.rt.leader_view_of(&r.out, r.sora.status());
        lead_pos.push(lead.position);
        stick_mag.push(stick.map_or(0.0, |x| x.1));
        for tr in tracks.iter_mut() {
            let v = r.core.rt.view(tr.chara).unwrap();
            tr.radius = v.capsule.0;
            tr.pos.push(v.position);
            tr.sim.push(SimSample { t: (t + STEP) as f64, pos: v.position, yaw: v.yaw, leader: lead.position, clip_turn: v.anim.clip.starts_with("RunStart") || v.anim.clip.starts_with("Turn") });
            let id = kh_partyhost::poser::draw_id(tr.chara);
            let fb = foot(&r, id);
            if let Some(p) = r.core.poser.pose(id, 1.0) {
                let at = |i: usize| p.component.get(i).map_or(p.component_to_world.translation, |b| b.then(&p.component_to_world).translation);
                tr.drawn.push(DrawnSample { t: (t + STEP) as f64, pos: p.component_to_world.translation, yaw: { let f = p.component_to_world.rotation * Vec3::Y; f.y.atan2(f.x).to_degrees() }, feet: Some([at(fb.0), at(fb.1)]), leader: lead.position });
            }
            csv.push_str(&format!("{:.4},{},{:.1},{:.1},{:.1},{},{:.1},{:.1}\n", t, tr.chara.name(), v.position.x, v.position.y, v.yaw, v.state_name, lead.position.x, lead.position.y));
            if trace.is_some() && (k % 6 == 0 || std::env::var_os("FOL2_TRACE_ALL").is_some()) {
                let mid = r.core.rt.id(tr.chara).unwrap();
                let m = r.core.rt.party.member(mid).unwrap();
                let ai = r.core.rt.ai.iter().find(|(c, _)| *c == tr.chara).map(|(_, a)| a);
                let dest = ai.and_then(|a| match a.blackboard().get_by_name("Destination") { Some(kh_ai::bb::BbValue::Vector(d)) => Some(*d), _ => None });
                let adest = ai.and_then(|a| match a.blackboard().get_by_name("ActionDestination") { Some(kh_ai::bb::BbValue::Vector(d)) => Some(*d), _ => None });
                let task = ai.map(|a| a.trace().task).unwrap_or_default();
                let prev = if tr.pos.len() > 6 { tr.pos[tr.pos.len() - 7] } else { v.position };
                let fmt = |d: Option<Vec3>| d.filter(|d| d.x.abs() < 1e6).map_or("-".to_string(), |d| format!("({:6.0},{:6.0})", d.x, d.y));
                eprintln!("FT {t:6.2} {:<6} p ({:6.0},{:6.0}) v {:4.0} st {:<8} clip {:<12} mws {:4.0} task {:<28} dest {} adest {} steer {:?} lead ({:6.0},{:6.0}) sub {} dv {:4.0} hold {:3.1} brk {:4.0} live {} side {:3.1} blk {:3.1}", tr.chara.name(), v.position.x, v.position.y, h(v.position - prev).length() / (6.0 * STEP), v.state_name, v.anim.clip, m.cmc.max_walk_speed, task, fmt(dest), fmt(adest), m.steer.map(|s| (s.1.x as i32, s.1.y as i32)), lead.position.x, lead.position.y, m.loco.sub, m.loco.decel_v, m.loco.hold, m.loco.fol_brake, m.loco.fol_rel.is_some(), m.loco.side, m.loco.blocked);
            }
        }
    }
    if let Some(dir) = std::env::var_os("FOL2_OUT") {
        let dir = std::path::PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(format!("{}.csv", s.name.replace([' ', '/', '(', ')', '.'], "_"))), csv);
    }
    let a = (s.from / STEP) as usize;
    let b = n;
    let mut out = Vec::new();
    for (i, tr) in tracks.iter().enumerate() {
        let mut f = Follow { name: tr.chara.name().to_string(), min_mate: f32::MAX, min_lead: f32::MAX, min_wall: f32::MAX, ..Default::default() };
        motion(&tr.pos, a, b, &mut f);
        let mate = &tracks[1 - i];
        let settle = ((s.secs - 5.0) / STEP) as usize;
        let (mut sm, mut sa, mut sc, mut ns) = (0.0, 0.0, 0.0, 0u32);
        for k in a..b {
            let dm = h(tr.pos[k] - mate.pos[k]).length() - tr.radius - mate.radius;
            f.min_mate = f.min_mate.min(dm);
            f.min_lead = f.min_lead.min(h(tr.pos[k] - lead_pos[k]).length() - tr.radius - 35.0);
            if s.half > 0.0 && tr.pos[k].x > 700.0 {
                f.min_wall = f.min_wall.min(s.half - tr.pos[k].y.abs() - tr.radius);
            }
            if k >= settle {
                sm += dm;
                sa += (tr.pos[k].x - mate.pos[k].x).abs();
                sc += (tr.pos[k].y - mate.pos[k].y).abs();
                ns += 1;
            }
        }
        f.mean_mate = sm / ns.max(1) as f32;
        f.along = sa / ns.max(1) as f32;
        f.across = sc / ns.max(1) as f32;
        if f.min_wall == f32::MAX {
            f.min_wall = 0.0;
        }
        if s.kind == Kind::WalkRun {
            let (mut wf, mut wl, mut rf, mut rl) = (0.0, 0.0, 0.0, 0.0);
            for k in a.max(1)..b {
                let t = k as f32 * STEP;
                let into = t % 4.0;
                let vf = h(tr.pos[k] - tr.pos[k - 1]).length() / STEP;
                let vl = h(lead_pos[k] - lead_pos[k - 1]).length() / STEP;
                if t >= 4.0 && into >= 1.5 {
                    if stick_mag[k] < 0.5 {
                        wf += vf;
                        wl += vl;
                    } else {
                        rf += vf;
                        rl += vl;
                    }
                }
                if stick_mag[k] < 0.5 && t >= 4.0 && into >= 1.0 && vf > (vl * 1.4).max(vl + 120.0) && h(tr.pos[k] - lead_pos[k]).length() < 450.0 {
                    f.run_in_walk += 1;
                }
            }
            f.walk_ratio = wf / f32::max(wl, 1.0);
            f.run_ratio = rf / f32::max(rl, 1.0);
        }
        let mut acc: Vec<f32> = (a.max(2)..b).map(|k| (h(tr.pos[k] - tr.pos[k - 1]) - h(tr.pos[k - 1] - tr.pos[k - 2])).length() / (STEP * STEP)).collect();
        f.accel_p95 = pct(&mut acc, 0.95);
        f.acc_p99 = pct(&mut acc, 0.99);
        f.acc_max = acc.iter().cloned().fold(0.0, f32::max);
        // (FOL2 r2) the overshoot after the leader's stop (he runs +X): past the place he is at 2 s later, and the walk back to it
        // (2 s: before KH3's walk round the leader can begin)
        if let Some(st) = (1..n).find(|&k| stick_mag[k] == 0.0 && stick_mag[k - 1] > 0.0) {
            let end = (st + (2.0 / STEP) as usize).min(n - 1);
            let fin = tr.pos[end].x;
            let (mut far, mut far_k) = (fin, st);
            for k in st..=end {
                if tr.pos[k].x > far {
                    far = tr.pos[k].x;
                    far_k = k;
                }
            }
            f.over = far - fin;
            f.back = if f.over < 5.0 { 0.0 } else { (far_k + 1..=end).map(|k| h(tr.pos[k] - tr.pos[k - 1]).length()).sum::<f32>() };
        }
        let rep = metrics::report(&tr.drawn[a..], &tr.sim[a..], STEP);
        f.slide = rep.slide_median;
        f.sim_jerk = rep.sim_jerk_rms;
        f.yaw_snaps = rep.sim_snaps;
        out.push(f);
    }
    Some(out)
}

/// The table (no assertions; `fol2_follow_is_natural` asserts the acceptance bar).
#[test]
fn fol2_measure_all() {
    eprintln!("FOL2 | scenario | member | {HEADER}");
    for s in scenarios() {
        let Some(rows) = run(&s) else {
            eprintln!("SKIPPED: no AI pack / party pack");
            return;
        };
        for f in &rows {
            eprintln!("FOL2 | {:<22} | {:<6} | {}", s.name, f.name, line(f));
        }
    }
    kh_party::locomotion::FOL2_ON.store(true, std::sync::atomic::Ordering::Relaxed); // back to the default
    kh_ai::formation::ON.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// (FOL2) SM's smoothness table (sm_tests.rs) with the switch as FOL2 says (FOL2=off: before): no regression of the run.
#[test]
fn fol2_sm_regression() {
    let on = std::env::var("FOL2").map_or(true, |v| v != "off");
    kh_party::locomotion::FOL2_ON.store(on, std::sync::atomic::Ordering::Relaxed);
    kh_ai::formation::ON.store(on, std::sync::atomic::Ordering::Relaxed);
    eprintln!("FOL2 SM | scenario | member | rule | {} | gap mean | stops", super::sm::metrics::HEADER);
    for s in super::sm::scenarios() {
        let Some(o) = super::sm::run(&s, super::sm::Clock::Jitter, 30.0, None) else {
            eprintln!("SKIPPED");
            break;
        };
        for (m, rule, rep) in &o.rows {
            if rule != "interp" {
                continue;
            }
            let g = o.gap_mean.iter().find(|x| &x.0 == m).map_or(0.0, |x| x.1);
            let st = o.stops.iter().find(|x| &x.0 == m).map_or(0, |x| x.1);
            eprintln!("FOL2 SM | {:<22} | {:<7} | {} | {:5.0} | {}", s.name, m, super::sm::metrics::line(rep), g, st);
        }
    }
    kh_party::locomotion::FOL2_ON.store(true, std::sync::atomic::Ordering::Relaxed); // back to the default
    kh_ai::formation::ON.store(true, std::sync::atomic::Ordering::Relaxed);
}
